//! `.osg` decoder (spec 006 H0/H1, oracle `research/scripts/osg/osg.py`). Lenient on purpose:
//! the layout is still a spike hypothesis, so unexplained bytes are kept raw and oddities are
//! warnings; only a structurally impossible file is an error (§7).

use crate::codec::score_header::JudgementCounts;
use crate::codec::{FileKind, Reader};
use crate::diag::{DiagCode, Diagnostics};
use crate::error::CodecError;

const KIND: FileKind = FileKind::Osg;
const HEADER_LEN: usize = 4 + 4;
/// ScoreV1 record: t_ms, b4, 6 counts, score, max combo, combo, b25, hp_raw, b28.
pub const STRIDE_V1: usize = 4 + 1 + 6 * 2 + 4 + 2 + 2 + 1 + 2 + 1;
/// ScoreV2 appends two f64 (`f0`, `f1`).
pub const STRIDE_V2: usize = STRIDE_V1 + 2 * 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OsgScoreSystem {
    V1,
    V2,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OsgFile {
    /// The client build (seven values on the pilot, each equal to the paired `.osr` version).
    pub client_version: i32,
    /// From the stride; `None` for a file with no records.
    pub score_system: Option<OsgScoreSystem>,
    pub records: Vec<OsgRecord>,
}

/// One record per score-changing update (H1). `b4`, `b25` and `b28` are named but not
/// interpreted until the spike confirms them.
#[derive(Debug, Clone, PartialEq)]
pub struct OsgRecord {
    pub t_ms: i32,
    /// Cumulative, in `.osr` order (300, 100, 50, MAX, 200, miss).
    pub counts: JudgementCounts,
    pub score: i32,
    pub max_combo: u16,
    pub combo: u16,
    pub hp_raw: u16,
    pub b4: u8,
    pub b25: u8,
    pub b28: u8,
    /// `f0`, `f1`; present exactly in 45-byte records.
    pub v2: Option<[f64; 2]>,
}

pub fn decode_osg(bytes: &[u8]) -> Result<(OsgFile, Diagnostics), CodecError> {
    let mut r = Reader::new(bytes, KIND);
    let client_version = r.i32()?;
    let count_at = r.offset();
    let raw_count = r.i32()?;
    let count = usize::try_from(raw_count).map_err(|_| CodecError::InvalidCount {
        kind: KIND,
        offset: count_at,
        count: i64::from(raw_count),
    })?;
    let stride = stride_for(bytes.len() - HEADER_LEN, count).ok_or(CodecError::StrideMismatch {
        len: bytes.len() as u64,
        count: raw_count,
    })?;
    let mut diags = Diagnostics::new();
    let score_system = stride.map(|s| {
        if s == STRIDE_V2 {
            OsgScoreSystem::V2
        } else {
            OsgScoreSystem::V1
        }
    });
    let mut records = Vec::with_capacity(count);
    for _ in 0..count {
        records.push(read_record(
            &mut r,
            score_system == Some(OsgScoreSystem::V2),
        )?);
    }
    r.expect_eof()?;
    if records.is_empty() {
        diags.general(DiagCode::OsgEmptyGraph, "0 records");
    }
    let osg = OsgFile {
        client_version,
        score_system,
        records,
    };
    Ok((osg, diags))
}

/// H0 is the format gate: the body must split into `count` records of exactly one known
/// stride. `Some(None)` is the valid empty file.
fn stride_for(body: usize, count: usize) -> Option<Option<usize>> {
    if count == 0 {
        return (body == 0).then_some(None);
    }
    [STRIDE_V1, STRIDE_V2]
        .into_iter()
        .find(|&s| count.checked_mul(s) == Some(body))
        .map(Some)
}

fn read_record(r: &mut Reader<'_>, v2: bool) -> Result<OsgRecord, CodecError> {
    let t_ms = r.i32()?;
    let b4 = r.u8()?;
    let counts = JudgementCounts {
        n300: r.u16()?,
        n100: r.u16()?,
        n50: r.u16()?,
        geki: r.u16()?,
        katu: r.u16()?,
        miss: r.u16()?,
    };
    Ok(OsgRecord {
        t_ms,
        b4,
        counts,
        score: r.i32()?,
        max_combo: r.u16()?,
        combo: r.u16()?,
        b25: r.u8()?,
        hp_raw: r.u16()?,
        b28: r.u8()?,
        v2: if v2 { Some([r.f64()?, r.f64()?]) } else { None },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diag::DiagCode;
    use crate::error::CodecError;
    use crate::testkit::encode_osg;

    const V: i32 = 20_260_924;

    fn record(t_ms: i32, n300: u16, v2: Option<[f64; 2]>) -> OsgRecord {
        OsgRecord {
            t_ms,
            counts: JudgementCounts {
                n300,
                ..JudgementCounts::default()
            },
            score: i32::from(n300) * 300,
            max_combo: n300,
            combo: n300,
            hp_raw: 200,
            b4: 0,
            b25: 0,
            b28: u8::from(v2.is_some()),
            v2,
        }
    }

    fn file(records: Vec<OsgRecord>) -> OsgFile {
        let score_system = records.first().map(|r| {
            if r.v2.is_some() {
                OsgScoreSystem::V2
            } else {
                OsgScoreSystem::V1
            }
        });
        OsgFile {
            client_version: V,
            score_system,
            records,
        }
    }

    #[test]
    fn decodes_v1_record() {
        let mut bytes = Vec::new();
        bytes.extend(V.to_le_bytes());
        bytes.extend(1_i32.to_le_bytes());
        bytes.extend(1_234_i32.to_le_bytes());
        bytes.push(0);
        for c in [1u16, 2, 3, 4, 5, 6] {
            bytes.extend(c.to_le_bytes());
        }
        bytes.extend(98_765_i32.to_le_bytes());
        bytes.extend(40_u16.to_le_bytes());
        bytes.extend(12_u16.to_le_bytes());
        bytes.push(0);
        bytes.extend(187_u16.to_le_bytes());
        bytes.push(0);
        assert_eq!(bytes.len(), 8 + STRIDE_V1);
        let (osg, diags) = decode_osg(&bytes).unwrap();
        assert!(diags.is_empty(), "{diags:?}");
        assert_eq!(osg.client_version, V);
        assert_eq!(osg.score_system, Some(OsgScoreSystem::V1));
        let r = &osg.records[0];
        assert_eq!(r.t_ms, 1_234);
        assert_eq!(
            r.counts,
            JudgementCounts {
                n300: 1,
                n100: 2,
                n50: 3,
                geki: 4,
                katu: 5,
                miss: 6
            }
        );
        assert_eq!(
            (r.score, r.max_combo, r.combo, r.hp_raw),
            (98_765, 40, 12, 187)
        );
        assert_eq!(r.v2, None);
    }

    #[test]
    fn decodes_v2_record() {
        let osg = file(vec![
            record(10, 1, Some([150.0, 0.0])),
            record(20, 2, Some([300.0, 0.0])),
        ]);
        let bytes = encode_osg(&osg);
        assert_eq!(bytes.len(), 8 + 2 * STRIDE_V2);
        let (decoded, diags) = decode_osg(&bytes).unwrap();
        assert!(diags.is_empty(), "{diags:?}");
        assert_eq!(decoded, osg);
        assert_eq!(decoded.score_system, Some(OsgScoreSystem::V2));
        assert_eq!(decoded.records[1].v2, Some([300.0, 0.0]));
    }

    #[test]
    fn keeps_reserved_bytes_raw() {
        let mut r = record(5, 1, None);
        r.b4 = 0xaa;
        r.b25 = 1;
        r.b28 = 0x7f;
        let (decoded, _) = decode_osg(&encode_osg(&file(vec![r.clone()]))).unwrap();
        assert_eq!(decoded.records[0], r);
    }

    #[test]
    fn empty_graph_warns() {
        let bytes = encode_osg(&file(vec![]));
        assert_eq!(bytes.len(), 8);
        let (osg, diags) = decode_osg(&bytes).unwrap();
        assert!(osg.records.is_empty());
        assert_eq!(osg.score_system, None);
        assert!(diags.contains(DiagCode::OsgEmptyGraph));
    }

    #[test]
    fn rejects_truncated_header() {
        assert_eq!(
            decode_osg(&[1, 2, 3, 4, 5]),
            Err(CodecError::Truncated {
                kind: FileKind::Osg,
                offset: 4,
                needed: 4
            })
        );
    }

    #[test]
    fn rejects_negative_count() {
        let mut bytes = V.to_le_bytes().to_vec();
        bytes.extend((-1_i32).to_le_bytes());
        assert_eq!(
            decode_osg(&bytes),
            Err(CodecError::InvalidCount {
                kind: FileKind::Osg,
                offset: 4,
                count: -1
            })
        );
    }

    #[test]
    fn rejects_stride_mismatch() {
        let mut bytes = encode_osg(&file(vec![record(1, 1, None), record(2, 2, None)]));
        bytes.pop();
        assert_eq!(
            decode_osg(&bytes),
            Err(CodecError::StrideMismatch {
                len: bytes.len() as u64,
                count: 2
            })
        );
        let mut empty_with_body = encode_osg(&file(vec![]));
        empty_with_body.extend([0; STRIDE_V1]);
        assert!(matches!(
            decode_osg(&empty_with_body),
            Err(CodecError::StrideMismatch { count: 0, .. })
        ));
        // 37 bytes per record divides evenly but is neither layout.
        let mut odd = V.to_le_bytes().to_vec();
        odd.extend(1_i32.to_le_bytes());
        odd.extend([0; 37]);
        assert!(matches!(
            decode_osg(&odd),
            Err(CodecError::StrideMismatch { count: 1, .. })
        ));
    }
}
