//! `SyncPlays` (spec 003 Behaviour): catalog → ingest → archive → finish, one idempotent pass.
//! "Incremental" only means the pass finds little new work; there is no second code path.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use wolluf_core::{StageId, UnixUs, VersionKey, VersionKeyBuilder};
use wolluf_source_osu::codec::osu_db::{OsuDb, OsuDbBeatmap, decode_osu_db};
use wolluf_source_osu::snapshot::{Snapshot, SnapshotPolicy, read_stable};
use wolluf_source_osu::{CodecError, Diagnostics, SourceError};
use wolluf_store::repo::cache::{
    CatalogChart, Derivation, DerivationStatus, catalog_chart, derivation,
};
use wolluf_store::repo::ledger::{
    GameInstall, InstallId, NewSnapshot, SnapshotKind, SourceSnapshot, game_install,
    source_snapshot,
};

use crate::context::blocking_join_error;
use crate::errors::AppError;
use crate::jobs::dto::{JobKindDto, JobStageDto, JobSummaryDto, SyncSummaryDto};
use crate::jobs::{Job, JobCtx, JobFuture, JobSummary};

pub const CATALOG_STAGE: StageId = StageId::from_static("catalog");
/// Bump when the catalog rows derived from one osu!.db change (spec 003 "Versioned stages").
pub const CATALOG_VERSION: u32 = 1;

const OSU_DB: &str = "osu!.db";
const MANIA_MODE: u8 = 3;
/// Spec 003 "Stable read": one extra attempt for a DB osu! may still be writing.
const TORN_WRITE_RETRY: Duration = Duration::from_secs(2);
const DOMAIN_PLAYS: &str = "plays";

pub struct SyncPlaysJob {
    install_id: InstallId,
    policy: SnapshotPolicy,
}

impl SyncPlaysJob {
    pub fn new(install_id: InstallId) -> Self {
        Self {
            install_id,
            policy: SnapshotPolicy::default(),
        }
    }
}

impl Job for SyncPlaysJob {
    fn kind(&self) -> JobKindDto {
        JobKindDto::SyncPlays
    }

    fn dedupe_key(&self) -> String {
        format!("sync_plays:{}", self.install_id.0)
    }

    fn params(&self) -> serde_json::Value {
        serde_json::json!({ "installId": self.install_id.0 })
    }

    fn run(self: Box<Self>, ctx: JobCtx) -> JobFuture {
        Box::pin(async move {
            tokio::task::spawn_blocking(move || {
                let install = ctx
                    .user
                    .read(|c| game_install::get(c, self.install_id))?
                    .ok_or_else(|| {
                        AppError::not_found().with_arg("installId", self.install_id.0.to_string())
                    })?;
                Sync {
                    ctx: &ctx,
                    install,
                    policy: &self.policy,
                    summary: SyncSummaryDto::default(),
                    changed: false,
                }
                .run()
            })
            .await
            .map_err(blocking_join_error)?
        })
    }
}

struct Sync<'a> {
    ctx: &'a JobCtx,
    install: GameInstall,
    policy: &'a SnapshotPolicy,
    summary: SyncSummaryDto,
    changed: bool,
}

pub(crate) fn unix_us(t: SystemTime) -> UnixUs {
    match t.duration_since(UNIX_EPOCH) {
        Ok(d) => UnixUs(i64::try_from(d.as_micros()).unwrap_or(i64::MAX)),
        Err(e) => UnixUs(i64::try_from(e.duration().as_micros()).map_or(i64::MIN, |us| -us)),
    }
}

/// osu! runs on case-insensitive file systems, so `Scores.db` must be found too.
fn root_file(root: &Path, name: &str) -> PathBuf {
    std::fs::read_dir(root)
        .ok()
        .and_then(|entries| {
            entries
                .flatten()
                .find(|e| e.file_name().to_string_lossy().eq_ignore_ascii_case(name))
                .map(|e| e.path())
        })
        .unwrap_or_else(|| root.join(name))
}

