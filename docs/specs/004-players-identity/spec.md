# 004 Players and identity

Status: Draft
Phase: F0 · Owner: TWulfZ · Date: 2026-09-28
Links: architecture §3, §4 (D2, D6, D11–D14, D17), §5.2–§5.6, §6.1, §7, §8, §10, §12 (F0) · ADR 0005 (identity scopes and auto-selection; amended by T1) · research `03-maniahub-rejudge-drills-sessions-audit.txt` (scores.db findings, lines ~307, 343, 347), `00-plan-es.md` (§ "Selección de jugadores", item 10) · oracle `research/scripts/audit/osudb.py`, `plays.py`
Depends on: 001 workspace-foundation (crates, `VersionKey`, lints, check-layers), 002 osu-stable-codecs (scores.db + cfg codecs, `cfg_files`), 003 store-ledger-sync (user.db baseline with the identity/profile tables, `play`/`alias` ledger, cache.db skeleton, `SyncPlays` job and follow-up chaining), 005 T3 (`AppError`), 005 desktop-shell-cli (Tauri shell, specta export, UI skeleton, setup flow). 006 osg-spike is **not** a dependency (see Domain rules R9).

## Problem
scores.db mixes the user's own plays, recorded under several names (some typed while offline: `""`, `W`, `w`, `Wulf`, `s`, and even the garbage cfg string), with replays by other people. Skill must be computed only for the identity the user selects (G3), so F0 has to show every name with enough context and pre-select the right ones. "Right" means `TWulfZ` auto-included and the offline names suggested but not ticked. Decisions must persist, and the user can also build profiles for other players and view "All players" without that ever feeding the self profile. The result has to be a stable scope (`scope_hash`) that F1–F3 key their fold on.

## Scope
- In:
  - Pure `app::features::players::{names, stats, heuristics, scope}` modules: normalization, alias statistics, signal scoring, tiers, scope canonical form and hash.
  - `wolluf-store` repositories for `identity_decision`, `profile`, `profile_alias`, identity facts and sample-play queries, identity `feedback_event` rows, and cache.db `alias_stats`.
  - `PlayersService` (the `pub` service of the feature): list aliases, decide, profiles, default, scope resolution, and the `RefreshIdentity` step chained after `SyncPlays`.
  - Six IPC commands with specta DTOs (`players_*`) and their Tauri command file.
  - UI slice `features/players`: the "Which of these are you?" wizard, Settings → Identity (same table), ScopePicker, Merged/Compare toggle, NotSelfBanner.
  - The `ScopeHash` newtype in `wolluf-core`.
  - An ADR 0005 amendment and an architecture §5.6 table update for the signal changes below.
- Out (non-goals):
  - osu! API `/me` linking (F3, O4). The linked-account signal is implemented as pure code, but its input is always `None` in F0.
  - Skill/session fold per scope (F3). `skill_compare` and head-to-head views (F3).
  - Scope GC of fold rows (F3, when fold rows exist).
  - Play exclusions (`exclude_play` feedback, F3). The exclusion policy id is part of the scope from day one.
  - Using `.osg` presence as a signal (after ADR 0012 from 006).
  - Profile rename and delete commands (not in the §8 command table; add them later through a spec amendment).
  - Parsing the DBs and syncing the ledger (002/003).

## Behaviour
1. After the first `SyncPlays` finishes, `RefreshIdentity` runs:
   - it recomputes `alias_stats`;
   - it creates the singleton `self` profile ("Me", default, merged) if none exists;
   - it reconciles that profile's `origin=auto` rows to the current auto tier.
   `players_list_aliases` then returns every raw name with stats, tier, score, reasons and decision, plus `wizardNeeded = true`.
2. The wizard lists aliases grouped by tier: **You** (auto, ticked), **Probably you** (highlighted, **unticked**, 3 sample plays each), **Other names** (unticked), **Probably other players** (unticked, muted). Each row shows play count, per-keymode counts (7K first), date range, online/offline split, replays available, and top charts. A reasons popover lists every fired signal with its sign and points.
3. "Select all" is a tri-state checkbox over every row. If it ticks any "Probably other players" row, an inline warning appears ("These look like other people's replays"); the user can still confirm.
4. **Confirm** sends one `players_decide_alias` batch:
   - every ticked row → `me`;
   - every row that was auto and got unticked → `not_me`;
   - other unticked rows get no decision and stay proposals.
   It also sets `completesWizard=true`. `wizardNeeded` becomes false, and one identity `feedback_event` is written per decision.
