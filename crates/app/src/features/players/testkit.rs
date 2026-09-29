//! Synthetic pilot-shaped identity fixture (spec 004 AC3). Third-party names are anonymized
//! (docs/conventions.md, fixtures); the R2 play counts are kept so ordering is realistic.

use std::collections::BTreeMap;

use wolluf_core::AliasId;

use super::selection::{AliasFacts, SelectionInputs};

pub(crate) const PILOT_CFG_USERNAME: &str = "TWulfZasdasdasd d jSS||";

/// (raw name, plays) in R2 order; alias ids are 1-based positions in this table.
pub(crate) const PILOT_ALIASES: [(&str, u32); 10] = [
    ("TWulfZ", 3_019),
    ("", 1_395),
    ("W", 344),
    ("Rosalind", 68),
    ("s", 62),
    ("w", 33),
    ("Kovacs", 30),
    ("TWulfZasdasdasd d jSS||", 27),
    ("Wulf", 10),
    ("Sterling", 1),
];

pub(crate) fn pilot_alias_id(raw_name: &str) -> AliasId {
    let pos = PILOT_ALIASES
        .iter()
        .position(|(name, _)| *name == raw_name)
        .unwrap_or_else(|| panic!("{raw_name:?} is not a pilot alias"));
    AliasId(i64::try_from(pos).unwrap() + 1)
}

pub(crate) fn pilot_like_fixture() -> SelectionInputs {
    SelectionInputs {
        aliases: PILOT_ALIASES
            .iter()
            .map(|(name, n_plays)| AliasFacts {
                alias_id: pilot_alias_id(name),
                raw_name: name.as_bytes().to_vec(),
                n_plays: *n_plays,
            })
            .collect(),
        cfg_username: Some(PILOT_CFG_USERNAME.to_owned()),
        linked_username: None,
        decisions: BTreeMap::new(),
    }
}
