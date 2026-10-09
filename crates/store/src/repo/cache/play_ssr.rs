//! Per-play Etterna-style SSRs under one `play_ssr` key (ADR 0024). Disposable like every cache.db
//! table: rebuilt from the vault and the ledger, never read by user.db.

use std::collections::BTreeSet;

use wolluf_core::{PlayId, VersionKey};

use crate::db::{Conn, Tx};
use crate::error::StoreError;
use crate::repo::sql::{enum_col, fixed, int, str_enum};

str_enum! {
    pub enum PlaySsrStatus {
        Counted => "counted",
        Incomplete => "incomplete",
        ScoreV2 => "score_v2",
        UnsupportedMods => "unsupported_mods",
        LnHeavy => "ln_heavy",
        CalcRejected => "calc_rejected",
        NoChart => "no_chart",
    }
}

/// `centi` is SSR × 100 in `chart_msd` skillset order, Overall first. A row is `Counted` exactly
/// when it carries both `goal_permyriad` and `centi`; any other row has no `centi`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlaySsrRow {
    pub play_id: PlayId,
    pub status: PlaySsrStatus,
    pub rate_milli: u16,
    pub goal_permyriad: Option<u16>,
    pub centi: Option<[i32; 8]>,
}

const SKILLSETS: &str =
    "overall, stream, jumpstream, handstream, stamina, jackspeed, chordjack, technical";

const GOAL_MAX_PERMYRIAD: u16 = 10_000;

fn check(rows: &[PlaySsrRow]) -> Result<(), StoreError> {
    let mut seen = BTreeSet::new();
    for r in rows {
        let counted = r.status == PlaySsrStatus::Counted;
        let complete = r.goal_permyriad.is_some() && r.centi.is_some();
        // An excluded play may keep its goal for display, but SSRs only exist for counted ones.
        if counted != complete || (!counted && r.centi.is_some()) {
            return Err(StoreError::InvalidData(format!(
                "play_ssr {}: status {} with goal {:?} and skillsets {}",
                r.play_id,
                r.status.as_str(),
                r.goal_permyriad,
                if r.centi.is_some() { "set" } else { "unset" }
            )));
        }
        if r.goal_permyriad.is_some_and(|g| g > GOAL_MAX_PERMYRIAD) {
            return Err(StoreError::InvalidData(format!(
                "play_ssr {}: goal {:?} above {GOAL_MAX_PERMYRIAD}",
                r.play_id, r.goal_permyriad
            )));
        }
        if !seen.insert(r.play_id) {
            return Err(StoreError::InvalidData(format!(
                "play_ssr {}: twice in one batch",
                r.play_id
            )));
        }
    }
    Ok(())
}

