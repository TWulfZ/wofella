//! The `[Events]` section of a `.osu`, outside the chart parse: the stored chart never carries
//! media, so a background read on demand leaves every parse key untouched.

use std::convert::Infallible;

use rosu_map::{DecodeBeatmap, DecodeState};

/// lazer's `LegacyEventType.Background`, written either as its number or its name.
const BACKGROUND: [&str; 2] = ["0", "Background"];
const FILENAME_FIELD: usize = 2;

/// The file the first background event names, relative to the set folder with `/`
/// separators; `None` without one. Video and storyboard events are not backgrounds.
pub fn background_name(osu: &[u8]) -> Option<String> {
    rosu_map::from_bytes::<Events>(osu).ok()?.background
}

#[derive(Default)]
struct Events {
    background: Option<String>,
}

impl DecodeState for Events {
    fn create(_version: i32) -> Self {
        Self::default()
    }
}

fn background_of(line: &str) -> Option<String> {
    let mut fields = line.split(',');
    if !BACKGROUND.contains(&fields.next()?.trim()) {
        return None;
    }
    let name = fields
        .nth(FILENAME_FIELD - 1)?
        .trim()
        .trim_matches('"')
        .trim();
    (!name.is_empty()).then(|| name.replace('\\', "/"))
}

impl DecodeBeatmap for Events {
    type Error = Infallible;
    type State = Self;

    fn parse_events(state: &mut Self, line: &str) -> Result<(), Infallible> {
        if state.background.is_none() {
            state.background = background_of(line);
        }
        Ok(())
    }

    fn parse_general(_: &mut Self, _: &str) -> Result<(), Infallible> {
        Ok(())
    }

    fn parse_editor(_: &mut Self, _: &str) -> Result<(), Infallible> {
        Ok(())
    }

    fn parse_metadata(_: &mut Self, _: &str) -> Result<(), Infallible> {
        Ok(())
    }

    fn parse_difficulty(_: &mut Self, _: &str) -> Result<(), Infallible> {
        Ok(())
    }

    fn parse_timing_points(_: &mut Self, _: &str) -> Result<(), Infallible> {
        Ok(())
    }

    fn parse_colors(_: &mut Self, _: &str) -> Result<(), Infallible> {
        Ok(())
    }

    fn parse_hit_objects(_: &mut Self, _: &str) -> Result<(), Infallible> {
        Ok(())
    }

    fn parse_variables(_: &mut Self, _: &str) -> Result<(), Infallible> {
        Ok(())
    }

    fn parse_catch_the_beat(_: &mut Self, _: &str) -> Result<(), Infallible> {
        Ok(())
    }

    fn parse_mania(_: &mut Self, _: &str) -> Result<(), Infallible> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn osu(events: &str) -> Vec<u8> {
        format!(
            "osu file format v14\n\n[General]\nMode: 3\n\n[Events]\n{events}\n\n\
             [TimingPoints]\n0,500,4,1,0,100,1,0\n"
        )
        .into_bytes()
    }

    #[test]
    fn background_name_reads_the_first_background_event() {
        let quoted = osu("//Background and Video events\n0,0,\"bg one.jpg\",0,0\n");
        assert_eq!(background_name(&quoted).as_deref(), Some("bg one.jpg"));
        let bare = osu("0,0,BG.PNG,0,0");
        assert_eq!(background_name(&bare).as_deref(), Some("BG.PNG"));
        let named = osu("Background,0,\"sub\\dir\\bg.jpg\"");
        assert_eq!(background_name(&named).as_deref(), Some("sub/dir/bg.jpg"));
        let after_video =
            osu("Video,0,\"clip.mp4\"\n1,0,\"other.avi\"\n0,0,\"bg.jpg\",0,0\n0,0,\"second.jpg\"");
        assert_eq!(background_name(&after_video).as_deref(), Some("bg.jpg"));
    }

    #[test]
    fn background_name_survives_bom_and_crlf() {
        let mut bytes = b"\xef\xbb\xbf".to_vec();
        bytes.extend(
            "osu file format v14\r\n\r\n[Events]\r\n0,0,\"bg.jpg\",0,0\r\n\r\n[HitObjects]\r\n"
                .as_bytes(),
        );
        assert_eq!(background_name(&bytes).as_deref(), Some("bg.jpg"));
    }

    #[test]
    fn background_name_is_none_without_a_usable_background() {
        assert_eq!(background_name(&osu("")), None);
        assert_eq!(background_name(&osu("Video,0,\"clip.mp4\"")), None);
        assert_eq!(background_name(&osu("0,0,\"\",0,0")), None);
        assert_eq!(background_name(&osu("0,0")), None);
        assert_eq!(
            background_name(&osu("Sprite,Background,Centre,\"sb.png\",320,240")),
            None
        );
        assert_eq!(background_name(b"not an osu file"), None);
        let elsewhere = b"osu file format v14\n\n[General]\n0,0,\"bg.jpg\",0,0\n".to_vec();
        assert_eq!(background_name(&elsewhere), None);
    }
}
