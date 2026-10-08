use wolluf_app::errors::IpcError;
use wolluf_app::features::rate_copies::dto::RateCopyPlanDto;
use wolluf_app::jobs::dto::JobId;

use super::Ctx;
use crate::error::to_ipc;

/// Previews a rate copy of the chart (ADR 0025); writes nothing. `nightcore` makes the pitch
/// follow the rate.
#[tauri::command]
#[specta::specta]
#[tracing::instrument(skip_all)]
pub async fn rate_copy_plan(
    ctx: Ctx<'_>,
    md5: String,
    rate_milli: u16,
    nightcore: bool,
) -> Result<RateCopyPlanDto, IpcError> {
    let plan = ctx.rate_copies().plan(&md5, rate_milli, nightcore).await;
    plan.map_err(to_ipc)
}

/// The only path that mints an `ExportPermit` (D9): starts the `rate_copy` job.
#[tauri::command]
#[specta::specta]
#[tracing::instrument(skip_all)]
pub async fn rate_copy_confirm(ctx: Ctx<'_>, preview_id: String) -> Result<JobId, IpcError> {
    ctx.rate_copies().confirm(&preview_id).await.map_err(to_ipc)
}
