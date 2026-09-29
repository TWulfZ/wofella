#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! Spec 003 AC16: cache.db is disposable. Deleting it and syncing once rebuilds the same
//! derived state an incremental history produced, and user.db does not change.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use md5::{Digest, Md5};
use wolluf_app::context::{AppContext, AppPaths, InstallId};
use wolluf_app::jobs::JobStatusDto;
use wolluf_core::{BlobSha256, FixedClock, UnixUs};
use wolluf_engine::stage::{chart_label, chart_parse};
use wolluf_source_osu::testkit::{
    BeatmapBuilder, FakeInstall, OsrBuilder, OsuDbBuilder, ScoreBuilder, ScoresDbBuilder,
};
use wolluf_store::open_cache_db;
use wolluf_store::repo::cache::{CatalogChart, Derivation, catalog_chart, derivation};

const T0: UnixUs = UnixUs(1_790_637_236_636_000);
const CHART_A: &[u8] = b"osu file format v14\n[Metadata]\nTitle:A\n";
const CHART_B: &[u8] = b"osu file format v14\n[Metadata]\nTitle:B\n";
const CHART_B_EDITED: &[u8] = b"osu file format v14\n[Metadata]\nTitle:B (edited)\n";

fn md5_hex(bytes: &[u8]) -> String {
    Md5::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn write(root: &Path, rel: &Path, bytes: &[u8]) {
    let path = root.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, bytes).unwrap();
}

fn osu_db(md5s: &[&str]) -> Vec<u8> {
    md5s.iter()
        .fold(OsuDbBuilder::new(), |db, md5| {
            db.beatmap(BeatmapBuilder::mania(md5, 7).build())
        })
        .encode()
}

struct Install {
    root: PathBuf,
    data: PathBuf,
    chart_a: String,
    chart_b: String,
    chart_c: String,
}

/// Plays on three charts, replays for two of them, one chart edited in place (a
/// `chart_archive` skip row) and one chart missing from the first osu!.db.
fn install(dir: &Path) -> Install {
    let (chart_a, chart_b) = (md5_hex(CHART_A), md5_hex(CHART_B));
    let chart_c = md5_hex(b"chart c arrives with the second osu!.db");
    let scores = [
        ScoreBuilder::mania(&chart_a, "TWulfZ", 1),
        ScoreBuilder::mania(&chart_b, "TWulfZ", 2),
        ScoreBuilder::mania(&chart_c, "Rosalind", 3),
    ];
    let mut fake = FakeInstall::new()
        .osu_db(osu_db(&[&chart_a, &chart_b]))
        .scores_db(
            scores
                .iter()
                .fold(ScoresDbBuilder::new(), |db, s| db.score(s.clone().build()))
                .encode(),
        )
        .song(format!("{chart_a}/{chart_a}.osu"), CHART_A.to_vec())
        .song(format!("{chart_b}/{chart_b}.osu"), CHART_B_EDITED.to_vec());
    for score in scores.iter().take(2) {
        let osr = OsrBuilder::new(score.clone());
        fake = fake.replay(&osr.file_name().unwrap(), osr.build());
    }
    let root = dir.join("osu!");
    for (rel, bytes) in fake.files() {
        write(&root, &rel, &bytes);
    }
    Install {
        root,
        data: dir.join("data"),
        chart_a,
        chart_b,
        chart_c,
    }
}

fn open(data: &Path) -> AppContext {
    AppContext::open(
        AppPaths::from_data_dir(data.to_path_buf()),
        Arc::new(FixedClock::new(T0)),
    )
    .unwrap()
}

async fn sync(ctx: &AppContext, install: InstallId) {
    let job = ctx.plays().sync_and_wait(install).await.unwrap();
    assert_eq!(job.status, JobStatusDto::Ok, "{job:?}");
}

