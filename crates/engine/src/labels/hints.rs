//! Weak pattern and axis hints from chart names (feature `name-hints`): a pack, folder or
//! difficulty name that states a pattern ("Minijack", "Bracket Pack") or a skill ("Jack
//! Practice") is chart-level evidence, never a gold label. Matching is whole words, case-folded
//! with `str::to_lowercase` and split on every non-alphanumeric char, so it is Unicode-safe;
//! there is no compatibility folding (full-width `ＪＡＣＫ` is another word). Abbreviations
//! (`JT`, `CJ`, `JS`) are not keywords: as words they are mostly initials. The targets are the
//! 7K ids of ADR 0017; hints run only where the keymode profile enables label sources.

use wolluf_core::{AxisId, PatternId};

use super::scan::has_rate_marker;
use super::{ChartLabel, DELETE_MARKER, LabelInput, extract_labels, scale, skill, source};
use Scope::{Anywhere, SkillContext};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum HintTarget {
    Pattern(PatternId),
    Axis(AxisId),
}

impl HintTarget {
    pub fn id(&self) -> &str {
        match self {
            Self::Pattern(p) => p.as_str(),
            Self::Axis(a) => a.as_str(),
        }
    }

    /// The persisted `chart_label.scale` of the hint row.
    pub const fn scale(&self) -> &'static str {
        match self {
            Self::Pattern(_) => scale::HINT_PATTERN,
            Self::Axis(_) => scale::HINT_AXIS,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hint {
    pub target: HintTarget,
    /// The matched words, lowercased and space-joined, or the label skill tag it came from.
    pub token: String,
}

/// Where a keyword counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    /// Mapping jargon: no song title says "jumptrill".
    Anywhere,
    /// Common English ("Speed of Light", "Jack Johnson"): only in a difficulty name, a label
    /// skill tag, or a folder that names a pack, practice, training, dan or course.
    SkillContext,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HintKeyword {
    /// Stable, never renumbered.
    pub id: &'static str,
    /// Lowercase words joined by one space; the last word also matches with a trailing `s`.
    pub phrases: &'static [&'static str],
    pub target: HintTarget,
    pub scope: Scope,
    pub why: &'static str,
}

const fn pattern(id: &'static str) -> HintTarget {
    HintTarget::Pattern(PatternId::from_static(id))
}

const fn axis(id: &'static str) -> HintTarget {
    HintTarget::Axis(AxisId::from_static(id))
}

const fn kw(
    id: &'static str,
    phrases: &'static [&'static str],
    target: HintTarget,
    scope: Scope,
    why: &'static str,
) -> HintKeyword {
    HintKeyword {
        id,
        phrases,
        target,
        scope,
        why,
    }
}

const JACK: &str = "7k.regular.jack";
const TECH: &str = "7k.regular.tech";
const SPEED: &str = "7k.regular.speed";
const STREAM: &str = "7k.regular.stream";
const LN_GENERAL: &str = "7k.ln.general";
const LN_TECH: &str = "7k.ln.tech";