/// Writes each play's row under `vkey`, replacing that play's previous row under the same key;
/// other keys stay until `prune_except`. One invalid row fails the whole batch before any write.
pub fn replace_many(tx: &Tx<'_>, vkey: VersionKey, rows: &[PlaySsrRow]) -> Result<(), StoreError> {
    check(rows)?;
    let mut insert = tx.0.prepare_cached(&format!(
        "INSERT OR REPLACE INTO play_ssr (play_id, vkey, status, rate_milli, goal_permyriad, {SKILLSETS})
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)"
    ))?;
    for r in rows {
        let c = r.centi.map_or([None; 8], |c| c.map(Some));
        insert.execute(rusqlite::params![
            r.play_id.0,
            vkey.0,
            r.status.as_str(),
            r.rate_milli,
            r.goal_permyriad,
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

// One JSON parameter instead of one placeholder per play keeps any request under SQLite's
// bound-variable limit without chunking; `unhex` turns it back into the blob key.
fn id_list(play_ids: &[PlayId]) -> Result<String, StoreError> {
    let hex: Vec<String> = play_ids.iter().map(ToString::to_string).collect();
    serde_json::to_string(&hex).map_err(|e| StoreError::InvalidData(format!("play_ssr ids: {e}")))
}

// The writer stores all eight skillsets or none; a partial row is corrupt, not "unset".
fn centi(row: &rusqlite::Row<'_>, at: usize) -> rusqlite::Result<Option<[i32; 8]>> {
    let mut cols = [None; 8];
    for (i, slot) in cols.iter_mut().enumerate() {
        *slot = row.get::<_, Option<i32>>(at + i)?;
    }
    match cols.iter().filter(|c| c.is_some()).count() {
        0 => Ok(None),
        8 => Ok(Some(cols.map(Option::unwrap_or_default))),
        n => Err(rusqlite::Error::FromSqlConversionFailure(
            at,
            rusqlite::types::Type::Null,
            Box::new(StoreError::InvalidData(format!(
                "play_ssr: {n} of 8 skillsets set"
            ))),
        )),
    }
}

/// Rows under `vkey` for those of `play_ids` that have one, in play id order.
pub fn get_many(
    conn: Conn<'_>,
    vkey: VersionKey,
    play_ids: &[PlayId],
) -> Result<Vec<PlaySsrRow>, StoreError> {
    if play_ids.is_empty() {
        return Ok(Vec::new());
    }
    let mut stmt = conn.0.prepare_cached(&format!(
        "SELECT play_id, status, rate_milli, goal_permyriad, {SKILLSETS} FROM play_ssr
         WHERE vkey = ?1 AND play_id IN (SELECT unhex(value) FROM json_each(?2))
         ORDER BY play_id"
    ))?;
    let rows = stmt
        .query_map((vkey.0, id_list(play_ids)?), |row| {
            Ok(PlaySsrRow {
                play_id: PlayId(fixed(row, 0)?),
                status: enum_col(row, 1, PlaySsrStatus::parse)?,
                rate_milli: int(row, 2)?,
                goal_permyriad: row.get(3)?,
                centi: centi(row, 4)?,
            })
        })?
        .collect::<Result<_, _>>()?;
    Ok(rows)
}

/// Those of `play_ids` with no row under `vkey`, deduplicated in play id order: the backfill
/// queue. An excluded play has a row and is not missing.
pub fn missing_among(
    conn: Conn<'_>,
    vkey: VersionKey,
    play_ids: &[PlayId],
) -> Result<Vec<PlayId>, StoreError> {
    if play_ids.is_empty() {
        return Ok(Vec::new());
    }
    let mut stmt = conn.0.prepare_cached(
        "SELECT DISTINCT unhex(j.value) AS id FROM json_each(?2) j
         WHERE NOT EXISTS (SELECT 1 FROM play_ssr s WHERE s.play_id = unhex(j.value) AND s.vkey = ?1)
         ORDER BY id",
    )?;
    let ids = stmt
        .query_map((vkey.0, id_list(play_ids)?), |row| {
            Ok(PlayId(fixed(row, 0)?))
        })?
        .collect::<Result<_, _>>()?;
    Ok(ids)
}

/// GC hook (§5.5); an empty `keep` clears the table. Each keymode has its own `play_ssr` key, so
/// callers pass every current one. Returns the number of rows deleted.
pub fn prune_except(tx: &Tx<'_>, keep: &[VersionKey]) -> Result<u64, StoreError> {
    super::prune_vkeys_except(tx, "play_ssr", keep)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::open_cache_db;
    use crate::db::DbHandle;

    const VKEY_1: VersionKey = VersionKey([1; 32]);
    const VKEY_2: VersionKey = VersionKey([2; 32]);

    fn id(byte: u8) -> PlayId {
        PlayId([byte; 32])
    }

    fn db() -> (tempfile::TempDir, DbHandle) {
        let dir = tempfile::tempdir().unwrap();
        let db = open_cache_db(&dir.path().join("cache.db")).unwrap();
        (dir, db)
    }

    fn counted(byte: u8, base: i32) -> PlaySsrRow {
        PlaySsrRow {
            play_id: id(byte),
            status: PlaySsrStatus::Counted,
            rate_milli: 1000,
            goal_permyriad: Some(9_300),
            centi: Some(std::array::from_fn(|i| base + i32::try_from(i).unwrap())),
        }
    }

    fn excluded(byte: u8, status: PlaySsrStatus) -> PlaySsrRow {
        PlaySsrRow {
            play_id: id(byte),
            status,
            rate_milli: 1500,
            goal_permyriad: None,
            centi: None,
        }
    }

    fn put(db: &DbHandle, vkey: VersionKey, rows: &[PlaySsrRow]) -> Result<(), StoreError> {
        let rows = rows.to_vec();
        db.write(move |tx| replace_many(tx, vkey, &rows))
    }

    fn read(db: &DbHandle, vkey: VersionKey, ids: &[PlayId]) -> Vec<PlaySsrRow> {
        db.read(|c| get_many(c, vkey, ids)).unwrap()
    }

    fn missing(db: &DbHandle, vkey: VersionKey, ids: &[PlayId]) -> Vec<PlayId> {
        db.read(|c| missing_among(c, vkey, ids)).unwrap()
    }

    #[test]
    fn round_trip_is_keyed_and_play_id_ordered() {
        let (_dir, db) = db();
        let rows = [
            counted(9, 2_400),
            excluded(3, PlaySsrStatus::ScoreV2),
            counted(5, 1_800),
        ];
        put(&db, VKEY_1, &rows).unwrap();
        put(&db, VKEY_2, &[counted(3, 3_000)]).unwrap();

        // Every status survives the text column.
        let every: Vec<_> = PlaySsrStatus::ALL
            .iter()
            .filter(|s| **s != PlaySsrStatus::Counted)
            .zip(20_u8..)
            .map(|(s, b)| excluded(b, *s))
            .collect();
        put(&db, VKEY_1, &every).unwrap();

        let got = read(&db, VKEY_1, &[id(9), id(3), id(5), id(7)]);
        assert_eq!(got, [rows[1], rows[2], rows[0]], "absent id 7 is skipped");
        assert_eq!(read(&db, VKEY_2, &[id(9), id(3)]), [counted(3, 3_000)]);
        let ids: Vec<_> = every.iter().map(|r| r.play_id).collect();
        assert_eq!(read(&db, VKEY_1, &ids), every);
        assert!(read(&db, VKEY_1, &[]).is_empty());
    }

    #[test]
    fn replace_overwrites_a_play_under_the_same_key_only() {
        let (_dir, db) = db();
        put(&db, VKEY_1, &[counted(1, 2_000), counted(2, 2_100)]).unwrap();
        put(&db, VKEY_2, &[counted(1, 3_000)]).unwrap();

        let now_rejected = excluded(1, PlaySsrStatus::CalcRejected);
        put(&db, VKEY_1, &[now_rejected]).unwrap();

        assert_eq!(
            read(&db, VKEY_1, &[id(1), id(2)]),
            [now_rejected, counted(2, 2_100)]
        );
        assert_eq!(read(&db, VKEY_2, &[id(1)]), [counted(1, 3_000)]);
    }

    #[test]
    fn counted_iff_goal_and_centi_is_enforced_atomically() {
        let (_dir, db) = db();
        put(&db, VKEY_1, &[counted(1, 2_000)]).unwrap();

        let no_goal = PlaySsrRow {
            goal_permyriad: None,
            ..counted(2, 0)
        };
        let no_centi = PlaySsrRow {
            centi: None,
            ..counted(2, 0)
        };
        let excluded_with_both = PlaySsrRow {
            status: PlaySsrStatus::Incomplete,
            ..counted(2, 0)
        };
        let excluded_with_centi = PlaySsrRow {
            goal_permyriad: None,
            ..excluded_with_both
        };
        let goal_above_one = PlaySsrRow {
            goal_permyriad: Some(10_001),
            ..counted(2, 0)
        };
        let duplicate = [counted(2, 0), counted(2, 1)];
        for bad in [
            no_goal,
            no_centi,
            excluded_with_both,
            excluded_with_centi,
            goal_above_one,
        ] {
            // The valid row before the bad one must not land either.
            let err = put(&db, VKEY_1, &[counted(1, 9_999), bad]).unwrap_err();
            assert!(
                matches!(err, StoreError::InvalidData(_)),
                "{bad:?}: {err:?}"
            );
        }
        // A non-counted row may keep its goal for display.
        let excluded_with_goal = PlaySsrRow {
            centi: None,
            ..excluded_with_both
        };
        put(&db, VKEY_1, &[excluded_with_goal]).unwrap();
        assert!(matches!(
            put(&db, VKEY_1, &duplicate).unwrap_err(),
            StoreError::InvalidData(_)
        ));
        assert_eq!(
            read(&db, VKEY_1, &[id(1), id(2)]),
            [counted(1, 2_000), excluded_with_goal]
        );
    }

    #[test]
    fn missing_lists_requested_plays_without_a_row_under_the_key() {
        let (_dir, db) = db();
        put(
            &db,
            VKEY_1,
            &[counted(2, 2_000), excluded(4, PlaySsrStatus::NoChart)],
        )
        .unwrap();
        put(&db, VKEY_2, &[counted(5, 2_000)]).unwrap();

        // An excluded row is a processed play, not a missing one; duplicates collapse.
        assert_eq!(
            missing(&db, VKEY_1, &[id(5), id(4), id(3), id(2), id(5)]),
            [id(3), id(5)]
        );
        assert_eq!(missing(&db, VKEY_2, &[id(2), id(5)]), [id(2)]);
        assert!(missing(&db, VKEY_1, &[]).is_empty());
    }

    #[test]
    fn many_ids_fit_in_one_statement() {
        let (_dir, db) = db();
        // Above SQLite's default 32 766 bound-variable limit.
        let ids: Vec<PlayId> = (0_u32..40_000)
            .map(|n| {
                let mut b = [0; 32];
                b[..4].copy_from_slice(&n.to_be_bytes());
                PlayId(b)
            })
            .collect();
        let rows: Vec<_> = ids[..10]
            .iter()
            .map(|p| PlaySsrRow {
                play_id: *p,
                ..excluded(0, PlaySsrStatus::LnHeavy)
            })
            .collect();
        put(&db, VKEY_1, &rows).unwrap();
        assert_eq!(read(&db, VKEY_1, &ids), rows);
        assert_eq!(missing(&db, VKEY_1, &ids), ids[10..]);
    }

    #[test]
    fn prune_keeps_only_the_given_keys() {
        let (_dir, db) = db();
        put(&db, VKEY_1, &[counted(1, 2_000), counted(2, 2_000)]).unwrap();
        put(&db, VKEY_2, &[counted(1, 3_000)]).unwrap();

        assert_eq!(db.write(|tx| prune_except(tx, &[VKEY_2])).unwrap(), 2);
        assert!(read(&db, VKEY_1, &[id(1), id(2)]).is_empty());
        assert_eq!(read(&db, VKEY_2, &[id(1)]), [counted(1, 3_000)]);
        assert_eq!(db.write(|tx| prune_except(tx, &[])).unwrap(), 1);
        assert!(read(&db, VKEY_2, &[id(1)]).is_empty());
    }

    #[test]
    fn a_partially_set_skillset_row_is_reported_not_read_as_unset() {
        let (_dir, db) = db();
        db.write(|tx| {
            Ok(tx.0.execute(
                "INSERT INTO play_ssr (play_id, vkey, status, rate_milli, overall)
                 VALUES (?1, ?2, 'no_chart', 1000, 2500)",
                (id(1).0, VKEY_1.0),
            )?)
        })
        .unwrap();
        assert!(db.read(|c| get_many(c, VKEY_1, &[id(1)])).is_err());
    }

    #[test]
    fn check_constraints_hold() {
        let (_dir, db) = db();
        let insert = |play_id: &'static str, vkey: &'static str, status: &'static str| {
            db.write(move |tx| {
                Ok(tx.0.execute(
                    &format!(
                        "INSERT INTO play_ssr (play_id, vkey, status, rate_milli)
                         VALUES ({play_id}, {vkey}, ?1, 1000)"
                    ),
                    [status],
                )?)
            })
        };
        assert!(insert("zeroblob(32)", "zeroblob(32)", "pending").is_err());
        assert!(insert("zeroblob(31)", "zeroblob(32)", "no_chart").is_err());
        assert!(insert("zeroblob(32)", "zeroblob(33)", "no_chart").is_err());
        assert!(insert("'p'", "zeroblob(32)", "no_chart").is_err(), "STRICT");
        assert_eq!(
            insert("zeroblob(32)", "zeroblob(32)", "no_chart").unwrap(),
            1
        );
        assert!(
            insert("zeroblob(32)", "zeroblob(32)", "counted").is_err(),
            "one row per play and key"
        );
    }
}
