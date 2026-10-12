-- Migration 030: Replace Mission system with ExecutionRun canonical identity and persistence

CREATE TABLE IF NOT EXISTS execution_runs (
    id BLOB PRIMARY KEY,
    session_id BLOB REFERENCES sessions(id),
    objective TEXT NOT NULL,
    status TEXT NOT NULL,
    workspace_root TEXT NOT NULL DEFAULT '',
    mode TEXT NOT NULL DEFAULT 'safe',
    policy_context TEXT NOT NULL DEFAULT '{}',
    constraints TEXT NOT NULL DEFAULT '[]',
    requirements TEXT NOT NULL DEFAULT '[]',
    success_criteria TEXT NOT NULL DEFAULT '[]',
    started_at TEXT,
    completed_at TEXT,
    parent_run_id BLOB REFERENCES execution_runs(id),
    task_graph_id BLOB,
    last_applied_sequence INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_execution_runs_status ON execution_runs(status);
CREATE INDEX IF NOT EXISTS idx_execution_runs_session ON execution_runs(session_id);
CREATE INDEX IF NOT EXISTS idx_execution_runs_parent ON execution_runs(parent_run_id);

-- Migrate all existing data from missions to execution_runs
INSERT OR IGNORE INTO execution_runs (
    id, session_id, objective, status, workspace_root, mode,
    policy_context, constraints, requirements, success_criteria,
    started_at, completed_at, parent_run_id, task_graph_id,
    last_applied_sequence, created_at, updated_at
)
SELECT 
    m.id,
    s.id,
    m.objective,
    m.status,
    m.workspace_root,
    m.mode,
    m.policy_context,
    m.constraints,
    m.requirements,
    m.success_criteria,
    m.started_at,
    m.completed_at,
    m.parent_mission_id,
    m.task_graph_id,
    m.last_applied_sequence,
    m.created_at,
    m.updated_at
FROM missions m
LEFT JOIN sessions s ON s.active_mission_id = m.id OR s.mission_id = m.id;

-- Remove any transitional triggers to eliminate permanent dual-write architecture
DROP TRIGGER IF EXISTS trg_missions_to_execution_runs_insert;
DROP TRIGGER IF EXISTS trg_missions_to_execution_runs_update;
DROP TRIGGER IF EXISTS trg_execution_runs_to_missions_insert;
DROP TRIGGER IF EXISTS trg_execution_runs_to_missions_update;

-- 1. sessions
ALTER TABLE sessions ADD COLUMN active_run_id BLOB REFERENCES execution_runs(id);
UPDATE sessions SET active_run_id = COALESCE(active_mission_id, mission_id) WHERE active_run_id IS NULL;
CREATE INDEX IF NOT EXISTS idx_sessions_run_id ON sessions(active_run_id);
DROP INDEX IF EXISTS idx_sessions_mission_id;
ALTER TABLE sessions DROP COLUMN mission_id;
ALTER TABLE sessions ADD COLUMN mission_id BLOB;
UPDATE sessions SET mission_id = active_mission_id;
CREATE INDEX IF NOT EXISTS idx_sessions_mission_id ON sessions(mission_id);

-- 2. tasks
ALTER TABLE tasks ADD COLUMN run_id BLOB REFERENCES execution_runs(id);
UPDATE tasks SET run_id = mission_id WHERE run_id IS NULL;
CREATE INDEX IF NOT EXISTS idx_tasks_run_id ON tasks(run_id);

-- 3. task_graphs
ALTER TABLE task_graphs ADD COLUMN run_id BLOB REFERENCES execution_runs(id);
UPDATE task_graphs SET run_id = mission_id WHERE run_id IS NULL;
CREATE INDEX IF NOT EXISTS idx_task_graphs_run_id ON task_graphs(run_id);

-- 4. event_log
ALTER TABLE event_log ADD COLUMN run_id BLOB;
UPDATE event_log SET run_id = mission_id WHERE run_id IS NULL;
CREATE INDEX IF NOT EXISTS idx_event_log_run_seq ON event_log(run_id, sequence);

-- 5. agents
ALTER TABLE agents ADD COLUMN run_id BLOB REFERENCES execution_runs(id);
UPDATE agents SET run_id = mission_id WHERE run_id IS NULL;
CREATE INDEX IF NOT EXISTS idx_agents_run_id ON agents(run_id);

-- 6. agent_handoffs
ALTER TABLE agent_handoffs ADD COLUMN run_id BLOB REFERENCES execution_runs(id);
UPDATE agent_handoffs SET run_id = mission_id WHERE run_id IS NULL;

-- 7. agent_executions
ALTER TABLE agent_executions ADD COLUMN run_id BLOB REFERENCES execution_runs(id);
UPDATE agent_executions SET run_id = mission_id WHERE run_id IS NULL;

-- 8. resource_leases
ALTER TABLE resource_leases ADD COLUMN run_id BLOB REFERENCES execution_runs(id);
UPDATE resource_leases SET run_id = mission_id WHERE run_id IS NULL;

-- 9. model_invocations
ALTER TABLE model_invocations ADD COLUMN run_id BLOB REFERENCES execution_runs(id);
UPDATE model_invocations SET run_id = mission_id WHERE run_id IS NULL;

-- 10. repository_baselines
ALTER TABLE repository_baselines ADD COLUMN run_id BLOB REFERENCES execution_runs(id);
UPDATE repository_baselines SET run_id = mission_id WHERE run_id IS NULL;

-- 11. tool_executions
ALTER TABLE tool_executions ADD COLUMN run_id BLOB REFERENCES execution_runs(id);
UPDATE tool_executions SET run_id = mission_id WHERE run_id IS NULL;

-- 12. checkpoints
ALTER TABLE checkpoints ADD COLUMN run_id BLOB REFERENCES execution_runs(id);
UPDATE checkpoints SET run_id = mission_id WHERE run_id IS NULL;
CREATE INDEX IF NOT EXISTS idx_checkpoints_run_seq ON checkpoints(run_id, sequence);

-- 13. verification_results
ALTER TABLE verification_results ADD COLUMN run_id BLOB REFERENCES execution_runs(id);
UPDATE verification_results SET run_id = mission_id WHERE run_id IS NULL;

-- 14. policy_audit_logs
ALTER TABLE policy_audit_logs ADD COLUMN run_id BLOB REFERENCES execution_runs(id);
UPDATE policy_audit_logs SET run_id = mission_id WHERE run_id IS NULL;

-- 15. policy_decisions
ALTER TABLE policy_decisions ADD COLUMN run_id BLOB REFERENCES execution_runs(id);
UPDATE policy_decisions SET run_id = mission_id WHERE run_id IS NULL;

-- 16. approval_requests
ALTER TABLE approval_requests ADD COLUMN run_id BLOB REFERENCES execution_runs(id);
UPDATE approval_requests SET run_id = mission_id WHERE run_id IS NULL;

-- 17. policy_grants
ALTER TABLE policy_grants ADD COLUMN run_id BLOB REFERENCES execution_runs(id);
UPDATE policy_grants SET run_id = mission_id WHERE run_id IS NULL;

-- 18. jobs
ALTER TABLE jobs ADD COLUMN run_id BLOB REFERENCES execution_runs(id);
UPDATE jobs SET run_id = mission_id WHERE run_id IS NULL;

-- 19. verification_checks
ALTER TABLE verification_checks ADD COLUMN run_id BLOB REFERENCES execution_runs(id);
UPDATE verification_checks SET run_id = mission_id WHERE run_id IS NULL;

-- 20. completion_gate_decisions
ALTER TABLE completion_gate_decisions ADD COLUMN run_id BLOB REFERENCES execution_runs(id);
UPDATE completion_gate_decisions SET run_id = mission_id WHERE run_id IS NULL;

-- 21. recovery_attempts
ALTER TABLE recovery_attempts ADD COLUMN run_id BLOB REFERENCES execution_runs(id);
UPDATE recovery_attempts SET run_id = mission_id WHERE run_id IS NULL;

-- 22. git_commits
ALTER TABLE git_commits ADD COLUMN run_id TEXT;
UPDATE git_commits SET run_id = mission_id WHERE run_id IS NULL;

-- 23. telemetry_spans
ALTER TABLE telemetry_spans ADD COLUMN run_id TEXT;
UPDATE telemetry_spans SET run_id = mission_id WHERE run_id IS NULL;

-- 24. metric_samples
ALTER TABLE metric_samples ADD COLUMN run_id TEXT;
UPDATE metric_samples SET run_id = mission_id WHERE run_id IS NULL;

-- 25. completion_reports
ALTER TABLE completion_reports ADD COLUMN run_id TEXT;
UPDATE completion_reports SET run_id = mission_id WHERE run_id IS NULL;

-- 26. workflow_runs & workflow_step_runs
ALTER TABLE workflow_runs ADD COLUMN run_id BLOB REFERENCES execution_runs(id);
ALTER TABLE workflow_step_runs ADD COLUMN run_id BLOB REFERENCES execution_runs(id);
UPDATE workflow_step_runs SET run_id = mission_id WHERE run_id IS NULL;

-- 27. artifacts
ALTER TABLE artifacts ADD COLUMN run_id BLOB REFERENCES execution_runs(id);
UPDATE artifacts SET run_id = mission_id WHERE run_id IS NULL;
CREATE INDEX IF NOT EXISTS idx_artifacts_run ON artifacts(run_id);

-- 28. crash_recovery_scans
ALTER TABLE crash_recovery_scans ADD COLUMN run_id BLOB REFERENCES execution_runs(id);
UPDATE crash_recovery_scans SET run_id = mission_id WHERE run_id IS NULL;

-- 29. change_proposals
ALTER TABLE change_proposals ADD COLUMN run_id BLOB REFERENCES execution_runs(id);
UPDATE change_proposals SET run_id = mission_id WHERE run_id IS NULL;

-- 30. change_provenance
ALTER TABLE change_provenance ADD COLUMN run_id BLOB REFERENCES execution_runs(id);
UPDATE change_provenance SET run_id = mission_id WHERE run_id IS NULL;

-- 31. engineering_decisions
ALTER TABLE engineering_decisions ADD COLUMN run_id BLOB REFERENCES execution_runs(id);
UPDATE engineering_decisions SET run_id = mission_id WHERE run_id IS NULL;

-- 32. engineering_assumptions
ALTER TABLE engineering_assumptions ADD COLUMN run_id BLOB REFERENCES execution_runs(id);
UPDATE engineering_assumptions SET run_id = mission_id WHERE run_id IS NULL;

-- 33. failure_diagnoses
ALTER TABLE failure_diagnoses ADD COLUMN run_id BLOB REFERENCES execution_runs(id);
UPDATE failure_diagnoses SET run_id = mission_id WHERE run_id IS NULL;

-- 34. review_findings
ALTER TABLE review_findings ADD COLUMN run_id BLOB REFERENCES execution_runs(id);
UPDATE review_findings SET run_id = mission_id WHERE run_id IS NULL;

-- 35. verification_records
ALTER TABLE verification_records ADD COLUMN run_id BLOB REFERENCES execution_runs(id);
UPDATE verification_records SET run_id = mission_id WHERE run_id IS NULL;
