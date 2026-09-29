-- user.db v1: the F0 subset of architecture §5.3 (spec 003 Data, ADR 0014).
-- Times are RFC 3339 UTC strings with milliseconds; hashes and ids are 32-byte BLOBs.

CREATE TABLE meta (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
) STRICT;

-- Telemetry pseudonym + deletion secret; never an osu! id. Exactly one row.
CREATE TABLE install (
    id         TEXT PRIMARY KEY,
    secret     BLOB NOT NULL CHECK (length(secret) = 32),
    created_at TEXT NOT NULL
) STRICT;

CREATE TABLE game_install (
    id             INTEGER PRIMARY KEY,
    game           TEXT NOT NULL CHECK (game = 'osu_stable'),
    root_path      TEXT NOT NULL,
    client_version INTEGER NULL,
    detected_at    TEXT NOT NULL,
    UNIQUE (game, root_path)
) STRICT;

CREATE TABLE source_snapshot (
    id             INTEGER PRIMARY KEY,
    install_id     INTEGER NOT NULL REFERENCES game_install (id),
    kind           TEXT NOT NULL
                   CHECK (kind IN ('osu_db', 'scores_db', 'collection_db', 'cfg', 'songs_scan')),
    sha256         BLOB NOT NULL CHECK (length(sha256) = 32),
    size           INTEGER NOT NULL,
    mtime          TEXT NOT NULL,
    format_version INTEGER NULL,
    imported_at    TEXT NOT NULL
) STRICT;
CREATE INDEX source_snapshot_latest ON source_snapshot (install_id, kind, id);

CREATE TABLE blob (
    sha256      BLOB PRIMARY KEY CHECK (length(sha256) = 32),
    kind        TEXT NOT NULL CHECK (kind IN ('osr', 'osg', 'osu')),
    size        INTEGER NOT NULL,
    origin_path TEXT NOT NULL,
    first_seen  TEXT NOT NULL
) STRICT;

-- Raw bytes are the key; normalization is for matching only; '' is a valid alias.
CREATE TABLE alias (
    id       INTEGER PRIMARY KEY,
    game     TEXT NOT NULL,
    raw_name BLOB NOT NULL,
    UNIQUE (game, raw_name)
) STRICT;

-- Immutable ledger; re-ingest is idempotent through the natural key (ADR 0006, ADR 0014).
CREATE TABLE play (
    id              BLOB PRIMARY KEY CHECK (length(id) = 32),
    alias_id        INTEGER NOT NULL REFERENCES alias (id),
    chart_md5       TEXT NOT NULL,
    origin          TEXT NOT NULL CHECK (origin IN ('scores_db', 'replay_only')),
    filetime        TEXT NOT NULL,
    played_at_utc   TEXT NOT NULL,
    mods            INTEGER NOT NULL,
    score_system    TEXT NOT NULL CHECK (score_system IN ('v1', 'v2')),
    counts_json     TEXT NOT NULL,
    max_combo       INTEGER NOT NULL,
    score           INTEGER NOT NULL,
    native_acc      REAL NULL,
    passed          INTEGER NULL CHECK (passed IN (0, 1)),
    online_score_id TEXT NULL,
    client_version  INTEGER NOT NULL,
    replay_sha      BLOB NULL REFERENCES blob (sha256),
    osg_sha         BLOB NULL REFERENCES blob (sha256),
    chart_sha       BLOB NULL REFERENCES blob (sha256),
    snapshot_id     INTEGER NULL REFERENCES source_snapshot (id),
    ingested_at     TEXT NOT NULL,
    CHECK (origin <> 'scores_db' OR snapshot_id IS NOT NULL),
    CHECK (origin <> 'replay_only' OR replay_sha IS NOT NULL)
) STRICT;
CREATE INDEX play_chart_md5 ON play (chart_md5);
CREATE INDEX play_alias_id ON play (alias_id);
CREATE INDEX play_played_at ON play (played_at_utc);

CREATE TRIGGER play_no_delete BEFORE DELETE ON play
BEGIN
    SELECT RAISE(ABORT, 'play ledger is append-only');
END;

