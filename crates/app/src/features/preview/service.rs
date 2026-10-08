//! `PreviewService` (ADR 0024): the uncalibrated skill preview of each resolved scope, read from
//! the `play_ssr` cache. Only the scope's own aliases' plays are read (ADR 0005).

use std::collections::BTreeMap;

use wolluf_core::{ChartMd5, Keymode, PlayId, UnixUs};
use wolluf_engine::preview::{PlayerRating, RatedPlay, aggregate_with, player_rating};
use wolluf_engine::stage::difficulty::{CALC_VERSION, SKILLSET_IDS};
use wolluf_store::DbHandle;
use wolluf_store::repo::cache::play_ssr::{self as ssr_repo, PlaySsrRow, PlaySsrStatus};
use wolluf_store::repo::cache::{CatalogChart, catalog_chart};
use wolluf_store::repo::ledger::{Play, play};
use wolluf_store::time::format_rfc3339_ms;

use super::dto::{
    DanEstimateDto, DanThirdDto, EvidenceDto, EvidenceTierDto, ExclusionCountDto,
    METHOD_ETTERNA_RATING, PreviewStateDto, SkillPreviewDto, SkillsetRatingDto, TopPlayDto,
    TrendPointDto, warning,
};
use super::job::ssr_vkey;
use super::params::PreviewServiceParams;
use crate::context::{AppContext, blocking_join_error};
use crate::errors::AppError;
use crate::features::library::folder_of;
use crate::features::players::identity::EntryRef;
use crate::features::players::scope::{MergeMode, ResolvedScope};
use crate::jobs::JobKindDto;

/// A chart without a rate tag plays at its own speed.
const NATIVE_RATE_MILLI: u32 = 1_000;
const CENTI: f32 = 100.0;
const US_PER_MS: i64 = 1_000;
const US_PER_MS_F64: f64 = 1_000.0;
/// `YYYY-MM` of an RFC 3339 timestamp.
const MONTH_LEN: usize = 7;

pub struct PreviewService<'a> {
    ctx: &'a AppContext,
    params: PreviewServiceParams,
}

impl<'a> PreviewService<'a> {
    pub fn new(ctx: &'a AppContext) -> Self {
        Self::with_params(ctx, PreviewServiceParams::default())
    }

    pub fn with_params(ctx: &'a AppContext, params: PreviewServiceParams) -> Self {
        Self { ctx, params }
    }

    /// One preview per scope `entry` resolves to; `merge` overrides the profile's merge mode.
    pub async fn skill(
        &self,
        entry: EntryRef,
        keymode: Keymode,
        merge: Option<MergeMode>,
    ) -> Result<Vec<SkillPreviewDto>, AppError> {
        let scopes = self
            .ctx
            .players()
            .resolve_scopes(entry, keymode, merge)
            .await?;
        // Read before the cache, so a job that ends in between still reads as computing.
        let job_pending = self.ctx.jobs().is_active(JobKindDto::ComputePlaySsr);
        let (user, cache) = (self.ctx.user_db().clone(), self.ctx.cache_db().clone());
        let params = self.params.clone();
        tokio::task::spawn_blocking(move || {
            scopes
                .iter()
                .map(|scope| preview(&user, &cache, &params, scope, job_pending))
                .collect()
        })
        .await
        .map_err(blocking_join_error)?
    }
}

fn warnings(keymode: Keymode) -> Vec<String> {
    let mut codes = vec![warning::UNCALIBRATED, warning::GOAL_ESTIMATED];
    if keymode == Keymode::K7 {
        codes.extend([
            warning::K7_LESS_VALIDATED,
            warning::LN_NOT_MEASURED,
            warning::K7_TECH_NOT_MEASURED,
        ]);
    }
    codes.into_iter().map(str::to_owned).collect()
}

/// A counted play with what the rating and the top-plays list need.
struct Counted<'a> {
    play: &'a Play,
    chart: &'a CatalogChart,
    row: PlaySsrRow,
    centi: [i32; 8],
    rate_milli: u16,
    family: String,
}

