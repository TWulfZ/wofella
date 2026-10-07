# 0020 Session dominant-pattern labels

- Status: Accepted
- Date: 2026-10-07

## Context
The pilot wants to label maps right after playing them, while the feel is fresh: wolluf stays open, every map the self player finishes in osu! stable joins a pending list, and the player names the map's **dominant pattern** (the pattern of its dense parts), or says there is none. A Label → Progress page shows how much has been labelled.

This answer is a different kind of evidence from the gold set:
- The gold set (`segment_label`, ADR 0017, ADR 0018) is blind: the sampler picks a window, the labeller sees nothing about the engine or the map's reputation, and the set stays an unbiased test set for per-pattern precision (architecture §6, §12).
- A session answer is about a map the player chose and played. It covers the whole map, not an anchored window, and it is one pattern, not a set. It is weaker and biased by selection, so it must never enter the gold set, its stats, its export or the sampler's exclusions.

Architecture §5.3 says a payload shape change bumps `v` or adds a new kind, never rewrites rows, and readers reject what they do not know.

Detection uses what already exists. `Data/r/<md5>-<FILETIME>.osr` is the reliable "play finished" signal; scores.db may flush minutes later, and a quit writes nothing (research 03; ADR 0014). `source_osu::watch` and `app::watch::InstallWatcher` already turn those writes into `SyncPlays` submits, but no shell started them.

## Decision
- **A new persisted kind, `play_label`**, in `feedback_event` (no schema change):
  - subject `{"chart_md5", "keymode"}`: the chart, since the answer is about the map;
  - payload `{"v": 1, "action": "assert_dominant" | "assert_none", "origin": "player_session", "pattern"?: PatternId}`, where `assert_dominant` needs `pattern` and `assert_none` (no clear pattern) must not carry one. Its `v` is versioned apart from `segment_label`;
  - context adds `play_id` (hex `PlayId` or `null`) to the usual `{app_version, manifest_hash, pack_id, scope_hash}`: which play the answer followed, kept as provenance, not as the subject.

  Pattern ids are the keymode's taxonomy ids only (ADR 0017), picked from a structured list. `wolluf_store::repo::labels::play_labels` decodes strictly and rejects an unknown `v`, origin or action, a missing or extra `pattern`, and unknown payload fields.
- **Latest wins per chart.** Several answers for one chart may exist; the effective one is the latest that no undo cancels. Progress counts charts, so relabelling a map adds no contribution; its daily series counts each chart once, on the day of its effective answer, so the days add up to the total.
- **Undo stays per kind.** `undo` keeps its shape (subject `{"event_id"}`, payload `{"undone_kind"}`). `append_undo` takes back only a `segment_label` and `append_play_label_undo` only a `play_label`, each for the same profile and once. The gold screen cannot cancel a session answer, nor the reverse.
- **Session = plays since the app opened.** The context records its start time. `session_plays(keymode)` lists the self scope's plays (self aliases only, ADR 0005) with `played_at_utc` at or after it, newest first, joined with the catalog and each chart's effective answer. Before a self profile exists, nothing is pending.
- **Live updates.** The desktop starts `SessionService::start` once the context opens: it watches the selected install (the newest registered one; re-pointed when setup registers another) and runs a tracker. After each `DataChanged{plays|players}` the tracker diffs the session's self plays against the ones it already announced and emits `SessionPlayAdded {playId, md5}` per new play. When the opt-in setting `session.notify_on_play` is on, it also emits `AttentionRequested`, but only for a new pending map: a chart not already in the session, with no effective answer, listed in the catalog and in a keymode with a taxonomy (a retry, another keymode or a map osu!.db does not list yet never flashes). The desktop maps `AttentionRequested` to `request_user_attention(Informational)` on the main window: a taskbar flash, not an OS notification. Closing the context stops the watcher and the tracker.

## Alternatives considered
- **Reuse `segment_label` with a whole-chart anchor and `origin: "player_session"`.** Rejected: `gold_labels()` reads every `segment_label`, so every gold reader (stats, export, sampler exclusions, the overlap check) would need an origin filter, and one missed filter silently mixes chosen, non-blind answers into the test set. A whole-chart anchor would also block every window of that chart for the blind sampler, and `patterns` is a set where the session answer is exactly one pattern. A separate kind makes the separation structural: gold readers never see it, with no filter to forget.
- **The play as subject.** Rejected: the answer is about the map, and the same map played twice should not need two answers. The play id stays in the context.
- **Session from scores.db only.** Rejected: scores.db may flush minutes late and the replay is the reliable signal; `SyncPlays` already imports orphan replays as `replay_only` plays, so both sources feed the same ledger query.
- **OS notifications.** Rejected by the pilot (Linux and wine users); a window flash needs no extra plugin.

## Consequences
- `gold_labels()`, `label_stats`, the gold export and the sampler's exclusions are unchanged by construction; tests store a session label and assert they return the same values.
- Session answers are not used by evaluation, skill or the engine. Using them later (for example as weak supervision) needs its own decision.
- Architecture §5.3 lists `play_label` next to `segment_label` and `undo` through this ADR.
- The commands `session_plays`, `session_label_submit`, `session_label_undo`, `label_progress`, `settings_get_session_notify`, `settings_set_session_notify` and the event `session-play-added` are appended to the IPC lists (ADR 0009).
- The flash is a Rust-side window call, so the webview capabilities need no new permission.
