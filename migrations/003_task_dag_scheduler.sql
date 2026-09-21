-- Migration 003: Task DAG, dependencies, and durable resource leases (D-01, D-03, D-13, DAG-01, DAG-04)

-- Authoritative task_graphs aggregate table
CREATE TABLE IF NOT EXISTS task_graphs (
    id BLOB PRIMARY KEY,
    mission_id BLOB NOT NULL REFERENCES missions(id),
    revision INTEGER NOT NULL,
    plan_id TEXT NOT NULL,
    status TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE (mission_id, revision)
);

CREATE INDEX IF NOT EXISTS idx_task_graphs_mission_id ON task_graphs(mission_id);

-- Expand tasks table with runtime scheduling columns
ALTER TABLE tasks ADD COLUMN task_graph_id BLOB REFERENCES task_graphs(id);
ALTER TABLE tasks ADD COLUMN candidate_key TEXT NOT NULL DEFAULT '';
ALTER TABLE tasks ADD COLUMN role TEXT NOT NULL DEFAULT 'implementer';
ALTER TABLE tasks ADD COLUMN priority INTEGER NOT NULL DEFAULT 100;
ALTER TABLE tasks ADD COLUMN max_retries INTEGER NOT NULL DEFAULT 3;
ALTER TABLE tasks ADD COLUMN retry_count INTEGER NOT NULL DEFAULT 0;
ALTER TABLE tasks ADD COLUMN capabilities TEXT NOT NULL DEFAULT '[]';
ALTER TABLE tasks ADD COLUMN verification TEXT NOT NULL DEFAULT '{}';
ALTER TABLE tasks ADD COLUMN estimates TEXT NOT NULL DEFAULT '{}';
ALTER TABLE tasks ADD COLUMN required_resources TEXT NOT NULL DEFAULT '[]';
ALTER TABLE tasks ADD COLUMN blocking_reason TEXT;
ALTER TABLE tasks ADD COLUMN fingerprint TEXT NOT NULL DEFAULT '';
ALTER TABLE tasks ADD COLUMN result TEXT;
ALTER TABLE tasks ADD COLUMN started_at TEXT;
ALTER TABLE tasks ADD COLUMN completed_at TEXT;

CREATE INDEX IF NOT EXISTS idx_tasks_task_graph_id ON tasks(task_graph_id);
CREATE INDEX IF NOT EXISTS idx_tasks_graph_candidate ON tasks(task_graph_id, candidate_key);

-- Authoritative task_dependencies DAG edges table
CREATE TABLE IF NOT EXISTS task_dependencies (
    task_graph_id BLOB NOT NULL REFERENCES task_graphs(id),
    prerequisite_task_id BLOB NOT NULL REFERENCES tasks(id),
    dependent_task_id BLOB NOT NULL REFERENCES tasks(id),
    kind TEXT NOT NULL DEFAULT 'hard_prerequisite',
    PRIMARY KEY (task_graph_id, prerequisite_task_id, dependent_task_id)
);

CREATE INDEX IF NOT EXISTS idx_task_deps_dependent ON task_dependencies(task_graph_id, dependent_task_id);
CREATE INDEX IF NOT EXISTS idx_task_deps_prerequisite ON task_dependencies(task_graph_id, prerequisite_task_id);

-- Durable resource_leases table for crash-recovery and concurrency audit
CREATE TABLE IF NOT EXISTS resource_leases (
    id BLOB PRIMARY KEY,
    task_id BLOB NOT NULL REFERENCES tasks(id),
    mission_id BLOB NOT NULL REFERENCES missions(id),
    resource_key TEXT NOT NULL,
    lock_mode TEXT NOT NULL,
    scope TEXT NOT NULL,
    acquired_at TEXT NOT NULL,
    owner_generation INTEGER NOT NULL,
    released_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_resource_leases_task_id ON resource_leases(task_id);
CREATE INDEX IF NOT EXISTS idx_resource_leases_resource_key ON resource_leases(resource_key);
CREATE INDEX IF NOT EXISTS idx_resource_leases_active ON resource_leases(resource_key, released_at);
