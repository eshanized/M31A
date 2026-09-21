-- Migration 018: Autonomous Code Modification & Change Authority (Phase 23, KRN-23, CHG-01..CHG-05)

CREATE TABLE IF NOT EXISTS change_proposals (
    id BLOB PRIMARY KEY,
    task_id BLOB NOT NULL,
    mission_id BLOB NOT NULL,
    state TEXT NOT NULL,
    intent_json TEXT NOT NULL,
    change_surface_json TEXT NOT NULL,
    preconditions_json TEXT NOT NULL,
    mutations_json TEXT NOT NULL,
    assumptions_json TEXT NOT NULL,
    risk_level TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_change_proposals_task ON change_proposals(task_id);
CREATE INDEX IF NOT EXISTS idx_change_proposals_mission ON change_proposals(mission_id);
CREATE INDEX IF NOT EXISTS idx_change_proposals_state ON change_proposals(state);

CREATE TABLE IF NOT EXISTS change_provenance (
    id BLOB PRIMARY KEY,
    proposal_id BLOB NOT NULL,
    task_id BLOB NOT NULL,
    mission_id BLOB NOT NULL,
    requirement_keys TEXT NOT NULL,
    affected_files TEXT NOT NULL,
    affected_symbols TEXT NOT NULL,
    pre_mutation_hash TEXT NOT NULL,
    post_mutation_hash TEXT NOT NULL,
    diff_summary TEXT NOT NULL,
    verification_passed INTEGER NOT NULL,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_change_provenance_proposal ON change_provenance(proposal_id);
CREATE INDEX IF NOT EXISTS idx_change_provenance_task ON change_provenance(task_id);
CREATE INDEX IF NOT EXISTS idx_change_provenance_mission ON change_provenance(mission_id);
