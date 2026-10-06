# 0019 Skin assets over IPC

- Status: Accepted
- Date: 2026-10-06

## Context
The Label screen (ADR 0018) draws charts procedurally, at a px/ms speed, in a narrow fixed-width stage. The pilot reads 7K every day through their own osu! skin at their own speed, and found the procedural view uncomfortable next to it. Labelling comfort decides how many gold labels get made, and F1 deliverable 4 needs 200–300 (architecture §12). The Label screen should look like the pilot's in-game skin: column widths, HitPosition, notes, LN bodies, keys and stage, at stable's scroll speed.

What can be reused (`docs/research/06-mania-skin-and-scroll.md`):
- osu! stable is closed source. lazer's comments link `peppy/osu-stable-reference`, which is private.
- lazer (ppy/osu, MIT) reimplements legacy mania skins: the `skin.ini` decoder, texture lookup and the piece geometry. It is pinned here at `6359741babba4ced42da1fcda738c35370fdf3f4`.
- lazer's default legacy skin images come from ppy/osu-resources, which is CC BY-NC 4.0.

The pilot's `Skins/` folder (research 06, pilot survey) shows what a reader must survive:
- 27 folders; 26 skin.ini, of which 25 have a `Keys: 7` block;
- `Skin.ini` and `skin.ini` both occur;
- references with `\` separators and subfolders, and file names that match only when case is ignored;
- 2 TIFF files named `.png`;
- LN bodies up to 138×40000 px, one of them in the active skin;
- sparse `@2x` coverage;
- junk files beside the images.

The active skin's folder name and the scroll speed are in the user cfg (`Skin`, `ManiaSpeed`).

Constraints that already hold:
- The osu! folder is read-only (D9, G6). The webview never holds a path (ADR 0018).
- The CSP is `default-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; connect-src ipc: http://ipc.localhost`, and the capability surface is the one ADR 0009 lists.
- The cfg reader is an allowlist, and the `Password` value never enters wolluf-owned memory (spec 002 R9).
- Caps and constants live in param structs (D17). A new crate dependency needs an ADR.

