//! FIFO runner with coalescing, a trailing re-run and follow-ups (spec 003 "Job runner").
//!
//! Coalescing is one rule: a submit whose dedupe key is already *queued* returns that job's id.
//! A submit while the same key is *running* therefore queues exactly one re-run, and every
//! further submit lands on it, so an event arriving mid-sync is never lost and never stacks.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex, PoisonError, Weak};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tokio::runtime::Handle;
use tokio::sync::{Notify, broadcast};
use tokio_util::sync::CancellationToken;
use tracing::Instrument;
use wolluf_core::{Clock, ErrorCode, UnixUs};
use wolluf_store::repo::cache::{JobRun, JobStatus, NewJobRun, job_run};
use wolluf_store::time::format_rfc3339_ms;
use wolluf_store::{DbHandle, Vault};

use crate::context::blocking_join_error;
use crate::errors::{AppError, ErrorCodeDto};
use crate::events::{AppEvent, JobFinishedDto};
use crate::jobs::dto::{JobDto, JobErrorDto, JobId, JobKindDto, JobStatusDto, JobSummaryDto};
use crate::jobs::{Job, JobCtx, ProgressSink};

/// Enough history for the job tray (20 finished) and `wolluf jobs list`.
const DEFAULT_LIST_LIMIT: u32 = 50;

struct Queued {
    id: JobId,
    key: String,
    kind: JobKindDto,
    job: Box<dyn Job>,
}

struct Running {
    id: JobId,
    cancel: CancellationToken,
}

#[derive(Default)]
struct State {
    queue: VecDeque<Queued>,
    running: Option<Running>,
    ids: ulid::Generator,
}

#[derive(Clone)]
struct Deps {
    user: DbHandle,
    cache: DbHandle,
    vault: Vault,
    cpu: Arc<rayon::ThreadPool>,
    clock: Arc<dyn Clock>,
    events: broadcast::Sender<AppEvent>,
}

struct Inner {
    state: Mutex<State>,
    wake: Notify,
    shutdown: CancellationToken,
    deps: Deps,
}

/// `summary_json` holds either the job's summary or the error that ended it.
#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum Stored {
    Summary(JobSummaryDto),
    Error { error: JobErrorDto },
}

/// Dropping the runner (with its `AppContext`) cancels the running job and stops the worker.
pub struct JobRunner {
    inner: Arc<Inner>,
}

/// Submits from background threads (the watcher) without keeping the runner alive: once the
/// context is gone, submits are dropped.
#[derive(Clone)]
pub struct JobSubmitter {
    inner: Weak<Inner>,
}

impl JobSubmitter {
    /// `None` when the runner has shut down.
    pub fn submit(&self, job: Box<dyn Job>) -> Option<JobId> {
        let inner = self.inner.upgrade()?;
        if inner.shutdown.is_cancelled() {
            return None;
        }
        Some(submit(&inner, job))
    }
}

impl Drop for JobRunner {
    fn drop(&mut self) {
        self.inner.shutdown.cancel();
        if let Some(running) = &self.inner.lock().running {
            running.cancel.cancel();
        }
    }
}

impl Inner {
    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

fn to_system_time(t: UnixUs) -> SystemTime {
    match u64::try_from(t.0) {
        Ok(us) => UNIX_EPOCH + Duration::from_micros(us),
        Err(_) => UNIX_EPOCH,
    }
}

fn status_dto(status: JobStatus) -> JobStatusDto {
    match status {
        JobStatus::Queued => JobStatusDto::Queued,
        JobStatus::Running => JobStatusDto::Running,
        JobStatus::Ok => JobStatusDto::Ok,
        JobStatus::Failed => JobStatusDto::Failed,
        JobStatus::Cancelled => JobStatusDto::Cancelled,
    }
}

fn job_dto(run: JobRun) -> Option<JobDto> {
    let kind = JobKindDto::parse(&run.kind)?;
    let (summary, error) = match run.summary.map(serde_json::from_value::<Stored>) {
        Some(Ok(Stored::Summary(s))) => (Some(s), None),
        Some(Ok(Stored::Error { error })) => (None, Some(error)),
        Some(Err(_)) | None => (None, None),
    };
    Some(JobDto {
        id: JobId(run.id.to_string()),
        kind,
        status: status_dto(run.status),
        started: run.started.map(format_rfc3339_ms),
        ended: run.ended.map(format_rfc3339_ms),
        summary,
        error,
    })
}

impl JobRunner {
    pub(crate) fn start(
        handle: &Handle,
        user: DbHandle,
        cache: DbHandle,
        vault: Vault,
        cpu: Arc<rayon::ThreadPool>,
        clock: Arc<dyn Clock>,
        events: broadcast::Sender<AppEvent>,
    ) -> Self {
        let inner = Arc::new(Inner {
            state: Mutex::new(State::default()),
            wake: Notify::new(),
            shutdown: CancellationToken::new(),
            deps: Deps {
                user,
                cache,
                vault,
                cpu,
                clock,
                events,
            },
        });
        handle.spawn(worker(inner.clone()));
        Self { inner }
    }

