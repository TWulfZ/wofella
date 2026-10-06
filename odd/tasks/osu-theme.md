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
- [x] T3 — Setup screen, identity wizard, job tray, not-self banner and primitive polish. Acceptance: UI gate green, screenshots. Route: delegated (workflow: writer → 4-lens review → 3-skeptic verify → one fix pass → final gate). Tier: medium. Commit: `feat(ui): restyle setup, identity wizard and job tray in osu! style`
- [ ] T4 — Follow-ups from the final completeness check, pending the user's review: themed error alerts (setup detect, alias load, save conflict) instead of bare red text; failed last sync shows its error on home; completed setup step gets a check icon and a done state for screen readers; the bottom-right corner (jobs pill vs sticky confirm bar vs toast) and the auto-opened tray covering home tiles; button glow transition excludes box-shadow; badge success/warning variants were not added (status uses icon + coloured text); badge/checkbox 50% focus halos; dialog overlay restyle unused so unverified. Then feature close. Route: —. Tier: medium. Commit: —

## Progress
- 2026-10-06 T1: CSS-only, no meaningful RED (stated exception). `tsc --noEmit && lint && test`: 126 passed; `build`: ok. Screenshots (Playwright + mocked `__TAURI_INTERNALS__`, scratch harness) show dark tokens under a light OS scheme.
- 2026-10-06 T2: RED 5 failed (`Unable to find an element with the text: Your osu!mania training hub`, `New plays`, `Ya conocidas`, `Language, folders and identity`, `Idioma, carpetas e identidad`) → GREEN. Spot check `tsc && lint && test`: 131 passed. Screenshots: home, settings.
- 2026-10-06 T3: RED 5 failed (`Unable to find role="list" and name "Setup steps"`, `Ready to use`, region `Confirm your names`, steps in wizard, region `Save your names`) → GREEN. Review: 4 lenses, 38 raw → 14 confirmed by ≥2/3 skeptics, all applied in one pass (RED 4 failed: queued job progressbar, ScopePicker title, alias/chart title tooltips, root error banner → GREEN). Final gate `tsc && lint && test`: 138 passed; build ok. Spot check: same gate, 138 passed.

## Next step
User reviews the screenshots and T4's list; implement the accepted follow-ups, then close (full gate block, `git rm` this file).
