# Pattern previews

Branch `feat/pattern-previews` from `main` @ 8682288 · opened 2026-10-06

## Objective
In the Label screen every pattern is a card in a grid with a mini playfield of a canonical example, grouped under large axis headings, with a larger preview on hover or keyboard focus and a search over pattern names.

## Problem and why
- The pilot asked for a preview per tag, like the VSRG Pattern Glossary rev.4, then for a card grid with large axis headings and an enlarged preview on hover (2026-10-06).
- Examples are synthetic and live next to the taxonomy, so the label screen stays blind (ADR 0018: no segments or real chart data in labelling) and the previews cannot drift from the engine: a test asserts the engine detects each example as its own pattern.
- Rejected: hand-drawn previews in TypeScript (they duplicate the rules and drift, as burst and delay already did in #10); real chart snippets from the library (break blindness, and need segments over IPC).

## Scope
- Authorized: `crates/engine` (new `examples` module), `crates/app/src/features/labeling` (examples DTO + service), `apps/desktop/src-tauri` (one new command), `bindings.ts` via `cargo xtask bindings`, `apps/desktop/ui/src/features/playfield` (static `PatternPreview`), `apps/desktop/ui/src/features/label` (pattern grid replaces chips, search), `apps/desktop/ui/src/shared/ui` (hover-card wrapper), label locales, ADR 0018 amendment.
- Out of scope: answer bar position, top toolbar and chart search (`feat/label-navigation`), chart header media, animation of previews (later), the session labelling flow.

## Constraints
- D1: app reaches engine only; examples are engine data mapped to DTOs in app (ADR 0009: no specta on domain types, no 64-bit fields, `<feature>_<verb>` command names, append-only `collect_commands!`).
- Validation uses default params and the default 7K layout (`k7.313_right_thumb`); tag-only patterns (`hand_imbalance`, `thumb`, `ln.general.density`) are accepted as a secondary tag.
- The pattern search input must be a text field so the type-to-answer redirect skips it; the Answer textbox keeps its accessible name.
- UI: preview opens on hover and on focus (not hover-only), respects `prefers-reduced-motion`, empty search state with a hint, contrast ≥ 4.5:1 on the dark theme.
- TDD: strict. Runners: `cargo nextest run -p wolluf-engine`, `cargo nextest run -p wolluf-app`, `pnpm -C apps/desktop/ui test`.
- Delivery: ~700 authored lines forecast (26 example definitions dominate) — single PR unless the user prefers slices.

## Acceptance criteria
- Every K7 taxonomy id has exactly one example; the engine detects it as the primary of the segment covering the example's display midpoint (tag-only ids: in that segment's secondary tags) → `examples::tests::every_example_is_detected_as_its_own_pattern`.
- `label_pattern_examples(keymode)` returns one `ChartWindowDto`-shaped example per id → app service test + bindings drift check.
- `PatternPreview` draws a static window without a clock → vitest.
- The Label screen shows axis sections with large headings and a card per pattern with its preview; clicking a card toggles it in the answer as chips did; hover or focus shows the enlarged preview with the description; the search filters cards by name, key or description and shows an empty state → LabelScreen tests.

## Tasks
- [x] T1 — Engine `examples` module (26 examples, row builder with optional red line, display span) + detection test. Acceptance: first criterion. Route: delegated (writer: engine module + tests). Tier: medium. Commit: feat(engine): serve a synthetic example chart per pattern
- [x] T2 — `label_pattern_examples` command + DTO + bindings; ADR 0018 amendment (synthetic examples keep labelling blind). Acceptance: second criterion. Route: delegated with T1 (same writer ran `cargo xtask bindings` as single owner in the workflow). Tier: medium. Commit: feat(engine): serve a synthetic example chart per pattern
- [x] T3 — `PatternPreview` static canvas + hover-card wrapper + pattern card grid with search, wired into the Label screen. Acceptance: third and fourth criteria. Route: delegated (UI writer + wiring writer). Tier: medium. Commit: feat(ui): pick patterns from a card grid with live previews
- [ ] T4 — Close: full gates, Windows build for the pilot, remove this document. Route: inline. Tier: medium. Commit: —

## Progress
- 2026-10-06 T1+T2 (workflow wf_e34fa9df-793, backend writer): RED `examples_cover_the_taxonomy_exactly_once`, `every_example_is_detected_as_its_own_pattern`, `pattern_examples_give_one_synthetic_window_per_pattern` → GREEN; 26 examples pass under default params without rule changes. Verifier: all criteria met; corrections applied by the parent: keymode dispatch moved to `examples::for_keymode` (D5) with a per-profile coverage test, DTO mapping moved to `ChartWindowDto::from_window` (D12), validation layout taken from the registry profile, bindings name list extended, ADR 0018 amendment. Gates: fmt ok, clippy ok, check-layers 0 violations, stage-lock ok, `cargo nextest run --workspace` 962 passed / 19 skipped, bindings drift none.
- 2026-10-06 Finding (out of scope, not fixed): the backend writer saw a short higher-priority candidate split a long lower-priority section into sub-minimum runs that are all withdrawn, leaving no segment (seen with short rolls inside delay, short brackets inside handstream). Needs its own investigation.

- 2026-10-06 T3 (UI writer + wiring writer): RED PatternPreview 3/3 and PatternGrid 12/12 against stubs, 3 new LabelScreen tests → GREEN. PatternChips removed (helpers in `components/patterns.ts`); i18n keys under `label.patternGrid.*` because `label.patterns` is already a string. Gates: tsc ok, lint ok, `pnpm -C apps/desktop/ui test` 45 files / 414 tests passed. Not covered: hover (focus is tested; Radix hover is not exercised in jsdom).

## Next step
T4: push the branch as a draft PR, hand the pilot the `desktop-windows` portable exe, then the full gate block and `git rm` this document.
