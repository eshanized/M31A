-- no-transaction
-- Migration 031: Retire legacy mission foreign key constraints
--
-- Drops legacy foreign keys referencing missions(id) across child tables
-- while preserving mission_id columns and data for backward compatibility.
-- TaskGraphs table is recreated to replace UNIQUE(mission_id, revision)
-- with UNIQUE(run_id, revision).

PRAGMA foreign_keys = OFF;

-- Drop existing indexes on legacy mission_id columns
DROP INDEX IF EXISTS idx_tasks_mission_id;
DROP INDEX IF EXISTS idx_agents_mission_id;
DROP INDEX IF EXISTS idx_event_log_mission_id;
DROP INDEX IF EXISTS idx_event_log_mission_seq;
DROP INDEX IF EXISTS idx_task_graphs_mission_id;
DROP INDEX IF EXISTS idx_resource_leases_mission_id;
DROP INDEX IF EXISTS idx_agent_executions_mission_id;
DROP INDEX IF EXISTS idx_agent_handoffs_mission_id;
DROP INDEX IF EXISTS idx_model_invocations_mission_id;
DROP INDEX IF EXISTS idx_repository_baselines_mission;
DROP INDEX IF EXISTS idx_repository_baselines_mission_id;
DROP INDEX IF EXISTS idx_tool_executions_mission_id;
DROP INDEX IF EXISTS idx_checkpoints_mission_id;
DROP INDEX IF EXISTS idx_checkpoints_mission_seq;
DROP INDEX IF EXISTS idx_verification_results_mission_id;
DROP INDEX IF EXISTS idx_artifacts_mission;
DROP INDEX IF EXISTS idx_artifacts_mission_id;
DROP INDEX IF EXISTS idx_policy_audit_logs_mission_id;
DROP INDEX IF EXISTS idx_policy_decisions_mission;
DROP INDEX IF EXISTS idx_policy_decisions_mission_id;
DROP INDEX IF EXISTS idx_approval_requests_mission_id;
DROP INDEX IF EXISTS idx_policy_grants_mission;
DROP INDEX IF EXISTS idx_policy_grants_mission_id;
DROP INDEX IF EXISTS idx_jobs_mission;
DROP INDEX IF EXISTS idx_jobs_mission_id;
DROP INDEX IF EXISTS idx_verification_checks_mission;
DROP INDEX IF EXISTS idx_verification_checks_mission_id;
DROP INDEX IF EXISTS idx_completion_gate_mission;
DROP INDEX IF EXISTS idx_completion_gate_decisions_mission_id;
DROP INDEX IF EXISTS idx_recovery_attempts_mission;
DROP INDEX IF EXISTS idx_recovery_attempts_mission_id;
DROP INDEX IF EXISTS idx_workflow_step_runs_mission_id;
DROP INDEX IF EXISTS idx_crash_recovery_scans_mission;
DROP INDEX IF EXISTS idx_crash_recovery_scans_mission_id;
DROP INDEX IF EXISTS idx_change_proposals_mission;
DROP INDEX IF EXISTS idx_change_proposals_mission_id;
DROP INDEX IF EXISTS idx_change_provenance_mission;
DROP INDEX IF EXISTS idx_change_provenance_mission_id;
DROP INDEX IF EXISTS idx_eng_decisions_mission;
DROP INDEX IF EXISTS idx_engineering_decisions_mission_id;
DROP INDEX IF EXISTS idx_eng_assumptions_mission;
DROP INDEX IF EXISTS idx_engineering_assumptions_mission_id;
DROP INDEX IF EXISTS idx_failure_diagnoses_mission;
DROP INDEX IF EXISTS idx_failure_diagnoses_mission_id;
DROP INDEX IF EXISTS idx_review_findings_mission;
DROP INDEX IF EXISTS idx_review_findings_mission_id;
DROP INDEX IF EXISTS idx_verification_records_mission;
DROP INDEX IF EXISTS idx_verification_records_mission_id;
DROP INDEX IF EXISTS idx_sessions_active_mission_id;

