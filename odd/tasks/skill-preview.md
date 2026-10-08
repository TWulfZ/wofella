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
- `warnings` codes: `uncalibrated`, `k7_less_validated`, `ln_not_measured`, `goal_estimated`, `k7_tech_not_measured` (7K Technical ≈ 0.18 on almost every chart). Skillset ids are MinaCalc's bare ids, as in `ChartMsdDto` (amended 2026-10-08).
- Dan input: `aggregate_rating` over the counted plays' Overall SSR (chart-Overall scale of the table), not the mean of the 7 skillsets.
- Play rate = mod rate × the chart's own rate when its difficulty name carries a rate tag (`family.rs`), so a rate copy and its original never share a (family, rate) slot by accident.
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
- [x] T5 — App: `ComputePlaySsr` job (follow-up of IndexLibrary and SyncPlays), `preview_skill` service. Route: delegated (app owner). Tier: high (identity scope). Commit: `feat(app): compute play SSRs and serve the skill preview`
- [x] T6 — Shells: `preview_skill` command, CLI `wolluf preview skill`. Route: delegated. Tier: medium. Commit: `feat(app): compute play SSRs and serve the skill preview` (same commit: the shells only compile with the service)
- [x] T7 — UI Skill page. Route: delegated (ui owner). Tier: medium. Commit: `feat(ui): show the uncalibrated skill preview`
- [x] T8 — Method fixes from the pilot: ScoreV2 rice plays counted with V2 windows (LN head/tail judgements), 7K Overall = mean of the skillsets MinaCalc measures (Technical excluded, params per keymode), dan input = Etterna aggregate of completed (non-NoFail) plays' chart Overall MSD at the played rate, `computing` only while a job is pending or queued, family tags without a parsable rate kept in the key, multiplier markers in params, identity coverage. ADR 0024 amended. Route: delegated (engine + app owner). Tier: high (identity, stage keys). Commit: `fix(preview): count ScoreV2, drop 7K Technical from Overall, rate dans by clears`

## Progress
- 2026-10-08 T2: RED (37 failing behaviour tests on stubs) → GREEN. Upstream `aggregate_skill` compiled with g++ 13 and matched bit for bit on 10 cases ([30] → 23.73, not ≈30); Wife3 within 1e-6 on 13 offsets; no per-skillset re-selection upstream (`SortTopSSRPtrs` l.911-937).
- 2026-10-08 T4: RED (11 failing) → GREEN, 102 store tests.
- 2026-10-08 Verifier (high): FAIL — Simpson missing step width (σ=20 gave 0.9930 vs 0.9777), miss→50 could lower the goal, symmetric windows, `family_key` panic on U+3000/U+00A0, exclusion params outside the vkey, erf A2 1 ulp off, tie-break by index, loose store invariant, v6 dev caches without `play_ssr`. One scoped correction: all fixed; the Gaussian σ fit still failed the monotone proptest (EZ OD0 1850 MAX/18 miss: 9639 → 9632), so the goal became per-judgement uniform-mean Wife3 (ADR 0024 amended). Reference plays at OD 8: 1500/400/50/15/5/10 → 9396, 500/400/150/60/20/15 → 7333 (scipy match). `cargo nextest run -p wolluf-engine -p wolluf-store` 264 passed; stage-lock ok, lock diff adds only `play_ssr`; clippy ok. NOTICE entry for the rating/Wife3 ports added by the parent.

- 2026-10-08 T3: RED (`ModuleNotFoundError: fit`; 2 Rust tests on the empty table) → GREEN (13 Python, 178 engine). Fit on 263 charts from 13 public packs (REFORM courses, 10th/Alpha practice, Journey β→γ, Gamma++); leave-one-pack-out exact 36.9%, within one dan 78.3%, MAE 0.886 (median-dan baseline MAE 1.829); jack packs read ~1 dan low, stamina/tech high. Table: 1st 13.20 … 10th 25.82, α 26.81, β 27.70, γ 29.19, δ 31.37, ε 33.61. Verifier caveat (carried into T5): the table maps a chart's Overall; a player's Overall is the mean of 7 skillset ratings, so the player-level input and its validation belong to T5. `stage-lock --check` clean.
- 2026-10-08 T5 attempt 1 failed: the parent's SendMessage to the workflow's T5 agent resumed a second instance; both edited `crates/app` and both stopped. Leftovers (duplicate `ComputePlaySsrSummaryDto`, `JobStageDto::{PlaySsr,Ssr}`, `is_pending`/`is_active`, `difficulty_keys`, `preview/{dto,params,testkit}.rs` without `job`/`service`) leave `wolluf-app` uncompilable. Shell `preview_skill` and CLI `wolluf preview skill` are written against `ctx.preview().skill(EntryRef, Keymode, Option<MergeMode>)` but unverified in the tree.

