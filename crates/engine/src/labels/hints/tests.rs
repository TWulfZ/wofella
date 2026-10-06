use std::collections::BTreeSet;

use proptest::prelude::*;
use wolluf_core::id::is_valid_stable_id;

use super::*;
use crate::labels::KOMEIJIDOVE_SETS;
use crate::taxonomy;

// Ids are spelled out as literals on purpose: they are persisted, so a rename must break these tests.
fn hints_of(
    folder: &str,
    version: &str,
    creator: &str,
    set_id: Option<i32>,
) -> Vec<(String, String)> {
    extract_hints(&LabelInput {
        folder,
        version,
        creator,
        set_id,
    })
    .into_iter()
    .map(|h| (h.target.id().to_owned(), h.token))
    .collect()
}

fn named(folder: &str, version: &str) -> Vec<(String, String)> {
    hints_of(folder, version, "Mapper", None)
}

fn pairs(expected: &[(&str, &str)]) -> Vec<(String, String)> {
    expected
        .iter()
        .map(|(t, k)| ((*t).to_owned(), (*k).to_owned()))
        .collect()
}

const SONG: &str = "100 Artist - Song";

#[test]
fn hints_leaf_words_map_to_patterns() {
    let cases: &[(&str, &str, &str)] = &[
        ("Minijack", "regular.jack.minijack", "minijack"),
        ("Mini Jacks", "regular.jack.minijack", "mini jacks"),
        ("Longjack", "regular.jack.longjack", "longjack"),
        ("Long Jack", "regular.jack.longjack", "long jack"),
        ("Chordjack", "regular.jack.chordjack", "chordjack"),
        ("Chord Jack 1.1x", "regular.jack.chordjack", "chord jack"),
        ("Anchors", "regular.jack.anchor", "anchors"),
        ("Bracket", "regular.stream.bracket", "bracket"),
        ("Chord Trill", "regular.stream.chordtrill", "chord trill"),
        ("Chordtrills", "regular.stream.chordtrill", "chordtrills"),
        ("Jumptrill", "regular.stream.jumptrill", "jumptrill"),
        ("Jump-Trill", "regular.stream.jumptrill", "jump trill"),
        ("Split Trill", "regular.stream.split_trill", "split trill"),
        ("Trills", "regular.stream.trill", "trills"),
        ("Rolls", "regular.stream.roll", "rolls"),
        ("Stairs", "regular.stream.roll", "stairs"),
        ("Jumpstream", "regular.stream.jumpstream", "jumpstream"),
        ("Jump Stream", "regular.stream.jumpstream", "jump stream"),
        ("Handstream", "regular.stream.handstream", "handstream"),
        ("Hand Stream", "regular.stream.handstream", "hand stream"),
        (
            "Light Chordstream",
            "regular.stream.chordstream_light",
            "light chordstream",
        ),
        (
            "Dense Chord Stream",
            "regular.stream.chordstream_dense",
            "dense chord stream",
        ),
        ("Burst", "regular.speed.burst", "burst"),
        ("Inverse", "ln.inverse.gap", "inverse"),
        ("LN Inverse", "ln.inverse.gap", "ln inverse"),
        ("Release", "ln.release.timing", "release"),
        ("Shield", "ln.tech.shield", "shield"),
        ("LN Hybrid", "ln.tech.hybrid", "ln hybrid"),
    ];
    for (version, target, token) in cases {
        assert_eq!(
            named(SONG, version),
            pairs(&[(target, token)]),
            "{version:?}"
        );
    }
}

