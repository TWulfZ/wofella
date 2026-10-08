//! `wolluf preview skill` and `wolluf preview recs` over `PreviewService` (ADR 0024). The per-play
//! SSR cache they read is refreshed by `ComputePlaySsr`, which the app chains after every
//! `IndexLibrary` and the preview starts itself when plays were never rated under the current
//! key; the CLI waits for that run.

use std::process::ExitCode;

use wolluf_app::context::AppContext;
use wolluf_app::errors::AppError;
use wolluf_app::features::players::dto::{self as players_dto, profile_id};
use wolluf_app::features::players::identity::EntryRef;
use wolluf_app::features::players::scope::MergeMode;
use wolluf_app::features::preview::dto::{
    EXCLUSION_PENDING, PreviewStateDto, RecsModeDto, RecsPreviewDto, RecsStateDto, SkillPreviewDto,
    TopPlayDto,
};
use wolluf_core::UnixUs;

use crate::cli::{MergeArg, PreviewCmd, PreviewRecsArgs, PreviewSkillArgs, RecsModeArg, ScopeArg};
use crate::cmd::library::msd;
use crate::cmd::players::rfc3339_ms;
use crate::exit;
use crate::follow;
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
        PreviewCmd::Recs(args) => {
            let r = recs(ctx, args).await?;
            if json {
                render::json(&r)?;
            } else {
                render::text(&recs_text(&r))?;
            }
        }
    }
    Ok(exit::exit_code(exit::SUCCESS))
}

