//! osu! stable `.osu` decoder, osu!mania only. rosu-map supplies the line layer (BOM, UTF-16,
//! lossy UTF-8, section dispatch, comment lines); the fields are read here because rosu-map drops
//! failing lines silently, maps unknown modes to osu!standard, and clamps and merges timing lines
//! the lazer way, while this decoder must report oddities and keep red and green lines raw.

use std::convert::Infallible;

use rosu_map::{DecodeBeatmap, DecodeState};
use wolluf_core::{Keymode, TimeUs};

use super::{ChartDecoder, Decoded};
use crate::diag::{DiagCode, Diagnostics};
use crate::error::ChartError;
use crate::model::{Chart, ChartMeta, Note, NoteKind, TimingKind, TimingPoint};

const MANIA_MODE: i32 = 3;
const PLAYFIELD_WIDTH: i64 = 512;
/// lazer's `Parsing.MAX_COORDINATE_VALUE`; larger coordinates are malformed.
const MAX_COORDINATE: f64 = 131_072.0;
/// osu! stores times as 32-bit milliseconds.
const MAX_ABS_MS: f64 = i32::MAX as f64;
/// osu!'s default CircleSize when the key is absent.
const DEFAULT_CIRCLE_SIZE: u8 = 5;
const MIN_KEYS: f64 = 1.0;
const MAX_KEYS: f64 = 16.0;
const DEFAULT_METER: u32 = 4;
const HOLD_BIT: i32 = 128;
const CIRCLE_BIT: i32 = 1;
const VERSION_PREFIX: &[u8] = b"osu file format v";

#[derive(Debug, Clone, Copy, Default)]
pub struct OsuDecoder;

impl ChartDecoder for OsuDecoder {
    fn format_id(&self) -> &'static str {
        "osu"
    }

    fn decode(&self, bytes: &[u8]) -> Result<Decoded, ChartError> {
        let raw: RawOsu = rosu_map::from_bytes(bytes)?;
        let mut diags = raw.diags;

        let mode = match raw.mode {
            None => 0,
            Some(text) => text
                .parse::<i32>()
                .map_err(|_| ChartError::InvalidMode(text))?,
        };
        if mode != MANIA_MODE {
            return Err(ChartError::UnsupportedMode(mode));
        }

        let format_version = match has_version_line(bytes) {
            Some(false) => {
                diags.push(DiagCode::MissingFormatVersion, "");
                None
            }
            _ => Some(raw.version),
        };

        let keymode = keymode_from(raw.circle_size, &mut diags);
        let keys = i64::from(keymode.columns());
        let notes = raw
            .objects
            .into_iter()
            .map(|obj| Note {
                t: obj.t,
                // research 01 l.58, l.82: floor(x*K/512) clamped, never exact-x matching.
                col: u8::try_from(
                    (obj.x * keys)
                        .div_euclid(PLAYFIELD_WIDTH)
                        .clamp(0, keys - 1),
                )
                .unwrap_or(0),
                kind: obj.kind,
            })
            .collect();

        let meta = ChartMeta {
            title: raw.title,
            artist: raw.artist,
            version: raw.version_name,
            creator: raw.creator,
            set_id: raw.set_id,
            beatmap_id: raw.beatmap_id,
            od: raw.od,
            hp: raw.hp,
            audio_filename: raw.audio_filename,
            format_version,
        };
        let chart = Chart::from_notes(keymode, meta, raw.timing, notes, &mut diags);
        Ok(Decoded {
            chart,
            diagnostics: diags,
        })
    }
}

fn keymode_from(circle_size: Option<f64>, diags: &mut Diagnostics) -> Keymode {
    let keys = match circle_size {
        None => {
            diags.push(DiagCode::MissingCircleSize, "");
            DEFAULT_CIRCLE_SIZE
        }
        Some(cs) => {
            let rounded = cs.round();
            if !(MIN_KEYS..=MAX_KEYS).contains(&rounded) {
                diags.push(DiagCode::KeymodeClamped, format!("circle_size={cs}"));
            }
            rounded.clamp(MIN_KEYS, MAX_KEYS) as u8
        }
    };
    // `keys` is within 1..=16 by construction, so the fallback is unreachable.
    Keymode::new(keys).unwrap_or(Keymode::K7)
}