#[test]
fn hints_generic_words_map_to_axes() {
    let cases: &[(&str, &str, &str)] = &[
        ("Jack", "7k.regular.jack", "jack"),
        ("JACKS", "7k.regular.jack", "jacks"),
        ("Tech", "7k.regular.tech", "tech"),
        ("Technical", "7k.regular.tech", "technical"),
        ("Speed", "7k.regular.speed", "speed"),
        ("Stream", "7k.regular.stream", "stream"),
        // ADR 0017: stamina is derived, not an axis; Jinjin's stamina slot is chordstream.
        ("Stamina", "7k.regular.stream", "stamina"),
        // Light or dense is not said, so only the axis is.
        ("Chordstream", "7k.regular.stream", "chordstream"),
        ("LN", "7k.ln.general", "ln"),
        ("LNs", "7k.ln.general", "lns"),
        ("Long Notes", "7k.ln.general", "long notes"),
        ("Hard +FLN", "7k.ln.general", "fln"),
        ("LN Tech", "7k.ln.tech", "ln tech"),
    ];
    for (version, target, token) in cases {
        assert_eq!(
            named(SONG, version),
            pairs(&[(target, token)]),
            "{version:?}"
        );
    }
}

#[test]
fn hints_longest_phrase_wins_and_consumes_its_words() {
    assert_eq!(
        named(SONG, "LN Tech"),
        pairs(&[("7k.ln.tech", "ln tech")]),
        "no ln.general and no regular.tech"
    );
    assert_eq!(
        named(SONG, "Split Trill"),
        pairs(&[("regular.stream.split_trill", "split trill")])
    );
    // A skill pair (kasumi99's `Chord//Bracket`), not a chord-trill shape.
    assert_eq!(
        named(SONG, "Chord//Bracket"),
        pairs(&[("regular.stream.bracket", "bracket")])
    );
    assert_eq!(
        named(SONG, "Dense Chordstream"),
        pairs(&[("regular.stream.chordstream_dense", "dense chordstream")])
    );
}

#[test]
fn hints_are_unique_per_target_first_token_wins() {
    assert_eq!(
        named(SONG, "Rolls & Stairs"),
        pairs(&[("regular.stream.roll", "rolls")])
    );
    assert_eq!(
        named("100 Various - Jack & Speed Pack", "LN Hybrid Jacks"),
        pairs(&[
            ("ln.tech.hybrid", "ln hybrid"),
            ("7k.regular.jack", "jacks"),
            ("7k.regular.speed", "speed"),
        ]),
        "difficulty name first, then the folder"
    );
}

#[test]
fn hints_common_words_in_a_song_folder_need_a_pack_context() {
    for folder in [
        "100 Artist - Speed of Light",
        "100 Jack Johnson - Song",
        "100 Artist - Burst The Gravity",
        "100 Artist - Release",
        "100 Artist - Stream of Tears",
    ] {
        assert_eq!(named(folder, "Insane"), [], "{folder:?}");
    }
    assert_eq!(
        named("100 Various Artists - Speed Practice", "Insane"),
        pairs(&[("7k.regular.speed", "speed")])
    );
    assert_eq!(
        named("100 Mapper - Stream Packs", "Insane"),
        pairs(&[("7k.regular.stream", "stream")])
    );
    // Jargon needs no context.
    assert_eq!(
        named("100 Artist - Jumptrill Song", "Insane"),
        pairs(&[("regular.stream.jumptrill", "jumptrill")])
    );
}

#[test]
fn hints_need_whole_words() {
    for (folder, version) in [
        ("100 Michael Jackson - Song Pack", "Hard"),
        (SONG, "Jacket"),
        (SONG, "Hijack"),
        (SONG, "Speedcore"),
        (SONG, "LNG"),
        (SONG, "Technique"),
        (SONG, "Streamer"),
        (SONG, "Rolling"),
        (SONG, "Trillion"),
    ] {
        assert_eq!(named(folder, version), [], "{folder:?} {version:?}");
    }
}

#[test]
fn hints_blocked_phrases_give_nothing() {
    assert_eq!(named(SONG, "Tech N9ne"), []);
    assert_eq!(named("100 Tech N9ne - Song Pack", "Hard"), []);
    assert_eq!(named(SONG, "Speed Core"), []);
    // The block only consumes its own words.
    assert_eq!(
        named(SONG, "Tech N9ne Tech"),
        pairs(&[("7k.regular.tech", "tech")])
    );
}

