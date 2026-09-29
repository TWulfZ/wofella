//! `cargo xtask fixtures dbs`: a minimized, anonymized extract of the corpus osu!.db, scores.db
//! and collection.db (spec 002 Design, "Fixtures"; R14: no real names or md5s land in git).

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use anyhow::{Context, bail, ensure};
use md5::{Digest, Md5};
use wolluf_source_osu::codec::OsuString;
use wolluf_source_osu::codec::collection_db::{
    Collection, CollectionDb, decode_collection_db, encode_collection_db,
};
use wolluf_source_osu::codec::osu_db::{OsuDb, OsuDbBeatmap, decode_osu_db};
use wolluf_source_osu::codec::score_header::OnlineId;
use wolluf_source_osu::codec::scores_db::{
    ScoreRecord, ScoresDb, ScoresDbBeatmap, decode_scores_db,
};
use wolluf_source_osu::testkit::{encode_osu_db, encode_scores_db};

use super::{OutFile, hex, sha256_hex};

pub(crate) struct SelectionParams {
    pub(crate) beatmaps: usize,
    pub(crate) max_scores: usize,
    /// One beatmap must carry at least this many scores (scores.db groups several per md5).
    pub(crate) multi_scores: usize,
    pub(crate) collections: usize,
    /// Collection members with no osu!.db entry kept per collection: all 99 pilot collection md5s
    /// are such dangling references, so restricting to selected beatmaps alone leaves them empty.
    pub(crate) max_dangling_members: usize,
    pub(crate) max_timing_points: usize,
    pub(crate) modes: &'static [u8],
    pub(crate) mania_keymodes: &'static [u8],
}

pub(crate) const PARAMS: SelectionParams = SelectionParams {
    beatmaps: 12,
    max_scores: 40,
    multi_scores: 2,
    collections: 2,
    max_dangling_members: 2,
    max_timing_points: 3,
    modes: &[0, 1, 2, 3],
    mania_keymodes: &[4, 7],
};

/// `fixtures_are_anonymized` accepts `md5("wolluf-fixture:<n>")` for n below 1000 only.
const FIXTURE_MD5_TAG: &str = "wolluf-fixture:";
const MAX_FIXTURE_MD5: usize = 1000;
const ANON_ID_BASE: i64 = 1000;
const OSU_DB_PLAYER: &str = "fixture";
const MD5_HEX_LEN: usize = 32;
/// Shorter names (the pilot has one-letter aliases) occur by chance in binary fields and in the
/// fixture's own `<field>-<n>` text, so they are checked against decoded strings instead.
const MIN_SCANNED_NAME_LEN: usize = 4;

pub(crate) const SELECTION_RULE: &str = "Candidates are osu!.db entries with a valid md5, deduplicated and sorted by original md5. \
Requirements, in order: a beatmap with an empty-name score, one with a ScoreV2 score, one with a positive online id, one with at least 2 \
scores, one member of each of the first 2 collections (file order) that has one in osu!.db, every mode 0-3, mania keymodes 4 and 7, \
one entry with timing points, one entry per ranked status seen. An unmet requirement takes the candidate with the \
fewest scores, then the lowest md5; the rest is filled the same way up to 12 entries. Scores are all scores of the \
selected beatmaps (at most 40). Collections keep their selected members plus the first 2 members that have no \
osu!.db entry at all (dangling references, as every pilot collection member is).";

pub(crate) const ANONYMIZATION_RULE: &str = "md5s (beatmap and replay) become md5(\"wolluf-fixture:<n>\"), numbered from 1 in output order, \
consistent across the three files. Strings become <field>-<n> with n the beatmap number (absent and empty strings \
are kept); collection names become collection-<k>. Players become player-01.. by first appearance, the empty name \
is kept; the osu!.db header name becomes fixture. Positive online ids become 1000+k and the rest 0; positive \
beatmap and set ids become 1000+n and thread ids 0. Timing points are cut to 3 per entry; version headers, \
counts, mods, ticks and star ratings are kept. Generation fails if an original md5 or player/cfg name appears.";

pub(crate) struct CorpusDbs {
    pub(crate) osu_db: OsuDb,
    pub(crate) scores_db: ScoresDb,
    pub(crate) collection_db: CollectionDb,
    /// Names that live outside the three DBs (cfg `Username`, the Windows account name).
    pub(crate) extra_names: Vec<Vec<u8>>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DbExtract {
    pub(crate) osu_db: OsuDb,
    pub(crate) scores_db: ScoresDb,
    pub(crate) collection_db: CollectionDb,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Selection {
    /// Indices into `osu_db.beatmaps`, sorted by original md5.
    pub(crate) beatmaps: Vec<usize>,
    /// Indices into `collection_db.collections`, in file order.
    pub(crate) collections: Vec<usize>,
}

struct Candidate<'a> {
    index: usize,
    md5: &'a [u8],
    beatmap: &'a OsuDbBeatmap,
    scores: Vec<&'a ScoreRecord>,
}

impl Candidate<'_> {
    fn key(&self) -> (usize, &[u8]) {
        (self.scores.len(), self.md5)
    }
}

