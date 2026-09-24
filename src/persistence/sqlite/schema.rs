//! Schema definitions and migration runner
//!
//! Uses sqlx::migrate! macro to load and run migrations from ./migrations directory.
//! Per RESEARCH.md Pattern 4, migrations are versioned and explicit.

use sqlx::migrate::Migrator;
use sqlx::sqlite::SqlitePool;
use std::path::Path;

/// Static migrator that loads migrations from ./migrations at compile time.
static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

/// Runs all pending migrations against the given pool.
///
/// Returns Ok(()) on success, or sqlx::Error if migration fails.
pub async fn run_migrations(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    MIGRATOR.run(pool).await?;
    Ok(())
}

/// Creates a SQLite pool and runs all pending migrations.
///
/// This is the primary initialization function for the database.
/// It combines pool creation (with WAL mode and busy_timeout) and migration execution.
pub async fn initialize_database(db_path: &Path) -> Result<SqlitePool, sqlx::Error> {
    let pool = super::pool::create_pool(db_path).await?;
    run_migrations(&pool).await?;
    Ok(pool)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_initialize_database_creates_tables() {
        let _dir = tempdir().unwrap(); // Keep TempDir alive
        let db_path = _dir.path().join("test.db");

        let pool = initialize_database(&db_path).await.unwrap();

        // Verify all required tables exist
        let tables: Vec<(String,)> = sqlx::query_as(
            "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name"
        )
        .fetch_all(&pool)
        .await
        .unwrap();

        let table_names: Vec<String> = tables.into_iter().map(|(name,)| name).collect();

        // Check for required tables from migrations/001_initial.sql
        assert!(
            table_names.contains(&"missions".to_string()),
            "missions table missing"
        );
        assert!(
            table_names.contains(&"tasks".to_string()),
            "tasks table missing"
        );
        assert!(
            table_names.contains(&"agents".to_string()),
            "agents table missing"
        );
        assert!(
            table_names.contains(&"event_log".to_string()),
            "event_log table missing"
        );
        assert!(
            table_names.contains(&"sessions".to_string()),
            "sessions table missing"
        );
        // Check for required tables from migrations/005_models_context_repo.sql
        assert!(
            table_names.contains(&"model_invocations".to_string()),
            "model_invocations table missing"
        );
        assert!(
            table_names.contains(&"repo_cache_files".to_string()),
            "repo_cache_files table missing"
        );
        assert!(
            table_names.contains(&"repo_cache_symbols".to_string()),
            "repo_cache_symbols table missing"
        );
        assert!(
            table_names.contains(&"repo_cache_edges".to_string()),
            "repo_cache_edges table missing"
        );
        assert!(
            table_names.contains(&"repository_baselines".to_string()),
            "repository_baselines table missing"
        );
        // Check for required tables from migrations/006_capabilities_tools_jobs.sql
        assert!(
            table_names.contains(&"tool_executions".to_string()),
            "tool_executions table missing"
        );
        assert!(
            table_names.contains(&"background_jobs".to_string()),
            "background_jobs table missing"
        );
        assert!(
            table_names.contains(&"capability_health_records".to_string()),
            "capability_health_records table missing"
        );
        // Check for required tables from migrations/007_checkpoints.sql
        assert!(
            table_names.contains(&"checkpoints".to_string()),
            "checkpoints table missing"
        );
        assert!(
            table_names.contains(&"verification_results".to_string()),
            "verification_results table missing"
        );
        assert!(
            table_names.contains(&"policy_audit_logs".to_string()),
            "policy_audit_logs table missing"
        );
        // Check for required tables from migrations/008_policy_sandbox_jobs.sql
        assert!(
            table_names.contains(&"policy_decisions".to_string()),
            "policy_decisions table missing"
        );
        assert!(
            table_names.contains(&"approval_requests".to_string()),
            "approval_requests table missing"
        );
        assert!(
            table_names.contains(&"policy_grants".to_string()),
            "policy_grants table missing"
        );
        assert!(
            table_names.contains(&"jobs".to_string()),
            "jobs table missing"
        );
        // Check for required tables from migrations/009_verification_recovery_checkpoints.sql
        assert!(
            table_names.contains(&"verification_checks".to_string()),
            "verification_checks table missing"
        );
        assert!(
            table_names.contains(&"requirement_check_coverage".to_string()),
            "requirement_check_coverage table missing"
        );
        assert!(
            table_names.contains(&"completion_gate_decisions".to_string()),
            "completion_gate_decisions table missing"
        );
        assert!(
            table_names.contains(&"recovery_attempts".to_string()),
            "recovery_attempts table missing"
        );
        assert!(
            table_names.contains(&"checkpoint_artifacts".to_string()),
            "checkpoint_artifacts table missing"
        );
        // Check for required tables from migrations/012_interactive_sessions.sql
        assert!(
            table_names.contains(&"conversation_messages".to_string()),
            "conversation_messages table missing"
        );
        // Check for required tables from migrations/013_workflow_orchestration.sql
        assert!(
            table_names.contains(&"workflow_runs".to_string()),
            "workflow_runs table missing"
        );
        assert!(
            table_names.contains(&"workflow_step_runs".to_string()),
            "workflow_step_runs table missing"
        );
        assert!(
            table_names.contains(&"workflow_artifacts".to_string()),
            "workflow_artifacts table missing"
        );
        // Check for unified artifacts table from migrations/014_artifact_ledger.sql
        assert!(
            table_names.contains(&"artifacts".to_string()),
            "artifacts table missing"
        );
        // Check for crash recovery scans table from migrations/016_crash_recovery_scans.sql
        assert!(
            table_names.contains(&"crash_recovery_scans".to_string()),
            "crash_recovery_scans table missing"
        );
        // Check for system_state table from migrations/017_system_state.sql
        assert!(
            table_names.contains(&"system_state".to_string()),
            "system_state table missing"
        );
        // Check for engineering memory tables from migrations/019_engineering_memory.sql
        assert!(
            table_names.contains(&"engineering_decisions".to_string()),
            "engineering_decisions table missing"
        );
        assert!(
            table_names.contains(&"engineering_assumptions".to_string()),
            "engineering_assumptions table missing"
        );
        assert!(
            table_names.contains(&"failure_diagnoses".to_string()),
            "failure_diagnoses table missing"
        );
        assert!(
            table_names.contains(&"review_findings".to_string()),
            "review_findings table missing"
        );
        assert!(
            table_names.contains(&"verification_records".to_string()),
            "verification_records table missing"
        );
        assert!(
            table_names.contains(&"_sqlx_migrations".to_string()),
            "migrations tracking table missing"
        );
    }

    #[tokio::test]
    async fn test_run_migrations_idempotent() {
        let _dir = tempdir().unwrap(); // Keep TempDir alive
        let db_path = _dir.path().join("test.db");

        let pool = initialize_database(&db_path).await.unwrap();

        // Run migrations again - should be idempotent
        run_migrations(&pool).await.unwrap();

        // Verify tables still exist
        let tables: Vec<(String,)> = sqlx::query_as(
            "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name"
        )
        .fetch_all(&pool)
        .await
        .unwrap();

        assert!(!tables.is_empty());
    }
}
