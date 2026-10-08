//! MinaCalc skillsets per chart and rate under one `difficulty` key (ADR 0023). Plain store
//! types: the store does not depend on `wolluf-difficulty`, so the app maps its `MsdTable`.

use std::collections::BTreeMap;

use rusqlite::OptionalExtension;
use wolluf_core::{ChartMd5, VersionKey};

use crate::db::{Conn, Tx};
use crate::error::StoreError;
use crate::repo::sql::{enum_col, int, parsed, str_enum};

str_enum! {
    pub enum MsdStatusRow {
        Rated => "rated",
        LnHeavy => "ln_heavy",
        CalcRejected => "calc_rejected",
    }
}

/// `centi` is MSD × 100 in `wolluf_minacalc::SKILLSET_IDS` order, Overall first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MsdRow {
    pub rate_milli: u16,
    pub centi: [i32; 8],
}

/// One chart's outcome; `rows` is non-empty only when `Rated`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChartMsd {
    pub status: MsdStatusRow,
    pub hold_share_permille: u16,
    pub rows: Vec<MsdRow>,
}

const SKILLSETS: &str =
    "overall, stream, jumpstream, handstream, stamina, jackspeed, chordjack, technical";

fn centi(row: &rusqlite::Row<'_>, at: usize) -> rusqlite::Result<[i32; 8]> {
    let mut out = [0; 8];
    for (i, slot) in out.iter_mut().enumerate() {
        *slot = row.get(at + i)?;
    }
    Ok(out)
}

/// Replaces the chart's status and rate rows under `vkey`; other keys stay until `prune_except`.
/// A rated status without rate rows, rate rows on an unrated status, or two rows at one rate fail
/// the transaction.
pub fn replace_for(
    tx: &Tx<'_>,
    md5: ChartMd5,
    vkey: VersionKey,
    msd: &ChartMsd,
) -> Result<(), StoreError> {
    if (msd.status == MsdStatusRow::Rated) == msd.rows.is_empty() {
        return Err(StoreError::InvalidData(format!(
            "chart_msd {md5}: {} rate rows on status {}",
            msd.rows.len(),
            msd.status.as_str()
        )));
    }
    let (md5, vkey) = (md5.to_string(), vkey.0);
    tx.0.prepare_cached("DELETE FROM chart_msd WHERE md5 = ?1 AND vkey = ?2")?
        .execute((&md5, &vkey))?;
    tx.0.prepare_cached(
        "INSERT OR REPLACE INTO chart_msd_status (md5, vkey, status, hold_share_permille)
         VALUES (?1, ?2, ?3, ?4)",
    )?
    .execute((&md5, &vkey, msd.status.as_str(), msd.hold_share_permille))?;
    let mut insert = tx.0.prepare_cached(&format!(
        "INSERT INTO chart_msd (md5, vkey, rate_milli, {SKILLSETS})
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)"
    ))?;
    for r in &msd.rows {
        let c = r.centi;
        insert.execute(rusqlite::params![
            md5,
            vkey,
            r.rate_milli,
            c[0],
            c[1],
            c[2],
            c[3],
            c[4],
            c[5],
            c[6],
            c[7],
        ])?;
    }
    Ok(())
}

/// `None` while the chart has no status row under `vkey`. Rows come in rate order.
pub fn get(
    conn: Conn<'_>,
    md5: ChartMd5,
    vkey: VersionKey,
) -> Result<Option<ChartMsd>, StoreError> {
    let (md5, vkey) = (md5.to_string(), vkey.0);
    let status = conn
        .0
        .prepare_cached(
            "SELECT status, hold_share_permille FROM chart_msd_status WHERE md5 = ?1 AND vkey = ?2",
        )?
        .query_row((&md5, &vkey), |row| {
            Ok((enum_col(row, 0, MsdStatusRow::parse)?, int(row, 1)?))
        })
        .optional()?;
    let Some((status, hold_share_permille)) = status else {
        return Ok(None);
    };
    let mut stmt = conn.0.prepare_cached(&format!(
        "SELECT rate_milli, {SKILLSETS} FROM chart_msd
         WHERE md5 = ?1 AND vkey = ?2 ORDER BY rate_milli"
    ))?;
    let rows = stmt
        .query_map((&md5, &vkey), |row| {
            Ok(MsdRow {
                rate_milli: int(row, 0)?,
                centi: centi(row, 1)?,
            })
        })?
        .collect::<Result<_, _>>()?;
    Ok(Some(ChartMsd {
        status,
        hold_share_permille,
        rows,
    }))
}

