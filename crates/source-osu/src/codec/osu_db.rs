//! osu!.db (`research/scripts/rejudge/legacy_db.md`, oracle `rejudge/osudb.py`). Decoded fully
//! in memory; there is no per-entry length since 20191106, so a bad entry fails the whole file.

use wolluf_core::{ChartMd5, Keymode};

use crate::codec::version::{OSU_DB_ENTRY_SIZE_REMOVED, OSU_DB_INT_FLOAT_PAIRS, admit, finish};
use crate::codec::{FileKind, OsuString, Reader};
use crate::diag::{DiagCode, Diagnostics};
use crate::error::CodecError;

const KIND: FileKind = FileKind::OsuDb;
pub(crate) const PAIR_INT_TAG: u8 = 0x08;
pub(crate) const PAIR_FLOAT_TAG: u8 = 0x0c;
pub(crate) const PAIR_DOUBLE_TAG: u8 = 0x0d;
const MAX_RANKED_STATUS: u8 = 7;
const MANIA_MODE: u8 = 3;
const MAX_MODE: u8 = MANIA_MODE;
const STAR_RATING_MODES: usize = 4;
const TIMING_POINT_SIZE: usize = 8 + 8 + 1;
const FLOAT_PAIR_SIZE: usize = 1 + 4 + 1 + 4;
const DOUBLE_PAIR_SIZE: usize = 1 + 4 + 1 + 8;
/// Smallest entry at or above 20191106: 13 absent strings, the fixed-width fields, and the
/// five list counts (4 star lists + timing points) with every list empty.
pub(crate) const MIN_BEATMAP_SIZE: usize = 13 // strings
    + 1 + 3 * 2 + 8 // ranked status, object counts, modified ticks
    + 4 * 4 + 8 // AR CS HP OD, slider velocity
    + 4 * 4 // star-rating list counts
    + 3 * 4 + 4 // drain, total, preview, timing-point count
    + 3 * 4 + 4 + 2 + 4 + 1 // ids, grades, local offset, stack leniency, mode
    + 2 + 1 + 8 + 1 // online offset, unplayed, last played, osz2
    + 8 + 5 + 4 + 1; // last checked, flags, modified Int, scroll speed

#[derive(Debug, Clone, PartialEq)]
pub struct OsuDb {
    pub version: i32,
    pub folder_count: i32,
    /// Raw boolean byte.
    pub account_unlocked: u8,
    pub unlock_ticks: i64,
    pub player_name: OsuString,
    pub beatmaps: Vec<OsuDbBeatmap>,
    pub permissions: i32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StarRating {
    pub mods: i32,
    /// Int-Float values from 20250107 are widened losslessly.
    pub stars: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TimingPoint {
    pub bpm: f64,
    pub offset: f64,
    /// Raw boolean byte; 0 means inherited.
    pub uninherited: u8,
}

/// Every field of the `legacy_db.md` table, in file order. Booleans are raw bytes and times are
/// raw ticks so an encode reproduces the input exactly.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct OsuDbBeatmap {
    pub artist: OsuString,
    pub artist_unicode: OsuString,
    pub title: OsuString,
    pub title_unicode: OsuString,
    pub creator: OsuString,
    pub difficulty: OsuString,
    pub audio_file: OsuString,
    pub md5: OsuString,
    pub osu_file: OsuString,
    pub ranked_status: u8,
    pub n_circles: u16,
    pub n_sliders: u16,
    pub n_spinners: u16,
    pub last_modified_ticks: i64,
    pub approach_rate: f32,
    pub circle_size: f32,
    pub hp_drain: f32,
    pub overall_difficulty: f32,
    pub slider_velocity: f64,
    /// osu!, taiko, catch, mania.
    pub star_ratings: [Vec<StarRating>; STAR_RATING_MODES],
    pub drain_time_s: i32,
    pub total_time_ms: i32,
    pub preview_time_ms: i32,
    pub timing_points: Vec<TimingPoint>,
    /// The wiki calls this "Difficulty ID": the per-difficulty `/b/` id.
    pub beatmap_id: i32,
    /// The wiki calls this "Beatmap ID": the `/s/` set id, the oracle's `bid`.
    pub beatmapset_id: i32,
    pub thread_id: i32,
    /// osu!, taiko, catch, mania.
    pub grades: [u8; STAR_RATING_MODES],
    pub local_offset: i16,
    pub stack_leniency: f32,
    pub mode: u8,
    pub source: OsuString,
    pub tags: OsuString,
    pub online_offset: i16,
    pub title_font: OsuString,
    pub unplayed: u8,
    pub last_played_ticks: i64,
    pub is_osz2: u8,
    pub folder: OsuString,
    pub last_checked_ticks: i64,
    pub ignore_sound: u8,
    pub ignore_skin: u8,
    pub disable_storyboard: u8,
    pub disable_video: u8,
    pub visual_override: u8,
    /// The wiki's "Last modification time (?)" Int.
    pub last_modified_raw: i32,
    pub mania_scroll_speed: u8,
}

impl OsuDbBeatmap {
    pub fn beatmap_md5(&self) -> Option<ChartMd5> {
        parse_md5(&self.md5)
    }

