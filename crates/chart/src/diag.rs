//! Non-fatal decode oddities with stable string codes (conventions "Errors": lenient parsers).
//! `detail` carries times, columns and counts only, never titles or file names.

macro_rules! diag_codes {
    ($($variant:ident => $text:literal),+ $(,)?) => {
        /// Append-only and never renumbered: the strings are persisted with parse results.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub enum DiagCode {
            $($variant),+
        }

        impl DiagCode {
            pub const ALL: &'static [Self] = &[$(Self::$variant),+];

            pub const fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $text),+
                }
            }

            pub fn from_str_id(s: &str) -> Option<Self> {
                match s {
                    $($text => Some(Self::$variant),)+
                    _ => None,
                }
            }
        }
    };
}

diag_codes! {
    DuplicateNote => "chart.duplicate_note",
    LnTailNotAfterHead => "chart.ln_tail_not_after_head",
    OverlappingNote => "chart.overlapping_note",
    ColumnOutOfRange => "chart.column_out_of_range",
    MissingFormatVersion => "osu.missing_format_version",
    MalformedLine => "osu.malformed_line",
    UnsupportedHitObject => "osu.unsupported_hit_object",
    BadTimingPoint => "osu.bad_timing_point",
    MissingCircleSize => "osu.missing_circle_size",
    KeymodeClamped => "osu.keymode_clamped",
}

impl std::fmt::Display for DiagCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub code: DiagCode,
    pub detail: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Diagnostics(Vec<Diagnostic>);

impl Diagnostics {
    pub const fn new() -> Self {
        Self(Vec::new())
    }

    pub fn push(&mut self, code: DiagCode, detail: impl Into<String>) {
        self.0.push(Diagnostic {
            code,
            detail: detail.into(),
        });
    }

    pub fn as_slice(&self) -> &[Diagnostic] {
        &self.0
    }

    pub fn iter(&self) -> std::slice::Iter<'_, Diagnostic> {
        self.0.iter()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn contains(&self, code: DiagCode) -> bool {
        self.0.iter().any(|d| d.code == code)
    }

    pub fn codes(&self) -> Vec<DiagCode> {
        self.0.iter().map(|d| d.code).collect()
    }
}

impl<'a> IntoIterator for &'a Diagnostics {
    type Item = &'a Diagnostic;
    type IntoIter = std::slice::Iter<'a, Diagnostic>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use wolluf_core::id::is_valid_stable_id;

    use super::*;

    #[test]
    fn diag_codes_are_unique_stable_ids() {
        let all: Vec<&str> = DiagCode::ALL.iter().map(|c| c.as_str()).collect();
        let unique: BTreeSet<&str> = all.iter().copied().collect();
        assert_eq!(unique.len(), all.len());
        for code in DiagCode::ALL {
            assert!(is_valid_stable_id(code.as_str()), "{code}");
            assert_eq!(DiagCode::from_str_id(code.as_str()), Some(*code));
        }
    }
}
