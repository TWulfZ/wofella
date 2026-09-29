// Release builds must not open a console window next to the app on Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::process::ExitCode;
use std::time::Duration;

/// Bounds exit when a blocking task (a DB write, a vault put) is still running.
const SHUTDOWN_GRACE: Duration = Duration::from_secs(5);

fn main() -> ExitCode {
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_name("wolluf-rt")
        .build()
    {
        Ok(runtime) => runtime,
        Err(e) => {
            eprintln!("error[INTERNAL]: tokio runtime: {e}");
            return ExitCode::FAILURE;
        }
    };
    // One runtime for Tauri commands and wolluf-app: two would split the blocking pools and
    // make shutdown order fragile (spec 005 Runtime).
    tauri::async_runtime::set(runtime.handle().clone());
    let code = wolluf_desktop::run(runtime.handle().clone());
    runtime.shutdown_timeout(SHUTDOWN_GRACE);
    code
}
