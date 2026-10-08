use tauri::{AppHandle, Runtime};
use tauri_plugin_opener::OpenerExt;
use wolluf_app::errors::{AppError, IpcError};

use super::Ctx;
use crate::error::to_ipc;

/// A pure shell side effect with no app logic, so it lives here and not in `wolluf-app`
/// (ADR 0009).
#[tauri::command]
#[specta::specta]
#[tracing::instrument(skip_all)]
pub async fn app_open_logs_dir<R: Runtime>(
    app: AppHandle<R>,
    ctx: Ctx<'_>,
) -> Result<(), IpcError> {
    let dir = ctx.paths().logs_dir();
    app.opener()
        .open_path(dir.to_string_lossy(), None::<&str>)
        .map_err(|e| to_ipc(AppError::internal(format!("open {}: {e}", dir.display()))))
}

#[tauri::command]
#[specta::specta]
#[tracing::instrument(skip_all)]
pub async fn app_open_exports_dir<R: Runtime>(
    app: AppHandle<R>,
    ctx: Ctx<'_>,
) -> Result<(), IpcError> {
    let dir = ctx.labeling().exports_dir().await.map_err(to_ipc)?;
    app.opener()
        .open_path(dir.to_string_lossy(), None::<&str>)
        .map_err(|e| to_ipc(AppError::internal(format!("open {}: {e}", dir.display()))))
}
