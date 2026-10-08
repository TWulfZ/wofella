#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use proptest::prelude::*;
use wolluf_drills::{DrillError, RateCopyParams, rate_copy};

const FIXTURE: &str = "osu file format v14

[General]
AudioFilename: audio.mp3
AudioLeadIn: 0
PreviewTime: 12345
Countdown: 0
SampleSet: Soft
StackLeniency: 0.7
Mode: 3
LetterboxInBreaks: 0
SpecialStyle: 0
WidescreenStoryboard: 0

[Editor]
Bookmarks: 1000,2001,30000
DistanceSpacing: 1
BeatDivisor: 4
GridSize: 4
TimelineZoom: 1

[Metadata]
Title:Synthetic Song
TitleUnicode:Synthetic Song
Artist:Test Artist
ArtistUnicode:Test Artist
Creator:fixture-mapper
Version:Insane
Source:
Tags:synthetic test
BeatmapID:123456
BeatmapSetID:7890

[Difficulty]
HPDrainRate:8
CircleSize:7
OverallDifficulty:8
ApproachRate:5
SliderMultiplier:1.4
SliderTickRate:1

[Events]
//Background and Video events
0,0,\"bg.jpg\",0,0
Video,500,\"video.mp4\"
//Break Periods
2,10000,15000
//Storyboard Layer 0 (Background)
//Storyboard Layer 1 (Fail)
//Storyboard Layer 2 (Pass)
//Storyboard Layer 3 (Foreground)
//Storyboard Sound Samples

[TimingPoints]
-30,300,4,2,1,60,1,0
1000.5,-50,4,2,1,60,0,0
20000,250,4,2,1,60,1,1

[HitObjects]
36,192,1000,1,0,0:0:0:0:
109,192,1001,1,0,0:0:0:0:
182,192,2000,128,0,2500:0:0:0:0:
256,192,3000,1,2,3:2:0:40:
329,192,3001,128,0,4000:1:2:0:50:
";

fn params() -> RateCopyParams {
    RateCopyParams::default()
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8(bytes.to_vec()).unwrap()
}

/// The non-blank, non-comment lines of one section, terminators stripped.
fn section(osu: &str, name: &str) -> Vec<String> {
    let header = format!("[{name}]");
    let mut inside = false;
    let mut out = Vec::new();
    for line in osu.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            inside = trimmed == header;
            continue;
        }
        if inside && !trimmed.is_empty() && !trimmed.starts_with("//") {
            out.push(line.to_owned());
        }
    }
    out
}

fn replace_line(src: &str, from: &str, to: &str) -> String {
    assert!(src.contains(from), "fixture has no line {from:?}");
    src.replacen(from, to, 1)
}

#[test]
fn rate_1150_rewrites_every_time_field() {
    let copy = rate_copy(FIXTURE.as_bytes(), 1150, &params()).unwrap();
    let out = text(&copy.osu);

    assert_eq!(
        section(&out, "General"),
        [
            "AudioFilename: audio 1.15x.ogg",
            "AudioLeadIn: 0",
            "PreviewTime: 10735",
            "Countdown: 0",
            "SampleSet: Soft",
            "StackLeniency: 0.7",
            "Mode: 3",
            "LetterboxInBreaks: 0",
            "SpecialStyle: 0",
            "WidescreenStoryboard: 0",
        ]
    );
    assert_eq!(section(&out, "Editor")[0], "Bookmarks: 870,1740,26087");
    assert_eq!(
        section(&out, "Metadata"),
        [
            "Title:Synthetic Song",
            "TitleUnicode:Synthetic Song",
            "Artist:Test Artist",
            "ArtistUnicode:Test Artist",
            "Creator:fixture-mapper",
            "Version:Insane 1.15x (230bpm)",
            "Source:",
            "Tags:synthetic test wofella",
            "BeatmapID:0",
            "BeatmapSetID:7890",
        ]
    );
    assert_eq!(section(&out, "Difficulty"), section(FIXTURE, "Difficulty"));
    assert_eq!(
        section(&out, "Events"),
        ["0,0,\"bg.jpg\",0,0", "2,8696,13043"]
    );
    assert_eq!(
        section(&out, "TimingPoints"),
        [
            "-26.087,260.8695652173913,4,2,1,60,1,0",
            "870,-50,4,2,1,60,0,0",
            "17391.304,217.3913043478261,4,2,1,60,1,1",
        ]
    );
    assert_eq!(
        section(&out, "HitObjects"),
        [
            "36,192,870,1,0,0:0:0:0:",
            "109,192,870,1,0,0:0:0:0:",
            "182,192,1739,128,0,2174:0:0:0:0:",
            "256,192,2609,1,2,3:2:0:40:",
            "329,192,2610,128,0,3478:1:2:0:50:",
        ]
    );

    assert_eq!(copy.version, "Insane 1.15x (230bpm)");
    assert_eq!(copy.audio_filename, "audio 1.15x.ogg");
    assert_eq!(copy.source_audio, "audio.mp3");
    assert_eq!(
        copy.osu_filename,
        "Test Artist - Synthetic Song (fixture-mapper) [Insane 1.15x (230bpm)].osu"
    );
}

