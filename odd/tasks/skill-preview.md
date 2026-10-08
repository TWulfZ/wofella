# Skill preview (uncalibrated, MinaCalc + Etterna rating)

Branch `feat/beta-skill-preview` (single PR with `k4-difficulty-index`, `recs-preview`, `rate-copies`) · opened 2026-10-08

## Objective
The desktop Skill page shows, for the selected scope and keymode (4K and 7K), an Overall rating, a 7-skillset radar, a monthly trend, the plays that count, and for 4K an estimated dan, all marked "Beta · uncalibrated · MinaCalc 527".

## Problem and why
Testers should see a skill view before F2/F3 exist (user, 2026-10-08). ADR 0024 defines the method and the exceptions it takes to ADR 0002 and §5.1/§6/§8.

## Scope
- Authorized: `crates/engine/src/preview/**` and a `play_ssr` stage, `crates/store` (`play_ssr` table, cache schema 7: dev data dirs already held a v6 cache without it), `crates/app/src/features/preview/**` + a `ComputePlaySsr` job, `apps/desktop/src-tauri` (`preview_skill`), `apps/cli` (`wolluf preview skill`), `apps/desktop/ui` (Skill route, preview slice, i18n en/es), `research/scripts/dan4k/**`, `docs/research/07-k4-dan-from-msd.md`, ADR 0024, NOTICE (Etterna rating + Wife3).
- Out of scope: recommendations (feature `recs-preview`), rate-copy generation, any user.db write, θ/σ.

## Constraints
- Nothing persisted in user.db; `play_ssr` rows in cache.db keyed by vkey (ADR 0024, D15).
- Only the selected scope's plays feed a rating (ADR 0005; `PlayersService::resolve_scopes`, `crates/app/src/features/players/service.rs:152`).
- Calculator floats never in a golden (ADR 0022); the goal model uses `libm` only (D3) and may be golden-tested.
- Thresholds in params (D17): goal cap 0.965, aggregation constants, evidence tiers, exclusion rules, family rule, dan table.
- Ports credited in NOTICE in the same commit (Etterna `ScoreManager.cpp` rating, `RageUtil.h` Wife3; MIT).
- TDD strict. Runners: `cargo nextest run -p <crate>`, `pnpm -C apps/desktop/ui test`.

### Frozen contracts
Engine `wolluf_engine::preview` (pure):
- `PlayMods::from_bits(i32)`: `ez, hr, dt, ht, nc, random, coop, score_v2, mirror, key_mod` (bits in `research/scripts/rejudge/osr_wiki.md:65-101`); `rate_milli()` = 1500 DT/NC, 750 HT, else 1000.
- `Completeness::of(counts, score_system, n_objects, n_ln) -> Complete | Incomplete`: V1 total ≥ `n_objects` (a V1 total of `n_objects + n_ln` is complete: research 03 l.165); V2 total ≥ `n_objects + n_ln`.
- `goal_permyriad(counts, od, mods, &GoalParams) -> Option<u16>`: each judgement scores the mean Wife3 J4 points over its error interval (uniform density) on stable V1 windows (`docs/research/03-…:121-127`; MAX 16, 300 `64-3·OD`, 200 `97-3·OD`, 100 `127-3·OD`, 50 `151-3·OD`, early side only; late 100 edge O−1; HR ÷1.4, EZ ×1.4 except MISS; 0.5 ms pad), miss −5.5; capped at `GoalParams.cap = 0.965`. Monotone in every judgement (amended 2026-10-08, ADR 0024).
- `aggregate_rating(ssrs: &[f32]) -> f32`: Etterna `AggregateScores` port (`0.1, 1.05` start, `10.24` res, 11 iterations as upstream), `libm::erfc`.
- `rate_pbs → top2_per_family → per-skillset rating`; Overall = mean of the 7 skillsets; result clamped 0–100.
- `family_key(set_folder, version) -> String`: version with a trailing rate tag (`1.15x`, `(207bpm)`, `[1.2x]`, `x1.2`) stripped; params-driven.
- `DanTable4k` (params): ordered `(label, lower_bound_centi)` with Low/Mid/High thirds; `estimate(overall_centi) -> Option<DanEstimate { label, third, margin_centi }>`.
- `RatedPlay` carries `played_at_ms`; ties go to the earlier play.
- Stage `play_ssr` v1: key = STAGE, VERSION, `GoalParams` hash, `ExclusionParams` hash, the keymode's `difficulty` vkey (`vkey(difficulty_vkey, &GoalParams, &ExclusionParams)`). Golden over goal outputs for synthetic counts (no SSR floats).

Store (cache.db):
```sql
CREATE TABLE play_ssr (
  play_id BLOB NOT NULL CHECK (length(play_id) = 32), vkey BLOB NOT NULL CHECK (length(vkey) = 32),
  status TEXT NOT NULL CHECK (status IN ('counted','incomplete','score_v2','unsupported_mods','ln_heavy','calc_rejected','no_chart')),
  rate_milli INTEGER NOT NULL, goal_permyriad INTEGER NULL,
  overall INTEGER NULL, stream INTEGER NULL, jumpstream INTEGER NULL, handstream INTEGER NULL,
  stamina INTEGER NULL, jackspeed INTEGER NULL, chordjack INTEGER NULL, technical INTEGER NULL,
  PRIMARY KEY (play_id, vkey)) STRICT, WITHOUT ROWID;
```

