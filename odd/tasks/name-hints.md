# Pattern hints from names + mania-tracker integration draft

Branch `feat/name-hints` from `feat/f1-patterns` @ 60cd577 (stacked on PRs #6/#7) · opened 2026-10-05

## Objective
1. Every chart whose pack, folder or difficulty name states a pattern or skill (e.g. "bracket pack", "Jack Practice", KomeijiDove/Road to Gamma skill tags) carries weak pattern/axis hints in the library index. A corpus report shows how far the engine's segments agree with those hints.
2. A draft integration contract with mania-tracker (aleju03) is ready to share: vocabulary, label format, labelling protocol, chart-feature exchange and licensing.

## Problem and why
Section-level human labels are expensive. Names give chart-level weak labels at scale: aleju03 used exactly this on ~200k charts to tune 4K speed/tech. They let us measure and calibrate the engine on the whole library (the corpus run showed burst over-firing and trill/jumptrill under-firing), and choose which sections humans should look at (disagreements first). The pilot and aleju03 plan a shared crowd-labelling page on mania-tracker, which needs an agreed contract first.

## Scope
- Authorized:
  - engine `labels`: hint extraction (keyword dictionary → `PatternId` or `AxisId`, the matched token as evidence), pure;
  - hints stored as `chart_label` rows (new scales `hint_pattern` / `hint_axis`, `source = name_hint`), with a `chart_label` VERSION bump plus stage-lock;
  - app: hints written in IndexLibrary; a hint-vs-segment agreement query;
  - CLI `wolluf library hints`;
  - corpus report;
  - `docs/integration/mania-tracker.md` (draft).
- Out of scope: changing engine thresholds (eval, deliverable 4), any network code, building the web page.

## Constraints
- Hints are weak evidence, never gold. They never reach `wolluf label`, because the gold protocol stays blind.
- Keywords live in a data table with stable ids; they match whole words only, case-insensitive, and tolerate common spellings ("jumptrill", "jump trill", "JT"?). Each keyword decision is listed in the table with a WHY.
- Ambiguous generic words map to an axis, not a leaf: "jack" → `7k.regular.jack`, "tech" → `7k.regular.tech`, "LN" → `7k.ln.general`. Explicit leaf words map to leaves: "minijack", "bracket", "jumptrill", "inverse", "release", "shield".
- TDD strict. Delivery: ~600 lines plus the doc.

## Acceptance criteria
- Keyword table tests: positives, negatives (no hint from "Jackson" or "Tech N9ne"), and the leaf vs axis split → `cargo nextest run -p wolluf-engine hints`.
- Pilot corpus prints, per hinted pattern/axis: hinted charts, the mean share of segmented time on that pattern/axis in hinted charts, the library baseline share, and the lift → `corpus_name_hints`.
- The integration draft covers vocabulary, label JSON, protocol (gold vs correct queues, consensus, reliability, anti-bot), chart-feature exchange, recommendations API sketch, licensing, privacy and an open-questions list.

## Tasks
- [ ] T1: engine hint extraction + chart_label rows + VERSION bump + stage-lock. Route: delegated. Tier: medium. Commit: —
- [ ] T2: app IndexLibrary writes hints; agreement query; CLI `library hints`; corpus report. Route: delegated (same writer). Tier: medium. Commit: —
- [x] T3: `docs/integration/mania-tracker.md` draft. Route: inline. Tier: passive. Commit: `docs: draft mania-tracker integration contract`
- [ ] T4: close: gates, corpus, remove this document. Route: inline. Tier: passive. Commit: —

## Progress
- 2026-10-05 T3: draft written (vocabulary, label JSON, two-queue protocol, consensus/reliability/anti-bot, feature exchange, recommendations API sketch, licensing/privacy, open questions). Readback done.

## Next step
T1+T2: delegate the writer; T3 is written inline in parallel.
