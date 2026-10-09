//! `preview_recs` (ADR 0024, `preview.band_recs@1`): charts × rate in a band around the selected
//! scope's preview rating, read from `chart_msd` and the `play_ssr` cache. Only the scope's own
//! plays feed the rating and the played set (ADR 0005).

use std::collections::{BTreeMap, BTreeSet};

use wolluf_core::{ChartMd5, Keymode, PlayId, VersionKey};
use wolluf_engine::preview::recs::{Candidate, Mode, OVERALL, RecItem, RecsParams, recommend};
use wolluf_engine::preview::{FamilyParams, player_rating};
use wolluf_engine::stage::difficulty::{CALC_VERSION, SKILLSET_IDS};
use wolluf_store::DbHandle;
use wolluf_store::repo::cache::{CatalogChart, chart_msd};

use super::ComputePlaySsrJob;
use super::dto::{
    METHOD_BAND_RECS, ReasonDto, RecItemDto, RecsModeDto, RecsPreviewDto, RecsStateDto,
};
use super::params::PreviewServiceParams;
use super::service::{PreviewService, rated, read_scope, unattempted, warnings};
use crate::context::blocking_join_error;
use crate::errors::AppError;
use crate::features::library::folder_of;
use crate::features::players::identity::EntryRef;
use crate::features::players::scope::{MergeMode, ResolvedScope};
use crate::jobs::JobKindDto;

impl PreviewService<'_> {
    /// The recommendations of the first scope `entry` resolves to. Separate resolves one scope
    /// per alias but the page shows one list, so it takes the first one, in the order
    /// `resolve_scopes` returns them. `any_rate_override` replaces the stored "Enable rates"
    /// setting for this call without changing it.
    pub async fn recs(
        &self,
        entry: EntryRef,
        keymode: Keymode,
        mode: RecsModeDto,
        skillset: Option<String>,
        merge: Option<MergeMode>,
        any_rate_override: Option<bool>,
    ) -> Result<RecsPreviewDto, AppError> {
        let mode = engine_mode(mode, skillset.as_deref())?;
        let any_rate = match any_rate_override {
            Some(on) => on,
            None => self.ctx.settings().recs_any_rate().await?,
        };
        let req = RecsRequest { mode, any_rate };
        let Some(scope) = self
            .ctx
            .players()
            .resolve_scopes(entry, keymode, merge)
            .await?
            .into_iter()
            .next()
        else {
            let mut out = empty("", keymode, any_rate);
            out.focus = mode_focus(req.mode);
            return Ok(out);
        };
        // Read before the cache, as the skill preview does: a job that ends in between reads as
        // computing once, never as ready over rows it has not written yet.
        let job_pending = self.ctx.jobs().is_active(JobKindDto::ComputePlaySsr);
        let (user, cache) = (self.ctx.user_db().clone(), self.ctx.cache_db().clone());
        let params = self.params.clone();
        let (mut out, start_job) = tokio::task::spawn_blocking(move || {
            let (out, pending) = recs_for_scope(&user, &cache, &params, &scope, req, job_pending)?;
            let pending: BTreeSet<PlayId> = pending.into_iter().collect();
            let start_job = !job_pending && !pending.is_empty() && unattempted(&cache, &pending)?;
            Ok::<_, AppError>((out, start_job))
        })
        .await
        .map_err(blocking_join_error)??;
        if start_job {
            self.ctx.jobs().submit(Box::new(ComputePlaySsrJob));
            out.state = RecsStateDto::Computing;
        }
        Ok(out)
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) struct RecsRequest {
    pub(super) mode: Mode,
    pub(super) any_rate: bool,
}

/// `INVALID_INPUT` when Skillset mode names no MinaCalc skillset id; other modes ignore it.
fn engine_mode(mode: RecsModeDto, skillset: Option<&str>) -> Result<Mode, AppError> {
    match mode {
        RecsModeDto::Deficit => Ok(Mode::Deficit),
        RecsModeDto::Push => Ok(Mode::Push),
        RecsModeDto::Skillset => skillset
            .and_then(|id| SKILLSET_IDS.iter().position(|s| *s == id))
            .map(Mode::Skillset)
            .ok_or_else(|| AppError::invalid_input().with_arg("skillset", skillset.unwrap_or(""))),
    }
}

