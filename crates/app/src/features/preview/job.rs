//! `ComputePlaySsr` (ADR 0024): one `play_ssr` row per ledger play of every alias on a catalog
//! chart rated by MinaCalc, memoized per play under the keymode's `play_ssr` key. Every alias is
//! cached because the All players entry reads them; the service filters by scope (ADR 0005).

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::mpsc;

use wolluf_core::{ChartMd5, ErrorCode, PlayId, UnixUs, VersionKey};
use wolluf_engine::preview::{Completeness, PlayCounts, PlayMods, PreviewParams, goal_permyriad};
use wolluf_engine::rows_blob::decode_rows;
use wolluf_engine::stage::difficulty::Calc;
use wolluf_engine::stage::play_ssr;
use wolluf_store::repo::cache::chart_msd::{self, MsdStatusRow};
use wolluf_store::repo::cache::play_ssr::{self as ssr_repo, PlaySsrRow, PlaySsrStatus};
use wolluf_store::repo::cache::{CatalogChart, catalog_chart, chart_parsed};
use wolluf_store::repo::ledger::{Play, ScoreSystem, alias, play};
use wolluf_store::{Conn, DbHandle, StoreError};

use crate::context::blocking_join_error;
use crate::errors::AppError;
use crate::features::library::{Keys, Raters};
use crate::jobs::dto::{ComputePlaySsrSummaryDto, JobKindDto, JobStageDto, JobSummaryDto};
use crate::jobs::{ItemError, Job, JobCtx, JobFuture, JobSummary, panic_text};

pub(super) const DOMAIN_PREVIEW: &str = "preview";
/// Architecture §7: writer transactions of ~500–5k rows.
const WRITE_BATCH: usize = 500;
const WRITE_QUEUE: usize = 2 * WRITE_BATCH;
const PERMYRIAD: f32 = 10_000.0;

#[derive(Debug, Default)]
pub struct ComputePlaySsrJob;

impl ComputePlaySsrJob {
    pub const DEDUPE_KEY: &'static str = "preview.play_ssr";
}

impl Job for ComputePlaySsrJob {
    fn kind(&self) -> JobKindDto {
        JobKindDto::ComputePlaySsr
    }

    fn dedupe_key(&self) -> String {
        Self::DEDUPE_KEY.to_owned()
    }

    fn params(&self) -> serde_json::Value {
        serde_json::json!({})
    }

    fn run(self: Box<Self>, ctx: JobCtx) -> JobFuture {
        Box::pin(async move {
            tokio::task::spawn_blocking(move || compute(&ctx, &PreviewParams::default()))
                .await
                .map_err(blocking_join_error)?
        })
    }
}

thread_local! {
    // MinaCalc keeps native scratch buffers and is not Sync; the rayon workers outlive a job, so
    // each builds its calculator once.
    static CALC: RefCell<Option<Calc>> = const { RefCell::new(None) };
}

/// The `play_ssr` key of every keymode rated by MinaCalc, with the `difficulty` key under it.
#[derive(Debug, Clone, Copy)]
struct KeymodeKeys {
    keymode: u8,
    difficulty: VersionKey,
    ssr: VersionKey,
}

pub(super) fn ssr_vkey(
    keymode: u8,
    params: &PreviewParams,
) -> Result<Option<VersionKey>, AppError> {
    Ok(keymode_keys(params)?
        .into_iter()
        .find(|k| k.keymode == keymode)
        .map(|k| k.ssr))
}

fn keymode_keys(params: &PreviewParams) -> Result<Vec<KeymodeKeys>, AppError> {
    let raters = Raters::current(Keys::current()?.parse)?;
    raters
        .keys()
        .iter()
        .map(|&(keymode, difficulty)| {
            let ssr = play_ssr::vkey(difficulty, &params.goal, &params.exclusion)
                .map_err(|e| AppError::internal(format!("play_ssr vkey: {e}")))?;
            Ok(KeymodeKeys {
                keymode,
                difficulty,
                ssr,
            })
        })
        .collect()
}

struct Item {
    play: Play,
    keys: KeymodeKeys,
    od: f32,
}

