//! Versioned SQLite migration engine and schema registry.
//!
//! Provides transactional forward schema migrations, tracks applied versions in
//! `_m31a_schema_migrations` with SHA-256 checksum validation, and guarantees
//! clean database upgrades across M31A storage releases.

use anyhow::{Context, Result};
use chrono::Utc;
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// A single versioned migration step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Migration {
    pub version: u32,
    pub name: &'static str,
    pub sql: &'static str,
}

impl Migration {
    pub const fn new(version: u32, name: &'static str, sql: &'static str) -> Self {
        Self { version, name, sql }
    }

    /// Compute SHA-256 checksum of the migration SQL string.
    pub fn checksum(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(self.sql.as_bytes());
        format!("{:x}", hasher.finalize())
    }
}

/// Record of an applied migration in SQLite.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppliedMigrationRecord {
    pub version: u32,
    pub name: String,
    pub applied_at_epoch_ms: i64,
    pub checksum: String,
}

/// The built-in forward schema migrations catalog for M31A SQLite state storage.
pub const BUILTIN_MIGRATIONS: &[Migration] = &[
    Migration::new(
        1,
        "initial_core_schema",
        r#"
        CREATE TABLE IF NOT EXISTS threads (
            id TEXT PRIMARY KEY,
            rollout_path TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            source TEXT NOT NULL,
            model_provider TEXT NOT NULL,
            cwd TEXT NOT NULL,
            title TEXT NOT NULL,
            sandbox_policy TEXT NOT NULL,
            approval_mode TEXT NOT NULL,
            is_archived INTEGER NOT NULL DEFAULT 0,
            parent_thread_id TEXT,
            section_id TEXT
        );

        CREATE TABLE IF NOT EXISTS thread_turns (
            id TEXT PRIMARY KEY,
            thread_id TEXT NOT NULL REFERENCES threads(id) ON DELETE CASCADE,
            turn_index INTEGER NOT NULL,
            status TEXT NOT NULL,
            created_at INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS thread_items (
            id TEXT PRIMARY KEY,
            thread_id TEXT NOT NULL REFERENCES threads(id) ON DELETE CASCADE,
            turn_id TEXT,
            item_type TEXT NOT NULL,
            content TEXT NOT NULL,
            created_at INTEGER NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_thread_turns_thread ON thread_turns(thread_id);
        CREATE INDEX IF NOT EXISTS idx_thread_items_thread ON thread_items(thread_id);
        CREATE INDEX IF NOT EXISTS idx_threads_updated_at ON threads(updated_at);
        "#,
    ),
    Migration::new(
        2,
        "goals_and_checkpoints",
        r#"
        CREATE TABLE IF NOT EXISTS thread_goals (
            thread_id TEXT PRIMARY KEY REFERENCES threads(id) ON DELETE CASCADE,
            goal_id TEXT NOT NULL,
            objective TEXT NOT NULL,
            status TEXT NOT NULL CHECK(status IN ('active', 'paused', 'blocked', 'budget_limited', 'complete')),
            token_budget INTEGER,
            tokens_used INTEGER NOT NULL DEFAULT 0,
            time_used_seconds INTEGER NOT NULL DEFAULT 0,
            created_at_ms INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS checkpoints (
            id TEXT PRIMARY KEY,
            thread_id TEXT NOT NULL,
            label TEXT NOT NULL,
            git_commit TEXT,
            turn_index INTEGER NOT NULL,
            modified_files TEXT NOT NULL,
            metadata TEXT,
            created_at INTEGER NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_checkpoints_thread_created ON checkpoints(thread_id, created_at DESC);
        "#,
    ),
    Migration::new(
        3,
        "agent_graph_edges",
        r#"
        CREATE TABLE IF NOT EXISTS thread_spawn_edges (
            parent_thread_id TEXT NOT NULL,
            child_thread_id TEXT PRIMARY KEY,
            agent_role TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'open',
            created_at INTEGER NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_thread_spawn_edges_parent ON thread_spawn_edges(parent_thread_id);
        CREATE INDEX IF NOT EXISTS idx_thread_spawn_edges_parent_status ON thread_spawn_edges(parent_thread_id, status);
        "#,
    ),
    Migration::new(
        4,
        "collaboration_message_board",
        r#"
        CREATE TABLE IF NOT EXISTS channels (
            name TEXT PRIMARY KEY NOT NULL,
            topic TEXT NOT NULL,
            created_at INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS subscriptions (
            board TEXT NOT NULL,
            channel TEXT NOT NULL,
            subscriber TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            PRIMARY KEY (board, channel, subscriber)
        );

        CREATE TABLE IF NOT EXISTS posts (
            seq INTEGER PRIMARY KEY AUTOINCREMENT,
            board TEXT NOT NULL,
            id TEXT NOT NULL,
            channel TEXT NOT NULL,
            root TEXT NOT NULL,
            author TEXT NOT NULL,
            timestamp INTEGER NOT NULL,
            body_search TEXT NOT NULL,
            payload TEXT NOT NULL,
            request_id TEXT NOT NULL,
            request TEXT NOT NULL,
            UNIQUE(board, id),
            UNIQUE(board, request_id)
        );

        CREATE INDEX IF NOT EXISTS posts_board_channel ON posts(board, channel, seq);
        CREATE INDEX IF NOT EXISTS posts_board_channel_timestamp ON posts(board, channel, timestamp, seq);
        "#,
    ),
    Migration::new(
        5,
        "thread_sections",
        r#"
        CREATE TABLE IF NOT EXISTS thread_sections (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            color TEXT,
            icon TEXT,
            created_at INTEGER NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_threads_section_id ON threads(section_id);
        "#,
    ),
    Migration::new(
        6,
        "notes_and_history_recovery",
        r#"
        CREATE TABLE IF NOT EXISTS rollout_notes (
            agent_id TEXT NOT NULL,
            virtual_path TEXT NOT NULL,
            content TEXT NOT NULL,
            size_bytes INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            PRIMARY KEY (agent_id, virtual_path)
        );
        CREATE INDEX IF NOT EXISTS idx_rollout_notes_prefix ON rollout_notes(agent_id, virtual_path);

        CREATE TABLE IF NOT EXISTS history_windows (
            window_id TEXT PRIMARY KEY,
            session_id TEXT NOT NULL,
            agent_name TEXT NOT NULL,
            created_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS history_items (
            item_id TEXT PRIMARY KEY,
            window_id TEXT NOT NULL,
            session_id TEXT NOT NULL,
            agent_name TEXT NOT NULL,
            role TEXT NOT NULL,
            tool_namespace TEXT,
            tool_name TEXT,
            content TEXT NOT NULL,
            ordinal INTEGER NOT NULL,
            created_at TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_hist_win_session ON history_windows(session_id, agent_name);
        CREATE INDEX IF NOT EXISTS idx_hist_item_win ON history_items(window_id, ordinal);
        CREATE INDEX IF NOT EXISTS idx_hist_item_session ON history_items(session_id, agent_name);
        "#,
    ),
    Migration::new(
        7,
        "audit_logs_and_task_queue",
        r#"
        CREATE TABLE IF NOT EXISTS audit_logs (
            id TEXT PRIMARY KEY,
            timestamp_epoch_ms INTEGER NOT NULL,
            level TEXT NOT NULL,
            target_crate TEXT NOT NULL,
            thread_id TEXT,
            message TEXT NOT NULL,
            metadata_json TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS task_queue (
            id TEXT PRIMARY KEY,
            thread_id TEXT NOT NULL,
            priority INTEGER NOT NULL DEFAULT 0,
            task_kind TEXT NOT NULL,
            payload_json TEXT NOT NULL,
            status TEXT NOT NULL,
            retries INTEGER NOT NULL DEFAULT 0,
            max_retries INTEGER NOT NULL DEFAULT 3,
            error_message TEXT,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_audit_timestamp ON audit_logs(timestamp_epoch_ms);
        CREATE INDEX IF NOT EXISTS idx_audit_thread ON audit_logs(thread_id);
        CREATE INDEX IF NOT EXISTS idx_queue_status_prio ON task_queue(status, priority DESC, created_at ASC);
        "#,
    ),
    Migration::new(
        8,
        "retained_facts",
        r#"
        CREATE TABLE IF NOT EXISTS retained_facts (
            id TEXT PRIMARY KEY,
            thread_id TEXT NOT NULL,
            kind TEXT NOT NULL,
            origin TEXT NOT NULL,
            ordinal INTEGER NOT NULL,
            content TEXT NOT NULL,
            model_hash TEXT,
            created_at INTEGER NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_retained_facts_thread ON retained_facts(thread_id, kind, origin, ordinal);
        "#,
    ),
    Migration::new(
        9,
        "revert_cas_lineage",
        r#"
        ALTER TABLE threads ADD COLUMN forked_from_ordinal_exclusive INTEGER;
        "#,
    ),
    Migration::new(
        10,
        "spawn_edge_depth",
        r#"
        ALTER TABLE thread_spawn_edges ADD COLUMN depth INTEGER NOT NULL DEFAULT 0;
        CREATE INDEX IF NOT EXISTS idx_thread_spawn_edges_depth ON thread_spawn_edges(depth);
        "#,
    ),
    Migration::new(
        11,
        "execution_claims_and_inbox",
        r#"
        CREATE TABLE IF NOT EXISTS thread_execution_claims (
            claim_id TEXT PRIMARY KEY,
            thread_id TEXT NOT NULL REFERENCES threads(id) ON DELETE CASCADE,
            turn_index INTEGER NOT NULL,
            status TEXT NOT NULL CHECK(status IN ('claimed', 'running', 'completed', 'aborted', 'crashed')),
            pid INTEGER NOT NULL,
            prompt_preview TEXT,
            started_at_ms INTEGER NOT NULL,
            updated_at_ms INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_thread_execution_claims_thread ON thread_execution_claims(thread_id, status);

        CREATE TABLE IF NOT EXISTS session_inbox (
            id TEXT PRIMARY KEY,
            thread_id TEXT NOT NULL REFERENCES threads(id) ON DELETE CASCADE,
            kind TEXT NOT NULL CHECK(kind IN ('prompt', 'steer', 'system')),
            payload TEXT NOT NULL,
            status TEXT NOT NULL CHECK(status IN ('queued', 'active', 'consumed', 'dropped')),
            created_at_ms INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_session_inbox_thread ON session_inbox(thread_id, status);
        "#,
    ),
    Migration::new(
        12,
        "path_locks_schema",
        r#"
        CREATE TABLE IF NOT EXISTS path_locks (
            id TEXT PRIMARY KEY,
            thread_id TEXT NOT NULL,
            path TEXT NOT NULL,
            is_dir INTEGER NOT NULL DEFAULT 0,
            locked_by TEXT NOT NULL,
            created_at_epoch_ms INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_path_locks_thread ON path_locks(thread_id, path);
        "#,
    ),
    Migration::new(
        13,
        "evidence_records_and_session_memories",
        r#"
        CREATE TABLE IF NOT EXISTS evidence_records (
            id TEXT PRIMARY KEY,
            requirement_id TEXT NOT NULL,
            evidence_type TEXT NOT NULL,
            description TEXT NOT NULL,
            verified INTEGER NOT NULL DEFAULT 1,
            turn_index INTEGER,
            timestamp_epoch_ms INTEGER NOT NULL,
            details_json TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_evidence_records_req ON evidence_records(requirement_id, verified);

        CREATE TABLE IF NOT EXISTS session_memories (
            id TEXT PRIMARY KEY,
            topic TEXT NOT NULL,
            detail TEXT NOT NULL,
            citation_count INTEGER NOT NULL DEFAULT 1,
            last_used_at_epoch_ms INTEGER,
            created_at_epoch_ms INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_session_memories_topic ON session_memories(topic);
        "#,
    ),
    Migration::new(
        14,
        "thread_ownership_and_session_todos",
        r#"
        CREATE TABLE IF NOT EXISTS thread_ownership (
            thread_id TEXT PRIMARY KEY,
            owner_client_id TEXT NOT NULL,
            created_at_epoch_ms INTEGER NOT NULL,
            updated_at_epoch_ms INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_thread_ownership_owner ON thread_ownership(owner_client_id);

        CREATE TABLE IF NOT EXISTS session_todos (
            id TEXT PRIMARY KEY,
            thread_id TEXT NOT NULL REFERENCES threads(id) ON DELETE CASCADE,
            content TEXT NOT NULL,
            status TEXT NOT NULL CHECK(status IN ('pending', 'in_progress', 'completed', 'cancelled')),
            position INTEGER NOT NULL DEFAULT 0,
            created_at_epoch_ms INTEGER NOT NULL,
            updated_at_epoch_ms INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_session_todos_thread ON session_todos(thread_id, position);
        "#,
    ),
    Migration::new(
        15,
        "workflow_orchestration_and_artifacts",
        r#"
        CREATE TABLE IF NOT EXISTS workflows (
            id TEXT PRIMARY KEY,
            thread_id TEXT,
            workspace_root TEXT NOT NULL,
            objective TEXT NOT NULL,
            workflow_type TEXT NOT NULL,
            state TEXT NOT NULL CHECK(state IN ('created', 'analyzing', 'planning', 'plan_ready', 'awaiting_approval', 'ready', 'running', 'verifying', 'integration_pending', 'completed', 'failed', 'blocked', 'cancelled')),
            plan_revision INTEGER NOT NULL DEFAULT 1,
            configuration_json TEXT NOT NULL DEFAULT '{}',
            created_at_epoch_ms INTEGER NOT NULL,
            updated_at_epoch_ms INTEGER NOT NULL,
            started_at_epoch_ms INTEGER,
            completed_at_epoch_ms INTEGER,
            cancellation_reason TEXT,
            overall_result_json TEXT,
            resource_usage_json TEXT
        );

        CREATE TABLE IF NOT EXISTS workflow_plans (
            workflow_id TEXT NOT NULL REFERENCES workflows(id) ON DELETE CASCADE,
            revision INTEGER NOT NULL,
            rationale TEXT NOT NULL,
            created_at_epoch_ms INTEGER NOT NULL,
            PRIMARY KEY (workflow_id, revision)
        );

        CREATE TABLE IF NOT EXISTS workflow_tasks (
            id TEXT NOT NULL,
            workflow_id TEXT NOT NULL REFERENCES workflows(id) ON DELETE CASCADE,
            plan_revision INTEGER NOT NULL DEFAULT 1,
            name TEXT NOT NULL,
            description TEXT NOT NULL,
            assigned_role TEXT NOT NULL,
            assigned_agent_id TEXT,
            priority INTEGER NOT NULL DEFAULT 0,
            wave_index INTEGER NOT NULL DEFAULT 1,
            dependencies_json TEXT NOT NULL DEFAULT '[]',
            acceptance_criteria_json TEXT NOT NULL DEFAULT '[]',
            required_verification_json TEXT NOT NULL DEFAULT '[]',
            declared_read_paths_json TEXT NOT NULL DEFAULT '[]',
            declared_write_paths_json TEXT NOT NULL DEFAULT '[]',
            workspace_isolation TEXT NOT NULL DEFAULT 'worktree',
            status TEXT NOT NULL CHECK(status IN ('pending', 'ready', 'claimed', 'running', 'verifying', 'completed', 'retry_pending', 'failed', 'blocked', 'cancelled', 'needs_approval')),
            retry_count INTEGER NOT NULL DEFAULT 0,
            max_retries INTEGER NOT NULL DEFAULT 3,
            started_at_epoch_ms INTEGER,
            completed_at_epoch_ms INTEGER,
            worker_id TEXT,
            lease_expires_at_epoch_ms INTEGER,
            error_details TEXT,
            input_artifacts_json TEXT NOT NULL DEFAULT '[]',
            output_artifacts_json TEXT NOT NULL DEFAULT '[]',
            verification_result_json TEXT,
            PRIMARY KEY (workflow_id, id)
        );

        CREATE TABLE IF NOT EXISTS workflow_task_dependencies (
            workflow_id TEXT NOT NULL,
            task_id TEXT NOT NULL,
            depends_on_task_id TEXT NOT NULL,
            PRIMARY KEY (workflow_id, task_id, depends_on_task_id),
            FOREIGN KEY (workflow_id, task_id) REFERENCES workflow_tasks(workflow_id, id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS workflow_events (
            id TEXT PRIMARY KEY,
            workflow_id TEXT NOT NULL REFERENCES workflows(id) ON DELETE CASCADE,
            task_id TEXT,
            agent_id TEXT,
            event_type TEXT NOT NULL,
            timestamp_epoch_ms INTEGER NOT NULL,
            correlation_id TEXT,
            payload_json TEXT NOT NULL,
            schema_version INTEGER NOT NULL DEFAULT 1
        );

        CREATE TABLE IF NOT EXISTS workflow_artifacts (
            id TEXT PRIMARY KEY,
            workflow_id TEXT NOT NULL REFERENCES workflows(id) ON DELETE CASCADE,
            task_id TEXT,
            name TEXT NOT NULL,
            artifact_type TEXT NOT NULL,
            file_path TEXT NOT NULL,
            content_sha256 TEXT NOT NULL,
            size_bytes INTEGER NOT NULL,
            mime_type TEXT NOT NULL,
            created_at_epoch_ms INTEGER NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_workflows_state ON workflows(state);
        CREATE INDEX IF NOT EXISTS idx_workflows_updated ON workflows(updated_at_epoch_ms);
        CREATE INDEX IF NOT EXISTS idx_workflow_tasks_wf_status ON workflow_tasks(workflow_id, status);
        CREATE INDEX IF NOT EXISTS idx_workflow_tasks_lease ON workflow_tasks(status, lease_expires_at_epoch_ms);
        CREATE INDEX IF NOT EXISTS idx_workflow_events_wf_ts ON workflow_events(workflow_id, timestamp_epoch_ms ASC);
        CREATE INDEX IF NOT EXISTS idx_workflow_artifacts_wf_name ON workflow_artifacts(workflow_id, name);
        "#,
    ),
    Migration::new(
        16,
        "workflow_execution_integrity",
        r#"
        ALTER TABLE workflow_tasks ADD COLUMN task_kind TEXT NOT NULL DEFAULT 'implementation';
        ALTER TABLE workflow_tasks ADD COLUMN attempt_id INTEGER NOT NULL DEFAULT 1;
        ALTER TABLE workflow_tasks ADD COLUMN execution_result_json TEXT;
        CREATE INDEX IF NOT EXISTS idx_workflow_tasks_kind ON workflow_tasks(workflow_id, task_kind);
        "#,
    ),
    Migration::new(
        17,
        "workflow_approvals_and_lock_leases",
        r#"
        CREATE TABLE IF NOT EXISTS workflow_approvals (
            id TEXT PRIMARY KEY,
            workflow_id TEXT NOT NULL REFERENCES workflows(id) ON DELETE CASCADE,
            task_id TEXT NOT NULL,
            attempt_id INTEGER NOT NULL,
            change_set_hash TEXT NOT NULL,
            patch_preview TEXT,
            status TEXT NOT NULL CHECK(status IN ('pending', 'approved', 'rejected', 'cancelled')),
            requested_at_epoch_ms INTEGER NOT NULL,
            decided_at_epoch_ms INTEGER,
            decision_actor TEXT,
            decision_reason TEXT
        );

        CREATE INDEX IF NOT EXISTS idx_workflow_approvals_wf_task ON workflow_approvals(workflow_id, task_id, attempt_id);
        CREATE INDEX IF NOT EXISTS idx_workflow_approvals_wf_status ON workflow_approvals(workflow_id, status);
        "#,
    ),
    Migration::new(
        18,
        "demand_driven_orchestration_and_worktrees",
        r#"
        CREATE TABLE IF NOT EXISTS managed_worktrees (
            id TEXT PRIMARY KEY,
            thread_id TEXT NOT NULL,
            workflow_id TEXT,
            task_id TEXT,
            worktree_path TEXT NOT NULL,
            branch TEXT NOT NULL,
            base_sha TEXT NOT NULL,
            head_sha TEXT NOT NULL,
            source_root TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'active' CHECK(status IN ('active', 'preserved', 'reconciled', 'cleaned_up')),
            unintegrated_diff TEXT,
            created_at_epoch_ms INTEGER NOT NULL,
            updated_at_epoch_ms INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS workflow_integrations (
            id TEXT PRIMARY KEY,
            workflow_id TEXT NOT NULL REFERENCES workflows(id) ON DELETE CASCADE,
            task_id TEXT NOT NULL,
            attempt_id INTEGER NOT NULL,
            status TEXT NOT NULL CHECK(status IN ('created', 'executing', 'result_ready', 'integration_pending', 'verified', 'promoted', 'failed', 'cancelled', 'rejected', 'retry_pending')),
            source_branch TEXT NOT NULL,
            source_base_sha TEXT NOT NULL,
            target_base_sha TEXT NOT NULL,
            current_target_sha TEXT,
            change_set_hash TEXT NOT NULL,
            patch_artifact_path TEXT,
            modified_files_json TEXT NOT NULL DEFAULT '[]',
            verification_passed INTEGER NOT NULL DEFAULT 0,
            error_details TEXT,
            created_at_epoch_ms INTEGER NOT NULL,
            updated_at_epoch_ms INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS workflow_conflicts (
            id TEXT PRIMARY KEY,
            workflow_id TEXT NOT NULL REFERENCES workflows(id) ON DELETE CASCADE,
            reporting_task_id TEXT NOT NULL,
            conflicting_task_id TEXT,
            resource_path TEXT NOT NULL,
            conflict_kind TEXT NOT NULL CHECK(conflict_kind IN ('genuine_dependency', 'integration_conflict', 'shared_resource_contention', 'requires_human_intervention')),
            description TEXT NOT NULL,
            suggested_dependency TEXT,
            resolution_json TEXT,
            reported_at_epoch_ms INTEGER NOT NULL,
            resolved_at_epoch_ms INTEGER
        );

        CREATE INDEX IF NOT EXISTS idx_managed_worktrees_thread ON managed_worktrees(thread_id);
        CREATE INDEX IF NOT EXISTS idx_managed_worktrees_wf_task ON managed_worktrees(workflow_id, task_id);
        CREATE INDEX IF NOT EXISTS idx_managed_worktrees_status ON managed_worktrees(status);
        CREATE INDEX IF NOT EXISTS idx_workflow_integrations_wf_task ON workflow_integrations(workflow_id, task_id, attempt_id);
        CREATE INDEX IF NOT EXISTS idx_workflow_integrations_hash ON workflow_integrations(change_set_hash);
        CREATE INDEX IF NOT EXISTS idx_workflow_integrations_status ON workflow_integrations(status);
        CREATE INDEX IF NOT EXISTS idx_workflow_conflicts_wf_task ON workflow_conflicts(workflow_id, reporting_task_id);
        "#,
    ),
    Migration::new(
        19,
        "repository_intelligence_index",
        r#"
        CREATE TABLE IF NOT EXISTS repo_indexes (
            repo_id TEXT PRIMARY KEY,
            root_path TEXT NOT NULL,
            repo_name TEXT NOT NULL,
            git_branch TEXT,
            git_head_sha TEXT,
            status TEXT NOT NULL,
            is_monorepo INTEGER NOT NULL DEFAULT 0,
            files_indexed INTEGER NOT NULL DEFAULT 0,
            symbols_indexed INTEGER NOT NULL DEFAULT 0,
            updated_at_epoch_ms INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS repo_indexed_files (
            id TEXT PRIMARY KEY,
            repo_id TEXT NOT NULL REFERENCES repo_indexes(repo_id) ON DELETE CASCADE,
            path TEXT NOT NULL,
            relative_path TEXT NOT NULL,
            content_hash TEXT NOT NULL,
            language TEXT NOT NULL,
            size_bytes INTEGER NOT NULL,
            is_test INTEGER NOT NULL DEFAULT 0,
            package_id TEXT,
            updated_at_epoch_ms INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS repo_symbols (
            id TEXT PRIMARY KEY,
            repo_id TEXT NOT NULL REFERENCES repo_indexes(repo_id) ON DELETE CASCADE,
            file_path TEXT NOT NULL,
            name TEXT NOT NULL,
            qualified_name TEXT NOT NULL,
            kind TEXT NOT NULL,
            language TEXT NOT NULL,
            start_line INTEGER NOT NULL,
            end_line INTEGER NOT NULL,
            signature TEXT NOT NULL,
            parent_scope TEXT,
            package_id TEXT,
            visibility TEXT NOT NULL,
            is_public INTEGER NOT NULL,
            doc_comment TEXT
        );

        CREATE TABLE IF NOT EXISTS repo_relationships (
            id TEXT PRIMARY KEY,
            repo_id TEXT NOT NULL REFERENCES repo_indexes(repo_id) ON DELETE CASCADE,
            source_id TEXT NOT NULL,
            target_id TEXT,
            source_symbol TEXT NOT NULL,
            target_symbol TEXT NOT NULL,
            kind TEXT NOT NULL,
            file_path TEXT NOT NULL,
            line INTEGER NOT NULL,
            status TEXT NOT NULL,
            confidence REAL NOT NULL,
            provenance TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_repo_files_repo ON repo_indexed_files(repo_id);
        CREATE INDEX IF NOT EXISTS idx_repo_files_path ON repo_indexed_files(repo_id, path);
        CREATE INDEX IF NOT EXISTS idx_repo_symbols_repo ON repo_symbols(repo_id);
        CREATE INDEX IF NOT EXISTS idx_repo_symbols_name ON repo_symbols(name);
        CREATE INDEX IF NOT EXISTS idx_repo_symbols_file ON repo_symbols(repo_id, file_path);
        CREATE INDEX IF NOT EXISTS idx_repo_rel_repo ON repo_relationships(repo_id);
        CREATE INDEX IF NOT EXISTS idx_repo_rel_target ON repo_relationships(target_symbol);
        CREATE INDEX IF NOT EXISTS idx_repo_rel_source ON repo_relationships(source_symbol);
        "#,
    ),
    Migration::new(
        20,
        "verified_execution_engine",
        r#"
        CREATE TABLE IF NOT EXISTS execution_journal_events (
            id TEXT PRIMARY KEY,
            workflow_id TEXT NOT NULL REFERENCES workflows(id) ON DELETE CASCADE,
            task_id TEXT NOT NULL,
            attempt_id INTEGER NOT NULL,
            worker_id TEXT NOT NULL,
            operation_id TEXT,
            event_type TEXT NOT NULL,
            previous_state TEXT,
            new_state TEXT,
            resource_identity TEXT,
            manifest_hash TEXT,
            payload_json TEXT NOT NULL DEFAULT '{}',
            timestamp_epoch_ms INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS change_set_manifests (
            id TEXT PRIMARY KEY,
            workflow_id TEXT NOT NULL REFERENCES workflows(id) ON DELETE CASCADE,
            task_id TEXT NOT NULL,
            attempt_id INTEGER NOT NULL,
            worker_id TEXT NOT NULL,
            agent_id TEXT,
            manifest_hash TEXT NOT NULL UNIQUE,
            source_base_sha TEXT NOT NULL,
            target_commit_sha TEXT,
            is_frozen INTEGER NOT NULL DEFAULT 0,
            manifest_json TEXT NOT NULL,
            created_at_epoch_ms INTEGER NOT NULL,
            frozen_at_epoch_ms INTEGER
        );

        CREATE TABLE IF NOT EXISTS pre_integration_snapshots (
            id TEXT PRIMARY KEY,
            integration_id TEXT NOT NULL,
            workflow_id TEXT NOT NULL REFERENCES workflows(id) ON DELETE CASCADE,
            task_id TEXT NOT NULL,
            attempt_id INTEGER NOT NULL,
            relative_path TEXT NOT NULL,
            original_exists INTEGER NOT NULL,
            original_content_hash TEXT,
            original_mode INTEGER,
            preimage_storage_path TEXT,
            preimage_blob BLOB,
            status TEXT NOT NULL CHECK(status IN ('prepared', 'restored', 'failed')),
            created_at_epoch_ms INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS external_effect_intents (
            id TEXT PRIMARY KEY,
            workflow_id TEXT NOT NULL REFERENCES workflows(id) ON DELETE CASCADE,
            task_id TEXT NOT NULL,
            attempt_id INTEGER NOT NULL,
            worker_id TEXT NOT NULL,
            logical_operation_id TEXT NOT NULL,
            tool_name TEXT NOT NULL,
            target_service TEXT NOT NULL,
            authorized_destination TEXT NOT NULL,
            request_fingerprint TEXT NOT NULL,
            idempotency_key TEXT,
            lifecycle_status TEXT NOT NULL,
            is_reversible INTEGER NOT NULL DEFAULT 0,
            compensation_procedure TEXT,
            error_details TEXT,
            created_at_epoch_ms INTEGER NOT NULL,
            updated_at_epoch_ms INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS verification_records (
            id TEXT PRIMARY KEY,
            workflow_id TEXT NOT NULL REFERENCES workflows(id) ON DELETE CASCADE,
            task_id TEXT NOT NULL,
            attempt_id INTEGER NOT NULL,
            manifest_hash TEXT NOT NULL,
            source_base_sha TEXT NOT NULL,
            target_commit_sha TEXT,
            verification_plan_hash TEXT NOT NULL,
            requirement_id TEXT NOT NULL,
            category TEXT NOT NULL,
            operation TEXT NOT NULL,
            exit_code INTEGER,
            status TEXT NOT NULL,
            tests_total INTEGER,
            tests_passed INTEGER,
            tests_failed INTEGER,
            tests_skipped INTEGER,
            duration_ms INTEGER NOT NULL,
            diagnostics TEXT,
            artifact_refs_json TEXT NOT NULL DEFAULT '[]',
            timestamp_epoch_ms INTEGER NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_exec_journal_wf_task ON execution_journal_events(workflow_id, task_id, attempt_id);
        CREATE INDEX IF NOT EXISTS idx_exec_journal_ts ON execution_journal_events(timestamp_epoch_ms ASC);
        CREATE INDEX IF NOT EXISTS idx_manifests_wf_task ON change_set_manifests(workflow_id, task_id, attempt_id);
        CREATE INDEX IF NOT EXISTS idx_manifests_hash ON change_set_manifests(manifest_hash);
        CREATE INDEX IF NOT EXISTS idx_pre_snapshots_integ ON pre_integration_snapshots(integration_id);
        CREATE INDEX IF NOT EXISTS idx_pre_snapshots_wf_task ON pre_integration_snapshots(workflow_id, task_id, attempt_id);
        CREATE INDEX IF NOT EXISTS idx_external_intents_wf_task ON external_effect_intents(workflow_id, task_id, attempt_id);
        CREATE INDEX IF NOT EXISTS idx_external_intents_logical ON external_effect_intents(workflow_id, logical_operation_id);
        CREATE INDEX IF NOT EXISTS idx_external_intents_idempotency ON external_effect_intents(idempotency_key);
        CREATE INDEX IF NOT EXISTS idx_verif_records_wf_task ON verification_records(workflow_id, task_id, attempt_id);
        CREATE INDEX IF NOT EXISTS idx_verif_records_manifest ON verification_records(manifest_hash);
        "#,
    ),
    Migration::new(
        21,
        "autonomous_repository_maintenance",
        r#"
        CREATE TABLE IF NOT EXISTS maintenance_signals (
            id TEXT PRIMARY KEY,
            repo_id TEXT NOT NULL,
            source_provider TEXT NOT NULL,
            category TEXT NOT NULL,
            provider_event_id TEXT,
            external_url TEXT,
            event_timestamp_ms INTEGER NOT NULL,
            observed_revision TEXT,
            severity TEXT NOT NULL,
            dedup_fingerprint TEXT NOT NULL,
            trust_status TEXT NOT NULL,
            metadata_json TEXT NOT NULL DEFAULT '{}',
            evidence_refs_json TEXT NOT NULL DEFAULT '[]',
            ingested_at_epoch_ms INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS maintenance_cases (
            id TEXT PRIMARY KEY,
            repo_id TEXT NOT NULL,
            category TEXT NOT NULL,
            title TEXT NOT NULL,
            problem_statement TEXT NOT NULL,
            dedup_key TEXT NOT NULL,
            status TEXT NOT NULL,
            priority TEXT NOT NULL,
            severity TEXT NOT NULL,
            confidence_score REAL NOT NULL,
            affected_packages_json TEXT NOT NULL DEFAULT '[]',
            affected_files_json TEXT NOT NULL DEFAULT '[]',
            affected_symbols_json TEXT NOT NULL DEFAULT '[]',
            evidence_refs_json TEXT NOT NULL DEFAULT '[]',
            related_issues_json TEXT NOT NULL DEFAULT '[]',
            related_prs_json TEXT NOT NULL DEFAULT '[]',
            workflow_id TEXT REFERENCES workflows(id) ON DELETE SET NULL,
            active_plan_id TEXT,
            remediation_history_json TEXT NOT NULL DEFAULT '[]',
            human_approval_state TEXT NOT NULL DEFAULT 'none',
            created_at_epoch_ms INTEGER NOT NULL,
            updated_at_epoch_ms INTEGER NOT NULL,
            last_observed_at_epoch_ms INTEGER NOT NULL,
            resolved_at_epoch_ms INTEGER
        );

        CREATE TABLE IF NOT EXISTS maintenance_case_signals (
            case_id TEXT NOT NULL REFERENCES maintenance_cases(id) ON DELETE CASCADE,
            signal_id TEXT NOT NULL REFERENCES maintenance_signals(id) ON DELETE CASCADE,
            associated_at_epoch_ms INTEGER NOT NULL,
            PRIMARY KEY (case_id, signal_id)
        );

        CREATE TABLE IF NOT EXISTS maintenance_plans (
            id TEXT PRIMARY KEY,
            case_id TEXT NOT NULL REFERENCES maintenance_cases(id) ON DELETE CASCADE,
            repo_id TEXT NOT NULL,
            baseline_sha TEXT NOT NULL,
            category TEXT NOT NULL,
            problem_classification TEXT NOT NULL,
            selected_strategy TEXT NOT NULL,
            hypotheses_json TEXT NOT NULL DEFAULT '[]',
            expected_files_json TEXT NOT NULL DEFAULT '[]',
            expected_packages_json TEXT NOT NULL DEFAULT '[]',
            relevant_tests_json TEXT NOT NULL DEFAULT '[]',
            verification_requirements_json TEXT NOT NULL DEFAULT '[]',
            estimated_risk TEXT NOT NULL,
            required_autonomy_tier INTEGER NOT NULL,
            requires_approval INTEGER NOT NULL DEFAULT 0,
            approval_state TEXT NOT NULL DEFAULT 'pending',
            workflow_id TEXT REFERENCES workflows(id) ON DELETE SET NULL,
            plan_hash TEXT NOT NULL,
            created_at_epoch_ms INTEGER NOT NULL,
            updated_at_epoch_ms INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS maintenance_executions (
            id TEXT PRIMARY KEY,
            case_id TEXT NOT NULL REFERENCES maintenance_cases(id) ON DELETE CASCADE,
            workflow_id TEXT NOT NULL REFERENCES workflows(id) ON DELETE CASCADE,
            plan_id TEXT NOT NULL REFERENCES maintenance_plans(id) ON DELETE CASCADE,
            manifest_hash TEXT,
            attempt_id INTEGER NOT NULL DEFAULT 1,
            worker_id TEXT NOT NULL,
            execution_state TEXT NOT NULL,
            verification_report_hash TEXT,
            promotion_state TEXT NOT NULL DEFAULT 'none',
            pr_number INTEGER,
            pr_url TEXT,
            failure_classification TEXT,
            duration_ms INTEGER NOT NULL DEFAULT 0,
            tokens_used INTEGER NOT NULL DEFAULT 0,
            created_at_epoch_ms INTEGER NOT NULL,
            completed_at_epoch_ms INTEGER
        );

        CREATE TABLE IF NOT EXISTS maintenance_schedules (
            id TEXT PRIMARY KEY,
            repo_id TEXT NOT NULL,
            job_kind TEXT NOT NULL,
            cron_or_interval_secs INTEGER NOT NULL,
            is_enabled INTEGER NOT NULL DEFAULT 1,
            last_run_epoch_ms INTEGER,
            next_run_epoch_ms INTEGER NOT NULL,
            consecutive_failures INTEGER NOT NULL DEFAULT 0,
            policy_override_json TEXT,
            created_at_epoch_ms INTEGER NOT NULL,
            updated_at_epoch_ms INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS maintenance_audit_trail (
            id TEXT PRIMARY KEY,
            case_id TEXT REFERENCES maintenance_cases(id) ON DELETE SET NULL,
            repo_id TEXT NOT NULL,
            event_type TEXT NOT NULL,
            actor TEXT NOT NULL,
            policy_version TEXT NOT NULL,
            details_json TEXT NOT NULL DEFAULT '{}',
            timestamp_epoch_ms INTEGER NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_maint_signals_fingerprint ON maintenance_signals(dedup_fingerprint);
        CREATE INDEX IF NOT EXISTS idx_maint_signals_repo ON maintenance_signals(repo_id, event_timestamp_ms DESC);
        CREATE INDEX IF NOT EXISTS idx_maint_cases_repo ON maintenance_cases(repo_id, status);
        CREATE INDEX IF NOT EXISTS idx_maint_cases_dedup ON maintenance_cases(repo_id, dedup_key);
        CREATE INDEX IF NOT EXISTS idx_maint_cases_workflow ON maintenance_cases(workflow_id);
        CREATE INDEX IF NOT EXISTS idx_maint_plans_case ON maintenance_plans(case_id);
        CREATE INDEX IF NOT EXISTS idx_maint_exec_case ON maintenance_executions(case_id);
        CREATE INDEX IF NOT EXISTS idx_maint_exec_wf ON maintenance_executions(workflow_id);
        CREATE INDEX IF NOT EXISTS idx_maint_sched_repo ON maintenance_schedules(repo_id, is_enabled);
        CREATE INDEX IF NOT EXISTS idx_maint_audit_case ON maintenance_audit_trail(case_id, timestamp_epoch_ms ASC);
        "#,
    ),
];

/// Migration engine responsible for running and validating SQLite migrations.
pub struct MigrationEngine;

impl MigrationEngine {
    /// Initialize the schema migrations tracking table if not present.
    pub fn init_migrations_table(conn: &Connection) -> Result<()> {
        conn.execute(
            r#"
            CREATE TABLE IF NOT EXISTS _m31a_schema_migrations (
                version INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                applied_at_epoch_ms INTEGER NOT NULL,
                checksum TEXT NOT NULL
            );
            "#,
            [],
        )
        .context("Failed initializing _m31a_schema_migrations tracking table")?;
        Ok(())
    }

    /// Retrieve all applied migrations sorted ascending by version.
    pub fn get_applied_migrations(conn: &Connection) -> Result<Vec<AppliedMigrationRecord>> {
        Self::init_migrations_table(conn)?;
        let mut stmt = conn.prepare(
            "SELECT version, name, applied_at_epoch_ms, checksum FROM _m31a_schema_migrations ORDER BY version ASC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(AppliedMigrationRecord {
                version: row.get(0)?,
                name: row.get(1)?,
                applied_at_epoch_ms: row.get(2)?,
                checksum: row.get(3)?,
            })
        })?;

        let mut list = Vec::new();
        for r in rows {
            list.push(r?);
        }
        Ok(list)
    }

    /// Apply all pending migrations in `migrations` catalog. Returns the number of newly applied migrations.
    pub fn apply_all(conn: &mut Connection, migrations: &[Migration]) -> Result<u32> {
        Self::init_migrations_table(conn)?;
        let applied = Self::get_applied_migrations(conn)?;
        let applied_versions: std::collections::HashMap<u32, String> = applied
            .into_iter()
            .map(|a| (a.version, a.checksum))
            .collect();

        let mut new_applied_count = 0;
        let now = Utc::now().timestamp_millis();

        for m in migrations {
            if let Some(existing_checksum) = applied_versions.get(&m.version) {
                // Verify checksum integrity
                let expected = m.checksum();
                if existing_checksum != &expected {
                    tracing::warn!(
                        "Migration #{} ({}) checksum discrepancy: db={}, code={}",
                        m.version,
                        m.name,
                        existing_checksum,
                        expected
                    );
                }
                continue;
            }

            // Apply migration in a transaction
            let tx = conn.transaction().context(format!(
                "Failed starting transaction for migration #{}",
                m.version
            ))?;
            tx.execute_batch(m.sql).context(format!(
                "Failed executing SQL batch for migration #{} ({})",
                m.version, m.name
            ))?;

            tx.execute(
                "INSERT OR REPLACE INTO _m31a_schema_migrations (version, name, applied_at_epoch_ms, checksum) VALUES (?1, ?2, ?3, ?4)",
                params![m.version, m.name, now, m.checksum()],
            )
            .context(format!("Failed recording migration #{} in tracking table", m.version))?;

            tx.commit().context(format!(
                "Failed committing transaction for migration #{}",
                m.version
            ))?;

            new_applied_count += 1;
        }

        Ok(new_applied_count)
    }

    /// Return the highest applied migration version or 0 if none applied.
    pub fn current_version(conn: &Connection) -> Result<u32> {
        Self::init_migrations_table(conn)?;
        let mut stmt =
            conn.prepare("SELECT COALESCE(MAX(version), 0) FROM _m31a_schema_migrations")?;
        let ver: u32 = stmt.query_row([], |row| row.get(0))?;
        Ok(ver)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_migration_checksum_deterministic() {
        let m = Migration::new(1, "test", "CREATE TABLE abc (id TEXT PRIMARY KEY);");
        let hash1 = m.checksum();
        let hash2 = m.checksum();
        assert_eq!(hash1, hash2);
        assert_eq!(hash1.len(), 64);
    }

    #[test]
    fn test_apply_all_migrations_and_idempotence() {
        let mut conn = Connection::open_in_memory().unwrap();

        let applied = MigrationEngine::apply_all(&mut conn, BUILTIN_MIGRATIONS).unwrap();
        assert_eq!(applied as usize, BUILTIN_MIGRATIONS.len());

        let ver = MigrationEngine::current_version(&conn).unwrap();
        assert_eq!(ver, BUILTIN_MIGRATIONS.last().unwrap().version);

        // Re-applying must be a clean no-op
        let second_applied = MigrationEngine::apply_all(&mut conn, BUILTIN_MIGRATIONS).unwrap();
        assert_eq!(second_applied, 0);

        // Verify tables exist
        let tables: Vec<String> = {
            let mut stmt = conn
                .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name ASC")
                .unwrap();
            stmt.query_map([], |r| r.get(0))
                .unwrap()
                .map(|r| r.unwrap())
                .collect()
        };

        assert!(tables.contains(&"_m31a_schema_migrations".to_string()));
        assert!(tables.contains(&"threads".to_string()));
        assert!(tables.contains(&"thread_turns".to_string()));
        assert!(tables.contains(&"thread_items".to_string()));
        assert!(tables.contains(&"thread_goals".to_string()));
        assert!(tables.contains(&"checkpoints".to_string()));
        assert!(tables.contains(&"thread_spawn_edges".to_string()));
        assert!(tables.contains(&"channels".to_string()));
        assert!(tables.contains(&"subscriptions".to_string()));
        assert!(tables.contains(&"posts".to_string()));
        assert!(tables.contains(&"thread_sections".to_string()));
        assert!(tables.contains(&"rollout_notes".to_string()));
        assert!(tables.contains(&"history_windows".to_string()));
        assert!(tables.contains(&"history_items".to_string()));
        assert!(tables.contains(&"audit_logs".to_string()));
        assert!(tables.contains(&"task_queue".to_string()));
        assert!(tables.contains(&"thread_execution_claims".to_string()));
        assert!(tables.contains(&"session_inbox".to_string()));
        assert!(tables.contains(&"evidence_records".to_string()));
        assert!(tables.contains(&"session_memories".to_string()));
        assert!(tables.contains(&"execution_journal_events".to_string()));
        assert!(tables.contains(&"change_set_manifests".to_string()));
        assert!(tables.contains(&"pre_integration_snapshots".to_string()));
        assert!(tables.contains(&"external_effect_intents".to_string()));
        assert!(tables.contains(&"verification_records".to_string()));
        assert!(tables.contains(&"maintenance_signals".to_string()));
        assert!(tables.contains(&"maintenance_cases".to_string()));
        assert!(tables.contains(&"maintenance_case_signals".to_string()));
        assert!(tables.contains(&"maintenance_plans".to_string()));
        assert!(tables.contains(&"maintenance_executions".to_string()));
        assert!(tables.contains(&"maintenance_schedules".to_string()));
        assert!(tables.contains(&"maintenance_audit_trail".to_string()));
    }
}