/// The focus a mode names before any rating; Deficit has none until it sees one.
fn mode_focus(mode: Mode) -> String {
    match mode {
        Mode::Deficit => String::new(),
        Mode::Push => SKILLSET_IDS[OVERALL].to_owned(),
        Mode::Skillset(i) => SKILLSET_IDS[i].to_owned(),
    }
}

fn empty(scope_hash: &str, keymode: Keymode, any_rate: bool) -> RecsPreviewDto {
    RecsPreviewDto {
        scope_hash: scope_hash.to_owned(),
        keymode: keymode.columns(),
        method: METHOD_BAND_RECS.to_owned(),
        calc_version: CALC_VERSION,
        state: RecsStateDto::NoRating,
        any_rate,
        focus: String::new(),
        rating_centi: 0,
        band_centi: [0, 0],
        items: Vec::new(),
        warnings: warnings(keymode),
    }
}

/// Also returns the scope's plays without an SSR row, so the caller can start the job.
pub(super) fn recs_for_scope(
    user: &DbHandle,
    cache: &DbHandle,
    params: &PreviewServiceParams,
    scope: &ResolvedScope,
    req: RecsRequest,
    job_pending: bool,
) -> Result<(RecsPreviewDto, Vec<PlayId>), AppError> {
    let engine = &params.engine;
    let keymode = scope.keymode;
    let mut out = empty(&scope.hash.to_string(), keymode, req.any_rate);
    out.focus = mode_focus(req.mode);
    let read = read_scope(user, cache, engine, scope)?;
    let Some(keys) = read.keys else {
        return Ok((out, Vec::new()));
    };
    if job_pending {
        out.state = RecsStateDto::Computing;
    }
    let counted = read.counted(engine);
    let rated = rated(&counted);
    let overall = engine.overall.skillsets(keymode);
    let Some(rating) = player_rating(&rated, &engine.rating, &overall) else {
        return Ok((out, read.pending));
    };
    if !job_pending {
        out.state = RecsStateDto::Ready;
    }
    // Any play, counted or not: a chart the player failed or played with mods was still played.
    let played: BTreeSet<ChartMd5> = read.plays.iter().map(|p| p.chart_md5).collect();
    let recs_params = (params.recs)(keymode.columns());
    let rates = rates(&recs_params, req.any_rate);
    let candidates = candidates(
        cache,
        keys.difficulty,
        &rates,
        &read.catalog,
        &engine.family,
    )?;
    let picked = recommend(
        &rating,
        &candidates,
        &played,
        req.mode,
        req.any_rate,
        &recs_params,
    );
    out.focus = SKILLSET_IDS[picked.focus].to_owned();
    out.rating_centi = if picked.focus == OVERALL {
        rating.overall_centi
    } else {
        rating.skillsets_centi[picked.focus - 1]
    };
    out.band_centi = [picked.band.0, picked.band.1];
    out.items = picked
        .items
        .iter()
        .filter_map(|i| Some(item_dto(i, read.catalog.get(&i.md5)?)))
        .collect();
    Ok((out, read.pending))
}

fn rates(params: &RecsParams, any_rate: bool) -> Vec<u16> {
    let mut rates = params.base_rates_milli.clone();
    if any_rate {
        rates.extend(&params.grid_rates_milli);
    }
    rates.sort_unstable();
    rates.dedup();
    rates
}

/// Every chart MinaCalc rated under `difficulty` at one of `rates`. LN-heavy and calc-rejected
/// charts have a status but no rate rows, so they never become candidates.
pub(super) fn candidates(
    cache: &DbHandle,
    difficulty: VersionKey,
    rates: &[u16],
    catalog: &BTreeMap<ChartMd5, CatalogChart>,
    family: &FamilyParams,
) -> Result<Vec<Candidate>, AppError> {
    Ok(cache.read(|c| {
        let mut out = Vec::new();
        for &rate_milli in rates {
            for (md5, centi) in chart_msd::rated_at(c, difficulty, rate_milli)? {
                let Some(chart) = catalog.get(&md5) else {
                    continue;
                };
                out.push(Candidate {
                    md5,
                    set_key: set_key(chart),
                    rate_milli,
                    centi,
                    is_rate_copy: family.chart_rate_milli(&chart.version).is_some(),
                });
            }
        }
        Ok(out)
    })?)
}

