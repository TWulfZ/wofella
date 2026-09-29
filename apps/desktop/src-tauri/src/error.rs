//! The one exit from `AppError` to the wire (spec 005 Design).

use wolluf_app::errors::{AppError, IpcError};

/// Logs the full error first: release IPC drops `details` (§7 scrubbing), so the log file is
/// the only place a user's report can recover them from.
pub fn to_ipc(error: AppError) -> IpcError {
    tracing::warn!(
        code = error.code.as_str(),
        key = %error.message_key,
        details = error.details.as_deref(),
        "command failed"
    );
    IpcError::from(error)
}
