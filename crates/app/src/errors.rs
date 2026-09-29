//! Error contract shared by every feature and both shells (architecture §7, spec 005).

use std::borrow::Cow;
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use wolluf_core::ErrorCode;
use wolluf_source_osu::SourceError;
use wolluf_store::StoreError;

/// Dotted i18n keys owned by the app slices; `error.json` in the UI carries each of them.
pub mod keys {
    pub const INSTANCE_RUNNING: &str = "error.instance_running";
    pub const DATA_DIR_INSIDE_OSU: &str = "error.data_dir_inside_osu";
    pub const LAZER_NOT_SUPPORTED: &str = "setup.error.lazer_not_supported";
}

/// Spec 004 IPC: the players slice's `players.error.*` keys (`players.json` in the UI).
pub mod players_keys {
    pub const UNKNOWN_ALIAS: &str = "players.error.unknown_alias";
    pub const UNKNOWN_PROFILE: &str = "players.error.unknown_profile";
    pub const DUPLICATE_ALIAS: &str = "players.error.duplicate_alias";
    pub const INVALID_LABEL: &str = "players.error.invalid_label";
    pub const EMPTY_ALIASES: &str = "players.error.empty_aliases";
    pub const SELF_OVERLAP: &str = "players.error.self_overlap";
    pub const INVALID_KEYMODE: &str = "players.error.invalid_keymode";
}

/// Rust never builds user-facing prose: the UI localises `message_key` with `args` (§7).
/// `args` holds strings only, so no number can exceed 2^53 on the wire, and a `BTreeMap` keeps
/// the wire order deterministic.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{code}: {message_key}")]
pub struct AppError {
    pub code: ErrorCode,
    pub message_key: Cow<'static, str>,
    pub args: BTreeMap<String, String>,
    pub details: Option<String>,
    pub retryable: bool,
}

impl AppError {
    /// Uses the generic `error.code.<CODE>` key, which the UI has for every code; slices
    /// override it with their own dotted key through [`AppError::with_key`].
    pub fn new(code: ErrorCode) -> Self {
        Self {
            code,
            message_key: Cow::Owned(format!("error.code.{}", code.as_str())),
            args: BTreeMap::new(),
            details: None,
            retryable: default_retryable(code),
        }
    }

    pub fn with_key(mut self, key: impl Into<Cow<'static, str>>) -> Self {
        self.message_key = key.into();
        self
    }

    pub fn with_arg(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.args.insert(key.into(), value.into());
        self
    }

    pub fn with_details(mut self, details: impl Into<String>) -> Self {
        self.details = Some(details.into());
        self
    }

    pub fn with_retryable(mut self, retryable: bool) -> Self {
        self.retryable = retryable;
        self
    }

    pub fn osu_dir_not_found(path: impl Into<String>) -> Self {
        Self::new(ErrorCode::OsuDirNotFound).with_arg("path", path)
    }

    pub fn unsupported_format() -> Self {
        Self::new(ErrorCode::UnsupportedFormat)
    }

    pub fn parse_failed() -> Self {
        Self::new(ErrorCode::ParseFailed)
    }

    pub fn osu_running() -> Self {
        Self::new(ErrorCode::OsuRunning)
    }

    pub fn consent_required() -> Self {
        Self::new(ErrorCode::ConsentRequired)
    }

    pub fn signature_invalid() -> Self {
        Self::new(ErrorCode::SignatureInvalid)
    }

    pub fn not_found() -> Self {
        Self::new(ErrorCode::NotFound)
    }

    pub fn invalid_input() -> Self {
        Self::new(ErrorCode::InvalidInput)
    }

    pub fn conflict() -> Self {
        Self::new(ErrorCode::Conflict)
    }

    pub fn cancelled() -> Self {
        Self::new(ErrorCode::Cancelled)
    }

    pub fn internal(details: impl Into<String>) -> Self {
        Self::new(ErrorCode::Internal).with_details(details)
    }
}

/// `details` keeps the store's own text for logs and debug builds; release IPC strips it.
impl From<StoreError> for AppError {
    fn from(error: StoreError) -> Self {
        let base = Self::new(error.code()).with_details(error.to_string());
        match error {
            StoreError::InstanceLocked(_) => base.with_key(keys::INSTANCE_RUNNING),
            _ => base,
        }
    }
}

/// The code comes from `SourceError::code()` (ADR 0015 mapping); paths travel as `args.path` so
/// the UI can name the folder without Rust building prose.
impl From<SourceError> for AppError {
    fn from(error: SourceError) -> Self {
        let details = error.to_string();
        let base = match &error {
            SourceError::InvalidInstall { path, .. } => {
                Self::osu_dir_not_found(path.to_string_lossy())
            }
            SourceError::LazerInstall { path } => Self::new(error.code())
                .with_key(keys::LAZER_NOT_SUPPORTED)
                .with_arg("path", path.to_string_lossy()),
            _ => Self::new(error.code()),
        };
        base.with_details(details)
    }
}

/// Only a running osu! clears up without the user changing anything: the snapshot retries
/// once the game stops writing its DBs (ADR 0014).
fn default_retryable(code: ErrorCode) -> bool {
    matches!(code, ErrorCode::OsuRunning)
}

/// Wire mirror of core's `ErrorCode`: domain types never derive specta (D13).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCodeDto {
    OsuDirNotFound,
    UnsupportedFormat,
    ParseFailed,
    OsuRunning,
    ConsentRequired,
    SignatureInvalid,
    NotFound,
    InvalidInput,
    Conflict,
    Cancelled,
    Internal,
}

