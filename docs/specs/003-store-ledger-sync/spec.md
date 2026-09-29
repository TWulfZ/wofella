# 003 Store, play ledger and SyncPlays

Status: Draft
Phase: F0 · Owner: twulfz · Date: 2026-09-28
Links: architecture §3, §4 (D2, D6, D7, D8, D9, D15), §5.2–§5.5, §7, §8 (events), §10 (store tests), §12 F0, §13 O9; ADR 0003 (storage split, from spec 001); ADR 0006 (`PlayId` encoding, from spec 001); ADR 0014 (proposed here, T1; number assigned at the F0 review); research `03-maniahub-rejudge-drills-sessions-audit.txt` (L115, L130–131, L161, L168–169, L307, L327–344), `00-plan-es.md` (L53–54, L70), `research/scripts/audit/osudb.py` + `sdb.py` (oracles), `research/scripts/rejudge/legacy_db.md` (scores.db layout)

## Problem
Nothing in wolluf works until the pilot's plays are in a durable ledger that wolluf owns. osu! stable can drop replays from `Data/r` and change or delete maps in `Songs`, and a decoder bug must never lose data. This spec turns the read-only osu! install into three wolluf-owned stores in the app data dir:
- `user.db`, the ledger of irreplaceable facts;
- `cache.db`, disposable derived data;
- a content-addressed vault holding the original `.osr`, `.osg` and `.osu` bytes.

It also adds the `SyncPlays` job that fills them, runs again on its own when osu! writes a new play, and never ingests a play twice. The identity work (004) and every later phase read from these stores.

## Scope
- In:
  - `wolluf-store`:
    - connection management (WAL, one writer thread per DB, a small read pool, a single-instance lock);
    - user.db migration `0001` covering the F0 subset of tables, with forward-only migrations, a `VACUUM INTO` backup before each migration, and refusal to open a newer schema;
    - the cache.db v1 skeleton with delete-and-rebuild on a `CACHE_SCHEMA_VERSION` mismatch or a corrupt file;
    - the content-addressed vault;
    - repositories for the ledger tables (`game_install`, `source_snapshot`, `blob`, `alias`, `play`, `feedback_event` append, `settings`, `meta`, `install`) and for the cache tables (`derivation`, `catalog_chart`, `job_run`, `item_failure`).
  - `wolluf-source-osu`, IO modules only:
    - `snapshot`: a stable in-memory read of a DB file plus sha256, size and mtime;
    - `replay_dir`: a `Data/r` name index;
    - `songs`: reading a chart and verifying its md5;
    - `watch`: a debounced notify watcher.
  - `wolluf-core`: nothing new. The `PlayId` natural key, `DotNetTicks`/`FileTime` and `BlobSha256` are 001's (single owner, ADR 0006); this spec consumes them.
  - `wolluf-app`:
    - `context::AppPaths` (data-dir resolution with an explicit override for the CLI, logs dir, guard), the context open and install registration;
    - the `jobs` runner (tokio orchestration, rayon CPU, `CancellationToken`, ≤ 10 Hz progress, `catch_unwind` per item, coalescing, `item_failure`);
    - `features::plays::sync` (the `SyncPlays` job: catalog, ingest, archive);
    - wiring the watcher to job submission;
    - job and event DTOs.
  - Tests: unit, store and app tests on synthetic fixtures, plus one `#[ignore]` corpus test.
- Out (non-goals):
  - The codecs for osu!.db, scores.db and the `.osr` header (spec 002). This spec consumes them.
  - Alias stats computation, identity heuristics, and the profile, profile_alias and identity_decision repositories (spec 004). The tables are created here.
  - Install detection (spec 002), and Tauri commands, the CLI `wolluf sync` subcommand and `setup_*` (spec 005).
  - `.osg` decoding (spec 006). Only its bytes are archived here.
  - Turning orphan `Data/r` replays (no scores.db row) into plays. Their bytes are archived but no play row is created (see Risks).
  - Pass/fail classification. It needs chart object counts or the replay life graph, so it is a derived stage (F2).
  - The vault "self-only" setting, vault compression and vault GC (O9, revisit with the sizes measured below).
  - The collection.db and cfg snapshots (spec 004 reads the cfg; F4 writes collections).
  - The interactive/bulk pool split (F3), and any user.db tables outside the F0 subset (`linked_account`, `rec_impression`, `report_impression`, `drill`, `consent`, `param_pack`). Later migrations add them.

## Behaviour
- **First run.**
  - `AppContext::open(paths, clock)` creates `<data>/user.db`, `<data>/cache.db`, `<data>/vault/`, `<data>/backups/` and `<data>/wolluf.lock`.
  - It applies migrations and inserts the single `install` row (UUIDv4 plus a 32-byte random secret).
  - A second process opening the same data dir gets `AppError{code: CONFLICT}` (message key `error.instance_running`) and touches nothing. `AppError` is 005 T3's; this spec only adds its `From<StoreError>` mapping and message keys.
