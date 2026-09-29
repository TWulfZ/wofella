# wolluf conventions

Status: accepted 2026-09-28 (spec 001 T15). This file expands architecture §11. The architecture and its ADRs win on conflict; change this file in the same PR as the rule it describes.

## Language
- Code, identifiers, comments, doc comments, TODOs, docs, ADRs, specs and commit messages are in **English**, whatever language the conversation or the surrounding file uses.
- Only user-facing strings (i18n values in `es`/`en`, UI copy, error messages a human reads) follow the product locale. Rust never builds user-facing prose: it returns a stable `message_key` plus args (§7), and the UI localises.

## Comments
Comments explain **why**, never what. The code already says what it does.

Write a comment only for:
- a **business rule or domain invariant** the code cannot express (why a validation exists, what osu! stable actually does);
- a **non-obvious technical decision**: a race, an ordering that looks arbitrary but is load-bearing, a library or DB workaround, a rejected alternative worth recording;
- a **reference** to an ADR, spec, research file or ticket that carries the reasoning (`// ADR 0014`, `// research 03 l.168`).

Never comment a name that already explains itself, restate the next line in prose, or narrate a well-named function. Prefer one line; use a block only for a genuine invariant or trap. If the explanation already lives in a spec or ADR, link it instead of copying it. This rule overrides "match the surrounding file's density": match a neighbour's style, not its volume.

## Commits
- Conventional Commits (`feat`, `fix`, `docs`, `build`, `test`, `refactor`, `chore`, `ci`), imperative mood, ≤ 72 characters, English.
- **Subject only by default.** Add a body only for heavy commits (a migration, a breaking change, an architectural tradeoff), and use it for the why and what breaks, never a bullet list restating the diff.
- Never add `Co-Authored-By` or any other attribution trailer.
- One task of a spec's `tasks.md` = one commit. `Cargo.lock` is committed together with the manifest change that moved it.

## ADRs
- MADR format from `.claude/skills/wolluf-sdd/templates/adr.md`, stored as `docs/adr/NNNN-kebab-title.md` with `Status: Proposed | Accepted | Superseded by NNNN` and the four headings Context, Decision, Alternatives considered, Consequences.
- An ADR is **required** for:
  - any new internal crate edge or any change to `xtask/layers.toml` (D1). The PR that touches `layers.toml` names the ADR;
  - storage changes: a new store, a user.db table that changes the ledger's meaning, the vault layout, a change to the `PlayId` or `VersionKey` encoding (ADR 0003, ADR 0006);
  - a change to the meaning or scale of θ (§9.4);
  - any cross-cutting decision (errors, IPC, licensing, privacy);
  - a choice between real alternatives that a future reader would otherwise re-litigate.
- Numbers are final once assigned. 0010, 0011 and 0013 are reserved for F1–F3 (§11). An amendment edits the ADR in place under a dated `## Amendment` section and keeps the original text readable; a reversal writes a new ADR and marks the old one `Superseded by`.
- A change to `docs/architecture.md` lands in the same PR as its ADR.

## Stable ids and versioning
- Axes, patterns, stages, error codes, job kinds, feedback kinds, snapshot and blob kinds, identity enums (`me`/`not_me`, `cfg_username`, …) are **stable strings**. They are persisted and never renumbered or reused with another meaning. Rename by adding a new id and migrating readers.
- Enum discriminants never reach disk or the wire; serialise the stable string.
- `StableId` shape: 1–64 bytes of `[a-z0-9_]+` segments joined by `.` (spec 001).
- Every derived artifact carries its `VersionKey` (D15). When the outputs of a versioned stage change, bump its `const VERSION` and run `cargo xtask stage-lock` in the same commit. CI (`stage-lock --check`) fails a golden change without a bump.
- IPC ids above 2^53 (play ids, FILETIME, online score ids) cross as strings (§8).

## Data rules
- Every persisted blob format has a format-version header.
- No `HashMap`/`HashSet` iteration order reaches an output; use `BTreeMap`, `BTreeSet` or a sorted `Vec` (D3). Reductions run in a fixed order, and transcendentals go through `libm`.
- Thresholds and weights live in param structs, never inline (D17). Until the engine's param pack exists (F1), their defaults ship as the struct's `Default`.
- Anything that changes a derived number is a `feedback_event`, an `identity_decision` or a profile row, never a `settings` key (§5.3). `settings` holds UI preferences only.
- The play ledger is immutable; re-ingest is idempotent through the natural key (§5.3, ADR 0014).
- Times: `TimeUs` for map time, `UnixUs` for wall time; persisted wall times are RFC 3339 UTC with milliseconds.

## Errors
- Libraries use one `thiserror` enum per crate with precise variants. `anyhow` is allowed only in `apps/cli` and `xtask`.
- `unwrap`, `expect`, `panic!`, `todo!`, `unimplemented!` and `dbg!` are denied workspace-wide, bins included; bins use `?` with `anyhow`.
- Errors that reach the UI are `AppError { code: ErrorCode, message_key, args, details, retryable }`. `ErrorCode` is the closed §7 list of stable strings; adding one needs a spec and a UI mapping.
- Parsers are lenient and collect `Diagnostics` for odd sections. Unknown format versions are handled as ADR 0015 decides (accepted with a warning only after full structural validation); nothing guesses silently.