## Decision
- **Second read root: `<install>/Skins`.** `wolluf_source_osu::skins` reads skins in place and never writes. Containment is textual, with the same rules as `read_song_file` (ADR 0018):
  - **Skin name.** The webview sends a skin folder name and a key count, never a path. The name must be one non-blank `Component::Normal` with no `/`, `\` or NUL, and it must appear in the current listing of `Skins/`.
  - **`skin.ini`.** It is found case-insensitively in the skin root.
  - **Image references.** These are the only relative paths accepted, and they come from `skin.ini`, never from the webview. A reference is split on `/` and `\`. Every part must be a normal component: no `..`, `.`, root or drive prefix, and a depth cap. Each part is matched case-insensitively against the directory listing, so a reference resolves the same on Linux CI as on NTFS.
  - **Links.** Symlinks and junctions are followed on purpose, as in ADR 0018: a skin folder moved to another drive is a legitimate setup, and only someone who can already write into `Skins/` can plant one.
  - **Reads.** Every read opens the file, checks type and size on the handle, and reads at most one byte past the cap.
- **`skin.ini` is parsed in Rust.** `crates/source-osu/src/codec/skin_ini.rs` is a pure codec that ports lazer's decoder rules (ppy/osu @ `6359741babba4ced42da1fcda738c35370fdf3f4`: `LegacyDecoder`, `LegacySkinDecoder`, `LegacyManiaSkinDecoder`, the `LegacySkin` mania lookups):
  - UTF-8 with or without a BOM; CRLF;
  - lines before `Keys:` belong to that block, and the first duplicate block wins;
  - image indices are 0-based and colour indices 1-based;
  - an unparsable list entry reads as 0;
  - Version defaults to 1.0, and `latest` means 2.7.

  Its deliberate deviations from lazer are listed in its module doc. Lengths stay in stable's 480-high units; scaling is the renderer's job.
- **Commands.** Two typed commands return `Result<T, IpcError>` (ADR 0009):
  - `skin_list` returns the skin folders with the key counts each defines and the skin.ini mtime (a string, per ADR 0009's 64-bit rule), plus `current`, the cfg `Skin` value when that folder exists.
  - `skin_get(name, keys)` returns the resolved `[Mania]` block for that key count, with lazer's all-defaults block when the skin has none (research 06). It also returns every resolved image once (deduplicated by file) as `{ id, mime, base64, scale, w, h }`, per-slot references to those ids, and per-slot diagnostics.
- **Image resolution follows lazer** (research 06, "Image resolution"):
  - the slot's `skin.ini` value, or the default name (7K: `1 2 1 S 1 2 1`);
  - any `@2x` in the name is stripped;
  - for animatable slots, `name-0` before `name`; only frame 0 is sent;
  - `name@2x` (scale 2) before `name` (scale 1);
  - the name as given, then `.png`, then `.jpg`.
- **Images are checked, not decoded.** The magic bytes decide the type: a PNG signature with an IHDR chunk, or a JPEG SOI followed by a SOF marker. `w` and `h` come from that header. Anything else, such as a TIFF named `.png`, is a missing slot with a diagnostic.
- **Caps.** They live in a skins param struct (D17):
  - `skin.ini` bytes;
  - bytes per image;
  - pixel **area** per image, not pixels per side, so the active skin's 138×40000 body is accepted;
  - decoded pixels per skin: past the budget, further images become missing slots, so the webview's decoded memory stays bounded;
  - total payload bytes per `skin_get`;
  - reference depth.

  Defaults are set from the pilot survey and can be tuned in params without a new ADR.
  - An image over its byte or area cap is a missing slot with a diagnostic.
  - A payload over its cap is `UNSUPPORTED_FORMAT` with `error.skin_too_large` and `bytes`/`maxBytes` args, mirroring `error.chart_audio_too_large`.
  - An unknown or rejected skin name is `NOT_FOUND`. The name must equal an entry of the current `Skins/` listing byte for byte, so OS name normalisation cannot alias a folder. Other IO errors are `INTERNAL`.
  - A skin with no `skin.ini` loads as lazer loads it: version `latest`, default mania config, plus a `skin.ini_missing` diagnostic. Stable shows such folders as skins too.
  - No new `ErrorCode`.
- **CSP and capabilities are unchanged.** The webview turns the base64 into bytes, wraps them in a `Blob` and calls `createImageBitmap`, cropping tall LN bodies to the visible length with its source-rectangle form. No URL is fetched or assigned, so `img-src data:` stays in the CSP but is not used by skins, and no capability is added.
- **cfg allowlist.** The allowlist gains `Skin`, `ManiaSpeed` (1–40, anything else is ignored), `ManiaSpeedBPMScale` and `UsePerBeatmapManiaSpeed` (spec 002 R9):
  - **Password safety.** The key is matched before the value is touched, and `Password` is never read. A bare CR ends a line, as it does in stable's reader, so `Skin = a\rPassword = …` cannot carry the secret into `skin`.
  - **`#` in `Skin`.** `Skin` keeps any `#`: it is part of the folder name, not a comment.
- **Default skin images are not bundled.** ppy/osu-resources is CC BY-NC 4.0, which an MIT app cannot redistribute.
  - **Missing slots.** Every missing slot falls back to the procedural drawing, one slot at a time.
  - **Gate.** As in lazer, skin rendering applies only when the column-0 key image resolves; otherwise the whole stage stays procedural.
- **Scroll modes.** `ScrollMode` is `osu` or `pxPerMs`:
  - **`osu`.** Stable's speed 1–40 in steps of 1. `pxPerMs = 0.035·n·H/480` for canvas height H, with the judgement line at `clamp(HitPosition, 240, 480)·H/480`. The time on screen is therefore `HitPosition·200/(7n)` ms, the community's `13720·(HitPosition/480)/n` with 480/0.035 = 13714.3 unrounded, and the same at any canvas size. The default speed is cfg `ManiaSpeed`.
  - **`pxPerMs`.** The fixed velocity the screen had before.
  - **Rate.** Real-time velocity stays constant under a rate, so in map time it is divided by the rate.
  - **lazer mode.** Reserved, not built: the same formula with a 0.1 step.
  - **BPM scaling.** Not reproduced, because its reference BPM is unverified (research 06).
  - **Params.** The constants (0.035, 480, 402, 240–480, 1–40) live in the stage params.

