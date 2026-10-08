//! The pure half of a plan: rewrite the chart in memory and decide whether the copy may be
//! written. No IO here; the service reads the files and the MSD status.

use wolluf_engine::stage::chart_parse::{ParsedChart, parse_chart};
use wolluf_engine::{DrillError, RateCopy, note_rows, rate_copy};

use super::dto::refusal;
use super::params::RateCopiesParams;

#[derive(Debug)]
pub(super) struct Assessment {
    /// `None` when the rewriter refused the chart.
    pub(super) copy: Option<RateCopy>,
    pub(super) refusal: Option<&'static str>,
}

/// `ln_heavy` is the chart's MSD status when it has one; `None` falls back to the same cut-off
/// over the file itself.
pub(super) fn assess(
    osu: &[u8],
    rate_milli: u16,
    ln_heavy: Option<bool>,
    params: &RateCopiesParams,
) -> Assessment {
    let refused = |id| Assessment {
        copy: None,
        refusal: Some(id),
    };
    let copy = match rate_copy(osu, rate_milli, &params.rewrite) {
        Ok(copy) => copy,
        Err(e) => return refused(drill_refusal(&e)),
    };
    // The rewriter matches literal `Sample` events only; one behind a variable is kept as is.
    if has_storyboard_samples(osu) {
        return refused(refusal::KEYSOUNDED);
    }
    let Ok(source) = parse_chart(osu) else {
        return refused(refusal::MALFORMED);
    };
    let ln_heavy = ln_heavy.unwrap_or_else(|| {
        note_rows(&source.chart).hold_share_permille >= params.ln_heavy_hold_share_permille
    });
    let refusal = if ln_heavy {
        Some(refusal::LN_HEAVY)
    } else {
        match parse_chart(&copy.osu) {
            Err(_) => Some(refusal::MALFORMED),
            Ok(rewritten)
                if collisions(&rewritten, params.collision_window_us)
                    > collisions(&source, params.collision_window_us) =>
            {
                Some(refusal::SAME_COLUMN_COLLISION)
            }
            Ok(_) => None,
        }
    };
    Assessment {
        copy: Some(copy),
        refusal,
    }
}

const fn drill_refusal(e: &DrillError) -> &'static str {
    match e {
        DrillError::UnsupportedMode => refusal::UNSUPPORTED_MODE,
        DrillError::Keysounded => refusal::KEYSOUNDED,
        DrillError::NoAudio => refusal::NO_AUDIO,
        DrillError::Malformed { .. } => refusal::MALFORMED,
        DrillError::RateOutOfRange { .. } => refusal::RATE_OUT_OF_RANGE,
        DrillError::IdentityRate => refusal::IDENTITY_RATE,
        DrillError::AlreadyRateCopy => refusal::ALREADY_RATE_COPY,
    }
}

/// Decoder diagnostics for objects it had to drop or turn into taps: what a merge looks like
/// once two objects share a millisecond.
const MERGE_DIAGNOSTICS: [&str; 3] = [
    "chart.duplicate_note",
    "chart.overlapping_note",
    "chart.ln_tail_not_after_head",
];

/// Objects of one column within `window_us` of the previous one, plus the merges the decoder
/// reported. Compared between source and copy, so a source's own oddities never refuse it.
fn collisions(parsed: &ParsedChart, window_us: i64) -> usize {
    let merged = parsed
        .diagnostics
        .iter()
        .filter(|d| MERGE_DIAGNOSTICS.contains(&d.code.as_str()))
        .count();
    let chart = &parsed.chart;
    let close: usize = (0..chart.keymode().columns())
        .map(|col| {
            let starts = chart
                .rows()
                .iter()
                .filter(|r| r.tap.contains(col) || r.ln_head.contains(col))
                .map(|r| r.t.0);
            let mut count = 0;
            let mut prev: Option<i64> = None;
            for t in starts {
                if prev.is_some_and(|p| t - p <= window_us) {
                    count += 1;
                }
                prev = Some(t);
            }
            count
        })
        .sum();
    merged + close
}

