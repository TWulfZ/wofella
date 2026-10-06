# wolluf × mania-tracker: integration contract (DRAFT)

Status: Draft for discussion between TWulfZ (wolluf) and aleju03 (mania-tracker) · 2026-10-05

wolluf is a local desktop companion. It reads the player's osu! stable files and replays, models per-pattern skill and recommends practice. mania-tracker is an online tracker with a large chart corpus (~200k charts), osu! login and a chart preview. The goal is to share three things: **one pattern vocabulary**, **one crowd-label dataset** and **one chart-feature exchange**. With those in place, each tool can recommend charts the other knows about.

## 1. Shared vocabulary
- 7K pattern ids and axes are in ADR 0017 (`docs/adr/0017-7k-pattern-vocabulary.md`): 26 dotted ids under 8 axes (delay added 2026-10-05) (`7k.regular.{jack,tech,speed,stream}`, `7k.ln.{general,tech,inverse,release}`).
- Ids are stable strings and never renumbered. A meaning change creates a new id.
- 4K: a parallel vocabulary will be agreed together, starting from mania-tracker's 4K speed/tech split and Etterna skillsets. Open.

## 2. Label format (section level)
One JSON object per label. This is the same shape `wolluf label export` writes today:
```json
{"md5":"<chart md5>","t0_us":29356000,"t1_us":33356000,"cols":[1,2,4],
 "patterns":["regular.stream.bracket"],"no_pattern":false,
 "flags":["unsure"],"thumb_pref":null,"labelled_at":"2026-10-05T18:00:00Z",
 "labeller":"<opaque id>","queue":"gold|correct","v":1}
```
- `cols` are 1-based. Times are chart time in µs, at rate 1.0.
- `no_pattern: true` means the labeller saw no clear pattern. It requires `patterns: []`.
- `thumb_pref` is optional (`left`/`right`). It records which thumb side the section favours in 7K.
- `labeller` is an opaque per-project id, never an osu! username in exported datasets.

## 3. Labelling protocol (crowd)
- **Two queues:**
  - **Gold**: the engine's suggestion is never shown; used to measure the engine.
  - **Correct**: the engine's segments are shown and the labeller confirms or fixes them; used to improve it.
- **Consensus:** every item gets at least 3 independent labellers. A label is accepted when a weighted majority agrees.
- **Reliability:** each labeller gets a weight from agreement with consensus and accuracy on hidden **control items** (seeded from the pilot's gold set), using a Dawid–Skene-style estimate or a plain agreement rate.
- **Anti-bot and anti-farm:**
  - osu! OAuth login, one account per person;
  - a minimum account standing (7K playcount or rank, to agree);
  - a per-hour rate limit and a minimum dwell time per item;
  - option order randomized;
  - outlier detection on labellers.
- **Ranking:** points = accepted labels × reliability. Volume alone never ranks.
- **Sampling:** items are stratified by difficulty level and density. Sections where the engine and name hints disagree, or where the engine is uncertain, come first.

## 4. Chart-feature exchange
- wolluf's engine (Rust, MIT) can run natively for batch jobs or compile to WASM for mania-tracker's TS backend.
- Per chart it produces, versioned by stage `VERSION` and version key:
  - **segments**: `t0`, `t1`, `cols`, primary pattern, axis, secondary patterns, purity, strength;
  - **difficulty per axis and section** (wolluf F1 deliverable 3, in progress);
  - **weak name hints** (pattern or axis from pack and difficulty names).
- Heavy work (re-running the engine over ~200k charts, fitting calibrators) runs as offline batches, not on the web server. The server stores and serves the results; they are small.

## 5. Recommendations
- **wolluf "in your library"**: computed locally from the player's replays and sessions. Nothing leaves the machine.
- **wolluf "to download"**: wolluf asks mania-tracker for candidate charts by `{keymode, axis or pattern, difficulty band}`, then ranks them locally with the player's skill model. The request carries no identity. The response holds chart ids, beatmapset ids and features.
- **mania-tracker "for you"**: computed online from the user's tracked scores. If the user opts in from wolluf, it can also use an aggregated skill vector (one value ± uncertainty per axis; no replays, no per-play data).

API sketch:
- `GET /api/v1/charts/candidates?keymode=7&axis=7k.regular.jack&band=9.0-9.6&limit=50`
- `GET /api/v1/charts/{md5}/features`
- `POST /api/v1/labels` (authenticated)
- `GET /api/v1/labels/export?since=…` (accepted labels only)

## 6. Licensing and privacy
- **Code:** wolluf is MIT. mania-tracker code shared with wolluf (the chart preview, and anything else ported) needs an explicit MIT license in the mania-tracker repo. Today only `algorithms/` is MIT.
- **Labels dataset:** CC-BY 4.0 (or CC0), attributed to the contributor community, so both projects and third parties can reuse it.
- **Charts and audio** stay where they already are. Labels and features refer to charts by md5 and beatmap ids only; neither project redistributes map files.
- **Privacy:**
  - Sharing a wolluf skill vector is opt-in, can be withdrawn and is deletable (wolluf ADR 0007 telemetry rules).
  - Ranking display names are opt-in.
- **osu! API:** each project respects its own rate limits and the API terms. wolluf never harvests the osu! API.

## 7. Open questions
1. 4K vocabulary and how it maps to mania-tracker's existing 4K tags.
2. Where control items come from beyond the pilot's gold set, and how many per session.
3. The minimum account standing for labellers.
4. Hosting of the labelling page (inside mania-tracker is the proposal) and moderation roles.
5. Versioning when wolluf's engine output changes: does mania-tracker recompute everything, or keep the old version keys side by side?
6. Attribution and branding in both apps.