-- 1. task_graphs: Recreate table to drop REFERENCES missions(id) and update UNIQUE constraint
CREATE TABLE task_graphs_new (
    id BLOB PRIMARY KEY,
    mission_id BLOB,
    run_id BLOB REFERENCES execution_runs(id),
    revision INTEGER NOT NULL,
    plan_id TEXT NOT NULL,
    status TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE (run_id, revision)
);
INSERT INTO task_graphs_new (id, mission_id, run_id, revision, plan_id, status, created_at, updated_at)
SELECT id, mission_id, run_id, revision, plan_id, status, created_at, updated_at FROM task_graphs;
DROP TABLE task_graphs;
ALTER TABLE task_graphs_new RENAME TO task_graphs;
CREATE INDEX IF NOT EXISTS idx_task_graphs_run_id ON task_graphs(run_id);
CREATE INDEX IF NOT EXISTS idx_task_graphs_mission_id ON task_graphs(mission_id);

-- 2. tasks
ALTER TABLE tasks DROP COLUMN mission_id;
ALTER TABLE tasks ADD COLUMN mission_id BLOB;
UPDATE tasks SET mission_id = run_id;
CREATE INDEX IF NOT EXISTS idx_tasks_mission_id ON tasks(mission_id);

-- 3. agents
ALTER TABLE agents DROP COLUMN mission_id;
ALTER TABLE agents ADD COLUMN mission_id BLOB;
UPDATE agents SET mission_id = run_id;
CREATE INDEX IF NOT EXISTS idx_agents_mission_id ON agents(mission_id);

-- 4. event_log
ALTER TABLE event_log DROP COLUMN mission_id;
ALTER TABLE event_log ADD COLUMN mission_id BLOB;
UPDATE event_log SET mission_id = run_id;
CREATE INDEX IF NOT EXISTS idx_event_log_mission_id ON event_log(mission_id);
CREATE INDEX IF NOT EXISTS idx_event_log_mission_seq ON event_log(mission_id, sequence);

-- 5. resource_leases
ALTER TABLE resource_leases DROP COLUMN mission_id;
ALTER TABLE resource_leases ADD COLUMN mission_id BLOB;
UPDATE resource_leases SET mission_id = run_id;
CREATE INDEX IF NOT EXISTS idx_resource_leases_mission_id ON resource_leases(mission_id);

-- 6. agent_executions
ALTER TABLE agent_executions DROP COLUMN mission_id;
ALTER TABLE agent_executions ADD COLUMN mission_id BLOB;
UPDATE agent_executions SET mission_id = run_id;
CREATE INDEX IF NOT EXISTS idx_agent_executions_mission_id ON agent_executions(mission_id);

-- 7. agent_handoffs
ALTER TABLE agent_handoffs DROP COLUMN mission_id;
ALTER TABLE agent_handoffs ADD COLUMN mission_id BLOB;
UPDATE agent_handoffs SET mission_id = run_id;
CREATE INDEX IF NOT EXISTS idx_agent_handoffs_mission_id ON agent_handoffs(mission_id);

-- 8. model_invocations
ALTER TABLE model_invocations DROP COLUMN mission_id;
ALTER TABLE model_invocations ADD COLUMN mission_id BLOB;
UPDATE model_invocations SET mission_id = run_id;
CREATE INDEX IF NOT EXISTS idx_model_invocations_mission_id ON model_invocations(mission_id);

-- 9. repository_baselines
ALTER TABLE repository_baselines DROP COLUMN mission_id;
ALTER TABLE repository_baselines ADD COLUMN mission_id BLOB;
UPDATE repository_baselines SET mission_id = run_id;
CREATE INDEX IF NOT EXISTS idx_repository_baselines_mission_id ON repository_baselines(mission_id);

-- 10. tool_executions
ALTER TABLE tool_executions DROP COLUMN mission_id;
ALTER TABLE tool_executions ADD COLUMN mission_id BLOB;
UPDATE tool_executions SET mission_id = run_id;
CREATE INDEX IF NOT EXISTS idx_tool_executions_mission_id ON tool_executions(mission_id);

-- 11. checkpoints
ALTER TABLE checkpoints DROP COLUMN mission_id;
ALTER TABLE checkpoints ADD COLUMN mission_id BLOB;
UPDATE checkpoints SET mission_id = run_id;
CREATE INDEX IF NOT EXISTS idx_checkpoints_mission_id ON checkpoints(mission_id);
CREATE INDEX IF NOT EXISTS idx_checkpoints_mission_seq ON checkpoints(mission_id, sequence);

-- 12. verification_results
ALTER TABLE verification_results DROP COLUMN mission_id;
ALTER TABLE verification_results ADD COLUMN mission_id BLOB;
UPDATE verification_results SET mission_id = run_id;
CREATE INDEX IF NOT EXISTS idx_verification_results_mission_id ON verification_results(mission_id);

