# MinaCalc calculator (vendored v527) and chart difficulty adapter

Branch `feat/beta-skill-preview` from `main` @ 1930f5f · opened 2026-10-08. The branch carries several features in one PR, at the user's request; each keeps its own document.

## Objective
`wolluf-difficulty` computes quantised MinaCalc v527 skillsets (MSD per rate, SSR per goal) for any 4K or 7K chart, on Linux and Windows.

## Problem and why
The beta skill and recommendation preview (user request 2026-10-08) needs per-skillset difficulty. MinaCalc is the only MIT, pre-calibrated 4K skillset calculator. `minacalc-rs` is stale and needs libclang at build time, so the C++ is vendored instead (ADR 0022).

## Scope
- Authorized: `crates/minacalc/**`, `crates/difficulty/**`, root `Cargo.toml`/`Cargo.lock`, `xtask/layers.toml`, NOTICE, ADR 0022, architecture §3/D8/§9.1 wording.
- Out of scope: engine stage, cache table, app wiring, UI (feature `k4-difficulty-index`); Sunny; dans.

## Constraints
- Domain crate: no IO, no clock (CLAUDE.md hard rules). `wolluf-minacalc` has no workspace dependency.
- D3 deviation per ADR 0022: calculator floats never enter a hash; outputs are centi-MSD `i32`.
- Params in structs (D17): rate grid, LN hold-share cut, quantisation.
- Licence: Etterna MIT, credited in NOTICE in the vendoring commit (ADR 0008).
- TDD: strict. Runner: `cargo nextest run -p wolluf-minacalc -p wolluf-difficulty`.
- Delivery: one PR for the whole branch (user decision 2026-10-08).

### Frozen API (agents code against this)
`wolluf-minacalc`:
- `pub const CALC_VERSION: i32 = 527;`
- `pub struct NoteRow { pub notes: u32, pub time_s: f32 }`: bit `c` = column `c` from the left; times strictly increasing.
- `pub struct Skillsets(pub [f32; 8])`: Overall, Stream, Jumpstream, Handstream, Stamina, JackSpeed, Chordjack, Technical. `pub const SKILLSET_IDS: [&str; 8]` = `overall, stream, jumpstream, handstream, stamina, jackspeed, chordjack, technical`.
- `pub enum CalcError { Empty, NotIncreasing { index: usize }, MaskOutOfRange { index: usize }, UnsupportedKeycount(u8), Native }` (thiserror is not a dependency here; implement `Display` and `Error` by hand).
- `pub struct Calc` (`Send`, not `Sync`): `Calc::new() -> Result<Calc, CalcError>`, `Calc::version() -> i32` (asks the native side), `fn msd(&mut self, rows: &[NoteRow], rate: f32, keycount: u8) -> Result<Skillsets, CalcError>` (ssr=false path, goal 0.93), `fn ssr(&mut self, rows: &[NoteRow], rate: f32, goal: f32, keycount: u8) -> Result<Skillsets, CalcError>`.

`wolluf-difficulty` (`minacalc` module):
- `MinaCalcParams { rate_grid_milli: Vec<u16> (700..=1500 step 50), ln_unrated_hold_share_permille: u16 (400) }` with `Default` and `params_hash() -> [u8; 32]` (blake3 over postcard).
- `note_rows(chart: &Chart) -> NoteRowsOut { rows: Vec<NoteRow>, hold_share_permille: u16 }`: notes = tap | LN head; tail-only rows dropped; seconds from the first row.
- `MsdTable { status: MsdStatus, hold_share_permille: u16, rows: Vec<MsdAtRate> }`, `MsdAtRate { rate_milli: u16, centi: [i32; 8] }`, `MsdStatus { Rated, Unrated(UnratedReason) }`, `UnratedReason { LnHeavy, CalcRejected }`.
- `msd_table(calc: &mut Calc, chart: &Chart, params: &MinaCalcParams) -> MsdTable`.
- `ssr_centi(calc: &mut Calc, rows: &[NoteRow], rate_milli: u16, goal: f32, keycount: u8) -> Result<[i32; 8], CalcError>`.

## Acceptance criteria
- Native calc builds with cc only and reports 527 → `cargo nextest run -p wolluf-minacalc`.
- A synthetic 4K stream chart yields 8 positive skillsets; invalid input returns `Err`, never crashes → minacalc tests incl. proptest.
- Adapter drops tail-only rows, keeps times strictly increasing and masks ≤ `(1<<K)-1`; MSD does not decrease as rate rises → difficulty tests.
- REFORM 2nd 4K dans come out strictly ordered → `#[ignore]` corpus test `corpus_minacalc_reform_order`.
- Windows: `test (windows-2025)` CI job green (needs the branch pushed; the user chose one PR, so this is checked when it opens).

## Tasks
- [x] T1 — ADR 0022, layers edge, workspace members, architecture wording. Acceptance: `cargo xtask check-layers` 0 violations. Route: inline. Tier: high (crate edge). Commit: `build: vendor-ready minacalc and difficulty crates (ADR 0022)`
- [ ] T2 — `wolluf-minacalc`: vendored v527 + patch + shim + safe `Calc`. Acceptance: version 527, 4K synthetic positive, errors on bad input, proptest no crash, deterministic per platform. Route: delegated (writing ≥2 non-trivial files). Tier: high (licensing, unsafe). Commit: —
- [ ] T3 — `wolluf-difficulty` adapter, params, `msd_table`, `ssr_centi`, corpus test. Acceptance as above. Route: delegated. Tier: medium. Commit: —

## Progress
- 2026-10-08 T1: written; `cargo xtask check-layers`: 12 members, 0 violations.

## Next step
Run the T2/T3 agents, verify, commit T1–T3.
