//! Chart families: a map and its rate copies count as one chart in the rating, as Etterna counts
//! one chart's rates (ADR 0024).

use super::params::FamilyParams;

/// `set_folder` plus `version` without trailing rate tags, under the default params.
pub fn family_key(set_folder: &str, version: &str) -> String {
    FamilyParams::default().family_key(set_folder, version)
}

impl FamilyParams {
    /// A folder name cannot hold `/`, so the separator never makes two keys collide.
    pub fn family_key(&self, set_folder: &str, version: &str) -> String {
        format!("{set_folder}/{}", self.strip_rate_tags(version))
    }

    /// Tags stack (`Insane 1.2x (240bpm)`), so stripping repeats until nothing changes.
    fn strip_rate_tags<'a>(&self, version: &'a str) -> &'a str {
        let mut rest = version.trim_end();
        while let Some(shorter) = self.strip_one(rest) {
            rest = shorter.trim_end();
        }
        rest
    }

    fn strip_one<'a>(&self, s: &'a str) -> Option<&'a str> {
        let last = s.chars().next_back()?;
        if let Some(&(open, _)) = self.brackets.iter().find(|(_, close)| *close == last) {
            let start = s.rfind(open)?;
            let inner = &s[start + open.len_utf8()..s.len() - last.len_utf8()];
            return self.is_tag(inner.trim()).then_some(&s[..start]);
        }
        // A bare tag must be its own word, so `Insanex` keeps its `x`.
        let start = s
            .char_indices()
            .rfind(|(_, c)| c.is_whitespace())
            .map_or(0, |(i, c)| i + c.len_utf8());
        self.is_tag(&s[start..]).then_some(&s[..start])
    }

    fn is_tag(&self, token: &str) -> bool {
        let lower = token.to_ascii_lowercase();
        let suffixed = self
            .number_suffixes
            .iter()
            .any(|sfx| lower.strip_suffix(sfx.as_str()).is_some_and(is_number));
        suffixed
            || self
                .number_prefixes
                .iter()
                .any(|pfx| lower.strip_prefix(pfx.as_str()).is_some_and(is_number))
    }
}

/// Digits with at most one `.`, at least one digit.
fn is_number(s: &str) -> bool {
    let mut digits = 0usize;
    let mut dots = 0usize;
    for b in s.bytes() {
        match b {
            b'0'..=b'9' => digits += 1,
            b'.' => dots += 1,
            _ => return false,
        }
    }
    digits > 0 && dots <= 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rate_copies_share_the_original_family() {
        let original = family_key("123 Artist - Title", "Insane");
        for copy in [
            "Insane 1.15x",
            "Insane 1.2X",
            "Insane (207bpm)",
            "Insane [1.2x]",
            "Insane x1.2",
            "Insane 1.2x (240bpm)",
            "Insane (1.2x)",
            "Insane  0.9x  ",
            "Insane 1x",
            "Insane(207BPM)",
        ] {
            assert_eq!(family_key("123 Artist - Title", copy), original, "{copy:?}");
        }
    }

    #[test]
    fn non_rate_suffixes_are_kept() {
        let insane = family_key("s", "Insane");
        for version in [
            "Insane 2",
            "Insane (Lv.23)",
            "Insane [Hard]",
            "Insanex",
            "Insane x",
            "Insane 1.2.3x",
            "Insane .x",
            "Insane bpm",
        ] {
            assert_ne!(family_key("s", version), insane, "{version:?}");
        }
        assert_ne!(family_key("a", "Insane"), family_key("b", "Insane"));
    }

    #[test]
    fn a_bare_rate_version_keeps_its_set() {
        assert_eq!(family_key("s", "1.2x"), family_key("s", "1.0x"));
        assert_ne!(family_key("s", "1.2x"), family_key("t", "1.2x"));
    }

    #[test]
    fn multibyte_whitespace_splits_words_without_panicking() {
        let insane = family_key("s", "Insane");
        assert_eq!(family_key("s", "Insane\u{3000}1.2x"), insane);
        assert_eq!(family_key("s", "Insane\u{a0}1.2x"), insane);
        assert_eq!(family_key("s", "\u{3000}1.2x"), family_key("s", ""));
        assert_ne!(family_key("s", "Insane\u{3000}Hard"), insane);
    }

    #[test]
    fn tags_follow_the_params() {
        let params = FamilyParams {
            number_suffixes: vec!["x".into()],
            ..FamilyParams::default()
        };
        assert_ne!(
            params.family_key("s", "Insane (207bpm)"),
            params.family_key("s", "Insane")
        );
        assert_eq!(
            params.family_key("s", "Insane (1.2x)"),
            params.family_key("s", "Insane")
        );
    }
}
