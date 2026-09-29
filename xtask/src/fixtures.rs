//! `cargo xtask fixtures dbs|synthetic` (spec 002 T16): regenerates the committed fixtures under
//! `fixtures/dbs/` and `fixtures/synthetic/`. The corpus is only ever read.

mod dbs;
mod synthetic;

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
use clap::Subcommand;
use sha2::{Digest, Sha256};
use wolluf_source_osu::cfg_files::{list_user_cfgs, read_user_cfg};
use wolluf_source_osu::codec::collection_db::decode_collection_db;
use wolluf_source_osu::codec::osu_db::decode_osu_db;
use wolluf_source_osu::codec::scores_db::decode_scores_db;

#[derive(Subcommand, Debug, Clone, PartialEq, Eq)]
pub(crate) enum FixturesCommand {
    /// Minimized, anonymized extracts of the corpus osu!.db, scores.db and collection.db.
    Dbs {
        /// osu! stable install to read (never written).
        #[arg(long, env = "WOLLUF_CORPUS")]
        corpus: PathBuf,
        /// Defaults to fixtures/dbs.
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Synthetic .osr files built from the source-osu test encoders.
    Synthetic {
        /// Defaults to fixtures/synthetic.
        #[arg(long)]
        out: Option<PathBuf>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OutFile {
    /// Relative to the output directory, `/`-separated.
    pub(crate) path: String,
    pub(crate) bytes: Vec<u8>,
}

pub(crate) fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

/// Suffix of every file a generator owns; stale ones (e.g. an old version in the name) are removed.
const DBS_SUFFIXES: &[&str] = &[".min.db", "MANIFEST.toml"];
const SYNTHETIC_SUFFIXES: &[&str] = &[".osr"];

pub(crate) fn run(root: &Path, command: FixturesCommand) -> anyhow::Result<()> {
    match command {
        FixturesCommand::Dbs { corpus, out } => {
            let out = out.unwrap_or_else(|| root.join("fixtures/dbs"));
            refuse_inside(&out, &corpus)?;
            let files = dbs::generate(&read_corpus(&corpus)?, &dbs::PARAMS)?;
            write_files(&out, &files, DBS_SUFFIXES)
        }
        FixturesCommand::Synthetic { out } => {
            let out = out.unwrap_or_else(|| root.join("fixtures/synthetic"));
            write_files(&out, &synthetic::generate()?, SYNTHETIC_SUFFIXES)
        }
    }
}

fn read_corpus(corpus: &Path) -> anyhow::Result<dbs::CorpusDbs> {
    let read = |name: &str| {
        let path = corpus.join(name);
        fs::read(&path).with_context(|| format!("reading {}", path.display()))
    };
    let (osu_db, _) = decode_osu_db(&read("osu!.db")?).context("decoding osu!.db")?;
    let (scores_db, _) = decode_scores_db(&read("scores.db")?).context("decoding scores.db")?;
    let (collection_db, _) =
        decode_collection_db(&read("collection.db")?).context("decoding collection.db")?;
    let mut extra_names = Vec::new();
    for cfg in list_user_cfgs(corpus).context("listing cfg files")? {
        extra_names.push(cfg.account.clone().into_bytes());
        let (user, _) = read_user_cfg(&cfg.path).context("reading a cfg file")?;
        extra_names.extend(user.username.map(String::into_bytes));
    }
    Ok(dbs::CorpusDbs {
        osu_db,
        scores_db,
        collection_db,
        extra_names,
    })
}

/// The corpus is read-only (CLAUDE.md); an `--out` pointing into it is refused before any write.
fn refuse_inside(out: &Path, corpus: &Path) -> anyhow::Result<()> {
    let corpus = fs::canonicalize(corpus)
        .with_context(|| format!("corpus {} is not readable", corpus.display()))?;
    let mut probe = out.to_path_buf();
    // `out` may not exist yet: canonicalize its nearest existing ancestor.
    let resolved = loop {
        if let Ok(p) = fs::canonicalize(&probe) {
            break p;
        }
        if !probe.pop() {
            break out.to_path_buf();
        }
    };
    if resolved.starts_with(&corpus) {
        bail!(
            "refusing to write fixtures inside the corpus {}",
            corpus.display()
        );
    }
    Ok(())
}

fn write_files(out: &Path, files: &[OutFile], owned_suffixes: &[&str]) -> anyhow::Result<()> {
    let mut keep = BTreeSet::new();
    for f in files {
        let path = out.join(&f.path);
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
        }
        fs::write(&path, &f.bytes).with_context(|| format!("writing {}", path.display()))?;
        keep.insert(path);
    }
    let dirs: BTreeSet<PathBuf> = keep
        .iter()
        .filter_map(|p| p.parent().map(Path::to_path_buf))
        .collect();
    for dir in dirs {
        for entry in fs::read_dir(&dir).with_context(|| format!("listing {}", dir.display()))? {
            let path = entry?.path();
            let owned = path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| owned_suffixes.iter().any(|s| n.ends_with(s)));
            if owned && path.is_file() && !keep.contains(&path) {
                fs::remove_file(&path).with_context(|| format!("removing {}", path.display()))?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(path: &str, bytes: &[u8]) -> OutFile {
        OutFile {
            path: path.into(),
            bytes: bytes.to_vec(),
        }
    }

    #[test]
    fn sha256_hex_matches_known_vector() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn write_files_prunes_only_stale_owned_files() {
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join("osu_db");
        fs::create_dir_all(&sub).unwrap();
        fs::write(sub.join("osu-20250101.min.db"), b"stale").unwrap();
        fs::write(sub.join("README.md"), b"not owned").unwrap();
        write_files(
            dir.path(),
            &[file("osu_db/osu-20260924.min.db", b"new")],
            DBS_SUFFIXES,
        )
        .unwrap();
        assert_eq!(fs::read(sub.join("osu-20260924.min.db")).unwrap(), b"new");
        assert!(!sub.join("osu-20250101.min.db").exists());
        assert!(sub.join("README.md").exists());
    }

    #[test]
    fn refuses_output_inside_the_corpus() {
        let corpus = tempfile::tempdir().unwrap();
        assert!(refuse_inside(&corpus.path().join("fixtures/dbs"), corpus.path()).is_err());
        assert!(refuse_inside(corpus.path(), corpus.path()).is_err());
        let elsewhere = tempfile::tempdir().unwrap();
        assert!(refuse_inside(elsewhere.path(), corpus.path()).is_ok());
    }
}
