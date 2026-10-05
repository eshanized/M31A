-- Migration 027: full execution-surface binding on execution_authorizations.
--
-- An authorization granted for one policy generation, workspace, role, mode,
-- or time window must not verify for another. New columns default NULL so
-- pre-existing rows load; verification treats NULL bindings as stale (fail
-- closed) rather than reinterpreting legacy rows under the new contract.

ALTER TABLE execution_authorizations ADD COLUMN policy_hash TEXT;
ALTER TABLE execution_authorizations ADD COLUMN workspace_root TEXT;
ALTER TABLE execution_authorizations ADD COLUMN agent_role TEXT;
ALTER TABLE execution_authorizations ADD COLUMN autonomy_mode TEXT;
ALTER TABLE execution_authorizations ADD COLUMN expires_at TEXT;
