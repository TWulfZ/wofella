//! Background jobs (architecture §7, spec 003 T14): tokio orchestrates, rayon runs CPU work,
//! one job at a time in F0, every item isolated by `catch_unwind`.

pub mod dto;
pub mod progress;
mod runner;

use std::future::Future;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use rayon::prelude::*;
use tokio_util::sync::CancellationToken;
use wolluf_core::{Clock, ErrorCode};
use wolluf_store::repo::cache::{ItemFailure, item_failure};
use wolluf_store::{DbHandle, Vault};

use crate::errors::AppError;

pub use dto::{JobId, JobKindDto, JobStageDto, JobStatusDto};
pub use progress::ProgressSink;
pub use runner::JobRunner;

pub type JobFuture = Pin<Box<dyn Future<Output = Result<JobSummary, AppError>> + Send>>;

pub trait Job: Send + 'static {
    fn kind(&self) -> JobKindDto;
    /// Jobs with equal keys coalesce (spec 003, "Job runner").
    fn dedupe_key(&self) -> String;
    /// Persisted in `job_run.params_json` for diagnostics.
    fn params(&self) -> serde_json::Value;
    fn run(self: Box<Self>, ctx: JobCtx) -> JobFuture;
}

/// What a finished job hands back to the runner.
#[derive(Default)]
pub struct JobSummary {
    pub summary: Option<dto::JobSummaryDto>,
    /// `DataChanged` domains, emitted only when non-empty (spec 003 step 4).
    pub changed: Vec<&'static str>,
    /// Enqueued through the same coalescing after `JobFinished` (004 chains `RefreshIdentity`).
    pub follow_ups: Vec<Box<dyn Job>>,
}

impl std::fmt::Debug for JobSummary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JobSummary")
            .field("summary", &self.summary)
            .field("changed", &self.changed)
            .field("follow_ups", &self.follow_ups.len())
            .finish()
    }
}

/// A per-item failure; recorded as `item_failure` and the job goes on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemError {
    pub code: ErrorCode,
    pub message: String,
}

impl ItemError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

impl From<AppError> for ItemError {
    fn from(e: AppError) -> Self {
        Self::new(
            e.code,
            e.details.unwrap_or_else(|| e.message_key.into_owned()),
        )
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ItemResult<R> {
    Done(R),
    Failed(ItemError),
    /// Not started because the job was cancelled.
    Skipped,
}

/// Everything a job may touch. Clones share the cancellation token, progress throttle and
/// failure counter.
#[derive(Clone)]
pub struct JobCtx {
    pub job_id: JobId,
    pub cancel: CancellationToken,
    pub progress: ProgressSink,
    pub user: DbHandle,
    pub cache: DbHandle,
    pub vault: Vault,
    pub cpu: Arc<rayon::ThreadPool>,
    pub clock: Arc<dyn Clock>,
    failed: Arc<AtomicU32>,
}

impl JobCtx {
    pub fn is_cancelled(&self) -> bool {
        self.cancel.is_cancelled()
    }

    pub fn check_cancelled(&self) -> Result<(), AppError> {
        if self.is_cancelled() {
            return Err(AppError::cancelled());
        }
        Ok(())
    }

    pub fn failed_items(&self) -> u32 {
        self.failed.load(Ordering::SeqCst)
    }

    /// Blocking (cache.db write). The first failure per `(job, item_ref)` is kept; repeats are
    /// not counted twice.
    pub fn record_failure(&self, item_ref: &str, error: &ItemError) -> Result<(), AppError> {
        let row = ItemFailure {
            job_id: self.ulid()?,
            item_ref: item_ref.to_owned(),
            code: error.code,
            message: error.message.clone(),
        };
        tracing::warn!(job = %self.job_id, item = item_ref, code = %error.code, "item failed");
        if self.cache.write(move |tx| item_failure::insert(tx, &row))? {
            self.failed.fetch_add(1, Ordering::SeqCst);
        }
        Ok(())
    }

    fn ulid(&self) -> Result<ulid::Ulid, AppError> {
        ulid::Ulid::from_string(&self.job_id.0)
            .map_err(|e| AppError::internal(format!("job id {}: {e}", self.job_id)))
    }

    /// Blocking; call from `spawn_blocking`. Fans `items` out to the CPU pool, checks
    /// cancellation before each item, turns a panic into an `INTERNAL` item failure and records
    /// every failure. Results keep the input order.
    pub fn run_items<T, R>(
        &self,
        stage: JobStageDto,
        items: &[T],
        item_ref: impl Fn(&T) -> String + Sync,
        f: impl Fn(&T) -> Result<R, ItemError> + Sync,
    ) -> Result<Vec<ItemResult<R>>, AppError>
    where
        T: Sync,
        R: Send,
    {
        let total = u32::try_from(items.len()).unwrap_or(u32::MAX);
        let done = AtomicU32::new(0);
        self.progress.report(stage, 0, total);
        let results: Vec<ItemResult<R>> = self.cpu.install(|| {
            items
                .par_iter()
                .map(|item| {
                    if self.is_cancelled() {
                        return ItemResult::Skipped;
                    }
                    let outcome = match catch_unwind(AssertUnwindSafe(|| f(item))) {
                        Ok(Ok(value)) => ItemResult::Done(value),
                        Ok(Err(e)) => ItemResult::Failed(e),
                        Err(payload) => ItemResult::Failed(ItemError::new(
                            ErrorCode::Internal,
                            panic_text(&*payload),
                        )),
                    };
                    let n = done.fetch_add(1, Ordering::SeqCst) + 1;
                    self.progress.report(stage, n, total);
                    outcome
                })
                .collect()
        });
        self.progress.flush();
        for (item, result) in items.iter().zip(&results) {
            if let ItemResult::Failed(e) = result {
                self.record_failure(&item_ref(item), e)?;
            }
        }
        Ok(results)
    }
}

pub(crate) fn panic_text(payload: &(dyn std::any::Any + Send)) -> String {
    payload
        .downcast_ref::<&str>()
        .map(|s| (*s).to_owned())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "non-string panic payload".to_owned())
}
