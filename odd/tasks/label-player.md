# Label player layout

Branch `feat/label-player` from `feat/label-polish` @ d7464ca (stacked on PR #18) · opened 2026-10-06

## Objective
The Label screen's playfield takes the whole left column like a video player: timeline and playback controls appear over its bottom edge on hover, the playback settings (audio offset, scroll mode and speed, fit, zoom, playback rate, skin) live in a side flyout, navigation sits in the map card, and the answer bar confirms Save and Skip by holding.

## Problem and why
Pilot feedback on the PR #18 build (2026-10-06, screenshots):
- The toolbar, timeline, transport and skin rows plus the footer leave too little height for the playfield; it should feel full-screen.
- The timeline should behave like video controls (shown on hover at the bottom of the playfield) and let the window be resized up to 60 s.
- Playback settings belong in a lazer-replay-style side flyout opened from an edge arrow; the default skin becomes a Settings preference with a live preview.
- The header background is blurred beyond recognition: a gradient blur (sharp at the top, blurred towards the text), plus a button opening the full image with the map's full details.
- Previous / Next / Random / Now playing move into the map card; the session counters leave the footer.
- Skip and Save become icon buttons that need a 1-second hold (a ring fills), because a mis-click loses progress.
- The collapsed answer line hides extra chips without feedback: show "+N"; clicking a chip takes focus to that pattern in the panel.
- Each axis section gets a small icon drawn from notes (jack: stacked chordjack notes; tech: tight clusters; speed: a stair; stream: a stream; LN axes: long-note shapes).

## Scope
- Authorized: `crates/app` (window resize with a max length param, chart details), `crates/store`/`crates/source-osu` (only if details need catalog fields such as tags/source; cache schema bump), `apps/desktop/src-tauri` (appended commands), `bindings.ts`, `apps/desktop/ui/src/**` (label, playfield, preferences, settings, locales), ADR 0018 amendment.
- Out of scope: session labelling, animated pattern previews, layout-aware segmentation.

## Constraints
- Window length 1–60 s, a param (D17); resizing keeps the window inside the chart span; labels still reject overlaps.
- Hold-to-confirm: 1000 ms, releasing early cancels; works with pointer and keyboard (hold Enter/Space on the focused button); visible ring progress, also with reduced motion (no easing, still progress); accessible description "Hold to …".
- Flyout and overlay controls are keyboard reachable (not hover-only): focus or a toggle button opens them; Esc closes the flyout.
- Default skin preference reuses the existing skin storage key so the Label screen and Settings agree.
- ADR 0009 appended commands; one bindings owner per stage. Tokens only, motion-safe, i18n en+es.
- TDD strict. Runners: `cargo nextest run -p wolluf-app`, `pnpm -C apps/desktop/ui test`.

## Acceptance criteria
- `label_resize_window(anchor, t0Ms, t1Ms)` clamps to the span and to 1–60 s; `chart_details(md5)` returns the card/dialog metadata → service tests.
- The playfield fills the column; timeline + play controls overlay its bottom on hover/focus; timeline window resizable by handles (pointer + keyboard) up to 60 s → UI tests.
- Side flyout holds audio offset, scroll mode, osu! speed, fit, zoom, playback rate and skin; opens by edge arrow, hover or keyboard → UI tests.
- Header: gradient blur, full-image dialog with details; nav buttons and session counters in the card; footer row gone → UI tests.
- Answer bar: icon Save/Skip with 1 s hold, "+N" overflow, chip click focuses the pattern card (opening its section) → UI tests.
- Axis icons render for every axis (RICE and LN) → tests.
- Settings: default skin with a live preview on a synthetic example → tests.

## Tasks
- [x] T1 — Backend: `label_resize_window`, `chart_details` (+ catalog fields if needed). Route: delegated. Tier: medium. Commit: feat: play label windows full height with hold-to-confirm answers
- [x] T2 — Axis icons from notes, used on the axis cards. Route: delegated. Tier: medium. Commit: feat: play label windows full height with hold-to-confirm answers
- [x] T3 — Right-panel components: HoldButton, AnswerBar (+N, icon hold Save/Skip, chip → focus pattern), ChartHeader (gradient blur, details dialog, nav slot, counters), PatternGrid focus API. Route: delegated. Tier: medium. Commit: feat: play label windows full height with hold-to-confirm answers
- [x] T4 — Settings: default skin with live preview. Route: delegated. Tier: medium. Commit: feat: play label windows full height with hold-to-confirm answers
- [x] T5 — Player layout: full-height playfield, hover overlay (timeline with resize, play/pause, time), settings flyout (incl. playback rate), integration of T3 components, toolbar/footer removal. Route: delegated (after T1–T3). Tier: medium. Commit: feat: play label windows full height with hold-to-confirm answers
- [ ] T6 — Close: verifier, gates, ADR amendment, Windows build. Route: inline + verifier. Commit: —

## Progress
- 2026-10-07 Workflow wf_9118b846-ded (6 agents) built T1–T5: backend `label_resize_window` (SamplerParams `max_window` 60 s, Widen capped too), `chart_details`, catalog `source`/`tags` (CACHE_SCHEMA_VERSION 5, CATALOG_VERSION 4); axis icons; HoldButton/AnswerBar/ChartHeader/PatternGrid focus handle; default-skin card in Settings; player layout (PlayerFrame, PlayerControls, PlaybackSettings, HeaderNav; SessionFooter/SessionToolbar/Transport/skins.ts removed). Gates at that point: workspace nextest 1014 passed / 19 skipped, clippy/fmt/check-layers/stage-lock ok, UI 56 files / 654 tests.
- 2026-10-07 Verifier: all 7 criteria met; M1 externally driven holds (Enter shortcut, pointer with keepFocus) do not cancel on window blur/visibilitychange; M2 chip click parks focus on a pattern card so Enter/Space removes it; M3 timeline handles cover narrow windows on long charts; L4 offset not scaled by rate; L5 rate slider restarts the loop each step; L6 AnswerBar a11y wiring (+N label inside aria-hidden, Save hint, disabled-Skip reason); L7 duplicate tag keys; L8 Settings skin fetch differs from the Label screen + endless pulse on example failure; L9 focusPattern persists the opened section.
- 2026-10-07 Correction round paused by the user mid-way; resumed 2026-10-07: a writer re-checked every finding and found all of them (M1–M3, L4–L9) already fixed in the checkpoint, each guarded by a test it saw fail when the fix was removed (the parent's earlier spot check was wrong). ADR 0018 amended (2026-10-07).
- 2026-10-07 Gates: fmt ok, clippy ok (also --all-features), check-layers 0 violations, stage-lock ok, lint-canary ok, deny ok, `cargo nextest run --workspace` 1014 passed / 19 skipped, bindings drift none, tsc ok, lint ok, UI 56 files / 677 tests.

## Next step
T6: hand the pilot the Windows build of the PR stacked on #18; after their check, `git rm` this document.
