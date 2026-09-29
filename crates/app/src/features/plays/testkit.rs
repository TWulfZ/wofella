//! Synthetic osu! installs for SyncPlays tests (spec 003 AC10), materialised in a tempdir from
//! 002's builders.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use md5::{Digest, Md5};
use wolluf_core::{FixedClock, UnixUs};
use wolluf_source_osu::codec::OsuString;
use wolluf_source_osu::codec::replay_name::{ReplayFileKind, ReplayFileName};
use wolluf_source_osu::codec::score_header::mods;
use wolluf_source_osu::testkit::{
    BeatmapBuilder, FakeInstall, OsrBuilder, OsuDbBuilder, ScoreBuilder, ScoresDbBuilder,
};

use crate::context::{AppContext, AppPaths, InstallId};
use crate::events::{AppEvent, JobFinishedDto};
use crate::features::plays::SyncPlaysJob;
use crate::jobs::dto::{JobStatusDto, JobSummaryDto, SyncSummaryDto};

pub(crate) const T0: UnixUs = UnixUs(1_790_637_236_636_000);
const WAIT: Duration = Duration::from_secs(30);

pub(crate) fn md5_hex(bytes: &[u8]) -> String {
    Md5::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

pub(crate) struct Fixture {
    pub(crate) dir: tempfile::TempDir,
    pub(crate) root: PathBuf,
    pub(crate) ctx: AppContext,
    pub(crate) install: InstallId,
}

pub(crate) fn write_file(root: &Path, rel: &Path, bytes: &[u8]) {
    let path = root.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, bytes).unwrap();
}

impl Fixture {
    pub(crate) async fn new(install: &FakeInstall) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("osu!");
        std::fs::create_dir_all(&root).unwrap();
        for (rel, bytes) in install.files() {
            write_file(&root, &rel, &bytes);
        }
        let ctx = AppContext::open(
            AppPaths::from_data_dir(dir.path().join("data")),
            Arc::new(FixedClock::new(T0)),
        )
        .unwrap();
        let install = ctx
            .register_install(root.clone(), Some(20260924))
            .await
            .unwrap();
        Self {
            dir,
            root,
            ctx,
            install,
        }
    }

    pub(crate) fn write(&self, rel: impl AsRef<Path>, bytes: &[u8]) {
        write_file(&self.root, rel.as_ref(), bytes);
    }

    /// Replaces cache.db with nothing and reopens the context, like a user deleting the file.
    pub(crate) fn reopen_without_cache(self) -> Self {
        let Self {
            dir,
            root,
            ctx,
            install,
        } = self;
        drop(ctx);
        let data = dir.path().join("data");
        for f in ["cache.db", "cache.db-wal", "cache.db-shm"] {
            let _ = std::fs::remove_file(data.join(f));
        }
        let ctx =
            AppContext::open(AppPaths::from_data_dir(data), Arc::new(FixedClock::new(T0))).unwrap();
        Self {
            dir,
            root,
            ctx,
            install,
        }
    }

    pub(crate) async fn sync(&self) -> (JobFinishedDto, SyncSummaryDto) {
        let mut rx = self.ctx.subscribe();
        let id = self
            .ctx
            .jobs()
            .submit(Box::new(SyncPlaysJob::new(self.install)));
        let finished = tokio::time::timeout(WAIT, async {
            loop {
                if let AppEvent::JobFinished(f) = rx.recv().await.unwrap()
                    && f.job_id == id
                {
                    return f;
                }
            }
        })
        .await
        .expect("sync finished in time");
        let job = self
            .ctx
            .jobs()
            .list(None)
            .await
            .unwrap()
            .into_iter()
            .find(|j| j.id == id)
            .unwrap();
        assert_eq!(
            finished.status,
            JobStatusDto::Ok,
            "sync failed: {:?}",
            job.error
        );
        let summary = match job.summary {
            Some(JobSummaryDto::SyncPlays(s)) => s,
            other => panic!("unexpected summary {other:?}"),
        };
        (finished, summary)
    }
}

