//! `PreviewService` (ADR 0024): the uncalibrated skill preview of each resolved scope, read from
//! the `play_ssr` cache. Only the scope's own aliases' plays are read (ADR 0005).

use std::collections::{BTreeMap, BTreeSet};

use wolluf_core::VersionKey;
use wolluf_core::{ChartMd5, Keymode, PlayId, UnixUs};
use wolluf_engine::preview::{
    PlayMods, RatedPlay, aggregate_with, player_rating, rate_pbs, top2_per_family,
};
use wolluf_engine::stage::difficulty::{CALC_VERSION, SKILLSET_IDS};
use wolluf_store::DbHandle;
use wolluf_store::repo::cache::chart_msd;
use wolluf_store::repo::cache::play_ssr::{self as ssr_repo, PlaySsrRow, PlaySsrStatus};
use wolluf_store::repo::cache::{CatalogChart, catalog_chart, item_failure, job_run};
use wolluf_store::repo::ledger::{Play, play};
use wolluf_store::time::format_rfc3339_ms;

use super::dto::{
    DanEstimateDto, DanThirdDto, EXCLUSION_PENDING, EvidenceDto, EvidenceTierDto,
    ExclusionCountDto, METHOD_ETTERNA_RATING, PreviewStateDto, SkillPreviewDto, SkillsetRatingDto,
    TopPlayDto, TrendPointDto, warning,
};
use super::job::{ComputePlaySsrJob, keys_for};
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
/// Every sync chain ends in `ComputePlaySsr`, so its last run is among the newest few; a run older
/// than this only costs one redundant job.
const RECENT_RUNS: u32 = 50;

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
        // Read before the cache: a job that ends in between reads as computing once, never as
        // ready over rows it has not written yet.
        let job_pending = self.ctx.jobs().is_active(JobKindDto::ComputePlaySsr);
        let (user, cache) = (self.ctx.user_db().clone(), self.ctx.cache_db().clone());
        let params = self.params.clone();
        let (mut previews, start_job) = tokio::task::spawn_blocking(move || {
            let read: Vec<(SkillPreviewDto, Vec<PlayId>)> = scopes
                .iter()
                .map(|scope| preview(&user, &cache, &params, scope, job_pending))
                .collect::<Result<_, AppError>>()?;
            let pending: BTreeSet<PlayId> = read.iter().flat_map(|(_, ids)| ids).copied().collect();
            let start_job = !job_pending && unattempted(&cache, &pending)?;
            Ok::<_, AppError>((read, start_job))
        })
        .await
        .map_err(blocking_join_error)??;
        if start_job {
            self.ctx.jobs().submit(Box::new(ComputePlaySsrJob));
            for (p, ids) in &mut previews {
                if !ids.is_empty() {
                    p.state = PreviewStateDto::Computing;
                }
            }
        }
        Ok(previews.into_iter().map(|(p, _)| p).collect())
    }
}

