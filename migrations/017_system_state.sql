-- Migration 017: Canonical system_state key-value store (SYS-01, D-22)
CREATE TABLE IF NOT EXISTS system_state (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_system_state_updated ON system_state(updated_at);
