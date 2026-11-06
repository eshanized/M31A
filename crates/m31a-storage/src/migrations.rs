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
    }
}
