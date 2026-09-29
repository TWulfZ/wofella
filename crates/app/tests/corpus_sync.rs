#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! Spec 003 AC17: the pilot corpus ingests, and a second sync adds nothing. The expected counts
//! come from the corpus files directly (scores.db and a `Data/r` listing), never from the ledger.

mod common;

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use wolluf_app::clock::SystemClock;
use wolluf_app::context::{AppContext, AppPaths, InstallId};
use wolluf_app::jobs::JobStatusDto;
use wolluf_app::jobs::dto::{JobSummaryDto, SyncSummaryDto};
use wolluf_core::{ChartMd5, Clock, DotNetTicks, FileTime};
use wolluf_source_osu::codec::osr::decode_osr;
use wolluf_source_osu::codec::replay_name::{ReplayFileKind, ReplayFileName};
use wolluf_source_osu::codec::score_header::ScoreHeader;
use wolluf_source_osu::codec::scores_db::decode_scores_db;
use wolluf_store::open_user_db;
use wolluf_store::repo::ledger::play;

const SCORES_DB: &str = "scores.db";
const REPLAY_DIR: &str = "Data/r";
const MANIA_MODE: u8 = 3;
/// Architecture §12's "4.3k plays": the older 7K-only count (research 03 L307).
const PILOT_MIN_PLAYS: u64 = 4_338;

type ReplayKey = (ChartMd5, FileTime);
type PlayKey = (ChartMd5, Vec<u8>, FileTime);

fn chart_md5(h: &ScoreHeader) -> ChartMd5 {
    let text = std::str::from_utf8(h.beatmap_md5.as_bytes().unwrap()).unwrap();
    ChartMd5::from_str(text).unwrap()
}

fn filetime(h: &ScoreHeader) -> FileTime {
    DotNetTicks(h.timestamp_ticks).to_filetime().unwrap()
}

/// The ledger's natural key over every mania record of one scores.db snapshot.
fn mania_play_keys(scores_db: &[u8]) -> (usize, BTreeSet<PlayKey>) {
    let (db, _) = decode_scores_db(scores_db).unwrap();
    let mania: Vec<&ScoreHeader> = db
        .scores()
        .map(|r| &r.header)
        .filter(|h| h.mode == MANIA_MODE)
        .collect();
    let keys = mania
        .iter()
        .map(|h| {
            (
                chart_md5(h),
                h.player.bytes_or_empty().to_vec(),
                filetime(h),
            )
        })
        .collect();
    (mania.len(), keys)
}

fn osr_files(root: &Path) -> Vec<(ReplayKey, std::path::PathBuf)> {
    let mut out: Vec<(ReplayKey, std::path::PathBuf)> = fs::read_dir(root.join(REPLAY_DIR))
        .unwrap()
        .filter_map(|e| {
            let path = e.unwrap().path();
            let name = ReplayFileName::parse(path.file_name()?.to_str()?)?;
            (name.kind == ReplayFileKind::Osr).then_some(((name.md5, name.filetime), path))
        })
        .collect();
    out.sort();
    out
}

struct Expected {
    mania_rows: usize,
    distinct_plays: u64,
    replay_only: u64,
    replays_linked: u64,
}

fn expected(root: &Path, scores_db: &[u8]) -> Expected {
    let (mania_rows, keys) = mania_play_keys(scores_db);
    let score_replays: BTreeSet<ReplayKey> = keys.iter().map(|(m, _, t)| (*m, *t)).collect();
    let osr = osr_files(root);
    let with_osr: BTreeSet<ReplayKey> = osr.iter().map(|(k, _)| *k).collect();
    let replay_only = osr
        .iter()
        .filter(|(k, _)| !score_replays.contains(k))
        .filter(|(_, path)| {
            let (file, _) = decode_osr(&fs::read(path).unwrap()).unwrap();
            file.header.mode == MANIA_MODE
        })
        .count();
    let replays_linked = keys
        .iter()
        .filter(|(m, _, t)| with_osr.contains(&(*m, *t)))
        .count();
    Expected {
        mania_rows,
        distinct_plays: keys.len() as u64,
        replay_only: replay_only as u64,
        replays_linked: replays_linked as u64,
    }
}

fn vault_files(dir: &Path) -> usize {
    let Ok(entries) = fs::read_dir(dir) else {
        return 0;
    };
    entries
        .map(|e| e.unwrap().path())
        .map(|p| if p.is_dir() { vault_files(&p) } else { 1 })
        .sum()
}

async fn timed_sync(ctx: &AppContext, install: InstallId) -> (SyncSummaryDto, Duration) {
    let start = Instant::now();
    let job = ctx.plays().sync_and_wait(install).await.unwrap();
    let elapsed = start.elapsed();
    assert_eq!(job.status, JobStatusDto::Ok, "{job:?}");
    let Some(JobSummaryDto::SyncPlays(summary)) = job.summary else {
        panic!("sync finished without a summary: {job:?}");
    };
    (summary, elapsed)
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "corpus: needs WOLLUF_CORPUS"]
async fn corpus_sync_pilot() {
    let root = common::corpus();
    let tree_before = common::tree_state(&root);
    let scores_db = fs::read(root.join(SCORES_DB)).unwrap();
    let want = expected(&root, &scores_db);

    let data = tempfile::tempdir().unwrap();
    let paths = AppPaths::from_data_dir(data.path().join("data"));
    let clock: Arc<dyn Clock> = Arc::new(SystemClock);
    let ctx = AppContext::open(paths.clone(), clock.clone()).unwrap();
    let install = ctx.register_install(root.clone(), None).await.unwrap();

    let (first, first_time) = timed_sync(&ctx, install).await;
    assert_eq!(
        fs::read(root.join(SCORES_DB)).unwrap(),
        scores_db,
        "scores.db changed during the sync (is osu! running?); the expected counts are stale"
    );
    let vault_after_first = vault_files(&paths.vault_dir());
    let (second, second_time) = timed_sync(&ctx, install).await;
    let vault_after_second = vault_files(&paths.vault_dir());
    drop(ctx);

    println!(
        "corpus_sync_pilot: first sync {first_time:?}, second sync {second_time:?}; \
         mania rows {}, distinct plays {}, replay-only {}, replays linked {}, vault files {}",
        want.mania_rows,
        want.distinct_plays,
        want.replay_only,
        want.replays_linked,
        vault_after_first
    );
    println!("first: {first:?}");
    println!("second: {second:?}");

    let user = open_user_db(&paths.user_db(), &paths.backups_dir(), clock.now()).unwrap();
    let plays = user.read(play::count).unwrap();
    assert_eq!(plays, want.distinct_plays + want.replay_only);
    assert!(
        plays >= PILOT_MIN_PLAYS,
        "{plays} plays, fewer than {PILOT_MIN_PLAYS}"
    );
    assert_eq!(u64::from(first.plays_new), want.distinct_plays);
    assert_eq!(u64::from(first.plays_replay_only), want.replay_only);
    assert_eq!(u64::from(first.replays_linked), want.replays_linked);

    assert_eq!(second.plays_new, 0);
    assert_eq!(second.plays_replay_only, 0);
    assert_eq!(vault_after_second, vault_after_first, "no new vault files");

    common::assert_unchanged("two syncs", &tree_before, &common::tree_state(&root));
}
