use crate::codec::osu_db::{
    OsuDb, OsuDbBeatmap, PAIR_DOUBLE_TAG, PAIR_FLOAT_TAG, PAIR_INT_TAG, StarRating, TimingPoint,
};
use crate::codec::version::{
    OSU_DB_ENTRY_SIZE_REMOVED, OSU_DB_INT_FLOAT_PAIRS, OSU_DB_NEWEST_VERIFIED,
};
use crate::codec::{OsuString, Writer};

const MANIA: u8 = 3;
/// "unsubmitted": the most common status for local maps.
const DEFAULT_RANKED_STATUS: u8 = 1;
const DEFAULT_OD: f32 = 8.0;
const DEFAULT_HP: f32 = 8.0;
const DEFAULT_TOTAL_TIME_MS: i32 = 120_000;

pub fn encode_osu_db(db: &OsuDb) -> Vec<u8> {
    encode(db)
}

#[derive(Debug, Clone)]
pub struct OsuDbBuilder {
    db: OsuDb,
}

impl Default for OsuDbBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl OsuDbBuilder {
    pub const fn new() -> Self {
        Self {
            db: OsuDb {
                version: OSU_DB_NEWEST_VERIFIED,
                folder_count: 0,
                account_unlocked: 1,
                unlock_ticks: 0,
                player_name: OsuString::Absent,
                beatmaps: Vec::new(),
                permissions: 0,
            },
        }
    }

    pub const fn version(mut self, version: i32) -> Self {
        self.db.version = version;
        self
    }

    pub fn player_name(mut self, name: OsuString) -> Self {
        self.db.player_name = name;
        self
    }

    pub fn beatmap(mut self, beatmap: OsuDbBeatmap) -> Self {
        self.db.beatmaps.push(beatmap);
        self.db.folder_count = i32::try_from(self.db.beatmaps.len()).unwrap_or(i32::MAX);
        self
    }

    pub fn build(self) -> OsuDb {
        self.db
    }

    pub fn encode(self) -> Vec<u8> {
        encode(&self.db)
    }
}

/// A mania entry whose `.osu` lives at `<md5>/<md5>.osu` unless overridden.
#[derive(Debug, Clone)]
pub struct BeatmapBuilder {
    b: OsuDbBeatmap,
}

impl BeatmapBuilder {
    pub fn mania(md5_hex: &str, keys: u8) -> Self {
        let text = |field: &str| OsuString::present(format!("{field}-{md5_hex}").into_bytes());
        Self {
            b: OsuDbBeatmap {
                artist: text("artist"),
                title: text("title"),
                creator: text("creator"),
                difficulty: text("difficulty"),
                audio_file: OsuString::present(*b"audio.mp3"),
                md5: OsuString::present(md5_hex.as_bytes()),
                osu_file: OsuString::present(format!("{md5_hex}.osu").into_bytes()),
                ranked_status: DEFAULT_RANKED_STATUS,
                circle_size: f32::from(keys),
                overall_difficulty: DEFAULT_OD,
                hp_drain: DEFAULT_HP,
                total_time_ms: DEFAULT_TOTAL_TIME_MS,
                mode: MANIA,
                folder: OsuString::present(md5_hex.as_bytes()),
                ..OsuDbBeatmap::default()
            },
        }
    }

    pub fn md5(mut self, md5: OsuString) -> Self {
        self.b.md5 = md5;
        self
    }

    pub const fn mode(mut self, mode: u8) -> Self {
        self.b.mode = mode;
        self
    }

    pub const fn circle_size(mut self, cs: f32) -> Self {
        self.b.circle_size = cs;
        self
    }

    pub const fn overall_difficulty(mut self, od: f32) -> Self {
        self.b.overall_difficulty = od;
        self
    }

    pub const fn ranked_status(mut self, status: u8) -> Self {
        self.b.ranked_status = status;
        self
    }

    pub const fn ids(mut self, beatmap_id: i32, beatmapset_id: i32) -> Self {
        self.b.beatmap_id = beatmap_id;
        self.b.beatmapset_id = beatmapset_id;
        self
    }

    pub const fn object_counts(mut self, circles: u16, sliders: u16, spinners: u16) -> Self {
        self.b.n_circles = circles;
        self.b.n_sliders = sliders;
        self.b.n_spinners = spinners;
        self
    }

