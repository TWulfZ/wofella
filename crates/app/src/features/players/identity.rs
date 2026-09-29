//! The blocking core of the players feature (spec 004): reads facts from the stores, runs the
//! pure selection/stats/scope modules and writes decisions and profiles. It takes DB handles
//! instead of the `AppContext`, so the `RefreshIdentity` job can run it from a `JobCtx`.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use wolluf_core::{AliasId, BlobSha256, ChartMd5, Clock, Game, Keymode, ProfileId, UnixUs};
use wolluf_source_osu::cfg_files::{list_user_cfgs, read_user_cfg};
use wolluf_store::repo::cache::catalog_chart;
use wolluf_store::repo::ledger::{Alias, SnapshotKind, game_install, play, source_snapshot};
use wolluf_store::repo::players::{
    self as repo, AliasOrigin, AliasStatsRow, DecisionVia, IdentityDecision, IdentityFacts,
    IdentityFeedback, NewProfile, ProfileKind, alias_stats, identity_decision, profile,
    profile_alias,
};
use wolluf_store::{DbHandle, StoreError, Tx};

use super::IdentityParams;
use super::scope::{self, MergeMode, ResolvedScope, ScopeAlias, ScopeEntry};
use super::selection::{
    AliasFacts, AliasSelection, AutoMatch, Decision, SELECTION_VERSION, SelectionInputs, select,
};
use super::stats::{AliasStats, KeymodeBucket, PlayFact, compute, stats_vkey};
use crate::errors::{AppError, players_keys as keys};

/// Persisted label of the singleton self profile (spec 004 Behaviour 1).
const SELF_LABEL: &str = "Me";
/// Spec 004 Data: the identity feedback event records the self merged 7K scope, the MVP
/// keymode, whatever keymode the user is looking at.
const FEEDBACK_SCOPE_KEYMODE: Keymode = Keymode::K7;
/// Spec 004 IPC: labels are 1–64 characters after trimming.
const LABEL_MAX_CHARS: usize = 64;
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone, PartialEq)]
pub struct TopChart {
    pub chart_md5: ChartMd5,
    pub title: Option<String>,
    pub version: Option<String>,
    pub n: u32,
}