5. A decision always wins:
   - `me` puts the alias in the self profile with `origin=user`;
   - `not_me` removes it from the self profile and excludes it from the anchor set;
   - a decision of `null` clears the row, and the alias falls back to heuristics.
   A later `RefreshIdentity` (new plays, changed cfg) never adds or removes an alias that has a decision. It only reconciles undecided `origin=auto` rows.
6. Settings → Identity reopens the same table with current decisions pre-applied (decided rows show a "confirmed" / "not you" chip).
7. Other profiles:
   - `players_create_profile` makes a `kind=other` profile from ≥ 1 alias;
   - `players_set_profile_aliases` edits the aliases and optionally `mergeMode`. On the self profile this is sugar: every alias added becomes `me` and every alias removed becomes `not_me`.
   - An alias cannot be both in the self set (decided `me` or auto) and in an `other` profile. Violations return `CONFLICT` with the offending profile ids.
8. `players_list_profiles(keymode)` returns the persisted profiles plus the virtual **All players** entry (`kind: "all_players"`, label key "mixed, not a person"). Each entry carries its resolved scopes for that keymode:
   - `merged` → one scope over all its aliases;
   - `separate` → one scope per alias, ordered by alias raw name bytes.
   The Merged/Compare toggle is shown only when the selected entry has > 1 alias. A URL `merge` param overrides the persisted mode for the view (and is the only mode source for All players).
9. `players_set_default(profileId)` makes that persisted profile the only default. All players cannot be default. The ScopePicker falls back to the default when the URL has no scope.
10. Whenever the active entry is not `kind=self`, the app shell shows the NotSelfBanner. For All players it reads "All players: mixed, not a person"; otherwise "Viewing {label}: not your profile. Nothing here affects your skill."
11. Edge cases:
    - no plays yet → the wizard shows an empty state linking to sync, and `wizardNeeded=false`;
    - cfg missing or unreadable → the cfg signal does not fire and `cfgUsernameAvailable=false` is shown as a hint;
    - an alias that normalizes to empty (`""`, `"||"`) never matches any name signal;
    - plays whose chart is not in the catalog count under keymode bucket `unknown`, and non-mania plays count under `non_mania` (the pilot `""` has 19 std plays);
    - a self profile with zero aliases has no scopes, and the UI says "No names selected";
    - duplicate alias ids in any input → `INVALID_INPUT`.

## Domain rules
- R1 scores.db mixes own plays under several names with other players' replays. Pilot snapshot (research 03 / 00-plan): 2,635 7K plays as `TWulfZ`, 1,630 under offline names (`""`, `W`, `w`, `Wulf`, `s`). `Madeline`, `Klinsx` and `StevenS` stay out unless the user opts them in. The cfg `Username` is `TWulfZasdasdasd d jSS||`.
- R2 Re-measured on 2026-09-28 (read-only probe with `osudb.py`; scores.db has grown to 4,989 scores). Counts drift, so tests assert tiers, never counts.

  | raw name | plays | online ids | share of plays within 30 min of a `TWulfZ` play | plays on charts `TWulfZ` never played |
  |---|---|---|---|---|
  | `TWulfZ` | 3,019 | 270 | n/a | n/a |
  | `""` | 1,395 (19 std) | 0 | 3% | 29% |
  | `W` | 344 | 0 | 2% | 34% |
  | `Madeline` | 68 | 0 | 10% | 32% |
  | `s` | 62 | 0 | 13% | 19% |
  | `w` | 33 | 0 | 0% | 48% |
  | `Klinsx` | 30 | 0 | 20% | 73% |
  | `TWulfZasdasdasd d jSS\|\|` | 27 | 0 | 11% | 15% |
  | `Wulf` | 10 | 0 | 0% | 10% |
  | `StevenS` | 1 | 1 | 0% | 0% |

  Every alias has its replay in `Data/r`. Only `TWulfZ` and `StevenS` have online score ids.