- 2026-10-08 T5/T6 redo: leftovers reconciled (one summary DTO, stage `play_ssr`, `is_active`, no dangling re-exports; SyncPlays → IndexLibrary → ComputePlaySsr). RED (1 engine, 12 app) → GREEN; engine+app+cli+desktop 571 passed before bindings; parent: `cargo xtask bindings` (+92/−3), `nextest -p wolluf-desktop` 29/29, `tsc --noEmit` clean. Verifier (high): no scope leak (self excludes a stronger second alias, All players includes it), user.db read-only, DTOs match. Pilot (fresh data dir, release): sync 5,292 plays, one job run 5,257 plays, 2,674 counted, 0 failures; rerun computes 0. 4K self ≈22.86, dan "10th low"; 7K self ≈20.82 with Technical 0.18. Findings that reopen the method (T8): 1,298 7K plays excluded as ScoreV2; 7K Technical drags Overall ≈3 down; the SSR-aggregate dan reads 2–3 dans below the pilot's completed REFORM courses (γ in Jack/Speed/Tech) because SSR punishes low accuracy while a dan clear is survival. Verifier lows carried into T8: `computing` forever when rows can never come (item error, cancelled job); tags stripped from the family without a parsable rate; `MULTIPLIER` const outside params; i18n `jobs.kind.compute_play_ssr` + `jobs.stage.play_ssr` (T7); identity coverage for separate/unticked/not_me.

- 2026-10-08 T7: RED (28 of 29 new tests on stubs) → GREEN; `pnpm tsc`, lint, vitest 76 files / 1076 tests. N-axis radar from the DTO (7K Technical dimmed when `k7_tech_not_measured`), dan chip, evidence, trend, top plays, warnings with Label CTA, computing/no_plays, one section per scope; job i18n keys added. Parent spot check: `tsc --noEmit` ok.

- 2026-10-09 T8: RED → GREEN. V2 MAX window `22.4−0.6·OD` / `24.9−1.1·OD` (floor +1e-6, rejudge.py), V2 tails approximated with note values; `OverallParams` per keymode (7K without Technical); dan input = Etterna-deduplicated aggregate of completed, non-NoFail plays' chart Overall MSD (`PlayMods.nf`); `computing` only while ComputePlaySsr is active, otherwise `ready` with a `pending` exclusion, and the preview enqueues the job itself when plays are pending (no stale empty view after a key bump); family tags kept unless they yield a rate, multiplier markers in `FamilyParams`; identity test: Separate splits two self aliases, an unticked alias never counts, a prefix alias present before a NotMe decision disappears after it. play_ssr VERSION 2 (lock: play_ssr only). Verifier (high) on the first pass: no scope leaks; defects fixed in the scoped correction (vacuous not_me, stale empty view). Parent: `cargo nextest run --workspace` 1314 passed / 20 skipped, stage-lock ok, clippy ok, fmt ok after `cargo fmt`, bindings regenerated (+2/−2). Corpus re-run pending (osu! was open).

- 2026-10-09 UI follow-up: cached ratings stay on screen with "Updating ratings…" while computing; ready-with-only-pending shows a note. RED (3) → GREEN, vitest 76 files / 1079. Commit `feat(ui): keep cached skill ratings on screen while they update`. Env: non-interactive pnpm needs `export PATH="$HOME/.local/share/fnm/aliases/default/bin:$PATH"` (else the Windows Node is picked up).
- 2026-10-09 Pilot (fresh data dir, release CLI, osu! closed): sync 6 m 32 s. 4K self ≈22.86, dan ≈Gamma (high) — consistent with the completed REFORM γ courses (was "10th low" with the SSR input); 154 counted (incomplete 9, LN-heavy 11, rejected 2). 7K self ≈24.29 with Technical out of Overall; 673 counted (ScoreV2 now included; was 577); incomplete 56, LN-heavy 1,138. Trends rise 14.4 (2022-04) → 22.9 (4K) and 5.0 (2022-08) → 24.3 (7K).

## Next step
Close the feature (full gates) together with recs-preview.
