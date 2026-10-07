use wolluf_app::errors::IpcError;
use wolluf_app::features::labeling::dto::{LabelEventDto, LabelProgressDto, SessionLabelSubmitDto};
use wolluf_app::features::session::dto::SessionPlaysDto;

use super::Ctx;
use crate::error::to_ipc;

/// Self plays of the keymode saved since the app started, newest first, with each chart's
/// session answer (ADR 0020).
#[tauri::command]
#[specta::specta]
#[tracing::instrument(skip_all)]
pub async fn session_plays(ctx: Ctx<'_>, keymode: u8) -> Result<SessionPlaysDto, IpcError> {
    ctx.session().plays(keymode).await.map_err(to_ipc)
}

/// Stores a played map's dominant pattern; `pattern` `None` is "no clear pattern".
#[tauri::command]
#[specta::specta]
#[tracing::instrument(skip_all)]
pub async fn session_label_submit(
    ctx: Ctx<'_>,
    req: SessionLabelSubmitDto,
) -> Result<LabelEventDto, IpcError> {
    ctx.labeling().session_submit(req).await.map_err(to_ipc)
}

#[tauri::command]
#[specta::specta]
#[tracing::instrument(skip_all)]
pub async fn session_label_undo(ctx: Ctx<'_>, event_id: String) -> Result<(), IpcError> {
    ctx.labeling().session_undo(&event_id).await.map_err(to_ipc)
}

/// Gold and session labelling of the keymode; days are cut at `utcOffsetMin` east of UTC.
#[tauri::command]
#[specta::specta]
#[tracing::instrument(skip_all)]
pub async fn label_progress(
    ctx: Ctx<'_>,
    keymode: u8,
    utc_offset_min: i16,
) -> Result<LabelProgressDto, IpcError> {
    ctx.labeling()
        .progress(keymode, utc_offset_min)
        .await
        .map_err(to_ipc)
}