/// stable stores no set id for unsubmitted maps (0 or -1, read as `None`), so their folder
/// stands in; the prefixes keep an id from colliding with a folder named like one.
fn set_key(chart: &CatalogChart) -> String {
    match chart.set_id {
        Some(id) => format!("set:{id}"),
        None => format!("folder:{}", folder_of(&chart.path)),
    }
}

fn item_dto(i: &RecItem, chart: &CatalogChart) -> RecItemDto {
    RecItemDto {
        md5: i.md5.to_string(),
        title: chart.title.clone(),
        artist: chart.artist.clone(),
        version: chart.version.clone(),
        creator: chart.creator.clone(),
        set_id: chart.set_id,
        beatmap_id: chart.beatmap_id,
        rate_milli: i.rate_milli,
        needs_rate_copy: i.needs_rate_copy,
        is_rate_copy: i.is_rate_copy,
        focus_centi: i.focus_centi,
        overall_centi: i.centi[OVERALL],
        skillsets_centi: i.centi[OVERALL + 1..].to_vec(),
        played: i.played,
        reasons: i
            .reasons
            .iter()
            .map(|r| ReasonDto {
                code: r.code().to_owned(),
                args: r.args(),
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use wolluf_core::{ErrorCode, Keymode};
    use wolluf_engine::preview::PreviewParams;
    use wolluf_store::repo::cache::catalog_chart;
    use wolluf_store::repo::cache::chart_msd::{self, ChartMsd, MsdRow, MsdStatusRow};
    use wolluf_store::repo::cache::play_ssr::{self as ssr_repo, PlaySsrRow, PlaySsrStatus};
    use wolluf_store::repo::ledger::alias;

    use super::*;
    use crate::features::library::testkit::Map;
    use crate::features::library::{Keys, Raters};
    use crate::features::players::selection::Decision;
    use crate::features::plays::testkit::Fixture;
    use crate::features::preview::dto::{RecItemDto, SkillPreviewDto, warning};
    use crate::features::preview::testkit::{
        OTHER, SELF, current_key, dense_4k, last_summary, play_id, score, ssr_runs, synced,
    };

    const PUSH_TARGET: i32 = 50;
    const BAND: (i32, i32) = (-50, 150);
    /// Far enough above any synthetic rating to sit outside every band.
    const FAR: i32 = 5_000;

    async fn self_entry(f: &Fixture) -> EntryRef {
        EntryRef::Profile(f.ctx.players().self_profile_id().await.unwrap().unwrap())
    }

    async fn recs(
        f: &Fixture,
        entry: EntryRef,
        keymode: Keymode,
        mode: RecsModeDto,
    ) -> RecsPreviewDto {
        f.ctx
            .preview()
            .recs(entry, keymode, mode, None, None, None)
            .await
            .unwrap()
    }

    async fn skill(f: &Fixture, entry: EntryRef, keymode: Keymode) -> SkillPreviewDto {
        let mut previews = f.ctx.preview().skill(entry, keymode, None).await.unwrap();
        assert_eq!(previews.len(), 1, "{previews:?}");
        previews.remove(0)
    }

    fn difficulty(keymode: u8) -> VersionKey {
        let parse = Keys::current().unwrap().parse;
        Raters::current(parse).unwrap().vkey(keymode).unwrap()
    }

    /// Replaces the chart's MinaCalc rows with hand-built ones.
    fn set_msd(f: &Fixture, m: &Map, rows: &[(u16, [i32; 8])]) {
        let md5: ChartMd5 = m.md5.parse().unwrap();
        let msd = ChartMsd {
            status: MsdStatusRow::Rated,
            hold_share_permille: 0,
            rows: rows
                .iter()
                .map(|&(rate_milli, centi)| MsdRow { rate_milli, centi })
                .collect(),
        };
        let vkey = difficulty(m.keys);
        f.ctx
            .cache_db()
            .write(move |tx| chart_msd::replace_for(tx, md5, vkey, &msd))
            .unwrap();
    }

    fn item<'a>(r: &'a RecsPreviewDto, m: &Map) -> Option<&'a RecItemDto> {
        r.items.iter().find(|i| i.md5 == m.md5)
    }

    fn codes(i: &RecItemDto) -> Vec<&str> {
        i.reasons.iter().map(|r| r.code.as_str()).collect()
    }

    /// Rosalind's play on Hard out-rates every self play: it would raise the self rating and mark
    /// Hard played if it leaked into the self scope.
    #[tokio::test]
    async fn recs_use_only_the_scopes_rating_and_played_set() {
        let a = Map::rice4("A").in_set(1);
        let b = Map::new("B", 4, dense_4k("B")).in_set(2);
        let hard = Map::new("Hard", 4, dense_4k("Hard")).in_set(3);
        let f = synced(
            &[a.clone(), b.clone(), hard.clone()],
            &[
                score(&a, SELF, 1),
                score(&b, SELF, 2),
                score(&hard, OTHER, 3),
            ],
        )
        .await;
        let me = self_entry(&f).await;
        let mine = skill(&f, me, Keymode::K4).await.overall_centi.unwrap();
        let everyone = skill(&f, EntryRef::AllPlayers, Keymode::K4)
            .await
            .overall_centi
            .unwrap();
        assert!(everyone > mine, "{everyone} vs {mine}");

        let target = mine + PUSH_TARGET;
        set_msd(&f, &hard, &[(1000, [target; 8])]);
        let r = recs(&f, me, Keymode::K4, RecsModeDto::Push).await;
        assert_eq!(r.state, RecsStateDto::Ready, "{r:?}");
        assert_eq!(r.focus, "overall");
        assert_eq!(r.rating_centi, mine);
        assert_eq!(r.band_centi, [target + BAND.0, target + BAND.1]);
        let h = item(&r, &hard).unwrap_or_else(|| panic!("{r:?}"));
        assert!(!h.played, "{h:?}");
        assert_eq!(codes(h), ["push", "unplayed"]);
        assert_eq!(h.reasons[0].args, [target.to_string()]);

        let target = everyone + PUSH_TARGET;
        set_msd(&f, &hard, &[(1000, [target; 8])]);
        let all = recs(&f, EntryRef::AllPlayers, Keymode::K4, RecsModeDto::Push).await;
        assert_eq!(all.rating_centi, everyone);
        assert_ne!(all.scope_hash, r.scope_hash);
        let h = item(&all, &hard).unwrap_or_else(|| panic!("{all:?}"));
        assert!(h.played, "{h:?}");
        assert_eq!(codes(h), ["push", "played_before"]);
    }

    /// Separate resolves one scope per self alias; the page shows the first. B's play is the
    /// stronger one, so it would move the rating if it leaked into the first scope.
    #[tokio::test]
    async fn separate_reads_the_first_resolved_scope() {
        const SECOND: &str = "Wulf";
        let a = Map::rice4("A").in_set(1);
        let b = Map::new("B", 4, dense_4k("B")).in_set(2);
        let f = synced(
            &[a.clone(), b.clone()],
            &[score(&a, SELF, 1), score(&b, SECOND, 2)],
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
        f.ctx
            .players()
            .decide(vec![(alias_id(SECOND), Some(Decision::Me))], false)
            .await
            .unwrap();
        let entry = self_entry(&f).await;
        let scopes = f
            .ctx
            .players()
            .resolve_scopes(entry, Keymode::K4, Some(MergeMode::Separate))
            .await
            .unwrap();
        assert_eq!(scopes.len(), 2, "{scopes:?}");
        // The other alias's chart, whichever alias resolves first.
        let theirs = if scopes[0].alias_ids == [alias_id(SELF)] {
            &b
        } else {
            &a
        };
        let first = f
            .ctx
            .preview()
            .skill(entry, Keymode::K4, Some(MergeMode::Separate))
            .await
            .unwrap()
            .into_iter()
            .find(|p| p.scope_hash == scopes[0].hash.to_string())
            .unwrap()
            .overall_centi
            .unwrap();
        let merged = f
            .ctx
            .preview()
            .skill(entry, Keymode::K4, Some(MergeMode::Merged))
            .await
            .unwrap()
            .remove(0)
            .overall_centi
            .unwrap();
        assert_ne!(
            merged, first,
            "the second alias's play moves a merged rating"
        );

        let target = first + PUSH_TARGET;
        set_msd(&f, theirs, &[(1000, [target; 8])]);
        let r = f
            .ctx
            .preview()
            .recs(
                entry,
                Keymode::K4,
                RecsModeDto::Push,
                None,
                Some(MergeMode::Separate),
                None,
            )
            .await
            .unwrap();
        assert_eq!(r.scope_hash, scopes[0].hash.to_string());
        assert_eq!(r.rating_centi, first, "{r:?}");
        assert_eq!(r.band_centi, [target + BAND.0, target + BAND.1]);
        let t = item(&r, theirs).unwrap_or_else(|| panic!("{r:?}"));
        assert!(!t.played, "{t:?}");
        assert_eq!(codes(t), ["push", "unplayed"]);
    }

    /// X is in band only at 1.2x; its rate copy sits in band at its own speed and at 1.2x more.
    #[tokio::test]
    async fn any_rate_adds_grid_rates_that_need_a_rate_copy() {
        let a = Map::rice4("A").in_set(1);
        let x = Map::new("X", 4, dense_4k("X")).in_set(2);
        let copy = Map::new("Copy", 4, dense_4k("Copy"))
            .named("300 wolluf - Copy", "Normal 1.2x")
            .in_set(3);
        let f = synced(&[a.clone(), x.clone(), copy.clone()], &[score(&a, SELF, 1)]).await;
        let me = self_entry(&f).await;
        let target = skill(&f, me, Keymode::K4).await.overall_centi.unwrap() + PUSH_TARGET;
        let far = [target + FAR; 8];
        set_msd(
            &f,
            &x,
            &[(750, far), (1000, far), (1200, [target; 8]), (1500, far)],
        );
        set_msd(&f, &copy, &[(1000, [target + 10; 8]), (1200, [target; 8])]);

        let off = recs(&f, me, Keymode::K4, RecsModeDto::Push).await;
        assert!(!off.any_rate);
        assert!(item(&off, &x).is_none(), "{off:?}");
        let c = item(&off, &copy).unwrap_or_else(|| panic!("{off:?}"));
        assert_eq!(c.rate_milli, 1000);
        assert!(c.is_rate_copy && !c.needs_rate_copy, "{c:?}");
        assert_eq!(codes(c), ["push", "unplayed", "rate_copy_in_library"]);
        for i in &off.items {
            assert!([750, 1000, 1500].contains(&i.rate_milli), "{i:?}");
            assert!(!i.needs_rate_copy, "{i:?}");
        }

        let ctx = &f.ctx;
        let once = |any_rate| async move {
            ctx.preview()
                .recs(me, Keymode::K4, RecsModeDto::Push, None, None, any_rate)
                .await
                .unwrap()
        };
        let on_once = once(Some(true)).await;
        assert!(on_once.any_rate);
        assert!(item(&on_once, &x).is_some(), "{on_once:?}");
        assert!(!f.ctx.settings().recs_any_rate().await.unwrap());

        f.ctx.settings().set_recs_any_rate(true).await.unwrap();
        let on = recs(&f, me, Keymode::K4, RecsModeDto::Push).await;
        assert!(on.any_rate);
        let xi = item(&on, &x).unwrap_or_else(|| panic!("{on:?}"));
        assert_eq!(xi.rate_milli, 1200);
        assert!(xi.needs_rate_copy && !xi.is_rate_copy, "{xi:?}");
        assert_eq!(codes(xi), ["push", "unplayed", "needs_rate_copy"]);
        assert_eq!(xi.reasons[2].args, ["1200"]);
        assert_eq!(xi.title, "X");
        assert_eq!(xi.set_id, Some(2));
        assert_eq!(xi.focus_centi, target);
        assert_eq!(xi.overall_centi, target);
        assert_eq!(xi.skillsets_centi, [target; 7]);
        let c = item(&on, &copy).unwrap_or_else(|| panic!("{on:?}"));
        assert_eq!(c.rate_milli, 1000, "a rate copy is never re-rated: {c:?}");

        let off_once = once(Some(false)).await;
        assert!(!off_once.any_rate);
        assert!(item(&off_once, &x).is_none(), "{off_once:?}");
        assert!(f.ctx.settings().recs_any_rate().await.unwrap());
    }

    /// A narrow band on the target itself, NM only, and a short list: each shows against charts
    /// the default params would list.
    #[tokio::test]
    async fn recs_params_come_from_the_service_params() {
        const NARROW: i32 = 10;
        let a = Map::rice4("A").in_set(1);
        let x = Map::new("X", 4, dense_4k("X")).in_set(2);
        let y = Map::new("Y", 4, dense_4k("Y")).in_set(3);
        let dt = Map::new("Dt", 4, dense_4k("Dt")).in_set(4);
        let f = synced(
            &[a.clone(), x.clone(), y.clone(), dt.clone()],
            &[score(&a, SELF, 1)],
        )
        .await;
        let me = self_entry(&f).await;
        let mine = skill(&f, me, Keymode::K4).await.overall_centi.unwrap();
        let far = [mine + FAR; 8];
        let at_rating = [mine + 1; 8];
        set_msd(&f, &x, &[(1000, at_rating)]);
        set_msd(&f, &y, &[(1000, at_rating)]);
        set_msd(&f, &dt, &[(1000, far), (1500, at_rating)]);

        let ctx = &f.ctx;
        let read = |recs: fn(u8) -> RecsParams| async move {
            let params = PreviewServiceParams {
                recs,
                ..PreviewServiceParams::default()
            };
            PreviewService::with_params(ctx, params)
                .recs(me, Keymode::K4, RecsModeDto::Push, None, None, None)
                .await
                .unwrap()
        };
        let default = read(RecsParams::for_keymode).await;
        assert_eq!(
            default.band_centi,
            [mine + PUSH_TARGET + BAND.0, mine + PUSH_TARGET + BAND.1]
        );
        assert!(item(&default, &dt).is_some(), "{default:?}");

        let narrow = read(|k| RecsParams {
            push_target_centi: 0,
            band_lo_centi: -NARROW,
            band_hi_centi: NARROW,
            base_rates_milli: vec![1000],
            ..RecsParams::for_keymode(k)
        })
        .await;
        assert_eq!(narrow.band_centi, [mine - NARROW, mine + NARROW]);
        assert!(
            item(&narrow, &x).is_some() && item(&narrow, &y).is_some(),
            "{narrow:?}"
        );
        assert!(item(&narrow, &dt).is_none(), "{narrow:?}");
        assert!(
            narrow.items.iter().all(|i| i.rate_milli == 1000),
            "{narrow:?}"
        );

        let short = read(|k| RecsParams {
            push_target_centi: 0,
            limit: 1,
            ..RecsParams::for_keymode(k)
        })
        .await;
        assert_eq!(short.items.len(), 1, "{short:?}");
    }

    /// MinaCalc's 7K Technical sits near 0.18 on real charts, so an unguarded Deficit would
    /// always pick it. The synthetic charts rate it high instead, so the play SSRs are pinned
    /// with Technical and Stamina lowest.
    #[tokio::test]
    async fn seven_k_deficit_never_focuses_technical_or_stamina() {
        const SSR: [i32; 8] = [2000, 3000, 2500, 3000, 200, 3000, 3000, 100];
        let s = Map::k7("Seven").in_set(1);
        let js = Map::k7("Jumps").in_set(2);
        let f = synced(&[s.clone(), js.clone()], &[score(&s, SELF, 1)]).await;
        let me = self_entry(&f).await;
        let id = play_id(&f, &s, SELF);
        f.ctx
            .cache_db()
            .write(move |tx| {
                ssr_repo::replace_many(
                    tx,
                    current_key(7),
                    &[PlaySsrRow {
                        play_id: id,
                        status: PlaySsrStatus::Counted,
                        rate_milli: 1000,
                        goal_permyriad: Some(9_650),
                        centi: Some(SSR),
                    }],
                )
            })
            .unwrap();
        let p = skill(&f, me, Keymode::K7).await;
        let lowest = p.skillsets.iter().min_by_key(|s| s.rating_centi).unwrap();
        assert!(
            ["technical", "stamina"].contains(&lowest.id.as_str()),
            "{p:?}"
        );
        let want = p
            .skillsets
            .iter()
            .filter(|s| s.id != "technical" && s.id != "stamina")
            .min_by_key(|s| s.rating_centi)
            .unwrap();
        assert_eq!(want.id, "jumpstream", "{p:?}");
        // Jumpstream leads every skillset but Technical, which stays inside its own margin.
        let r_js = want.rating_centi;
        set_msd(
            &f,
            &js,
            &[(1000, [r_js, 100, r_js, 100, 100, 100, 100, r_js + 100])],
        );

        let r = recs(&f, me, Keymode::K7, RecsModeDto::Deficit).await;
        assert_eq!(r.focus, "jumpstream", "{r:?}");
        assert_eq!(r.rating_centi, r_js);
        for code in [
            warning::UNCALIBRATED,
            warning::K7_LESS_VALIDATED,
            warning::K7_TECH_NOT_MEASURED,
        ] {
            assert!(r.warnings.iter().any(|w| w == code), "{code}: {r:?}");
        }
        let i = item(&r, &js).unwrap_or_else(|| panic!("{r:?}"));
        assert_eq!(i.focus_centi, r_js);
        assert_eq!(codes(i), ["deficit", "unplayed"]);
        assert_eq!(
            i.reasons[0].args,
            ["jumpstream".to_owned(), r_js.to_string()]
        );
    }

    #[tokio::test]
    async fn ln_heavy_charts_are_never_candidates() {
        let a = Map::rice4("A").in_set(1);
        let ln = Map::ln4("Holds").in_set(2);
        let f = synced(&[a.clone(), ln.clone()], &[score(&a, SELF, 1)]).await;
        let ln_md5: ChartMd5 = ln.md5.parse().unwrap();
        let a_md5: ChartMd5 = a.md5.parse().unwrap();
        let vkey = difficulty(4);
        let status = f
            .ctx
            .cache_db()
            .read(|c| chart_msd::get(c, ln_md5, vkey))
            .unwrap()
            .unwrap()
            .status;
        assert_eq!(status, MsdStatusRow::LnHeavy);

        let catalog: BTreeMap<ChartMd5, CatalogChart> = f
            .ctx
            .cache_db()
            .read(|c| catalog_chart::list_by_keymode(c, 4))
            .unwrap()
            .into_iter()
            .map(|c| (c.md5, c))
            .collect();
        assert!(catalog.contains_key(&ln_md5));
        let family = PreviewParams::default().family;
        let all = candidates(
            f.ctx.cache_db(),
            vkey,
            &[750, 1000, 1500],
            &catalog,
            &family,
        )
        .unwrap();
        assert!(all.iter().all(|c| c.md5 != ln_md5), "{all:?}");
        let mut a_rates: Vec<u16> = all
            .iter()
            .filter(|c| c.md5 == a_md5)
            .map(|c| c.rate_milli)
            .collect();
        a_rates.sort_unstable();
        assert_eq!(a_rates, [750, 1000, 1500]);
        let a_cand = all.iter().find(|c| c.md5 == a_md5).unwrap();
        assert_eq!(a_cand.set_key, "set:1");
        assert!(!a_cand.is_rate_copy);

        let me = self_entry(&f).await;
        f.ctx.settings().set_recs_any_rate(true).await.unwrap();
        for mode in [RecsModeDto::Push, RecsModeDto::Deficit] {
            let r = recs(&f, me, Keymode::K4, mode).await;
            assert!(item(&r, &ln).is_none(), "{r:?}");
        }
    }

    /// An unsubmitted map has no set id, so its folder groups the set.
    #[tokio::test]
    async fn sets_without_an_id_group_by_folder() {
        let a = Map::rice4("A").in_set(-1);
        let f = synced(std::slice::from_ref(&a), &[score(&a, SELF, 1)]).await;
        let catalog: BTreeMap<ChartMd5, CatalogChart> = f
            .ctx
            .cache_db()
            .read(|c| catalog_chart::list_by_keymode(c, 4))
            .unwrap()
            .into_iter()
            .map(|c| (c.md5, c))
            .collect();
        let family = PreviewParams::default().family;
        let all = candidates(f.ctx.cache_db(), difficulty(4), &[1000], &catalog, &family).unwrap();
        assert_eq!(all.len(), 1, "{all:?}");
        assert_eq!(all[0].set_key, "folder:100 wolluf - A");
    }

    #[tokio::test]
    async fn without_a_rating_the_list_is_empty() {
        let a = Map::rice4("A").in_set(1);
        let f = synced(std::slice::from_ref(&a), &[score(&a, SELF, 1)]).await;
        let me = self_entry(&f).await;
        let push = recs(&f, me, Keymode::K7, RecsModeDto::Push).await;
        assert_eq!(push.state, RecsStateDto::NoRating, "{push:?}");
        assert_eq!(push.focus, "overall");
        assert_eq!((push.rating_centi, push.band_centi), (0, [0, 0]));
        assert!(push.items.is_empty());
        assert_eq!(push.keymode, 7);
        assert_eq!(push.scope_hash.len(), 64);
        assert!(
            push.warnings
                .iter()
                .any(|w| w == warning::K7_LESS_VALIDATED)
        );
        let deficit = recs(&f, me, Keymode::K7, RecsModeDto::Deficit).await;
        assert_eq!(deficit.state, RecsStateDto::NoRating);
        assert_eq!(deficit.focus, "");
    }

    #[tokio::test]
    async fn computing_only_while_the_ssr_job_is_queued_or_running() {
        let a = Map::rice4("A").in_set(1);
        let f = synced(std::slice::from_ref(&a), &[score(&a, SELF, 1)]).await;
        let scope = f
            .ctx
            .players()
            .resolve_scopes(self_entry(&f).await, Keymode::K4, None)
            .await
            .unwrap()
            .remove(0);
        let params = PreviewServiceParams::default();
        let read = |job_pending| {
            let req = RecsRequest {
                mode: Mode::Push,
                any_rate: true,
            };
            recs_for_scope(
                f.ctx.user_db(),
                f.ctx.cache_db(),
                &params,
                &scope,
                req,
                job_pending,
            )
            .unwrap()
            .0
        };
        let active = read(true);
        assert_eq!(active.state, RecsStateDto::Computing, "{active:?}");
        assert!(active.rating_centi > 0, "the cached rating still shows");
        let idle = read(false);
        assert_eq!(idle.state, RecsStateDto::Ready, "{idle:?}");
        assert_eq!(idle.rating_centi, active.rating_centi);
    }

    /// A `play_ssr` VERSION bump orphans every row; the list starts the job itself, as the skill
    /// preview does, and a settled scope starts nothing.
    #[tokio::test]
    async fn pending_plays_start_the_ssr_job_and_read_as_computing() {
        let a = Map::rice4("A").in_set(1);
        let f = synced(std::slice::from_ref(&a), &[score(&a, SELF, 1)]).await;
        let me = self_entry(&f).await;
        let runs = ssr_runs(&f).await;
        f.ctx
            .cache_db()
            .write(|tx| ssr_repo::prune_except(tx, &[]))
            .unwrap();
        assert!(!f.ctx.jobs().is_active(JobKindDto::ComputePlaySsr));

        let orphaned = recs(&f, me, Keymode::K4, RecsModeDto::Push).await;
        assert_eq!(orphaned.state, RecsStateDto::Computing, "{orphaned:?}");
        f.ctx.jobs().wait_idle().await;
        assert_eq!(ssr_runs(&f).await, runs + 1);
        assert_eq!(last_summary(&f).await.computed, 1);

        let settled = recs(&f, me, Keymode::K4, RecsModeDto::Push).await;
        assert_eq!(settled.state, RecsStateDto::Ready, "{settled:?}");
        assert!(settled.rating_centi > 0, "{settled:?}");
        f.ctx.jobs().wait_idle().await;
        assert_eq!(
            ssr_runs(&f).await,
            runs + 1,
            "a settled list starts nothing"
        );
    }

    #[tokio::test]
    async fn skillset_mode_needs_a_known_skillset() {
        let a = Map::rice4("A").in_set(1);
        let f = synced(std::slice::from_ref(&a), &[score(&a, SELF, 1)]).await;
        let me = self_entry(&f).await;
        let preview = f.ctx.preview();
        let stream = preview
            .recs(
                me,
                Keymode::K4,
                RecsModeDto::Skillset,
                Some("stream".to_owned()),
                None,
                None,
            )
            .await
            .unwrap();
        assert_eq!(stream.focus, "stream");
        let p = skill(&f, me, Keymode::K4).await;
        assert_eq!(stream.rating_centi, p.skillsets[0].rating_centi);
        assert_eq!(stream.method, "preview.band_recs@1");
        assert_eq!(stream.calc_version, 527);
        for bad in [None, Some("nope".to_owned()), Some(String::new())] {
            let err = preview
                .recs(
                    me,
                    Keymode::K4,
                    RecsModeDto::Skillset,
                    bad.clone(),
                    None,
                    None,
                )
                .await
                .unwrap_err();
            assert_eq!(err.code, ErrorCode::InvalidInput, "{bad:?}");
        }
    }
}
