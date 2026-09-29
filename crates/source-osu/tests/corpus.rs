#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! Corpus harness (spec 002 AC14, AC15): oracle parity against `research/scripts/oracle/dump.py`,
//! byte-identical round trips, `Data/r` name/header consistency, the cfg username and detection.
//! Every test is `#[ignore]` and reads `WOLLUF_CORPUS` strictly read-only; oracle output and the
//! symlinked corpus views live in tempdirs.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant, SystemTime};

use wolluf_source_osu::cfg_files::{list_user_cfgs, read_user_cfg};
use wolluf_source_osu::codec::OsuString;
use wolluf_source_osu::codec::collection_db::{decode_collection_db, encode_collection_db};
use wolluf_source_osu::codec::osr::{check_name_consistency, decode_osr};
use wolluf_source_osu::codec::osu_db::{OsuDb, decode_osu_db};
use wolluf_source_osu::codec::replay_name::{ReplayFileKind, ReplayFileName};
use wolluf_source_osu::codec::score_header::{OnlineId, ScoreHeader};
use wolluf_source_osu::codec::scores_db::{ScoresDb, decode_scores_db};
use wolluf_source_osu::codec::version::OSU_DB_NEWEST_VERIFIED;
use wolluf_source_osu::install::{CandidateSource, DetectEnv, Platform, detect};
use wolluf_source_osu::paths::is_drvfs_path;

const CORPUS_ENV: &str = "WOLLUF_CORPUS";
const PYTHON_ENV: &str = "WOLLUF_PYTHON";
const DEFAULT_PYTHON: &str = "python3";
const OSU_DB: &str = "osu!.db";
const SCORES_DB: &str = "scores.db";
const COLLECTION_DB: &str = "collection.db";
const REPLAY_DIR: &str = "Data/r";
/// Detection never looks inside Songs, and walking ~8k beatmap folders on drvfs takes minutes, so
/// the before/after check records the Songs directory itself but not its subtree.
const SONGS_DIR: &str = "Songs";
/// Spec 002 AC14 names the pilot's cfg string explicitly: it is the garbage login 004 must cope with.
const PILOT_CFG_USERNAME: &str = "TWulfZasdasdasd d jSS||";
const OSU_DB_DECODE_BUDGET: Duration = Duration::from_secs(1);
const SCORES_DB_DECODE_BUDGET: Duration = Duration::from_millis(50);
/// Best of several runs, so one scheduler hiccup on a busy machine does not fail the budget.
const TIMING_RUNS: usize = 3;
const MAX_REPORTED: usize = 20;
#[cfg(not(feature = "test-support"))]
const NEEDS_TEST_SUPPORT: &str =
    "round trips need the testkit encoders: run with --all-features (or --features test-support)";

fn corpus() -> PathBuf {
    let Some(raw) = std::env::var_os(CORPUS_ENV) else {
        panic!(
            "{CORPUS_ENV} is unset; corpus tests need an osu! stable install, e.g. \
             {CORPUS_ENV}=\"/mnt/e/Games/osu!\" cargo nextest run -p wolluf-source-osu \
             --all-features --release --run-ignored only"
        );
    };
    let root = PathBuf::from(raw);
    assert!(
        root.join(OSU_DB).is_file(),
        "{CORPUS_ENV}={} has no {OSU_DB}",
        root.display()
    );
    root
}

fn read(root: &Path, rel: &str) -> Vec<u8> {
    let path = root.join(rel);
    fs::read(&path).unwrap_or_else(|e| panic!("reading {}: {e}", path.display()))
}

/// Sorted `.osr`/`.osg` names in `Data/r`, listed independently of `replay_dir::index` (which
/// skips names it cannot parse, the very thing these tests must see).
fn replay_dir_names(root: &Path) -> Vec<String> {
    let dir = root.join(REPLAY_DIR);
    let mut names: Vec<String> = fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("listing {}: {e}", dir.display()))
        .map(|entry| {
            let entry = entry.unwrap();
            assert!(
                entry.file_type().unwrap().is_file(),
                "{:?} is not a file",
                entry.path()
            );
            entry
                .file_name()
                .into_string()
                .unwrap_or_else(|n| panic!("non-UTF-8 name in {REPLAY_DIR}: {n:?}"))
        })
        .collect();
    names.sort();
    names
}