    /// osu!mania stores the key count in CS (`round(CS)`, spec 002).
    pub fn mania_keymode(&self) -> Option<Keymode> {
        if self.mode != MANIA_MODE {
            return None;
        }
        let keys = self.circle_size.round();
        if !(1.0..=f32::from(u8::MAX)).contains(&keys) {
            return None;
        }
        Keymode::new(keys as u8).ok()
    }
}

fn parse_md5(md5: &OsuString) -> Option<ChartMd5> {
    std::str::from_utf8(md5.as_bytes()?).ok()?.parse().ok()
}

pub fn decode_osu_db(bytes: &[u8]) -> Result<(OsuDb, Diagnostics), CodecError> {
    let mut r = Reader::new(bytes, KIND);
    let version = r.i32()?;
    let class = admit(KIND, version)?;
    let body = decode_body(&mut r, version);
    finish(KIND, version, class, body)
}

fn decode_body(r: &mut Reader<'_>, version: i32) -> Result<(OsuDb, Diagnostics), CodecError> {
    let mut diags = Diagnostics::new();
    let folder_count = r.i32()?;
    let account_unlocked = r.u8()?;
    let unlock_ticks = r.i64()?;
    let player_name = r.osu_string()?;
    let n = r.count(MIN_BEATMAP_SIZE)?;
    let mut beatmaps = Vec::with_capacity(n);
    for _ in 0..n {
        beatmaps.push(read_entry(r, version, &mut diags)?);
    }
    let permissions = r.i32()?;
    r.expect_eof()?;
    let db = OsuDb {
        version,
        folder_count,
        account_unlocked,
        unlock_ticks,
        player_name,
        beatmaps,
        permissions,
    };
    Ok((db, diags))
}

fn read_entry(
    r: &mut Reader<'_>,
    version: i32,
    diags: &mut Diagnostics,
) -> Result<OsuDbBeatmap, CodecError> {
    if version >= OSU_DB_ENTRY_SIZE_REMOVED {
        return read_beatmap(r, version, diags);
    }
    // Assumed to exclude the Int itself; untested on real data because no corpus file is older
    // than 20191106, but a wrong assumption fails loudly here rather than misaligning silently.
    let size_at = r.offset();
    let declared = r.i32()?;
    let start = r.position();
    let beatmap = read_beatmap(r, version, diags)?;
    let consumed = (r.position() - start) as u64;
    if u64::try_from(declared).ok() != Some(consumed) {
        return Err(CodecError::EntrySizeMismatch {
            offset: size_at,
            declared,
            consumed,
        });
    }
    Ok(beatmap)
}

fn read_beatmap(
    r: &mut Reader<'_>,
    version: i32,
    diags: &mut Diagnostics,
) -> Result<OsuDbBeatmap, CodecError> {
    let entry_at = r.offset();
    let artist = r.osu_string()?;
    let artist_unicode = r.osu_string()?;
    let title = r.osu_string()?;
    let title_unicode = r.osu_string()?;
    let creator = r.osu_string()?;
    let difficulty = r.osu_string()?;
    let audio_file = r.osu_string()?;
    let md5 = r.osu_string()?;
    if parse_md5(&md5).is_none() {
        diags.at_offset(
            DiagCode::OsuDbBadMd5,
            entry_at,
            "entry md5 is not 32 lowercase hex",
        );
    }
    let osu_file = r.osu_string()?;
    let ranked_at = r.offset();
    let ranked_status = r.u8()?;
    if ranked_status > MAX_RANKED_STATUS {
        diags.at_offset(
            DiagCode::OsuDbUnknownRankedStatus,
            ranked_at,
            format!("ranked status {ranked_status}"),
        );
    }
    let n_circles = r.u16()?;
    let n_sliders = r.u16()?;
    let n_spinners = r.u16()?;
    let last_modified_ticks = r.i64()?;
    let approach_rate = r.f32()?;
    let circle_size = r.f32()?;
    let hp_drain = r.f32()?;
    let overall_difficulty = r.f32()?;
    let slider_velocity = r.f64()?;
    let float_pairs = version >= OSU_DB_INT_FLOAT_PAIRS;
    let star_ratings = [
        read_star_ratings(r, float_pairs)?,
        read_star_ratings(r, float_pairs)?,
        read_star_ratings(r, float_pairs)?,
        read_star_ratings(r, float_pairs)?,
    ];
    let drain_time_s = r.i32()?;
    let total_time_ms = r.i32()?;
    let preview_time_ms = r.i32()?;
    let n_tp = r.count(TIMING_POINT_SIZE)?;
    let mut timing_points = Vec::with_capacity(n_tp);
    for _ in 0..n_tp {
        timing_points.push(TimingPoint {
            bpm: r.f64()?,
            offset: r.f64()?,
            uninherited: r.u8()?,
        });
    }
    let beatmap_id = r.i32()?;
    let beatmapset_id = r.i32()?;
    let thread_id = r.i32()?;
    let grades = [r.u8()?, r.u8()?, r.u8()?, r.u8()?];
    let local_offset = r.i16()?;
    let stack_leniency = r.f32()?;
    let mode_at = r.offset();
    let mode = r.u8()?;
    if mode > MAX_MODE {
        diags.at_offset(DiagCode::OsuDbUnknownMode, mode_at, format!("mode {mode}"));
    }
    Ok(OsuDbBeatmap {
        artist,
        artist_unicode,
        title,
        title_unicode,
        creator,
        difficulty,
        audio_file,
        md5,
        osu_file,
        ranked_status,
        n_circles,
        n_sliders,
        n_spinners,
        last_modified_ticks,
        approach_rate,
        circle_size,
        hp_drain,
        overall_difficulty,
        slider_velocity,
        star_ratings,
        drain_time_s,
        total_time_ms,
        preview_time_ms,
        timing_points,
        beatmap_id,
        beatmapset_id,
        thread_id,
        grades,
        local_offset,
        stack_leniency,
        mode,
        source: r.osu_string()?,
        tags: r.osu_string()?,
        online_offset: r.i16()?,
        title_font: r.osu_string()?,
        unplayed: r.u8()?,
        last_played_ticks: r.i64()?,
        is_osz2: r.u8()?,
        folder: r.osu_string()?,
        last_checked_ticks: r.i64()?,
        ignore_sound: r.u8()?,
        ignore_skin: r.u8()?,
        disable_storyboard: r.u8()?,
        disable_video: r.u8()?,
        visual_override: r.u8()?,
        last_modified_raw: r.i32()?,
        mania_scroll_speed: r.u8()?,
    })
}

fn expect_tag(r: &mut Reader<'_>, expected: u8) -> Result<(), CodecError> {
    let offset = r.offset();
    let found = r.u8()?;
    if found == expected {
        Ok(())
    } else {
        Err(CodecError::UnexpectedTag {
            kind: KIND,
            offset,
            expected,
            found,
        })
    }
}

/// The value tag is checked against the layout the version selects; a mismatch is an error,
/// never a fallback to the other width (§7 "the parser never guesses").
fn read_star_ratings(r: &mut Reader<'_>, float_pairs: bool) -> Result<Vec<StarRating>, CodecError> {
    let pair_size = if float_pairs {
        FLOAT_PAIR_SIZE
    } else {
        DOUBLE_PAIR_SIZE
    };
    let n = r.count(pair_size)?;
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        expect_tag(r, PAIR_INT_TAG)?;
        let mods = r.i32()?;
        let stars = if float_pairs {
            expect_tag(r, PAIR_FLOAT_TAG)?;
            f64::from(r.f32()?)
        } else {
            expect_tag(r, PAIR_DOUBLE_TAG)?;
            r.f64()?
        };
        out.push(StarRating { mods, stars });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codec::version::{OSU_DB_ENTRY_SIZE_REMOVED, OSU_DB_INT_FLOAT_PAIRS};
    use crate::diag::DiagCode;
    use crate::error::CodecError;
    use crate::testkit::{BeatmapBuilder, OsuDbBuilder, encode_osu_db};

    const MD5: &str = "0123456789abcdef0123456789abcdef";

    fn with_stars(version: i32) -> Vec<u8> {
        OsuDbBuilder::new()
            .version(version)
            .beatmap(
                BeatmapBuilder::mania(MD5, 7)
                    .star_rating(3, 64, 4.5)
                    .star_rating(0, 0, 2.25)
                    .build(),
            )
            .encode()
    }

    /// Byte offset of the first star-rating pair of the only entry.
    fn first_pair_offset(bytes: &[u8]) -> usize {
        // 0x08 immediately followed by the mods Int 0 and a Float/Double tag.
        bytes
            .windows(6)
            .position(|w| w[0] == 0x08 && w[1..5] == [0, 0, 0, 0] && (w[5] == 0x0c || w[5] == 0x0d))
            .unwrap()
    }

    #[test]
    fn decodes_int_float_pairs_from_20250107() {
        let bytes = with_stars(OSU_DB_INT_FLOAT_PAIRS);
        let at = first_pair_offset(&bytes);
        assert_eq!(bytes[at + 5], 0x0c);
        let (db, diags) = decode_osu_db(&bytes).unwrap();
        assert!(diags.is_empty(), "{diags:?}");
        let b = &db.beatmaps[0];
        assert_eq!(
            b.star_ratings[0],
            vec![StarRating {
                mods: 0,
                stars: 2.25
            }]
        );
        assert_eq!(
            b.star_ratings[3],
            vec![StarRating {
                mods: 64,
                stars: 4.5
            }]
        );
    }

    #[test]
    fn decodes_int_double_pairs_before_20250107() {
        let bytes = with_stars(OSU_DB_INT_FLOAT_PAIRS - 1);
        let at = first_pair_offset(&bytes);
        assert_eq!(bytes[at + 5], 0x0d);
        let (db, _) = decode_osu_db(&bytes).unwrap();
        assert_eq!(
            db.beatmaps[0].star_ratings[3],
            vec![StarRating {
                mods: 64,
                stars: 4.5
            }]
        );
        // A Double pair is 4 bytes longer than a Float pair.
        assert_eq!(
            bytes.len(),
            with_stars(OSU_DB_INT_FLOAT_PAIRS).len() + 2 * 4
        );
    }

    #[test]
    fn wrong_pair_tag_is_error() {
        let mut bytes = with_stars(OSU_DB_INT_FLOAT_PAIRS);
        let at = first_pair_offset(&bytes);
        bytes[at + 5] = 0x0d;
        assert_eq!(
            decode_osu_db(&bytes).map(|_| ()),
            Err(CodecError::UnexpectedTag {
                kind: FileKind::OsuDb,
                offset: (at + 5) as u64,
                expected: 0x0c,
                found: 0x0d,
            })
        );
        bytes[at + 5] = 0x0c;
        bytes[at] = 0x09;
        assert!(matches!(
            decode_osu_db(&bytes),
            Err(CodecError::UnexpectedTag {
                expected: 0x08,
                found: 0x09,
                ..
            })
        ));
    }

    #[test]
    fn entry_size_present_before_20191106() {
        let old = OsuDbBuilder::new()
            .version(OSU_DB_ENTRY_SIZE_REMOVED - 1)
            .beatmap(BeatmapBuilder::mania(MD5, 4).build())
            .encode();
        let new = OsuDbBuilder::new()
            .version(OSU_DB_ENTRY_SIZE_REMOVED)
            .beatmap(BeatmapBuilder::mania(MD5, 4).build())
            .encode();
        // Same Double pairs on both sides of the threshold, so the only difference is the Int.
        assert_eq!(old.len(), new.len() + 4);
        let (db, _) = decode_osu_db(&old).unwrap();
        assert_eq!(db.beatmaps[0].md5.as_bytes(), Some(MD5.as_bytes()));
    }

    #[test]
    fn entry_size_mismatch_is_error() {
        let mut bytes = OsuDbBuilder::new()
            .version(OSU_DB_ENTRY_SIZE_REMOVED - 1)
            .beatmap(BeatmapBuilder::mania(MD5, 4).build())
            .encode();
        let header_len = 4 + 4 + 1 + 8 + 1 + 4;
        let declared = i32::from_le_bytes(bytes[header_len..header_len + 4].try_into().unwrap());
        bytes[header_len..header_len + 4].copy_from_slice(&(declared + 1).to_le_bytes());
        assert_eq!(
            decode_osu_db(&bytes).map(|_| ()),
            Err(CodecError::EntrySizeMismatch {
                offset: header_len as u64,
                declared: declared + 1,
                consumed: declared as u64,
            })
        );
    }

    #[test]
    fn below_20140609_unsupported() {
        let bytes = OsuDbBuilder::new().version(20_140_608).encode();
        assert_eq!(
            decode_osu_db(&bytes).map(|_| ()),
            Err(CodecError::UnsupportedFormat {
                kind: FileKind::OsuDb,
                version: 20_140_608
            })
        );
    }

    #[test]
    fn trailing_bytes_rejected() {
        let mut bytes = with_stars(OSU_DB_INT_FLOAT_PAIRS);
        bytes.push(0);
        assert!(matches!(
            decode_osu_db(&bytes),
            Err(CodecError::TrailingBytes {
                kind: FileKind::OsuDb,
                remaining: 1,
                ..
            })
        ));
    }

    #[test]
    fn unknown_mode_is_diagnostic() {
        let bytes = OsuDbBuilder::new()
            .beatmap(
                BeatmapBuilder::mania(MD5, 7)
                    .mode(4)
                    .ranked_status(8)
                    .build(),
            )
            .beatmap(BeatmapBuilder::mania("not-an-md5", 7).build())
            .encode();
        let (db, diags) = decode_osu_db(&bytes).unwrap();
        assert_eq!(db.beatmaps.len(), 2);
        assert_eq!(
            diags.codes(),
            vec![
                DiagCode::OsuDbUnknownRankedStatus,
                DiagCode::OsuDbUnknownMode,
                DiagCode::OsuDbBadMd5
            ]
        );
        assert_eq!(db.beatmaps[0].mania_keymode(), None);
        assert_eq!(db.beatmaps[1].beatmap_md5(), None);
        assert!(
            diags
                .iter()
                .all(|d| d.offset.is_some() && !d.detail.contains("not-an-md5"))
        );
    }

    #[test]
    fn keymode_from_circle_size() {
        let k7 = BeatmapBuilder::mania(MD5, 7).build();
        assert_eq!(k7.mania_keymode(), Some(Keymode::K7));
        assert_eq!(k7.beatmap_md5(), Some(MD5.parse().unwrap()));
        let k4 = BeatmapBuilder::mania(MD5, 4).circle_size(4.4).build();
        assert_eq!(k4.mania_keymode(), Some(Keymode::K4));
        let std = BeatmapBuilder::mania(MD5, 7).mode(0).build();
        assert_eq!(std.mania_keymode(), None);
        for bad_cs in [0.0, 17.0, -1.0, f32::NAN] {
            assert_eq!(
                BeatmapBuilder::mania(MD5, 7)
                    .circle_size(bad_cs)
                    .build()
                    .mania_keymode(),
                None
            );
        }
        let upper = BeatmapBuilder::mania(&MD5.to_uppercase(), 7).build();
        assert_eq!(upper.beatmap_md5(), None);
        assert_eq!(
            BeatmapBuilder::mania(MD5, 7)
                .md5(OsuString::Absent)
                .build()
                .beatmap_md5(),
            None
        );
    }

    #[test]
    fn unverified_version_accepted_with_warning() {
        let bytes = OsuDbBuilder::new()
            .version(20_270_101)
            .beatmap(BeatmapBuilder::mania(MD5, 7).star_rating(3, 0, 1.0).build())
            .encode();
        let (db, diags) = decode_osu_db(&bytes).unwrap();
        assert_eq!(db.version, 20_270_101);
        assert_eq!(diags.codes(), vec![DiagCode::FormatUnverifiedVersion]);
    }

    #[test]
    fn unverified_version_with_trailing_bytes_is_unsupported() {
        let mut bytes = OsuDbBuilder::new()
            .version(20_270_101)
            .beatmap(BeatmapBuilder::mania(MD5, 7).build())
            .encode();
        bytes.extend([0, 0]);
        assert_eq!(
            decode_osu_db(&bytes).map(|_| ()),
            Err(CodecError::UnsupportedFormat {
                kind: FileKind::OsuDb,
                version: 20_270_101
            })
        );
    }

    #[test]
    fn min_beatmap_size_matches_empty_entry() {
        let mut w = crate::codec::Writer::new();
        crate::testkit::write_beatmap(&mut w, &OsuDbBeatmap::default(), OSU_DB_INT_FLOAT_PAIRS);
        assert_eq!(w.len(), MIN_BEATMAP_SIZE);
    }

    #[test]
    fn header_fields_roundtrip() {
        let db = OsuDbBuilder::new()
            .player_name(OsuString::present(*b"fixture"))
            .beatmap(
                BeatmapBuilder::mania(MD5, 7)
                    .timing_point(180.0, 0.0, 1)
                    .build(),
            )
            .build();
        let bytes = encode_osu_db(&db);
        let (decoded, _) = decode_osu_db(&bytes).unwrap();
        assert_eq!(decoded, db);
        assert_eq!(decoded.beatmaps[0].timing_points.len(), 1);
    }
}

#[cfg(test)]
mod props {
    use proptest::prelude::*;

