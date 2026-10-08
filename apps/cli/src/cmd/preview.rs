//! `wolluf preview skill` over `PreviewService` (ADR 0024). The per-play SSR cache it reads is
//! refreshed by `ComputePlaySsr`, which the app chains after every `IndexLibrary`, so
//! `wolluf library index` (or `wolluf sync`) is how the CLI runs it.

use std::process::ExitCode;

use wolluf_app::context::AppContext;
use wolluf_app::errors::AppError;
use wolluf_app::features::players::dto::{self as players_dto, profile_id};
use wolluf_app::features::players::identity::EntryRef;
use wolluf_app::features::players::scope::MergeMode;
use wolluf_app::features::preview::dto::{PreviewStateDto, SkillPreviewDto, TopPlayDto};
use wolluf_core::UnixUs;

use crate::cli::{MergeArg, PreviewCmd, PreviewSkillArgs, ScopeArg};
use crate::cmd::library::msd;
use crate::cmd::players::rfc3339_ms;
use crate::exit;
use crate::render;

/// The text view lists the best plays only; `--json` keeps every one the service returns.
const TOP_PLAYS_SHOWN: usize = 10;
const MILLI: f64 = 1_000.0;
const PERMYRIAD_PER_PERCENT: f64 = 100.0;
const US_PER_MS: f64 = 1_000.0;
/// `YYYY-MM-DD` of an RFC 3339 timestamp.
const DATE_LEN: usize = 10;

pub(crate) async fn run(ctx: &AppContext, cmd: PreviewCmd, json: bool) -> anyhow::Result<ExitCode> {
    match cmd {
        PreviewCmd::Skill(args) => {
            let previews = skill(ctx, args).await?;
            if json {
                render::json(&previews)?;
            } else {
                let blocks: Vec<String> = previews.iter().map(skill_text).collect();
                render::text(&blocks.join("\n"))?;
            }
        }
    }
    Ok(exit::exit_code(exit::SUCCESS))
}

async fn skill(ctx: &AppContext, args: PreviewSkillArgs) -> Result<Vec<SkillPreviewDto>, AppError> {
    let keymode = players_dto::keymode(u32::from(args.keys))?;
    let entry = entry(ctx, args.scope).await?;
    ctx.preview()
        .skill(entry, keymode, args.merge.map(merge))
        .await
}

async fn entry(ctx: &AppContext, scope: ScopeArg) -> Result<EntryRef, AppError> {
    Ok(match scope {
        ScopeArg::AllPlayers => EntryRef::AllPlayers,
        ScopeArg::Profile(id) => EntryRef::Profile(profile_id(id)),
        ScopeArg::SelfProfile => {
            EntryRef::Profile(ctx.players().self_profile_id().await?.ok_or_else(|| {
                AppError::not_found()
                    .with_arg("scope", "self")
                    .with_details("no self profile yet: run `wolluf sync` first")
            })?)
        }
    })
}

const fn merge(m: MergeArg) -> MergeMode {
    match m {
        MergeArg::Merged => MergeMode::Merged,
        MergeArg::Separate => MergeMode::Separate,
    }
}

/// The DTO's centi values carry no calibration, hence the `≈` on every rating (ADR 0024).
fn skill_text(p: &SkillPreviewDto) -> String {
    let mut out = format!(
        "Skill preview {}K · Beta · uncalibrated · MinaCalc {}\n",
        p.keymode, p.calc_version
    );
    let mut pairs = vec![
        ("scope", p.scope_hash.clone()),
        ("method", p.method.clone()),
        ("state", render::wire(&p.state)),
    ];
    if p.state == PreviewStateDto::Computing {
        pairs.push((
            "note",
            "SSRs are still being computed: run `wolluf library index`".to_owned(),
        ));
    }
    pairs.extend([
        ("overall", render::opt(p.overall_centi.map(approx))),
        (
            "dan",
            render::opt(p.dan.as_ref().map(|d| {
                format!(
                    "≈{} ({}), margin {}",
                    d.label,
                    render::wire(&d.third),
                    msd(d.margin_centi)
                )
            })),
        ),
        (
            "evidence",
            format!(
                "{} counted, tier {}",
                p.evidence.counted,
                render::wire(&p.evidence.tier)
            ),
        ),
        (
            "excluded",
            words(
                p.evidence
                    .excluded
                    .iter()
                    .map(|e| format!("{}={}", e.reason, e.count)),
            ),
        ),
        ("warnings", words(p.warnings.iter().cloned())),
    ]);
    out.push_str(&render::key_values(&pairs));
    if !p.skillsets.is_empty() {
        let rows: Vec<Vec<String>> = p
            .skillsets
            .iter()
            .map(|s| vec![s.id.clone(), approx(s.rating_centi)])
            .collect();
        out.push('\n');
        out.push_str(&render::table(&["SKILLSET", "RATING"], &rows));
    }
    if !p.top_plays.is_empty() {
        out.push('\n');
        out.push_str(&top_plays_table(&p.top_plays));
    }
    if !p.trend.is_empty() {
        let rows: Vec<Vec<String>> = p
            .trend
            .iter()
            .map(|t| vec![t.month.clone(), approx(t.overall_centi)])
            .collect();
        out.push_str("\nTREND\n");
        out.push_str(&render::table(&["MONTH", "OVERALL"], &rows));
    }
    out
}