    /// Never blocks: the job is queued in memory and its `job_run` row is written when it
    /// starts.
    pub fn submit(&self, job: Box<dyn Job>) -> JobId {
        submit(&self.inner, job)
    }

    pub fn submitter(&self) -> JobSubmitter {
        JobSubmitter {
            inner: Arc::downgrade(&self.inner),
        }
    }

    /// A queued job is dropped and recorded as cancelled; a running one has its token tripped
    /// and ends `cancelled` at its next item check.
    pub async fn cancel(&self, id: &JobId) -> Result<(), AppError> {
        let removed = {
            let mut state = self.inner.lock();
            if let Some(running) = state.running.as_ref().filter(|r| &r.id == id) {
                running.cancel.cancel();
                return Ok(());
            }
            let pos = state.queue.iter().position(|q| &q.id == id);
            pos.and_then(|p| state.queue.remove(p))
        };
        let Some(queued) = removed else {
            return Err(AppError::not_found().with_arg("jobId", id.0.clone()));
        };
        let deps = self.inner.deps.clone();
        let now = deps.clock.now();
        let error = AppError::cancelled();
        let row = new_run(&queued, JobStatus::Cancelled, None)?;
        let stored = stored_error(&error)?;
        let ulid = row.id;
        spawn_blocking(move || {
            deps.cache.write(move |tx| {
                job_run::insert(tx, &row)?;
                job_run::finish(tx, ulid, JobStatus::Cancelled, now, &stored)
            })
        })
        .await?;
        let _ = self
            .inner
            .deps
            .events
            .send(AppEvent::JobFinished(JobFinishedDto {
                job_id: queued.id,
                status: JobStatusDto::Cancelled,
                failed_items: 0,
            }));
        Ok(())
    }

    /// Queued jobs first (in queue order), then history newest first.
    pub async fn list(&self, limit: Option<u32>) -> Result<Vec<JobDto>, AppError> {
        let queued: Vec<JobDto> = self
            .inner
            .lock()
            .queue
            .iter()
            .map(|q| JobDto {
                id: q.id.clone(),
                kind: q.kind,
                status: JobStatusDto::Queued,
                started: None,
                ended: None,
                summary: None,
                error: None,
            })
            .collect();
        let cache = self.inner.deps.cache.clone();
        let limit = limit.unwrap_or(DEFAULT_LIST_LIMIT);
        let runs =
            spawn_blocking(move || cache.read(|conn| job_run::list_recent(conn, limit))).await?;
        Ok(queued
            .into_iter()
            .chain(runs.into_iter().filter_map(job_dto))
            .collect())
    }
}

async fn spawn_blocking<R: Send + 'static>(
    f: impl FnOnce() -> Result<R, wolluf_store::StoreError> + Send + 'static,
) -> Result<R, AppError> {
    Ok(tokio::task::spawn_blocking(f)
        .await
        .map_err(blocking_join_error)??)
}

fn submit(inner: &Inner, job: Box<dyn Job>) -> JobId {
    let key = job.dedupe_key();
    let now = inner.deps.clock.now();
    let mut state = inner.lock();
    if let Some(queued) = state.queue.iter().find(|q| q.key == key) {
        return queued.id.clone();
    }
    let ulid = match state.ids.generate_from_datetime(to_system_time(now)) {
        Ok(ulid) => ulid,
        Err(overflow) => overflow.commit_overflow_increment(),
    };
    let id = JobId(ulid.to_string());
    let kind = job.kind();
    state.queue.push_back(Queued {
        id: id.clone(),
        key,
        kind,
        job,
    });
    drop(state);
    inner.wake.notify_one();
    id
}

