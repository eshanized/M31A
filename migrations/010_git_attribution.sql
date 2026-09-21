-- Migration 010: Git Commit Attribution Records (GST-03, D-03)

CREATE TABLE IF NOT EXISTS git_commits (
    commit_hash TEXT PRIMARY KEY NOT NULL,
    mission_id TEXT NOT NULL,
    task_id TEXT NOT NULL,
    agent_role TEXT NOT NULL,
    model_id TEXT NOT NULL,
    verification_run_id TEXT,
    created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_git_commits_mission ON git_commits (mission_id);
CREATE INDEX IF NOT EXISTS idx_git_commits_task ON git_commits (task_id);
