//! Non-fatal decode oddities with stable string codes (spec 002 Design, §7 lenient parsers).
//! `detail` carries offsets, counts and versions only, never names, paths or cfg values.

use crate::stable::stable_str_enum;

stable_str_enum! {
    /// Append-only and never renumbered; 006 appends `osg.*`.
    pub enum DiagCode {
        FormatUnverifiedVersion => "format.unverified_version",
        OsuDbBadMd5 => "osu_db.bad_md5",
        OsuDbUnknownRankedStatus => "osu_db.unknown_ranked_status",
        OsuDbUnknownMode => "osu_db.unknown_mode",
        OsrEmptyPayload => "osr.empty_payload",
        OsrTrailingBytes => "osr.trailing_bytes",
        OsrNameMd5Mismatch => "osr.name_md5_mismatch",
        OsrNameTimeMismatch => "osr.name_time_mismatch",
        CfgMalformedLine => "cfg.malformed_line",
        CfgDuplicateKey => "cfg.duplicate_key",
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub code: DiagCode,
    pub offset: Option<u64>,
    pub line: Option<u32>,
    pub detail: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Diagnostics(Vec<Diagnostic>);

impl Diagnostics {
    pub const fn new() -> Self {
        Self(Vec::new())
    }

    pub fn push(&mut self, diagnostic: Diagnostic) {
        self.0.push(diagnostic);
    }

    pub fn at_offset(&mut self, code: DiagCode, offset: u64, detail: impl Into<String>) {
        self.push(Diagnostic {
            code,
            offset: Some(offset),
            line: None,
            detail: detail.into(),
        });
    }

    pub fn at_line(&mut self, code: DiagCode, line: u32) {
        self.push(Diagnostic {
            code,
            offset: None,
            line: Some(line),
            detail: String::new(),
        });
    }

    pub fn general(&mut self, code: DiagCode, detail: impl Into<String>) {
        self.push(Diagnostic {
            code,
            offset: None,
            line: None,
            detail: detail.into(),
        });
    }

    pub fn extend(&mut self, other: Self) {
        self.0.extend(other.0);
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
    fn diag_codes_are_stable_strings() {
        let expected = [
            "format.unverified_version",
            "osu_db.bad_md5",
            "osu_db.unknown_ranked_status",
            "osu_db.unknown_mode",
            "osr.empty_payload",
            "osr.trailing_bytes",
            "osr.name_md5_mismatch",
            "osr.name_time_mismatch",
            "cfg.malformed_line",
            "cfg.duplicate_key",
        ];
        let actual: Vec<&str> = DiagCode::ALL.iter().map(|c| c.as_str()).collect();
        // Prefix, not equality: later specs append codes, but never reorder or rename these.
        assert_eq!(&actual[..expected.len()], expected);
        let unique: BTreeSet<&str> = actual.iter().copied().collect();
        assert_eq!(unique.len(), actual.len());
        for code in DiagCode::ALL {
            assert!(is_valid_stable_id(code.as_str()), "{code}");
            assert_eq!(DiagCode::from_str_id(code.as_str()), Some(*code));
        }
    }

    #[test]
    fn collector_records_code_and_position() {
        let mut d = Diagnostics::new();
        d.at_offset(DiagCode::OsrTrailingBytes, 12, "8 bytes");
        d.at_line(DiagCode::CfgMalformedLine, 3);
        assert_eq!(d.len(), 2);
        assert!(d.contains(DiagCode::OsrTrailingBytes));
        assert!(!d.contains(DiagCode::OsuDbBadMd5));
        assert_eq!(d.as_slice()[1].line, Some(3));
        assert_eq!(d.as_slice()[0].offset, Some(12));
    }
}
