-- Migration 009: Verification checks, requirement coverage, completion gate decisions, and recovery audit (VER-04, FLC-05, CHK-01, D-04, D-06, D-13)

-- 1. Structured verification checks (VER-04, D-04)
CREATE TABLE IF NOT EXISTS verification_checks (
    id BLOB PRIMARY KEY,
    mission_id BLOB NOT NULL REFERENCES missions(id),
    task_id BLOB NOT NULL REFERENCES tasks(id),
    tier INTEGER NOT NULL,
    status TEXT NOT NULL,
    command_or_tool TEXT NOT NULL,
    inputs_normalized TEXT NOT NULL,
    evidence_artifact_id BLOB,
    summary TEXT NOT NULL,
    failure_class TEXT,
    snapshot_hash TEXT NOT NULL,
    created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_verification_checks_task ON verification_checks(task_id);
CREATE INDEX IF NOT EXISTS idx_verification_checks_mission ON verification_checks(mission_id);
CREATE INDEX IF NOT EXISTS idx_verification_checks_status ON verification_checks(status);

-- 2. Requirement to check coverage bindings (VER-04, D-04)
CREATE TABLE IF NOT EXISTS requirement_check_coverage (
    id BLOB PRIMARY KEY,
    requirement_id BLOB NOT NULL,
    check_id BLOB NOT NULL REFERENCES verification_checks(id),
    coverage_role TEXT NOT NULL,
    is_mandatory INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_req_coverage_req ON requirement_check_coverage(requirement_id);
CREATE INDEX IF NOT EXISTS idx_req_coverage_check ON requirement_check_coverage(check_id);

-- 3. Completion gate decisions (VER-05, D-04)
CREATE TABLE IF NOT EXISTS completion_gate_decisions (
    id BLOB PRIMARY KEY,
    mission_id BLOB NOT NULL REFERENCES missions(id),
    task_id BLOB REFERENCES tasks(id),
    gate_scope TEXT NOT NULL, -- 'task' or 'mission'
    decision TEXT NOT NULL,   -- 'satisfied' or 'deficient'
    snapshot_hash TEXT NOT NULL,
    violations_json TEXT,
    evidence_summary TEXT NOT NULL,
    created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_completion_gate_task ON completion_gate_decisions(task_id);
CREATE INDEX IF NOT EXISTS idx_completion_gate_mission ON completion_gate_decisions(mission_id);

-- 4. Bounded recovery attempt audit log (FLC-05, D-06)
CREATE TABLE IF NOT EXISTS recovery_attempts (
    id BLOB PRIMARY KEY,
    mission_id BLOB NOT NULL REFERENCES missions(id),
    task_id BLOB NOT NULL REFERENCES tasks(id),
    failure_class TEXT NOT NULL,
    strategy TEXT NOT NULL,
    attempt_number INTEGER NOT NULL,
    budget_consumed INTEGER NOT NULL,
    remaining_class_budget INTEGER NOT NULL,
    remaining_overall_budget INTEGER NOT NULL,
    backoff_delay_ms INTEGER NOT NULL,
    action_taken TEXT NOT NULL,
    result TEXT NOT NULL,
    created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_recovery_attempts_task ON recovery_attempts(task_id);
CREATE INDEX IF NOT EXISTS idx_recovery_attempts_class ON recovery_attempts(failure_class);

-- 5. Extended checkpoints & artifact references (CHK-01, D-13)
CREATE TABLE IF NOT EXISTS checkpoint_artifacts (
    id BLOB PRIMARY KEY,
    checkpoint_id BLOB NOT NULL REFERENCES checkpoints(id),
    artifact_id BLOB NOT NULL,
    sha256_hash TEXT NOT NULL,
    size_bytes INTEGER NOT NULL,
    artifact_type TEXT NOT NULL,
    created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_checkpoint_artifacts_cp ON checkpoint_artifacts(checkpoint_id);

-- 6. Add manifest metadata to checkpoints
ALTER TABLE checkpoints ADD COLUMN manifest_json TEXT;
ALTER TABLE checkpoints ADD COLUMN manifest_hash TEXT;