/// The spec 003 AC10 install: 6 mania scores (incl. `""` and a non-UTF-8 name), 1 osu!std
/// score, 1 exact duplicate; `.osr` for 5 plays and `.osg` for 3; one consistent orphan and one
/// orphan whose header md5 differs from its name; two Songs charts, one edited in place.
// The orphans are read by the archive-step tests (003 T17).
#[allow(dead_code)]
pub(crate) struct Ac10 {
    pub(crate) install: FakeInstall,
    pub(crate) scores: Vec<ScoreBuilder>,
    pub(crate) chart_a: String,
    pub(crate) chart_b: String,
    pub(crate) chart_c: String,
    pub(crate) orphan: ScoreBuilder,
    pub(crate) mismatched_orphan: ReplayFileName,
}

pub(crate) const NON_UTF8_NAME: &[u8] = &[0xff, 0xfe, b'x'];
const CHART_A: &[u8] = b"osu file format v14\n[Metadata]\nTitle:A\n";
const CHART_B_ORIGINAL: &[u8] = b"osu file format v14\n[Metadata]\nTitle:B\n";
const CHART_B_EDITED: &[u8] = b"osu file format v14\n[Metadata]\nTitle:B (edited)\n";

pub(crate) fn osr_name(score: &ScoreBuilder) -> ReplayFileName {
    OsrBuilder::new(score.clone()).file_name().unwrap()
}

pub(crate) fn osg_name(score: &ScoreBuilder) -> ReplayFileName {
    ReplayFileName {
        kind: ReplayFileKind::Osg,
        ..osr_name(score)
    }
}

pub(crate) fn osg_bytes(score: &ScoreBuilder) -> Vec<u8> {
    format!("osg for {:?}", osr_name(score).filetime).into_bytes()
}

pub(crate) fn scores_db(scores: &[ScoreBuilder]) -> Vec<u8> {
    scores
        .iter()
        .fold(ScoresDbBuilder::new(), |db, s| db.score(s.clone().build()))
        .encode()
}

pub(crate) fn ac10() -> Ac10 {
    let chart_a = md5_hex(CHART_A);
    let chart_b = md5_hex(CHART_B_ORIGINAL);
    let chart_c = md5_hex(b"chart c is not in osu!.db");
    let scores = vec![
        ScoreBuilder::mania(&chart_a, "TWulfZ", 1),
        ScoreBuilder::mania(&chart_a, "", 2),
        ScoreBuilder::mania(&chart_b, "x", 3).player(OsuString::present(NON_UTF8_NAME)),
        ScoreBuilder::mania(&chart_b, "W", 4).mods(mods::SCORE_V2),
        ScoreBuilder::mania(&chart_c, "TWulfZ", 5).online_id(4_567_890_123),
        ScoreBuilder::mania(&chart_c, "Rosalind", 6),
        ScoreBuilder::mania(&chart_a, "TWulfZ", 7).mode(0),
        ScoreBuilder::mania(&chart_a, "TWulfZ", 1),
    ];
    let orphan = ScoreBuilder::mania(&chart_a, "TWulfZ", 8);
    let mismatched = ScoreBuilder::mania(&chart_a, "TWulfZ", 9);
    let mismatched_orphan = ReplayFileName {
        md5: chart_b.parse().unwrap(),
        ..osr_name(&mismatched)
    };
    let osu_db = OsuDbBuilder::new()
        .beatmap(BeatmapBuilder::mania(&chart_a, 7).build())
        .beatmap(BeatmapBuilder::mania(&chart_b, 7).build())
        .encode();
    let mut install = FakeInstall::new()
        .osu_db(osu_db)
        .scores_db(scores_db(&scores))
        .song(format!("{chart_a}/{chart_a}.osu"), CHART_A.to_vec())
        .song(format!("{chart_b}/{chart_b}.osu"), CHART_B_EDITED.to_vec())
        .replay(&osr_name(&orphan), OsrBuilder::new(orphan.clone()).build())
        .replay(&mismatched_orphan, OsrBuilder::new(mismatched).build());
    for (i, score) in scores.iter().take(5).enumerate() {
        install = install.replay(&osr_name(score), OsrBuilder::new(score.clone()).build());
        if i < 3 {
            install = install.replay(&osg_name(score), osg_bytes(score));
        }
    }
    Ac10 {
        install,
        scores,
        chart_a,
        chart_b,
        chart_c,
        orphan,
        mismatched_orphan,
    }
}
