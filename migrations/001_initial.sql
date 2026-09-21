-- Initial database schema for M31A
-- Per D-06, UUIDv7 IDs are stored as BLOB (16 bytes) for SQLite efficiency

-- Missions table
CREATE TABLE missions (
    id BLOB PRIMARY KEY,
    objective TEXT NOT NULL,
    status TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

-- Tasks table
CREATE TABLE tasks (
    id BLOB PRIMARY KEY,
    mission_id BLOB NOT NULL REFERENCES missions(id),
    title TEXT NOT NULL,
    status TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

-- Agents table
CREATE TABLE agents (
    id BLOB PRIMARY KEY,
    mission_id BLOB NOT NULL REFERENCES missions(id),
    role TEXT NOT NULL,
    status TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

-- Event log table (for durable event ordering per D-11)
CREATE TABLE event_log (
    id BLOB PRIMARY KEY,
    sequence INTEGER NOT NULL,
    mission_id BLOB,
    event_type TEXT NOT NULL,
    payload TEXT NOT NULL,
    schema_version INTEGER NOT NULL,
    created_at TEXT NOT NULL
);

-- Create indexes for common query patterns
CREATE INDEX idx_tasks_mission_id ON tasks(mission_id);
CREATE INDEX idx_agents_mission_id ON agents(mission_id);
CREATE INDEX idx_event_log_mission_id ON event_log(mission_id);
CREATE INDEX idx_event_log_sequence ON event_log(sequence);