#[test]
fn rate_850_rewrites_every_time_field() {
    let copy = rate_copy(FIXTURE.as_bytes(), 850, &params()).unwrap();
    let out = text(&copy.osu);

    assert!(out.contains("\nPreviewTime: 14524\n"));
    assert_eq!(section(&out, "Editor")[0], "Bookmarks: 1176,2354,35294");
    assert_eq!(copy.version, "Insane 0.85x (170bpm)");
    assert_eq!(copy.audio_filename, "audio 0.85x.ogg");
    assert_eq!(
        section(&out, "Events"),
        ["0,0,\"bg.jpg\",0,0", "2,11765,17647"]
    );
    assert_eq!(
        section(&out, "TimingPoints"),
        [
            "-35.294,352.94117647058823,4,2,1,60,1,0",
            "1177.059,-50,4,2,1,60,0,0",
            "23529.412,294.11764705882354,4,2,1,60,1,1",
        ]
    );
    assert_eq!(
        section(&out, "HitObjects"),
        [
            "36,192,1176,1,0,0:0:0:0:",
            "109,192,1178,1,0,0:0:0:0:",
            "182,192,2353,128,0,2941:0:0:0:0:",
            // Companella rewrites `3` here as if it were an LN tail; it must stay byte-identical.
            "256,192,3529,1,2,3:2:0:40:",
            "329,192,3531,128,0,4706:1:2:0:50:",
        ]
    );
}

#[test]
fn only_time_fields_and_copy_metadata_differ_from_the_input() {
    let copy = rate_copy(FIXTURE.as_bytes(), 1150, &params()).unwrap();
    let out = text(&copy.osu);
    let changed: Vec<(&str, &str)> = FIXTURE
        .lines()
        .filter(|l| !l.starts_with("Video,"))
        .zip(out.lines())
        .filter(|(a, b)| a != b)
        .collect();
    assert_eq!(
        changed,
        [
            ("AudioFilename: audio.mp3", "AudioFilename: audio 1.15x.ogg"),
            ("PreviewTime: 12345", "PreviewTime: 10735"),
            ("Bookmarks: 1000,2001,30000", "Bookmarks: 870,1740,26087"),
            ("Version:Insane", "Version:Insane 1.15x (230bpm)"),
            ("Tags:synthetic test", "Tags:synthetic test wofella"),
            ("BeatmapID:123456", "BeatmapID:0"),
            ("2,10000,15000", "2,8696,13043"),
            (
                "-30,300,4,2,1,60,1,0",
                "-26.087,260.8695652173913,4,2,1,60,1,0"
            ),
            ("1000.5,-50,4,2,1,60,0,0", "870,-50,4,2,1,60,0,0"),
            (
                "20000,250,4,2,1,60,1,1",
                "17391.304,217.3913043478261,4,2,1,60,1,1"
            ),
            ("36,192,1000,1,0,0:0:0:0:", "36,192,870,1,0,0:0:0:0:"),
            ("109,192,1001,1,0,0:0:0:0:", "109,192,870,1,0,0:0:0:0:"),
            (
                "182,192,2000,128,0,2500:0:0:0:0:",
                "182,192,1739,128,0,2174:0:0:0:0:"
            ),
            ("256,192,3000,1,2,3:2:0:40:", "256,192,2609,1,2,3:2:0:40:"),
            (
                "329,192,3001,128,0,4000:1:2:0:50:",
                "329,192,2610,128,0,3478:1:2:0:50:"
            ),
        ]
    );
    assert_eq!(out.lines().count(), FIXTURE.lines().count() - 1);
    assert!(out.ends_with('\n'));
}