fn assert_none_reported(what: &str, mismatches: &[String]) {
    assert!(
        mismatches.is_empty(),
        "{what}: {} mismatches; first {}:\n{}",
        mismatches.len(),
        MAX_REPORTED.min(mismatches.len()),
        mismatches[..MAX_REPORTED.min(mismatches.len())].join("\n")
    );
}

fn assert_same_bytes(what: &str, original: &[u8], encoded: &[u8]) {
    if original == encoded {
        return;
    }
    let first_diff = original
        .iter()
        .zip(encoded)
        .position(|(a, b)| a != b)
        .unwrap_or(original.len().min(encoded.len()));
    panic!(
        "{what} re-encodes differently: original {} bytes, encoded {} bytes, first difference \
         at offset {first_diff}",
        original.len(),
        encoded.len()
    );
}

// --- oracle ---------------------------------------------------------------------------------

/// Runs `dump.py` on a view of the corpus holding only `inputs` (symlinks), so a test does not
/// pay for reading the 5k replays it will not compare. The view and output live in a tempdir;
/// dropping it removes the symlinks, never their targets.
struct Oracle {
    _dir: tempfile::TempDir,
    out: PathBuf,
}

impl Oracle {
    fn run(corpus: &Path, inputs: &[&str]) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let view = corpus_view(corpus, dir.path(), inputs);
        let out = dir.path().join("oracle");
        let script =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../research/scripts/oracle/dump.py");
        let python = std::env::var(PYTHON_ENV).unwrap_or_else(|_| DEFAULT_PYTHON.to_owned());
        let output = Command::new(&python)
            .arg(&script)
            .arg(&view)
            .arg(&out)
            // T14: no __pycache__ may land in research/.
            .env("PYTHONDONTWRITEBYTECODE", "1")
            .output()
            .unwrap_or_else(|e| panic!("spawning {python} (override with {PYTHON_ENV}): {e}"));
        assert!(
            output.status.success(),
            "dump.py failed ({}):\n{}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
        Self { _dir: dir, out }
    }

    fn rows(&self, file: &str) -> Vec<Row> {
        let path = self.out.join(file);
        fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("reading {}: {e}", path.display()))
            .lines()
            .enumerate()
            .map(|(i, line)| {
                Row(parse_object(line).unwrap_or_else(|e| panic!("{file}:{}: {e}", i + 1)))
            })
            .collect()
    }

    fn meta(&self) -> Row {
        let text = fs::read_to_string(self.out.join("meta.json")).unwrap();
        Row(parse_object(&text).unwrap_or_else(|e| panic!("meta.json: {e}")))
    }
}

#[cfg(unix)]
fn corpus_view(corpus: &Path, tmp: &Path, inputs: &[&str]) -> PathBuf {
    let view = tmp.join("corpus-view");
    for rel in inputs {
        let link = view.join(rel);
        fs::create_dir_all(link.parent().unwrap()).unwrap();
        std::os::unix::fs::symlink(corpus.join(rel), &link).unwrap();
    }
    view
}

/// Without symlinks the oracle reads the whole corpus; slower, same result.
#[cfg(not(unix))]
fn corpus_view(corpus: &Path, _tmp: &Path, _inputs: &[&str]) -> PathBuf {
    corpus.to_path_buf()
}

// --- flat JSON rows -------------------------------------------------------------------------
// dump.py rows are flat objects of strings, numbers, booleans and null. This parser covers
// exactly that because the crate has no JSON dev-dependency.

#[derive(Debug, Clone)]
enum Json {
    Null,
    Bool(bool),
    /// Raw token, parsed by the side that knows the expected type (i64 ticks do not fit f64).
    Num(String),
    Str(String),
}

struct Row(BTreeMap<String, Json>);

impl Row {
    fn int(&self, key: &str) -> i64 {
        match self.0.get(key) {
            Some(Json::Num(raw)) => raw.parse().unwrap_or_else(|e| panic!("{key}={raw}: {e}")),
            other => panic!("{key}: expected an integer, got {other:?}"),
        }
    }

