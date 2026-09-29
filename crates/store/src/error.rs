use std::path::PathBuf;

use thiserror::Error;
use wolluf_core::{BlobSha256, ErrorCode};

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("sqlite: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("migration: {0}")]
    Migration(#[from] rusqlite_migration::Error),
    #[error("user.db schema v{found} is newer than this build (latest v{latest})")]
    SchemaTooNew { found: u32, latest: u32 },
    #[error("io on {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("another wolluf instance holds {0}")]
    InstanceLocked(PathBuf),
    #[error("vault blob {0} does not match its content address")]
    VaultCorrupt(BlobSha256),
    #[error("vault blob {0} is missing")]
    BlobMissing(BlobSha256),
    #[error("the database writer thread is gone")]
    WriterGone,
    #[error("the database is closed")]
    Closed,
    #[error("a write job panicked: {0}")]
    WriterPanicked(String),
    #[error("stored value is invalid: {0}")]
    InvalidData(String),
    #[error("{0} not found")]
    NotFound(String),
    #[error("conflict: {0}")]
    Conflict(String),
}

impl StoreError {
    pub(crate) fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }

    /// The §7 code the app surfaces, so every caller maps store failures the same way.
    pub fn code(&self) -> ErrorCode {
        match self {
            Self::SchemaTooNew { .. } => ErrorCode::UnsupportedFormat,
            Self::InstanceLocked(_) => ErrorCode::Conflict,
            Self::NotFound(_) => ErrorCode::NotFound,
            Self::Conflict(_) => ErrorCode::Conflict,
            Self::Sqlite(_)
            | Self::Migration(_)
            | Self::Io { .. }
            | Self::VaultCorrupt(_)
            | Self::BlobMissing(_)
            | Self::WriterGone
            | Self::Closed
            | Self::WriterPanicked(_)
            | Self::InvalidData(_) => ErrorCode::Internal,
        }
    }
}