#[test]
fn hints_are_case_insensitive_and_unicode_safe() {
    assert_eq!(named(SONG, "jAcK"), pairs(&[("7k.regular.jack", "jack")]));
    assert_eq!(
        named(SONG, "ジャック Jack's"),
        pairs(&[("7k.regular.jack", "jack")])
    );
    assert_eq!(
        named(SONG, "🔥Jack🔥"),
        pairs(&[("7k.regular.jack", "jack")])
    );
    // No compatibility folding: full-width letters are another word.
    assert_eq!(named(SONG, "ＪＡＣＫ"), []);
    assert_eq!(named(SONG, "Jäck"), []);
    assert_eq!(named("", ""), []);
}

const KD_JACK: &str = "1877617 Various Artists - KomeijiDove 7K Jack Practice";
const RTG: &str = "999 Various - 7K Road to Gamma Dan Pack";

#[test]
fn hints_reuse_label_skill_tags() {
    assert_eq!(
        hints_of(KD_JACK, "~ 9th ~ Song", "KomeijiDove", Some(1877617)),
        pairs(&[("7k.regular.jack", "jack")]),
        "the slot and the folder agree"
    );
    assert_eq!(
        hints_of(
            "1888009 Various Artists - KomeijiDove 7K LN Inverse Practice",
            "~ 5th ~ Song",
            "KomeijiDove",
            None
        ),
        pairs(&[
            ("7k.ln.inverse", "ln_inverse"),
            ("ln.inverse.gap", "ln inverse"),
        ])
    );
    assert_eq!(
        hints_of(
            "1888027 Various - KD Practice",
            "~ 5th ~ Song",
            "KomeijiDove",
            Some(1888027)
        ),
        pairs(&[("7k.ln.release", "ln_release")])
    );
    assert_eq!(
        named(RTG, "Song // Speed 1.1x"),
        pairs(&[("7k.regular.speed", "speed")])
    );
    assert_eq!(
        named(RTG, "Song // Chordjack 1"),
        pairs(&[("regular.jack.chordjack", "chordjack")])
    );
    assert_eq!(
        named("Various - Gamma Practice Pack", "Song (Stamina)"),
        pairs(&[("7k.regular.stream", "stamina")])
    );
}

#[test]
fn hints_ignore_dan_overall_slots() {
    assert_eq!(
        named(
            "1340245 Jinjin - 7K Dan Course - Regular Dan Phase 1",
            "9th"
        ),
        []
    );
    // The folder's own "LN" is still a hint.
    assert_eq!(
        named("1467891 Jinjin - 7K Dan Course - LN Dan Phase 2", "Gamma"),
        pairs(&[("7k.ln.general", "ln")])
    );
}

#[test]
fn hints_skip_placeholder_difficulties() {
    assert_eq!(named("100 Various - Jack Pack", "Delete Upon download"), []);
}

#[test]
fn hint_labels_are_name_hint_rows() {
    let input = LabelInput {
        folder: "100 Various - Jack Pack",
        version: "Minijack 1.1x (200bpm)",
        creator: "Mapper",
        set_id: None,
    };
    let row = |scale: &str, target: &str, token: &str| ChartLabel {
        source: "name_hint".into(),
        scale: scale.into(),
        level_ord: None,
        level_text: target.into(),
        skill_tag: Some(token.into()),
        is_variant: true,
    };
    assert_eq!(
        hint_labels(&input, &extract_labels(&input)),
        [
            row("hint_pattern", "regular.jack.minijack", "minijack"),
            row("hint_axis", "7k.regular.jack", "jack"),
        ]
    );
}

#[test]
fn hint_labels_inherit_the_variant_flag_of_the_labels() {
    // A KomeijiDove section cut (`/` in the level) has no rate marker in its name.
    let input = LabelInput {
        folder: KD_JACK,
        version: "~ 9th/1 ~ Song",
        creator: "KomeijiDove",
        set_id: Some(1877617),
    };
    let labels = extract_labels(&input);
    assert!(labels[0].is_variant, "{labels:?}");
    let hints = hint_labels(&input, &labels);
    assert_eq!(hints.len(), 1, "{hints:?}");
    assert!(hints[0].is_variant);
}

