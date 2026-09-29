//! `Data/r` file names `<beatmap md5>-<FILETIME>.osr|.osg` (research 03 l.161, l.168). The only
//! `Data/r` name parser in the workspace: 003's `replay_dir::index` calls it.

use wolluf_core::{ChartMd5, FileTime};

const MD5_HEX_LEN: usize = 32;
/// `i64::MAX` has 19 decimal digits; longer suffixes cannot be a FILETIME.
const MAX_FILETIME_DIGITS: usize = 19;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ReplayFileKind {
    Osr,
    Osg,
}

impl ReplayFileKind {
    pub const fn extension(self) -> &'static str {
        match self {
            Self::Osr => "osr",
            Self::Osg => "osg",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ReplayFileName {
    pub md5: ChartMd5,
    pub filetime: FileTime,
    pub kind: ReplayFileKind,
}

impl ReplayFileName {
    /// Accepts exactly `^[0-9a-f]{32}-[0-9]{1,19}\.(osr|osg)$` with a canonical decimal (no
    /// leading zeros) that fits `i64`, so `format` reproduces the name byte for byte. osu! never
    /// writes uppercase hex or leading zeros; `.osr.tmp` and other partial writes are ignored.
    pub fn parse(name: &str) -> Option<Self> {
        let (stem, ext) = name.rsplit_once('.')?;
        let kind = match ext {
            "osr" => ReplayFileKind::Osr,
            "osg" => ReplayFileKind::Osg,
            _ => return None,
        };
        let (md5_hex, digits) = stem.split_once('-')?;
        if md5_hex.len() != MD5_HEX_LEN || digits.len() > MAX_FILETIME_DIGITS {
            return None;
        }
        Some(Self {
            md5: md5_hex.parse().ok()?,
            filetime: FileTime::parse_decimal(digits).ok()?,
            kind,
        })
    }

    pub fn format(&self) -> String {
        format!("{}-{}.{}", self.md5, self.filetime, self.kind.extension())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MD5: &str = "e956977ccc1d74a50ae48b43a868cc20";

    #[test]
    fn parses_osr_and_osg() {
        let osr = ReplayFileName::parse(&format!("{MD5}-134279471004225018.osr")).unwrap();
        assert_eq!(osr.md5, MD5.parse::<ChartMd5>().unwrap());
        assert_eq!(osr.filetime.get(), 134_279_471_004_225_018);
        assert_eq!(osr.kind, ReplayFileKind::Osr);
        let osg = ReplayFileName::parse(&format!("{MD5}-0.osg")).unwrap();
        assert_eq!(osg.kind, ReplayFileKind::Osg);
        assert_eq!(osg.filetime.get(), 0);
        assert_eq!(osg.format(), format!("{MD5}-0.osg"));
    }

    #[test]
    fn rejects_uppercase_tmp_and_missing_ext() {
        let upper = MD5.to_uppercase();
        for bad in [
            format!("{upper}-1.osr"),
            format!("{MD5}-1.osr.tmp"),
            format!("{MD5}-1"),
            format!("{MD5}-1."),
            format!("{MD5}-1.OSR"),
            format!("{MD5}-1.osu"),
            format!("{MD5}-abc.osr"),
            format!("{MD5}-.osr"),
            format!("{MD5}--1.osr"),
            format!("{MD5}-+1.osr"),
            format!("{MD5}1.osr"),
            format!("{}-1.osr", &MD5[..31]),
            format!("x{MD5}-1.osr"),
            format!("{MD5}-01.osr"),
            String::new(),
        ] {
            assert_eq!(ReplayFileName::parse(&bad), None, "{bad:?}");
        }
    }

    #[test]
    fn rejects_overflowing_filetime() {
        assert!(ReplayFileName::parse(&format!("{MD5}-9223372036854775807.osr")).is_some());
        assert_eq!(
            ReplayFileName::parse(&format!("{MD5}-9223372036854775808.osr")),
            None
        );
        assert_eq!(
            ReplayFileName::parse(&format!("{MD5}-12345678901234567890.osr")),
            None
        );
    }
}

#[cfg(test)]
mod props {
    use proptest::prelude::*;

    use super::*;

    proptest! {
        #[test]
        fn format_parse_roundtrip(md5 in any::<[u8; 16]>(), ft in 0..=i64::MAX, osg in any::<bool>()) {
            let name = ReplayFileName {
                md5: ChartMd5(md5),
                filetime: FileTime::new(ft).unwrap(),
                kind: if osg { ReplayFileKind::Osg } else { ReplayFileKind::Osr },
            };
            let text = name.format();
            prop_assert_eq!(ReplayFileName::parse(&text), Some(name));
        }
    }
}
