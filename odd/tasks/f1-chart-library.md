# F1 chart + layout + library index

Branch `feat/f1-chart-library` from `main` @ c9a14d6 · opened 2026-09-29 · worktree `../wolluf-f1`

## Objective
The pilot's 7K charts are parsed into normalized charts and indexed with their source labels. `wolluf library index|list` and `wolluf chart show <md5>` (an ASCII 7K playfield) work over the whole library, so manual section labelling can start.

## Problem and why
F1 needs a chart model before anything else (architecture §12 F1 row, §3 chart crate). F0 left `wolluf-chart` empty, and moved `.osu` parsing to F1 (spec 002 l.33, l.37). Manual labelling of 200–300 sections (the F1 gold set) needs a way to look at a chart window before the UI Playfield exists (deliverable 5).

## Scope
- Authorized:
  - `crates/chart`: model, decoder, `chart!` DSL, layout presets.
  - A new `crates/engine` crate:
    - the `chart_parse` stage;
    - the k7 profile;
    - pure label extraction from difficulty names (research 03 l.296–303; `research/scripts/audit/labels.py`);
    - the ASCII playfield render.
  - `crates/store`: cache `chart_parsed` and `chart_label`, bump `CACHE_SCHEMA_VERSION`.
  - `crates/app`: a `library` feature with the `IndexLibrary` job and the query service.
  - `apps/cli`: `library` and `chart` subcommands.
  - Root `Cargo.toml` (pin `rosu-map`, add the engine member); `stage_versions.lock`; `CLAUDE.md` commands.
- Out of scope:
  - pattern rules, difficulty and eval (deliverables 2–4);
  - the UI Library/Playfield (deliverable 5);
  - Songs-folder fallback for maps missing from osu!.db (spec 002 R11; the catalog stays osu!.db-driven);
  - O6: labels come only from local difficulty names, nothing is shipped;
  - desktop IPC commands for library views (arrive with deliverable 5; only the job DTO variants are added now).

## Constraints
- D2/D3: chart and engine are pure over `&[u8]` (no fs, no HashMap in outputs). Columns: `clamp(floor(x*K/512), 0, K-1)`, `K = round(CircleSize)`, Mode 3 only, hold = `type & 128` with endTime in extras (research 01 l.58, l.82).
- Crate edges: only those already in `xtask/layers.toml`. app → engine → chart; store and source-osu do not need chart. One `layers.toml` change: xtask → engine so that stage-lock computes the goldens (ADR 0016).
- Every derived row carries `vkey`. The `chart_parse` stage has `VERSION` and a `stage_versions.lock` entry (architecture §5.5).
- Default layout preset: 7K `3|1+3` right thumb (the pilot). Presets are data with stable string ids (architecture §3).
- TDD: strict (docs/conventions.md). Runner: `cargo nextest run -p <crate>`; corpus `WOLLUF_CORPUS="/mnt/e/Games/osu!" … --run-ignored only`.
- Delivery: ~1,500 authored lines forecast (> 400), so the user chooses single PR vs stacked slices at close.

## Acceptance criteria
- Decoder round-trips synthetic `chart!` fixtures and maps columns and LNs per the constraint → `cargo nextest run -p wolluf-chart`.
- Label extraction matches `labels.py` on every pattern class (BMS tables, O2Jam, Jinjin, KomeijiDove, Wild, Road to Gamma, variants) → `cargo nextest run -p wolluf-engine labels`.
- The corpus index of the pilot:
  - 7K parsed ≈ 18,589 unique md5 among catalog charts;
  - parse failures < 0.5%;
  - label counts within explained deltas of the audit (8,755 rows / 7,768 charts);
  - the second run parses 0 charts;

  → `WOLLUF_CORPUS=… cargo nextest run -p wolluf-app --run-ignored only -E 'test(corpus_library_index)'`.
- `wolluf chart show <md5> --from 30 --to 40` prints a 7-column playfield with LN bodies, timestamps and the layout's hand split → CLI integration test on a synthetic chart.

