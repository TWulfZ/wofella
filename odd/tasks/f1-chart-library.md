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
- Crate edges: only those already in `xtask/layers.toml`. app → engine → chart; store and source-osu do not need chart. No `layers.toml` change, so no ADR.
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
- [ ] T1 — `wolluf-chart` model + decoder + `chart!` DSL; pin `rosu-map 0.2.1`. Acceptance: decoder and DSL tests, proptest parse → rows invariants (sorted, no overlap per column, LN head < tail). Route: delegated (writer: model + decoder + DSL). Tier: medium. Commit: —
- [ ] T2 — `chart::layout` presets (7K `3|1+3` right thumb default, `3+1|3` left thumb, `4|3`, `3|4`, both thumbs; generic fallback for any K) mapping column → (hand, finger), mirror-aware. Acceptance: a unit test per preset. Route: delegated (same writer as T1, after T1). Tier: medium. Commit: —
- [ ] T3 — `wolluf-engine` crate: `chart_parse` stage (`VERSION = 1`, stage-lock golden), k7 profile, rows blob encode/decode (postcard + zstd, format-version header), `ParsedChart` summary (n_notes, n_ln, ln_ratio, length, nps), ASCII window render. Route: delegated. Tier: medium. Commit: —
- [x] T4 [P with T3] — store cache: `chart_parsed(md5, vkey, rows_blob, n_notes, n_ln, ln_ratio, length_ms)` and `chart_label(md5, vkey, source, scale, level_ord, level_text, skill_tag, is_variant)` + repos; `CACHE_SCHEMA_VERSION` 1 → 2. Route: delegated. Tier: medium. Commit: —
- [ ] T5 — engine `labels`: pure extraction from (folder, version, creator, set id) per source, porting `labels.py`. Route: delegated. Tier: medium. Commit: —
- [ ] T6 — app `library` feature: `IndexLibrary` job (7K per profile, played charts first, memo by (md5, vkey) in `derivation`, rayon, cancel, progress, item failures), chained after `SyncPlays`; query service (list with filters, get chart view); `JobKindDto` / `JobStartDto` / `JobSummaryDto` variants + bindings regen. Route: delegated. Tier: medium. Commit: —
- [ ] T7 — CLI `wolluf library index`, `wolluf library list [--keys --label --scale --limit]`, `wolluf chart show <md5> [--from s --to s --layout id]`. Route: delegated. Tier: medium. Commit: —
- [ ] T8 — corpus acceptance test `corpus_library_index` + timing. Route: inline. Tier: medium. Commit: —
- [ ] T9 — close: full gates + corpus; CLAUDE.md commands; remove this document. Route: inline. Tier: passive. Commit: —

## Progress
- 2026-09-29 T4: RED 49 build errors (missing API) plus a v1-rebuild assertion → GREEN 61/61 store tests, workspace 445 passed. Spot check `cargo nextest run -p wolluf-store` passed.
- Accepted change: the vkey for `chart_parsed`/`chart_label` is **stage-level** (stage + VERSION + pack/config, no per-chart input). Charts are memoized per md5 in `derivation.input_key`, which keeps D15 and the cross-chart queries (`list_filtered`, `counts_by_scale`). `schema_v1.sql` was renamed to `schema.sql`. Engine must write `ln_ratio = 0.0` when a chart has 0 notes (the column is REAL NOT NULL, and SQLite stores NaN as NULL).

## Next step
T1: delegate the chart model + decoder + DSL writer in the `../wolluf-f1` worktree, after pinning `rosu-map` in the root `Cargo.toml`.
