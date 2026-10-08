//! Synthetic libraries for the library tests: osu!.db rows plus their `.osu` files, written into
//! a temp install by `plays::testkit::Fixture`.

use std::time::Duration;

use wolluf_source_osu::codec::OsuString;
use wolluf_source_osu::codec::osu_db::OsuDbBeatmap;
use wolluf_source_osu::testkit::{BeatmapBuilder, FakeInstall, OsuDbBuilder, ScoreBuilder};

use crate::events::AppEvent;
use crate::features::plays::testkit::{Fixture, md5_hex, scores_db};
use crate::jobs::dto::{IndexLibrarySummaryDto, JobDto, JobKindDto, JobStatusDto, JobSummaryDto};

const WAIT: Duration = Duration::from_secs(30);

/// A valid v14 osu!mania file. `taps` are `(column, ms)`, `holds` `(column, head ms, tail ms)`.
/// `wolluf-chart`'s `OsuText` is out of reach: app may not depend on chart (layers.toml).
pub(crate) fn osu_text(
    keys: u8,
    title: &str,
    taps: &[(u8, i32)],
    holds: &[(u8, i32, i32)],
) -> Vec<u8> {
    osu_text_timed(keys, title, &["0,500,4,1,0,100,1,0"], taps, holds)
}

/// [`osu_text`] with its own `[TimingPoints]` lines.
pub(crate) fn osu_text_timed(
    keys: u8,
    title: &str,
    timing: &[&str],
    taps: &[(u8, i32)],
    holds: &[(u8, i32, i32)],
) -> Vec<u8> {
    let x = |col: u8| (2 * u32::from(col) + 1) * 256 / u32::from(keys);
    let timing = timing.join("\n");
    let mut text = format!(
        "osu file format v14\n\n[General]\nAudioFilename: audio.mp3\nMode: 3\n\n\
         [Metadata]\nTitle: {title}\nArtist: wolluf\nCreator: wolluf\nVersion: Test\n\n\
         [Difficulty]\nHPDrainRate: 8\nCircleSize: {keys}\nOverallDifficulty: 8\n\n\
         [TimingPoints]\n{timing}\n\n[HitObjects]\n"
    );
    for (col, t) in taps {
        text.push_str(&format!("{},192,{t},1,0,0:0:0:0:\n", x(*col)));
    }
    for (col, head, tail) in holds {
        text.push_str(&format!("{},192,{head},128,0,{tail}:0:0:0:0:\n", x(*col)));
    }
    text.into_bytes()
}

/// A 7K chart with one tap per column 0..=n, 250 ms apart; the title keeps the bytes unique.
pub(crate) fn taps_7k(title: &str, n: u8) -> Vec<u8> {
    let taps: Vec<(u8, i32)> = (0..n)
        .map(|i| (i % 7, 1_000 + i32::from(i) * 250))
        .collect();
    osu_text(7, title, &taps, &[])
}

/// Six presses in column 0, 100 ms apart from 1 s: one `regular.jack.longjack` segment.
pub(crate) fn jacks_7k(title: &str) -> Vec<u8> {
    let taps: Vec<(u8, i32)> = (0..6).map(|i| (0, 1_000 + i * 100)).collect();
    osu_text(7, title, &taps, &[])
}

/// 64 single taps 120 ms apart over the four columns: rice MinaCalc rates on every grid rate.
pub(crate) fn rice_4k(title: &str) -> Vec<u8> {
    let taps: Vec<(u8, i32)> = (0..64)
        .map(|i| ([0, 2, 1, 3][i % 4], 1_000 + i32::try_from(i).unwrap() * 120))
        .collect();
    osu_text(4, title, &taps, &[])
}

/// Mostly holds: the LN share is far above the difficulty stage's LN cut-off.
pub(crate) fn ln_heavy_4k(title: &str) -> Vec<u8> {
    let holds: Vec<(u8, i32, i32)> = (0..16)
        .map(|i| {
            let head = 1_000 + i * 300;
            (u8::try_from(i % 4).unwrap(), head, head + 250)
        })
        .collect();
    osu_text(4, title, &[(0, 6_000), (1, 6_100)], &holds)
}

/// One osu!.db row and what sits in `Songs/` for it.
#[derive(Debug, Clone)]
pub(crate) struct Map {
    pub(crate) md5: String,
    pub(crate) keys: u8,
    pub(crate) folder: String,
    pub(crate) file: String,
    pub(crate) title: String,
    pub(crate) version: String,
    pub(crate) creator: String,
    pub(crate) set_id: i32,
    /// stable's no-mod osu!mania star rating in osu!.db.
    pub(crate) stars: Option<f64>,
    pub(crate) source: String,
    /// Space-separated, as osu!.db stores them.
    pub(crate) tags: String,
    /// `None`: missing from `Songs/`.
    pub(crate) bytes: Option<Vec<u8>>,
}

