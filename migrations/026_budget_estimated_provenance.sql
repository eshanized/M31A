-- Migration 026: budget ledger estimated-consumption provenance.
--
-- Estimated (non-authoritative) usage must survive restart exactly like
-- authoritative usage, or a crash would silently reset estimated spend and
-- re-admit over-limit work. New columns default to 0 so pre-existing rows
-- hydrate as fully-authoritative (their provenance predates the split).

ALTER TABLE budget_ledger ADD COLUMN estimated_tokens INTEGER NOT NULL DEFAULT 0;
ALTER TABLE budget_ledger ADD COLUMN estimated_cost_microcents INTEGER NOT NULL DEFAULT 0;
