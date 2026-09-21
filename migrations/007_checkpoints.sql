-- Migration 007: Checkpoints, verification results, and policy audit logs (PST-01, PST-06, AUT-01)

CREATE TABLE IF NOT EXISTS checkpoints (
    id BLOB PRIMARY KEY,
    mission_id BLOB NOT NULL REFERENCES missions(id),
    sequence INTEGER NOT NULL,
    stage TEXT NOT NULL,
    cycle INTEGER NOT NULL,
    state_summary TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_checkpoints_mission_id ON checkpoints(mission_id);
CREATE INDEX IF NOT EXISTS idx_checkpoints_mission_seq ON checkpoints(mission_id, sequence);

CREATE TABLE IF NOT EXISTS verification_results (
    id BLOB PRIMARY KEY,
    mission_id BLOB NOT NULL REFERENCES missions(id),
    task_id BLOB REFERENCES tasks(id),
    strategy TEXT NOT NULL,
    passed INTEGER NOT NULL,
    details TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_verification_results_mission_id ON verification_results(mission_id);

CREATE TABLE IF NOT EXISTS policy_audit_logs (
    id BLOB PRIMARY KEY,
    mission_id BLOB NOT NULL REFERENCES missions(id),
    task_id BLOB REFERENCES tasks(id),
    agent_id BLOB REFERENCES agents(id),
    decision TEXT NOT NULL,
    reason TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_policy_audit_logs_mission_id ON policy_audit_logs(mission_id);
