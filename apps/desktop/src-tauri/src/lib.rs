//! Tauri desktop shell (spec 005): composition root only, no logic (D11).

pub mod bindings;
pub mod commands;
pub mod error;
pub mod events;

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;

use tauri::{App, AppHandle, Builder, Manager, RunEvent, Runtime, WebviewWindowBuilder};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
use tokio::runtime::Handle;
use wolluf_app::clock::SystemClock;
use wolluf_app::context::{AppContext, AppPaths};
use wolluf_app::errors::AppError;
use wolluf_app::logging::{self, LogGuard, LogOptions};

/// Spec 005: a start that cannot open the context exits 1 after the dialog.
const EXIT_STARTUP_FAILURE: i32 = 1;
const LOG_FILTER_ENV: &str = "WOLLUF_LOG";
/// The CLI defaults to `warn`; the desktop keeps `info` so a user's log file explains a sync.
const DEFAULT_LOG_FILTER: &str = "info";
const MAIN_WINDOW: &str = "main";
const FATAL_TITLE: &str = "wolluf";

/// Registers the plugins every build uses, generic so tests can pass the mock runtime.
pub fn with_plugins<R: Runtime>(builder: Builder<R>) -> Builder<R> {
    builder
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        // `tracing` owns the global logger; the plugin only forwards UI logs into it, and a
        // second `log::set_logger` would fail (spec 005 Plugins).
        .plugin(tauri_plugin_log::Builder::new().skip_logger().build())
}

/// Append-only: 004 adds its `players_*` commands here, and the bindings follow this list.
pub fn specta_builder<R: Runtime>() -> tauri_specta::Builder<R> {
    tauri_specta::Builder::<R>::new()
        .events(events::collect())
        .commands(tauri_specta::collect_commands![
            commands::setup::setup_detect_installs,
            commands::setup::setup_set_install_path,
            commands::setup::setup_status,
            commands::jobs::jobs_list,
            commands::jobs::jobs_start,
            commands::jobs::jobs_cancel,
            // The specta half only reads argument types, and `AppHandle` is skipped there; the Tauri half
            // strips the generic and infers `R`, so a concrete runtime here serves every `R`.
            commands::app::app_open_logs_dir::<tauri::Wry>,
            commands::players::players_list_aliases,
            commands::players::players_list_profiles,
            commands::players::players_set_profile_aliases,
            commands::players::players_decide_alias,
            commands::players::players_create_profile,
            commands::players::players_set_default,
            commands::chart::chart_window,
            commands::label::label_taxonomy,
            commands::label::label_sample,
            commands::label::label_resolve_patterns,
            commands::label::label_reshape,
            commands::label::label_submit,
            commands::label::label_undo,
            commands::label::label_stats,
        ])
}

pub fn manage_context<R: Runtime>(app: &App<R>, ctx: Arc<AppContext>) {
    app.manage(ctx);
}

/// `runtime` is the one `main` handed to `tauri::async_runtime::set`, so the context and the
/// commands share it.
pub fn run(runtime: Handle) -> ExitCode {
    let mut log_guard: Option<LogGuard> = None;
    let prepared = AppPaths::resolve(None)
        .map_err(|error| StartupFailure {
            error,
            logs_dir: None,
        })
        .and_then(|paths| {
            let guard = logging::init(log_options(&paths)).map_err(|error| StartupFailure {
                error,
                logs_dir: Some(paths.logs_dir()),
            })?;
            log_guard = Some(guard);
            Ok(paths)
        });

    let specta = specta_builder();
    let exit_runtime = runtime.clone();
    let built = with_plugins(tauri::Builder::default())
        .invoke_handler(specta.invoke_handler())
        .setup(move |app| {
            let opened = prepared.and_then(|paths| {
                let logs_dir = paths.logs_dir();
                AppContext::open_in(paths, Arc::new(SystemClock), runtime).map_err(|error| {
                    StartupFailure {
                        error,
                        logs_dir: Some(logs_dir),
                    }
                })
            });
            // A setup error panics inside Tauri's event loop, so every failure goes to the dialog.
            let started = opened.and_then(|ctx| {
                let logs_dir = ctx.paths().logs_dir();
                let bus = ctx.subscribe();
                manage_context(app, Arc::new(ctx));
                specta.mount_events(app);
                // Detached: it ends by itself when the context, and with it the bus, is dropped.
                drop(events::spawn_bridge(app.handle().clone(), bus));
                // Before the window opens, so a dev server reload from the rewrite happens before
                // the first page load rather than during it.
                #[cfg(debug_assertions)]
                export_dev_bindings(&specta);
                open_main_window(app).map_err(|e| StartupFailure {
                    error: AppError::internal(e.to_string()),
                    logs_dir: Some(logs_dir),
                })
            });
            if let Err(failure) = started {
                show_fatal(app.handle(), &failure);
            }
            Ok(())
        })
        .build(context());
    let code = match built {
        Ok(app) => app.run_return(move |app, event| {
            if let RunEvent::Exit = event {
                close_context(app, &exit_runtime);
            }
        }),
        Err(e) => {
            tracing::error!(error = %e, "tauri build failed");
            EXIT_STARTUP_FAILURE
        }
    };
    // Explicit: the file writer flushes only on drop, and the event loop is over.
    drop(log_guard);
    u8::try_from(code).map_or(ExitCode::FAILURE, ExitCode::from)
}