#[test]
fn refuses_the_identity_rate() {
    assert_eq!(
        rate_copy(FIXTURE.as_bytes(), 1000, &params()),
        Err(DrillError::IdentityRate)
    );
}

#[test]
fn refuses_a_chart_that_is_already_a_rate_copy() {
    let src = replace_line(FIXTURE, "Tags:synthetic test", "Tags:wofella synthetic");
    assert_eq!(
        rate_copy(src.as_bytes(), 1150, &params()),
        Err(DrillError::AlreadyRateCopy)
    );
    // Only a whole tag marks a copy.
    let src = replace_line(FIXTURE, "Tags:synthetic test", "Tags:wofellas  synthetic");
    assert!(rate_copy(src.as_bytes(), 1150, &params()).is_ok());
}

#[test]
fn timing_offsets_keep_the_source_decimals_beyond_three() {
    let src = replace_line(FIXTURE, "1000.5,-50,", "1000.12345,-50,");
    let copy = rate_copy(src.as_bytes(), 1250, &params()).unwrap();
    assert_eq!(
        section(&text(&copy.osu), "TimingPoints")[1],
        "800.09876,-50,4,2,1,60,0,0"
    );
}

#[test]
fn bookmarks_skip_empty_entries() {
    let src = replace_line(
        FIXTURE,
        "Bookmarks: 1000,2001,30000",
        "Bookmarks: 1000,,2001,",
    );
    let copy = rate_copy(src.as_bytes(), 1150, &params()).unwrap();
    assert_eq!(
        section(&text(&copy.osu), "Editor")[0],
        "Bookmarks: 870,1740"
    );
}

#[test]
fn unparseable_bookmarks_are_kept_unchanged() {
    let src = replace_line(FIXTURE, "Bookmarks: 1000,2001,30000", "Bookmarks: 1000,abc");
    let copy = rate_copy(src.as_bytes(), 1150, &params()).unwrap();
    assert_eq!(
        section(&text(&copy.osu), "Editor")[0],
        "Bookmarks: 1000,abc"
    );
}

#[test]
fn non_utf8_event_paths_survive() {
    let mut src = replace_line(
        FIXTURE,
        "//Storyboard Layer 0 (Background)\n",
        "//Storyboard Layer 0 (Background)\n\
         Animation,Foreground,Centre,\"sb/@.png\",320,240,4,100,LoopForever\n",
    )
    .into_bytes();
    let at = src.iter().position(|&b| b == b'@').unwrap();
    src[at] = 0xE9;
    let src = {
        let bg = b"0,0,\"bg";
        let at = src.windows(bg.len()).position(|w| w == bg).unwrap() + bg.len();
        [&src[..at], &[0xE9u8][..], &src[at..]].concat()
    };
    let copy = rate_copy(&src, 1250, &params()).unwrap();
    for needle in [
        &b"\n0,0,\"bg\xE9.jpg\",0,0\n"[..],
        b"\nAnimation,Foreground,Centre,\"sb/\xE9.png\",320,240,4,80,LoopForever\n",
    ] {
        assert!(
            copy.osu.windows(needle.len()).any(|w| w == needle),
            "{}",
            String::from_utf8_lossy(needle)
        );
    }
}

