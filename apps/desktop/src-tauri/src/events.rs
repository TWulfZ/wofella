//! Bridges 003's `AppEvent` bus to typed webview events (spec 005 Design, "Events").

use serde::Serialize;
use tauri::{AppHandle, Runtime};
use tauri_specta::Event;
use tokio::sync::broadcast::{self, error::RecvError};
use wolluf_app::events::{AppEvent, DataChangedDto, JobFinishedDto, JobProgressDto};

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

pub fn collect() -> tauri_specta::Events {
    tauri_specta::collect_events![JobProgress, JobFinished, DataChanged]
}

/// Needs `mount_events` on the app first: tauri-specta resolves event names through it.
pub fn spawn_bridge<R: Runtime>(
    app: AppHandle<R>,
    events: broadcast::Receiver<AppEvent>,
) -> tauri::async_runtime::JoinHandle<()> {
    tauri::async_runtime::spawn(bridge(app, events))
}

async fn bridge<R: Runtime>(app: AppHandle<R>, mut events: broadcast::Receiver<AppEvent>) {
    loop {
        let emitted = match events.recv().await {
            Ok(AppEvent::JobProgress(dto)) => JobProgress(dto).emit(&app),
            Ok(AppEvent::JobFinished(dto)) => JobFinished(dto).emit(&app),
            Ok(AppEvent::DataChanged(dto)) => DataChanged(dto).emit(&app),
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