#[test]
fn hints_keyword_table_is_well_formed() {
    let k7 = taxonomy::k7();
    let axes = taxonomy::axes(k7);
    let mut ids = BTreeSet::new();
    let mut phrases = BTreeSet::new();
    for k in &HINT_KEYWORDS {
        assert!(is_valid_stable_id(k.id), "{}", k.id);
        assert!(ids.insert(k.id), "duplicate id {}", k.id);
        assert!(!k.why.trim().is_empty(), "{} has no why", k.id);
        assert!(!k.phrases.is_empty(), "{}", k.id);
        match &k.target {
            HintTarget::Pattern(p) => assert!(taxonomy::by_id(k7, p.as_str()).is_some(), "{p}"),
            HintTarget::Axis(a) => assert!(axes.contains(&a), "{a}"),
        }
        for p in k.phrases {
            assert_eq!(
                words(p).join(" "),
                *p,
                "{} phrase {p:?} is not normalized",
                k.id
            );
            assert!(phrases.insert(*p), "phrase {p:?} in two keywords");
        }
    }
    for (p, why) in &BLOCKED_PHRASES {
        assert_eq!(words(p).join(" "), *p, "blocked {p:?} is not normalized");
        assert!(!why.trim().is_empty(), "{p}");
        assert!(phrases.insert(*p), "blocked {p:?} is also a keyword");
    }
}

#[test]
fn hints_keyword_table_covers_the_requested_leaves() {
    let targets: BTreeSet<&str> = HINT_KEYWORDS.iter().map(|k| k.target.id()).collect();
    for id in [
        "regular.jack.minijack",
        "regular.jack.longjack",
        "regular.jack.chordjack",
        "regular.jack.anchor",
        "regular.stream.bracket",
        "regular.stream.chordtrill",
        "regular.stream.jumptrill",
        "regular.stream.split_trill",
        "regular.stream.trill",
        "regular.stream.roll",
        "regular.stream.jumpstream",
        "regular.stream.handstream",
        "regular.stream.chordstream_light",
        "regular.stream.chordstream_dense",
        "regular.speed.burst",
        "ln.inverse.gap",
        "ln.release.timing",
        "ln.tech.shield",
        "ln.tech.hybrid",
        "7k.regular.jack",
        "7k.regular.tech",
        "7k.regular.speed",
        "7k.regular.stream",
        "7k.ln.general",
        "7k.ln.tech",
    ] {
        assert!(targets.contains(id), "no keyword for {id}");
    }
}

#[test]
fn hints_every_komeijidove_slot_has_an_axis() {
    let axes = taxonomy::axes(taxonomy::k7());
    for (_, slot) in KOMEIJIDOVE_SETS {
        let axis = SKILL_TAG_AXES
            .iter()
            .find(|(tag, _)| *tag == slot)
            .map(|(_, a)| a);
        assert!(axis.is_some_and(|a| axes.contains(&a)), "{slot}");
    }
}

#[test]
fn hints_keyword_table_snapshot() {
    let lines: Vec<String> = HINT_KEYWORDS
        .iter()
        .map(|k| {
            format!(
                "{} {} {} {:?} [{}]",
                k.id,
                k.target.scale(),
                k.target.id(),
                k.scope,
                k.phrases.join(", ")
            )
        })
        .collect();
    insta::assert_snapshot!("hint_keywords", lines.join("\n"));
}

proptest! {
    #[test]
    fn hints_never_panic_and_tokens_are_normalized(folder in "\\PC{0,40}", version in "\\PC{0,40}") {
        for h in extract_hints(&LabelInput { folder: &folder, version: &version, creator: "", set_id: None }) {
            prop_assert!(!h.token.is_empty());
            prop_assert_eq!(h.token.to_lowercase(), h.token.clone());
        }
    }
}