#[test]
fn audio_base_name_splits_only_the_file_name() {
    for (from, to) in [
        ("my.dir/song", "my.dir/song 1.15x.ogg"),
        ("my.dir\\song.mp3", "my.dir\\song 1.15x.ogg"),
        ("a.b.mp3", "a.b 1.15x.ogg"),
    ] {
        let src = replace_line(
            FIXTURE,
            "AudioFilename: audio.mp3",
            &format!("AudioFilename: {from}"),
        );
        let copy = rate_copy(src.as_bytes(), 1150, &params()).unwrap();
        assert_eq!(copy.audio_filename, to);
        assert_eq!(copy.source_audio, from);
    }
}

#[test]
fn cr_only_line_endings_are_read_like_stable() {
    let src = FIXTURE.replace('\n', "\r");
    let copy = rate_copy(src.as_bytes(), 1150, &params()).unwrap();
    let out = text(&copy.osu);
    assert!(!out.contains('\n'));
    assert!(out.contains("\rVersion:Insane 1.15x (230bpm)\r"));
    assert!(out.contains("\r182,192,1739,128,0,2174:0:0:0:0:\r"));
    assert!(out.ends_with('\r'));
}

#[test]
fn rate_label_keeps_two_decimals_and_a_third_only_when_needed() {
    let copy = rate_copy(FIXTURE.as_bytes(), 1100, &params()).unwrap();
    assert_eq!(copy.audio_filename, "audio 1.10x.ogg");
    assert_eq!(copy.version, "Insane 1.10x (220bpm)");
    // Two grid rates must never share an audio name, or the export would reuse the wrong file.
    let copy = rate_copy(FIXTURE.as_bytes(), 1155, &params()).unwrap();
    assert_eq!(copy.audio_filename, "audio 1.155x.ogg");
}

#[test]
fn times_round_half_up_from_the_original_value() {
    let src = replace_line(
        FIXTURE,
        "36,192,1000,1,0,0:0:0:0:",
        "36,192,2,1,0,0:0:0:0:\n36,192,-2,1,0,0:0:0:0:",
    );
    let src = replace_line(&src, "1000.5,-50,", "-2,-50,");
    let copy = rate_copy(src.as_bytes(), 800, &params()).unwrap();
    let out = text(&copy.osu);
    let hits = section(&out, "HitObjects");
    // 2 / 0.8 = 2.5 rounds up to 3; -2 / 0.8 = -2.5 rounds up to -2.
    assert_eq!(hits[0], "36,192,3,1,0,0:0:0:0:");
    assert_eq!(hits[1], "36,192,-2,1,0,0:0:0:0:");
    assert_eq!(section(&out, "TimingPoints")[1], "-2.5,-50,4,2,1,60,0,0");
}

#[test]
fn ln_tail_is_scaled_only_with_type_bit_128() {
    let src = replace_line(
        FIXTURE,
        "36,192,1000,1,0,0:0:0:0:",
        "36,192,1000,1,0,2000:0:0:0:\n36,192,1000,129,0,2000:0:0:0:0:",
    );
    let copy = rate_copy(src.as_bytes(), 1250, &params()).unwrap();
    let hits = section(&text(&copy.osu), "HitObjects");
    assert_eq!(hits[0], "36,192,800,1,0,2000:0:0:0:");
    assert_eq!(hits[1], "36,192,800,129,0,1600:0:0:0:0:");
}

#[test]
fn notes_without_hit_sample_and_ln_without_sample_are_kept() {
    let src = replace_line(
        FIXTURE,
        "36,192,1000,1,0,0:0:0:0:",
        "36,192,1000,1,0\n36,192,1000,128,0,2000",
    );
    let copy = rate_copy(src.as_bytes(), 1250, &params()).unwrap();
    let hits = section(&text(&copy.osu), "HitObjects");
    assert_eq!(hits[0], "36,192,800,1,0");
    assert_eq!(hits[1], "36,192,800,128,0,1600");
}

#[test]
fn green_lines_keep_their_sv_and_old_red_lines_scale() {
    let src = replace_line(
        FIXTURE,
        "1000.5,-50,4,2,1,60,0,0",
        "1000.5,-50,4,2,1,60,0,0\n2000,500,4,2,1,60\n3000,-200",
    );
    let copy = rate_copy(src.as_bytes(), 1250, &params()).unwrap();
    let timing = section(&text(&copy.osu), "TimingPoints");
    assert_eq!(timing[1], "800.4,-50,4,2,1,60,0,0");
    // Pre-v6 lines carry no uninherited flag: a positive beat length is a red line.
    assert_eq!(timing[2], "1600,400,4,2,1,60");
    assert_eq!(timing[3], "2400,-200");
}

