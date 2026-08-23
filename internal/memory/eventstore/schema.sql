-- Event Store Schema
-- Source: modernc.org/sqlite WAL mode + CONTEXT_M31A.md §11 (monotonic ordering, range queries)
-- Run: executed once at EventStore initialization

PRAGMA journal_mode = WAL;
PRAGMA foreign_keys = ON;
PRAGMA busy_timeout = 5000;

CREATE TABLE IF NOT EXISTS events (
    seq         INTEGER PRIMARY KEY AUTOINCREMENT,  -- Monotonic ordering
    id          TEXT NOT NULL UNIQUE,               -- UUID
    type        TEXT NOT NULL,                      -- EventType
    timestamp   INTEGER NOT NULL,                   -- Unix nanoseconds
    run_id      TEXT,                               -- UUID, nullable
    session_id  TEXT,                               -- UUID, nullable
    payload     BLOB NOT NULL,                      -- JSON payload
    metadata    BLOB                                -- JSON metadata (optional)
);

-- Efficient range queries by run/session/type
CREATE INDEX IF NOT EXISTS idx_events_run_seq      ON events(run_id, seq);
CREATE INDEX IF NOT EXISTS idx_events_session_seq  ON events(session_id, seq);
CREATE INDEX IF NOT EXISTS idx_events_type_seq     ON events(type, seq);
CREATE INDEX IF NOT EXISTS idx_events_timestamp    ON events(timestamp);

-- Projection checkpoints for fast resume
CREATE TABLE IF NOT EXISTS projections (
    name        TEXT PRIMARY KEY,                   -- e.g., "project", "requirements", "runs"
    last_seq    INTEGER NOT NULL,                   -- Last event seq applied
    updated_at  INTEGER NOT NULL                    -- Unix nanoseconds
);

-- Migration tracking
CREATE TABLE IF NOT EXISTS migrations (
    version     INTEGER PRIMARY KEY,
    description TEXT NOT NULL,
    applied_at  INTEGER NOT NULL
);