//! Version keys for derived data (architecture §5.5, D15). The byte encoding is frozen by
//! ADR 0006.

use crate::digest::{FieldHasher, hex_bytes};
use crate::error::CoreError;
use crate::id::StageId;

const VKEY_TAG: &[u8] = b"wolluf.vkey.v1";

/// Stands in for "no config" / "no input" so both cases still hash a fixed-width field.
const ZERO_DIGEST: [u8; 32] = [0; 32];

hex_bytes!(
    /// `blake3(stage_id, VERSION, pack-section hashes, config hash, input fingerprint)`.
    VersionKey,
    32
);

#[derive(Debug, Clone)]
pub struct VersionKeyBuilder {
    stage: StageId,
    version: u32,
    sections: Vec<(String, [u8; 32])>,
    config: [u8; 32],
    input: [u8; 32],
}

impl VersionKeyBuilder {
    pub fn new(stage: StageId, version: u32) -> Self {
        Self {
            stage,
            version,
            sections: Vec::new(),
            config: ZERO_DIGEST,
            input: ZERO_DIGEST,
        }
    }

    pub fn section(mut self, name: impl Into<String>, hash: [u8; 32]) -> Self {
        self.sections.push((name.into(), hash));
        self
    }

    pub fn config(mut self, hash: [u8; 32]) -> Self {
        self.config = hash;
        self
    }

    pub fn input(mut self, fingerprint: [u8; 32]) -> Self {
        self.input = fingerprint;
        self
    }

    /// Sorts sections by name so the key does not depend on declaration order.
    pub fn finish(mut self) -> Result<VersionKey, CoreError> {
        self.sections.sort_by(|a, b| a.0.cmp(&b.0));
        if let Some(dup) = self.sections.windows(2).find(|w| w[0].0 == w[1].0) {
            return Err(CoreError::DuplicateSection(dup[0].0.clone()));
        }
        let section_count = u32::try_from(self.sections.len()).unwrap_or(u32::MAX);
        let mut hasher = FieldHasher::new(VKEY_TAG);
        hasher
            .bytes(self.stage.as_str().as_bytes())
            .u32(self.version)
            .u32(section_count);
        for (name, hash) in &self.sections {
            hasher.bytes(name.as_bytes()).bytes(hash);
        }
        hasher.bytes(&self.config).bytes(&self.input);
        Ok(VersionKey(hasher.finish()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Computed once from the ADR 0006 encoding with an independent Python blake3 script, then
    // frozen: a change here invalidates every stored derivation.
    const GOLDEN: &str = "55d34b8447311fb1e170684439726896dba668efed73439a7b5a31b5c494ae68";
    const GOLDEN_DEFAULTS: &str =
        "6dbba84dc58e014e90d86ff6c4d7e42c4c451ce8b4e588fa813279edc461cadb";

    const STAGE: StageId = StageId::from_static("players.alias_stats");

    fn builder() -> VersionKeyBuilder {
        VersionKeyBuilder::new(STAGE, 1)
            .section("identity", [1; 32])
            .section("difficulty", [2; 32])
            .config([3; 32])
            .input([4; 32])
    }

    #[test]
    fn golden_vector() {
        let key = builder().finish().unwrap();
        assert_eq!(key.to_string(), GOLDEN);
        assert_eq!(GOLDEN.parse::<VersionKey>().unwrap(), key);
        let defaults = VersionKeyBuilder::new(STAGE, 1).finish().unwrap();
        assert_eq!(defaults.to_string(), GOLDEN_DEFAULTS);
    }

    #[test]
    fn section_order_is_irrelevant() {
        let reordered = VersionKeyBuilder::new(STAGE, 1)
            .input([4; 32])
            .section("difficulty", [2; 32])
            .config([3; 32])
            .section("identity", [1; 32]);
        assert_eq!(reordered.finish().unwrap(), builder().finish().unwrap());
    }

    #[test]
    fn duplicate_section_rejected() {
        let dup = builder().section("identity", [9; 32]).finish();
        assert_eq!(dup, Err(CoreError::DuplicateSection("identity".to_owned())));
    }

    #[test]
    fn any_field_change_changes_key() {
        let base = builder().finish().unwrap();
        let variants = [
            VersionKeyBuilder::new(StageId::from_static("players.alias_statz"), 1)
                .section("identity", [1; 32])
                .section("difficulty", [2; 32])
                .config([3; 32])
                .input([4; 32]),
            VersionKeyBuilder::new(STAGE, 2)
                .section("identity", [1; 32])
                .section("difficulty", [2; 32])
                .config([3; 32])
                .input([4; 32]),
            VersionKeyBuilder::new(STAGE, 1)
                .section("identitz", [1; 32])
                .section("difficulty", [2; 32])
                .config([3; 32])
                .input([4; 32]),
            VersionKeyBuilder::new(STAGE, 1)
                .section("identity", [9; 32])
                .section("difficulty", [2; 32])
                .config([3; 32])
                .input([4; 32]),
            VersionKeyBuilder::new(STAGE, 1)
                .section("identity", [1; 32])
                .config([3; 32])
                .input([4; 32]),
            builder().config([9; 32]),
            builder().input([9; 32]),
        ];
        for variant in variants {
            assert_ne!(variant.finish().unwrap(), base);
        }
        let explicit_zeros = VersionKeyBuilder::new(STAGE, 1)
            .config([0; 32])
            .input([0; 32]);
        assert_eq!(
            explicit_zeros.finish().unwrap(),
            VersionKeyBuilder::new(STAGE, 1).finish().unwrap()
        );
    }

    #[test]
    fn serde_is_hex_string() {
        let key = builder().finish().unwrap();
        assert_eq!(
            serde_json::to_string(&key).unwrap(),
            format!("\"{GOLDEN}\"")
        );
    }
}