/// `Some(false)` when a UTF-8 file does not open with the version header. UTF-16 files return
/// `None` and trust rosu-map, which has read the header by then.
fn has_version_line(bytes: &[u8]) -> Option<bool> {
    if bytes.starts_with(&[0xFF, 0xFE]) || bytes.starts_with(&[0xFE, 0xFF]) {
        return None;
    }
    let body = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
    let first = body
        .split(|&b| b == b'\n')
        .map(<[u8]>::trim_ascii)
        .find(|line| !line.is_empty());
    Some(first.is_some_and(|line| line.starts_with(VERSION_PREFIX)))
}

/// Round half away from zero to the nearest microsecond. One IEEE multiply plus `round` is exact
/// and platform-independent, and integer-ms inputs convert without loss.
fn ms_to_us(text: &str) -> Option<TimeUs> {
    let ms = parse_finite(text)?;
    if ms.abs() > MAX_ABS_MS {
        return None;
    }
    Some(TimeUs((ms * 1000.0).round() as i64))
}

fn parse_finite(text: &str) -> Option<f64> {
    text.trim().parse::<f64>().ok().filter(|v| v.is_finite())
}

fn trim_comment(line: &str) -> &str {
    line.find("//").map_or(line, |i| &line[..i]).trim()
}

fn key_value(line: &str) -> Option<(&str, &str)> {
    line.split_once(':').map(|(k, v)| (k.trim(), v.trim()))
}

struct RawObject {
    x: i64,
    t: TimeUs,
    kind: NoteKind,
}

#[derive(Default)]
struct RawOsu {
    version: i32,
    mode: Option<String>,
    audio_filename: String,
    title: String,
    artist: String,
    creator: String,
    version_name: String,
    beatmap_id: Option<u32>,
    set_id: Option<u32>,
    circle_size: Option<f64>,
    od: f32,
    hp: f32,
    timing: Vec<TimingPoint>,
    objects: Vec<RawObject>,
    diags: Diagnostics,
    timing_index: u32,
    object_index: u32,
}

impl RawOsu {
    fn malformed(&mut self, what: impl Into<String>) {
        self.diags.push(DiagCode::MalformedLine, what);
    }

    fn parse_id(&mut self, key: &str, value: &str) -> Option<u32> {
        match value.parse::<i64>() {
            Ok(id) => u32::try_from(id).ok().filter(|&id| id > 0),
            Err(_) => {
                self.malformed(format!("metadata.{key}"));
                None
            }
        }
    }

    fn parse_stat(&mut self, key: &str, value: &str) -> Option<f64> {
        let parsed = parse_finite(value);
        if parsed.is_none() {
            self.malformed(format!("difficulty.{key}"));
        }
        parsed
    }

    fn timing_line(&mut self, line: &str) {
        let index = self.timing_index;
        self.timing_index += 1;
        let fields: Vec<&str> = trim_comment(line).split(',').collect();
        let (Some(time), Some(beat_len)) = (fields.first(), fields.get(1)) else {
            self.malformed(format!("timing_points[{index}]"));
            return;
        };
        let (Some(t), Some(beat_len)) = (ms_to_us(time), parse_finite(beat_len)) else {
            self.malformed(format!("timing_points[{index}]"));
            return;
        };
        // Pre-v6 files have no `uninherited` field; they only had red lines.
        let uninherited = fields
            .get(6)
            .is_none_or(|flag| flag.trim().starts_with('1'));
        let kind = if uninherited {
            if beat_len <= 0.0 {
                self.diags.push(
                    DiagCode::BadTimingPoint,
                    format!("timing_points[{index}] beat_len={beat_len}"),
                );
                return;
            }
            let meter = fields
                .get(2)
                .and_then(|m| m.trim().parse::<u32>().ok())
                .filter(|&m| m > 0)
                .unwrap_or(DEFAULT_METER);
            TimingKind::Uninherited {
                beat_len_ms: beat_len,
                meter,
            }
        } else {
            // A green line's beat length is -100 / SV; a non-negative one means SV 1 (lazer).
            let sv = if beat_len < 0.0 {
                -100.0 / beat_len
            } else {
                1.0
            };
            TimingKind::Inherited { sv }
        };
        self.timing.push(TimingPoint { t, kind });
    }

