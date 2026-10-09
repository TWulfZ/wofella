use wolluf_app::errors::IpcError;
use wolluf_app::features::meta::dto::KeymodeDto;

use super::Ctx;

/// Enabled keymodes, ascending, so the UI never hard-codes them (D5). Infallible, but a `Result`
/// like every other command because the UI's `call` unwraps only that shape.
#[tauri::command]
#[specta::specta]
#[tracing::instrument(skip_all)]
pub async fn meta_keymodes(ctx: Ctx<'_>) -> Result<Vec<KeymodeDto>, IpcError> {
    Ok(ctx.meta().keymodes())
}
