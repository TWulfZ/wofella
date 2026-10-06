# osu! skins and scroll-speed modes in the label Playfield

Branch `feat/playfield-skins` from `feat/label-playfield` @ docs close · opened 2026-10-06

## Objective
The Label screen draws charts with the pilot's own osu! stable skin and scroll speed: osu! speed 1–40 (default from the cfg), the skin's column widths and HitPosition, notes, LNs, keys and stage, plus a larger playfield. Missing elements fall back to the current procedural drawing.

## Problem and why
The pilot found the procedural Playfield (px/ms speed, narrow fixed width) uncomfortable next to what they read in-game every day. Labelling comfort decides how many gold labels get made (deliverable 4 needs 200–300).

Research summary (2026-10-06 workflow; to be moved to `docs/research/` in T4):
- osu! stable is closed source.
- lazer (ppy/osu, MIT, @6359741b) reimplements legacy mania skins.
- The default skin assets are CC BY-NC 4.0 and are not bundled.
- 25 of 27 pilot skins have a `Keys: 7` block. Active skin and `ManiaSpeed = 30` come from the cfg.
- Stable on-screen time is `13720·(HitPosition/480)/n` ms.

Rejected:
- Porting stable code: it does not exist publicly.
- A TS skin.ini parser: it would need path-driven image commands from the webview.
- The asset protocol: rejected for the reasons in ADR 0018.
- Bundling default skin images: licence.

## Scope
- **Authorized:**
  - `crates/source-osu`: the `codec/skin_ini.rs` codec, read-only `skins.rs`, and the cfg allowlist keys `Skin`, `ManiaSpeed`, `ManiaSpeedBPMScale`, `UsePerBeatmapManiaSpeed`.
  - `crates/app` skins slice.
  - `apps/desktop/src-tauri` commands `skin_list`, `skin_get`.
  - `apps/desktop/ui` (`features/playfield`, `features/label`).
  - `docs/adr/0019-*`, `docs/research/`, NOTICE.
- **Out of scope:**
  - animated frames, lighting, upscroll/`UpsideDown`;
  - lazer scroll mode (the type only), SV-aware scrolling;
  - scrubbing and slow playback.

## Constraints
- **osu! folder:**
  - read-only: `Skins/` is a second read root with textual containment like `read_song_file`;
  - the webview sends only skin folder names, never paths;
  - no CSP or capability change (ADR 0009, ADR 0018).