-- Allowed: NULL -> value on the three blob links, and the one-way replay_only -> scores_db
-- upgrade that also sets snapshot_id (spec 003 Domain rules). `IS` compares NULLs as equal.
CREATE TRIGGER play_update_guard BEFORE UPDATE ON play
WHEN NOT (
        NEW.id IS OLD.id
    AND NEW.alias_id IS OLD.alias_id
    AND NEW.chart_md5 IS OLD.chart_md5
    AND NEW.filetime IS OLD.filetime
    AND NEW.played_at_utc IS OLD.played_at_utc
    AND NEW.mods IS OLD.mods
    AND NEW.score_system IS OLD.score_system
    AND NEW.counts_json IS OLD.counts_json
    AND NEW.max_combo IS OLD.max_combo
    AND NEW.score IS OLD.score
    AND NEW.native_acc IS OLD.native_acc
    AND NEW.passed IS OLD.passed
    AND NEW.online_score_id IS OLD.online_score_id
    AND NEW.client_version IS OLD.client_version
    AND NEW.ingested_at IS OLD.ingested_at
    AND (NEW.replay_sha IS OLD.replay_sha OR OLD.replay_sha IS NULL)
    AND (NEW.osg_sha IS OLD.osg_sha OR OLD.osg_sha IS NULL)
    AND (NEW.chart_sha IS OLD.chart_sha OR OLD.chart_sha IS NULL)
    AND (
            (NEW.origin IS OLD.origin AND NEW.snapshot_id IS OLD.snapshot_id)
         OR (OLD.origin = 'replay_only' AND NEW.origin = 'scores_db'
             AND OLD.snapshot_id IS NULL AND NEW.snapshot_id IS NOT NULL)
    )
)
BEGIN
    SELECT RAISE(ABORT, 'play ledger is immutable except NULL-guarded links and the origin upgrade');
END;

-- The user's answer; the auto rule never overrides it.
CREATE TABLE identity_decision (
    alias_id   INTEGER PRIMARY KEY REFERENCES alias (id),
    decision   TEXT NOT NULL CHECK (decision IN ('me', 'not_me')),
    decided_at TEXT NOT NULL
) STRICT;

CREATE TABLE profile (
    id         INTEGER PRIMARY KEY,
    kind       TEXT NOT NULL CHECK (kind IN ('self', 'other')),
    label      TEXT NOT NULL,
    is_default INTEGER NOT NULL DEFAULT 0 CHECK (is_default IN (0, 1)),
    merge_mode TEXT NOT NULL CHECK (merge_mode IN ('merged', 'separate')),
    created_at TEXT NOT NULL
) STRICT;
CREATE UNIQUE INDEX profile_single_default ON profile (is_default) WHERE is_default = 1;
CREATE UNIQUE INDEX profile_single_self ON profile (kind) WHERE kind = 'self';

CREATE TABLE profile_alias (
    profile_id INTEGER NOT NULL REFERENCES profile (id),
    alias_id   INTEGER NOT NULL REFERENCES alias (id),
    origin     TEXT NOT NULL CHECK (origin IN ('auto', 'user')),
    added_at   TEXT NOT NULL,
    PRIMARY KEY (profile_id, alias_id)
) STRICT;

CREATE TABLE feedback_event (
    id              TEXT PRIMARY KEY,
    ts              TEXT NOT NULL,
    profile_id      INTEGER NULL REFERENCES profile (id),
    kind            TEXT NOT NULL,
    subject_json    TEXT NOT NULL,
    payload_json    TEXT NOT NULL,
    context_json    TEXT NOT NULL,
    telemetry_state TEXT NOT NULL DEFAULT 'local_only'
                    CHECK (telemetry_state IN ('local_only', 'sent', 'withdrawn'))
) STRICT;

-- Append-only: model state must stay replayable from these rows (architecture §5.3 Rule).
CREATE TRIGGER feedback_event_no_delete BEFORE DELETE ON feedback_event
BEGIN
    SELECT RAISE(ABORT, 'feedback_event is append-only');
END;

CREATE TRIGGER feedback_event_update_guard BEFORE UPDATE ON feedback_event
WHEN NOT (
        NEW.id IS OLD.id
    AND NEW.ts IS OLD.ts
    AND NEW.profile_id IS OLD.profile_id
    AND NEW.kind IS OLD.kind
    AND NEW.subject_json IS OLD.subject_json
    AND NEW.payload_json IS OLD.payload_json
    AND NEW.context_json IS OLD.context_json
)
BEGIN
    SELECT RAISE(ABORT, 'feedback_event only allows telemetry_state updates');
END;

-- UI/UX preferences only; nothing that changes a derived number (architecture §5.3 Rule).
CREATE TABLE settings (
    key  TEXT PRIMARY KEY,
    json TEXT NOT NULL
) STRICT;