/// The longest phrase at a position wins and consumes its words, so "LN Tech" is only
/// `7k.ln.tech` and "split trill" only `split_trill`. Ambiguous generic words map to an axis,
/// explicit leaf words to a leaf (feature `name-hints`).
pub static HINT_KEYWORDS: [HintKeyword; 28] = [
    kw(
        "minijack",
        &["minijack", "mini jack"],
        pattern("regular.jack.minijack"),
        Anywhere,
        "ADR 0017 leaf: exactly two notes in one column",
    ),
    kw(
        "longjack",
        &["longjack", "long jack"],
        pattern("regular.jack.longjack"),
        Anywhere,
        "ADR 0017 leaf: three or more notes in one column",
    ),
    kw(
        "chordjack",
        &["chordjack", "chord jack"],
        pattern("regular.jack.chordjack"),
        Anywhere,
        "ADR 0017 leaf; also a Road to Gamma and practice-pack skill tag",
    ),
    kw(
        "anchor",
        &["anchor"],
        pattern("regular.jack.anchor"),
        SkillContext,
        "ADR 0017 leaf, but an everyday word in titles",
    ),
    kw(
        "bracket",
        &["bracket"],
        pattern("regular.stream.bracket"),
        Anywhere,
        "ADR 0017 keeps the wiki/MinaCalc meaning; a pack using Interlude's meaning makes this \
         hint disagree, which the agreement report shows",
    ),
    kw(
        "chordtrill",
        &["chordtrill", "chord trill"],
        pattern("regular.stream.chordtrill"),
        Anywhere,
        "ADR 0017 leaf; `Chord//Bracket` packs name two skills, so no `chord bracket` phrase",
    ),
    kw(
        "jumptrill",
        &["jumptrill", "jump trill"],
        pattern("regular.stream.jumptrill"),
        Anywhere,
        "ADR 0017 leaf; `jump-trill` splits into the two-word phrase",
    ),
    kw(
        "split_trill",
        &["split trill", "splittrill"],
        pattern("regular.stream.split_trill"),
        Anywhere,
        "ADR 0017 leaf",
    ),
    kw(
        "trill",
        &["trill"],
        pattern("regular.stream.trill"),
        Anywhere,
        "ADR 0017 leaf; rare as a title word",
    ),
    kw(
        "roll",
        &["roll", "stair", "staircase"],
        pattern("regular.stream.roll"),
        SkillContext,
        "ADR 0017 leaf covers rolls and stairs; `roll` is common in titles (\"Rock and Roll\")",
    ),
    kw(
        "jumpstream",
        &["jumpstream", "jump stream"],
        pattern("regular.stream.jumpstream"),
        Anywhere,
        "ADR 0017 leaf",
    ),
    kw(
        "handstream",
        &["handstream", "hand stream"],
        pattern("regular.stream.handstream"),
        Anywhere,
        "ADR 0017 leaf",
    ),
    kw(
        "chordstream",
        &["chordstream", "chord stream"],
        axis(STREAM),
        Anywhere,
        "light vs dense is not said, so only the stream axis is",
    ),
    kw(
        "chordstream_light",
        &["light chordstream", "light chord stream"],
        pattern("regular.stream.chordstream_light"),
        Anywhere,
        "the leaf only when the name says light",
    ),
    kw(
        "chordstream_dense",
        &["dense chordstream", "dense chord stream"],
        pattern("regular.stream.chordstream_dense"),
        Anywhere,
        "the leaf only when the name says dense",
    ),
    kw(
        "burst",
        &["burst"],
        pattern("regular.speed.burst"),
        SkillContext,
        "ADR 0017 leaf, but common in titles (\"Burst The Gravity\")",
    ),
    kw(
        "delay",
        &["delay", "delaymaster"],
        pattern("regular.speed.delay"),
        Anywhere,
        "ADR 0017 leaf: BMS difficulty jargon (`DELAYMASTER`); `delayed` never matches",
    ),
    kw(
        "inverse",
        &["inverse", "ln inverse"],
        pattern("ln.inverse.gap"),
        SkillContext,
        "ADR 0017 leaf; `LN inverse` is the leaf, not a separate LN claim",
    ),
    kw(
        "release",
        &["release", "ln release"],
        pattern("ln.release.timing"),
        SkillContext,
        "ADR 0017 leaf; `release` is common in titles",
    ),
    kw(
        "shield",
        &["shield", "ln shield"],
        pattern("ln.tech.shield"),
        SkillContext,
        "ADR 0017 leaf; `shield` is common in titles",
    ),
    kw(
        "hybrid",
        &["hybrid", "ln hybrid"],
        pattern("ln.tech.hybrid"),
        SkillContext,
        "ADR 0017 leaf; `hybrid` is common in titles",
    ),
    kw(
        "jack",
        &["jack"],
        axis(JACK),
        SkillContext,
        "generic: any jack leaf; also a first name",
    ),
    kw(
        "tech",
        &["tech", "technical"],
        axis(TECH),
        SkillContext,
        "generic tech skill (Jinjin's tech slot)",
    ),
    kw(
        "speed",
        &["speed"],
        axis(SPEED),
        SkillContext,
        "generic speed skill; common in titles",
    ),
    kw(
        "stream",
        &["stream"],
        axis(STREAM),
        SkillContext,
        "generic stream skill; common in titles",
    ),
    kw(
        "stamina",
        &["stamina"],
        axis(STREAM),
        SkillContext,
        "ADR 0017: stamina is derived, not an axis; stamina maps are long (chord)streams, \
         Jinjin's chordstream-stamina slot",
    ),
    kw(
        "ln",
        &["ln", "long note", "longnote", "fln", "full ln"],
        axis(LN_GENERAL),
        Anywhere,
        "generic LN: the general LN axis, not a leaf; `FLN` is a full-LN conversion",
    ),
    kw(
        "ln_tech",
        &["ln tech", "ln technical"],
        axis(LN_TECH),
        Anywhere,
        "Jinjin's LN tech slot: hybrid or shield, not said which",
    ),
];

/// Phrases that consume their words without a hint.
pub static BLOCKED_PHRASES: [(&str, &str); 2] = [
    ("tech n9ne", "an artist, not the tech skill"),
    (
        "speed core",
        "speedcore is a genre; as one word it never matches `speed`",
    ),
];

/// A folder holding one of these words names a curated set, so common words count there.
const PACK_MARKERS: [&str; 5] = ["pack", "practice", "training", "dan", "course"];

