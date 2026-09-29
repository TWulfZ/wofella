//! `osu!.<account>.cfg` reader restricted to an allowlist of keys. The file also holds a
//! `Password` line whose value must never enter wolluf-owned memory, logs or storage (spec 002
//! R9), so a key is matched before its value is touched and non-allowlisted values are skipped.

use crate::diag::{DiagCode, Diagnostics};

const UTF8_BOM: &[u8] = b"\xef\xbb\xbf";
const COMMENT: u8 = b'#';
const KEY_USERNAME: &[u8] = b"Username";
const KEY_BEATMAP_DIRECTORY: &[u8] = b"BeatmapDirectory";

/// Values are verbatim apart from trimming ASCII whitespace at both ends: the username is
/// interpreted only by 004's session-user match (spec 002 R9).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UserCfg {
    pub username: Option<String>,
    pub beatmap_directory: Option<String>,
}

#[derive(Clone, Copy)]
enum Key {
    Username,
    BeatmapDirectory,
}

fn allowlisted(key: &[u8]) -> Option<Key> {
    if key.eq_ignore_ascii_case(KEY_USERNAME) {
        Some(Key::Username)
    } else if key.eq_ignore_ascii_case(KEY_BEATMAP_DIRECTORY) {
        Some(Key::BeatmapDirectory)
    } else {
        None
    }
}

/// Diagnostics carry line numbers only, never line content.
pub fn parse_user_cfg(bytes: &[u8]) -> (UserCfg, Diagnostics) {
    let bytes = bytes.strip_prefix(UTF8_BOM).unwrap_or(bytes);
    let mut cfg = UserCfg::default();
    let mut diags = Diagnostics::new();
    let mut seen_username = false;
    let mut seen_beatmap_directory = false;
    for (idx, raw_line) in bytes.split(|&b| b == b'\n').enumerate() {
        let line_no = u32::try_from(idx + 1).unwrap_or(u32::MAX);
        let line = raw_line.trim_ascii();
        if line.is_empty() || line[0] == COMMENT {
            continue;
        }
        let Some(eq) = line.iter().position(|&b| b == b'=') else {
            diags.at_line(DiagCode::CfgMalformedLine, line_no);
            continue;
        };
        let Some(key) = allowlisted(line[..eq].trim_ascii()) else {
            continue;
        };
        let value = line[eq + 1..].trim_ascii();
        let value = (!value.is_empty()).then(|| String::from_utf8_lossy(value).into_owned());
        let (slot, seen) = match key {
            Key::Username => (&mut cfg.username, &mut seen_username),
            Key::BeatmapDirectory => (&mut cfg.beatmap_directory, &mut seen_beatmap_directory),
        };
        if *seen {
            diags.at_line(DiagCode::CfgDuplicateKey, line_no);
        }
        *seen = true;
        *slot = value;
    }
    (cfg, diags)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diag::DiagCode;

    #[test]
    fn parses_username_with_inner_spaces() {
        let (cfg, diags) =
            parse_user_cfg(b"Username = TWulfZasdasdasd d jSS||\r\nVolumeUniversal = 50\r\n");
        assert_eq!(cfg.username.as_deref(), Some("TWulfZasdasdasd d jSS||"));
        assert!(diags.is_empty());
    }

    #[test]
    fn parses_beatmap_directory() {
        let (cfg, _) = parse_user_cfg(b"BeatmapDirectory = D:\\osu songs\r\n");
        assert_eq!(cfg.beatmap_directory.as_deref(), Some("D:\\osu songs"));
        let (cfg, _) = parse_user_cfg(b"BeatmapDirectory = \r\nUsername=\t\r\n");
        assert_eq!(cfg.beatmap_directory, None);
        assert_eq!(cfg.username, None);
    }

    #[test]
    fn key_match_case_insensitive() {
        let (cfg, _) = parse_user_cfg(b"username = a\nBEATMAPDIRECTORY = Songs\n");
        assert_eq!(cfg.username.as_deref(), Some("a"));
        assert_eq!(cfg.beatmap_directory.as_deref(), Some("Songs"));
    }

    #[test]
    fn bom_tolerated() {
        let (cfg, diags) = parse_user_cfg(b"\xef\xbb\xbfUsername = W\r\n");
        assert_eq!(cfg.username.as_deref(), Some("W"));
        assert!(diags.is_empty());
    }

    #[test]
    fn duplicate_key_last_wins() {
        let (cfg, diags) =
            parse_user_cfg(b"Username = first\n# comment = x\n\nUsername = second\nnot a pair\n");
        assert_eq!(cfg.username.as_deref(), Some("second"));
        assert_eq!(
            diags.codes(),
            vec![DiagCode::CfgDuplicateKey, DiagCode::CfgMalformedLine]
        );
        assert_eq!(diags.as_slice()[0].line, Some(4));
        assert_eq!(diags.as_slice()[1].line, Some(5));
        assert!(diags.iter().all(|d| d.detail.is_empty()));
    }

    #[test]
    fn password_value_never_in_output() {
        let bytes =
            b"Username = W\r\nPassword = WOLLUF_SENTINEL_9f3a\r\nPassword WOLLUF_SENTINEL_9f3a\r\n";
        let (cfg, diagnostics) = parse_user_cfg(bytes);
        let printed = format!("{:?}", (cfg, diagnostics));
        assert!(!printed.contains("WOLLUF_SENTINEL_9f3a"), "{printed}");
        assert!(!printed.contains("9f3a"));
    }
}