IPC (camelCase): `preview_skill(entry: EntryRefDto, keymode: u8, merge: Option<MergeModeDto>) -> Vec<SkillPreviewDto>` (one per resolved scope):
- `SkillPreviewDto { scopeHash, keymode, method: "preview.etterna_rating@1", calcVersion, state: "ready" | "computing" | "no_plays", overallCenti: Option<i32>, skillsets: Vec<SkillsetRatingDto { id, ratingCenti }>, dan: Option<DanEstimateDto { label, third: "low"|"mid"|"high", marginCenti }>, evidence: EvidenceDto { counted: u32, tier: "low"|"medium"|"ok", excluded: Vec<ExclusionCountDto { reason, count }> }, topPlays: Vec<TopPlayDto { playId, md5, title, version, rateMilli, goalPermyriad, overallCenti, dominantSkillset, playedAtMs }>, trend: Vec<TrendPointDto { month: "YYYY-MM", overallCenti }>, warnings: Vec<String> }`.
- `warnings` codes: `uncalibrated`, `k7_less_validated`, `ln_not_measured`, `goal_estimated`.
- `DataChanged` domain `preview` after `ComputePlaySsr`.

## Acceptance criteria
- Goal model: monotone in each judgement (proptest over counts, OD, mods); all-MAX ≈ cap; reference plays match an independent numeric model; golden on synthetic counts → engine tests.
- Rating: hand-computed cases; adding a lower SSR never lowers a rating; two plays at one rate never both count; rate copies of one map share a family → engine tests.
- Pilot 4K Overall ≈ 24 (report, not gated); exclusion counts reported → corpus run.
- 4K dan table fitted on public dan-pack charts, leave-one-pack-out error reported → `docs/research/07-k4-dan-from-msd.md`.
- `preview_skill` returns ready data for self, only that scope's plays → app tests.
- UI: radar with N axes from the DTO, beta badge, method disclosure, top plays, 7K warning + Label CTA, computing state → vitest.

## Tasks
- [x] T1 — ADR 0024 (NOTICE entries land with the T2 port). Route: inline. Tier: high (exception to binding docs). Commit: `docs: accept an uncalibrated skill and recs preview (ADR 0024)`
- [x] T2 — Engine `preview` domain + `play_ssr` stage. Route: delegated (engine owner). Tier: high (stage-lock). Commit: `feat(engine): rate players with an Etterna-style preview`
- [x] T3 — 4K dan table: fit script, research note, params values. Route: delegated (research owner, after T2 and a corpus index). Tier: medium. Commit: `feat(engine): estimate 4K dans from MinaCalc Overall`
- [x] T4 — Store `play_ssr`. Route: delegated (store owner, parallel with T2). Tier: medium. Commit: `feat(store): cache per-play SSRs for the preview`
- [ ] T5 — App: `ComputePlaySsr` job (follow-up of IndexLibrary and SyncPlays), `preview_skill` service. Route: delegated (app owner). Tier: high (identity scope). Commit: —
- [ ] T6 — Shells: `preview_skill` command, CLI `wolluf preview skill`. Route: delegated. Tier: medium. Commit: —
- [ ] T7 — UI Skill page. Route: delegated (ui owner). Tier: medium. Commit: —

## Progress
- 2026-10-08 T2: RED (37 failing behaviour tests on stubs) → GREEN. Upstream `aggregate_skill` compiled with g++ 13 and matched bit for bit on 10 cases ([30] → 23.73, not ≈30); Wife3 within 1e-6 on 13 offsets; no per-skillset re-selection upstream (`SortTopSSRPtrs` l.911-937).
- 2026-10-08 T4: RED (11 failing) → GREEN, 102 store tests.
- 2026-10-08 Verifier (high): FAIL — Simpson missing step width (σ=20 gave 0.9930 vs 0.9777), miss→50 could lower the goal, symmetric windows, `family_key` panic on U+3000/U+00A0, exclusion params outside the vkey, erf A2 1 ulp off, tie-break by index, loose store invariant, v6 dev caches without `play_ssr`. One scoped correction: all fixed; the Gaussian σ fit still failed the monotone proptest (EZ OD0 1850 MAX/18 miss: 9639 → 9632), so the goal became per-judgement uniform-mean Wife3 (ADR 0024 amended). Reference plays at OD 8: 1500/400/50/15/5/10 → 9396, 500/400/150/60/20/15 → 7333 (scipy match). `cargo nextest run -p wolluf-engine -p wolluf-store` 264 passed; stage-lock ok, lock diff adds only `play_ssr`; clippy ok. NOTICE entry for the rating/Wife3 ports added by the parent.

- 2026-10-08 T3: RED (`ModuleNotFoundError: fit`; 2 Rust tests on the empty table) → GREEN (13 Python, 178 engine). Fit on 263 charts from 13 public packs (REFORM courses, 10th/Alpha practice, Journey β→γ, Gamma++); leave-one-pack-out exact 36.9%, within one dan 78.3%, MAE 0.886 (median-dan baseline MAE 1.829); jack packs read ~1 dan low, stamina/tech high. Table: 1st 13.20 … 10th 25.82, α 26.81, β 27.70, γ 29.19, δ 31.37, ε 33.61. Verifier caveat (carried into T5): the table maps a chart's Overall; a player's Overall is the mean of 7 skillset ratings, so the player-level input and its validation belong to T5. `stage-lock --check` clean.
- 2026-10-08 T5 attempt 1 failed: the parent's SendMessage to the workflow's T5 agent resumed a second instance; both edited `crates/app` and both stopped. Leftovers (duplicate `ComputePlaySsrSummaryDto`, `JobStageDto::{PlaySsr,Ssr}`, `is_pending`/`is_active`, `difficulty_keys`, `preview/{dto,params,testkit}.rs` without `job`/`service`) leave `wolluf-app` uncompilable. Shell `preview_skill` and CLI `wolluf preview skill` are written against `ctx.preview().skill(EntryRef, Keymode, Option<MergeMode>)` but unverified in the tree.

## Next step
Redo T5 with one owner: reconcile the leftovers, finish job + service, then verify the shells in the tree.