/// Whether some pending play was never tried under the current key: a `play_ssr` VERSION bump
/// orphans every row, and only `IndexLibrary` chains the job. Plays the last run failed on stay
/// pending instead, or every read would rerun a job that fails them again.
fn unattempted(cache: &DbHandle, pending: &BTreeSet<PlayId>) -> Result<bool, AppError> {
    if pending.is_empty() {
        return Ok(false);
    }
    let kind = JobKindDto::ComputePlaySsr.as_str();
    Ok(cache.read(|c| {
        let Some(last) = job_run::list_recent(c, RECENT_RUNS)?
            .into_iter()
            .find(|r| r.kind == kind)
        else {
            return Ok(true);
        };
        let failed: BTreeSet<String> = item_failure::list(c, last.id)?
            .into_iter()
            .map(|f| f.item_ref)
            .collect();
        Ok(pending.iter().any(|id| !failed.contains(&id.to_string())))
    })?)
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
) -> Result<(SkillPreviewDto, Vec<PlayId>), AppError> {
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
    let Some(keys) = keys_for(keymode.columns(), engine)?.filter(|_| !plays.is_empty()) else {
        return Ok((out, Vec::new()));
    };
    // Oldest first: the trend rates growing prefixes and ties go to the earlier play.
    plays.sort_by_key(|p| (p.played_at, p.id));
    let ids: Vec<PlayId> = plays.iter().map(|p| p.id).collect();
    let rows: BTreeMap<PlayId, PlaySsrRow> = cache
        .read(|c| ssr_repo::get_many(c, keys.ssr, &ids))?
        .into_iter()
        .map(|r| (r.play_id, r))
        .collect();
    // A row that never comes (an item failure, a cancelled job) must not hold the page in
    // `computing`: only a live job does, and missing rows are reported as pending.
    out.state = if job_pending {
        PreviewStateDto::Computing
    } else {
        PreviewStateDto::Ready
    };
    let pending: Vec<PlayId> = plays
        .iter()
        .filter(|p| !rows.contains_key(&p.id))
        .map(|p| p.id)
        .collect();
    out.evidence.excluded = PlaySsrStatus::ALL
        .iter()
        .filter(|s| **s != PlaySsrStatus::Counted)
        .map(|&status| {
            let n = rows.values().filter(|r| r.status == status).count();
            (status.as_str(), n)
        })
        .chain([(EXCLUSION_PENDING, pending.len())])
        .filter(|(_, n)| *n > 0)
        .map(|(reason, n)| ExclusionCountDto {
            reason: reason.to_owned(),
            count: u32::try_from(n).unwrap_or(u32::MAX),
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
    let overall = engine.overall.skillsets(keymode);
    let Some(rating) = player_rating(&rated, &engine.rating, &overall) else {
        return Ok((out, pending));
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
        out.dan = dan(cache, keys.difficulty, &counted, params)?;
    }
    out.top_plays = rating
        .counted
        .iter()
        .take(params.top_plays)
        .map(|&i| top_play(&counted[i]))
        .collect();
    out.trend = trend(&counted, &rated, &overall, params);
    Ok((out, pending))
}

/// ADR 0024 (amended): the table maps a chart's Overall MSD and a dan course is passed by
/// surviving, so the input aggregates the Overall MSD, at the played mod rate, of the charts the
/// scope cleared: counted plays (complete, not excluded) without NoFail, which cannot fail.
/// Etterna's one PB per rate and two best rates per chart keep a chart replayed many times from
/// weighing more than twice. A chart without an MSD row at that rate is skipped.
fn dan(
    cache: &DbHandle,
    difficulty: VersionKey,
    counted: &[Counted<'_>],
    params: &PreviewServiceParams,
) -> Result<Option<DanEstimateDto>, AppError> {
    let cleared: Vec<&Counted<'_>> = counted
        .iter()
        .filter(|c| !PlayMods::from_bits(c.play.mods).nf)
        .collect();
    let mut by_rate: BTreeMap<u16, Vec<ChartMd5>> = BTreeMap::new();
    for c in &cleared {
        by_rate
            .entry(c.row.rate_milli)
            .or_default()
            .push(c.chart.md5);
    }
    let msd: BTreeMap<(u16, ChartMd5), i32> = cache.read(|conn| {
        let mut out = BTreeMap::new();
        for (rate, md5s) in &mut by_rate {
            md5s.sort_unstable();
            md5s.dedup();
            for (md5, overall) in chart_msd::overall_at(conn, difficulty, *rate, md5s)? {
                out.insert((*rate, md5), overall);
            }
        }
        Ok(out)
    })?;
    let rated: Vec<RatedPlay<'_>> = cleared
        .iter()
        .filter_map(|c| {
            let overall = *msd.get(&(c.row.rate_milli, c.chart.md5))?;
            let mut ssr_centi = [0; 8];
            ssr_centi[0] = overall;
            Some(RatedPlay {
                family: &c.family,
                rate_milli: c.rate_milli,
                ssr_centi,
                played_at_ms: c.play.played_at.0.div_euclid(US_PER_MS),
            })
        })
        .collect();
    let r = &params.engine.rating;
    let mut overall: Vec<f32> = top2_per_family(&rated, &rate_pbs(&rated), r)
        .into_iter()
        .map(|i| rated[i].ssr_centi[0] as f32 / CENTI)
        .collect();
    if overall.is_empty() {
        return Ok(None);
    }
    // `SortTopSSRPtrs` feeds the aggregate in ascending order; float sums depend on it.
    overall.sort_by(f32::total_cmp);
    let aggregate = aggregate_with(&overall, &r.aggregate).clamp(r.min_rating, r.max_rating);
    let Some(estimate) = params
        .engine
        .dan_k4
        .estimate((aggregate * CENTI).round() as i32)
    else {
        return Ok(None);
    };
    Ok(Some(DanEstimateDto {
        label: estimate.label,
        third: match estimate.third {
            wolluf_engine::preview::DanThird::Low => DanThirdDto::Low,
            wolluf_engine::preview::DanThird::Mid => DanThirdDto::Mid,
            wolluf_engine::preview::DanThird::High => DanThirdDto::High,
        },
        margin_centi: estimate.margin_centi,
    }))
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
    overall: &[bool; 7],
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
        if let Some(r) = player_rating(&rated[..=end], &params.engine.rating, overall) {
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
    use std::collections::BTreeMap;

    use wolluf_core::{ChartMd5, Keymode, PlayId};
    use wolluf_engine::preview::{DanTable4k, PreviewParams, aggregate_rating};
    use wolluf_store::repo::cache::chart_msd;
    use wolluf_store::repo::cache::chart_parsed;
    use wolluf_store::repo::cache::play_ssr as ssr_repo;
    use wolluf_store::repo::ledger::alias;

    use super::*;
    use crate::features::library::testkit::Map;
    use crate::features::library::{Keys, Raters};
    use crate::features::players::selection::Decision;
    use crate::features::plays::testkit::Fixture;
    use crate::features::preview::dto::{EvidenceTierDto, PreviewStateDto, warning};
    use crate::features::preview::testkit::{
        DT, OTHER, RANDOM, SELF, counts, current_key, dense_4k, last_summary, month_ticks, play_id,
        plays, recompute, row, score, synced,
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

    /// ADR 0024 (amended): 7K Technical is shown but stays out of Overall.
    #[tokio::test]
    async fn seven_k_overall_leaves_technical_out() {
        let s = Map::k7("Seven");
        let j = Map::jacks("Jacks");
        let f = synced(
            &[s.clone(), j.clone()],
            &[score(&s, SELF, 1), score(&j, SELF, 2)],
        )
        .await;
        let p = one(&f, self_entry(&f).await, Keymode::K7).await;
        let ids: Vec<&str> = p.skillsets.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids.len(), 7);
        assert_eq!(ids[6], "technical");
        let ratings: Vec<f64> = p
            .skillsets
            .iter()
            .map(|s| f64::from(s.rating_centi))
            .collect();
        let six = ratings[..6].iter().sum::<f64>() / 6.0;
        let seven = ratings.iter().sum::<f64>() / 7.0;
        assert!((six - seven).abs() > 2.0, "{p:?}");
        let overall = f64::from(p.overall_centi.unwrap());
        assert!((overall - six).abs() <= 1.0, "{overall} vs {six}: {p:?}");
    }

    #[tokio::test]
    async fn a_keymode_without_plays_reads_as_no_plays() {
        let a = Map::rice4("A");
        let f = synced(std::slice::from_ref(&a), &[score(&a, SELF, 1)]).await;
        let none = one(&f, self_entry(&f).await, Keymode::K7).await;
        assert_eq!(none.state, PreviewStateDto::NoPlays);
        assert_eq!(none.overall_centi, None);
        assert!(none.skillsets.is_empty() && none.top_plays.is_empty() && none.trend.is_empty());
        assert_eq!(none.evidence.counted, 0);
    }

    async fn ssr_runs(f: &Fixture) -> usize {
        f.ctx
            .jobs()
            .list(None)
            .await
            .unwrap()
            .iter()
            .filter(|j| j.kind == JobKindDto::ComputePlaySsr)
            .count()
    }

    /// A `play_ssr` VERSION bump orphans every row, and only `IndexLibrary` chains the job, so
    /// the preview starts it rather than reading ready with every play pending until an index.
    #[tokio::test]
    async fn orphaned_rows_start_the_ssr_job_and_read_as_computing() {
        let a = Map::rice4("A");
        let f = synced(std::slice::from_ref(&a), &[score(&a, SELF, 1)]).await;
        let entry = self_entry(&f).await;
        let runs = ssr_runs(&f).await;
        f.ctx
            .cache_db()
            .write(|tx| ssr_repo::prune_except(tx, &[]))
            .unwrap();
        assert!(!f.ctx.jobs().is_active(JobKindDto::ComputePlaySsr));

        let orphaned = one(&f, entry, Keymode::K4).await;
        assert_eq!(orphaned.state, PreviewStateDto::Computing, "{orphaned:?}");
        assert_eq!(excluded(&orphaned), [("pending", 1)]);
        f.ctx.jobs().wait_idle().await;
        assert_eq!(ssr_runs(&f).await, runs + 1);
        assert_eq!(last_summary(&f).await.computed, 1);

        let settled = one(&f, entry, Keymode::K4).await;
        assert_eq!(settled.state, PreviewStateDto::Ready, "{settled:?}");
        assert_eq!(settled.evidence.counted, 1);
        assert!(excluded(&settled).is_empty(), "{settled:?}");
        f.ctx.jobs().wait_idle().await;
        assert_eq!(
            ssr_runs(&f).await,
            runs + 1,
            "a settled preview starts nothing"
        );
    }

    fn excluded(p: &SkillPreviewDto) -> Vec<(&str, u32)> {
        p.evidence
            .excluded
            .iter()
            .map(|e| (e.reason.as_str(), e.count))
            .collect()
    }

    /// B's rows blob is corrupted, so its play fails as an item and never gets a row.
    #[tokio::test]
    async fn an_item_failure_with_an_idle_runner_is_ready_with_a_pending_play() {
        let a = Map::rice4("A");
        let b = Map::new("B", 4, dense_4k("B"));
        let f = synced(
            &[a.clone(), b.clone()],
            &[score(&a, SELF, 1), score(&b, SELF, 2)],
        )
        .await;
        let parse = Keys::current().unwrap().parse;
        let md5: ChartMd5 = b.md5.parse().unwrap();
        f.ctx
            .cache_db()
            .write(move |tx| {
                let mut parsed = chart_parsed::get(tx.conn(), md5, parse)?.unwrap();
                parsed.rows_blob = vec![0xff];
                chart_parsed::put(tx, &parsed)?;
                ssr_repo::prune_except(tx, &[])
            })
            .unwrap();
        let s = recompute(&f).await;
        assert_eq!(s.failed_items, 1, "{s:?}");
        assert!(!f.ctx.jobs().is_active(JobKindDto::ComputePlaySsr));

        let runs = ssr_runs(&f).await;
        let p = one(&f, self_entry(&f).await, Keymode::K4).await;
        assert_eq!(p.state, PreviewStateDto::Ready, "{p:?}");
        assert_eq!(p.evidence.counted, 1);
        assert_eq!(top_ids(&p), [hex(play_id(&f, &a, SELF))]);
        assert_eq!(excluded(&p), [("pending", 1)]);
        f.ctx.jobs().wait_idle().await;
        assert_eq!(
            ssr_runs(&f).await,
            runs,
            "a play the last run failed is not retried"
        );
    }

    #[tokio::test]
    async fn computing_only_while_the_job_is_queued_or_running() {
        let a = Map::rice4("A");
        let f = synced(std::slice::from_ref(&a), &[score(&a, SELF, 1)]).await;
        let scope = f
            .ctx
            .players()
            .resolve_scopes(self_entry(&f).await, Keymode::K4, None)
            .await
            .unwrap()
            .remove(0);
        let params = PreviewServiceParams::default();
        let read = |job_active| {
            preview(
                f.ctx.user_db(),
                f.ctx.cache_db(),
                &params,
                &scope,
                job_active,
            )
            .unwrap()
            .0
        };
        let active = read(true);
        assert_eq!(active.state, PreviewStateDto::Computing, "{active:?}");
        // The rows already there still show while the job runs.
        assert_eq!(active.evidence.counted, 1);
        assert_eq!(read(false).state, PreviewStateDto::Ready);
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

    /// ADR 0024 (amended): a dan is cleared by surviving, so the dan reads the cleared charts'
    /// Overall MSD at the played rate, never the plays' SSRs; NoFail and incomplete plays are not
    /// clears, and one chart at one rate counts once.
    #[tokio::test]
    async fn dan_reads_the_aggregate_of_cleared_charts_msd() {
        const NF: u32 = 1;
        let a = Map::rice4("A");
        let b = Map::new("B", 4, dense_4k("B"));
        let nofail = Map::new("NoFail", 4, dense_4k("NoFail"));
        let short = Map::new("Short", 4, dense_4k("Short"));
        let f = synced(
            &[a.clone(), b.clone(), nofail.clone(), short.clone()],
            &[
                score(&a, SELF, 1),
                score(&a, SELF, 2).mods(DT),
                score(&b, SELF, 3),
                score(&b, SELF, 4),
                score(&nofail, SELF, 5).mods(NF),
                score(&short, SELF, 6).counts(counts(10, 0, 0)),
            ],
        )
        .await;
        let parse = Keys::current().unwrap().parse;
        let difficulty = Raters::current(parse).unwrap().vkey(4).unwrap();
        let msd_at = |m: &Map, rate: u16| -> f32 {
            let md5: ChartMd5 = m.md5.parse().unwrap();
            let rows = f
                .ctx
                .cache_db()
                .read(|c| chart_msd::overall_at(c, difficulty, rate, &[md5]))
                .unwrap();
            rows[&md5] as f32 / 100.0
        };
        let mut cleared = vec![msd_at(&a, 1000), msd_at(&a, 1500), msd_at(&b, 1000)];
        cleared.sort_by(f32::total_cmp);
        let msd = (aggregate_rating(&cleared) * 100.0).round() as i32;

        let ssr_of = |id| row(&f, current_key(4), id).unwrap().centi.unwrap()[0] as f32 / 100.0;
        let ids = plays(&f);
        let mut ssrs = vec![ssr_of(ids[0].3), ssr_of(ids[1].3), ssr_of(ids[2].3)];
        ssrs.sort_by(f32::total_cmp);
        let ssr = (aggregate_rating(&ssrs) * 100.0).round() as i32;
        assert_ne!(msd, ssr);
        // A bound between the two inputs tells which one the dan read.
        let bound = (msd + ssr) / 2;
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
        let want = if msd >= bound { high } else { low };
        assert_eq!(dan.label, want, "msd {msd}, ssr {ssr}");
        let lower = if want == high { bound } else { 0 };
        // Exact: a NoFail, incomplete or repeated play in the input would move the aggregate.
        assert_eq!(dan.margin_centi, msd - lower);
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

    /// ADR 0005: Separate gives each self alias its own preview, and an alias that is unticked or
    /// marked not me never feeds one, even with a stronger play. `PREFIX` is a cfg-prefix variant
    /// of the session user, so it is self until marked not me.
    #[tokio::test]
    async fn separate_self_aliases_split_plays_and_never_read_unticked_or_not_me() {
        const SECOND: &str = "Wulf";
        const UNTICKED: &str = "w";
        const PREFIX: &str = "TWulf";
        let a = Map::rice4("A");
        let b = Map::rice4("B");
        let hard = Map::new("Hard", 4, dense_4k("Hard"));
        let f = synced(
            &[a.clone(), b.clone(), hard.clone()],
            &[
                score(&a, SELF, 1),
                score(&b, SELF, 2),
                score(&a, SECOND, 3),
                score(&hard, UNTICKED, 4),
                score(&hard, OTHER, 5),
                score(&hard, PREFIX, 6),
            ],
        )
        .await;
        let alias_id = |name: &str| {
            f.ctx
                .user_db()
                .read(alias::list)
                .unwrap()
                .into_iter()
                .find(|a| a.raw_name == name.as_bytes())
                .unwrap()
                .id
        };
        let entry = self_entry(&f).await;
        let separate = || async {
            f.ctx
                .preview()
                .skill(entry, Keymode::K4, Some(MergeMode::Separate))
                .await
                .unwrap()
        };
        let not_me = hex(play_id(&f, &hard, PREFIX));
        let before = separate().await;
        assert!(
            before.iter().any(|p| top_ids(p) == [not_me.clone()]),
            "the prefix alias starts in the self profile: {before:?}"
        );

        f.ctx
            .players()
            .decide(
                vec![
                    (alias_id(SECOND), Some(Decision::Me)),
                    (alias_id(PREFIX), Some(Decision::NotMe)),
                ],
                false,
            )
            .await
            .unwrap();
        let scopes = f
            .ctx
            .players()
            .resolve_scopes(entry, Keymode::K4, Some(MergeMode::Separate))
            .await
            .unwrap();
        let previews = separate().await;
        assert_eq!(previews.len(), 2, "{previews:?}");

        let mut want: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for (name, maps) in [(SELF, vec![&a, &b]), (SECOND, vec![&a])] {
            let scope = scopes
                .iter()
                .find(|s| s.alias_ids == [alias_id(name)])
                .unwrap_or_else(|| panic!("no scope for {name}: {scopes:?}"));
            let mut ids: Vec<String> = maps.iter().map(|m| hex(play_id(&f, m, name))).collect();
            ids.sort();
            want.insert(scope.hash.to_string(), ids);
        }
        let got: BTreeMap<String, Vec<String>> = previews
            .iter()
            .map(|p| {
                let mut ids = top_ids(p);
                ids.sort();
                (p.scope_hash.clone(), ids)
            })
            .collect();
        assert_eq!(got, want);
        let strangers = [
            hex(play_id(&f, &hard, UNTICKED)),
            hex(play_id(&f, &hard, OTHER)),
            not_me,
        ];
        for p in &previews {
            assert!(
                p.top_plays.iter().all(|t| !strangers.contains(&t.play_id)),
                "{p:?}"
            );
        }
    }
}
