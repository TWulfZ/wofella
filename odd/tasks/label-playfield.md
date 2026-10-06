# Label screen with a Canvas2D Playfield and section audio

Branch `feat/label-playfield` from `feat/engine-gaps` @ 8d55b59 · opened 2026-10-06

## Objective
A desktop **Label** screen runs the same blind gold-set rounds as `wolluf label`. It shows the window on an osu!mania-style Canvas2D Playfield with time-proportional spacing and loops that section's audio, read-only from the osu! folder.

## Problem and why
The pilot could not read patterns from the CLI's ASCII rows: there is no time-proportional spacing, no LN bodies and no column colour. With 0 gold labels, deliverable 4 (per-pattern precision on 200–300 segments, architecture §12 F1) is blocked. Architecture §8 (`docs/architecture.md:577`) already names the Canvas2D Playfield as the hand-labelling tool.

Rejected: porting mania-hub's replay renderer. It lives outside `algorithms/`, the root has no LICENSE as of ef02a57d, and `ReplayCanvas.ts` is a single 8,967-line file tied to its replay and skin model.

## Scope
- **Authorized:**
  - `crates/app` (library: `chart_window`, `chart_audio`);
  - `crates/source-osu` (read-only song file read);
  - `apps/desktop/src-tauri` (commands `chart_*`, `label_*`);
  - `apps/desktop/ui` (`features/playfield`, `features/label`, route, nav, i18n, query keys);
  - `docs/adr/0018-*`, `docs/architecture.md` §8.
- **Out of scope:**
  - segment overlay and relabel (deliverable 5 proper, after the gold set);
  - SV rendering;
  - a skin system;
  - Rust-side audio decoding (`wolluf-audio` is F4);
  - user-saved layouts.

## Constraints
- **osu! folder:**
  - read-only;
  - the webview sends only an md5, never a path;
  - path components are checked like `read_chart_verified` (D9, G6);
  - no new capability, plugin or CSP change (ADR 0009:55).
- **Blind labelling:** no segments in the window DTO (`apps/cli/src/cmd/label.rs:416`).
- **Events:** same `segment_label` v1 events through `LabelingService` (ADR 0017, architecture §5.3).
- **IPC:**
  - shells are about 10 lines (D11);
  - `@tauri-apps/*` only in `ui/src/ipc` (D14);
  - no 64-bit fields in DTOs (ADR 0009).
- **i18n:** es and en for every new string.
- **Branch:** do not edit `__root.tsx` or `styles.css`, which conflict with `feat/osu-theme`.
- **Dependencies:** `feat/engine-gaps` (#10, open), and #9 (open).
- **TDD:** strict. Runners: `cargo nextest run -p <crate> <filter>`, `pnpm -C apps/desktop/ui test <filter>`.
- **Delivery:** about 1,700 authored lines in one PR (pilot's choice, 2026-10-06).

## Acceptance criteria
- `chart_window` returns notes (LNs straddling `from` included), the red line before `from`, the layout hands and the span → `cargo nextest run -p wolluf-app chart_window`.
- `chart_audio` reads only `<Songs>/<chart folder>/<AudioFilename>`, rejects `..`, absolute and empty names, and caps the size → `cargo nextest run -p wolluf-source-osu song_file`, `-p wolluf-app chart_audio`.
- Label rounds work end to end on mock IPC: sample, draw, answer, submit, undo, reshape, done → `pnpm -C apps/desktop/ui test label`.
- Manual check: on the pilot's install, audio loops in sync with the scroll.

## Tasks
- [x] T1 — `chart_window`: extraction in `engine::window`, DTO and service in app, command. Route: delegated (writer, plus one correction). Tier: medium. Commit: `feat(desktop): expose chart window and labeling commands` (with T2: they share `lib.rs`, the smoke tests and `bindings.ts`)
- [x] T2 — `label_*` commands (taxonomy, sample, resolve_patterns, reshape, submit, undo, stats); the sampler takes `FnMut -> Future` so `label_sample` is `Send`. Route: delegated (writer). Tier: medium. Commit: same as T1
- [ ] T3 — `chart_audio` read-only plus ADR 0018 and the architecture §8 edit. Route: delegated (writer). Tier: **high** (independent verifier). Commit: —
- [x] T4 — Pure Playfield projection: time to y, LN clipping, beat lines, hand separators, shading outside the window. Route: delegated (writer). Tier: medium. Commit: `feat(ui): add pure playfield projection`
- [ ] T5 — Canvas Playfield plus `AudioClock` (WebAudio loop, offset slider, no-audio fallback). Route: delegated (writer). Tier: medium. Commit: —
- [x] T6 — Answer parser (REPL grammar) and session reducer. Route: delegated (writer). Tier: medium. Commit: `feat(ui): add label answer parser and session reducer`
- [ ] T7 — Label screen, route, nav, i18n es/en, pattern chips. Route: delegated (writer). Tier: medium. Commit: —
- [ ] T8 — Close: full gates, `CLAUDE.md` Desktop notes, manual audio check, remove this document. Tier: passive. Commit: —

## Progress
- 2026-10-06 T4+T6: RED: all 4 test files failed to resolve the missing modules → GREEN. Writer: `pnpm test playfield` 13 passed, `pnpm test label` 16 passed, tsc and lint clean, full UI suite 155 passed. Parent spot check: `pnpm test playfield label`: 29 passed; tsc ok.
- 2026-10-06 Accepted change: T1's writer re-exported `wolluf_chart` types from `engine` so the app could name them. That bypasses `layers.toml`, so the extraction moves into an engine module (like `render.rs`) and the app only maps it to DTOs (one scoped correction).
- 2026-10-06 T1+T2: RED: build failures on missing `chart_window`/types; smoke tests `Command chart_window not found`, `Command label_taxonomy not found`; `service_futures_are_send` failed with `Send is not general enough` → GREEN. After the correction: engine 94, app 178, desktop 17 passed. Parent: `cargo xtask bindings` regenerated; `nextest -p wolluf-desktop -p wolluf-app -p wolluf-engine`: 289 passed; clippy workspace clean; fmt clean; check-layers 0 violations; `grep wolluf_chart crates/app/src`: none.

## Next step
T3 (`chart_audio` + ADR 0018, high tier with verifier) and T5 (canvas + AudioClock) in parallel; then T7.
