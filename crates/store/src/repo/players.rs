//! Identity and profile repositories (spec 004 T6): decisions, profiles, profile aliases, the
//! identity facts the selection reads, identity feedback events, and cache.db `alias_stats`.

use std::collections::{BTreeMap, BTreeSet};

use rusqlite::{OptionalExtension, Row};
use serde_json::json;
use wolluf_core::{
    AliasId, ChartMd5, DotNetTicks, FileTime, Game, PlayId, ProfileId, ScopeHash, UnixUs,
    VersionKey,
};

use crate::db::{Conn, Tx};
use crate::error::StoreError;
use crate::repo::ledger::{Alias, NewFeedbackEvent};
use crate::repo::sql::{enum_col, fixed, int, json_value, opt_time, parsed, str_enum, time};
use crate::time::{format_rfc3339_ms, parse_rfc3339_ms};

str_enum! {
    /// The user's answer for an alias (§5.6); it always wins over the auto rule.
    pub enum IdentityDecision {
        Me => "me",
        NotMe => "not_me",
    }
}

str_enum! {
    pub enum ProfileKind {
        SelfProfile => "self",
        Other => "other",
    }
}

str_enum! {
    pub enum MergeMode {
        Merged => "merged",
        Separate => "separate",
    }
}

str_enum! {
    /// `auto` rows follow the session-user rule; `user` rows were put there by a decision.
    pub enum AliasOrigin {
        Auto => "auto",
        User => "user",
    }
}

