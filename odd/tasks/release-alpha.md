# Alpha release pipeline and label export

Branch `feat/release-alpha` from `main` @ 3189970 · opened 2026-10-06

## Objective
Pushing a `v*` tag publishes a GitHub pre-release with the portable `wolluf.exe` and the NSIS installer, and the Label screen can export the gold labels to a file a tester can send back.

## Problem and why
The pilot confirmed the Label screen on Windows (2026-10-06) and wants testers to download it easily. CI artifacts expire after 14 days and need a GitHub login. Testers' labels stay in their local `user.db`, and export only exists in the CLI (`wolluf label export`).

Rejected:
- Tauri's updater plugin now: it needs a new plugin, a capability and a signing key (ADR 0009), and it does not cover the portable exe. Deferred until there are real users.
- A "Save as…" dialog: it needs `dialog:allow-save`, a capability change. Writing into `<data dir>/exports/` and opening that folder uses the opener scope that already exists.

## Scope
- **Authorized:**
  - `.github/workflows/release.yml`;
  - `crates/app` labeling export;
  - `apps/desktop/src-tauri` commands `label_export` and `app_open_exports_dir`;
  - `apps/desktop/ui` Label screen button;
  - CLAUDE.md, README.
- **Out of scope:**
  - auto-update, code signing;
  - uploading labels to a server (mania-tracker integration).

## Constraints
- No new capability or CSP change (ADR 0009): the opener scope `$LOCALDATA/wolluf/**` already covers `<data dir>/exports/`.
- The export never writes into an osu! install (D9; `export_to` already refuses).
- Release builds run only on tags. The version comes from the workspace `Cargo.toml` (`0.1.0`), and the tag must start with `v<version>`. A `-` suffix makes it a pre-release.
- TDD: strict. Runners: `cargo nextest run -p wolluf-app label_export`, `-p wolluf-desktop`, `pnpm -C apps/desktop/ui test label`.
- Delivery: about 300 authored lines, one PR.

## Acceptance criteria
- `label_export` writes `<data dir>/exports/gold-7k-<UTC timestamp>.jsonl` with the self profile's labels and returns its path and count → `cargo nextest run -p wolluf-app label_export`, desktop smoke.
- The Label screen's "Export labels" button exports and opens the folder; es and en → `pnpm -C apps/desktop/ui test label`.
- A tag `v0.1.0-alpha.1` builds and creates a pre-release with `wolluf.exe` and the installer; a tag not matching the version fails → first tag run (the pilot approves the tag).

## Tasks
- [ ] T1 — Label export to the data dir + open folder (app, commands, UI button). Route: delegated (writer). Tier: medium. Commit: —
- [ ] T2 — Release workflow on `v*` tags (version check, build, portable + installer, pre-release). Route: inline. Tier: medium. Commit: —
- [ ] T3 — Close: docs (CLAUDE.md release steps, README download), full gates, remove this document; then the pilot approves pushing the first tag. Tier: passive. Commit: —

## Progress

## Next step
T1 writer and T2 inline in parallel.
