-- Phase 35: Human-Governed Planning & Pre-Execution Lifecycle
--
-- Persists durable lifecycle state, versioned plan revisions, versioned task sets,
-- dynamic discovery questions/answers, and cryptographic/audit-bound execution authorizations.
--
-- Principle: "The model proposes. The runtime decides."
--
-- 1. session_lifecycle_state: Current active lifecycle stage per session.
-- 2. plan_revisions: Immutable revision history of proposed, edited, and accepted plans.
-- 3. task_revisions: Immutable revision history of executable candidate task graphs.
-- 4. discovery_questions: Dynamic uncertainty-driven questions with provenance-aware answers.
-- 5. execution_authorizations: Explicit launch authorization records bound to exact plan/task revisions.

-- Current lifecycle state per session
CREATE TABLE IF NOT EXISTS session_lifecycle_state (
    session_id          BLOB NOT NULL PRIMARY KEY,
    stage               TEXT NOT NULL,
    plan_revision       INTEGER NOT NULL DEFAULT 0,
    task_revision       INTEGER NOT NULL DEFAULT 0,
    authorization_id    BLOB,
    created_at          TEXT NOT NULL,
    updated_at          TEXT NOT NULL,
    FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_lifecycle_stage
    ON session_lifecycle_state (stage);

-- Immutable history of plan revisions (plan.json content is authoritative)
CREATE TABLE IF NOT EXISTS plan_revisions (
    id                  BLOB NOT NULL PRIMARY KEY,
    session_id          BLOB NOT NULL,
    revision            INTEGER NOT NULL,
    plan_id             TEXT NOT NULL,
    content_json        TEXT NOT NULL,
    created_by          TEXT NOT NULL,
    author_type         TEXT NOT NULL,
    supersedes_revision INTEGER,
    status              TEXT NOT NULL,
    created_at          TEXT NOT NULL,
    FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE,
    UNIQUE (session_id, revision)
);

CREATE INDEX IF NOT EXISTS idx_plan_revisions_lookup
    ON plan_revisions (session_id, revision);

-- Immutable history of task revisions (tasks.json content is authoritative)
CREATE TABLE IF NOT EXISTS task_revisions (
    id                  BLOB NOT NULL PRIMARY KEY,
    session_id          BLOB NOT NULL,
    revision            INTEGER NOT NULL,
    plan_revision       INTEGER NOT NULL,
    tasks_json          TEXT NOT NULL,
    created_by          TEXT NOT NULL,
    author_type         TEXT NOT NULL,
    supersedes_revision INTEGER,
    status              TEXT NOT NULL,
    created_at          TEXT NOT NULL,
    FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE,
    UNIQUE (session_id, revision)
);

CREATE INDEX IF NOT EXISTS idx_task_revisions_lookup
    ON task_revisions (session_id, revision);

-- Dynamic discovery questions derived from uncertainty and unresolved unknowns
CREATE TABLE IF NOT EXISTS discovery_questions (
    id                  BLOB NOT NULL PRIMARY KEY,
    session_id          BLOB NOT NULL,
    question_id         TEXT NOT NULL,
    target_unknown      TEXT NOT NULL,
    reason              TEXT NOT NULL,
    text                TEXT NOT NULL,
    options_json        TEXT NOT NULL DEFAULT '[]',
    allow_freeform      INTEGER NOT NULL DEFAULT 1,
    blocking            INTEGER NOT NULL DEFAULT 1,
    status              TEXT NOT NULL DEFAULT 'pending',
    answer              TEXT,
    answered_by         TEXT,
    answered_at         TEXT,
    created_at          TEXT NOT NULL,
    FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_discovery_questions_session
    ON discovery_questions (session_id, question_id);

-- Execution launch authorizations bound to specific plan & task revisions
CREATE TABLE IF NOT EXISTS execution_authorizations (
    id                  BLOB NOT NULL PRIMARY KEY,
    session_id          BLOB NOT NULL,
    plan_revision       INTEGER NOT NULL,
    task_revision       INTEGER NOT NULL,
    decision            TEXT NOT NULL,
    authorized_by       TEXT NOT NULL,
    authorized_at       TEXT NOT NULL,
    invalidation_reason TEXT,
    FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_authorizations_session
    ON execution_authorizations (session_id, plan_revision, task_revision);