/// Overall centi at `rate_milli` for those of `md5s` that are rated under `vkey`.
pub fn overall_at(
    conn: Conn<'_>,
    vkey: VersionKey,
    rate_milli: u16,
    md5s: &[ChartMd5],
) -> Result<BTreeMap<ChartMd5, i32>, StoreError> {
    if md5s.is_empty() {
        return Ok(BTreeMap::new());
    }
    // One JSON parameter instead of one placeholder per md5 keeps any page size under SQLite's
    // bound-variable limit without chunking.
    let wanted: Vec<String> = md5s.iter().map(ToString::to_string).collect();
    let wanted = serde_json::to_string(&wanted)
        .map_err(|e| StoreError::InvalidData(format!("chart_msd md5 list: {e}")))?;
    let mut stmt = conn.0.prepare_cached(
        "SELECT md5, overall FROM chart_msd
         WHERE vkey = ?1 AND rate_milli = ?2 AND md5 IN (SELECT value FROM json_each(?3))
         ORDER BY md5",
    )?;
    let rows = stmt
        .query_map((vkey.0, rate_milli, wanted), |row| {
            Ok((parsed(row, 0, str::parse::<ChartMd5>)?, row.get(1)?))
        })?
        .collect::<Result<_, _>>()?;
    Ok(rows)
}

/// Every rated chart's skillsets at `rate_milli` under `vkey`, by md5.
pub fn rated_at(
    conn: Conn<'_>,
    vkey: VersionKey,
    rate_milli: u16,
) -> Result<Vec<(ChartMd5, [i32; 8])>, StoreError> {
    let mut stmt = conn.0.prepare_cached(&format!(
        "SELECT md5, {SKILLSETS} FROM chart_msd WHERE vkey = ?1 AND rate_milli = ?2 ORDER BY md5"
    ))?;
    let rows = stmt
        .query_map((vkey.0, rate_milli), |row| {
            Ok((parsed(row, 0, str::parse::<ChartMd5>)?, centi(row, 1)?))
        })?
        .collect::<Result<_, _>>()?;
    Ok(rows)
}

/// Catalog charts of `keymode` with no status row under `vkey`, by md5: the backfill queue.
pub fn missing_for_keymode(
    conn: Conn<'_>,
    vkey: VersionKey,
    keymode: u8,
) -> Result<Vec<ChartMd5>, StoreError> {
    let mut stmt = conn.0.prepare_cached(
        "SELECT c.md5 FROM catalog_chart c
         WHERE c.keymode = ?1
           AND NOT EXISTS (SELECT 1 FROM chart_msd_status s WHERE s.md5 = c.md5 AND s.vkey = ?2)
         ORDER BY c.md5",
    )?;
    let rows = stmt
        .query_map((keymode, vkey.0), |row| {
            parsed(row, 0, str::parse::<ChartMd5>)
        })?
        .collect::<Result<_, _>>()?;
    Ok(rows)
}