    fn hit_object_line(&mut self, line: &str) {
        let index = self.object_index;
        self.object_index += 1;
        let fields: Vec<&str> = trim_comment(line).split(',').collect();
        let (Some(x), Some(time), Some(kind)) = (fields.first(), fields.get(2), fields.get(3))
        else {
            self.malformed(format!("hit_objects[{index}]"));
            return;
        };
        let x = parse_finite(x).filter(|x| x.abs() <= MAX_COORDINATE);
        let (Some(x), Some(t), Ok(kind)) = (x, ms_to_us(time), kind.trim().parse::<i32>()) else {
            self.malformed(format!("hit_objects[{index}]"));
            return;
        };
        let kind = if kind & HOLD_BIT != 0 {
            let end = fields
                .get(5)
                .and_then(|extras| extras.split(':').next())
                .and_then(ms_to_us);
            match end {
                Some(end) => NoteKind::Hold { end },
                None => {
                    self.malformed(format!("hit_objects[{index}] end_time"));
                    NoteKind::Tap
                }
            }
        } else if kind & CIRCLE_BIT != 0 {
            NoteKind::Tap
        } else {
            self.diags.push(
                DiagCode::UnsupportedHitObject,
                format!("hit_objects[{index}] type={kind}"),
            );
            return;
        };
        self.objects.push(RawObject {
            // lazer and rosu-map truncate x to an integer before mapping columns.
            x: x.trunc() as i64,
            t,
            kind,
        });
    }
}

impl DecodeState for RawOsu {
    fn create(version: i32) -> Self {
        Self {
            version,
            ..Self::default()
        }
    }
}

impl DecodeBeatmap for RawOsu {
    type Error = Infallible;
    type State = Self;

    fn parse_general(state: &mut Self, line: &str) -> Result<(), Infallible> {
        match key_value(trim_comment(line)) {
            Some(("Mode", value)) => state.mode = Some(value.to_owned()),
            Some(("AudioFilename", value)) => state.audio_filename = value.to_owned(),
            _ => {}
        }
        Ok(())
    }

    // Titles may legitimately contain `//`, so metadata lines keep it (as lazer does).
    fn parse_metadata(state: &mut Self, line: &str) -> Result<(), Infallible> {
        let Some((key, value)) = key_value(line) else {
            return Ok(());
        };
        match key {
            "Title" => state.title = value.to_owned(),
            "Artist" => state.artist = value.to_owned(),
            "Creator" => state.creator = value.to_owned(),
            "Version" => state.version_name = value.to_owned(),
            "BeatmapID" => state.beatmap_id = state.parse_id(key, value),
            "BeatmapSetID" => state.set_id = state.parse_id(key, value),
            _ => {}
        }
        Ok(())
    }