#[test]
fn preview_time_minus_one_stays() {
    let src = replace_line(FIXTURE, "PreviewTime: 12345", "PreviewTime: -1");
    let copy = rate_copy(src.as_bytes(), 1150, &params()).unwrap();
    assert!(text(&copy.osu).contains("\nPreviewTime: -1\n"));
}

#[test]
fn tags_are_created_when_absent() {
    let src = replace_line(FIXTURE, "Tags:synthetic test\n", "");
    let copy = rate_copy(src.as_bytes(), 1150, &params()).unwrap();
    let meta = section(&text(&copy.osu), "Metadata");
    assert_eq!(meta.last().unwrap(), "Tags:wofella");
}

#[test]
fn storyboard_commands_and_colour_events_scale() {
    let src = replace_line(
        FIXTURE,
        "//Storyboard Layer 0 (Background)\n",
        "//Storyboard Layer 0 (Background)\n\
         Sprite,Background,Centre,\"sb/a,b.png\",320,240\n \
         F,0,1000,2000,0,1\n \
         L,500,3\n  \
         M,0,100,,320,240\n \
         T,HitSoundClap,1000,2000\n\
         Animation,Foreground,Centre,\"sb/x,y.png\",320,240,4,100,LoopForever\n\
         _S,0,1000,,1\n\
         3,1000,163,162,255\n",
    );
    let copy = rate_copy(src.as_bytes(), 1250, &params()).unwrap();
    assert_eq!(
        section(&text(&copy.osu), "Events"),
        [
            "0,0,\"bg.jpg\",0,0",
            "2,8000,12000",
            "Sprite,Background,Centre,\"sb/a,b.png\",320,240",
            " F,0,800,1600,0,1",
            " L,400,3",
            "  M,0,80,,320,240",
            " T,HitSoundClap,800,1600",
            "Animation,Foreground,Centre,\"sb/x,y.png\",320,240,4,80,LoopForever",
            "_S,0,800,,1",
            "3,800,163,162,255",
        ]
    );
}

#[test]
fn filename_strips_characters_windows_rejects() {
    let src = replace_line(FIXTURE, "Artist:Test Artist", "Artist:AC/DC <live>");
    let src = replace_line(&src, "Title:Synthetic Song", "Title:Why? \"Yes\": a|b*\\");
    let copy = rate_copy(src.as_bytes(), 1150, &params()).unwrap();
    assert_eq!(
        copy.osu_filename,
        "ACDC live - Why Yes ab (fixture-mapper) [Insane 1.15x (230bpm)].osu"
    );
}

#[test]
fn crlf_and_bom_are_preserved() {
    let crlf = FIXTURE.replace('\n', "\r\n");
    let mut src = vec![0xEF, 0xBB, 0xBF];
    src.extend_from_slice(crlf.as_bytes());
    let copy = rate_copy(&src, 1150, &params()).unwrap();
    assert!(copy.osu.starts_with(&[0xEF, 0xBB, 0xBF]));
    let out = text(&copy.osu[3..]);
    assert!(out.starts_with("osu file format v14\r\n"));
    assert_eq!(out.matches('\n').count(), out.matches("\r\n").count());
    assert!(out.contains("\r\nVersion:Insane 1.15x (230bpm)\r\n"));
    assert!(out.contains("\r\n182,192,1739,128,0,2174:0:0:0:0:\r\n"));
}

#[test]
fn inserted_tags_line_uses_the_file_line_ending() {
    let src = replace_line(FIXTURE, "Tags:synthetic test\n", "").replace('\n', "\r\n");
    let copy = rate_copy(src.as_bytes(), 1150, &params()).unwrap();
    let out = text(&copy.osu);
    assert!(out.contains("\r\nBeatmapSetID:7890\r\nTags:wofella\r\n\r\n[Difficulty]"));
}

