use wolluf_app::errors::IpcError;
use wolluf_app::jobs::dto::{JobDto, JobId, JobStartDto};

use super::Ctx;
use crate::error::to_ipc;

/// The whole history: the tray keeps its own cap of finished jobs (spec 005 Behaviour).
#[tauri::command]
#[specta::specta]
#[tracing::instrument(skip_all)]
pub async fn jobs_list(ctx: Ctx<'_>) -> Result<Vec<JobDto>, IpcError> {
    ctx.job_service().list(None).await.map_err(to_ipc)
}

#[tauri::command]
#[specta::specta]
#[tracing::instrument(skip_all)]
pub async fn jobs_start(ctx: Ctx<'_>, request: JobStartDto) -> Result<JobId, IpcError> {
    ctx.job_service().start(request).await.map_err(to_ipc)
}

#[tauri::command]
#[specta::specta]
#[tracing::instrument(skip_all)]
pub async fn jobs_cancel(ctx: Ctx<'_>, id: JobId) -> Result<(), IpcError> {
    ctx.job_service().cancel(&id).await.map_err(to_ipc)
}
