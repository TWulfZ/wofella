//! `chart_label`: difficulty-name labels of a chart (`crate::labels`). The key is stage-level,
//! like `chart_parse`'s: labels depend only on the catalog strings and this code.

use wolluf_core::{StageId, VersionKey, VersionKeyBuilder};

use crate::error::EngineError;

pub const STAGE: StageId = StageId::from_static("chart_label");
pub const VERSION: u32 = 1;

/// The stage-level key of `chart_label` rows.
pub fn vkey() -> Result<VersionKey, EngineError> {
    Ok(VersionKeyBuilder::new(STAGE, VERSION).finish()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stage_id_and_version_are_stable() {
        assert_eq!(STAGE.as_str(), "chart_label");
        assert_eq!(VERSION, 1);
    }

    // Frozen: a change re-keys every stored label row.
    #[test]
    fn vkey_is_stage_level_and_frozen() {
        let key = vkey().unwrap();
        assert_eq!(key, vkey().unwrap());
        assert_eq!(
            key.to_string(),
            "e6bc56df4e3e4ea7cbb3f6e386c1be62639087f87ad786a061a12c1249edb53f"
        );
        assert_ne!(key, crate::stage::chart_parse::vkey().unwrap());
    }
}