fn top_plays_table(plays: &[TopPlayDto]) -> String {
    let shown = &plays[..plays.len().min(TOP_PLAYS_SHOWN)];
    let rows: Vec<Vec<String>> = shown
        .iter()
        .enumerate()
        .map(|(i, t)| {
            vec![
                (i + 1).to_string(),
                approx(t.overall_centi),
                goal(t.goal_permyriad),
                rate(t.rate_milli),
                t.dominant_skillset.clone(),
                played_on(t.played_at_ms),
                format!("{} [{}]", t.title, t.version),
            ]
        })
        .collect();
    format!(
        "TOP PLAYS ({} of {})\n{}",
        shown.len(),
        plays.len(),
        render::table(
            &[
                "#", "OVERALL", "GOAL", "RATE", "SKILLSET", "PLAYED", "CHART"
            ],
            &rows
        )
    )
}

fn words(items: impl Iterator<Item = String>) -> String {
    let items: Vec<String> = items.collect();
    if items.is_empty() {
        "-".to_owned()
    } else {
        items.join(" ")
    }
}

fn approx(centi: i32) -> String {
    format!("≈{}", msd(centi))
}

fn rate(milli: u16) -> String {
    format!("{:.2}x", f64::from(milli) / MILLI)
}

fn goal(permyriad: u16) -> String {
    format!("{:.2}%", f64::from(permyriad) / PERMYRIAD_PER_PERCENT)
}

/// UTC date; `as` saturates, and any real timestamp is far inside i64 microseconds.
fn played_on(ms: f64) -> String {
    let mut s = rfc3339_ms(UnixUs((ms * US_PER_MS) as i64));
    s.truncate(DATE_LEN);
    s
}

#[cfg(test)]
mod tests {
    use wolluf_app::features::preview::dto::{
        DanEstimateDto, DanThirdDto, EvidenceDto, EvidenceTierDto, ExclusionCountDto,
        SkillsetRatingDto, TrendPointDto,
    };

    use super::*;

    const SKILLSETS: [&str; 7] = [
        "stream",
        "jumpstream",
        "handstream",
        "stamina",
        "jackspeed",
        "chordjack",
        "technical",
    ];

    fn top(n: usize) -> TopPlayDto {
        let n16 = u16::try_from(n).unwrap();
        let n32 = i32::try_from(n).unwrap();
        TopPlayDto {
            play_id: format!("{n:064x}"),
            md5: format!("{n:032x}"),
            title: format!("Song {n}"),
            version: "Hard".to_owned(),
            rate_milli: 1_000 + 50 * n16,
            goal_permyriad: 9_650 - n16,
            overall_centi: 2_800 - n32,
            dominant_skillset: "stream".to_owned(),
            // 2026-09-28T23:13:56.636Z
            played_at_ms: 1_790_637_236_636.0,
        }
    }

    fn ready() -> SkillPreviewDto {
        SkillPreviewDto {
            scope_hash: "ab".repeat(32),
            keymode: 4,
            method: "preview.etterna_rating@1".to_owned(),
            calc_version: 527,
            state: PreviewStateDto::Ready,
            overall_centi: Some(2_413),
            skillsets: SKILLSETS
                .iter()
                .zip([2_310, 2_605, 2_401, 2_250, 2_480, 2_399, 2_444])
                .map(|(id, c)| SkillsetRatingDto {
                    id: (*id).to_owned(),
                    rating_centi: c,
                })
                .collect(),
            dan: Some(DanEstimateDto {
                label: "Gamma".to_owned(),
                third: DanThirdDto::Mid,
                margin_centi: 42,
            }),
            evidence: EvidenceDto {
                counted: 37,
                tier: EvidenceTierDto::Ok,
                excluded: vec![
                    ExclusionCountDto {
                        reason: "incomplete".to_owned(),
                        count: 3,
                    },
                    ExclusionCountDto {
                        reason: "score_v2".to_owned(),
                        count: 1,
                    },
                ],
            },
            top_plays: (0..12).map(top).collect(),
            trend: vec![
                TrendPointDto {
                    month: "2026-08".to_owned(),
                    overall_centi: 2_200,
                },
                TrendPointDto {
                    month: "2026-09".to_owned(),
                    overall_centi: 2_413,
                },
            ],
            warnings: vec!["uncalibrated".to_owned(), "goal_estimated".to_owned()],
        }
    }

