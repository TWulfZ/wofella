//! The `chart_parsed.rows_blob` encoding: a format-version byte, then zstd over postcard of a
//! serde mirror of the chart. The mirror lives here so `wolluf-chart` stays free of serde.
//!
//! The zstd frame carries a content checksum and the payload must fill it exactly, so a corrupt
//! blob fails to decode instead of yielding another chart.
//!
//! Only rows are stored: LN pairs follow from them, because LNs of one column never overlap, so
//! each head pairs with the next tail of its column. Decoding rebuilds the chart through
//! `Chart::from_notes` and rejects any blob whose rows do not come back unchanged.

use serde::{Deserialize, Serialize};
use wolluf_chart::{Chart, ChartMeta, Diagnostics, Note, NoteKind, Row, TimingKind, TimingPoint};
use wolluf_core::{ColMask, Keymode, TimeUs};
use zstd::stream::raw::CParameter;

use crate::error::EngineError;

/// Append-only: a new layout gets a new byte and a decoder arm. It also enters the
/// `chart_parse` config hash, so a format change re-keys the cache.
pub const ROWS_FORMAT_V1: u8 = 1;

/// Only affects size and speed; any level decodes with the same reader.
const ZSTD_LEVEL: i32 = 3;

#[derive(Debug, Serialize, Deserialize)]
struct RowsV1 {
    keymode: u8,
    meta: MetaV1,
    timing: Vec<TimingV1>,
    rows: Vec<RowV1>,
}

/// `dt` is the time since the previous row (the first row's since 0): small varints.
#[derive(Debug, Serialize, Deserialize)]
struct RowV1 {
    dt: i64,
    tap: u16,
    head: u16,
    tail: u16,
}

