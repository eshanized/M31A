-- Migration 012: Interactive developer sessions, typed conversation turns, and @mentions (PRD §01, MSN-05)

-- Add workspace_root and active_mission_id to sessions table
ALTER TABLE sessions ADD COLUMN workspace_root TEXT NOT NULL DEFAULT '.';
ALTER TABLE sessions ADD COLUMN active_mission_id BLOB REFERENCES missions(id);

-- Create typed conversation_messages table for durable multi-turn history
CREATE TABLE IF NOT EXISTS conversation_messages (
    id BLOB PRIMARY KEY,
    session_id BLOB NOT NULL REFERENCES sessions(id),
    sequence INTEGER NOT NULL,
    kind TEXT NOT NULL,
    content TEXT NOT NULL,
    payload_json TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_conversation_messages_session_seq
    ON conversation_messages(session_id, sequence);
