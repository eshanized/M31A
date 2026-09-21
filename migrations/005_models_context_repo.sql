-- Migration 005: Model invocations, repository cache, and baseline drift detection (MDL-05, REP-02, REP-05)

CREATE TABLE IF NOT EXISTS model_invocations (
    id BLOB PRIMARY KEY,
    mission_id BLOB NOT NULL REFERENCES missions(id),
    task_id BLOB NOT NULL REFERENCES tasks(id),
    agent_id BLOB NOT NULL REFERENCES agents(id),
    step_number INTEGER NOT NULL,
    provider TEXT NOT NULL,
    model_name TEXT NOT NULL,
    attempt_number INTEGER NOT NULL DEFAULT 1,
    outcome TEXT NOT NULL,
    prompt_tokens INTEGER NOT NULL DEFAULT 0,
    completion_tokens INTEGER NOT NULL DEFAULT 0,
    total_tokens INTEGER NOT NULL DEFAULT 0,
    usage_source TEXT NOT NULL,
    routing_reason TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_model_invocations_task_id ON model_invocations(task_id);
CREATE INDEX IF NOT EXISTS idx_model_invocations_mission_id ON model_invocations(mission_id);

CREATE TABLE IF NOT EXISTS repo_cache_files (
    file_path TEXT PRIMARY KEY,
    content_hash TEXT NOT NULL,
    language TEXT NOT NULL,
    last_indexed_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS repo_cache_symbols (
    id TEXT PRIMARY KEY,
    file_path TEXT NOT NULL REFERENCES repo_cache_files(file_path) ON DELETE CASCADE,
    name TEXT NOT NULL,
    qualified_name TEXT NOT NULL,
    kind TEXT NOT NULL,
    start_line INTEGER NOT NULL,
    end_line INTEGER NOT NULL,
    signature TEXT NOT NULL,
    fact_class TEXT NOT NULL,
    doc_comment TEXT
);

CREATE INDEX IF NOT EXISTS idx_repo_cache_symbols_file ON repo_cache_symbols(file_path);
CREATE INDEX IF NOT EXISTS idx_repo_cache_symbols_name ON repo_cache_symbols(name);

CREATE TABLE IF NOT EXISTS repo_cache_edges (
    source_symbol_id TEXT NOT NULL,
    target_symbol_id TEXT NOT NULL,
    edge_kind TEXT NOT NULL,
    PRIMARY KEY (source_symbol_id, target_symbol_id, edge_kind)
);

CREATE TABLE IF NOT EXISTS repository_baselines (
    id BLOB PRIMARY KEY,
    mission_id BLOB NOT NULL REFERENCES missions(id),
    task_id BLOB REFERENCES tasks(id),
    git_commit TEXT,
    fingerprint_map TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_repository_baselines_mission ON repository_baselines(mission_id);