## Alternatives considered
- **Tauri asset protocol scoped to `Skins/`.** Rejected for the reasons in ADR 0018:
  - it needs `assetProtocol.enable`, a scope over the whole `Skins/` tree (junk files, `.exe`, `.zip` included) and a CSP entry for the asset scheme;
  - it hands the webview filesystem paths;
  - lookup (case, `\`, `@2x`, frames) would still have to happen somewhere before the URL is built.
- **A TypeScript `skin.ini` parser with per-image commands.** Rejected:
  - the webview would need the folder listing and the raw references, then send paths back to path-driven read commands, which breaks ADR 0018's rule that the webview never holds a path;
  - it would put a codec outside the tested Rust `source-osu` layer, beside the cfg/osr/osu!.db codecs it belongs with;
  - a 7K skin would cost up to 49 slot lookups as separate round trips instead of one call.
- **Decoding images in Rust (`image` crate).** It would decode the TIFF bodies and downscale tall images before IPC. Rejected:
  - it needs a new dependency and its own ADR;
  - it adds decode time and memory on the app side;
  - it serves only 2 TIFF files, both in skins that are not active.

  The webview already decodes PNG and JPEG natively, and `createImageBitmap` crops.
- **Bundling the default skin images.** Rejected: CC BY-NC 4.0 is incompatible with distributing wolluf under MIT. The procedural drawing is the default skin.
- **Porting stable's code.** Not possible: it is not public.

## Consequences
- **What the Label screen gets.**
  - It draws the pilot's skin with the CSP and capabilities ADR 0009 left. The bindings stay fully typed.
  - The default scroll speed matches the game's (speed 30 at HitPosition 428 shows 407.6 ms).
- **Payload cost.**
  - A `skin_get` costs one read per resolved image and a base64 payload about 4/3 of their bytes: under 1.5 MiB for 21 of the pilot's 25 7K skins.
  - The UI caches per (skin, key count, ini mtime) and closes its `ImageBitmap`s when the skin changes.
- **`Skins/` is a second read root beside `Songs/`.** `source-osu` must keep tests for:
  - traversal (`..`, absolute and drive-prefixed references);
  - a followed link;
  - case-insensitive resolution;
  - a TIFF magic case.

  The corpus check `corpus_skins` is `#[ignore]` and read-only, like the other corpus harnesses.
- **ppy/osu port.** NOTICE carries a ppy/osu entry for the ported decoder and lookup logic (ADR 0008).
- **Unverified approximations.** Several rendering rules are lazer's approximations of stable, unverified against stable:
  - key image height (no 1.6 factor in lazer);
  - the RepeatBottom tiling (lazer stretches with a self-described guess; wolluf tiles from the bottom);
  - the all-defaults fallback for a skin without `Keys: 7`;
  - the `NoteBodyStyle` enum, where the wiki's 0/1/2 disagrees with lazer's 0/2/3/4;
  - `-0` frames winning over the plain name;
  - an explicit reference to a missing file.

  The pilot's side-by-side check against a stable screenshot on the Windows build settles them. A mismatch is fixed in the renderer or its params and recorded in research 06. Only a change to the IPC contract or the read rules needs a new ADR.
- **Image fallbacks.** Images above the caps and non-PNG/JPEG files are drawn procedurally. A skin can look partly default and still be labelled.
- **Reuse beyond labelling.** F4 drills and a replay view can reuse `skin_get`. Animation frames, lighting and upscroll extend the DTO; they do not need a new ADR. A streaming need supersedes this ADR rather than adding a second transport.
