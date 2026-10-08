# 4K profile and MinaCalc difficulty index

Branch `feat/beta-skill-preview` (single PR with `minacalc-calculator`, `skill-preview`, `recs-preview`, `rate-copies`) · opened 2026-10-08

## Objective
`library index` rates every 4K and 7K chart with MinaCalc v527 across the rate grid, 4K charts are indexed without pattern segments, and the desktop shows a keymode switcher and the MSD of a chart.

## Problem and why
The skill and recommendation preview reads per-chart skillsets from cache.db. 4K is disabled in the engine registry (`crates/engine/src/profile.rs:46-51`) and has no vocabulary yet; ADR 0023 enables it without one.

## Scope
- Authorized: `crates/engine` (profile, `stage/difficulty.rs`, stage registry, goldens), `stage_versions.lock`, `crates/chart/src/layout.rs` (`DEFAULT_K4`), `crates/store` (`chart_msd`, cache v6), `crates/app` (index wiring, meta keymodes, chart MSD query), `apps/desktop/src-tauri` (commands), `apps/cli` (MSD in `library list`, `chart info`), `apps/desktop/ui` (keymode switcher, settings keymode, thumb flags, MSD block), ADR 0023, architecture §9.1/§12 wording, CLAUDE.md commands.
- Out of scope: 4K pattern rules and taxonomy, skill, recommendations, dans.

## Constraints
- 7K `patterns` key and `segment` rows unchanged; `stage_versions.lock` only gains `difficulty` (ADR 0023).
- Calculator floats never enter a golden; centi may differ by 1 across platforms (ADR 0022).
- D5: no hard-coded keymode lists in features or UI; keymodes come from `Registry` / `meta_keymodes`.
- D11: shell commands ≤ 10 lines, no logic.
- Store is the only SQL owner (D6); cache rows keyed by vkey (D15).
- TDD strict. Runners: `cargo nextest run -p <crate>`, `pnpm -C apps/desktop/ui test`.

### Frozen contracts
Engine:
- `pub enum CalcId { MinaCalc }` with `as_str() = "minacalc"`.
- `KeymodeProfile { keymode, default_layout, label_sources, taxonomy: Option<&'static [PatternDef]>, calculators: &'static [CalcId] }`.
- `stage::difficulty`: `STAGE = "difficulty"`, `VERSION = 1`, `vkey(chart_parse_vkey, keymode, &MinaCalcParams) -> Result<VersionKey, EngineError>` (like `patterns::vkey`; amended 2026-10-08), `run(calc, chart, params) -> MsdTable`.

Store (cache.db):
```sql
CREATE TABLE chart_msd (
  md5 TEXT NOT NULL, vkey BLOB NOT NULL CHECK (length(vkey) = 32), rate_milli INTEGER NOT NULL,
  overall INTEGER NOT NULL, stream INTEGER NOT NULL, jumpstream INTEGER NOT NULL, handstream INTEGER NOT NULL,
  stamina INTEGER NOT NULL, jackspeed INTEGER NOT NULL, chordjack INTEGER NOT NULL, technical INTEGER NOT NULL,
  PRIMARY KEY (md5, vkey, rate_milli)) STRICT, WITHOUT ROWID;
CREATE TABLE chart_msd_status (
  md5 TEXT NOT NULL, vkey BLOB NOT NULL CHECK (length(vkey) = 32), status TEXT NOT NULL CHECK (status IN ('rated','ln_heavy','calc_rejected')),
  hold_share_permille INTEGER NOT NULL, PRIMARY KEY (md5, vkey)) STRICT, WITHOUT ROWID;
```
(vkey as a 32-byte BLOB like every other derived table, amended 2026-10-08; columns in centi-MSD; `status` row exists for every processed chart, rate rows only when `rated`).

App/IPC DTOs (camelCase):
- `KeymodeDto { keymode: u8, hasPatterns: bool, calculators: Vec<String>, defaultLayout: String, hasThumb: bool }`; command `meta_keymodes() -> Vec<KeymodeDto>`.
- `ChartMsdDto { md5, status: "rated" | "ln_heavy" | "calc_rejected" | "pending", holdSharePermille: u16, calcVersion: i32, skillsets: Vec<String> (the 8 ids), rates: Vec<MsdRateDto> }`, `MsdRateDto { rateMilli: u16, centi: Vec<i32> }`; command `chart_msd(md5) -> ChartMsdDto`.
- `LibraryChartDto` gains `msdOverallCenti: Option<i32>` (rate 1.0, rated charts only).

