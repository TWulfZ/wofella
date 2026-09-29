# 004 Players and identity

Status: Draft
Phase: F0 · Owner: TWulfZ · Date: 2026-09-28
Links: architecture §3, §4 (D2, D6, D11–D14, D17), §5.2–§5.6, §6.1, §7, §8, §10, §12 (F0) · ADR 0005 (identity scopes and auto-selection; amended by T1 to the session-user rule) · research `03-maniahub-rejudge-drills-sessions-audit.txt` (scores.db findings, lines ~307, 343, 347), `00-plan-es.md` (§ "Selección de jugadores", item 10) · oracle `research/scripts/audit/osudb.py`, `plays.py`
Depends on: 001 workspace-foundation (crates, `VersionKey`, lints, check-layers), 002 osu-stable-codecs (scores.db + cfg codecs, `cfg_files`), 003 store-ledger-sync (user.db baseline with the identity/profile tables, `play`/`alias` ledger, cache.db skeleton, `SyncPlays` job and follow-up chaining), 005 T3 (`AppError`), 005 desktop-shell-cli (Tauri shell, specta export, UI skeleton, setup flow). 006 osg-spike is **not** a dependency (see Domain rules R9).

## Problem
scores.db mixes the user's own plays, recorded under several names (some typed while offline: `""`, `W`, `w`, `Wulf`, `s`, and even the garbage cfg string), with replays by other people. Skill must be computed only for the identity the user selects (G3), so F0 has to show every name with enough context for the user to pick. Offline names are often nothing like the user's alias, so wolluf does not try to guess which are theirs: it pre-selects only the current session user (the alias matching the osu! login of the newest cfg) and lists every other name unticked for the user to add. Decisions must persist, and the user can also build profiles for other players and view "All players" without that ever feeding the self profile. The result has to be a stable scope (`scope_hash`) that F1–F3 key their fold on.

## Scope
- In:
  - Pure `app::features::players::{names, stats, selection, scope}` modules: normalization, alias statistics, the session-user auto-selection rule, scope canonical form and hash.
  - `wolluf-store` repositories for `identity_decision`, `profile`, `profile_alias`, identity facts, identity `feedback_event` rows, and cache.db `alias_stats`.
  - `PlayersService` (the `pub` service of the feature): list aliases, decide, profiles, default, scope resolution, and the `RefreshIdentity` step chained after `SyncPlays`.
  - Six IPC commands with specta DTOs (`players_*`) and their Tauri command file.
  - UI slice `features/players`: the "Which of these are you?" wizard, Settings → Identity (same table), ScopePicker, Merged/Compare toggle, NotSelfBanner.
  - The `ScopeHash` newtype in `wolluf-core`.
  - An ADR 0005 amendment and architecture §5.6/§8 updates that replace the scored heuristics with the session-user rule (R3).
- Out (non-goals):
  - osu! API `/me` linking (F3, O4). The linked-account match is implemented as pure code, but its input is always `None` in F0.
  - Skill/session fold per scope (F3). `skill_compare` and head-to-head views (F3).
  - Scope GC of fold rows (F3, when fold rows exist).
  - Play exclusions (`exclude_play` feedback, F3). The exclusion policy id is part of the scope from day one.
  - Inferring which other aliases are the user: scored signals, tiers, "Probably you" suggestions, "probably another player" flags, `.osg` presence (removed by the user on 2026-09-28, R3).
  - Profile rename and delete commands (not in the §8 command table; add them later through a spec amendment).
  - Parsing the DBs and syncing the ledger (002/003).

## Behaviour
1. After the first `SyncPlays` finishes, `RefreshIdentity` runs:
   - it recomputes `alias_stats`;
   - it creates the singleton `self` profile ("Me", default, merged) if none exists;
   - it reconciles that profile's `origin=auto` rows to the current auto set (R6).
   `players_list_aliases` then returns every raw name with stats, auto match and decision, plus `wizardNeeded = true`.
