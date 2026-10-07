//! Settings DTOs (D13): camelCase on the wire, no 64-bit integers (spec 005).

use serde::{Deserialize, Serialize};

use crate::features::library::dto::ColumnDto;

/// A column layout preset: which hand and finger press each column.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct HandLayoutDto {
    pub id: String,
    /// One per column, leftmost first.
    pub columns: Vec<ColumnDto>,
}
