# Rate copies (generated .osu + time-stretched audio)

Branch `feat/beta-skill-preview` (single PR with `skill-preview`, `recs-preview`) · opened 2026-10-08

## Objective
From a recommendation (or a chart), the user generates a rate-edited copy at any grid rate: wofella previews the files, the user confirms, and a job writes a new `.osu` and an Ogg audio file into the chart's set folder; after F5 in song select the copy is playable in stable.

## Problem and why
The user wants recommendations at any rate and wofella to generate the rates (2026-10-08). ADR 0025 records the design, the licence constraints and the rejected tools.

## Scope
- Authorized: new crates `crates/drills` (`wolluf-drills`), `crates/signalsmith` (`wolluf-signalsmith`), `crates/audio` (`wolluf-audio`); `crates/engine` (re-export only, if the app needs drills through the engine); `crates/app` (`export` module with `ExportPermit`, a `rate_copies` feature slice, a job); `apps/desktop/src-tauri` (plan/confirm commands); `apps/cli` (`wolluf rate-copy plan|create`); `apps/desktop/ui` (dialog from recommendation cards and chart details, job progress, "press F5" notice); root `Cargo.toml`/`Cargo.lock`, `xtask/layers.toml`, NOTICE, ADR 0025, architecture §3/§12.
- Out of scope: NC (pitch-follows-rate), .osz import, collection.db, keysounded maps, LN-heavy maps, cut drills, drill outcomes as evidence.

## Constraints
- D9: only `app::export` writes into the osu! folder, with an `ExportPermit` minted by `confirm(preview_id)`; trybuild compile-fail test for writing without a permit.
- Never overwrite; new files only; reuse an existing audio at that rate.
- No GPL/LGPL in-process (ADR 0008); Signalsmith vendored with cc only (ADR 0025, ADR 0022 build rules).
- Domain crates do no IO (`wolluf-drills`, `wolluf-signalsmith`).
- TDD strict; DSP checked by tolerances (onset drift ≤ 2 ms at 0.70–1.50x on a click track), never hashes.

### Frozen contracts
- `wolluf_drills::rate_copy(osu: &[u8], rate_milli: u16, &RateCopyParams) -> Result<RateCopy, DrillError>`; `RateCopy { osu: Vec<u8>, version: String, audio_filename: String, source_audio: String, osu_filename: String }`. Rules in ADR 0025. Errors: `UnsupportedMode`, `Keysounded`, `NoAudio`, `Malformed { line }`.
- `wolluf_signalsmith::Stretcher::new(channels, sample_rate)`, `stretch(&mut self, input: &[f32] interleaved, rate: f32) -> Vec<f32>` (pitch kept).
- `wolluf_audio::render_rate(audio: &[u8], ext_hint: &str, rate_milli: u16, &AudioParams) -> Result<Vec<u8> /* ogg */, AudioError>`.
- IPC: `rate_copy_plan(md5, rateMilli) -> RateCopyPlanDto { previewId, md5, rateMilli, folder, osuFilename, version, audioFilename, audioExists, osuExists, refusal: Option<String> }`; `rate_copy_confirm(previewId) -> JobIdDto` (mints the permit, starts job `rate_copy`); job emits DataChanged `library` when done.

## Acceptance criteria
- Rewriter: rate 1000 is the identity on times; LN tails only with type bit 128; hit-sample fields byte-identical; t′·r within 0.5 ms of t; untouched sections byte-identical; deterministic → drills tests + proptest.
- Stretch: click-track onset drift ≤ 2 ms, output length = input/r ± 10 ms → signalsmith/audio tests.
- Export: no write without a permit (trybuild); never overwrites; refuses keysounded/LN-heavy → app tests.
- Corpus: one rate copy of a pilot 4K chart rendered into a scratch copy of its folder, decoded back, `.osu` parsed by our decoder with the expected rate-scaled times → `#[ignore]` test.
- UI: plan dialog, confirm, progress, F5 notice → vitest.
- Windows E2E (manual, by the pilot): copy appears after F5 and plays in sync.

## Tasks
- [x] T1 — ADR 0025, crate skeletons, layers edges, workspace deps, architecture wording. Route: inline. Tier: high (crate edges, native code). Commit: `build: add rate-copy crates and their edges (ADR 0025)`
- [ ] T2 — `wolluf-drills` rewriter. Route: delegated. Tier: medium. Commit: —
- [ ] T3 — `wolluf-signalsmith` vendored + `wolluf-audio` decode/stretch/encode. Route: delegated. Tier: high (unsafe, licensing). Commit: —
- [ ] T4 — `app::export` + `ExportPermit` + rate-copy service and job; shells; CLI. Route: delegated. Tier: high (D9). Commit: —
- [ ] T5 — UI dialog and wiring from recommendations and chart details. Route: delegated. Tier: medium. Commit: —

## Progress
- 2026-10-08 T1: `cargo check` of the three skeletons ok (vorbis_rs 0.5.6 builds its C with cc, no bindgen); `cargo xtask check-layers` 15 members, 0 violations; `cargo deny check` ok.

## Next step
T1 inline, then T2 ∥ T3.