2. The wizard lists the auto-selected aliases first (ticked, with the chip "matches your osu! login"), then every other alias **unticked**, sorted by play count descending (ties by raw name bytes), with no tier, score or suggestion. Each row shows play count, per-keymode counts (7K first), date range, online/offline split, replays available, and top charts. If no alias matches (R5), nothing is ticked and the wizard asks the user to tick the names that are theirs.
3. "Select all" is a tri-state checkbox over every row.
4. **Confirm** sends one `players_decide_alias` batch:
   - every ticked row → `me`;
   - every row that was auto and got unticked → `not_me`;
   - other unticked rows get no decision and stay unticked.
   It also sets `completesWizard=true`. `wizardNeeded` becomes false, and one identity `feedback_event` is written per decision.
5. A decision always wins:
   - `me` puts the alias in the self profile with `origin=user`;
   - `not_me` removes it from the self profile, and the auto rule never selects it again;
   - a decision of `null` clears the row, and the alias falls back to the auto rule (R6).
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
    - cfg missing or unreadable → no alias is auto-selected, the wizard asks, and `cfgUsernameAvailable=false` is shown as a hint;
    - several aliases match → all of them are auto-selected (pilot: `TWulfZ` by prefix and `TWulfZasdasdasd d jSS||` by equality);
    - an alias that normalizes to empty (`""`, `"||"`) never matches;
    - plays whose chart is not in the catalog count under keymode bucket `unknown`, and non-mania plays count under `non_mania` (the pilot `""` has 19 std plays);
    - a self profile with zero aliases has no scopes, and the UI says "No names selected";
    - duplicate alias ids in any input → `INVALID_INPUT`.

## Domain rules
- R1 scores.db mixes own plays under several names with other players' replays. Pilot snapshot (research 03 / 00-plan): 2,635 7K plays as `TWulfZ`, 1,630 under offline names (`""`, `W`, `w`, `Wulf`, `s`). `Madeline`, `Klinsx` and `StevenS` stay out unless the user opts them in. The cfg `Username` is `TWulfZasdasdasd d jSS||`.
- R2 Re-measured on 2026-09-28 (read-only probe with `osudb.py`; scores.db has grown to 4,989 scores). Counts drift, so tests assert selection, never counts.

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
- R3 **Amendment to §5.6 (user decision 2026-09-28):** offline players use names far from their alias, so wolluf does not try to validate them. The pilot data agrees: temporal interleaving ranks `""`/`W` (2–3%) below `Madeline`/`Klinsx` (10–20%) because offline names were used while `TWulfZ` was not logged in, and nothing in R2 separates the user's offline names from other people's local plays. Consequences:
  - The §5.6 scored signals, the tiers (including "Probably you") and the "probably another player" flag are removed.
  - wolluf auto-selects only the current session user (R5, R6). Every other alias is listed unticked with no suggestion, and the user adds any of them by hand.
  - This amends §5.6, so it needs ADR 0005 (T1).
- R4 Normalization (§5.6) = NFKC → full Unicode default case fold → NFKC → keep only `char::is_alphanumeric`. `TWulfZasdasdasd d jSS||` → `twulfzasdasdasddjss`.
- R5 Session-user match: an alias is the current session user only when its normalized length is ≥ 4 chars (Unicode scalar count) and, against the normalized `Username` of the newest cfg (R13), one of these holds:
  - the names are equal;
  - the normalized cfg name starts with the alias.
  F3 adds the same test against the linked osu! account username (input `None` in F0). There is no fuzzy match. `""`, `W`, `w`, `s` can never match; `wulf` does not match `twulfz…` (not a prefix); `twulfs` does not match either.