## Tasks
- [x] T1 — `wolluf-chart` model + decoder + `chart!` DSL; pin `rosu-map 0.2.1`. Acceptance: decoder and DSL tests, proptest parse → rows invariants (sorted, no overlap per column, LN head < tail). Route: delegated (writer: model + decoder + DSL). Tier: medium. Commit: `feat(chart): add chart model, osu decoder, chart! DSL and layouts`
- [x] T2 — `chart::layout` presets (7K `3|1+3` right thumb default, `3+1|3` left thumb, `4|3`, `3|4`, both thumbs; generic fallback for any K) mapping column → (hand, finger), mirror-aware. Acceptance: a unit test per preset. Route: delegated (same writer as T1, after T1). Tier: medium. Commit: `feat(chart): add chart model, osu decoder, chart! DSL and layouts`
- [x] T3 — `wolluf-engine` crate: `chart_parse` stage (`VERSION = 1`, stage-lock golden), k7 profile, rows blob encode/decode (postcard + zstd, format-version header), `ParsedChart` summary (n_notes, n_ln, ln_ratio, length, nps), ASCII window render. Route: delegated. Tier: medium. Commit: `feat(engine): add chart_parse stage, rows blob, k7 profile and ASCII render`
- [x] T4 [P with T3] — store cache: `chart_parsed(md5, vkey, rows_blob, n_notes, n_ln, ln_ratio, length_ms)` and `chart_label(md5, vkey, source, scale, level_ord, level_text, skill_tag, is_variant)` + repos; `CACHE_SCHEMA_VERSION` 1 → 2. Route: delegated. Tier: medium. Commit: —
- [x] T5 — engine `labels`: pure extraction from (folder, version, creator, set id) per source, porting `labels.py`. Route: delegated. Tier: medium. Commit: `feat(engine): add engine crate with difficulty-name label extraction`
- [x] T6 — app `library` feature: `IndexLibrary` job (7K per profile, played charts first, memo by (md5, vkey) in `derivation`, rayon, cancel, progress, item failures), chained after `SyncPlays`; query service (list with filters, get chart view); `JobKindDto` / `JobStartDto` / `JobSummaryDto` variants + bindings regen. Route: delegated. Tier: medium. Commit: `feat(app): index the chart library after each sync`
- [ ] T7 — CLI `wolluf library index`, `wolluf library list [--keys --label --scale --limit]`, `wolluf chart show <md5> [--from s --to s --layout id]`. Route: delegated. Tier: medium. Commit: —
- [ ] T8 — corpus acceptance test `corpus_library_index` + timing. Route: inline. Tier: medium. Commit: —
- [ ] T9 — close: full gates + corpus; CLAUDE.md commands; remove this document. Route: inline. Tier: passive. Commit: —

## Progress
- 2026-09-29 T4: RED 49 build errors (missing API) plus a v1-rebuild assertion → GREEN 61/61 store tests, workspace 445 passed. Spot check `cargo nextest run -p wolluf-store` passed.
- Accepted change: the vkey for `chart_parsed`/`chart_label` is **stage-level** (stage + VERSION + pack/config, no per-chart input). Charts are memoized per md5 in `derivation.input_key`, which keeps D15 and the cross-chart queries (`list_filtered`, `counts_by_scale`). `schema_v1.sql` was renamed to `schema.sql`. Engine must write `ln_ratio = 0.0` when a chart has 0 notes (the column is REAL NOT NULL, and SQLite stores NaN as NULL).

- 2026-09-29 T1+T2 (one commit: lib.rs wires both modules): RED build failures per module plus a mutation check on proptest invariants → GREEN 53/53 chart tests. Spot check `cargo nextest run -p wolluf-chart --all-features` passed. Workspace clippy was red only in engine labels.rs, which T5 is still writing.
- Accepted change: rosu-map is used only for reading lines (BOM, UTF-16, section dispatch). Fields are parsed by our own `RawOsu`, because rosu-map silently drops malformed lines, maps unknown modes to std, clamps and removes timing points, and clamps inverted LN tails. `Hand::Both` was added so the `k7.both_thumbs` preset can mark column 4.

- 2026-09-29 T5, done before T3 because it is independent of chart: RED 13 build errors → GREEN 10/10. Fuzzed against the real labels.py: 1.2M ASCII-digit cases with 0 mismatches after a `//`-split regression fix.
- Accepted changes: identical BMS tags on one chart are merged (store PK), so BMS rows may come in under the audit's 7,755. O2Jam scale ids are lowercased (`o2jam_h`), because they are persisted StableIds; RED `o2jam_levels` → GREEN, 10/10.

- 2026-09-29 T3: RED 82 build errors → GREEN 43/43, plus a mutation check on the blob delta decode. The independent verifier (High tier: rows blob is a persisted encoding) approved with notes, and one scoped correction was applied: frozen v1 payload bytes, zstd content checksum (mutation check: without it a bit flip decoded a 15K chart), trailing-bytes reject, dense LN proptest, render width from the longest timestamp, and explicit golden formatting. Result: 49/49 engine tests, and `chart_parse` golden `ba3334e1…`.
- Accepted change: xtask → wolluf-engine edge (ADR 0016); postcard pinned with `default-features = false`, because `heapless-cas` pulled in the unmaintained atomic-polyfill (RUSTSEC-2023-0089); `chart_label` is registered in the stage lock alongside `chart_parse`.

- 2026-09-29 T6: RED 22 build errors → GREEN, app 117 → 130 tests. Mutation checks each failed their test: skip counted as memo, cancel check removed, played-first sort removed. Shell follow-ups: the CLI `jobs` match arm and the UI `LastSync` guard on the summary union. Parent added `KeymodeProfile::layout_by_id` (RED render with `k7.313_left_thumb` → GREEN) so render accepts any preset of the chart's keymode. Workspace 561 passed; clippy, check-layers, stage-lock, deny, bindings regenerated, and UI tsc/lint/vitest (126) all clean.
- Accepted changes:
  - An IO failure writes no derivation row, so it is retried.
  - Missing file and md5 mismatch are recorded as skipped, not failed.
  - Labels have their own memo and come from catalog names, so charts that are not on disk still get labels.
  - `songs_dir()` moved to `context.rs`.
  - `get()` re-parses the file for its diagnostics count; the store keeps none.

## Next step
T7: CLI `wolluf library index|list` and `wolluf chart show`.
