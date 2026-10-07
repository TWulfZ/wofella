use wolluf_app::errors::IpcError;
use wolluf_app::features::library::dto::{
    ChartAudioDto, ChartDetailsDto, ChartImageDto, ChartWindowDto,
};

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

/// The chart's audio file for the playfield's WebAudio loop; the webview sends only the md5.
#[tauri::command]
#[specta::specta]
#[tracing::instrument(skip_all)]
pub async fn chart_audio(ctx: Ctx<'_>, md5: String) -> Result<ChartAudioDto, IpcError> {
    ctx.library().chart_audio(&md5).await.map_err(to_ipc)
}

/// The chart's `[Events]` background; `None` when it names none or the file cannot be shown.
#[tauri::command]
#[specta::specta]
#[tracing::instrument(skip_all)]
pub async fn chart_background(
    ctx: Ctx<'_>,
    md5: String,
) -> Result<Option<ChartImageDto>, IpcError> {
    ctx.library().chart_background(&md5).await.map_err(to_ipc)
}

/// The catalog row and parse counts the label card and its details dialog show.
#[tauri::command]
#[specta::specta]
#[tracing::instrument(skip_all)]
pub async fn chart_details(ctx: Ctx<'_>, md5: String) -> Result<ChartDetailsDto, IpcError> {
    ctx.library().chart_details(&md5).await.map_err(to_ipc)
}