/// Tauri state cannot be taken back out, so the shared context shuts down in place: the
/// running job is recorded as cancelled and no handle under the data dir outlives the loop.
/// Absent when startup failed. Runs on the main thread, outside the runtime.
fn close_context<R: Runtime>(app: &AppHandle<R>, runtime: &Handle) {
    if let Some(ctx) = app.try_state::<Arc<AppContext>>() {
        runtime.block_on(ctx.shutdown());
    }
}

/// Keeps `bindings.ts` current during `cargo tauri dev` (§8). A failure only warns: the UI may
/// run from a checkout where the file is read-only, and CI's drift check still catches it.
#[cfg(debug_assertions)]
fn export_dev_bindings<R: Runtime>(specta: &tauri_specta::Builder<R>) {
    let path = bindings::committed_path();
    if let Err(e) = bindings::export_with(specta, &path) {
        tracing::warn!(error = %e, path = %path.display(), "bindings export failed");
    }
}

// The macro expands to std HashMaps inside Tauri's asset tables; wolluf never iterates them.
#[allow(clippy::disallowed_types)]
fn context() -> tauri::Context {
    tauri::generate_context!()
}

fn log_options(paths: &AppPaths) -> LogOptions {
    LogOptions {
        filter: std::env::var(LOG_FILTER_ENV).unwrap_or_else(|_| DEFAULT_LOG_FILTER.to_owned()),
        json_dir: Some(paths.logs_dir()),
        console: cfg!(debug_assertions),
    }
}

/// The window is declared with `create: false`, so a failed start never shows a half-working
/// UI (spec 005 Behaviour, startup failure).
fn open_main_window<R: Runtime>(app: &App<R>) -> Result<(), String> {
    let config = app
        .config()
        .app
        .windows
        .iter()
        .find(|w| w.label == MAIN_WINDOW)
        .ok_or_else(|| format!("tauri.conf.json has no {MAIN_WINDOW:?} window"))?;
    WebviewWindowBuilder::from_config(app, config)
        .and_then(WebviewWindowBuilder::build)
        .map(drop)
        .map_err(|e| e.to_string())
}

struct StartupFailure {
    error: AppError,
    logs_dir: Option<PathBuf>,
}

/// Non-blocking with an exit callback: `blocking_show` would wait on the main thread, which the
/// dialog itself needs before the event loop runs.
fn show_fatal<R: Runtime>(app: &AppHandle<R>, failure: &StartupFailure) {
    tracing::error!(
        code = failure.error.code.as_str(),
        key = %failure.error.message_key,
        details = failure.error.details.as_deref(),
        "startup failed"
    );
    let handle = app.clone();
    app.dialog()
        .message(fatal_message(&failure.error, failure.logs_dir.as_deref()))
        .title(FATAL_TITLE)
        .kind(MessageDialogKind::Error)
        .buttons(MessageDialogButtons::Ok)
        .show(move |_| handle.exit(EXIT_STARTUP_FAILURE));
}

/// Not localized: the UI and its i18n never loaded. Code and key are what a bug report needs.
fn fatal_message(error: &AppError, logs_dir: Option<&Path>) -> String {
    let args: Vec<String> = error.args.iter().map(|(k, v)| format!("{k}={v}")).collect();
    let logs = logs_dir.map_or_else(|| "unavailable".to_owned(), |d| d.display().to_string());
    format!(
        "wolluf could not start.\n\nerror[{}]: {} {{{}}}\n\nLogs: {logs}",
        error.code.as_str(),
        error.message_key,
        args.join(", ")
    )
}

#[cfg(test)]
mod scaffold {
    use std::sync::Arc;

    use tauri::test::{mock_builder, mock_context, noop_assets};
    use wolluf_core::{FixedClock, UnixUs};

    use super::*;

    const T0: UnixUs = UnixUs(1_790_637_236_636_000);

    #[test]
    fn builds_with_mock_runtime() {
        let dir = tempfile::tempdir().unwrap();
        let paths = AppPaths::from_data_dir(dir.path().join("data"));
        let ctx = AppContext::open(paths, Arc::new(FixedClock::new(T0))).unwrap();

        // `setup` hooks only run once an event loop starts, which the mock runtime never does,
        // so the context is managed on the built app exactly as `run`'s hook does it.
        let app = with_plugins(mock_builder())
            .build(mock_context(noop_assets()))
            .unwrap();
        manage_context(&app, Arc::new(ctx));

        let managed = app.state::<Arc<AppContext>>();
        assert_eq!(managed.paths().data_dir(), dir.path().join("data"));
        for plugin in ["dialog", "opener", "log"] {
            assert!(
                app.handle().remove_plugin(plugin),
                "{plugin} not registered"
            );
        }
    }

    #[test]
    fn fatal_message_names_code_key_args_and_logs() {
        let error = AppError::conflict()
            .with_key("error.instance_running")
            .with_arg("path", "/d");
        let text = fatal_message(&error, Some(Path::new("/d/logs")));
        assert!(
            text.contains("error[CONFLICT]: error.instance_running {path=/d}"),
            "{text}"
        );
        assert!(text.ends_with("Logs: /d/logs"), "{text}");
        let text = fatal_message(&error, None);
        assert!(text.ends_with("Logs: unavailable"), "{text}");
    }
}
