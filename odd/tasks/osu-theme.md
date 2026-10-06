# osu!-style desktop theme

Branch `feat/osu-theme` from `main` @ 2ac4f9b · opened 2026-10-06

## Objective
The desktop UI looks like an osu! companion tool (osu!-web palette, pink accent, mania-flavoured brand) instead of the stock shadcn neutral theme.

## Problem and why
The F0 shell ships shadcn's neutral defaults; osu! players expect the dark osu!-web look shared by tosu, maps analyzers and osu!trainer. The user chose **dark-only** on 2026-10-06, superseding spec 005 "Styling" (dark follows `prefers-color-scheme`). A light osu! variant was rejected: no reference tool has one and it doubles the contrast work.

## Scope
- Authorized: `apps/desktop/ui/` (styles, `index.html`, `shared/ui`, `app/`, `routes/`, `features/*/components`, i18n `en`/`es`), one exact-pinned font package.
- Out of scope: new screens or data (library, playfield, charts), the osu! logo or any official osu! asset, IPC or Rust changes.

## Constraints
- D14 boundaries and the shadcn layout (`shared/ui`) stay as they are; the UI computes no domain values (architecture §8).
- Text contrast ≥ 4.5:1 on every surface; focus rings visible; `prefers-reduced-motion` honoured.
- No CDN assets: Tauri runs offline, so fonts come from `@fontsource-variable/*` (OFL).
- Depends on: F0 shell (merged).
- TDD: strict where behaviour changes (new markup, i18n keys). Pure token/CSS changes have no meaningful RED; their check is the UI gate plus a rendered screenshot. Runner: `pnpm -C apps/desktop/ui test`
- Delivery: ~450 authored lines forecast

## Acceptance criteria
- Dark osu! tokens apply regardless of the OS theme, including shadcn `dark:` variants → `pnpm -C apps/desktop/ui build` + screenshot
- Shell, home, settings, setup, identity and job tray use the new header, page headers and accents → UI gate green + screenshots

## Tasks
- [x] T1 — osu! dark tokens (hue 333 surfaces, pink primary, blue/lime/yellow/purple accents), Exo 2 display font, class-based dark variant, triangles backdrop. Acceptance: UI gate green, build ok. Route: inline. Tier: medium. Commit: `feat(ui): add dark osu!-style theme tokens and Exo 2`
- [x] T2 — Shell header (mania brand mark, osu!-web tab nav), `PageHeader`, home stat tiles, settings restyle. Acceptance: UI gate green, screenshots. Route: delegated: writer touches 2+ non-trivial files. Tier: medium. Commit: `feat(ui): restyle shell, home and settings in osu!-web style`
- [ ] T3 — Setup screen, identity wizard, job tray, not-self banner and primitive polish; feature close. Acceptance: full UI gate green, screenshots. Route: delegated: writer touches 2+ non-trivial files. Tier: medium. Commit: —

## Progress
- 2026-10-06 T1: CSS-only, no meaningful RED (stated exception). `tsc --noEmit && lint && test`: 126 passed; `build`: ok. Screenshots (Playwright + mocked `__TAURI_INTERNALS__`, scratch harness) show dark tokens under a light OS scheme.
- 2026-10-06 T2: RED 5 failed (`Unable to find an element with the text: Your osu!mania training hub`, `New plays`, `Ya conocidas`, `Language, folders and identity`, `Idioma, carpetas e identidad`) → GREEN. Spot check `tsc && lint && test`: 131 passed. Screenshots: home, settings.

## Next step
T3: setup screen, identity wizard, job tray, not-self banner, primitives; then feature close.
