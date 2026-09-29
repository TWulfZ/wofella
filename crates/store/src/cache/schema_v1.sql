-- cache.db v1 (architecture §5.4, spec 003 Data). Disposable: a schema change bumps
-- CACHE_SCHEMA_VERSION and the file is rebuilt; there are no migrations, ever.
-- Times are RFC 3339 UTC strings with milliseconds, like user.db.

-- Memo of every derived item: staleness, per-item failures, "recompute only what changed".
CREATE TABLE derivation (
    stage       TEXT NOT NULL,
    input_key   TEXT NOT NULL,
    vkey        BLOB NOT NULL CHECK (length(vkey) = 32),
    status      TEXT NOT NULL CHECK (status IN ('ok', 'failed', 'skipped')),
    error_code  TEXT NULL,
    error_msg   TEXT NULL,
    duration_ms INTEGER NULL,
    PRIMARY KEY (stage, input_key, vkey)
) STRICT;

-- Mania entries of the latest osu!.db snapshot; `snapshot_id` points into user.db, so there is
-- no foreign key across files.
CREATE TABLE catalog_chart (
    md5         TEXT PRIMARY KEY,
    keymode     INTEGER NOT NULL,
    title       TEXT NOT NULL,
    artist      TEXT NOT NULL,
    version     TEXT NOT NULL,
    creator     TEXT NOT NULL,
    set_id      INTEGER NULL,
    beatmap_id  INTEGER NULL,
    path        TEXT NOT NULL,
    od          REAL NOT NULL,
    hp          REAL NOT NULL,
    length_ms   INTEGER NOT NULL,
    snapshot_id INTEGER NOT NULL
) STRICT;

CREATE TABLE job_run (
    id           TEXT PRIMARY KEY,
    kind         TEXT NOT NULL,
    params_json  TEXT NOT NULL,
    status       TEXT NOT NULL CHECK (status IN ('queued', 'running', 'ok', 'failed', 'cancelled')),
    started      TEXT NULL,
    ended        TEXT NULL,
    summary_json TEXT NULL
) STRICT;

CREATE TABLE item_failure (
    job_id   TEXT NOT NULL REFERENCES job_run (id),
    item_ref TEXT NOT NULL,
    code     TEXT NOT NULL,
    message  TEXT NOT NULL,
    PRIMARY KEY (job_id, item_ref)
) STRICT;