-- 13. artifacts
ALTER TABLE artifacts DROP COLUMN mission_id;
ALTER TABLE artifacts ADD COLUMN mission_id BLOB;
UPDATE artifacts SET mission_id = run_id;
CREATE INDEX IF NOT EXISTS idx_artifacts_mission_id ON artifacts(mission_id);

-- 14. policy_audit_logs
ALTER TABLE policy_audit_logs DROP COLUMN mission_id;
ALTER TABLE policy_audit_logs ADD COLUMN mission_id BLOB;
UPDATE policy_audit_logs SET mission_id = run_id;
CREATE INDEX IF NOT EXISTS idx_policy_audit_logs_mission_id ON policy_audit_logs(mission_id);

-- 15. policy_decisions
ALTER TABLE policy_decisions DROP COLUMN mission_id;
ALTER TABLE policy_decisions ADD COLUMN mission_id BLOB;
UPDATE policy_decisions SET mission_id = run_id;
CREATE INDEX IF NOT EXISTS idx_policy_decisions_mission_id ON policy_decisions(mission_id);

-- 16. approval_requests
ALTER TABLE approval_requests DROP COLUMN mission_id;
ALTER TABLE approval_requests ADD COLUMN mission_id BLOB;
UPDATE approval_requests SET mission_id = run_id;
CREATE INDEX IF NOT EXISTS idx_approval_requests_mission_id ON approval_requests(mission_id);

-- 17. policy_grants
ALTER TABLE policy_grants DROP COLUMN mission_id;
ALTER TABLE policy_grants ADD COLUMN mission_id BLOB;
UPDATE policy_grants SET mission_id = run_id;
CREATE INDEX IF NOT EXISTS idx_policy_grants_mission_id ON policy_grants(mission_id);

-- 18. jobs
ALTER TABLE jobs DROP COLUMN mission_id;
ALTER TABLE jobs ADD COLUMN mission_id BLOB;
UPDATE jobs SET mission_id = run_id;
CREATE INDEX IF NOT EXISTS idx_jobs_mission_id ON jobs(mission_id);

-- 19. verification_checks
ALTER TABLE verification_checks DROP COLUMN mission_id;
ALTER TABLE verification_checks ADD COLUMN mission_id BLOB;
UPDATE verification_checks SET mission_id = run_id;
CREATE INDEX IF NOT EXISTS idx_verification_checks_mission_id ON verification_checks(mission_id);

-- 20. completion_gate_decisions
ALTER TABLE completion_gate_decisions DROP COLUMN mission_id;
ALTER TABLE completion_gate_decisions ADD COLUMN mission_id BLOB;
UPDATE completion_gate_decisions SET mission_id = run_id;
CREATE INDEX IF NOT EXISTS idx_completion_gate_decisions_mission_id ON completion_gate_decisions(mission_id);

-- 21. recovery_attempts: Recreate table to drop REFERENCES missions(id) and make task_id nullable
CREATE TABLE recovery_attempts_new (
    id BLOB PRIMARY KEY,
    mission_id BLOB,
    run_id BLOB REFERENCES execution_runs(id),
    task_id BLOB REFERENCES tasks(id),
    failure_class TEXT NOT NULL,
    strategy TEXT NOT NULL,
    attempt_number INTEGER NOT NULL,
    budget_consumed INTEGER NOT NULL,
    remaining_class_budget INTEGER NOT NULL,
    remaining_overall_budget INTEGER NOT NULL,
    backoff_delay_ms INTEGER NOT NULL,
    action_taken TEXT NOT NULL,
    result TEXT NOT NULL,
    created_at TEXT NOT NULL,
    mutation_fingerprint TEXT,
    semantic_signature TEXT
);
INSERT INTO recovery_attempts_new (id, mission_id, run_id, task_id, failure_class, strategy, attempt_number, budget_consumed, remaining_class_budget, remaining_overall_budget, backoff_delay_ms, action_taken, result, created_at, mutation_fingerprint, semantic_signature)
SELECT id, COALESCE(run_id, mission_id), COALESCE(run_id, mission_id), task_id, failure_class, strategy, attempt_number, budget_consumed, remaining_class_budget, remaining_overall_budget, backoff_delay_ms, action_taken, result, created_at, mutation_fingerprint, semantic_signature FROM recovery_attempts;
DROP TABLE recovery_attempts;
ALTER TABLE recovery_attempts_new RENAME TO recovery_attempts;
CREATE INDEX IF NOT EXISTS idx_recovery_attempts_run_id ON recovery_attempts(run_id);
CREATE INDEX IF NOT EXISTS idx_recovery_attempts_mission_id ON recovery_attempts(mission_id);
CREATE INDEX IF NOT EXISTS idx_recovery_attempts_task_id ON recovery_attempts(task_id);
CREATE INDEX IF NOT EXISTS idx_recovery_attempts_class ON recovery_attempts(failure_class);