- R6 Auto set = every undecided alias that passes R5. If none passes, nothing is preselected. Every other alias is listed unticked, with no suggestion, sorted by play count descending.
- R7 `identity_decision` is the user's answer and always wins; the auto rule only proposes for undecided aliases (§5.3, §5.6). Identity decisions are also captured as `feedback_event` (§6.1 "On identity").
- R8 `scope = (alias set, keymode, exclusion policy)` and `scope_hash = blake3(canonical form)` (§5.6). The alias key is the raw name bytes per game (`alias` table comment in §5.3: raw bytes are the key, normalization is only for matching, `''` is a valid alias). Local autoincrement ids never enter the hash, so a rebuilt user.db gives identical hashes.
- R9 `.osg` presence correlates with local play (`StevenS` 0/1 and `Klinsx` 4/30, versus ≥ 97% for local names). However, `TWulfZ` has 245 post-2026-04-30 plays without one and `W` has `.osg` files from before that date, so its meaning is unknown until 006/ADR 0012. It is not used for selection (R3).
- R10 Other players' plays never feed a self profile or telemetry (G3, D10). The All-players entry is virtual and always shows the banner (§5.6, §8).
- R11 The parameters (`min_norm_len`, `top_charts_n`) live in `IdentityParams` (D17). F0 has no param pack yet (engine arrives in F1), so the defaults ship as `IdentityParams::default()`; F1 moves them into pack section `identity`.
- R12 Ids above 2^53 (play ids, online score ids) cross IPC as strings (§8). Alias and profile ids are small local integers and cross as numbers.
- R13 The cfg `Username` is read live at refresh time from the newest `osu!.<account>.cfg` of the install through 002's `cfg_files::{list_user_cfgs, read_user_cfg}`; it is never persisted (003 keeps cfg out of `source_snapshot`, and 002 never copies the `Password` value). The auto outcome is persisted through `profile_alias(origin=auto)`.

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
| blake3 | 1.8.7 | CC0/Apache-2.0 | app (scope hash), already expected from 001 for `VersionKey` |
| specta / tauri-specta | 2.0.0-rc.25 (as pinned by 005) | MIT | app dto / desktop |

**Types (prose).**
- `core::ScopeHash([u8; 32])`: an opaque id with hex `Display`/`FromStr`. `wolluf-core` holds ids, and F3's store and engine key on it without depending on app.
- `players::names`:
  - `normalize(&str) -> String`;
  - `session_match(alias_norm, login_norm, &IdentityParams) -> Option<SessionMatchKind {Equal, Prefix}>` (R5).
- `players::stats`: `PlayFact {play_id, alias_id, t: TimeUs, chart_md5, keymode: KeymodeBucket, has_online_id, has_replay}`. `compute(&[AliasRef], &[PlayFact], &IdentityParams) -> Vec<AliasStats>` produces:
  - `n_plays`;
  - `by_keymode: Vec<(KeymodeBucket, u32)>` sorted, with buckets `k1`…`k16`, `unknown`, `non_mania`;
  - `first_t`, `last_t`, `n_online`, `n_with_replay`;
  - `top_charts` (top N by count; ties broken by md5 ascending).
  Deterministic: `BTreeMap` only, no `HashMap` iteration (D3).
- `players::selection` is pure and IO-free, following D2 in spirit although app is not a domain crate. `SelectionInputs {aliases: Vec<AliasFacts{alias_id, raw_name, n_plays}>, cfg_username: Option<String>, linked_username: Option<String>, decisions: BTreeMap<AliasId, Decision>}` → `Vec<AliasSelection {alias_id, norm_len, auto_match: Option<AutoMatch{source: MatchSource, kind: SessionMatchKind}>, decision: Option<Decision>, selected: bool}>`:
  - `auto_match` is R5 against the cfg login, then the linked username;
  - `selected = decision == me ∨ (decision = none ∧ auto_match ≠ none)` (R6, R7);
  - the output is sorted by (auto match first, `n_plays` desc, raw_name bytes) and does not depend on input order.