#[derive(Debug, Serialize, Deserialize)]
enum TimingV1 {
    Uninherited {
        t: i64,
        beat_len_ms: f64,
        meter: u32,
    },
    Inherited {
        t: i64,
        sv: f64,
    },
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct MetaV1 {
    title: String,
    artist: String,
    version: String,
    creator: String,
    set_id: Option<u32>,
    beatmap_id: Option<u32>,
    od: f32,
    hp: f32,
    audio_filename: String,
    format_version: Option<i32>,
}

pub fn encode_rows(chart: &Chart) -> Result<Vec<u8>, EngineError> {
    let raw = postcard::to_allocvec(&mirror(chart)).map_err(corrupt)?;
    let mut zstd = zstd::bulk::Compressor::new(ZSTD_LEVEL).map_err(corrupt)?;
    // Without it a flipped bit in a literal (a title, a delta) can decode to another valid chart.
    zstd.set_parameter(CParameter::ChecksumFlag(true))
        .map_err(corrupt)?;
    let mut blob = vec![ROWS_FORMAT_V1];
    blob.extend(zstd.compress(&raw).map_err(corrupt)?);
    Ok(blob)
}

pub fn decode_rows(blob: &[u8]) -> Result<Chart, EngineError> {
    let (&format, body) = blob.split_first().ok_or(EngineError::EmptyRowsBlob)?;
    if format != ROWS_FORMAT_V1 {
        return Err(EngineError::UnsupportedRowsFormat(format));
    }
    let raw = zstd::decode_all(body).map_err(corrupt)?;
    let (payload, rest): (RowsV1, _) = postcard::take_from_bytes(&raw).map_err(corrupt)?;
    if !rest.is_empty() {
        return Err(corrupt(format!("{} trailing bytes", rest.len())));
    }
    rebuild(payload)
}

fn corrupt(err: impl std::fmt::Display) -> EngineError {
    EngineError::CorruptRowsBlob(err.to_string())
}

fn mirror(chart: &Chart) -> RowsV1 {
    let meta = chart.meta();
    let mut prev = TimeUs::ZERO;
    let rows = chart
        .rows()
        .iter()
        .map(|row| {
            let dt = row.t.0.wrapping_sub(prev.0);
            prev = row.t;
            RowV1 {
                dt,
                tap: row.tap.bits(),
                head: row.ln_head.bits(),
                tail: row.ln_tail.bits(),
            }
        })
        .collect();
    let timing = chart
        .timing()
        .iter()
        .map(|p| match p.kind {
            TimingKind::Uninherited { beat_len_ms, meter } => TimingV1::Uninherited {
                t: p.t.0,
                beat_len_ms,
                meter,
            },
            TimingKind::Inherited { sv } => TimingV1::Inherited { t: p.t.0, sv },
        })
        .collect();
    RowsV1 {
        keymode: chart.keymode().columns(),
        meta: MetaV1 {
            title: meta.title.clone(),
            artist: meta.artist.clone(),
            version: meta.version.clone(),
            creator: meta.creator.clone(),
            set_id: meta.set_id,
            beatmap_id: meta.beatmap_id,
            od: meta.od,
            hp: meta.hp,
            audio_filename: meta.audio_filename.clone(),
            format_version: meta.format_version,
        },
        timing,
        rows,
    }
}

fn rebuild(payload: RowsV1) -> Result<Chart, EngineError> {
    let keymode = Keymode::new(payload.keymode).map_err(corrupt)?;
    let mask = |bits: u16| ColMask::from_bits(keymode, bits).map_err(corrupt);

    let mut rows = Vec::with_capacity(payload.rows.len());
    let mut t = TimeUs::ZERO;
    for raw in &payload.rows {
        t = t
            .checked_add(TimeUs(raw.dt))
            .ok_or_else(|| corrupt("row time overflows"))?;
        rows.push(Row {
            t,
            tap: mask(raw.tap)?,
            ln_head: mask(raw.head)?,
            ln_tail: mask(raw.tail)?,
        });
    }

    let mut open: Vec<Option<TimeUs>> = vec![None; usize::from(keymode.columns())];
    let mut notes = Vec::new();
    for row in &rows {
        notes.extend(row.tap.iter().map(|col| Note {
            t: row.t,
            col,
            kind: NoteKind::Tap,
        }));
        // Order is moot for valid rows: `from_notes` drops a head at its column's previous tail,
        // so no row ends and starts an LN in one column; the final check rejects such a blob.
        for col in row.ln_tail.iter() {
            let head = open
                .get_mut(usize::from(col))
                .and_then(Option::take)
                .ok_or_else(|| corrupt(format!("LN tail without head at col {col}")))?;
            notes.push(Note {
                t: head,
                col,
                kind: NoteKind::Hold { end: row.t },
            });
        }
        for col in row.ln_head.iter() {
            match open.get_mut(usize::from(col)) {
                Some(slot @ None) => *slot = Some(row.t),
                _ => return Err(corrupt(format!("LN head inside an LN at col {col}"))),
            }
        }
    }
    if let Some(col) = open.iter().position(Option::is_some) {
        return Err(corrupt(format!("unclosed LN at col {col}")));
    }

    let meta = payload.meta;
    let meta = ChartMeta {
        title: meta.title,
        artist: meta.artist,
        version: meta.version,
        creator: meta.creator,
        set_id: meta.set_id,
        beatmap_id: meta.beatmap_id,
        od: meta.od,
        hp: meta.hp,
        audio_filename: meta.audio_filename,
        format_version: meta.format_version,
    };
    let timing = payload
        .timing
        .into_iter()
        .map(|p| match p {
            TimingV1::Uninherited {
                t,
                beat_len_ms,
                meter,
            } => TimingPoint {
                t: TimeUs(t),
                kind: TimingKind::Uninherited { beat_len_ms, meter },
            },
            TimingV1::Inherited { t, sv } => TimingPoint {
                t: TimeUs(t),
                kind: TimingKind::Inherited { sv },
            },
        })
        .collect();

    let mut diags = Diagnostics::new();
    let chart = Chart::from_notes(keymode, meta, timing, notes, &mut diags);
    if !diags.is_empty() || chart.rows() != rows.as_slice() {
        return Err(corrupt("rows are not a normalized chart"));
    }
    Ok(chart)
}

#[cfg(test)]
mod tests {
    use wolluf_chart::testkit::OsuText;
    use wolluf_chart::{ChartDecoder, OsuDecoder, chart};
    use wolluf_core::{Keymode, TimeUs};

    use super::*;

    fn decoded(text: &OsuText) -> Chart {
        OsuDecoder.decode(text.build().as_bytes()).unwrap().chart
    }

    fn compress(payload: &RowsV1) -> Vec<u8> {
        let raw = postcard::to_allocvec(payload).unwrap();
        let mut blob = vec![ROWS_FORMAT_V1];
        blob.extend(zstd::encode_all(raw.as_slice(), 0).unwrap());
        blob
    }

