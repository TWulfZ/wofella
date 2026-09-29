//! Process-wide events the shells bridge to the UI (architecture §8, spec 003). The desktop
//! wraps each payload for tauri-specta; the CLI reads them to print progress.

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct DataChangedDto {
    /// Query-key roots the UI invalidates (`plays`, `players`, `setup`, `jobs`).
    pub domains: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AppEvent {
    DataChanged(DataChangedDto),
}

impl AppEvent {
    pub fn data_changed(domains: &[&str]) -> Self {
        Self::DataChanged(DataChangedDto {
            domains: domains.iter().map(|d| (*d).to_owned()).collect(),
        })
    }
}
