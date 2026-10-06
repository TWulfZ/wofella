# 06 osu!mania skins and scroll speed

Status: verified on 2026-10-06 from the sources below and a read-only survey of the pilot's `Skins/` folder. Decision: `docs/adr/0019-skin-assets-over-ipc.md`.

**What "stable-faithful" means here.** osu! stable is closed source. lazer's comments link `peppy/osu-stable-reference`, which is private (404). Every rule below comes from the osu! wiki, from lazer's legacy-skin code (which imitates stable), or from measurements quoted in ppy/osu PRs. None comes from stable code. Items marked **unverified** are lazer's or the wiki's claim with no stable evidence; the pilot's side-by-side check against a stable screenshot settles them.

## Sources

| Tag | Source | Pin |
|---|---|---|
| **L** | ppy/osu (lazer), MIT, `LICENCE` "Copyright (c) 2025 ppy Pty Ltd" | `6359741babba4ced42da1fcda738c35370fdf3f4`. Base URL `https://github.com/ppy/osu/blob/6359741babba4ced42da1fcda738c35370fdf3f4/` |
| **F** | ppy/osu-framework, MIT | `ee17a3198189ca000a21da6219801cc40ef2a380` |
| **W** | ppy/osu-wiki: `wiki/Skinning/skin.ini/en.md`, `wiki/Game_mode/osu!mania/en.md`, `wiki/Client/Options/en.md`, `wiki/Skinning/FAQ/en.md` | `bfe68330c3e733a8e90c43298965444f522db3d2` |
| **P** | ppy/osu PR #13901 "Adjust mania speed range" (stable `SpeedMania` values), PR #26131 "Make mania scroll speed independent of hit position" (merged 2023-12-26), PR #8887 "Fix speed adjustment mods affecting mania scroll speed" (merged 2020-04-29), issue #9868 (comment 675327188), issue #26464 | `https://github.com/ppy/osu/pull/<n>` |
| **T** | Forum topic 367649 (2015): "13720ms between the time when the first pixel of a note appears, and the time when th[e note is judged]" | `https://osu.ppy.sh/community/forums/topics/367649` |
| **R** | ppy/osu-resources `LICENCE.md`: Creative Commons Attribution-NonCommercial 4.0 International | `master`, read 2026-10-06 |

Line numbers below are for the pinned files.

## Licensing
- lazer's logic can be ported with a NOTICE entry (ADR 0008). The README excludes the "osu!" and "ppy" branding from the licence (README L145), so no logos.
- lazer's fallback skin `DefaultLegacySkin` loads its images from osu-resources (`osu.Game/Skinning/DefaultLegacySkin.cs` L42, `"Skins/Legacy"`). Those are CC BY-NC 4.0 (**R**) and cannot ship in an MIT app. A missing element is drawn procedurally instead.
- User skins are third-party art. They are read in place at runtime and never copied into the repo or fixtures.

## `[Mania]` keys (W `Skinning/skin.ini` L435–800)
Lengths are in stable's 480-high virtual space ("based on a height of 480 pixels", L440). lazer converts them to its 768-high space with `STABLE_MAGIC_SCALE_FACTOR = 1.6f` (L `osu.Game/Skinning/LegacySkin.cs` L36). The wiki says `skin.ini` commands are case-sensitive (L10); lazer matches keys with ordinal comparisons.

| Key | Wiki default | lazer (`osu.Game/Skinning/LegacyManiaSkinConfiguration.cs`) |
|---|---|---|
| `Keys` | required per block, values 1–10, 12, 14, 16, 18 (L462–480) | `Keys` ctor, L62–75 |
| `ColumnStart` / `ColumnRight` | 136 / 19 (L481–488) | stored, unused: L49–50 ("Unimplemented properties", L46) |
| `ColumnSpacing` | 0, `keys−1` values (L489–495) | L67 |
| `ColumnWidth` | 30 (L496–501) | L17, L74 |
| `ColumnLineWidth` | 2, `keys+1` values (L502–505) | L66, L73. Not scaled by 1.6 (`LegacyManiaSkinDecoder.cs` L78) |
| `BarlineHeight` | 1.2 | L39 |
| `WidthForNoteHeightScale` | the narrowest column (L518–522) | L27, L77 |
| `HitPosition` | 402 (L523–528) | L19, L35. Clamped to 240–480 on read (`LegacyManiaSkinDecoder.cs` L94) |
| `LightPosition` | 413 (L529–534) | L36 |
| `JudgementLine` | `0` or `1`, no default given (L545–549) | on (L40) |
| `KeysUnderNotes` / `UpsideDown` | 0 (L594, L600) | L41, L51 (`UpsideDown` unimplemented) |
| `NoteBodyStyle`, `NoteBodyStyle#` | values 0, 1, 2; default 1 (L657–669) | **disagrees**: enum `Stretch = 0, RepeatTop = 2, RepeatBottom = 3, RepeatTopAndBottom = 4`, with `Repeat = 1` commented out as "listed as the default on [the wiki], but is seemingly not according to the source" (L88–98). Default by version, below |
| `Colour#` | `0,0,0,255`; `#` starts at 1 (L670–676) | `$"Colour{ColumnIndex + 1}"` (`LegacySkin.cs` L184) |
| `KeyImage#[D]`, `NoteImage#[H/L/T]` | image paths (L716–745) | `$"NoteImage{ColumnIndex}"`, 0-based (`LegacySkin.cs` L214–237) |

