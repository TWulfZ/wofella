# 0002 Deterministic algorithms and small statistics, not deep learning

- Status: Accepted
- Date: 2026-09-28

## Context
wolluf has to label patterns, estimate per-axis skill, and recommend a chart, a section and a rate. It must explain every number and improve from feedback (architecture G1, G4). The forces:
- **Data volume.** The pilot has about 4.3k 7K plays (research `00-plan-es.md` l.53: 4,338 7K scores in scores.db). Opt-in cross-player data does not exist and will not before F5 (~10 users). A deep model fitted to one player's history would be mostly prior.
- **Prior art.** Every open-source VSRG pattern labeller found is rule-based. That is a design signal, not proof of reliability: no engine publishes accuracy against human labels (research `01-landscape-verified.txt` l.54, l.84). mania-hub, the most advanced tracker, trains nothing on player results (research 00 l.29). Where ML appears in prior art, it sits on top of deterministic features as a chart-side calibrator: Companella's GBM dan estimator runs over MinaCalc, Interlude and Sunny features (research 01 l.41).
- **A known failure to avoid.** Interlude removed its per-pattern ratings (0.7.28.3) because they credited whole-chart accuracy to every pattern, kept only the maximum, and decayed the mean without uncertainty (research 00 l.34; research 01 l.108).
- **Reproducibility.** Derived numbers must be identical on WSL and Windows and replayable from raw data (G1, D3). A deterministic pipeline with versioned stages gives that for free.
- The research verdict: deterministic algorithms plus a small per-player Bayesian model, no deep learning (research 00 l.77–l.79; research 01 l.102).

## Decision
wolluf is built from two kinds of component (architecture §2):
1. **Deterministic algorithms** for parsing, re-judging, pattern detection and segmentation, difficulty features, session rules, drill cutting and recommendation scoring. They change only through code, which bumps the stage `VERSION` (ADR 0006).
2. **Small statistical models**, each with interpretable parameters and an uncertainty, fitted by Bayes' rule or least squares: per-axis skill θ_a ~ N(μ, σ²) with a per-play offset τ_p and shrunk pattern offsets; chart difficulty calibration (ridge or isotonic, IRT item offsets only where the data suffices); feedback-derived preference and calibration offsets.

Rules and constraints that follow:
- The thresholds and weights in rules are parameters. They live in versioned param structs and pack sections, never inline (D17), so data can tune a rule without changing what it reads like.
- Every statistical update is checked against the numbers it replaces. Prequential logging (§6.1) and the offline eval gate (§6.5) must show a better predictive score before a new pack or model ships.
- **Larger ML enters later, if at all**, only behind an existing trait (`SegmentLabeler`, `DifficultyCalculator`, `SkillModel`, `Explainer`) and only if it beats the incumbent on the frozen eval suites. Inference would use `tract` (pure-Rust ONNX), not `ort`, to avoid shipping a native runtime. The model hash goes into the version key.
- An LLM, if one is added, only explains structured `Why` and report data. It never produces or changes a number.
- Python is allowed for research notebooks and oracles only. It is never on the release path: packs come from the Rust fitters (F5).

## Alternatives considered
- **Deep learning for skill or difficulty now.** Rejected: at ~4.3k plays for one player a Bayesian filter is better posed and it is explainable, and there is no cross-player data to train on. It would also need a GPU or a native runtime and would break the "reproduce from raw" guarantee unless it was pinned very carefully.
- **Learned pattern classifier from day 1.** Rejected: there is no labelled gold set yet (F1 builds it through relabel capture). A classifier would need exactly the labels the rule engine and the Playfield produce first. It can replace the rule runner later through `SegmentLabeler` if it wins S2.
- **Pure heuristics with no statistics (MSD bands around a weighted mean, as in Companella's recommender).** Rejected: there is no uncertainty, so a rating from 3 plays looks as solid as one from 300, which repeats Interlude's failure. Feedback also cannot move such a system in a measurable way.
- **Elo/Glicko per pattern.** Kept as prior art but not chosen: a Gaussian filter with τ_p and pattern offsets is the same idea with explicit per-play correlation, and it supports the step-append == full-refold property test.

## Consequences
- Every number the UI shows can be traced to rules, parameters and data, and the "Why" panels render real reasons.
- The eval gate (§6.5) and prequential logs become load-bearing infrastructure. They must exist before any model change ships.
- Some ceiling on accuracy is accepted in exchange for explainability and small-data robustness. The traits keep the upgrade path open.
- Domain crates stay pure Rust with no ML runtime dependency, which keeps builds and the licence surface small (ADR 0004, ADR 0008).
