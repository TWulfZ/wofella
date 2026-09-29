//! Codec and source errors (spec 002 Behaviour "Errors", ADR 0015 error mapping).

use std::io;
use std::path::PathBuf;

use thiserror::Error;
use wolluf_core::ErrorCode;

use crate::codec::FileKind;

/// Every decode failure of a pure codec. Offsets are byte positions in the decoded buffer.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CodecError {
    #[error("{kind}: format version {version} is not supported")]
    UnsupportedFormat { kind: FileKind, version: i32 },
    #[error("{kind}: truncated at byte {offset}, a read needed {needed} bytes")]
    Truncated {
        kind: FileKind,
        offset: u64,
        needed: u64,
    },
    #[error("{kind}: string tag {tag:#04x} at byte {offset}, expected 0x00 or 0x0b")]
    BadStringTag {
        kind: FileKind,
        offset: u64,
        tag: u8,
    },
    #[error("{kind}: tag {found:#04x} at byte {offset}, expected {expected:#04x}")]
    UnexpectedTag {
        kind: FileKind,
        offset: u64,
        expected: u8,
        found: u8,
    },
    #[error("{kind}: unexpected value for {field} at byte {offset}")]
    UnexpectedValue {
        kind: FileKind,
        offset: u64,
        field: &'static str,
    },
    #[error("{kind}: invalid element count {count} at byte {offset}")]
    InvalidCount {
        kind: FileKind,
        offset: u64,
        count: i64,
    },
    #[error("{kind}: ULEB128 at byte {offset} does not fit in 5 bytes / 32 bits")]
    Uleb128Overflow { kind: FileKind, offset: u64 },
    #[error("osu_db: entry at byte {offset} declares {declared} bytes but consumed {consumed}")]
    EntrySizeMismatch {
        offset: u64,
        declared: i32,
        consumed: u64,
    },
    #[error("{kind}: {remaining} trailing bytes after byte {offset}")]
    TrailingBytes {
        kind: FileKind,
        offset: u64,
        remaining: u64,
    },
}

impl CodecError {
    pub const fn code(&self) -> ErrorCode {
        match self {
            Self::UnsupportedFormat { .. } => ErrorCode::UnsupportedFormat,
            _ => ErrorCode::ParseFailed,
        }
    }

    /// A file osu! is still writing shows up as too short or too long; 003 retries once on
    /// these before reporting `PARSE_FAILED`.
    pub const fn is_possibly_torn_write(&self) -> bool {
        matches!(self, Self::Truncated { .. } | Self::TrailingBytes { .. })
    }
}

/// Errors of the IO helpers (install detection, cfg files, snapshots).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SourceError {
    #[error(transparent)]
    Codec(#[from] CodecError),
    #[error("io error {kind:?} on {}", .path.display())]
    Io { path: PathBuf, kind: io::ErrorKind },
    #[error("{} is not an osu! stable install, missing {missing:?}", .path.display())]
    InvalidInstall {
        path: PathBuf,
        missing: Vec<&'static str>,
    },
    #[error("{} is an osu!lazer install", .path.display())]
    LazerInstall { path: PathBuf },
}

impl SourceError {
    pub fn io(path: impl Into<PathBuf>, err: &io::Error) -> Self {
        Self::Io {
            path: path.into(),
            kind: err.kind(),
        }
    }

    /// A missing install root is reported as `InvalidInstall`, so `Io` here is never "the
    /// install is gone" and maps to `INTERNAL`.
    pub const fn code(&self) -> ErrorCode {
        match self {
            Self::Codec(e) => e.code(),
            Self::Io { .. } => ErrorCode::Internal,
            Self::InvalidInstall { .. } => ErrorCode::OsuDirNotFound,
            Self::LazerInstall { .. } => ErrorCode::UnsupportedFormat,
        }
    }

    pub const fn is_possibly_torn_write(&self) -> bool {
        match self {
            Self::Codec(e) => e.is_possibly_torn_write(),
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_follow_adr_0015_mapping() {
        let kind = FileKind::OsuDb;
        let unsupported = CodecError::UnsupportedFormat { kind, version: 1 };
        let truncated = CodecError::Truncated {
            kind,
            offset: 0,
            needed: 4,
        };
        assert_eq!(unsupported.code(), ErrorCode::UnsupportedFormat);
        assert_eq!(truncated.code(), ErrorCode::ParseFailed);
        assert!(truncated.is_possibly_torn_write());
        assert!(!unsupported.is_possibly_torn_write());
        let trailing = CodecError::TrailingBytes {
            kind,
            offset: 0,
            remaining: 1,
        };
        assert!(trailing.is_possibly_torn_write());
        assert!(SourceError::from(trailing).is_possibly_torn_write());
        assert_eq!(
            SourceError::LazerInstall {
                path: PathBuf::from("x")
            }
            .code(),
            ErrorCode::UnsupportedFormat
        );
        assert_eq!(
            SourceError::InvalidInstall {
                path: PathBuf::from("x"),
                missing: vec!["osu!.db"]
            }
            .code(),
            ErrorCode::OsuDirNotFound
        );
        assert_eq!(
            SourceError::Io {
                path: PathBuf::from("x"),
                kind: io::ErrorKind::PermissionDenied
            }
            .code(),
            ErrorCode::Internal
        );
    }
}