-- 22. workflow_step_runs
ALTER TABLE workflow_step_runs DROP COLUMN mission_id;
ALTER TABLE workflow_step_runs ADD COLUMN mission_id BLOB;
UPDATE workflow_step_runs SET mission_id = run_id;
CREATE INDEX IF NOT EXISTS idx_workflow_step_runs_mission_id ON workflow_step_runs(mission_id);

-- 23. crash_recovery_scans
ALTER TABLE crash_recovery_scans DROP COLUMN mission_id;
ALTER TABLE crash_recovery_scans ADD COLUMN mission_id BLOB;
UPDATE crash_recovery_scans SET mission_id = run_id;
CREATE INDEX IF NOT EXISTS idx_crash_recovery_scans_mission_id ON crash_recovery_scans(mission_id);

-- 24. change_proposals
ALTER TABLE change_proposals DROP COLUMN mission_id;
ALTER TABLE change_proposals ADD COLUMN mission_id BLOB;
UPDATE change_proposals SET mission_id = run_id;
CREATE INDEX IF NOT EXISTS idx_change_proposals_mission_id ON change_proposals(mission_id);

-- 25. change_provenance
ALTER TABLE change_provenance DROP COLUMN mission_id;
ALTER TABLE change_provenance ADD COLUMN mission_id BLOB;
UPDATE change_provenance SET mission_id = run_id;
CREATE INDEX IF NOT EXISTS idx_change_provenance_mission_id ON change_provenance(mission_id);

-- 26. engineering_decisions
ALTER TABLE engineering_decisions DROP COLUMN mission_id;
ALTER TABLE engineering_decisions ADD COLUMN mission_id BLOB;
UPDATE engineering_decisions SET mission_id = run_id;
CREATE INDEX IF NOT EXISTS idx_engineering_decisions_mission_id ON engineering_decisions(mission_id);

-- 27. engineering_assumptions
ALTER TABLE engineering_assumptions DROP COLUMN mission_id;
ALTER TABLE engineering_assumptions ADD COLUMN mission_id BLOB;
UPDATE engineering_assumptions SET mission_id = run_id;
CREATE INDEX IF NOT EXISTS idx_engineering_assumptions_mission_id ON engineering_assumptions(mission_id);

-- 28. failure_diagnoses
ALTER TABLE failure_diagnoses DROP COLUMN mission_id;
ALTER TABLE failure_diagnoses ADD COLUMN mission_id BLOB;
UPDATE failure_diagnoses SET mission_id = run_id;
CREATE INDEX IF NOT EXISTS idx_failure_diagnoses_mission_id ON failure_diagnoses(mission_id);

-- 29. review_findings
ALTER TABLE review_findings DROP COLUMN mission_id;
ALTER TABLE review_findings ADD COLUMN mission_id BLOB;
UPDATE review_findings SET mission_id = run_id;
CREATE INDEX IF NOT EXISTS idx_review_findings_mission_id ON review_findings(mission_id);

-- 30. verification_records
ALTER TABLE verification_records DROP COLUMN mission_id;
ALTER TABLE verification_records ADD COLUMN mission_id BLOB;
UPDATE verification_records SET mission_id = run_id;
CREATE INDEX IF NOT EXISTS idx_verification_records_mission_id ON verification_records(mission_id);

-- 31. sessions: drop active_mission_id foreign key constraint
ALTER TABLE sessions DROP COLUMN active_mission_id;
ALTER TABLE sessions ADD COLUMN active_mission_id BLOB;
UPDATE sessions SET active_mission_id = active_run_id;
CREATE INDEX IF NOT EXISTS idx_sessions_active_mission_id ON sessions(active_mission_id);

PRAGMA foreign_keys = ON;
