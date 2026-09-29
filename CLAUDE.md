# wolluf

Training companion for osu!mania **stable**. It diagnoses per-pattern weaknesses from replays, tracks skill per session, recommends chart + section + rate, and cuts practice drills. MVP is 7K (rice + LN); everything is keymode-generic (4K next). Stack: Tauri 2, Rust workspace, React 19 + Vite + TS.

## Sources of truth (read before non-trivial work)
- `docs/architecture.md`: layers, dependency rules D1–D17, storage, feedback loop, phases F0–F5. **Binding.** Change it only through an ADR.
- `docs/adr/`: accepted decisions (MADR).
- `docs/specs/<id>-<slug>/`: the spec and tasks for each feature or phase.
- `docs/research/`: verified domain findings (hit windows, formats, calculators, prior art, the pilot user's data). Grep these files before searching the web.
- `research/scripts/`: Python prototypes that act as oracles for porting (osu!.db / scores.db readers, replay re-judge harness).

## Workflow
Any feature, phase task or behavioural change goes through the **`wolluf-sdd` skill**: spec → tasks → TDD implementation → gates → commit. Only trivial fixes skip it.

## Environment
- Rust lives in `~/.cargo/bin`. Non-interactive shells need `export PATH="$HOME/.cargo/bin:$PATH"`.
- The pilot's osu! install is at `/mnt/e/Games/osu!` (`WOLLUF_CORPUS`). **Read-only, always.** Tests and tools never write there.
- WSL2 cannot memory-read osu!, so live E2E runs on a Windows build. F0–F3 work entirely from local files.

## Hard rules (details in architecture §4)
- Domain crates (below `engine`) do no IO: no fs, net, env or `SystemTime::now`. Time, params and seeds are passed in.
- Only `wolluf-store` contains SQL. Only `app::export` may write into the osu! folder, and only with an `ExportPermit`.
- No new crate dependency edge without an ADR (`cargo xtask check-layers` enforces this).
- Thresholds and weights live in param structs, never inline. Stable string ids (axes, patterns, error codes) are never renumbered.
- Every derived artifact carries its `VersionKey`. Bump the stage `VERSION` when outputs change (stage-lock CI).
- Skill is computed only for the selected identity scope. Other players' plays never feed a self profile or telemetry.
- No GPL/LGPL in-process. Sunny is reimplemented clean-room from the PDF. MIT ports (Interlude `prelude/`, mania-hub `algorithms/`, LeoBlack) are credited in NOTICE.
- Never commit real beatmaps, audio, replays or the user's DBs. Fixtures are synthetic or minimized and anonymized.

## Domain facts that are easy to get wrong
- The 7K Regular axes are **jack, tech, speed, stream**. Stamina is derived. The LN axes are general, tech, inverse, release.
- This stable build (osu!.db 20260924) saves **failed plays** to scores.db and `Data/r` (fails appear from 2026-04 on). The `Data/r` naming is `<beatmap md5>-<FILETIME>.osr`, and `.osg` is undocumented (F0 spike).
- The cfg `Username` can be garbage (`TWulfZasdasdasd d jSS||`), so identity uses the tiered heuristics in architecture §5.6.
- Replay time must accumulate **all** frames, including lead-in; osrparse is wrong here. Rate-mod windows are `floor(base × rate)` in map time. Under ScoreV2, LN heads and tails are judged separately. LN judging is approximate, so tag it with a confidence.
- `osu-db` on crates.io: 0.3.0 (2021) cannot read the current osu!.db. Prefer our own codec, validated against `research/scripts/*/osudb.py` and `sdb.py`.

## Commands
Fill these in as they come to exist:
- `cargo nextest run --workspace`
- `cargo xtask check-layers`
- `cargo xtask bindings`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `pnpm -C apps/desktop/ui test`
- Corpus harness: `WOLLUF_CORPUS=/mnt/e/Games/osu! cargo nextest run --run-ignored only`
