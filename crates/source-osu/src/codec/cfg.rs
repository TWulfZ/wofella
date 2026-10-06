//! `osu!.<account>.cfg` reader restricted to an allowlist of keys. The file also holds a
//! `Password` line whose value must never enter wolluf-owned memory, logs or storage (spec 002
//! R9), so a key is matched before its value is touched and non-allowlisted values are skipped.

use std::ops::RangeInclusive;

use crate::diag::{DiagCode, Diagnostics};

const UTF8_BOM: &[u8] = b"\xef\xbb\xbf";
const COMMENT: u8 = b'#';
/// Stable's scroll-speed slider bounds (wiki `Game_mode/osu!mania`); a value outside them is not
/// a speed stable would use, so it is dropped rather than clamped.
const MANIA_SPEED: RangeInclusive<u8> = 1..=40;

/// Values are verbatim apart from trimming ASCII whitespace at both ends: the username is
/// interpreted only by 004's session-user match (spec 002 R9), and `skin` is a folder name under
/// `Skins/` whose leading `-`, inner spaces and `#` are part of the name.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UserCfg {
    pub username: Option<String>,
    pub beatmap_directory: Option<String>,
    pub skin: Option<String>,
    pub mania_speed: Option<u8>,
    pub mania_speed_bpm_scale: Option<bool>,
    pub use_per_beatmap_mania_speed: Option<bool>,
}

#[derive(Clone, Copy)]
enum Key {
    Username,
    BeatmapDirectory,
    Skin,
    ManiaSpeed,
    ManiaSpeedBpmScale,
    UsePerBeatmapManiaSpeed,
}

const ALLOWLIST: [(&[u8], Key); 6] = [
    (b"Username", Key::Username),
    (b"BeatmapDirectory", Key::BeatmapDirectory),
    (b"Skin", Key::Skin),
    (b"ManiaSpeed", Key::ManiaSpeed),
    (b"ManiaSpeedBPMScale", Key::ManiaSpeedBpmScale),
    (b"UsePerBeatmapManiaSpeed", Key::UsePerBeatmapManiaSpeed),
];

fn allowlisted(key: &[u8]) -> Option<Key> {
    ALLOWLIST
        .iter()
        .find(|(name, _)| key.eq_ignore_ascii_case(name))
        .map(|&(_, k)| k)
}

fn text(value: &[u8]) -> Option<String> {
    (!value.is_empty()).then(|| String::from_utf8_lossy(value).into_owned())
}

fn mania_speed(value: &[u8]) -> Option<u8> {
    std::str::from_utf8(value)
        .ok()?
        .parse::<u8>()
        .ok()
        .filter(|s| MANIA_SPEED.contains(s))
}

fn flag(value: &[u8]) -> Option<bool> {
    match value {
        b"1" => Some(true),
        b"0" => Some(false),
        _ => None,
    }
}

