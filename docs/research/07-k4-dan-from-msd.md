# 07 4K dan from Overall MSD

Status: fitted on 2026-10-08, read-only, from a scratch data dir built from the pilot's install. 2,777 indexed 4K charts, 2,352 of them rated. Feeds the `DanTable4k` default (`crates/engine/src/preview/dan.rs`, ADR 0024).

The estimate maps one number, MinaCalc v527 Overall at 1.0x (`msdOverallCenti`), to a dan from 1st to Epsilon plus a Low/Mid/High third. It is a beta label, not a calibrated skill tier.

## Data and labels

The only labels are the dan names that pack authors wrote into their own titles and difficulty names. The fit uses no Daniel or Sunny code and no third-party label set. The LeoBlack dan benchmark (`01-landscape-verified.txt:47`, `:145`) is not used because its samples carry no reuse grant (`02-7k-bms-stable-verified.txt:155`).

`research/scripts/dan4k/fit.py` reads the `library list` JSON and applies these rules in order (it prints them, plus every match and skip):

1. A title that names another keymode (5K to 10K) is skipped.
2. The title must look like a pack: `dan`, `practice`, `pack`, `collection` or `journey`.
3. Thumb-play packs are left out of the fit, since thumb play is a different input scale. They are kept as an external check.
4. `Pre-<tier>` packs are skipped because they sit between two dans.
5. Placeholder difficulties (`delete`, `do not play`) are skipped.
6. LN-heavy charts (`lnRatio > 0.20`, or LN in the title) are skipped. MinaCalc ignores holds.
7. Unrated charts (no `msdOverallCenti`) are skipped.
8. The dan comes from the difficulty name when it has `~ X ~`, `[X …]` or `X - `. Otherwise it comes from a tier that opens the pack title (`10th Dan …`, `Alpha Jack …`). A tier word in any other position is not read: in "Beta Jack trill" it is part of the song name, and in "Dan ~ REFORM ~ 2nd Pack" it names the pack.
9. A `(Marathon)` course chart with a rate in its name is a rate variant of the course and is skipped.
10. 1st to 10th map to 1 to 10, then Alpha 11, Beta 12, Gamma 13, Delta 14, Epsilon 15. `EXTRA-X` counts as X. Sub-tiers (Low, Mid, High, Peak) are ignored.
11. The pack id is the title, so re-uploads of the same pack by different creators fold into one leave-one-pack-out fold.

### Matched: 263 charts in 13 packs

| pack | kind | charts | dans |
|---|---|---|---|
| Dan ~ REFORM ~ JackMap Pack | course | 15 | 1st–Epsilon, one each |
| Dan ~ REFORM ~ SpeedMap Pack | course | 15 | 1st–Epsilon, one each |
| Dan ~ REFORM ~ StaminaMap Pack | course | 15 | 1st–Epsilon, one each |
| Dan ~ REFORM ~ TechMap Pack | course | 15 | 1st–Epsilon, one each |
| Dan ~ REFORM ~ 2nd Pack (2 uploads) | course | 19 | 6th–10th ×2, Alpha ×1, Beta–Epsilon ×2 |
| 10th Dan Practice Pack ~ Jack ~ | practice | 23 | 10th |
| 10th Dan Tech Practice Pack | practice | 22 | 10th |
| Alpha Jack Practice Pack | practice | 26 | Alpha |
| Alpha Tech Practice Pack | practice | 30 | Alpha |
| Alpha speedjack pack 2 | practice | 23 | Alpha |
| The Journey from Beta to Gamma (Tech) | practice | 10 | Beta 5, Gamma 5 |
| Gamma++ Stream/Stamina Collection | practice | 25 | Gamma 20, Delta 5 |
| Gamma++ Tech Collection | practice | 25 | Gamma 19, Delta 6 |

Skipped candidates:
- thumb scale: 4K Thumb Dan - B Pack (15 charts).
- pre-tier: 4K Pre-Alpha Practice Pack (11).
- course rate variant: one REFORM JackMap chart (Delta at 0.95x).
- no dan token: two REFORM JackMap extras ("Beta Jack trill" and its 0.9x copy), plus SDVX and trash packs.
- placeholders: one per practice pack.
- LN-heavy: 3 charts.
- not a pack: song titles such as "Last Remote - Type gamma".

