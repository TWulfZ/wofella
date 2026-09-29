# 0005 Identity scopes and auto-selection

- Status: Accepted, amended 2026-09-28 (see Amendment)
- Date: 2026-09-28

## Context
osu! stable's scores.db mixes the user's own plays with downloaded replays of other players, and it has no notion of "me". On the pilot, scores.db (version 20260924) holds 4,973 scores. 2,635 are under `TWulfZ` and 1,630 more are under offline names that are probably the user's (`""`, `W`, `w`, `Wulf`, …), plus a few names that are probably other players (research `03-maniahub-rejudge-drills-sessions-audit.txt` l.307, l.343, l.347). The `""` name alone covers 1,235 7K plays. The cfg `Username` cannot be trusted verbatim: on the pilot it is `TWulfZasdasdasd d jSS||` (CLAUDE.md).

Skill must be computed only for the identity set the user selects (G3). Other players' plays must stay viewable, but they must never contaminate a self profile or telemetry (D10). Mixing someone else's 1,000 plays into the user's θ would make every downstream number wrong without any visible error.

The source proposals disagreed between auto-selecting the top-1 candidate and using confidence tiers (Appendix A).

## Decision
As written in architecture §5.6:
- **Aliases are keyed by raw name bytes** per game: `alias(game, raw_name)`. Normalization is used for matching only, and `""` is a valid alias (§5.3).
- **Listing.** `alias_stats` (cache.db) gives every raw name with its play count, count per keymode, date range, online/offline split, replay availability and top charts.
- **Scoring** (superseded by the Amendment) is a transparent additive model in `app::features::players::heuristics`. It is pure and table-tested, and every signal becomes a visible reason:
  - exact match to a linked osu! account's `username` or `previous_usernames` (strong +, optional, `/me` only);
  - a cfg `Username` match after NFKC + casefold + stripping non-alphanumerics: equal, cfg name starts with the alias, or Jaro-Winkler ≥ 0.92 on the prefix, **only for aliases whose normalized length is ≥ 4** (strong +);
  - most frequent name among plays with an online score id (medium +);
  - temporal interleaving with confirmed-self plays within 30 min (medium +, the key signal for offline aliases);
  - online scores that never interleave, charts never otherwise played, and dates before the first self play (−, the downloaded-replay signature).
- **Tiers** (superseded by the Amendment). Auto-included (≥ 0.9) needs at least one strong positive signal, and aliases shorter than 4 normalized characters are never auto-included. "Probably you" (0.5–0.9) is highlighted but unchecked. Everything else is listed normally or flagged as probably another player.
- **Decisions persist and always win.** `identity_decision(alias_id, me | not_me)` stores the user's answer, and heuristics only propose for aliases without one (after the Amendment: the session-user rule only proposes).
- **Selection.** Multi-select, "Select all", `other` profiles for comparison, and a Merged / Compare separately toggle when more than one alias or player is selected. "All players" is a virtual profile labelled "mixed, not a person", and a banner shows whenever the view is not a self profile.
- **Scope.** `scope = (alias set, keymode, exclusion policy)`, `scope_hash = blake3(canonical form)`. Evidence is scope-independent. Only the fold (skill, offsets, sessions, reports) is keyed by `scope_hash`, so switching scopes is a refold that takes seconds and never re-judges. Fold rows for scopes unused for 30 days are deleted at startup.
- **Isolation.** Other players' plays are fully viewable, but they never feed a self profile or telemetry. Telemetry builders require a `SelfScope`, which can only be obtained from a `kind=self` profile (D10, ADR 0007).

## Alternatives considered
- **Auto-select only the top-1 candidate.** Rejected: it would silently drop the user's offline aliases, which hold about a third of the pilot's plays. It also breaks for users whose online name changed.
- **Treat every alias as the user.** Rejected: downloaded replays of other players would enter θ and telemetry, which violates G3 and D10.
- **Prefix matching with no length guard.** Rejected: `""` is a prefix of every name and `W` of many, so they would always be auto-included. The ≥ 4 guard and the strong-signal requirement fix that bug from the source proposal.
- **Normalized names as the key.** Rejected: distinct raw names would merge irreversibly. Raw bytes keep every alias distinct, and normalization stays a matching aid.

