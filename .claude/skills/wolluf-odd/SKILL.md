---
name: wolluf-odd
description: Organic Driven Development (ODD) workflow for the wolluf repo. Use for every wolluf request that asks to change the repo (code, tests, migrations, IPC, params, fixtures, CI, docs), including roadmap phase work (F1–F5), features, bug fixes and refactors, and when resuming interrupted wolluf work. After exploring, small understood work is done directly with no documents; substantial work keeps exactly one odd/tasks/<feature-name>.md. Then test-first tasks, applicable gates and a work-unit commit per task.
---

# wolluf Organic Driven Development

Small, understood work stays small and leaves no documents. Substantial work keeps **one** feature document, removed at close. `CLAUDE.md` hard rules, architecture §4 (D1–D17) and `docs/conventions.md` bind every change and are not restated here.

Run these steps in order on every change request, without being asked.

## 1. Authorize
- Explaining, investigating, reviewing, auditing, comparing, proposing and planning are **read-only** unless the user explicitly asks for implementation: no file writes, no writer agents, no feature document. A requested plan goes in the reply.
- If it is unclear whether a change is wanted, ask one question and stay read-only until it is answered.

## 2. Explore
Before proposing or writing, in proportion to the request, read:
- the `docs/architecture.md` sections the change touches, plus §12 for phase work;
- `docs/adr/` and `docs/research/` for the domain facts involved (grep them, never recall them);
- the code and tests involved.

`docs/specs/` is the F0 record: read it for context. The only edit allowed there is ticking the open items in `000-f0-index.md` "Close status" as they are resolved.

Reuse an investigation already done in this session, or recorded in the feature document, instead of repeating it.

## 3. Resolve uncertainty, only when it is real
- Research only a **named** uncertainty: `docs/research/` and the `research/scripts/` oracles before the web.
- Ask **one** focused question only for a product decision that neither `docs/research/` nor the architecture answers. Then stop and wait.
- Challenge at most one premise, and only when it is consequential and unproven.
- A deterministic failure needs a fix, not a debate. There is no questionnaire and no proposal phase.

## 4. Classify (after exploring)
Count the **work-unit commits** the work needs. One work-unit commit is one independently verifiable behaviour with its tests.
- **Substantial**: two or more, or work likely to be interrupted before it is done (it spans sessions, or waits mid-way on a user decision). Go to §5.
- **Small**: exactly one, and already understood. Skip §5: if on `main`, create `<type>/<feature-name>` first, then make one commit there.
- Risk, file count and line count never decide this. Risk sets the verification tier and file count sets the route (both §6).
- A roadmap phase is never one feature. Each independently mergeable §12 deliverable gets its own feature document; there is no phase document.
- Reclassify when the facts change: if small work turns out to need a second commit, stop and do §5 before the next write.

## 5. Track before the first write (substantial only)
1. If on `main`, create the branch `<type>/<feature-name>` (Conventional Commit type, kebab-case name). When resuming, reuse the existing branch and name.
2. Create `odd/tasks/<feature-name>.md` from `templates/feature.md` without asking permission, then tell the user in one line: the path and the number of tasks. Never overwrite another feature's document.
3. It is the only planning artifact: no separate plan, spec, design or task file. Record rationale only for meaningful accepted changes; it is not a decision journal.
4. The document goes into each task's work-unit commit, so git is its recovery copy.
5. When the user, a verification or a gate changes the intent: keep valid completed and unrelated work; add genuinely new tasks, or reopen invalidated ones with a reason, and revise their checks. Findings alone never widen scope. A business-scope change needs the user's approval.
6. Code comments, ADRs and research files never cite the feature document, because it is removed at close. They cite the ADR, research line or architecture section instead.

**ADRs** stay a separate artifact. Write one from `templates/adr.md`, in the commit of the task that needs it, only when ADR 0001's required list applies: a crate edge or `layers.toml` change, storage or a persisted encoding, the θ scale, a cross-crate rule, a re-litigable alternative, or a deviation from the architecture.

## 6. Implement task by task
**Route.** Each task takes one route; for substantial work, record it on the task line.
- *Direct inline*: deciding or verifying needs 1–3 files, or the change is one mechanical, understood file with no open design question.
- *Delegated direct*, mandatory when a trigger fires:
  - understanding needs 4 or more files → one mapping agent;
  - writing touches 2 or more non-trivial files → one bounded writer (a mechanical second-file edit does not count);
  - broad research, or reading that prepares a write → one agent.
- Backstop: after about 20 tool calls, 5 exploratory reads or 2 non-mechanical edits without delegating, delegate the next bounded unit.
- Size and risk alone never force delegation.