- R3 **Finding that contradicts §5.6:** on pilot data, temporal interleaving is *not* the key signal for offline aliases. Offline names were used during periods when `TWulfZ` was not logged in, so `""`/`W` interleave less (2–3%) than `Madeline`/`Klinsx` (10–20%). Session co-membership with a 120 min gap gives the same ordering. Consequences:
  - Interleaving stays in the model with its §5.6 strength but a threshold (≥ 25% share) that no pilot alias reaches.
  - The positive evidence for offline aliases comes from a new **local-offline** signal: ≥ 95% of plays have no online id and do have a replay.
  - Negatives cover the rest: a new **distinct-nickname** signal, plus "charts never otherwise played" (§5.6).
  - This amends §5.6, so it needs ADR 0005 (T1).
- R4 Normalization (§5.6) = NFKC → full Unicode default case fold → NFKC → keep only `char::is_alphanumeric`. `TWulfZasdasdasd d jSS||` → `twulfzasdasdasddjss`.
- R5 The cfg match (§5.6) fires only when the normalized alias length is ≥ 4 chars (Unicode scalar count) and one of these holds:
  - the names are equal;
  - the normalized cfg name starts with the alias;
  - Jaro-Winkler(alias, cfg prefix of the same char length) ≥ 0.92.
  `""`, `W`, `w`, `s` can never match. `wulf` does not match `twulfz…`, because the prefix is `twul` and JW ≈ 0.83.
- R6 Tiers (§5.6):
  - Auto requires score ≥ 0.9, at least one strong signal and normalized length ≥ 4;
  - "Probably you" is 0.5–0.9, shown unticked;
  - the rest are listed, and flagged "probably another player" when score < 0.2 and at least one negative fired.
- R7 `identity_decision` is the user's answer, and heuristics only propose for undecided aliases (§5.3, §5.6). Identity decisions are also captured as `feedback_event` (§6.1 "On identity").
- R8 `scope = (alias set, keymode, exclusion policy)` and `scope_hash = blake3(canonical form)` (§5.6). The alias key is the raw name bytes per game (`alias` table comment in §5.3: raw bytes are the key, normalization is only for matching, `''` is a valid alias). Local autoincrement ids never enter the hash, so a rebuilt user.db gives identical hashes.
- R9 `.osg` presence correlates with local play (`StevenS` 0/1 and `Klinsx` 4/30, versus ≥ 97% for local names). However, `TWulfZ` has 245 post-2026-04-30 plays without one and `W` has `.osg` files from before that date, so its meaning is unknown until 006/ADR 0012. It is not a signal in this spec.
- R10 Other players' plays never feed a self profile or telemetry (G3, D10). The All-players entry is virtual and always shows the banner (§5.6, §8).
- R11 Thresholds and weights live in `IdentityParams` (D17). F0 has no param pack yet (engine arrives in F1), so the defaults ship as `IdentityParams::default()`; F1 moves them into pack section `identity`.
- R12 Ids above 2^53 (play ids, online score ids) cross IPC as strings (§8). Alias and profile ids are small local integers and cross as numbers.
- R13 The cfg `Username` is read live at refresh time from the newest `osu!.<account>.cfg` of the install through 002's `cfg_files::{list_user_cfgs, read_user_cfg}`; it is never persisted (003 keeps cfg out of `source_snapshot`, and 002 never copies the `Password` value). The heuristic outcome is persisted through `profile_alias(origin=auto)`.

## Design
**Crates and edges.** No new internal edge:
- `core` gets `ScopeHash`;
- `store` gets the players repositories and `alias_stats`;
- `app` gets `features::players`;
- `desktop` gets `commands/players.rs`.
The edges used, app→store, app→source-osu (cfg codec only) and app→core, already exist in §4. New external deps (the orchestrator adds them to `[workspace.dependencies]`; all are allowed by deny.toml):

| crate | version | license | used by |
|---|---|---|---|
| unicode-normalization | 0.1.25 | MIT/Apache-2.0 | app (players::names) |
| caseless | 0.2.2 | MIT | app (players::names, full case fold; `str::to_lowercase` is not a case fold: `ß`) |
| strsim | 0.11.1 | MIT | app (players::names, `jaro_winkler`) |
| blake3 | 1.8.7 | CC0/Apache-2.0 | app (scope hash), already expected from 001 for `VersionKey` |
| specta / tauri-specta | 2.0.0-rc.25 (as pinned by 005) | MIT | app dto / desktop |

