# 05 Pattern engine: name-hint lift on the pilot library

Status: measured on 2026-10-05, read-only, on a copy of the pilot data dir. 18,333 parsed 7K charts. Command: `wolluf library hints` after `wolluf library index`.

**Lift** is the mean share of segmented time on charts whose name hints a target, divided by the library-wide mean share. A lift above 1 means the engine finds more of that pattern where mappers say it is.

**Caveats:**
- Hints are weak labels. Pack, folder and difficulty words are not gold labels.
- Several hint sets are tiny. Some are one chart at several rates: the trill and jumptrill sets are 4 distinct charts plus rate copies. Read lifts on fewer than about 20 charts as anecdotes.
- Lift is not accuracy. The gold set (`wolluf label`) measures accuracy (F1 deliverable 4).

## Engine v1 → v4 (patterns stage VERSION)

| target | hinted charts | v1 lift | v4 lift |
|---|---|---|---|
| `7k.regular.jack` | 107 | 2.54 | 2.42 |
| `7k.regular.speed` | 58 | 0.57 | 2.49 |
| `7k.regular.stream` | 99 | 1.52 | 1.48 |
| `7k.regular.tech` | 53 | none (0% segmented) | 1.61 |
| `regular.speed.delay` | 47 | (no id) | 6.53 |
| `regular.speed.burst` | 23 | 0.66 | 1.10 |
| `regular.stream.bracket` | 192 | 4.69 | 4.39 |
| `regular.jack.chordjack` | 41 | 4.37 | 4.40 |
| `ln.inverse.gap` | 37 | 14.36 | 14.34 |

## What changed, with the numbers that drove each change

**v2: `regular.speed.delay` added** (ADR 0017 amendment).
- Charts named "delay" (BMS ディレイ: the Delay Master Pack, `[DELAYMASTER]` BMS difficulties, Jinjin speed practice) give it a lift of 6.0–6.9 across the later versions.
- The speed axis rose from 0.57 to 1.30 with delay alone. This is consistent with the Jinjin 7K dans filing delay under speed.

**v3: `regular.tech.irregular` owns rows.**
- It now sits in the priority table instead of `tag_only`, placed below delay and above the generic streams.
- The tech axis went from 0% segmented time to a lift of 1.82. No Regular axis lift fell. `7k.ln.tech` (15 charts) went from 1.83 to 1.60.
- `hand_imbalance` and `thumb` stay tags, because they modify whatever pattern they sit on.

**v4: burst needs 2× the local pace and 4+ rows**, up from 1.5× and 3+.
- At v3, burst held 512k segments with a mean of 0.33 s, and its lift was below 1. It was firing on ornaments (two fast rows) and on 1.5× rhythm changes (1/6 inside 1/4).
- Sweep:

  | burst setting | speed | tech | burst lift | burst seconds |
  |---|---|---|---|---|
  | 1.5×, 3 rows | 1.31 | 1.82 | 0.70 | 167,971 |
  | 2.0×, 3 rows | 2.14 | 1.75 | 0.79 | 47,567 |
  | 2.0×, 3 rows, burst ranked below irregular | 2.10 | 1.53 | 0.79 | 39,447 |
  | **2.0×, 4 rows (kept)** | **2.49** | **1.61** | **1.10** | **28,747** |

  Ranking burst lower also cost bracket (4.80 → 4.41) and delay (6.94 → 6.21), so it was rejected.
- The jack axis dipped from 2.67 to 2.42 across v4. Rows that burst used to own now partly fall to jack patterns in the baseline as well, which raises the denominator.

## Trill and jumptrill stay at ~0%, and that is not a detector bug
- The 18 hinted charts are 4 distinct charts plus rate copies.
- Rendering them (`wolluf chart show --segments`) shows three things:
  - single-hand bracket shapes, such as 5 against [4, 6];
  - a [123]/[4567] whole-hand alternation, which ADR 0017 files as `split_trill` while the mapper calls it "Jumptrill";
  - a 27 ms joke roll.
- mania-hub's `trillRunShare` also requires exact two-row alternation.
- Relaxing `alternations` would need gold labels, not these hints.
