# 0024 Uncalibrated beta preview of skill and recommendations

- Status: Accepted
- Date: 2026-10-08

## Context
Testers (4K-heavy) should see a skill view and recommended maps in the beta, before F2 (judge) and F3 (skill model, recommender) exist (user request, 2026-10-08). The binding design rejects exactly the cheap version of this:
- ADR 0002 rejects "MSD bands around a weighted mean, as in Companella's recommender" as the skill model.
- Architecture §5.1 rejects crediting whole-chart accuracy to every pattern.
- §6 makes every shown recommendation list a `rec_impression`, and §8 names the real commands `skill_overview` and `recs_list`.

What is available now: MinaCalc v527 chart skillsets per rate (ADR 0022, 0023), and stable judgement counts per play in the ledger. Etterna's player rating is a published, MIT method over exactly those inputs: score-specific ratings (SSR) per play, top 2 rate-PBs per chart, `aggregate_skill(0.1, 1.05, 0, 10.24)` per skillset, Overall = mean of the 7 skillsets (`ScoreManager.cpp` CalcPlayerRating, SortTopSSRPtrs).

## Decision
A time-boxed preview, separate from the F3 contracts:
- **Namespace.** App slice `features/preview`, commands `preview_skill` and `preview_recs`, method ids `preview.etterna_rating@1` and `preview.band_recs@1`, output ids are MinaCalc's own skillset ids (`overall`, `stream`, …, the same as `chart_msd`). No `4k.*` or `7k.*` AxisId is frozen by it.
- **Persistence.** Nothing in user.db: no θ, no `rec_impression`. Per-play SSRs live in a disposable cache.db table keyed by vkey (D15), rebuilt from the vault.
- **Goal model.** stable stores no Wife%. Each judgement of a play scores the mean Wife3 J4 points (Etterna `RageUtil.h`, MIT) over its own error interval on stable's OD windows (rate-scaled `floor(base × rate)`, HR/EZ applied, asymmetric late side: a late hit past the 100 edge O−1 is a miss), a miss scores −5.5, and the goal is the points share capped at 0.965 like Etterna. A Gaussian σ fitted to the counts was tried first and rejected: whatever the fit, turning a miss into a 50 could lower the goal (a 1,850-MAX EZ play at OD 0 went from 9639 to 9632), and goals must be monotone in every judgement. Raw osu! accuracy is not used: on the pilot's 4K plays it inflates Overall from about 24.0 to 26.4.
- **Plays counted.** Only the selected identity scope (ADR 0005). Excluded and counted per reason: incomplete plays (judgement total below the chart's objects), ScoreV2, Random/key-conversion/co-op mods, LN-heavy and calc-rejected charts. Rate = mod rate × the chart's own rate (a rate copy is its own chart).
- **Both keymodes.** 4K and 7K show Overall plus a 7-skillset radar. 7K carries a stronger warning: MinaCalc's 7K logic is far less validated than its 4K one, its LN axes are not measured, and it links to the Label screen to help calibrate.
- **Dan.** 4K shows an estimated dan through a table fitted by wolluf on public dan-course charts (`docs/research/07-k4-dan-from-msd.md`). The table maps a chart's Overall MSD, so the player-side input is the Etterna aggregate of the counted plays' Overall SSRs (same scale), not the mean of the 7 skillset ratings. Daniel's table is not used: its numbers sit on a Sunny-derived rating that cannot be ported (no licence), not on MSD.
- **Recommendations.** Charts × rate whose skillset MSD falls in a band around the player's rating (Deficit: weakest skillset; Push: Overall), the skillset dominant in the chart, LN-heavy excluded, one pick per beatmapset, reasons as i18n codes. Default rates: NM, HT 0.75, DT 1.5 and rate copies already in the library. A setting "enable rates" adds every grid rate (0.70–1.50 step 0.05), shown with a "generate rate copy" action.
- **Labelling.** Every number is shown as "≈" with a "Beta · uncalibrated · MinaCalc 527" badge and a method disclosure.
- **Removal.** The preview is deleted, not migrated, when `skill_overview` (θ ± σ) and `recs_list` ship.

## Alternatives considered
- Companella's weighted mean of chart MSD by accuracy and recency: no evidence of accuracy, and the rejected ADR 0002 heuristic without the Etterna aggregation.
- Splitting 7K Overall onto jack/tech/speed/stream by pattern segment time: the engine has 0 gold labels and tech has almost no primary segments, so the split would look precise and be arbitrary.
- Waiting for F3: nothing for testers for weeks.

## Consequences
- Testers will anchor on preview numbers. The namespace, the badge and the removal rule contain that.
- The goal model can be ±0.5–1 pp off per play; the rating inherits that bias. It must be re-evaluated once the F2 re-judge gives real Wife3.
- MinaCalc version bumps recompute chart MSD and play SSRs (both keyed by `minacalc@<version>`).
