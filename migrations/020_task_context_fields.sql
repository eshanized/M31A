-- Phase 29 (Gap 1): persist rich task context so worker agents receive
-- target-specific work descriptions, completion criteria, requirement
-- traceability, and assumptions instead of title-strings alone.
-- Additive only: existing rows read back with empty defaults.

ALTER TABLE tasks ADD COLUMN description TEXT;
ALTER TABLE tasks ADD COLUMN completion_criteria TEXT NOT NULL DEFAULT '[]';
ALTER TABLE tasks ADD COLUMN requirement_keys TEXT NOT NULL DEFAULT '[]';
ALTER TABLE tasks ADD COLUMN assumptions TEXT NOT NULL DEFAULT '[]';