#[derive(Debug, Clone, Copy)]
enum Requirement {
    EmptyNameScore,
    ScoreV2Score,
    OnlineIdScore,
    MultiScore(usize),
    Collection(usize),
    Mode(u8),
    Keymode(u8),
    TimingPoints,
    RankedStatus(u8),
}

impl Requirement {
    fn met_by(self, c: &Candidate<'_>, collections: &CollectionDb) -> bool {
        match self {
            Self::EmptyNameScore => c
                .scores
                .iter()
                .any(|s| s.header.player.as_bytes() == Some(&[])),
            Self::ScoreV2Score => c.scores.iter().any(|s| s.header.is_score_v2()),
            Self::OnlineIdScore => c.scores.iter().any(|s| s.online_id.positive().is_some()),
            Self::MultiScore(n) => c.scores.len() >= n,
            Self::Collection(i) => collections.collections.get(i).is_some_and(|coll| {
                coll.beatmap_md5s
                    .iter()
                    .any(|m| m.as_bytes() == Some(c.md5))
            }),
            Self::Mode(mode) => c.beatmap.mode == mode,
            Self::Keymode(keys) => c.beatmap.mania_keymode().map(|k| k.columns()) == Some(keys),
            Self::TimingPoints => !c.beatmap.timing_points.is_empty(),
            Self::RankedStatus(status) => c.beatmap.ranked_status == status,
        }
    }

    fn describe(self) -> String {
        match self {
            Self::EmptyNameScore => "a score with an empty player name".into(),
            Self::ScoreV2Score => "a ScoreV2 score".into(),
            Self::OnlineIdScore => "a score with a positive online id".into(),
            Self::MultiScore(n) => format!("a beatmap with at least {n} scores"),
            Self::Collection(i) => format!("a member of collection #{i}"),
            Self::Mode(mode) => format!("a beatmap of mode {mode}"),
            Self::Keymode(keys) => format!("a mania {keys}K beatmap"),
            Self::TimingPoints => "a beatmap with timing points".into(),
            Self::RankedStatus(status) => format!("a beatmap with ranked status {status}"),
        }
    }
}

fn scores_by_md5(scores_db: &ScoresDb) -> BTreeMap<&[u8], Vec<&ScoreRecord>> {
    let mut out: BTreeMap<&[u8], Vec<&ScoreRecord>> = BTreeMap::new();
    for group in &scores_db.beatmaps {
        if let Some(md5) = group.md5.as_bytes() {
            out.entry(md5).or_default().extend(group.scores.iter());
        }
    }
    out
}

fn candidates(corpus: &CorpusDbs) -> Vec<Candidate<'_>> {
    let scores = scores_by_md5(&corpus.scores_db);
    let mut by_md5: BTreeMap<&[u8], Candidate<'_>> = BTreeMap::new();
    for (index, beatmap) in corpus.osu_db.beatmaps.iter().enumerate() {
        let Some(md5) = beatmap.md5.as_bytes() else {
            continue;
        };
        if beatmap.beatmap_md5().is_none() || by_md5.contains_key(md5) {
            continue;
        }
        by_md5.insert(
            md5,
            Candidate {
                index,
                md5,
                beatmap,
                scores: scores.get(md5).cloned().unwrap_or_default(),
            },
        );
    }
    by_md5.into_values().collect()
}