impl Map {
    pub(crate) fn new(title: &str, keys: u8, bytes: Vec<u8>) -> Self {
        Self {
            md5: md5_hex(&bytes),
            keys,
            folder: format!("100 wolluf - {title}"),
            file: format!("wolluf - {title} (wolluf) [Normal].osu"),
            title: title.to_owned(),
            version: "Normal".to_owned(),
            creator: "wolluf".to_owned(),
            set_id: 100,
            stars: None,
            source: String::new(),
            tags: String::new(),
            bytes: Some(bytes),
        }
    }

    pub(crate) fn k7(title: &str) -> Self {
        Self::new(title, 7, taps_7k(title, 8))
    }

    pub(crate) fn jacks(title: &str) -> Self {
        Self::new(title, 7, jacks_7k(title))
    }

    pub(crate) fn rice4(title: &str) -> Self {
        Self::new(title, 4, rice_4k(title))
    }

    pub(crate) fn ln4(title: &str) -> Self {
        Self::new(title, 4, ln_heavy_4k(title))
    }

    pub(crate) fn named(mut self, folder: &str, version: &str) -> Self {
        self.folder = folder.to_owned();
        self.version = version.to_owned();
        self
    }

    /// Every `Map` starts in set 100, which one-pick-per-set consumers would fold together.
    pub(crate) fn in_set(mut self, set_id: i32) -> Self {
        self.set_id = set_id;
        self
    }

    pub(crate) fn rated(mut self, stars: f64) -> Self {
        self.stars = Some(stars);
        self
    }

    pub(crate) fn described(mut self, source: &str, tags: &str) -> Self {
        self.source = source.to_owned();
        self.tags = tags.to_owned();
        self
    }

    pub(crate) fn missing(mut self) -> Self {
        self.bytes = None;
        self
    }

    /// The file was edited after osu!.db recorded its md5.
    pub(crate) fn edited(mut self) -> Self {
        self.bytes = Some(taps_7k(&format!("{} (edited)", self.title), 9));
        self
    }

    /// osu!.db stores a nested Songs folder with Windows separators (`Normal\<set>`).
    pub(crate) fn nested_in(mut self, parent: &str) -> Self {
        self.folder = format!("{parent}\\{}", self.folder);
        self
    }

    /// Where the file sits under `Songs/` on this host.
    pub(crate) fn rel_path(&self) -> String {
        format!("{}/{}", self.folder.replace('\\', "/"), self.file)
    }

    pub(crate) fn beatmap(&self) -> OsuDbBeatmap {
        let builder = BeatmapBuilder::mania(&self.md5, self.keys)
            .folder(&self.folder)
            .osu_file(&self.file)
            .title(&self.title)
            .ids(1, self.set_id);
        let mut b = match self.stars {
            Some(stars) => builder.star_rating(3, 0, stars),
            None => builder,
        }
        .build();
        b.difficulty = OsuString::present(self.version.as_bytes());
        b.creator = OsuString::present(self.creator.as_bytes());
        b.source = OsuString::present(self.source.as_bytes());
        b.tags = OsuString::present(self.tags.as_bytes());
        b
    }
}

/// `played` charts get one score each.
pub(crate) fn install(maps: &[Map], played: &[&Map]) -> FakeInstall {
    let osu_db = maps
        .iter()
        .fold(OsuDbBuilder::new(), |db, m| db.beatmap(m.beatmap()))
        .encode();
    let scores: Vec<ScoreBuilder> = played
        .iter()
        .zip(1..)
        .map(|(m, i)| ScoreBuilder::mania(&m.md5, "TWulfZ", i))
        .collect();
    maps.iter()
        .filter_map(|m| Some((m.rel_path(), m.bytes.clone()?)))
        .fold(
            FakeInstall::new()
                .osu_db(osu_db)
                .scores_db(scores_db(&scores)),
            |install, (rel, bytes)| install.song(rel, bytes),
        )
}

/// Syncs (which chains `IndexLibrary`) and returns the chained index job.
pub(crate) async fn synced(maps: &[Map], played: &[&Map]) -> (Fixture, IndexLibrarySummaryDto) {
    let f = Fixture::new(&install(maps, played)).await;
    f.sync().await;
    let job = last_index_job(&f).await;
    let summary = summary(&job);
    (f, summary)
}

pub(crate) async fn last_index_job(f: &Fixture) -> JobDto {
    f.ctx
        .jobs()
        .list(None)
        .await
        .unwrap()
        .into_iter()
        .find(|j| j.kind == JobKindDto::IndexLibrary)
        .expect("an IndexLibrary job ran")
}

pub(crate) fn summary(job: &JobDto) -> IndexLibrarySummaryDto {
    assert_eq!(job.status, JobStatusDto::Ok, "{job:?}");
    match &job.summary {
        Some(JobSummaryDto::IndexLibrary(s)) => s.clone(),
        other => panic!("unexpected summary {other:?}"),
    }
}

/// Runs one standalone index and waits for it.
pub(crate) async fn reindex(f: &Fixture) -> (JobDto, IndexLibrarySummaryDto) {
    let mut rx = f.ctx.subscribe();
    let id = f.ctx.library().index().await.unwrap();
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
    .expect("index finished in time");
    let job = last_index_job(f).await;
    assert_eq!(job.id, id);
    let s = summary(&job);
    (job, s)
}
