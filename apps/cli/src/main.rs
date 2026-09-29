//! `wolluf`: the developer CLI over `wolluf-app` (spec 005). It opens the same data dir as the
//! desktop and prints the same DTOs.

mod cli;
mod cmd;
mod exit;
mod progress;
mod render;

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

use clap::Parser;
use wolluf_app::clock::SystemClock;
use wolluf_app::context::{AppContext, AppPaths};
use wolluf_app::errors::AppError;
use wolluf_app::logging::{self, LogGuard, LogOptions};

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
    let Cli {
        data_dir,
        json,
        log,
        command,
    } = cli;
    match command {
        // The spike tools read only the files they are given: no data dir and no instance
        // lock, so they also run while the desktop is open.
        Command::Osg(cmd) => {
            let _log = logging::init(LogOptions {
                filter: log,
                json_dir: None,
                console: true,
            })?;
            cmd::osg::run(cmd, json).await
        }
        Command::Setup(cmd) => {
            let s = Session::open(data_dir, log).await?;
            cmd::setup::run(&s.ctx, cmd, json).await
        }
        Command::Sync(_) => {
            let s = Session::open(data_dir, log).await?;
            cmd::sync::run(&s.ctx, json).await
        }
        Command::Players(cmd) => {
            let s = Session::open(data_dir, log).await?;
            cmd::players::run(&s.ctx, cmd, json).await
        }
        Command::Jobs(cmd) => {
            let s = Session::open(data_dir, log).await?;
            cmd::jobs::run(&s.ctx, cmd, json).await
        }
    }
}

/// Field order is drop order: the context closes before the log guard flushes, so its
/// shutdown records still reach the file.
struct Session {
    ctx: AppContext,
    _log: LogGuard,
}

impl Session {
    async fn open(data_dir: Option<PathBuf>, log: String) -> anyhow::Result<Self> {
        let paths = AppPaths::resolve(data_dir)?;
        // tracing-appender prunes old files at startup and prints to stderr when the dir is missing.
        let logs_dir = paths.logs_dir();
        std::fs::create_dir_all(&logs_dir)
            .map_err(|e| AppError::internal(format!("create {}: {e}", logs_dir.display())))?;
        let log = logging::init(LogOptions {
            filter: log,
            json_dir: Some(paths.logs_dir()),
            console: true,
        })?;
        // Opening migrates user.db and takes the instance lock: blocking work.
        let ctx =
            tokio::task::spawn_blocking(move || AppContext::open(paths, Arc::new(SystemClock)))
                .await??;
        Ok(Self { ctx, _log: log })
    }
}
