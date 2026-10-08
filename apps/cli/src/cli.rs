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
    /// The indexed chart library.
    #[command(subcommand)]
    Library(LibraryCmd),
    /// One indexed chart.
    #[command(subcommand)]
    Chart(ChartCmd),
    /// Blind gold-set labelling: label sampled chart windows by pattern (interactive, stdin).
    Label(LabelArgs),
    /// Uncalibrated previews (ADR 0024); `wolluf library index` refreshes their per-play cache.
    #[command(subcommand)]
    Preview(PreviewCmd),
    /// Rate-edited copies written into the chart's set folder (ADR 0025).
    #[command(subcommand)]
    RateCopy(RateCopyCmd),
    /// `.osg` spike tools: they only read the given files and never open the data dir.
    #[command(subcommand)]
    Osg(OsgCmd),
}

#[derive(Debug, Subcommand)]
pub(crate) enum RateCopyCmd {
    /// Names, existing files and refusals of a copy; writes nothing.
    Plan(RateCopyArgs),
    /// Plan, confirm and wait for the job that writes the copy's .osu and .ogg.
    Create(RateCopyCreateArgs),
}

#[derive(Debug, Args)]
pub(crate) struct RateCopyArgs {
    #[arg(value_name = "MD5")]
    pub(crate) md5: String,
    /// Music rate, e.g. 1.15 (at most three decimals).
    #[arg(long, value_name = "RATE", value_parser = parse_rate_milli)]
    pub(crate) rate: u16,
}

#[derive(Debug, Args)]
pub(crate) struct RateCopyCreateArgs {
    #[command(flatten)]
    pub(crate) copy: RateCopyArgs,
    /// Confirms the write into the osu! Songs folder.
    #[arg(long, required = true)]
    pub(crate) yes: bool,
}

