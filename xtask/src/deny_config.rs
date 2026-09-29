//! Guards the licence gate in the root `deny.toml` (D16, ADR 0008): the allowlist is exactly the
//! spec 001 AC14 set, so widening it needs a deliberate test change in review.

mod tests {
    use std::collections::BTreeSet;

    const DENY_TOML: &str = include_str!("../../deny.toml");

    const EXPECTED: [&str; 10] = [
        "MIT",
        "Apache-2.0",
        "Apache-2.0 WITH LLVM-exception",
        "BSD-2-Clause",
        "BSD-3-Clause",
        "MPL-2.0",
        "Zlib",
        "ISC",
        "Unicode-3.0",
        "CC0-1.0",
    ];

    const COPYLEFT_PREFIXES: [&str; 3] = ["GPL", "LGPL", "AGPL"];

    fn deny() -> toml::Table {
        toml::from_str(DENY_TOML).unwrap()
    }

    #[test]
    fn allowlist_is_exact() {
        let deny = deny();
        let allow: Vec<&str> = deny["licenses"]["allow"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        let unique: BTreeSet<&str> = allow.iter().copied().collect();
        assert_eq!(unique.len(), allow.len(), "duplicate entries: {allow:?}");
        assert_eq!(unique, BTreeSet::from(EXPECTED));
        for id in &allow {
            let upper = id.to_ascii_uppercase();
            assert!(
                !COPYLEFT_PREFIXES.iter().any(|p| upper.contains(p)),
                "copyleft licence in allowlist: {id}"
            );
        }
        let exceptions = deny["licenses"]
            .get("exceptions")
            .and_then(toml::Value::as_array)
            .map_or(0, Vec::len);
        assert_eq!(exceptions, 0, "per-crate licence exceptions need an ADR");
    }

    #[test]
    fn gate_settings_match_spec() {
        let deny = deny();
        assert_eq!(deny["licenses"]["private"]["ignore"].as_bool(), Some(true));
        assert_eq!(deny["bans"]["multiple-versions"].as_str(), Some("warn"));
        assert_eq!(deny["sources"]["unknown-registry"].as_str(), Some("deny"));
        assert_eq!(deny["sources"]["unknown-git"].as_str(), Some("deny"));
    }
}