    fn file(&self) -> &str {
        match self.0.get("file") {
            Some(Json::Str(s)) => s,
            other => panic!("file: expected a string, got {other:?}"),
        }
    }
}

fn parse_object(text: &str) -> Result<BTreeMap<String, Json>, String> {
    let mut p = JsonCursor {
        chars: text.char_indices().peekable(),
    };
    let mut out = BTreeMap::new();
    p.expect('{')?;
    if p.peek() == Some('}') {
        p.bump();
    } else {
        loop {
            let key = p.string()?;
            p.expect(':')?;
            let value = p.value()?;
            out.insert(key, value);
            match p.next_token()? {
                ',' => continue,
                '}' => break,
                c => return Err(format!("expected , or }} got {c:?}")),
            }
        }
    }
    p.skip_ws();
    match p.chars.next() {
        None => Ok(out),
        Some((at, c)) => Err(format!("trailing {c:?} at {at}")),
    }
}

struct JsonCursor<'a> {
    chars: std::iter::Peekable<std::str::CharIndices<'a>>,
}

impl JsonCursor<'_> {
    fn skip_ws(&mut self) {
        while self
            .chars
            .next_if(|(_, c)| c.is_ascii_whitespace())
            .is_some()
        {}
    }

    fn peek(&mut self) -> Option<char> {
        self.skip_ws();
        self.chars.peek().map(|&(_, c)| c)
    }

    fn bump(&mut self) -> Option<char> {
        self.chars.next().map(|(_, c)| c)
    }

    fn next_token(&mut self) -> Result<char, String> {
        self.skip_ws();
        self.bump().ok_or_else(|| "unexpected end".to_owned())
    }

    fn expect(&mut self, want: char) -> Result<(), String> {
        match self.next_token()? {
            c if c == want => Ok(()),
            c => Err(format!("expected {want:?} got {c:?}")),
        }
    }

    fn value(&mut self) -> Result<Json, String> {
        match self.peek() {
            Some('"') => self.string().map(Json::Str),
            Some(_) => {
                let mut word = String::new();
                while let Some((_, c)) = self
                    .chars
                    .next_if(|&(_, c)| !matches!(c, ',' | '}') && !c.is_ascii_whitespace())
                {
                    word.push(c);
                }
                Ok(match word.as_str() {
                    "null" => Json::Null,
                    "true" => Json::Bool(true),
                    "false" => Json::Bool(false),
                    _ => Json::Num(word),
                })
            }
            None => Err("unexpected end".to_owned()),
        }
    }

    fn string(&mut self) -> Result<String, String> {
        self.expect('"')?;
        let mut s = String::new();
        loop {
            match self.bump().ok_or("unterminated string")? {
                '"' => return Ok(s),
                '\\' => match self.bump().ok_or("unterminated escape")? {
                    '"' => s.push('"'),
                    '\\' => s.push('\\'),
                    '/' => s.push('/'),
                    'b' => s.push('\u{8}'),
                    'f' => s.push('\u{c}'),
                    'n' => s.push('\n'),
                    'r' => s.push('\r'),
                    't' => s.push('\t'),
                    'u' => {
                        let hi = self.hex4()?;
                        let code = if (0xD800..0xDC00).contains(&hi) {
                            self.expect('\\')?;
                            self.expect('u')?;
                            let lo = self.hex4()?;
                            0x10000 + ((hi - 0xD800) << 10) + (lo - 0xDC00)
                        } else {
                            hi
                        };
                        s.push(char::from_u32(code).ok_or_else(|| format!("bad \\u{code:x}"))?);
                    }
                    c => return Err(format!("bad escape \\{c}")),
                },
                c => s.push(c),
            }
        }
    }

    fn hex4(&mut self) -> Result<u32, String> {
        let digits: String = (0..4).filter_map(|_| self.bump()).collect();
        u32::from_str_radix(&digits, 16).map_err(|e| format!("\\u{digits}: {e}"))
    }
}

