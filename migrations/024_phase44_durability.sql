-- Migration 024: Phase 44 durability closure.
--
-- 1. tool_mutation_fence: durable deduplication for committed mutating tool
--    actions. Identity is semantic (task + tool + canonical input hash), never
--    timestamp-only. A recorded success suppresses duplicate side effects;
--    recorded failure still permits legitimate retry.
CREATE TABLE IF NOT EXISTS tool_mutation_fence (
    task_id BLOB NOT NULL,
    tool_name TEXT NOT NULL,
    fingerprint TEXT NOT NULL,
    outcome TEXT NOT NULL,
    output_hash TEXT,
    outcome_json TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    PRIMARY KEY (task_id, tool_name, fingerprint)
);

CREATE INDEX IF NOT EXISTS idx_mutation_fence_task ON tool_mutation_fence(task_id);

-- 2. budget_ledger: admission-critical budget consumption that must survive
--    restart. Updated on every settlement; hydrated into a fresh enforcer on
--    startup so a restart can never re-admit over-consumed budgets.
CREATE TABLE IF NOT EXISTS budget_ledger (
    mission_id BLOB PRIMARY KEY,
    consumed_tokens INTEGER NOT NULL DEFAULT 0,
    consumed_cost_microcents INTEGER NOT NULL DEFAULT 0,
    consumed_artifact_bytes INTEGER NOT NULL DEFAULT 0,
    steps_consumed INTEGER NOT NULL DEFAULT 0,
    calls_consumed INTEGER NOT NULL DEFAULT 0,
    retries_consumed INTEGER NOT NULL DEFAULT 0,
    updated_at TEXT NOT NULL
);

-- 3. recovery_attempts: durable mutation/attempt identity for duplicate-attempt
--    protection across restart (hydrates the in-memory tracker).
ALTER TABLE recovery_attempts ADD COLUMN mutation_fingerprint TEXT;
ALTER TABLE recovery_attempts ADD COLUMN semantic_signature TEXT;