## Acceptance criteria
- K4 profile enabled, taxonomy-less, `k4.generic`, no thumb → engine profile tests.
- `difficulty` stage golden covers 4K and 7K fixtures; `stage-lock --check` diff only adds `difficulty`; 7K `patterns` key literal unchanged → engine tests + `cargo xtask stage-lock --check`.
- `chart_msd` round-trips by vkey and old vkeys are pruned → store tests.
- Indexing a 4K fixture chart writes `chart_msd` rows and no segments; a 7K chart gets both → app tests.
- Corpus: every 4K chart and every rice 7K chart gets a status row; timing recorded → `corpus_library_index` run.
- UI: keymode switcher lists keymodes from `meta_keymodes`, hidden under 2; thumb flag buttons hidden on a layout without a thumb; chart details show MSD → vitest.

## Tasks
- [x] T1 — ADR 0023 + architecture §9.1/§12 amendments. Route: inline. Tier: high (re-litigable, cross-crate). Commit: `docs: enable 4K through a calculator-only profile (ADR 0023)`
- [x] T2 — Engine: profile `Option` taxonomy + calculators, K4 profile, `DEFAULT_K4`, `difficulty` stage + golden + lock. Route: delegated (engine owner). Tier: high (stage-lock, persisted key). Commit: `feat(engine): enable 4K and rate charts in a difficulty stage`
- [x] T3 — Store: `chart_msd` tables, repo, cache v6. Route: delegated (store owner, parallel with T2). Tier: medium. Commit: `feat(store): cache MinaCalc skillsets per chart and rate`
- [x] T4 — App: index runs the difficulty stage, skips segmentation without a taxonomy, played charts first; meta keymodes; chart MSD query; library list MSD. Route: delegated (app owner). Tier: medium. Commit: `feat(app): rate indexed charts with MinaCalc and expose keymodes`
- [x] T5 — Shells: `meta_keymodes`, `chart_msd` commands; CLI MSD column and `chart info` table; bindings. Route: delegated (desktop, cli owners). Tier: medium (additive IPC). Commit: `feat: serve keymodes and chart MSD over IPC and the CLI`
- [ ] T6 — UI: keymode switcher from `meta_keymodes`, settings keymode from the URL, thumb flags hidden without a thumb, MSD block in chart details. Route: delegated (ui owner). Tier: medium. Commit: —
- [ ] T7 — Investigate the 461 `calc_rejected` charts (4K 37, 7K 424) and MSD dips on tiny charts; fix or document. Route: delegated (research). Tier: medium. Commit: —

## Progress
- 2026-10-08 T2: RED (E0425 `DEFAULT_K4`; 9 profile errors; 28 unresolved stage names; 5 app tests pinning "4K disabled") → GREEN. Frozen keys: difficulty K7 `25087453…`, K4 `37e2514d…`; lock diff +4/−0 (`difficulty` v1 only); 7K patterns key test untouched and green.
- 2026-10-08 T3: RED → GREEN, `cargo nextest run -p wolluf-store` 93/93. Store API: `repo::cache::chart_msd::{replace_for, get, overall_at, rated_at, missing_for_keymode, prune_except}`.
- 2026-10-08 Verifier (high): PASS; `cargo nextest run --workspace` 1108 passed, 20 skipped; stage-lock, clippy, fmt, layers, deny, lint-canary ok. Scoped correction applied: vkey BLOB(32) like other cache tables (contract amended), `Rated` without rows rejected (RED observed), shared `hash_field` helper (no key moved). Re-run: store+engine+app 446 passed, 6 skipped; stage-lock ok; clippy ok. Spot check by parent: `cargo xtask stage-lock --check` ok.
- Note for T4: `missing_for_keymode` lists catalog charts, including ones that never parse; plan difficulty work from the parse memo, not from that list.

- 2026-10-08 T4: RED (24 compile errors on the new API) → GREEN, `cargo nextest run -p wolluf-app` 255 passed, 6 skipped. Corpus `corpus_library_index` (release, osu! closed): 4K 2,777 catalog / 2,777 status (2,351 rated, 389 LN-heavy, 37 rejected); 7K 18,565 / 18,339 (14,467 rated, 3,448 LN-heavy, 424 rejected; 226 missing = md5 drift, never parsed). First index 266.0 s for 21,116 charts, second 838 ms. Deviations accepted: `Calc::new()` failure is an unmemoized item failure; a catalog/file keymode mismatch memoizes as ParseFailed with no status row.
- 2026-10-08 T5: RED ("Command meta_keymodes not found"; 5 CLI tests) → GREEN. `cargo nextest run -p wolluf-desktop` 28/28 after `cargo xtask bindings` (+50 lines); `wolluf-cli` 86/86; `pnpm tsc --noEmit` clean (parent). `meta_keymodes` returns `Result` like every command (UI `call()` unwraps Results).

## Next step
T6 UI, T7 calc_rejected investigation.