- `MatchSource` = `cfg_username | linked_account`, `SessionMatchKind` = `equal | prefix`, `Decision` = `me | not_me`: stable strings, never renumbered.
- `IdentityParams` defaults:

  | field | default |
  |---|---|
  | `min_norm_len` | 4 |
  | `top_charts_n` | 5 |

  Expected pilot outcome, which is the basis of AC3/AC15:

  | alias | auto match | selected |
  |---|---|---|
  | `TWulfZ` | cfg prefix | yes |
  | `TWulfZasdasdasd d jSS\|\|` | cfg equal | yes |
  | `""`, `W`, `w`, `Wulf`, `s` | none | no |
  | `Madeline`, `Klinsx`, `StevenS` | none | no |

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
- `players::stats` is a versioned stage: `ALIAS_STATS_VERSION = 1`. `vkey = VersionKey(stage "players.alias_stats", VERSION, hash(IdentityParams stats fields), input fingerprint = blake3(sorted play ids ‖ latest osu_db snapshot sha))`. `players::selection` carries `SELECTION_VERSION = 1`, which is returned in the DTO. Selections are recomputed per call and never persisted, except as `profile_alias(origin=auto)` membership.

## Data
- user.db migration: **no.** The feature uses `alias`, `play`, `identity_decision`, `profile`, `profile_alias`, `feedback_event` and `settings` from 003's `0001_init`, which also creates both indexes this feature relies on:
  - at most one `kind=self` profile (partial unique index `ON profile(kind) WHERE kind='self'`);
  - at most one `is_default=1` (partial unique index `ON profile(is_default) WHERE is_default=1`); the service keeps exactly one by switching it in one transaction.
  New `settings` key `identity.wizard_completed_at` (a UX flag only; it changes no derived number).
- `feedback_event` rows: `kind='identity_decision'`, `subject_json={"alias":{"game":"osu_stable","raw_name_b64":…}}`, `payload_json={"decision":"me"|"not_me"|null,"via":"wizard"|"settings"|"profile_edit"}`, `context_json={app_version, manifest_hash:null, pack_id:null, scope_hash:<self merged k7 scope after the change>}`, `telemetry_state='local_only'`.
- cache.db change: yes. `alias_stats` becomes `alias_stats(alias_id, vkey, n_plays, n_by_keymode_json, first_ts, last_ts, n_with_replay, n_online_ids, top_charts_json, PK(alias_id, vkey))`. This adds `vkey` (every derived table carries one, §5.4) and `top_charts_json` (§5.6 "top charts"). T6 adds the table to 003's cache `schema_v1.sql`; v1 is unreleased until F0 closes, so `CACHE_SCHEMA_VERSION` stays 1.
- Vault/blob changes: none.