No Signicial pack is in the pilot's library.

## Method

1. **Per-dan mean, pack-balanced.** For each dan, take the mean Overall of each pack's charts at that dan, then average those pack means. Otherwise a 30-chart practice pack would outweigh the one chart per dan of a course pack. The chart-level mean is printed alongside for comparison.
2. **Monotone.** Weighted pool-adjacent-violators (the weights are the pack counts) on the dan-ordered means. On this data the means already rise, so PAV changes nothing. 2nd and 3rd are nearly tied (16.15 vs 16.17).
3. **Bounds.** A dan's lower bound is the midpoint between its mean and the previous dan's mean. The 1st-dan bound is extrapolated by half the 1st→2nd gap. A tie is bumped by 1 centi so that every dan stays reachable.
4. **Thirds.** Low, Mid and High split `[lower, next lower)` in equal thirds. The top dan uses `top_span_centi`, the width of the dan below it.

| dan | charts | packs | chart mean | pack-balanced mean | min | max |
|---|---|---|---|---|---|---|
| 1st | 4 | 4 | 14.18 | 14.18 | 12.75 | 16.09 |
| 2nd | 4 | 4 | 16.15 | 16.15 | 14.66 | 18.15 |
| 3rd | 4 | 4 | 16.17 | 16.17 | 15.41 | 17.23 |
| 4th | 4 | 4 | 18.78 | 18.78 | 17.10 | 20.76 |
| 5th | 4 | 4 | 20.51 | 20.51 | 19.22 | 22.09 |
| 6th | 6 | 5 | 22.50 | 22.35 | 20.11 | 24.09 |
| 7th | 6 | 5 | 23.34 | 23.22 | 21.20 | 24.61 |
| 8th | 6 | 5 | 24.41 | 24.34 | 21.83 | 26.15 |
| 9th | 6 | 5 | 25.55 | 25.43 | 24.12 | 26.25 |
| 10th | 51 | 7 | 26.06 | 26.22 | 23.32 | 28.65 |
| Alpha | 84 | 8 | 26.80 | 27.40 | 23.71 | 29.65 |
| Beta | 11 | 6 | 28.21 | 27.99 | 27.14 | 29.61 |
| Gamma | 50 | 8 | 30.76 | 30.39 | 27.56 | 33.03 |
| Delta | 17 | 7 | 32.17 | 32.35 | 30.60 | 33.46 |
| Epsilon | 6 | 5 | 34.88 | 34.88 | 33.23 | 36.92 |

## The table (`DanTable4k::default`)

| dan | lower bound | Mid from | High from |
|---|---|---|---|
| 1st | 13.20 | 13.86 | 14.52 |
| 2nd | 15.17 | 15.50 | 15.83 |
| 3rd | 16.16 | 16.60 | 17.04 |
| 4th | 17.48 | 18.21 | 18.93 |
| 5th | 19.65 | 20.25 | 20.84 |
| 6th | 21.43 | 21.88 | 22.33 |
| 7th | 22.78 | 23.12 | 23.45 |
| 8th | 23.78 | 24.15 | 24.52 |
| 9th | 24.88 | 25.20 | 25.51 |
| 10th | 25.82 | 26.15 | 26.48 |
| Alpha | 26.81 | 27.11 | 27.41 |
| Beta | 27.70 | 28.20 | 28.70 |
| Gamma | 29.19 | 29.92 | 30.65 |
| Delta | 31.37 | 32.12 | 32.87 |
| Epsilon | 33.61 | 34.36 | 35.11 |

`top_span_centi = 224`. Below 13.20 there is no estimate.

## Validation

Leave-one-pack-out: each pack is refitted out of the table and its charts are predicted. Errors are in dan units, and a chart below the 1st bound counts as 0. Bias is the mean signed error (predicted − labelled).