/// Whether a storyboard (an `.osb`, or a `.osu`'s `[Events]`) plays samples: stable mixes those
/// into the song, and the copy's stretched audio would leave them at the old times.
pub(super) fn has_storyboard_samples(storyboard: &[u8]) -> bool {
    let storyboard = storyboard
        .strip_prefix(b"\xEF\xBB\xBF")
        .unwrap_or(storyboard);
    let lines = || storyboard.split(|&b| b == b'\n' || b == b'\r');
    // stable expands with the variables read so far; the full set also covers a `[Variables]`
    // section placed after the events, should a reader collect them first.
    let mut all = Vec::new();
    let _ = scan(lines(), &mut all, |_, _| false);
    let mut so_far = Vec::new();
    scan(lines(), &mut so_far, |line, so_far| {
        is_sample(line, so_far) || is_sample(line, &all)
    })
}

type Variables = Vec<(Vec<u8>, Vec<u8>)>;

/// Walks the sections, keeping `variables` up to date, until `event` holds for an event line.
fn scan<'a>(
    lines: impl Iterator<Item = &'a [u8]>,
    variables: &mut Variables,
    event: impl Fn(&[u8], &Variables) -> bool,
) -> bool {
    // stable reads a storyboard without a section header as events.
    let mut section: &[u8] = b"[Events]";
    for line in lines {
        let trimmed = line.trim_ascii();
        if trimmed.starts_with(b"[") {
            section = trimmed;
            continue;
        }
        if section.eq_ignore_ascii_case(b"[Variables]") {
            if let Some(eq) = trimmed.iter().position(|&b| b == b'=') {
                let key = trimmed[..eq].trim_ascii();
                let value = trimmed[eq + 1..].trim_ascii();
                if key.is_empty() {
                    continue;
                }
                match variables.iter_mut().find(|(k, _)| k == key) {
                    Some((_, v)) => *v = value.to_vec(),
                    None => variables.push((key.to_vec(), value.to_vec())),
                }
            }
            continue;
        }
        // Indented lines are commands of the object above; depth is read before expansion.
        let command = line.starts_with(b" ") || line.starts_with(b"_");
        if section.eq_ignore_ascii_case(b"[Events]") && !command && event(trimmed, variables) {
            return true;
        }
    }
    false
}

/// Expansion rounds and the size an expanded line may grow to; past either the variables do
/// not settle and the line cannot be shown to be safe.
const MAX_EXPANSION_ROUNDS: usize = 16;
const MAX_EXPANDED_LINE: usize = 64 * 1024;

fn is_sample(line: &[u8], variables: &Variables) -> bool {
    let Some(line) = expand(line, variables) else {
        return true;
    };
    let kind = line.split(|&b| b == b',').next().unwrap_or_default();
    let kind = kind.trim_ascii();
    kind == b"Sample" || kind == b"5"
}

/// stable's substitution: every variable replaced in order, repeated until nothing changes.
fn expand(line: &[u8], variables: &Variables) -> Option<Vec<u8>> {
    let mut line = line.to_vec();
    for _ in 0..MAX_EXPANSION_ROUNDS {
        if !line.contains(&b'$') {
            return Some(line);
        }
        let before = line.clone();
        for (key, value) in variables {
            line = replace_all(&line, key, value);
            if line.len() > MAX_EXPANDED_LINE {
                return None;
            }
        }
        if line == before {
            return Some(line);
        }
    }
    None
}

fn replace_all(hay: &[u8], from: &[u8], to: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(hay.len());
    let mut rest = hay;
    while let Some(at) = rest.windows(from.len()).position(|w| w == from) {
        out.extend_from_slice(&rest[..at]);
        out.extend_from_slice(to);
        rest = &rest[at + from.len()..];
    }
    out.extend_from_slice(rest);
    out
}