    use super::*;
    use crate::codec::version::{OSU_DB_INT_FLOAT_PAIRS, OSU_DB_MIN, OSU_DB_NEWEST_VERIFIED};
    use crate::testkit::encode_osu_db;

    fn s() -> impl Strategy<Value = OsuString> {
        prop_oneof![
            Just(OsuString::Absent),
            proptest::collection::vec(any::<u8>(), 0..12).prop_map(OsuString::Present),
        ]
    }

    fn finite_f32() -> impl Strategy<Value = f32> {
        any::<f32>().prop_filter("finite", |f| f.is_finite())
    }

    fn finite_f64() -> impl Strategy<Value = f64> {
        any::<f64>().prop_filter("finite", |f| f.is_finite())
    }

    /// Stars are generated as f32 so they survive the Int-Float layout exactly.
    fn stars() -> impl Strategy<Value = Vec<StarRating>> {
        proptest::collection::vec(
            (any::<i32>(), finite_f32()).prop_map(|(mods, s)| StarRating {
                mods,
                stars: f64::from(s),
            }),
            0..3,
        )
    }

    fn beatmap() -> impl Strategy<Value = OsuDbBeatmap> {
        let strings = proptest::collection::vec(s(), 14);
        let counts = (
            any::<u8>(),
            any::<u16>(),
            any::<u16>(),
            any::<u16>(),
            any::<i64>(),
        );
        let diff = (
            finite_f32(),
            finite_f32(),
            finite_f32(),
            finite_f32(),
            finite_f64(),
        );
        let star_sets = (stars(), stars(), stars(), stars());
        let times = (any::<i32>(), any::<i32>(), any::<i32>());
        let tps = proptest::collection::vec(
            (finite_f64(), finite_f64(), any::<u8>()).prop_map(|(bpm, offset, uninherited)| {
                TimingPoint {
                    bpm,
                    offset,
                    uninherited,
                }
            }),
            0..3,
        );
        let ids = (
            any::<i32>(),
            any::<i32>(),
            any::<i32>(),
            any::<[u8; 4]>(),
            any::<i16>(),
            finite_f32(),
            any::<u8>(),
        );
        let tail = (
            any::<i16>(),
            any::<u8>(),
            any::<i64>(),
            any::<u8>(),
            any::<i64>(),
            any::<[u8; 5]>(),
            any::<i32>(),
            any::<u8>(),
        );
        (strings, counts, diff, star_sets, times, tps, ids, tail).prop_map(
            |(
                st,
                (rs, c, sl, sp, lm),
                (ar, cs, hp, od, sv),
                (s0, s1, s2, s3),
                (dt, tt, pt),
                timing_points,
                (bid, sid, tid, grades, lo, sl2, mode),
                (oo, up, lp, osz2, lc, flags, lmr, mss),
            ): (Vec<OsuString>, _, _, _, _, _, _, _)| OsuDbBeatmap {
                artist: st[0].clone(),
                artist_unicode: st[1].clone(),
                title: st[2].clone(),
                title_unicode: st[3].clone(),
                creator: st[4].clone(),
                difficulty: st[5].clone(),
                audio_file: st[6].clone(),
                md5: st[7].clone(),
                osu_file: st[8].clone(),
                ranked_status: rs,
                n_circles: c,
                n_sliders: sl,
                n_spinners: sp,
                last_modified_ticks: lm,
                approach_rate: ar,
                circle_size: cs,
                hp_drain: hp,
                overall_difficulty: od,
                slider_velocity: sv,
                star_ratings: [s0, s1, s2, s3],
                drain_time_s: dt,
                total_time_ms: tt,
                preview_time_ms: pt,
                timing_points,
                beatmap_id: bid,
                beatmapset_id: sid,
                thread_id: tid,
                grades,
                local_offset: lo,
                stack_leniency: sl2,
                mode,
                source: st[9].clone(),
                tags: st[10].clone(),
                online_offset: oo,
                title_font: st[11].clone(),
                unplayed: up,
                last_played_ticks: lp,
                is_osz2: osz2,
                folder: st[12].clone(),
                last_checked_ticks: lc,
                ignore_sound: flags[0],
                ignore_skin: flags[1],
                disable_storyboard: flags[2],
                disable_video: flags[3],
                visual_override: flags[4],
                last_modified_raw: lmr,
                mania_scroll_speed: mss,
            },
        )
    }

