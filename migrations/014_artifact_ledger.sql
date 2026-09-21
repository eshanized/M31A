-- Migration 014: Unified artifact ledger and provenance tracking (ART-01, ART-02, D-13)

CREATE TABLE IF NOT EXISTS artifacts (
    id BLOB PRIMARY KEY,
    name TEXT NOT NULL,
    logical_path TEXT NOT NULL,
    content_hash TEXT NOT NULL,
    size_bytes INTEGER NOT NULL,
    extension TEXT NOT NULL,
    version INTEGER NOT NULL DEFAULT 1,
    status TEXT NOT NULL DEFAULT 'valid',
    workflow_run_id BLOB,
    step_run_id BLOB,
    mission_id BLOB,
    task_id BLOB,
    producer_role TEXT,
    prompt_id TEXT,
    prompt_version INTEGER,
    model TEXT,
    parent_artifact_id BLOB,
    metadata_json TEXT,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_artifacts_hash ON artifacts(content_hash);
CREATE INDEX IF NOT EXISTS idx_artifacts_workflow ON artifacts(workflow_run_id);
CREATE INDEX IF NOT EXISTS idx_artifacts_step ON artifacts(step_run_id);
CREATE INDEX IF NOT EXISTS idx_artifacts_mission ON artifacts(mission_id);
CREATE INDEX IF NOT EXISTS idx_artifacts_task ON artifacts(task_id);