## IPC / UI
All DTOs live in `app::features::players::dto`, derive `serde` (camelCase) + `specta::Type`, and map from domain types (D13). `aliasId` and `profileId` are `u32` in DTOs (checked conversion from core's `i64` row ids, overflow → `INTERNAL`), because 005's bindings export fails on 64-bit integers. Commands live in `apps/desktop/src-tauri/src/commands/players.rs` and are ≤ 10 lines each (D11).

| command | input | output |
|---|---|---|
| `players_list_aliases` | none | `AliasListDto {selectionVersion, cfgUsernameAvailable, wizardNeeded, aliases: AliasRowDto[]}` |
| `players_list_profiles` | `keymode: number` | `ProfileEntryDto[]` (persisted + virtual All players) |
| `players_set_profile_aliases` | `SetProfileAliasesInput {profileId, aliasIds, mergeMode?}` | `ProfileEntryDto` |
| `players_decide_alias` | `DecideAliasInput {decisions: {aliasId, decision: "me"\|"not_me"\|null}[], completesWizard}` | `AliasListDto` |
| `players_create_profile` | `CreateProfileInput {label (1–64 chars after trim), aliasIds (≥1), mergeMode?}` | `ProfileEntryDto` (kind other) |
| `players_set_default` | `profileId: number` | `null` |

- `AliasRowDto`:
  - `{aliasId, rawName, isEmptyName, normalizedLength, nPlays, byKeymode: {bucket, n}[], firstPlayedAt, lastPlayedAt (RFC 3339 UTC), nOnline, nOffline, nWithReplay, topCharts: {chartMd5, title?, version?, n}[]}`;
  - `{autoMatch: {source: "cfg_username"|"linked_account", kind: "equal"|"prefix"} | null}`;
  - `{decision: "me"|"not_me"|null, selected, inSelfProfile}`.
  Rows arrive in the R6 order; the UI does not re-sort them.
- `ProfileEntryDto`: `{ref: {kind:"profile", id} | {kind:"all_players"}, profileKind: "self"|"other"|"all_players", label, isDefault, mergeMode, aliasIds, scopes: {scopeHash (hex), aliasIds, keymode}[]}`.
- Errors (`AppError` codes from §7), with `message_key` `players.error.*`:
  - `NOT_FOUND`: unknown alias or profile;
  - `INVALID_INPUT`: bad label, empty alias set for `other`, duplicate ids, All players as default;
  - `CONFLICT`: self/other overlap, a second self profile.
- Events: `DataChanged{domains:["players"]}` after refresh, decide and profile edits.
- UI slice `ui/src/features/players/`:
  - `queries.ts`: keys `['players','aliases']`, `['players','profiles',keymode]`;
  - `components/`: `IdentityWizard`, `AliasTable`, `AliasRow`, `AutoMatchChip`, `SelectAllCheckbox`, `ScopePicker`, `MergeCompareToggle`, `NotSelfBanner`;
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
  - the auto-match chip renders only from `autoMatch` (the UI computes nothing, §8);
  - all strings are in es/en i18n.

## Acceptance criteria
- [ ] AC1: Normalization matches R4 on the table (`TWulfZasdasdasd d jSS||`→`twulfzasdasdasddjss`, fullwidth `ＴＷｕｌｆＺ`→`twulfz`, `Straße`→`strasse`, `x_Wulf-`→`xwulf`, `||`→``, `""`→``) → `cargo nextest run -p wolluf-app -E 'test(players::names::tests::normalize_table)'`.
- [ ] AC2: The session-user match follows R5:
  - `twulfz` matches the garbage cfg (prefix), and the garbage alias matches it (equal);
  - `twulfs` does not (no fuzzy match);
  - `""`, `w`, `s` and `wulf` do not;
  - a 3-char alias equal to a 3-char login does not (length guard).
  → `test(players::names::tests::session_match_table)`.
- [ ] AC3: On the synthetic pilot-shaped fixture (third-party names anonymized to `Rosalind`/`Kovacs`/`Sterling`, with the R2 counts preserved), the selection is exactly the Design pilot table: `TWulfZ` (and the cfg-string alias) selected with an auto match; `""`, `W`, `w`, `Wulf`, `s`, `Rosalind`, `Kovacs` and `Sterling` unselected with `auto_match = None`, sorted by play count desc → `test(players::selection::tests::pilot_like_selection)`.
- [ ] AC4: No alias with normalized length < 4 is ever auto-selected, whatever the login (proptest over random names with the login forced to equal or extend the alias) → `test(players::selection::tests::short_alias_never_auto)`.
- [ ] AC5: The output is independent of input order (proptest shuffling aliases; the result is byte-equal) → `test(players::selection::tests::order_independent)`.
- [ ] AC6: With the cfg missing, or a login that matches no alias, nothing is selected. A `not_me` decision on the matching alias unselects it, and a `me` decision on `""` selects it; no other row changes → `test(players::selection::tests::{no_match_preselects_nothing, decisions_override_auto})`.
- [ ] AC7: `alias_stats` aggregation is correct: keymode buckets incl. `unknown`/`non_mania`, date range, online/offline, replay count and top-chart tie-break → `test(players::stats::tests::aggregates_table)`.
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
  - clearing a decision restores the auto rule's result (selected only if it matches the login);
  - changing the self alias set changes the self `scope_hash`.
  → `cargo nextest run -p wolluf-app -E 'test(players::service::tests::)'`.
- [ ] AC12: Profile invariants: creating a second self profile → `CONFLICT`; an `other` profile containing a self alias → `CONFLICT`, and so does the reverse via `decide(me)`; an empty label or empty aliases → `INVALID_INPUT`; `set_default` leaves exactly one default; All players is never persisted → `test(players::service::tests::profile_invariants)`.
- [ ] AC13: The six commands are registered, and `bindings.ts` is regenerated and clean → `cargo xtask bindings && git diff --exit-code apps/desktop/ui/src/ipc/bindings.ts`. The DTO snapshot (insta over `specta` export of the players types) shows the `autoMatch` shape and no `bigint` field → `test(players::dto::tests::dto_shape)`.
- [ ] AC14: UI (vitest + typed `mockIPC` fixture mirroring AC3):
  - auto rows are ticked and listed first with the auto-match chip; every other row is unticked, has no suggestion, and keeps the DTO's play-count order;
  - with no auto match, nothing is ticked and the "tick the names that are yours" prompt shows;
  - Select all ticks every row;
  - Confirm sends the exact Behaviour-4 batch (ticked → `me`, unticked-auto → `not_me`, others omitted) with `completesWizard: true`;
  - the banner shows for `other` and `all_players` and is absent for self;
  - the toggle is hidden for single-alias entries.
  → `pnpm -C apps/desktop/ui test -- features/players`.
- [ ] AC15: F0 exit on the real corpus (read-only, through 003's in-memory snapshots):
  - `TWulfZ` is selected with an auto match (cfg prefix);
  - `""`, `W`, `w`, `Wulf`, `s`, `Madeline`, `Klinsx` and `StevenS` are unselected with `autoMatch = null`;
  - the self profile holds exactly the auto-selected aliases.
  → `WOLLUF_CORPUS="/mnt/e/Games/osu!" cargo nextest run -p wolluf-app --run-ignored only -E 'test(players_corpus_selection)'`.
- [ ] AC16: The layer rules hold (no `rusqlite` in app, no new internal edge) → `cargo xtask check-layers`. `cargo clippy --workspace --all-targets -- -D warnings` is clean.
- [ ] AC17: ADR 0005 records the R3 amendment with the R2 evidence, and architecture §5.6 and §8 describe the session-user rule with no tiers → `grep -q 'session user' docs/architecture.md && grep -q 'session user' docs/adr/0005-*.md && ! grep -qiE 'probably you|tiers pre-applied|jaro-winkler' docs/architecture.md CLAUDE.md`.

## Risks / open questions
- **Resolved (user decision 2026-09-28):** no heuristic suggestions. Only the session user is auto-selected (R3, R5); the `distinct_nickname` question is moot. Cost: a user with many offline names ticks them by hand once, and decisions persist.
- The newest cfg's login may not be the user (shared PC, a friend logged in last). Then that friend's alias is auto-selected; the user unticks it once and `not_me` persists (R7).
- Cross-spec assumptions, settled at the F0 review: 003 maps `online_id ≤ 0` to NULL; the cfg username is read live (R13), not stored by 003; 003's `0001_init` creates the identity/profile tables and both `profile` partial indexes; 003's `JobRunner` enqueues follow-ups from `JobSummary`; 001 T20 creates ADR 0005 and T1 here amends it; `AppError` is 005 T3's.
- `caseless` has not been updated since 2024-12. If it lags a Unicode version, ICU4X `icu_casemap` is the fallback. Swap it only if a test fails.
- `alias_stats` is persisted as §5.4 specifies, although recomputing it takes milliseconds at 5k plays. Keeping the table costs one vkey read and avoids diverging from the architecture.
- Selection is per person, over all keymodes together; counts per keymode are informational only.

## Deviations (filled at close)
