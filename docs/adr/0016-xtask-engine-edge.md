# 0016 xtask depends on wolluf-engine for stage goldens

- Status: Accepted
- Date: 2026-09-29

## Context
The stage lock pins `(VERSION, hash of the quantised golden outputs)` for every versioned stage, and CI fails when the goldens change without a VERSION bump (architecture §5.5, ADR 0006). The first stages, `chart_parse` and `chart_label`, arrived with `wolluf-engine` in F1. Until now `cargo xtask stage-lock` registered 0 stages, because xtask had no path to the engine. `xtask/layers.toml` allows xtask only `wolluf-source-osu` (fixtures), so adding the edge needs an ADR (D1).

## Decision
xtask depends on `wolluf-engine` with feature `test-support`, which exposes `stage::goldens()`. `cargo xtask stage-lock` and `--check` compute the lock from that list. The edge is added to `xtask/layers.toml`.

## Alternatives considered
- A test inside `wolluf-engine` compares its goldens with `stage_versions.lock`. Rejected: the lock would then be written by hand or by a second tool, and `stage-lock` (the command that writes it, and the one CI calls) would stay a stub.
- Goldens exported as a generated file that xtask reads. Rejected: one more generated artifact to keep in sync, with no gain over a dev-tool dependency.

## Consequences
- xtask is a dev tool that never ships, so the edge adds no runtime coupling. `test-support` code stays out of release builds of the app.
- A new versioned stage registers in both `wolluf_engine::stage::REGISTERED` and `stage::goldens()`, and gets its golden checked in CI without touching xtask. The engine test `goldens_cover_every_stage_and_are_deterministic` fails unless both lists hold the same `(id, VERSION)` pairs in the same order.
