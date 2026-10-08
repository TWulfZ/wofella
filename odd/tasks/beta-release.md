# Beta release v0.1.0-beta.1

Branch `release/v0.1.0-beta.1` from `feat/session-labelling` @ aa84fa7 · opened 2026-10-07

## Objective
A `v0.1.0-beta.1` tag publishes a GitHub pre-release with the Windows portable exe and installer, built from the full #16→#20 stack plus PR #14 (release workflow, Export labels), with a README friends can follow.

## Problem and why
- The pilot wants a first version for friends to test before final changes (2026-10-07). Pilot decision: release branch plus tag, main untouched; tag `v0.1.0-beta.1`.
- PR #14 `feat/release-alpha` holds `release.yml`, Export labels and a README, but it is based on main and conflicts with the Label screen rewrite in 13 files.

## Scope
- Authorized: merging `feat/release-alpha` with its conflicts resolved, `README.md`, the `CLAUDE.md` release line, `bindings.ts` via `cargo xtask bindings`.
- Out of scope: merging any PR into main, code signing, auto-update, screenshots (the pilot adds them).

## Constraints
- The export uses the current gold export format (segment_label v2 selection, ADR 0021); `wolluf label export` and the screen write the same thing.
- ADR 0009: the export opener scope stays `$LOCALDATA/wolluf/**`.
- The tag starts with `v` + the workspace version (`release.yml` check).
- TDD: the merge keeps both sides' tests green; README is passive (readback against the code).

## Acceptance criteria
- Merge resolved, all `CLAUDE.md` gates green → gate block output.
- README claims match the app → readback by an independent checker.
- Tag pushed → `release` workflow green and a pre-release with two `.exe` assets.

## Tasks
- [x] T1 — Merge `feat/release-alpha` (release.yml, Export labels, CLAUDE.md release line) onto the stack. Route: workflow (writer + two review lenses). Tier: medium. Commit: Merge branch 'feat/release-alpha' into release/v0.1.0-beta.1
- [ ] T2 — README for the beta (en/es): what it does, download, first run, labelling, Progress, export, screenshot slots, the banner `docs/images/banner.png` (logo A1, Exo 2 800 italic wordmark, tagline, osu!web dark) at the top, and the 1280×640 `docs/images/social-preview.png` (uploaded by the pilot in Settings → Social preview); remove this document. Route: workflow (writer + fact-check). Tier: passive. Commit: —
- [x] T4 — Visible-only rename wolluf → wofella (pilot decision 2026-10-07): productName, window title, UI copy, release assets and title, README, brand images; crates, identifier `dev.wolluf.desktop`, data dir, env vars, CLI, localStorage keys and CI artifact names keep wolluf. Route: workflow (writer + README + audit). Tier: medium. Commit: feat: show the product as wofella
- [ ] T3 — Tag `v0.1.0-beta.1`, push branch and tag, check the release run and assets. Route: inline. Commit: —

## Progress
- 2026-10-07 T1 (workflow wf_d0ad19f9-f66): 13 conflicts resolved, stack behaviour kept; Export labels moved from the removed SessionFooter to the Playback settings flyout and the plan-finished panel, through the shared `export_lines` (screen file byte-identical to `wolluf label export` for 7K, tested). Gates: fmt, clippy (+all-features), check-layers, stage-lock, lint-canary, deny ok; nextest 1058 passed / 19 skipped; bindings stable; UI 71 files / 1025 tests. Two lenses: 0 confirmed findings. Spot check `cargo nextest run -p wolluf-app label_export`: 5 passed. Open (minor): if opening the folder fails after the file is written, the UI shows only the error.
- 2026-10-07 T4 (workflow wf_cbe84cb7-227): productName/title/publisher, startup dialog, UI copy en+es, release assets `wofella-<tag>-*`; tauri-cli 2.12 keeps `wolluf-desktop.exe` without `mainBinaryName`, NSIS becomes `wofella_<ver>_x64-setup.exe` (glob still matches). Audit: publisher fallback and the earlier-install README note fixed. Gates: fmt, clippy, nextest 1058 / 19 skipped, bindings stable, tsc, lint, UI 71 / 1025. Spot check `cargo nextest run -p wolluf-desktop fatal`: 1 passed. Not done (pilot's call): `mainBinaryName` (exe/process still wolluf-desktop.exe), CI portable `wolluf.exe`.

## Next step
T2: commit README + brand images (pending the pilot's name decision), then T3.
