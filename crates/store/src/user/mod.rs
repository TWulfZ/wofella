//! user.db: the irreplaceable ledger (architecture §5.2–§5.3, spec 003). Forward-only
//! migrations, a `VACUUM INTO` backup before each one, and refusal to open a newer schema.

use std::path::{Path, PathBuf};

use rusqlite::{Connection, OpenFlags};
use rusqlite_migration::{M, Migrations};
use wolluf_core::UnixUs;

use crate::db::{Conn, DbHandle, DbKind};
use crate::error::StoreError;
use crate::time::format_compact_utc;

/// Append only: a released entry is never edited, because `fixtures/userdb/vN.db` pins it.
const USER_MIGRATIONS: &[M<'static>] = &[M::up(include_str!("migrations/0001_init.sql"))];

pub const USER_SCHEMA_LATEST: u32 = USER_MIGRATIONS.len() as u32;

/// Backups older than this many migrations are pruned (spec 003).
pub const BACKUPS_KEPT: usize = 5;

const BACKUP_PREFIX: &str = "user-v";
const BACKUP_SUFFIX: &str = ".db";
const BACKUP_STAMP_LEN: usize = "20260928T231356Z".len();

const META_SCHEMA_CREATED_WITH: &str = "schema_created_with";
/// The workspace shares one version, so the store's version is the app's.
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn open_user_db(path: &Path, backups_dir: &Path, now: UnixUs) -> Result<DbHandle, StoreError> {
    open_with(path, backups_dir, now, USER_MIGRATIONS)
}

fn open_with(
    path: &Path,
    backups_dir: &Path,
    now: UnixUs,
    migrations: &'static [M<'static>],
) -> Result<DbHandle, StoreError> {
    let latest = schema_len(migrations)?;
    // Checked on a read-only connection first: enabling WAL would rewrite the header of a file
    // a newer wolluf owns, and "refuse" must mean "touch nothing".
    if path.exists() {
        let probe = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        refuse_newer(user_version(&probe)?, latest)?;
    }
    DbHandle::open(path, DbKind::User, |conn| {
        let from = user_version(conn)?;
        refuse_newer(from, latest)?;
        if from > 0 && from < latest {
            backup(conn, backups_dir, from, now)?;
        }
        Migrations::from_slice(migrations).to_latest(conn)?;
        conn.execute(
            "INSERT OR IGNORE INTO meta (key, value) VALUES (?1, ?2)",
            (META_SCHEMA_CREATED_WITH, APP_VERSION),
        )?;
        Ok(())
    })
}

/// For diagnostics exports (architecture §7), which report schema versions.
pub fn schema_version(conn: Conn<'_>) -> Result<u32, StoreError> {
    user_version(conn.0)
}

fn schema_len(migrations: &[M<'_>]) -> Result<u32, StoreError> {
    u32::try_from(migrations.len())
        .map_err(|_| StoreError::InvalidData("migration count exceeds u32".into()))
}

fn user_version(conn: &Connection) -> Result<u32, StoreError> {
    let v: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    u32::try_from(v).map_err(|_| StoreError::InvalidData(format!("user_version {v}")))
}

fn refuse_newer(found: u32, latest: u32) -> Result<(), StoreError> {
    if found > latest {
        return Err(StoreError::SchemaTooNew { found, latest });
    }
    Ok(())
}

fn backup(conn: &Connection, dir: &Path, from: u32, now: UnixUs) -> Result<(), StoreError> {
    std::fs::create_dir_all(dir).map_err(|e| StoreError::io(dir, e))?;
    let target = dir.join(format!(
        "{BACKUP_PREFIX}{from}-{}{BACKUP_SUFFIX}",
        format_compact_utc(now)
    ));
    let target_text = target
        .to_str()
        .ok_or_else(|| StoreError::InvalidData(format!("non-UTF-8 backup path {target:?}")))?;
    conn.execute("VACUUM INTO ?1", [target_text])?;
    prune_backups(dir)
}

/// The stamp sorts chronologically as text, so the newest backups are the largest stamps.
fn backup_stamp(name: &str) -> Option<&str> {
    let rest = name
        .strip_prefix(BACKUP_PREFIX)?
        .strip_suffix(BACKUP_SUFFIX)?;
    let (version, stamp) = rest.split_once('-')?;
    let well_formed = !version.is_empty()
        && version.bytes().all(|b| b.is_ascii_digit())
        && stamp.len() == BACKUP_STAMP_LEN;
    well_formed.then_some(stamp)
}

fn prune_backups(dir: &Path) -> Result<(), StoreError> {
    let entries = std::fs::read_dir(dir).map_err(|e| StoreError::io(dir, e))?;
    let mut backups: Vec<(String, PathBuf)> = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| StoreError::io(dir, e))?;
        let name = entry.file_name();
        if let Some(stamp) = name.to_str().and_then(backup_stamp) {
            backups.push((stamp.to_owned(), entry.path()));
        }
    }
    backups.sort_by(|a, b| b.cmp(a));
    for (_, path) in backups.into_iter().skip(BACKUPS_KEPT) {
        std::fs::remove_file(&path).map_err(|e| StoreError::io(&path, e))?;
    }
    Ok(())
}

