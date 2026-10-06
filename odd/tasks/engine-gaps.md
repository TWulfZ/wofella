# 7K engine gap fixes

Branch `feat/engine-gaps` from `feat/name-hints` @ b9347c3 · opened 2026-10-05

## Objective
The 7K engine gives segmented time to every Regular axis (tech included), detects trills and delay, and stops over-firing burst, with each change justified by name-hint lift on the pilot library.

## Problem and why
The pilot corpus run (F1 D2/name hints) segmented 47.3% of 18,333 7K charts' time but:
- tech axis has 0% segmented time: all tech rules are `tag_only` (`SegmentParams.tag_only`), and `library hints` counts primaries only;
- trill/jumptrill ≈ 0%;
- burst takes 12.7% of segmented time with hint lift < 1 (it outranks every stream shape and accepts 1.5× local pace);
- delay, the BMS ディレイ shape that Jinjin 7K dans file under "speed", has no id. Approved by the pilot 2026-10-05; seed from mania-hub `algorithms/dan-estimator/features.ts` `offGridRowShare` (MIT).

Rejected: adopting mania-hub's chart-level detectors as the runtime (they are chart-level, no hand model); they stay a seed and a future head-to-head oracle.

## Scope
- Authorized: `crates/patterns`, `crates/engine` (taxonomy, hints, patterns stage, goldens), `crates/app` library hint report, `docs/adr/` (0017 amendment), `stage_versions.lock`, NOTICE.
- Out of scope: head-to-head eval against mania-hub (queue item 2), difficulty (D3), player-model use of tags.

## Constraints
- Thresholds in param structs (D17); new id appended, never renumbered (ADR 0017).
- patterns `VERSION` bump + stage-lock when outputs change.
- MIT port credited in NOTICE.
- Depends on: PR #9 (`feat/name-hints`, retargeted to main, open).
- TDD: strict. Runner: `cargo nextest run -p wolluf-patterns`, `-p wolluf-engine`, `-p wolluf-app`.
- Delivery: ~600 authored lines forecast.

## Acceptance criteria
- `regular.speed.delay` exists, detected on staggered fine-grid runs, not on 1/4 streams → patterns unit tests + taxonomy snapshot.
- Tech axis has segmented time and a hint lift > 1 on tech-hinted charts → `wolluf library hints` on the pilot copy.
- ~~Trill/jumptrill segmented time > 0 with lift > 1~~ dropped 2026-10-05 (see Progress).
- Burst share drops and its lift is reported → same.

## Tasks
- [x] T1 — Baseline: record `library hints` / `library patterns` on a copy of the pilot data dir. Route: inline. Tier: passive. Commit: none (measurement only; numbers below)
- [x] T2 — `regular.speed.delay`: ADR 0017 amendment, taxonomy id + key `d`, rule + params, priority, `delay` name hint, golden fixture, NOTICE. Route: inline (one non-trivial file, rest mechanical). Tier: medium. Commit: `feat(patterns): add regular.speed.delay for off-grid staggered flow`
- [x] T3 — Tech primary: `regular.tech.irregular` competes for rows (priority after delay, before the generic streams). Route: inline. Tier: medium. Commit: `feat(patterns): let irregular timing own segments on the tech axis`
- [ ] T4 — Burst retune (and trill only if evidence appears), driven by measurement. Route: inline. Tier: medium. Commit: —
- [ ] T5 — Close: VERSION bump, stage-lock, full gates, corpus `corpus_patterns`, before/after table in `docs/research/`, remove this document. Tier: medium. Commit: —

## Progress
- 2026-10-05 T1 baseline (scratch copy of the pilot data dir, 18,333 7K charts, patterns v1): speed axis lift 0.57 (58 hinted), tech 0.0% / no lift (54), burst 512,578 segments averaging 0.33 s, lift 0.66 (23); trill 0.0% (9 hinted), jumptrill 0.0% (9).
- 2026-10-05 Accepted change: trill relaxation dropped from scope. The 18 trill/jumptrill-hinted charts are 4 distinct charts plus rate copies; they show bracket shapes, a [123]/[4567] alternation (`split_trill` under ADR 0017) and a joke roll, so the 0% is vocabulary granularity on a tiny sample, not an `alternations` bug. mania-hub's `trillRunShare` also requires exact alternation.
- 2026-10-05 T2: mutation RED (`delay_min_divisor` 8 + floor −10 ms → 2 of 5 delay tests fail) → GREEN. `cargo nextest run --workspace`: 861 passed. clippy: clean. fmt: clean. `cargo xtask stage-lock --check`: ok (patterns v2, chart_label v3). Pilot copy: delay hint lift 6.02 (47 charts, 25.2% vs 4.2%), speed axis lift 0.57 → 1.30.
- 2026-10-05 T3: RED `delay_and_irregular_beat_the_single_stream_they_read_as` fails with irregular back in `tag_only` → GREEN. `cargo nextest run --workspace`: 862 passed. clippy, fmt, `stage-lock --check` (patterns v3): ok. Pilot copy: tech axis lift – → 1.82 (53 charts, 4.2% vs 2.3%), irregular 9,220 segments / 25,113 s; no other lift fell (delay 6.02 → 6.25, ln.tech 1.83 → 1.60 is the only drop, within noise of 15 charts). Tag-time reporting dropped: tech is measurable without it.
- 2026-10-05 Accepted change: the hint report keeps counting primaries only.

## Next step
T4: burst retune. Try `burst_min_density_ratio_permille` 2000 and/or moving burst below the stream shapes, re-index the scratch copy (`wolluf --data-dir <scratch>/dd library index`), compare burst count/seconds and speed lift.
