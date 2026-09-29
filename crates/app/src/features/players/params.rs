//! Identity thresholds (D17). F1 moves the defaults into param pack section `identity`.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IdentityParams {
    /// Below this many Unicode scalars a normalized alias never auto-matches: `""`, `w` and
    /// `s` are prefixes of almost every login (ADR 0005, prefix without a length guard).
    pub min_norm_len: u32,
    pub top_charts_n: u32,
}

impl Default for IdentityParams {
    fn default() -> Self {
        Self {
            min_norm_len: 4,
            top_charts_n: 5,
        }
    }
}