## Consequences
- The first-run wizard "Which of these are you?" needs `alias_stats` and the heuristics to exist in F0 (spec 004).
- Changing the alias selection changes `scope_hash` and triggers a background refold, never a re-judge.
- Identity table tests are an F0 exit criterion (§12).
- Spec 004 T1 amends this ADR once the user's 2026-09-28 decision on session-user auto-selection is recorded. The raw-byte key, persisted decisions, scopes and isolation stay as written here.

## Amendment 2026-09-28: auto-select only the session user
Recorded by spec 004 (R2, R3, R5, R6) from the user's decision of 2026-09-28. It replaces **Scoring** and **Tiers** above; the raw-byte key, listing, persisted decisions, selection, scopes and isolation stay as written.

### Context
Offline players type names far from their alias, so wolluf cannot validate them. The pilot data agrees. Re-measured on 2026-09-28 with a read-only `osudb.py` probe (scores.db has grown to 4,989 scores; counts drift, so tests assert selection, never counts):

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

Temporal interleaving, the signal the original decision called "the key signal for offline aliases", ranks the user's `""` and `W` (2–3%) *below* `Madeline` and `Klinsx` (10–20%): offline names were used exactly when `TWulfZ` was not logged in. No other column separates the user's offline names from other people's local plays either. Every alias has its replay in `Data/r`, and only `TWulfZ` and `StevenS` have online score ids. `.osg` presence was also considered and rejected: its meaning is unknown until ADR 0012 (spec 004 R9).

### Decision
- The scored signals, the tiers (including "Probably you") and the "probably another player" flag are **removed**.
- **Normalization**: NFKC → full Unicode default case fold → NFKC → keep only `char::is_alphanumeric`. `TWulfZasdasdasd d jSS||` → `twulfzasdasdasddjss`.
- **Session-user match**: an alias is the current session user only when its normalized length is ≥ 4 Unicode scalars and the normalized `Username` of the newest `osu!.<account>.cfg` either **equals** it or **starts with** it. There is no fuzzy match (no Jaro-Winkler). F3 adds the same test against the linked osu! account username; in F0 that input is always `None`.
- **Auto set** = every undecided alias that matches. If none matches, nothing is preselected and the wizard asks the user to tick their names.
- **Every other alias** is listed unticked, with no suggestion, sorted by play count descending (ties by raw name bytes).
- **Decisions persist and always win**: `me` adds the alias to the self profile, `not_me` removes it and the auto rule never selects it again, and clearing a decision falls back to the auto rule. A refresh with new plays or a changed cfg only reconciles undecided `origin=auto` rows.
- The rule lives in `app::features::players::selection` (pure, table-tested); `min_norm_len = 4` lives in `IdentityParams` (D17). The cfg is read live at refresh time and never persisted.

Expected pilot outcome: `TWulfZ` (cfg prefix) and `TWulfZasdasdasd d jSS||` (cfg equal) are auto-selected; `""`, `W`, `w`, `Wulf`, `s`, `Madeline`, `Klinsx` and `StevenS` are listed unticked.

### Alternatives considered
- **Keep the scored model and tiers.** Rejected: on the R2 evidence its key offline signal would rank other players above the user's own offline names, so "Probably you" would mislead exactly where it is meant to help.
- **Fuzzy matching (Jaro-Winkler ≥ 0.92) against the login.** Rejected: it adds false positives (`twulfs`) for no case the prefix rule misses on real data, and it is harder to explain in the UI.
- **Suggest via `.osg` presence.** Rejected until ADR 0012 settles what `.osg` means.

### Consequences
- A user with many offline names ticks them by hand once; the decisions persist.
- If the newest cfg belongs to someone else (a shared PC), that alias is auto-selected; the user unticks it once and `not_me` persists.
- The first-run wizard needs `alias_stats` and the session-user rule in F0, not the heuristics. Identity table tests (spec 004 AC1–AC12) remain an F0 exit criterion.
- Architecture §5.6, the §8 Identity UX line, §10, §12 and CLAUDE.md are updated in the same change.
