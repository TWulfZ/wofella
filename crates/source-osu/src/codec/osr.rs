//! `.osr` replay header and payload span (`research/scripts/rejudge/osr_wiki.md`). The LZMA
//! payload is not decoded here; frame parsing is F2's and must follow `rejudge.py::read_osr`.

use std::ops::Range;

use wolluf_core::DotNetTicks;

use crate::codec::replay_name::ReplayFileName;
use crate::codec::score_header::{OnlineId, ScoreHeader, read_online_id, read_score_header};
use crate::codec::version::{VersionClass, admit, finish};
use crate::codec::{FileKind, Reader};
use crate::diag::{DiagCode, Diagnostics};
use crate::error::CodecError;

const KIND: FileKind = FileKind::Osr;

#[derive(Debug, Clone, PartialEq)]
pub struct OsrFile {
    pub header: ScoreHeader,
    /// Byte span of the LZMA_ALONE stream inside the decoded buffer; `None` when the declared
    /// length is ≤ 0 (3 pilot files of 126 bytes).
    pub payload: Option<Range<usize>>,
    pub online_id: OnlineId,
    pub target_practice: Option<f64>,
}

pub fn decode_osr(bytes: &[u8]) -> Result<(OsrFile, Diagnostics), CodecError> {
    let mut r = Reader::new(bytes, KIND);
    let header = read_score_header(&mut r)?;
    let version = header.version;
    let class = admit(KIND, version)?;
    let body = decode_tail(&mut r, header, class);
    finish(KIND, version, class, body)
}

fn decode_tail(
    r: &mut Reader<'_>,
    header: ScoreHeader,
    class: VersionClass,
) -> Result<(OsrFile, Diagnostics), CodecError> {
    let mut diags = Diagnostics::new();
    let len_at = r.offset();
    let len = r.i32()?;
    let payload = match usize::try_from(len) {
        Ok(n) if n > 0 => {
            let start = r.position();
            r.bytes(n)?;
            Some(start..start + n)
        }
        _ => {
            diags.at_offset(
                DiagCode::OsrEmptyPayload,
                len_at,
                format!("declared payload length {len}"),
            );
            None
        }
    };
    let online_id = read_online_id(r, header.version)?;
    let target_practice = if header.has_target_practice() {
        Some(r.f64()?)
    } else {
        None
    };
    if r.remaining() > 0 {
        // ADR 0015: an unverified build must end exactly where the known layout ends.
        if class == VersionClass::Unverified {
            r.expect_eof()?;
        }
        diags.at_offset(
            DiagCode::OsrTrailingBytes,
            r.offset(),
            format!("{} trailing bytes", r.remaining()),
        );
    }
    let osr = OsrFile {
        header,
        payload,
        online_id,
        target_practice,
    };
    Ok((osr, diags))
}

/// Cross-checks a `Data/r` name against the header it names (research 03 l.168: 5,030/5,030
/// consistent on the pilot).
pub fn check_name_consistency(name: &ReplayFileName, header: &ScoreHeader) -> Diagnostics {
    let mut diags = Diagnostics::new();
    let name_md5 = name.md5.to_string();
    if header.beatmap_md5.as_bytes() != Some(name_md5.as_bytes()) {
        diags.general(
            DiagCode::OsrNameMd5Mismatch,
            "header md5 differs from file name",
        );
    }
    if DotNetTicks(header.timestamp_ticks).to_filetime() != Some(name.filetime) {
        diags.general(
            DiagCode::OsrNameTimeMismatch,
            "header ticks differ from file name",
        );
    }
    diags
}

#[cfg(test)]
mod tests {
    use wolluf_core::{DotNetTicks, FileTime};

    use super::*;
    use crate::codec::OsuString;
    use crate::codec::replay_name::ReplayFileKind;
    use crate::codec::score_header::mods;
    use crate::diag::DiagCode;
    use crate::error::CodecError;
    use crate::testkit::{OsrBuilder, ScoreBuilder};

    const MD5: &str = "0123456789abcdef0123456789abcdef";
    const LZMA: [u8; 13] = [
        0x5d, 0, 0, 0x20, 0, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
    ];

    #[test]
    fn decodes_header_fields() {
        let score = ScoreBuilder::mania(MD5, "Rosalind", 3)
            .mods(mods::SCORE_V2)
            .online_id(4_242)
            .life_bar(OsuString::present(*b"0|1,500|0.9"));
        let expected = score.header().clone();
        let bytes = OsrBuilder::new(score).payload(LZMA.to_vec()).build();
        let (osr, diags) = decode_osr(&bytes).unwrap();
        assert!(diags.is_empty(), "{diags:?}");
        assert_eq!(osr.header, expected);
        assert_eq!(osr.online_id, OnlineId::I64(4_242));
        assert_eq!(osr.target_practice, None);
    }

