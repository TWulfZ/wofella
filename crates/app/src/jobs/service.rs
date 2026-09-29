//! `JobService` (spec 003 IPC): the one door shells use for jobs. `start` is the registry that
//! maps a `JobStartDto` kind to the feature service that owns the job.

use crate::context::{AppContext, InstallId};
use crate::errors::AppError;
use crate::features::plays::PlaysService;
use crate::jobs::dto::{JobDto, JobId, JobStartDto};

pub struct JobService<'a> {
    ctx: &'a AppContext,
}

impl<'a> JobService<'a> {
    pub fn new(ctx: &'a AppContext) -> Self {
        Self { ctx }
    }

    pub async fn start(&self, request: JobStartDto) -> Result<JobId, AppError> {
        match request {
            JobStartDto::SyncPlays(p) => {
                PlaysService::new(self.ctx)
                    .sync(InstallId(i64::from(p.install_id)))
                    .await
            }
        }
    }

    /// Queued jobs first, then history newest first.
    pub async fn list(&self, limit: Option<u32>) -> Result<Vec<JobDto>, AppError> {
        self.ctx.jobs().list(limit).await
    }

    pub async fn cancel(&self, id: &JobId) -> Result<(), AppError> {
        self.ctx.jobs().cancel(id).await
    }
}
