-- Phase 36.3: Exact artifact content hashes on execution_authorizations (INVARIANT C)
ALTER TABLE execution_authorizations ADD COLUMN plan_content_hash TEXT;
ALTER TABLE execution_authorizations ADD COLUMN task_content_hash TEXT;