    #[test]
    fn ready_text_shows_every_section_and_the_top_ten() {
        let text = skill_text(&ready());
        let want = format!(
            "\
Skill preview 4K · Beta · uncalibrated · MinaCalc 527
scope     {scope}
method    preview.etterna_rating@1
state     ready
overall   ≈24.13
dan       ≈Gamma (mid), margin 0.42
evidence  37 counted, tier ok
excluded  incomplete=3 score_v2=1
warnings  uncalibrated goal_estimated

SKILLSET    RATING
stream      ≈23.10
jumpstream  ≈26.05
handstream  ≈24.01
stamina     ≈22.50
jackspeed   ≈24.80
chordjack   ≈23.99
technical   ≈24.44

TOP PLAYS (10 of 12)
#   OVERALL  GOAL    RATE   SKILLSET  PLAYED      CHART
1   ≈28.00   96.50%  1.00x  stream    2026-09-28  Song 0 [Hard]
2   ≈27.99   96.49%  1.05x  stream    2026-09-28  Song 1 [Hard]
3   ≈27.98   96.48%  1.10x  stream    2026-09-28  Song 2 [Hard]
4   ≈27.97   96.47%  1.15x  stream    2026-09-28  Song 3 [Hard]
5   ≈27.96   96.46%  1.20x  stream    2026-09-28  Song 4 [Hard]
6   ≈27.95   96.45%  1.25x  stream    2026-09-28  Song 5 [Hard]
7   ≈27.94   96.44%  1.30x  stream    2026-09-28  Song 6 [Hard]
8   ≈27.93   96.43%  1.35x  stream    2026-09-28  Song 7 [Hard]
9   ≈27.92   96.42%  1.40x  stream    2026-09-28  Song 8 [Hard]
10  ≈27.91   96.41%  1.45x  stream    2026-09-28  Song 9 [Hard]

TREND
MONTH    OVERALL
2026-08  ≈22.00
2026-09  ≈24.13
",
            scope = "ab".repeat(32)
        );
        assert_eq!(text, want);
    }

    #[test]
    fn computing_and_empty_states_say_what_to_do() {
        let mut p = ready();
        p.state = PreviewStateDto::Computing;
        let text = skill_text(&p);
        assert!(text.contains("state     computing\n"), "{text}");
        assert!(
            text.contains("note      SSRs are still being computed: run `wolluf library index`\n"),
            "{text}"
        );

        let empty = SkillPreviewDto {
            state: PreviewStateDto::NoPlays,
            overall_centi: None,
            skillsets: vec![],
            dan: None,
            evidence: EvidenceDto {
                counted: 0,
                tier: EvidenceTierDto::Low,
                excluded: vec![],
            },
            top_plays: vec![],
            trend: vec![],
            keymode: 7,
            warnings: vec!["uncalibrated".to_owned(), "k7_less_validated".to_owned()],
            ..ready()
        };
        let want = format!(
            "\
Skill preview 7K · Beta · uncalibrated · MinaCalc 527
scope     {scope}
method    preview.etterna_rating@1
state     no_plays
overall   -
dan       -
evidence  0 counted, tier low
excluded  -
warnings  uncalibrated k7_less_validated
",
            scope = "ab".repeat(32)
        );
        assert_eq!(skill_text(&empty), want);
    }

    #[test]
    fn cells() {
        assert_eq!(approx(2_413), "≈24.13");
        assert_eq!(approx(5), "≈0.05");
        assert_eq!(rate(1_000), "1.00x");
        assert_eq!(rate(1_500), "1.50x");
        assert_eq!(rate(750), "0.75x");
        assert_eq!(goal(9_650), "96.50%");
        assert_eq!(goal(10_000), "100.00%");
        assert_eq!(played_on(1_790_637_236_636.0), "2026-09-28");
        assert_eq!(played_on(0.0), "1970-01-01");
        assert_eq!(merge(MergeArg::Separate), MergeMode::Separate);
    }
}