fn stat(path: &Path) -> Result<(u64, UnixUs), AppError> {
    let meta =
        std::fs::metadata(path).map_err(|_| AppError::osu_dir_not_found(path.to_string_lossy()))?;
    let mtime = meta
        .modified()
        .map_err(|e| AppError::internal(format!("mtime of {}: {e}", path.display())))?;
    Ok((meta.len(), unix_us(mtime)))
}

fn lossy(s: &wolluf_source_osu::codec::OsuString) -> String {
    s.to_string_lossy()
        .map(|c| c.into_owned())
        .unwrap_or_default()
}

/// Mania entries only; the first entry wins when osu!.db lists one md5 twice (a map copied into
/// two folders), because `catalog_chart.md5` is the key.
fn catalog_rows(db: &OsuDb) -> Vec<CatalogChart> {
    let mut rows = std::collections::BTreeMap::new();
    for b in db.beatmaps.iter().filter(|b| b.mode == MANIA_MODE) {
        let Some(md5) = b.beatmap_md5() else { continue };
        rows.entry(md5).or_insert_with(|| catalog_row(md5, b));
    }
    rows.into_values().collect()
}

fn catalog_row(md5: wolluf_core::ChartMd5, b: &OsuDbBeatmap) -> CatalogChart {
    let positive = |id: i32| (id > 0).then_some(id);
    CatalogChart {
        md5,
        // osu!mania keeps the key count in CS (spec 003 step 1: `round(CS)`).
        keymode: b.circle_size.round().clamp(0.0, f32::from(u8::MAX)) as u8,
        title: lossy(&b.title),
        artist: lossy(&b.artist),
        version: lossy(&b.difficulty),
        creator: lossy(&b.creator),
        set_id: positive(b.beatmapset_id),
        beatmap_id: positive(b.beatmap_id),
        path: format!("{}/{}", lossy(&b.folder), lossy(&b.osu_file)),
        od: f64::from(b.overall_difficulty),
        hp: f64::from(b.hp_drain),
        length_ms: u32::try_from(b.total_time_ms).unwrap_or(0),
    }
}

fn catalog_vkey(osu_db_sha: wolluf_core::BlobSha256) -> Result<VersionKey, AppError> {
    VersionKeyBuilder::new(CATALOG_STAGE, CATALOG_VERSION)
        .input(osu_db_sha.0)
        .finish()
        .map_err(|e| AppError::internal(format!("catalog vkey: {e}")))
}