pub(crate) fn select(corpus: &CorpusDbs, params: &SelectionParams) -> anyhow::Result<Selection> {
    // Every beatmap and replay md5 gets its own fixture number.
    ensure!(
        params.beatmaps + params.max_scores + params.collections * params.max_dangling_members
            < MAX_FIXTURE_MD5,
        "the selection budget exceeds the fixture md5 range"
    );
    let candidates = candidates(corpus);
    let md5s: BTreeSet<&[u8]> = candidates.iter().map(|c| c.md5).collect();
    let collections: Vec<usize> = (0..corpus.collection_db.collections.len())
        .take(params.collections)
        .collect();
    ensure!(
        collections.len() == params.collections,
        "the corpus has {} collections, {} needed",
        collections.len(),
        params.collections
    );
    let reachable = |i: usize| {
        corpus.collection_db.collections.get(i).is_some_and(|coll| {
            coll.beatmap_md5s
                .iter()
                .any(|m| m.as_bytes().is_some_and(|m| md5s.contains(m)))
        })
    };

    let statuses: BTreeSet<u8> = candidates.iter().map(|c| c.beatmap.ranked_status).collect();
    let requirements = [
        Requirement::EmptyNameScore,
        Requirement::ScoreV2Score,
        Requirement::OnlineIdScore,
        Requirement::MultiScore(params.multi_scores),
    ]
    .into_iter()
    .chain(
        collections
            .iter()
            .filter(|&&i| reachable(i))
            .map(|&i| Requirement::Collection(i)),
    )
    .chain(params.modes.iter().map(|&m| Requirement::Mode(m)))
    .chain(
        params
            .mania_keymodes
            .iter()
            .map(|&k| Requirement::Keymode(k)),
    )
    .chain([Requirement::TimingPoints])
    .chain(statuses.into_iter().map(Requirement::RankedStatus));

    let mut chosen: BTreeSet<usize> = BTreeSet::new();
    let pick = |chosen: &BTreeSet<usize>, pred: &dyn Fn(&Candidate<'_>) -> bool| {
        candidates
            .iter()
            .enumerate()
            .filter(|(i, c)| !chosen.contains(i) && pred(c))
            .min_by(|(_, a), (_, b)| a.key().cmp(&b.key()))
            .map(|(i, _)| i)
    };
    for req in requirements {
        let met = |c: &Candidate<'_>| req.met_by(c, &corpus.collection_db);
        if chosen.iter().any(|&i| candidates.get(i).is_some_and(met)) {
            continue;
        }
        match pick(&chosen, &met) {
            Some(i) => chosen.insert(i),
            None => bail!("the corpus has no candidate for {}", req.describe()),
        };
    }
    ensure!(
        chosen.len() <= params.beatmaps,
        "requirements need {} beatmaps, the budget is {}",
        chosen.len(),
        params.beatmaps
    );
    while chosen.len() < params.beatmaps {
        match pick(&chosen, &|_| true) {
            Some(i) => chosen.insert(i),
            None => bail!("osu!.db has fewer than {} usable beatmaps", params.beatmaps),
        };
    }

    let picked: Vec<&Candidate<'_>> = chosen.iter().filter_map(|&i| candidates.get(i)).collect();
    let n_scores: usize = picked.iter().map(|c| c.scores.len()).sum();
    ensure!(
        n_scores <= params.max_scores,
        "the selection holds {n_scores} scores, at most {} allowed",
        params.max_scores
    );
    // `chosen` indexes the md5-sorted candidate list, so this is already md5 order.
    Ok(Selection {
        beatmaps: picked.iter().map(|c| c.index).collect(),
        collections,
    })
}

pub(crate) fn fixture_md5(n: usize) -> String {
    hex(&Md5::digest(format!("{FIXTURE_MD5_TAG}{n}")))
}

#[derive(Default)]
struct Md5Map {
    map: BTreeMap<Vec<u8>, OsuString>,
}

impl Md5Map {
    /// Absent and empty values carry no identity and keep their on-disk shape.
    fn map(&mut self, md5: &OsuString) -> OsuString {
        match md5.as_bytes() {
            None | Some([]) => md5.clone(),
            Some(bytes) => {
                let next = self.map.len() + 1;
                self.map
                    .entry(bytes.to_vec())
                    .or_insert_with(|| OsuString::present(fixture_md5(next)))
                    .clone()
            }
        }
    }

    fn get(&self, md5: &OsuString) -> Option<OsuString> {
        self.map.get(md5.as_bytes()?).cloned()
    }
}

fn text(s: &OsuString, field: &str, n: usize) -> OsuString {
    match s.as_bytes() {
        None | Some([]) => s.clone(),
        Some(_) => OsuString::present(format!("{field}-{n}")),
    }
}

fn anon_id(id: i32, n: usize) -> i32 {
    if id > 0 {
        i32::try_from(ANON_ID_BASE)
            .unwrap_or(i32::MAX)
            .saturating_add(i32::try_from(n).unwrap_or(i32::MAX))
    } else {
        0
    }
}

