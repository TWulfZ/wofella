//! The clap tree (spec 005 Behaviour, "CLI"). 006 adds `Command::Osg`.

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

/// Spec 005: the CLI is quiet by default; the desktop defaults to `info`.
const DEFAULT_LOG_FILTER: &str = "warn";

#[derive(Debug, Parser)]
#[command(
    name = "wolluf",
    version,
    about = "wolluf developer CLI (osu!mania stable)"
)]
pub(crate) struct Cli {
    /// Overrides WOLLUF_DATA_DIR and the platform data dir.
    #[arg(long, global = true, value_name = "DIR")]
    pub(crate) data_dir: Option<PathBuf>,
    /// Print the command result as JSON on stdout.
    #[arg(long, global = true)]
    pub(crate) json: bool,
    /// tracing EnvFilter directives for stderr and the JSON log file.
    #[arg(
        long,
        global = true,
        env = "WOLLUF_LOG",
        default_value = DEFAULT_LOG_FILTER,
        value_name = "FILTER"
    )]
    pub(crate) log: String,
    #[command(subcommand)]
    pub(crate) command: Command,
}

#[derive(Debug, Subcommand)]
pub(crate) enum Command {
    /// Find, register and inspect the osu! stable install.
    #[command(subcommand)]
    Setup(SetupCmd),
    /// Sync plays from the registered install and wait for the job.
    Sync(SyncArgs),
    /// Players seen in scores.db.
    #[command(subcommand)]
    Players(PlayersCmd),
    /// Background job history.
    #[command(subcommand)]
    Jobs(JobsCmd),
}

#[derive(Debug, Subcommand)]
pub(crate) enum SetupCmd {
    /// List install candidates (env, registry, known folders, drive scan).
    Detect,
    /// Validate and register an install folder.
    Set {
        #[arg(value_name = "PATH")]
        path: String,
    },
    /// Show the registered install, identity readiness and the last sync.
    Status,
}

#[derive(Debug, Args)]
pub(crate) struct SyncArgs {}

#[derive(Debug, Subcommand)]
pub(crate) enum PlayersCmd {
    /// Aliases with stats, auto match and decision.
    List,
}

#[derive(Debug, Subcommand)]
pub(crate) enum JobsCmd {
    /// Job history, newest first.
    List {
        #[arg(long, value_name = "N")]
        limit: Option<u32>,
    },
}

#[cfg(test)]
mod tests {
    use clap::CommandFactory;

    use super::*;
    use crate::exit;

    #[test]
    fn clap_tree_is_consistent() {
        Cli::command().debug_assert();
    }

    #[test]
    fn global_flags_after_subcommand() {
        let cli = Cli::try_parse_from(["wolluf", "setup", "status", "--json", "--data-dir", "/d"])
            .unwrap();
        assert!(cli.json);
        assert_eq!(cli.data_dir, Some(PathBuf::from("/d")));
        assert!(matches!(cli.command, Command::Setup(SetupCmd::Status)));
    }

    #[test]
    fn usage_errors_use_the_input_exit_code() {
        let err = Cli::try_parse_from(["wolluf", "--bogus"]).unwrap_err();
        assert_eq!(err.exit_code(), i32::from(exit::USAGE));
    }
}