// --- field comparison -----------------------------------------------------------------------

/// The Rust side of one compared record; oracle values are parsed into the same variant.
#[derive(Debug, Clone)]
enum Val {
    Null,
    Int(i64),
    Float(f64),
    Str(String),
}

impl PartialEq for Val {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Null, Self::Null) => true,
            (Self::Int(a), Self::Int(b)) => a == b,
            (Self::Float(a), Self::Float(b)) => a == b || (a.is_nan() && b.is_nan()),
            (Self::Str(a), Self::Str(b)) => a == b,
            _ => false,
        }
    }
}

impl Val {
    fn oracle(like: &Self, json: &Json) -> Self {
        match (like, json) {
            (_, Json::Null) => Self::Null,
            (_, Json::Str(s)) => Self::Str(s.clone()),
            (_, Json::Bool(b)) => Self::Int(i64::from(*b)),
            (Self::Float(_), Json::Num(raw)) => raw
                .parse()
                .map_or_else(|_| Self::Str(raw.clone()), Self::Float),
            (_, Json::Num(raw)) => raw
                .parse()
                .map_or_else(|_| Self::Str(raw.clone()), Self::Int),
        }
    }
}

/// Python decodes with `errors='replace'`, the same maximal-subpart policy as `from_utf8_lossy`.
fn lossy(s: &OsuString) -> Val {
    s.to_string_lossy()
        .map_or(Val::Null, |c| Val::Str(c.into_owned()))
}

/// The audit reader maps an absent string to `''`.
fn lossy_or_empty(s: &OsuString) -> Val {
    Val::Str(s.to_string_lossy().unwrap_or_default().into_owned())
}

fn int(v: impl Into<i64>) -> Val {
    Val::Int(v.into())
}

/// Every oracle key must be compared (so a field the oracle gains cannot be skipped silently),
/// apart from bookkeeping keys in `ignore`.
fn compare(
    label: &str,
    rust: &BTreeMap<&str, Val>,
    oracle: &Row,
    ignore: &[&str],
    out: &mut Vec<String>,
) {
    let oracle_keys: BTreeSet<&str> = oracle
        .0
        .keys()
        .map(String::as_str)
        .filter(|k| !ignore.contains(k))
        .collect();
    let rust_keys: BTreeSet<&str> = rust.keys().copied().collect();
    if oracle_keys != rust_keys {
        out.push(format!(
            "{label}: key sets differ: rust {rust_keys:?} oracle {oracle_keys:?}"
        ));
        return;
    }
    for (key, value) in rust {
        let theirs = Val::oracle(value, &oracle.0[*key]);
        if *value != theirs {
            out.push(format!("{label} {key}: rust {value:?} oracle {theirs:?}"));
        }
    }
}

fn score_header_fields(h: &ScoreHeader) -> BTreeMap<&'static str, Val> {
    BTreeMap::from([
        ("mode", int(h.mode)),
        ("version", int(h.version)),
        ("beatmap_md5", lossy_or_empty(&h.beatmap_md5)),
        ("player", lossy_or_empty(&h.player)),
        ("replay_md5", lossy_or_empty(&h.replay_md5)),
        ("n300", int(h.counts.n300)),
        ("n100", int(h.counts.n100)),
        ("n50", int(h.counts.n50)),
        ("geki", int(h.counts.geki)),
        ("katu", int(h.counts.katu)),
        ("miss", int(h.counts.miss)),
        ("score", int(h.score)),
        ("max_combo", int(h.max_combo)),
        ("perfect", int(h.perfect)),
        // The oracle reads mods as a signed Int.
        ("mods", int(h.mods.cast_signed())),
        ("lifebar", lossy_or_empty(&h.life_bar)),
        ("ticks", int(h.timestamp_ticks)),
    ])
}

fn online_id_value(id: OnlineId) -> Val {
    match id {
        OnlineId::Absent => Val::Null,
        OnlineId::I32(v) => int(v),
        OnlineId::I64(v) => int(v),
    }
}

// --- AC14: oracle parity --------------------------------------------------------------------

