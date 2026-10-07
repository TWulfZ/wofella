# 0021 Gold label selection origin

- Status: Accepted
- Date: 2026-10-07

## Context
The gold set (`segment_label`, ADR 0017, ADR 0018) exists to measure the pattern engine without bias: the sampler picks a chart and a window, and the labeller answers blind (architecture §6, §12). The Label screen now lets the labeller choose both:
- **Now playing** opens the chart osu! stable is playing, or the newest self replay (ADR 0018, amendment 2026-10-06);
- **session maps** open a chart the player just played, from the session list (ADR 0020);
- **timeline moves and resizes** (and the CLI's `w+`/`w-`/`n`/`p`) put the window where the labeller wants it.

A window chosen this way is still a valid answer, but it is selected by the labeller: a map they like, a section they noticed. Mixed into the test set unmarked, it biases per-pattern precision towards what the labeller finds memorable. The `segment_label` v1 payload does not record how a window was chosen, so these answers cannot be told apart from blind ones. Architecture §5.3 forbids rewriting stored rows: a shape change bumps `v`.

## Decision
- **Payload v2.** Writers write only `{"v": 2, "action", "origin": "gold", "patterns", "flags", "selection": {"pick", "window"}}`:
  - `pick`: how the chart was reached. `sampled` (a stratified sampler round), `random` (any eligible chart, outside the stratified plan), `now_playing` (the chart osu! is playing or the newest self replay), `session` (a map opened from the session list).
  - `window`: `sampled` when the stored window is exactly the one the pick offered; `moved` once the labeller moved, resized, widened, narrowed or shifted it, even if it ends on the offered bounds again.
- **v1 stays readable as unknown.** `gold_labels()` decodes v1 rows (no `selection`) as `GoldSelection::Unknown` and v2 rows as `Recorded`. The decoder is strict for both: unknown payload, flag or selection fields, a v1 row with a `selection`, a v2 row without one, `null`, an unknown `pick` or `window`, or an `origin` other than `gold` are `InvalidData`. No row is rewritten.
- **Blind** means `pick` ∈ {`sampled`, `random`} and `window` = `sampled`. Unknown is never blind.
- **Declared by the client.** `LabelSubmitDto.selection` is required. The service can check only one part of it: `pick = session` needs a chart the self profile played (ADR 0005), else `INVALID_INPUT` with `pick`. Whether a `sampled` or `random` window really came from that command, unmoved, cannot be verified from the anchor alone; the clients (Label screen, `wolluf label`) are trusted to declare it.
- **Reporting.** `label_stats` and `label_progress` keep counting every gold label and add `blind`/`goldBlind` and `perSelection` (unknown first, then by pick and window). The gold export writes `"selection": {"pick", "window"}` on every row, with `"unknown"` for both on v1 rows. Evaluation (per-pattern precision, F1 deliverable 4) uses blind labels by default and reports the rest, unknown included, separately.

## Alternatives considered
- **Block gold labelling of chosen charts and windows** (Now playing, session maps, moved windows store nothing, or only session answers). Rejected by the pilot: those answers are good labels, the screen would lose most of its use while playing, and the separation is achieved by recording the origin instead.
- **Ignore the origin.** Rejected: chosen windows would silently bias the test set, and nothing could undo it later because the rows cannot be rewritten.
- **A separate kind for chosen windows.** Rejected: unlike a session answer (ADR 0020), a chosen window is the same answer shape on the same anchor, and it must keep blocking overlapping windows in the sampler and the submit overlap check. A field on the one kind keeps every gold reader unchanged.
- **Infer the origin on the server** (track offered windows per session). Rejected for now: it needs server-side session state for every offered window, and a moved window would still need the client to say so.

## Consequences
- `SEGMENT_LABEL_V` is 2; the v1 reader stays forever. ADR 0018's "the same `segment_label` v1 events as the CLI" now reads v2.
- The CLI `wolluf label` always samples, so it writes `pick = sampled` with `window = sampled`, or `moved` after any successful `w+`/`w-`/`n`/`p` in that round. `wolluf label stats` prints `blind` and a `SELECTION` table.
- The pilot's labels stored before this ADR export and count as unknown; evaluating with them is an explicit choice.
- The mania-tracker label exchange (`docs/integration/mania-tracker.md` §2) carries `selection`, so crowd labels keep the same blind/chosen split. The field is additive to the draft exchange, whose `v` stays 1; `unknown` in both fields marks labels stored before this ADR, and blind has the meaning above.
- The desktop must send `selection` on every `label_submit`; the Label screen tracks the pick of the current chart and whether its window was moved.
