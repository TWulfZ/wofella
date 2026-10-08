# 0023 K4 profile without a pattern taxonomy, and a MinaCalc difficulty stage

- Status: Accepted
- Date: 2026-10-08

## Context
The beta preview (ADR 0024) needs MinaCalc skillsets for every 4K and 7K chart in the library, and 4K must reach testers before its pattern vocabulary exists. Architecture §9.1 plans 4K as one step: a `k4.toml` profile with axes, rules, calculators and thumb presets, scheduled in F5. Three facts change that plan:
- Profiles are Rust constants (`crates/engine/src/profile.rs`), not TOML files.
- 4K has no thumb: Prelude (`Layout.fs:8`), Quaver and `k4.generic` all split the hands 2|2.
- The 7K patterns stage cannot take 4K rules without re-keying every 7K segment: its config hash covers every rule in `rules::all()`, and its golden pins "4K yields no segments" (`crates/engine/src/stage/golden.rs:236-240,562-566`).

## Decision
- `KeymodeProfile` gains `taxonomy: Option<&'static [PatternDef]>` and `calculators: &'static [CalcId]`. A profile without a taxonomy is indexed and rated, but is never segmented, labelled or prompted for session labels. The compiler makes every consumer handle that case.
- K4 is enabled now with `k4.generic`, no thumb presets, `label_sources = false`, `taxonomy = None` and `calculators = [minacalc]`. K7 keeps its taxonomy and gains `calculators = [minacalc]`.
- A new versioned stage `difficulty` (v1) runs MinaCalc v527 (ADR 0022) for every profile with a calculator. Its key is `difficulty`, VERSION, `MinaCalcParams` hash, `minacalc@527`, keymode and the upstream `chart_parse` key. Its golden hashes the adapter output (note rows, hold share, rate grid), never calculator floats.
- cache.db gains `chart_msd(md5, vkey, rate_milli, 8 centi columns, hold_share_permille, status)`, keyed by vkey (D15) and rebuilt from the vault. `CACHE_SCHEMA_VERSION` goes from 5 to 6.
- The 4K pattern vocabulary, its axes and a per-keymode patterns stage (`patterns_k4`, so 4K rule changes never re-key 7K) are a later ADR, written with a 4K player.

## Alternatives considered
- Wait for the 4K vocabulary before enabling K4: testers would get nothing in 4K for weeks, although MinaCalc needs no pattern rules.
- Feed 4K into the existing `patterns` stage: bumps its VERSION and recomputes all 7K segments, now and on every 4K rule change.
- Compute MSD on demand without a stage: 2,746 4K plus 18.5k 7K charts × 17 rates is too slow per request, and the result would carry no VersionKey.

## Consequences
- Architecture §9.1 steps 2, 3 and 9 are amended: profiles are constants, 4K uses `k4.generic` with no thumb variants, and 4K patterns get their own stage. §12 pulls the k4 profile and MinaCalc forward from F5.
- Switching to this branch rebuilds cache.db once (schema 6).
- `segment` rows and the 7K `patterns` key stay byte-identical; the stage lock only gains the `difficulty` entry.