#[test]
#[ignore = "corpus: needs WOLLUF_CORPUS"]
fn osu_db_matches_python_oracle() {
    let root = corpus();
    let oracle = Oracle::run(&root, &[OSU_DB]);
    let (db, _) = decode_osu_db(&read(&root, OSU_DB)).unwrap();
    assert_eq!(i64::from(db.version), oracle.meta().int("osu_db_version"));
    let rows = oracle.rows("osu_db.jsonl");
    assert_eq!(db.beatmaps.len(), rows.len(), "entry count");
    let mut mismatches = Vec::new();
    for (i, (b, row)) in db.beatmaps.iter().zip(&rows).enumerate() {
        assert_eq!(
            row.int("idx"),
            i64::try_from(i).unwrap(),
            "oracle rows out of order"
        );
        let rust = BTreeMap::from([
            ("md5", lossy(&b.md5)),
            ("folder", lossy(&b.folder)),
            ("file", lossy(&b.osu_file)),
            ("mode", int(b.mode)),
            ("cs", Val::Float(f64::from(b.circle_size))),
            ("od", Val::Float(f64::from(b.overall_difficulty))),
            ("hp", Val::Float(f64::from(b.hp_drain))),
            ("artist", lossy(&b.artist)),
            ("title", lossy(&b.title)),
            ("diff", lossy(&b.difficulty)),
            ("creator", lossy(&b.creator)),
            // rejudge/osudb.py's `bid` is the second id Int, i.e. the beatmap *set* id.
            ("bid", int(b.beatmapset_id)),
        ]);
        compare(&format!("entry {i}"), &rust, row, &["idx"], &mut mismatches);
    }
    assert_none_reported("osu!.db vs oracle", &mismatches);
}

#[test]
#[ignore = "corpus: needs WOLLUF_CORPUS"]
fn scores_db_matches_python_oracle() {
    let root = corpus();
    let oracle = Oracle::run(&root, &[OSU_DB, SCORES_DB]);
    let (db, _) = decode_scores_db(&read(&root, SCORES_DB)).unwrap();
    let meta = oracle.meta();
    assert_eq!(i64::from(db.version), meta.int("scores_db_version"));
    assert_eq!(
        i64::try_from(db.beatmaps.len()).unwrap(),
        meta.int("scores_db_beatmaps")
    );
    let rows = oracle.rows("scores_db.jsonl");
    let records: Vec<_> = db
        .beatmaps
        .iter()
        .flat_map(|b| b.scores.iter().map(move |s| (&b.md5, s)))
        .collect();
    assert_eq!(records.len(), rows.len(), "score count");
    let mut mismatches = Vec::new();
    for (i, ((group_md5, s), row)) in records.iter().zip(&rows).enumerate() {
        assert_eq!(
            row.int("idx"),
            i64::try_from(i).unwrap(),
            "oracle rows out of order"
        );
        let mut rust = score_header_fields(&s.header);
        rust.insert("online_id", online_id_value(s.online_id));
        rust.insert("db_md5", lossy_or_empty(group_md5));
        if let Some(tp) = s.target_practice {
            rust.insert("tp_acc", Val::Float(tp));
        }
        compare(&format!("score {i}"), &rust, row, &["idx"], &mut mismatches);
    }
    assert_none_reported("scores.db vs oracle", &mismatches);
}

#[test]
#[ignore = "corpus: needs WOLLUF_CORPUS"]
fn osr_headers_match_python_oracle() {
    let root = corpus();
    let oracle = Oracle::run(&root, &[OSU_DB, REPLAY_DIR]);
    let by_file: BTreeMap<String, Row> = oracle
        .rows("osr_headers.jsonl")
        .into_iter()
        .map(|row| (row.file().to_owned(), row))
        .collect();
    let osr_names: Vec<String> = replay_dir_names(&root)
        .into_iter()
        .filter(|n| n.ends_with(".osr"))
        .collect();
    let ours: BTreeSet<&str> = osr_names.iter().map(String::as_str).collect();
    let theirs: BTreeSet<&str> = by_file.keys().map(String::as_str).collect();
    assert_eq!(
        ours, theirs,
        "the oracle and the test list different .osr files"
    );
    let mut mismatches = Vec::new();
    for name in &osr_names {
        let bytes = read(&root.join(REPLAY_DIR), name);
        match decode_osr(&bytes) {
            Ok((osr, _)) => compare(
                name,
                &score_header_fields(&osr.header),
                &by_file[name],
                &["file"],
                &mut mismatches,
            ),
            Err(e) => mismatches.push(format!("{name}: decode failed: {e}")),
        }
    }
    assert_none_reported(".osr headers vs oracle", &mismatches);
}