    #[test]
    fn payload_span_points_at_lzma() {
        let bytes = OsrBuilder::new(ScoreBuilder::mania(MD5, "a", 1))
            .payload(LZMA.to_vec())
            .build();
        let (osr, _) = decode_osr(&bytes).unwrap();
        let span = osr.payload.clone().unwrap();
        assert_eq!(&bytes[span.clone()], &LZMA);
        // Only the 8-byte online id follows the payload, as in every pilot file.
        assert_eq!(bytes.len() - span.end, 8);
    }

    #[test]
    fn empty_payload_is_none_with_diagnostic() {
        let bytes = OsrBuilder::new(ScoreBuilder::mania(MD5, "a", 1).online_id(99))
            .empty_payload()
            .build();
        let (osr, diags) = decode_osr(&bytes).unwrap();
        assert_eq!(osr.payload, None);
        assert_eq!(osr.online_id.positive(), Some(99));
        assert_eq!(diags.codes(), vec![DiagCode::OsrEmptyPayload]);
    }

    #[test]
    fn payload_beyond_eof_truncated() {
        let bytes = OsrBuilder::new(ScoreBuilder::mania(MD5, "a", 1))
            .payload(LZMA.to_vec())
            .build();
        let cut = &bytes[..bytes.len() - 8 - 1];
        assert!(matches!(
            decode_osr(cut),
            Err(CodecError::Truncated {
                kind: FileKind::Osr,
                needed: 13,
                ..
            })
        ));
    }

    #[test]
    fn lazer_version_unsupported() {
        let bytes = OsrBuilder::new(ScoreBuilder::mania(MD5, "a", 1).version(30_000_001))
            .payload(LZMA.to_vec())
            .build();
        assert_eq!(
            decode_osr(&bytes).map(|_| ()),
            Err(CodecError::UnsupportedFormat {
                kind: FileKind::Osr,
                version: 30_000_001
            })
        );
    }

    #[test]
    fn trailing_bytes_diagnostic() {
        let bytes = OsrBuilder::new(ScoreBuilder::mania(MD5, "a", 1))
            .payload(LZMA.to_vec())
            .trailing(vec![1, 2, 3])
            .build();
        let (_, diags) = decode_osr(&bytes).unwrap();
        assert_eq!(diags.codes(), vec![DiagCode::OsrTrailingBytes]);
        // On an unverified build the same leftover is a structural failure (ADR 0015).
        let newer = OsrBuilder::new(ScoreBuilder::mania(MD5, "a", 1).version(20_270_101))
            .payload(LZMA.to_vec())
            .trailing(vec![1, 2, 3])
            .build();
        assert!(matches!(
            decode_osr(&newer),
            Err(CodecError::UnsupportedFormat { .. })
        ));
        let tp = OsrBuilder::new(
            ScoreBuilder::mania(MD5, "a", 1)
                .mods(mods::TARGET_PRACTICE)
                .target_practice(0.5),
        )
        .payload(LZMA.to_vec())
        .build();
        let (osr, diags) = decode_osr(&tp).unwrap();
        assert_eq!(osr.target_practice, Some(0.5));
        assert!(diags.is_empty());
    }

    #[test]
    fn name_consistency_diagnostics() {
        let score = ScoreBuilder::mania(MD5, "a", 1);
        let header = score.header().clone();
        let name = OsrBuilder::new(score).file_name().unwrap();
        assert_eq!(name.kind, ReplayFileKind::Osr);
        assert!(check_name_consistency(&name, &header).is_empty());
        let other_md5 = ReplayFileName {
            md5: "f".repeat(32).parse().unwrap(),
            ..name
        };
        assert_eq!(
            check_name_consistency(&other_md5, &header).codes(),
            vec![DiagCode::OsrNameMd5Mismatch]
        );
        let later = FileTime::new(name.filetime.get() + 1).unwrap();
        let other_time = ReplayFileName {
            filetime: later,
            ..name
        };
        assert_eq!(
            check_name_consistency(&other_time, &header).codes(),
            vec![DiagCode::OsrNameTimeMismatch]
        );
        let mut before_1601 = header.clone();
        before_1601.timestamp_ticks = DotNetTicks(0).0;
        assert_eq!(
            check_name_consistency(&name, &before_1601).codes(),
            vec![DiagCode::OsrNameTimeMismatch]
        );
    }
}

#[cfg(test)]
mod props {
    use proptest::prelude::*;

    use super::*;
    use crate::testkit::{OsrBuilder, ScoreBuilder, encode_osr};

    proptest! {
        #[test]
        fn arbitrary_bytes_never_panic(bytes in proptest::collection::vec(any::<u8>(), 0..256)) {
            let _ = decode_osr(&bytes);
        }

        #[test]
        fn decode_encode_bytes_identical(payload in proptest::collection::vec(any::<u8>(), 1..64), nth in 0..1000i64) {
            let bytes = OsrBuilder::new(ScoreBuilder::mania("0123456789abcdef0123456789abcdef", "p", nth))
                .payload(payload)
                .build();
            let (osr, _) = decode_osr(&bytes).unwrap();
            let span = osr.payload.clone().map(|r| &bytes[r]);
            prop_assert_eq!(encode_osr(&osr.header, span, osr.online_id, osr.target_practice), bytes);
        }
    }
}