| fold | n | exact | within 1 | MAE | bias |
|---|---|---|---|---|---|
| **all held-out charts** | 263 | **36.9%** | **78.3%** | **0.886** | −0.16 |
| reference: constant median dan of the fold | 263 | 31.9% | 55.5% | 1.829 | +0.41 |
| in-sample (fit on all) | 263 | 39.5% | 80.6% | 0.833 | −0.16 |
| REFORM SpeedMap | 15 | 66.7% | 100% | 0.333 | +0.07 |
| REFORM TechMap | 15 | 53.3% | 93.3% | 0.533 | −0.13 |
| REFORM 2nd Pack | 19 | 57.9% | 94.7% | 0.474 | +0.37 |
| REFORM JackMap | 15 | 20.0% | 66.7% | 1.200 | −1.07 |
| REFORM StaminaMap | 15 | 20.0% | 60.0% | 1.200 | +1.20 |
| 10th Dan Practice ~ Jack ~ | 23 | 8.7% | 39.1% | 1.609 | −1.52 |
| 10th Dan Tech Practice | 22 | 36.4% | 54.5% | 1.091 | +1.00 |
| Alpha Jack Practice | 26 | 3.8% | 69.2% | 1.346 | −1.27 |
| Alpha Tech Practice | 30 | 36.7% | 93.3% | 0.700 | +0.57 |
| Alpha speedjack pack 2 | 23 | 8.7% | 56.5% | 1.565 | −1.30 |
| Journey from Beta to Gamma (Tech) | 10 | 40.0% | 100% | 0.600 | −0.20 |
| Gamma++ Stream/Stamina | 25 | 64.0% | 100% | 0.360 | +0.36 |
| Gamma++ Tech | 25 | 72.0% | 100% | 0.280 | +0.12 |

External check, not fitted: the thumb-play course (10 charts) scores 10% exact, 90% within one dan, MAE 1.0 and bias +1.0. Eight charts read one dan high, one (9th) reads two dans high and one (10th) is exact, so thumb courses ask for more MSD per dan.

How to read these numbers: the estimate is reliable to about one dan (within 1 on 78% of held-out charts) and rarely exact. The errors follow the skillset. Jack packs read about one dan low (bias −1.07 to −1.52), because the same dan gives jack charts less Overall. REFORM Stamina and the 10th/Alpha tech practice packs read high (+0.57 to +1.20). One Overall number cannot remove that bias. The gap is largest at 10th and Alpha, where whole practice packs sit on one skillset.

## Caveats

- **Pack authors' scales differ.** REFORM is one author's ladder. The practice packs are separate authors' opinions of a tier and can disagree with REFORM by a full dan for one skillset. In REFORM, Alpha and Beta are nearly level in MSD (Alpha courses 27.08–29.17, Beta 27.47–28.15), so the Alpha–Beta boundary is the least determined.
- **MinaCalc v527 only.** The bounds are in v527 Overall at 1.0x. A calc bump (ADR 0022) needs a refit.
- **Rice only.** LN-heavy charts are excluded and the estimate means nothing on LN play.
- **Small n at the bottom and the top.** 1st–5th rest on the 4 REFORM skill courses, one chart each. Epsilon rests on 6 charts. Nothing above Epsilon is in the library, so Zeta and higher are not estimated: they read as Epsilon High.
- **One library.** Only packs the pilot happens to have. No Signicial courses were found.
- **Daniel's table is not used.** ADR 0024 gives the reason as a Sunny-derived rating. `01-landscape-verified.txt` does not establish that dependency:
  - Daniel is a 4K rice dan estimator, Alpha to Theta, MIT (`:88`, `:123`, `:194`).
  - Its first listing as a "Sunny-style SR" source (`:211`, `:229`) is corrected at `:243`: "4K rice only … It bundles an MSD binary".
  - The Sunny reimplementation constraint is at `:7`, `:248`.
  - Daniel's benchmark MAE is at `:92`.

  Whatever Daniel's rating is built on, this table stays independent of its code and its labels, so licence and provenance questions do not reach it.

## How to refit

1. Close osu! (`tasklist.exe | grep -i "osu!.exe"` must print nothing).
2. Build a scratch data dir. Never use the real one:
   `wolluf --data-dir <scratch>/dd setup set "/mnt/e/Games/osu!"`, then `sync` (it chains `library index` and MSD), then `library index`.
3. `wolluf --data-dir <scratch>/dd --json library list --keys 4 --limit 100000 > k4.json`. The CLI takes any limit. If a page comes back full, add `--offset` pages and pass every file.
4. `python3 -I research/scripts/dan4k/fit.py k4.json --report report.json`. Check the matched and skipped lists, then copy the printed Rust rows into `DanTable4k::default` and update this note.
5. Self-check of the script: `python3 -I research/scripts/dan4k/test_fit.py`.
