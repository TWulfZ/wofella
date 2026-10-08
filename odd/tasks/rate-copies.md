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
- `wolluf_drills::rate_copy(osu: &[u8], rate_milli: u16, &RateCopyParams) -> Result<RateCopy, DrillError>`; `RateCopy { osu: Vec<u8>, version: String, audio_filename: String, source_audio: String, osu_filename: String }`. Rules in ADR 0025. Errors: `UnsupportedMode`, `Keysounded`, `NoAudio`, `Malformed { line }`, `RateOutOfRange { rate_milli }`, `IdentityRate`, `AlreadyRateCopy` (amended 2026-10-08). The set's `.osb` is not seen by the rewriter: the app refuses sets whose `.osb` has `Sample` events.
- `wolluf_signalsmith::Stretcher::new(channels, sample_rate) -> Result<Stretcher, StretchError>`, `stretch(&mut self, input: &[f32] interleaved, rate: f32) -> Result<Vec<f32>, StretchError>` (pitch kept; amended: input validation needs an error).
- `wolluf_audio::render_rate(audio: &[u8], ext_hint: &str, rate_milli: u16, &AudioParams) -> Result<Vec<u8> /* ogg */, AudioError>`; also `decode` and `Pcm`. `AudioParams { vorbis_quality, mp3_gapless }`.
- IPC: `rate_copy_plan(md5, rateMilli) -> RateCopyPlanDto { previewId, md5, rateMilli, folder, osuFilename, version, audioFilename, audioExists, osuExists, refusal: Option<String> }`; `rate_copy_confirm(previewId) -> JobIdDto` (mints the permit, starts job `rate_copy`); job emits DataChanged `library` when done.

## Acceptance criteria
- Rewriter: rate 1000 is refused (and is the identity on times inside the rewriter); LN tails only with type bit 128; hit-sample fields byte-identical; |t′ − t/r| ≤ 0.5 ms (amended: |t′·r − t| ≤ r/2 is the real bound when r > 1); untouched sections byte-identical; deterministic → drills tests + proptest.
- Stretch: click-track onset drift ≤ 2 ms, output length = input/r ± 10 ms → signalsmith/audio tests.
- Export: no write without a permit (trybuild); never overwrites; refuses keysounded/LN-heavy → app tests.
- Corpus: one rate copy of a pilot 4K chart rendered into a scratch copy of its folder, decoded back, `.osu` parsed by our decoder with the expected rate-scaled times → `#[ignore]` test.
- UI: plan dialog, confirm, progress, F5 notice → vitest.
- Windows E2E (manual, by the pilot): copy appears after F5 and plays in sync.

## Tasks
- [x] T1 — ADR 0025, crate skeletons, layers edges, workspace deps, architecture wording. Route: inline. Tier: high (crate edges, native code). Commit: `build: add rate-copy crates and their edges (ADR 0025)`
- [x] T2 — `wolluf-drills` rewriter. Route: delegated. Tier: medium. Commit: `feat(drills): rewrite .osu files to a rate`
- [x] T3 — `wolluf-signalsmith` vendored + `wolluf-audio` decode/stretch/encode. Route: delegated. Tier: high (unsafe, licensing). Commit: `feat(audio): time-stretch chart audio to Ogg Vorbis`
- [x] T4 — `app::export` + `ExportPermit` + rate-copy service and job; shells; CLI. Route: delegated. Tier: high (D9). Commit: `feat(app): export rate copies behind an ExportPermit`
- [ ] T5 — UI dialog and wiring from recommendations and chart details. Route: delegated. Tier: medium. Commit: —

## Progress
- 2026-10-08 T1: `cargo check` of the three skeletons ok (vorbis_rs 0.5.6 builds its C with cc, no bindgen); `cargo xtask check-layers` 15 members, 0 violations; `cargo deny check` ok.
- 2026-10-08 T2: RED (20 of 21 on a stub) → GREEN, 29 tests; exact decimal arithmetic for `round_half_up(t / r)`; Video line dropped; storyboard commands retimed; spinner ends scaled; old negative-beatLength lines never rescaled; rate label gets a 3rd decimal only when needed (1.155x vs 1.16x).
- 2026-10-08 T3: RED → GREEN, 29 tests + `!Sync` doctest. Signalsmith Stretch 1.4.0 (`a670068d`) + Linear 0.6.4 (`de55e6a5`), cc only, `exact()` offline with a fixed seed. Click-track onset drift 0.477 / 0.141 / 0.156 / 0.023 ms at 0.70 / 0.85 / 1.15 / 1.50 (limit 2 ms), same after Vorbis round trip; 3-min stereo at 1.2x in 2.66 s release (decode 45 ms, stretch 1.80 s, encode ~0.8 s).
- 2026-10-08 Verifier (high): PASS with conditions (NOTICE, contract wording, MP3 encoder delay untested). One scoped correction: `AudioParams.mp3_gapless` (Symphonia's own `gapless` switch; LAME-tagged test records 228,624 vs 230,400 frames), decode buffer reserved from the frame count (capped at 30 min), source freed before encode, non-UTF-8 kept event lines, tolerant Bookmarks, offsets keep `max(3, source)` decimals, `IdentityRate` and `AlreadyRateCopy`, audio name split on the file-name component, lone CR read as a line break, unused deps removed. Re-run: drills+signalsmith+audio+minacalc 83 passed; clippy, fmt, deny, layers ok. NOTICE entries for Signalsmith added by the parent. Open for the Windows E2E: whether stable's BASS trims LAME delay like Symphonia (default `mp3_gapless = true`).

- 2026-10-09 T4: RED → GREEN (CLI code before its test: no strict RED there). `app::export` is the only writer; `ExportPermit` private, minted by crate-private `confirm(preview_id)`; previews in memory (15 min, 32 max); trybuild compile-fail tests (private fields, private confirm). Refusal codes: unsupported_mode, keysounded, no_audio, audio_missing, malformed, rate_out_of_range, identity_rate, already_rate_copy, ln_heavy, same_column_collision, unsafe_name, already_exists. Job `rate_copy` (stages render_audio, write), reports `nextStep: refresh_osu_then_sync`. Shells `rate_copy_plan`, `rate_copy_confirm`; CLI `wolluf rate-copy plan|create --rate R [--yes]`. Verifier (high, D9): PASS — single writer, no overwrite, traversal closed. One scoped correction: temp + hard-link publish, all-or-nothing per copy, reused audio must be a non-empty Ogg Vorbis file, name limits, storyboard variable expansion, `already_exists`, tests for each. Known residual: a folder swapped for a symlink between the check and the open (needs openat-style IO, new dependency). app+drills 359 passed; parent: `cargo xtask bindings` (+52/−3), desktop+cli 139/139, `tsc` clean, layers and deny ok. Architecture D9 names the create-only exemption.

## Next step
T5 UI: wire "Generate rate copy" (recommendations `onGenerateRateCopy`, chart details) to plan → confirm dialog → job progress → "press F5 in osu!, then sync"; i18n for refusal codes incl. `already_exists`, `export.error.target_not_a_file`, `rate_copy.error.audio_target_unusable`.
