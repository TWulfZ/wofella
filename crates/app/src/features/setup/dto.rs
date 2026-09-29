//! Setup DTOs (spec 005 Design). camelCase on the wire, no 64-bit integers (the bindings export
//! fails on them), timestamps as RFC 3339 strings.

use serde::Serialize;
use wolluf_source_osu::install::CandidateSource;

use crate::jobs::dto::JobDto;

/// Mirrors 002's `CandidateSource`; the exhaustive `From` keeps them in step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum InstallSourceDto {
    Env,
    Registry,
    LocalAppData,
    WslUserProfile,
    DriveScan,
    ProgramFiles,
}

impl From<CandidateSource> for InstallSourceDto {
    fn from(source: CandidateSource) -> Self {
        match source {
            CandidateSource::Env => Self::Env,
            CandidateSource::Registry => Self::Registry,
            CandidateSource::LocalAppData => Self::LocalAppData,
            CandidateSource::WslUserProfile => Self::WslUserProfile,
            CandidateSource::DriveScan => Self::DriveScan,
            CandidateSource::ProgramFiles => Self::ProgramFiles,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct InstallCandidateDto {
    pub path: String,
    pub source: InstallSourceDto,
    pub valid: bool,
    pub osu_db_version: Option<i32>,
    /// File names the UI shows as missing (`osu!.db`, `osu!.exe`, `scores.db`, `directory`).
    pub missing: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct InstallDto {
    pub id: u32,
    pub root_path: String,
    pub osu_db_version: Option<i32>,
    pub detected_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SetupStatusDto {
    pub install: Option<InstallDto>,
    pub identity_ready: bool,
    pub data_dir: String,
    pub logs_dir: String,
    pub app_version: String,
    pub last_sync: Option<JobDto>,
}
