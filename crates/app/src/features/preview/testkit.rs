//! Synthetic installs for the preview tests: a self alias (the cfg login) and a second player.

use std::time::Duration;

use wolluf_core::{ChartMd5, DotNetTicks, PlayId, UnixUs, VersionKey};
use wolluf_engine::preview::PreviewParams;
use wolluf_engine::stage::play_ssr;
use wolluf_source_osu::codec::score_header::JudgementCounts;
use wolluf_source_osu::testkit::{FakeInstall, OsuDbBuilder, ScoreBuilder};
use wolluf_store::repo::cache::play_ssr::{self as ssr_repo, PlaySsrRow};
use wolluf_store::repo::ledger::{alias, play};

use crate::events::AppEvent;
use crate::features::library::testkit::{Map, osu_text};
use crate::features::library::{Keys, Raters};
use crate::features::players::testkit::pilot_cfg;
use crate::features::plays::testkit::{Fixture, scores_db};
use crate::jobs::dto::{ComputePlaySsrSummaryDto, JobKindDto, JobStatusDto, JobSummaryDto};

use super::ComputePlaySsrJob;

pub(crate) const SELF: &str = "TWulfZ";
pub(crate) const OTHER: &str = "Rosalind";
pub(crate) const RANDOM: u32 = 1 << 21;
pub(crate) const SCORE_V2: u32 = 1 << 29;
pub(crate) const DT: u32 = 1 << 6;
const WAIT: Duration = Duration::from_secs(60);

/// 72 jumps 70 ms apart: harder than `Map::rice4`, and its 144 notes fit the 150 judgements of a
/// default `ScoreBuilder` play, so that play is complete.
pub(crate) fn dense_4k(title: &str) -> Vec<u8> {
    const JUMPS: [(u8, u8); 4] = [(0, 1), (2, 3), (1, 2), (0, 3)];
    let taps: Vec<(u8, i32)> = (0..72)
        .flat_map(|i: i32| {
            let (a, b) = JUMPS[usize::try_from(i).unwrap() % JUMPS.len()];
            let t = 1_000 + i * 70;
            [(a, t), (b, t)]
        })
        .collect();
    osu_text(4, title, &taps, &[])
}

/// The first second of a UTC month in .NET ticks.
pub(crate) fn month_ticks(year: i32, month: u32) -> DotNetTicks {
    const DOTNET_TO_UNIX_TICKS: i64 = 621_355_968_000_000_000;
    const TICKS_PER_SECOND: i64 = 10_000_000;
    // Days from 1970-01-01 (Howard Hinnant's days_from_civil).
    let (y, m) = if month <= 2 {
        (year - 1, month + 9)
    } else {
        (year, month - 3)
    };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * m + 2) / 5;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + i32::try_from(doy).unwrap();
    let days = i64::from(era) * 146_097 + i64::from(doe) - 719_468;
    DotNetTicks(DOTNET_TO_UNIX_TICKS + days * 86_400 * TICKS_PER_SECOND)
}

pub(crate) fn counts(max: u16, n300: u16, miss: u16) -> JudgementCounts {
    JudgementCounts {
        geki: max,
        n300,
        miss,
        ..JudgementCounts::default()
    }
}

/// `nth` keeps every play's natural key distinct and orders `played_at` one second apart.
pub(crate) fn score(m: &Map, player: &str, nth: i64) -> ScoreBuilder {
    ScoreBuilder::mania(&m.md5, player, nth)
}

/// The cfg login is `SELF`, so the self profile holds that alias alone (ADR 0005).
pub(crate) fn install(maps: &[Map], scores: &[ScoreBuilder]) -> FakeInstall {
    let osu_db = maps
        .iter()
        .fold(OsuDbBuilder::new(), |db, m| db.beatmap(m.beatmap()))
        .encode();
    maps.iter()
        .filter_map(|m| Some((m.rel_path(), m.bytes.clone()?)))
        .fold(
            FakeInstall::new()
                .osu_db(osu_db)
                .scores_db(scores_db(scores))
                .cfg("fixture", pilot_cfg(SELF)),
            |install, (rel, bytes)| install.song(rel, bytes),
        )
}

