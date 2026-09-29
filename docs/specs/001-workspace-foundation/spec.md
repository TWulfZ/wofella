# 001 Workspace foundation

Status: Draft
Phase: F0 · Owner: twulfz · Date: 2026-09-28
Links: architecture §3, §4 (D1–D3, D6, D9, D16, D17), §5.1, §5.5, §7 (errors), §10 (CI), §11, §12 (F0); ADRs 0001–0009 (written by this spec's tasks); research `03-maniahub-rejudge-drills-sessions-audit.txt` l.161/168 (Data/r FILETIME = ticks − offset), `01-landscape-verified.txt` l.63/76/106/119 (licences: slider LGPL, tosu LGPL, gosumemory GPL, Quaver MPL, prelude MIT).
Sibling F0 specs: 002 osu-stable-codecs, 003 store-ledger-sync, 004 players-identity, 005 desktop-shell-cli, 006 osg-spike. They all build on this one.

## Problem
Nothing exists yet but docs. Every other F0 spec needs a compiling Cargo workspace, a single place where dependency versions are pinned, the shared `wolluf-core` vocabulary (ids, time, keymode, version keys, error codes, clock), and the machinery that keeps the architecture honest from the first commit: `cargo xtask check-layers` (D1, D2, D6, D9), lint policy (unwrap/panic, D3 determinism), licence gate (D16) and CI on the real target (Windows) plus the dev host (Linux/WSL). If this lands late or loose, the other five specs either duplicate it or rot the boundaries before they are enforced.

## Scope
- In:
  - Root `Cargo.toml`: members for the F0 crates only (§3/§12): `crates/core` (wolluf-core), `crates/chart` (wolluf-chart), `crates/source-osu` (wolluf-source-osu), `crates/store` (wolluf-store), `crates/app` (wolluf-app), `apps/desktop/src-tauri` (wolluf-desktop), `apps/cli` (wolluf-cli, binary `wolluf`), `xtask`. `[workspace.package]`, `[workspace.dependencies]` with **the whole F0 pin set** (see Design), `[workspace.lints]`, `[profile.*]` tweaks.
  - `rust-toolchain.toml`, `rustfmt.toml`, `clippy.toml`, `.cargo/config.toml` (xtask alias), `.config/nextest.toml`, `.gitignore`, `.gitattributes`.
  - Stub crates for chart, source-osu, store, app, desktop, cli: they compile, declare only allowed edges, and contain no behaviour. Their content belongs to 002–006.
  - `wolluf-core` F0 types: stable string ids (`AxisId`, `PatternId`, `StageId`), `TimeUs`, `UnixUs`, `DotNetTicks`, `FileTime`, `RateMilli`, `Keymode`, `ColMask`, `Game`, `ChartMd5`, `BlobSha256`, `PlayId`, `AliasId`, `ProfileId`, `VersionKey` (+ builder), `ErrorCode`, `Clock` trait + `FixedClock`.
  - xtask: `check-layers` (full), `lint-canary` (full), `stage-lock [--check]` (real parser, no stages yet), `bindings` (delegating stub).
  - `xtask/layers.toml`, `stage_versions.lock` (empty), `deny.toml`, `.github/workflows/ci.yml`, `NOTICE`, `docs/conventions.md`.
  - The project licence (Q1, resolved): `LICENSE` with the MIT text and `Copyright (c) 2026 TWulfZ`; `license = "MIT"` in `[workspace.package]`, inherited by every member (`license.workspace = true`). `publish = false` stays until the first release.
  - ADRs 0001–0009 (§11), written as tasks of this spec.
- Out (non-goals):
  - Any format codec, snapshot, watcher (002, 003). Any SQL, migration, vault, writer thread (003). Identity selection, profiles (004). Tauri app, `tauri.conf.json`, React UI, real CLI commands, `SystemClock`, `AppError` (005). `.osg` (006).
  - `wolluf-chart` types (`Chart`, rows, LN pairs, layout, `chart!` DSL): F1 spec. The F0 crate is an empty shell so the member set matches §12.
  - `Evidence`, `SegmentAnchor`, `AccuracyCurve` in core: F1/F3 (no F0 consumer).
  - xtask `fixtures`, `parity`, `eval`, `pack-sign`, `nightly`; `release.yml`, `pack.yml`; param-pack schema validation and synthetic eval in CI (F1+).
  - Third-party licence bundle for installers (release spec, F4). Publishing any crate (`publish = false` until the first release).

## Behaviour
- `cargo build --workspace` / `cargo nextest run --workspace` succeed on ubuntu-24.04 and windows-2025 with the pinned toolchain; the repo's `rustc --version` is `1.98.1`.
- `cargo xtask check-layers` exits 0 on the real workspace and prints one line per violation otherwise (`<crate>: <rule-id>: <detail>`), exit code 1. Rules:
  - `L1 unlisted-crate`: a workspace member missing from `layers.toml`.
  - `L2 forbidden-edge`: an internal dependency (any kind: normal, build, dev) not in the crate's `allowed` list.
  - `L3 layers-cycle`: the `allowed` graph in `layers.toml` itself has a cycle.
  - `L4 banned-dep`: a crate whose layer bans a package finds it in its **transitive normal+build** tree (domain: `rusqlite`, `libsqlite3-sys`, `tokio`, `tauri`, `reqwest`, `notify`).
  - `L5 restricted-direct-dep`: a package listed in `layers.toml` `[restricted_deps]` appears as a direct dependency of a crate not on its list. F0 table: `rusqlite`, `rusqlite_migration`, `libsqlite3-sys` → `wolluf-store` (D6); `notify`, `notify-debouncer-full` → `wolluf-source-osu`; `tokio`, `tokio-util`, `rayon` → `wolluf-app`, `wolluf-desktop`, `wolluf-cli` (003).
  - `L6 banned-api`: a forbidden token in `src/**/*.rs` after stripping `//` comments (domain: `std::fs|net|env|process` incl. `use std::{…fs…}` forms, `SystemTime::now`, `Instant::now`, `thread_rng`, `rand::random`, `allow(clippy::disallowed_`; source-osu: `fs::write`, `File::create`, `OpenOptions`, `create_dir`, `remove_file`, `remove_dir`, `fs::rename`, `fs::copy`, `hard_link`, `symlink`, `set_permissions`; every crate but store: `rusqlite::`).
  - `L7 non-workspace-dep`: a member manifest declares a dependency with its own version instead of `workspace = true` (pins live in one file).
  - `L8 lints-not-inherited`: a member manifest lacks `[lints] workspace = true`.
- `cargo xtask lint-canary` runs clippy on `xtask/lint-canary/` (a standalone crate outside the workspace that inherits the root `clippy.toml` by directory lookup) and exits 0 only if every expected lint fires: `unwrap_used`, `expect_used`, `panic`, `disallowed_types` (HashMap, HashSet), `disallowed_methods` (`SystemTime::now`, `f64::powf`, `f32::sin`), and no "unknown config field / path does not resolve" warning appears. It proves the policy is live, not just written.
- `cargo xtask stage-lock --check` parses `stage_versions.lock`; with zero stages registered and zero in the lock it exits 0 and prints `stage-lock: 0 stages (engine arrives in F1)`. A malformed lock or a lock entry with no registered stage exits 1. Without `--check` it rewrites the lock (no-op in F0).
- `cargo xtask bindings` runs `cargo run -p wolluf-desktop --bin export-bindings` if `cargo metadata` shows that bin target; otherwise prints `bindings: skipped (wolluf-desktop has no export-bindings bin yet, see spec 005)` and exits 0.
- `cargo deny check` exits 0; any crate under GPL/LGPL/AGPL (or anything outside the allowlist) fails it.
- Core edge cases: `Keymode::new(0)` and `Keymode::new(17)` are errors; `ColMask` ops never touch bits ≥ keymode; `RateMilli::new(0)` is an error; `TimeUs::as_ms_floor(TimeUs(-1)) == -1` (floor, not truncation); `DotNetTicks::to_filetime` is `None` before 1601-01-01; `AxisId::parse("7K.Speed")` is an error (lowercase only); `ErrorCode::from_str("NOPE")` is an error.

## Domain rules
- Domain crates are IO-free and deterministic; banned deps and APIs are checked by xtask (architecture §4 D2). This spec treats `core` and `chart` as domain.
- Outputs contain no `HashMap`/`HashSet` iteration order; transcendentals go through `libm` (D3). Enforced workspace-wide by `clippy.toml` `disallowed-types` / `disallowed-methods`; non-domain crates may `#[allow]` locally with a one-line WHY; domain crates may not (L6 bans the `allow` token there).
- Only `store` has SQL (D6); `source-osu` has no fs write calls (D9); no new crate edge without an ADR (D1) → `layers.toml` changes need an ADR reference in the PR.
- Stable string ids are persisted and never renumbered; enum discriminants never reach disk or the wire (§11). `ErrorCode` strings are the closed list of §7.
- `TimeUs(i64)` µs, `RateMilli(u32)`, `Keymode(u8)`, `ColMask(u16)` (§5.1).
- `vkey = blake3(stage_id, VERSION, declared pack-section hashes, config hash, input fingerprint)` (§5.5); this spec pins the byte encoding.
- `play.id = blake3(game, chart_md5, raw_name, filetime)`; raw name bytes are the alias key, `""` is valid (§5.3). scores.db and the replay header store **.NET ticks** (100 ns since 0001-01-01; oracle `research/scripts/audit/osudb.py::ticks_to_dt`); the `Data/r` suffix is the same instant as FILETIME = ticks − 504 911 232 000 000 000 (research 03 l.168, 4362/4362 verified). The id hashes the `FileTime` value, as §5.3 names it (bijective with ticks from 1601 on); 003 stores the same value in `play.filetime`. This spec is the single owner of `PlayId`, `FileTime` and the conversions; 002 and 003 consume them.
- Licences: MIT/Apache/BSD/MPL/Zlib/ISC allowed; GPL/LGPL/AGPL banned in-process (§3 deny.toml, D16; research 01 l.63, 106). This spec adds `Unicode-3.0` (required by `unicode-ident`, pulled by every proc-macro) and `CC0-1.0` (licence of `notify` 8.2.0, needed by 003); both are permissive, recorded in ADR 0008.
- Toolchain pinned to 1.98.x so derived numbers are identical across machines (§3).
- `thiserror` in libraries, `anyhow` only in CLI and xtask (§7, §11). `unwrap/expect/panic` denied in libraries (§7).

## Design
- **Workspace** (`Cargo.toml`): `resolver = "3"`; `[workspace.package]` `version = "0.1.0"`, `edition = "2024"`, `rust-version = "1.98"`, `publish = false` (until the first release), `license = "MIT"` (Q1); every member inherits `version`, `edition`, `rust-version`, `license` and `publish` with `.workspace = true`. Internal crates are listed in `[workspace.dependencies]` by path so members write `wolluf-core.workspace = true`. `[profile.dev.package.insta]` and `similar` at `opt-level = 3` (insta docs). No release-profile tuning yet, and no profile ever sets `panic = "abort"`: 003's per-item `catch_unwind` needs unwinding.
- **Pinned dependencies** (crates.io, checked 2026-09-28; the orchestrator owns this table, siblings only add `x.workspace = true`):

  | Use | Pins |
  |---|---|
  | core | `thiserror 2.0.21`, `serde 1.0.229` (derive), `blake3 1.8.7`, `libm 0.2.16` |
  | codecs / hashing (002, 003) | `md-5 0.11.0`, `sha2 0.11.0`, `zstd 0.14.0`, `postcard 1.1.3` (alloc) |
  | source-osu IO (002, 003) | `sysinfo 0.39.6` (no default features, `system`), `winreg 0.56.0` (`cfg(windows)` only), `notify 8.2.0`, `notify-debouncer-full 0.7.0` |
  | store (003) | `rusqlite 0.40.2` (bundled), `rusqlite_migration 2.6.0`, `crossbeam-channel 0.5.17` |
  | app / jobs (003–005) | `tokio 1.53.1`, `tokio-util 0.7.19`, `rayon 1.12.0`, `uuid 1.26.1` (v4), `ulid 3.0.0`, `directories 6.0.0`, `unicode-normalization 0.1.25`, `caseless 0.2.2`, `tracing 0.1.44`, `tracing-subscriber 0.3.23`, `tracing-appender 0.2.5`, `serde_json 1.0.151` |
  | shells (005) | `tauri 2.12.0`, `tauri-build 2.7.0`, `tauri-plugin-dialog 2.8.0`, `tauri-plugin-opener 2.6.0`, `tauri-plugin-log 2.10.0`, `specta =2.0.0-rc.25`, `tauri-specta =2.0.0-rc.25`, `specta-typescript 0.0.12`, `clap 4.6.7` (derive), `anyhow 1.0.104` |
  | xtask | `cargo_metadata 0.23.1`, `toml 1.1.6`, `regex 1.13.1`, `anyhow`, `clap` |
  | tests | `proptest 1.11.0`, `insta 1.48.0`, `trybuild 1.0.121`, `tempfile 3.27.0`, `assert_cmd 2.2.2`, `predicates 3.1.4` |

  This is the complete F0 set: sibling specs 002–006 add no pin of their own (they were collected here at the F0 review). No LZMA crate: `.osr` payload decompression is F2.

  Tooling (not crates): `cargo-nextest 0.9.146`, `cargo-deny 0.20.2`, Node 24, pnpm 11. RC pins use `=` because semver does not protect pre-releases.
- **Lints** (`[workspace.lints]`, inherited by every member, bins included; stricter than §7 on purpose so bins use `?` + anyhow too):
  - rust: `unsafe_code = "deny"`, `unreachable_pub = "warn"`.
  - clippy: `unwrap_used`, `expect_used`, `panic`, `todo`, `unimplemented`, `dbg_macro`, `disallowed_types`, `disallowed_methods` = `"deny"`.
  - `clippy.toml`: `allow-unwrap-in-tests`, `allow-expect-in-tests`, `allow-panic-in-tests = true`; `disallowed-types` = `std::collections::HashMap`, `std::collections::HashSet` (reason: D3, use `BTreeMap`/`BTreeSet` or a sorted `Vec`); `disallowed-methods` = `std::time::SystemTime::now` (reason: inject `Clock`) and the `f64`/`f32` transcendentals `exp, exp2, exp_m1, ln, ln_1p, log, log2, log10, powf, powi, sin, cos, tan, sin_cos, asin, acos, atan, atan2, sinh, cosh, tanh, asinh, acosh, atanh, cbrt, hypot` (reason: D3, use `libm`). `sqrt` and `mul_add` stay allowed (IEEE correctly rounded).
  - Integration-test files (`tests/*.rs`) may open with `#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]` because clippy's in-tests allowances cover only `#[test]` bodies; this is the only blanket allow (conventions.md).
- **Toolchain**: `rust-toolchain.toml` `channel = "1.98.1"`, `components = ["rustfmt", "clippy"]`, `profile = "minimal"`. `rustfmt.toml`: `style_edition = "2024"`. `.gitattributes`: `* text=auto eol=lf`, binary for `*.osr *.osg *.osu *.db *.zst *.png` (goldens must hash identically on WSL and Windows, D3).
- **wolluf-core** (deps: `thiserror`, `serde`, `blake3` only; modules `id`, `time`, `keymode`, `digest`, `vkey`, `error`, `clock`; one `CoreError` thiserror enum):
  - `StableId` rule: 1–64 bytes, `[a-z0-9_]+` segments joined by `.`. `AxisId`, `PatternId`, `StageId` wrap `Cow<'static, str>`; `parse(&str) -> Result`, `const fn from_static(&'static str)` validates with `assert!` so an invalid literal in a `const` fails compilation; `as_str`; `Ord`; serde as a plain string (`try_from = "String"`).
  - `TimeUs(i64)` chart time: `from_ms`, `as_ms_floor` (`div_euclid`), `checked_add/sub`, `Add/Sub`, `Ord`. `UnixUs(i64)` wall-clock instant, a separate type so map time and wall time cannot mix. `DotNetTicks(i64)`: `to_unix_us() -> UnixUs` (offset 621 355 968 000 000 000 ticks, floor), `to_filetime() -> Option<FileTime>` (offset `DOTNET_TO_FILETIME_TICKS = 504 911 232 000 000 000`; `None` before 1601-01-01). `FileTime(i64)` (never negative): `to_dotnet_ticks() -> Option<DotNetTicks>` (`None` on overflow), `Display`/`parse_decimal` as the plain decimal text used by the `Data/r` suffix and the `play.filetime` column.
  - `RateMilli(u32)`: `ONE = 1000`, `new` rejects 0.
  - `Keymode(u8)`: `new` accepts 1..=16 (the `ColMask(u16)` width); consts `K4`, `K7`; `columns()`. `ColMask(u16)`: bit *i* = column *i*, 0 = leftmost (matches `floor(x·K/512)`); `EMPTY`, `full(Keymode)`, `single`, `from_cols`, `contains`, `len`, `iter()` ascending, `is_subset_of`, `mirror(Keymode)` (Mirror mod, §5.1).
  - `Game` enum with stable strings (`OsuStable` ↔ `"osu_stable"`). `ChartMd5([u8;16])` and `BlobSha256([u8;32])` parse/print lowercase hex (internal hex helper, no dep). `AliasId(i64)`, `ProfileId(i64)` row-id newtypes. `PlayId([u8;32])::derive(Game, ChartMd5, raw_name: &[u8], FileTime)`, hex `Display`. `ScopeHash([u8;32])` is added to this module by 004 T2.
  - Hash encoding for both `PlayId` and `VersionKey`: a domain tag (`b"wolluf.play.v1"` / `b"wolluf.vkey.v1"`), then each field as `u32` LE length + bytes; integers LE fixed width. For `PlayId` the fields are: game stable string bytes, the 16 raw md5 bytes, raw name bytes, `FileTime` as `i64` LE. The golden vector uses the pilot tuple `(osu_stable, e956977ccc1d74a50ae48b43a868cc20, b"TWulfZ", FileTime(134350010443098880))`. The encoding is recorded in ADR 0006 and never changes (a change would duplicate the whole ledger). `VersionKeyBuilder::new(StageId, version: u32).section(name, [u8;32]).config([u8;32]).input([u8;32]).finish() -> Result<VersionKey>`; sections are sorted by name inside `finish`, duplicates are an error, missing config/input default to 32 zero bytes. `VersionKey` prints as 64-char lowercase hex. Golden vectors are committed in tests, so an encoding change is visible.
  - `ErrorCode`: closed enum, `ALL`, `as_str`, `FromStr`, serde as the string; strings exactly `OSU_DIR_NOT_FOUND, UNSUPPORTED_FORMAT, PARSE_FAILED, OSU_RUNNING, CONSENT_REQUIRED, SIGNATURE_INVALID, NOT_FOUND, INVALID_INPUT, CONFLICT, CANCELLED, INTERNAL` (§7). `AppError` itself is 005's.
  - `trait Clock: Send + Sync { fn now(&self) -> UnixUs; }` (D8 b). `FixedClock` (atomic, `set`/`advance`) ships in core for tests everywhere. `SystemClock` lives in `wolluf-app` (005) and is the single `#[allow(clippy::disallowed_methods)]` site for `SystemTime::now`.
- **Stub crates**: `wolluf-chart` → core. `wolluf-source-osu` → core. `wolluf-store` → core. `wolluf-app` → core, store, source-osu. `wolluf-desktop` (plain `fn main`, no tauri yet) → app, core. `wolluf-cli` (`[[bin]] name = "wolluf"`, `fn main() -> anyhow::Result<()>`) → app, core.
- **`xtask/layers.toml`** lists every crate of §3, present or future, as `[crates.<name>] layer = "domain" | "engine" | "adapter" | "app" | "shell" | "tool"` plus an explicit `allowed = [...]` of direct internal deps. The lists are the §4 DAG edges plus the shortcuts needed while `engine` does not exist (store/source-osu → core, chart), plus the tool edge `xtask → wolluf-source-osu` (feature `test-support`, used by 002's `cargo xtask fixtures`); every entry must point downward (L3 checks acyclicity). `[layers.<layer>] banned_deps`, `banned_direct_deps`, `banned_apis` hold the rule data of Behaviour. Explicit lists were chosen over "transitive closure of §4" because closure would let `cli` depend on `store`, which D11 forbids.
- **xtask** (`anyhow`, `clap`, `cargo_metadata`, `toml`, `regex`): subcommands `check-layers`, `lint-canary`, `stage-lock [--check]`, `bindings`. Logic lives in pure functions over parsed metadata / file text so tests use inline fixtures, no real cargo runs except `lint-canary`.
- `stage_versions.lock`: TOML, `[stages.<stage_id>] version = u32, golden = "<blake3 hex>"`; F0 file has only a header comment.
- Versioned stages affected: none (no stage exists in F0). Param-pack sections: none.

## Data
- user.db migration: no. cache.db change: no. Vault or blob changes: no. (003 owns all three.)

## IPC / UI
none. The `bindings` delegation contract for 005: a bin target `export-bindings` in `wolluf-desktop` that writes `apps/desktop/ui/src/ipc/bindings.ts`.

## Acceptance criteria
- [ ] AC1: The workspace has exactly the 8 F0 members and builds → `cargo metadata --no-deps --format-version 1 | jq '.packages | length'` = 8; `cargo build --workspace --all-targets` passes.
- [ ] AC2: Toolchain pinned → `rustc --version` in the repo prints `rustc 1.98.1`.
- [ ] AC3: Format and lints clean → `cargo fmt --all --check`; `cargo clippy --workspace --all-targets -- -D warnings`.
- [ ] AC4: Lint policy is live → `cargo xtask lint-canary` exits 0 (all expected lints fire, no unknown config key, no unresolved path); test `lint_canary::tests::canary_lints_match_root` keeps the canary's copied `[lints]` identical to the root.
- [ ] AC5: Core ids → tests `id::tests::{parses_dotted_lowercase, rejects_uppercase_empty_and_long, from_static_in_const, serde_is_plain_string}` pass.
- [ ] AC6: Core time/rate → tests `time::tests::{ms_floor_rounds_down_for_negatives, dotnet_ticks_unix_epoch_is_zero, filetime_epoch_is_zero, filetime_before_1601_is_none, filetime_roundtrip, filetime_decimal_matches_data_r}` (ticks `639190703004225018` → FileTime text `134279471004225018`) and `rate_zero_rejected` pass; proptest `time::props::filetime_ticks_bijective` passes.
- [ ] AC7: Keymode/ColMask → tests `keymode::tests::{rejects_0_and_17, k7_has_7_columns}` and proptests `keymode::props::{mirror_is_involution, ops_stay_within_keymode, iter_is_ascending_and_matches_len}` pass.
- [ ] AC8: Digests and ids → tests `digest::tests::{md5_hex_roundtrip, rejects_bad_hex, play_id_golden_vector, play_id_distinguishes_empty_alias, play_id_length_prefix_prevents_concat_collision}` pass (`""` and `"W"` give distinct ids; moving bytes between adjacent fields changes the id).
- [ ] AC9: Version keys → tests `vkey::tests::{golden_vector, section_order_is_irrelevant, duplicate_section_rejected, any_field_change_changes_key}` pass.
- [ ] AC10: Error codes are the closed §7 list → `error::tests::roundtrip_all` and insta golden `error_code_strings` (the ordered string list) pass.
- [ ] AC11: Clock → `clock::tests::fixed_clock_set_and_advance` passes; `cargo tree -p wolluf-core -e normal --depth 1` lists only thiserror, serde, blake3.
- [ ] AC12: Layer checker → `cargo xtask check-layers` exits 0 on the repo, and `cargo nextest run -p xtask` passes `check_layers::tests::{rejects_unlisted_crate, rejects_upward_edge, rejects_cli_to_store, rejects_cycle_in_layers_toml, rejects_tokio_in_domain_tree, rejects_rusqlite_outside_store, rejects_tokio_direct_in_store, rejects_std_fs_in_domain, rejects_braced_use_std_fs_in_domain, ignores_commented_tokens, rejects_file_create_in_source_osu, rejects_allow_disallowed_in_domain, rejects_non_workspace_dep, rejects_missing_workspace_lints}`.
- [ ] AC13: Stage lock / bindings stubs → `cargo xtask stage-lock --check` exits 0 and prints the 0-stage line; tests `stage_lock::tests::{empty_lock_passes, malformed_lock_fails, unregistered_stage_fails}` and `bindings::tests::skips_without_export_bin` pass; `cargo xtask bindings` exits 0 with the skip line.
- [ ] AC14: Licence gate → `cargo deny check` exits 0; test `deny_config::tests::allowlist_is_exact` asserts the allowlist equals {MIT, Apache-2.0, Apache-2.0 WITH LLVM-exception, BSD-2-Clause, BSD-3-Clause, MPL-2.0, Zlib, ISC, Unicode-3.0, CC0-1.0} and contains no GPL/LGPL/AGPL id.
- [ ] AC15: CI → `actionlint .github/workflows/ci.yml` is clean, and the first PR run of `ci` is green on ubuntu-24.04 and windows-2025 (link recorded under Deviations at close).
- [ ] AC16: Docs and licence → `NOTICE`, `docs/conventions.md` exist; `LICENSE` is the MIT text and contains `Copyright (c) 2026 TWulfZ`; `cargo metadata --no-deps --format-version 1 | jq -c '[.packages[] | [.license, .publish]] | unique'` = `[["MIT",[]]]`; `ls docs/adr/000[1-9]-*.md | wc -l` = 9; each ADR has `Status: Accepted` and the four template headings (`grep -c '^## ' ≥ 4`).
- [ ] AC17: Every gate command of `wolluf-sdd` §4 that exists after this spec runs clean locally from a fresh clone (fmt, clippy, check-layers, stage-lock --check, nextest, deny).

## Risks / open questions
- **Resolved (user decision 2026-09-28) — Q1, the project's own licence: MIT**, copyright 2026 TWulfZ. `LICENSE` and `license = "MIT"` land in F0 (T1, T15); `publish = false` and cargo-deny `[licenses.private] ignore = true` stay until the first release; ADR 0008 records it (T23).
- **Resolved at F0 review — §3/§7 "snapshot-copy" vs D9:** 002 and 003 agree on in-memory snapshots (`fs::read` + size/mtime re-check; osu!.db 31.2 MB, scores.db 652 KB, collection.db 3.5 KB on the pilot). ADR 0014 (003 T1) records it and fixes the §3/§7 wording; ADR 0004 only references it.
- **Resolved at F0 review — ticks vs FILETIME:** `PlayId` hashes `FileTime` (§5.3 wording kept) and `play.filetime` stores its decimal text; ticks are converted once at ingest through `DotNetTicks::to_filetime`.
- Clippy support for primitive-type paths (`f64::powf`) in `disallowed-methods` and the `allow-panic-in-tests` key: expected to work on 1.98, verified by `lint-canary`. Fallback: move transcendental bans to the xtask L6 grep for domain crates and use `#![cfg_attr(test, allow(clippy::panic))]`.
- `disallowed_types` may fire inside tauri/specta macro expansions in `wolluf-desktop` (005). Mitigation: local `#[allow]` with WHY at the macro call site; still banned in domain crates.
- tauri-specta is a release candidate (rc.25, 2026-05). ADR 0009 keeps the §8 fallback (ts-rs + hand wrapper inside `ipc/`).
- Explicit `allowed` lists can drift from §4. Mitigation: `layers.toml` header links §4; L3 rejects cycles; changes need an ADR (D1).
- `cargo-deny` is not installed locally yet (only nextest is); T13 installs 0.20.2.
- rustup auto-install from `rust-toolchain.toml` is not relied on in CI: the workflow runs `rustup toolchain install` explicitly.

## Deviations (filled at close)
