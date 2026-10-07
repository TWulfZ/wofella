# 0009 IPC via tauri-specta, and the F0 desktop shell decisions

- Status: Accepted
- Date: 2026-09-28

## Context
The React UI talks to Rust only through Tauri commands and events (D14). A hand-maintained TypeScript mirror of the Rust DTOs drifts silently: a renamed field compiles on both sides and fails at runtime. The contract must be generated from the Rust types, committed, and checked for drift in CI (§8). Open decision O10 asked whether tauri-specta v2 is solid enough, or whether ts-rs plus hand-written wrappers is safer.

Facts checked 2026-09-28 (spec 005):
- tauri-specta and specta are at release candidate `2.0.0-rc.25` (2026-05-08), with nothing released since. Their event and typed-error APIs have shifted between RCs.
- tauri-specta rc.25 requires `tauri ^2`. Tauri 3 is at `3.0.0-alpha.3` (2026-09-26).
- JavaScript numbers lose precision above 2^53, and FILETIME values and online score ids exceed it (§8).

Spec 005 also made shell decisions that deviate from architecture §3 or extend §8. They are recorded here so that 005 T1 only verifies them.

## Decision
**Generator and pins.**
- tauri-specta generates `apps/desktop/ui/src/ipc/bindings.ts` (commands, DTOs and typed events).
- Pins: `specta =2.0.0-rc.25` (derive), `tauri-specta =2.0.0-rc.25` (features `derive`, `typescript`), `specta-typescript 0.0.12`, `tauri 2.12.0`, `tauri-build 2.7.0`. The RC pins use `=` because semver gives pre-releases no protection. Tauri 3 is excluded until tauri-specta supports it.

**The `export-bindings` contract.**
- `wolluf-desktop` has a bin target `export-bindings` that writes `apps/desktop/ui/src/ipc/bindings.ts` with LF line endings and a `// @ts-nocheck` + generated-file header.
- `cargo xtask bindings` runs `cargo run -p wolluf-desktop --bin export-bindings` when `cargo metadata` shows that target. Until then it prints a skip line and exits 0 (spec 001).
- The bin and the debug-build startup export call the same `export_bindings(path)` function, which uses the `tauri_specta::Builder` from `specta_builder()`. Its `collect_commands!` / `collect_events!` lists are append-only.
- The file is committed. CI regenerates it and runs `git diff --exit-code` on it.
- **BigInt export stays failing** (the specta-typescript default, or set explicitly), so any `i64`/`u64`/`usize` field in a DTO breaks the export instead of losing precision silently. Ids above 2^53 cross IPC as strings.

**Contract rules (§8, D11, D13, D14).**
- Commands are named `<feature>_<verb>`, are async, and return `Result<T, IpcError>`.
- DTOs live in `app::features::*::dto` and derive `specta::Type`. Domain types never derive specta.
- tauri-specta needs `tauri_specta::Event` on the payload type, and the orphan rule forbids implementing it on app types. The shell therefore declares `#[serde(transparent)]` wrappers (`JobProgress`, `JobFinished`, `DataChanged`) around the app DTOs.
- `invoke`/`listen` appear only in `ui/src/ipc/`.

**Fallback (closes O10).** tauri-specta v2 is the generator for F0. If a later RC or release regresses, the fallback is ts-rs for the types plus a hand-written command wrapper confined to `ui/src/ipc/`. The swap stays mechanical because commands are thin and uniformly named, and features only import `ipc/`.

**Shell decisions from spec 005.**
- **Opener instead of shell.** The desktop registers `tauri-plugin-dialog 2.8.0`, `tauri-plugin-opener 2.6.0` and `tauri-plugin-log 2.10.0`, not `tauri-plugin-shell`. This deviates from §3's "dialog, shell, log". Shell's `open` is deprecated in favour of the opener plugin, and F0 spawns no processes, so registering shell would only add execute permissions to the capability surface. Shell comes back when a sidecar exists (tosu, F5). The opener opens the logs folder in F0 and the `.osz` import in F4. Capabilities: `core:default`, `dialog:allow-open`, `log:default`, and `opener:allow-open-path` scoped to the data dir.
- **`tauri-plugin-log` is built with `skip_logger()`.** It only exposes the JS → Rust log command, and the global logger stays `tracing` (§7). Installing both would fail `log::set_logger`. The `tracing-log` bridge carries the UI's records into the same JSON log file.
- **`app_open_logs_dir`** is a new, shell-only command. It calls the opener's `open_path` on `<data>/logs`. It lives in the shell because it is a pure shell side effect with no app logic (D11). It opens a new `app_` group in the §8 command table.
- **TypeScript is held at 6.0.3**, not 7.x, because `typescript-eslint 8.71.0` declares the peer range `>=4.8.4 <6.1.0`. Revisit when that range widens.
- The architecture edits for these deviations (§3 plugin list, §8 `app_` group, CLI name) land at 005's close (005 T20).

## Alternatives considered
- **ts-rs plus hand-written command wrappers now.** Rejected for F0: it generates types but not the command and event surface, so the wrappers would be hand-maintained drift. It stays the documented fallback.
- **Hand-written TypeScript types.** Rejected: they cannot be drift-checked, and D14's typed contract becomes a convention.
- **Tauri 3 alpha.** Rejected: it is an alpha, and tauri-specta rc.25 requires `tauri ^2`.
- **`tauri-plugin-shell` as in §3.** Rejected for F0: it has a deprecated `open` and adds unneeded execute permissions (see above).
- **`tauri-plugin-log` as the global logger.** Rejected: `tracing` is the logger everywhere (§7), and two global loggers conflict.
- **TypeScript 7.** Rejected for now: the lint toolchain does not support it yet.

## Consequences
- The UI contract is generated and drift-checked from the first command. A DTO change that forgets `cargo xtask bindings` fails CI.
- wolluf depends on an RC. Spec 005 T10–T12 may need adjusting to rc.25 specifics, and the fallback cost is bounded by thin, uniformly named commands.
- 64-bit integers can never silently reach JavaScript.
- The capability surface in F0 is only dialog, log and a scoped opener. Adding shell later needs a reason (a sidecar) and an update to this ADR.

## Amendment 2026-10-07: opening osu! beatmap pages
- The capability gains `opener:allow-open-url` scoped to `https://osu.ppy.sh/*` only, so the Label screen's details dialog can open a map's beatmapset page (`/beatmapsets/{setId}#mania/{beatmapId}`) in the system browser. The UI calls it through `ui/src/ipc/opener.ts` like `openPath`; no other host is reachable and nothing is fetched by the webview.