/// Diagnostics carry line numbers only, never line content.
pub fn parse_user_cfg(bytes: &[u8]) -> (UserCfg, Diagnostics) {
    let bytes = bytes.strip_prefix(UTF8_BOM).unwrap_or(bytes);
    let mut cfg = UserCfg::default();
    let mut diags = Diagnostics::new();
    let mut seen = [false; ALLOWLIST.len()];
    // Stable's reader also ends a line at a bare CR; splitting only on LF would let a Password
    // line ride inside an allowlisted value (spec 002 R9).
    let lines = bytes
        .split(|&b| b == b'\n')
        .enumerate()
        .flat_map(|(idx, line)| line.split(|&b| b == b'\r').map(move |part| (idx, part)));
    for (idx, raw_line) in lines {
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
        let seen = &mut seen[key as usize];
        if *seen {
            diags.at_line(DiagCode::CfgDuplicateKey, line_no);
        }
        *seen = true;
        match key {
            Key::Username => cfg.username = text(value),
            Key::BeatmapDirectory => cfg.beatmap_directory = text(value),
            Key::Skin => cfg.skin = text(value),
            Key::ManiaSpeed => cfg.mania_speed = mania_speed(value),
            Key::ManiaSpeedBpmScale => cfg.mania_speed_bpm_scale = flag(value),
            Key::UsePerBeatmapManiaSpeed => cfg.use_per_beatmap_mania_speed = flag(value),
        }
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
    fn skin_is_the_folder_name_verbatim() {
        let bytes =
            "SkinSamples = 0\r\nSkin = -   #테스트 스킨 (Synthetic v2)\r\nSkinFilter = x\r\n";
        let (cfg, diags) = parse_user_cfg(bytes.as_bytes());
        assert_eq!(cfg.skin.as_deref(), Some("-   #테스트 스킨 (Synthetic v2)"));
        assert!(diags.is_empty());
        let (cfg, _) = parse_user_cfg(b"Skin = \r\n");
        assert_eq!(cfg.skin, None);
    }

    #[test]
    fn mania_speed_in_range_or_none() {
        for (raw, want) in [
            ("1", Some(1)),
            ("30", Some(30)),
            ("40", Some(40)),
            ("0", None),
            ("41", None),
            ("-3", None),
            ("12.5", None),
            ("fast", None),
            ("", None),
        ] {
            let line = format!("ManiaSpeed = {raw}\r\n");
            let (cfg, _) = parse_user_cfg(line.as_bytes());
            assert_eq!(cfg.mania_speed, want, "ManiaSpeed = {raw}");
        }
    }

    #[test]
    fn mania_speed_flags_are_zero_or_one() {
        let (cfg, diags) =
            parse_user_cfg(b"ManiaSpeedBPMScale = 1\r\nUsePerBeatmapManiaSpeed = 0\r\n");
        assert_eq!(cfg.mania_speed_bpm_scale, Some(true));
        assert_eq!(cfg.use_per_beatmap_mania_speed, Some(false));
        assert!(diags.is_empty());
        let (cfg, _) =
            parse_user_cfg(b"ManiaSpeedBPMScale = 2\r\nUsePerBeatmapManiaSpeed = yes\r\n");
        assert_eq!(cfg.mania_speed_bpm_scale, None);
        assert_eq!(cfg.use_per_beatmap_mania_speed, None);
        assert_eq!(UserCfg::default().mania_speed, None);
    }

    #[test]
    fn new_keys_report_duplicates_last_wins() {
        let (cfg, diags) =
            parse_user_cfg(b"ManiaSpeed = 10\nSkin = a\nManiaSpeed = 20\nSkin = b\n");
        assert_eq!(cfg.mania_speed, Some(20));
        assert_eq!(cfg.skin.as_deref(), Some("b"));
        assert_eq!(
            diags.codes(),
            vec![DiagCode::CfgDuplicateKey, DiagCode::CfgDuplicateKey]
        );
    }

    #[test]
    fn password_secret_never_reaches_any_field() {
        let bytes = b"Skin = s\r\nPassword = secret\r\nManiaSpeed = 30\r\nPassword=secret\r\n";
        let (cfg, diagnostics) = parse_user_cfg(bytes);
        for field in [&cfg.username, &cfg.beatmap_directory, &cfg.skin] {
            assert!(!field.as_deref().unwrap_or("").contains("secret"));
        }
        assert_eq!(cfg.skin.as_deref(), Some("s"));
        assert_eq!(cfg.mania_speed, Some(30));
        let printed = format!("{:?}", (cfg, diagnostics));
        assert!(!printed.contains("secret"), "{printed}");
    }

    #[test]
    fn a_bare_cr_ends_a_line_so_password_never_joins_an_allowlisted_value() {
        let (cfg, _) =
            parse_user_cfg(b"Skin = a\rPassword = secret\rUsername = u\rPassword = secret\r");
        assert_eq!(cfg.skin.as_deref(), Some("a"));
        assert_eq!(cfg.username.as_deref(), Some("u"));
        assert!(!format!("{cfg:?}").contains("secret"));
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