fn anonymize_beatmap(
    b: &OsuDbBeatmap,
    n: usize,
    md5: OsuString,
    params: &SelectionParams,
) -> OsuDbBeatmap {
    let mut out = b.clone();
    out.artist = text(&b.artist, "artist", n);
    out.artist_unicode = text(&b.artist_unicode, "artist_unicode", n);
    out.title = text(&b.title, "title", n);
    out.title_unicode = text(&b.title_unicode, "title_unicode", n);
    out.creator = text(&b.creator, "creator", n);
    out.difficulty = text(&b.difficulty, "difficulty", n);
    out.audio_file = text(&b.audio_file, "audio", n);
    out.md5 = md5;
    out.osu_file = text(&b.osu_file, "file", n);
    out.source = text(&b.source, "source", n);
    out.tags = text(&b.tags, "tags", n);
    out.title_font = text(&b.title_font, "font", n);
    out.folder = text(&b.folder, "folder", n);
    out.beatmap_id = anon_id(b.beatmap_id, n);
    out.beatmapset_id = anon_id(b.beatmapset_id, n);
    out.thread_id = 0;
    out.timing_points.truncate(params.max_timing_points);
    out
}

struct ScoreAnonymizer<'a> {
    md5s: &'a mut Md5Map,
    players: BTreeMap<Vec<u8>, OsuString>,
    online_ids: i64,
}

impl ScoreAnonymizer<'_> {
    fn player(&mut self, player: &OsuString) -> OsuString {
        match player.as_bytes() {
            None | Some([]) => player.clone(),
            Some(bytes) => {
                let next = self.players.len() + 1;
                self.players
                    .entry(bytes.to_vec())
                    .or_insert_with(|| OsuString::present(format!("player-{next:02}")))
                    .clone()
            }
        }
    }

    fn online_id(&mut self, id: OnlineId) -> OnlineId {
        let anon = if id.positive().is_some() {
            self.online_ids += 1;
            ANON_ID_BASE + self.online_ids
        } else {
            0
        };
        match id {
            OnlineId::Absent => OnlineId::Absent,
            OnlineId::I32(_) => OnlineId::I32(i32::try_from(anon).unwrap_or(i32::MAX)),
            OnlineId::I64(_) => OnlineId::I64(anon),
        }
    }

    fn score(&mut self, s: &ScoreRecord) -> ScoreRecord {
        let mut out = s.clone();
        out.header.beatmap_md5 = self.md5s.map(&s.header.beatmap_md5);
        out.header.player = self.player(&s.header.player);
        out.header.replay_md5 = self.md5s.map(&s.header.replay_md5);
        out.online_id = self.online_id(s.online_id);
        out
    }
}

pub(crate) fn anonymize(
    corpus: &CorpusDbs,
    selection: &Selection,
    params: &SelectionParams,
) -> DbExtract {
    let mut md5s = Md5Map::default();
    let selected: Vec<&OsuDbBeatmap> = selection
        .beatmaps
        .iter()
        .filter_map(|&i| corpus.osu_db.beatmaps.get(i))
        .collect();
    let beatmaps: Vec<OsuDbBeatmap> = selected
        .iter()
        .enumerate()
        .map(|(i, b)| {
            let md5 = md5s.map(&b.md5);
            anonymize_beatmap(b, i + 1, md5, params)
        })
        .collect();

    let scores = scores_by_md5(&corpus.scores_db);
    let mut anon = ScoreAnonymizer {
        md5s: &mut md5s,
        players: BTreeMap::new(),
        online_ids: 0,
    };
    let mut groups = Vec::new();
    for b in &selected {
        let Some(records) = b.md5.as_bytes().and_then(|m| scores.get(m)) else {
            continue;
        };
        let md5 = anon.md5s.map(&b.md5);
        let scores = records.iter().map(|s| anon.score(s)).collect();
        groups.push(ScoresDbBeatmap { md5, scores });
    }

    let in_osu_db: BTreeSet<&[u8]> = corpus
        .osu_db
        .beatmaps
        .iter()
        .filter_map(|b| b.md5.as_bytes())
        .collect();
    let mut collections = Vec::new();
    for (k, coll) in selection
        .collections
        .iter()
        .filter_map(|&i| corpus.collection_db.collections.get(i))
        .enumerate()
    {
        let mut members = Vec::new();
        let mut dangling = 0;
        for m in &coll.beatmap_md5s {
            if let Some(selected) = md5s.get(m) {
                members.push(selected);
            } else if dangling < params.max_dangling_members
                && !m.as_bytes().is_some_and(|b| in_osu_db.contains(b))
            {
                dangling += 1;
                members.push(md5s.map(m));
            }
        }
        collections.push(Collection {
            name: text(&coll.name, "collection", k + 1),
            beatmap_md5s: members,
        });
    }

    DbExtract {
        osu_db: OsuDb {
            folder_count: i32::try_from(beatmaps.len()).unwrap_or(i32::MAX),
            player_name: OsuString::present(OSU_DB_PLAYER),
            beatmaps,
            ..corpus.osu_db.clone()
        },
        scores_db: ScoresDb {
            version: corpus.scores_db.version,
            beatmaps: groups,
        },
        collection_db: CollectionDb {
            version: corpus.collection_db.version,
            collections,
        },
    }
}