**Test-first.** Strict TDD is on; the source is `docs/conventions.md` "Tests and gates".
- Behaviour: observe **RED** (a failing assertion, or a build failure on the API that does not exist yet), then **GREEN**, then refactor with the tests green.
- Runners: `cargo nextest run -p <crate> <filter>` and `pnpm -C apps/desktop/ui test`. A nextest filter matches test names, not the integration-test binary, so run a whole `tests/<name>.rs` with `--test <name>` (spec 006 T7).
- When there is no meaningful runnable RED (passive docs, a pure refactor that must stay green, mechanical config, a Python oracle whose output is the check, an `#[ignore]` corpus harness), state the exception and run the proportionate check instead.
- Never invent RED/GREEN evidence or a runner.

**Gates.** Run focused checks while iterating. At task close, run every gate the change touches; the commands are in `CLAUDE.md` "Commands".

| Change touches | Gates at task close |
|---|---|
| any Rust | fmt, clippy, `nextest run --workspace` |
| `Cargo.toml`, `Cargo.lock` or `xtask/layers.toml` | + check-layers, deny |
| `clippy.toml` or `[workspace.lints]` | + lint-canary |
| a versioned stage's output | + `VERSION` bump, `cargo xtask stage-lock`, then `--check` |
| app DTOs or commands | + `cargo xtask bindings`, the drift check, and `bindings.ts` committed |
| `apps/desktop/ui` | tsc, lint, vitest |
| codecs, sync, identity, judging, patterns, difficulty or model behaviour | + the relevant corpus harness (`--run-ignored only`, with osu! closed) |
| fixture generators | rerun them; `git diff fixtures/` stays empty |
| docs only | readback: links resolve and claims match their cited sources |

A bug found in the corpus gets a synthetic regression fixture. A failing gate is reported with its output. It is never hidden, silently skipped or "fixed" by loosening a test. A pre-existing failure is named as pre-existing, with evidence that it also fails on `main`.

**Verification tier**, chosen per work-unit commit and never per checkbox:
- **Passive** (docs, comments, the feature document): readback.
- **Medium** (an ordinary behaviour change): whoever wrote it runs the checks and reports `<command>: <observed result>`.
- **High or unclear**: medium, plus an independent read-only verifier agent that checks the diff against the task's acceptance and architecture §4. High covers `app::export` and `ExportPermit`, migrations, persisted encodings (`PlayId`, `VersionKey`), identity-scope selection (ADR 0005), telemetry consent (D10), `layers.toml`, a breaking IPC change, and licensing or NOTICE.
- When a writer agent did the work, the parent re-runs one reported command as a spot check.
- Allow at most one scoped correction per verified change. Do not loop until clean.

**Check-off and commit**
- Tick a task only after its outcome and checks were **observed**. Record failed, skipped, unavailable or pending checks as such. A checkbox grants no approval.
- Each task closes with at least one work-unit commit that follows `docs/conventions.md` "Commits". Tests, docs and the feature document go in the same commit as the behaviour. Record the commit subject on the task line.
- Committing is part of this workflow. Push, PR and merge are always the user's decision.
- Delivery: if the forecast or the running count of authored changed lines (generated files excluded) passes about 400, ask once whether to ship a single PR or stacked slices. 400 is a heuristic, not a cap: never split artificially, drop tests or minify to fit under it.

## 7. Close
- When a task or the feature ends, report the verified outcome, every failed, skipped or pending check, the commits, and the next step.
- Feature close is the last task:
  1. Run the full `CLAUDE.md` gate block, plus the corpus run when behaviour changed.
  2. Move durable knowledge to its permanent home, only if some appeared: a binding design change → `docs/architecture.md` + an ADR; a verified domain fact → `docs/research/`; a new command or easy-to-miss fact → `CLAUDE.md`.
  3. `git rm odd/tasks/<feature-name>.md` in that last work-unit commit; git history keeps it. When the user drops a feature, its document is removed the same way.

## Resume
- Never infer active work from memory. Find it with `ls odd/tasks/` on the current branch or, on `main`, `git branch --list` for a `<type>/<feature-name>` branch.
- Read the feature document, then `git log main..HEAD`, then the code and tests it names. Reconcile first: reopen, with a reason, any ticked task whose proof no longer holds. Then continue with the next unfinished task.
- If the working tree and the committed document disagree irreconcilably, keep both versions and ask only about the conflict.

## Delegated and parallel agents
The parent reads and reconciles the feature document, then gives each agent a prompt with these headings:
- `## Feature document`: the path, which the agent reads before any edit (omitted for small work);
- `## Task`: the task IDs, their acceptance, and the constraints that apply;
- `## Allowed edit surfaces`: exact repo-relative paths or narrow globs, never `.` or an absolute path;
- `## Verification`: the exact commands, plus TDD mode and runner;
- `## Known environmental failures`: only when there are any.

Rules:
- Parallel agents need disjoint write sets: one owner per crate or `apps/*` directory at a time.
- Only the parent edits the root `Cargo.toml`, `Cargo.lock`, `bindings.ts` (via `cargo xtask bindings`), `docs/architecture.md`, `CLAUDE.md` and the feature document.
- Agents do not commit. The parent stages only the task's paths and makes the work-unit commit, because concurrent commits race on the shared git index.