    fn osu_db() -> impl Strategy<Value = OsuDb> {
        (
            prop_oneof![
                Just(OSU_DB_MIN),
                OSU_DB_MIN..OSU_DB_INT_FLOAT_PAIRS,
                OSU_DB_INT_FLOAT_PAIRS..=OSU_DB_NEWEST_VERIFIED
            ],
            any::<i32>(),
            any::<u8>(),
            any::<i64>(),
            s(),
            proptest::collection::vec(beatmap(), 0..3),
            any::<i32>(),
        )
            .prop_map(
                |(
                    version,
                    folder_count,
                    account_unlocked,
                    unlock_ticks,
                    player_name,
                    beatmaps,
                    permissions,
                )| OsuDb {
                    version,
                    folder_count,
                    account_unlocked,
                    unlock_ticks,
                    player_name,
                    beatmaps,
                    permissions,
                },
            )
    }

    proptest! {
        #[test]
        fn encode_decode_roundtrip(db in osu_db()) {
            let bytes = encode_osu_db(&db);
            let (decoded, _) = decode_osu_db(&bytes).unwrap();
            prop_assert_eq!(&decoded, &db);
            prop_assert_eq!(encode_osu_db(&decoded), bytes);
        }

        #[test]
        fn arbitrary_bytes_never_panic(bytes in proptest::collection::vec(any::<u8>(), 0..512)) {
            let _ = decode_osu_db(&bytes);
        }
    }
}
