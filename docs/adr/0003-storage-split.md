# 0003 Storage: user.db, cache.db and a content-addressed vault

- Status: Accepted
- Date: 2026-09-28

## Context
wolluf keeps two kinds of data with opposite durability needs (architecture §5.2–§5.4):
- **Irreplaceable facts and human input.** The play ledger outlives osu!'s own files: `Data/r` gets cleaned and maps are updated or deleted in Songs. Identity decisions, profiles, feedback events, impressions, consent and installed packs are also irreplaceable. Losing them loses history that cannot be rebuilt.
- **Everything computed.** Parsed charts, segments, difficulty, judged notes, evidence, skill traces, sessions and alias stats are pure functions of the raw data, the code version and the param pack (G1). They change whenever an engine stage or a pack changes.

The decoders are the weak link. `.osr` parsing has known time-drift bugs in the ecosystem (osrparse; research `03-maniahub-rejudge-drills-sessions-audit.txt` l.111, l.156), LN judging is approximate, and the `.osg` format is still unknown (O1). If a decoded form were stored as the raw record, a decoder bug would corrupt data permanently.

The source proposals disagreed on three SQLite tiers versus two (Appendix A).

## Decision
Two SQLite files plus a vault, all in the app data dir and **never inside the osu! folder**:

| Store | Holds | Policy |
|---|---|---|
| `user.db` | The play ledger (`play` keyed by the natural-key hash of ADR 0006), aliases, identity decisions, profiles, feedback events, impressions, drills, consent, param packs, source snapshot provenance, UI settings | Irreplaceable. Forward-only migrations (`rusqlite_migration`), a `VACUUM INTO` backup before every migration, tested against every released schema in `fixtures/userdb/`. A newer schema than the binary knows is refused, never downgraded. |
| `cache.db` | Every derived table, each row carrying its `vkey` (ADR 0006) | Disposable. On a `CACHE_SCHEMA_VERSION` mismatch or corruption it is deleted and rebuilt. **No migrations, ever.** "Rebuild everything" means deleting one file. |
| `vault/blobs/ab/cd/<sha256>` | The original `.osr`, `.osg` and `.osu` bytes for every play and drill chart | Immutable and content-addressed. Decoded forms are derivations in cache.db, so a decoder bug is fixed by bumping the stage `VERSION` and recomputing. |

Rules:
- **Raw versus derived is the dividing line.** Anything that changes a derived number lives in `feedback_event` (append-only), `identity_decision` or the profile tables. It never lives in `settings`, which holds UI preferences only (§5.3). That is what makes every model state replayable.
- Both DBs use WAL, one writer thread each, and a small read pool. No write ever needs to be atomic across the two files, so there is no ATTACH.
- Only `wolluf-store` touches either file or the vault (D6, ADR 0004).
- **Vault default (O9, F0 policy):** archive every play that has a replay, whoever played it, plus the chart bytes of every chart with a play. An alias reclassified as "me" later still needs its replay after `Data/r` is cleaned. `.osg` bytes are kept raw until ADR 0012 (006 spike) decides their handling. A later setting may limit archiving to self profiles, and the UI then says reproducibility is lost for the others.

## Alternatives considered
- **Three SQLite tiers (raw / user / cache).** Rejected: a raw tier holding irreplaceable replay data needs exactly user.db's durability policy, so it is user.db under another name. ATTACH + WAL gives atomicity per file only, so cross-file transactions would be a footgun.
- **Storing decoded replay events as the raw record.** Rejected: a decoder bug (for example the osrparse time drift) would permanently corrupt the ledger. Keeping the original bytes makes every decode re-runnable.
- **Migrating cache.db like user.db.** Rejected: every migration is code that has to be tested forever, for data that can be recomputed in minutes. Delete and rebuild is simpler and always correct.
- **Blobs inside SQLite.** Rejected: 600+ MB of immutable bytes (pilot estimate: ≈ 91 MB `.osr` + 511 MB `.osg` + ≈ 50 MB charts, spec 003) would bloat backups and the `VACUUM INTO` before migrations. A content-addressed directory deduplicates for free and can be verified by re-hashing.
- **Archiving only self-profile plays.** Rejected as the default: identity decisions can change later (ADR 0005), and a replay not archived before `Data/r` is cleaned is gone.

## Consequences
- Every derived number can be recomputed from user.db + vault + code version + pack (G1). A decoder or rule fix costs a recompute, not data loss.
- user.db migrations are the one place where schema work is expensive and must be tested against historical fixtures.
- cache.db schema changes are cheap: bump `CACHE_SCHEMA_VERSION` and let it rebuild.
- Disk use grows with the vault (hundreds of MB on the pilot). O9 stays open for the self-only setting and compression. Any change to the vault's byte contract (for example zstd at rest) needs a new ADR.
- Spec 003 implements this ADR in F0. ADR 0014 records the F0 ledger refinements (in-memory snapshots, nullable `play.passed`, `play.origin`).
