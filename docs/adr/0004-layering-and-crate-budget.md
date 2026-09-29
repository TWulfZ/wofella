# 0004 Layering, crate DAG and crate budget

- Status: Accepted
- Date: 2026-09-28

## Context
A solo developer has to keep shipping without the codebase rotting (G5). Rust's module privacy cannot stop `patterns` from importing `skill`, or `cli` from reaching into `store`. Only the crate graph can. At the same time, every crate has a cost: build time, manifest upkeep, and one more boundary to route through. Domain results must be reproducible and identical on WSL (dev) and Windows (target) (G1, D3), and the osu! folder must be read-only by construction (G6, D9). Rules that live only in a document drift, so they have to be checked mechanically from the first commit (spec 001).

## Decision
**Layers and DAG.** The crates form the DAG of architecture §4, from bottom to top:
- `domain` (pure, deterministic): core, chart, patterns, difficulty, judge, skill, session, recommend, drills, eval;
- `engine`: registry, keymode profiles, manifest, staleness planning;
- `adapter` (IO): store, source-osu, online, audio;
- `app`;
- `shell`: desktop, cli;
- `tool`: xtask.

Dependencies point downward only (D1). Adapters never depend on `app` or on each other (D7). `skill` depends only on `core` (D4). Only `store` has SQL (D6).

**Explicit allowed lists.** `xtask/layers.toml` lists every §3 crate, present or future, with its layer and an explicit `allowed = [...]` of direct internal dependencies. The lists are the §4 edges plus two kinds of additions:
- shortcuts needed while `engine` does not exist: store and source-osu → core (and chart once it has consumers);
- the tool edge `xtask → wolluf-source-osu` (feature `test-support`), used by `cargo xtask fixtures` (spec 002).

Any change to `layers.toml` needs an ADR (D1).

**Enforcement** (`cargo xtask check-layers`, spec 001; one line `<crate>: <rule-id>: <detail>` per violation, exit 1):
- L1: every workspace member is listed. L2: no internal edge, of any dependency kind, outside `allowed`. L3: the `allowed` graph is acyclic.
- L4: domain crates have no `rusqlite`, `libsqlite3-sys`, `tokio`, `tauri`, `reqwest` or `notify` in their transitive normal+build tree.
- L5: restricted direct dependencies. `rusqlite`, `rusqlite_migration` and `libsqlite3-sys` → store only. `notify` and `notify-debouncer-full` → source-osu only. `tokio`, `tokio-util` and `rayon` → app and the shells only.
- L6: a banned-API grep over `src/**` with `//` comments stripped. Domain crates: `std::fs|net|env|process` (including braced `use` forms), `SystemTime::now`, `Instant::now`, `thread_rng`, `rand::random`, and the token `allow(clippy::disallowed_`. source-osu: every fs write, rename, copy, create-dir, remove, link and permission call. Every crate but store: `rusqlite::`.
- L7: member manifests use `x.workspace = true`, so every version pin lives in the root `Cargo.toml`. L8: every member inherits `[lints] workspace = true`.

**Lint policy.** `[workspace.lints]` is inherited by every member, bins included:
- rust `unsafe_code = "deny"`, `unreachable_pub = "warn"`;
- clippy `unwrap_used`, `expect_used`, `panic`, `todo`, `unimplemented`, `dbg_macro`, `disallowed_types` and `disallowed_methods` = deny.

`clippy.toml` bans `HashMap`/`HashSet` (D3: iteration order), `SystemTime::now` (inject `Clock`, D8), and the `f64`/`f32` transcendentals (D3: use `libm`). `sqrt` and `mul_add` stay allowed because IEEE 754 requires them to be correctly rounded. This is stricter than §7 on purpose: bins also use `?` + `anyhow`. The only permitted escapes are:
- a local `#[allow]` with a one-line WHY in non-domain crates (for example a macro expansion in the desktop shell);
- `SystemClock` in `wolluf-app`, the single allowed site for `SystemTime::now`;
- `#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]` at the top of integration-test files, because clippy's in-test allowances only cover `#[test]` bodies.

Domain crates may not `#[allow]` a disallowed lint at all (L6). `cargo xtask lint-canary` proves the policy is live by running clippy on a crate that violates each rule once.

**Crate budget.** A new crate must earn a distinct dependency footprint or a distinct fixture set; anything else is a module (§3). Product features are modules of `wolluf-app`, `layout` lives in `chart`, and identity logic lives in `app::features::players`. Crates are created only when their roadmap phase starts. F0 has 7 crates plus xtask: core, chart, source-osu, store, app, desktop, cli.

**D9 stays literal.** source-osu reads osu!'s DBs into memory (`fs::read` with a size and mtime re-check) instead of copying them to a temp file, so the crate contains no fs write call at all. ADR 0014 (spec 003) records this and fixes the §3/§7 "snapshot-copy" wording.

**F0 deviation: `wolluf-chart` is an empty crate.** §12 lists "chart (types)" for F0, but nothing in F0 consumes chart types. F0 takes chart metadata from osu!.db, and 003 only verifies `.osu` md5s. The crate exists so the member set and the layers file match §12, and `Chart`, rows, LN pairs, layout and the `chart!` DSL move to the F1 spec, where they are designed against their first consumer.

## Alternatives considered
- **`allowed` computed as the transitive closure of §4.** Rejected: closure would let `cli` depend on `store`, which D11 forbids (shells go through `app`). Explicit lists also make every shortcut visible and reviewable.
- **One big crate with module privacy.** Rejected: `pub(crate)` cannot stop a domain module from importing an IO module. Domain builds would also pull tokio, rusqlite and tauri, and fixture tests would slow down.
- **A crate per feature or per concern (layout, identity, jobs).** Rejected by the crate budget: each crate adds build and manifest overhead without a distinct footprint.
- **Relying on review or on cargo-deny `bans` alone.** Rejected: review misses transitive edges, and cargo-deny cannot express "allowed for crate X only" or grep for banned APIs.
- **Transcendental bans only in domain crates via the L6 grep.** Kept as the fallback in spec 001, but not needed: clippy's `disallowed-methods` resolves primitive paths such as `f64::powf` on toolchain 1.98.1, which lint-canary verified.

## Consequences
- An upward or sideways edge fails CI with a precise message, and fixing it means either restructuring or writing an ADR.
- Domain crates build and test without any IO dependency, so fixture tests run in milliseconds.
- `layers.toml` has to be updated when a phase adds a crate (F1: patterns, difficulty, engine, eval), and each update cites its ADR. The shortcut edges (store/source-osu → core) are revisited when `engine` arrives.
- The lint policy costs some ceremony (`?` everywhere, `BTreeMap` over `HashMap`) in exchange for platform-identical outputs.