pub(crate) struct Denylist {
    md5s: BTreeSet<Vec<u8>>,
    names: BTreeSet<Vec<u8>>,
}

pub(crate) fn denylist(corpus: &CorpusDbs) -> Denylist {
    let mut md5s = BTreeSet::new();
    let mut add_md5 = |s: &OsuString| {
        if let Some(b) = s.as_bytes().filter(|b| b.len() == MD5_HEX_LEN) {
            md5s.insert(b.to_ascii_lowercase());
        }
    };
    for b in &corpus.osu_db.beatmaps {
        add_md5(&b.md5);
    }
    for group in &corpus.scores_db.beatmaps {
        add_md5(&group.md5);
        for s in &group.scores {
            add_md5(&s.header.beatmap_md5);
            add_md5(&s.header.replay_md5);
        }
    }
    for coll in &corpus.collection_db.collections {
        coll.beatmap_md5s.iter().for_each(&mut add_md5);
    }

    let names = corpus
        .scores_db
        .scores()
        .filter_map(|s| s.header.player.as_bytes())
        .chain(corpus.osu_db.player_name.as_bytes())
        .chain(corpus.extra_names.iter().map(Vec::as_slice))
        .filter(|n| !n.is_empty())
        .map(<[u8]>::to_vec)
        .collect();
    Denylist { md5s, names }
}

fn extract_strings(x: &DbExtract) -> Vec<&OsuString> {
    let mut out = vec![&x.osu_db.player_name];
    for b in &x.osu_db.beatmaps {
        out.extend([
            &b.artist,
            &b.artist_unicode,
            &b.title,
            &b.title_unicode,
            &b.creator,
            &b.difficulty,
            &b.audio_file,
            &b.md5,
            &b.osu_file,
            &b.source,
            &b.tags,
            &b.title_font,
            &b.folder,
        ]);
    }
    for group in &x.scores_db.beatmaps {
        out.push(&group.md5);
        for s in &group.scores {
            let h = &s.header;
            out.extend([&h.beatmap_md5, &h.player, &h.replay_md5, &h.life_bar]);
        }
    }
    for coll in &x.collection_db.collections {
        out.push(&coll.name);
        out.extend(coll.beatmap_md5s.iter());
    }
    out
}

/// Error messages name the file and offset only, never the leaked value (it is private data).
pub(crate) fn check_leaks(
    extract: &DbExtract,
    files: &[OutFile],
    deny: &Denylist,
) -> anyhow::Result<()> {
    for s in extract_strings(extract) {
        if s.as_bytes().is_some_and(|b| deny.names.contains(b)) {
            bail!("an original player or cfg name survived anonymization in a string field");
        }
    }
    for f in files {
        for (offset, window) in f.bytes.windows(MD5_HEX_LEN).enumerate() {
            if deny.md5s.contains(window) {
                bail!("{}: an original md5 appears at byte {offset}", f.path);
            }
        }
        for name in deny
            .names
            .iter()
            .filter(|n| n.len() >= MIN_SCANNED_NAME_LEN)
        {
            if f.bytes.windows(name.len()).any(|w| w == name.as_slice()) {
                bail!(
                    "{}: an original name of {} bytes appears",
                    f.path,
                    name.len()
                );
            }
        }
    }
    Ok(())
}

fn manifest(x: &DbExtract, files: &[OutFile]) -> String {
    let mut m = String::new();
    let _ = writeln!(
        m,
        "# Generated by `cargo xtask fixtures dbs --corpus <osu! dir>` (spec 002 T16); do not edit."
    );
    let _ = writeln!(m, "\n[selection]");
    let _ = writeln!(m, "rule = {:?}", SELECTION_RULE);
    let _ = writeln!(m, "beatmaps = {}", x.osu_db.beatmaps.len());
    let _ = writeln!(m, "scores = {}", x.scores_db.scores().count());
    let _ = writeln!(m, "collections = {}", x.collection_db.collections.len());
    let _ = writeln!(m, "\n[anonymization]");
    let _ = writeln!(m, "rule = {:?}", ANONYMIZATION_RULE);
    let versions = [
        x.osu_db.version,
        x.scores_db.version,
        x.collection_db.version,
    ];
    for (f, version) in files.iter().zip(versions) {
        let _ = writeln!(m, "\n[[file]]");
        let _ = writeln!(m, "path = {:?}", f.path);
        let _ = writeln!(m, "format_version = {version}");
        let _ = writeln!(m, "sha256 = {:?}", sha256_hex(&f.bytes));
    }
    m
}