    pub fn folder(mut self, folder: &str) -> Self {
        self.b.folder = OsuString::present(folder.as_bytes());
        self
    }

    pub fn osu_file(mut self, file: &str) -> Self {
        self.b.osu_file = OsuString::present(file.as_bytes());
        self
    }

    pub fn title(mut self, title: &str) -> Self {
        self.b.title = OsuString::present(title.as_bytes());
        self
    }

    /// `mode_index` 0–3 = osu!, taiko, catch, mania.
    pub fn star_rating(mut self, mode_index: usize, mods: i32, stars: f64) -> Self {
        if let Some(list) = self.b.star_ratings.get_mut(mode_index) {
            list.push(StarRating { mods, stars });
        }
        self
    }

    pub fn timing_point(mut self, bpm: f64, offset: f64, uninherited: u8) -> Self {
        self.b.timing_points.push(TimingPoint {
            bpm,
            offset,
            uninherited,
        });
        self
    }

    pub fn build(self) -> OsuDbBeatmap {
        self.b
    }
}

pub(crate) fn write_beatmap(w: &mut Writer, b: &OsuDbBeatmap, version: i32) {
    for s in [
        &b.artist,
        &b.artist_unicode,
        &b.title,
        &b.title_unicode,
        &b.creator,
        &b.difficulty,
        &b.audio_file,
        &b.md5,
        &b.osu_file,
    ] {
        w.osu_string(s);
    }
    w.u8(b.ranked_status);
    w.u16(b.n_circles);
    w.u16(b.n_sliders);
    w.u16(b.n_spinners);
    w.i64(b.last_modified_ticks);
    w.f32(b.approach_rate);
    w.f32(b.circle_size);
    w.f32(b.hp_drain);
    w.f32(b.overall_difficulty);
    w.f64(b.slider_velocity);
    let float_pairs = version >= OSU_DB_INT_FLOAT_PAIRS;
    for list in &b.star_ratings {
        w.count(list.len());
        for sr in list {
            w.u8(PAIR_INT_TAG);
            w.i32(sr.mods);
            if float_pairs {
                w.u8(PAIR_FLOAT_TAG);
                w.f32(sr.stars as f32);
            } else {
                w.u8(PAIR_DOUBLE_TAG);
                w.f64(sr.stars);
            }
        }
    }
    w.i32(b.drain_time_s);
    w.i32(b.total_time_ms);
    w.i32(b.preview_time_ms);
    w.count(b.timing_points.len());
    for tp in &b.timing_points {
        w.f64(tp.bpm);
        w.f64(tp.offset);
        w.u8(tp.uninherited);
    }
    w.i32(b.beatmap_id);
    w.i32(b.beatmapset_id);
    w.i32(b.thread_id);
    for g in b.grades {
        w.u8(g);
    }
    w.i16(b.local_offset);
    w.f32(b.stack_leniency);
    w.u8(b.mode);
    w.osu_string(&b.source);
    w.osu_string(&b.tags);
    w.i16(b.online_offset);
    w.osu_string(&b.title_font);
    w.u8(b.unplayed);
    w.i64(b.last_played_ticks);
    w.u8(b.is_osz2);
    w.osu_string(&b.folder);
    w.i64(b.last_checked_ticks);
    for flag in [
        b.ignore_sound,
        b.ignore_skin,
        b.disable_storyboard,
        b.disable_video,
        b.visual_override,
    ] {
        w.u8(flag);
    }
    w.i32(b.last_modified_raw);
    w.u8(b.mania_scroll_speed);
}

fn encode(db: &OsuDb) -> Vec<u8> {
    let mut w = Writer::new();
    w.i32(db.version);
    w.i32(db.folder_count);
    w.u8(db.account_unlocked);
    w.i64(db.unlock_ticks);
    w.osu_string(&db.player_name);
    w.count(db.beatmaps.len());
    for b in &db.beatmaps {
        if db.version >= OSU_DB_ENTRY_SIZE_REMOVED {
            write_beatmap(&mut w, b, db.version);
        } else {
            let mut entry = Writer::new();
            write_beatmap(&mut entry, b, db.version);
            w.count(entry.len());
            w.bytes(entry.as_bytes());
        }
    }
    w.i32(db.permissions);
    w.into_bytes()
}