/// The whole on-disk state: main file plus WAL. The store's read-only pool connections close
/// last, and a read-only connection never checkpoints, so committed pages may still sit in the
/// WAL. Without a checkpoint the WAL only appends, so any commit changes this pair.
fn user_db_state(data: &Path) -> (BlobSha256, Option<BlobSha256>) {
    let sha = |p: PathBuf| {
        std::fs::read(p)
            .ok()
            .map(|b| wolluf_store::vault::sha256(&b))
    };
    (
        sha(data.join("user.db")).expect("user.db exists"),
        sha(data.join("user.db-wal")),
    )
}

struct CacheDump {
    catalog: Vec<CatalogChart>,
    derivations: Vec<Derivation>,
}

fn dump_cache(data: &Path) -> CacheDump {
    let cache = open_cache_db(&data.join("cache.db")).unwrap();
    CacheDump {
        catalog: cache.read(catalog_chart::list_all).unwrap(),
        derivations: cache.read(derivation::list_all).unwrap(),
    }
}

/// Rows derived from `osu_db_sha`: the catalog row keyed by it and the chart-archive skips
/// keyed `<md5>:<sha>`.
fn derived_from(osu_db_sha: &str, d: &Derivation) -> bool {
    d.input_key == osu_db_sha || d.input_key.ends_with(&format!(":{osu_db_sha}"))
}

#[tokio::test(flavor = "multi_thread")]
async fn cache_rebuild_from_empty_equals_incremental() {
    let dir = tempfile::tempdir().unwrap();
    let inst = install(dir.path());

    let ctx = open(&inst.data);
    let id = ctx.register_install(inst.root.clone(), None).await.unwrap();
    sync(&ctx, id).await;
    let first_osu_db = osu_db_sha(&inst.root);
    write(
        &inst.root,
        Path::new("osu!.db"),
        &osu_db(&[&inst.chart_a, &inst.chart_b, &inst.chart_c]),
    );
    sync(&ctx, id).await;
    let current_osu_db = osu_db_sha(&inst.root);
    let user_after_incremental = user_db_state(&inst.data);
    drop(ctx);
    let incremental = dump_cache(&inst.data);

    for f in ["cache.db", "cache.db-wal", "cache.db-shm"] {
        let _ = std::fs::remove_file(inst.data.join(f));
    }
    let ctx = open(&inst.data);
    sync(&ctx, id).await;
    let user_after_rebuild = user_db_state(&inst.data);
    drop(ctx);
    let rebuilt = dump_cache(&inst.data);

    assert_eq!(incremental.catalog, rebuilt.catalog);
    assert_eq!(rebuilt.catalog.len(), 3);
    assert_eq!(
        user_after_rebuild, user_after_incremental,
        "user.db unchanged"
    );

    // cache.db has no row deletion for derivations yet, so the incremental history keeps the
    // memo rows of the superseded osu!.db; nothing reads them once the sha changed. Library
    // rows are keyed by chart md5 alone, so both histories must hold the same ones.
    let (live, stale): (Vec<Derivation>, Vec<Derivation>) = incremental
        .derivations
        .into_iter()
        .partition(|d| derived_from(&current_osu_db, d) || is_library(d));
    assert!(
        live.iter().any(is_library),
        "the chained library index is part of the compared state: {live:?}"
    );
    assert_eq!(live, rebuilt.derivations);
    assert!(
        live.iter().any(|d| d.input_key.starts_with(&inst.chart_b)),
        "the edited chart's skip row is part of the compared state: {live:?}"
    );
    assert!(
        stale.iter().all(|d| derived_from(&first_osu_db, d)),
        "only rows of the superseded osu!.db differ: {stale:?}"
    );
}

fn is_library(d: &Derivation) -> bool {
    [chart_parse::STAGE, chart_label::STAGE].contains(&d.stage)
}

/// The osu!.db sha as the derivation keys spell it.
fn osu_db_sha(root: &Path) -> String {
    wolluf_store::vault::sha256(&std::fs::read(root.join("osu!.db")).unwrap()).to_string()
}