fn preview(
    user: &DbHandle,
    cache: &DbHandle,
    params: &PreviewServiceParams,
    scope: &ResolvedScope,
    job_pending: bool,
) -> Result<SkillPreviewDto, AppError> {
    let engine = &params.engine;
    let keymode = scope.keymode;
    let mut out = SkillPreviewDto {
        scope_hash: scope.hash.to_string(),
        keymode: keymode.columns(),
        method: METHOD_ETTERNA_RATING.to_owned(),
        calc_version: CALC_VERSION,
        state: PreviewStateDto::NoPlays,
        overall_centi: None,
        skillsets: Vec::new(),
        dan: None,
        evidence: EvidenceDto {
            counted: 0,
            tier: tier(engine.evidence.tier(0)),
            excluded: Vec::new(),
        },
        top_plays: Vec::new(),
        trend: Vec::new(),
        warnings: warnings(keymode),
    };
    let catalog: BTreeMap<ChartMd5, CatalogChart> = cache
        .read(|c| catalog_chart::list_by_keymode(c, keymode.columns()))?
        .into_iter()
        .map(|c| (c.md5, c))
        .collect();
    let mut plays: Vec<Play> = user
        .read(|c| play::since(c, &scope.alias_ids, UnixUs(0)))?
        .into_iter()
        .filter(|p| catalog.contains_key(&p.chart_md5))
        .collect();
    let Some(vkey) = ssr_vkey(keymode.columns(), engine)?.filter(|_| !plays.is_empty()) else {
        return Ok(out);
    };
    // Oldest first: the trend rates growing prefixes and ties go to the earlier play.
    plays.sort_by_key(|p| (p.played_at, p.id));
    let ids: Vec<PlayId> = plays.iter().map(|p| p.id).collect();
    let rows: BTreeMap<PlayId, PlaySsrRow> = cache
        .read(|c| ssr_repo::get_many(c, vkey, &ids))?
        .into_iter()
        .map(|r| (r.play_id, r))
        .collect();
    out.state = if rows.len() < plays.len() || job_pending {
        PreviewStateDto::Computing
    } else {
        PreviewStateDto::Ready
    };
    out.evidence.excluded = PlaySsrStatus::ALL
        .iter()
        .filter(|s| **s != PlaySsrStatus::Counted)
        .filter_map(|&status| {
            let n = rows.values().filter(|r| r.status == status).count();
            (n > 0).then(|| ExclusionCountDto {
                reason: status.as_str().to_owned(),
                count: u32::try_from(n).unwrap_or(u32::MAX),
            })
        })
        .collect();

    let counted: Vec<Counted<'_>> = plays
        .iter()
        .filter_map(|p| {
            let row = *rows.get(&p.id)?;
            let centi = row.centi.filter(|_| row.status == PlaySsrStatus::Counted)?;
            let chart = catalog.get(&p.chart_md5)?;
            let chart_rate = engine
                .family
                .chart_rate_milli(&chart.version)
                .map_or(NATIVE_RATE_MILLI, u32::from);
            let rate = u32::from(row.rate_milli) * chart_rate / NATIVE_RATE_MILLI;
            Some(Counted {
                play: p,
                chart,
                row,
                centi,
                rate_milli: u16::try_from(rate).unwrap_or(u16::MAX),
                family: engine
                    .family
                    .family_key(folder_of(&chart.path), &chart.version),
            })
        })
        .collect();
    let rated: Vec<RatedPlay<'_>> = counted
        .iter()
        .map(|c| RatedPlay {
            family: &c.family,
            rate_milli: c.rate_milli,
            ssr_centi: c.centi,
            played_at_ms: c.play.played_at.0.div_euclid(US_PER_MS),
        })
        .collect();
    let Some(rating) = player_rating(&rated, &engine.rating) else {
        return Ok(out);
    };
    let n_counted = u32::try_from(rating.counted.len()).unwrap_or(u32::MAX);
    out.overall_centi = Some(rating.overall_centi);
    out.skillsets = SKILLSET_IDS[1..]
        .iter()
        .zip(rating.skillsets_centi)
        .map(|(id, rating_centi)| SkillsetRatingDto {
            id: (*id).to_owned(),
            rating_centi,
        })
        .collect();
    out.evidence.counted = n_counted;
    out.evidence.tier = tier(engine.evidence.tier(n_counted));
    if keymode == Keymode::K4 {
        out.dan = dan(&counted, &rating, params);
    }
    out.top_plays = rating
        .counted
        .iter()
        .take(params.top_plays)
        .map(|&i| top_play(&counted[i]))
        .collect();
    out.trend = trend(&counted, &rated, params);
    Ok(out)
}

