-- Migration 011: Telemetry, Budgets, and Completion Reports (OBS-01, BST-01, RPT-01)

CREATE TABLE IF NOT EXISTS telemetry_spans (
    span_id TEXT PRIMARY KEY NOT NULL,
    trace_id TEXT NOT NULL,
    parent_span_id TEXT,
    mission_id TEXT NOT NULL,
    task_id TEXT,
    agent_id TEXT,
    name TEXT NOT NULL,
    kind TEXT NOT NULL,
    start_time_us INTEGER NOT NULL,
    end_time_us INTEGER,
    duration_us INTEGER,
    status TEXT NOT NULL,
    error_message TEXT,
    attributes_json TEXT NOT NULL DEFAULT '{}'
);

CREATE INDEX IF NOT EXISTS idx_telemetry_spans_mission ON telemetry_spans (mission_id);
CREATE INDEX IF NOT EXISTS idx_telemetry_spans_trace ON telemetry_spans (trace_id);
CREATE INDEX IF NOT EXISTS idx_telemetry_spans_task ON telemetry_spans (task_id);

CREATE TABLE IF NOT EXISTS metric_samples (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    mission_id TEXT NOT NULL,
    timestamp_us INTEGER NOT NULL,
    metric_name TEXT NOT NULL,
    metric_value REAL NOT NULL,
    metric_unit TEXT NOT NULL,
    labels_json TEXT NOT NULL DEFAULT '{}'
);

CREATE INDEX IF NOT EXISTS idx_metric_samples_mission_name ON metric_samples (mission_id, metric_name);

CREATE TABLE IF NOT EXISTS budget_grants (
    grant_id TEXT PRIMARY KEY NOT NULL,
    mission_id TEXT NOT NULL,
    actor TEXT NOT NULL,
    reason TEXT NOT NULL,
    limits_json TEXT NOT NULL,
    granted_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_budget_grants_mission ON budget_grants (mission_id);

CREATE TABLE IF NOT EXISTS completion_reports (
    mission_id TEXT PRIMARY KEY NOT NULL,
    schema_version INTEGER NOT NULL,
    status TEXT NOT NULL,
    report_md_artifact_id TEXT NOT NULL,
    report_json_artifact_id TEXT NOT NULL,
    tasks_succeeded INTEGER NOT NULL,
    tasks_failed INTEGER NOT NULL,
    verification_passed INTEGER NOT NULL,
    token_usage_json TEXT NOT NULL,
    evidence_count INTEGER NOT NULL,
    created_at TEXT NOT NULL
);
