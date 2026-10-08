//! Meta DTOs (D13): camelCase on the wire, no 64-bit integers (spec 005).

use serde::{Deserialize, Serialize};

/// One keymode the engine indexes (ADR 0023).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct KeymodeDto {
    pub keymode: u8,
    /// Whether the keymode has a pattern taxonomy, so segments, labelling and session labels
    /// exist for it.
    pub has_patterns: bool,
    /// Calculator ids, e.g. `minacalc`.
    pub calculators: Vec<String>,
    /// A layout preset id.
    pub default_layout: String,
    /// Whether the default layout has a thumb column; thumb-side choices only make sense then.
    pub has_thumb: bool,
}