/// Label skill tags that name an axis outright: the KomeijiDove slots and the matching Road to
/// Gamma tags. Other tags go through the keyword table.
pub static SKILL_TAG_AXES: [(&str, AxisId); 8] = [
    ("jack", AxisId::from_static(JACK)),
    ("tech", AxisId::from_static(TECH)),
    ("speed", AxisId::from_static(SPEED)),
    ("stream", AxisId::from_static(STREAM)),
    ("ln_general", AxisId::from_static(LN_GENERAL)),
    ("ln_tech", AxisId::from_static(LN_TECH)),
    ("ln_inverse", AxisId::from_static("7k.ln.inverse")),
    ("ln_release", AxisId::from_static("7k.ln.release")),
];

/// A dan's overall slot spans every skill of its track, so it hints nothing; the folder's own
/// words still do.
const OVERALL_TAGS: [&str; 2] = [skill::REGULAR_OVERALL, skill::LN_OVERALL];

/// Hints of one chart: label skill tags first, then the difficulty name, then the folder; one
/// per target, the first token winning.
pub fn extract_hints(input: &LabelInput<'_>) -> Vec<Hint> {
    hints_with(input, &extract_labels(input))
}

/// The `chart_label` rows of the chart's hints, given its labels; a hint is a variant when the
/// name has a rate marker or one of the labels is a variant.
pub fn hint_labels(input: &LabelInput<'_>, labels: &[ChartLabel]) -> Vec<ChartLabel> {
    let is_variant = has_rate_marker(input.version) || labels.iter().any(|l| l.is_variant);
    hints_with(input, labels)
        .into_iter()
        .map(|h| ChartLabel {
            source: source::NAME_HINT.into(),
            scale: h.target.scale().into(),
            level_ord: None,
            level_text: h.target.id().into(),
            skill_tag: Some(h.token),
            is_variant,
        })
        .collect()
}

fn hints_with(input: &LabelInput<'_>, labels: &[ChartLabel]) -> Vec<Hint> {
    if input.version.starts_with(DELETE_MARKER) {
        return Vec::new();
    }
    let mut found = Vec::new();
    for tag in labels.iter().filter_map(|l| l.skill_tag.as_deref()) {
        if OVERALL_TAGS.contains(&tag) {
            continue;
        }
        match SKILL_TAG_AXES.iter().find(|(t, _)| *t == tag) {
            Some((_, a)) => found.push(Hint {
                target: HintTarget::Axis(a.clone()),
                token: tag.to_owned(),
            }),
            None => scan(&words(tag), true, &mut found),
        }
    }
    scan(&words(input.version), true, &mut found);
    let folder = words(input.folder);
    let pack = folder
        .iter()
        .any(|w| PACK_MARKERS.iter().any(|m| word_matches(w, m)));
    scan(&folder, pack, &mut found);
    let mut out: Vec<Hint> = Vec::with_capacity(found.len());
    for h in found {
        if !out.iter().any(|o| o.target == h.target) {
            out.push(h);
        }
    }
    out
}

fn words(s: &str) -> Vec<String> {
    s.split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_lowercase)
        .collect()
}

fn word_matches(word: &str, wanted: &str) -> bool {
    word == wanted || word.strip_suffix('s') == Some(wanted)
}

/// Number of words `phrase` matches at the start of `ws`; only its last word may be plural.
fn phrase_len(ws: &[String], phrase: &str) -> Option<usize> {
    let parts: Vec<&str> = phrase.split(' ').collect();
    let last = parts.len().checked_sub(1)?;
    let hit = parts.len() <= ws.len()
        && parts.iter().enumerate().all(|(i, p)| {
            if i == last {
                word_matches(&ws[i], p)
            } else {
                ws[i] == *p
            }
        });
    hit.then_some(parts.len())
}

fn scan(ws: &[String], skill_context: bool, out: &mut Vec<Hint>) {
    let mut i = 0;
    while i < ws.len() {
        let rest = &ws[i..];
        let blocked = BLOCKED_PHRASES
            .iter()
            .filter_map(|(p, _)| phrase_len(rest, p))
            .max();
        let keyword = HINT_KEYWORDS
            .iter()
            .flat_map(|k| k.phrases.iter().map(move |p| (k, p)))
            .filter_map(|(k, p)| Some((phrase_len(rest, p)?, k)))
            .max_by_key(|(n, _)| *n);
        match (blocked, keyword) {
            (Some(b), Some((n, _))) if b >= n => i += b,
            (Some(b), None) => i += b,
            (_, Some((n, k))) => {
                if k.scope == Scope::Anywhere || skill_context {
                    out.push(Hint {
                        target: k.target.clone(),
                        token: rest[..n].join(" "),
                    });
                }
                i += n;
            }
            (None, None) => i += 1,
        }
    }
}

#[cfg(test)]
mod tests;