/// Plays with no row under their keymode's key, plus `no_chart` rows whose chart is now parsed
/// and rated: a parse that lands later must not leave the play excluded forever.
fn plan(
    conn: Conn<'_>,
    plays: Vec<Play>,
    catalog: &BTreeMap<ChartMd5, CatalogChart>,
    keys: &[KeymodeKeys],
    parse: VersionKey,
) -> Result<(Vec<Item>, u32), StoreError> {
    let mut by_keymode: BTreeMap<u8, Vec<Item>> = BTreeMap::new();
    for p in plays {
        let Some(chart) = catalog.get(&p.chart_md5) else {
            continue;
        };
        let Some(k) = keys.iter().find(|k| k.keymode == chart.keymode) else {
            continue;
        };
        by_keymode.entry(k.keymode).or_default().push(Item {
            od: chart.od as f32,
            keys: *k,
            play: p,
        });
    }
    let mut due = Vec::new();
    let mut total = 0_u32;
    for items in by_keymode.into_values() {
        total += count(items.len());
        let Some(k) = items.first().map(|i| i.keys) else {
            continue;
        };
        let ids: Vec<PlayId> = items.iter().map(|i| i.play.id).collect();
        let mut wanted: BTreeSet<PlayId> = ssr_repo::missing_among(conn, k.ssr, &ids)?
            .into_iter()
            .collect();
        let no_chart: BTreeSet<PlayId> = ssr_repo::get_many(conn, k.ssr, &ids)?
            .into_iter()
            .filter(|r| r.status == PlaySsrStatus::NoChart)
            .map(|r| r.play_id)
            .collect();
        let mut ready: BTreeMap<ChartMd5, bool> = BTreeMap::new();
        for item in items.iter().filter(|i| no_chart.contains(&i.play.id)) {
            let md5 = item.play.chart_md5;
            let now_rated = match ready.get(&md5) {
                Some(r) => *r,
                None => {
                    let r = chart_parsed::exists(conn, md5, parse)?
                        && chart_msd::get(conn, md5, k.difficulty)?.is_some();
                    ready.insert(md5, r);
                    r
                }
            };
            if now_rated {
                wanted.insert(item.play.id);
            }
        }
        due.extend(items.into_iter().filter(|i| wanted.contains(&i.play.id)));
    }
    Ok((due, total))
}

fn count(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

fn engine_counts(c: wolluf_store::repo::ledger::PlayCounts) -> PlayCounts {
    PlayCounts {
        max: c.max,
        n300: c.n300,
        n200: c.n200,
        n100: c.n100,
        n50: c.n50,
        miss: c.miss,
    }
}

struct Env<'a> {
    cache: &'a DbHandle,
    parse: VersionKey,
    params: &'a PreviewParams,
}

/// The rules of ADR 0024 in order: mods the chart's notes no longer describe, ScoreV2, no
/// parse, too few judgements, then the chart's own MSD status. The goal is kept on excluded rows
/// for display; the rate is the mod rate the SSR is computed at.
fn rate_play(env: &Env<'_>, item: &Item) -> Result<PlaySsrRow, ItemError> {
    let internal = |e: String| ItemError::new(ErrorCode::Internal, e);
    let p = &item.play;
    let mods = PlayMods::from_bits(p.mods);
    let counts = engine_counts(p.counts);
    let goal = goal_permyriad(counts, item.od, mods, &env.params.goal);
    let row = |status, centi| PlaySsrRow {
        play_id: p.id,
        status,
        rate_milli: mods.rate_milli(),
        goal_permyriad: goal,
        centi,
    };
    if mods.unsupported(&env.params.exclusion) {
        return Ok(row(PlaySsrStatus::UnsupportedMods, None));
    }
    if mods.score_v2 || p.score_system == ScoreSystem::V2 {
        return Ok(row(PlaySsrStatus::ScoreV2, None));
    }
    let md5 = p.chart_md5;
    let (parsed, msd) = env
        .cache
        .read(|c| {
            Ok((
                chart_parsed::get(c, md5, env.parse)?,
                chart_msd::get(c, md5, item.keys.difficulty)?,
            ))
        })
        .map_err(|e: StoreError| internal(e.to_string()))?;
    let (Some(parsed), Some(msd)) = (parsed, msd) else {
        return Ok(row(PlaySsrStatus::NoChart, None));
    };
    let complete = Completeness::of(&counts, mods.score_system(), parsed.n_notes, parsed.n_ln);
    let Some(goal) = goal.filter(|_| complete == Completeness::Complete) else {
        return Ok(row(PlaySsrStatus::Incomplete, None));
    };
    match msd.status {
        MsdStatusRow::LnHeavy => return Ok(row(PlaySsrStatus::LnHeavy, None)),
        MsdStatusRow::CalcRejected => return Ok(row(PlaySsrStatus::CalcRejected, None)),
        MsdStatusRow::Rated => {}
    }
    let chart =
        decode_rows(&parsed.rows_blob).map_err(|e| internal(format!("rows blob of {md5}: {e}")))?;
    let ssr = CALC.with_borrow_mut(|slot| {
        let calc = match slot {
            Some(calc) => calc,
            None => slot
                .insert(Calc::new().map_err(|e| internal(format!("MinaCalc unavailable: {e}")))?),
        };
        Ok::<_, ItemError>(play_ssr::run(
            calc,
            &chart,
            mods.rate_milli(),
            f32::from(goal) / PERMYRIAD,
        ))
    })?;
    Ok(match ssr {
        Ok(centi) => row(PlaySsrStatus::Counted, Some(centi)),
        Err(_) => row(PlaySsrStatus::CalcRejected, None),
    })
}

