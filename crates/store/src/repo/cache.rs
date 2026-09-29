//! cache.db repositories (spec 003 Design `repo/cache.rs`).

use std::collections::BTreeMap;

use wolluf_core::{ChartMd5, ErrorCode, StageId, UnixUs, VersionKey};

use rusqlite::{OptionalExtension, Row};

use crate::db::{Conn, Tx};
use crate::error::StoreError;
use crate::repo::ledger::SnapshotId;
use crate::repo::sql::{enum_col, fixed, int, json_value, opt_parsed, opt_time, parsed, str_enum};
use crate::time::format_rfc3339_ms;

/// One mania entry of osu!.db (spec 003 step 1).
#[derive(Debug, Clone, PartialEq)]
pub struct CatalogChart {
    pub md5: ChartMd5,
    /// `round(CS)` as osu! stores it; not validated against `Keymode`, so odd maps still list.
    pub keymode: u8,
    pub title: String,
    pub artist: String,
    pub version: String,
    pub creator: String,
    pub set_id: Option<i32>,
    pub beatmap_id: Option<i32>,
    /// Relative to the install's `Songs` directory.
    pub path: String,
    pub od: f64,
    pub hp: f64,
    pub length_ms: u32,
}

str_enum! {
    pub enum DerivationStatus {
        Ok => "ok",
        Failed => "failed",
        Skipped => "skipped",
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Derivation {
    pub stage: StageId,
    pub input_key: String,
    pub vkey: VersionKey,
    pub status: DerivationStatus,
    pub error_code: Option<ErrorCode>,
    pub error_msg: Option<String>,
    pub duration_ms: Option<u32>,
}

str_enum! {
    pub enum JobStatus {
        Queued => "queued",
        Running => "running",
        Ok => "ok",
        Failed => "failed",
        Cancelled => "cancelled",
    }
}

impl JobStatus {
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Ok | Self::Failed | Self::Cancelled)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct NewJobRun {
    pub id: ulid::Ulid,
    pub kind: String,
    pub params: serde_json::Value,
    pub status: JobStatus,
    pub started: Option<UnixUs>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct JobRun {
    pub id: ulid::Ulid,
    pub kind: String,
    pub params: serde_json::Value,
    pub status: JobStatus,
    pub started: Option<UnixUs>,
    pub ended: Option<UnixUs>,
    pub summary: Option<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemFailure {
    pub job_id: ulid::Ulid,
    /// A play id hex, chart md5 or file name: whatever identifies the item to the user.
    pub item_ref: String,
    pub code: ErrorCode,
    pub message: String,
}

fn ms(t: UnixUs) -> String {
    format_rfc3339_ms(t)
}

pub mod catalog_chart {
    use super::*;

    const COLUMNS: &str = "md5, keymode, title, artist, version, creator, set_id, beatmap_id, path, od, hp, length_ms";

    fn from_row(row: &Row<'_>) -> rusqlite::Result<CatalogChart> {
        Ok(CatalogChart {
            md5: parsed(row, 0, str::parse::<ChartMd5>)?,
            keymode: int(row, 1)?,
            title: row.get(2)?,
            artist: row.get(3)?,
            version: row.get(4)?,
            creator: row.get(5)?,
            set_id: row.get(6)?,
            beatmap_id: row.get(7)?,
            path: row.get(8)?,
            od: row.get(9)?,
            hp: row.get(10)?,
            length_ms: int(row, 11)?,
        })
    }

    /// Replaces the whole catalog inside the caller's transaction, so readers see either the
    /// old snapshot or the new one, never a mix (spec 003 step 1).
    pub fn replace_all(
        tx: &Tx<'_>,
        snapshot_id: SnapshotId,
        rows: &[CatalogChart],
    ) -> Result<(), StoreError> {
        tx.0.execute("DELETE FROM catalog_chart", [])?;
        let mut insert = tx.0.prepare_cached(&format!(
            "INSERT INTO catalog_chart ({COLUMNS}, snapshot_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)"
        ))?;
        for c in rows {
            insert.execute(rusqlite::params![
                c.md5.to_string(),
                c.keymode,
                c.title,
                c.artist,
                c.version,
                c.creator,
                c.set_id,
                c.beatmap_id,
                c.path,
                c.od,
                c.hp,
                c.length_ms,
                snapshot_id.0,
            ])?;
        }
        Ok(())
    }

    pub fn get(conn: Conn<'_>, md5: ChartMd5) -> Result<Option<CatalogChart>, StoreError> {
        Ok(conn
            .0
            .query_row(
                &format!("SELECT {COLUMNS} FROM catalog_chart WHERE md5 = ?1"),
                [md5.to_string()],
                from_row,
            )
            .optional()?)
    }

    /// Ordered by md5, so two catalogs compare equal exactly when their contents do (AC16).
    pub fn list_all(conn: Conn<'_>) -> Result<Vec<CatalogChart>, StoreError> {
        let mut stmt = conn
            .0
            .prepare(&format!("SELECT {COLUMNS} FROM catalog_chart ORDER BY md5"))?;
        let rows = stmt.query_map([], from_row)?.collect::<Result<_, _>>()?;
        Ok(rows)
    }

    pub fn keymodes(conn: Conn<'_>) -> Result<BTreeMap<ChartMd5, u8>, StoreError> {
        let mut stmt = conn.0.prepare("SELECT md5, keymode FROM catalog_chart")?;
        let rows = stmt
            .query_map([], |row| {
                Ok((parsed(row, 0, str::parse::<ChartMd5>)?, int(row, 1)?))
            })?
            .collect::<Result<_, _>>()?;
        Ok(rows)
    }

    /// The osu!.db snapshot the catalog was built from; `None` while the catalog is empty.
    pub fn snapshot_id(conn: Conn<'_>) -> Result<Option<SnapshotId>, StoreError> {
        let id: Option<i64> =
            conn.0
                .query_row("SELECT max(snapshot_id) FROM catalog_chart", [], |r| {
                    r.get(0)
                })?;
        Ok(id.map(SnapshotId))
    }
}

pub mod derivation {
    use super::*;

    const COLUMNS: &str = "stage, input_key, vkey, status, error_code, error_msg, duration_ms";

    fn from_row(row: &Row<'_>) -> rusqlite::Result<Derivation> {
        Ok(Derivation {
            stage: parsed(row, 0, StageId::parse)?,
            input_key: row.get(1)?,
            vkey: VersionKey(fixed(row, 2)?),
            status: enum_col(row, 3, DerivationStatus::parse)?,
            error_code: opt_parsed(row, 4, str::parse::<ErrorCode>)?,
            error_msg: row.get(5)?,
            duration_ms: row.get(6)?,
        })
    }

    /// Upsert: a rerun of the same (stage, input, vkey) records its latest outcome.
    pub fn put(tx: &Tx<'_>, d: &Derivation) -> Result<(), StoreError> {
        tx.0.prepare_cached(&format!(
            "INSERT OR REPLACE INTO derivation ({COLUMNS}) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)"
        ))?
        .execute((
            d.stage.as_str(),
            &d.input_key,
            d.vkey.0,
            d.status.as_str(),
            d.error_code.map(ErrorCode::as_str),
            &d.error_msg,
            d.duration_ms,
        ))?;
        Ok(())
    }

    pub fn get(
        conn: Conn<'_>,
        stage: &StageId,
        input_key: &str,
        vkey: VersionKey,
    ) -> Result<Option<Derivation>, StoreError> {
        Ok(conn
            .0
            .prepare_cached(&format!(
                "SELECT {COLUMNS} FROM derivation
                 WHERE stage = ?1 AND input_key = ?2 AND vkey = ?3"
            ))?
            .query_row((stage.as_str(), input_key, vkey.0), from_row)
            .optional()?)
    }

    pub fn list_all(conn: Conn<'_>) -> Result<Vec<Derivation>, StoreError> {
        let mut stmt = conn.0.prepare(&format!(
            "SELECT {COLUMNS} FROM derivation ORDER BY stage, input_key, vkey"
        ))?;
        let rows = stmt.query_map([], from_row)?.collect::<Result<_, _>>()?;
        Ok(rows)
    }
}

pub mod job_run {
    use super::*;

    const COLUMNS: &str = "id, kind, params_json, status, started, ended, summary_json";

    fn from_row(row: &Row<'_>) -> rusqlite::Result<JobRun> {
        let summary: Option<String> = row.get(6)?;
        Ok(JobRun {
            id: parsed(row, 0, ulid::Ulid::from_string)?,
            kind: row.get(1)?,
            params: json_value(row, 2)?,
            status: enum_col(row, 3, JobStatus::parse)?,
            started: opt_time(row, 4)?,
            ended: opt_time(row, 5)?,
            summary: match summary {
                Some(_) => Some(json_value(row, 6)?),
                None => None,
            },
        })
    }

    pub fn insert(tx: &Tx<'_>, job: &NewJobRun) -> Result<(), StoreError> {
        tx.0.execute(
            &format!("INSERT INTO job_run ({COLUMNS}) VALUES (?1, ?2, ?3, ?4, ?5, NULL, NULL)"),
            (
                job.id.to_string(),
                &job.kind,
                job.params.to_string(),
                job.status.as_str(),
                job.started.map(ms),
            ),
        )?;
        Ok(())
    }

    pub fn start(tx: &Tx<'_>, id: ulid::Ulid, started: UnixUs) -> Result<bool, StoreError> {
        let n = tx.0.execute(
            "UPDATE job_run SET status = 'running', started = ?2 WHERE id = ?1",
            (id.to_string(), ms(started)),
        )?;
        Ok(n == 1)
    }

    pub fn finish(
        tx: &Tx<'_>,
        id: ulid::Ulid,
        status: JobStatus,
        ended: UnixUs,
        summary: &serde_json::Value,
    ) -> Result<bool, StoreError> {
        if !status.is_terminal() {
            return Err(StoreError::InvalidData(format!(
                "job cannot finish as {}",
                status.as_str()
            )));
        }
        let n = tx.0.execute(
            "UPDATE job_run SET status = ?2, ended = ?3, summary_json = ?4 WHERE id = ?1",
            (
                id.to_string(),
                status.as_str(),
                ms(ended),
                summary.to_string(),
            ),
        )?;
        Ok(n == 1)
    }

    pub fn get(conn: Conn<'_>, id: ulid::Ulid) -> Result<Option<JobRun>, StoreError> {
        Ok(conn
            .0
            .query_row(
                &format!("SELECT {COLUMNS} FROM job_run WHERE id = ?1"),
                [id.to_string()],
                from_row,
            )
            .optional()?)
    }

    /// Newest first; ULIDs sort by creation time.
    pub fn list_recent(conn: Conn<'_>, limit: u32) -> Result<Vec<JobRun>, StoreError> {
        let mut stmt = conn.0.prepare(&format!(
            "SELECT {COLUMNS} FROM job_run ORDER BY id DESC LIMIT ?1"
        ))?;
        let rows = stmt
            .query_map([limit], from_row)?
            .collect::<Result<_, _>>()?;
        Ok(rows)
    }
}

pub mod item_failure {
    use super::*;

    /// Keeps the first failure recorded for an item in a job; returns whether this one was new.
    pub fn insert(tx: &Tx<'_>, f: &ItemFailure) -> Result<bool, StoreError> {
        let n =
            tx.0.prepare_cached(
                "INSERT INTO item_failure (job_id, item_ref, code, message) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT (job_id, item_ref) DO NOTHING",
            )?
            .execute((
                f.job_id.to_string(),
                &f.item_ref,
                f.code.as_str(),
                &f.message,
            ))?;
        Ok(n == 1)
    }

    pub fn list(conn: Conn<'_>, job_id: ulid::Ulid) -> Result<Vec<ItemFailure>, StoreError> {
        let mut stmt = conn.0.prepare(
            "SELECT job_id, item_ref, code, message FROM item_failure
             WHERE job_id = ?1 ORDER BY item_ref",
        )?;
        let rows = stmt
            .query_map([job_id.to_string()], |row| {
                Ok(ItemFailure {
                    job_id: parsed(row, 0, ulid::Ulid::from_string)?,
                    item_ref: row.get(1)?,
                    code: parsed(row, 2, str::parse::<ErrorCode>)?,
                    message: row.get(3)?,
                })
            })?
            .collect::<Result<_, _>>()?;
        Ok(rows)
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use serde_json::json;

    use super::*;
    use crate::cache::open_cache_db;

    const T0: UnixUs = UnixUs(1_790_637_236_636_000);

    fn chart(md5: &str, title: &str) -> CatalogChart {
        CatalogChart {
            md5: ChartMd5::from_str(md5).unwrap(),
            keymode: 7,
            title: title.into(),
            artist: "Artist".into(),
            version: "7K Another".into(),
            creator: "Mapper".into(),
            set_id: Some(123),
            beatmap_id: None,
            path: "123 Artist - Title/map.osu".into(),
            od: 8.5,
            hp: 7.0,
            length_ms: 150_000,
        }
    }

    const MD5_A: &str = "e956977ccc1d74a50ae48b43a868cc20";
    const MD5_B: &str = "0123456789abcdef0123456789abcdef";
    const MD5_C: &str = "ffffffffffffffffffffffffffffffff";

    #[test]
    fn catalog_replace_all_is_atomic() {
        let dir = tempfile::tempdir().unwrap();
        let db = open_cache_db(&dir.path().join("cache.db")).unwrap();
        let first = vec![chart(MD5_B, "B"), chart(MD5_A, "A")];
        let rows = first.clone();
        db.write(move |tx| catalog_chart::replace_all(tx, SnapshotId(1), &rows))
            .unwrap();

        // A duplicate md5 fails midway; the whole replacement must roll back.
        let broken = vec![chart(MD5_C, "C"), chart(MD5_C, "C again")];
        assert!(
            db.write(move |tx| catalog_chart::replace_all(tx, SnapshotId(2), &broken))
                .is_err()
        );
        let (all, snapshot) = db
            .read(|c| Ok((catalog_chart::list_all(c)?, catalog_chart::snapshot_id(c)?)))
            .unwrap();
        assert_eq!(all, first);
        assert_eq!(snapshot, Some(SnapshotId(1)));

        let second = vec![chart(MD5_C, "C")];
        let rows = second.clone();
        db.write(move |tx| catalog_chart::replace_all(tx, SnapshotId(3), &rows))
            .unwrap();
        let (got, missing, keymodes) = db
            .read(|c| {
                Ok((
                    catalog_chart::get(c, ChartMd5::from_str(MD5_C).unwrap())?,
                    catalog_chart::get(c, ChartMd5::from_str(MD5_A).unwrap())?,
                    catalog_chart::keymodes(c)?,
                ))
            })
            .unwrap();
        assert_eq!(got, Some(second[0].clone()));
        assert_eq!(missing, None);
        assert_eq!(keymodes, [(ChartMd5::from_str(MD5_C).unwrap(), 7)].into());
    }

    #[test]
    fn derivation_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let db = open_cache_db(&dir.path().join("cache.db")).unwrap();
        let skipped = Derivation {
            stage: StageId::from_static("chart_archive"),
            input_key: format!("{MD5_A}:{}", "ab".repeat(32)),
            vkey: VersionKey([7; 32]),
            status: DerivationStatus::Skipped,
            error_code: Some(ErrorCode::Conflict),
            error_msg: Some("md5 mismatch".into()),
            duration_ms: Some(12),
        };
        let ok = Derivation {
            stage: StageId::from_static("catalog"),
            input_key: "cd".repeat(32),
            vkey: VersionKey([9; 32]),
            status: DerivationStatus::Ok,
            error_code: None,
            error_msg: None,
            duration_ms: None,
        };
        let (a, b) = (skipped.clone(), ok.clone());
        db.write(move |tx| {
            derivation::put(tx, &a)?;
            derivation::put(tx, &b)?;
            // Re-putting the same key replaces the row instead of failing.
            derivation::put(tx, &b)
        })
        .unwrap();
        let (got, other_vkey, all) = db
            .read(|c| {
                Ok((
                    derivation::get(c, &skipped.stage, &skipped.input_key, skipped.vkey)?,
                    derivation::get(c, &skipped.stage, &skipped.input_key, VersionKey([8; 32]))?,
                    derivation::list_all(c)?,
                ))
            })
            .unwrap();
        assert_eq!(got, Some(skipped.clone()));
        assert_eq!(other_vkey, None);
        assert_eq!(all, vec![ok, skipped]);
    }

    #[test]
    fn job_run_finish_sets_summary() {
        let dir = tempfile::tempdir().unwrap();
        let db = open_cache_db(&dir.path().join("cache.db")).unwrap();
        let id = ulid::Ulid::from_parts(1, 1);
        let job = NewJobRun {
            id,
            kind: "sync_plays".into(),
            params: json!({"installId": 1}),
            status: JobStatus::Queued,
            started: None,
        };
        db.write(move |tx| job_run::insert(tx, &job)).unwrap();
        assert!(db.write(move |tx| job_run::start(tx, id, T0)).unwrap());
        let summary = json!({"playsNew": 6, "failedItems": 0});
        let s = summary.clone();
        let ended = UnixUs(T0.0 + 2_000_000);
        assert!(
            db.write(move |tx| job_run::finish(tx, id, JobStatus::Ok, ended, &s))
                .unwrap()
        );
        let run = db.read(|c| job_run::get(c, id)).unwrap().unwrap();
        assert_eq!(
            run,
            JobRun {
                id,
                kind: "sync_plays".into(),
                params: json!({"installId": 1}),
                status: JobStatus::Ok,
                started: Some(T0),
                ended: Some(ended),
                summary: Some(summary),
            }
        );
        // Finishing into a non-terminal status is a caller bug.
        assert!(
            db.write(move |tx| job_run::finish(tx, id, JobStatus::Running, ended, &json!({})))
                .is_err()
        );
        let recent = db.read(|c| job_run::list_recent(c, 10)).unwrap();
        assert_eq!(recent.len(), 1);
    }

    #[test]
    fn item_failure_first_wins() {
        let dir = tempfile::tempdir().unwrap();
        let db = open_cache_db(&dir.path().join("cache.db")).unwrap();
        let id = ulid::Ulid::from_parts(2, 2);
        let job = NewJobRun {
            id,
            kind: "sync_plays".into(),
            params: json!({}),
            status: JobStatus::Running,
            started: Some(T0),
        };
        let first = ItemFailure {
            job_id: id,
            item_ref: "b".repeat(64),
            code: ErrorCode::Conflict,
            message: "stored row wins".into(),
        };
        let mut again = first.clone();
        again.code = ErrorCode::Internal;
        let other = ItemFailure {
            item_ref: "a".repeat(64),
            ..first.clone()
        };
        let (f, a, o) = (first.clone(), again, other.clone());
        let inserted = db
            .write(move |tx| {
                job_run::insert(tx, &job)?;
                Ok((
                    item_failure::insert(tx, &f)?,
                    item_failure::insert(tx, &a)?,
                    item_failure::insert(tx, &o)?,
                ))
            })
            .unwrap();
        assert_eq!(inserted, (true, false, true));
        assert_eq!(
            db.read(|c| item_failure::list(c, id)).unwrap(),
            vec![other, first]
        );
    }
}