**Types (prose).**
- `core::ScopeHash([u8; 32])`: an opaque id with hex `Display`/`FromStr`. `wolluf-core` holds ids, and F3's store and engine key on it without depending on app.
- `players::names`:
  - `normalize(&str) -> String`;
  - `cfg_matches(alias_norm, cfg_norm, &IdentityParams) -> Option<CfgMatchKind {Equal, Prefix, JaroWinkler(milli)}>`;
  - `resembles(alias_norm, anchor_norm, &IdentityParams) -> bool`. It is true when the alias is non-empty and one of these holds:
    - the anchor contains the alias, or the alias contains the anchor;
    - the alias has ≥ 4 chars and JW on the prefix is ≥ 0.92.
- `players::stats`: `PlayFact {play_id, alias_id, t: TimeUs, chart_md5, keymode: KeymodeBucket, has_online_id, has_replay}`. `compute(&[AliasRef], &[PlayFact], &IdentityParams) -> Vec<AliasStats>` produces:
  - `n_plays`;
  - `by_keymode: Vec<(KeymodeBucket, u32)>` sorted, with buckets `k1`…`k16`, `unknown`, `non_mania`;
  - `first_t`, `last_t`, `n_online`, `n_with_replay`;
  - `top_charts` (top N by count; ties broken by md5 ascending);
  - `cooccurrence: Vec<(AliasId, u32)>`, the anchor-independent pairwise count of this alias's plays within `interleave_window` of any play of the other alias, sorted by id.
  Deterministic: `BTreeMap` only, no `HashMap` iteration (D3).
- `players::heuristics` is pure and IO-free, following D2 in spirit although app is not a domain crate. `IdentityInputs {aliases: Vec<AliasFacts{alias_id, raw_name, plays: Vec<PlayFact>}>, cfg_username: Option<String>, linked: Option<LinkedNames{username, previous}>, decisions: BTreeMap<AliasId, Decision>}` → `Vec<AliasAssessment {alias_id, norm_len, score_milli: u16 (0..=1000), tier: Tier, guard_capped: bool, reasons: Vec<Reason{signal: SignalId, points: i16, args: ReasonArgs}>, decision: Option<Decision>}>`. The algorithm runs in fixed passes:
  1. Normalize the names. Evaluate the anchor-free signals: `linked_account`, `cfg_username`, `top_online_name`, `local_offline`.
  2. `anchors = decided(me) ∪ {strong ∧ norm_len ≥ min_len ∧ ¬decided(not_me)}`. Anchor names for resemblance = anchor aliases' normalized names plus the normalized cfg and linked names.
  3. For non-anchor aliases, evaluate the anchor-relative signals: `interleave`, `online_never_interleave`, `charts_not_otherwise_played`, `before_first_self`, `name_resembles_self`, `distinct_nickname`. When there are no anchors, the anchor-relative signals are skipped, not counted as failed.
  4. `score = clamp(base + Σ points, 0, 1000)`, then tier per R6.
  Integer milli-points and integer cross-multiplied ratios keep the result bit-identical across machines. JW (f64) is only compared against a threshold. The output is sorted by (tier, n_plays desc, raw_name bytes) and does not depend on input order.
