//! scores.db (`legacy_db.md`, `research/scripts/audit/osudb.py::read_scores_db`).

use crate::codec::score_header::{OnlineId, ScoreHeader, read_online_id, read_score_header};
use crate::codec::version::{admit, finish, is_lazer};
use crate::codec::{FileKind, OsuString, Reader};
use crate::diag::Diagnostics;
use crate::error::CodecError;

const KIND: FileKind = FileKind::ScoresDb;
/// Marker `Int` after the timestamp; it is the replay format's payload length slot.
const SCORE_MARKER: i32 = -1;
/// Smallest beatmap entry: an absent-md5 tag plus the `Int` score count.
const MIN_BEATMAP_SIZE: usize = 1 + 4;
/// Smallest score record: mode, version, three absent strings, six counts, score, combo,
/// perfect, mods, absent life bar, ticks and the marker (no online id below 20121008).
const MIN_SCORE_SIZE: usize = 1 + 4 + 3 + 12 + 4 + 2 + 1 + 4 + 1 + 8 + 4;

#[derive(Debug, Clone, PartialEq)]
pub struct ScoresDb {
    pub version: i32,
    pub beatmaps: Vec<ScoresDbBeatmap>,
}

impl ScoresDb {
    pub fn scores(&self) -> impl Iterator<Item = &ScoreRecord> {
        self.beatmaps.iter().flat_map(|b| b.scores.iter())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ScoresDbBeatmap {
    pub md5: OsuString,
    pub scores: Vec<ScoreRecord>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ScoreRecord {
    pub header: ScoreHeader,
    pub online_id: OnlineId,
    /// Present exactly when the Target Practice mod bit is set.
    pub target_practice: Option<f64>,
}

pub fn decode_scores_db(bytes: &[u8]) -> Result<(ScoresDb, Diagnostics), CodecError> {
    let mut r = Reader::new(bytes, KIND);
    let version = r.i32()?;
    let class = admit(KIND, version)?;
    let body =
        decode_body(&mut r).map(|beatmaps| (ScoresDb { version, beatmaps }, Diagnostics::new()));
    finish(KIND, version, class, body)
}

fn decode_body(r: &mut Reader<'_>) -> Result<Vec<ScoresDbBeatmap>, CodecError> {
    let n = r.count(MIN_BEATMAP_SIZE)?;
    let mut beatmaps = Vec::with_capacity(n);
    for _ in 0..n {
        let md5 = r.osu_string()?;
        let m = r.count(MIN_SCORE_SIZE)?;
        let mut scores = Vec::with_capacity(m);
        for _ in 0..m {
            scores.push(read_score_record(r)?);
        }
        beatmaps.push(ScoresDbBeatmap { md5, scores });
    }
    r.expect_eof()?;
    Ok(beatmaps)
}

fn read_score_record(r: &mut Reader<'_>) -> Result<ScoreRecord, CodecError> {
    let header = read_score_header(r)?;
    if is_lazer(header.version) {
        return Err(CodecError::UnsupportedFormat {
            kind: KIND,
            version: header.version,
        });
    }
    let marker_at = r.offset();
    if r.i32()? != SCORE_MARKER {
        return Err(CodecError::UnexpectedValue {
            kind: KIND,
            offset: marker_at,
            field: "score_marker",
        });
    }
    let online_id = read_online_id(r, header.version)?;
    let target_practice = if header.has_target_practice() {
        Some(r.f64()?)
    } else {
        None
    };
    Ok(ScoreRecord {
        header,
        online_id,
        target_practice,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codec::score_header::{OnlineId, mods};
    use crate::error::CodecError;
    use crate::testkit::{ScoreBuilder, ScoresDbBuilder, encode_scores_db};

    const MD5_A: &str = "0123456789abcdef0123456789abcdef";
    const MD5_B: &str = "fedcba9876543210fedcba9876543210";

    #[test]
    fn decodes_multiple_scores_per_beatmap() {
        let bytes = ScoresDbBuilder::new()
            .score(ScoreBuilder::mania(MD5_A, "Rosalind", 1).build())
            .score(ScoreBuilder::mania(MD5_A, "Kovacs", 2).build())
            .score(ScoreBuilder::mania(MD5_B, "Rosalind", 3).build())
            .encode();
        let (db, diags) = decode_scores_db(&bytes).unwrap();
        assert!(diags.is_empty());
        assert_eq!(db.version, 20_260_924);
        assert_eq!(db.beatmaps.len(), 2);
        assert_eq!(db.beatmaps[0].md5.as_bytes(), Some(MD5_A.as_bytes()));
        assert_eq!(db.beatmaps[0].scores.len(), 2);
        assert_eq!(
            db.beatmaps[0].scores[1].header.player,
            OsuString::present(*b"Kovacs")
        );
        assert_eq!(db.beatmaps[1].scores.len(), 1);
        assert_eq!(db.scores().count(), 3);
    }

    #[test]
    fn marker_must_be_minus_one() {
        let db = ScoresDbBuilder::new()
            .score(ScoreBuilder::mania(MD5_A, "Rosalind", 1).build())
            .build();
        let mut bytes = encode_scores_db(&db);
        // The marker sits right before the trailing i64 online id.
        let marker_at = bytes.len() - 8 - 4;
        assert_eq!(&bytes[marker_at..marker_at + 4], &(-1_i32).to_le_bytes());
        bytes[marker_at..marker_at + 4].copy_from_slice(&0_i32.to_le_bytes());
        assert_eq!(
            decode_scores_db(&bytes).map(|_| ()),
            Err(CodecError::UnexpectedValue {
                kind: FileKind::ScoresDb,
                offset: marker_at as u64,
                field: "score_marker",
            })
        );
    }

    #[test]
    fn online_id_width_by_record_version() {
        let bytes = ScoresDbBuilder::new()
            .score(
                ScoreBuilder::mania(MD5_A, "a", 1)
                    .version(20_220_424)
                    .online_id(1 << 40)
                    .build(),
            )
            .score(
                ScoreBuilder::mania(MD5_A, "b", 2)
                    .version(20_130_101)
                    .online_id(77)
                    .build(),
            )
            .score(
                ScoreBuilder::mania(MD5_A, "c", 3)
                    .version(20_100_101)
                    .online_id(5)
                    .build(),
            )
            .encode();
        let (db, _) = decode_scores_db(&bytes).unwrap();
        let ids: Vec<OnlineId> = db.scores().map(|s| s.online_id).collect();
        assert_eq!(
            ids,
            vec![OnlineId::I64(1 << 40), OnlineId::I32(77), OnlineId::Absent]
        );
        let lazer = ScoresDbBuilder::new()
            .score(
                ScoreBuilder::mania(MD5_A, "a", 1)
                    .version(30_000_001)
                    .build(),
            )
            .encode();
        assert!(matches!(
            decode_scores_db(&lazer),
            Err(CodecError::UnsupportedFormat {
                version: 30_000_001,
                ..
            })
        ));
    }

    #[test]
    fn target_practice_adds_f64() {
        let bytes = ScoresDbBuilder::new()
            .score(
                ScoreBuilder::mania(MD5_A, "a", 1)
                    .mods(mods::TARGET_PRACTICE)
                    .target_practice(0.75)
                    .build(),
            )
            .encode();
        let plain = ScoresDbBuilder::new()
            .score(ScoreBuilder::mania(MD5_A, "a", 1).build())
            .encode();
        assert_eq!(bytes.len(), plain.len() + 8);
        let (db, _) = decode_scores_db(&bytes).unwrap();
        assert_eq!(db.beatmaps[0].scores[0].target_practice, Some(0.75));
        let (db, _) = decode_scores_db(&plain).unwrap();
        assert_eq!(db.beatmaps[0].scores[0].target_practice, None);
    }

    #[test]
    fn non_utf8_player_preserved() {
        let raw = vec![0xff, 0xfe, b'W', 0x80];
        let bytes = ScoresDbBuilder::new()
            .score(
                ScoreBuilder::mania(MD5_A, "", 1)
                    .player(OsuString::Present(raw.clone()))
                    .build(),
            )
            .encode();
        let (db, _) = decode_scores_db(&bytes).unwrap();
        assert_eq!(
            db.beatmaps[0].scores[0].header.player.as_bytes(),
            Some(&raw[..])
        );
        assert_eq!(encode_scores_db(&db), bytes);
    }

    #[test]
    fn empty_player_is_present_not_absent() {
        let bytes = ScoresDbBuilder::new()
            .score(ScoreBuilder::mania(MD5_A, "", 1).build())
            .score(
                ScoreBuilder::mania(MD5_A, "", 2)
                    .player(OsuString::Absent)
                    .build(),
            )
            .encode();
        let (db, _) = decode_scores_db(&bytes).unwrap();
        assert_eq!(
            db.beatmaps[0].scores[0].header.player,
            OsuString::present(*b"")
        );
        assert_eq!(db.beatmaps[0].scores[1].header.player, OsuString::Absent);
    }

    #[test]
    fn eof_and_version_rules() {
        let mut bytes = ScoresDbBuilder::new()
            .score(ScoreBuilder::mania(MD5_A, "a", 1).build())
            .encode();
        bytes.push(0);
        assert!(matches!(
            decode_scores_db(&bytes),
            Err(CodecError::TrailingBytes { .. })
        ));
        let old = ScoresDbBuilder::new().version(20_140_608).encode();
        assert!(matches!(
            decode_scores_db(&old),
            Err(CodecError::UnsupportedFormat { .. })
        ));
        let newer = ScoresDbBuilder::new()
            .version(20_270_101)
            .score(ScoreBuilder::mania(MD5_A, "a", 1).build())
            .encode();
        let (_, diags) = decode_scores_db(&newer).unwrap();
        assert!(diags.contains(crate::diag::DiagCode::FormatUnverifiedVersion));
    }
}

#[cfg(test)]
mod props {
    use proptest::prelude::*;

    use super::*;
    use crate::codec::score_header::{JudgementCounts, OnlineId, ScoreHeader, mods};
    use crate::testkit::encode_scores_db;

    fn osu_string() -> impl Strategy<Value = OsuString> {
        prop_oneof![
            Just(OsuString::Absent),
            proptest::collection::vec(any::<u8>(), 0..40).prop_map(OsuString::Present),
        ]
    }

    fn record() -> impl Strategy<Value = ScoreRecord> {
        let counts = any::<[u16; 6]>().prop_map(|c| JudgementCounts {
            n300: c[0],
            n100: c[1],
            n50: c[2],
            geki: c[3],
            katu: c[4],
            miss: c[5],
        });
        (
            (
                any::<u8>(),
                20_000_000..30_000_000i32,
                osu_string(),
                osu_string(),
                osu_string(),
            ),
            (
                counts,
                any::<i32>(),
                any::<u16>(),
                any::<u8>(),
                any::<u32>(),
                osu_string(),
                any::<i64>(),
            ),
            (any::<i64>(), -1e6..1e6f64),
        )
            .prop_map(
                |(
                    (mode, version, beatmap_md5, player, replay_md5),
                    (counts, score, max_combo, perfect, mods, life_bar, ticks),
                    (id, tp),
                )| {
                    ScoreRecord {
                        header: ScoreHeader {
                            mode,
                            version,
                            beatmap_md5,
                            player,
                            replay_md5,
                            counts,
                            score,
                            max_combo,
                            perfect,
                            mods,
                            life_bar,
                            timestamp_ticks: ticks,
                        },
                        online_id: OnlineId::for_version(version, id),
                        target_practice: (mods & mods::TARGET_PRACTICE != 0).then_some(tp),
                    }
                },
            )
    }

    fn scores_db() -> impl Strategy<Value = ScoresDb> {
        let beatmap = (osu_string(), proptest::collection::vec(record(), 0..4))
            .prop_map(|(md5, scores)| ScoresDbBeatmap { md5, scores });
        (
            20_140_609..=20_260_924i32,
            proptest::collection::vec(beatmap, 0..4),
        )
            .prop_map(|(version, beatmaps)| ScoresDb { version, beatmaps })
    }

    proptest! {
        #[test]
        fn encode_decode_roundtrip(db in scores_db()) {
            let bytes = encode_scores_db(&db);
            let (decoded, diags) = decode_scores_db(&bytes).unwrap();
            prop_assert!(diags.is_empty());
            prop_assert_eq!(&decoded, &db);
            prop_assert_eq!(encode_scores_db(&decoded), bytes);
        }

        #[test]
        fn arbitrary_bytes_never_panic(bytes in proptest::collection::vec(any::<u8>(), 0..256)) {
            let _ = decode_scores_db(&bytes);
        }
    }
}
