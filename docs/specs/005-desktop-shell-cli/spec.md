# 005 Desktop shell and CLI

Status: Draft
Phase: F0 · Owner: twulfz · Date: 2026-09-28
Links: architecture §3 (layout), §4 (D1, D9, D11, D12, D13, D14), §5.2 (data dir), §5.6 (identity UX), §7 (jobs, errors, logging), §8 (IPC + UI), §10 (UI tests, CI), §12 (F0), §13 O10; ADR 0009 (IPC via tauri-specta, authored by 001); research `00-plan-es.md` l.217 (WSL toolchain), `02-7k-bms-stable-verified.txt` l.182/214 (WSL2 cannot read osu! memory, Windows is the real target).
Sibling F0 specs: 001 workspace-foundation (workspace, pins, xtask `bindings` delegation, CI skeleton, core `ErrorCode`; assigns `AppError` and `SystemClock` to this spec), 002 osu-stable-codecs, 003 store-ledger-sync (`AppPaths`, `AppContext`, instance lock, `JobService`, `SyncPlays`, `AppEvent`), 004 players-identity (players service, DTOs, `commands/players.rs`, `features/players`, routes `setup.identity.tsx` / `settings.identity.tsx`), 006 osg-spike (`wolluf osg` subcommand under this spec's clap root).

## Problem
After 001–004 the backend can ingest plays and score identities, but nobody can use it: there is no window, no typed contract to the UI, no way to point wolluf at an osu! install, no visible progress for a 4.3k-play sync, and no CLI for harnesses and debugging. This spec builds the two thin shells over `wolluf-app` (the Tauri desktop and the `wolluf` CLI), the typed IPC contract with its drift check, the React skeleton every later feature slice plugs into, and the F0 screens: first-run install detection, the global job tray, the app shell that hosts 004's identity wizard and header controls, and settings. It also fixes the error contract end to end (`AppError` → `IpcError` → localized UI message), so every later feature reacts per error code instead of parsing prose.

## Scope
- In:
  - `wolluf-app::errors`: `AppError`, `ErrorCodeDto`, `IpcError`, `From<AppError> for IpcError`. 001 assigns `AppError` to this spec, and 003/004 build on it, so it is the first app task.
  - `wolluf-app::clock::SystemClock` (001 assigns it here). It is the single allowed `SystemTime::now` site.
  - `wolluf-app::logging`: tracing init shared by both shells.
  - `wolluf-app::features::setup`: install detection, set install path, setup status (service + DTOs).
  - `apps/desktop/src-tauri` (`wolluf-desktop`): the Tauri 2 app; thin commands for the `setup` and `jobs` groups plus `app_open_logs_dir`; the event bridge (`JobProgress`, `JobFinished`, `DataChanged`); the tauri-specta export (bin `export-bindings`, debug-startup export, drift test); the dialog, opener and log plugins.
  - `apps/desktop/ui`: a React 19 + Vite + TS strict project on pnpm, with TanStack Router (file routes) + Query, Tailwind v4 + shadcn/ui, i18n es/en, a Zustand job-tray store, eslint boundaries plus a ban on `@tauri-apps/*` outside `src/ipc/`, and vitest with a typed `mockIPC`.
  - F0 screens:
    - app shell: header, nav, error boundary, first-run guard, and slots where 004's ScopePicker, Merged/Compare toggle and NotSelfBanner are mounted;
    - `/setup/` (detect, browse, confirm install);
    - the global job tray;
    - `/` home status;
    - `/settings/` (language, data dir, open logs folder, version).
  - `apps/cli` (`wolluf-cli`, binary `wolluf`, clap):
    - subcommands `setup detect|set|status`, `sync`, `players list`, `jobs list`;
    - global flags `--data-dir`, `--json`, `--log`;
    - the root `Command` enum that 006 extends with `Osg`.
  - CI additions to 001's `ci.yml`: exact Node 24.15 / pnpm 11.17 versions in 001's guarded `ui` job, and a Windows NSIS bundle build job. (001 T14 already ships the `ui` job and the bindings drift step.)
  - Dev docs in `CLAUDE.md` Commands: the WSL dev loop and a Windows build note.
- Out (non-goals):
  - Everything in 004's players slice: service, heuristics, DTOs, `commands/players.rs`, `ui/src/features/players/*`, the route files `routes/setup.identity.tsx` and `routes/settings.identity.tsx`, and the ScopePicker/toggle/banner components.
  - Everything in 003: data-dir resolution (`AppPaths`), the instance lock, the data-dir-inside-osu guard, store, migrations, the `app::jobs` runtime, `JobService`, `SyncPlays`, the watcher and the `AppEvent` definitions. This spec consumes them (see Design, "Consumed interfaces").
  - The `wolluf osg` subcommand implementation (006).
  - Deferred to later phases:
    - failed-item drill-down, "Export diagnostics", updater, tosu `LiveState`;
    - `meta_*` and every other non-F0 command group in the §8 table;
    - Playwright smokes (§10 asks for "a few"; they start in F1, when there is a screen worth smoking);
    - ECharts, TanStack Table/Virtual (F1 library).
  - `tauri-plugin-shell` (see Design, "Plugins"; the deviation from §3 is handed to ADR 0009).
  - Code signing and `release.yml` (release spec).

## Behaviour
- **First run (desktop).**
  - On launch the root route loads `setup_status`:
    - no install configured → redirect to `/setup/`;
    - install configured but `identityReady = false` → redirect to 004's `/setup/identity`;
    - otherwise the requested route renders.
  - The guard runs in the root route's `beforeLoad` through `queryClient.ensureQueryData`, so the wrong screen never flashes.
- **`/setup/`.**
  - Calls `setup_detect_installs` and lists the candidates. Each shows its path, a source badge (002's `CandidateSource`: `env`, `registry`, `local_app_data`, `wsl_user_profile`, `drive_scan`, `program_files`), the `osu!.db` version and any missing files.
  - "Browse…" opens the native folder picker (dialog plugin, wrapped in `src/ipc/dialog.ts`).
  - Confirming calls `setup_set_install_path(path)`. On success:
    1. the UI calls `jobs_start` for `sync_plays` on the returned install;
    2. the job tray opens;
    3. the app navigates to `/setup/identity`, where 004 shows its waiting state until `DataChanged{domains:["players"]}` arrives.
- **Install discovery and validation** are 002's `source_osu::install` (single owner; 002 T12 absorbed this spec's former T2 at the F0 review). The setup service maps them 1:1:
  - `detect(&DetectEnv)` → one `InstallCandidateDto` per candidate, invalid ones included with `valid = false` and `missing`; the source badge is 002's `CandidateSource` (`env`, `registry`, `local_app_data`, `wsl_user_profile`, `drive_scan`, `program_files`). On the pilot the registry value `HKCR\osustable.File.osz\shell\open\command` is `"E:\Games\osu!\osu!.exe" "%1"` (verified 2026-09-28), read through `reg.exe` on WSL.
  - `validate_install(path)` errors map to: `InvalidInstall` → `OSU_DIR_NOT_FOUND` with `args.path`; `LazerInstall` → `UNSUPPORTED_FORMAT`, message key `setup.error.lazer_not_supported`. A missing `scores.db` is valid (reported in `missing`). A non-UTF-8 path → `INVALID_INPUT` (checked here, because DTOs carry paths as strings).
  - `osuDbVersion` is `InstallInfo.osu_db_version` (`20260924` on the pilot).
  - Registering the install goes through 003's `AppContext::register_install`, which also runs the data-dir-inside-osu guard (`INVALID_INPUT`, `error.data_dir_inside_osu`).
- **Setup status** returns `{install?, identityReady, dataDir, logsDir, appVersion, lastSync?}`.
  - `identityReady` comes from 004's players service (the negation of `wizardNeeded`; D12).
  - `lastSync` is the newest `sync_plays` entry from 003's `JobService::list`.
- **Job tray** (global, bottom-right, collapsible).
  - It hydrates from `jobs_list` on mount; after that, live state comes from events.
  - Each job shows its i18n kind name, stage, `done/total`, a progress bar, the ETA and a Cancel button (`jobs_cancel`).
  - On `JobFinished` it shows a status badge (003's `ok | failed | cancelled`). When `failedItems > 0` it also shows "N items failed" (no drill-down in F0).
  - Finished jobs stay listed until dismissed, or until 20 newer finished jobs push them out.
- **Events.**
  - `DataChanged{domains}` invalidates exactly the queries whose key starts with each domain.
  - If the Rust bridge's broadcast receiver lags, events have been dropped (003's bus holds 256). The bridge then emits `DataChanged{domains:["jobs"]}`, so the UI re-hydrates the tray from `jobs_list` instead of showing a stuck job.
- **Errors in the UI.** Every command result goes through `ipc/client.ts`:
  - success unwraps to the value;
  - an error throws `IpcFailure`, which holds an `IpcError` narrowed by `code`;
  - a rejection that is not a structured `IpcError` (unknown command, argument deserialization failure, plugin error) becomes `code: "INTERNAL"`, `messageKey: "error.code.INTERNAL"`, with the raw value in `details`.

  The UI shows `t(messageKey, args)` and falls back to `error.code.<CODE>` when the key is missing. It never shows prose built in Rust. `retryable = true` adds a Retry button.
- **Startup failure (desktop).** If `AppContext::open` fails (unwritable data dir, migration failure, `CONFLICT` because another wolluf process holds the lock), the shell shows a native blocking dialog with the error code, the message key and the logs path, then exits with code 1. It never starts a half-working UI.
- **Language.**
  - es/en. The initial language comes from `navigator.language` (`es*` → es, anything else → en) and can be changed in `/settings/`.
  - The choice is persisted in `localStorage`, wrapped in try/catch. It is a per-machine UI preference and moves to user.db `settings` once a settings command exists.
- **CLI** (`wolluf`). Every subcommand uses the same data dir and DTOs as the desktop.
  - `wolluf setup detect [--json]`: table of candidates. `wolluf setup set <PATH>`. `wolluf setup status [--json]`.
  - `wolluf sync [--json]`:
    - starts `SyncPlays` for the configured install;
    - prints throttled progress to stderr, only when stderr is a TTY;
    - waits for `JobFinished`, then prints the job summary (a JSON document on stdout with `--json`);
    - on Ctrl-C, cancels the job, waits for `JobFinished{cancelled}` and exits 130.
  - `wolluf players list [--json]`: aliases with stats, tier, decision and reasons (004's DTO).
  - `wolluf jobs list [--json] [--limit N]`: job history, newest first.
  - **Exit codes** (shared with 006):

    | Exit | Meaning |
    |---|---|
    | 0 | Success. A sync with failed items still exits 0 and prints the count. |
    | 1 | Environment or runtime failure (`OSU_RUNNING`, `CONFLICT`, `INTERNAL`, `SIGNATURE_INVALID`, `CONSENT_REQUIRED`) or a job that ended `failed`. |
    | 2 | Fix-your-input (clap usage errors, `INVALID_INPUT`, `NOT_FOUND`, `OSU_DIR_NOT_FOUND`, `PARSE_FAILED`, `UNSUPPORTED_FORMAT`). |
    | 130 | `CANCELLED`. |

  - Errors print to stderr as `error[<CODE>]: <messageKey> {k=v, …}`. The CLI is a dev tool and is not localized.
  - Global flags:
    - `--data-dir <DIR>` is handed to 003's `AppPaths` as an explicit override, taking precedence over `WOLLUF_DATA_DIR`;
    - `--log <FILTER>` (env `WOLLUF_LOG`) defaults to `warn` for the CLI; the desktop defaults to `info`.

## Domain rules
- Shells contain no logic. A command maps the DTO, calls one app function and maps the error. More than ~10 lines, or any branching on domain data, fails review (architecture §4 D11).
- DTOs crossing IPC live in `app::features::*::dto` and derive `specta::Type`; domain types never derive specta (D13).
  - `ErrorCode` lives in `wolluf-core` (001), so the wire enum is an app-level mirror, `ErrorCodeDto`, converted by an exhaustive `match`.
  - Convention for every DTO, following 004: `#[serde(rename_all = "camelCase")]`, and enum values as snake_case strings (`sync_plays`). The one exception is `ErrorCodeDto`, which is SCREAMING_SNAKE (§7).
- The UI talks to Rust only through the generated `ipc/bindings.ts`. `invoke`/`listen`, and any `@tauri-apps/*` import, are allowed only in `ui/src/ipc/`. Features import only `shared/`, `ipc/` and other features' `index.ts` (D14).
- A feature calls another feature only through its `pub` service (D12). Setup reads identity readiness through 004's `PlayersService` and job history through 003's `JobService`.
- `ErrorCode` is the closed stable list `OSU_DIR_NOT_FOUND, UNSUPPORTED_FORMAT, PARSE_FAILED, OSU_RUNNING, CONSENT_REQUIRED, SIGNATURE_INVALID, NOT_FOUND, INVALID_INPUT, CONFLICT, CANCELLED, INTERNAL`. The UI localizes from the message key and never receives prose built in Rust (§7).
- Commands (§8):
  - naming is `<feature>_<verb>`;
  - every command is async and returns `Result<T, IpcError>`;
  - ids above 2^53 are serialized as strings (`JobId` is a ULID string, 003);
  - long operations enqueue a job and return a `JobId` immediately;
  - progress arrives through global typed events, not per-call Channels.
- Events: `JobProgress {jobId, kind, stage, done, total, etaMs}` (≤ 10 Hz, throttled by 003), `JobFinished {jobId, status, failedItems}`, `DataChanged {domains[]}` (§8, 003).
- TanStack Query is the only cache of backend data. Keys are `[domain, ...args, scopeHash, manifestHash]` (§8). F0 has no manifest (the engine arrives in F1), so the key factory leaves out `manifestHash` until F1 adds it in one place.
- Scope, keymode and comparison targets live in the URL. Zustand holds only ephemeral cross-view UI state, such as the job tray (§8).
- The UI never computes domain values (§8).
- The osu! folder is read-only, and source-osu has no fs write calls (D9). `install` only stats paths and reads 4 bytes of `osu!.db`. The shells keep all state in the app data dir (§5.2).
- Logging (§7):
  - `tracing` everywhere;
  - JSON lines through `tracing-appender`, rolling daily, 14 files kept;
  - a pretty console in dev;
  - UI errors forwarded through `tauri-plugin-log`;
  - no analytics and no crash upload.
- `anyhow` only in the CLI and xtask; `thiserror` in libraries (§7, §11). 001's workspace lints deny `unwrap/expect/panic` in bins too.
- WSL2 cannot read osu! memory. F0 runs entirely from local files, and live E2E is a Windows build (research 02 l.182, l.214; CLAUDE.md Environment).

## Design
- **Crate edges.** All of these are already in 001's `layers.toml`: `wolluf-desktop` → `wolluf-app`, `wolluf-core`; `wolluf-cli` → `wolluf-app`, `wolluf-core`; `wolluf-app` → `wolluf-source-osu`. There is no new internal edge, so D1 needs no ADR.
- **Pins** (crates.io and npm, checked 2026-09-28).
  - Already in 001's table:
    - Tauri and IPC: `tauri 2.12.0`, `tauri-build 2.7.0`, `specta =2.0.0-rc.25` (derive), `tauri-specta =2.0.0-rc.25` (features `derive`, `typescript`), `specta-typescript 0.0.12`.
    - CLI and runtime: `clap 4.6.7` (derive, env), `anyhow 1.0.104`, `tokio 1.53.1`.
    - Logging: `tracing 0.1.44`, `tracing-subscriber 0.3.23` (json, env-filter), `tracing-appender 0.2.5`.
    - Other: `directories 6.0.0`, `serde_json 1.0.151`, `insta 1.48.0`, `tempfile 3.27.0`.
  - Also pinned by 001 T1 (collected at the F0 review): `tauri-plugin-dialog 2.8.0`, `tauri-plugin-opener 2.6.0`, `tauri-plugin-log 2.10.0`, `winreg 0.56.0` (used by 002's install), `assert_cmd 2.2.2`, `predicates 3.1.4`. Feature `test` on `tauri` goes in the desktop dev-dependencies only.
  - Tauri 3 (`3.0.0-alpha.3`, 2026-09-26) is excluded: tauri-specta rc.25 requires `tauri ^2`.
  - Tooling:
    - `tauri-cli 2.12.0`, installed with `cargo install tauri-cli --version 2.12.0 --locked` and run from `apps/desktop/src-tauri`, where `tauri.conf.json` lives, so no CLI path discovery is involved;
    - Node `24.15.0` (installed; jsdom 30 requires `^24.15.0`);
    - pnpm `11.17.0`, declared through `packageManager`.
  - npm: exact versions (no `^`), with the lockfile committed.
    - App: `react`/`react-dom 19.3.0`, `@tanstack/react-router 1.170.40`, `@tanstack/react-query 5.104.0`, `i18next 26.4.2`, `react-i18next 17.0.15`, `zustand 5.0.15`.
    - Tauri JS, matching the Rust crate versions: `@tauri-apps/api 2.12.0`, `@tauri-apps/plugin-dialog 2.8.0`, `@tauri-apps/plugin-log 2.10.0`, `@tauri-apps/plugin-opener 2.6.0`.
    - Build: `vite 8.3.1`, `@vitejs/plugin-react 6.1.1`, `@tanstack/router-plugin 1.168.41`, `tailwindcss`/`@tailwindcss/vite 4.3.3`, `shadcn 4.21.0` (dev CLI), `@tanstack/react-query-devtools 5.104.0` (dev only).
    - **`typescript 6.0.3`**, not 7.0.2: `typescript-eslint 8.71.0` declares `typescript >=4.8.4 <6.1.0`.
    - Tests: `vitest 5.0.2`, `jsdom 30.1.1`, `@testing-library/react 16.3.3`, `@testing-library/dom` 10.x, `@testing-library/user-event 14.6.7`, `@testing-library/jest-dom 7.0.1`.
    - Lint: `eslint 10.11.0`, `typescript-eslint 8.71.0`, `eslint-plugin-boundaries 7.2.0`, `eslint-plugin-react-hooks 7.1.1`.
- **`wolluf-app::errors`.**
  - `AppError { code: ErrorCode, message_key: Cow<'static, str>, args: BTreeMap<String, String>, details: Option<String>, retryable: bool }`.
    - It derives `thiserror::Error`, with Display = `code: message_key`.
    - It has constructor helpers per code (`AppError::osu_dir_not_found(path)`, …) and `From<source_osu::SourceError>` (via its `code()`). 002/003 add `From` impls for their own error enums in their crates' app-side modules; an unknown DB version maps to `UNSUPPORTED_FORMAT`.
    - `args` holds strings only, in a `BTreeMap`, so the wire order is deterministic and no number can exceed 2^53.
    - Message keys are full dotted i18n keys owned by the emitting slice (`error.instance_running` 003, `players.error.*` 004, `setup.error.*` here). The generic fallback per code is `error.code.<CODE>`.
  - `ErrorCodeDto` has the same variants, derives `Serialize` + `specta::Type`, and uses `rename_all = "SCREAMING_SNAKE_CASE"`. `From<ErrorCode>` is an exhaustive `match`, so a new core code does not compile until it is mirrored.
  - `IpcError { code: ErrorCodeDto, message_key, args, details: Option<String>, retryable }` derives `specta::Type` and serializes as camelCase.
    - `IpcError::from_app(e, include_details: bool)`.
    - `From<AppError>` passes `cfg!(debug_assertions)`, so release builds never send `details` (it can hold local paths; §7 scrubbing). The full error is logged instead.
  - TS side (`ipc/client.ts`): `type IpcError = { [C in ErrorCode]: Omit<RawIpcError, "code"> & { code: C } }[ErrorCode]`. This is a true discriminated union derived from the generated type, at zero runtime cost.
- **`wolluf-app::clock::SystemClock`** implements core's `Clock`. It is the single `#[allow(clippy::disallowed_methods)]` site for `SystemTime::now` (001).
- **`wolluf-app::logging`**: `init(LogOptions { filter, json_dir: Option<PathBuf>, console: bool }) -> Result<LogGuard, AppError>`.
  - The JSON file layer uses `tracing_appender::rolling::Builder` (daily, `max_log_files(14)`, prefix `wolluf`, suffix `jsonl`) in `<data>/logs`.
  - A pretty console layer is added when `console` is set.
  - The `tracing-log` bridge in `tracing-subscriber` captures `log` records. That is how `tauri-plugin-log` records (UI logs) land in the same file.
- **`wolluf-app::features::setup`** (`mod.rs` with `pub struct SetupService`, plus `dto.rs`):
  - `detect_installs() -> Vec<InstallCandidateDto>`.
  - `set_install_path(path: String) -> InstallDto` validates, registers through 003's install registration (which runs the data-dir guard and persists `game_install`) and emits `DataChanged{["setup"]}`.
  - `status() -> SetupStatusDto`.
  - DTOs:
    - `InstallCandidateDto { path, source: InstallSourceDto, valid, osuDbVersion: Option<i32>, missing: Vec<String> }`, where `InstallSourceDto` mirrors 002's `CandidateSource` through an exhaustive `match`;
    - `InstallDto { id, rootPath, osuDbVersion: Option<i32>, detectedAt: String /* RFC 3339 */ }`, where `id` is a `u32` or a string, never `i64`, because the bigint export fails on it;
    - `SetupStatusDto { install: Option<InstallDto>, identityReady: bool, dataDir: String, logsDir: String, appVersion: String, lastSync: Option<JobDto> }`.
  - Architecture §3 lists the app features without `setup`, although §8 has the `setup` command group. The close task adds `setup` to the §3 list.
- **`wolluf-source-osu::install`** is 002's (see Behaviour). This spec adds no code to `wolluf-source-osu`.
- **`apps/desktop/src-tauri` (`wolluf-desktop`).**
  - **Layout:**
    - `src/main.rs` calls `wolluf_desktop::run()`;
    - `src/lib.rs` holds `run` and `specta_builder()`;
    - `src/commands/{mod.rs, setup.rs, jobs.rs, app.rs}`; 004 adds `players.rs`;
    - `src/events.rs`;
    - `src/error.rs` holds `fn to_ipc(e: AppError) -> IpcError`, which logs at `warn` inside the command span and then converts;
    - `src/bindings.rs`, `src/bin/export_bindings.rs`, `build.rs`, `tauri.conf.json`, `capabilities/default.json`, `icons/`.
  - **Runtime.** `main` builds one multi-thread tokio runtime and calls `tauri::async_runtime::set(handle)` before the builder, so Tauri commands and `wolluf-app` share a single runtime. Two runtimes would split the blocking pools and make shutdown order fragile.
  - **`setup` hook:**
    1. `AppPaths::resolve(None)` (003);
    2. `logging::init` (file, plus console in debug);
    3. `AppContext::open(paths, SystemClock)` (003);
    4. `manage(Arc<AppContext>)`;
    5. spawn the event bridge;
    6. in debug builds, export the bindings (§8).

    If `open` fails, show a blocking native dialog and exit 1.
  - **Commands** (each ≤ 10 lines):
    - `setup_detect_installs() -> Vec<InstallCandidateDto>`;
    - `setup_set_install_path(path: String) -> InstallDto`;
    - `setup_status() -> SetupStatusDto`;
    - `jobs_list() -> Vec<JobDto>`;
    - `jobs_start(request: JobStartDto) -> JobId`, which maps 1:1 to `JobService::start(kind, params)` (003 owns the DTO);
    - `jobs_cancel(id: JobId) -> ()`;
    - `app_open_logs_dir() -> ()`: the opener plugin's `open_path` on `<data>/logs`. It lives in the shell because it is a pure shell side effect with no app logic.
  - **Events** (`events.rs`).
    - tauri-specta requires `tauri_specta::Event` on the payload type, and the orphan rule forbids implementing it on app types. The shell therefore declares `#[serde(transparent)]` wrappers `JobProgress(JobProgressDto)`, `JobFinished(JobFinishedDto)` and `DataChanged(DataChangedDto)`. Their wire names become `job-progress`, `job-finished` and `data-changed`.
    - The bridge task loops on the `AppEvent` broadcast receiver from `AppContext`, matches the variant and emits the wrapper.
    - `RecvError::Lagged(n)` → warn and emit `DataChanged{["jobs"]}`. `Closed` → exit the loop.
  - **Bindings.**
    - `specta_builder()` returns the `tauri_specta::Builder` with `collect_commands!` and `collect_events!`. The list is append-only, and 004 appends its `players_*` commands.
    - `export_bindings(path)` writes with `specta_typescript::Typescript`. The header is `// @ts-nocheck` plus a generated-file notice.
    - BigInt export is left at "fail", or set to it explicitly, so any `i64/u64/usize` field in a DTO breaks the export instead of silently losing precision.
    - The `export-bindings` bin (called by 001's `cargo xtask bindings`) and the debug-startup export call the same function.
    - Output: `apps/desktop/ui/src/ipc/bindings.ts`, LF line endings (001's `.gitattributes`).
  - **Plugins.**
    - `tauri-plugin-dialog`: the folder picker.
    - `tauri-plugin-opener`: open the logs folder now; `.osz` import in F4.
    - `tauri-plugin-log`, built with `skip_logger()`. It only exposes the JS → Rust log command, and the global logger stays `tracing`; installing both would fail `log::set_logger`.
    - **Deviation from §3's "dialog, shell, log".** `tauri-plugin-shell`'s `open` is deprecated in favour of `tauri-plugin-opener`, and F0 spawns no processes. Registering shell would only add execute permissions to the capability surface. Shell comes back when a sidecar exists (tosu, F5). This deviation is handed to 001's ADR 0009 (T1), and §3 is updated at close.
  - **`tauri.conf.json`:**
    - `productName "wolluf"`, `identifier "dev.wolluf.desktop"` (it must not end in `.app`);
    - `build.devUrl "http://localhost:1420"`, `build.frontendDist "../ui/dist"`;
    - `build.beforeDevCommand {"script": "pnpm dev", "cwd": "../ui"}`, `build.beforeBuildCommand {"script": "pnpm build", "cwd": "../ui"}`;
    - one window, 1280×800, min 1024×680;
    - CSP `default-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; connect-src ipc: http://ipc.localhost`;
    - `bundle.targets ["nsis"]`.
  - **Capabilities:** `core:default`, `dialog:allow-open`, `log:default`, and `opener:allow-open-path` scoped to the data dir.
- **`apps/desktop/ui`.**
  - **Project.** A standalone pnpm project (own `package.json` and `pnpm-lock.yaml`; `pnpm-workspace.yaml` only for pnpm 11 build-script approvals), matching the `pnpm -C apps/desktop/ui …` gate commands. Scripts: `dev`, `build` (`tsc --noEmit && vite build`), `lint`, `test` (`vitest run`), `typecheck`.
  - **Vite:**
    - port 1420 with `strictPort`, `clearScreen: false`, and a watch ignore on `../src-tauri/**`;
    - plugins `@tanstack/router-plugin/vite` (before react), `@vitejs/plugin-react`, `@tailwindcss/vite`;
    - alias `@/` → `src/`.

    `src/routeTree.gen.ts` is committed so `tsc` works without running Vite. eslint ignores it and `bindings.ts`.
  - **tsconfig:** `strict`, `noUncheckedIndexedAccess`, `exactOptionalPropertyTypes`, `noUnusedLocals`, `noUnusedParameters`, `verbatimModuleSyntax`, `moduleResolution "bundler"`.
  - **Layout (§8):**
    - `src/app/`: providers, `queryClient`, router, `ErrorBoundary`, `nav.ts`, `styles.css`;
    - `src/ipc/`: `bindings.ts` (generated), `client.ts`, `eventBridge.ts`, `dialog.ts`, `opener.ts`, `log.ts`, `mocks.ts`;
    - `src/shared/`: `ui/` (shadcn components), `i18n/`, `queryKeys.ts`, formatters;
    - `src/features/{setup,jobs}/`: components, a `queries.ts` key factory, `index.ts`;
    - `src/routes/`, using TanStack flat file naming: `__root.tsx`, `index.tsx`, `setup.index.tsx`, `settings.index.tsx`.
    - `setup.index.tsx` and `settings.index.tsx` are deliberately `.index` leaves. No `setup.tsx`/`settings.tsx` layout exists, so 004's `setup.identity.tsx` and `settings.identity.tsx` are siblings and not children rendered inside an Outlet.
  - **Styling.** Tailwind v4 CSS-first: `@import "tailwindcss"` plus `@theme` tokens in `styles.css`, with the dark variant following `prefers-color-scheme`.
  - **Components.** shadcn/ui, Radix flavour, through `shadcn init`, with `components.json` pointing at `src/shared/ui`. F0 components: button, card, badge, progress, dialog, checkbox, table, tooltip, scroll-area, separator, sonner.
  - **Router.** The root route's `validateSearch` owns the global search params and delegates to a validator exported by `features/players/index.ts` (004: `scope`, `keymode`, `merge`). Validators are hand-written; there is no schema library. The root layout mounts 004's ScopePicker, MergeCompareToggle and NotSelfBanner in the header.
  - **Query.** One `QueryClient`:
    - `staleTime: Infinity` for backend data, because invalidation is event-driven, not time-driven;
    - `retry: false`, because retryable errors are shown to the user.

    Key factory: `qk(domain, ...args)` in `shared/queryKeys.ts`.
  - **`ipc/client.ts`:** `call<T>(p: Promise<Result<T, RawIpcError>>): Promise<T>`, `class IpcFailure extends Error { error: IpcError }`, the type guard `isIpcError(e, code?)`, and `toIpcError(unknown)` (INTERNAL synthesis). Feature code calls `call(commands.setupStatus())`.
  - **`ipc/eventBridge.ts`:** `startEventBridge(queryClient, trayStore) → unlisten`, using the generated `events.*.listen`.
    - `DataChanged` → `invalidateQueries({ queryKey: [domain] })` per domain;
    - `JobProgress` and `JobFinished` → tray store.
  - **`ipc/mocks.ts`** (test-only).
    - `mockCommands(handlers)`: handlers are keyed by the generated camelCase command names, and each handler's return type is the command's success type.
      - Errors are thrown with `mockIpcError(code, args?)`. It is a plain object, so tauri-specta's wrapper yields `{status:"error"}`.
      - At runtime it converts camelCase → snake_case, which is valid because every command is `<feature>_<verb>`.
      - It is built on `mockIPC` from `@tauri-apps/api/mocks`, with `shouldMockEvents: true`.
    - `emitMockEvent(name, payload)` is typed from the generated events.
    - Argument objects are typed loosely (`Record<string, unknown>`). Return types are the part that catches contract drift.
  - **Job tray store** (`features/jobs/store.ts`, Zustand): `Map<jobId, TrayJob>`, an `open` flag, `applyProgress`, `applyFinished`, `hydrate(JobDto[])`, `dismiss`; at most 20 finished jobs are kept.
  - **eslint** (flat config):
    - `typescript-eslint` strict-type-checked and `react-hooks`;
    - `no-restricted-imports` banning `@tauri-apps/*` everywhere except `src/ipc/**`;
    - `eslint-plugin-boundaries` with elements `app`, `ipc`, `shared`, `feature` (captures the slice name) and `route`, and the D14 allow matrix:
      - feature → shared, ipc, and another feature's `index.ts` only;
      - shared → shared;
      - route → feature index, shared, ipc;
      - app → everything.
    - A lint self-test (`src/__lint__/lint-rules.test.ts`) runs ESLint's Node API on fixture files under `lint-fixtures/` and asserts that the expected rule ids fire, so a config refactor cannot silently disable the rules.
  - **i18n.** i18next + react-i18next with bundled resources (no HTTP backend) and a single `translation` namespace.
    - It is assembled from per-domain files `src/shared/i18n/locales/<lng>/<domain>.json`, whose top-level key is the domain (`error`, `common`, `setup`, `jobs`, `settings`; 004 adds `players`).
    - Interpolation uses `{{arg}}`.
    - This spec writes `error.json` in full: the `error.code.<CODE>` fallbacks for all 11 codes, plus the keys named by 003 (`error.instance_running`, `error.data_dir_inside_osu`).
  - **Vitest:** `environment: "jsdom"`, `setupFiles` registering jest-dom, and `clearMocks()` from `@tauri-apps/api/mocks` after each test.
- **`apps/cli` (`wolluf-cli`, bin `wolluf`).**
  - `src/main.rs`: `fn main() -> anyhow::Result<ExitCode>` with `#[tokio::main]`.
  - `src/cli.rs`: clap derive, `Cli { global flags, command: Command }` and `enum Command { Setup(SetupCmd), Sync(SyncArgs), Players(PlayersCmd), Jobs(JobsCmd) }`. 006 adds `Osg(OsgCmd)`.
  - `src/cmd/{setup,sync,players,jobs}.rs`: each calls one app service and renders the result.
  - `src/render.rs`: plain-text tables with fixed column widths, and JSON through `serde_json` of the same DTOs.
  - `src/progress.rs`: rewrites a stderr line when stderr is a TTY (`std::io::IsTerminal`); no progress-bar crate.
  - `src/exit.rs`: maps `ErrorCode` and job status to an `ExitCode` (table in Behaviour). 006 reuses it.
  - `sync` uses `JobService::start` plus the event receiver, so it can show progress and cancel. 003's `sync_blocking` is used only if it exposes both.
  - Logging goes through the same `logging::init` as the desktop: console to stderr, and a JSON file too, so CLI runs appear in the same logs.
- **Consumed interfaces** (built elsewhere; the tasks that need them are marked):
  - **003:**
    - `AppPaths::resolve(override_dir: Option<PathBuf>)` and `AppPaths::logs_dir` (003 T13).
    - `AppContext::open(paths, clock)`, including the `CONFLICT` instance lock.
    - The `AppEvent` broadcast `subscribe()`.
    - `JobService::{start, list, cancel}` with `JobKindDto`, `JobStartDto` (or kind + params), `JobDto`, `JobId(String)`, `JobProgressDto`, `JobFinishedDto`, `DataChangedDto`.
    - `AppContext::register_install` with the data-dir guard.
    - A synthetic fixture install for the CLI sync tests.
  - **004:**
    - `PlayersService::{wizard_needed() -> bool, list_aliases() -> AliasListDto}` (004 T8);
    - `features/players/index.ts` exporting `validateGlobalSearch`, `ScopePicker`, `MergeCompareToggle` and `NotSelfBanner` (004 T13);
    - its route files `setup.identity.tsx` and `settings.identity.tsx`.
  - **002:** `install::{DetectEnv, detect, validate_install, CandidateSource, InstallInfo}` and `SourceError::code()`.
- Versioned stages affected: none. Param-pack sections: none.

## Data
- user.db migration: no. `game_install` is 003's; setup writes only through 003's registration.
- cache.db change: no. Job history is 003's `job_run`.
- Vault or blob changes: no.
- New on-disk artifacts:
  - `<data>/logs/wolluf.<date>.jsonl`, 14 files kept;
  - `localStorage["wolluf.lang"]` in the webview.

## IPC / UI
- Commands owned by this spec: `setup_detect_installs`, `setup_set_install_path(path)`, `setup_status`, `jobs_list`, `jobs_start(request)`, `jobs_cancel(id)`, `app_open_logs_dir`. The `app_` group is new relative to the §8 table (a shell-only side effect) and is added to the §8 table at close.
- Commands registered in the same builder by 004: the `players_*` group.
- DTOs:
  - owned here: `IpcError`, `ErrorCodeDto`, `InstallCandidateDto`, `InstallSourceDto`, `InstallDto`, `SetupStatusDto`;
  - consumed from 003: `JobDto`, `JobKindDto`, `JobStartDto`, `JobId`, `JobProgressDto`, `JobFinishedDto`, `DataChangedDto`.
- Events: `job-progress`, `job-finished`, `data-changed`.
- Routes:
  - `/`: home status (install path, last sync summary, links);
  - `/setup/`;
  - `/settings/`: language, data dir, logs folder button, app version, and a link to 004's `/settings/identity`;
  - 004's `/setup/identity` and `/settings/identity` are reached through the guard and the nav.

  The nav registry is `src/app/nav.ts`, with one entry per top-level route.
- Query keys: `["setup", "status"]`, `["setup", "candidates"]`, `["jobs", "list"]`.

## Acceptance criteria
- [ ] AC1: Install discovery and validation are correct and read-only → owned by 002 AC10 (incl. env first, registry open-command parsing, file → parent, lazer, missing scores.db, 4-byte version read, dedupe) and 002 AC12 (`check-layers`, no write API).
- [ ] AC2: The pilot install validates → owned by 002 AC14 `detect_finds_corpus_install` (valid, `osu_db_version == 20260924`, nothing written).
- [ ] AC3: Error mapping → `cargo nextest run -p wolluf-app errors` passes `errors::tests::{error_code_dto_mirrors_core_all, ipc_error_json_per_code, details_stripped_without_flag, args_serialize_sorted}`. The insta snapshot shows all 11 codes as SCREAMING_SNAKE strings and camelCase field names.
- [ ] AC4: Clock and logging → `clock::tests::system_clock_is_after_2026_09_01` and `logging::tests::writes_json_line_to_dir` pass.
- [ ] AC5: Setup service → `cargo nextest run -p wolluf-app setup` passes `features::setup::tests::{set_valid_path_persists_and_emits_setup_changed, set_invalid_path_returns_osu_dir_not_found_with_path_arg, set_lazer_returns_unsupported_format, status_without_install, status_identity_ready_follows_players_service}`. The tests use in-memory SQLite with 003's real migrations and temp fake installs.
- [ ] AC6: Commands are wired and thin → `cargo nextest run -p wolluf-desktop` passes `commands_smoke::{setup_status_ok, setup_set_install_path_error_has_code, jobs_start_returns_string_id}` through `tauri::test::mock_builder` + `get_ipc_response`. Every `#[tauri::command]` body is ≤ 10 lines (a PR review checklist item).
- [ ] AC7: Event bridge → `event_bridge::{forwards_job_progress_payload, forwards_data_changed, lagged_receiver_emits_jobs_data_changed}` pass in `wolluf-desktop`.
- [ ] AC8: Bindings are generated, deterministic and precision-safe:
  - `cargo xtask bindings && git diff --exit-code apps/desktop/ui/src/ipc/bindings.ts` exits 0;
  - `wolluf-desktop` tests `bindings::{committed_file_is_up_to_date, export_is_deterministic, bigint_field_fails_export}` pass.
- [ ] AC9: UI contract client → `pnpm -C apps/desktop/ui exec vitest run --typecheck src/ipc` passes:
  - `client.test.ts`: unwraps ok; narrows `OSU_RUNNING`; a non-structured rejection → `INTERNAL` with `details`;
  - `eventBridge.test.ts`: `DataChanged{players}` invalidates `["players", …]` and not `["setup", …]`; progress and finished events reach the tray store;
  - `mocks.test-d.ts`: a handler returning the wrong type fails the typecheck.
- [ ] AC10: Boundaries hold → `pnpm -C apps/desktop/ui lint` exits 0, and `src/__lint__/lint-rules.test.ts` asserts that:
  - an `@tauri-apps/api/core` import in a feature → `no-restricted-imports`;
  - a feature importing another feature's non-index file → a `boundaries/*` violation;
  - `shared` importing a feature → a violation.
- [ ] AC11: Localization is complete:
  - `src/shared/i18n/i18n.test.ts` passes (en/es key sets identical per domain file; no empty strings);
  - the Rust `wolluf-desktop` test `i18n_error_keys::every_error_code_has_en_and_es_fallback` passes. It reads `ui/src/shared/i18n/locales/*/error.json` and checks `error.code.<CODE>` for every `ErrorCode::ALL`.
- [ ] AC12: First-run guard → `src/routes/__root.test.tsx`: `install: null` → location `/setup/`; install set and `identityReady: false` → `/setup/identity`; both set → the requested route.
- [ ] AC13: Setup screen → `src/features/setup/SetupScreen.test.tsx`:
  - candidates render with source badges and missing files;
  - an invalid confirm shows the `error.code.OSU_DIR_NOT_FOUND` text with the path;
  - lazer shows `setup.error.lazer_not_supported`;
  - a valid confirm calls `setup_set_install_path`, then `jobs_start` (`sync_plays`), in that order, and navigates to `/setup/identity`;
  - Browse uses the mocked dialog result.
- [ ] AC14: Job tray → `src/features/jobs/JobTray.test.tsx`:
  - it hydrates running jobs from `jobs_list`;
  - `job-progress` events update `done/total`;
  - Cancel calls `jobs_cancel(id)`;
  - `job-finished` with `failedItems: 37` shows "37 items failed" in en and the es string in es;
  - the 21st finished job evicts the oldest.
- [ ] AC15: UI gates → `pnpm -C apps/desktop/ui exec tsc --noEmit && pnpm -C apps/desktop/ui lint && pnpm -C apps/desktop/ui test` exits 0, and `pnpm -C apps/desktop/ui build` produces `dist/index.html`.
- [ ] AC16: CLI → `cargo nextest run -p wolluf-cli` passes:
  - `cli_setup::{detect_json_lists_env_candidate, set_then_status_json}`;
  - `cli_errors::{sync_without_install_exits_2_osu_dir_not_found, bad_flag_exits_2, locked_data_dir_exits_1_conflict}`;
  - `cli_sync::{first_sync_reports_new_plays, second_sync_adds_zero}`, over 003's synthetic fixture install;
  - `cli_players::list_json_shape`;
  - `cli_jobs::list_shows_finished_sync`.

  All of them use `assert_cmd` with a temp `--data-dir`.
- [ ] AC17: The WSL dev loop works → the T20 manual checklist is recorded as done:
  - `cargo tauri dev` from `apps/desktop/src-tauri` opens the window on WSLg;
  - `/setup/` lists `/mnt/e/Games/osu!` with source `registry` (read through `reg.exe`; `drive_scan` if interop is disabled);
  - confirming starts a sync whose progress shows in the tray, then lands on `/setup/identity`.
- [ ] AC18: Windows build → CI job `desktop-windows` (windows-2025) runs `cargo tauri build --bundles nsis` and uploads the installer artifact. A manual smoke on the pilot machine records that `/setup/` shows `E:\Games\osu!` with source `registry`.
- [ ] AC19: Every `wolluf-sdd` §4 gate that exists runs clean: fmt, clippy `-D warnings`, check-layers, nextest, bindings drift, tsc/lint/vitest.

## Risks / open questions
- **tauri-specta is an RC** (`=2.0.0-rc.25`, 2026-05-08, nothing released since). Its API for events and typed errors has shifted between RCs, so T10–T12 may need adjusting to rc.25 specifics. The fallback stays §8/O10 (ts-rs plus a hand-written wrapper confined to `ipc/`). That swap is cheap because commands are thin and uniformly named.
- **BigInt default in specta-typescript 0.0.12** is assumed to fail on 64-bit integers. `bigint_field_fails_export` proves it; if the default has changed, the builder sets it explicitly.
- **`mockIPC(..., { shouldMockEvents: true })`** is assumed to exist in `@tauri-apps/api 2.12`. If it does not, `startEventBridge` takes an injectable `listen` and the tests pass a fake.
- **`tauri::generate_context!` and `frontendDist`.** In non-`custom-protocol` builds (plain `cargo build`/`nextest`) assets are not embedded, so Rust tests should not need `ui/dist`. If they do, the CI Rust job runs `pnpm -C apps/desktop/ui build` first, and T9 records it.
- **WSLg rendering.** webkit2gtk under WSLg can show a blank window. The documented workaround is `WEBKIT_DISABLE_DMABUF_RENDERER=1`; it goes into the dev notes, not the code.
- **Cross-spec items settled at the F0 review:** `AppError` is this spec's T3 and lands before 003 T13 and 004's service tasks; 003 adds `AppPaths::resolve(override_dir)`, `logs_dir`, `register_install` and camelCase DTOs (incl. `JobStartDto`); 004 exposes `wizard_needed()` and exports `validateGlobalSearch`; install discovery is 002's; CLI exit codes follow the Behaviour table (006 uses the same); this spec writes `error.json` incl. 003's keys and 004 writes `players.json`; `collect_commands!` in `lib.rs` is an append-only edit point shared with 004 (004 T11 runs after T12).
- **§3 plugin list deviation** (opener instead of shell) and the new `app_open_logs_dir` command are recorded in ADR 0009 by 001 T24; T1 here only verifies (and amends if something is missing).
- **Architecture naming inconsistency.** §3 names the CLI `wolluf`, while §6.5/§9.4 write `wolluf-cli eval`. This spec pins package `wolluf-cli`, binary `wolluf` (as 001 does), and the close task fixes the §6.5/§9.4 text.
- **TypeScript 7** is held back by `typescript-eslint`'s peer range. Revisit when that range widens.
- Registry discovery only finds installs made by the official installer, which registers the `.osz` handler. Portable copies are found through Browse or `WOLLUF_OSU_DIR`.

## Deviations (filled at close)
