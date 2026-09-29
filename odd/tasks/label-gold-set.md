# Gold-set labelling (`wolluf label`)

Branch `feat/label-gold-set` from `main` @ d22ebc5 · opened 2026-09-29

## Objective
The pilot labels 7K chart sections by pattern from the terminal: `wolluf label` shows a sampled window and records the answer. `wolluf label export` writes the gold set to `fixtures/labels/` as anchors plus pattern ids only, with no map content.

## Problem and why
F1 exit criterion: per-pattern precision measured on 200–300 hand-labelled segments (architecture §12 F1 row). Labels are anchored by time and never by derived segment ids (§5.1 `SegmentAnchor`). They are stored as append-only `feedback_event`s (§5.3, §6.1) so the future Playfield relabel reuses them. Labelling is **blind**: the pattern engine (deliverable 2) is built in parallel and never shows suggestions here, so the gold set stays an unbiased test set.

## Scope
- Authorized:
  - engine: the k7 pattern taxonomy as data (stable `PatternId`s, parent axis, short key, one-line description);
  - core: `SegmentAnchor` if missing;
  - store: `feedback_event` insert/list queries for gold labels (no schema change unless unavoidable);
  - app: a `labeling` feature (sampler, submit, undo as a compensating event, stats, export);
  - CLI: `wolluf label`, `wolluf label stats`, `wolluf label export`;
  - `fixtures/labels/`.
- Out of scope: UI labelling (deliverable 5), engine suggestions, telemetry/crowd labels (F5).

## Constraints
- Taxonomy ids are persisted and never renumbered. Axes: `regular.{jack,tech,speed,stream}` and `ln.{general,tech,inverse,release}` (CLAUDE.md domain facts).
- Sampling is deterministic from a seed. It is stratified by labelled dan/level (chart_label) and by density, played charts first, and never repeats an anchor already labelled. Window defaults to 4 s and can be widened or narrowed while labelling.
- A section may carry several patterns plus flags: `mixed`, `unsure`, `skip`. Undo appends a compensating event, because feedback_event is append-only.
- Export: JSONL sorted by (md5, t0), with fields md5, t0_us, t1_us, cols, patterns, flags, labelled_at. No note rows, no titles, no player names.
- TDD: strict. Runner `cargo nextest run -p <crate>`.
- Delivery: ~900 lines forecast. Single PR.

## Acceptance criteria
- The taxonomy lists every id under the 8 axes with unique short keys → `cargo nextest run -p wolluf-engine taxonomy`.
- The sampler is deterministic per seed, stratified across levels, and skips labelled anchors → app unit tests.
- Submit, undo and export round-trip, and the export contains no map content → app + CLI tests.
- `wolluf label` REPL works end to end on a synthetic install → CLI integration test (stdin-scripted session).

## Tasks
- [x] T1: pattern taxonomy + `SegmentAnchor` + store queries + app `labeling` service + CLI commands. Acceptance: all criteria above. Route: delegated (writer across engine/core/store/app/cli; one feature). Tier: medium. Commit: `feat(label): add gold-set labelling for 7K chart sections`
- [ ] T2: close. Full gates; CLAUDE.md commands; remove this document. Route: inline. Tier: passive. Commit: —

## Progress
- 2026-09-29 T1 implemented (RED/GREEN per layer; workspace 631 passed). An independent verifier (High tier: persists into user.db) asked for changes: export guard bypass via relative/`..` paths, payload `v`/`action`, D11 logic out of the CLI, submit window/overlap checks, one sampling loop, identity tests + `self_profile_id`, "played first" scoped to self. The scoped correction and the vocabulary pass are applied: workspace 652 passed; payload v1 with `assert_set`/`assert_none`; optional `thumb_pref`; 25 patterns frozen by snapshot. Spot check `--test cli_label` passed.
- Accepted change (user decisions on 7K vocabulary, 2026-09-29):
  - minijack = exactly 2 consecutive notes in one column; longjack = 3+;
  - `jackspeed` removed, because speed belongs to difficulty;
  - bracket = two or more simultaneous trills within a hand (wiki/MinaCalc);
  - new `chordbracket` = a 2–3-note bracket or chord shape moving without jacks (Interlude's "Brackets"); alternating chords of more than 4 notes count as trill/jumptrill;
  - the thumb is not a separate hand, but labels may carry an optional `thumb_pref` left/right so thumb-side preference can later be learned from users;
  - `ln.inverse.gap` / `ln.release.timing` renames;
  - a stored `none` answer for "no clear pattern".
  At close this vocabulary gets an ADR, because the ids are persisted.

## Next step
T2: document the payload shapes in architecture §5.3 + ADR for the 7K vocabulary; CLAUDE.md commands; full gates; pilot smoke with osu! closed; remove this document.
