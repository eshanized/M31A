-- Migration 006: Tool executions, background jobs, and capability health records (TL-03, CTL-01, CTL-03, D-12)

CREATE TABLE IF NOT EXISTS tool_executions (
    id BLOB PRIMARY KEY,
    mission_id BLOB NOT NULL REFERENCES missions(id),
    task_id BLOB NOT NULL REFERENCES tasks(id),
    agent_id BLOB NOT NULL REFERENCES agents(id),
    tool_id TEXT NOT NULL,
    attempt_number INTEGER NOT NULL DEFAULT 1,
    success INTEGER NOT NULL,
    duration_ms INTEGER NOT NULL,
    error_category TEXT,
    error_code TEXT,
    artifact_id BLOB,
    effective_risk TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_tool_executions_task_id ON tool_executions(task_id);
CREATE INDEX IF NOT EXISTS idx_tool_executions_mission_id ON tool_executions(mission_id);
CREATE INDEX IF NOT EXISTS idx_tool_executions_tool_id ON tool_executions(tool_id);

CREATE TABLE IF NOT EXISTS background_jobs (
    id BLOB PRIMARY KEY,
    task_id BLOB NOT NULL REFERENCES tasks(id),
    agent_id BLOB NOT NULL REFERENCES agents(id),
    command TEXT NOT NULL,
    state TEXT NOT NULL,
    exit_code INTEGER,
    artifact_id BLOB,
    started_at TEXT NOT NULL,
    completed_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_background_jobs_task_id ON background_jobs(task_id);
CREATE INDEX IF NOT EXISTS idx_background_jobs_state ON background_jobs(state);

CREATE TABLE IF NOT EXISTS capability_health_records (
    capability_id TEXT NOT NULL,
    provider TEXT NOT NULL,
    previous_state TEXT NOT NULL,
    new_state TEXT NOT NULL,
    reason TEXT NOT NULL,
    consecutive_failures INTEGER NOT NULL,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_capability_health_records_cap ON capability_health_records(capability_id);