- `SignalId` is a stable string enum, persisted in reasons and never renumbered: `linked_account`, `cfg_username`, `top_online_name`, `interleave`, `local_offline`, `name_resembles_self`, `online_never_interleave`, `charts_not_otherwise_played`, `before_first_self`, `distinct_nickname`. `Tier` = `auto | probable | neutral | likely_other`. `Decision` = `me | not_me`.
- `IdentityParams` defaults (milli-points unless noted):

  | field | default | notes |
  |---|---|---|
  | `base` | 250 | |
  | `linked_account` | +650 | strong |
  | `cfg_username` | +650 | strong; alias norm_len ≥ `min_norm_len` |
  | `top_online_name` | +250 | medium; the most frequent raw name among plays with an online id, ≥ `top_online_min_plays`=5 and ≥ 500‰ of online plays |
  | `interleave` | +250 | medium; share of the alias's plays within `interleave_window`=30 min of an anchor play ≥ 250‰ |
  | `local_offline` | +300 | medium (new, R3); share of plays with no online id and with a replay ≥ 950‰ |
  | `name_resembles_self` | +150 | weak (new) |
  | `online_never_interleave` | −300 | online-id share ≥ 500‰, interleave share = 0, not `top_online_name` |
  | `charts_not_otherwise_played` | −250 | share of plays on charts no anchor played ≥ 600‰ |
  | `before_first_self` | −250 | share of plays before the first anchor play ≥ 500‰ |
  | `distinct_nickname` | −200 | weak (new); norm_len ≥ 4 and the name resembles no anchor name |
  | `min_norm_len` | 4 | |
  | `jw_min` | 0.92 | |
  | `auto_min` / `probable_min` / `likely_other_max` | 900 / 500 / 200 | |
  | `top_charts_n` / `sample_plays_n` | 5 / 3 | |

  Expected pilot outcome, which is the basis of AC3/AC15:

  | alias | points | score | tier |
  |---|---|---|---|
  | `TWulfZ` | 250 + 650 cfg + 250 top_online | 1000 (clamped) | auto |
  | `TWulfZasdasdasd…` | 250 + 650 + 300 | 1000 | auto |
  | `""` | 250 + 300 | 550 | probable |
  | `W`, `w`, `Wulf`, `s` | 250 + 300 + 150 | 700 | probable |
  | `Madeline` | 250 + 300 − 200 | 350 | neutral |
  | `Klinsx` | 250 + 300 − 200 − 250 | 100 | likely_other |
  | `StevenS` | 250 − 300 − 200 | 0 | likely_other |

- `players::scope`:
  - `ExclusionPolicy` is a stable string id; F0 has only `standard@1`.
  - `Scope {game, alias_raw_names: BTreeSet<Vec<u8>>, keymode: Keymode, exclusion: ExclusionPolicy}`.
  - `canonical_bytes(&Scope)` is: `b"wolluf-scope/1\0"`, then `game`, `\0`, then keymode as ASCII decimal, `\0`, then the policy id, `\0`, then the alias count (u32 LE), then for each alias in byte order its length (u32 LE) followed by its bytes.
  - `scope_hash = ScopeHash(blake3(canonical_bytes))`.
  - `resolve(entry, keymode, merge) -> Vec<ResolvedScope{hash, alias_ids, label_args}>`.
