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
- `stage::difficulty`: `STAGE = "difficulty"`, `VERSION = 1`, `vkey(chart_parse_vkey, keymode, &MinaCalcParams) -> VersionKey`, `run(calc, chart, params) -> MsdTable`.

Store (cache.db):
```sql
CREATE TABLE chart_msd (
  md5 TEXT NOT NULL, vkey TEXT NOT NULL, rate_milli INTEGER NOT NULL,
  overall INTEGER NOT NULL, stream INTEGER NOT NULL, jumpstream INTEGER NOT NULL, handstream INTEGER NOT NULL,
  stamina INTEGER NOT NULL, jackspeed INTEGER NOT NULL, chordjack INTEGER NOT NULL, technical INTEGER NOT NULL,
  PRIMARY KEY (md5, vkey, rate_milli)) STRICT, WITHOUT ROWID;
CREATE TABLE chart_msd_status (
  md5 TEXT NOT NULL, vkey TEXT NOT NULL, status TEXT NOT NULL CHECK (status IN ('rated','ln_heavy','calc_rejected')),
  hold_share_permille INTEGER NOT NULL, PRIMARY KEY (md5, vkey)) STRICT, WITHOUT ROWID;
```
(columns in centi-MSD; `status` row exists for every processed chart, rate rows only when `rated`).

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
- [ ] T2 — Engine: profile `Option` taxonomy + calculators, K4 profile, `DEFAULT_K4`, `difficulty` stage + golden + lock. Route: delegated (engine owner). Tier: high (stage-lock, persisted key). Commit: —
- [ ] T3 — Store: `chart_msd` tables, repo, cache v6. Route: delegated (store owner, parallel with T2). Tier: medium. Commit: —
- [ ] T4 — App: index runs the difficulty stage, skips segmentation without a taxonomy, played charts first; meta keymodes; chart MSD query; library list MSD. Route: delegated (app owner). Tier: medium. Commit: —
- [ ] T5 — Shells + UI: `meta_keymodes`, `chart_msd` commands; CLI MSD; UI keymode switcher, settings keymode from URL, thumb flags prop, MSD block; bindings. Route: delegated (desktop, cli, ui owners). Tier: medium (additive IPC). Commit: —

## Progress

## Next step
T1 inline, then T2 ∥ T3.
