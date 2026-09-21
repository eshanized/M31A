-- Phase 32: Intent Understanding & Adaptive Task Formation
--
-- Persists the canonical IntentState for a session as a versioned JSON blob.
-- This allows sessions to resume with full intent context (goal, unknowns,
-- assumptions, decisions, formed tasks, research, steering) without
-- re-interpreting the user's request from scratch.
--
-- Design decisions:
--   1. JSON blob (not fully normalized): IntentState evolves frequently
--      during a phase; normalizing every sub-field would require excessive
--      schema churn. The blob is indexed by session_id + version for audit.
--   2. Additive append: intent_versions preserves all historical versions
--      for auditability. The current state is always max(version) per session.
--   3. Foreign key to sessions table for referential integrity.

-- Canonical current intent state per session (mutable, single row per session).
CREATE TABLE IF NOT EXISTS session_intent_state (
    session_id       BLOB NOT NULL PRIMARY KEY,
    version          INTEGER NOT NULL DEFAULT 0,
    raw_prompt       TEXT NOT NULL,
    intent_json      TEXT NOT NULL,
    created_at       TEXT NOT NULL,
    updated_at       TEXT NOT NULL,
    FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE
);

-- Immutable append-only log of every intent state version (audit trail).
CREATE TABLE IF NOT EXISTS intent_state_versions (
    id               BLOB NOT NULL PRIMARY KEY,
    session_id       BLOB NOT NULL,
    version          INTEGER NOT NULL,
    intent_json      TEXT NOT NULL,
    change_summary   TEXT,
    created_at       TEXT NOT NULL,
    FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_intent_state_versions_session
    ON intent_state_versions (session_id, version);

-- Consequential decisions that became real AskUser interactions (Decision Records).
-- Records the full lifecycle: question → options → user answer or evidence basis.
CREATE TABLE IF NOT EXISTS intent_decision_records (
    id                  BLOB NOT NULL PRIMARY KEY,
    session_id          BLOB NOT NULL,
    decision_id         TEXT NOT NULL,
    question            TEXT NOT NULL,
    options_json        TEXT NOT NULL DEFAULT '[]',
    consequence_rationale TEXT NOT NULL,
    resolution_type     TEXT,
    resolution_json     TEXT,
    created_at          TEXT NOT NULL,
    resolved_at         TEXT,
    FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_intent_decision_session
    ON intent_decision_records (session_id);