- `PlayersService` (`pub`, the only entry point for other features, D12):
  - `list_aliases()`;
  - `wizard_needed() -> bool` (cheap; 005's `setup_status.identityReady` is its negation);
  - `list_profiles(keymode)`;
  - `decide(batch, completes_wizard)`: one user.db transaction that writes the decisions, the feedback events and the self-profile reconcile;
  - `set_profile_aliases`, `create_profile`, `set_default`;
  - `resolve_scopes(entry, keymode, merge)`;
  - `refresh()`: stats, self-profile bootstrap and auto reconcile. It is chained after every `SyncPlays` through 003's follow-up mechanism as the `RefreshIdentity` job (dedupe key `players.refresh`) and emits `DataChanged{domains:["players"]}`.
  - The cfg username is read per R13 from the install root stored in `game_install`. Missing or unreadable → `None` and `cfgUsernameAvailable=false`.
- `players::stats` is a versioned stage: `ALIAS_STATS_VERSION = 1`. `vkey = VersionKey(stage "players.alias_stats", VERSION, hash(IdentityParams stats fields), input fingerprint = blake3(sorted play ids ‖ latest osu_db snapshot sha))`. `players::heuristics` carries `HEURISTICS_VERSION = 1`, which is returned in the DTO. Assessments are recomputed per call and never persisted, except as `profile_alias(origin=auto)` membership.

## Data
- user.db migration: **no.** The feature uses `alias`, `play`, `identity_decision`, `profile`, `profile_alias`, `feedback_event` and `settings` from 003's `0001_init`, which also creates both indexes this feature relies on:
  - at most one `kind=self` profile (partial unique index `ON profile(kind) WHERE kind='self'`);
  - at most one `is_default=1` (partial unique index `ON profile(is_default) WHERE is_default=1`); the service keeps exactly one by switching it in one transaction.
  New `settings` key `identity.wizard_completed_at` (a UX flag only; it changes no derived number).
- `feedback_event` rows: `kind='identity_decision'`, `subject_json={"alias":{"game":"osu_stable","raw_name_b64":…}}`, `payload_json={"decision":"me"|"not_me"|null,"via":"wizard"|"settings"|"profile_edit"}`, `context_json={app_version, manifest_hash:null, pack_id:null, scope_hash:<self merged k7 scope after the change>}`, `telemetry_state='local_only'`.
- cache.db change: yes. `alias_stats` becomes `alias_stats(alias_id, vkey, n_plays, n_by_keymode_json, first_ts, last_ts, n_with_replay, n_online_ids, top_charts_json, cooccurrence_json, PK(alias_id, vkey))`. This adds `vkey` (every derived table carries one, §5.4) and `top_charts_json` (§5.6 "top charts"). T6 adds the table to 003's cache `schema_v1.sql`; v1 is unreleased until F0 closes, so `CACHE_SCHEMA_VERSION` stays 1.
- Vault/blob changes: none.

## IPC / UI
All DTOs live in `app::features::players::dto`, derive `serde` (camelCase) + `specta::Type`, and map from domain types (D13). `aliasId` and `profileId` are `u32` in DTOs (checked conversion from core's `i64` row ids, overflow → `INTERNAL`), because 005's bindings export fails on 64-bit integers. Commands live in `apps/desktop/src-tauri/src/commands/players.rs` and are ≤ 10 lines each (D11).

| command | input | output |
|---|---|---|
| `players_list_aliases` | none | `AliasListDto {heuristicsVersion, cfgUsernameAvailable, wizardNeeded, aliases: AliasRowDto[]}` |
| `players_list_profiles` | `keymode: number` | `ProfileEntryDto[]` (persisted + virtual All players) |
| `players_set_profile_aliases` | `SetProfileAliasesInput {profileId, aliasIds, mergeMode?}` | `ProfileEntryDto` |
| `players_decide_alias` | `DecideAliasInput {decisions: {aliasId, decision: "me"\|"not_me"\|null}[], completesWizard}` | `AliasListDto` |
| `players_create_profile` | `CreateProfileInput {label (1–64 chars after trim), aliasIds (≥1), mergeMode?}` | `ProfileEntryDto` (kind other) |
| `players_set_default` | `profileId: number` | `null` |

- `AliasRowDto`:
  - `{aliasId, rawName, isEmptyName, normalizedLength, nPlays, byKeymode: {bucket, n}[], firstPlayedAt, lastPlayedAt (RFC 3339 UTC), nOnline, nOffline, nWithReplay, topCharts: {chartMd5, title?, version?, n}[]}`;
  - `{tier, score (0–1000), guardCapped, reasons: {signal, polarity: "positive"|"negative", strength: "strong"|"medium"|"weak", points, args}[]}`;
  - `{decision: "me"|"not_me"|null, inSelfProfile, sampledPlays: {playId (string), chartMd5, title?, version?, playedAt, nativeAcc, hasReplay}[]}` (filled for `probable` only);
  - `{cooccurrence: {aliasId, n}[]}`.
- `ProfileEntryDto`: `{ref: {kind:"profile", id} | {kind:"all_players"}, profileKind: "self"|"other"|"all_players", label, isDefault, mergeMode, aliasIds, scopes: {scopeHash (hex), aliasIds, keymode}[]}`.
- Errors (`AppError` codes from §7), with `message_key` `players.error.*`:
  - `NOT_FOUND`: unknown alias or profile;
  - `INVALID_INPUT`: bad label, empty alias set for `other`, duplicate ids, All players as default;
  - `CONFLICT`: self/other overlap, a second self profile.
- Events: `DataChanged{domains:["players"]}` after refresh, decide and profile edits.
- UI slice `ui/src/features/players/`:
  - `queries.ts`: keys `['players','aliases']`, `['players','profiles',keymode]`;
  - `components/`: `IdentityWizard`, `AliasTable`, `AliasRow`, `TierBadge`, `ReasonList`, `SamplePlays`, `SelectAllCheckbox`, `ScopePicker`, `MergeCompareToggle`, `NotSelfBanner`;
  - `index.ts`: exports the wizard, ScopePicker, toggle, banner, and `validateGlobalSearch` (the `scope|keymode|merge` validator 005's root route delegates to).
- Routes: `routes/setup.identity.tsx` (the wizard) and `routes/settings.identity.tsx` (`AliasTable` in edit mode). The typed search params are `scope` (`self` | `p:<id>` | `all`), `keymode` and `merge`. 005's shell mounts ScopePicker, toggle and banner in the header, and 005's setup flow navigates to `/setup/identity` when `wizardNeeded`.
- Wizard states:
  - `loading` (skeleton rows);
  - `empty` (no aliases; CTA to sync);
  - `error` (per `IpcError.code`);
  - `syncing` (list refreshes on `DataChanged`, and the Confirm button is disabled while a `RefreshIdentity` job runs);
  - `ready`;
  - `submitting`;
  - `done` (navigate to the default profile).
- Display rules:
  - `""` renders as the i18n "(no name)" with the raw `""` in monospace;
  - reasons render only from DTO args (the UI computes nothing, §8);
  - all strings are in es/en i18n.

## Acceptance criteria
- [ ] AC1: Normalization matches R4 on the table (`TWulfZasdasdasd d jSS||`→`twulfzasdasdasddjss`, fullwidth `ＴＷｕｌｆＺ`→`twulfz`, `Straße`→`strasse`, `x_Wulf-`→`xwulf`, `||`→``, `""`→``) → `cargo nextest run -p wolluf-app -E 'test(players::names::tests::normalize_table)'`.
- [ ] AC2: The cfg match follows R5:
  - `twulfz` matches the garbage cfg (prefix), and the garbage alias matches it (equal);
  - `twulfs` matches (JW);
  - `""`, `w`, `s` and `wulf` do not.
  → `test(players::names::tests::cfg_match_table)`.
- [ ] AC3: On the synthetic pilot-shaped fixture (third-party names anonymized to `Rosalind`/`Kovacs`/`Sterling`, with the R2 shares preserved), the tiers are exactly the Design pilot table → `test(players::heuristics::tests::pilot_like_tiers)`.
- [ ] AC4: No alias with normalized length < 4 is ever `auto`, whatever its signals (proptest over random facts + forced strong signal) → `test(players::heuristics::tests::short_alias_never_auto)`.
- [ ] AC5: The output is independent of input order (proptest shuffling aliases and plays; the result is byte-equal) → `test(players::heuristics::tests::order_independent)`.
- [ ] AC6: A `not_me` decision on a strong alias removes it from anchors, and anchor-relative signals of the other aliases change accordingly. A `me` decision on `""` makes it an anchor → `test(players::heuristics::tests::decisions_shape_anchors)`.
- [ ] AC7: `alias_stats` aggregation is correct: keymode buckets incl. `unknown`/`non_mania`, date range, online/offline, replay count, top-chart tie-break and pairwise co-occurrence → `test(players::stats::tests::aggregates_table)`.
- [ ] AC8: The scope canonical bytes and hash match a committed golden. The hash is invariant to alias order and changes when any alias, the keymode or the policy changes. Alias ids do not enter the hash → `test(players::scope::tests::)` (`canonical_form_golden`, `hash_invariants`).
- [ ] AC9: `resolve` gives merged → 1 scope; separate over 3 aliases → 3 scopes in byte order; All players → the union of all aliases; a self profile with zero aliases → 0 scopes → `test(players::scope::tests::resolve_table)`.
- [ ] AC10: Store repositories on in-memory SQLite with real migrations cover:
  - decision upsert and clear;
  - self singleton and default uniqueness enforced by index;
  - `profile_alias` origin preserved;
  - the identity `feedback_event` written in the same transaction;
  - `alias_stats` read requires the vkey.
  → `cargo nextest run -p wolluf-store -E 'test(players::)'`.
- [ ] AC11: The service, seeded with the pilot-shaped fixture, behaves as follows:
  - first refresh creates the self profile with exactly the auto aliases (`origin=auto`) as default;
  - `decide(not_me)` on an auto alias removes it, and it stays removed after another `refresh()` with new plays;
  - `decide(me)` on `""` adds it with `origin=user`;
  - clearing a decision restores the heuristic proposal;
  - changing the self alias set changes the self `scope_hash`.
  → `cargo nextest run -p wolluf-app -E 'test(players::service::tests::)'`.
- [ ] AC12: Profile invariants: creating a second self profile → `CONFLICT`; an `other` profile containing a self alias → `CONFLICT`, and so does the reverse via `decide(me)`; an empty label or empty aliases → `INVALID_INPUT`; `set_default` leaves exactly one default; All players is never persisted → `test(players::service::tests::profile_invariants)`.
- [ ] AC13: The six commands are registered, and `bindings.ts` is regenerated and clean → `cargo xtask bindings && git diff --exit-code apps/desktop/ui/src/ipc/bindings.ts`. The DTO snapshot (insta over `specta` export of the players types) shows `playId: string` → `test(players::dto::tests::dto_shape)`.
- [ ] AC14: UI (vitest + typed `mockIPC` fixture mirroring AC3):
  - the wizard groups rows by tier; auto rows are ticked and probable rows unticked and highlighted with 3 sample plays;
  - Select all ticks every row and shows the other-players warning;
  - Confirm sends the exact Behaviour-4 batch (ticked → `me`, unticked-auto → `not_me`, others omitted) with `completesWizard: true`;
  - the banner shows for `other` and `all_players` and is absent for self;
  - the toggle is hidden for single-alias entries.
  → `pnpm -C apps/desktop/ui test -- features/players`.
- [ ] AC15: F0 exit on the real corpus (read-only, through 003's in-memory snapshots):
  - `TWulfZ` is `auto`;
  - `""`, `W` and `Wulf` are `probable`;
  - `Madeline`, `Klinsx` and `StevenS` are neither `auto` nor `probable`.
  → `WOLLUF_CORPUS="/mnt/e/Games/osu!" cargo nextest run -p wolluf-app --run-ignored only -E 'test(players_corpus_tiers)'`.
- [ ] AC16: The layer rules hold (no `rusqlite` in app, no new internal edge) → `cargo xtask check-layers`. `cargo clippy --workspace --all-targets -- -D warnings` is clean.
- [ ] AC17: ADR 0005 records the R3 amendment with the R2 evidence, and architecture §5.6 lists the new signals and thresholds → `grep -q local_offline docs/architecture.md docs/adr/0005-*.md`.

## Risks / open questions
- **Product decision (needs user confirmation before T4 merges):** the `distinct_nickname` penalty treats a full, unrelated nickname typed locally (`Madeline`) as "probably someone else". Behaviourally, `Madeline` is indistinguishable from the user's offline names (local, replay present, same days; R2). Without the penalty it would land in "Probably you" (550). The default here keeps it neutral, matching research 03 ("keep Madeline, Klinsx and StevenS out unless the user says otherwise"). The user may know `Madeline` is them; the decision UI covers that either way.
- The weak `name_resembles_self` signal lets any single letter found in the garbage cfg (`s` ⊂ `twulfzasdasdasddjss`) earn +150. That is acceptable because it can never reach auto (R6 guard), but it is noise. Revisit it with more users.
- `w` sits at 48% charts-not-otherwise-played against a 60% threshold. Small aliases are near thresholds; AC15 will catch drift as the corpus grows.
- Cross-spec assumptions, settled at the F0 review: 003 maps `online_id ≤ 0` to NULL; the cfg username is read live (R13), not stored by 003; 003's `0001_init` creates the identity/profile tables and both `profile` partial indexes; 003's `JobRunner` enqueues follow-ups from `JobSummary`; 001 T20 creates ADR 0005 and T1 here amends it; `AppError` is 005 T3's.
- `caseless` has not been updated since 2024-12. If it lags a Unicode version, ICU4X `icu_casemap` is the fallback. Swap it only if a test fails.
- `alias_stats` is persisted as §5.4 specifies, although recomputing it takes milliseconds at 5k plays. Keeping the table costs one vkey read and avoids diverging from the architecture.
- The heuristics reason over all keymodes together (identity is per person); counts per keymode are informational only.

## Deviations (filled at close)
