# 0006 Versioned derivations, stage-lock and pack-section keys

- Status: Accepted
- Date: 2026-09-28

## Context
Every derived number must be reproducible from raw observations, code version and param pack (G1). Derived data rots silently in two classic ways:
- someone changes a stage's output and forgets to invalidate what was computed before;
- a parameter change invalidates too much (re-judging 4.3k replays because a calibration table moved) or too little.

The ledger also needs a stable identity for each play, so that re-ingesting the same scores.db adds zero rows (§12 F0 exit) and so that derived rows can reference plays across rebuilds. These identities and keys are persisted: an encoding change would duplicate the whole ledger or orphan every cache row. The encoding therefore has to be pinned once, here (spec 001 is its single owner).

## Decision
**Stage versions.**
- Every engine stage has `const VERSION: u32`, bumped whenever its outputs change.
- `stage_versions.lock` (TOML, `[stages.<stage_id>] version = <u32>, golden = "<blake3 hex>"`) pins each stage's VERSION together with a hash of its quantised golden outputs over the committed fixtures.
- `cargo xtask stage-lock --check` runs in CI and fails when the goldens change without a VERSION bump, or when the lock names a stage that is not registered. `cargo xtask stage-lock` rewrites the lock after an intended change.

**Pack-section keys.** A param pack is a `pack_id` plus one hash per section. Each stage declares `pack_sections()`, and only those sections enter its key. A difficulty recalibration therefore never re-judges replays.

**Version key.** `vkey = blake3(stage_id, VERSION, declared pack-section hashes, config hash, input fingerprint)` (§5.5). The config hash covers the layout id, the label-override hash for the chart and the scope hash where relevant.
- Built by `VersionKeyBuilder::new(StageId, version).section(name, [u8; 32])….config([u8; 32]).input([u8; 32]).finish()`. Sections are sorted by name inside `finish`, so call order is irrelevant. A duplicate section name is an error. A missing config or input hash is 32 zero bytes.
- Printed as 64 lowercase hex characters.
- **D15:** every derived row carries its `vkey`, and nothing derived is read without a key check.

**Staleness and coexistence.** Staleness planning is pure (in `engine`, from F1) and compares stored `derivation` rows with the current keys over the static DAG of §5.5. Rows under old keys are kept until GC, which retains the last 2 keys per stage. While a cascade runs, reads fall back to the previous key and the view is flagged "recomputing". Rollback reactivates the previous pack, whose rows usually still exist. `EngineManifest` (every stage's id and VERSION plus the active pack id) is recorded in every feedback event and impression.

**Frozen byte encodings.** Both hashes are BLAKE3 (32-byte output) over the byte string built as follows:
1. an ASCII domain tag, written as raw bytes with no length prefix;
2. then the fields in the order listed. Every byte-string field, including fixed-size digests, is written as its length (`u32` little-endian) followed by its bytes. Integers are written little-endian at fixed width, with no length prefix.

| Hash | Tag | Fields in order |
|---|---|---|
| `PlayId` | `wolluf.play.v1` | game stable string (`osu_stable`) · chart md5 (the 16 raw bytes) · raw player name bytes (`""` is valid and gives length 0) · `FileTime` as `i64` |
| `VersionKey` | `wolluf.vkey.v1` | stage id string · VERSION as `u32` · section count as `u32` · for each section in ascending byte order of its name: section name, section hash (32 bytes) · config hash (32 bytes) · input fingerprint (32 bytes) |

- The length prefixes make field boundaries unambiguous: moving bytes from one field into the next changes the hash, and the names `""` and `"W"` always give distinct ids.
- `PlayId` hashes the `FileTime` value that §5.3 names. scores.db and `.osr` headers store .NET ticks (100 ns since 0001-01-01). They are converted once at ingest through `DotNetTicks::to_filetime` (FILETIME = ticks − 504 911 232 000 000 000; research `03-maniahub-rejudge-drills-sessions-audit.txt` l.168, 4,362/4,362 names verified). This is bijective with ticks from 1601 on. `play.filetime` stores the same value as decimal text, which is also the `Data/r` file-name suffix.
- **Golden vector.** For the pilot tuple `(osu_stable, e956977ccc1d74a50ae48b43a868cc20, b"TWulfZ", FileTime(134350010443098880))`, `PlayId` = `f3ad5bfc0d2ce9ddbf14a94b24f56ee30c7e9057e46d6019e6cfed0b3110c6b7`. This was computed from the encoding above with blake3 1.8.7. The core test `digest::tests::play_id_golden_vector` must reproduce it. The `VersionKey` golden vector is frozen in `vkey::tests::golden_vector` (`55d34b8447311fb1e170684439726896dba668efed73439a7b5a31b5c494ae68` for that test's builder inputs).
- **These encodings never change.** A new encoding would get a new tag (`.v2`) and a migration ADR. ADR 0014 and every later spec reference this ADR and never re-pin the encoding.

**F0 scope.** No engine stage exists yet, so the F0 lock contains only a header. The F0 derivations (`catalog`, `players.alias_stats`) already carry `VersionKey`s, and they join `stage_versions.lock` when the engine registry arrives in F1.

## Alternatives considered
- **Timestamps or "last computed" flags instead of keys.** Rejected: they cannot tell which inputs changed, and they give no way to keep old and new results side by side for rollback.
- **One hash for the whole pack.** Rejected: every pack release would invalidate every stage, including the expensive re-judge.
- **Relying on reviewers to bump VERSION.** Rejected: a forgotten bump is the classic way derived data rots. The golden hash in `stage_versions.lock` makes CI catch it.
- **`PlayId` from the ticks, or from the scores.db replay md5.** Ticks were rejected because §5.3 names FILETIME and the `Data/r` name carries FILETIME, so one value serves the key, the column and the file link. The replay md5 was rejected because orphan `Data/r` replays and scores without a replay must still get ids, and the natural key must match between scores.db and `.osr` headers.
- **Plain concatenation or a text format (JSON, `a|b|c`) for the hash input.** Rejected: plain concatenation collides across field boundaries, and a text format depends on escaping and number formatting. Length-prefixed binary is unambiguous and cheap.
- **Double-buffered active-key pointers or fold checkpoints.** Rejected (Appendix A): old-key rows plus a read-fallback rule are enough, and a full refold takes seconds.

## Consequences
- A forgotten VERSION bump fails CI instead of corrupting user data.
- Recomputation is exactly as wide as the change: a pack section change touches only the stages that declare it.
- `PlayId` is stable forever. Re-ingest is idempotent through it, and cross-DB references (cache.db → user.db) stay valid across cache rebuilds.
- Golden fixtures and quantisation rules become part of every stage's contract. Changing them is itself a lock change.
- Any future need to change either encoding costs a new tag and a ledger migration. That cost is intended.
