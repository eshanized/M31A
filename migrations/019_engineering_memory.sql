-- Migration 019: Long-Horizon Engineering Memory & Project State Continuity (Phase 25)
-- Canonical SQLite tables for engineering decisions, assumptions, failure diagnoses,
-- review findings, and revision-aware verification records.

-- 1. Architectural Decisions (Project & Mission scope)
CREATE TABLE IF NOT EXISTS engineering_decisions (
    id TEXT PRIMARY KEY NOT NULL,
    scope TEXT NOT NULL, -- 'project' or 'mission'
    mission_id BLOB REFERENCES missions(id),
    title TEXT NOT NULL,
    context TEXT NOT NULL,
    decision TEXT NOT NULL,
    rationale TEXT NOT NULL,
    alternatives_json TEXT NOT NULL DEFAULT '[]',
    consequences_json TEXT NOT NULL DEFAULT '[]',
    status TEXT NOT NULL, -- 'proposed', 'accepted', 'rejected', 'superseded', 'needs_operator_decision'
    superseded_by TEXT REFERENCES engineering_decisions(id),
    linked_requirements_json TEXT NOT NULL DEFAULT '[]',
    linked_files_json TEXT NOT NULL DEFAULT '[]',
    linked_symbols_json TEXT NOT NULL DEFAULT '[]',
    provenance_actor TEXT NOT NULL,
    provenance_reason TEXT,
    version INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_eng_decisions_scope ON engineering_decisions(scope);
CREATE INDEX IF NOT EXISTS idx_eng_decisions_status ON engineering_decisions(status);
CREATE INDEX IF NOT EXISTS idx_eng_decisions_mission ON engineering_decisions(mission_id);

-- 2. Engineering Assumptions & Lifecycle
CREATE TABLE IF NOT EXISTS engineering_assumptions (
    id BLOB PRIMARY KEY NOT NULL,
    scope TEXT NOT NULL, -- 'project' or 'mission'
    mission_id BLOB NOT NULL REFERENCES missions(id),
    task_id BLOB REFERENCES tasks(id),
    statement TEXT NOT NULL,
    status TEXT NOT NULL, -- 'active', 'tested', 'confirmed', 'invalidated'
    evidence_summary TEXT,
    target_file TEXT,
    target_symbol TEXT,
    expected_hash TEXT,
    invalidated_by TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_eng_assumptions_mission ON engineering_assumptions(mission_id);
CREATE INDEX IF NOT EXISTS idx_eng_assumptions_task ON engineering_assumptions(task_id);
CREATE INDEX IF NOT EXISTS idx_eng_assumptions_status ON engineering_assumptions(status);
CREATE INDEX IF NOT EXISTS idx_eng_assumptions_file ON engineering_assumptions(target_file);

-- 3. Failure Diagnoses & Repair Memory
CREATE TABLE IF NOT EXISTS failure_diagnoses (
    id BLOB PRIMARY KEY NOT NULL,
    mission_id BLOB NOT NULL REFERENCES missions(id),
    task_id BLOB NOT NULL REFERENCES tasks(id),
    failure_signature TEXT NOT NULL,
    failure_class TEXT NOT NULL,
    error_message TEXT NOT NULL,
    snapshot_hash TEXT NOT NULL,
    hypothesis TEXT NOT NULL,
    root_cause TEXT NOT NULL,
    recommended_action TEXT NOT NULL,
    repair_proposal_json TEXT,
    repair_status TEXT NOT NULL, -- 'proposed', 'applied', 'verified_success', 'verified_failure', 'abandoned'
    recurrence_count INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_failure_diagnoses_mission ON failure_diagnoses(mission_id);
CREATE INDEX IF NOT EXISTS idx_failure_diagnoses_task ON failure_diagnoses(task_id);
CREATE INDEX IF NOT EXISTS idx_failure_diagnoses_sig ON failure_diagnoses(failure_signature);
CREATE INDEX IF NOT EXISTS idx_failure_diagnoses_status ON failure_diagnoses(repair_status);

-- 4. Review Findings & Durable Status
CREATE TABLE IF NOT EXISTS review_findings (
    id BLOB PRIMARY KEY NOT NULL,
    mission_id BLOB NOT NULL REFERENCES missions(id),
    task_id BLOB NOT NULL REFERENCES tasks(id),
    check_id BLOB REFERENCES verification_checks(id),
    file_path TEXT NOT NULL,
    line_start INTEGER,
    line_end INTEGER,
    severity TEXT NOT NULL, -- 'info', 'warning', 'error', 'critical_security'
    description TEXT NOT NULL,
    recommendation TEXT NOT NULL,
    status TEXT NOT NULL, -- 'open', 'addressed', 'deferred', 'rejected_with_rationale', 'escalated'
    resolution_rationale TEXT,
    resolved_by TEXT,
    resolved_at TEXT,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_review_findings_mission ON review_findings(mission_id);
CREATE INDEX IF NOT EXISTS idx_review_findings_task ON review_findings(task_id);
CREATE INDEX IF NOT EXISTS idx_review_findings_status ON review_findings(status);
CREATE INDEX IF NOT EXISTS idx_review_findings_file ON review_findings(file_path);

-- 5. Revision-Aware Verification Records
CREATE TABLE IF NOT EXISTS verification_records (
    id BLOB PRIMARY KEY NOT NULL,
    mission_id BLOB NOT NULL REFERENCES missions(id),
    task_id BLOB NOT NULL REFERENCES tasks(id),
    requirement_key TEXT NOT NULL,
    check_id BLOB NOT NULL REFERENCES verification_checks(id),
    snapshot_hash TEXT NOT NULL,
    files_verified_json TEXT NOT NULL DEFAULT '[]',
    passed INTEGER NOT NULL,
    validity_status TEXT NOT NULL, -- 'valid', 'stale_due_to_drift', 'invalidated_by_mutation'
    invalidated_at TEXT,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_verification_records_mission ON verification_records(mission_id);
CREATE INDEX IF NOT EXISTS idx_verification_records_task ON verification_records(task_id);
CREATE INDEX IF NOT EXISTS idx_verification_records_req ON verification_records(requirement_key);
CREATE INDEX IF NOT EXISTS idx_verification_records_status ON verification_records(validity_status);