/// ADR 0024: the dan table maps a chart's Overall MSD, so the player side aggregates the
/// counted plays' Overall SSRs on that scale instead of reusing the mean of the 7 skillsets.
fn dan(
    counted: &[Counted<'_>],
    rating: &PlayerRating,
    params: &PreviewServiceParams,
) -> Option<DanEstimateDto> {
    let r = &params.engine.rating;
    let mut overall: Vec<f32> = rating
        .counted
        .iter()
        .map(|&i| counted[i].centi[0] as f32 / CENTI)
        .collect();
    // `SortTopSSRPtrs` feeds the aggregate in ascending order; float sums depend on it.
    overall.sort_by(f32::total_cmp);
    let aggregate = aggregate_with(&overall, &r.aggregate).clamp(r.min_rating, r.max_rating);
    let estimate = params
        .engine
        .dan_k4
        .estimate((aggregate * CENTI).round() as i32)?;
    Some(DanEstimateDto {
        label: estimate.label,
        third: match estimate.third {
            wolluf_engine::preview::DanThird::Low => DanThirdDto::Low,
            wolluf_engine::preview::DanThird::Mid => DanThirdDto::Mid,
            wolluf_engine::preview::DanThird::High => DanThirdDto::High,
        },
        margin_centi: estimate.margin_centi,
    })
}

fn tier(t: wolluf_engine::preview::EvidenceTier) -> EvidenceTierDto {
    match t {
        wolluf_engine::preview::EvidenceTier::Low => EvidenceTierDto::Low,
        wolluf_engine::preview::EvidenceTier::Medium => EvidenceTierDto::Medium,
        wolluf_engine::preview::EvidenceTier::Ok => EvidenceTierDto::Ok,
    }
}

fn top_play(c: &Counted<'_>) -> TopPlayDto {
    // Ties keep the first skillset in `SKILLSET_IDS` order.
    let dominant = (1..SKILLSET_IDS.len())
        .rev()
        .max_by_key(|&k| c.centi[k])
        .unwrap_or(0);
    TopPlayDto {
        play_id: c.play.id.to_string(),
        md5: c.chart.md5.to_string(),
        title: c.chart.title.clone(),
        version: c.chart.version.clone(),
        rate_milli: c.rate_milli,
        goal_permyriad: c.row.goal_permyriad.unwrap_or_default(),
        overall_centi: c.centi[0],
        dominant_skillset: SKILLSET_IDS[dominant].to_owned(),
        played_at_ms: c.play.played_at.0 as f64 / US_PER_MS_F64,
    }
}

fn month(t: UnixUs) -> String {
    let mut s = format_rfc3339_ms(t);
    s.truncate(MONTH_LEN);
    s
}

/// Overall at the end of each month with a counted play, over every counted play up to then.
/// `rated` is oldest first, so each point rates a prefix.
fn trend(
    counted: &[Counted<'_>],
    rated: &[RatedPlay<'_>],
    params: &PreviewServiceParams,
) -> Vec<TrendPointDto> {
    let mut points: Vec<TrendPointDto> = Vec::new();
    for (end, c) in counted.iter().enumerate() {
        let m = month(c.play.played_at);
        let last_of_month = counted
            .get(end + 1)
            .is_none_or(|next| month(next.play.played_at) != m);
        if !last_of_month {
            continue;
        }
        if let Some(r) = player_rating(&rated[..=end], &params.engine.rating) {
            points.push(TrendPointDto {
                month: m,
                overall_centi: r.overall_centi,
            });
        }
    }
    points
}

#[cfg(test)]
mod tests {
    use wolluf_core::{Keymode, PlayId};
    use wolluf_engine::preview::{DanTable4k, PreviewParams, aggregate_rating};
    use wolluf_store::repo::cache::play_ssr as ssr_repo;

    use super::*;
    use crate::features::library::testkit::Map;
    use crate::features::plays::testkit::Fixture;
    use crate::features::preview::dto::{EvidenceTierDto, PreviewStateDto, warning};
    use crate::features::preview::testkit::{
        DT, OTHER, RANDOM, SELF, current_key, dense_4k, month_ticks, play_id, plays, row, score,
        synced,
    };

    async fn self_entry(f: &Fixture) -> EntryRef {
        EntryRef::Profile(f.ctx.players().self_profile_id().await.unwrap().unwrap())
    }

    async fn one(f: &Fixture, entry: EntryRef, keymode: Keymode) -> SkillPreviewDto {
        let mut previews = f.ctx.preview().skill(entry, keymode, None).await.unwrap();
        assert_eq!(previews.len(), 1, "{previews:?}");
        previews.remove(0)
    }

    fn top_ids(p: &SkillPreviewDto) -> Vec<String> {
        p.top_plays.iter().map(|t| t.play_id.clone()).collect()
    }

    fn hex(id: PlayId) -> String {
        id.to_string()
    }

    /// Rosalind's play out-rates every self play, so it would move the self rating if it leaked.
    #[tokio::test]
    async fn self_reads_only_its_aliases_and_all_players_reads_everyone() {
        let a = Map::rice4("A");
        let b = Map::rice4("B");
        let hard = Map::new("Hard", 4, dense_4k("Hard"));
        let f = synced(
            &[a.clone(), b.clone(), hard.clone()],
            &[
                score(&a, SELF, 1),
                score(&b, SELF, 2),
                score(&hard, OTHER, 3),
            ],
        )
        .await;
        let theirs = play_id(&f, &hard, OTHER);
        let theirs_overall = row(&f, current_key(4), theirs).unwrap().centi.unwrap()[0];
        for m in [&a, &b] {
            let mine = row(&f, current_key(4), play_id(&f, m, SELF)).unwrap();
            assert!(mine.centi.unwrap()[0] < theirs_overall, "{mine:?}");
        }

        let me = one(&f, self_entry(&f).await, Keymode::K4).await;
        assert_eq!(me.state, PreviewStateDto::Ready, "{me:?}");
        let mut mine = top_ids(&me);
        mine.sort();
        let mut want = vec![hex(play_id(&f, &a, SELF)), hex(play_id(&f, &b, SELF))];
        want.sort();
        assert_eq!(mine, want);
        assert_eq!(me.evidence.counted, 2);
        assert!(
            me.top_plays
                .iter()
                .all(|t| t.overall_centi < theirs_overall)
        );

        let all = one(&f, EntryRef::AllPlayers, Keymode::K4).await;
        assert!(top_ids(&all).contains(&hex(theirs)), "{all:?}");
        assert_eq!(all.top_plays[0].play_id, hex(theirs));
        assert_eq!(all.top_plays[0].title, "Hard");
        assert_eq!(all.evidence.counted, 3);
        assert!(
            all.overall_centi.unwrap() > me.overall_centi.unwrap(),
            "{all:?} {me:?}"
        );
        assert_ne!(all.scope_hash, me.scope_hash);
    }

    #[tokio::test]
    async fn ready_preview_carries_the_frozen_shape() {
        let a = Map::rice4("A");
        let f = synced(
            std::slice::from_ref(&a),
            &[score(&a, SELF, 1), score(&a, SELF, 2).mods(RANDOM)],
        )
        .await;
        let p = one(&f, self_entry(&f).await, Keymode::K4).await;
        assert_eq!(p.keymode, 4);
        assert_eq!(p.method, "preview.etterna_rating@1");
        assert_eq!(p.calc_version, 527);
        assert_eq!(p.scope_hash.len(), 64);
        let ids: Vec<&str> = p.skillsets.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(
            ids,
            [
                "stream",
                "jumpstream",
                "handstream",
                "stamina",
                "jackspeed",
                "chordjack",
                "technical"
            ]
        );
        let mean = p
            .skillsets
            .iter()
            .map(|s| f64::from(s.rating_centi))
            .sum::<f64>()
            / 7.0;
        assert!(
            (f64::from(p.overall_centi.unwrap()) - mean).abs() <= 1.0,
            "{p:?}"
        );
        assert_eq!(p.evidence.counted, 1);
        assert_eq!(p.evidence.tier, EvidenceTierDto::Low);
        let excluded: Vec<(&str, u32)> = p
            .evidence
            .excluded
            .iter()
            .map(|e| (e.reason.as_str(), e.count))
            .collect();
        assert_eq!(excluded, [("unsupported_mods", 1)]);
        let top = &p.top_plays[0];
        assert_eq!((top.title.as_str(), top.version.as_str()), ("A", "Normal"));
        assert_eq!(top.md5, a.md5);
        assert_eq!(top.rate_milli, 1000);
        assert!(top.goal_permyriad > 9_000, "{top:?}");
        assert!(!top.dominant_skillset.is_empty() && top.dominant_skillset != "overall");
        assert_eq!(
            p.warnings,
            [warning::UNCALIBRATED, warning::GOAL_ESTIMATED].map(str::to_owned)
        );
    }

    #[tokio::test]
    async fn seven_k_carries_its_warnings_and_no_dan() {
        let s = Map::k7("Seven");
        let f = synced(std::slice::from_ref(&s), &[score(&s, SELF, 1)]).await;
        let p = one(&f, self_entry(&f).await, Keymode::K7).await;
        assert_eq!(p.state, PreviewStateDto::Ready, "{p:?}");
        assert_eq!(p.dan, None);
        for code in [
            warning::UNCALIBRATED,
            warning::GOAL_ESTIMATED,
            warning::K7_LESS_VALIDATED,
            warning::LN_NOT_MEASURED,
            warning::K7_TECH_NOT_MEASURED,
        ] {
            assert!(p.warnings.iter().any(|w| w == code), "{code}: {p:?}");
        }
    }

    #[tokio::test]
    async fn no_plays_and_computing_states() {
        let a = Map::rice4("A");
        let f = synced(std::slice::from_ref(&a), &[score(&a, SELF, 1)]).await;
        let entry = self_entry(&f).await;

        let none = one(&f, entry, Keymode::K7).await;
        assert_eq!(none.state, PreviewStateDto::NoPlays);
        assert_eq!(none.overall_centi, None);
        assert!(none.skillsets.is_empty() && none.top_plays.is_empty() && none.trend.is_empty());
        assert_eq!(none.evidence.counted, 0);

        f.ctx
            .cache_db()
            .write(|tx| ssr_repo::prune_except(tx, &[]))
            .unwrap();
        let computing = one(&f, entry, Keymode::K4).await;
        assert_eq!(computing.state, PreviewStateDto::Computing, "{computing:?}");
        assert_eq!(computing.overall_centi, None);
    }

    #[tokio::test]
    async fn rate_copies_rate_at_their_own_rate() {
        let a = Map::rice4("A").named("100 wolluf - A", "Normal");
        let copy = Map::new("A copy", 4, dense_4k("A copy")).named("100 wolluf - A", "Normal 1.2x");
        let f = synced(
            &[a.clone(), copy.clone()],
            &[
                score(&a, SELF, 1),
                score(&copy, SELF, 2),
                score(&copy, SELF, 3).mods(DT),
            ],
        )
        .await;
        let p = one(&f, self_entry(&f).await, Keymode::K4).await;
        let mut rates: Vec<u16> = p.top_plays.iter().map(|t| t.rate_milli).collect();
        rates.sort_unstable();
        // One family at three rates (1.0, 1.2, 1.2 × DT): Etterna keeps its two best, and the
        // rice original is the weakest.
        assert_eq!(rates, [1200, 1800], "{p:?}");
        assert_eq!(p.evidence.counted, 2, "{p:?}");
        let stored = row(&f, current_key(4), play_id_at(&f, 3)).unwrap();
        assert_eq!(stored.rate_milli, 1500, "the cache keeps the mod rate");
    }

    fn play_id_at(f: &Fixture, nth: usize) -> PlayId {
        plays(f)[nth - 1].3
    }

    #[tokio::test]
    async fn dan_reads_the_aggregate_of_overall_ssrs() {
        let a = Map::rice4("A");
        let b = Map::new("B", 4, dense_4k("B"));
        let f = synced(
            &[a.clone(), b.clone()],
            &[score(&a, SELF, 1), score(&b, SELF, 2)],
        )
        .await;
        let mut overall: Vec<f32> = [&a, &b]
            .iter()
            .map(|m| {
                row(&f, current_key(4), play_id(&f, m, SELF))
                    .unwrap()
                    .centi
                    .unwrap()[0] as f32
                    / 100.0
            })
            .collect();
        overall.sort_by(f32::total_cmp);
        let aggregate = (aggregate_rating(&overall) * 100.0).round() as i32;
        let mean_of_7 = one(&f, self_entry(&f).await, Keymode::K4)
            .await
            .overall_centi
            .unwrap();
        assert_ne!(aggregate, mean_of_7);
        // A bound between the two inputs tells which one the dan read.
        let bound = (aggregate + mean_of_7) / 2;
        let (low, high) = ("Below", "Above");
        let params = PreviewServiceParams {
            engine: PreviewParams {
                dan_k4: DanTable4k {
                    dans: vec![(low.to_owned(), 0), (high.to_owned(), bound)],
                    top_span_centi: 300,
                },
                ..PreviewParams::default()
            },
            ..PreviewServiceParams::default()
        };
        let p = PreviewService::with_params(&f.ctx, params)
            .skill(self_entry(&f).await, Keymode::K4, None)
            .await
            .unwrap()
            .remove(0);
        let dan = p.dan.unwrap();
        let want = if aggregate >= bound { high } else { low };
        assert_eq!(dan.label, want, "aggregate {aggregate}, mean {mean_of_7}");
        let lower = if want == high { bound } else { 0 };
        assert_eq!(dan.margin_centi, aggregate - lower);
    }

    #[tokio::test]
    async fn trend_has_one_point_per_month_with_counted_plays() {
        let a = Map::rice4("A");
        let b = Map::new("B", 4, dense_4k("B"));
        let f = synced(
            &[a.clone(), b.clone()],
            &[
                score(&a, SELF, 1).ticks(month_ticks(2026, 5)),
                score(&b, SELF, 2).ticks(month_ticks(2026, 7)),
            ],
        )
        .await;
        let p = one(&f, self_entry(&f).await, Keymode::K4).await;
        let months: Vec<&str> = p.trend.iter().map(|t| t.month.as_str()).collect();
        assert_eq!(months, ["2026-05", "2026-07"]);
        assert!(p.trend[0].overall_centi < p.trend[1].overall_centi, "{p:?}");
        assert_eq!(p.trend[1].overall_centi, p.overall_centi.unwrap());
        assert!(p.top_plays.iter().all(|t| t.played_at_ms > 1.7e12));
    }
}
