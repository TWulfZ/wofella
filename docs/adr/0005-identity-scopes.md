# 0005 Identity scopes and auto-selection

- Status: Accepted
- Date: 2026-09-28

## Context
osu! stable's scores.db mixes the user's own plays with downloaded replays of other players, and it has no notion of "me". On the pilot, scores.db (version 20260924) holds 4,973 scores. 2,635 are under `TWulfZ` and 1,630 more are under offline names that are probably the user's (`""`, `W`, `w`, `Wulf`, …), plus a few names that are probably other players (research `03-maniahub-rejudge-drills-sessions-audit.txt` l.307, l.343, l.347). The `""` name alone covers 1,235 7K plays. The cfg `Username` cannot be trusted verbatim: on the pilot it is `TWulfZasdasdasd d jSS||` (CLAUDE.md).

Skill must be computed only for the identity set the user selects (G3). Other players' plays must stay viewable, but they must never contaminate a self profile or telemetry (D10). Mixing someone else's 1,000 plays into the user's θ would make every downstream number wrong without any visible error.

The source proposals disagreed between auto-selecting the top-1 candidate and using confidence tiers (Appendix A).

## Decision
As written in architecture §5.6:
- **Aliases are keyed by raw name bytes** per game: `alias(game, raw_name)`. Normalization is used for matching only, and `""` is a valid alias (§5.3).
- **Listing.** `alias_stats` (cache.db) gives every raw name with its play count, count per keymode, date range, online/offline split, replay availability and top charts.
- **Scoring** is a transparent additive model in `app::features::players::heuristics`. It is pure and table-tested, and every signal becomes a visible reason:
  - exact match to a linked osu! account's `username` or `previous_usernames` (strong +, optional, `/me` only);
  - a cfg `Username` match after NFKC + casefold + stripping non-alphanumerics: equal, cfg name starts with the alias, or Jaro-Winkler ≥ 0.92 on the prefix, **only for aliases whose normalized length is ≥ 4** (strong +);
  - most frequent name among plays with an online score id (medium +);
  - temporal interleaving with confirmed-self plays within 30 min (medium +, the key signal for offline aliases);
  - online scores that never interleave, charts never otherwise played, and dates before the first self play (−, the downloaded-replay signature).
- **Tiers.** Auto-included (≥ 0.9) needs at least one strong positive signal, and aliases shorter than 4 normalized characters are never auto-included. "Probably you" (0.5–0.9) is highlighted but unchecked. Everything else is listed normally or flagged as probably another player.
- **Decisions persist and always win.** `identity_decision(alias_id, me | not_me)` stores the user's answer, and heuristics only propose for aliases without one.
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