    fn parse_difficulty(state: &mut Self, line: &str) -> Result<(), Infallible> {
        let Some((key, value)) = key_value(trim_comment(line)) else {
            return Ok(());
        };
        match key {
            "CircleSize" => state.circle_size = state.parse_stat(key, value),
            "OverallDifficulty" => {
                if let Some(od) = state.parse_stat(key, value) {
                    state.od = od as f32;
                }
            }
            "HPDrainRate" => {
                if let Some(hp) = state.parse_stat(key, value) {
                    state.hp = hp as f32;
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn parse_timing_points(state: &mut Self, line: &str) -> Result<(), Infallible> {
        state.timing_line(line);
        Ok(())
    }

    fn parse_hit_objects(state: &mut Self, line: &str) -> Result<(), Infallible> {
        state.hit_object_line(line);
        Ok(())
    }

    fn parse_editor(_: &mut Self, _: &str) -> Result<(), Infallible> {
        Ok(())
    }

    fn parse_events(_: &mut Self, _: &str) -> Result<(), Infallible> {
        Ok(())
    }

    fn parse_colors(_: &mut Self, _: &str) -> Result<(), Infallible> {
        Ok(())
    }

    fn parse_variables(_: &mut Self, _: &str) -> Result<(), Infallible> {
        Ok(())
    }

    fn parse_catch_the_beat(_: &mut Self, _: &str) -> Result<(), Infallible> {
        Ok(())
    }

    fn parse_mania(_: &mut Self, _: &str) -> Result<(), Infallible> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use wolluf_core::{ColMask, Keymode, TimeUs};

    use super::*;
    use crate::diag::DiagCode;
    use crate::error::ChartError;
    use crate::model::{LnPair, TimingKind, TimingPoint};
    use crate::testkit::OsuText;

    fn decode(text: &OsuText) -> Decoded {
        OsuDecoder.decode(text.build().as_bytes()).unwrap()
    }

    fn column_of(keys: u8, x: &str) -> u8 {
        let text = OsuText::mania(keys).hit_object(&format!("{x},192,1000,1,0,0:0:0:0:"));
        let decoded = decode(&text);
        assert!(decoded.diagnostics.is_empty(), "{:?}", decoded.diagnostics);
        let cols: Vec<u8> = decoded.chart.rows()[0].tap.iter().collect();
        assert_eq!(cols.len(), 1);
        cols[0]
    }

    fn ms(v: i32) -> TimeUs {
        TimeUs::from_ms(v)
    }

    #[test]
    fn column_mapping_k4_edges() {
        let cases = [
            ("0", 0),
            ("127", 0),
            ("128", 1),
            ("255", 1),
            ("256", 2),
            ("383", 2),
            ("384", 3),
            ("511", 3),
            ("512", 3),
            ("700", 3),
            ("-5", 0),
            ("64", 0),
            ("448", 3),
        ];
        for (x, col) in cases {
            assert_eq!(column_of(4, x), col, "x={x}");
        }
    }

    #[test]
    fn column_mapping_k7_edges() {
        let cases = [
            ("0", 0),
            ("73", 0),
            ("74", 1),
            ("146", 1),
            ("147", 2),
            ("219", 2),
            ("220", 3),
            ("292", 3),
            ("293", 4),
            ("365", 4),
            ("366", 5),
            ("438", 5),
            ("439", 6),
            ("511", 6),
            ("73.9", 0),
        ];
        for (x, col) in cases {
            assert_eq!(column_of(7, x), col, "x={x}");
        }
        // The x values stable's editor writes for 7K.
        for (col, x) in ["36", "109", "182", "256", "329", "402", "475"]
            .iter()
            .enumerate()
        {
            assert_eq!(usize::from(column_of(7, x)), col, "x={x}");
        }
    }

    #[test]
    fn keymode_is_rounded_circle_size_clamped() {
        let km = |cs: &str| {
            decode(&OsuText::mania(7).difficulty("CircleSize", cs))
                .chart
                .keymode()
        };
        assert_eq!(km("7"), Keymode::K7);
        assert_eq!(km("6.5"), Keymode::K7);
        assert_eq!(km("4.4"), Keymode::K4);
        let clamped = decode(&OsuText::mania(7).difficulty("CircleSize", "18"));
        assert_eq!(clamped.chart.keymode(), Keymode::new(16).unwrap());
        assert_eq!(clamped.diagnostics.codes(), vec![DiagCode::KeymodeClamped]);
        let zero = decode(&OsuText::mania(7).difficulty("CircleSize", "0"));
        assert_eq!(zero.chart.keymode(), Keymode::new(1).unwrap());
        assert_eq!(zero.diagnostics.codes(), vec![DiagCode::KeymodeClamped]);
    }

    #[test]
    fn missing_circle_size_defaults_to_five_with_a_diagnostic() {
        let decoded = decode(&OsuText::mania(7).remove("Difficulty", "CircleSize"));
        assert_eq!(decoded.chart.keymode(), Keymode::new(5).unwrap());
        assert_eq!(
            decoded.diagnostics.codes(),
            vec![DiagCode::MissingCircleSize]
        );
    }

    #[test]
    fn hold_note_becomes_an_ln_pair() {
        let decoded = decode(
            &OsuText::mania(7)
                .hit_object("256,192,1000,128,0,1500:0:0:0:0:")
                .hit_object("36,192,1500,1,0,0:0:0:0:"),
        );
        assert!(decoded.diagnostics.is_empty(), "{:?}", decoded.diagnostics);
        let chart = decoded.chart;
        assert_eq!(
            chart.ln_pairs(),
            &[LnPair {
                head: ms(1000),
                tail: ms(1500),
                col: 3
            }]
        );
        assert_eq!(chart.rows().len(), 2);
        assert_eq!(
            chart.rows()[0].ln_head,
            ColMask::single(Keymode::K7, 3).unwrap()
        );
        assert_eq!(
            chart.rows()[1].ln_tail,
            ColMask::single(Keymode::K7, 3).unwrap()
        );
        assert_eq!(
            chart.rows()[1].tap,
            ColMask::single(Keymode::K7, 0).unwrap()
        );
    }

    #[test]
    fn hold_type_bit_wins_over_other_bits() {
        // 132 = hold | new combo, as some editors write.
        let decoded = decode(&OsuText::mania(7).hit_object("256,192,1000,132,0,1200:0:0:0:0:"));
        assert_eq!(decoded.chart.ln_pairs().len(), 1);
    }

    #[test]
    fn hold_with_tail_not_after_head_degrades_to_tap() {
        let decoded = decode(&OsuText::mania(7).hit_object("256,192,1000,128,0,900:0:0:0:0:"));
        assert_eq!(
            decoded.diagnostics.codes(),
            vec![DiagCode::LnTailNotAfterHead]
        );
        assert!(decoded.chart.ln_pairs().is_empty());
        assert_eq!(
            decoded.chart.rows()[0].tap,
            ColMask::single(Keymode::K7, 3).unwrap()
        );
    }

    #[test]
    fn hold_without_end_time_degrades_to_tap() {
        let decoded = decode(&OsuText::mania(7).hit_object("256,192,1000,128,0"));
        assert_eq!(decoded.diagnostics.codes(), vec![DiagCode::MalformedLine]);
        assert_eq!(
            decoded.chart.rows()[0].tap,
            ColMask::single(Keymode::K7, 3).unwrap()
        );
    }

    #[test]
    fn fractional_ms_round_to_nearest_microsecond() {
        let decoded = decode(
            &OsuText::mania(7)
                .hit_object("36,192,1000.4,1,0,0:0:0:0:")
                .hit_object("109,192,12.5,1,0,0:0:0:0:")
                .hit_object("182,192,-0.0004,1,0,0:0:0:0:")
                .hit_object("256,192,-3,1,0,0:0:0:0:"),
        );
        let times: Vec<i64> = decoded.chart.rows().iter().map(|r| r.t.0).collect();
        assert_eq!(times, vec![-3000, 0, 12_500, 1_000_400]);
    }

    #[test]
    fn unsupported_modes_are_typed_errors() {
        for (mode, expected) in [("0", 0), ("1", 1), ("2", 2), ("5", 5)] {
            let text = OsuText::mania(7).general("Mode", mode).build();
            match OsuDecoder.decode(text.as_bytes()) {
                Err(ChartError::UnsupportedMode(m)) => assert_eq!(m, expected),
                other => panic!("mode {mode}: {other:?}"),
            }
        }
        let missing = OsuText::mania(7).remove("General", "Mode").build();
        assert!(matches!(
            OsuDecoder.decode(missing.as_bytes()),
            Err(ChartError::UnsupportedMode(0))
        ));
        let garbage = OsuText::mania(7).general("Mode", "mania").build();
        assert!(matches!(
            OsuDecoder.decode(garbage.as_bytes()),
            Err(ChartError::InvalidMode(_))
        ));
    }

    #[test]
    fn malformed_and_unsupported_lines_are_skipped_with_diagnostics() {
        let decoded = decode(
            &OsuText::mania(7)
                .hit_object("garbage")
                .hit_object("36,192")
                .hit_object("36,192,abc,1,0")
                .hit_object("36,192,100,2,0,B|200:200,1,100")
                .hit_object("36,192,100,8,0,300")
                .hit_object("109,192,200,1,0,0:0:0:0:")
                .timing_line("nonsense")
                .timing_line("500,0,4,1,0,100,1,0"),
        );
        assert_eq!(
            decoded.diagnostics.codes(),
            // [TimingPoints] precedes [HitObjects] in the file.
            vec![
                DiagCode::MalformedLine,
                DiagCode::BadTimingPoint,
                DiagCode::MalformedLine,
                DiagCode::MalformedLine,
                DiagCode::MalformedLine,
                DiagCode::UnsupportedHitObject,
                DiagCode::UnsupportedHitObject,
            ]
        );
        assert_eq!(decoded.chart.rows().len(), 1);
        assert_eq!(decoded.chart.rows()[0].t, ms(200));
    }

    #[test]
    fn duplicate_notes_are_reported() {
        let decoded = decode(
            &OsuText::mania(7)
                .hit_object("36,192,100,1,0,0:0:0:0:")
                .hit_object("40,192,100,1,0,0:0:0:0:"),
        );
        assert_eq!(decoded.diagnostics.codes(), vec![DiagCode::DuplicateNote]);
        assert_eq!(decoded.chart.rows().len(), 1);
    }

    #[test]
    fn metadata_and_difficulty_are_read() {
        let decoded = decode(
            &OsuText::mania(7)
                .metadata("Title", "Song: Remix")
                .metadata("Artist", "Someone")
                .metadata("Creator", "Mapper")
                .metadata("Version", "7K Another")
                .metadata("BeatmapID", "123")
                .metadata("BeatmapSetID", "-1")
                .difficulty("OverallDifficulty", "8.5")
                .difficulty("HPDrainRate", "7")
                .general("AudioFilename", "audio.mp3"),
        );
        let meta = decoded.chart.meta();
        assert_eq!(meta.title, "Song: Remix");
        assert_eq!(meta.artist, "Someone");
        assert_eq!(meta.creator, "Mapper");
        assert_eq!(meta.version, "7K Another");
        assert_eq!(meta.beatmap_id, Some(123));
        assert_eq!(meta.set_id, None);
        assert_eq!(meta.od, 8.5);
        assert_eq!(meta.hp, 7.0);
        assert_eq!(meta.audio_filename, "audio.mp3");
        assert_eq!(meta.format_version, Some(14));
    }

    #[test]
    fn missing_format_version_is_reported() {
        let decoded = decode(&OsuText::mania(7).without_version());
        assert_eq!(decoded.chart.meta().format_version, None);
        assert_eq!(
            decoded.diagnostics.codes(),
            vec![DiagCode::MissingFormatVersion]
        );
    }

    #[test]
    fn timing_lines_are_kept_raw() {
        let decoded = decode(
            &OsuText::mania(7)
                .timing_line("1000,-50,4,1,0,100,0,0")
                .timing_line("0,500,3,1,0,100,1,0")
                .timing_line("2000.5,-2000,4,1,0,100,0,0")
                .timing_line("3000,300"),
        );
        assert!(decoded.diagnostics.is_empty(), "{:?}", decoded.diagnostics);
        assert_eq!(
            decoded.chart.timing(),
            &[
                TimingPoint {
                    t: ms(0),
                    kind: TimingKind::Uninherited {
                        beat_len_ms: 500.0,
                        meter: 3
                    }
                },
                TimingPoint {
                    t: ms(1000),
                    kind: TimingKind::Inherited { sv: 2.0 }
                },
                TimingPoint {
                    t: TimeUs(2_000_500),
                    kind: TimingKind::Inherited { sv: 0.05 }
                },
                TimingPoint {
                    t: ms(3000),
                    kind: TimingKind::Uninherited {
                        beat_len_ms: 300.0,
                        meter: 4
                    }
                },
            ]
        );
    }

    #[test]
    fn utf8_bom_and_crlf_are_accepted() {
        let text = OsuText::mania(7)
            .hit_object("36,192,100,1,0,0:0:0:0:")
            .build();
        let mut bytes = vec![0xEF, 0xBB, 0xBF];
        bytes.extend_from_slice(text.replace('\n', "\r\n").as_bytes());
        let decoded = OsuDecoder.decode(&bytes).unwrap();
        assert!(decoded.diagnostics.is_empty(), "{:?}", decoded.diagnostics);
        assert_eq!(decoded.chart.meta().format_version, Some(14));
        assert_eq!(decoded.chart.rows().len(), 1);
    }

    #[test]
    fn empty_input_is_unsupported_mode_not_a_panic() {
        assert!(matches!(
            OsuDecoder.decode(b""),
            Err(ChartError::UnsupportedMode(0))
        ));
    }

    #[test]
    fn decoded_fixture_snapshot() {
        let text = OsuText::mania(7)
            .metadata("Title", "Fixture")
            .metadata("Version", "7K Snapshot")
            .metadata("BeatmapID", "42")
            .metadata("BeatmapSetID", "7")
            .difficulty("OverallDifficulty", "8.5")
            .timing_line("0,375,4,1,0,100,1,0")
            .timing_line("1500,-133.333333333333,4,1,0,100,0,0")
            .hit_object("36,192,0,1,0,0:0:0:0:")
            .hit_object("256,192,0,128,0,750:0:0:0:0:")
            .hit_object("109,192,375,1,0,0:0:0:0:")
            .hit_object("475,192,375,1,0,0:0:0:0:")
            .hit_object("475,192,375,1,0,0:0:0:0:")
            .hit_object("182,192,750,128,0,1500:0:0:0:0:")
            .hit_object("256,192,750,1,0,0:0:0:0:")
            .hit_object("402,192,1125,1,0,0:0:0:0:")
            .hit_object("329,192,1500.5,1,0,0:0:0:0:");
        let decoded = decode(&text);
        let chart = &decoded.chart;
        let snapshot = format!(
            "keymode: {}\nmeta: {:#?}\ntiming: {:#?}\nln_pairs: {:?}\ndiagnostics: {:?}\nrows:\n{}",
            chart.keymode().columns(),
            chart.meta(),
            chart.timing(),
            chart.ln_pairs(),
            decoded.diagnostics.as_slice(),
            crate::testkit::render_rows(chart),
        );
        insta::assert_snapshot!(snapshot);
    }

    #[test]
    fn format_id_is_osu() {
        assert_eq!(OsuDecoder.format_id(), "osu");
    }
}

#[cfg(test)]
mod props {
    use proptest::prelude::*;
    use wolluf_core::Keymode;

    use super::*;
    use crate::diag::Diagnostics;
    use crate::model::ChartMeta;
    use crate::testkit::{OsuText, check_invariants};

    fn arb_hit_object() -> impl Strategy<Value = String> {
        (
            -20i32..540,
            -100i32..5_000,
            prop::sample::select(vec![1, 5, 128, 132, 2, 8, 0]),
            -200i32..800,
        )
            .prop_map(|(x, t, kind, len)| format!("{x},192,{t},{kind},0,{}:0:0:0:0:", t + len))
    }

    proptest! {
        #[test]
        fn decoded_rows_hold_invariants(
            cs in 0u8..=20,
            objects in prop::collection::vec(arb_hit_object(), 0..150),
        ) {
            let text = objects
                .iter()
                .fold(OsuText::mania(7).difficulty("CircleSize", &cs.to_string()), |t, o| t.hit_object(o))
                .build();
            let first = OsuDecoder.decode(text.as_bytes()).unwrap();
            prop_assert_eq!(check_invariants(&first.chart), Ok(()));
            let second = OsuDecoder.decode(text.as_bytes()).unwrap();
            prop_assert_eq!(first, second);
        }

        #[test]
        fn normalized_charts_round_trip_through_osu_text(
            keys in 1u8..=16,
            notes in crate::model::props::arb_notes(16),
        ) {
            let keymode = Keymode::new(keys).unwrap();
            let mut diags = Diagnostics::new();
            let chart = Chart::from_notes(keymode, ChartMeta::default(), Vec::new(), notes, &mut diags);
            let decoded = OsuDecoder.decode(OsuText::from_chart(&chart).build().as_bytes()).unwrap();
            prop_assert!(decoded.diagnostics.is_empty(), "{:?}", decoded.diagnostics);
            prop_assert_eq!(decoded.chart.keymode(), chart.keymode());
            prop_assert_eq!(decoded.chart.rows(), chart.rows());
            prop_assert_eq!(decoded.chart.ln_pairs(), chart.ln_pairs());
        }

        #[test]
        fn arbitrary_bytes_never_panic(bytes in prop::collection::vec(any::<u8>(), 0..512)) {
            let _ = OsuDecoder.decode(&bytes);
        }

        #[test]
        fn mania_header_with_arbitrary_body_never_panics(body in "[ -~\n]{0,400}") {
            let text = format!("osu file format v14\n[General]\nMode: 3\n[Difficulty]\nCircleSize: 7\n[HitObjects]\n{body}");
            if let Ok(decoded) = OsuDecoder.decode(text.as_bytes()) {
                prop_assert_eq!(check_invariants(&decoded.chart), Ok(()));
            }
        }
    }
}
