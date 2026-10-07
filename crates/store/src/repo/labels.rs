//! Pattern labels as append-only `feedback_event` rows (architecture §5.3, §6.1): a
//! `segment_label` event per gold-labelled anchor, a `play_label` event per session answer
//! about a played map's dominant pattern (ADR 0020), and an `undo` event that compensates one.

use std::collections::BTreeSet;

use rusqlite::OptionalExtension;
use serde::Deserialize;
use serde_json::json;
use wolluf_core::{
    ChartMd5, ColMask, Keymode, PatternId, PlayId, ProfileId, SegmentAnchor, TimeUs, UnixUs,
};

use crate::db::{Conn, Tx};
use crate::error::StoreError;
use crate::repo::ledger::{NewFeedbackEvent, feedback_event};
use crate::repo::sql::{parsed, time};

/// Stable feedback kinds (docs/conventions.md): persisted, never renamed.
pub const SEGMENT_LABEL_KIND: &str = "segment_label";
pub const UNDO_KIND: &str = "undo";
/// `payload.origin` of a label typed in the blind labeller; Playfield relabels (F1 deliverable
/// 5) will carry their own origin, so the gold set stays separable.
pub const GOLD_ORIGIN: &str = "gold";
/// `segment_label` payload layout. Persisted and replayed forever (architecture §5.3): a
/// changed shape bumps this or becomes a new kind (docs/conventions.md, stable ids), and
/// readers reject what they do not know instead of guessing.
pub const SEGMENT_LABEL_V: u32 = 1;
/// The anchor carries exactly these patterns: a full set, not a delta against the engine.
pub const ASSERT_SET_ACTION: &str = "assert_set";
/// The user looked and saw no clear pattern: a stored answer, unlike a skip, which is never
/// stored. Its `patterns` is always empty.
pub const ASSERT_NONE_ACTION: &str = "assert_none";

/// ADR 0020: the dominant pattern of a map the player just played. Never part of the gold set:
/// the map was chosen and played, not sampled blind.
pub const PLAY_LABEL_KIND: &str = "play_label";
pub const PLAYER_SESSION_ORIGIN: &str = "player_session";
/// `play_label` payload layout, versioned apart from `segment_label` (ADR 0020).
pub const PLAY_LABEL_V: u32 = 1;
pub const ASSERT_DOMINANT_ACTION: &str = "assert_dominant";

/// What the user answered for one anchor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GoldAnswer {
    /// Non-empty; stored sorted and deduplicated.
    Patterns(Vec<PatternId>),
    NoPattern,
}