impl Sync<'_> {
    fn run(mut self) -> Result<JobSummary, AppError> {
        self.catalog()?;
        self.ctx.check_cancelled()?;
        self.summary.failed_items = self.ctx.failed_items();
        Ok(JobSummary {
            summary: Some(JobSummaryDto::SyncPlays(self.summary)),
            changed: if self.changed {
                vec![DOMAIN_PLAYS]
            } else {
                Vec::new()
            },
            follow_ups: Vec::new(),
        })
    }

    fn root(&self) -> &Path {
        &self.install.root_path
    }

    /// Reads with the stable-read rule and decodes, giving a torn write one late retry.
    fn read_decoded<T>(
        &self,
        path: &Path,
        decode: impl Fn(&[u8]) -> Result<(T, Diagnostics), CodecError>,
    ) -> Result<(Snapshot, T), AppError> {
        let snapshot = read_stable(path, self.policy)?;
        match decode(&snapshot.bytes) {
            Ok((value, _)) => Ok((snapshot, value)),
            Err(e) if e.is_possibly_torn_write() => {
                (self.policy.sleep)(TORN_WRITE_RETRY);
                let snapshot = read_stable(path, self.policy)?;
                let (value, _) = decode(&snapshot.bytes).map_err(SourceError::from)?;
                Ok((snapshot, value))
            }
            Err(e) => Err(SourceError::from(e).into()),
        }
    }

    fn catalog_built(&self, sha: wolluf_core::BlobSha256) -> Result<bool, AppError> {
        let vkey = catalog_vkey(sha)?;
        let key = sha.to_string();
        Ok(self
            .ctx
            .cache
            .read(|c| derivation::get(c, &CATALOG_STAGE, &key, vkey))?
            .is_some())
    }

    /// Step 1. Returns the osu!.db snapshot the catalog now reflects.
    fn catalog(&mut self) -> Result<SourceSnapshot, AppError> {
        let progress = &self.ctx.progress;
        progress.report(JobStageDto::Catalog, 0, 1);
        let path = root_file(self.root(), OSU_DB);
        let (size, mtime) = stat(&path)?;
        let install_id = self.install.id;
        let latest = self
            .ctx
            .user
            .read(|c| source_snapshot::latest(c, install_id, SnapshotKind::OsuDb))?;
        if let Some(latest) = latest.as_ref().filter(|l| l.matches_stat(size, mtime))
            && self.catalog_built(latest.sha256)?
        {
            progress.report(JobStageDto::Catalog, 1, 1);
            return Ok(latest.clone());
        }

        let snapshot = read_stable(&path, self.policy)?;
        let reused = latest.filter(|l| l.sha256 == snapshot.sha256);
        if let Some(known) = &reused
            && self.catalog_built(known.sha256)?
        {
            progress.report(JobStageDto::Catalog, 1, 1);
            return Ok(known.clone());
        }
        let (snapshot, db) = match decode_osu_db(&snapshot.bytes) {
            Ok((db, _)) => (snapshot, db),
            Err(e) if e.is_possibly_torn_write() => self.read_decoded(&path, decode_osu_db)?,
            Err(e) => return Err(SourceError::from(e).into()),
        };
        let row = match reused.filter(|r| r.sha256 == snapshot.sha256) {
            Some(row) => row,
            None => self.insert_snapshot(SnapshotKind::OsuDb, &snapshot, db.version)?,
        };

        let rows = catalog_rows(&db);
        let vkey = catalog_vkey(row.sha256)?;
        let snapshot_id = row.id;
        let input_key = row.sha256.to_string();
        self.ctx.cache.write(move |tx| {
            catalog_chart::replace_all(tx, snapshot_id, &rows)?;
            derivation::put(
                tx,
                &Derivation {
                    stage: CATALOG_STAGE,
                    input_key,
                    vkey,
                    status: DerivationStatus::Ok,
                    error_code: None,
                    error_msg: None,
                    duration_ms: None,
                },
            )
        })?;
        self.changed = true;
        progress.report(JobStageDto::Catalog, 1, 1);
        Ok(row)
    }

    fn insert_snapshot(
        &self,
        kind: SnapshotKind,
        snapshot: &Snapshot,
        format_version: i32,
    ) -> Result<SourceSnapshot, AppError> {
        let new = NewSnapshot {
            install_id: self.install.id,
            kind,
            sha256: snapshot.sha256,
            size: snapshot.size,
            mtime: unix_us(snapshot.mtime),
            format_version: Some(format_version),
            imported_at: self.ctx.clock.now(),
        };
        let install_id = self.install.id;
        let row = self.ctx.user.write(move |tx| {
            source_snapshot::insert(tx, &new)?;
            source_snapshot::latest(tx.conn(), install_id, kind)
        })?;
        row.ok_or_else(|| AppError::internal("snapshot row vanished after insert"))
    }
}

#[cfg(test)]
mod tests {
    use wolluf_source_osu::testkit::{BeatmapBuilder, FakeInstall, OsuDbBuilder};
    use wolluf_store::repo::cache::catalog_chart;

    use super::*;
    use crate::features::plays::testkit::{Fixture, md5_hex};

    fn osu_db(md5s: &[&str]) -> Vec<u8> {
        md5s.iter()
            .fold(OsuDbBuilder::new(), |db, md5| {
                db.beatmap(BeatmapBuilder::mania(md5, 7).build())
            })
            .beatmap(
                BeatmapBuilder::mania(&md5_hex(b"std map"), 4)
                    .mode(0)
                    .build(),
            )
            .encode()
    }

    fn snapshots(f: &Fixture, kind: SnapshotKind) -> Option<SourceSnapshot> {
        f.ctx
            .user_db()
            .read(|c| source_snapshot::latest(c, f.install, kind))
            .unwrap()
    }

