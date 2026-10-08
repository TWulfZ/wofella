# Skill preview (uncalibrated, MinaCalc + Etterna rating)

Branch `feat/beta-skill-preview` (single PR with `k4-difficulty-index`, `recs-preview`, `rate-copies`) · opened 2026-10-08

## Objective
The desktop Skill page shows, for the selected scope and keymode (4K and 7K), an Overall rating, a 7-skillset radar, a monthly trend, the plays that count, and for 4K an estimated dan, all marked "Beta · uncalibrated · MinaCalc 527".

## Problem and why
Testers should see a skill view before F2/F3 exist (user, 2026-10-08). ADR 0024 defines the method and the exceptions it takes to ADR 0002 and §5.1/§6/§8.

## Scope
- Authorized: `crates/engine/src/preview/**` and a `play_ssr` stage, `crates/store` (`play_ssr` table, cache schema stays 6 since it has not shipped), `crates/app/src/features/preview/**` + a `ComputePlaySsr` job, `apps/desktop/src-tauri` (`preview_skill`), `apps/cli` (`wolluf preview skill`), `apps/desktop/ui` (Skill route, preview slice, i18n en/es), `research/scripts/dan4k/**`, `docs/research/07-k4-dan-from-msd.md`, ADR 0024, NOTICE (Etterna rating + Wife3).
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
- `goal_permyriad(counts, od, mods, &GoalParams) -> Option<u16>`: Gaussian σ by maximum likelihood on stable V1 windows (`docs/research/03-…:121-127`; MAX 16, 300 `64-3·OD`, 200 `97-3·OD`, 100 `127-3·OD`, 50 `151-3·OD`; HR ÷1.4, EZ ×1.4 except MISS), real-time ms; expected Wife3 J4 of that σ with the observed miss share; capped at `GoalParams.cap = 0.965`.
- `aggregate_rating(ssrs: &[f32]) -> f32`: Etterna `AggregateScores` port (`0.1, 1.05` start, `10.24` res, 11 iterations as upstream), `libm::erfc`.
- `rate_pbs → top2_per_family → per-skillset rating`; Overall = mean of the 7 skillsets; result clamped 0–100.
- `family_key(set_folder, version) -> String`: version with a trailing rate tag (`1.15x`, `(207bpm)`, `[1.2x]`, `x1.2`) stripped; params-driven.
- `DanTable4k` (params): ordered `(label, lower_bound_centi)` with Low/Mid/High thirds; `estimate(overall_centi) -> Option<DanEstimate { label, third, margin_centi }>`.
- Stage `play_ssr` v1: key = STAGE, VERSION, `GoalParams` hash, the keymode's `difficulty` vkey. Golden over goal outputs for synthetic counts (no SSR floats).

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
- Goal model: monotone in each judgement; all-MAX ≈ cap; golden on synthetic counts → engine tests.
- Rating: hand-computed cases; adding a lower SSR never lowers a rating; two plays at one rate never both count; rate copies of one map share a family → engine tests.
- Pilot 4K Overall ≈ 24 (report, not gated); exclusion counts reported → corpus run.
- 4K dan table fitted on public dan-pack charts, leave-one-pack-out error reported → `docs/research/07-k4-dan-from-msd.md`.
- `preview_skill` returns ready data for self, only that scope's plays → app tests.
- UI: radar with N axes from the DTO, beta badge, method disclosure, top plays, 7K warning + Label CTA, computing state → vitest.

## Tasks
- [x] T1 — ADR 0024 (NOTICE entries land with the T2 port). Route: inline. Tier: high (exception to binding docs). Commit: `docs: accept an uncalibrated skill and recs preview (ADR 0024)`
- [ ] T2 — Engine `preview` domain + `play_ssr` stage. Route: delegated (engine owner). Tier: high (stage-lock). Commit: —
- [ ] T3 — 4K dan table: fit script, research note, params values. Route: delegated (research owner, after T2 and a corpus index). Tier: medium. Commit: —
- [ ] T4 — Store `play_ssr`. Route: delegated (store owner, parallel with T2). Tier: medium. Commit: —
- [ ] T5 — App: `ComputePlaySsr` job (follow-up of IndexLibrary and SyncPlays), `preview_skill` service. Route: delegated (app owner). Tier: high (identity scope). Commit: —
- [ ] T6 — Shells: `preview_skill` command, CLI `wolluf preview skill`. Route: delegated. Tier: medium. Commit: —
- [ ] T7 — UI Skill page. Route: delegated (ui owner). Tier: medium. Commit: —

## Progress

## Next step
Commit T1, then T2 ∥ T4.