- **Data dir.**
  - `AppPaths::resolve(override_dir: Option<PathBuf>)`. Resolution order:
    1. the explicit override (the CLI's `--data-dir`, spec 005);
    2. `WOLLUF_DATA_DIR`, read only in `app::context`;
    3. otherwise `directories::ProjectDirs::from("", "", "wolluf").data_local_dir()`. On Windows that is `%LOCALAPPDATA%\wolluf\data`; on Linux `$XDG_DATA_HOME/wolluf`, which defaults to `~/.local/share/wolluf`.
  - `AppPaths` also exposes `logs_dir = <data>/logs` (005's logging writes there).
  - If the canonicalised data dir equals or lies under any `game_install.root_path`, the result is `INVALID_INPUT` (`error.data_dir_inside_osu`). The same check runs in `AppContext::register_install(root, client_version) -> InstallId`, which 005's setup service calls; it upserts `game_install` on `(game, root_path)`.
- **SyncPlays(install_id)**, submitted by the caller or by the watcher. It is one idempotent pass; "incremental" means the pass finds little new work, not a separate code path. The steps run in order:
  1. **Catalog.**
     - If `osu!.db` (size, mtime) equals the last `osu_db` snapshot, the step is skipped.
     - Otherwise the file is read into memory (stable-read rule below) and hashed. An unchanged sha256 reuses the snapshot row.
     - A new sha256 inserts a `source_snapshot` row and then, in one cache.db transaction, replaces all `catalog_chart` rows with the mania (`mode = 3`) entries. `keymode = round(CS)`. A `derivation(stage='catalog', input_key=<osu_db sha hex>)` row is written.
  2. **Ingest.**
     - `scores.db` is always read (≈ 650 KB) and hashed. A new sha256 inserts a `source_snapshot` row; an unchanged one skips to step 3.
     - Records with `mode ≠ 3` are counted as `skipped_non_mania` and skipped.
     - For every other record, the alias `(game='osu_stable', raw_name bytes)` is upserted, the ticks are converted with `DotNetTicks::to_filetime`, `play.id = PlayId::derive(Game::OsuStable, md5, raw_name, filetime)` is computed (001), and the play goes in with `INSERT OR IGNORE`, in write batches of ≤ 2,000 rows. `play.filetime` is the `FileTime` decimal text; `online_score_id` is `OnlineId::positive()` as decimal text, NULL when the id is ≤ 0 (002).
     - If a key already exists with identical ledger fields (`mods, score, max_combo, counts_json, client_version`), nothing happens. If it exists with *different* fields, the stored row wins and an `item_failure(code=CONFLICT, item_ref=<play id hex>)` is recorded.
  3. **Archive.**
     - `Data/r` is listed once. Names accepted by 002's `ReplayFileName::parse` form an index keyed by (md5, filetime); other names are ignored.
     - For each play with `replay_sha IS NULL`, or with `osg_sha IS NULL` and an `.osg` present, the file is read, its `.osr` header is checked (header beatmap md5 == `chart_md5` and header ticks == the play's ticks), the file goes into the vault, a `blob` row is inserted, and the play is updated through `SET replay_sha = ? WHERE id = ? AND replay_sha IS NULL`. The same applies to `osg_sha`.
     - A header mismatch records `item_failure(CONFLICT)` and leaves the play unlinked.
     - `.osr` files with no play (orphans) are archived as blobs only and counted as `orphan_replays`.
     - For each distinct `chart_md5` among plays with `chart_sha IS NULL`:
       - If it is in `catalog_chart`, `Songs/<path>` is read and its md5 is checked. On a match the bytes go into the vault and every such play gets `chart_sha` set.
       - On a mismatch (the map was edited in place) it is counted as `chart_md5_mismatch`, and a `derivation(stage='chart_archive', input_key=<md5>:<osu_db sha>, status='skipped', error_code='CONFLICT')` row is written. That chart is not reread until the osu!.db snapshot changes.
       - If it is not in the catalog, it is counted as `chart_unavailable`.
  4. **Finish.**
     - `job_run.summary_json` gets these counters: `plays_new, plays_existing, conflicts, skipped_non_mania, replays_linked, osg_linked, charts_archived, chart_unavailable, chart_md5_mismatch, orphan_replays, failed_items`.
     - The job emits `JobFinished` and `DataChanged{domains:["plays"]}`, but only if something changed. `players` is emitted by 004's chained `RefreshIdentity` once alias stats are fresh, so the wizard never reloads on stale stats.
- **Stable read (snapshot).**
  - The file is stat'd, read fully, and stat'd again. If size or mtime changed during the read, the read is retried with backoff (250 ms, 500 ms, 1 s). After 3 changed reads the step fails with `OSU_RUNNING` (retryable).
  - A codec truncation or trailing-bytes error on a freshly written file gets one extra retry after 2 s before it becomes `PARSE_FAILED`.
  - An unknown format version fails at once with `UNSUPPORTED_FORMAT` and no retry (§7).
- **Watcher.**
  - `source_osu::watch` watches `<root>/Data/r` (non-recursive) and `<root>` (non-recursive, filtered to `scores.db`), debounced 5 s. Each debounced batch submits `SyncPlays(install_id)`.
  - Native mode is used on Windows. Poll mode (5 s interval) is used when the root is on a WSL drvfs mount (`/mnt/<letter>/…`), where inotify does not see writes made by Windows processes.
- **Job runner.**
  - `JobService::start(kind, params)` returns a `JobId` (ULID) immediately.
  - In F0 jobs run one at a time, FIFO.
  - A finished job may return follow-up job requests in its `JobSummary`; the runner enqueues them (through the same coalescing) after emitting `JobFinished`. 004 uses this to chain `RefreshIdentity` after every `SyncPlays`.
  - Coalescing uses `dedupe_key = "sync_plays:<install_id>"`:
    - a submit while the same key is *queued* returns the queued `JobId`;
    - a submit while it is *running* sets one trailing re-run flag (repeated submits do not stack), so an event that arrives mid-sync is never lost.
  - `cancel(id)` trips the job's `CancellationToken`. Items are checked before each unit of work. Everything committed so far stays committed, the job ends `cancelled` with `CANCELLED`, and a later run completes the rest because every step is idempotent.
  - Progress events `JobProgress{job_id, kind, stage, done, total, eta_ms}` are throttled to ≥ 100 ms apart per job, and the last one is always delivered.
  - A panic in an item is caught (`catch_unwind`), recorded as `item_failure(code=INTERNAL, message=<panic payload>)`, and the job continues.
- **Edge cases.**
  - An exact duplicate record inside one scores.db collapses to one play. This happens in the corpus: records 3728 and 3729, md5 `e956977c…`, player `TWulfZ`.
  - `raw_name = ""` is a valid alias.
  - Non-UTF-8 name bytes are preserved as they are.
  - A missing `Data/r` directory counts as 0 replays, not as an error.
  - A missing `scores.db` returns `OSU_DIR_NOT_FOUND`.
  - Ticks below the FILETIME epoch make that record fail with `INVALID_INPUT` (item failure) while the job continues.
  - A cache.db that is corrupt or has a different `user_version` is deleted (together with its `-wal`/`-shm` files) and recreated at open. user.db is never deleted.

## Domain rules
- The osu! folder is read-only. Only `app::export` writes there (D9, CLAUDE.md). `source-osu` gets snapshots through `fs::read` into memory, never through a temp-file copy, which keeps D9's "no fs write calls" literal. This resolves the conflict with the §7 wording "snapshot-copied to temp" (ADR 0014).
- SQL exists only in `wolluf-store` (D6). Repositories take and return core or store types, never rusqlite rows. The app never sees `rusqlite`.
- The store is not behind traits. Tests use in-memory or temp SQLite with the real migrations (D8, §10).
- scores.db layout: per beatmap `md5, n`, then per score: mode u8, version i32, beatmap md5, player, replay md5, 6×u16 counts (300, 100, 50, geki = MAX, katu = 200, miss), score i32, max combo u16, perfect u8, mods i32, lifebar string (empty), ticks i64, i32 = −1, online id i64, and an f64 only if mods bit 23 is set (`legacy_db.md`; `osudb.py::read_scores_db`).
- Every score in the pilot scores.db has `db_md5 == beatmap_md5` (0 differ, measured 2026-09-28).
- The `Data/r` name is `<beatmap md5>-<FILETIME>.osr|.osg`. FILETIME = .NET ticks − 504 911 232 000 000 000, matching 4362/4362 replays (research 03 L161, L168).
- scores.db ticks are **UTC**. The last score (ticks → 2026-09-28 23:13:56) precedes the scores.db mtime of 2026-09-28 18:18:56 −05:00 = 23:18:56 UTC by 5 minutes (measured 2026-09-28; pin in ADR 0014). `played_at_utc` is therefore derived with no timezone conversion.
- Mods: ScoreV2 is bit 29, so `score_system = v2` when it is set and `v1` otherwise. Target Practice is bit 23 (`osudb.py::MODS`).
- Native accuracy, for display only (§5.1), uses the `osudb.py` oracle formulas:
  - V1: `(300·(MAX+300) + 200·200 + 100·100 + 50·50) / (300·total)`;
  - V2: `(305·MAX + 300·n300 + 200·n200 + 100·n100 + 50·n50) / (305·total)`;
  - NULL when total = 0.
- Failed plays *are* saved to scores.db and `Data/r` on client 20260924 (research 03 L115, L160). scores.db has no pass flag and its lifebar is empty, so `play.passed` stays NULL in F0 (ADR 0014; the column becomes nullable).
- Alias raw bytes are the key, and `''` is valid (§5.3). Name normalisation belongs to 004 and is used for matching only.
- The play ledger is immutable. Re-ingest is idempotent through the natural key (§5.3, §10 "same scores.db → 0 new rows"). The only permitted updates are NULL → value on `replay_sha`, `osg_sha` and `chart_sha`. DB triggers enforce this.
- The vault archives every play that has a replay, whoever played it (§5.2 vault policy), and chart bytes for every chart with a play.
- Corpus sizes (measured 2026-09-28, for O9):

  | Item | Count | Size |
  |---|---|---|
  | `.osr` files | 5,030 | 91 MB |
  | `.osg` files | 4,682 | 511 MB |
  | scores.db rows | 4,989, of which 4,970 mania (4,969 distinct natural keys) and 19 osu!std | |
  | mania rows with a `Data/r` .osr | 4,970 | |
  | mania rows with a `.osg` | 4,627 | |
  | orphan `.osr` (all mania, header timestamp == name) | 42 | |

  The "4.3k" in architecture §12 is the older 7K-only count, 4,338 (research 03 L307).
- Jobs: tokio orchestrates, rayon runs CPU work, CPU work never runs on the async runtime, there is one writer thread per DB, transactions hold ~500–5k rows, and each item runs under `catch_unwind` with its failure recorded (§7).
- Only stable string ids are persisted: job kinds, snapshot kinds, blob kinds, stage ids and error codes (§11).

## Design
- **Crates and edges.** Only edges already in §4 are used: `core → store`, `core → source-osu`, and `store, source-osu, core → app`. In F0 `store` does not depend on `engine`, which does not exist yet; the §4 edge `engine → store` is unused and allowed later. `source-osu` does not depend on `store` (D7). New external dependencies, pinned in `[workspace.dependencies]` by the orchestrator (T2), current on 2026-09-28:

  | Crate | Version | Used in | Notes |
  |---|---|---|---|
  | rusqlite | 0.40.2 | store | features `bundled` (same SQLite on WSL and Windows) |
  | rusqlite_migration | 2.6.0 | store | requires rusqlite ^0.40 |
  | sha2 | 0.11.0 | store, source-osu | |
  | crossbeam-channel | 0.5.17 | store | writer queue |
  | uuid | 1.26.1 | store | `v4` |
  | ulid | 3.0.0 | store, app | |
  | serde / serde_json | 1.0.229 / 1.0.151 | store, app | |
  | md-5 | 0.11.0 | source-osu | chart md5 verification |
  | notify | 8.2.0 | source-osu | |
  | notify-debouncer-full | 0.7.0 | source-osu | depends on notify ^8.2 |
  | tokio | 1.53.1 | app | `rt-multi-thread`, `sync`, `time`, `macros` |
  | tokio-util | 0.7.19 | app | `CancellationToken` |
  | rayon | 1.12.0 | app | |
  | directories | 6.0.0 | app | |
  | thiserror | 2.0.21 | all libs | |
  | tempfile | 3.27.0 | dev-dependency | |
  | blake3 | 1.8.7 | core | already present from 001 for `VersionKey` |

  All of these are pinned by 001 T1; this spec only references them. specta follows spec 005's pin (2.0.0-rc.25 today). The single-instance lock uses `std::fs::File::try_lock` (stable since Rust 1.89, toolchain 1.98.1), so no lock crate is added. check-layers allows `rusqlite*` only in store, `notify*` only in source-osu, and `tokio*`/`rayon` only in app and the shells (001's L5 `[restricted_deps]` table).
- **core.** Nothing new: `PlayId::derive(Game, ChartMd5, &[u8], FileTime)`, `DotNetTicks::to_filetime`, `FileTime` and `BlobSha256` come from 001 (encoding frozen in ADR 0006 with the pilot golden vector). Uppercase or malformed md5 text is rejected by `ChartMd5` parsing before a key is derived.
- **store** (`crates/store/src/`):
  - `db.rs`, `DbHandle { writer: Writer, readers: ReadPool }`:
    - `Writer` owns one `Connection` on a dedicated thread fed by a `crossbeam_channel::bounded(256)` of boxed `FnOnce(&mut Transaction) -> R` jobs. `Writer::call(f)` blocks the caller and returns `R`; the app wraps it in `spawn_blocking`.
    - `ReadPool` holds N = 4 read-only connections behind `Mutex`es, acquired round-robin with `try_lock` and falling back to a blocking lock.
    - Pragmas: `journal_mode=WAL`, `foreign_keys=ON`, `busy_timeout=5000`. `synchronous` is `FULL` on user.db (irreplaceable) and `NORMAL` on cache.db.
    - No tokio in store.
  - `lock.rs`: `InstanceLock` holds an exclusive `try_lock` on `<data>/wolluf.lock` for the process lifetime.
  - `user/migrations/0001_init.sql` and `user/mod.rs`:
    - `open_user_db(path, backups_dir, now)` reads `PRAGMA user_version`.
    - If the stored version is greater than the latest known, it fails with `StoreError::SchemaTooNew` → `UNSUPPORTED_FORMAT`.
    - If the version is > 0 and older than latest, it runs `VACUUM INTO '<backups>/user-v<from>-<yyyymmddThhmmssZ>.db'` first, then `Migrations::to_latest`. The 5 newest backups are kept.
    - The migration list is `const USER_MIGRATIONS` and is validated by `Migrations::validate()` in a test.
  - `cache/schema_v1.sql` and `cache/mod.rs`: `pub const CACHE_SCHEMA_VERSION: u32 = 1`, stored in `PRAGMA user_version`. On a mismatch, or on `SQLITE_CORRUPT`/`SQLITE_NOTADB`, the file set is deleted and recreated. There are no migrations, ever.
  - `vault.rs`, `Vault { root }`:
    - `put(bytes) -> BlobSha256` writes to `vault/tmp/<ulid>`, fsyncs, renames to `vault/blobs/<h[0..2]>/<h[2..4]>/<h>` and fsyncs the directory on unix. If the final file exists with the same size it is a no-op.
    - `get(sha: BlobSha256) -> Vec<u8>` re-hashes and returns `StoreError::VaultCorrupt` on a mismatch.
    - `path(sha)` is exposed for diagnostics.
    - The file is made durable before its `blob` row is inserted. A crash in between leaves an unreferenced file, which is harmless and gets no GC in F0.
  - `repo/ledger.rs`: `game_install` (insert, get, list), `source_snapshot` (latest by kind, insert), `alias` (upsert → id), `play` (`insert_batch(&[NewPlay]) -> InsertOutcome{new, existing, conflicts: Vec<PlayId>}`, `unlinked_replays()`, `set_replay_sha/osg_sha/chart_sha` with NULL guards, `count()`), `blob` (insert-or-ignore), `feedback_event::append`, `settings` get/set, `meta`, `install`.
  - `repo/cache.rs`: `catalog_chart::replace_all(snapshot_id, rows)`, `catalog_chart::get(md5)`, `derivation::{get, put}`, `job_run::{insert, finish}`, `item_failure::insert`.
  - Profile, identity and alias_stats repositories are added by 004 into `repo/players.rs`; 004 also adds the `alias_stats` table to cache schema v1 (unreleased until F0 closes, so no version bump).
  - Errors: a `StoreError` thiserror enum.
- **source-osu** (IO modules beside 002's pure codecs):
  - `snapshot::read_stable(path, policy) -> Snapshot{bytes, sha256, size, mtime}`;
  - `replay_dir::index(root) -> BTreeMap<(ChartMd5, FileTime), ReplayFiles{osr: Option<PathBuf>, osg: Option<PathBuf>}>` (`BTreeMap` for deterministic order, D3 spirit); names go through 002's `ReplayFileName::parse`, never a second regex;
  - `songs::read_chart_verified(root, rel_path, expected_md5) -> Result<Vec<u8>, ChartReadError{Missing | Md5Mismatch | Io}>`;
  - `watch::spawn(root, mode: WatchMode{Native, Poll{interval}}, debounce) -> (WatchHandle, Receiver<SourceChange{ScoresDb | ReplayDir}>)`.
  - None of these writes to the filesystem, which D9's banned-API grep enforces. Their tests create files, so they live in `crates/source-osu/tests/*.rs` (L6 also scans `#[cfg(test)]` code in `src/`).
- **app**:
  - `context.rs`: `AppPaths::resolve(override_dir)` and `AppContext::open(paths, clock)`, which acquires the lock and opens the DBs, the vault, the runtime handle, the rayon pool (`num_threads = max(1, cores − 1)`) and the event bus (`tokio::sync::broadcast::Sender<AppEvent>`, capacity 256, `subscribe()` public for 005's bridge and CLI), and `register_install`.
  - `jobs/`:
    - `Job` trait: `kind() -> &'static str`, `dedupe_key() -> String`, and `run(self: Box<Self>, ctx: JobCtx) -> Pin<Box<dyn Future<Output = Result<JobSummary, AppError>> + Send>>`.
    - `JobCtx { cancel, progress: ProgressSink, user: DbHandle, cache: DbHandle, vault, cpu: Arc<ThreadPool> }`.
    - `JobRunner` (queue, coalescing, re-run flag, follow-up enqueue).
    - `run_items(items, f)`, which fans items out to rayon with a per-item cancellation check and `catch_unwind(AssertUnwindSafe(..))`, and collects `ItemResult`.
    - `ProgressSink` is throttled with the injected `core::Clock`.
  - `events.rs`: `AppEvent::{JobProgress, JobFinished, DataChanged}`. 005's shell bridges them to Tauri.
  - `features/plays/{mod.rs, sync.rs, dto.rs}`: `SyncPlaysJob`, `PlaysService::sync(install_id) -> JobId`, and a `sync_blocking` helper for the CLI.
  - `watch.rs`: maps `SourceChange` to `JobService::start(SyncPlays)`.
- **Versioned stages.** Only the `catalog` stage, with `const VERSION: u32 = 1`, and its `vkey = VersionKeyBuilder::new("catalog", 1).input(osu_db sha)` from 001's core type. Ingest and archive write raw facts and carry no vkey. There are no param-pack sections. 001's `stage-lock --check` runs with 0 registered stages: `catalog` joins the lock when the engine's stage registry exists (F1).

## Data
- **user.db migration: yes**, `0001_init`, all tables `STRICT`. Times are RFC 3339 UTC strings with milliseconds (`2026-09-28T23:13:56.636Z`). Hashes and ids are `BLOB(32)`.
  ```
  meta(key TEXT PK, value TEXT NOT NULL)
  install(id TEXT PK /*uuid v4*/, secret BLOB NOT NULL, created_at TEXT NOT NULL)   -- exactly one row
  game_install(id INTEGER PK, game TEXT NOT NULL CHECK(game='osu_stable'), root_path TEXT NOT NULL,
               client_version INTEGER NULL, detected_at TEXT NOT NULL, UNIQUE(game, root_path))
  source_snapshot(id INTEGER PK, install_id INTEGER NOT NULL REFERENCES game_install, kind TEXT NOT NULL
               CHECK(kind IN ('osu_db','scores_db','collection_db','cfg','songs_scan')), sha256 BLOB NOT NULL,
               size INTEGER NOT NULL, mtime TEXT NOT NULL, format_version INTEGER NULL, imported_at TEXT NOT NULL)
               + INDEX(install_id, kind, id)
  blob(sha256 BLOB PK, kind TEXT NOT NULL CHECK(kind IN ('osr','osg','osu')), size INTEGER NOT NULL,
       origin_path TEXT NOT NULL, first_seen TEXT NOT NULL)
  alias(id INTEGER PK, game TEXT NOT NULL, raw_name BLOB NOT NULL, UNIQUE(game, raw_name))
  play(id BLOB PK CHECK(length(id)=32), alias_id INTEGER NOT NULL REFERENCES alias, chart_md5 TEXT NOT NULL,
       filetime TEXT NOT NULL, played_at_utc TEXT NOT NULL, mods INTEGER NOT NULL,
       score_system TEXT NOT NULL CHECK(score_system IN ('v1','v2')),
       counts_json TEXT NOT NULL /*{"max","n300","n200","n100","n50","miss"}*/, max_combo INTEGER NOT NULL,
       score INTEGER NOT NULL, native_acc REAL NULL, passed INTEGER NULL CHECK(passed IN (0,1)),
       online_score_id TEXT NULL, client_version INTEGER NOT NULL,
       replay_sha BLOB NULL REFERENCES blob, osg_sha BLOB NULL REFERENCES blob, chart_sha BLOB NULL REFERENCES blob,
       snapshot_id INTEGER NOT NULL REFERENCES source_snapshot, ingested_at TEXT NOT NULL)
       + INDEX(chart_md5), INDEX(alias_id), INDEX(played_at_utc)
       + TRIGGER play_no_delete (RAISE ABORT); TRIGGER play_update_guard: abort unless only replay_sha/osg_sha/chart_sha
         change and each changed one was NULL
  identity_decision(alias_id INTEGER PK REFERENCES alias, decision TEXT NOT NULL CHECK(decision IN ('me','not_me')),
       decided_at TEXT NOT NULL)
  profile(id INTEGER PK, kind TEXT NOT NULL CHECK(kind IN ('self','other')), label TEXT NOT NULL,
       is_default INTEGER NOT NULL DEFAULT 0, merge_mode TEXT NOT NULL CHECK(merge_mode IN ('merged','separate')),
       created_at TEXT NOT NULL) + UNIQUE INDEX ON profile(is_default) WHERE is_default=1
       + UNIQUE INDEX ON profile(kind) WHERE kind='self'   -- 004's self singleton
  profile_alias(profile_id INTEGER REFERENCES profile, alias_id INTEGER REFERENCES alias,
       origin TEXT NOT NULL CHECK(origin IN ('auto','user')), added_at TEXT NOT NULL, PK(profile_id, alias_id))
  feedback_event(id TEXT PK /*ULID*/, ts TEXT NOT NULL, profile_id INTEGER NULL REFERENCES profile, kind TEXT NOT NULL,
       subject_json TEXT NOT NULL, payload_json TEXT NOT NULL, context_json TEXT NOT NULL,
       telemetry_state TEXT NOT NULL DEFAULT 'local_only' CHECK(telemetry_state IN ('local_only','sent','withdrawn')))
       + TRIGGER no_delete; TRIGGER update only telemetry_state
  settings(key TEXT PK, json TEXT NOT NULL)
  ```
  - `meta` holds `schema_created_with` = app version.
  - `fixtures/userdb/v1.db` is generated by a test helper and committed, so every later migration is tested from it (§10).
- **cache.db change: yes**, new, `CACHE_SCHEMA_VERSION = 1`, with columns as in §5.4:
  - `derivation`: PK(stage, input_key, vkey);
  - `catalog_chart`: md5 PK, keymode, title, artist, version, creator, set_id, beatmap_id, path, od, hp, length_ms, snapshot_id;
  - `job_run`: id ULID PK, kind, params_json, status `queued|running|ok|failed|cancelled`, started, ended, summary_json;
  - `item_failure`: job_id, item_ref, code, message, PK(job_id, item_ref).
- **Vault: yes**, new. The layout is `<data>/vault/blobs/ab/cd/<sha256 hex>` plus `<data>/vault/tmp/`. It holds original `.osr`, `.osg` and `.osu` bytes, uncompressed and immutable. Expected pilot size is ≈ 91 MB osr + 511 MB osg + charts (≈ 50 MB estimated, §5.2).

## IPC / UI
There are no Tauri commands in this spec; spec 005 adds `jobs_start`, `jobs_list` and `jobs_cancel` over these services. The app layer provides:
- **Services:**
  - `JobService::{start(JobKindDto, params) -> JobId, list() -> Vec<JobDto>, cancel(JobId)}`;
  - `PlaysService::sync(install_id) -> JobId`.
- **DTOs** (`serde` + `specta::Type`, in `app::jobs::dto`):
  - `JobId(String)`, a ULID;
  - `JobKindDto = "sync_plays"`;
  - `JobDto {id, kind, status, started, ended, summary}`;
  - `SyncSummaryDto`, holding the counters listed under Behaviour.
- **DTO convention** (005): `#[serde(rename_all = "camelCase")]` on every struct, so the Rust fields below are snake_case and the wire is camelCase (`jobId`, `failedItems`, `etaMs`); enum values are snake_case strings. No `i64`/`u64` field (bindings export fails on them); timestamps are RFC 3339 strings.
- `JobStartDto`, a union tagged on `kind`: `{kind: "sync_plays", installId: u32}`.
- **Events** (`app::events`, payload types `JobProgressDto`, `JobFinishedDto`, `DataChangedDto`, which 005 bridges and exports through tauri-specta):
  - `JobProgress {job_id, kind, stage: "catalog"|"ingest"|"archive", done: u32, total: u32, eta_ms: Option<u32>}`, at most 10 Hz;
  - `JobFinished {job_id, status, failed_items: u32}`;
  - `DataChanged {domains: Vec<String>}`.
- **Errors:** `AppError` codes `OSU_DIR_NOT_FOUND, UNSUPPORTED_FORMAT, PARSE_FAILED, OSU_RUNNING, INVALID_INPUT, CONFLICT, CANCELLED, INTERNAL`. There are no new codes, and the i18n keys are listed under Behaviour.

## Acceptance criteria
- [ ] AC1: The natural key is frozen and unambiguous → provided by 001 AC8 (`digest::tests::{play_id_golden_vector, play_id_length_prefix_prevents_concat_collision, rejects_bad_hex}`, golden over the pilot tuple `(osu_stable, e956977c…, TWulfZ, FileTime(134350010443098880))`). This spec adds `app::features::plays::tests::ingest_uses_core_play_id` (a synced play's id equals `PlayId::derive` over its record).
- [ ] AC2: FILETIME conversion matches the `Data/r` naming → provided by 001 AC6 (`time::tests::{filetime_decimal_matches_data_r, filetime_before_1601_is_none}`: ticks `639190703004225018` → `134279471004225018`).
- [ ] AC3: user.db migrates from zero, validates, and refuses a newer schema.
  - Test: `store::user::tests::migrations_validate`.
  - Test: `migrate_from_zero_creates_all_tables` (the 12 tables listed above, plus both partial unique indexes on `profile`).
  - Test: `schema_too_new_is_refused` (user_version 999 → `SchemaTooNew`, file unchanged by sha).
  - Test: `migrates_from_fixture_v1`, run against `fixtures/userdb/v1.db`.
- [ ] AC4: A backup is written before a migration and is restorable.
  - Test: `store::user::tests::vacuum_into_backup_before_migrate`. It uses an injected test migration list v1 → v2 and checks that `backups/user-v1-*.db` exists, opens with `user_version = 1` and holds the seeded rows.
  - Test: `backup_retention_keeps_5`.
- [ ] AC5: The play ledger is immutable at the DB level.
  - Test: `store::repo::tests::play_ledger_immutable`. `DELETE` aborts, an `UPDATE score` aborts, `replay_sha` NULL → value succeeds, and value → other value aborts.
  - Test: `feedback_event_append_only`.
- [ ] AC6: cache.db rebuilds on a version mismatch or corruption and never touches user.db.
  - Test: `store::cache::tests::version_mismatch_deletes_and_recreates`.
  - Test: `garbage_file_is_rebuilt`.
  - Test: `rebuild_leaves_user_db_untouched` (user.db sha unchanged).
- [ ] AC7: The vault is content-addressed, idempotent, atomic and verified.
  - Test: `store::vault::tests::put_layout_ab_cd_sha`.
  - Test: `put_twice_same_path_no_rewrite`.
  - Test: `no_partial_file_after_failed_write`.
  - Test: `get_detects_corruption`.
- [ ] AC8: One writer per DB means no `SQLITE_BUSY` under concurrency.
  - Test: `store::db::tests::concurrent_writes_serialized` (8 threads × 1,000 inserts → 8,000 rows, 0 errors).
  - Test: `readers_see_committed_rows`.
  - Test: `second_instance_lock_conflicts`.
- [ ] AC9: The stable read retries while a file is changing and gives up with `OSU_RUNNING`.
  - Test: `crates/source-osu/tests/snapshot.rs::retries_when_size_changes`.
  - Test: `gives_up_after_3`.
  - Test: `crates/source-osu/tests/replay_dir.rs::parses_valid_names_ignores_others`.
  - Test: `crates/source-osu/tests/songs.rs::md5_mismatch_reported`.
- [ ] AC10: SyncPlays ingests a synthetic install correctly. Test: `app::features::plays::tests::sync_ingests_fixture_install`. The fixture tree is built in a tempdir from 002's test-support builders and holds:
  - 6 mania scores (including alias `""` and a non-UTF-8 name), 1 osu!std score and 1 exact duplicate;
  - `.osr` files for 5 plays, `.osg` for 3, and 1 orphan `.osr`;
  - 2 Songs charts, one of them edited so its md5 no longer matches.

  Expected summary: `plays_new=6, skipped_non_mania=1, replays_linked=5, osg_linked=3, orphan_replays=1, charts_archived=1, chart_md5_mismatch=1`.
- [ ] AC11: **Re-ingest adds 0 rows.**
  - Test: `sync_twice_adds_zero_rows`. The play, alias and blob counts and the vault file count are identical after the second run, and `plays_new=0`.
  - Test: `conflicting_duplicate_records_item_failure` (same key, different score → 1 play, 1 `CONFLICT`).
- [ ] AC12: The osu! folder is never written.
  - Test: `sync_never_writes_install_root`. It compares a recursive (path, size, mtime, sha256) manifest of the fixture root before and after a sync, with the root set read-only on unix.
  - Test: `data_dir_inside_osu_root_rejected`.
- [ ] AC13: Late `Data/r` files are linked on the next sync, and header mismatches are refused.
  - Test: `replay_added_later_links_on_resync`.
  - Test: `osr_header_md5_mismatch_not_linked`.
- [ ] AC14: The job runner coalesces, cancels, throttles and isolates panics.
  - Test: `app::jobs::tests::submit_while_queued_returns_same_id`.
  - Test: `submit_while_running_sets_single_rerun`.
  - Test: `cancel_midway_then_rerun_completes` (after cancel `status=cancelled`; a second run gives a final state identical to an uncancelled run).
  - Test: `progress_at_most_10hz` (mock clock; 1,000 items in 50 ms produce ≤ 1 event plus the final one).
  - Test: `panicking_item_recorded_job_continues` (1 `item_failure` with `code=INTERNAL`, the other items done).
- [ ] AC15: The watcher debounces and triggers exactly one sync.
  - Test: `crates/source-osu/tests/watch.rs::burst_debounced_to_one` (poll mode, 20 file writes in a tempdir → 1 change batch within the 5 s window; the debounce is injectable for the test).
  - Test: `app::watch::tests::change_submits_sync_plays`.
- [ ] AC16: **Cache rebuild from empty equals the incremental state.** Test: `app::features::plays::tests::cache_rebuild_from_empty_equals_incremental`. It runs sync twice with an osu!.db change in between, then deletes cache.db and syncs once. `catalog_chart` and the `derivation` rows compare equal (ordered dump), and user.db is unchanged.
- [ ] AC17: **The pilot corpus ingests.** Command: `WOLLUF_CORPUS="/mnt/e/Games/osu!" cargo nextest run -p wolluf-app --run-ignored only -E 'test(corpus_sync_pilot)'`. The test:
  - uses a tempdir data dir;
  - asserts `plays == distinct (md5, raw_name, filetime) among mode=3 records` of the same snapshot, and `plays ≥ 4,338`. The 2026-09-28 oracle values are 4,969 plays from 4,970 rows.
  - asserts `replays_linked` == the number of plays whose `Data/r` .osr exists, computed independently in the test from a directory listing;
  - asserts that a second sync gives `plays_new = 0` and no new vault files;
  - prints wall times for the first and second runs;
  - writes nothing under `WOLLUF_CORPUS`.
- [ ] AC18: The gates pass: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo xtask check-layers` (rusqlite only in store, notify only in source-osu, tokio/rayon only in app and the shells), and `cargo nextest run --workspace`.

## Risks / open questions
- **WSL drvfs does not deliver inotify events** for writes by Windows processes. The mitigation is poll mode on `/mnt/*`. Windows builds use native events, so live verification of those waits for the Windows E2E checklist.
- **Vault size (O9).** `.osg` files are 5.6× the size of the `.osr` files (511 MB vs 91 MB). If 006 shows `.osg` is redundant or cheap to regenerate, revisit this with an ADR: skip `.osg`, or zstd-at-rest with the sha taken over the original bytes. F0 archives raw bytes because that is the reversible choice.
- **Orphan replays** (42 in the pilot, all mania, headers valid). They may be plays deleted from scores.db. Their bytes are archived now so nothing is lost. A later spec could create plays from `.osr` headers, since the header uses the scores.db record layout (`osudb.py::score_body`). That needs a product decision on whether a replay without a score row counts as a play.
- **`passed` is NULL** in F0. This deviates from the NOT NULL column in §5.3 and is recorded in ADR 0014 and the architecture update (T1). F2 derives pass/fail in cache.db.
- **First sync on WSL** reads ≈ 600 MB over drvfs, which can take minutes. Progress events and cancel-then-resume cover it. The corpus test reports the time and has no hard budget.
- **ADR number:** 0014 is assigned to this spec at the F0 review (002 has 0015, 006 has 0012).
- **Torn reads while osu! writes scores.db** are handled by the stable-read retry and the parse retry. The failure mode is a retryable `OSU_RUNNING`, never partial ingest, because parsing happens before any write.

## Deviations (filled at close)