#[derive(Debug, Default)]
struct Written {
    rows: u32,
    counted: u32,
}

fn write_batch(
    cache: &DbHandle,
    batch: Vec<(VersionKey, PlaySsrRow)>,
    written: &mut Written,
) -> Result<(), AppError> {
    if batch.is_empty() {
        return Ok(());
    }
    let mut by_key: BTreeMap<VersionKey, Vec<PlaySsrRow>> = BTreeMap::new();
    for (vkey, row) in batch {
        by_key.entry(vkey).or_default().push(row);
    }
    let rows = count(by_key.values().map(Vec::len).sum());
    let counted = count(
        by_key
            .values()
            .flatten()
            .filter(|r| r.status == PlaySsrStatus::Counted)
            .count(),
    );
    cache.write(move |tx| {
        for (vkey, rows) in &by_key {
            ssr_repo::replace_many(tx, *vkey, rows)?;
        }
        Ok(())
    })?;
    written.rows += rows;
    written.counted += counted;
    Ok(())
}

fn write_loop(
    cache: &DbHandle,
    writes: mpsc::Receiver<(VersionKey, PlaySsrRow)>,
) -> Result<Written, AppError> {
    let mut written = Written::default();
    let mut batch = Vec::with_capacity(WRITE_BATCH);
    for w in writes {
        batch.push(w);
        if batch.len() >= WRITE_BATCH {
            write_batch(cache, std::mem::take(&mut batch), &mut written)?;
        }
    }
    write_batch(cache, batch, &mut written)?;
    Ok(written)
}

