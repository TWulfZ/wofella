# 0007 Telemetry privacy, re-identification and osu! ToS

- Status: Accepted
- Date: 2026-09-28

## Context
Global model improvement (G4) eventually needs data from more than one player: difficulty calibration, IRT discriminations, population priors and pattern thresholds. Two external constraints limit how that data may be obtained:
- The osu! API v2 terms forbid mass harvesting of scores, and the data.ppy.sh dumps state "Permission is NOT implicitly granted to deploy this in production use of any kind" (research `01-landscape-verified.txt` l.200; `00-plan-es.md` l.75).
- Public osu! scores expose (chart md5, time, accuracy). Anything wolluf uploads that approximates those three values can be linked back to a named account.

wolluf also holds other players' plays locally (downloaded replays in scores.db, ADR 0005). Uploading any of them would share data about people who never consented. A privacy rule that depends on discipline will eventually be broken by a refactor, so G6 requires privacy to be enforced by types.

## Decision
**Architecture §6.4 applies as written.**
- **No telemetry backend and no upload before F5**, and not before there are about 10 real opt-in users. The *local* capture schema (`feedback_event`, `rec_impression`, `report_impression`, `skill_trace`) exists from F0/F1, so no signal is lost meanwhile.
- **Consent.** Explicit and scoped (`model_telemetry`), revocable, and shown with a preview of the literal next batch. Revoking stops uploads and calls the delete endpoint with the install secret. `feedback_event.telemetry_state` shows exactly what was shared.
- **Batches are built at send time, never at ingest**, from the current self scope minus current exclusions. An alias marked `not_me` later, or a play excluded later, therefore never leaks.
- **Typed gates (D10).** Telemetry builders take `&ConsentToken`, minted only from a current consent row, and a `SelfScope`, obtainable only from a `kind=self` profile. Both have private constructors, and trybuild compile-fail tests plus a privacy regression test guard them.
- **What is sent** (self scopes only, keyed by a random install UUID that is never an osu! id): app version and manifest; per segment the pattern, axis, chart md5, rate, d, `y` quantised, n, `p_pred`, model version and a random per-play grouping id; a jittered relative day index instead of timestamps; explicit feedback and label corrections as anchors; recommendation outcomes.
- **What is never sent:** player names, osu! ids, tokens, file paths; replays, per-note offsets, chart note rows; whole-play accuracy or score, exact times; other players' plays, whatever the selection; logs (§7).
- **osu! ToS.** No harvesting of the osu! API or site. OAuth is used only for the logged-in user's own `/me` (optional, from F3). data.ppy.sh dumps are never used in production. All global data comes from consenting users' own files.
- **Server** (F5): append-only object storage, a per-install rate limit, per-contributor weight caps and a delete endpoint. There is no training and no domain logic on the server.

**Re-identification: documented residual risk.** Dropping whole-play accuracy and exact times and jittering days is necessary but not sufficient, because Σ y·n over a play's segments approximates its accuracy. The mitigations are per-play segment subsampling and quantisation of `y`. **The parameters (subsampling rate, y quantisation step, day jitter) and whether the residual risk is acceptable at all (O3) are explicitly deferred to F5.** They are decided before any upload exists, by amending this ADR, and the consent UI states the residual risk in plain words. §6.4's "0.5 pp" quantisation is a starting point, not a decision.

## Alternatives considered
- **Harvesting public scores through the osu! API or the data.ppy.sh dumps.** Rejected: it violates the API terms and the dump licence, and the models would train on people who never agreed to it.
- **Building batches at ingest time.** Rejected: an alias later reclassified as `not_me`, or a play excluded later, would already sit in the outbox.
- **Uploading per-note offsets or whole-play accuracy for richer fitting.** Rejected: the re-identification and ToS exposure is not worth the gain at this scale (Appendix A).
- **Privacy by convention (code review only).** Rejected: G6 requires types that make the wrong call uncompilable.
- **Usage analytics or crash upload.** Rejected as a non-goal (§1, §7): the only channel that ever leaves the machine is opt-in model telemetry.

## Consequences
- Until F5 wolluf makes no network calls for telemetry. F0–F4 privacy is guaranteed by the absence of a transport.
- F3's `SelfScope` and F5's `ConsentToken` must keep private constructors, and their compile-fail tests are part of the release gate.
- Global fitting will have less data than a harvesting approach would give. The eval gate (§6.5) decides whether each pack still earns its release.
- O3 must be closed by an amendment to this ADR before the F5 transport ships.
