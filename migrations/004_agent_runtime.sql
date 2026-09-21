-- Migration 004: Agent runtime persistence, handoffs, and execution records (AGT-05, AGT-06, D-14)

-- Table for AGT-06 durable agent handoffs
CREATE TABLE IF NOT EXISTS agent_handoffs (
    id BLOB PRIMARY KEY,
    mission_id BLOB NOT NULL REFERENCES missions(id),
    source_task_id BLOB NOT NULL REFERENCES tasks(id),
    source_agent_id BLOB NOT NULL REFERENCES agents(id),
    source_role TEXT NOT NULL,
    target_task_id BLOB REFERENCES tasks(id),
    target_role TEXT NOT NULL,
    target_agent_id BLOB REFERENCES agents(id),
    reason TEXT NOT NULL,
    required_inputs TEXT NOT NULL DEFAULT '[]',
    artifacts TEXT NOT NULL DEFAULT '[]',
    unresolved_questions TEXT NOT NULL DEFAULT '[]',
    task_state TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_agent_handoffs_mission_id ON agent_handoffs(mission_id);
CREATE INDEX IF NOT EXISTS idx_agent_handoffs_source_task_id ON agent_handoffs(source_task_id);
CREATE INDEX IF NOT EXISTS idx_agent_handoffs_target_task_id ON agent_handoffs(target_task_id);

-- Alter table agents adding runtime fields
ALTER TABLE agents ADD COLUMN task_id BLOB REFERENCES tasks(id);
ALTER TABLE agents ADD COLUMN profile_fingerprint TEXT NOT NULL DEFAULT '';
ALTER TABLE agents ADD COLUMN max_steps INTEGER NOT NULL DEFAULT 50;
ALTER TABLE agents ADD COLUMN steps_consumed INTEGER NOT NULL DEFAULT 0;
ALTER TABLE agents ADD COLUMN model_name TEXT NOT NULL DEFAULT '';
ALTER TABLE agents ADD COLUMN completed_at TEXT;

-- Table for supervised agent execution history
CREATE TABLE IF NOT EXISTS agent_executions (
    id BLOB PRIMARY KEY,
    agent_id BLOB NOT NULL REFERENCES agents(id),
    task_id BLOB NOT NULL REFERENCES tasks(id),
    mission_id BLOB NOT NULL REFERENCES missions(id),
    generation INTEGER NOT NULL DEFAULT 1,
    status TEXT NOT NULL,
    steps_consumed INTEGER NOT NULL DEFAULT 0,
    model_name TEXT NOT NULL,
    outcome TEXT,
    started_at TEXT NOT NULL,
    completed_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_agent_executions_task_id ON agent_executions(task_id);
CREATE INDEX IF NOT EXISTS idx_agent_executions_agent_id ON agent_executions(agent_id);
