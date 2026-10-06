# Community trill vocabulary

Branch `feat/trill-vocabulary` from `main` @ 6e51c83 · opened 2026-10-06

## Objective
The trill family speaks the osu! wiki / VSRG glossary language: `chordbracket` becomes `chordtrill`, and `jumptrill` / `split_trill` split chord alternations by hands as the wiki does.

## Problem and why
- `regular.stream.chordbracket` is a wolluf coinage (ADR 0017 took it from Interlude's "Brackets"). The community term for that shape is chordtrill (VSRG Pattern Glossary rev.4 "CHORDTRILL" panel; osu! wiki Beatmap/Pattern/osu!mania/Trill). In the pilot's 21 870-chart library the only "chord…bracket" name is `kasumi99's Chord//Bracket`, i.e. two skills, which the hint table currently misreads as `chordbracket`.
- osu! wiki Trill: a chordtrill is two alternating chords; a jumptrill is a chordtrill whose chords are each played with one hand; a split trill one whose chords each need both hands. Today `split_trill` means "one side per hand", single notes included (`rules/split_trill.rs`), which is the wiki's jumptrill / two-hand trill, and `jumptrill` takes any chord alternation.
- Re-meaning ids in place is normally forbidden (ADR 0017 Consequences, `docs/conventions.md` "Rename by adding a new id"). It is safe now: 0 `segment_label` events in every data dir (`~/.local/share/wolluf`, `~/.local/share/wolluf-ui`, Windows `AppData/Local/wolluf/data`, checked 2026-10-06), no label fixtures, no label exchange with mania-tracker yet. Segments and hints in cache.db are derived and re-derive on a stage VERSION bump. The ADR amendment records the exception; the rule is back in force from the first stored label.
- Rejected: minimal rename only (leaves `split_trill` inverted against the wiki). The user chose the full wiki alignment on 2026-10-06.

## Scope
- Authorized: `crates/patterns` (trill, jumptrill, split_trill, chordbracket→chordtrill rules, params, axes, segment tests), `crates/engine` (taxonomy, hints, stage versions, golden), `crates/app` test counts if touched, `stage_versions.lock`, ADR 0017 amendment, `CLAUDE.md` domain line, research 05 (new hint-lift numbers).
- Out of scope: new community ids (double stream, double stair, reverse shield, jumpjack, grace…), pattern previews in the UI (next feature), longjack threshold (deliberate 3+, ADR 0017).

## Definitions (target)
Stream-fast and disjoint-from-previous as today (`common::alternations`, `stream_gap_ok`). Hands from the layout; the `Hand::Both` column belongs to neither hand.
- `trill`: single notes, two columns alternating, any hands (absorbs today's single-note `split_trill`).
- `jumptrill`: two chords (2+ notes) alternating, each chord wholly within one hand, the two chords on different hands.
- `split_trill`: two chords alternating, each chord with notes on both the left and the right hand.
- `chordtrill` (key `ct`): a run of stream-fast chord rows (2+ notes, no upper limit), each disjoint from the previous row, where each row either repeats the row two before (strict alternation) or interleaves with the previous row (not an Interlude roll). Jumptrills and split trills are chordtrills too; they outrank it, so it stays as their secondary tag.
- Priority (unchanged slots): … burst, split_trill, jumptrill, bracket, trill, chordtrill, roll … A whole-row both-hand alternation such as `[1357][246]` is a split trill even though each hand's part is a bracket.

## Constraints
- Ids are stable strings; this rename is a recorded one-time exception (ADR 0017 amendment).
- Thresholds stay params (D17); `chordbracket_min_rows` becomes `chordtrill_min_rows` (3).
- Stage-lock: bump `patterns` and `chart_label` VERSION, regenerate the lock.
- TDD: strict. Runner: `cargo nextest run -p wolluf-patterns`, `cargo nextest run -p wolluf-engine`.
- Delivery: ~350 authored lines forecast.

## Acceptance criteria
- Taxonomy lists `regular.stream.chordtrill` (key `ct`) and no `chordbracket` → `taxonomy_k7_ids` snapshot.
- `Chord//Bracket` no longer hints a chord trill; "Chordtrill" / "Chord Trill" do → `hints_leaf_words_map_to_patterns`.
- `xxx..../....xxx` is a jumptrill, `x.x.x.x/.x.x.x.` a split trill, `xxxx.../....xxx` a chordtrill, single-note 3/4 under either thumb preset a trill → rule unit tests and segment precedence tests.
- Golden covers every id, stage-lock check passes → `patterns_golden_covers_every_pattern_id`, `cargo xtask stage-lock --check`.

## Tasks
- [x] T1 — Rename `chordbracket` → `chordtrill` (id, key `ct`, rule, param, priority, axes, taxonomy, hint phrases, stage versions, lock), behaviour unchanged except hints; ADR 0017 amendment (community-term principle, the rename, the exception); CLAUDE.md line. Acceptance: literal-id tests and snapshots name chordtrill; `Chord//Bracket` hints bracket only. Route: inline (mechanical rename). Tier: medium. Commit: feat(patterns): rename chordbracket to the community term chordtrill
- [ ] T2 — Redefine trill / jumptrill / split_trill / chordtrill by hands per the definitions above, with tests, segment precedence tests and golden fixtures; ADR amendment definitions; taxonomy descriptions; stage VERSION bump. Acceptance: the four shapes in Acceptance resolve as stated. Route: delegated (writer: 2+ non-trivial files in `crates/patterns` + engine golden). Tier: medium. Commit: —
- [ ] T3 — Close: full gate block, `corpus_patterns` harness, `wolluf library hints` before/after on a copy of the data dir, numbers into research 05, remove this document. Route: inline. Tier: medium. Commit: —

## Progress
- 2026-10-06 T1: RED `taxonomy_vocabulary_decisions` (no `regular.stream.chordtrill`) and `hints_leaf_words_map_to_patterns` failed → GREEN after the rename; frozen params hash and both vkeys re-frozen, patterns VERSION 4→5, chart_label 3→4, lock regenerated. `cargo fmt --all --check`: ok. `cargo clippy --workspace --all-targets -- -D warnings`: ok. `cargo nextest run --workspace`: 950 passed, 19 skipped. `cargo xtask stage-lock --check`: ok. Pre-existing failures: none.

## Next step
T2: delegate the trill-family redefinition to one writer in `crates/patterns` + `crates/engine/src/stage/golden.rs`, `taxonomy.rs` descriptions; RED first with the four shapes in Acceptance.
