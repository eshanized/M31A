# Maintenance Engine Storage Schema Specification

The Autonomous Repository Maintenance Engine persists its entire state in SQLite via Migration 21 (`autonomous_repository_maintenance`).

## 1. Relational Schema

```sql
-- Ingested signals
CREATE TABLE IF NOT EXISTS maintenance_signals (
    id TEXT PRIMARY KEY,
    repo_id TEXT NOT NULL,
    provider TEXT NOT NULL,
    category TEXT NOT NULL,
    provider_event_id TEXT,
    external_url TEXT,
    event_timestamp_ms INTEGER NOT NULL,
    observed_revision TEXT,
    severity TEXT NOT NULL,
    summary TEXT NOT NULL,
    raw_payload_json TEXT NOT NULL DEFAULT '{}',
    ingested_at_epoch_ms INTEGER NOT NULL,
    dedup_fingerprint TEXT NOT NULL,
    trust_status TEXT NOT NULL DEFAULT 'unverified'
);

-- Persistent maintenance cases
CREATE TABLE IF NOT EXISTS maintenance_cases (
    id TEXT PRIMARY KEY,
    repo_id TEXT NOT NULL,
    category TEXT NOT NULL,
    title TEXT NOT NULL,
    problem_statement TEXT NOT NULL,
    dedup_key TEXT NOT NULL,
    status TEXT NOT NULL,
    priority TEXT NOT NULL,
    severity TEXT NOT NULL,
    confidence_score REAL NOT NULL,
    affected_files_json TEXT NOT NULL DEFAULT '[]',
    affected_symbols_json TEXT NOT NULL DEFAULT '[]',
    evidence_references_json TEXT NOT NULL DEFAULT '[]',
    related_issue_id TEXT,
    related_pr_id TEXT,
    active_plan_id TEXT,
    workflow_id TEXT,
    remediation_history_json TEXT NOT NULL DEFAULT '[]',
    approval_state TEXT NOT NULL DEFAULT 'none',
    created_at_epoch_ms INTEGER NOT NULL,
    updated_at_epoch_ms INTEGER NOT NULL,
    resolved_at_epoch_ms INTEGER,
    last_observed_epoch_ms INTEGER NOT NULL
);

-- M:N association between signals and cases
CREATE TABLE IF NOT EXISTS maintenance_case_signals (
    case_id TEXT NOT NULL REFERENCES maintenance_cases(id) ON DELETE CASCADE,
    signal_id TEXT NOT NULL REFERENCES maintenance_signals(id) ON DELETE CASCADE,
    PRIMARY KEY (case_id, signal_id)
);

-- Evidence-based maintenance plans
CREATE TABLE IF NOT EXISTS maintenance_plans (
    id TEXT PRIMARY KEY,
    case_id TEXT NOT NULL REFERENCES maintenance_cases(id) ON DELETE CASCADE,
    repo_id TEXT NOT NULL,
    baseline_sha TEXT NOT NULL,
    category TEXT NOT NULL,
    problem_classification TEXT NOT NULL,
    selected_strategy TEXT NOT NULL,
    hypotheses_json TEXT NOT NULL DEFAULT '[]',
    expected_files_json TEXT NOT NULL DEFAULT '[]',
    expected_packages_json TEXT NOT NULL DEFAULT '[]',
    relevant_tests_json TEXT NOT NULL DEFAULT '[]',
    verification_requirements_json TEXT NOT NULL DEFAULT '[]',
    estimated_risk TEXT NOT NULL,
    required_autonomy_tier TEXT NOT NULL,
    requires_approval INTEGER NOT NULL DEFAULT 0,
    approval_state TEXT NOT NULL DEFAULT 'pending',
    workflow_id TEXT,
    plan_hash TEXT NOT NULL,
    created_at_epoch_ms INTEGER NOT NULL,
    updated_at_epoch_ms INTEGER NOT NULL
);

-- Authoritative execution attempts
CREATE TABLE IF NOT EXISTS maintenance_executions (
    id TEXT PRIMARY KEY,
    case_id TEXT NOT NULL REFERENCES maintenance_cases(id) ON DELETE CASCADE,
    workflow_id TEXT NOT NULL REFERENCES workflows(id) ON DELETE CASCADE,
    plan_id TEXT NOT NULL REFERENCES maintenance_plans(id) ON DELETE CASCADE,
    manifest_hash TEXT,
    attempt_id INTEGER NOT NULL DEFAULT 1,
    worker_id TEXT NOT NULL,
    execution_state TEXT NOT NULL,
    verification_report_hash TEXT,
    promotion_state TEXT NOT NULL DEFAULT 'none',
    pr_number INTEGER,
    pr_url TEXT,
    failure_classification TEXT,
    duration_ms INTEGER NOT NULL DEFAULT 0,
    tokens_used INTEGER NOT NULL DEFAULT 0,
    created_at_epoch_ms INTEGER NOT NULL,
    completed_at_epoch_ms INTEGER
);

-- Recurring schedules
CREATE TABLE IF NOT EXISTS maintenance_schedules (
    id TEXT PRIMARY KEY,
    repo_id TEXT NOT NULL,
    job_kind TEXT NOT NULL,
    cron_or_interval_secs INTEGER NOT NULL,
    is_enabled INTEGER NOT NULL DEFAULT 1,
    last_run_epoch_ms INTEGER,
    next_run_epoch_ms INTEGER NOT NULL,
    consecutive_failures INTEGER NOT NULL DEFAULT 0,
    policy_override_json TEXT,
    created_at_epoch_ms INTEGER NOT NULL,
    updated_at_epoch_ms INTEGER NOT NULL
);

-- Tamper-evident audit trail
CREATE TABLE IF NOT EXISTS maintenance_audit_trail (
    id TEXT PRIMARY KEY,
    case_id TEXT REFERENCES maintenance_cases(id) ON DELETE SET NULL,
    repo_id TEXT NOT NULL,
    event_type TEXT NOT NULL,
    actor TEXT NOT NULL,
    policy_version TEXT NOT NULL,
    details_json TEXT NOT NULL DEFAULT '{}',
    timestamp_epoch_ms INTEGER NOT NULL
);
```

## 2. Indexes

- `idx_maint_signals_fingerprint` on `maintenance_signals(dedup_fingerprint)`: Constant-time dedup checks.
- `idx_maint_signals_repo` on `maintenance_signals(repo_id, event_timestamp_ms DESC)`: Fast repo signal retrieval.
- `idx_maint_cases_repo` on `maintenance_cases(repo_id, status)`: Fast filtered queries by case status.
- `idx_maint_cases_dedup` on `maintenance_cases(repo_id, dedup_key)`: Grouping incoming signals into open cases.
- `idx_maint_cases_workflow` on `maintenance_cases(workflow_id)`: Joins between cases and workflows.
- `idx_maint_plans_case` on `maintenance_plans(case_id)`: Active plan retrieval.
- `idx_maint_exec_case` on `maintenance_executions(case_id)`: Execution attempt history lookup.
- `idx_maint_sched_due` on `maintenance_schedules(is_enabled, next_run_epoch_ms)`: High-efficiency scheduler polling.