/// Exhaustive on purpose: a new core code must not compile until it is mirrored here.
impl From<ErrorCode> for ErrorCodeDto {
    fn from(code: ErrorCode) -> Self {
        match code {
            ErrorCode::OsuDirNotFound => Self::OsuDirNotFound,
            ErrorCode::UnsupportedFormat => Self::UnsupportedFormat,
            ErrorCode::ParseFailed => Self::ParseFailed,
            ErrorCode::OsuRunning => Self::OsuRunning,
            ErrorCode::ConsentRequired => Self::ConsentRequired,
            ErrorCode::SignatureInvalid => Self::SignatureInvalid,
            ErrorCode::NotFound => Self::NotFound,
            ErrorCode::InvalidInput => Self::InvalidInput,
            ErrorCode::Conflict => Self::Conflict,
            ErrorCode::Cancelled => Self::Cancelled,
            ErrorCode::Internal => Self::Internal,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct IpcError {
    pub code: ErrorCodeDto,
    pub message_key: String,
    pub args: BTreeMap<String, String>,
    pub details: Option<String>,
    pub retryable: bool,
}

impl IpcError {
    pub fn from_app(error: AppError, include_details: bool) -> Self {
        Self {
            code: error.code.into(),
            message_key: error.message_key.into_owned(),
            args: error.args,
            details: error.details.filter(|_| include_details),
            retryable: error.retryable,
        }
    }
}

/// Release builds never send `details`: it can hold local paths (§7 scrubbing). Shells log
/// the full `AppError` before converting.
impl From<AppError> for IpcError {
    fn from(error: AppError) -> Self {
        Self::from_app(error, cfg!(debug_assertions))
    }
}

#[cfg(test)]
mod tests {
    use wolluf_core::ErrorCode;

    use super::*;

    #[test]
    fn error_code_dto_mirrors_core_all() {
        for code in ErrorCode::ALL {
            let json = serde_json::to_string(&ErrorCodeDto::from(*code)).unwrap();
            assert_eq!(json, format!("\"{}\"", code.as_str()));
        }
    }

    #[test]
    fn ipc_error_json_per_code() {
        let errors: Vec<IpcError> = ErrorCode::ALL
            .iter()
            .map(|code| IpcError::from_app(AppError::new(*code), false))
            .collect();
        let json = serde_json::to_string_pretty(&errors).unwrap();
        insta::assert_snapshot!("ipc_error_json_per_code", json);
    }

    #[test]
    fn details_stripped_without_flag() {
        let err = AppError::internal("disk full at /home/someone/data")
            .with_key("error.instance_running")
            .with_arg("path", "/home/someone/data");
        let stripped = IpcError::from_app(err.clone(), false);
        assert_eq!(stripped.details, None);
        assert_eq!(stripped.message_key, "error.instance_running");
        assert_eq!(
            stripped.args.get("path").map(String::as_str),
            Some("/home/someone/data")
        );
        let kept = IpcError::from_app(err, true);
        assert_eq!(
            kept.details.as_deref(),
            Some("disk full at /home/someone/data")
        );
    }

    #[test]
    fn args_serialize_sorted() {
        let err = AppError::new(ErrorCode::InvalidInput)
            .with_arg("zeta", "1")
            .with_arg("alpha", "2")
            .with_arg("mid", "3");
        let json = serde_json::to_string(&IpcError::from_app(err, false)).unwrap();
        assert!(
            json.contains(r#""args":{"alpha":"2","mid":"3","zeta":"1"}"#),
            "{json}"
        );
        assert!(
            json.contains(r#""messageKey":"error.code.INVALID_INPUT""#),
            "{json}"
        );
    }

    #[test]
    fn store_errors_map_to_codes_and_keys() {
        let locked = AppError::from(StoreError::InstanceLocked("/d/wolluf.lock".into()));
        assert_eq!(locked.code, ErrorCode::Conflict);
        assert_eq!(locked.message_key, keys::INSTANCE_RUNNING);
        let newer = AppError::from(StoreError::SchemaTooNew {
            found: 9,
            latest: 1,
        });
        assert_eq!(newer.code, ErrorCode::UnsupportedFormat);
        assert_eq!(newer.message_key, "error.code.UNSUPPORTED_FORMAT");
        assert!(newer.details.is_some());
    }

    #[test]
    fn source_errors_map_to_codes_and_keys() {
        let running = AppError::from(SourceError::Changing {
            path: "/osu/scores.db".into(),
        });
        assert_eq!(running.code, ErrorCode::OsuRunning);
        assert!(running.retryable);
        let invalid = AppError::from(SourceError::InvalidInstall {
            path: "/x".into(),
            missing: vec!["osu!.db"],
        });
        assert_eq!(invalid.code, ErrorCode::OsuDirNotFound);
        assert_eq!(invalid.args.get("path").map(String::as_str), Some("/x"));
        let lazer = AppError::from(SourceError::LazerInstall { path: "/l".into() });
        assert_eq!(lazer.code, ErrorCode::UnsupportedFormat);
        assert_eq!(lazer.message_key, keys::LAZER_NOT_SUPPORTED);
    }

    #[test]
    fn display_is_code_and_key() {
        let err = AppError::osu_dir_not_found("/mnt/x");
        assert_eq!(
            err.to_string(),
            "OSU_DIR_NOT_FOUND: error.code.OSU_DIR_NOT_FOUND"
        );
        assert_eq!(err.args.get("path").map(String::as_str), Some("/mnt/x"));
        assert!(AppError::new(ErrorCode::OsuRunning).retryable);
        assert!(!AppError::new(ErrorCode::ParseFailed).retryable);
    }
}
