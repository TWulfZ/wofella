---
name: wolluf-sdd
description: Spec-driven development workflow for the wolluf repo. Use for ANY feature, roadmap phase task (F0–F5), new pattern rule, schema change, IPC command, model/param change or behavioural bug fix in wolluf — before writing code. Produces docs/specs/<id>-<slug>/{spec,tasks}.md, then implements task-by-task with TDD and runs the project gates.
---

# wolluf spec-driven development

Every change leaves a spec a future reader can check the code against. Specs are short and testable, and their claims come from `docs/architecture.md` and `docs/research/`, never from memory.

## 0. Orient (always)
1. Read `docs/architecture.md`: the sections the change touches, plus §4 dependency rules and §12 for the current phase.
2. Grep `docs/research/` and `docs/adr/` for the domain facts involved (formats, hit windows, axes, prior art). Cite them in the spec.
3. Check `docs/specs/` for an existing spec covering this work. If one exists, extend it; don't duplicate it.

## 1. Spec: `docs/specs/<NNN>-<slug>/spec.md`
- Copy `templates/spec.md`. Take the next free NNN; a phase gets a range, e.g. F0 = 001–019.
- Fill **every** section. Write "none" when a section does not apply; never delete it.
- Acceptance criteria must be observable and testable: a command, a test name, or a UI state. No "works well".
- Record any decision that is cross-cutting, adds a crate edge, changes storage or the θ scale, or picks between real alternatives in `docs/adr/NNNN-*.md` from `templates/adr.md`, and link it.
- Stop and ask the user when the spec reveals a product decision that neither the research nor the architecture answers.

## 2. Tasks: `docs/specs/<NNN>-<slug>/tasks.md`
- Copy `templates/tasks.md`. Tasks are ordered, each fits in one commit, and each names its **verification command**.
- Order the work bottom-up along the crate DAG: core → domain → store/adapters → app → shell/UI.
- Every behaviour task starts with a failing test.
- Mark tasks that can run in parallel (they touch disjoint crates) with `[P]`.

## 3. Implement (per task)
1. Write the failing test first: unit test, `chart!`/fixture snippet, proptest, insta golden, or trybuild.
2. Write the minimum code, following the architecture rules:
   - no IO in domain crates;
   - SQL only in store;
   - thresholds live in param structs;
   - stable string ids;
   - `VersionKey` on derived data;
   - `thiserror` in libraries.
3. Code comments are English and explain WHY only (a business rule, a non-obvious decision, or a reference to an ADR or research file).
4. When outputs of a versioned stage change: bump `VERSION` and run `cargo xtask stage-lock`.
5. When the IPC surface changes: run `cargo xtask bindings` and commit `bindings.ts`.
6. Tick the task in tasks.md and commit (Conventional Commits, subject only, English, no trailers).

## 4. Gates (before declaring a task or spec done)
Run what exists; ignore steps that are not built yet.
```
export PATH="$HOME/.cargo/bin:$PATH"
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo xtask check-layers
cargo xtask stage-lock --check
cargo nextest run --workspace
cargo xtask bindings && git diff --exit-code apps/desktop/ui/src/ipc/bindings.ts
pnpm -C apps/desktop/ui exec tsc --noEmit && pnpm -C apps/desktop/ui lint && pnpm -C apps/desktop/ui test
# when parsing/judging/model behaviour changed:
WOLLUF_CORPUS="/mnt/e/Games/osu!" cargo nextest run --workspace --run-ignored only
```
- A gate that fails is reported with its output. It is never hidden, skipped silently or "fixed" by loosening the test.
- Corpus runs read `/mnt/e/Games/osu!` **read-only**. Real maps, replays and DBs never land in git; derive synthetic or minimized fixtures instead.

## 5. Close
- Set the spec `Status: Done`, listing any deviations from the plan and the reasons.
- If the implementation changed a documented design, update `docs/architecture.md` in the same PR, together with its ADR.
- Update the project `CLAUDE.md` "Commands" or "Domain facts" sections only when a new durable fact appeared.

## Parallel work with workflows
Only tasks marked `[P]` touching disjoint crates may run as parallel agents in the same tree. Give each agent:
- the spec path;
- its task ids;
- the crate directories it may edit.

Root `Cargo.toml` and `[workspace.dependencies]` are edited only by the orchestrator.
