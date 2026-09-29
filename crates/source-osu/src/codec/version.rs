//! Format kinds and the ADR 0015 version policy. The header `Int` of every stable file is the
//! client build that last wrote it, not a format revision, so only documented layout thresholds
//! change how bytes are read.

use crate::diag::{DiagCode, Diagnostics};
use crate::error::CodecError;
use crate::stable::stable_str_enum;

stable_str_enum! {
    /// Append-only: 006 adds `osg`.
    pub enum FileKind {
        OsuDb => "osu_db",
        ScoresDb => "scores_db",
        CollectionDb => "collection_db",
        Osr => "osr",
        Cfg => "cfg",
    }
}

/// Byte-sized AR/CS/HP/OD and an extra short exist below this build; no current client writes them.
pub const OSU_DB_MIN: i32 = 20_140_609;
/// The per-entry size int exists only below this build.
pub const OSU_DB_ENTRY_SIZE_REMOVED: i32 = 20_191_106;
/// Star-rating pairs switch from Int-Double to Int-Float at this build.
pub const OSU_DB_INT_FLOAT_PAIRS: i32 = 20_250_107;
pub const OSU_DB_NEWEST_VERIFIED: i32 = 20_260_924;
pub const SCORES_DB_MIN: i32 = 20_140_609;
pub const SCORES_DB_NEWEST_VERIFIED: i32 = 20_260_924;
pub const COLLECTION_DB_MIN: i32 = 20_140_609;
pub const COLLECTION_DB_NEWEST_VERIFIED: i32 = 20_260_624;
/// `.osr` has no layout floor: the only older difference is the missing online id, which
/// [`online_id_width`] handles, so every non-negative build is readable.
pub const OSR_MIN: i32 = 0;
pub const OSR_NEWEST_VERIFIED: i32 = 20_260_924;
/// lazer `LegacyScoreDecoder` (fetched 2026-09-28).
pub const ONLINE_ID_I32_FROM: i32 = 20_121_008;
pub const ONLINE_ID_I64_FROM: i32 = 20_140_721;
/// lazer writes legacy score/replay versions from this value on.
pub const LAZER_FROM: i32 = 30_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VersionPolicy {
    pub min: i32,
    pub newest_verified: i32,
}

impl FileKind {
    /// `None` for formats without a version header (cfg).
    pub const fn policy(self) -> Option<VersionPolicy> {
        let (min, newest_verified) = match self {
            Self::OsuDb => (OSU_DB_MIN, OSU_DB_NEWEST_VERIFIED),
            Self::ScoresDb => (SCORES_DB_MIN, SCORES_DB_NEWEST_VERIFIED),
            Self::CollectionDb => (COLLECTION_DB_MIN, COLLECTION_DB_NEWEST_VERIFIED),
            Self::Osr => (OSR_MIN, OSR_NEWEST_VERIFIED),
            Self::Cfg => return None,
        };
        Some(VersionPolicy {
            min,
            newest_verified,
        })
    }

