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
- Params in structs (D17): rate grid, LN hold-share cut, quantisation. Hold share = LN objects ÷ all objects, in permille (the docs/research/03 definition); a chart at or above the cut is `Unrated(LnHeavy)`.
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
- Adapter drops tail-only rows, keeps times strictly increasing and masks ≤ `(1<<K)-1`; MSD Overall does not decrease as rate rises on map-length charts (v527 penalises fast rolls on short charts, e.g. a 30 s 1-3-2-4 roll scores 15.36 at 1.2 and 11.06 at 1.3) → difficulty tests.
- REFORM 2nd 4K dans come out strictly ordered → `#[ignore]` corpus test `corpus_minacalc_reform_order`.
- Windows: `test (windows-2025)` CI job green (needs the branch pushed; the user chose one PR, so this is checked when it opens).

## Tasks
- [x] T1 — ADR 0022, layers edge, workspace members, architecture wording. Acceptance: `cargo xtask check-layers` 0 violations. Route: inline. Tier: high (crate edge). Commit: `build: vendor-ready minacalc and difficulty crates (ADR 0022)`
- [x] T2 — `wolluf-minacalc`: vendored v527 + patch + shim + safe `Calc`. Acceptance: version 527, 4K synthetic positive, errors on bad input, proptest no crash, deterministic per platform. Route: delegated (writing ≥2 non-trivial files). Tier: high (licensing, unsafe). Commit: `feat(minacalc): vendor Etterna MinaCalc v527 behind a C ABI`
- [x] T3 — `wolluf-difficulty` adapter, params, `msd_table`, `ssr_centi`, corpus test. Acceptance as above. Route: delegated. Tier: medium. Commit: `feat(difficulty): rate charts with MinaCalc skillsets per rate`

## Progress
- 2026-10-08 T1: written; `cargo xtask check-layers`: 12 members, 0 violations.

- 2026-10-08 T2: RED `E0432 unresolved imports wolluf_minacalc::{CALC_VERSION, Calc, …}` → GREEN. `cargo nextest run -p wolluf-minacalc`: 15/15. ASan/UBSan run found signed overflow in upstream `fastpow` (`PatternModHelpers.h:25`); fixed with `-fwrapv` on GCC/Clang, outputs bit-identical before and after. MSVC has no `-fwrapv` (formally UB there); aarch64 unsupported (`sse2neon.h` not vendored).
- 2026-10-08 T3: RED (missing module) → GREEN. `cargo nextest run -p wolluf-difficulty`: 18 passed, 1 skipped. Corpus `corpus_minacalc_reform_order` (release, osu! closed): 6th 22.83 < 7th 23.57 < 8th 24.52 < 9th 26.05 < 10th 26.12 < β 27.55 < γ 30.45 < δ 32.30 < ε 34.88 (no EXTRA-ALPHA .osu in the pilot pack).
- 2026-10-08 Verifier (high tier): FAIL on missing NOTICE entry; one scoped correction applied: NOTICE entry, `static_assert(NUM_Skillset == 8)` in the shim, corpus test panics when the env var is unset, centi comment and ADR wording (a centi can differ by 1 across platforms), unused difficulty deps removed. Re-run: `cargo nextest run -p wolluf-minacalc -p wolluf-difficulty`: 33 passed, 1 skipped; clippy workspace clean; check-layers 0 violations; deny ok. Accepted, not changed: a 50,000 s scaled-time cap can still let a garbage chart allocate ~100–200 MB inside one `Calc` (estimate, not measured). Pending: `test (windows-2025)` CI, at PR time.

## Next step
Feature `k4-difficulty-index`: engine K4 profile + difficulty stage + `chart_msd` cache table + index wiring.