async fn skill(ctx: &AppContext, args: PreviewSkillArgs) -> Result<Vec<SkillPreviewDto>, AppError> {
    let keymode = players_dto::keymode(u32::from(args.keys))?;
    let entry = entry(ctx, args.scope).await?;
    let merge = args.merge.map(merge);
    let previews = ctx.preview().skill(entry, keymode, merge).await?;
    if previews
        .iter()
        .all(|p| p.state != PreviewStateDto::Computing)
    {
        return Ok(previews);
    }
    // Closing the context would cancel the SSR job the preview started; a Ctrl-C stops waiting.
    tokio::select! {
        biased;
        () = follow::ctrl_c() => return Ok(previews),
        () = ctx.jobs().wait_idle() => {}
    }
    ctx.preview().skill(entry, keymode, merge).await
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

/// `--any-rate` replaces the stored setting for this call; without it the setting decides.
async fn recs(ctx: &AppContext, args: PreviewRecsArgs) -> Result<RecsPreviewDto, AppError> {
    let keymode = players_dto::keymode(u32::from(args.keys))?;
    let entry = entry(ctx, args.scope).await?;
    let merge = args.merge.map(merge);
    let mode = recs_mode(args.mode);
    let any_rate = args.any_rate.then_some(true);
    let preview = ctx.preview();
    let read = || preview.recs(entry, keymode, mode, args.skillset.clone(), merge, any_rate);
    let r = read().await?;
    if r.state != RecsStateDto::Computing {
        return Ok(r);
    }
    // Closing the context would cancel the SSR job the list started; a Ctrl-C stops waiting.
    tokio::select! {
        biased;
        () = follow::ctrl_c() => return Ok(r),
        () = ctx.jobs().wait_idle() => {}
    }
    read().await
}

const fn recs_mode(m: RecsModeArg) -> RecsModeDto {
    match m {
        RecsModeArg::Deficit => RecsModeDto::Deficit,
        RecsModeArg::Push => RecsModeDto::Push,
        RecsModeArg::Skillset => RecsModeDto::Skillset,
    }
}

fn recs_text(r: &RecsPreviewDto) -> String {
    let mut out = format!(
        "Recommendations {}K · Beta · uncalibrated · MinaCalc {}\n",
        r.keymode, r.calc_version
    );
    let mut pairs = vec![
        ("scope", r.scope_hash.clone()),
        ("method", r.method.clone()),
        ("state", render::wire(&r.state)),
    ];
    match r.state {
        RecsStateDto::Computing => pairs.push((
            "note",
            "SSRs are still being computed: run `wolluf library index`".to_owned(),
        )),
        RecsStateDto::NoRating => pairs.push((
            "note",
            "no rating in this keymode yet: play and `wolluf sync` first".to_owned(),
        )),
        RecsStateDto::Ready => {}
    }
    let rated = r.state != RecsStateDto::NoRating;
    pairs.extend([
        (
            "focus",
            render::opt(Some(r.focus.clone()).filter(|f| !f.is_empty())),
        ),
        ("rating", render::opt(rated.then(|| approx(r.rating_centi)))),
        (
            "band",
            render::opt(
                rated
                    .then(|| format!("{} to {}", approx(r.band_centi[0]), approx(r.band_centi[1]))),
            ),
        ),
        (
            "rates",
            if r.any_rate {
                "every grid rate (off-base rates need a rate copy)"
            } else {
                "NM, HT, DT and rate copies in the library"
            }
            .to_owned(),
        ),
        ("warnings", words(r.warnings.iter().cloned())),
    ]);
    out.push_str(&render::key_values(&pairs));
    if !rated {
        return out;
    }
    let rows: Vec<Vec<String>> = r
        .items
        .iter()
        .enumerate()
        .map(|(n, i)| {
            vec![
                (n + 1).to_string(),
                format!("{} [{}]", i.title, i.version),
                rate(i.rate_milli),
                approx(i.focus_centi),
                approx(i.overall_centi),
                if i.played { "yes" } else { "no" }.to_owned(),
                words(i.reasons.iter().map(|reason| {
                    if reason.args.is_empty() {
                        reason.code.clone()
                    } else {
                        format!("{}({})", reason.code, reason.args.join(","))
                    }
                })),
            ]
        })
        .collect();
    out.push_str(&format!("\nPICKS ({})\n", rows.len()));
    if !rows.is_empty() {
        out.push_str(&render::table(
            &[
                "#", "CHART", "RATE", "FOCUS", "OVERALL", "PLAYED", "REASONS",
            ],
            &rows,
        ));
    }
    out
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
    let pending = p
        .evidence
        .excluded
        .iter()
        .find(|e| e.reason == EXCLUSION_PENDING)
        .map_or(0, |e| e.count);
    if p.state == PreviewStateDto::Computing {
        pairs.push((
            "note",
            "SSRs are still being computed: run `wolluf library index`".to_owned(),
        ));
    } else if pending > 0 {
        pairs.push((
            "note",
            format!("{pending} plays have no SSR yet: run `wolluf library index`"),
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
        DanEstimateDto, DanThirdDto, EvidenceDto, EvidenceTierDto, ExclusionCountDto, ReasonDto,
        RecItemDto, SkillsetRatingDto, TrendPointDto,
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
    fn pending_plays_say_what_to_do() {
        let mut p = ready();
        p.evidence.excluded.push(ExclusionCountDto {
            reason: "pending".to_owned(),
            count: 2,
        });
        let text = skill_text(&p);
        assert!(text.contains("state     ready\n"), "{text}");
        assert!(
            text.contains("note      2 plays have no SSR yet: run `wolluf library index`\n"),
            "{text}"
        );
        assert!(!skill_text(&ready()).contains("note "));
    }

    fn rec(n: u8, rate_milli: u16, needs_rate_copy: bool, played: bool) -> RecItemDto {
        let reason = |code: &str, args: &[&str]| ReasonDto {
            code: code.to_owned(),
            args: args.iter().map(|a| (*a).to_owned()).collect(),
        };
        let mut reasons = vec![
            reason("deficit", &["jumpstream", "2413"]),
            reason(if played { "played_before" } else { "unplayed" }, &[]),
        ];
        if needs_rate_copy {
            reasons.push(reason("needs_rate_copy", &[&rate_milli.to_string()]));
        }
        RecItemDto {
            md5: format!("{n:032x}"),
            title: format!("Song {n}"),
            artist: "wolluf".to_owned(),
            version: "Hard".to_owned(),
            creator: "wolluf".to_owned(),
            set_id: Some(i32::from(n)),
            beatmap_id: None,
            rate_milli,
            needs_rate_copy,
            is_rate_copy: false,
            focus_centi: 2_450 + i32::from(n),
            overall_centi: 2_300,
            skillsets_centi: vec![2_000; 7],
            played,
            reasons,
        }
    }

    fn recs_ready() -> RecsPreviewDto {
        RecsPreviewDto {
            scope_hash: "ab".repeat(32),
            keymode: 4,
            method: "preview.band_recs@1".to_owned(),
            calc_version: 527,
            state: RecsStateDto::Ready,
            any_rate: true,
            focus: "jumpstream".to_owned(),
            rating_centi: 2_413,
            band_centi: [2_363, 2_563],
            items: vec![rec(1, 1_200, true, false), rec(2, 1_000, false, true)],
            warnings: vec!["uncalibrated".to_owned(), "goal_estimated".to_owned()],
        }
    }

    #[test]
    fn recs_text_shows_the_band_and_one_row_per_pick() {
        let want = format!(
            "\
Recommendations 4K · Beta · uncalibrated · MinaCalc 527
scope     {scope}
method    preview.band_recs@1
state     ready
focus     jumpstream
rating    ≈24.13
band      ≈23.63 to ≈25.63
rates     every grid rate (off-base rates need a rate copy)
warnings  uncalibrated goal_estimated

PICKS (2)
#  CHART          RATE   FOCUS   OVERALL  PLAYED  REASONS
1  Song 1 [Hard]  1.20x  ≈24.51  ≈23.00   no      deficit(jumpstream,2413) unplayed needs_rate_copy(1200)
2  Song 2 [Hard]  1.00x  ≈24.52  ≈23.00   yes     deficit(jumpstream,2413) played_before
",
            scope = "ab".repeat(32)
        );
        assert_eq!(recs_text(&recs_ready()), want);
    }

    #[test]
    fn recs_without_a_rating_say_what_to_do() {
        let none = RecsPreviewDto {
            state: RecsStateDto::NoRating,
            any_rate: false,
            focus: String::new(),
            rating_centi: 0,
            band_centi: [0, 0],
            items: vec![],
            ..recs_ready()
        };
        let want = format!(
            "\
Recommendations 4K · Beta · uncalibrated · MinaCalc 527
scope     {scope}
method    preview.band_recs@1
state     no_rating
note      no rating in this keymode yet: play and `wolluf sync` first
focus     -
rating    -
band      -
rates     NM, HT, DT and rate copies in the library
warnings  uncalibrated goal_estimated
",
            scope = "ab".repeat(32)
        );
        assert_eq!(recs_text(&none), want);

        let mut computing = recs_ready();
        computing.state = RecsStateDto::Computing;
        let text = recs_text(&computing);
        assert!(
            text.contains("note      SSRs are still being computed: run `wolluf library index`\n"),
            "{text}"
        );
        let mut empty = recs_ready();
        empty.items.clear();
        assert!(
            recs_text(&empty).ends_with("\nPICKS (0)\n"),
            "{}",
            recs_text(&empty)
        );
    }

    #[test]
    fn recs_modes_map_to_the_wire() {
        assert_eq!(recs_mode(RecsModeArg::Deficit), RecsModeDto::Deficit);
        assert_eq!(recs_mode(RecsModeArg::Push), RecsModeDto::Push);
        assert_eq!(recs_mode(RecsModeArg::Skillset), RecsModeDto::Skillset);
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
