use wolluf_app::errors::IpcError;
use wolluf_app::features::settings::dto::HandLayoutDto;

use super::Ctx;
use crate::error::to_ipc;

/// The keymode's column layout presets, the profile's default first.
#[tauri::command]
#[specta::specta]
#[tracing::instrument(skip_all)]
pub async fn settings_hand_layouts(
    ctx: Ctx<'_>,
    keymode: u8,
) -> Result<Vec<HandLayoutDto>, IpcError> {
    ctx.settings().hand_layouts(keymode).map_err(to_ipc)
}

/// The chosen layout preset id, else the profile's default.
#[tauri::command]
#[specta::specta]
#[tracing::instrument(skip_all)]
pub async fn settings_get_hand_layout(ctx: Ctx<'_>, keymode: u8) -> Result<String, IpcError> {
    ctx.settings().hand_layout(keymode).await.map_err(to_ipc)
}

/// `INVALID_INPUT` unless `layoutId` is a preset of the keymode.
#[tauri::command]
#[specta::specta]
#[tracing::instrument(skip_all)]
pub async fn settings_set_hand_layout(
    ctx: Ctx<'_>,
    keymode: u8,
    layout_id: String,
) -> Result<(), IpcError> {
    ctx.settings()
        .set_hand_layout(keymode, &layout_id)
        .await
        .map_err(to_ipc)
}

/// Whether a finished map flashes the window; off until the user turns it on.
#[tauri::command]
#[specta::specta]
#[tracing::instrument(skip_all)]
pub async fn settings_get_session_notify(ctx: Ctx<'_>) -> Result<bool, IpcError> {
    ctx.settings().session_notify().await.map_err(to_ipc)
}

#[tauri::command]
#[specta::specta]
#[tracing::instrument(skip_all)]
pub async fn settings_set_session_notify(ctx: Ctx<'_>, on: bool) -> Result<(), IpcError> {
    ctx.settings().set_session_notify(on).await.map_err(to_ipc)
}
