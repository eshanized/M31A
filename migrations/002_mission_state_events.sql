-- Migration 002: Mission state, sessions, and causal event logging enhancements (D-01, D-13, D-15, PST-01)

-- Expand missions table with all aggregate fields
ALTER TABLE missions ADD COLUMN workspace_root TEXT NOT NULL DEFAULT '';
ALTER TABLE missions ADD COLUMN mode TEXT NOT NULL DEFAULT 'safe';
ALTER TABLE missions ADD COLUMN policy_context TEXT NOT NULL DEFAULT '{}';
ALTER TABLE missions ADD COLUMN budget TEXT NOT NULL DEFAULT '{}';
ALTER TABLE missions ADD COLUMN constraints TEXT NOT NULL DEFAULT '[]';
ALTER TABLE missions ADD COLUMN requirements TEXT NOT NULL DEFAULT '[]';
ALTER TABLE missions ADD COLUMN success_criteria TEXT NOT NULL DEFAULT '[]';
ALTER TABLE missions ADD COLUMN started_at TEXT;
ALTER TABLE missions ADD COLUMN completed_at TEXT;
ALTER TABLE missions ADD COLUMN parent_mission_id BLOB REFERENCES missions(id);
ALTER TABLE missions ADD COLUMN task_graph_id BLOB;
ALTER TABLE missions ADD COLUMN last_applied_sequence INTEGER NOT NULL DEFAULT 0;

-- Create sessions table for interaction and execution streams
CREATE TABLE IF NOT EXISTS sessions (
    id BLOB PRIMARY KEY,
    mission_id BLOB NOT NULL REFERENCES missions(id),
    status TEXT NOT NULL,
    metadata TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    closed_at TEXT
);
CREATE INDEX IF NOT EXISTS idx_sessions_mission_id ON sessions(mission_id);

-- Expand event_log table with actor, category, correlation_id, causation_id, and session_id
ALTER TABLE event_log ADD COLUMN session_id BLOB REFERENCES sessions(id);
ALTER TABLE event_log ADD COLUMN actor TEXT NOT NULL DEFAULT 'system';
ALTER TABLE event_log ADD COLUMN category TEXT NOT NULL DEFAULT 'durable';
ALTER TABLE event_log ADD COLUMN correlation_id TEXT;
ALTER TABLE event_log ADD COLUMN causation_id BLOB;

-- Create composite index for sequence-ordered event retrieval per mission
CREATE INDEX IF NOT EXISTS idx_event_log_mission_seq ON event_log(mission_id, sequence);
