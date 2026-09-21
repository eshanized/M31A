-- Migration 016: Crash recovery scans table (AD-011, D-14)
CREATE TABLE IF NOT EXISTS crash_recovery_scans (
    id BLOB PRIMARY KEY,
    mission_id BLOB NOT NULL REFERENCES missions(id),
    classification TEXT NOT NULL,
    checkpoint_id BLOB,
    reconciled_jobs_count INTEGER NOT NULL,
    killed_process_groups_count INTEGER NOT NULL,
    sealed_spools_count INTEGER NOT NULL,
    explanation TEXT NOT NULL,
    scanned_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_crash_recovery_scans_mission ON crash_recovery_scans(mission_id);
