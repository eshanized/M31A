-- Migration 008: Policy decisions, interactive approval requests, session grants, and supervised jobs (POL-05, D-04, D-09, D-10, D-13)

CREATE TABLE IF NOT EXISTS policy_decisions (
    id BLOB PRIMARY KEY,
    mission_id BLOB NOT NULL REFERENCES missions(id),
    task_id BLOB REFERENCES tasks(id),
    agent_id BLOB REFERENCES agents(id),
    tool_call_id TEXT,
    tool_or_capability TEXT NOT NULL,
    matched_rule_id TEXT,
    matched_layer TEXT NOT NULL,
    precedence_rank INTEGER NOT NULL,
    decision TEXT NOT NULL,
    normalized_args_hash TEXT NOT NULL,
    resource_scope TEXT NOT NULL,
    authority_source TEXT NOT NULL,
    policy_hash TEXT NOT NULL,
    explanation TEXT NOT NULL,
    created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_policy_decisions_mission ON policy_decisions(mission_id);
CREATE INDEX IF NOT EXISTS idx_policy_decisions_task ON policy_decisions(task_id);

CREATE TABLE IF NOT EXISTS approval_requests (
    id BLOB PRIMARY KEY,
    mission_id BLOB NOT NULL REFERENCES missions(id),
    task_id BLOB REFERENCES tasks(id),
    agent_id BLOB REFERENCES agents(id),
    tool_call_id TEXT NOT NULL,
    tool_or_capability TEXT NOT NULL,
    normalized_args_json TEXT NOT NULL,
    redacted_args_json TEXT NOT NULL,
    affected_resources TEXT NOT NULL,
    risk_classification TEXT NOT NULL,
    matched_rule_id TEXT,
    policy_hash TEXT NOT NULL,
    reason TEXT NOT NULL,
    resolution_state TEXT NOT NULL,
    resolution_scope TEXT,
    resolved_by TEXT,
    expires_at TEXT,
    created_at TEXT NOT NULL,
    resolved_at TEXT
);
CREATE INDEX IF NOT EXISTS idx_approval_requests_task ON approval_requests(task_id);
CREATE INDEX IF NOT EXISTS idx_approval_requests_state ON approval_requests(resolution_state);

CREATE TABLE IF NOT EXISTS policy_grants (
    id BLOB PRIMARY KEY,
    mission_id BLOB NOT NULL REFERENCES missions(id),
    task_id BLOB REFERENCES tasks(id),
    scope_type TEXT NOT NULL,
    tool_or_capability TEXT NOT NULL,
    resource_pattern TEXT NOT NULL,
    arg_constraints_json TEXT,
    created_from_request_id BLOB REFERENCES approval_requests(id),
    policy_hash TEXT NOT NULL,
    revoked INTEGER NOT NULL DEFAULT 0,
    expires_at TEXT,
    created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_policy_grants_mission ON policy_grants(mission_id);
CREATE INDEX IF NOT EXISTS idx_policy_grants_tool ON policy_grants(tool_or_capability);

CREATE TABLE IF NOT EXISTS jobs (
    id BLOB PRIMARY KEY,
    mission_id BLOB NOT NULL REFERENCES missions(id),
    task_id BLOB NOT NULL REFERENCES tasks(id),
    agent_id BLOB NOT NULL REFERENCES agents(id),
    tool_call_id TEXT,
    command TEXT NOT NULL,
    args_json TEXT NOT NULL,
    working_dir TEXT NOT NULL,
    state TEXT NOT NULL,
    pid INTEGER,
    provider TEXT NOT NULL,
    resource_limits_json TEXT NOT NULL,
    stdout_spool_path TEXT,
    stderr_spool_path TEXT,
    artifact_id BLOB,
    exit_code INTEGER,
    failure_reason TEXT,
    heartbeat_at TEXT,
    recovery_metadata_json TEXT,
    submitted_at TEXT NOT NULL,
    started_at TEXT,
    completed_at TEXT
);
CREATE INDEX IF NOT EXISTS idx_jobs_mission ON jobs(mission_id);
CREATE INDEX IF NOT EXISTS idx_jobs_task ON jobs(task_id);
CREATE INDEX IF NOT EXISTS idx_jobs_state ON jobs(state);