## Lint policy and allowances
The policy lives in root `Cargo.toml` `[workspace.lints]` and `clippy.toml`; `cargo xtask lint-canary` proves it is live. Every member sets `[lints] workspace = true` (check-layers L8).

Allowed exceptions, and only these:
- **Tests.** `clippy.toml` sets `allow-unwrap-in-tests`, `allow-expect-in-tests` and `allow-panic-in-tests`, which cover `#[test]` bodies and `#[cfg(test)]` modules.
- **Integration-test files** (`tests/*.rs`) may open with exactly `#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]`, because clippy's in-test allowances do not reach helpers outside `#[test]` functions there. This is the **only blanket allow** in the repo.
- **`SystemClock`** in `wolluf-app` (spec 005) is the single `#[allow(clippy::disallowed_methods)]` site for `SystemTime::now`. Everything else takes a `&dyn Clock` (D8 b).
- **Non-domain crates** (adapters, app, shells, xtask) may add a local `#[allow(clippy::disallowed_…)]` on the smallest item with a one-line WHY, e.g. a `HashMap` required by a third-party macro expansion. **Domain crates may not**: check-layers L6 bans the `allow(clippy::disallowed_` token there.
- Never add `#![allow(...)]` at crate level to silence a lint, and never weaken `clippy.toml` or `[workspace.lints]` to make a build pass.

## Dependencies and layers
- Every version is pinned once in root `[workspace.dependencies]`; members write `x.workspace = true` (check-layers L7). Only the orchestrator edits the root manifest.
- Internal edges follow `xtask/layers.toml`, which mirrors §4. Changing `layers.toml` (a new crate, a new allowed edge, a restricted-deps entry) **needs an ADR** referenced in the PR (D1).
- `rusqlite*` only in `wolluf-store` (D6), `notify*` only in `wolluf-source-osu`, `tokio*`/`rayon` only in `wolluf-app` and the shells (check-layers L5).
- A new crate must earn a distinct dependency footprint or fixture set (§3 crate budget) and is created only when its phase starts.

## Licensing and NOTICE
- The project is MIT (`LICENSE`, copyright 2026 TWulfZ); `publish = false` until the first release (ADR 0008).
- No GPL, LGPL or AGPL code in-process (D16). `cargo deny check` enforces the allowlist in `deny.toml`; tosu and similar tools stay out-of-process.
- **Ported code updates `NOTICE` in the same commit that brings it in**: project, upstream path and revision, licence, copyright line, and the path of the port. Ported files also keep a one-line header naming the origin. Planned MIT ports: Interlude `prelude/`, mania-hub `algorithms/`, LeoBlack.
- Sunny is a clean-room reimplementation from the paper: never read or copy GPL/LGPL sources while writing it.

## Fixtures and the corpus
- **Never commit** real beatmaps, audio, replays, skins or the user's osu! DBs, and no player names beyond those a spec explicitly allows.
- Committed fixtures are either **synthetic** (built by DSLs and test-support builders) or **minimized and anonymized** extracts produced by `cargo xtask fixtures` (spec 002), with a leak check. Third-party player names are replaced (e.g. `Rosalind`, `Kovacs`, `Sterling`); md5 values are replaced with derived ones.
- The pilot corpus at `WOLLUF_CORPUS` (`/mnt/e/Games/osu!`) is **read-only, always**. Tests and tools only read it; outputs go to a tempdir or the app data dir, and a corpus test asserts nothing under the root changed.
- Corpus tests are `#[ignore]` and run with `WOLLUF_CORPUS="/mnt/e/Games/osu!" cargo nextest run --run-ignored only`. They assert invariants and selections against independently computed oracles, never hard-coded counts that drift.
- Every bug found in the corpus gets a synthetic regression fixture (§10).

## Tests and gates
- Behaviour work is test-first: write the failing test, then the minimum code (`wolluf-sdd` §3).
- Domain tests are pure and millisecond-fast. Store tests use in-memory or temp SQLite with the real migrations; no repository fakes (D8).
- Golden outputs use `insta`; property tests use `proptest`; compile-fail gates use `trybuild`.
- Before a task or spec is declared done, the `wolluf-sdd` §4 gate block runs. A failing gate is reported with its output, never skipped or fixed by loosening a test.

## New-feature checklist
Copy into the PR description:
- app slice (`app::features::<name>`), service, DTOs with `specta::Type`;
- commands in `apps/desktop/src-tauri/src/commands/<name>.rs` and `cargo xtask bindings`;
- UI slice and route;
- user.db migration or `CACHE_SCHEMA_VERSION` bump?
- stage `VERSION` bumped and `stage-lock` updated?
- eval report attached (F1+)?
- i18n strings in es and en?
- ADR needed (new edge, storage, θ, cross-cutting)?
