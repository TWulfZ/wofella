//! Which catalog chart osu! is playing: stable's window title names it as
//! `Artist - Title [Difficulty]`; the service falls back to the newest self replay.

use crate::features::library::dto::LibraryChartDto;

/// D17: how far the replay fallback looks, not an inline number.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NowPlayingParams {
    /// `.osr` headers read, newest first, before giving up on finding a self replay; each read
    /// is a file open, slow on a WSL-mounted install.
    pub max_replay_headers: usize,
}

impl Default for NowPlayingParams {
    fn default() -> Self {
        Self {
            max_replay_headers: 64,
        }
    }
}

/// Catalog charts whose `{artist} - {title} [{version}]` is `wanted`, comparing with runs of
/// whitespace collapsed. Exact-case matches win; only without one do case-insensitive matches
/// count. More than one result is ambiguous and left to the caller.
pub fn title_matches<'c>(wanted: &str, charts: &'c [LibraryChartDto]) -> Vec<&'c LibraryChartDto> {
    let wanted = normalise(wanted);
    let named: Vec<(String, &LibraryChartDto)> = charts
        .iter()
        .map(|c| {
            let name = format!("{} - {} [{}]", c.artist, c.title, c.version);
            (normalise(&name), c)
        })
        .collect();
    let exact: Vec<&LibraryChartDto> = named
        .iter()
        .filter(|(name, _)| *name == wanted)
        .map(|(_, c)| *c)
        .collect();
    if !exact.is_empty() {
        return exact;
    }
    let wanted = wanted.to_lowercase();
    named
        .iter()
        .filter(|(name, _)| name.to_lowercase() == wanted)
        .map(|(_, c)| *c)
        .collect()
}

/// Collapsed runs of whitespace, none just inside the version brackets: osu!.db keeps stray
/// spaces such as `[Hard ]` that the window title may or may not carry.
fn normalise(s: &str) -> String {
    s.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .replace("[ ", "[")
        .replace(" ]", "]")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chart(md5: &str, artist: &str, title: &str, version: &str) -> LibraryChartDto {
        LibraryChartDto {
            md5: md5.to_owned(),
            title: title.to_owned(),
            artist: artist.to_owned(),
            version: version.to_owned(),
            creator: String::new(),
            keymode: 7,
            n_notes: 1,
            n_ln: 0,
            ln_ratio: 0.0,
            length_ms: 1,
            nps: 1.0,
            stars: None,
            msd_overall_centi: None,
            labels: Vec::new(),
        }
    }

    fn md5s(found: &[&LibraryChartDto]) -> Vec<String> {
        found.iter().map(|c| c.md5.clone()).collect()
    }

    #[test]
    fn titles_match_on_artist_title_and_version() {
        let charts = [
            chart("a", "Camellia", "Exit", "7K Insane"),
            chart("b", "Camellia", "Exit", "7K Hard"),
            chart("c", "Camellia", "Exit - Extended", "7K Insane"),
        ];
        assert_eq!(
            md5s(&title_matches("Camellia - Exit [7K Insane]", &charts)),
            ["a"]
        );
        assert_eq!(
            md5s(&title_matches(
                "Camellia - Exit - Extended [7K Insane]",
                &charts
            )),
            ["c"]
        );
        assert!(title_matches("Camellia - Exit [7K Easy]", &charts).is_empty());
    }

    #[test]
    fn whitespace_is_normalised_and_exact_case_wins() {
        let charts = [
            chart("a", "DJ  Noriken", "Kick", "Hard "),
            chart("b", "artist", "Eight", "x"),
            chart("c", "artist", "eight", "x"),
        ];
        assert_eq!(
            md5s(&title_matches(" DJ Noriken -  Kick [Hard]", &charts)),
            ["a"]
        );
        assert_eq!(md5s(&title_matches("artist - eight [x]", &charts)), ["c"]);
        assert_eq!(
            md5s(&title_matches("ARTIST - EIGHT [X]", &charts)),
            ["b", "c"],
            "without an exact match both case-insensitive ones are candidates"
        );
    }
}