/// Syncs and waits for the chained identity refresh, index and SSR jobs.
pub(crate) async fn synced(maps: &[Map], scores: &[ScoreBuilder]) -> Fixture {
    let f = Fixture::new(&install(maps, scores)).await;
    f.sync().await;
    f
}

pub(crate) fn current_key(keymode: u8) -> VersionKey {
    let parse = Keys::current().unwrap().parse;
    let difficulty = Raters::current(parse).unwrap().vkey(keymode).unwrap();
    let p = PreviewParams::default();
    play_ssr::vkey(difficulty, &p.goal, &p.exclusion).unwrap()
}

/// Every ledger play as `(player, chart, played_at, id)`, oldest first.
pub(crate) fn plays(f: &Fixture) -> Vec<(String, ChartMd5, UnixUs, PlayId)> {
    let aliases = f.ctx.user_db().read(alias::list).unwrap();
    let ids: Vec<_> = aliases.iter().map(|a| a.id).collect();
    let mut out: Vec<_> = f
        .ctx
        .user_db()
        .read(|c| play::since(c, &ids, UnixUs(0)))
        .unwrap()
        .into_iter()
        .map(|p| {
            let name = aliases
                .iter()
                .find(|a| a.id == p.alias_id)
                .map(|a| String::from_utf8_lossy(&a.raw_name).into_owned())
                .unwrap();
            (name, p.chart_md5, p.played_at, p.id)
        })
        .collect();
    out.sort_by_key(|p| p.2);
    out
}

pub(crate) fn play_id(f: &Fixture, m: &Map, player: &str) -> PlayId {
    let md5: ChartMd5 = m.md5.parse().unwrap();
    let found: Vec<PlayId> = plays(f)
        .into_iter()
        .filter(|p| p.0 == player && p.1 == md5)
        .map(|p| p.3)
        .collect();
    assert_eq!(found.len(), 1, "{player} on {}", m.title);
    found[0]
}

pub(crate) fn row(f: &Fixture, vkey: VersionKey, id: PlayId) -> Option<PlaySsrRow> {
    f.ctx
        .cache_db()
        .read(|c| ssr_repo::get_many(c, vkey, &[id]))
        .unwrap()
        .into_iter()
        .next()
}

/// Submits one `ComputePlaySsr` and waits for it and anything it chains.
pub(crate) async fn recompute(f: &Fixture) -> ComputePlaySsrSummaryDto {
    let mut rx = f.ctx.subscribe();
    let id = f.ctx.jobs().submit(Box::new(ComputePlaySsrJob));
    tokio::time::timeout(WAIT, async {
        loop {
            if let AppEvent::JobFinished(fin) = rx.recv().await.unwrap()
                && fin.job_id == id
            {
                return;
            }
        }
    })
    .await
    .expect("ComputePlaySsr finished in time");
    f.ctx.jobs().wait_idle().await;
    last_summary(f).await
}

/// `ComputePlaySsr` runs in the job history, finished or not.
pub(crate) async fn ssr_runs(f: &Fixture) -> usize {
    f.ctx
        .jobs()
        .list(None)
        .await
        .unwrap()
        .iter()
        .filter(|j| j.kind == JobKindDto::ComputePlaySsr)
        .count()
}

pub(crate) async fn last_summary(f: &Fixture) -> ComputePlaySsrSummaryDto {
    let job = f
        .ctx
        .jobs()
        .list(None)
        .await
        .unwrap()
        .into_iter()
        .find(|j| j.kind == JobKindDto::ComputePlaySsr)
        .expect("a ComputePlaySsr job ran");
    assert_eq!(job.status, JobStatusDto::Ok, "{job:?}");
    match job.summary {
        Some(JobSummaryDto::ComputePlaySsr(s)) => s,
        other => panic!("unexpected summary {other:?}"),
    }
}
