//! Chart families: a map and its rate copies count as one chart in the rating, as Etterna counts
//! one chart's rates (ADR 0024).

use super::params::FamilyParams;

const MILLI: f64 = 1_000.0;

/// `set_folder` plus `version`, without its trailing tags when they carry a rate, under the
/// default params.
pub fn family_key(set_folder: &str, version: &str) -> String {
    FamilyParams::default().family_key(set_folder, version)
}

/// The rate a rate copy was generated at, from its difficulty name, under the default params.
pub fn chart_rate_milli(version: &str) -> Option<u16> {
    FamilyParams::default().chart_rate_milli(version)
}

impl FamilyParams {
    /// A folder name cannot hold `/`, so the separator never makes two keys collide. Tags are
    /// stripped only when they yield a rate: a copy whose rate is unknown would otherwise share
    /// its original's (family, rate) slot while playing at another speed.
    pub fn family_key(&self, set_folder: &str, version: &str) -> String {
        let version = if self.chart_rate_milli(version).is_some() {
            self.strip_rate_tags(version)
        } else {
            version.trim_end()
        };
        format!("{set_folder}/{version}")
    }

    /// The multiplier of the last trailing multiplier tag (`1.15x`, `x1.2`, `[1.2x]`); `None`
    /// without one, or when it rounds to 0 ms per s or past `u16`.
    pub fn chart_rate_milli(&self, version: &str) -> Option<u16> {
        let mut rest = version.trim_end();
        while let Some((shorter, tag)) = self.strip_one(rest) {
            if let Some(n) = self.multiplier(tag) {
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
        self.multiplier(token).is_some() || {
            let lower = token.to_ascii_lowercase();
            self.marker_tags.contains(&lower)
                || self
                    .other_suffixes
                    .iter()
                    .any(|sfx| lower.strip_suffix(sfx.as_str()).is_some_and(is_number))
        }
    }

    /// The number of a multiplier tag, as written.
    fn multiplier(&self, token: &str) -> Option<String> {
        let lower = token.to_ascii_lowercase();
        let suffixed = self
            .multiplier_suffixes
            .iter()
            .find_map(|sfx| lower.strip_suffix(sfx.as_str()).filter(|n| is_number(n)));
        let prefixed = || {
            self.multiplier_prefixes
                .iter()
                .find_map(|pfx| lower.strip_prefix(pfx.as_str()).filter(|n| is_number(n)))
        };
        suffixed.or_else(prefixed).map(str::to_owned)
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
            "Insane [1.2x]",
            "Insane x1.2",
            "Insane 1.2x (240bpm)",
            "Insane (240bpm) 1.3x",
            "Insane (1.2x)",
            "Insane  0.9x  ",
            "Insane 1x",
        ] {
            assert_eq!(family_key("123 Artist - Title", copy), original, "{copy:?}");
        }
    }

    #[test]
    fn nc_copies_share_the_original_family_and_carry_their_rate() {
        let dt = "Normal 1.25x (150bpm)";
        let nc = "Normal 1.25x NC (150bpm)";
        assert_eq!(chart_rate_milli(nc), Some(1250));
        assert_eq!(family_key("s", nc), family_key("s", dt));
        assert_eq!(family_key("s", nc), family_key("s", "Normal"));
    }

    #[test]
    fn a_marker_alone_is_not_a_rate_copy() {
        assert_eq!(chart_rate_milli("Normal NC"), None);
        assert_eq!(family_key("s", "Normal NC"), "s/Normal NC");
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

    /// A stripped tag with no rate would put the copy in its original's (family, rate) slot at
    /// the original's rate.
    #[test]
    fn tags_without_a_parsable_rate_stay_in_the_key() {
        let insane = family_key("s", "Insane");
        for version in [
            "Insane (207bpm)",
            "Insane(207BPM)",
            "Insane 0x",
            "Insane 0.0001x",
            "Insane 99999x",
            "Insane 1.2x 0x",
        ] {
            assert_ne!(family_key("s", version), insane, "{version:?}");
            assert_eq!(
                family_key("s", version),
                format!("s/{version}"),
                "{version:?}"
            );
        }
        assert_eq!(family_key("s", "Insane (207bpm)  "), "s/Insane (207bpm)");
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
        let no_bpm = FamilyParams {
            other_suffixes: Vec::new(),
            ..FamilyParams::default()
        };
        // Without `bpm` as a tag, the multiplier is no longer the last tag.
        assert_eq!(no_bpm.chart_rate_milli("Insane 1.2x (240bpm)"), None);
        assert_ne!(
            no_bpm.family_key("s", "Insane 1.2x (240bpm)"),
            no_bpm.family_key("s", "Insane")
        );
        assert_eq!(
            no_bpm.family_key("s", "Insane (1.2x)"),
            no_bpm.family_key("s", "Insane")
        );

        let times = FamilyParams {
            multiplier_suffixes: vec!["x".into(), "\u{d7}".into()],
            multiplier_prefixes: vec!["x".into(), "\u{d7}".into()],
            ..FamilyParams::default()
        };
        assert_eq!(chart_rate_milli("Insane 1.2\u{d7}"), None);
        assert_eq!(times.chart_rate_milli("Insane 1.2\u{d7}"), Some(1200));
        assert_eq!(times.chart_rate_milli("Insane \u{d7}1.3"), Some(1300));
        assert_eq!(
            times.family_key("s", "Insane [1.2\u{d7}]"),
            times.family_key("s", "Insane")
        );

        let no_prefix = FamilyParams {
            multiplier_prefixes: Vec::new(),
            ..FamilyParams::default()
        };
        assert_eq!(no_prefix.chart_rate_milli("Insane x1.2"), None);
        assert_eq!(no_prefix.family_key("s", "Insane x1.2"), "s/Insane x1.2");
    }
}
