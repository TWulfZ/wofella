use wolluf_core::DotNetTicks;

use crate::codec::score_header::{
    JudgementCounts, OnlineId, ScoreHeader, write_online_id, write_score_header,
};
use crate::codec::scores_db::{ScoreRecord, ScoresDb, ScoresDbBeatmap};
use crate::codec::version::SCORES_DB_NEWEST_VERIFIED;
use crate::codec::{OsuString, Writer};

const MANIA: u8 = 3;
const SCORE_MARKER: i32 = -1;
/// 2026-04-25 00:00 UTC in .NET ticks, the pilot's first saved fail; `nth` offsets from here
/// keep builder timestamps plausible and distinct.
const BASE_TICKS: i64 = 639_126_720_000_000_000;
const TICKS_PER_SECOND: i64 = 10_000_000;

pub fn encode_scores_db(db: &ScoresDb) -> Vec<u8> {
    let mut w = Writer::new();
    w.i32(db.version);
    w.count(db.beatmaps.len());
    for b in &db.beatmaps {
        w.osu_string(&b.md5);
        w.count(b.scores.len());
        for s in &b.scores {
            write_score_record(&mut w, s);
        }
    }
    w.into_bytes()
}

pub(crate) fn write_score_record(w: &mut Writer, s: &ScoreRecord) {
    write_score_header(w, &s.header);
    w.i32(SCORE_MARKER);
    write_online_id(w, s.online_id);
    if let Some(tp) = s.target_practice {
        w.f64(tp);
    }
}

/// One score (or `.osr` header) with plausible mania defaults.
#[derive(Debug, Clone)]
pub struct ScoreBuilder {
    header: ScoreHeader,
    online_id: i64,
    target_practice: Option<f64>,
}

impl ScoreBuilder {
    /// `nth` spaces timestamps one second apart so every built play has its own natural key.
    pub fn mania(md5_hex: &str, player: &str, nth: i64) -> Self {
        Self {
            header: ScoreHeader {
                mode: MANIA,
                version: SCORES_DB_NEWEST_VERIFIED,
                beatmap_md5: OsuString::present(md5_hex.as_bytes()),
                player: OsuString::present(player.as_bytes()),
                replay_md5: OsuString::present(format!("{:032x}", nth).into_bytes()),
                counts: JudgementCounts {
                    geki: 100,
                    n300: 50,
                    ..JudgementCounts::default()
                },
                score: 900_000,
                max_combo: 150,
                perfect: 1,
                mods: 0,
                life_bar: OsuString::present(*b""),
                timestamp_ticks: BASE_TICKS + nth * TICKS_PER_SECOND,
            },
            online_id: 0,
            target_practice: None,
        }
    }

    pub const fn mode(mut self, mode: u8) -> Self {
        self.header.mode = mode;
        self
    }

    pub const fn version(mut self, version: i32) -> Self {
        self.header.version = version;
        self
    }

    pub fn player(mut self, player: OsuString) -> Self {
        self.header.player = player;
        self
    }

    pub fn beatmap_md5(mut self, md5: OsuString) -> Self {
        self.header.beatmap_md5 = md5;
        self
    }

    pub const fn counts(mut self, counts: JudgementCounts) -> Self {
        self.header.counts = counts;
        self
    }

    pub const fn score(mut self, score: i32) -> Self {
        self.header.score = score;
        self
    }

    pub const fn max_combo(mut self, max_combo: u16) -> Self {
        self.header.max_combo = max_combo;
        self
    }

    pub const fn mods(mut self, mods: u32) -> Self {
        self.header.mods = mods;
        self
    }

    pub fn life_bar(mut self, life_bar: OsuString) -> Self {
        self.header.life_bar = life_bar;
        self
    }

    pub const fn ticks(mut self, ticks: DotNetTicks) -> Self {
        self.header.timestamp_ticks = ticks.0;
        self
    }

    pub const fn online_id(mut self, id: i64) -> Self {
        self.online_id = id;
        self
    }

    pub const fn target_practice(mut self, value: f64) -> Self {
        self.target_practice = Some(value);
        self
    }

    pub fn header(&self) -> &ScoreHeader {
        &self.header
    }

    pub fn build_header(self) -> ScoreHeader {
        self.header
    }

    pub fn build(self) -> ScoreRecord {
        let online_id = OnlineId::for_version(self.header.version, self.online_id);
        let target_practice = self
            .header
            .has_target_practice()
            .then(|| self.target_practice.unwrap_or_default());
        ScoreRecord {
            header: self.header,
            online_id,
            target_practice,
        }
    }
}

/// Groups scores by beatmap md5 in first-seen order, as osu! does.
#[derive(Debug, Clone)]
pub struct ScoresDbBuilder {
    db: ScoresDb,
}

impl Default for ScoresDbBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl ScoresDbBuilder {
    pub const fn new() -> Self {
        Self {
            db: ScoresDb {
                version: SCORES_DB_NEWEST_VERIFIED,
                beatmaps: Vec::new(),
            },
        }
    }

    pub const fn version(mut self, version: i32) -> Self {
        self.db.version = version;
        self
    }

    pub fn score(mut self, record: ScoreRecord) -> Self {
        let md5 = record.header.beatmap_md5.clone();
        match self.db.beatmaps.iter_mut().find(|b| b.md5 == md5) {
            Some(b) => b.scores.push(record),
            None => self.db.beatmaps.push(ScoresDbBeatmap {
                md5,
                scores: vec![record],
            }),
        }
        self
    }

    pub fn build(self) -> ScoresDb {
        self.db
    }

    pub fn encode(self) -> Vec<u8> {
        encode_scores_db(&self.db)
    }
}
