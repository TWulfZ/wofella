use wolluf_app::errors::IpcError;
use wolluf_app::features::library::dto::ChartWindowDto;

use super::Ctx;
use crate::error::to_ipc;

/// Notes, timing lines and layout of `[fromMs, toMs]` for the playfield; no segments.
#[tauri::command]
#[specta::specta]
#[tracing::instrument(skip_all)]
pub async fn chart_window(
    ctx: Ctx<'_>,
    md5: String,
    from_ms: i32,
    to_ms: i32,
    layout_id: Option<String>,
) -> Result<ChartWindowDto, IpcError> {
    ctx.library()
        .chart_window(&md5, from_ms, to_ms, layout_id)
        .await
        .map_err(to_ipc)
}