pub(super) fn compute(ctx: &JobCtx, params: &PreviewParams) -> Result<JobSummary, AppError> {
    let parse = Keys::current()?.parse;
    let keys = keymode_keys(params)?;
    let mut summary = ComputePlaySsrSummaryDto::default();
    let catalog: BTreeMap<ChartMd5, CatalogChart> = ctx
        .cache
        .read(catalog_chart::list_all)?
        .into_iter()
        .map(|c| (c.md5, c))
        .collect();
    let aliases: Vec<_> = ctx
        .user
        .read(alias::list)?
        .into_iter()
        .map(|a| a.id)
        .collect();
    let plays = ctx.user.read(|c| play::since(c, &aliases, UnixUs(0)))?;
    let (items, total) = ctx.cache.read(|c| plan(c, plays, &catalog, &keys, parse))?;
    summary.plays_total = total;
    summary.skipped_memoized = total - count(items.len());

    let env = Env {
        cache: &ctx.cache,
        parse,
        params,
    };
    let (writes, queue) = mpsc::sync_channel::<(VersionKey, PlaySsrRow)>(WRITE_QUEUE);
    let (results, written) = std::thread::scope(|s| {
        let writer = s.spawn(|| write_loop(&ctx.cache, queue));
        let results = ctx.run_items(
            JobStageDto::PlaySsr,
            &items,
            |i| i.play.id.to_string(),
            |i| {
                let row = rate_play(&env, i)?;
                // A closed queue means the writer failed; the job ends with its error.
                let _ = writes.send((i.keys.ssr, row));
                Ok(())
            },
        );
        drop(writes);
        let written = writer.join().unwrap_or_else(|p| {
            Err(AppError::internal(format!(
                "play_ssr writer panicked: {}",
                panic_text(&*p)
            )))
        });
        (results, written)
    });
    let written = written?;
    results?;
    summary.computed = written.rows;
    summary.counted = written.counted;
    summary.excluded = written.rows - written.counted;
    ctx.check_cancelled()?;

    let keep: Vec<VersionKey> = keys.iter().map(|k| k.ssr).collect();
    let pruned = ctx
        .cache
        .write(move |tx| ssr_repo::prune_except(tx, &keep))?;
    summary.failed_items = ctx.failed_items();
    let changed = written.rows > 0 || pruned > 0;
    Ok(JobSummary {
        summary: Some(JobSummaryDto::ComputePlaySsr(summary)),
        changed: if changed {
            vec![DOMAIN_PREVIEW]
        } else {
            Vec::new()
        },
        follow_ups: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use wolluf_core::VersionKey;
    use wolluf_engine::preview::{GoalParams, PreviewParams};
    use wolluf_engine::stage::play_ssr;
    use wolluf_store::repo::cache::play_ssr::{self as ssr_repo, PlaySsrRow, PlaySsrStatus};

    use super::*;
    use crate::features::library::testkit::{Map, osu_text, reindex};
    use crate::features::library::{Keys, Raters};
    use crate::features::preview::testkit::{
        DT, OTHER, RANDOM, SCORE_V2, SELF, counts, current_key, last_summary, play_id, recompute,
        row, score, synced,
    };
    use crate::jobs::JobKindDto;

    fn status(f: &crate::features::plays::testkit::Fixture, m: &Map, player: &str) -> PlaySsrRow {
        let key = current_key(m.keys);
        row(f, key, play_id(f, m, player)).unwrap_or_else(|| panic!("no row for {}", m.title))
    }

    #[tokio::test]
    async fn every_rule_gives_its_status() {
        let a = Map::rice4("A");
        let random = Map::rice4("Random");
        let v2 = Map::rice4("V2");
        let short = Map::rice4("Short");
        let ln = Map::ln4("Ln");
        let gone = Map::rice4("Gone").missing();
        let empty = Map::new("Empty", 4, osu_text(4, "Empty", &[], &[]));
        let k7 = Map::k7("Seven");
        let maps = [&a, &random, &v2, &short, &ln, &gone, &empty, &k7].map(Clone::clone);
        let scores = [
            score(&a, SELF, 1),
            score(&random, SELF, 2).mods(RANDOM),
            score(&v2, SELF, 3).mods(SCORE_V2),
            // 10 judgements on a 64-note chart.
            score(&short, SELF, 4).counts(counts(10, 0, 0)),
            score(&ln, SELF, 5),
            score(&gone, SELF, 6),
            score(&empty, SELF, 7),
            score(&k7, OTHER, 8),
        ];
        let f = synced(&maps, &scores).await;

        let counted = status(&f, &a, SELF);
        assert_eq!(counted.status, PlaySsrStatus::Counted, "{counted:?}");
        assert_eq!(counted.rate_milli, 1000);
        let goal = counted.goal_permyriad.unwrap();
        assert!((9_000..=9_650).contains(&goal), "{goal}");
        let centi = counted.centi.unwrap();
        assert!(centi[0] > 0, "{centi:?}");
        for (m, want) in [
            (&random, PlaySsrStatus::UnsupportedMods),
            (&v2, PlaySsrStatus::ScoreV2),
            (&short, PlaySsrStatus::Incomplete),
            (&ln, PlaySsrStatus::LnHeavy),
            (&gone, PlaySsrStatus::NoChart),
            (&empty, PlaySsrStatus::CalcRejected),
        ] {
            let r = status(&f, m, SELF);
            assert_eq!(r.status, want, "{}: {r:?}", m.title);
            assert_eq!(r.centi, None, "{}", m.title);
        }
        // Other players' plays are cached too: the All players entry reads them.
        assert_eq!(status(&f, &k7, OTHER).status, PlaySsrStatus::Counted);

        let s = last_summary(&f).await;
        assert_eq!(s.plays_total, 8, "{s:?}");
        assert_eq!(s.computed, 8, "{s:?}");
        assert_eq!((s.counted, s.excluded), (2, 6), "{s:?}");
        assert_eq!(s.failed_items, 0, "{s:?}");
    }

    #[tokio::test]
    async fn double_time_is_rated_at_its_rate() {
        let a = Map::rice4("A");
        let f = synced(
            std::slice::from_ref(&a),
            &[score(&a, SELF, 1), score(&a, SELF, 2).mods(DT)],
        )
        .await;
        let ids = crate::features::preview::testkit::plays(&f);
        let rows: Vec<PlaySsrRow> = ids
            .iter()
            .map(|p| row(&f, current_key(4), p.3).unwrap())
            .collect();
        assert_eq!(rows[0].rate_milli, 1000);
        assert_eq!(rows[1].rate_milli, 1500);
        assert!(
            rows[1].centi.unwrap()[0] > rows[0].centi.unwrap()[0],
            "{rows:?}"
        );
    }

    #[tokio::test]
    async fn a_rerun_computes_nothing() {
        let a = Map::rice4("A");
        let b = Map::k7("B");
        let f = synced(
            &[a.clone(), b.clone()],
            &[score(&a, SELF, 1), score(&b, SELF, 2)],
        )
        .await;
        let s = recompute(&f).await;
        assert_eq!(s.plays_total, 2, "{s:?}");
        assert_eq!((s.computed, s.skipped_memoized), (0, 2), "{s:?}");
    }

    #[tokio::test]
    async fn a_chart_that_comes_back_is_rated() {
        let a = Map::rice4("A");
        let bytes = a.bytes.clone().unwrap();
        let gone = a.clone().missing();
        let f = synced(std::slice::from_ref(&gone), &[score(&gone, SELF, 1)]).await;
        assert_eq!(status(&f, &a, SELF).status, PlaySsrStatus::NoChart);

        f.write(format!("Songs/{}", a.rel_path()), &bytes);
        reindex(&f).await;
        f.ctx.jobs().wait_idle().await;
        assert_eq!(status(&f, &a, SELF).status, PlaySsrStatus::Counted);
    }

    #[tokio::test]
    async fn a_new_key_recomputes_and_prunes_the_old_one() {
        let a = Map::rice4("A");
        let f = synced(std::slice::from_ref(&a), &[score(&a, SELF, 1)]).await;
        let old = current_key(4);
        let id = play_id(&f, &a, SELF);
        assert!(row(&f, old, id).is_some());

        let params = PreviewParams {
            goal: GoalParams {
                cap: 0.9,
                ..GoalParams::default()
            },
            ..PreviewParams::default()
        };
        let ctx = f.ctx.jobs().test_ctx(JobKindDto::ComputePlaySsr);
        let summary = compute(&ctx, &params).unwrap();
        let Some(JobSummaryDto::ComputePlaySsr(s)) = summary.summary else {
            panic!("{summary:?}");
        };
        assert_eq!((s.computed, s.skipped_memoized), (1, 0), "{s:?}");
        assert_eq!(summary.changed, vec!["preview"]);

        let parse = Keys::current().unwrap().parse;
        let difficulty = Raters::current(parse).unwrap().vkey(4).unwrap();
        let new: VersionKey = play_ssr::vkey(difficulty, &params.goal, &params.exclusion).unwrap();
        let fresh = row(&f, new, id).unwrap();
        assert!(fresh.goal_permyriad.unwrap() <= 9_000, "{fresh:?}");
        assert_eq!(row(&f, old, id), None);
        let left = f
            .ctx
            .cache_db()
            .read(|c| ssr_repo::missing_among(c, old, &[id]))
            .unwrap();
        assert_eq!(left, vec![id]);
    }
}