/// `AudioFilename` of a `.osu`'s `[General]` section.
pub(super) fn audio_filename(osu: &[u8]) -> Option<String> {
    let osu = osu.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(osu);
    let mut general = false;
    for line in osu.split(|&b| b == b'\n' || b == b'\r') {
        let trimmed = line.trim_ascii();
        if trimmed.starts_with(b"[") {
            general = trimmed == b"[General]";
            continue;
        }
        let Some(colon) = trimmed.iter().position(|&b| b == b':') else {
            continue;
        };
        if general && trimmed[..colon].trim_ascii() == b"AudioFilename" {
            let value = trimmed[colon + 1..].trim_ascii();
            return Some(String::from_utf8_lossy(value).into_owned());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::super::testkit::OsuChart;
    use super::*;

    fn params() -> RateCopiesParams {
        RateCopiesParams::default()
    }

    fn refusal_of(
        chart: &OsuChart<'_>,
        rate_milli: u16,
        ln_heavy: Option<bool>,
    ) -> Option<&'static str> {
        assess(&chart.bytes(), rate_milli, ln_heavy, &params()).refusal
    }

    #[test]
    fn a_plain_chart_gets_its_copy_names() {
        let a = assess(&OsuChart::k7("T").bytes(), 1150, Some(false), &params());
        assert_eq!(a.refusal, None);
        let copy = a.copy.unwrap();
        assert_eq!(copy.version, "Normal 1.15x (138bpm)");
        assert_eq!(
            copy.osu_filename,
            "wolluf - T (wolluf) [Normal 1.15x (138bpm)].osu"
        );
        assert_eq!(copy.audio_filename, "audio 1.15x.ogg");
        assert_eq!(copy.source_audio, "audio.wav");
    }

    #[test]
    fn rewriter_refusals_map_to_stable_ids() {
        let k7 = OsuChart::k7("T");
        assert_eq!(
            refusal_of(&k7, 1000, Some(false)),
            Some(refusal::IDENTITY_RATE)
        );
        assert_eq!(
            refusal_of(&k7, 2500, Some(false)),
            Some(refusal::RATE_OUT_OF_RANGE)
        );
        assert_eq!(
            refusal_of(&k7, 400, Some(false)),
            Some(refusal::RATE_OUT_OF_RANGE)
        );
        let copy = OsuChart {
            tags: "dan wofella",
            ..OsuChart::k7("T")
        };
        assert_eq!(
            refusal_of(&copy, 1100, Some(false)),
            Some(refusal::ALREADY_RATE_COPY)
        );
        let keysounded = OsuChart {
            tap_sample_file: "kick.wav",
            ..OsuChart::k7("T")
        };
        assert_eq!(
            refusal_of(&keysounded, 1100, Some(false)),
            Some(refusal::KEYSOUNDED)
        );
        let storyboard = OsuChart {
            events: &["Sample,1000,0,\"kick.wav\",70"],
            ..OsuChart::k7("T")
        };
        assert_eq!(
            refusal_of(&storyboard, 1100, Some(false)),
            Some(refusal::KEYSOUNDED)
        );
        let silent = OsuChart {
            audio: "",
            ..OsuChart::k7("T")
        };
        assert_eq!(
            refusal_of(&silent, 1100, Some(false)),
            Some(refusal::NO_AUDIO)
        );
        let refused = assess(&k7.bytes(), 1000, Some(false), &params());
        assert!(refused.copy.is_none());
    }

    #[test]
    fn ln_heavy_follows_the_msd_status_then_the_file() {
        assert_eq!(
            refusal_of(&OsuChart::k7("T"), 1100, Some(true)),
            Some(refusal::LN_HEAVY)
        );
        let ln = OsuChart::ln_heavy("L");
        assert_eq!(refusal_of(&ln, 1100, None), Some(refusal::LN_HEAVY));
        assert_eq!(refusal_of(&OsuChart::k7("T"), 1100, None), None);
    }

    #[test]
    fn copies_that_merge_objects_of_one_column_are_refused() {
        let close = OsuChart {
            taps: vec![(0, 1_000), (0, 1_002), (1, 1_500)],
            ..OsuChart::k7("C")
        };
        // 1000/2 and 1002/2 land 1 ms apart; at 1.10x they stay 2 ms apart.
        assert_eq!(
            refusal_of(&close, 2000, Some(false)),
            Some(refusal::SAME_COLUMN_COLLISION)
        );
        assert_eq!(refusal_of(&close, 1100, Some(false)), None);
        // A tail 1 ms before a tap may share its millisecond in the copy: 999/2 and 1000/2.
        let hold_into_tap = OsuChart {
            taps: vec![(0, 1_000)],
            holds: vec![(0, 500, 999)],
            ..OsuChart::k7("H")
        };
        assert_eq!(
            refusal_of(&hold_into_tap, 2000, Some(false)),
            Some(refusal::SAME_COLUMN_COLLISION)
        );
        // Collisions the source already has are not the copy's doing.
        let source_dup = OsuChart {
            taps: vec![(0, 1_000), (0, 1_000), (1, 1_500)],
            ..OsuChart::k7("D")
        };
        assert_eq!(refusal_of(&source_dup, 1100, Some(false)), None);
        let source_close = OsuChart {
            taps: vec![(0, 999), (0, 1_000), (1, 1_500)],
            ..OsuChart::k7("D")
        };
        assert_eq!(refusal_of(&source_close, 2000, Some(false)), None);
    }

    #[test]
    fn osb_samples_are_found_in_the_events_section_only() {
        assert!(has_storyboard_samples(
            b"[Events]\r\nSample,0,0,\"a.wav\",100\r\n"
        ));
        assert!(has_storyboard_samples(
            b"\xEF\xBB\xBF[Events]\n//c\n5,250,0,\"a.wav\"\n"
        ));
        assert!(has_storyboard_samples(b"Sample,0,0,\"a.wav\",100"));
        assert!(!has_storyboard_samples(
            b"[Events]\r\nSprite,Foreground,Centre,\"Sample.png\",320,240\r\n M,0,0,1000,0,0\r\n"
        ));
        assert!(!has_storyboard_samples(
            b"[Variables]\r\n$s=Sample,0\r\n[Events]\r\n"
        ));
        assert!(!has_storyboard_samples(b""));
    }

    #[test]
    fn storyboard_variables_are_expanded_before_matching() {
        assert!(has_storyboard_samples(
            b"[Variables]\r\n$s=Sample\r\n[Events]\r\n$s,0,0,\"a.wav\",100\r\n"
        ));
        assert!(has_storyboard_samples(
            b"[Variables]\n$k=5,250\n[Events]\n$k,0,\"a.wav\"\n"
        ));
        // Values may name other variables; stable expands until nothing changes.
        assert!(has_storyboard_samples(
            b"[Variables]\n$a=$b\n$b=Sample\n[Events]\n$a,0,0,\"a.wav\"\n"
        ));
        // A value that never settles cannot be shown to be safe.
        assert!(has_storyboard_samples(
            b"[Variables]\n$a=x$a\n[Events]\n$a,0,0\n"
        ));
        assert!(!has_storyboard_samples(
            b"[Variables]\n$s=Sample\n[Events]\nSprite,Foreground,Centre,\"$s.png\",320,240\n"
        ));
        assert!(!has_storyboard_samples(
            b"[Variables]\n$p=Sprite\n[Events]\n$p,Foreground,Centre,\"a.png\",320,240\n"
        ));
        assert!(!has_storyboard_samples(
            b"[Variables]\n$s=Sample\n[Events]\nSprite,Foreground,Centre,\"a.png\",0,0\n $s,0\n"
        ));
    }

    #[test]
    fn chart_storyboard_variables_are_keysounded() {
        let chart = OsuChart {
            variables: &["$s=Sample"],
            events: &["$s,1000,0,\"kick.wav\",70"],
            ..OsuChart::k7("T")
        };
        assert_eq!(
            refusal_of(&chart, 1100, Some(false)),
            Some(refusal::KEYSOUNDED)
        );
        let harmless = OsuChart {
            variables: &["$s=Sample"],
            events: &["Sprite,Foreground,Centre,\"$s.png\",320,240"],
            ..OsuChart::k7("T")
        };
        assert_eq!(refusal_of(&harmless, 1100, Some(false)), None);
    }

    #[test]
    fn audio_filename_is_read_from_the_general_section() {
        assert_eq!(
            audio_filename(b"osu file format v14\r\n[General]\r\nAudioFilename:  a 1.10x.ogg \r\n"),
            Some("a 1.10x.ogg".to_owned())
        );
        assert_eq!(
            audio_filename(b"\xEF\xBB\xBF[Metadata]\nAudioFilename: x.ogg\n[General]\nMode: 3\n"),
            None
        );
        assert_eq!(audio_filename(b""), None);
    }
}
