# Recommendations preview (MinaCalc bands)

Branch `feat/beta-skill-preview` (single PR with `k4-difficulty-index`, `skill-preview`, `rate-copies`) · opened 2026-10-08

## Objective
The desktop Recommended page lists charts × rate near the player's preview rating for 4K and 7K, in Deficit (weakest skillset), Push (Overall) and Skillset (chosen) modes, with a reason per pick; a setting "Enable rates" widens the rates from NM/HT/DT and existing rate copies to the whole 0.70–1.50 grid.

## Problem and why
Testers asked for recommended maps in the beta (user, 2026-10-08). ADR 0024 defines the band method and why no `rec_impression` is written yet.

## Scope
- Authorized: `crates/engine/src/preview/recs.rs` (+ params), `crates/app/src/features/preview` (recs service, the setting), `crates/app/src/features/settings` (the `preview.recs.any_rate` key), `apps/desktop/src-tauri` (`preview_recs`, setting commands if needed), `apps/cli` (`wolluf preview recs`), `apps/desktop/ui` (Recommended route, settings toggle, i18n).
- Out of scope: generating rate copies (feature `rate-copies`; until it lands the UI shows the action disabled), `rec_impression`, any learning from outcomes.

## Constraints
- Same namespace, labelling and removal rule as ADR 0024. No user.db rows except the one boolean setting.
- Only the selected scope's rating and played-chart set (ADR 0005).
- Thresholds in params (D17): band, dominance margins, per-keymode excluded focus skillsets, limits, rate sets.
- Deterministic ordering (D3): ties by md5 then rate.
- TDD strict.

### Frozen contracts
Engine `wolluf_engine::preview::recs` (pure):
- `RecsParams { band_lo_centi: -50, band_hi_centi: 150, push_target_centi: 50, dominance_margin_centi: 0, tech_dominance_margin_centi: 150, excluded_focus: per keymode (7K: technical, stamina; 4K: stamina), limit: 30, base_rates_milli: [750, 1000, 1500], grid_rates_milli: 700..=1500 step 50 }`.
- `Mode { Deficit, Push, Skillset(usize) }`; Deficit focus = lowest rated skillset not in `excluded_focus`.
- `recommend(rating: &PlayerRating, candidates: &[Candidate], played: &BTreeSet<ChartMd5-like>, mode, any_rate: bool, &RecsParams) -> RecsOut { focus, band: (lo, hi), items: Vec<RecItem> }`; `Candidate { md5, set_key, rate_milli, centi: [i32; 8], is_rate_copy: bool }`.
- Picks: focus value in `[R + lo, R + hi]` (Push: Overall around `R + push_target`); focus skillset dominant (≥ every other non-overall skillset − margin; Technical needs its own margin); one pick per `set_key`; unplayed first, then closeness to the band target; `needs_rate_copy` when the rate is not in `base_rates_milli` and the chart is not already a rate copy.
- Reasons: codes `deficit`, `push`, `skillset`, `unplayed`, `played_before`, `needs_rate_copy`, `rate_copy_in_library`, with args.

IPC (camelCase): `preview_recs(entry: EntryRefDto, keymode: u8, mode: "deficit" | "push" | "skillset", skillset: Option<String>, merge: Option<MergeModeDto>) -> RecsPreviewDto`:
- `RecsPreviewDto { scopeHash, keymode, method: "preview.band_recs@1", calcVersion, state: "ready" | "computing" | "no_rating", anyRate: bool, focus: String, ratingCenti: i32, bandCenti: [i32; 2], items: Vec<RecItemDto>, warnings: Vec<String> }`.
- `RecItemDto { md5, title, artist, version, creator, setId: Option<i32>, beatmapId: Option<i32>, rateMilli, needsRateCopy, isRateCopy, focusCenti, overallCenti, skillsetsCenti: Vec<i32>, played: bool, reasons: Vec<ReasonDto { code, args: Vec<String> } > }`.
- Setting: `preview.recs.any_rate` (bool, default false), read and written through the settings service; exposed in Settings as "Enable rates".

## Acceptance criteria
- Band, dominance, one-per-set, unplayed-first, rate gating and determinism → engine tests (hand-built candidates).
- `preview_recs` uses only the scope's rating and plays; `anyRate` follows the setting → app tests.
- Corpus report: pilot 4K and 7K lists (top 10 per mode), not gated.
- UI: mode tabs, skillset picker, cards with rate badge, reasons, "Generate rate copy" shown for `needsRateCopy` (disabled until `rate-copies`), Settings toggle → vitest.

## Tasks
- [x] T1 — Engine `preview::recs`. Route: delegated (engine owner). Tier: medium. Commit: `feat(engine): recommend charts in a band around the preview rating`
- [x] T2 — App recs service + setting; shells (`preview_recs`, CLI `wolluf preview recs`); bindings. Route: delegated (app, desktop, cli owners). Tier: medium. Commit: `feat(app): serve band recommendations and the any-rate setting`
- [ ] T3 — UI Recommended page + Settings toggle. Route: delegated (ui owner). Tier: medium. Commit: —

## Progress
- 2026-10-08 T1: RED (13 of 14 failing on an empty `recommend`) → GREEN, engine 177/177 at the time; clippy and fmt clean. `RecsParams` has no `Default`: `RecsParams::for_keymode(k)` (7K excludes stamina and technical, others stamina) because `recommend` takes no keymode.

- 2026-10-09 T2: RED (compile on the new API) → GREEN. `PreviewService::recs(entry, keymode, mode, skillset, merge, any_rate_override)` (None reads `preview.recs.any_rate`); candidates from `chart_msd::rated_at` per allowed rate; one pick per `set:<id>` or `folder:<folder>`; Separate uses the first resolved scope; recs start the SSR job when plays are pending. Tauri `preview_recs`, `settings_get/set_recs_any_rate`; CLI `wolluf preview recs [--keys N] [--mode …] [--skillset ID] [--any-rate]`. Verifier (ADR 0005): scope isolation holds; one scoped correction (job-wiring test through the public API, single override entry point, `PreviewServiceParams.recs` injection, stronger Separate test; each new test proven by a deliberate break). app+cli+desktop 418/419 before bindings; parent: `cargo xtask bindings`, desktop tests, `tsc --noEmit`. Played is per chart md5 (a rate copy does not mark its original), as contracted.

## Next step
Start after `skill-preview` T5 (the rating service it reads).