    #[test]
    fn round_trips_rows_lns_timing_and_meta() {
        let dsl = chart![step = 125, start = 500;
            "[..x..[",
            "|.x.x.|",
            "|x...x]",
            "]..x...",
            "xxxxxxx",
        ];
        let chart = decoded(
            &OsuText::from_chart(&dsl)
                .metadata("Title", "Round Trip")
                .metadata("BeatmapSetID", "42")
                .difficulty("OverallDifficulty", "8.5")
                .timing_line("500,333.3333,4,1,0,100,1,0")
                .timing_line("750,-50,4,1,0,100,0,0"),
        );
        assert_eq!(chart.timing().len(), 2);
        let blob = encode_rows(&chart).unwrap();
        assert_eq!(blob[0], ROWS_FORMAT_V1);
        assert_eq!(decode_rows(&blob).unwrap(), chart);
    }

    #[test]
    fn empty_chart_round_trips() {
        let chart = decoded(&OsuText::mania(4));
        assert!(chart.rows().is_empty());
        assert_eq!(decode_rows(&encode_rows(&chart).unwrap()).unwrap(), chart);
    }

    #[test]
    fn rejects_empty_unknown_format_and_garbage() {
        assert!(matches!(decode_rows(&[]), Err(EngineError::EmptyRowsBlob)));
        let mut blob = encode_rows(&chart![step = 100; "x......"]).unwrap();
        blob[0] = 2;
        assert!(matches!(
            decode_rows(&blob),
            Err(EngineError::UnsupportedRowsFormat(2))
        ));
        assert!(matches!(
            decode_rows(&[ROWS_FORMAT_V1, 1, 2, 3]),
            Err(EngineError::CorruptRowsBlob(_))
        ));
        let truncated = compress(&RowsV1 {
            keymode: 7,
            meta: MetaV1::default(),
            timing: Vec::new(),
            rows: Vec::new(),
        });
        assert!(decode_rows(&truncated[..truncated.len() - 2]).is_err());
    }

    #[test]
    fn rejects_payloads_that_are_not_a_normalized_chart() {
        let row = |dt: i64, tap: u16, head: u16, tail: u16| RowV1 {
            dt,
            tap,
            head,
            tail,
        };
        let bad = [
            ("keymode", 0, vec![row(0, 1, 0, 0)]),
            ("column outside keymode", 7, vec![row(0, 1 << 7, 0, 0)]),
            ("tail without head", 7, vec![row(0, 0, 0, 1)]),
            ("unclosed ln", 7, vec![row(0, 0, 1, 0)]),
            ("head inside ln", 7, vec![row(0, 0, 1, 0), row(10, 0, 1, 0)]),
            (
                "overlapping masks",
                7,
                vec![row(0, 1, 1, 0), row(10, 0, 0, 1)],
            ),
            (
                "rows not increasing",
                7,
                vec![row(10, 1, 0, 0), row(0, 2, 0, 0)],
            ),
            ("empty row", 7, vec![row(0, 0, 0, 0)]),
            (
                "time overflow",
                7,
                vec![row(i64::MAX, 1, 0, 0), row(1, 1, 0, 0)],
            ),
        ];
        for (name, keymode, rows) in bad {
            let blob = compress(&RowsV1 {
                keymode,
                meta: MetaV1::default(),
                timing: Vec::new(),
                rows,
            });
            assert!(
                matches!(decode_rows(&blob), Err(EngineError::CorruptRowsBlob(_))),
                "{name}"
            );
        }
    }

    /// The v1 payload of [`v1_fixture`], hand-derived from the postcard wire format. Frozen: a
    /// layout change is a new `ROWS_FORMAT` byte and decoder arm, never an edit here.
    const V1_PAYLOAD_HEX: &str = concat!(
        "04",                   // keymode
        "0154",                 // title "T"
        "00",                   // artist ""
        "024864",               // version "Hd"
        "00",                   // creator ""
        "012a",                 // set_id Some(42)
        "00",                   // beatmap_id None
        "00000841",             // od 8.5f32 LE
        "0000e040",             // hp 7.0f32 LE
        "0161",                 // audio_filename "a"
        "011c",                 // format_version Some(14), zigzag
        "02",                   // timing len
        "00000000000000407f40", // Uninherited t=0 beat_len_ms=500.0
        "04",                   // meter 4
        "01a0c21e",             // Inherited t=250_000 (zigzag varint)
        "000000000000e03f",     // sv 0.5
        "04",                   // rows len
        "00050000",             // dt=0 tap=0b0101
        "c09a0c000200",         // dt=100_000 head=0b0010
        "c09a0c080000",         // dt=100_000 tap=0b1000
        "c09a0c000002",         // dt=100_000 tail=0b0010
    );

