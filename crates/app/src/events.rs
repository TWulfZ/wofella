//! Process-wide events the shells bridge to the UI (architecture §8, spec 003). The desktop
//! wraps each payload for tauri-specta; the CLI reads them to print progress.

use serde::Serialize;

use crate::jobs::dto::{JobId, JobKindDto, JobStageDto, JobStatusDto};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct JobProgressDto {
    pub job_id: JobId,
    pub kind: JobKindDto,
    pub stage: JobStageDto,
    pub done: u32,
    pub total: u32,
    pub eta_ms: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct JobFinishedDto {
    pub job_id: JobId,
    pub status: JobStatusDto,
    pub failed_items: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct DataChangedDto {
    /// Query-key roots the UI invalidates (`plays`, `players`, `setup`, `jobs`, `library`).
    pub domains: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppEvent {
    JobProgress(JobProgressDto),
    JobFinished(JobFinishedDto),
    DataChanged(DataChangedDto),
}

impl AppEvent {
    pub fn data_changed(domains: &[&str]) -> Self {
        Self::DataChanged(DataChangedDto {
            domains: domains.iter().map(|d| (*d).to_owned()).collect(),
        })
    }
}