// --- AC14: round trips ----------------------------------------------------------------------

#[cfg(feature = "test-support")]
fn encode_osu_db(db: &OsuDb) -> Vec<u8> {
    wolluf_source_osu::testkit::encode_osu_db(db)
}

#[cfg(not(feature = "test-support"))]
fn encode_osu_db(_: &OsuDb) -> Vec<u8> {
    panic!("{NEEDS_TEST_SUPPORT}")
}

#[cfg(feature = "test-support")]
fn encode_scores_db(db: &ScoresDb) -> Vec<u8> {
    wolluf_source_osu::testkit::encode_scores_db(db)
}

#[cfg(not(feature = "test-support"))]
fn encode_scores_db(_: &ScoresDb) -> Vec<u8> {
    panic!("{NEEDS_TEST_SUPPORT}")
}

#[test]
#[ignore = "corpus: needs WOLLUF_CORPUS"]
fn osu_db_roundtrip_byte_identical() {
    let bytes = read(&corpus(), OSU_DB);
    let (db, _) = decode_osu_db(&bytes).unwrap();
    assert_same_bytes(OSU_DB, &bytes, &encode_osu_db(&db));
}

#[test]
#[ignore = "corpus: needs WOLLUF_CORPUS"]
fn scores_db_roundtrip_byte_identical() {
    let bytes = read(&corpus(), SCORES_DB);
    let (db, _) = decode_scores_db(&bytes).unwrap();
    assert_same_bytes(SCORES_DB, &bytes, &encode_scores_db(&db));
}

#[test]
#[ignore = "corpus: needs WOLLUF_CORPUS"]
fn collection_db_roundtrip_byte_identical() {
    let bytes = read(&corpus(), COLLECTION_DB);
    let (db, _) = decode_collection_db(&bytes).unwrap();
    assert_same_bytes(COLLECTION_DB, &bytes, &encode_collection_db(&db));
}

// --- AC14: names, cfg, detection ------------------------------------------------------------

#[test]
#[ignore = "corpus: needs WOLLUF_CORPUS"]
fn data_r_names_consistent_with_headers() {
    let root = corpus();
    let dir = root.join(REPLAY_DIR);
    let names = replay_dir_names(&root);
    assert!(!names.is_empty(), "{} is empty", dir.display());
    let mut mismatches = Vec::new();
    for name in &names {
        let Some(parsed) = ReplayFileName::parse(name) else {
            mismatches.push(format!("{name}: does not parse"));
            continue;
        };
        if parsed.format() != *name {
            mismatches.push(format!("{name}: formats back as {}", parsed.format()));
        }
        if parsed.kind != ReplayFileKind::Osr {
            continue;
        }
        match decode_osr(&read(&dir, name)) {
            Ok((osr, _)) => {
                for d in check_name_consistency(&parsed, &osr.header).iter() {
                    mismatches.push(format!("{name}: {:?}", d.code));
                }
            }
            Err(e) => mismatches.push(format!("{name}: decode failed: {e}")),
        }
    }
    assert_none_reported("Data/r names vs headers", &mismatches);
}

#[test]
#[ignore = "corpus: needs WOLLUF_CORPUS"]
fn cfg_username_is_verbatim() {
    let root = corpus();
    let cfgs = list_user_cfgs(&root).unwrap();
    let newest = cfgs.first().expect("no osu!.<account>.cfg in the corpus");
    let (cfg, _) = read_user_cfg(&newest.path).unwrap();
    assert_eq!(cfg.username.as_deref(), Some(PILOT_CFG_USERNAME));
}