#[test]
fn non_utf8_metadata_bytes_survive() {
    let mut src = FIXTURE.as_bytes().to_vec();
    let at = FIXTURE.find("Version:Insane").unwrap() + "Version:".len();
    src.splice(at..at, [0xE9u8]);
    let copy = rate_copy(&src, 1150, &params()).unwrap();
    let needle: &[u8] = b"Version:\xE9Insane 1.15x (230bpm)";
    assert!(copy.osu.windows(needle.len()).any(|w| w == needle));
    assert_eq!(copy.version, "\u{FFFD}Insane 1.15x (230bpm)");
}

#[test]
fn refuses_other_modes() {
    let src = replace_line(FIXTURE, "Mode: 3", "Mode: 0");
    assert_eq!(
        rate_copy(src.as_bytes(), 1150, &params()),
        Err(DrillError::UnsupportedMode)
    );
    let src = replace_line(FIXTURE, "Mode: 3\n", "");
    assert_eq!(
        rate_copy(src.as_bytes(), 1150, &params()),
        Err(DrillError::UnsupportedMode)
    );
}

#[test]
fn refuses_keysounded_charts() {
    let cases = [
        (
            "//Storyboard Sound Samples",
            "//Storyboard Sound Samples\nSample,1000,0,\"kick.wav\",70",
        ),
        (
            "//Storyboard Sound Samples",
            "//Storyboard Sound Samples\n5,1000,0,\"kick.wav\",70",
        ),
        (
            "36,192,1000,1,0,0:0:0:0:",
            "36,192,1000,1,0,0:0:0:0:kick.wav",
        ),
        (
            "182,192,2000,128,0,2500:0:0:0:0:",
            "182,192,2000,128,0,2500:0:0:0:0:kick.wav",
        ),
    ];
    for (from, to) in cases {
        let src = replace_line(FIXTURE, from, to);
        assert_eq!(
            rate_copy(src.as_bytes(), 1150, &params()),
            Err(DrillError::Keysounded),
            "{to}"
        );
    }
}

#[test]
fn refuses_charts_without_audio() {
    for to in ["AudioFilename: ", "AudioFilename: virtual", ""] {
        let src = replace_line(FIXTURE, "AudioFilename: audio.mp3", to);
        assert_eq!(
            rate_copy(src.as_bytes(), 1150, &params()),
            Err(DrillError::NoAudio),
            "{to:?}"
        );
    }
}

#[test]
fn refuses_malformed_lines_with_their_line_number() {
    let line = FIXTURE
        .lines()
        .position(|l| l == "109,192,1001,1,0,0:0:0:0:")
        .unwrap()
        + 1;
    let src = replace_line(FIXTURE, "109,192,1001,", "109,192,abc,");
    assert_eq!(
        rate_copy(src.as_bytes(), 1150, &params()),
        Err(DrillError::Malformed { line })
    );

    let line = FIXTURE.lines().position(|l| l.starts_with("-30,")).unwrap() + 1;
    let src = replace_line(FIXTURE, "-30,300,", "-30,x,");
    assert_eq!(
        rate_copy(src.as_bytes(), 1150, &params()),
        Err(DrillError::Malformed { line })
    );

    let src = replace_line(FIXTURE, "182,192,2000,128,0,2500:", "182,192,2000,128,0,:");
    assert!(matches!(
        rate_copy(src.as_bytes(), 1150, &params()),
        Err(DrillError::Malformed { .. })
    ));
}

#[test]
fn refuses_charts_without_a_red_line() {
    let src = replace_line(FIXTURE, "-30,300,4,2,1,60,1,0\n", "");
    let src = replace_line(&src, "20000,250,4,2,1,60,1,1\n", "");
    assert!(matches!(
        rate_copy(src.as_bytes(), 1150, &params()),
        Err(DrillError::Malformed { .. })
    ));
}

#[test]
fn refuses_rates_outside_the_params_range() {
    for rate in [0, 499, 2001] {
        assert_eq!(
            rate_copy(FIXTURE.as_bytes(), rate, &params()),
            Err(DrillError::RateOutOfRange { rate_milli: rate })
        );
    }
}

