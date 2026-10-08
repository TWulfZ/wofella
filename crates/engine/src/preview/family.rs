//! Chart families: a map and its rate copies count as one chart in the rating, as Etterna counts
//! one chart's rates (ADR 0024).

use super::params::FamilyParams;

/// The marker rate-copy tools put on a speed multiplier; `bpm` tags carry no rate.
const MULTIPLIER: &str = "x";
const MILLI: f64 = 1_000.0;

/// `set_folder` plus `version` without trailing rate tags, under the default params.
pub fn family_key(set_folder: &str, version: &str) -> String {
    FamilyParams::default().family_key(set_folder, version)
}

/// The rate a rate copy was generated at, from its difficulty name, under the default params.
pub fn chart_rate_milli(version: &str) -> Option<u16> {
    FamilyParams::default().chart_rate_milli(version)
}

impl FamilyParams {
    /// A folder name cannot hold `/`, so the separator never makes two keys collide.
    pub fn family_key(&self, set_folder: &str, version: &str) -> String {
        format!("{set_folder}/{}", self.strip_rate_tags(version))
    }

    /// The multiplier of the last trailing `x` tag (`1.15x`, `x1.2`, `[1.2x]`); `None` without one,
    /// or when it rounds to 0 ms per s or past `u16`.
    pub fn chart_rate_milli(&self, version: &str) -> Option<u16> {
        let mut rest = version.trim_end();
        while let Some((shorter, tag)) = self.strip_one(rest) {
            let lower = tag.to_ascii_lowercase();
            let number = lower
                .strip_suffix(MULTIPLIER)
                .or_else(|| lower.strip_prefix(MULTIPLIER))
                .filter(|n| is_number(n));
            if let Some(n) = number {
                let milli = (n.parse::<f64>().ok()? * MILLI).round();
                return (milli >= 1.0 && milli <= f64::from(u16::MAX)).then_some(milli as u16);
            }
            rest = shorter.trim_end();
        }
        None
    }

    /// Tags stack (`Insane 1.2x (240bpm)`), so stripping repeats until nothing changes.
    fn strip_rate_tags<'a>(&self, version: &'a str) -> &'a str {
        let mut rest = version.trim_end();
        while let Some((shorter, _)) = self.strip_one(rest) {
            rest = shorter.trim_end();
        }
        rest
    }

    /// `(what precedes the last tag, the tag without brackets)`.
    fn strip_one<'a>(&self, s: &'a str) -> Option<(&'a str, &'a str)> {
        let last = s.chars().next_back()?;
        if let Some(&(open, _)) = self.brackets.iter().find(|(_, close)| *close == last) {
            let start = s.rfind(open)?;
            let inner = s[start + open.len_utf8()..s.len() - last.len_utf8()].trim();
            return self.is_tag(inner).then_some((&s[..start], inner));
        }
        // A bare tag must be its own word, so `Insanex` keeps its `x`.
        let start = s
            .char_indices()
            .rfind(|(_, c)| c.is_whitespace())
            .map_or(0, |(i, c)| i + c.len_utf8());
        let tag = &s[start..];
        self.is_tag(tag).then_some((&s[..start], tag))
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
    fn rate_copies_carry_their_rate() {
        for (version, milli) in [
            ("Insane 1.15x", 1150),
            ("Insane 1.2X", 1200),
            ("Insane x1.2", 1200),
            ("Insane [1.2x]", 1200),
            ("Insane (1.2x)", 1200),
            ("Insane 0.85x", 850),
            ("Insane 1x", 1000),
            ("Insane .9x", 900),
            ("Insane 1.2x (240bpm)", 1200),
            ("Insane (240bpm) 1.3x", 1300),
            ("Insane\u{3000}1.25x", 1250),
            ("1.4x", 1400),
        ] {
            assert_eq!(chart_rate_milli(version), Some(milli), "{version:?}");
        }
    }

    #[test]
    fn versions_without_a_multiplier_have_no_rate() {
        for version in [
            "Insane",
            "Insane (207bpm)",
            "Insane 2",
            "Insanex",
            "Insane x",
            "Insane 1.2.3x",
            "Insane 1.2x Hard",
            "Insane 0x",
            "Insane 0.0001x",
            "Insane 99999x",
            "",
        ] {
            assert_eq!(chart_rate_milli(version), None, "{version:?}");
        }
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
