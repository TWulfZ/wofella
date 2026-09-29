//! The clap tree (spec 005 Behaviour, "CLI"; spec 006 adds `osg`).

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

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
    /// `.osg` spike tools: they only read the given files and never open the data dir.
    #[command(subcommand)]
    Osg(OsgCmd),
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

#[derive(Debug, Subcommand)]
pub(crate) enum OsgCmd {
    /// Decode one `.osg` file.
    Dump(OsgDumpArgs),
    /// Check the structural invariants over every `.osg` in `<ROOT>/Data/r`.
    Survey(OsgSurveyArgs),
}

#[derive(Debug, Args)]
pub(crate) struct OsgDumpArgs {
    #[arg(value_name = "PATH")]
    pub(crate) path: PathBuf,
    /// The global `--json` is a shorthand for `--format json`.
    #[arg(long, value_enum, default_value_t = OsgFormat::Table)]
    pub(crate) format: OsgFormat,
    /// Show at most N rows; the header and diagnostics are always complete.
    #[arg(long, value_name = "N")]
    pub(crate) limit: Option<usize>,
    /// Print the derived judgement events instead of the raw records.
    #[arg(long)]
    pub(crate) events: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum OsgFormat {
    Table,
    Json,
    Csv,
}

#[derive(Debug, Args)]
pub(crate) struct OsgSurveyArgs {
    /// The osu! stable folder; it is only ever read.
    #[arg(long, value_name = "ROOT")]
    pub(crate) corpus: PathBuf,
    /// Exit 1 when an invariant fails outside the listed exception classes.
    #[arg(long)]
    pub(crate) strict: bool,
    /// Survey only the first N `.osg` files, in `Data/r` order.
    #[arg(long, value_name = "N")]
    pub(crate) max_files: Option<usize>,
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
    fn osg_dump_defaults_to_table_records() {
        let cli = Cli::try_parse_from(["wolluf", "osg", "dump", "a.osg"]).unwrap();
        let Command::Osg(OsgCmd::Dump(args)) = cli.command else {
            panic!("{:?}", cli.command);
        };
        assert_eq!(args.path, PathBuf::from("a.osg"));
        assert_eq!(args.format, OsgFormat::Table);
        assert_eq!((args.limit, args.events), (None, false));
    }

    #[test]
    fn osg_survey_requires_corpus() {
        assert!(Cli::try_parse_from(["wolluf", "osg", "survey"]).is_err());
        let cli = Cli::try_parse_from([
            "wolluf",
            "osg",
            "survey",
            "--corpus",
            "/c",
            "--strict",
            "--max-files",
            "3",
        ])
        .unwrap();
        let Command::Osg(OsgCmd::Survey(args)) = cli.command else {
            panic!("{:?}", cli.command);
        };
        assert_eq!(args.corpus, PathBuf::from("/c"));
        assert!(args.strict);
        assert_eq!(args.max_files, Some(3));
    }

    #[test]
    fn usage_errors_use_the_input_exit_code() {
        let err = Cli::try_parse_from(["wolluf", "--bogus"]).unwrap_err();
        assert_eq!(err.exit_code(), i32::from(exit::USAGE));
    }
}