    const fn has_lazer_range(self) -> bool {
        matches!(self, Self::Osr | Self::ScoresDb)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VersionClass {
    TooOld,
    Verified,
    Unverified,
    Lazer,
}

pub const fn is_lazer(version: i32) -> bool {
    version >= LAZER_FROM
}

pub const fn classify(kind: FileKind, version: i32) -> VersionClass {
    let Some(policy) = kind.policy() else {
        return VersionClass::Verified;
    };
    if kind.has_lazer_range() && is_lazer(version) {
        VersionClass::Lazer
    } else if version < policy.min {
        VersionClass::TooOld
    } else if version <= policy.newest_verified {
        VersionClass::Verified
    } else {
        VersionClass::Unverified
    }
}

/// Rejects `TooOld` and `Lazer` at once; the caller then decodes and passes the result through
/// [`finish`].
pub fn admit(kind: FileKind, version: i32) -> Result<VersionClass, CodecError> {
    match classify(kind, version) {
        VersionClass::TooOld | VersionClass::Lazer => {
            Err(CodecError::UnsupportedFormat { kind, version })
        }
        class => Ok(class),
    }
}

/// ADR 0015: on an unverified build any structural failure most likely means a layout change,
/// so it is reported as `UnsupportedFormat` rather than `PARSE_FAILED`.
pub fn finish<T>(
    kind: FileKind,
    version: i32,
    class: VersionClass,
    result: Result<(T, Diagnostics), CodecError>,
) -> Result<(T, Diagnostics), CodecError> {
    match (class, result) {
        (VersionClass::Unverified, Ok((value, mut diags))) => {
            diags.general(
                DiagCode::FormatUnverifiedVersion,
                format!("{kind} version {version}"),
            );
            Ok((value, diags))
        }
        (VersionClass::Unverified, Err(_)) => Err(CodecError::UnsupportedFormat { kind, version }),
        (_, result) => result,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnlineIdWidth {
    Absent,
    I32,
    I64,
}

pub const fn online_id_width(version: i32) -> OnlineIdWidth {
    if version >= ONLINE_ID_I64_FROM {
        OnlineIdWidth::I64
    } else if version >= ONLINE_ID_I32_FROM {
        OnlineIdWidth::I32
    } else {
        OnlineIdWidth::Absent
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn below_min_is_too_old() {
        assert_eq!(classify(FileKind::OsuDb, 20_140_608), VersionClass::TooOld);
        assert_eq!(
            classify(FileKind::ScoresDb, 20_140_608),
            VersionClass::TooOld
        );
        assert_eq!(classify(FileKind::CollectionDb, 0), VersionClass::TooOld);
        assert_eq!(classify(FileKind::Osr, -1), VersionClass::TooOld);
        assert!(matches!(
            admit(FileKind::OsuDb, 20_140_608),
            Err(CodecError::UnsupportedFormat {
                kind: FileKind::OsuDb,
                version: 20_140_608
            })
        ));
    }

    #[test]
    fn verified_range() {
        for (kind, newest) in [
            (FileKind::OsuDb, 20_260_924),
            (FileKind::ScoresDb, 20_260_924),
            (FileKind::CollectionDb, 20_260_624),
        ] {
            assert_eq!(classify(kind, 20_140_609), VersionClass::Verified);
            assert_eq!(classify(kind, newest), VersionClass::Verified);
            assert_eq!(classify(kind, newest + 1), VersionClass::Unverified);
        }
        assert_eq!(classify(FileKind::Osr, 20_121_007), VersionClass::Verified);
        assert_eq!(
            admit(FileKind::OsuDb, 20_260_924),
            Ok(VersionClass::Verified)
        );
    }

    #[test]
    fn newer_is_unverified() {
        assert_eq!(
            classify(FileKind::OsuDb, 20_270_101),
            VersionClass::Unverified
        );
        assert_eq!(
            classify(FileKind::CollectionDb, 20_260_924),
            VersionClass::Unverified
        );
        assert_eq!(
            admit(FileKind::ScoresDb, 29_999_999),
            Ok(VersionClass::Unverified)
        );
        // Unverified files are admitted, but a structural failure escalates to UNSUPPORTED_FORMAT
        // and success carries the warning.
        let err = CodecError::TrailingBytes {
            kind: FileKind::OsuDb,
            offset: 9,
            remaining: 1,
        };
        let escalated = finish::<()>(
            FileKind::OsuDb,
            20_270_101,
            VersionClass::Unverified,
            Err(err.clone()),
        );
        assert_eq!(
            escalated,
            Err(CodecError::UnsupportedFormat {
                kind: FileKind::OsuDb,
                version: 20_270_101
            })
        );
        let kept = finish::<()>(
            FileKind::OsuDb,
            20_260_924,
            VersionClass::Verified,
            Err(err.clone()),
        );
        assert_eq!(kept, Err(err));
        let (_, diags) = finish(
            FileKind::OsuDb,
            20_270_101,
            VersionClass::Unverified,
            Ok(((), Diagnostics::new())),
        )
        .unwrap();
        assert!(diags.contains(DiagCode::FormatUnverifiedVersion));
    }

    #[test]
    fn lazer_range_rejected() {
        assert_eq!(classify(FileKind::Osr, 30_000_000), VersionClass::Lazer);
        assert_eq!(
            classify(FileKind::ScoresDb, 30_000_001),
            VersionClass::Lazer
        );
        assert!(is_lazer(30_000_000));
        assert!(!is_lazer(29_999_999));
        assert!(matches!(
            admit(FileKind::Osr, 30_000_001),
            Err(CodecError::UnsupportedFormat {
                kind: FileKind::Osr,
                ..
            })
        ));
    }

    #[test]
    fn online_id_width_thresholds() {
        assert_eq!(online_id_width(20_121_007), OnlineIdWidth::Absent);
        assert_eq!(online_id_width(20_121_008), OnlineIdWidth::I32);
        assert_eq!(online_id_width(20_140_720), OnlineIdWidth::I32);
        assert_eq!(online_id_width(20_140_721), OnlineIdWidth::I64);
    }
}