/// GC hook (§5.5) over both tables; an empty `keep` clears them. Each keymode has its own
/// `difficulty` key, so callers pass every current one. Returns the number of rows deleted.
pub fn prune_except(tx: &Tx<'_>, keep: &[VersionKey]) -> Result<u64, StoreError> {
    Ok(super::prune_vkeys_except(tx, "chart_msd", keep)?
        + super::prune_vkeys_except(tx, "chart_msd_status", keep)?)
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;
    use crate::cache::open_cache_db;
    use crate::db::DbHandle;
    use crate::repo::cache::{CatalogChart, catalog_chart};
    use crate::repo::ledger::SnapshotId;

    const MD5_A: &str = "e956977ccc1d74a50ae48b43a868cc20";
    const MD5_B: &str = "0123456789abcdef0123456789abcdef";
    const MD5_C: &str = "ffffffffffffffffffffffffffffffff";
    const VKEY_1: VersionKey = VersionKey([1; 32]);
    const VKEY_2: VersionKey = VersionKey([2; 32]);

    fn md5(s: &str) -> ChartMd5 {
        ChartMd5::from_str(s).unwrap()
    }

    fn db() -> (tempfile::TempDir, DbHandle) {
        let dir = tempfile::tempdir().unwrap();
        let db = open_cache_db(&dir.path().join("cache.db")).unwrap();
        (dir, db)
    }

    fn row(rate_milli: u16, base: i32) -> MsdRow {
        MsdRow {
            rate_milli,
            centi: std::array::from_fn(|i| base + i32::try_from(i).unwrap()),
        }
    }

    fn rated(base: i32) -> ChartMsd {
        ChartMsd {
            status: MsdStatusRow::Rated,
            hold_share_permille: 120,
            rows: vec![row(900, base - 100), row(1000, base), row(1100, base + 100)],
        }
    }

    fn unrated(status: MsdStatusRow) -> ChartMsd {
        ChartMsd {
            status,
            hold_share_permille: 750,
            rows: Vec::new(),
        }
    }

    fn put(db: &DbHandle, md5_hex: &str, vkey: VersionKey, msd: &ChartMsd) {
        let (m, msd) = (md5(md5_hex), msd.clone());
        db.write(move |tx| replace_for(tx, m, vkey, &msd)).unwrap();
    }

    fn get_one(db: &DbHandle, md5_hex: &str, vkey: VersionKey) -> Option<ChartMsd> {
        db.read(|c| get(c, md5(md5_hex), vkey)).unwrap()
    }

    #[test]
    fn round_trip_is_keyed_and_rate_ordered() {
        let (_dir, db) = db();
        // Stored out of rate order: reads must still come back ascending.
        let mut shuffled = rated(2_500);
        shuffled.rows.reverse();
        put(&db, MD5_A, VKEY_1, &shuffled);
        put(&db, MD5_A, VKEY_2, &rated(3_000));
        put(&db, MD5_B, VKEY_1, &unrated(MsdStatusRow::LnHeavy));
        put(&db, MD5_C, VKEY_1, &unrated(MsdStatusRow::CalcRejected));

        assert_eq!(get_one(&db, MD5_A, VKEY_1), Some(rated(2_500)));
        assert_eq!(get_one(&db, MD5_A, VKEY_2), Some(rated(3_000)));
        assert_eq!(
            get_one(&db, MD5_B, VKEY_1),
            Some(unrated(MsdStatusRow::LnHeavy))
        );
        assert_eq!(
            get_one(&db, MD5_C, VKEY_1),
            Some(unrated(MsdStatusRow::CalcRejected))
        );
        assert_eq!(get_one(&db, MD5_B, VKEY_2), None);
    }

    #[test]
    fn replace_drops_previous_rows_of_the_same_key_only() {
        let (_dir, db) = db();
        put(&db, MD5_A, VKEY_1, &rated(2_500));
        put(&db, MD5_A, VKEY_2, &rated(3_000));

        let fewer = ChartMsd {
            status: MsdStatusRow::Rated,
            hold_share_permille: 80,
            rows: vec![row(1000, 2_600)],
        };
        put(&db, MD5_A, VKEY_1, &fewer);
        assert_eq!(get_one(&db, MD5_A, VKEY_1), Some(fewer));

        // A chart that turns unrated must not keep its old rate rows.
        put(&db, MD5_A, VKEY_1, &unrated(MsdStatusRow::CalcRejected));
        assert_eq!(
            get_one(&db, MD5_A, VKEY_1),
            Some(unrated(MsdStatusRow::CalcRejected))
        );
        assert_eq!(db.read(|c| rated_at(c, VKEY_1, 1000)).unwrap(), Vec::new());
        assert_eq!(get_one(&db, MD5_A, VKEY_2), Some(rated(3_000)));
    }

    #[test]
    fn unrated_with_rows_and_duplicate_rates_are_rejected_atomically() {
        let (_dir, db) = db();
        put(&db, MD5_A, VKEY_1, &rated(2_500));

        let bogus = ChartMsd {
            rows: vec![row(1000, 1)],
            ..unrated(MsdStatusRow::LnHeavy)
        };
        let m = md5(MD5_A);
        assert!(
            db.write(move |tx| replace_for(tx, m, VKEY_1, &bogus))
                .is_err()
        );
        let duplicate = ChartMsd {
            rows: vec![row(1000, 1), row(1000, 2)],
            ..rated(0)
        };
        assert!(
            db.write(move |tx| replace_for(tx, m, VKEY_1, &duplicate))
                .is_err()
        );
        assert_eq!(get_one(&db, MD5_A, VKEY_1), Some(rated(2_500)));
    }

    #[test]
    fn rated_without_rows_is_rejected_atomically() {
        let (_dir, db) = db();
        put(&db, MD5_A, VKEY_1, &rated(2_500));

        let empty = ChartMsd {
            rows: Vec::new(),
            ..rated(0)
        };
        let m = md5(MD5_A);
        assert!(
            db.write(move |tx| replace_for(tx, m, VKEY_1, &empty))
                .is_err()
        );
        assert_eq!(get_one(&db, MD5_A, VKEY_1), Some(rated(2_500)));
    }

    #[test]
    fn status_and_vkey_check_constraints_hold() {
        let (_dir, db) = db();
        let insert = |vkey: &'static str, status: &'static str| {
            db.write(move |tx| {
                Ok(tx.0.execute(
                    &format!("INSERT INTO chart_msd_status VALUES ('m', {vkey}, ?1, 0)"),
                    [status],
                )?)
            })
        };
        assert!(insert("zeroblob(32)", "pending").is_err());
        assert!(insert("zeroblob(31)", "rated").is_err());
        assert!(insert("'k'", "rated").is_err());
        assert_eq!(insert("zeroblob(32)", "rated").unwrap(), 1);
    }

    #[test]
    fn overall_at_reads_many_md5s_at_one_rate() {
        let (_dir, db) = db();
        put(&db, MD5_A, VKEY_1, &rated(2_500));
        put(&db, MD5_B, VKEY_1, &rated(1_800));
        put(&db, MD5_B, VKEY_2, &rated(9_999));
        put(&db, MD5_C, VKEY_1, &unrated(MsdStatusRow::LnHeavy));

        let all = [md5(MD5_C), md5(MD5_B), md5(MD5_A)];
        let got = db.read(|c| overall_at(c, VKEY_1, 1000, &all)).unwrap();
        assert_eq!(got, [(md5(MD5_A), 2_500), (md5(MD5_B), 1_800)].into());

        let got = db
            .read(|c| overall_at(c, VKEY_1, 1100, &[md5(MD5_B)]))
            .unwrap();
        assert_eq!(got, [(md5(MD5_B), 1_900)].into());
        assert!(
            db.read(|c| overall_at(c, VKEY_1, 1050, &all))
                .unwrap()
                .is_empty(),
            "no row at an off-grid rate"
        );
        assert!(
            db.read(|c| overall_at(c, VKEY_1, 1000, &[]))
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn rated_at_lists_every_rated_chart_by_md5() {
        let (_dir, db) = db();
        put(&db, MD5_A, VKEY_1, &rated(2_500));
        put(&db, MD5_B, VKEY_1, &rated(1_800));
        put(&db, MD5_B, VKEY_2, &rated(9_999));
        put(&db, MD5_C, VKEY_1, &unrated(MsdStatusRow::CalcRejected));

        let got = db.read(|c| rated_at(c, VKEY_1, 900)).unwrap();
        assert_eq!(
            got,
            vec![
                (md5(MD5_B), row(900, 1_700).centi),
                (md5(MD5_A), row(900, 2_400).centi),
            ]
        );
    }

    fn catalog(md5_hex: &str, keymode: u8) -> CatalogChart {
        CatalogChart {
            md5: md5(md5_hex),
            keymode,
            title: "T".into(),
            artist: "A".into(),
            version: "V".into(),
            creator: "C".into(),
            set_id: None,
            beatmap_id: None,
            path: "p.osu".into(),
            od: 8.0,
            hp: 7.0,
            length_ms: 60_000,
            stars: None,
            source: String::new(),
            tags: String::new(),
        }
    }

    #[test]
    fn missing_for_keymode_lists_unprocessed_charts_of_that_keymode() {
        let (_dir, db) = db();
        const MD5_D: &str = "00000000000000000000000000000004";
        let rows = vec![
            catalog(MD5_A, 4),
            catalog(MD5_B, 4),
            catalog(MD5_C, 7),
            catalog(MD5_D, 4),
        ];
        db.write(move |tx| catalog_chart::replace_all(tx, SnapshotId(1), &rows))
            .unwrap();
        // An unrated status still counts as processed; a row under another key does not.
        put(&db, MD5_A, VKEY_1, &unrated(MsdStatusRow::LnHeavy));
        put(&db, MD5_D, VKEY_2, &rated(2_000));

        let got = db.read(|c| missing_for_keymode(c, VKEY_1, 4)).unwrap();
        assert_eq!(got, vec![md5(MD5_D), md5(MD5_B)]);
        let got = db.read(|c| missing_for_keymode(c, VKEY_1, 7)).unwrap();
        assert_eq!(got, vec![md5(MD5_C)]);
        assert!(
            db.read(|c| missing_for_keymode(c, VKEY_1, 6))
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn prune_keeps_only_the_given_keys_in_both_tables() {
        let (_dir, db) = db();
        const VKEY_3: VersionKey = VersionKey([3; 32]);
        put(&db, MD5_A, VKEY_1, &rated(2_500));
        put(&db, MD5_B, VKEY_1, &unrated(MsdStatusRow::LnHeavy));
        put(&db, MD5_A, VKEY_2, &rated(3_000));
        put(&db, MD5_A, VKEY_3, &unrated(MsdStatusRow::CalcRejected));

        // VKEY_1: 2 status + 3 rate rows.
        let deleted = db.write(|tx| prune_except(tx, &[VKEY_2, VKEY_3])).unwrap();
        assert_eq!(deleted, 5);
        assert_eq!(get_one(&db, MD5_A, VKEY_1), None);
        assert_eq!(get_one(&db, MD5_B, VKEY_1), None);
        assert_eq!(get_one(&db, MD5_A, VKEY_2), Some(rated(3_000)));
        assert_eq!(
            get_one(&db, MD5_A, VKEY_3),
            Some(unrated(MsdStatusRow::CalcRejected))
        );

        assert_eq!(db.write(|tx| prune_except(tx, &[])).unwrap(), 5);
        assert_eq!(get_one(&db, MD5_A, VKEY_2), None);
    }
}
