-- Migration 029: Completely remove budget system (tables, indexes, columns)

DROP INDEX IF EXISTS idx_budget_grants_mission;
DROP TABLE IF EXISTS budget_grants;
DROP INDEX IF EXISTS idx_budget_ledger_mission;
DROP TABLE IF EXISTS budget_ledger;
ALTER TABLE missions DROP COLUMN budget;
