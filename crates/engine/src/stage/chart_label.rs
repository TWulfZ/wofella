//! `chart_label`: difficulty-name labels of a chart (`crate::labels`) followed by its weak name
//! hints (`crate::labels::hints`). The key is stage-level, like `chart_parse`'s: rows depend only
//! on the catalog strings and this code.

use wolluf_core::{StageId, VersionKey, VersionKeyBuilder};

use crate::error::EngineError;
use crate::labels::hints::hint_labels;
use crate::labels::{ChartLabel, LabelInput, extract_labels};

pub const STAGE: StageId = StageId::from_static("chart_label");
/// 2: name hints (`source = name_hint`).
pub const VERSION: u32 = 3;

/// Every `chart_label` row of one chart: the labels in `extract_labels` order, then the hints.
pub fn run(input: &LabelInput<'_>) -> Vec<ChartLabel> {
    let mut rows = extract_labels(input);
    let hints = hint_labels(input, &rows);
    rows.extend(hints);
    rows
}

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
        assert_eq!(VERSION, 3);
    }

    // Frozen: a change re-keys every stored label row.
    #[test]
    fn vkey_is_stage_level_and_frozen() {
        let key = vkey().unwrap();
        assert_eq!(key, vkey().unwrap());
        assert_eq!(
            key.to_string(),
            "e9d693f48df88f55576233ed0f6193c2d14a0c7415e6c0ef8ff5f2ecbfa8d741"
        );
        assert_ne!(key, crate::stage::chart_parse::vkey().unwrap());
    }

    #[test]
    fn run_appends_hints_after_the_labels() {
        let input = LabelInput {
            folder: "1877617 Various Artists - KomeijiDove 7K Jack Practice",
            version: "~ 9th ~ Minijack Song",
            creator: "KomeijiDove",
            set_id: Some(1877617),
        };
        let rows = run(&input);
        let ids: Vec<(&str, &str, &str)> = rows
            .iter()
            .map(|l| (l.source.as_str(), l.scale.as_str(), l.level_text.as_str()))
            .collect();
        assert_eq!(
            ids,
            [
                ("komeijidove_practice", "jinjin_dan", "9th"),
                ("name_hint", "hint_axis", "7k.regular.jack"),
                ("name_hint", "hint_pattern", "regular.jack.minijack"),
            ]
        );
        let plain = LabelInput {
            folder: "100 Artist - Song",
            version: "Hard",
            creator: "Mapper",
            set_id: None,
        };
        assert!(run(&plain).is_empty());
    }
}
