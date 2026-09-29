//! Name normalization and the session-user match (spec 004 R4, R5).

use caseless::Caseless;
use unicode_normalization::UnicodeNormalization;

use super::IdentityParams;

/// Stable strings: persisted in DTOs and never renumbered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SessionMatchKind {
    Equal,
    Prefix,
}

impl SessionMatchKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Equal => "equal",
            Self::Prefix => "prefix",
        }
    }
}

/// NFKC → full default case fold → NFKC → alphanumerics only. The second NFKC is needed
/// because case folding can produce denormalized sequences; `to_lowercase` is not a case fold
/// (`ß` must become `ss`).
pub fn normalize(raw: &str) -> String {
    raw.nfkc()
        .default_case_fold()
        .nfkc()
        .filter(|c| c.is_alphanumeric())
        .collect()
}

/// Both inputs are already [`normalize`]d. No fuzzy matching (ADR 0005 amendment).
pub fn session_match(
    alias_norm: &str,
    login_norm: &str,
    params: &IdentityParams,
) -> Option<SessionMatchKind> {
    if norm_len(alias_norm) < params.min_norm_len {
        return None;
    }
    if alias_norm == login_norm {
        Some(SessionMatchKind::Equal)
    } else if login_norm.starts_with(alias_norm) {
        Some(SessionMatchKind::Prefix)
    } else {
        None
    }
}

/// Unicode scalar count, saturating: no osu! name comes near `u32::MAX` scalars.
pub fn norm_len(norm: &str) -> u32 {
    u32::try_from(norm.chars().count()).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::players::IdentityParams;

    #[test]
    fn normalize_table() {
        let table = [
            ("TWulfZasdasdasd d jSS||", "twulfzasdasdasddjss"),
            ("ＴＷｕｌｆＺ", "twulfz"),
            ("Straße", "strasse"),
            ("x_Wulf-", "xwulf"),
            ("||", ""),
            ("", ""),
            ("TWulfZ", "twulfz"),
            ("ΣΊΣΥΦΟΣ", "σίσυφοσ"),
        ];
        for (raw, expected) in table {
            assert_eq!(normalize(raw), expected, "{raw:?}");
        }
    }

    #[test]
    fn session_match_table() {
        let params = IdentityParams::default();
        let login = normalize("TWulfZasdasdasd d jSS||");
        let table = [
            ("TWulfZ", Some(SessionMatchKind::Prefix)),
            ("TWulfZasdasdasd d jSS||", Some(SessionMatchKind::Equal)),
            ("TWulfS", None),
            ("", None),
            ("w", None),
            ("W", None),
            ("s", None),
            ("Wulf", None),
            ("TWulfZasdasdasd d jSS|| extra", None),
        ];
        for (alias, expected) in table {
            assert_eq!(
                session_match(&normalize(alias), &login, &params),
                expected,
                "{alias:?}"
            );
        }
        // Length guard: equality alone is not enough below `min_norm_len`.
        assert_eq!(session_match("twu", "twu", &params), None);
        assert_eq!(session_match("twu", "twulfz", &params), None);
        assert_eq!(
            session_match("twul", "twul", &params),
            Some(SessionMatchKind::Equal)
        );
        // Unicode scalars, not bytes: four 2-byte letters pass the guard.
        assert_eq!(
            session_match("σίσυ", "σίσυφοσ", &params),
            Some(SessionMatchKind::Prefix)
        );
        assert_eq!(session_match("", "", &params), None);
    }

    #[test]
    fn session_match_kind_strings() {
        assert_eq!(SessionMatchKind::Equal.as_str(), "equal");
        assert_eq!(SessionMatchKind::Prefix.as_str(), "prefix");
    }
}
