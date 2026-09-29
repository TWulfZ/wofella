use wolluf_app::errors::IpcError;
use wolluf_app::features::setup::dto::{InstallCandidateDto, InstallDto, SetupStatusDto};

use super::Ctx;
use crate::error::to_ipc;

#[tauri::command]
#[specta::specta]
#[tracing::instrument(skip_all)]
pub async fn setup_detect_installs(ctx: Ctx<'_>) -> Result<Vec<InstallCandidateDto>, IpcError> {
    ctx.setup().detect_installs().await.map_err(to_ipc)
}

#[tauri::command]
#[specta::specta]
#[tracing::instrument(skip_all)]
pub async fn setup_set_install_path(ctx: Ctx<'_>, path: String) -> Result<InstallDto, IpcError> {
    ctx.setup().set_install_path(path).await.map_err(to_ipc)
}

#[tauri::command]
#[specta::specta]
#[tracing::instrument(skip_all)]
pub async fn setup_status(ctx: Ctx<'_>) -> Result<SetupStatusDto, IpcError> {
    ctx.setup().status().await.map_err(to_ipc)
}