- **cfg:** the `Password` line is never read or kept (spec 002 R9 allowlist).
- **Licensing:** lazer logic ported with credit in NOTICE (`ppy/osu @ 6359741babba4ced42da1fcda738c35370fdf3f4`). No ppy/osu-resources images. No user skins in the repo or in fixtures: fixtures are synthetic PNGs generated in tests.
- **Params:** caps and constants in param structs (D17).
- **Depends on:** #12 (`feat/label-playfield`), #10, #9.
- **TDD:** strict. Runners: `cargo nextest run -p <crate> <filter>`, `pnpm -C apps/desktop/ui test <filter>`.
- **Delivery:** about 1,800 authored lines, one PR (the pilot chose a single PR for #12; same here).

## Acceptance criteria
- osu! speed n with HitPosition 428 shows 407.6 ms on screen; the speed defaults to cfg `ManiaSpeed`; F3/F4 change it ±1 → `pnpm test playfield`.
- `skin.ini` parsing follows lazer's decoder rules → `cargo nextest run -p wolluf-source-osu skin_ini`. Rules:
  - BOM and CRLF are handled;
  - lines before `Keys:` belong to that block, and the first duplicate block wins;
  - image indices are 0-based and colour indices 1-based;
  - Version defaults to 1.0, and `latest` means 2.7.
- Skin assets resolve case-insensitively, `\`→`/`, `@2x` first, magic-checked and capped; traversal is rejected → `cargo nextest run -p wolluf-source-osu skins`; corpus `corpus_skins` (ignored, read-only).
- The Label screen renders the active skin and falls back per slot → `pnpm test playfield`, plus the pilot's side-by-side check on the Windows build.

## Tasks
- [x] T1 — Scroll modes (`osu` 1–40 / `pxPerMs`), judgeY from HitPosition, paused view at the chosen speed, F3/F4, prefs migration. Route: delegated (writer). Tier: medium. Commit: `feat(ui): add osu! scroll speed mode and a larger playfield` (with T2: shared files)
- [x] T2 — Larger playfield: width from columns × height, zoom. Route: delegated (writer). Tier: medium. Commit: same as T1
- [x] T3 — cfg allowlist keys `Skin`, `ManiaSpeed`, `ManiaSpeedBPMScale`, `UsePerBeatmapManiaSpeed` (codec only; exposure to the UI moves to T7). Route: delegated (writer) + parent fix. Tier: **high** (verifier). Commit: `feat(source-osu): read skin and mania speed from the osu! cfg`
- [x] T4 — ADR 0019 (skin assets over IPC), `docs/research/06-mania-skin-and-scroll.md`, NOTICE. Route: delegated (docs writer) + parent amendment. Tier: passive. Commit: `docs: record skin assets over IPC and mania skin research`
- [x] T5 — `codec/skin_ini.rs` pure parser. Route: delegated (writer). Tier: medium. Commit: `feat(source-osu): parse skin.ini mania sections as lazer does`
- [x] T6 — `source-osu/src/skins.rs` read-only resolution (lookup chain, IHDR and magic, caps), `corpus_skins`. Route: delegated (writer + one correction). Tier: **high** (verifier). Commit: `feat(source-osu): resolve skin images read-only with lazer's lookup`
- [x] T7 — app skins slice plus `skin_list` (with current, cfg speed defaults, ini mtime) and `skin_get` commands, errors, bindings. Route: delegated (writer) + parent fixes. Tier: **high** (verifier). Commit: `feat(desktop): serve osu! skins and cfg speed defaults over IPC`
- [x] T8 — UI skin loader (ImageBitmap cache, tall-body crop) and picker defaulting to cfg `Skin`; cfg `ManiaSpeed` default. Route: delegated (writer) + parent fixes. Tier: medium. Commit: `feat(ui): pick an osu! skin and default speed in the label screen`
- [x] T9 — Skin renderer (`skinLayout.ts`, `draw.ts`): columns, lines, hint, notes, LN Stretch/RepeatBottom, flipped tail, keys, stage, per-slot fallback. Route: delegated (writer). Tier: medium. Commit: `feat(ui): render the playfield with an osu! legacy mania skin`
- [ ] T10 — Close: full gates, corpus run (osu! closed), the pilot's side-by-side check, remove this document. Tier: high. Commit: —

## Progress
- 2026-10-06 T1+T2:
  - **RED:** `stage` module missing; 9 rewritten Playfield tests (canvas width 1400 vs 525); `readScrollPrefs is not a function`. The F3/F4 test failed with the key map removed. All → GREEN.
  - **Results:** speed 30 / HitPosition 428 → 407.6 ms at any height. UI: tsc and lint clean, vitest 38 files / 297 passed, build ok.
  - **Open:** F3/F4 direction. The wiki copy says F3 faster / F4 slower; the implementation is F3 −1 / F4 +1. The pilot decides.
- 2026-10-06 T3+T5:
  - **RED:** missing `UserCfg` fields, then 5 of 13 cfg tests; stub parser 19 of 23 → GREEN.
  - **Corpus:** `corpus_skin_ini` ran with osu! closed: 26 parsed, 25 with `Keys: 7`.
  - **Verifier:** confirmed a Password leak on bare-CR files (`Skin = a\rPassword = …` kept the secret inside `skin`). RED `a_bare_cr_ends_a_line…` (`left: Some("a\rPassword = secret…")`) → fixed by splitting lines on CR as stable does. Suggestion taken: `clamp` replaced with `max/min`, which cannot panic on bad params.
  - **Checks:** nextest source-osu+app 341 passed; clippy and fmt clean.
- 2026-10-06 T4: docs writer verified every lazer/wiki line and URL (7 inherited line ranges corrected); survey anonymised (counts only). Parent amended the ADR so a skin without `skin.ini` loads with lazer defaults (as the code and lazer do), names must match the listing byte for byte, and a per-skin decoded-pixel budget applies; NOTICE credits ppy/osu @6359741b.
- 2026-10-06 T6:
  - **RED:** the stubbed module failed all 15 integration tests → GREEN.
  - **Verifier:** no escape (`..`, UNC, drive, NUL, `.@2x.` → `..`, ADS forms on Windows checked read-only) and no exhaustion (2 GiB sparse files, FIFO, `/dev/zero`, 2³¹² IHDR).
  - **Mismatches with ADR 0019, fixed in one correction, each with RED:**
    - `-0` before plain;
    - folder must equal a listed entry;
    - followed-link test (RED forced by disabling link follow);
    - IHDR length 13 / SOF ≥ 8;
    - per-skin pixel budget 64 Mi px;
    - `skin.ini` found past a truncated listing.
  - **Corpus `corpus_skins`** (osu! closed): 27 skins, 25 with 7K. Active skin: 47 slots, 17 files, 344,763 bytes, no diagnostics.
  - **Parent:** source-osu 179 passed; clippy, fmt, check-layers clean; no write calls in `skins.rs`.
- 2026-10-06 T7:
  - **RED:** 6 of 7 app tests failed against the stub; smoke and i18n tests failed → GREEN.
  - **Verifier:** constraints hold — folder-only input, allowlisted cfg only, current only when listed, caps before base64, chart_audio unchanged after the base64 move.
  - **Confirmed and fixed by the parent:**
    - stale bindings (regenerated);
    - missing `skin.ini` mtime cache key (RED `no field ini_mtime` → added as an epoch-ns string through `SkinEntry` and the DTO);
    - image-specific copy for an oversized `skin.ini` (now a generic "skin too large", en and es).
  - **Also by the parent:** a Password-sentinel guard on the serialized list, and the ADR updated to the real command and field names.
  - **Gates:** nextest source-osu+app+desktop 396 passed; clippy, fmt, check-layers clean.
- 2026-10-06 T9:
  - **RED:** missing modules and `spans` → GREEN, 98 playfield tests.
  - **Parent:** a non-positive `widthForNoteHeightScale` hid every note (RED `expected +0 to be close to 35.16`) → now falls back to the narrowest column as lazer does. UI vitest 326 passed; tsc and lint clean.
  - **Unverified approximations, to be checked side by side:** key height ×k (not lazer's ×k/1.6), body tiling, zoom not moving the judgement line.
- 2026-10-06 T8:
  - **RED:** loadSkin 8/8, useLoadedSkin 5/5, Playfield width, prefs 4, LabelScreen skins 11/11 → GREEN.
  - **Review:** no confirmed defects. Two of the seven suggestions were applied by the parent:
    - the stale entry is cleared in the cleanup, so a returning DTO object cannot surface closed bitmaps;
    - `imageOrientation: "none"` (RED via mutation: 1 failed) so bitmaps match the header sizes.
  - **Deferred** (recorded, not done):
    - cancelling superseded loads;
    - crop to the visible length instead of 4096 px (affects Stretch bodies);
    - an early F3/F4 before the skin list arrives overrides the cfg speed;
    - server diagnostics not shown;
    - extra exports.
  - **Gates:** UI vitest 42 files / 357 passed; tsc, lint, build ok.

## Next step
T10: CI installer for the pilot; side-by-side with stable (active skin, a Stretch skin with a tall body, F3/F4 direction); full gate block; remove this document. Research material: `<scratchpad>/skins-research/{PLAN,REPORTS}.md` and the lazer sources under `<scratchpad>/skins-research/lazer-legacy-mania/src/`.