    fn v1_fixture() -> Chart {
        let ms = TimeUs::from_ms;
        let tap = |t, col| Note {
            t,
            col,
            kind: NoteKind::Tap,
        };
        let meta = ChartMeta {
            title: "T".to_owned(),
            version: "Hd".to_owned(),
            set_id: Some(42),
            beatmap_id: None,
            od: 8.5,
            hp: 7.0,
            audio_filename: "a".to_owned(),
            format_version: Some(14),
            ..ChartMeta::default()
        };
        let timing = vec![
            TimingPoint {
                t: ms(0),
                kind: TimingKind::Uninherited {
                    beat_len_ms: 500.0,
                    meter: 4,
                },
            },
            TimingPoint {
                t: ms(250),
                kind: TimingKind::Inherited { sv: 0.5 },
            },
        ];
        let notes = vec![
            tap(ms(0), 0),
            tap(ms(0), 2),
            Note {
                t: ms(100),
                col: 1,
                kind: NoteKind::Hold { end: ms(300) },
            },
            tap(ms(200), 3),
        ];
        let mut diags = Diagnostics::new();
        let chart = Chart::from_notes(Keymode::K4, meta, timing, notes, &mut diags);
        assert!(diags.is_empty(), "{diags:?}");
        chart
    }

    fn unhex(hex: &str) -> Vec<u8> {
        (0..hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
            .collect()
    }

    #[test]
    fn v1_payload_bytes_are_frozen() {
        let chart = v1_fixture();
        let raw = postcard::to_allocvec(&mirror(&chart)).unwrap();
        assert_eq!(raw, unhex(V1_PAYLOAD_HEX));
        let mut blob = vec![ROWS_FORMAT_V1];
        blob.extend(zstd::encode_all(unhex(V1_PAYLOAD_HEX).as_slice(), 0).unwrap());
        assert_eq!(decode_rows(&blob).unwrap(), chart);
    }

    #[test]
    fn frame_carries_a_content_checksum() {
        let blob = encode_rows(&chart![step = 100; "x......"]).unwrap();
        let body = &blob[1..];
        assert_eq!(body[..4], [0x28, 0xb5, 0x2f, 0xfd]);
        // Frame_Header_Descriptor bit 2 is Content_Checksum_flag (RFC 8878 §3.1.1.1.1).
        assert_ne!(body[4] & 0b100, 0, "descriptor {:#010b}", body[4]);
    }

    // The descriptor's unused bit (RFC 8878 §3.1.1.1.1) is ignored by decoders, so a flip there
    // may still decode, but only to the same chart.
    #[test]
    fn no_bit_flip_in_the_compressed_body_decodes_to_another_chart() {
        let chart = chart![step = 125;
            "[..x..[",
            "|.x.x.|",
            "]x...x]",
        ];
        let blob = encode_rows(&chart).unwrap();
        for i in 1..blob.len() {
            for bit in 0..8 {
                let mut flipped = blob.clone();
                flipped[i] ^= 1 << bit;
                match decode_rows(&flipped) {
                    Err(_) => {}
                    Ok(decoded) => assert_eq!(decoded, chart, "byte {i} bit {bit}"),
                }
            }
        }
    }

    #[test]
    fn rejects_trailing_bytes_after_the_payload() {
        let payload = RowsV1 {
            keymode: 7,
            meta: MetaV1::default(),
            timing: Vec::new(),
            rows: Vec::new(),
        };
        let mut raw = postcard::to_allocvec(&payload).unwrap();
        raw.push(0);
        let mut blob = vec![ROWS_FORMAT_V1];
        blob.extend(zstd::encode_all(raw.as_slice(), 0).unwrap());
        assert!(matches!(
            decode_rows(&blob),
            Err(EngineError::CorruptRowsBlob(_))
        ));
    }

    #[test]
    fn row_times_are_delta_encoded() {
        let chart = chart![step = 100, start = 60_000; "x......", ".x....."];
        let blob = encode_rows(&chart).unwrap();
        let raw = zstd::decode_all(&blob[1..]).unwrap();
        let payload: RowsV1 = postcard::from_bytes(&raw).unwrap();
        let dts: Vec<i64> = payload.rows.iter().map(|r| r.dt).collect();
        assert_eq!(dts, [60_000_000, 100_000]);
        assert_eq!(TimeUs(dts.iter().sum()), chart.rows()[1].t);
    }
}

#[cfg(test)]
mod props {
    use proptest::prelude::*;
    use wolluf_chart::{ChartMeta, Diagnostics, Note, NoteKind, TimingKind, TimingPoint};
    use wolluf_core::{Keymode, TimeUs};