    fn catalog(f: &Fixture) -> Vec<CatalogChart> {
        f.ctx.cache_db().read(catalog_chart::list_all).unwrap()
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn catalog_skipped_when_osu_db_unchanged() {
        let a = md5_hex(b"chart a");
        let f = Fixture::new(&FakeInstall::new().osu_db(osu_db(&[&a]))).await;
        let (_, first) = f.sync().await;
        assert_eq!(first.failed_items, 0);
        let snap = snapshots(&f, SnapshotKind::OsuDb).unwrap();
        assert_eq!(catalog(&f).len(), 1, "std maps stay out of the catalog");

        // Same size and mtime but garbage bytes: only a real read would notice.
        let path = f.root.join(OSU_DB);
        let meta = std::fs::metadata(&path).unwrap();
        let garbage = vec![0xee; usize::try_from(meta.len()).unwrap()];
        std::fs::write(&path, &garbage).unwrap();
        std::fs::File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_modified(meta.modified().unwrap())
            .unwrap();
        let (fin, _) = f.sync().await;
        assert_eq!(fin.failed_items, 0);
        assert_eq!(snapshots(&f, SnapshotKind::OsuDb).unwrap().id, snap.id);
        assert_eq!(catalog(&f).len(), 1);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn catalog_replaced_on_new_sha() {
        let (a, b, c) = (md5_hex(b"a"), md5_hex(b"b"), md5_hex(b"c"));
        let f = Fixture::new(&FakeInstall::new().osu_db(osu_db(&[&a, &b]))).await;
        f.sync().await;
        let first = snapshots(&f, SnapshotKind::OsuDb).unwrap();
        let md5s =
            |f: &Fixture| -> Vec<String> { catalog(f).iter().map(|c| c.md5.to_string()).collect() };
        let mut expected = vec![a.clone(), b.clone()];
        expected.sort();
        assert_eq!(md5s(&f), expected);

        f.write(OSU_DB, &osu_db(&[&c]));
        f.sync().await;
        let second = snapshots(&f, SnapshotKind::OsuDb).unwrap();
        assert_ne!(first.id, second.id);
        assert_ne!(first.sha256, second.sha256);
        assert_eq!(md5s(&f), vec![c]);
        let charts = catalog(&f);
        assert_eq!(charts[0].keymode, 7);
        assert_eq!(charts[0].path, format!("{0}/{0}.osu", charts[0].md5));
        let vkey = catalog_vkey(second.sha256).unwrap();
        let row = f
            .ctx
            .cache_db()
            .read(|c| derivation::get(c, &CATALOG_STAGE, &second.sha256.to_string(), vkey))
            .unwrap()
            .unwrap();
        assert_eq!(row.status, DerivationStatus::Ok);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn catalog_rebuilt_after_cache_loss() {
        let a = md5_hex(b"a");
        let f = Fixture::new(&FakeInstall::new().osu_db(osu_db(&[&a]))).await;
        f.sync().await;
        let snap = snapshots(&f, SnapshotKind::OsuDb).unwrap();
        let f = f.reopen_without_cache();
        f.sync().await;
        assert_eq!(catalog(&f).len(), 1);
        assert_eq!(
            snapshots(&f, SnapshotKind::OsuDb).unwrap().id,
            snap.id,
            "an unchanged osu!.db reuses its snapshot row"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn missing_osu_db_fails_with_osu_dir_not_found() {
        let f = Fixture::new(&FakeInstall::new()).await;
        std::fs::remove_file(f.root.join(OSU_DB)).unwrap();
        let mut rx = f.ctx.subscribe();
        let id = f.ctx.jobs().submit(Box::new(SyncPlaysJob::new(f.install)));
        loop {
            if let crate::events::AppEvent::JobFinished(fin) = rx.recv().await.unwrap()
                && fin.job_id == id
            {
                assert_eq!(fin.status, crate::jobs::JobStatusDto::Failed);
                break;
            }
        }
        let job = f.ctx.jobs().list(None).await.unwrap().remove(0);
        assert_eq!(
            job.error.map(|e| e.code),
            Some(crate::errors::ErrorCodeDto::OsuDirNotFound)
        );
    }
}