fn new_run(
    queued: &Queued,
    status: JobStatus,
    started: Option<UnixUs>,
) -> Result<NewJobRun, AppError> {
    Ok(NewJobRun {
        id: ulid::Ulid::from_string(&queued.id.0)
            .map_err(|e| AppError::internal(format!("job id: {e}")))?,
        kind: queued.kind.as_str().to_owned(),
        params: queued.job.params(),
        status,
        started,
    })
}

fn stored_error(error: &AppError) -> Result<serde_json::Value, AppError> {
    serde_json::to_value(Stored::Error {
        error: JobErrorDto {
            code: ErrorCodeDto::from(error.code),
            message_key: error.message_key.to_string(),
        },
    })
    .map_err(|e| AppError::internal(format!("summary json: {e}")))
}

async fn worker(inner: Arc<Inner>) {
    loop {
        let next = {
            let mut state = inner.lock();
            let next = state.queue.pop_front();
            if let Some(q) = &next {
                state.running = Some(Running {
                    id: q.id.clone(),
                    cancel: inner.shutdown.child_token(),
                });
            }
            next
        };
        match next {
            Some(queued) => {
                if let Err(e) = run_one(&inner, queued).await {
                    tracing::error!(error = %e, details = ?e.details, "job bookkeeping failed");
                }
                inner.lock().running = None;
            }
            None => {
                tokio::select! {
                    () = inner.wake.notified() => {}
                    () = inner.shutdown.cancelled() => return,
                }
            }
        }
        if inner.shutdown.is_cancelled() {
            return;
        }
    }
}