type TreeState = BTreeMap<PathBuf, (u64, Option<SystemTime>)>;

/// Length and mtime of every entry under `root`, without following symlinks.
fn tree_state(root: &Path) -> TreeState {
    fn walk(root: &Path, dir: &Path, out: &mut TreeState) {
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            let meta = fs::symlink_metadata(&path).unwrap();
            out.insert(path.clone(), (meta.len(), meta.modified().ok()));
            if meta.is_dir() && path != root.join(SONGS_DIR) {
                walk(root, &path, out);
            }
        }
    }
    let mut out = TreeState::new();
    walk(root, root, &mut out);
    out
}

/// `<drive>/Games/osu!` or `<drive>/osu!`: the two layouts the drive scan probes.
fn drive_root_of(install: &Path) -> Option<PathBuf> {
    let parent = install.parent()?;
    if parent.file_name()? == "Games" {
        parent.parent().map(Path::to_path_buf)
    } else {
        Some(parent.to_path_buf())
    }
}

#[test]
#[ignore = "corpus: needs WOLLUF_CORPUS"]
fn detect_finds_corpus_install() {
    let root = fs::canonicalize(corpus()).unwrap();
    let drive =
        drive_root_of(&root).expect("the corpus must sit at <drive>/Games/osu! or <drive>/osu!");
    let env = DetectEnv {
        platform: if is_drvfs_path(&root) {
            Platform::Wsl
        } else {
            Platform::Linux
        },
        drive_roots: vec![drive],
        ..DetectEnv::default()
    };
    let before = tree_state(&root);
    let found = detect(&env);
    let after = tree_state(&root);

    let (candidate, info) = found
        .iter()
        .find_map(|(c, r)| {
            let same = fs::canonicalize(&c.root).is_ok_and(|p| p == root);
            same.then_some((c, r))
        })
        .unwrap_or_else(|| panic!("{} not among candidates {found:?}", root.display()));
    assert_eq!(candidate.source, CandidateSource::DriveScan);
    let info = info
        .as_ref()
        .unwrap_or_else(|e| panic!("corpus install invalid: {e}"));
    // A newer header means osu! updated: verify it and bump the ADR 0015 constant.
    assert_eq!(info.osu_db_version, OSU_DB_NEWEST_VERIFIED);

    let changed: Vec<_> = before
        .keys()
        .chain(after.keys())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .filter(|p| before.get(*p) != after.get(*p))
        .collect();
    assert!(
        changed.is_empty(),
        "entries under the corpus changed during detection (is osu! running?): {changed:?}"
    );
}

// --- AC15: speed ----------------------------------------------------------------------------

fn best_decode_time(bytes: &[u8], decode: impl Fn(&[u8])) -> Duration {
    (0..TIMING_RUNS)
        .map(|_| {
            let start = Instant::now();
            decode(bytes);
            start.elapsed()
        })
        .min()
        .unwrap()
}

fn assert_within_budget(what: &str, took: Duration, budget: Duration) {
    if cfg!(debug_assertions) {
        eprintln!(
            "{what}: {took:?}; the {budget:?} budget applies to release builds only (spec 002 AC15)"
        );
        return;
    }
    assert!(took < budget, "{what} took {took:?}, budget {budget:?}");
}

#[test]
#[ignore = "corpus: needs WOLLUF_CORPUS"]
fn osu_db_decode_under_1s() {
    let bytes = read(&corpus(), OSU_DB);
    let took = best_decode_time(&bytes, |b| {
        let (db, _) = decode_osu_db(b).unwrap();
        assert!(!db.beatmaps.is_empty());
    });
    assert_within_budget("osu!.db decode", took, OSU_DB_DECODE_BUDGET);
}

#[test]
#[ignore = "corpus: needs WOLLUF_CORPUS"]
fn scores_db_decode_under_50ms() {
    let bytes = read(&corpus(), SCORES_DB);
    let took = best_decode_time(&bytes, |b| {
        let (db, _) = decode_scores_db(b).unwrap();
        assert!(db.scores().next().is_some());
    });
    assert_within_budget("scores.db decode", took, SCORES_DB_DECODE_BUDGET);
}
