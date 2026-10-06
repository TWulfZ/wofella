use wolluf_app::errors::IpcError;
use wolluf_app::features::skins::dto::{SkinDto, SkinListDto};

use super::Ctx;
use crate::error::to_ipc;

/// The skin folders of the catalog install, plus the cfg's active skin and mania speed.
#[tauri::command]
#[specta::specta]
#[tracing::instrument(skip_all)]
pub async fn skin_list(ctx: Ctx<'_>) -> Result<SkinListDto, IpcError> {
    ctx.skins().list().await.map_err(to_ipc)
}

/// One skin's `[Mania]` block and images for `keymode`; the webview sends only the folder name.
#[tauri::command]
#[specta::specta]
#[tracing::instrument(skip_all)]
pub async fn skin_get(ctx: Ctx<'_>, folder: String, keymode: u8) -> Result<SkinDto, IpcError> {
    ctx.skins().get(&folder, keymode).await.map_err(to_ipc)
}