crate::repo::sql::str_enum! {
    /// Which thumb the section feels better with; absent means neutral. The thumb is not a
    /// separate hand (layouts place it in one), so this is a preference, not a pattern.
    pub enum ThumbPref {
        Left => "left",
        Right => "right",
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewGoldLabel {
    pub id: ulid::Ulid,
    pub ts: UnixUs,
    pub profile_id: ProfileId,
    pub keymode: Keymode,
    pub anchor: SegmentAnchor,
    pub answer: GoldAnswer,
    pub mixed: bool,
    pub unsure: bool,
    pub thumb_pref: Option<ThumbPref>,
    pub app_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoldLabel {
    pub id: ulid::Ulid,
    pub ts: UnixUs,
    pub profile_id: ProfileId,
    pub keymode: Keymode,
    pub anchor: SegmentAnchor,
    pub answer: GoldAnswer,
    pub mixed: bool,
    pub unsure: bool,
    pub thumb_pref: Option<ThumbPref>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewUndo {
    pub id: ulid::Ulid,
    pub ts: UnixUs,
    pub profile_id: ProfileId,
    /// The `segment_label` event this one cancels.
    pub target: ulid::Ulid,
    pub app_version: String,
}

/// The single pattern that dominates a played map, or none clear.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DominantAnswer {
    Pattern(PatternId),
    NoPattern,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewPlayLabel {
    pub id: ulid::Ulid,
    pub ts: UnixUs,
    pub profile_id: ProfileId,
    pub chart_md5: ChartMd5,
    pub keymode: Keymode,
    /// The play the answer came after; context only, the subject is the chart.
    pub play_id: Option<PlayId>,
    pub answer: DominantAnswer,
    pub app_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayLabel {
    pub id: ulid::Ulid,
    pub ts: UnixUs,
    pub profile_id: ProfileId,
    pub chart_md5: ChartMd5,
    pub keymode: Keymode,
    pub play_id: Option<PlayId>,
    pub answer: DominantAnswer,
}

#[derive(Deserialize)]
struct PlaySubjectJson {
    chart_md5: String,
    keymode: u8,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PlayPayloadJson {
    v: u32,
    action: String,
    origin: String,
    pattern: Option<String>,
}

#[derive(Deserialize)]
struct PlayContextJson {
    play_id: Option<String>,
}

#[derive(Deserialize)]
struct SubjectJson {
    anchor: AnchorJson,
}

#[derive(Deserialize)]
struct AnchorJson {
    chart_md5: String,
    t0_us: i64,
    t1_us: i64,
    /// 0-based, column 0 leftmost, as `ColMask`.
    cols: Vec<u8>,
    keymode: u8,
}

#[derive(Deserialize)]
struct PayloadJson {
    v: u32,
    action: String,
    patterns: Vec<String>,
    flags: FlagsJson,
}

#[derive(Deserialize)]
struct FlagsJson {
    mixed: bool,
    unsure: bool,
    thumb_pref: Option<String>,
}

/// No engine manifest or pack exists yet, as for the identity events (spec 004 Data).
fn context(app_version: &str) -> serde_json::Value {
    json!({
        "app_version": app_version,
        "manifest_hash": null,
        "pack_id": null,
        "scope_hash": null,
    })
}

pub fn append_gold_label(tx: &Tx<'_>, l: &NewGoldLabel) -> Result<(), StoreError> {
    let (action, patterns): (&str, BTreeSet<&str>) = match &l.answer {
        GoldAnswer::Patterns(p) if p.is_empty() => {
            return Err(StoreError::InvalidData(format!(
                "label {} asserts an empty pattern set",
                l.id
            )));
        }
        GoldAnswer::Patterns(p) => (ASSERT_SET_ACTION, p.iter().map(PatternId::as_str).collect()),
        GoldAnswer::NoPattern => (ASSERT_NONE_ACTION, BTreeSet::new()),
    };
    let mut flags = json!({"mixed": l.mixed, "unsure": l.unsure});
    if let Some(thumb) = l.thumb_pref {
        flags["thumb_pref"] = json!(thumb.as_str());
    }
    let a = &l.anchor;
    feedback_event::append(
        tx,
        &NewFeedbackEvent {
            id: l.id,
            ts: l.ts,
            profile_id: Some(l.profile_id),
            kind: SEGMENT_LABEL_KIND.to_owned(),
            subject: json!({"anchor": {
                "chart_md5": a.chart_md5().to_string(),
                "t0_us": a.t0_us().0,
                "t1_us": a.t1_us().0,
                "cols": a.cols().iter().collect::<Vec<u8>>(),
                "keymode": l.keymode.columns(),
            }}),
            payload: json!({
                "v": SEGMENT_LABEL_V,
                "action": action,
                "origin": GOLD_ORIGIN,
                "patterns": patterns,
                "flags": flags,
            }),
            context: context(&l.app_version),
        },
    )
}

/// `NOT_FOUND` unless the target is a `segment_label` of the same profile; `CONFLICT` when it
/// was already undone.
pub fn append_undo(tx: &Tx<'_>, u: &NewUndo) -> Result<(), StoreError> {
    append_undo_of(tx, u, SEGMENT_LABEL_KIND)
}

/// [`append_undo`] for a `play_label`: each undo only ever takes back its own kind, so the gold
/// screen cannot cancel a session answer or the reverse.
pub fn append_play_label_undo(tx: &Tx<'_>, u: &NewUndo) -> Result<(), StoreError> {
    append_undo_of(tx, u, PLAY_LABEL_KIND)
}

fn append_undo_of(tx: &Tx<'_>, u: &NewUndo, kind: &str) -> Result<(), StoreError> {
    let target = u.target.to_string();
    let exists: Option<i64> =
        tx.0.query_row(
            "SELECT 1 FROM feedback_event WHERE id = ?1 AND kind = ?2 AND profile_id = ?3",
            (&target, kind, u.profile_id.0),
            |r| r.get(0),
        )
        .optional()?;
    if exists.is_none() {
        return Err(StoreError::NotFound(format!("{kind} event {target}")));
    }
    if undone_ids(tx.conn())?.contains(&target) {
        return Err(StoreError::Conflict(format!(
            "{kind} event {target} is already undone"
        )));
    }
    feedback_event::append(
        tx,
        &NewFeedbackEvent {
            id: u.id,
            ts: u.ts,
            profile_id: Some(u.profile_id),
            kind: UNDO_KIND.to_owned(),
            subject: json!({"event_id": target}),
            payload: json!({"undone_kind": kind}),
            context: context(&u.app_version),
        },
    )
}

pub fn append_play_label(tx: &Tx<'_>, l: &NewPlayLabel) -> Result<(), StoreError> {
    let payload = match &l.answer {
        DominantAnswer::Pattern(p) => json!({
            "v": PLAY_LABEL_V,
            "action": ASSERT_DOMINANT_ACTION,
            "origin": PLAYER_SESSION_ORIGIN,
            "pattern": p.as_str(),
        }),
        DominantAnswer::NoPattern => json!({
            "v": PLAY_LABEL_V,
            "action": ASSERT_NONE_ACTION,
            "origin": PLAYER_SESSION_ORIGIN,
        }),
    };
    let mut context = context(&l.app_version);
    context["play_id"] = json!(l.play_id.map(|p| p.to_string()));
    feedback_event::append(
        tx,
        &NewFeedbackEvent {
            id: l.id,
            ts: l.ts,
            profile_id: Some(l.profile_id),
            kind: PLAY_LABEL_KIND.to_owned(),
            subject: json!({"chart_md5": l.chart_md5.to_string(), "keymode": l.keymode.columns()}),
            payload,
            context,
        },
    )
}

/// The profile's `play_label` answers that no undo cancels, in append order.
pub fn play_labels(conn: Conn<'_>, profile_id: ProfileId) -> Result<Vec<PlayLabel>, StoreError> {
    let undone = undone_ids(conn)?;
    let mut stmt = conn.0.prepare_cached(
        "SELECT id, ts, subject_json, payload_json, context_json FROM feedback_event
         WHERE kind = ?1 AND profile_id = ?2 ORDER BY id",
    )?;
    let rows = stmt
        .query_map((PLAY_LABEL_KIND, profile_id.0), |row| {
            Ok((
                parsed(row, 0, ulid::Ulid::from_string)?,
                time(row, 1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    rows.into_iter()
        .filter(|(id, ..)| !undone.contains(&id.to_string()))
        .map(|(id, ts, subject, payload, context)| {
            decode_play_label(id, ts, profile_id, &subject, &payload, &context)
        })
        .collect()
}

fn decode_play_label(
    id: ulid::Ulid,
    ts: UnixUs,
    profile_id: ProfileId,
    subject: &str,
    payload: &str,
    context: &str,
) -> Result<PlayLabel, StoreError> {
    let invalid =
        |e: &dyn std::fmt::Display| StoreError::InvalidData(format!("play label {id}: {e}"));
    let s: PlaySubjectJson = serde_json::from_str(subject).map_err(|e| invalid(&e))?;
    let p: PlayPayloadJson = serde_json::from_str(payload).map_err(|e| invalid(&e))?;
    let c: PlayContextJson = serde_json::from_str(context).map_err(|e| invalid(&e))?;
    if p.v != PLAY_LABEL_V || p.origin != PLAYER_SESSION_ORIGIN {
        return Err(invalid(&format!("payload v {} origin {:?}", p.v, p.origin)));
    }
    let answer = match (p.action.as_str(), p.pattern) {
        (ASSERT_DOMINANT_ACTION, Some(pattern)) => {
            DominantAnswer::Pattern(PatternId::parse(&pattern).map_err(|e| invalid(&e))?)
        }
        (ASSERT_NONE_ACTION, None) => DominantAnswer::NoPattern,
        (action, pattern) => {
            return Err(invalid(&format!(
                "action {action:?} with pattern {pattern:?}"
            )));
        }
    };
    Ok(PlayLabel {
        id,
        ts,
        profile_id,
        chart_md5: s.chart_md5.parse().map_err(|e| invalid(&e))?,
        keymode: Keymode::new(s.keymode).map_err(|e| invalid(&e))?,
        play_id: c
            .play_id
            .map(|p| p.parse::<PlayId>())
            .transpose()
            .map_err(|e| invalid(&e))?,
        answer,
    })
}

fn undone_ids(conn: Conn<'_>) -> Result<BTreeSet<String>, StoreError> {
    let mut stmt = conn.0.prepare_cached(
        "SELECT json_extract(subject_json, '$.event_id') FROM feedback_event WHERE kind = ?1",
    )?;
    let ids = stmt
        .query_map([UNDO_KIND], |r| r.get::<_, String>(0))?
        .collect::<Result<_, _>>()?;
    Ok(ids)
}

/// The profile's labels that no undo cancels, in append order.
pub fn gold_labels(conn: Conn<'_>, profile_id: ProfileId) -> Result<Vec<GoldLabel>, StoreError> {
    let undone = undone_ids(conn)?;
    let mut stmt = conn.0.prepare_cached(
        "SELECT id, ts, subject_json, payload_json FROM feedback_event
         WHERE kind = ?1 AND profile_id = ?2 ORDER BY id",
    )?;
    let rows = stmt
        .query_map((SEGMENT_LABEL_KIND, profile_id.0), |row| {
            Ok((
                parsed(row, 0, ulid::Ulid::from_string)?,
                time(row, 1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    rows.into_iter()
        .filter(|(id, ..)| !undone.contains(&id.to_string()))
        .map(|(id, ts, subject, payload)| decode(id, ts, profile_id, &subject, &payload))
        .collect()
}

fn decode(
    id: ulid::Ulid,
    ts: UnixUs,
    profile_id: ProfileId,
    subject: &str,
    payload: &str,
) -> Result<GoldLabel, StoreError> {
    let invalid = |e: &dyn std::fmt::Display| StoreError::InvalidData(format!("label {id}: {e}"));
    let SubjectJson { anchor: a } = serde_json::from_str(subject).map_err(|e| invalid(&e))?;
    let p: PayloadJson = serde_json::from_str(payload).map_err(|e| invalid(&e))?;
    if p.v != SEGMENT_LABEL_V {
        return Err(invalid(&format!("payload v {}", p.v)));
    }
    let none = match (p.action.as_str(), p.patterns.is_empty()) {
        (ASSERT_SET_ACTION, false) => false,
        (ASSERT_NONE_ACTION, true) => true,
        (action, empty) => {
            return Err(invalid(&format!(
                "action {action:?} with {} patterns",
                if empty { "no" } else { "some" }
            )));
        }
    };
    let thumb_pref = p
        .flags
        .thumb_pref
        .as_deref()
        .map(|t| ThumbPref::parse(t).ok_or_else(|| invalid(&format!("thumb_pref {t:?}"))))
        .transpose()?;
    let keymode = Keymode::new(a.keymode).map_err(|e| invalid(&e))?;
    let md5 = a.chart_md5.parse().map_err(|e| invalid(&e))?;
    let cols = ColMask::from_cols(keymode, a.cols).map_err(|e| invalid(&e))?;
    let anchor = SegmentAnchor::new(md5, TimeUs(a.t0_us), TimeUs(a.t1_us), cols, keymode)
        .map_err(|e| invalid(&e))?;
    let answer = if none {
        GoldAnswer::NoPattern
    } else {
        GoldAnswer::Patterns(
            p.patterns
                .iter()
                .map(|s| PatternId::parse(s))
                .collect::<Result<_, _>>()
                .map_err(|e| invalid(&e))?,
        )
    };
    Ok(GoldLabel {
        id,
        ts,
        profile_id,
        keymode,
        anchor,
        answer,
        mixed: p.flags.mixed,
        unsure: p.flags.unsure,
        thumb_pref,
    })
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;
    use wolluf_core::{ColMask, Keymode, PatternId, ProfileId, SegmentAnchor, TimeUs, UnixUs};

    use super::*;
    use crate::repo::ledger::feedback_event;
    use crate::repo::ledger::tests::{MD5_A, MD5_B, T0, md5, tx};
    use crate::repo::players::{MergeMode, NewProfile, ProfileKind, profile};
    use crate::user::testkit::migrated;

    fn anchor(md5_hex: &str, t0_ms: i32, t1_ms: i32) -> SegmentAnchor {
        SegmentAnchor::new(
            md5(md5_hex),
            TimeUs::from_ms(t0_ms),
            TimeUs::from_ms(t1_ms),
            ColMask::full(Keymode::K7),
            Keymode::K7,
        )
        .unwrap()
    }

    fn me(conn: &mut Connection) -> ProfileId {
        let tx = tx(conn);
        let id = profile::insert(
            &tx,
            &NewProfile {
                kind: ProfileKind::SelfProfile,
                label: "Me".into(),
                is_default: true,
                merge_mode: MergeMode::Merged,
                created_at: T0,
            },
        )
        .unwrap();
        tx.0.commit().unwrap();
        id
    }

    fn label(id: u64, profile_id: ProfileId, anchor: SegmentAnchor) -> NewGoldLabel {
        NewGoldLabel {
            id: ulid::Ulid::from_parts(1, u128::from(id)),
            ts: UnixUs(T0.0 + id as i64 * 1_000),
            profile_id,
            keymode: Keymode::K7,
            anchor,
            answer: GoldAnswer::Patterns(vec![
                PatternId::from_static("regular.stream.jumpstream"),
                PatternId::from_static("regular.jack.minijack"),
            ]),
            mixed: true,
            unsure: false,
            thumb_pref: None,
            app_version: "0.1.0".into(),
        }
    }

    fn undo(id: u64, profile_id: ProfileId, target: u64) -> NewUndo {
        NewUndo {
            id: ulid::Ulid::from_parts(1, u128::from(id)),
            ts: UnixUs(T0.0 + id as i64 * 1_000),
            profile_id,
            target: ulid::Ulid::from_parts(1, u128::from(target)),
            app_version: "0.1.0".into(),
        }
    }

    #[test]
    fn gold_label_roundtrips_through_feedback_event() {
        let mut conn = migrated();
        let me = me(&mut conn);
        let tx = tx(&mut conn);
        let new = label(1, me, anchor(MD5_A, 1_000, 5_000));
        append_gold_label(&tx, &new).unwrap();

        let rows = feedback_event::list(tx.conn()).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].kind, SEGMENT_LABEL_KIND);
        assert_eq!(
            rows[0].subject,
            serde_json::json!({"anchor": {
                "chart_md5": MD5_A, "t0_us": 1_000_000, "t1_us": 5_000_000,
                "cols": [0, 1, 2, 3, 4, 5, 6], "keymode": 7,
            }})
        );
        assert_eq!(
            rows[0].payload,
            serde_json::json!({
                "v": 1,
                "action": "assert_set",
                "origin": "gold",
                "patterns": ["regular.jack.minijack", "regular.stream.jumpstream"],
                "flags": {"mixed": true, "unsure": false},
            })
        );
        assert_eq!(rows[0].context["app_version"], "0.1.0");

        let listed = gold_labels(tx.conn(), me).unwrap();
        assert_eq!(listed.len(), 1);
        let got = &listed[0];
        assert_eq!((got.id, got.ts, got.anchor), (new.id, new.ts, new.anchor));
        assert_eq!(got.keymode, Keymode::K7);
        assert_eq!(
            got.answer,
            GoldAnswer::Patterns(vec![
                PatternId::from_static("regular.jack.minijack"),
                PatternId::from_static("regular.stream.jumpstream"),
            ]),
            "sorted, as stored"
        );
        assert_eq!((got.mixed, got.unsure, got.thumb_pref), (true, false, None));
    }

    #[test]
    fn no_pattern_answer_and_thumb_side_roundtrip() {
        let mut conn = migrated();
        let me = me(&mut conn);
        let tx = tx(&mut conn);
        let none = NewGoldLabel {
            answer: GoldAnswer::NoPattern,
            mixed: false,
            unsure: true,
            thumb_pref: Some(ThumbPref::Left),
            ..label(1, me, anchor(MD5_A, 0, 4_000))
        };
        let right = NewGoldLabel {
            thumb_pref: Some(ThumbPref::Right),
            ..label(2, me, anchor(MD5_B, 0, 4_000))
        };
        append_gold_label(&tx, &none).unwrap();
        append_gold_label(&tx, &right).unwrap();
        let rows = feedback_event::list(tx.conn()).unwrap();
        assert_eq!(
            rows[0].payload,
            serde_json::json!({
                "v": 1,
                "action": "assert_none",
                "origin": "gold",
                "patterns": [],
                "flags": {"mixed": false, "unsure": true, "thumb_pref": "left"},
            })
        );
        assert_eq!(rows[1].payload["action"], "assert_set");
        assert_eq!(rows[1].payload["flags"]["thumb_pref"], "right");
        let got = gold_labels(tx.conn(), me).unwrap();
        assert_eq!(
            (got[0].answer.clone(), got[0].unsure, got[0].thumb_pref),
            (GoldAnswer::NoPattern, true, Some(ThumbPref::Left))
        );
        assert_eq!(got[1].thumb_pref, Some(ThumbPref::Right));
        let names: Vec<&str> = ThumbPref::ALL.iter().map(|t| t.as_str()).collect();
        assert_eq!(names, ["left", "right"]);
    }

    #[test]
    fn undo_compensates_and_listing_keeps_only_effective_labels() {
        let mut conn = migrated();
        let me = me(&mut conn);
        let tx = tx(&mut conn);
        append_gold_label(&tx, &label(1, me, anchor(MD5_A, 0, 4_000))).unwrap();
        append_gold_label(&tx, &label(2, me, anchor(MD5_B, 0, 4_000))).unwrap();
        append_undo(&tx, &undo(3, me, 1)).unwrap();

        let rows = feedback_event::list(tx.conn()).unwrap();
        assert_eq!(rows.len(), 3, "append-only: the undone label stays");
        assert_eq!(rows[2].kind, UNDO_KIND);
        assert_eq!(
            rows[2].subject,
            serde_json::json!({"event_id": ulid::Ulid::from_parts(1, 1).to_string()})
        );
        let ids: Vec<ulid::Ulid> = gold_labels(tx.conn(), me)
            .unwrap()
            .iter()
            .map(|l| l.id)
            .collect();
        assert_eq!(ids, [ulid::Ulid::from_parts(1, 2)]);
    }

    #[test]
    fn undo_rejects_unknown_and_already_undone_targets() {
        let mut conn = migrated();
        let me = me(&mut conn);
        let tx = tx(&mut conn);
        append_gold_label(&tx, &label(1, me, anchor(MD5_A, 0, 4_000))).unwrap();
        assert!(matches!(
            append_undo(&tx, &undo(2, me, 9)),
            Err(StoreError::NotFound(_))
        ));
        append_undo(&tx, &undo(3, me, 1)).unwrap();
        assert!(matches!(
            append_undo(&tx, &undo(4, me, 1)),
            Err(StoreError::Conflict(_))
        ));
        assert!(
            matches!(
                append_undo(&tx, &undo(5, me, 3)),
                Err(StoreError::NotFound(_))
            ),
            "only labels can be undone here"
        );
    }

    #[test]
    fn gold_labels_are_per_profile() {
        let mut conn = migrated();
        let me = me(&mut conn);
        let tx = tx(&mut conn);
        append_gold_label(&tx, &label(1, me, anchor(MD5_A, 0, 4_000))).unwrap();
        assert!(
            gold_labels(tx.conn(), ProfileId(me.0 + 1))
                .unwrap()
                .is_empty()
        );
        assert!(matches!(
            append_undo(&tx, &undo(2, ProfileId(me.0 + 1), 1)),
            Err(StoreError::NotFound(_))
        ));
    }

    #[test]
    fn decode_rejects_unknown_shapes_and_empty_patterns() {
        let good = serde_json::json!({
            "v": 1, "action": "assert_set", "origin": "gold",
            "patterns": ["regular.jack.minijack"], "flags": {"mixed": false, "unsure": false},
        });
        let with = |key: &str, value: serde_json::Value| {
            let mut p = good.clone();
            p[key] = value;
            p
        };
        let mut missing_v = good.clone();
        missing_v.as_object_mut().unwrap().remove("v");
        let none = with("action", serde_json::json!("assert_none"));
        let mut flags = good["flags"].clone();
        flags["thumb_pref"] = serde_json::json!("middle");
        for payload in [
            with("v", serde_json::json!(2)),
            with("action", serde_json::json!("deny")),
            with("patterns", serde_json::json!([])),
            none,
            with("flags", flags),
            missing_v,
        ] {
            let mut conn = migrated();
            let me = me(&mut conn);
            let tx = tx(&mut conn);
            append_gold_label(&tx, &label(1, me, anchor(MD5_A, 0, 4_000))).unwrap();
            let stored = feedback_event::list(tx.conn()).unwrap().remove(0);
            feedback_event::append(
                &tx,
                &crate::repo::ledger::NewFeedbackEvent {
                    id: ulid::Ulid::from_parts(1, 2),
                    ts: T0,
                    profile_id: Some(me),
                    kind: SEGMENT_LABEL_KIND.into(),
                    subject: stored.subject,
                    payload: payload.clone(),
                    context: stored.context,
                },
            )
            .unwrap();
            assert!(
                matches!(gold_labels(tx.conn(), me), Err(StoreError::InvalidData(_))),
                "{payload}"
            );
        }
    }

    #[test]
    fn append_refuses_an_empty_pattern_set() {
        let mut conn = migrated();
        let me = me(&mut conn);
        let tx = tx(&mut conn);
        let empty = NewGoldLabel {
            answer: GoldAnswer::Patterns(vec![]),
            ..label(1, me, anchor(MD5_A, 0, 4_000))
        };
        assert!(matches!(
            append_gold_label(&tx, &empty),
            Err(StoreError::InvalidData(_))
        ));
        assert!(feedback_event::list(tx.conn()).unwrap().is_empty());
    }

    fn play_label(id: u64, profile_id: ProfileId, answer: DominantAnswer) -> NewPlayLabel {
        NewPlayLabel {
            id: ulid::Ulid::from_parts(1, u128::from(id)),
            ts: UnixUs(T0.0 + id as i64 * 1_000),
            profile_id,
            chart_md5: md5(MD5_A),
            keymode: Keymode::K7,
            play_id: Some(wolluf_core::PlayId([7; 32])),
            answer,
            app_version: "0.1.0".into(),
        }
    }

    fn jumpstream() -> DominantAnswer {
        DominantAnswer::Pattern(PatternId::from_static("regular.stream.jumpstream"))
    }

    #[test]
    fn play_label_roundtrips_through_feedback_event() {
        let mut conn = migrated();
        let me = me(&mut conn);
        let tx = tx(&mut conn);
        let new = play_label(1, me, jumpstream());
        append_play_label(&tx, &new).unwrap();
        let none = NewPlayLabel {
            play_id: None,
            chart_md5: md5(MD5_B),
            ..play_label(2, me, DominantAnswer::NoPattern)
        };
        append_play_label(&tx, &none).unwrap();

        let rows = feedback_event::list(tx.conn()).unwrap();
        assert_eq!(rows[0].kind, PLAY_LABEL_KIND);
        assert_eq!(
            rows[0].subject,
            serde_json::json!({"chart_md5": MD5_A, "keymode": 7})
        );
        assert_eq!(
            rows[0].payload,
            serde_json::json!({
                "v": 1,
                "action": "assert_dominant",
                "origin": "player_session",
                "pattern": "regular.stream.jumpstream",
            })
        );
        assert_eq!(rows[0].context["play_id"], "07".repeat(32));
        assert_eq!(rows[0].context["app_version"], "0.1.0");
        assert_eq!(
            rows[1].payload,
            serde_json::json!({"v": 1, "action": "assert_none", "origin": "player_session"})
        );
        assert_eq!(rows[1].context["play_id"], serde_json::Value::Null);

        let got = play_labels(tx.conn(), me).unwrap();
        assert_eq!(got.len(), 2);
        assert_eq!(
            (got[0].id, got[0].ts, got[0].chart_md5, got[0].keymode),
            (new.id, new.ts, new.chart_md5, Keymode::K7)
        );
        assert_eq!(
            (got[0].play_id, got[0].answer.clone()),
            (new.play_id, jumpstream())
        );
        assert_eq!(
            (got[1].play_id, got[1].answer.clone()),
            (None, DominantAnswer::NoPattern)
        );
        assert!(
            play_labels(tx.conn(), ProfileId(me.0 + 1))
                .unwrap()
                .is_empty()
        );
    }

    /// ADR 0020: a session label is a weaker, chosen-not-blind source; the gold set never sees it.
    #[test]
    fn gold_readers_and_play_label_readers_stay_apart() {
        let mut conn = migrated();
        let me = me(&mut conn);
        let tx = tx(&mut conn);
        append_gold_label(&tx, &label(1, me, anchor(MD5_A, 0, 4_000))).unwrap();
        append_play_label(&tx, &play_label(2, me, jumpstream())).unwrap();
        let gold: Vec<ulid::Ulid> = gold_labels(tx.conn(), me)
            .unwrap()
            .iter()
            .map(|l| l.id)
            .collect();
        assert_eq!(gold, [ulid::Ulid::from_parts(1, 1)]);
        let session: Vec<ulid::Ulid> = play_labels(tx.conn(), me)
            .unwrap()
            .iter()
            .map(|l| l.id)
            .collect();
        assert_eq!(session, [ulid::Ulid::from_parts(1, 2)]);
    }

    #[test]
    fn play_label_undo_compensates_once_and_never_crosses_kinds() {
        let mut conn = migrated();
        let me = me(&mut conn);
        let tx = tx(&mut conn);
        append_gold_label(&tx, &label(1, me, anchor(MD5_A, 0, 4_000))).unwrap();
        append_play_label(&tx, &play_label(2, me, jumpstream())).unwrap();
        append_play_label(&tx, &play_label(3, me, DominantAnswer::NoPattern)).unwrap();

        assert!(
            matches!(
                append_undo(&tx, &undo(4, me, 2)),
                Err(StoreError::NotFound(_))
            ),
            "the gold undo never takes back a session label"
        );
        assert!(
            matches!(
                append_play_label_undo(&tx, &undo(4, me, 1)),
                Err(StoreError::NotFound(_))
            ),
            "the session undo never takes back a gold label"
        );
        assert!(matches!(
            append_play_label_undo(&tx, &undo(4, ProfileId(me.0 + 1), 3)),
            Err(StoreError::NotFound(_))
        ));
        append_play_label_undo(&tx, &undo(5, me, 3)).unwrap();
        assert!(matches!(
            append_play_label_undo(&tx, &undo(6, me, 3)),
            Err(StoreError::Conflict(_))
        ));

        let rows = feedback_event::list(tx.conn()).unwrap();
        let last = rows.last().unwrap();
        assert_eq!(last.kind, UNDO_KIND);
        assert_eq!(
            last.payload,
            serde_json::json!({"undone_kind": "play_label"})
        );
        let ids: Vec<ulid::Ulid> = play_labels(tx.conn(), me)
            .unwrap()
            .iter()
            .map(|l| l.id)
            .collect();
        assert_eq!(ids, [ulid::Ulid::from_parts(1, 2)]);
        assert_eq!(gold_labels(tx.conn(), me).unwrap().len(), 1);
    }

    #[test]
    fn play_label_decode_rejects_unknown_shapes() {
        let good = serde_json::json!({
            "v": 1, "action": "assert_dominant", "origin": "player_session",
            "pattern": "regular.jack.minijack",
        });
        let with = |key: &str, value: serde_json::Value| {
            let mut p = good.clone();
            p[key] = value;
            p
        };
        let mut no_pattern = good.clone();
        no_pattern.as_object_mut().unwrap().remove("pattern");
        for payload in [
            with("v", serde_json::json!(2)),
            with("action", serde_json::json!("assert_set")),
            with("action", serde_json::json!("assert_none")),
            with("pattern", serde_json::json!("not a pattern id!")),
            with("origin", serde_json::json!("gold")),
            with("flags", serde_json::json!({})),
            no_pattern,
        ] {
            let mut conn = migrated();
            let me = me(&mut conn);
            let tx = tx(&mut conn);
            feedback_event::append(
                &tx,
                &crate::repo::ledger::NewFeedbackEvent {
                    id: ulid::Ulid::from_parts(1, 2),
                    ts: T0,
                    profile_id: Some(me),
                    kind: PLAY_LABEL_KIND.into(),
                    subject: serde_json::json!({"chart_md5": MD5_A, "keymode": 7}),
                    payload: payload.clone(),
                    context: serde_json::json!({"app_version": "0.1.0", "play_id": null}),
                },
            )
            .unwrap();
            assert!(
                matches!(play_labels(tx.conn(), me), Err(StoreError::InvalidData(_))),
                "{payload}"
            );
        }
    }

    #[test]
    fn last_event_id_is_the_greatest_ulid() {
        let mut conn = migrated();
        let me = me(&mut conn);
        let tx = tx(&mut conn);
        assert_eq!(feedback_event::last_id(tx.conn()).unwrap(), None);
        append_gold_label(&tx, &label(7, me, anchor(MD5_A, 0, 4_000))).unwrap();
        append_gold_label(&tx, &label(3, me, anchor(MD5_B, 0, 4_000))).unwrap();
        assert_eq!(
            feedback_event::last_id(tx.conn()).unwrap(),
            Some(ulid::Ulid::from_parts(1, 7))
        );
    }
}