## Parser rules (lazer)
- **Encoding.** `StreamReader(stream, Encoding.UTF8, true, …)`: UTF-8 by default, a BOM is honoured (`osu.Game/IO/LineBufferedReader.cs` L22). No evidence was found of another encoding in either client; stable's encoding is **unverified**.
- **Lines.** Right-trimmed (`osu.Game/Beatmaps/Formats/LegacyDecoder.cs` L58); blank and `//` lines are skipped (L81); a trailing `//` comment is cut (L103); `[Section]` lines switch section (L60); `Key: Value` splits on the first `:` and trims both sides (L155–157). Colours are `R,G,B[,A]` with alpha defaulting to 255 (L119–120).
- **`[General] Version`.** `latest` means 2.7 (`LegacySkinDecoder.cs` L36–39, `SkinConfiguration.cs` L17); a missing Version means 1.0 (`LegacySkinDecoder.cs` L66–72).
- **`[Mania]` blocks** (`LegacyManiaSkinDecoder.cs`):
  - lines before `Keys:` are held and applied to the block it opens (L54–59, L67–71);
  - a new section drops held lines (L25–31);
  - a duplicate `Keys` value creates a block that is never added to the output, so the **first block wins** (L43–51);
  - a list entry that does not parse reads as 0, "to match stable behaviour" (L197–216, issue #26464).
- **Missing key-count block.** lazer creates an all-defaults block (ColumnWidth 30, HitPosition 402) and still uses the skin's default-named images (`LegacySkin.cs` L143–144). What stable does is **unverified**.
- **LN body style by version.** An explicit `NoteBodyStyle` wins. Otherwise Version < 2.5 gives `Stretch` and ≥ 2.5 gives `RepeatBottom` (`LegacySkin.cs` L202–210).
- **Skin gate.** lazer uses the legacy mania pieces only when the skin has a Version and the column-0 key image resolves (`osu.Game.Rulesets.Mania/Skinning/Legacy/ManiaLegacySkinTransformer.cs` L76–81, L151).

## Image resolution (lazer)
- **Default names.** In an odd key count, the centre column is special (`osu.Game.Rulesets.Mania/Beatmaps/StageDefinition.cs` L32) and uses `S`. Every other column uses `distanceToEdge % 2 == 0 ? "1" : "2"` (`…/Skinning/Legacy/LegacyManiaColumnElement.cs` L33–41). 7K is therefore `1 2 1 S 1 2 1`: `mania-note{t}`, `mania-note{t}H`, `mania-note{t}L`, `mania-note{t}T`, `mania-key{t}`, `mania-key{t}D`.
- **Chains.** Note: `NoteImage#` ?? `mania-note{t}` (`LegacyNotePiece.cs` L99–100). Head: H, then Note (`LegacyHoldNoteHeadPiece.cs` L49–50). Tail: T, then H, then Note (`LegacyHoldNoteTailPiece.cs` L58–60), drawn with its direction inverted (L49). Body: L only.
- **Frames.** For an animatable element, `name-0` is tried before `name`, and frames are read until the first gap (`osu.Game/Skinning/LegacySkinExtensions.cs` L66–109). Notes, heads and bodies are animatable (`LegacyNotePiece.cs` L102, `LegacyBodyPiece.cs` L85). Stable's preference is **unverified**; a preview draws frame 0.
- **HD.** Any `@2x` in the requested name is stripped ("stable happens to check for that and strip them"), then `name@2x` is tried with scale 2 before `name` with scale 1 (`LegacySkin.cs` L563–581). Display size is pixel size ÷ scale. The wiki says stable uses HD images from 800 px window height (W `Skinning/FAQ` L192).
- **Extensions.** The name as given, then `name.png`, then `name.jpg` (F `osu.Framework/IO/Stores/ResourceStore.cs` L148–155, `osu.Framework/Graphics/Textures/TextureLoaderStore.cs` L27–28).
- **Case and separators.** `\` becomes `/` (F `osu.Framework/Extensions/ExtensionMethods.cs` L286–287), and files match through a lower-cased map (`osu.Game/Skinning/RealmBackedResourceStore.cs` L48–59). The wiki confirms extra folders are allowed in a skin (W `Skinning/FAQ` L29).

## Geometry (lazer; `k = canvasHeight / 480`)
- Column widths, spacing and WidthForNoteHeightScale: ×k. Column lines: lazer leaves them unscaled and applies an x-scale of 0.740 (`…/Legacy/LegacyStageBackground.cs` L121, L134); the right line shows only from Version 2.4 or on the last column (L101).
- Judgement line: `clamp(HitPosition, 240, 480)·k` from the top.
- Note: column width wide, `texH·WFNHS/texW` tall, in display units (`LegacyNotePiece.cs` L63–64).
- Hit target: y-scale `0.9 × 1.6025`; the judgement line is 1 unit at alpha 0.9 (`LegacyHitTarget.cs` L45, L53–55).
- Colours: alpha 0 becomes 1 (`osu.Game/Skinning/LegacyColourCompatibility.cs` L22–26); column backgrounds apply alpha twice, so the effective alpha is A² (L38–42).
- **Approximations, unverified against stable:**
  - key sprites are bottom-anchored with no 1.6 factor on their height (`LegacyKeyArea.cs` L50–64);
  - repeat body styles: lazer anchors the body sprite at the tail end (`Origin`/`Anchor = TopCentre` in downscroll, `LegacyBodyPiece.cs`), so the image's row 0 faces the tail. Percy skins rely on this: their very tall bodies (138×40000 on the pilot's install) start with transparent rows and a rounded cap, which make the tail look cut. wolluf anchors every repeat style at the tail too, tiles toward the head at the column's width-fit (natural) aspect, and crops bodies taller than its cap to their top rows. A cropped or strip-shaped body (height ≥ 8 × width) is drawn once from the tail, and its last few rows are stretched over the rest of a longer hold; it is never tiled again, so the cap cannot reappear mid-hold. Still unverified against stable: the vertical scale. wolluf uses the natural aspect (31.7 stable px of lead-in for the pilot's percy body), while lazer stretches the sprite by `max(1, 32800 / DrawHeight)` with the comment "i dunno this looks about right??" (`LegacyBodyPiece.cs` L198–209), about 48.7 px;
  - the all-defaults block for a skin without `Keys: 7`;
  - the `NoteBodyStyle` enum mismatch between wiki and lazer. lazer's decoder reads the value with `Enum.TryParse`, which accepts `1` as an undefined enum value; that value is not `Stretch`, so it draws with repeat wrapping at any skin version. wolluf therefore maps `1` to `RepeatBottom` regardless of version (`crates/source-osu/src/codec/skin_ini.rs`). Whether stable's wiki "Repeat" looks the same is unverified.

## Scroll speed

### osu! stable
- **Velocity.** `v = 21n/600 = 0.035·n` virtual px/ms for speed `n`, independent of HitPosition (P: issue #9868 comment 675327188, "`402 * (60000 / 100) / 21 / 40`" in PR #13901).
- **Time on screen.** Notes scroll only down to the judgement line, so `T = HitPosition / (0.035·n) = HitPosition·200/(7n)` ms. PR #13901 quotes stable's own `SpeedMania.TimeAt(…, 402)`: 287.14 ms at n = 40 and 11485.71 ms at n = 1. The community form `13720·(HitPosition/480)/n` (T) is the same formula with 480/0.035 = 13714.3 rounded to 13720; it gives 407.8 ms instead of 407.6 ms at n = 30, HitPosition 428.
- **Canvas.** With `k = H/480`, `pxPerMs = 0.035·n·H/480` and the judgement line at `HitPosition·k`, so T does not depend on canvas height.
- **Range.** 1 to 40 (W `Game_mode/osu!mania` L43).
- **BPM option.** "Scale osu!mania scroll speed with BPM", default `Disabled`, cfg key `ManiaSpeedBPMScale` (W `Client/Options` L188). The `Game_mode/osu!mania` page still calls BPM scaling "the current default" (L49), which contradicts the Options table. When enabled, T gives the effective speed as `n·BPM/100`; which BPM (current timing point or main BPM) is **unverified**.
- **Per-beatmap speed.** "Remember osu!mania scroll speed per-beatmap", default `Disabled`, cfg key `UsePerBeatmapManiaSpeed` (W `Client/Options` L189). Where stable stores per-map values is **unverified**.
- **Rate mods.** lazer multiplies its time range by tempo × frequency so that real-time speed stays constant under DT/HT (P #8887; `DrawableManiaRuleset.cs` L175). In map time the velocity is therefore divided by the rate. That stable behaves the same way is an inference.

### F3/F4 direction: unverified
The wiki says `F3` is faster and `F4` slower (W `Game_mode/osu!mania` L67; L41 gives `Ctrl`/`Shift` with `+` faster and `-` slower). The Label screen binds F3 to −1 and F4 to +1. Which one stable does is **unverified pending the pilot**, who uses the keys daily. Once confirmed, record the answer here and align the binding.

### lazer
- `ComputeScrollTime(s) = MAX_TIME_RANGE / s` with `MAX_TIME_RANGE = 11485` and `MIN_TIME_RANGE = 290` (`osu.Game.Rulesets.Mania/UI/DrawableManiaRuleset.cs` L38, L43, L183). 11485/40 = 287.1, so the 290 floor is slightly off the formula.
- Since PR #26131, `updateTimeRange` scales by `(768 − hitPosition)/(768 − DEFAULT_HIT_POSITION)` with `DEFAULT_HIT_POSITION = (480 − 402)·1.6` (L167–175; `LegacyManiaSkinConfiguration.cs` L19). That reproduces stable's `T = 11485/n · HitPosition/402`. Non-legacy skins use `Stage.HIT_TARGET_POSITION = 110` (`osu.Game.Rulesets.Mania/UI/Stage.cs` L38).
- Setting: default 8, range 1–40, step 0.1 (`osu.Game.Rulesets.Mania/Configuration/ManiaRulesetConfigManager.cs` L24). A lazer mode in wolluf would be the same formula with a 0.1 step; it is reserved, not built.
- Scroll algorithms: Sequential (SV-aware) is the default, Constant comes with the ConstantSpeed mod (`osu.Game/Rulesets/UI/Scrolling/DrawableScrollingRuleset.cs` L172–199; `ManiaModConstantSpeed.cs` L30). The Label screen keeps Constant.

### Conversion table
`T402` and `T428` are the times on screen at HitPosition 402 (default) and 428; `T480` is the full 480 height.

| n | T402 ms | T428 ms | T480 ms | virtual px/ms | px/ms at H = 720 | px/ms at H = 1080 |
|---|---|---|---|---|---|---|
| 1 | 11485.7 | 12228.6 | 13714.3 | 0.035 | 0.0525 | 0.0788 |
| 5 | 2297.1 | 2445.7 | 2742.9 | 0.175 | 0.2625 | 0.3938 |
| 10 | 1148.6 | 1222.9 | 1371.4 | 0.350 | 0.5250 | 0.7875 |
| 15 | 765.7 | 815.2 | 914.3 | 0.525 | 0.7875 | 1.1812 |
| 20 | 574.3 | 611.4 | 685.7 | 0.700 | 1.0500 | 1.5750 |
| 25 | 459.4 | 489.1 | 548.6 | 0.875 | 1.3125 | 1.9688 |
| 28 | 410.2 | 436.7 | 489.8 | 0.980 | 1.4700 | 2.2050 |
| 30 | 382.9 | 407.6 | 457.1 | 1.050 | 1.5750 | 2.3625 |
| 35 | 328.2 | 349.4 | 391.8 | 1.225 | 1.8375 | 2.7563 |
| 40 | 287.1 | 305.7 | 342.9 | 1.400 | 2.1000 | 3.1500 |

## Pilot survey (anonymised)
Read-only scan of `<install>/Skins` on 2026-10-06 with a Python scanner (`python3 -I`), plus the `corpus_skin_ini` run of the Rust codec with osu! closed. Counts only; no skin or person is named. The scanner resolved the 49 slots a 7K playfield needs (`NoteImage0..6` with H/L/T, `KeyImage0..6` with D, five stage images, two lighting images).

**Folders and files**
- 27 skin folders. 26 contain a skin.ini (16 `skin.ini`, 10 `Skin.ini`); one is empty.
- 25 have exactly one `[Mania] Keys: 7` block. One defines only other key counts and also carries a nested `skin.ini` in a subfolder, which is not read. The Rust codec agrees: 26 parsed, 25 with `Keys: 7`.
- Duplicate `Keys` blocks appear in 2 skins (for 9K and 18K, not 7K).
- Keys commented out with a leading backtick appear in one skin.ini and in one nested file.
- Folders hold 8.7–77.7 MB each and 769.5 MB in total. They also contain `.psd`, `.zip`, `.rar`, `.exe`, `.lnk` and `.png~` files and backup copies of images, so images are resolved from references, never by scanning the folder.

**Encoding and lines**
- 17 ASCII, 9 UTF-8 with non-ASCII text, none with a BOM, none invalid UTF-8.
- 24 use CRLF, 2 use LF only. The largest skin.ini is 16.7 KB.
- `[General] Version`: 14 `latest`, 9 `2.5`, one each of `2.4`, `2.7` and `3`.
- `[General] Name` differs from the folder name and is not unique, so skins are keyed by folder name.

**7K values**
- ColumnWidth 35–70 (42 most common). HitPosition 400–465. ColumnStart 140–360.
- ColumnLineWidth always has 8 values. ColumnSpacing is set in 1 skin, with 6 values.
- WidthForNoteHeightScale is set in 21 skins, `NoteBodyStyle` in none, `UpsideDown` is always 0.
- The active skin (cfg `Skin`): HitPosition 428, ColumnWidth 42 × 7, WidthForNoteHeightScale 42. With cfg `ManiaSpeed = 30` and `ManiaSpeedBPMScale = 0`, that is 407.6 ms on screen.

**Images**
- `@2x` is sparse: of the 49 slots, 10 skins resolve none to an `@2x` file, 9 resolve one, and the maximum is 16.
- The 7K working set is 16–69 files. It is 79 KiB–1.1 MiB in 21 of 25 skins, about 5.4 MiB in 2, and about 22 MiB in 2 (each includes the TIFF below). The active skin needs under 1 MiB.
- Note images are mostly 256×150 or 270×150; the largest is 1024×1024.
- The largest asset dimensions are LN bodies of 138×40000; bodies 20000–40000 px tall occur in 25 references across 14 skins, including the active skin.
- **TIFF named `.png`:** 2 files in 2 skins (not the active one), byte-identical, uncompressed RGBA TIFF (`II*\0`), 138×40000, 22.7 MB.
- Frames (`name-0`) were found for 60 slot references in 24 skins, mostly StageBottom and lighting. One skin animates its notes and LN heads (14 slot references).

**Path quirks**
- 200 slot references in 12 skins use `\` as the separator. References are up to 3 components deep, and some point into another key count's folder (a 7K block using `4K\…`).
- 41 slot resolutions in 14 skins match a file only when case is ignored, on the stem or the extension (`.pnG`, `lightingN` against `LightingN`).
- No reference carries an extension.
- 19 slots in 10 skins resolve to nothing. 8 of them are `StageHint`, mostly a deliberately broken reference with `@2x` in the middle of the name. Others are a missing LN tail (7), stage side, bottom or lighting.
- Elements are hidden on purpose with `_blank`, `blank.png` or 1×1 images.
- One reference in the active skin points to a subfolder where the file does not exist, while the file sits in the skin root. lazer would then use the default element; stable's behaviour is **unverified**.
- Folder names start with `-` followed by spaces, contain `#`, non-ASCII letters, `!`, `'`, `;`, `+` and commas, and two differ only by a ` (1)` suffix. The cfg `Skin` value is the exact folder name; its `#` is not a comment.

## Open questions for the pilot's side-by-side check
- Key image height (lazer: no 1.6 factor).
- RepeatBottom tiling against lazer's stretch hack.
- Stable's fallback for a skin without `Keys: 7` and for an explicit path that does not exist.
- `-0` frames against the plain name.
- `NoteBodyStyle` values (wiki 0/1/2 against lazer's 0/2/3/4).
- F3/F4 direction.
