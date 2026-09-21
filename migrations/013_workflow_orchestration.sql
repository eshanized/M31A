-- Migration 013: Workflow orchestration kernel, step runs, and artifact tracking

CREATE TABLE IF NOT EXISTS workflow_runs (
    id BLOB PRIMARY KEY,
    definition_id TEXT NOT NULL,
    definition_version INTEGER NOT NULL DEFAULT 1,
    workspace_root TEXT NOT NULL,
    status TEXT NOT NULL,
    mode TEXT NOT NULL DEFAULT 'standard',
    current_step_key TEXT,
    started_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    completed_at TEXT,
    error_summary TEXT
);

CREATE TABLE IF NOT EXISTS workflow_step_runs (
    id BLOB PRIMARY KEY,
    workflow_run_id BLOB NOT NULL REFERENCES workflow_runs(id) ON DELETE CASCADE,
    step_key TEXT NOT NULL,
    status TEXT NOT NULL,
    assigned_agent_id BLOB REFERENCES agents(id),
    mission_id BLOB REFERENCES missions(id),
    attempt_count INTEGER NOT NULL DEFAULT 1,
    started_at TEXT NOT NULL,
    completed_at TEXT,
    halt_reason TEXT,
    UNIQUE (workflow_run_id, step_key)
);

CREATE TABLE IF NOT EXISTS workflow_artifacts (
    id BLOB PRIMARY KEY,
    workflow_run_id BLOB NOT NULL REFERENCES workflow_runs(id) ON DELETE CASCADE,
    step_run_id BLOB NOT NULL REFERENCES workflow_step_runs(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    path TEXT NOT NULL,
    content_hash TEXT NOT NULL,
    version INTEGER NOT NULL DEFAULT 1,
    status TEXT NOT NULL DEFAULT 'valid',
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_workflow_runs_status ON workflow_runs(status);
CREATE INDEX IF NOT EXISTS idx_step_runs_workflow ON workflow_step_runs(workflow_run_id);
CREATE INDEX IF NOT EXISTS idx_workflow_artifacts_run ON workflow_artifacts(workflow_run_id);
CREATE INDEX IF NOT EXISTS idx_workflow_artifacts_step ON workflow_artifacts(step_run_id);