async fn run_one(inner: &Arc<Inner>, queued: Queued) -> Result<(), AppError> {
    let deps = inner.deps.clone();
    let cancel = inner
        .lock()
        .running
        .as_ref()
        .map(|r| r.cancel.clone())
        .unwrap_or_default();
    let started = deps.clock.now();
    let row = new_run(&queued, JobStatus::Running, Some(started))?;
    let ulid = row.id;
    let cache = deps.cache.clone();
    spawn_blocking(move || cache.write(move |tx| job_run::insert(tx, &row))).await?;

    let progress = ProgressSink::new(
        queued.id.clone(),
        queued.kind,
        deps.clock.clone(),
        deps.events.clone(),
    );
    let ctx = JobCtx {
        job_id: queued.id.clone(),
        cancel,
        progress: progress.clone(),
        user: deps.user.clone(),
        cache: deps.cache.clone(),
        vault: deps.vault.clone(),
        cpu: deps.cpu.clone(),
        clock: deps.clock.clone(),
        failed: Arc::default(),
    };
    let failed = ctx.failed.clone();
    let span = tracing::info_span!("job", id = %queued.id, kind = queued.kind.as_str());
    // Its own task, so a panic in the job is a JoinError here instead of killing the worker.
    let outcome = match tokio::spawn(queued.job.run(ctx).instrument(span)).await {
        Ok(result) => result,
        Err(e) if e.is_panic() => Err(AppError::internal(format!(
            "job panicked: {}",
            crate::jobs::panic_text(&*e.into_panic())
        ))),
        Err(e) => Err(AppError::internal(format!("job task: {e}"))),
    };
    progress.flush();

    let failed_items = failed.load(std::sync::atomic::Ordering::SeqCst);
    let (status, stored, summary) = match outcome {
        Ok(summary) => {
            let stored = match &summary.summary {
                Some(s) => serde_json::to_value(Stored::Summary(s.clone()))
                    .map_err(|e| AppError::internal(format!("summary json: {e}")))?,
                None => serde_json::Value::Object(serde_json::Map::new()),
            };
            (JobStatus::Ok, stored, Some(summary))
        }
        Err(e) => {
            let status = if e.code == ErrorCode::Cancelled {
                JobStatus::Cancelled
            } else {
                tracing::warn!(job = %queued.id, code = %e.code, details = ?e.details, "job failed");
                JobStatus::Failed
            };
            (status, stored_error(&e)?, None)
        }
    };
    let ended = deps.clock.now();
    let cache = deps.cache.clone();
    spawn_blocking(move || {
        cache.write(move |tx| job_run::finish(tx, ulid, status, ended, &stored))
    })
    .await?;

    let _ = deps.events.send(AppEvent::JobFinished(JobFinishedDto {
        job_id: queued.id,
        status: status_dto(status),
        failed_items,
    }));
    if let Some(summary) = summary {
        if !summary.changed.is_empty() {
            let _ = deps.events.send(AppEvent::data_changed(&summary.changed));
        }
        for follow_up in summary.follow_ups {
            submit(inner, follow_up);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::{Condvar, Mutex};

    use wolluf_core::FixedClock;
    use wolluf_store::repo::cache::item_failure;

    use super::*;
    use crate::context::{AppContext, AppPaths};
    use crate::jobs::{ItemError, ItemResult, JobFuture, JobStageDto, JobSummary};

    const T0: UnixUs = UnixUs(1_790_637_236_636_000);
    const WAIT: Duration = Duration::from_secs(20);

    /// A latch tests open by hand: jobs block on it to be observed "running".
    #[derive(Default)]
    struct Gate {
        open: Mutex<bool>,
        cv: Condvar,
        waiting: AtomicU32,
    }

    impl Gate {
        fn wait(&self) {
            self.waiting.fetch_add(1, Ordering::SeqCst);
            let mut open = self.open.lock().unwrap();
            while !*open {
                open = self.cv.wait(open).unwrap();
            }
        }

        fn open(&self) {
            *self.open.lock().unwrap() = true;
            self.cv.notify_all();
        }

        async fn until_waiting(&self, n: u32) {
            let deadline = tokio::time::Instant::now() + WAIT;
            while self.waiting.load(Ordering::SeqCst) < n {
                assert!(tokio::time::Instant::now() < deadline, "gate never reached");
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        }
    }

    type Items = Arc<Mutex<BTreeSet<u32>>>;

    #[derive(Clone)]
    struct TestJob {
        key: &'static str,
        runs: Arc<AtomicU32>,
        gate: Option<Arc<Gate>>,
        items: u32,
        done: Items,
        panic_on: Option<u32>,
        /// Items at or above this index wait on the gate (cancellation tests).
        gate_from_item: Option<u32>,
        follow_up: Option<Box<TestJob>>,
    }

    impl TestJob {
        fn new(key: &'static str) -> Self {
            Self {
                key,
                runs: Arc::default(),
                gate: None,
                items: 0,
                done: Arc::default(),
                panic_on: None,
                gate_from_item: None,
                follow_up: None,
            }
        }
    }

    impl Job for TestJob {
        fn kind(&self) -> JobKindDto {
            JobKindDto::SyncPlays
        }

        fn dedupe_key(&self) -> String {
            self.key.to_owned()
        }

        fn params(&self) -> serde_json::Value {
            serde_json::json!({ "key": self.key })
        }

        fn run(self: Box<Self>, ctx: JobCtx) -> JobFuture {
            Box::pin(async move {
                self.runs.fetch_add(1, Ordering::SeqCst);
                let job = *self;
                tokio::task::spawn_blocking(move || {
                    if let (Some(gate), None) = (&job.gate, job.gate_from_item) {
                        gate.wait();
                    }
                    let items: Vec<u32> = (0..job.items).collect();
                    let results = ctx.run_items(
                        JobStageDto::Ingest,
                        &items,
                        |i| format!("item-{i}"),
                        |&i| {
                            if job.panic_on == Some(i) {
                                panic!("item {i} exploded");
                            }
                            if let (Some(gate), Some(from)) = (&job.gate, job.gate_from_item)
                                && i >= from
                            {
                                gate.wait();
                            }
                            job.done.lock().unwrap().insert(i);
                            Ok::<_, ItemError>(i)
                        },
                    )?;
                    ctx.check_cancelled()?;
                    let done = results
                        .iter()
                        .filter(|r| matches!(r, ItemResult::Done(_)))
                        .count();
                    Ok(JobSummary {
                        summary: None,
                        changed: if done > 0 { vec!["plays"] } else { vec![] },
                        follow_ups: job
                            .follow_up
                            .map(|f| vec![f as Box<dyn Job>])
                            .unwrap_or_default(),
                    })
                })
                .await
                .map_err(blocking_join_error)?
            })
        }
    }

    fn open(dir: &std::path::Path) -> AppContext {
        AppContext::open(
            AppPaths::from_data_dir(dir.join("data")),
            Arc::new(FixedClock::new(T0)),
        )
        .unwrap()
    }

    async fn finished(rx: &mut broadcast::Receiver<AppEvent>, id: &JobId) -> JobFinishedDto {
        tokio::time::timeout(WAIT, async {
            loop {
                if let AppEvent::JobFinished(f) = rx.recv().await.unwrap()
                    && &f.job_id == id
                {
                    return f;
                }
            }
        })
        .await
        .expect("job finished in time")
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn submit_while_queued_returns_same_id() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = open(dir.path());
        let mut rx = ctx.subscribe();
        let gate = Arc::new(Gate::default());
        let blocker = TestJob {
            gate: Some(gate.clone()),
            ..TestJob::new("blocker")
        };
        let blocker_id = ctx.jobs().submit(Box::new(blocker));
        gate.until_waiting(1).await;
        let a = ctx.jobs().submit(Box::new(TestJob::new("sync_plays:1")));
        let b = ctx.jobs().submit(Box::new(TestJob::new("sync_plays:1")));
        let other = ctx.jobs().submit(Box::new(TestJob::new("sync_plays:2")));
        assert_eq!(a, b);
        assert_ne!(a, other);
        assert_ne!(a, blocker_id);
        let listed = ctx.jobs().list(None).await.unwrap();
        let queued: Vec<&JobId> = listed
            .iter()
            .filter(|j| j.status == JobStatusDto::Queued)
            .map(|j| &j.id)
            .collect();
        assert_eq!(queued, vec![&a, &other]);
        gate.open();
        finished(&mut rx, &other).await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn submit_while_running_sets_single_rerun() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = open(dir.path());
        let mut rx = ctx.subscribe();
        let gate = Arc::new(Gate::default());
        let job = TestJob {
            gate: Some(gate.clone()),
            ..TestJob::new("sync_plays:1")
        };
        let runs = job.runs.clone();
        let first = ctx.jobs().submit(Box::new(job.clone()));
        gate.until_waiting(1).await;
        let second = ctx.jobs().submit(Box::new(job.clone()));
        let third = ctx.jobs().submit(Box::new(job.clone()));
        assert_ne!(first, second, "a running job is not reused");
        assert_eq!(second, third, "repeated submits do not stack");
        gate.open();
        assert_eq!(finished(&mut rx, &first).await.status, JobStatusDto::Ok);
        assert_eq!(finished(&mut rx, &second).await.status, JobStatusDto::Ok);
        assert_eq!(runs.load(Ordering::SeqCst), 2);
        let history = ctx.jobs().list(None).await.unwrap();
        assert_eq!(history.len(), 2);
        assert!(history.iter().all(|j| j.status == JobStatusDto::Ok));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn follow_up_enqueued_after_finish() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = open(dir.path());
        let mut rx = ctx.subscribe();
        let child = TestJob::new("players.refresh");
        let child_runs = child.runs.clone();
        let parent = TestJob {
            follow_up: Some(Box::new(child)),
            ..TestJob::new("sync_plays:1")
        };
        let parent_id = ctx.jobs().submit(Box::new(parent));
        let mut order = Vec::new();
        tokio::time::timeout(WAIT, async {
            while order.len() < 2 {
                if let AppEvent::JobFinished(f) = rx.recv().await.unwrap() {
                    order.push(f.job_id);
                }
            }
        })
        .await
        .unwrap();
        assert_eq!(
            order[0], parent_id,
            "the parent finishes before its follow-up"
        );
        assert_ne!(order[1], parent_id);
        assert_eq!(child_runs.load(Ordering::SeqCst), 1);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn cancel_midway_then_rerun_completes() {
        const ITEMS: u32 = 200;
        const GATED_FROM: u32 = 10;
        let dir = tempfile::tempdir().unwrap();
        let ctx = open(dir.path());
        let mut rx = ctx.subscribe();
        let gate = Arc::new(Gate::default());
        let job = TestJob {
            items: ITEMS,
            gate: Some(gate.clone()),
            gate_from_item: Some(GATED_FROM),
            ..TestJob::new("sync_plays:1")
        };
        let done = job.done.clone();
        let id = ctx.jobs().submit(Box::new(job.clone()));
        gate.until_waiting(1).await;
        ctx.jobs().cancel(&id).await.unwrap();
        gate.open();
        let first = finished(&mut rx, &id).await;
        assert_eq!(first.status, JobStatusDto::Cancelled);
        let partial = done.lock().unwrap().len();
        assert!(
            partial < ITEMS as usize,
            "cancel stopped the run ({partial})"
        );

        let rerun = TestJob {
            gate: None,
            gate_from_item: None,
            ..job
        };
        let id2 = ctx.jobs().submit(Box::new(rerun));
        assert_eq!(finished(&mut rx, &id2).await.status, JobStatusDto::Ok);
        let expected: BTreeSet<u32> = (0..ITEMS).collect();
        assert_eq!(
            *done.lock().unwrap(),
            expected,
            "same final state as an uncancelled run"
        );
        let listed = ctx.jobs().list(None).await.unwrap();
        let cancelled = listed.iter().find(|j| j.id == id).unwrap();
        assert_eq!(cancelled.status, JobStatusDto::Cancelled);
        assert_eq!(
            cancelled.error.as_ref().map(|e| e.code),
            Some(ErrorCodeDto::Cancelled)
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn cancel_queued_job_and_unknown_id() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = open(dir.path());
        let mut rx = ctx.subscribe();
        let gate = Arc::new(Gate::default());
        let blocker = TestJob {
            gate: Some(gate.clone()),
            ..TestJob::new("blocker")
        };
        let blocker_id = ctx.jobs().submit(Box::new(blocker));
        gate.until_waiting(1).await;
        let queued = TestJob::new("sync_plays:1");
        let queued_runs = queued.runs.clone();
        let queued_id = ctx.jobs().submit(Box::new(queued));
        ctx.jobs().cancel(&queued_id).await.unwrap();
        assert_eq!(
            finished(&mut rx, &queued_id).await.status,
            JobStatusDto::Cancelled
        );
        let err = ctx
            .jobs()
            .cancel(&JobId("01ARZ3NDEKTSV4RRFFQ69G5FAV".into()))
            .await
            .unwrap_err();
        assert_eq!(err.code, ErrorCode::NotFound);
        gate.open();
        finished(&mut rx, &blocker_id).await;
        assert_eq!(queued_runs.load(Ordering::SeqCst), 0);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn panicking_item_recorded_job_continues() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = open(dir.path());
        let mut rx = ctx.subscribe();
        let job = TestJob {
            items: 20,
            panic_on: Some(3),
            ..TestJob::new("sync_plays:1")
        };
        let done = job.done.clone();
        let id = ctx.jobs().submit(Box::new(job));
        let fin = finished(&mut rx, &id).await;
        assert_eq!(fin.status, JobStatusDto::Ok);
        assert_eq!(fin.failed_items, 1);
        assert_eq!(done.lock().unwrap().len(), 19);
        let ulid = ulid::Ulid::from_string(&id.0).unwrap();
        let failures = ctx
            .cache_db()
            .read(|c| item_failure::list(c, ulid))
            .unwrap();
        assert_eq!(failures.len(), 1);
        assert_eq!(failures[0].item_ref, "item-3");
        assert_eq!(failures[0].code, ErrorCode::Internal);
        assert!(
            failures[0].message.contains("exploded"),
            "{:?}",
            failures[0]
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn data_changed_follows_job_finished() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = open(dir.path());
        let mut rx = ctx.subscribe();
        let id = ctx.jobs().submit(Box::new(TestJob {
            items: 2,
            ..TestJob::new("k")
        }));
        finished(&mut rx, &id).await;
        let next = tokio::time::timeout(WAIT, rx.recv())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(next, AppEvent::data_changed(&["plays"]));
    }
}
