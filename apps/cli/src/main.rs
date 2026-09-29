//! `wolluf`: the developer CLI over `wolluf-app` (spec 005). It opens the same data dir as the
//! desktop and prints the same DTOs.

mod cli;
mod cmd;
mod exit;
mod progress;
mod render;

use std::process::ExitCode;
use std::sync::Arc;

use clap::Parser;
use wolluf_app::clock::SystemClock;
use wolluf_app::context::{AppContext, AppPaths};
use wolluf_app::errors::AppError;
use wolluf_app::logging::{self, LogOptions};

use cli::{Cli, Command};

#[tokio::main]
async fn main() -> anyhow::Result<ExitCode> {
    // Clap prints usage errors and exits 2 itself, which is the spec's input-error code.
    let cli = Cli::parse();
    match run(cli).await {
        Ok(code) => Ok(code),
        Err(e) => match e.downcast_ref::<AppError>() {
            Some(app) => {
                render::error(app);
                Ok(exit::exit_code(exit::for_error(app.code)))
            }
            None => Err(e),
        },
    }
}

async fn run(cli: Cli) -> anyhow::Result<ExitCode> {
    let paths = AppPaths::resolve(cli.data_dir)?;
    // tracing-appender prunes old files at startup and prints to stderr when the dir is missing.
    let logs_dir = paths.logs_dir();
    std::fs::create_dir_all(&logs_dir)
        .map_err(|e| AppError::internal(format!("create {}: {e}", logs_dir.display())))?;
    let _log = logging::init(LogOptions {
        filter: cli.log,
        json_dir: Some(paths.logs_dir()),
        console: true,
    })?;
    // Opening migrates user.db and takes the instance lock: blocking work.
    let ctx = tokio::task::spawn_blocking(move || AppContext::open(paths, Arc::new(SystemClock)))
        .await??;
    match cli.command {
        Command::Setup(cmd) => cmd::setup::run(&ctx, cmd, cli.json).await,
        Command::Sync(_) => cmd::sync::run(&ctx, cli.json).await,
        Command::Players(cmd) => cmd::players::run(&ctx, cmd, cli.json).await,
        Command::Jobs(cmd) => cmd::jobs::run(&ctx, cmd, cli.json).await,
    }
}
