//! `wolluf chart show|info` over `LibraryService::render`, `get` and `chart_msd` (F1).

use std::process::ExitCode;

use serde::Serialize;
use wolluf_app::context::AppContext;
use wolluf_app::features::library::dto::{
    ChartDetailDto, ChartLabelDto, ChartMsdDto, MsdStatusDto, SegmentDto,
};

use crate::cli::ChartCmd;
use crate::cmd::label::clock;
use crate::cmd::library::{duration, label, ln_percent, msd};
use crate::exit;
use crate::render;

pub(crate) async fn run(ctx: &AppContext, cmd: ChartCmd, json: bool) -> anyhow::Result<ExitCode> {
    match cmd {
        ChartCmd::Show(args) => {
            let (from_ms, to_ms) = args.window();
            let text = ctx
                .library()
                .render(
                    &args.md5,
                    from_ms,
                    to_ms,
                    args.layout.as_deref(),
                    args.segments,
                )
                .await?;
            if json {
                render::json(&text)?;
            } else {
                render::text(&text)?;
            }
        }
        ChartCmd::Info { md5 } => {
            let info = ChartInfo {
                detail: ctx.library().get(&md5).await?,
                msd: ctx.library().chart_msd(&md5).await?,
            };
            if json {
                render::json(&info)?;
            } else {
                render::text(&info_text(&info))?;
            }
        }
    }
    Ok(exit::exit_code(exit::SUCCESS))
}

/// The detail's keys stay at the top level, so `--json` only gains `msd`.
#[derive(Serialize)]
struct ChartInfo {
    #[serde(flatten)]
    detail: ChartDetailDto,
    msd: ChartMsdDto,
}

const PERMILLE_PER_PERCENT: f64 = 10.0;
const MILLI: f64 = 1_000.0;

fn info_text(info: &ChartInfo) -> String {
    let m = &info.msd;
    let mut pairs = detail_pairs(&info.detail);
    pairs.extend([
        ("msd", render::wire(&m.status)),
        ("calc version", m.calc_version.to_string()),
        (
            "hold share",
            format!(
                "{:.1}%",
                f64::from(m.hold_share_permille) / PERMILLE_PER_PERCENT
            ),
        ),
    ]);
    let mut text = render::key_values(&pairs);
    if m.status == MsdStatusDto::Rated {
        text.push('\n');
        text.push_str(&msd_table(m));
    }
    text
}

/// One row per rate, one column per skillset in the DTO's order.
fn msd_table(m: &ChartMsdDto) -> String {
    let headers: Vec<String> = std::iter::once("RATE".to_owned())
        .chain(m.skillsets.iter().map(|s| s.to_uppercase()))
        .collect();
    let headers: Vec<&str> = headers.iter().map(String::as_str).collect();
    let rows: Vec<Vec<String>> = m
        .rates
        .iter()
        .map(|r| {
            std::iter::once(format!("{:.2}", f64::from(r.rate_milli) / MILLI))
                .chain(r.centi.iter().map(|&c| msd(c)))
                .collect()
        })
        .collect();
    render::table(&headers, &rows)
}

fn label_line(l: &ChartLabelDto) -> String {
    let mut line = format!("{} {}", l.source, label(l));
    if let Some(tag) = &l.skill_tag {
        line.push_str(&format!(" ({tag})"));
    }
    line
}

fn detail_pairs(d: &ChartDetailDto) -> Vec<(&'static str, String)> {
    let c = &d.chart;
    let mut pairs = vec![
        ("md5", c.md5.clone()),
        ("title", c.title.clone()),
        ("artist", c.artist.clone()),
        ("version", c.version.clone()),
        ("creator", c.creator.clone()),
        ("keys", c.keymode.to_string()),
        ("notes", c.n_notes.to_string()),
        ("ln", c.n_ln.to_string()),
        ("ln%", ln_percent(c.ln_ratio)),
        ("length", duration(c.length_ms)),
        ("nps", format!("{:.2}", c.nps)),
        ("diagnostics", render::opt(d.diagnostics)),
    ];
    if c.labels.is_empty() {
        pairs.push(("labels", "-".to_owned()));
    }
    pairs.extend(c.labels.iter().map(|l| ("label", label_line(l))));
    if d.segments.is_empty() {
        pairs.push(("segments", "-".to_owned()));
    }
    pairs.extend(d.segments.iter().map(|s| ("segment", segment_line(s))));
    pairs
}

/// `t0-t1 key pattern purity strength [+secondary,…]`.
fn segment_line(s: &SegmentDto) -> String {
    let mut line = format!(
        "{}-{} {} {} purity {} strength {}",
        clock(s.t0_ms),
        clock(s.t1_ms),
        s.key,
        s.pattern_id,
        s.purity,
        s.strength
    );
    if !s.secondary.is_empty() {
        line.push_str(&format!(" +{}", s.secondary.join(",")));
    }
    line
}
