//! The rule registry. Rules live in `rules/<id>.rs`, one file each (architecture §9.2).

use crate::rule::PatternRule;

/// Fixed order: overlap resolution falls back to it after priority and strength, so appending
/// is safe and reordering changes outputs.
pub fn all() -> &'static [&'static dyn PatternRule] {
    &[]
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    #[test]
    fn rule_ids_are_unique() {
        let ids: BTreeSet<String> = all().iter().map(|r| r.id().to_string()).collect();
        assert_eq!(ids.len(), all().len());
    }
}
