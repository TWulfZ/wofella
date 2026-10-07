//! Bridges 003's `AppEvent` bus to typed webview events (spec 005 Design, "Events").

use serde::Serialize;
use tauri::{AppHandle, Manager, Runtime, UserAttentionType};
use tauri_specta::Event;
use tokio::sync::broadcast::{self, error::RecvError};
use wolluf_app::events::{
    AppEvent, DataChangedDto, JobFinishedDto, JobProgressDto, SessionPlayAddedDto,
};

use crate::MAIN_WINDOW;

/// Query-key root the UI re-hydrates the job tray from (`jobs_list`).
const DOMAIN_JOBS: &str = "jobs";

// `tauri_specta::Event` cannot be implemented on app types (orphan rule), so the shell wraps
// them; `transparent` keeps the wire payload identical to the DTO.
#[derive(Debug, Clone, Serialize, specta::Type, tauri_specta::Event)]
#[serde(transparent)]
pub struct JobProgress(pub JobProgressDto);

#[derive(Debug, Clone, Serialize, specta::Type, tauri_specta::Event)]
#[serde(transparent)]
pub struct JobFinished(pub JobFinishedDto);

#[derive(Debug, Clone, Serialize, specta::Type, tauri_specta::Event)]
#[serde(transparent)]
pub struct DataChanged(pub DataChangedDto);

#[derive(Debug, Clone, Serialize, specta::Type, tauri_specta::Event)]
#[serde(transparent)]
pub struct SessionPlayAdded(pub SessionPlayAddedDto);

/// Append-only, like the command list: the bindings follow it.
pub fn collect() -> tauri_specta::Events {
    tauri_specta::collect_events![JobProgress, JobFinished, DataChanged, SessionPlayAdded]
}

/// Needs `mount_events` on the app first: tauri-specta resolves event names through it.
pub fn spawn_bridge<R: Runtime>(
    app: AppHandle<R>,
    events: broadcast::Receiver<AppEvent>,
) -> tauri::async_runtime::JoinHandle<()> {
    spawn_bridge_with(app, events, flash_main_window)
}

/// [`spawn_bridge`] with the window flash swapped, so tests can observe it.
pub fn spawn_bridge_with<R: Runtime>(
    app: AppHandle<R>,
    events: broadcast::Receiver<AppEvent>,
    attention: impl Fn(&AppHandle<R>) + Send + 'static,
) -> tauri::async_runtime::JoinHandle<()> {
    tauri::async_runtime::spawn(bridge(app, events, attention))
}

/// A taskbar flash, not an OS notification (ADR 0020): Linux and wine users get no toasts. A
/// Rust-side window call, so no webview capability is involved.
fn flash_main_window<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window(MAIN_WINDOW)
        && let Err(e) = window.request_user_attention(Some(UserAttentionType::Informational))
    {
        tracing::warn!(error = %e, "window attention request failed");
    }
}

async fn bridge<R: Runtime>(
    app: AppHandle<R>,
    mut events: broadcast::Receiver<AppEvent>,
    attention: impl Fn(&AppHandle<R>),
) {
    loop {
        let emitted = match events.recv().await {
            Ok(AppEvent::JobProgress(dto)) => JobProgress(dto).emit(&app),
            Ok(AppEvent::JobFinished(dto)) => JobFinished(dto).emit(&app),
            Ok(AppEvent::DataChanged(dto)) => DataChanged(dto).emit(&app),
            Ok(AppEvent::SessionPlayAdded(dto)) => SessionPlayAdded(dto).emit(&app),
            Ok(AppEvent::AttentionRequested) => {
                attention(&app);
                Ok(())
            }
            // Dropped events may include a `JobFinished`; re-hydrating the tray from `jobs_list`
            // is what keeps it from showing a stuck job (spec 005 Behaviour, "Events").
            Err(RecvError::Lagged(skipped)) => {
                tracing::warn!(skipped, "event bridge lagged");
                DataChanged(DataChangedDto {
                    domains: vec![DOMAIN_JOBS.to_owned()],
                })
                .emit(&app)
            }
            Err(RecvError::Closed) => break,
        };
        if let Err(e) = emitted {
            tracing::warn!(error = %e, "event emit failed");
        }
    }
}