str_enum! {
    pub enum DecisionVia {
        Wizard => "wizard",
        Settings => "settings",
        ProfileEdit => "profile_edit",
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecisionRow {
    pub decision: IdentityDecision,
    pub decided_at: UnixUs,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewProfile {
    pub kind: ProfileKind,
    pub label: String,
    pub is_default: bool,
    pub merge_mode: MergeMode,
    pub created_at: UnixUs,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Profile {
    pub id: ProfileId,
    pub kind: ProfileKind,
    pub label: String,
    pub is_default: bool,
    pub merge_mode: MergeMode,
    pub created_at: UnixUs,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProfileAlias {
    pub profile_id: ProfileId,
    pub alias_id: AliasId,
    pub origin: AliasOrigin,
    pub added_at: UnixUs,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReconcileOutcome {
    pub added: Vec<AliasId>,
    pub removed: Vec<AliasId>,
}

/// One play as the identity listing needs it. `played_at` comes from the exact FILETIME, not
/// from the millisecond text column.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdentityPlayFact {
    pub play_id: PlayId,
    pub alias_id: AliasId,
    pub played_at: UnixUs,
    pub chart_md5: ChartMd5,
    pub has_online_id: bool,
    pub has_replay: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IdentityFacts {
    pub aliases: Vec<Alias>,
    pub plays: Vec<IdentityPlayFact>,
}

/// Input of the identity `feedback_event` (spec 004 Data); the JSON shapes are built here so
/// every writer produces the same rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdentityFeedback {
    pub id: ulid::Ulid,
    pub ts: UnixUs,
    pub profile_id: Option<ProfileId>,
    pub game: Game,
    pub raw_name: Vec<u8>,
    /// `None` records a cleared decision.
    pub decision: Option<IdentityDecision>,
    pub via: DecisionVia,
    pub app_version: String,
    pub scope_hash: Option<ScopeHash>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AliasStatsRow {
    pub alias_id: AliasId,
    pub n_plays: u32,
    pub n_by_keymode: serde_json::Value,
    pub first_ts: Option<UnixUs>,
    pub last_ts: Option<UnixUs>,
    pub n_with_replay: u32,
    pub n_online_ids: u32,
    pub top_charts: serde_json::Value,
}

pub const IDENTITY_FEEDBACK_KIND: &str = "identity_decision";
pub const WIZARD_COMPLETED_KEY: &str = "identity.wizard_completed_at";

fn ms(t: UnixUs) -> String {
    format_rfc3339_ms(t)
}

/// Unique-index violations are domain conflicts (a second self profile, a second default), so
/// the app can answer `CONFLICT` instead of `INTERNAL`.
fn unique_as_conflict(e: rusqlite::Error, what: &str) -> StoreError {
    match &e {
        rusqlite::Error::SqliteFailure(f, _)
            if f.extended_code == rusqlite::ffi::SQLITE_CONSTRAINT_UNIQUE =>
        {
            StoreError::Conflict(what.to_owned())
        }
        _ => e.into(),
    }
}

pub mod identity_decision {
    use super::*;

    pub fn upsert(
        tx: &Tx<'_>,
        alias_id: AliasId,
        decision: IdentityDecision,
        now: UnixUs,
    ) -> Result<(), StoreError> {
        tx.0.execute(
            "INSERT INTO identity_decision (alias_id, decision, decided_at) VALUES (?1, ?2, ?3)
             ON CONFLICT (alias_id)
             DO UPDATE SET decision = excluded.decision, decided_at = excluded.decided_at",
            (alias_id.0, decision.as_str(), ms(now)),
        )?;
        Ok(())
    }

    /// Returns the alias to the auto rule (R6); false when it had no decision.
    pub fn clear(tx: &Tx<'_>, alias_id: AliasId) -> Result<bool, StoreError> {
        let n = tx.0.execute(
            "DELETE FROM identity_decision WHERE alias_id = ?1",
            [alias_id.0],
        )?;
        Ok(n == 1)
    }

    pub fn list(conn: Conn<'_>) -> Result<BTreeMap<AliasId, DecisionRow>, StoreError> {
        let mut stmt = conn
            .0
            .prepare("SELECT alias_id, decision, decided_at FROM identity_decision")?;
        let rows = stmt
            .query_map([], |row| {
                Ok((
                    AliasId(row.get(0)?),
                    DecisionRow {
                        decision: enum_col(row, 1, IdentityDecision::parse)?,
                        decided_at: time(row, 2)?,
                    },
                ))
            })?
            .collect::<Result<_, _>>()?;
        Ok(rows)
    }
}

pub mod profile {
    use super::*;

    const COLUMNS: &str = "id, kind, label, is_default, merge_mode, created_at";

    fn from_row(row: &Row<'_>) -> rusqlite::Result<Profile> {
        Ok(Profile {
            id: ProfileId(row.get(0)?),
            kind: enum_col(row, 1, ProfileKind::parse)?,
            label: row.get(2)?,
            is_default: row.get(3)?,
            merge_mode: enum_col(row, 4, MergeMode::parse)?,
            created_at: time(row, 5)?,
        })
    }

    pub fn insert(tx: &Tx<'_>, p: &NewProfile) -> Result<ProfileId, StoreError> {
        tx.0.execute(
            "INSERT INTO profile (kind, label, is_default, merge_mode, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            (
                p.kind.as_str(),
                &p.label,
                p.is_default,
                p.merge_mode.as_str(),
                ms(p.created_at),
            ),
        )
        .map_err(|e| unique_as_conflict(e, "self profile or default profile already exists"))?;
        Ok(ProfileId(tx.0.last_insert_rowid()))
    }

    pub fn get(conn: Conn<'_>, id: ProfileId) -> Result<Option<Profile>, StoreError> {
        Ok(conn
            .0
            .query_row(
                &format!("SELECT {COLUMNS} FROM profile WHERE id = ?1"),
                [id.0],
                from_row,
            )
            .optional()?)
    }

    pub fn list(conn: Conn<'_>) -> Result<Vec<Profile>, StoreError> {
        let mut stmt = conn
            .0
            .prepare(&format!("SELECT {COLUMNS} FROM profile ORDER BY id"))?;
        let rows = stmt.query_map([], from_row)?.collect::<Result<_, _>>()?;
        Ok(rows)
    }

    pub fn self_profile(conn: Conn<'_>) -> Result<Option<Profile>, StoreError> {
        Ok(conn
            .0
            .query_row(
                &format!("SELECT {COLUMNS} FROM profile WHERE kind = 'self'"),
                [],
                from_row,
            )
            .optional()?)
    }

    /// The partial unique index allows one default, so the old one is cleared first; the
    /// caller's transaction makes the switch atomic.
    pub fn set_default(tx: &Tx<'_>, id: ProfileId) -> Result<(), StoreError> {
        if get(tx.conn(), id)?.is_none() {
            return Err(StoreError::NotFound(format!("profile {}", id.0)));
        }
        tx.0.execute(
            "UPDATE profile SET is_default = 0 WHERE is_default = 1 AND id <> ?1",
            [id.0],
        )?;
        tx.0.execute("UPDATE profile SET is_default = 1 WHERE id = ?1", [id.0])?;
        Ok(())
    }

    pub fn set_merge_mode(tx: &Tx<'_>, id: ProfileId, mode: MergeMode) -> Result<bool, StoreError> {
        let n = tx.0.execute(
            "UPDATE profile SET merge_mode = ?2 WHERE id = ?1",
            (id.0, mode.as_str()),
        )?;
        Ok(n == 1)
    }
}

pub mod profile_alias {
    use super::*;

    const COLUMNS: &str = "profile_id, alias_id, origin, added_at";

    fn from_row(row: &Row<'_>) -> rusqlite::Result<ProfileAlias> {
        Ok(ProfileAlias {
            profile_id: ProfileId(row.get(0)?),
            alias_id: AliasId(row.get(1)?),
            origin: enum_col(row, 2, AliasOrigin::parse)?,
            added_at: time(row, 3)?,
        })
    }

    pub fn list(conn: Conn<'_>, profile_id: ProfileId) -> Result<Vec<ProfileAlias>, StoreError> {
        let mut stmt = conn.0.prepare(&format!(
            "SELECT {COLUMNS} FROM profile_alias WHERE profile_id = ?1 ORDER BY alias_id"
        ))?;
        let rows = stmt
            .query_map([profile_id.0], from_row)?
            .collect::<Result<_, _>>()?;
        Ok(rows)
    }

    pub fn list_all(conn: Conn<'_>) -> Result<Vec<ProfileAlias>, StoreError> {
        let mut stmt = conn.0.prepare(&format!(
            "SELECT {COLUMNS} FROM profile_alias ORDER BY profile_id, alias_id"
        ))?;
        let rows = stmt.query_map([], from_row)?.collect::<Result<_, _>>()?;
        Ok(rows)
    }

    /// Upsert: an existing row takes the new origin (e.g. auto → user after a `me` decision)
    /// and keeps its `added_at`.
    pub fn add(
        tx: &Tx<'_>,
        profile_id: ProfileId,
        alias_id: AliasId,
        origin: AliasOrigin,
        now: UnixUs,
    ) -> Result<(), StoreError> {
        tx.0.prepare_cached(
            "INSERT INTO profile_alias (profile_id, alias_id, origin, added_at)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT (profile_id, alias_id) DO UPDATE SET origin = excluded.origin",
        )?
        .execute((profile_id.0, alias_id.0, origin.as_str(), ms(now)))?;
        Ok(())
    }

    pub fn remove(
        tx: &Tx<'_>,
        profile_id: ProfileId,
        alias_id: AliasId,
    ) -> Result<bool, StoreError> {
        let n = tx
            .0
            .prepare_cached("DELETE FROM profile_alias WHERE profile_id = ?1 AND alias_id = ?2")?
            .execute((profile_id.0, alias_id.0))?;
        Ok(n == 1)
    }

    /// Makes the profile's alias set exactly `entries`.
    pub fn replace(
        tx: &Tx<'_>,
        profile_id: ProfileId,
        entries: &[(AliasId, AliasOrigin)],
        now: UnixUs,
    ) -> Result<(), StoreError> {
        let wanted: BTreeSet<AliasId> = entries.iter().map(|(id, _)| *id).collect();
        for row in list(tx.conn(), profile_id)? {
            if !wanted.contains(&row.alias_id) {
                remove(tx, profile_id, row.alias_id)?;
            }
        }
        for &(alias_id, origin) in entries {
            add(tx, profile_id, alias_id, origin, now)?;
        }
        Ok(())
    }

    /// Moves the `auto` rows to `auto_set` (R6) and never touches `user` rows. Deciding which
    /// aliases are undecided is the caller's job; this only applies the result.
    pub fn reconcile_auto(
        tx: &Tx<'_>,
        profile_id: ProfileId,
        auto_set: &BTreeSet<AliasId>,
        now: UnixUs,
    ) -> Result<ReconcileOutcome, StoreError> {
        let current = list(tx.conn(), profile_id)?;
        let present: BTreeSet<AliasId> = current.iter().map(|r| r.alias_id).collect();
        let mut outcome = ReconcileOutcome::default();
        for row in &current {
            if row.origin == AliasOrigin::Auto && !auto_set.contains(&row.alias_id) {
                remove(tx, profile_id, row.alias_id)?;
                outcome.removed.push(row.alias_id);
            }
        }
        for &alias_id in auto_set.difference(&present) {
            add(tx, profile_id, alias_id, AliasOrigin::Auto, now)?;
            outcome.added.push(alias_id);
        }
        Ok(outcome)
    }
}

pub fn identity_facts(conn: Conn<'_>) -> Result<IdentityFacts, StoreError> {
    let aliases = crate::repo::ledger::alias::list(conn)?;
    // Numeric order: FILETIME text of different lengths would not sort chronologically.
    let mut stmt = conn.0.prepare(
        "SELECT id, alias_id, filetime, chart_md5, online_score_id IS NOT NULL,
                replay_sha IS NOT NULL
         FROM play ORDER BY alias_id, CAST(filetime AS INTEGER), id",
    )?;
    let plays = stmt
        .query_map([], |row| {
            Ok(IdentityPlayFact {
                play_id: PlayId(fixed(row, 0)?),
                alias_id: AliasId(row.get(1)?),
                played_at: parsed(row, 2, |s| {
                    FileTime::parse_decimal(s)
                        .map_err(|e| e.to_string())?
                        .to_dotnet_ticks()
                        .map(DotNetTicks::to_unix_us)
                        .ok_or_else(|| "FILETIME overflows .NET ticks".to_owned())
                })?,
                chart_md5: parsed(row, 3, str::parse::<ChartMd5>)?,
                has_online_id: row.get(4)?,
                has_replay: row.get(5)?,
            })
        })?
        .collect::<Result<_, _>>()?;
    Ok(IdentityFacts { aliases, plays })
}

pub fn append_identity_feedback(tx: &Tx<'_>, f: &IdentityFeedback) -> Result<(), StoreError> {
    crate::repo::ledger::feedback_event::append(
        tx,
        &NewFeedbackEvent {
            id: f.id,
            ts: f.ts,
            profile_id: f.profile_id,
            kind: IDENTITY_FEEDBACK_KIND.to_owned(),
            subject: json!({
                "alias": {"game": f.game.as_str(), "raw_name_b64": base64(&f.raw_name)}
            }),
            payload: json!({
                "decision": f.decision.map(IdentityDecision::as_str),
                "via": f.via.as_str(),
            }),
            // No engine yet in F0, so no manifest or pack (spec 004 Data).
            context: json!({
                "app_version": f.app_version,
                "manifest_hash": null,
                "pack_id": null,
                "scope_hash": f.scope_hash.map(|h| h.to_string()),
            }),
        },
    )
}

pub fn wizard_completed_at(conn: Conn<'_>) -> Result<Option<UnixUs>, StoreError> {
    match crate::repo::ledger::settings::get(conn, WIZARD_COMPLETED_KEY)? {
        None => Ok(None),
        Some(serde_json::Value::String(text)) => Ok(Some(parse_rfc3339_ms(&text)?)),
        Some(other) => Err(StoreError::InvalidData(format!(
            "{WIZARD_COMPLETED_KEY} = {other}"
        ))),
    }
}

pub fn set_wizard_completed_at(tx: &Tx<'_>, now: UnixUs) -> Result<(), StoreError> {
    crate::repo::ledger::settings::set(tx, WIZARD_COMPLETED_KEY, &json!(ms(now)))
}

/// RFC 4648 standard alphabet with padding, for raw alias bytes inside JSON (spec 004 Data).
fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    const SEXTET: u32 = 0b11_1111;
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = chunk
            .iter()
            .enumerate()
            .fold(0_u32, |acc, (i, &b)| acc | u32::from(b) << (16 - 8 * i));
        for (i, shift) in [18, 12, 6, 0].into_iter().enumerate() {
            if i <= chunk.len() {
                out.push(char::from(ALPHABET[((n >> shift) & SEXTET) as usize]));
            } else {
                out.push('=');
            }
        }
    }
    out
}

pub mod alias_stats {
    use super::*;

    /// Replaces the rows stored under `vkey`; rows under other keys stay until `prune_except`.
    pub fn put(tx: &Tx<'_>, vkey: VersionKey, rows: &[AliasStatsRow]) -> Result<(), StoreError> {
        tx.0.execute("DELETE FROM alias_stats WHERE vkey = ?1", [vkey.0])?;
        let mut insert = tx.0.prepare_cached(
            "INSERT INTO alias_stats (alias_id, vkey, n_plays, n_by_keymode_json, first_ts,
                 last_ts, n_with_replay, n_online_ids, top_charts_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        )?;
        for r in rows {
            insert.execute((
                r.alias_id.0,
                vkey.0,
                r.n_plays,
                r.n_by_keymode.to_string(),
                r.first_ts.map(ms),
                r.last_ts.map(ms),
                r.n_with_replay,
                r.n_online_ids,
                r.top_charts.to_string(),
            ))?;
        }
        Ok(())
    }

    /// Only rows computed under `vkey` are returned: stale stats are invisible, never mixed in.
    pub fn get_all(conn: Conn<'_>, vkey: VersionKey) -> Result<Vec<AliasStatsRow>, StoreError> {
        let mut stmt = conn.0.prepare(
            "SELECT alias_id, n_plays, n_by_keymode_json, first_ts, last_ts, n_with_replay,
                    n_online_ids, top_charts_json
             FROM alias_stats WHERE vkey = ?1 ORDER BY alias_id",
        )?;
        let rows = stmt
            .query_map([vkey.0], |row| {
                Ok(AliasStatsRow {
                    alias_id: AliasId(row.get(0)?),
                    n_plays: int(row, 1)?,
                    n_by_keymode: json_value(row, 2)?,
                    first_ts: opt_time(row, 3)?,
                    last_ts: opt_time(row, 4)?,
                    n_with_replay: int(row, 5)?,
                    n_online_ids: int(row, 6)?,
                    top_charts: json_value(row, 7)?,
                })
            })?
            .collect::<Result<_, _>>()?;
        Ok(rows)
    }

    /// GC hook (§5.5 keeps recent keys); returns the number of rows deleted.
    pub fn prune_except(tx: &Tx<'_>, keep: &[VersionKey]) -> Result<u64, StoreError> {
        let mut stmt = tx.0.prepare("SELECT DISTINCT vkey FROM alias_stats")?;
        let stored = stmt
            .query_map([], |row| fixed(row, 0).map(VersionKey))?
            .collect::<Result<Vec<_>, _>>()?;
        let mut deleted = 0;
        for vkey in stored.into_iter().filter(|v| !keep.contains(v)) {
            deleted +=
                tx.0.execute("DELETE FROM alias_stats WHERE vkey = ?1", [vkey.0])?;
        }
        Ok(deleted as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::open_cache_db;
    use crate::repo::ledger::tests::{MD5_A, MD5_B, T0, new_play, osr_blob, scores_snapshot, tx};
    use crate::repo::ledger::{alias, feedback_event, play};
    use crate::user::open_user_db;
    use crate::user::testkit::migrated;

    fn me_profile(kind: ProfileKind, is_default: bool) -> NewProfile {
        NewProfile {
            kind,
            label: "Me".into(),
            is_default,
            merge_mode: MergeMode::Merged,
            created_at: T0,
        }
    }

    #[test]
    fn base64_rfc4648_vectors() {
        for (input, expected) in [
            (&b""[..], ""),
            (b"f", "Zg=="),
            (b"fo", "Zm8="),
            (b"foo", "Zm9v"),
            (b"foob", "Zm9vYg=="),
            (b"fooba", "Zm9vYmE="),
            (b"foobar", "Zm9vYmFy"),
            (&[0xff, 0x41], "/0E="),
        ] {
            assert_eq!(base64(input), expected);
        }
    }

    #[test]
    fn decision_upsert_and_clear() {
        let mut conn = migrated();
        let tx = tx(&mut conn);
        let a = alias::upsert(&tx, Game::OsuStable, b"").unwrap();
        let b = alias::upsert(&tx, Game::OsuStable, b"W").unwrap();
        identity_decision::upsert(&tx, a, IdentityDecision::Me, T0).unwrap();
        identity_decision::upsert(&tx, b, IdentityDecision::Me, T0).unwrap();
        let later = UnixUs(T0.0 + 1_000);
        identity_decision::upsert(&tx, b, IdentityDecision::NotMe, later).unwrap();
        assert_eq!(
            identity_decision::list(tx.conn()).unwrap(),
            [
                (
                    a,
                    DecisionRow {
                        decision: IdentityDecision::Me,
                        decided_at: T0
                    }
                ),
                (
                    b,
                    DecisionRow {
                        decision: IdentityDecision::NotMe,
                        decided_at: later
                    }
                ),
            ]
            .into()
        );
        assert!(identity_decision::clear(&tx, a).unwrap());
        assert!(!identity_decision::clear(&tx, a).unwrap());
        assert_eq!(identity_decision::list(tx.conn()).unwrap().len(), 1);
    }

    #[test]
    fn self_profile_singleton_index() {
        let mut conn = migrated();
        let tx = tx(&mut conn);
        let me = profile::insert(&tx, &me_profile(ProfileKind::SelfProfile, true)).unwrap();
        let second = profile::insert(&tx, &me_profile(ProfileKind::SelfProfile, false));
        assert!(matches!(second, Err(StoreError::Conflict(_))));
        profile::insert(&tx, &me_profile(ProfileKind::Other, false)).unwrap();
        profile::insert(&tx, &me_profile(ProfileKind::Other, false)).unwrap();
        assert_eq!(profile::self_profile(tx.conn()).unwrap().unwrap().id, me);
        assert_eq!(profile::list(tx.conn()).unwrap().len(), 3);
    }

    #[test]
    fn single_default_index() {
        let mut conn = migrated();
        let tx = tx(&mut conn);
        let me = profile::insert(&tx, &me_profile(ProfileKind::SelfProfile, true)).unwrap();
        let other = profile::insert(&tx, &me_profile(ProfileKind::Other, false)).unwrap();
        assert!(matches!(
            profile::insert(&tx, &me_profile(ProfileKind::Other, true)),
            Err(StoreError::Conflict(_))
        ));
        profile::set_default(&tx, other).unwrap();
        let defaults: Vec<ProfileId> = profile::list(tx.conn())
            .unwrap()
            .into_iter()
            .filter(|p| p.is_default)
            .map(|p| p.id)
            .collect();
        assert_eq!(defaults, vec![other]);
        profile::set_default(&tx, me).unwrap();
        assert!(profile::get(tx.conn(), me).unwrap().unwrap().is_default);
        assert!(!profile::get(tx.conn(), other).unwrap().unwrap().is_default);
        assert!(matches!(
            profile::set_default(&tx, ProfileId(999)),
            Err(StoreError::NotFound(_))
        ));
        // A missing target must not have cleared the current default.
        assert!(profile::get(tx.conn(), me).unwrap().unwrap().is_default);
        assert!(profile::set_merge_mode(&tx, other, MergeMode::Separate).unwrap());
        assert_eq!(
            profile::get(tx.conn(), other).unwrap().unwrap().merge_mode,
            MergeMode::Separate
        );
    }

    #[test]
    fn reconcile_auto_keeps_user_rows() {
        let mut conn = migrated();
        let tx = tx(&mut conn);
        let me = profile::insert(&tx, &me_profile(ProfileKind::SelfProfile, true)).unwrap();
        let [a, b, c, d] = [&b"TWulfZ"[..], b"cfg-string", b"", b"W"]
            .map(|n| alias::upsert(&tx, Game::OsuStable, n).unwrap());
        profile_alias::add(&tx, me, c, AliasOrigin::User, T0).unwrap();
        profile_alias::add(&tx, me, b, AliasOrigin::Auto, T0).unwrap();

        let later = UnixUs(T0.0 + 5_000);
        let outcome = profile_alias::reconcile_auto(&tx, me, &[a, c].into(), later).unwrap();
        assert_eq!(
            outcome,
            ReconcileOutcome {
                added: vec![a],
                removed: vec![b]
            }
        );
        let rows = profile_alias::list(tx.conn(), me).unwrap();
        assert_eq!(
            rows.iter()
                .map(|r| (r.alias_id, r.origin))
                .collect::<Vec<_>>(),
            vec![(a, AliasOrigin::Auto), (c, AliasOrigin::User)]
        );
        // A user row is never downgraded or dropped by the auto rule.
        assert_eq!(rows[1].added_at, T0);
        let again = profile_alias::reconcile_auto(&tx, me, &[a, c].into(), later).unwrap();
        assert_eq!(again, ReconcileOutcome::default());

        // replace keeps origins as given and preserves added_at of kept rows.
        profile_alias::replace(
            &tx,
            me,
            &[(a, AliasOrigin::User), (d, AliasOrigin::User)],
            later,
        )
        .unwrap();
        let rows = profile_alias::list(tx.conn(), me).unwrap();
        assert_eq!(
            rows.iter()
                .map(|r| (r.alias_id, r.origin, r.added_at))
                .collect::<Vec<_>>(),
            vec![(a, AliasOrigin::User, later), (d, AliasOrigin::User, later)]
        );
        assert!(profile_alias::remove(&tx, me, d).unwrap());
        assert!(!profile_alias::remove(&tx, me, d).unwrap());
        assert_eq!(profile_alias::list_all(tx.conn()).unwrap().len(), 1);
    }

    #[test]
    fn feedback_event_same_txn() {
        let dir = tempfile::tempdir().unwrap();
        let db = open_user_db(&dir.path().join("user.db"), &dir.path().join("b"), T0).unwrap();
        let alias_id = db
            .write(|tx| alias::upsert(tx, Game::OsuStable, &[0xff, 0x41]))
            .unwrap();
        let feedback = IdentityFeedback {
            id: ulid::Ulid::from_parts(7, 7),
            ts: T0,
            profile_id: None,
            game: Game::OsuStable,
            raw_name: vec![0xff, 0x41],
            decision: Some(IdentityDecision::Me),
            via: DecisionVia::Wizard,
            app_version: "0.1.0".into(),
            scope_hash: Some(ScopeHash([0xab; 32])),
        };

        let f = feedback.clone();
        let aborted: Result<(), StoreError> = db.write(move |tx| {
            identity_decision::upsert(tx, alias_id, IdentityDecision::Me, T0)?;
            append_identity_feedback(tx, &f)?;
            Err(StoreError::Conflict("later step failed".into()))
        });
        assert!(aborted.is_err());
        let (decisions, events) = db
            .read(|c| Ok((identity_decision::list(c)?, feedback_event::list(c)?)))
            .unwrap();
        assert!(decisions.is_empty() && events.is_empty());

        let f = feedback.clone();
        db.write(move |tx| {
            identity_decision::upsert(tx, alias_id, IdentityDecision::Me, T0)?;
            append_identity_feedback(tx, &f)
        })
        .unwrap();
        let (decisions, events) = db
            .read(|c| Ok((identity_decision::list(c)?, feedback_event::list(c)?)))
            .unwrap();
        assert_eq!(decisions.len(), 1);
        assert_eq!(events.len(), 1);
        let e = &events[0];
        assert_eq!(e.kind, IDENTITY_FEEDBACK_KIND);
        assert_eq!(e.telemetry_state, "local_only");
        assert_eq!(
            e.subject,
            json!({"alias": {"game": "osu_stable", "raw_name_b64": "/0E="}})
        );
        assert_eq!(e.payload, json!({"decision": "me", "via": "wizard"}));
        assert_eq!(
            e.context,
            json!({
                "app_version": "0.1.0",
                "manifest_hash": null,
                "pack_id": null,
                "scope_hash": "ab".repeat(32),
            })
        );

        let cleared = IdentityFeedback {
            id: ulid::Ulid::from_parts(8, 8),
            decision: None,
            via: DecisionVia::Settings,
            scope_hash: None,
            ..feedback
        };
        db.write(move |tx| append_identity_feedback(tx, &cleared))
            .unwrap();
        let events = db.read(feedback_event::list).unwrap();
        assert_eq!(
            events[1].payload,
            json!({"decision": null, "via": "settings"})
        );
        assert_eq!(events[1].context["scope_hash"], json!(null));
    }

    #[test]
    fn alias_stats_read_requires_vkey() {
        let dir = tempfile::tempdir().unwrap();
        let db = open_cache_db(&dir.path().join("cache.db")).unwrap();
        let (v1, v2) = (VersionKey([1; 32]), VersionKey([2; 32]));
        let row = AliasStatsRow {
            alias_id: AliasId(3),
            n_plays: 42,
            n_by_keymode: json!([{"bucket": "k7", "n": 40}, {"bucket": "unknown", "n": 2}]),
            first_ts: Some(T0),
            last_ts: None,
            n_with_replay: 41,
            n_online_ids: 5,
            top_charts: json!([{"chartMd5": MD5_A, "n": 12}]),
        };
        let rows = vec![row.clone()];
        db.write(move |tx| alias_stats::put(tx, v1, &rows)).unwrap();
        assert_eq!(
            db.read(|c| alias_stats::get_all(c, v1)).unwrap(),
            vec![row.clone()]
        );
        assert!(db.read(|c| alias_stats::get_all(c, v2)).unwrap().is_empty());

        // put replaces the rows of its own vkey only.
        let newer = AliasStatsRow {
            n_plays: 43,
            ..row.clone()
        };
        let rows = vec![newer.clone()];
        db.write(move |tx| alias_stats::put(tx, v2, &rows)).unwrap();
        let rows = vec![];
        db.write(move |tx| alias_stats::put(tx, v1, &rows)).unwrap();
        assert!(db.read(|c| alias_stats::get_all(c, v1)).unwrap().is_empty());
        assert_eq!(
            db.read(|c| alias_stats::get_all(c, v2)).unwrap(),
            vec![newer]
        );

        let rows = vec![row];
        db.write(move |tx| alias_stats::put(tx, v1, &rows)).unwrap();
        assert_eq!(
            db.write(move |tx| alias_stats::prune_except(tx, &[v2]))
                .unwrap(),
            1
        );
        assert!(db.read(|c| alias_stats::get_all(c, v1)).unwrap().is_empty());
    }

    #[test]
    fn identity_facts_roundtrip() {
        let mut conn = migrated();
        let tx = tx(&mut conn);
        let snap = scores_snapshot(&tx);
        let named = alias::upsert(&tx, Game::OsuStable, b"TWulfZ").unwrap();
        let empty = alias::upsert(&tx, Game::OsuStable, b"").unwrap();
        // The pilot golden tuple's FILETIME (ADR 0006).
        let filetime = 134_350_010_443_098_880;
        let online = new_play(named, b"TWulfZ", MD5_A, filetime, Some(snap));
        let mut offline = new_play(empty, b"", MD5_B, filetime + 1, Some(snap));
        offline.online_score_id = None;
        offline.replay_sha = Some(osr_blob(&tx, 0x01));
        play::insert_batch(&tx, &[online.clone(), offline.clone()]).unwrap();

        let facts = identity_facts(tx.conn()).unwrap();
        assert_eq!(
            facts
                .aliases
                .iter()
                .map(|a| a.raw_name.clone())
                .collect::<Vec<_>>(),
            vec![b"TWulfZ".to_vec(), Vec::new()]
        );
        let at = |ft: i64| {
            FileTime::new(ft)
                .and_then(FileTime::to_dotnet_ticks)
                .map(DotNetTicks::to_unix_us)
                .unwrap()
        };
        assert_eq!(
            facts.plays,
            vec![
                IdentityPlayFact {
                    play_id: online.id,
                    alias_id: named,
                    played_at: at(filetime),
                    chart_md5: online.chart_md5,
                    has_online_id: true,
                    has_replay: false,
                },
                IdentityPlayFact {
                    play_id: offline.id,
                    alias_id: empty,
                    played_at: at(filetime + 1),
                    chart_md5: offline.chart_md5,
                    has_online_id: false,
                    has_replay: true,
                },
            ]
        );
    }

    #[test]
    fn wizard_completed_flag() {
        let mut conn = migrated();
        let tx = tx(&mut conn);
        assert_eq!(wizard_completed_at(tx.conn()).unwrap(), None);
        set_wizard_completed_at(&tx, T0).unwrap();
        assert_eq!(wizard_completed_at(tx.conn()).unwrap(), Some(T0));
    }

    #[test]
    fn identity_enum_strings_are_stable() {
        let all: Vec<&str> = IdentityDecision::ALL
            .iter()
            .map(|v| v.as_str())
            .chain(ProfileKind::ALL.iter().map(|v| v.as_str()))
            .chain(MergeMode::ALL.iter().map(|v| v.as_str()))
            .chain(AliasOrigin::ALL.iter().map(|v| v.as_str()))
            .chain(DecisionVia::ALL.iter().map(|v| v.as_str()))
            .collect();
        assert_eq!(
            all,
            [
                "me",
                "not_me",
                "self",
                "other",
                "merged",
                "separate",
                "auto",
                "user",
                "wizard",
                "settings",
                "profile_edit"
            ]
        );
    }
}