#[test]
fn rounding_error_is_bounded_in_the_copy_time_base_not_the_source_one() {
    // 4 / 1.15 = 3.478 -> 3, so t'·r = 3.45 is 0.55 ms from t: only |t' - t/r| <= 0.5 holds.
    let src = replace_line(FIXTURE, "36,192,1000,", "36,192,4,");
    let copy = rate_copy(src.as_bytes(), 1150, &params()).unwrap();
    assert_eq!(
        section(&text(&copy.osu), "HitObjects")[0],
        "36,192,3,1,0,0:0:0:0:"
    );
}

fn chart_with(objects: &[(i64, Option<i64>)]) -> String {
    let mut src = FIXTURE.split("[HitObjects]").next().unwrap().to_owned();
    src.push_str("[HitObjects]\n");
    for (i, (t, tail)) in objects.iter().enumerate() {
        let x = 36 + 73 * (i % 7);
        match tail {
            Some(end) => src.push_str(&format!("{x},192,{t},128,0,{end}:0:0:0:0:\n")),
            None => src.push_str(&format!("{x},192,{t},1,0,0:0:0:0:\n")),
        }
    }
    src
}

/// 2·|t'·r − t| ≤ r in exact milli-units, i.e. |t' − t/r| ≤ 0.5 ms.
fn within_half_ms(scaled: i64, original: i64, rate: u16) -> bool {
    let rate = i64::from(rate);
    2 * (scaled * rate - original * 1000).abs() <= rate
}

proptest! {
    #[test]
    fn scaled_times_stay_within_half_a_millisecond(
        objects in prop::collection::vec(
            (-5_000i64..=900_000, prop::option::of(1i64..=20_000)),
            1..40,
        ),
        rate in 700u16..=1500,
    ) {
        let objects: Vec<(i64, Option<i64>)> =
            objects.into_iter().map(|(t, len)| (t, len.map(|l| t + l))).collect();
        let src = chart_with(&objects);
        let a = rate_copy(src.as_bytes(), rate, &params()).unwrap();
        let b = rate_copy(src.as_bytes(), rate, &params()).unwrap();
        prop_assert_eq!(&a, &b);

        let out = text(&a.osu);
        let hits = section(&out, "HitObjects");
        prop_assert_eq!(hits.len(), objects.len());
        for (line, (t, tail)) in hits.iter().zip(&objects) {
            let fields: Vec<&str> = line.split(',').collect();
            let scaled: i64 = fields[2].parse().unwrap();
            prop_assert!(within_half_ms(scaled, *t, rate), "{t} -> {scaled} at {rate}");
            match tail {
                Some(end) => {
                    let (end_field, sample) = fields[5].split_once(':').unwrap();
                    let scaled_end: i64 = end_field.parse().unwrap();
                    prop_assert!(within_half_ms(scaled_end, *end, rate));
                    prop_assert_eq!(sample, "0:0:0:0:");
                }
                None => prop_assert_eq!(fields[5], "0:0:0:0:"),
            }
        }
    }
}

#[test]
fn spinner_end_time_is_a_time_and_scales() {
    let src = replace_line(
        FIXTURE,
        "36,192,1000,1,0,0:0:0:0:",
        "256,192,1000,8,0,2000,0:0:0:0:",
    );
    let copy = rate_copy(src.as_bytes(), 1250, &params()).unwrap();
    let hits = section(&text(&copy.osu), "HitObjects");
    assert_eq!(hits[0], "256,192,800,8,0,1600,0:0:0:0:");
}

#[test]
fn inserting_into_a_trailing_section_without_final_break() {
    let head = FIXTURE.split("[Metadata]").next().unwrap();
    let tail = FIXTURE.split("[Difficulty]").nth(1).unwrap();
    let src = format!("{head}[Difficulty]{tail}\n[Metadata]\nVersion:Insane");
    let copy = rate_copy(src.as_bytes(), 1150, &params()).unwrap();
    let out = text(&copy.osu);
    assert!(out.ends_with("\n[Metadata]\nVersion:Insane 1.15x (230bpm)\nTags:wofella"));
}
