-- Migration 032: Canonical ExecutionRun consolidation
--
-- Reconciles all legacy mission rows into execution_runs, ensures all child tables
-- populate canonical run_id, and consolidates sessions.active_run_id as authoritative.

INSERT OR IGNORE INTO execution_runs (
    id, session_id, objective, status, workspace_root, mode,
    policy_context, constraints, requirements, success_criteria,
    started_at, completed_at, parent_run_id, task_graph_id,
    last_applied_sequence, created_at, updated_at
)
SELECT 
    m.id,
    s.id,
    COALESCE(m.objective, 'Legacy execution'),
    COALESCE(m.status, 'completed'),
    COALESCE(m.workspace_root, ''),
    COALESCE(m.mode, 'safe'),
    COALESCE(m.policy_context, '{}'),
    COALESCE(m.constraints, '[]'),
    COALESCE(m.requirements, '[]'),
    COALESCE(m.success_criteria, '[]'),
    m.started_at,
    m.completed_at,
    m.parent_mission_id,
    m.task_graph_id,
    COALESCE(m.last_applied_sequence, 0),
    COALESCE(m.created_at, datetime('now')),
    COALESCE(m.updated_at, datetime('now'))
FROM missions m
LEFT JOIN sessions s ON s.active_mission_id = m.id OR s.active_run_id = m.id;

-- Reconcile sessions active_run_id
UPDATE sessions
SET active_run_id = COALESCE(active_run_id, active_mission_id, mission_id)
WHERE active_run_id IS NULL AND (active_mission_id IS NOT NULL OR mission_id IS NOT NULL);

-- Reconcile child tables with canonical run_id
UPDATE tasks SET run_id = mission_id WHERE run_id IS NULL AND mission_id IS NOT NULL;
UPDATE task_graphs SET run_id = mission_id WHERE run_id IS NULL AND mission_id IS NOT NULL;
UPDATE event_log SET run_id = mission_id WHERE run_id IS NULL AND mission_id IS NOT NULL;
UPDATE agents SET run_id = mission_id WHERE run_id IS NULL AND mission_id IS NOT NULL;
UPDATE agent_handoffs SET run_id = mission_id WHERE run_id IS NULL AND mission_id IS NOT NULL;
UPDATE agent_executions SET run_id = mission_id WHERE run_id IS NULL AND mission_id IS NOT NULL;
UPDATE resource_leases SET run_id = mission_id WHERE run_id IS NULL AND mission_id IS NOT NULL;
UPDATE model_invocations SET run_id = mission_id WHERE run_id IS NULL AND mission_id IS NOT NULL;
UPDATE repository_baselines SET run_id = mission_id WHERE run_id IS NULL AND mission_id IS NOT NULL;
UPDATE tool_executions SET run_id = mission_id WHERE run_id IS NULL AND mission_id IS NOT NULL;
UPDATE checkpoints SET run_id = mission_id WHERE run_id IS NULL AND mission_id IS NOT NULL;
UPDATE verification_results SET run_id = mission_id WHERE run_id IS NULL AND mission_id IS NOT NULL;
UPDATE artifacts SET run_id = mission_id WHERE run_id IS NULL AND mission_id IS NOT NULL;
UPDATE policy_audit_logs SET run_id = mission_id WHERE run_id IS NULL AND mission_id IS NOT NULL;
UPDATE policy_decisions SET run_id = mission_id WHERE run_id IS NULL AND mission_id IS NOT NULL;
UPDATE approval_requests SET run_id = mission_id WHERE run_id IS NULL AND mission_id IS NOT NULL;
UPDATE policy_grants SET run_id = mission_id WHERE run_id IS NULL AND mission_id IS NOT NULL;
UPDATE jobs SET run_id = mission_id WHERE run_id IS NULL AND mission_id IS NOT NULL;
UPDATE verification_checks SET run_id = mission_id WHERE run_id IS NULL AND mission_id IS NOT NULL;
UPDATE completion_gate_decisions SET run_id = mission_id WHERE run_id IS NULL AND mission_id IS NOT NULL;
UPDATE recovery_attempts SET run_id = mission_id WHERE run_id IS NULL AND mission_id IS NOT NULL;
UPDATE crash_recovery_scans SET run_id = mission_id WHERE run_id IS NULL AND mission_id IS NOT NULL;
UPDATE change_proposals SET run_id = mission_id WHERE run_id IS NULL AND mission_id IS NOT NULL;
UPDATE change_provenance SET run_id = mission_id WHERE run_id IS NULL AND mission_id IS NOT NULL;
UPDATE workflow_step_runs SET run_id = mission_id WHERE run_id IS NULL AND mission_id IS NOT NULL;
UPDATE engineering_decisions SET run_id = mission_id WHERE run_id IS NULL AND mission_id IS NOT NULL;
UPDATE engineering_assumptions SET run_id = mission_id WHERE run_id IS NULL AND mission_id IS NOT NULL;
UPDATE failure_diagnoses SET run_id = mission_id WHERE run_id IS NULL AND mission_id IS NOT NULL;
UPDATE review_findings SET run_id = mission_id WHERE run_id IS NULL AND mission_id IS NOT NULL;
UPDATE verification_records SET run_id = mission_id WHERE run_id IS NULL AND mission_id IS NOT NULL;