/// `1.15` -> 1150, in decimal so no float rounding picks the neighbouring rate.
fn parse_rate_milli(text: &str) -> Result<u16, String> {
    const MILLI_DIGITS: usize = 3;
    let invalid = || format!("`{text}` is not a rate like 1.15");
    let (int, frac) = text.trim().split_once('.').unwrap_or((text.trim(), ""));
    if int.is_empty()
        || frac.len() > MILLI_DIGITS
        || !int.bytes().chain(frac.bytes()).all(|b| b.is_ascii_digit())
    {
        return Err(invalid());
    }
    let digits = format!("{int}{frac:0<MILLI_DIGITS$}");
    digits.parse::<u16>().map_err(|_| invalid())
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
pub(crate) enum LibraryCmd {
    /// Parse and label the catalog's charts and wait for it and the jobs it chains (the skill
    /// preview's per-play SSRs); Ctrl-C cancels.
    Index,
    /// Indexed charts of one keymode, filtered by label or text.
    List(LibraryListArgs),
    /// Label rows and charts per scale.
    Scales,
    /// Primary pattern segments per pattern.
    Patterns,
    /// Per name-hint target: segmented share in hinted charts vs the library, and the lift.
    Hints,
}

#[derive(Debug, Args)]
pub(crate) struct LibraryListArgs {
    #[arg(long, value_name = "N", default_value_t = 7)]
    pub(crate) keys: u8,
    /// Only charts with a label of this scale.
    #[arg(long, value_name = "SCALE")]
    pub(crate) scale: Option<String>,
    /// Inclusive lower bound on the label level.
    #[arg(long, value_name = "X")]
    pub(crate) level_min: Option<f64>,
    /// Inclusive upper bound on the label level.
    #[arg(long, value_name = "Y")]
    pub(crate) level_max: Option<f64>,
    /// Only charts with a label from this source.
    #[arg(long, value_name = "SRC")]
    pub(crate) source: Option<String>,
    /// Case-insensitive substring of title, artist, difficulty name or creator.
    #[arg(long, value_name = "T")]
    pub(crate) text: Option<String>,
    #[arg(long, value_name = "N", default_value_t = 50)]
    pub(crate) limit: u32,
    #[arg(long, value_name = "N", default_value_t = 0)]
    pub(crate) offset: u32,
}

#[derive(Debug, Subcommand)]
pub(crate) enum PreviewCmd {
    /// Overall, skillsets, dan (4K), evidence, top plays and trend of one player scope.
    Skill(PreviewSkillArgs),
    /// Charts × rate in a band around the scope's preview rating, with a reason per pick.
    Recs(PreviewRecsArgs),
}

#[derive(Debug, Args)]
pub(crate) struct PreviewRecsArgs {
    #[arg(long, value_name = "N", default_value_t = 4)]
    pub(crate) keys: u8,
    #[arg(long, value_enum, value_name = "MODE", default_value = "deficit")]
    pub(crate) mode: RecsModeArg,
    /// A MinaCalc skillset id (`stream`, `jumpstream`, …); required by `--mode skillset`.
    #[arg(long, value_name = "ID", required_if_eq("mode", "skillset"))]
    pub(crate) skillset: Option<String>,
    /// `self`, `all` (every player) or `p:<profile id>`. Separate merge lists the first scope.
    #[arg(long, value_name = "SCOPE", default_value = "self", value_parser = parse_scope)]
    pub(crate) scope: ScopeArg,
    /// Overrides the profile's stored merge mode.
    #[arg(long, value_enum, value_name = "MODE")]
    pub(crate) merge: Option<MergeArg>,
    /// Every grid rate for this call only; the stored `preview.recs.any_rate` setting is neither
    /// read nor changed. Without it the setting decides.
    #[arg(long)]
    pub(crate) any_rate: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum RecsModeArg {
    /// The weakest skillset the keymode lets Deficit pick.
    Deficit,
    /// Overall, aimed above the player.
    Push,
    /// The skillset `--skillset` names.
    Skillset,
}

#[derive(Debug, Args)]
pub(crate) struct PreviewSkillArgs {
    /// 4K is the keymode the preview is validated on and the only one with a dan table.
    #[arg(long, value_name = "N", default_value_t = 4)]
    pub(crate) keys: u8,
    /// `self`, `all` (every player) or `p:<profile id>`.
    #[arg(long, value_name = "SCOPE", default_value = "self", value_parser = parse_scope)]
    pub(crate) scope: ScopeArg,
    /// Overrides the profile's stored merge mode.
    #[arg(long, value_enum, value_name = "MODE")]
    pub(crate) merge: Option<MergeArg>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ScopeArg {
    SelfProfile,
    AllPlayers,
    Profile(u32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum MergeArg {
    Merged,
    Separate,
}

const PROFILE_SCOPE_PREFIX: &str = "p:";

fn parse_scope(s: &str) -> Result<ScopeArg, String> {
    match s {
        "self" => Ok(ScopeArg::SelfProfile),
        "all" => Ok(ScopeArg::AllPlayers),
        _ => s
            .strip_prefix(PROFILE_SCOPE_PREFIX)
            .and_then(|id| id.parse::<u32>().ok())
            .map(ScopeArg::Profile)
            .ok_or_else(|| format!("`{s}` is not a scope: use `self`, `all` or `p:<profile id>`")),
    }
}

#[derive(Debug, Subcommand)]
pub(crate) enum ChartCmd {
    /// Print an ASCII playfield of one time window.
    Show(ChartShowArgs),
    /// Print the chart's metadata, summary, labels, pattern segments and MinaCalc MSD per rate.
    Info {
        #[arg(value_name = "MD5")]
        md5: String,
    },
}

/// Without `--to` the window is 20 s long, so a whole chart never floods the terminal.
const DEFAULT_WINDOW_MS: i32 = 20_000;

#[derive(Debug, Args)]
pub(crate) struct ChartShowArgs {
    #[arg(value_name = "MD5")]
    pub(crate) md5: String,
    /// Window start, in seconds (`30`, `30.5`) or `mm:ss[.fff]`.
    #[arg(long, value_name = "TIME", default_value = "0", value_parser = parse_time_ms)]
    pub(crate) from: i32,
    /// Window end, exclusive, same format; defaults to 20 s after `--from`.
    #[arg(long, value_name = "TIME", value_parser = parse_time_ms)]
    pub(crate) to: Option<i32>,
    /// Layout preset id (e.g. `k7.313_left_thumb`); defaults to the keymode's layout.
    #[arg(long, value_name = "ID")]
    pub(crate) layout: Option<String>,
    /// Name each row's pattern segment (default layout) and print a legend.
    #[arg(long)]
    pub(crate) segments: bool,
}

impl ChartShowArgs {
    /// `(from_ms, to_ms)`.
    pub(crate) fn window(&self) -> (i32, i32) {
        let to = self
            .to
            .unwrap_or_else(|| self.from.saturating_add(DEFAULT_WINDOW_MS));
        (self.from, to)
    }
}

const MS_PER_SECOND: f64 = 1_000.0;
const SECONDS_PER_MINUTE: f64 = 60.0;

/// `ss[.fff]` or `mm:ss[.fff]` to whole milliseconds, rounded. Input parsing only: the window
/// itself is validated by the app.
pub(crate) fn parse_time_ms(s: &str) -> Result<i32, String> {
    let bad = || format!("`{s}` is not a time: use seconds (`30.5`) or `mm:ss[.fff]`");
    let number = |part: &str| -> Result<f64, String> {
        // `f64::from_str` also takes `inf`, `nan`, `1e3` and signs, none of which is a time.
        if part.is_empty() || !part.chars().all(|c| c.is_ascii_digit() || c == '.') {
            return Err(bad());
        }
        part.parse::<f64>().map_err(|_| bad())
    };
    let s = s.trim();
    let seconds = match s.split_once(':') {
        None => number(s)?,
        Some((min, sec)) => {
            if !min.chars().all(|c| c.is_ascii_digit()) {
                return Err(bad());
            }
            let sec = number(sec)?;
            if sec >= SECONDS_PER_MINUTE {
                return Err(bad());
            }
            number(min)? * SECONDS_PER_MINUTE + sec
        }
    };
    let ms = (seconds * MS_PER_SECOND).round();
    if ms > f64::from(i32::MAX) {
        return Err(bad());
    }
    // Non-negative and at most i32::MAX by the checks above, so the cast is exact.
    Ok(ms as i32)
}

/// Without a subcommand, runs the labelling session.
#[derive(Debug, Args)]
#[command(args_conflicts_with_subcommands = true)]
pub(crate) struct LabelArgs {
    #[command(subcommand)]
    pub(crate) command: Option<LabelCmd>,
    #[command(flatten)]
    pub(crate) session: LabelSessionArgs,
}

#[derive(Debug, Args)]
pub(crate) struct LabelSessionArgs {
    /// Sampling seed; defaults to the current time and is printed, so a session can be replayed.
    #[arg(long, value_name = "N")]
    pub(crate) seed: Option<u64>,
    /// Window length, in seconds (`4`, `2.5`) or `mm:ss[.fff]`.
    #[arg(
        long = "window",
        value_name = "SECS",
        default_value = "4",
        value_parser = parse_window_ms
    )]
    pub(crate) window_ms: u32,
    #[arg(long, value_name = "N", default_value_t = 7)]
    pub(crate) keys: u8,
    /// Only charts with a label of this scale.
    #[arg(long, value_name = "S")]
    pub(crate) scale: Option<String>,
    /// Inclusive lower bound on the label level.
    #[arg(long, value_name = "X")]
    pub(crate) level_min: Option<f64>,
    /// Inclusive upper bound on the label level.
    #[arg(long, value_name = "Y")]
    pub(crate) level_max: Option<f64>,
}

/// Relative to the working directory, so running from the repo root lands in the committed
/// fixture folder (architecture §3 `fixtures/labels/`).
const DEFAULT_LABEL_EXPORT: &str = "fixtures/labels/gold-7k.jsonl";

#[derive(Debug, Subcommand)]
pub(crate) enum LabelCmd {
    /// Counts of the labelled set per pattern, axis, stratum and flag.
    Stats,
    /// Write the gold set as JSONL: anchors and pattern ids only, sorted by (md5, t0).
    Export {
        #[arg(long, value_name = "PATH", default_value = DEFAULT_LABEL_EXPORT)]
        out: PathBuf,
    },
}

fn parse_window_ms(s: &str) -> Result<u32, String> {
    let ms = parse_time_ms(s)?;
    u32::try_from(ms)
        .ok()
        .filter(|&ms| ms > 0)
        .ok_or_else(|| format!("`{s}` is not a window: it must be longer than 0 s"))
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
    fn rates_parse_in_thousandths() {
        for (text, milli) in [("1.15", 1150), ("1", 1000), ("0.7", 700), ("1.155", 1155)] {
            assert_eq!(parse_rate_milli(text), Ok(milli), "{text}");
        }
        for bad in ["", ".5", "1.1555", "1,15", "-1.1", "x", "70.000"] {
            assert!(parse_rate_milli(bad).is_err(), "{bad:?}");
        }
        let cli = Cli::try_parse_from(["wolluf", "rate-copy", "create", "abc", "--rate", "1.1"]);
        assert!(cli.is_err(), "create needs --yes");
        let cli = Cli::try_parse_from([
            "wolluf",
            "rate-copy",
            "create",
            "abc",
            "--rate",
            "1.1",
            "--yes",
        ])
        .unwrap();
        let Command::RateCopy(RateCopyCmd::Create(args)) = cli.command else {
            panic!("{:?}", cli.command);
        };
        assert_eq!((args.copy.md5.as_str(), args.copy.rate), ("abc", 1100));
    }

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
    fn time_accepts_seconds_and_minutes() {
        assert_eq!(parse_time_ms("0"), Ok(0));
        assert_eq!(parse_time_ms("30"), Ok(30_000));
        assert_eq!(parse_time_ms("30.25"), Ok(30_250));
        assert_eq!(parse_time_ms("1:05"), Ok(65_000));
        assert_eq!(parse_time_ms("01:05.5"), Ok(65_500));
        assert_eq!(parse_time_ms(" 2:00 "), Ok(120_000));
        assert_eq!(parse_time_ms("0.0004"), Ok(0));
        assert_eq!(parse_time_ms("0.0005"), Ok(1));
    }

    #[test]
    fn time_rejects_malformed_input() {
        for bad in [
            "", "abc", "-1", "1:60", "1:-5", ":30", "1:", "1:2:3", "nan", "inf", "1e3", "99999999",
        ] {
            assert!(parse_time_ms(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn library_list_defaults() {
        let cli = Cli::try_parse_from(["wolluf", "library", "list"]).unwrap();
        let Command::Library(LibraryCmd::List(args)) = cli.command else {
            panic!("{:?}", cli.command);
        };
        assert_eq!((args.keys, args.limit, args.offset), (7, 50, 0));
        assert_eq!(args.scale, None);
        assert_eq!((args.level_min, args.level_max), (None, None));
        assert_eq!((args.source, args.text), (None, None));
    }

    #[test]
    fn library_list_filters() {
        let cli = Cli::try_parse_from([
            "wolluf",
            "library",
            "list",
            "--keys",
            "4",
            "--scale",
            "satellite",
            "--level-min",
            "1.5",
            "--level-max",
            "3",
            "--source",
            "bms",
            "--text",
            "x",
            "--limit",
            "5",
            "--offset",
            "10",
        ])
        .unwrap();
        let Command::Library(LibraryCmd::List(args)) = cli.command else {
            panic!("{:?}", cli.command);
        };
        assert_eq!((args.keys, args.limit, args.offset), (4, 5, 10));
        assert_eq!(args.scale.as_deref(), Some("satellite"));
        assert_eq!((args.level_min, args.level_max), (Some(1.5), Some(3.0)));
        assert_eq!(args.source.as_deref(), Some("bms"));
        assert_eq!(args.text.as_deref(), Some("x"));
    }

    #[test]
    fn chart_show_window_defaults_to_the_first_20_seconds() {
        let cli = Cli::try_parse_from(["wolluf", "chart", "show", "abc"]).unwrap();
        let Command::Chart(ChartCmd::Show(args)) = cli.command else {
            panic!("{:?}", cli.command);
        };
        assert_eq!(args.md5, "abc");
        assert_eq!(args.window(), (0, 20_000));
        assert_eq!(args.layout, None);

        let cli = Cli::try_parse_from([
            "wolluf", "chart", "show", "abc", "--from", "1:00", "--layout", "k7.43",
        ])
        .unwrap();
        let Command::Chart(ChartCmd::Show(args)) = cli.command else {
            panic!("{:?}", cli.command);
        };
        assert_eq!(args.window(), (60_000, 80_000));
        assert_eq!(args.layout.as_deref(), Some("k7.43"));

        let cli = Cli::try_parse_from(["wolluf", "chart", "show", "abc", "--to", "0:05"]).unwrap();
        let Command::Chart(ChartCmd::Show(args)) = cli.command else {
            panic!("{:?}", cli.command);
        };
        assert_eq!(args.window(), (0, 5_000));
    }

    #[test]
    fn chart_show_segments_flag_and_library_patterns() {
        let cli = Cli::try_parse_from(["wolluf", "chart", "show", "abc", "--segments"]).unwrap();
        let Command::Chart(ChartCmd::Show(args)) = cli.command else {
            panic!("{:?}", cli.command);
        };
        assert!(args.segments);
        let cli = Cli::try_parse_from(["wolluf", "chart", "show", "abc"]).unwrap();
        let Command::Chart(ChartCmd::Show(args)) = cli.command else {
            panic!("{:?}", cli.command);
        };
        assert!(!args.segments);
        let cli = Cli::try_parse_from(["wolluf", "library", "patterns"]).unwrap();
        assert!(matches!(
            cli.command,
            Command::Library(LibraryCmd::Patterns)
        ));
    }

    #[test]
    fn chart_show_rejects_a_bad_time() {
        let err =
            Cli::try_parse_from(["wolluf", "chart", "show", "abc", "--from", "x"]).unwrap_err();
        assert_eq!(err.exit_code(), i32::from(exit::USAGE));
    }

    #[test]
    fn label_session_defaults() {
        let cli = Cli::try_parse_from(["wolluf", "label"]).unwrap();
        let Command::Label(args) = cli.command else {
            panic!("{:?}", cli.command);
        };
        assert!(args.command.is_none());
        let s = args.session;
        assert_eq!((s.seed, s.window_ms, s.keys), (None, 4_000, 7));
        assert_eq!((s.scale, s.level_min, s.level_max), (None, None, None));
    }

    #[test]
    fn label_session_flags() {
        let cli = Cli::try_parse_from([
            "wolluf",
            "label",
            "--seed",
            "18446744073709551615",
            "--window",
            "2.5",
            "--scale",
            "jinjin_dan_regular",
            "--level-min",
            "3",
            "--level-max",
            "7.5",
        ])
        .unwrap();
        let Command::Label(args) = cli.command else {
            panic!("{:?}", cli.command);
        };
        let s = args.session;
        assert_eq!((s.seed, s.window_ms), (Some(u64::MAX), 2_500));
        assert_eq!(s.scale.as_deref(), Some("jinjin_dan_regular"));
        assert_eq!((s.level_min, s.level_max), (Some(3.0), Some(7.5)));
        for bad in ["0", "x", "-1"] {
            assert!(
                Cli::try_parse_from(["wolluf", "label", "--window", bad]).is_err(),
                "{bad}"
            );
        }
    }

    #[test]
    fn label_subcommands() {
        let cli = Cli::try_parse_from(["wolluf", "label", "stats", "--json"]).unwrap();
        assert!(cli.json);
        let Command::Label(args) = cli.command else {
            panic!("{:?}", cli.command);
        };
        assert!(matches!(args.command, Some(LabelCmd::Stats)));

        let cli = Cli::try_parse_from(["wolluf", "label", "export"]).unwrap();
        let Command::Label(LabelArgs {
            command: Some(LabelCmd::Export { out }),
            ..
        }) = cli.command
        else {
            panic!("{:?}", cli.command);
        };
        assert_eq!(out, PathBuf::from("fixtures/labels/gold-7k.jsonl"));
        let cli = Cli::try_parse_from(["wolluf", "label", "export", "--out", "/x.jsonl"]).unwrap();
        let Command::Label(LabelArgs {
            command: Some(LabelCmd::Export { out }),
            ..
        }) = cli.command
        else {
            panic!("{:?}", cli.command);
        };
        assert_eq!(out, PathBuf::from("/x.jsonl"));
        assert!(Cli::try_parse_from(["wolluf", "label", "--seed", "1", "stats"]).is_err());
    }

    #[test]
    fn preview_skill_defaults_to_self_4k() {
        let cli = Cli::try_parse_from(["wolluf", "preview", "skill"]).unwrap();
        let Command::Preview(PreviewCmd::Skill(args)) = cli.command else {
            panic!("{:?}", cli.command);
        };
        assert_eq!(
            (args.keys, args.scope, args.merge),
            (4, ScopeArg::SelfProfile, None)
        );
    }

    #[test]
    fn preview_skill_flags() {
        for (scope, want) in [
            ("self", ScopeArg::SelfProfile),
            ("all", ScopeArg::AllPlayers),
            ("p:12", ScopeArg::Profile(12)),
        ] {
            let cli = Cli::try_parse_from([
                "wolluf", "preview", "skill", "--keys", "7", "--scope", scope, "--merge",
                "separate",
            ])
            .unwrap();
            let Command::Preview(PreviewCmd::Skill(args)) = cli.command else {
                panic!("{:?}", cli.command);
            };
            assert_eq!(
                (args.keys, args.scope, args.merge),
                (7, want, Some(MergeArg::Separate))
            );
        }
        for bad in ["", "me", "p:", "p:x", "p:-1", "P:1", "12"] {
            let err =
                Cli::try_parse_from(["wolluf", "preview", "skill", "--scope", bad]).unwrap_err();
            assert_eq!(err.exit_code(), i32::from(exit::USAGE), "{bad:?}");
        }
        assert!(Cli::try_parse_from(["wolluf", "preview", "skill", "--merge", "x"]).is_err());
    }

    #[test]
    fn preview_recs_defaults_to_self_4k_deficit_with_stored_rates() {
        let cli = Cli::try_parse_from(["wolluf", "preview", "recs"]).unwrap();
        let Command::Preview(PreviewCmd::Recs(args)) = cli.command else {
            panic!("{:?}", cli.command);
        };
        assert_eq!(
            (
                args.keys,
                args.mode,
                args.skillset,
                args.scope,
                args.merge,
                args.any_rate
            ),
            (
                4,
                RecsModeArg::Deficit,
                None,
                ScopeArg::SelfProfile,
                None,
                false
            )
        );
    }

    #[test]
    fn preview_recs_flags() {
        let cli = Cli::try_parse_from([
            "wolluf",
            "preview",
            "recs",
            "--keys",
            "7",
            "--mode",
            "skillset",
            "--skillset",
            "stream",
            "--scope",
            "all",
            "--merge",
            "separate",
            "--any-rate",
        ])
        .unwrap();
        let Command::Preview(PreviewCmd::Recs(args)) = cli.command else {
            panic!("{:?}", cli.command);
        };
        assert_eq!(
            (
                args.keys,
                args.mode,
                args.skillset.as_deref(),
                args.scope,
                args.merge,
                args.any_rate
            ),
            (
                7,
                RecsModeArg::Skillset,
                Some("stream"),
                ScopeArg::AllPlayers,
                Some(MergeArg::Separate),
                true
            )
        );
        for mode in ["deficit", "push"] {
            assert!(Cli::try_parse_from(["wolluf", "preview", "recs", "--mode", mode]).is_ok());
        }
        for bad in [
            &["--mode", "x"][..],
            &["--mode", "skillset"],
            &["--scope", "me"],
        ] {
            let err =
                Cli::try_parse_from(["wolluf", "preview", "recs"].iter().chain(bad)).unwrap_err();
            assert_eq!(err.exit_code(), i32::from(exit::USAGE), "{bad:?}");
        }
    }

    #[test]
    fn usage_errors_use_the_input_exit_code() {
        let err = Cli::try_parse_from(["wolluf", "--bogus"]).unwrap_err();
        assert_eq!(err.exit_code(), i32::from(exit::USAGE));
    }
}
