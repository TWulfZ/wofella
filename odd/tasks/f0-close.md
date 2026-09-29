# F0 close

Branch `chore/f0-close` from `chore/odd-workflow` @ 7410a2d · opened 2026-09-29

## Objective
Close the remaining F0 open items in `docs/specs/000-f0-index.md` "Close status", leaving every local gate green.

## Problem and why
The F0 close stage left these open. The user decided on 2026-09-29:
- accept targeted `cargo deny` ignores;
- accept ADR 0012 and ADR 0014;
- the WSL smoke passed.

A known inconsistency in how non-mania plays are counted is also still open (F0 close notes, item 3).

## Scope
- Authorized:
  - `deny.toml`;
  - ADR 0012 and 0014 status, plus ADR 0012 item 7 (the b25 handling in `crates/source-osu` codec::osg, the `crates/app` osg survey, the CLI test, spec 006 AC8 wording, architecture §13 O1/O9);
  - the non-mania counters in `crates/app` sync and the players stats;
  - ticking in `docs/specs/`.
- Out of scope:
  - the Windows NSIS smoke (manual, user);
  - the CI run link (needs a push, which the user decides);
  - any F1 work.

## Constraints
- ADR 0012 §Decision item 7; ADR 0015 version policy unchanged; D17 (no inline thresholds).
- TDD: strict (docs/conventions.md). Runner: `cargo nextest run --workspace`.
- Delivery: ~150 authored lines forecast.

## Acceptance criteria
- `cargo deny check` → advisories ok, bans ok, licenses ok, sources ok.
- Final-record b25 = 1 emits no `osg.nonzero_reserved`, and `wolluf osg survey --strict` does not count `b25_final_only` as an I3 failure → the osg unit tests, `apps/cli/tests/osg_cli.rs`, and the corpus `osg_corpus_invariants`.
- A first sync counts each non-mania play once → a sync unit test.
- ADR 0012 and 0014 are `Accepted`, and the F0 index "Close status" is updated.

## Tasks
- [x] T1: targeted advisory ignores for RUSTSEC-2024-0436 (paste via specta) and RUSTSEC-2024-0370 (proc-macro-error via tauri gtk), each with a reason. Acceptance: `cargo deny check` green. Route: inline. Tier: medium. Commit: `build: ignore unmaintained advisories pulled by specta and tauri`
- [x] T2: apply ADR 0012 item 7 and accept ADR 0012. Acceptance: final b25 is not a warning; survey I3 counts it as ok; AC8 reworded; §13 O1/O9 closed. Route: delegated (writer touches 2+ non-trivial files). Tier: medium. Commit: `fix(osg): treat final-record b25 as the full-combo flag`
- [x] T3: first sync counts non-mania plays once; players `non_mania` bucket semantics made consistent with ingest skipping mode ≠ 3. Route: delegated (4+ files to understand). Tier: medium. Commit: `fix(sync): count each non-mania play once per sync`
- [ ] T4: accept ADR 0014; tick the F0 index (deny, ADRs, WSL smoke 005 AC17); record what is still open (Windows smoke, CI link). Route: inline. Tier: passive. Commit: —

## Progress
- 2026-09-29 T1: RED `cargo deny check` → advisories FAILED (0436, 0370) → GREEN: advisories ok, bans ok, licenses ok, sources ok. `cargo nextest run -p xtask`: 50 passed.
- 2026-09-29 T2: RED 6/9 targeted tests (final b25 still warned, strict exit 1) → GREEN. Corpus `osg_corpus_invariants`: pass, I3 4682/4682, no osg.nonzero_reserved. `osg survey --strict`: 18 unexplained (down from 54). All 18 are the 9 known final_short files, which are outside T2. Spot check `cargo nextest run --workspace`: 439 passed.
- 2026-09-29 T3: RED `non_mania_score_with_replay_counted_once` left 2 right 1 → GREEN. The pilot's first and second syncs both give skipped_non_mania 19 (before: 38 on the first). The `non_mania` bucket was removed: it was never produced or persisted. Corpus sync + players: 2 passed.
- Accepted change: T3 dropped the `non_mania` bucket (DTO doc, en/es i18n key, UI fixture). That is simpler than keeping a bucket that is always empty.

## Next step
T4: accept ADR 0014; fix stale "ADR 0012 Proposed" mentions (CLAUDE.md, docs/research/04 l.3); update the F0 index Close status; then close the feature.
