# 0017 7K pattern vocabulary

- Status: Accepted
- Date: 2026-09-29

## Context
Gold labels (`segment_label` events in user.db) and the future pattern engine share one set of pattern ids. The ids are persisted, and gold labels are irreplaceable, so the vocabulary must be fixed before the first real label. The sources disagree:
- the osu! wiki counts a jack as 3+ notes, a minijack as 2 and a longjack as 4+;
- Interlude's 7K "Brackets" rule is really a dense-chord rule;
- MinaCalc assigns the 7K middle column to the left hand.

See research 01 l.81 and 02 l.7, l.40–44. The pilot (a 7K player, 3|1+3 right-thumb layout) decided the open points on 2026-09-29.

## Decision
- 25 patterns under the 8 axes (26 since the 2026-10-05 amendment), frozen by the snapshot in `crates/engine/src/taxonomy/snapshots/`.
- Jacks are defined by count: `regular.jack.minijack` is exactly 2 consecutive notes in one column, and `regular.jack.longjack` is 3+. There is no speed-based jack pattern, because speed is a difficulty dimension.
- `regular.stream.bracket` follows the wiki/MinaCalc meaning: two or more trills at the same time within one hand.
- `regular.stream.chordbracket` is a 2–3-note chord shape moving across columns without jacking (Interlude's "Brackets"). Alternating chords of more than 4 notes are `regular.stream.jumptrill`.
- The thumb is not a separate hand. The layout assigns it to one. `regular.tech.thumb` names thumb-column-heavy patterns. A gold label may carry an optional `thumb_pref` (left/right) so thumb-side preference can be checked against a hand-load heuristic now and learned from users later (F5).
- LN leaves are `ln.general.{density,chord}`, `ln.tech.{hybrid,shield}`, `ln.inverse.gap` and `ln.release.timing`.
- "No clear pattern" is a stored answer (`assert_none`), distinct from an unstored skip, so the engine's false positives are measurable.

## Alternatives considered
- Minijack as 3 notes under 150 ms (the earlier architecture example). Rejected: it conflicts with the wiki and with how 7K players use the word.
- A separate "thumb hand". Rejected: it misdescribes how the thumb is played, and it breaks hand-balance features. A per-label preference flag captures the real question.

## Consequences
- Renaming or re-meaning an id after this needs a new id plus a reader migration for stored labels.
- The pattern engine (F1 deliverable 2) implements rules against these definitions. Its thresholds, such as how long a trill must run, are params (D17), not part of this vocabulary.

## Amendment 2026-10-05: `regular.speed.delay`
- New id `regular.speed.delay` (key `d`) on `7k.regular.speed`, appended to the frozen list.
- Meaning: delay (BMS ディレイ), notes and small chords staggered off the 1/4 grid (1/6, 1/8, 1/12, 1/16… gaps, or gaps short enough to be a split chord). The Jinjin 7K dans file this shape under their "speed" slot, so in 7K delay is the speed axis's sustained leaf, and `regular.speed.burst` its short one. The pilot approved it on 2026-10-05.
- It is distinct from `regular.tech.irregular`: irregular is a rhythm-reading problem (mixed straight and triplet snaps, or off-snap gaps), while delay is a fine-subdivision flow. Their rows can overlap, and segment priority decides which one is primary.
- Thresholds are params (D17). The detector is seeded from mania-hub `algorithms/dan-estimator/features.ts` `offGridRowShare` (MIT, credited in NOTICE).
