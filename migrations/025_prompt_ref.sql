-- PromptOS wiring remediation v0.1.1: persist the typed prompt execution
-- binding selected for each task so the workflow-selected prompt survives
-- materialization into the durable task record and reaches worker context.
-- Additive only: legacy rows read back with NULL (role-default resolution).

ALTER TABLE tasks ADD COLUMN prompt_ref_id TEXT;
ALTER TABLE tasks ADD COLUMN prompt_ref_version INTEGER;
