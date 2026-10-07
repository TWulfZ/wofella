# Session labelling and label progress

Branch `feat/session-labelling` from `feat/label-player` @ round-5 commit (stacked on PR #19) · opened 2026-10-07

## Objective
While wolluf is open, every map the self player finishes in osu! stable joins the session's pending list; the player labels each map's dominant pattern from that list, sees their labelling progress on a new Label → Progress page, and gets a hover summary from the Label screen's counters.

## Problem and why
- Pilot decisions (2026-10-06/07): the session label is the single **dominant pattern** of the map (its dense parts); it is a separate, weaker source than the blind gold set (it is chosen and played, not blind), so it never feeds the S2 gold set; the session counts plays since wolluf opened; notifications are opt-in in Settings and mean a queued entry plus a taskbar flash — no OS notifications (Linux/wine users); the Label nav entry becomes a dropdown with "Label" and "Progress"; the counters in the map card show a popover with progress, the session's pending maps and a link to Progress; Progress later shows contribution rankings with other users ("coming soon").
- Detection (research map, live-session): `Data/r` replays `<md5>-<FILETIME>.osr` are the reliable "play finished" signal (research 03 l.161); scores.db may flush minutes later (ADR 0014 l.13); quits write nothing. `source_osu::watch` and `app::watch::InstallWatcher` exist but are not started by the desktop. Only self aliases count (ADR 0005); before the self profile exists, nothing is pending.

## Scope
- Authorized: `crates/store` (a new feedback kind writer/reader, no schema change if the `feedback_event` table suffices), `crates/app` (session feature: plays since app start for the self scope, session labels, progress stats; starting the install watcher), `crates/source-osu` (read-only use of the watcher/replay names), `apps/desktop/src-tauri` (watcher lifecycle, appended commands and events, the Rust-side window attention call), `bindings.ts`, `apps/desktop/ui/src/**` (Label dropdown, Progress route, pending list, counters popover, notification setting), `docs/adr/0020-*.md`, architecture §5.3 note via the ADR.
- Out of scope: tosu/memory reading, OS notifications, crowd ranking backend (shown as coming soon), using session labels in evaluation or skill.

## Constraints
- A new persisted event kind (ADR 0001 list) → ADR 0020; `gold_labels()` and every existing reader must ignore it (strict decoders must not break on it).
- ADR 0005: only plays whose alias is a self alias; other players' replays in `Data/r` never appear.
- D9: source-osu read-only; watcher in source-osu only (`notify` allowed there only).
- Watcher started once per selected install at app start, stopped on context close; poll mode on `/mnt/<drive>/` paths.
- Taxonomy ids only (structured picker), single pattern or "no clear pattern".
- ADR 0009 appended commands/events, one bindings owner per stage. UI tokens, motion-safe, a11y, i18n en+es.
- TDD strict.

## Acceptance criteria
- A new self replay in `Data/r` while the app runs makes its map appear in the session's pending list (event to the UI); another player's replay does not → app tests with the testkit install + watcher/sync tests.
- `session_label_submit` stores a dominant-pattern label of the new kind; gold-set readers and stats ignore it; undo works → store/app tests.
- Progress page: totals and per-pattern/per-axis counts of gold labels, session labels, recent activity, the session pending list with inline labelling, and a "coming soon" ranking card → UI tests.
- Label nav dropdown (Label, Progress); counters popover with progress + pending count + link → UI tests.
- Settings toggle "Notify when a song ends" (off by default): on → taskbar flash on a new pending map; off → no flash, the list still fills → tests.

## Tasks
- [x] T1 — Backend: watcher lifecycle, session plays since start (self only), `play_label` kind + ADR 0020, commands/events, progress stats, notify setting + attention request. Route: delegated. Tier: high (persisted encoding, identity scope). Commit: feat(app): track the session's plays and store their dominant pattern
- [x] T2 — UI: Label dropdown, Progress route, pending list with structured labelling, counters popover, Settings toggle. Route: delegated (after T1). Tier: medium. Commit: feat(ui): label the session's maps from a progress page
- [ ] T3 — Close: three-lens verification + fix, gates, Windows build. Route: workflow. Commit: —

## Progress
- 2026-10-07 T1 (workflow wf_35541f64-8ee, writer W3 + fix round): `play_label` kind (ADR 0020; strict decoder; latest per chart wins; per-kind undo), session service (start time, watcher on the newest install re-pointed on setup, tracker diffing self plays after `DataChanged`, `SessionPlayAdded` event, `AttentionRequested` → Rust-side `request_user_attention(Informational)` when `session.notify_on_play` is on), commands `session_plays`, `session_label_submit`, `session_label_undo`, `label_progress`, `settings_get/set_session_notify`. Review fixes: D12 (each feature owns its reads), start/stop epoch so a late watcher/tracker is dropped, progress days count each chart once. Parent: architecture §5.3/§8 updated. Gates: fmt, clippy (+all-features), check-layers, stage-lock, lint-canary, deny ok; nextest 1044 passed / 19 skipped; bindings stable.
- 2026-10-07 T2 (workflow wf_41ab5ef0-65c): routes `label.index` + `label.progress` (`/label` keeps working; `?chart=` opens a played chart through `label_window_at`), NavMenu dropdown Label/Progress with a pending badge, labelProgress feature (stat cards, RICE/LN axis and pattern bars, 30-day activity, recent labels, session list with single-pattern picker, hold-to-save, undo, open in Label screen, coming-soon ranking), live invalidation on `session-play-added`, counters popover in the map card, Settings notify switch. Three lenses → 13 findings, all fixed: list grouped by map, flash only for a new pending map (ADR 0020 updated), picker label-in-name, popover keyboard peek, plurals and Spanish copy, switch contrast, clock tick for relative times, dead keys/exports. Gates: all green (nextest 1045 / 19 skipped, UI 67 files / 938 tests, bindings unchanged).

## Next step
Pilot checks the Windows build of the PR stacked on #19 (play a few maps with wolluf open, with and without the notify switch); then `git rm` this document.