/// One row of the "Which of these are you?" table, in the R6 order.
#[derive(Debug, Clone, PartialEq)]
pub struct AliasRow {
    pub alias_id: AliasId,
    pub raw_name: Vec<u8>,
    pub norm_len: u32,
    pub stats: AliasStats,
    pub top_charts: Vec<TopChart>,
    pub auto_match: Option<AutoMatch>,
    pub decision: Option<Decision>,
    pub selected: bool,
    pub in_self_profile: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AliasList {
    pub selection_version: u32,
    pub cfg_username_available: bool,
    pub wizard_needed: bool,
    pub rows: Vec<AliasRow>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum EntryRef {
    Profile(ProfileId),
    /// Virtual "mixed, not a person"; never persisted, never default.
    AllPlayers,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    SelfProfile,
    Other,
    AllPlayers,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileEntry {
    pub entry: EntryRef,
    pub kind: EntryKind,
    /// Empty for All players: the UI shows its i18n label.
    pub label: String,
    pub is_default: bool,
    pub merge_mode: MergeMode,
    pub alias_ids: Vec<AliasId>,
    pub scopes: Vec<ResolvedScope>,
}

/// Where a decision batch comes from; recorded in the feedback event (spec 004 Data).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecisionSource {
    Wizard,
    Settings,
    ProfileEdit,
}

impl DecisionSource {
    fn via(self) -> DecisionVia {
        match self {
            Self::Wizard => DecisionVia::Wizard,
            Self::Settings => DecisionVia::Settings,
            Self::ProfileEdit => DecisionVia::ProfileEdit,
        }
    }
}

/// `n_by_keymode_json` rows, in the canonical bucket order.
#[derive(Serialize, Deserialize)]
struct BucketCount {
    bucket: String,
    n: u32,
}

/// `top_charts_json` rows.
#[derive(Serialize, Deserialize)]
struct ChartCount {
    md5: String,
    n: u32,
}

fn to_decision(d: IdentityDecision) -> Decision {
    match d {
        IdentityDecision::Me => Decision::Me,
        IdentityDecision::NotMe => Decision::NotMe,
    }
}

fn from_decision(d: Decision) -> IdentityDecision {
    match d {
        Decision::Me => IdentityDecision::Me,
        Decision::NotMe => IdentityDecision::NotMe,
    }
}

fn to_merge(m: repo::MergeMode) -> MergeMode {
    match m {
        repo::MergeMode::Merged => MergeMode::Merged,
        repo::MergeMode::Separate => MergeMode::Separate,
    }
}

fn from_merge(m: MergeMode) -> repo::MergeMode {
    match m {
        MergeMode::Merged => repo::MergeMode::Merged,
        MergeMode::Separate => repo::MergeMode::Separate,
    }
}

fn duplicates_rejected(ids: impl IntoIterator<Item = AliasId>) -> Result<(), AppError> {
    let mut seen = BTreeSet::new();
    for id in ids {
        if !seen.insert(id) {
            return Err(AppError::invalid_input()
                .with_key(keys::DUPLICATE_ALIAS)
                .with_arg("aliasId", id.0.to_string()));
        }
    }
    Ok(())
}

fn unknown_alias(id: AliasId) -> AppError {
    AppError::not_found()
        .with_key(keys::UNKNOWN_ALIAS)
        .with_arg("aliasId", id.0.to_string())
}

fn unknown_profile(id: ProfileId) -> AppError {
    AppError::not_found()
        .with_key(keys::UNKNOWN_PROFILE)
        .with_arg("profileId", id.0.to_string())
}

fn overlap(profiles: &BTreeSet<ProfileId>) -> AppError {
    let ids: Vec<String> = profiles.iter().map(|p| p.0.to_string()).collect();
    AppError::conflict()
        .with_key(keys::SELF_OVERLAP)
        .with_arg("profileIds", ids.join(","))
}

fn valid_label(label: &str) -> Result<String, AppError> {
    let trimmed = label.trim();
    let chars = trimmed.chars().count();
    if chars == 0 || chars > LABEL_MAX_CHARS {
        return Err(AppError::invalid_input().with_key(keys::INVALID_LABEL));
    }
    Ok(trimmed.to_owned())
}

fn keymode_bucket(keymodes: &BTreeMap<ChartMd5, u8>, md5: ChartMd5) -> KeymodeBucket {
    keymodes
        .get(&md5)
        .and_then(|k| Keymode::new(*k).ok())
        .map_or(KeymodeBucket::Unknown, KeymodeBucket::Keys)
}

fn encode_stats(s: &AliasStats) -> Result<AliasStatsRow, AppError> {
    let json = |v: serde_json::Result<serde_json::Value>| {
        v.map_err(|e| AppError::internal(format!("alias_stats json: {e}")))
    };
    let buckets: Vec<BucketCount> = s
        .by_keymode
        .iter()
        .map(|(b, n)| BucketCount {
            bucket: b.to_string(),
            n: *n,
        })
        .collect();
    let charts: Vec<ChartCount> = s
        .top_charts
        .iter()
        .map(|(md5, n)| ChartCount {
            md5: md5.to_string(),
            n: *n,
        })
        .collect();
    Ok(AliasStatsRow {
        alias_id: s.alias_id,
        n_plays: s.n_plays,
        n_by_keymode: json(serde_json::to_value(buckets))?,
        first_ts: s.first_played_at,
        last_ts: s.last_played_at,
        n_with_replay: s.n_with_replay,
        n_online_ids: s.n_online,
        top_charts: json(serde_json::to_value(charts))?,
    })
}

/// `None` when a row does not parse: the caller recomputes instead of trusting it.
fn decode_stats(row: &AliasStatsRow) -> Option<AliasStats> {
    let buckets: Vec<BucketCount> = serde_json::from_value(row.n_by_keymode.clone()).ok()?;
    let charts: Vec<ChartCount> = serde_json::from_value(row.top_charts.clone()).ok()?;
    Some(AliasStats {
        alias_id: row.alias_id,
        n_plays: row.n_plays,
        by_keymode: buckets
            .into_iter()
            .map(|b| Some((KeymodeBucket::parse(&b.bucket)?, b.n)))
            .collect::<Option<_>>()?,
        first_played_at: row.first_ts,
        last_played_at: row.last_ts,
        n_online: row.n_online_ids,
        n_with_replay: row.n_with_replay,
        top_charts: charts
            .into_iter()
            .map(|c| Some((c.md5.parse().ok()?, c.n)))
            .collect::<Option<_>>()?,
    })
}

/// The aliases a scope sees for a set of ids, with their raw names.
fn scope_aliases(names: &BTreeMap<AliasId, Vec<u8>>, ids: &[AliasId]) -> Vec<ScopeAlias> {
    ids.iter()
        .filter_map(|id| {
            names.get(id).map(|raw| ScopeAlias {
                alias_id: *id,
                raw_name: raw.clone(),
            })
        })
        .collect()
}

/// What the self profile should hold: every `me` alias as a user row, plus the undecided
/// session-user aliases (R6) as auto rows. An alias placed in an `other` profile is never
/// auto-added: the user already said it is someone else.
fn self_target(
    conn: wolluf_store::Conn<'_>,
    login: Option<&str>,
    params: &IdentityParams,
) -> Result<BTreeMap<AliasId, AliasOrigin>, StoreError> {
    let aliases = wolluf_store::repo::ledger::alias::list(conn)?;
    let decisions = identity_decision::list(conn)?;
    let others = other_members(conn)?;
    let inputs = SelectionInputs {
        aliases: aliases
            .iter()
            .map(|a| AliasFacts {
                alias_id: a.id,
                raw_name: a.raw_name.clone(),
                // Membership does not depend on the order, which is all play counts affect.
                n_plays: 0,
            })
            .collect(),
        cfg_username: login.map(str::to_owned),
        linked_username: None,
        decisions: decisions
            .iter()
            .map(|(id, row)| (*id, to_decision(row.decision)))
            .collect(),
    };
    let mut target = BTreeMap::new();
    for row in select(&inputs, params) {
        match row.decision {
            Some(Decision::Me) => {
                target.insert(row.alias_id, AliasOrigin::User);
            }
            Some(Decision::NotMe) => {}
            None if row.auto_match.is_some() && !others.contains_key(&row.alias_id) => {
                target.insert(row.alias_id, AliasOrigin::Auto);
            }
            None => {}
        }
    }
    Ok(target)
}

/// Alias → the `other` profiles holding it.
fn other_members(
    conn: wolluf_store::Conn<'_>,
) -> Result<BTreeMap<AliasId, BTreeSet<ProfileId>>, StoreError> {
    let others: BTreeSet<ProfileId> = profile::list(conn)?
        .into_iter()
        .filter(|p| p.kind == ProfileKind::Other)
        .map(|p| p.id)
        .collect();
    let mut members: BTreeMap<AliasId, BTreeSet<ProfileId>> = BTreeMap::new();
    for row in profile_alias::list_all(conn)? {
        if others.contains(&row.profile_id) {
            members
                .entry(row.alias_id)
                .or_default()
                .insert(row.profile_id);
        }
    }
    Ok(members)
}

struct SelfSync {
    profile_id: ProfileId,
    changed: bool,
}

/// Creates the self profile if missing ("Me", merged, default unless one exists) and moves its
/// rows to [`self_target`]. Only differences are written, so a no-op refresh commits nothing.
fn sync_self(
    tx: &Tx<'_>,
    login: Option<&str>,
    params: &IdentityParams,
    now: UnixUs,
) -> Result<SelfSync, StoreError> {
    let (profile_id, created) = match profile::self_profile(tx.conn())? {
        Some(p) => (p.id, false),
        None => {
            let has_default = profile::list(tx.conn())?.iter().any(|p| p.is_default);
            let id = profile::insert(
                tx,
                &NewProfile {
                    kind: ProfileKind::SelfProfile,
                    label: SELF_LABEL.to_owned(),
                    is_default: !has_default,
                    merge_mode: repo::MergeMode::Merged,
                    created_at: now,
                },
            )?;
            (id, true)
        }
    };
    let target = self_target(tx.conn(), login, params)?;
    let current: BTreeMap<AliasId, AliasOrigin> = profile_alias::list(tx.conn(), profile_id)?
        .into_iter()
        .map(|r| (r.alias_id, r.origin))
        .collect();
    let mut changed = created;
    for id in current.keys().filter(|id| !target.contains_key(id)) {
        profile_alias::remove(tx, profile_id, *id)?;
        changed = true;
    }
    for (id, origin) in &target {
        if current.get(id) != Some(origin) {
            profile_alias::add(tx, profile_id, *id, *origin, now)?;
            changed = true;
        }
    }
    Ok(SelfSync {
        profile_id,
        changed,
    })
}

/// The feedback context scope: the self profile merged, 7K, after the change.
fn self_scope_hash(
    tx: &Tx<'_>,
    profile_id: ProfileId,
) -> Result<Option<wolluf_core::ScopeHash>, StoreError> {
    let names: BTreeMap<AliasId, Vec<u8>> = wolluf_store::repo::ledger::alias::list(tx.conn())?
        .into_iter()
        .map(|a| (a.id, a.raw_name))
        .collect();
    let ids: Vec<AliasId> = profile_alias::list(tx.conn(), profile_id)?
        .iter()
        .map(|r| r.alias_id)
        .collect();
    let entry = ScopeEntry::Profile {
        label: SELF_LABEL.to_owned(),
        merge_mode: MergeMode::Merged,
        aliases: scope_aliases(&names, &ids),
    };
    Ok(
        scope::resolve(&entry, &[], FEEDBACK_SCOPE_KEYMODE, Some(MergeMode::Merged))
            .first()
            .map(|s| s.hash),
    )
}

fn to_system_time(t: UnixUs) -> std::time::SystemTime {
    crate::jobs::to_system_time(t)
}

/// A validation failure found inside a write transaction. Nothing has been written when it is
/// returned, so the (empty) transaction commits harmlessly.
type Checked<T> = Result<T, AppError>;

#[derive(Clone)]
pub(crate) struct Identity {
    pub(crate) user: DbHandle,
    pub(crate) cache: DbHandle,
    pub(crate) clock: Arc<dyn Clock>,
    pub(crate) params: IdentityParams,
}

impl Identity {
    /// Spec 004 R13: the `Username` of the newest `osu!.<account>.cfg` over every registered
    /// install, read live and never stored. Missing, unreadable or blank means no login.
    fn session_login(&self) -> Result<Option<String>, AppError> {
        let roots: Vec<PathBuf> = self
            .user
            .read(game_install::list)?
            .into_iter()
            .map(|i| i.root_path)
            .collect();
        let newest = roots
            .iter()
            .filter_map(|root| list_user_cfgs(root).ok())
            .flatten()
            .max_by(|a, b| (a.mtime, &b.path).cmp(&(b.mtime, &a.path)));
        Ok(newest
            .and_then(|cfg| read_user_cfg(&cfg.path).ok())
            .and_then(|(cfg, _)| cfg.username)
            .filter(|u| !u.trim().is_empty()))
    }

    /// The osu!.db snapshot the catalog was built from; it decides the keymode buckets.
    fn catalog_osu_db(&self) -> Result<Option<BlobSha256>, AppError> {
        let Some(catalog_snapshot) = self.cache.read(catalog_chart::snapshot_id)? else {
            return Ok(None);
        };
        for install in self.user.read(game_install::list)? {
            let latest = self
                .user
                .read(|c| source_snapshot::latest(c, install.id, SnapshotKind::OsuDb))?;
            if let Some(s) = latest.filter(|s| s.id == catalog_snapshot) {
                return Ok(Some(s.sha256));
            }
        }
        Ok(None)
    }

    /// Stats under the current vkey: read from cache.db when present, else computed and
    /// stored (older keys are pruned). Returns whether they had to be (re)computed.
    fn stats(&self, facts: &IdentityFacts) -> Result<(Vec<AliasStats>, bool), AppError> {
        let keymodes = self.cache.read(catalog_chart::keymodes)?;
        let plays: Vec<PlayFact> = facts
            .plays
            .iter()
            .map(|p| PlayFact {
                play_id: p.play_id,
                alias_id: p.alias_id,
                played_at: p.played_at,
                chart_md5: p.chart_md5,
                keymode: keymode_bucket(&keymodes, p.chart_md5),
                has_online_id: p.has_online_id,
                has_replay: p.has_replay,
            })
            .collect();
        let ids: Vec<_> = plays.iter().map(|p| p.play_id).collect();
        let vkey = stats_vkey(&self.params, &ids, self.catalog_osu_db()?)
            .map_err(|e| AppError::internal(format!("alias_stats vkey: {e}")))?;
        let cached = self.cache.read(|c| alias_stats::get_all(c, vkey))?;
        let alias_ids: Vec<AliasId> = facts.aliases.iter().map(|a| a.id).collect();
        let cached_ids: Vec<AliasId> = cached.iter().map(|r| r.alias_id).collect();
        if cached_ids == alias_ids
            && let Some(stats) = cached.iter().map(decode_stats).collect::<Option<Vec<_>>>()
        {
            return Ok((stats, false));
        }
        let stats = compute(&alias_ids, &plays, &self.params);
        let rows = stats
            .iter()
            .map(encode_stats)
            .collect::<Result<Vec<_>, _>>()?;
        self.cache.write(move |tx| {
            alias_stats::put(tx, vkey, &rows)?;
            alias_stats::prune_except(tx, &[vkey])?;
            Ok(())
        })?;
        Ok((stats, true))
    }

    /// Spec 004 Behaviour 1: stats, self-profile bootstrap and auto reconcile. Returns whether
    /// anything a listing shows changed.
    pub(crate) fn refresh(&self) -> Result<bool, AppError> {
        let login = self.session_login()?;
        let facts = self.user.read(repo::identity_facts)?;
        let (_, recomputed) = self.stats(&facts)?;
        let (params, now) = (self.params, self.clock.now());
        let synced = self
            .user
            .write(move |tx| sync_self(tx, login.as_deref(), &params, now))?;
        Ok(recomputed || synced.changed)
    }

    /// No plays means nothing to confirm yet; afterwards the wizard shows until confirmed.
    pub(crate) fn wizard_needed(&self) -> Result<bool, AppError> {
        Ok(self
            .user
            .read(|c| Ok(repo::wizard_completed_at(c)?.is_none() && play::count(c)? > 0))?)
    }

    pub(crate) fn list_aliases(&self) -> Result<AliasList, AppError> {
        let login = self.session_login()?;
        let facts = self.user.read(repo::identity_facts)?;
        let (stats, _) = self.stats(&facts)?;
        let decisions = self.user.read(identity_decision::list)?;
        let self_ids: BTreeSet<AliasId> = match self.user.read(profile::self_profile)? {
            Some(p) => self
                .user
                .read(|c| profile_alias::list(c, p.id))?
                .into_iter()
                .map(|r| r.alias_id)
                .collect(),
            None => BTreeSet::new(),
        };
        let stats: BTreeMap<AliasId, AliasStats> =
            stats.into_iter().map(|s| (s.alias_id, s)).collect();
        let names: BTreeMap<AliasId, &Alias> = facts.aliases.iter().map(|a| (a.id, a)).collect();
        let inputs = SelectionInputs {
            aliases: facts
                .aliases
                .iter()
                .map(|a| AliasFacts {
                    alias_id: a.id,
                    raw_name: a.raw_name.clone(),
                    n_plays: stats.get(&a.id).map_or(0, |s| s.n_plays),
                })
                .collect(),
            cfg_username: login.clone(),
            linked_username: None,
            decisions: decisions
                .iter()
                .map(|(id, row)| (*id, to_decision(row.decision)))
                .collect(),
        };
        let mut rows = Vec::new();
        for sel in select(&inputs, &self.params) {
            let AliasSelection {
                alias_id,
                norm_len,
                auto_match,
                decision,
                selected,
            } = sel;
            let (Some(alias), Some(stats)) = (names.get(&alias_id), stats.get(&alias_id)) else {
                continue;
            };
            let top_charts = stats
                .top_charts
                .iter()
                .map(|(md5, n)| {
                    let chart = self.cache.read(|c| catalog_chart::get(c, *md5))?;
                    Ok(TopChart {
                        chart_md5: *md5,
                        title: chart.as_ref().map(|c| c.title.clone()),
                        version: chart.map(|c| c.version),
                        n: *n,
                    })
                })
                .collect::<Result<Vec<_>, AppError>>()?;
            rows.push(AliasRow {
                alias_id,
                raw_name: alias.raw_name.clone(),
                norm_len,
                stats: stats.clone(),
                top_charts,
                auto_match,
                decision,
                selected,
                in_self_profile: self_ids.contains(&alias_id),
            });
        }
        Ok(AliasList {
            selection_version: SELECTION_VERSION,
            cfg_username_available: login.is_some(),
            wizard_needed: self.wizard_needed()?,
            rows,
        })
    }

    /// Spec 004 Behaviour 4–5: one user.db transaction writes the decisions, one feedback event
    /// each and the self reconcile. `None` clears a decision.
    pub(crate) fn decide(
        &self,
        batch: Vec<(AliasId, Option<Decision>)>,
        completes_wizard: bool,
        source: DecisionSource,
    ) -> Result<(), AppError> {
        duplicates_rejected(batch.iter().map(|(id, _)| *id))?;
        let login = self.session_login()?;
        let (params, now) = (self.params, self.clock.now());
        self.user
            .write(move |tx| -> Result<Checked<()>, StoreError> {
                if let Err(e) = decide_in(tx, &batch, source, login.as_deref(), &params, now)? {
                    return Ok(Err(e));
                }
                if completes_wizard {
                    repo::set_wizard_completed_at(tx, now)?;
                }
                Ok(Ok(()))
            })??;
        Ok(())
    }

    pub(crate) fn create_profile(
        &self,
        label: &str,
        alias_ids: Vec<AliasId>,
        merge_mode: Option<MergeMode>,
    ) -> Result<ProfileId, AppError> {
        let label = valid_label(label)?;
        if alias_ids.is_empty() {
            return Err(AppError::invalid_input().with_key(keys::EMPTY_ALIASES));
        }
        duplicates_rejected(alias_ids.iter().copied())?;
        let now = self.clock.now();
        let id = self
            .user
            .write(move |tx| -> Result<Checked<ProfileId>, StoreError> {
                if let Err(e) = check_other_aliases(tx, &alias_ids)? {
                    return Ok(Err(e));
                }
                let id = profile::insert(
                    tx,
                    &NewProfile {
                        kind: ProfileKind::Other,
                        label,
                        is_default: false,
                        merge_mode: from_merge(merge_mode.unwrap_or(MergeMode::Merged)),
                        created_at: now,
                    },
                )?;
                let entries: Vec<_> = alias_ids.iter().map(|a| (*a, AliasOrigin::User)).collect();
                profile_alias::replace(tx, id, &entries, now)?;
                Ok(Ok(id))
            })??;
        Ok(id)
    }

    /// On the self profile this is sugar over decisions (spec 004 Behaviour 7): added aliases
    /// become `me`, removed ones `not_me`.
    pub(crate) fn set_profile_aliases(
        &self,
        profile_id: ProfileId,
        alias_ids: Vec<AliasId>,
        merge_mode: Option<MergeMode>,
    ) -> Result<(), AppError> {
        duplicates_rejected(alias_ids.iter().copied())?;
        let login = self.session_login()?;
        let (params, now) = (self.params, self.clock.now());
        self.user
            .write(move |tx| -> Result<Checked<()>, StoreError> {
                let Some(p) = profile::get(tx.conn(), profile_id)? else {
                    return Ok(Err(unknown_profile(profile_id)));
                };
                let outcome = match p.kind {
                    ProfileKind::SelfProfile => {
                        let current: BTreeSet<AliasId> = profile_alias::list(tx.conn(), p.id)?
                            .iter()
                            .map(|r| r.alias_id)
                            .collect();
                        let wanted: BTreeSet<AliasId> = alias_ids.iter().copied().collect();
                        let batch: Vec<(AliasId, Option<Decision>)> = wanted
                            .difference(&current)
                            .map(|a| (*a, Some(Decision::Me)))
                            .chain(
                                current
                                    .difference(&wanted)
                                    .map(|a| (*a, Some(Decision::NotMe))),
                            )
                            .collect();
                        decide_in(
                            tx,
                            &batch,
                            DecisionSource::ProfileEdit,
                            login.as_deref(),
                            &params,
                            now,
                        )?
                    }
                    ProfileKind::Other => {
                        if alias_ids.is_empty() {
                            return Ok(
                                Err(AppError::invalid_input().with_key(keys::EMPTY_ALIASES)),
                            );
                        }
                        match check_other_aliases(tx, &alias_ids)? {
                            Ok(()) => {
                                let entries: Vec<_> =
                                    alias_ids.iter().map(|a| (*a, AliasOrigin::User)).collect();
                                profile_alias::replace(tx, p.id, &entries, now)?;
                                Ok(())
                            }
                            Err(e) => Err(e),
                        }
                    }
                };
                if outcome.is_ok()
                    && let Some(mode) = merge_mode
                {
                    profile::set_merge_mode(tx, p.id, from_merge(mode))?;
                }
                Ok(outcome)
            })??;
        Ok(())
    }

    pub(crate) fn set_default(&self, profile_id: ProfileId) -> Result<(), AppError> {
        self.user
            .write(move |tx| -> Result<Checked<()>, StoreError> {
                if profile::get(tx.conn(), profile_id)?.is_none() {
                    return Ok(Err(unknown_profile(profile_id)));
                }
                profile::set_default(tx, profile_id)?;
                Ok(Ok(()))
            })??;
        Ok(())
    }

    /// Persisted profiles in id order, then All players, each with its scopes for `keymode`.
    pub(crate) fn list_profiles(&self, keymode: Keymode) -> Result<Vec<ProfileEntry>, AppError> {
        let (profiles, rows, aliases) = self.user.read(|c| {
            Ok((
                profile::list(c)?,
                profile_alias::list_all(c)?,
                wolluf_store::repo::ledger::alias::list(c)?,
            ))
        })?;
        let names: BTreeMap<AliasId, Vec<u8>> =
            aliases.into_iter().map(|a| (a.id, a.raw_name)).collect();
        let all: Vec<ScopeAlias> = names
            .iter()
            .map(|(id, raw)| ScopeAlias {
                alias_id: *id,
                raw_name: raw.clone(),
            })
            .collect();
        let mut out = Vec::with_capacity(profiles.len() + 1);
        for p in profiles {
            let ids: Vec<AliasId> = rows
                .iter()
                .filter(|r| r.profile_id == p.id)
                .map(|r| r.alias_id)
                .collect();
            let merge_mode = to_merge(p.merge_mode);
            let entry = ScopeEntry::Profile {
                label: p.label.clone(),
                merge_mode,
                aliases: scope_aliases(&names, &ids),
            };
            out.push(ProfileEntry {
                entry: EntryRef::Profile(p.id),
                kind: match p.kind {
                    ProfileKind::SelfProfile => EntryKind::SelfProfile,
                    ProfileKind::Other => EntryKind::Other,
                },
                label: p.label,
                is_default: p.is_default,
                merge_mode,
                alias_ids: ids,
                scopes: scope::resolve(&entry, &all, keymode, None),
            });
        }
        out.push(ProfileEntry {
            entry: EntryRef::AllPlayers,
            kind: EntryKind::AllPlayers,
            label: String::new(),
            is_default: false,
            merge_mode: MergeMode::Merged,
            alias_ids: names.keys().copied().collect(),
            scopes: scope::resolve(&ScopeEntry::AllPlayers, &all, keymode, None),
        });
        Ok(out)
    }

    /// `merge` is the URL override (spec 004 Behaviour 8).
    pub(crate) fn resolve_scopes(
        &self,
        entry: EntryRef,
        keymode: Keymode,
        merge: Option<MergeMode>,
    ) -> Result<Vec<ResolvedScope>, AppError> {
        let entries = self.list_profiles(keymode)?;
        let found = entries
            .into_iter()
            .find(|e| e.entry == entry)
            .ok_or_else(|| match entry {
                EntryRef::Profile(id) => unknown_profile(id),
                EntryRef::AllPlayers => AppError::internal("All players entry missing"),
            })?;
        if merge.is_none_or(|m| m == found.merge_mode) {
            return Ok(found.scopes);
        }
        let names: BTreeMap<AliasId, Vec<u8>> = self
            .user
            .read(wolluf_store::repo::ledger::alias::list)?
            .into_iter()
            .map(|a| (a.id, a.raw_name))
            .collect();
        let all = scope_aliases(&names, &names.keys().copied().collect::<Vec<_>>());
        let scope_entry = match entry {
            EntryRef::Profile(_) => ScopeEntry::Profile {
                label: found.label,
                merge_mode: found.merge_mode,
                aliases: scope_aliases(&names, &found.alias_ids),
            },
            EntryRef::AllPlayers => ScopeEntry::AllPlayers,
        };
        Ok(scope::resolve(&scope_entry, &all, keymode, merge))
    }
}

/// Unknown aliases are `NOT_FOUND`; an alias in the self set (a `me` decision or an auto row)
/// cannot join an `other` profile (spec 004 Behaviour 7).
fn check_other_aliases(tx: &Tx<'_>, alias_ids: &[AliasId]) -> Result<Checked<()>, StoreError> {
    let known: BTreeSet<AliasId> = wolluf_store::repo::ledger::alias::list(tx.conn())?
        .iter()
        .map(|a| a.id)
        .collect();
    if let Some(missing) = alias_ids.iter().find(|a| !known.contains(a)) {
        return Ok(Err(unknown_alias(*missing)));
    }
    let Some(me) = profile::self_profile(tx.conn())? else {
        return Ok(Ok(()));
    };
    let decisions = identity_decision::list(tx.conn())?;
    let self_set: BTreeSet<AliasId> = profile_alias::list(tx.conn(), me.id)?
        .iter()
        .map(|r| r.alias_id)
        .chain(
            decisions
                .iter()
                .filter(|(_, d)| d.decision == IdentityDecision::Me)
                .map(|(id, _)| *id),
        )
        .collect();
    if alias_ids.iter().any(|a| self_set.contains(a)) {
        return Ok(Err(overlap(&BTreeSet::from([me.id]))));
    }
    Ok(Ok(()))
}

/// Applies a decision batch inside `tx`, then reconciles the self profile and records one
/// feedback event per decision with the resulting self scope.
fn decide_in(
    tx: &Tx<'_>,
    batch: &[(AliasId, Option<Decision>)],
    source: DecisionSource,
    login: Option<&str>,
    params: &IdentityParams,
    now: UnixUs,
) -> Result<Checked<()>, StoreError> {
    let names: BTreeMap<AliasId, Vec<u8>> = wolluf_store::repo::ledger::alias::list(tx.conn())?
        .into_iter()
        .map(|a| (a.id, a.raw_name))
        .collect();
    if let Some((missing, _)) = batch.iter().find(|(id, _)| !names.contains_key(id)) {
        return Ok(Err(unknown_alias(*missing)));
    }
    let others = other_members(tx.conn())?;
    let conflicting: BTreeSet<ProfileId> = batch
        .iter()
        .filter(|(_, d)| *d == Some(Decision::Me))
        .filter_map(|(id, _)| others.get(id))
        .flatten()
        .copied()
        .collect();
    if !conflicting.is_empty() {
        return Ok(Err(overlap(&conflicting)));
    }

    for (alias_id, decision) in batch {
        match decision {
            Some(d) => identity_decision::upsert(tx, *alias_id, from_decision(*d), now)?,
            None => {
                identity_decision::clear(tx, *alias_id)?;
            }
        }
    }
    let synced = sync_self(tx, login, params, now)?;
    let scope_hash = self_scope_hash(tx, synced.profile_id)?;
    let mut ids = ulid::Generator::new();
    for (alias_id, decision) in batch {
        let id = match ids.generate_from_datetime(to_system_time(now)) {
            Ok(id) => id,
            Err(overflow) => overflow.commit_overflow_increment(),
        };
        repo::append_identity_feedback(
            tx,
            &IdentityFeedback {
                id,
                ts: now,
                profile_id: Some(synced.profile_id),
                game: Game::OsuStable,
                raw_name: names.get(alias_id).cloned().unwrap_or_default(),
                decision: decision.map(from_decision),
                via: source.via(),
                app_version: APP_VERSION.to_owned(),
                scope_hash,
            },
        )?;
    }
    Ok(Ok(()))
}