#[cfg(test)]
pub(crate) mod testkit {
    //! Seed data shared by the migration, fixture and ledger-guard tests. Synthetic names only.

    use rusqlite::Connection;
    use rusqlite_migration::Migrations;

    use super::USER_MIGRATIONS;

    pub(crate) const PLAY_A: [u8; 32] = [0xa1; 32];
    pub(crate) const PLAY_B: [u8; 32] = [0xb2; 32];
    pub(crate) const BLOB_OSR: [u8; 32] = [0x01; 32];
    pub(crate) const BLOB_OSR_2: [u8; 32] = [0x02; 32];
    pub(crate) const BLOB_OSG: [u8; 32] = [0x03; 32];
    pub(crate) const SNAPSHOT_SHA: [u8; 32] = [0x5a; 32];
    pub(crate) const TS: &str = "2026-09-28T23:13:56.636Z";

    pub(crate) fn migrated() -> Connection {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "foreign_keys", true).unwrap();
        Migrations::from_slice(USER_MIGRATIONS)
            .to_latest(&mut conn)
            .unwrap();
        conn
    }

    /// One row in every table: a `scores_db` play (PLAY_A, no links) and a `replay_only` play
    /// (PLAY_B), plus the empty and a non-UTF-8 alias.
    pub(crate) fn seed(conn: &Connection) {
        conn.execute_batch(&format!(
            "
            INSERT INTO meta VALUES ('fixture', 'v1');
            INSERT INTO install VALUES ('00000000-0000-4000-8000-000000000001', zeroblob(32), '{TS}');
            INSERT INTO game_install VALUES (1, 'osu_stable', '/osu', 20260924, '{TS}');
            INSERT INTO source_snapshot VALUES (1, 1, 'scores_db', X'{snap}', 650000, '{TS}', 20260924, '{TS}');
            INSERT INTO blob VALUES (X'{osr}', 'osr', 1234, 'Data/r/a.osr', '{TS}');
            INSERT INTO blob VALUES (X'{osr2}', 'osr', 1234, 'Data/r/b.osr', '{TS}');
            INSERT INTO blob VALUES (X'{osg}', 'osg', 5678, 'Data/r/a.osg', '{TS}');
            INSERT INTO alias VALUES (1, 'osu_stable', X'');
            INSERT INTO alias VALUES (2, 'osu_stable', CAST('Alice' AS BLOB));
            INSERT INTO alias VALUES (3, 'osu_stable', X'FF41');
            INSERT INTO play VALUES (X'{a}', 2, 'e956977ccc1d74a50ae48b43a868cc20', 'scores_db',
                '134350010443098880', '{TS}', 536870912, 'v2',
                '{{\"max\":10,\"n300\":5,\"n200\":1,\"n100\":0,\"n50\":0,\"miss\":1}}', 12, 900000,
                0.95, NULL, '4567890123', 20260924, NULL, NULL, NULL, 1, '{TS}');
            INSERT INTO play VALUES (X'{b}', 1, 'e956977ccc1d74a50ae48b43a868cc20', 'replay_only',
                '134350010443098881', '{TS}', 0, 'v1',
                '{{\"max\":1,\"n300\":0,\"n200\":0,\"n100\":0,\"n50\":0,\"miss\":0}}', 1, 1000,
                1.0, NULL, NULL, 20260924, X'{osr2}', NULL, NULL, NULL, '{TS}');
            INSERT INTO identity_decision VALUES (2, 'me', '{TS}');
            INSERT INTO profile VALUES (1, 'self', 'Me', 1, 'merged', '{TS}');
            INSERT INTO profile_alias VALUES (1, 2, 'user', '{TS}');
            INSERT INTO feedback_event VALUES ('01J00000000000000000000000', '{TS}', 1,
                'identity_decision', '{{}}', '{{}}', '{{}}', 'local_only');
            INSERT INTO settings VALUES ('ui.theme', '\"dark\"');
            ",
            snap = hex(&SNAPSHOT_SHA),
            osr = hex(&BLOB_OSR),
            osr2 = hex(&BLOB_OSR_2),
            osg = hex(&BLOB_OSG),
            a = hex(&PLAY_A),
            b = hex(&PLAY_B),
        ))
        .unwrap();
    }

    pub(crate) fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02X}")).collect()
    }

    pub(crate) fn count(conn: &Connection, table: &str) -> i64 {
        conn.query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r.get(0))
            .unwrap()
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use sha2::{Digest, Sha256};

    use super::testkit::{count, migrated, seed};
    use super::*;

    const NOW: UnixUs = UnixUs(1_790_637_236_636_000);
    const TABLES: [&str; 12] = [
        "alias",
        "blob",
        "feedback_event",
        "game_install",
        "identity_decision",
        "install",
        "meta",
        "play",
        "profile",
        "profile_alias",
        "settings",
        "source_snapshot",
    ];
    const TEST_V2: &[M<'static>] = &[
        M::up(include_str!("migrations/0001_init.sql")),
        M::up("CREATE TABLE test_v2 (x INTEGER) STRICT;"),
    ];
    const BLESS_ENV: &str = "WOLLUF_BLESS_FIXTURES";

    fn fixture_v1() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/userdb/v1.db")
    }

    fn sha(path: &Path) -> Vec<u8> {
        Sha256::digest(std::fs::read(path).unwrap()).to_vec()
    }

    fn version_of(path: &Path) -> u32 {
        user_version(&Connection::open(path).unwrap()).unwrap()
    }

    fn names(conn: &Connection, kind: &str) -> Vec<String> {
        let mut stmt = conn
            .prepare(
                "SELECT name FROM sqlite_schema WHERE type = ?1 AND name NOT LIKE 'sqlite_%' ORDER BY name",
            )
            .unwrap();
        stmt.query_map([kind], |r| r.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    }

    /// Writes `fixtures/userdb/v1.db`: schema v1 plus the testkit seed, as one rollback-journal
    /// file. Runs only when blessing, so the committed fixture is what every later migration
    /// is tested from (§10).
    fn write_fixture_v1(target: &Path) {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "foreign_keys", true).unwrap();
        Migrations::from_slice(&USER_MIGRATIONS[..1])
            .to_latest(&mut conn)
            .unwrap();
        seed(&conn);
        let _ = std::fs::remove_file(target);
        conn.execute("VACUUM INTO ?1", [target.to_str().unwrap()])
            .unwrap();
    }

    #[test]
    fn migrations_validate() {
        Migrations::from_slice(USER_MIGRATIONS).validate().unwrap();
    }

    #[test]
    fn migrate_from_zero_creates_all_tables() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("user.db");
        let db = open_user_db(&path, &dir.path().join("backups"), NOW).unwrap();
        let (tables, indexes, created_with) = db
            .read(|c| {
                let created: String = c.0.query_row(
                    "SELECT value FROM meta WHERE key = 'schema_created_with'",
                    [],
                    |r| r.get(0),
                )?;
                Ok((names(c.0, "table"), names(c.0, "index"), created))
            })
            .unwrap();
        assert_eq!(tables, TABLES);
        assert!(indexes.contains(&"profile_single_default".to_owned()));
        assert!(indexes.contains(&"profile_single_self".to_owned()));
        assert_eq!(created_with, APP_VERSION);
        assert_eq!(db.read(schema_version).unwrap(), USER_SCHEMA_LATEST);
        drop(db);
        assert_eq!(version_of(&path), USER_SCHEMA_LATEST);
        // A fresh DB has nothing to back up.
        assert!(!dir.path().join("backups").exists());
    }

    #[test]
    fn schema_too_new_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("user.db");
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch("CREATE TABLE future (x INTEGER); PRAGMA user_version = 999;")
                .unwrap();
        }
        let before = sha(&path);
        let err = open_user_db(&path, &dir.path().join("backups"), NOW)
            .err()
            .unwrap();
        assert!(matches!(
            err,
            StoreError::SchemaTooNew {
                found: 999,
                latest: USER_SCHEMA_LATEST
            }
        ));
        assert_eq!(err.code(), wolluf_core::ErrorCode::UnsupportedFormat);
        assert_eq!(sha(&path), before);
    }

    #[test]
    fn vacuum_into_backup_before_migrate() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("user.db");
        let backups = dir.path().join("backups");
        {
            let db = open_with(&path, &backups, NOW, &TEST_V2[..1]).unwrap();
            db.write(|tx| {
                seed(&tx.0);
                Ok(())
            })
            .unwrap();
        }
        let db = open_with(&path, &backups, NOW, TEST_V2).unwrap();
        drop(db);
        assert_eq!(version_of(&path), 2);

        let backup = backups.join("user-v1-20260928T231356Z.db");
        let conn = Connection::open(&backup).unwrap();
        assert_eq!(user_version(&conn).unwrap(), 1);
        assert_eq!(count(&conn, "play"), 2);
        assert_eq!(count(&conn, "alias"), 3);
    }

    #[test]
    fn backup_retention_keeps_5() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("user.db");
        let backups = dir.path().join("backups");
        std::fs::create_dir_all(&backups).unwrap();
        for day in 1..=6 {
            std::fs::write(
                backups.join(format!("user-v1-2020010{day}T000000Z.db")),
                b"old",
            )
            .unwrap();
        }
        std::fs::write(backups.join("notes.txt"), b"keep").unwrap();
        drop(open_with(&path, &backups, NOW, &TEST_V2[..1]).unwrap());
        drop(open_with(&path, &backups, NOW, TEST_V2).unwrap());

        let mut left: Vec<String> = std::fs::read_dir(&backups)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        left.sort();
        assert_eq!(
            left,
            [
                "notes.txt",
                "user-v1-20200103T000000Z.db",
                "user-v1-20200104T000000Z.db",
                "user-v1-20200105T000000Z.db",
                "user-v1-20200106T000000Z.db",
                "user-v1-20260928T231356Z.db",
            ]
        );
    }

    #[test]
    fn migrates_from_fixture_v1() {
        if std::env::var_os(BLESS_ENV).is_some() {
            write_fixture_v1(&fixture_v1());
        }
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("user.db");
        // Copied, never opened in place: a migration must not mutate the committed fixture.
        std::fs::copy(fixture_v1(), &path).unwrap();
        assert_eq!(version_of(&path), 1);
        let db = open_user_db(&path, &dir.path().join("backups"), NOW).unwrap();
        let counts = db.read(|c| Ok(TABLES.map(|t| (t, count(c.0, t))))).unwrap();
        for (table, n) in counts {
            let expected = match table {
                "blob" => 3,
                "alias" => 3,
                "play" => 2,
                // The fixture's own row plus `schema_created_with`, which v1 fixtures lack.
                "meta" => 2,
                _ => 1,
            };
            assert_eq!(n, expected, "{table}");
        }
        drop(db);
        assert_eq!(version_of(&path), USER_SCHEMA_LATEST);
    }

    #[test]
    fn seed_fits_every_table() {
        let conn = migrated();
        seed(&conn);
        for table in TABLES {
            assert!(count(&conn, table) > 0, "{table}");
        }
    }
}