pub(crate) fn generate(
    corpus: &CorpusDbs,
    params: &SelectionParams,
) -> anyhow::Result<Vec<OutFile>> {
    let selection = select(corpus, params)?;
    let x = anonymize(corpus, &selection, params);
    let mut files = vec![
        OutFile {
            path: format!("osu_db/osu-{}.min.db", x.osu_db.version),
            bytes: encode_osu_db(&x.osu_db),
        },
        OutFile {
            path: format!("scores_db/scores-{}.min.db", x.scores_db.version),
            bytes: encode_scores_db(&x.scores_db),
        },
        OutFile {
            path: format!(
                "collection_db/collection-{}.min.db",
                x.collection_db.version
            ),
            bytes: encode_collection_db(&x.collection_db),
        },
    ];
    // A fixture the decoders reject would only fail later, in a golden test, far from the cause.
    let [osu, scores, collection] = [0, 1, 2].map(|i| files.get(i).map(|f| f.bytes.as_slice()));
    let osu = decode_osu_db(osu.unwrap_or_default()).context("osu!.db extract does not decode")?;
    let scores = decode_scores_db(scores.unwrap_or_default())
        .context("scores.db extract does not decode")?;
    let collection = decode_collection_db(collection.unwrap_or_default())
        .context("collection.db extract does not decode")?;
    ensure!(
        osu.0 == x.osu_db && scores.0 == x.scores_db && collection.0 == x.collection_db,
        "an extract does not round-trip through its decoder"
    );
    check_leaks(&x, &files, &denylist(corpus))?;
    let manifest = manifest(&x, &files);
    files.push(OutFile {
        path: "MANIFEST.toml".into(),
        bytes: manifest.into_bytes(),
    });
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;
    use wolluf_source_osu::codec::OsuString;
    use wolluf_source_osu::codec::collection_db::{Collection, decode_collection_db};
    use wolluf_source_osu::codec::osu_db::decode_osu_db;
    use wolluf_source_osu::codec::score_header::mods;
    use wolluf_source_osu::codec::scores_db::decode_scores_db;
    use wolluf_source_osu::testkit::{BeatmapBuilder, OsuDbBuilder, ScoreBuilder, ScoresDbBuilder};

    const OWNER: &str = "SecretOwner";
    const CFG_NAME: &str = "cfg-login-name";
    const SHORT_NAME: &str = "s";

    const SMALL: SelectionParams = SelectionParams {
        beatmaps: 10,
        max_scores: 40,
        multi_scores: 2,
        collections: 2,
        max_dangling_members: 2,
        max_timing_points: 3,
        modes: &[0, 1, 2, 3],
        mania_keymodes: &[4, 7],
    };

    fn md5(i: usize) -> String {
        format!("{:032x}", 0xabc_0000 + i)
    }

    fn present(s: &str) -> OsuString {
        OsuString::present(s.as_bytes())
    }

    /// md5(i) for i in 0..20. 0: std with 5 timing points, status 2; 1: taiko; 2: catch;
    /// 3: 4K; 4: 7K status 4 with three scores; 5: 7K with one score; the rest are 7K fillers.
    fn corpus() -> CorpusDbs {
        let mut osu = OsuDbBuilder::new().player_name(present(OWNER));
        for i in 0..20 {
            let mut b = BeatmapBuilder::mania(&md5(i), 7).ids(500 + i as i32, 600 + i as i32);
            b = match i {
                0 => (0..5).fold(b.mode(0).ranked_status(2), |b, k| {
                    b.timing_point(120.0, f64::from(k), 1)
                }),
                1 => b.mode(1),
                2 => b.mode(2),
                3 => b.circle_size(4.0),
                4 => b.ranked_status(4),
                _ => b,
            };
            osu = osu.beatmap(b.build());
        }
        let scores = ScoresDbBuilder::new()
            .score(
                ScoreBuilder::mania(&md5(4), "Alice", 1)
                    .mods(mods::SCORE_V2)
                    .build(),
            )
            .score(ScoreBuilder::mania(&md5(4), "", 2).build())
            .score(
                ScoreBuilder::mania(&md5(4), SHORT_NAME, 3)
                    .online_id(77)
                    .build(),
            )
            .score(ScoreBuilder::mania(&md5(5), "Alice", 4).build())
            .score(ScoreBuilder::mania(&md5(99), OWNER, 5).build())
            .build();
        let collection_db = CollectionDb {
            version: 20_260_624,
            collections: vec![
                Collection {
                    name: present("Nothing selectable"),
                    beatmap_md5s: vec![present(&md5(98))],
                },
                Collection {
                    name: present("Faves"),
                    beatmap_md5s: vec![present(&md5(99)), present(&md5(6))],
                },
                Collection {
                    name: present("Other"),
                    beatmap_md5s: vec![present(&md5(2))],
                },
            ],
        };
        CorpusDbs {
            osu_db: osu.build(),
            scores_db: scores,
            collection_db,
            extra_names: vec![CFG_NAME.as_bytes().to_vec()],
        }
    }

    fn selected_md5s(c: &CorpusDbs, s: &Selection) -> Vec<String> {
        s.beatmaps
            .iter()
            .map(|&i| {
                c.osu_db.beatmaps[i]
                    .md5
                    .to_string_lossy()
                    .unwrap()
                    .into_owned()
            })
            .collect()
    }

    #[test]
    fn selection_covers_every_requirement() {
        let c = corpus();
        let s = select(&c, &SMALL).unwrap();
        let md5s = selected_md5s(&c, &s);
        assert_eq!(md5s.len(), SMALL.beatmaps);
        let mut sorted = md5s.clone();
        sorted.sort();
        assert_eq!(md5s, sorted, "selection is ordered by original md5");
        for required in [0, 1, 2, 3, 4, 6] {
            assert!(md5s.contains(&md5(required)), "missing md5({required})");
        }
        assert_eq!(s.collections, vec![0, 1]);
    }

    #[test]
    fn selection_prefers_fewest_scores_then_md5() {
        let c = corpus();
        let s = select(&c, &SMALL).unwrap();
        let md5s = selected_md5s(&c, &s);
        // md5(5) has a score and is not needed by any requirement; zero-score fillers win.
        assert!(!md5s.contains(&md5(5)));
        assert!(md5s.contains(&md5(7)));
    }

    #[test]
    fn selection_fails_when_a_requirement_is_unmet() {
        let mut c = corpus();
        c.osu_db.beatmaps.retain(|b| b.mode != 1);
        let err = select(&c, &SMALL).unwrap_err().to_string();
        assert!(err.contains("mode 1"), "{err}");
    }

    #[test]
    fn selection_rejects_too_many_scores() {
        let c = corpus();
        let params = SelectionParams {
            max_scores: 2,
            ..SMALL
        };
        let err = select(&c, &params).unwrap_err().to_string();
        assert!(err.contains("scores"), "{err}");
    }

    #[test]
    fn anonymization_is_consistent_across_files() {
        let c = corpus();
        let s = select(&c, &SMALL).unwrap();
        let x = anonymize(&c, &s, &SMALL);
        let beatmap_md5s: Vec<OsuString> =
            x.osu_db.beatmaps.iter().map(|b| b.md5.clone()).collect();
        for (n, md5) in beatmap_md5s.iter().enumerate() {
            assert_eq!(md5, &present(&fixture_md5(n + 1)));
        }
        assert_eq!(x.scores_db.beatmaps.len(), 1, "only md5(4) has scores");
        let group = &x.scores_db.beatmaps[0];
        assert!(beatmap_md5s.contains(&group.md5));
        for score in &group.scores {
            assert_eq!(score.header.beatmap_md5, group.md5);
            let replay = score
                .header
                .replay_md5
                .to_string_lossy()
                .unwrap()
                .into_owned();
            let n = (1..1000).find(|&n| fixture_md5(n) == replay);
            assert!(n.is_some_and(|n| n > SMALL.beatmaps), "{replay}");
        }
        // Collection 0 holds only a dangling md5; collection 1 a dangling one and md5(6).
        let members: Vec<Vec<OsuString>> = x
            .collection_db
            .collections
            .iter()
            .map(|c| c.beatmap_md5s.clone())
            .collect();
        let first_dangling = SMALL.beatmaps + x.scores_db.scores().count() + 1;
        let md5_6 = beatmap_md5s[selected_md5s(&c, &s)
            .iter()
            .position(|m| *m == md5(6))
            .unwrap()]
        .clone();
        assert_eq!(
            members,
            vec![
                vec![present(&fixture_md5(first_dangling))],
                vec![present(&fixture_md5(first_dangling + 1)), md5_6],
            ]
        );
        let names: Vec<_> = x
            .collection_db
            .collections
            .iter()
            .map(|c| c.name.clone())
            .collect();
        assert_eq!(
            names,
            vec![present("collection-1"), present("collection-2")]
        );
    }

    #[test]
    fn strings_ids_and_timing_points_are_replaced() {
        let c = corpus();
        let s = select(&c, &SMALL).unwrap();
        let x = anonymize(&c, &s, &SMALL);
        assert_eq!(x.osu_db.version, c.osu_db.version);
        assert_eq!(x.osu_db.player_name, present("fixture"));
        let first = &x.osu_db.beatmaps[0];
        assert_eq!(first.artist, present("artist-1"));
        assert_eq!(first.title, present("title-1"));
        assert_eq!(first.creator, present("creator-1"));
        assert_eq!(first.difficulty, present("difficulty-1"));
        assert_eq!(first.audio_file, present("audio-1"));
        assert_eq!(first.osu_file, present("file-1"));
        assert_eq!(first.folder, present("folder-1"));
        assert_eq!(
            first.artist_unicode,
            OsuString::Absent,
            "absent stays absent"
        );
        assert_eq!(first.beatmap_id, 1001);
        assert_eq!(first.beatmapset_id, 1001);
        assert_eq!(first.thread_id, 0);
        assert!(
            x.osu_db
                .beatmaps
                .iter()
                .all(|b| b.timing_points.len() <= SMALL.max_timing_points)
        );
        assert!(
            x.osu_db
                .beatmaps
                .iter()
                .any(|b| b.timing_points.len() == SMALL.max_timing_points)
        );
    }

    #[test]
    fn players_numbered_by_first_appearance_and_online_ids_offset() {
        let c = corpus();
        let s = select(&c, &SMALL).unwrap();
        let x = anonymize(&c, &s, &SMALL);
        let players: Vec<_> = x
            .scores_db
            .scores()
            .map(|r| r.header.player.clone())
            .collect();
        assert_eq!(
            players,
            vec![present("player-01"), present(""), present("player-02")]
        );
        let ids: Vec<_> = x
            .scores_db
            .scores()
            .map(|r| r.online_id.positive())
            .collect();
        assert_eq!(ids, vec![None, None, Some(1001)]);
    }

    #[test]
    fn leak_check_rejects_original_md5s_and_names() {
        let c = corpus();
        let deny = denylist(&c);
        let s = select(&c, &SMALL).unwrap();
        let x = anonymize(&c, &s, &SMALL);
        let file = |bytes: &[u8]| OutFile {
            path: "t.bin".into(),
            bytes: bytes.to_vec(),
        };
        assert!(check_leaks(&x, &[file(b"clean artist-1 tags-1")], &deny).is_ok());
        let leaked_md5 = format!("xx{}yy", md5(19));
        assert!(check_leaks(&x, &[file(leaked_md5.as_bytes())], &deny).is_err());
        assert!(check_leaks(&x, &[file(OWNER.as_bytes())], &deny).is_err());
        assert!(check_leaks(&x, &[file(CFG_NAME.as_bytes())], &deny).is_err());
        // One-letter names are checked structurally: the byte scan would match any 's'.
        let mut bad = x.clone();
        bad.osu_db.beatmaps[0].tags = present(SHORT_NAME);
        assert!(check_leaks(&bad, &[file(b"")], &deny).is_err());
    }

    #[test]
    fn generation_is_deterministic_and_decodable() {
        let c = corpus();
        let a = generate(&c, &SMALL).unwrap();
        let b = generate(&c, &SMALL).unwrap();
        assert_eq!(a, b);
        let paths: Vec<&str> = a.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(
            paths,
            vec![
                "osu_db/osu-20260924.min.db",
                "scores_db/scores-20260924.min.db",
                "collection_db/collection-20260624.min.db",
                "MANIFEST.toml",
            ]
        );
        decode_osu_db(&a[0].bytes).unwrap();
        decode_scores_db(&a[1].bytes).unwrap();
        decode_collection_db(&a[2].bytes).unwrap();
        let manifest = String::from_utf8(a[3].bytes.clone()).unwrap();
        for f in &a[..3] {
            assert!(manifest.contains(&f.path));
            assert!(manifest.contains(&super::super::sha256_hex(&f.bytes)));
        }
        manifest.parse::<toml::Table>().unwrap();
    }

    #[test]
    fn fixture_md5_is_md5_of_the_tag() {
        // md5("wolluf-fixture:1"), computed independently with `printf %s ... | md5sum`.
        assert_eq!(fixture_md5(1), FIXTURE_1_MD5);
    }

    const FIXTURE_1_MD5: &str = "643833896a402cef06fd6ee5c120211a";
}