    use super::*;

    fn arb_notes(max_col: u8) -> impl Strategy<Value = Vec<Note>> {
        let note = (
            -2_000i64..200_000,
            0..max_col,
            prop::option::of(-50i64..4_000),
        )
            .prop_map(|(t, col, len)| {
                let t = TimeUs(t * 137);
                let kind = match len {
                    None => NoteKind::Tap,
                    Some(len) => NoteKind::Hold {
                        end: TimeUs(t.0 + len * 137),
                    },
                };
                Note { t, col, kind }
            });
        prop::collection::vec(note, 0..200)
    }

    fn arb_timing() -> impl Strategy<Value = Vec<TimingPoint>> {
        let point = (
            -5_000_000i64..500_000_000,
            prop::bool::ANY,
            -1.0e6f64..1.0e6,
            0u32..16,
        )
            .prop_map(|(t, red, value, meter)| TimingPoint {
                t: TimeUs(t),
                kind: if red {
                    TimingKind::Uninherited {
                        beat_len_ms: value,
                        meter,
                    }
                } else {
                    TimingKind::Inherited { sv: value }
                },
            });
        prop::collection::vec(point, 0..20)
    }

    fn arb_meta() -> impl Strategy<Value = ChartMeta> {
        (
            ".{0,12}",
            ".{0,12}",
            prop::option::of(any::<u32>()),
            0.0f32..11.0,
            prop::option::of(any::<i32>()),
        )
            .prop_map(|(title, version, set_id, od, format_version)| ChartMeta {
                title,
                version,
                set_id,
                od,
                format_version,
                ..ChartMeta::default()
            })
    }

    /// Up to 3 columns and 50 ms: dense enough that ties, touching LNs and overlaps are common.
    fn arb_dense_chart() -> impl Strategy<Value = (u8, Vec<Note>)> {
        (1u8..=3).prop_flat_map(|keys| {
            let note =
                (0i64..50, 0..keys, prop::option::of(-1i64..12)).prop_map(|(t, col, len)| Note {
                    t: TimeUs(t * 1_000),
                    col,
                    kind: match len {
                        None => NoteKind::Tap,
                        Some(len) => NoteKind::Hold {
                            end: TimeUs((t + len) * 1_000),
                        },
                    },
                });
            (Just(keys), prop::collection::vec(note, 0..40))
        })
    }

    proptest! {
        #[test]
        fn blob_round_trips_exactly(
            keys in 1u8..=16,
            notes in arb_notes(16),
            timing in arb_timing(),
            meta in arb_meta(),
        ) {
            let keymode = Keymode::new(keys).unwrap();
            let mut diags = Diagnostics::new();
            let chart = Chart::from_notes(keymode, meta, timing, notes, &mut diags);
            let blob = encode_rows(&chart).unwrap();
            prop_assert_eq!(decode_rows(&blob).unwrap(), chart);
        }

        #[test]
        fn dense_blob_round_trips_rows_and_ln_pairs((keys, notes) in arb_dense_chart()) {
            let keymode = Keymode::new(keys).unwrap();
            let mut diags = Diagnostics::new();
            let chart =
                Chart::from_notes(keymode, ChartMeta::default(), Vec::new(), notes, &mut diags);
            let decoded = decode_rows(&encode_rows(&chart).unwrap()).unwrap();
            prop_assert_eq!(decoded.rows(), chart.rows());
            prop_assert_eq!(decoded.ln_pairs(), chart.ln_pairs());
            prop_assert_eq!(decoded, chart);
        }
    }
}
