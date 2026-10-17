//! SQLite concrete implementation of ExecutionRunRepository.

use crate::error::M31AError;
use crate::ids::{ExecutionRunId, SessionId, TaskGraphId};
use crate::persistence::sqlite::repositories::ExecutionRunRepository;
use crate::state::execution_run::ExecutionRun;
use crate::state::intake::AutonomyMode;
use crate::state::policy_context::PolicyContext;
use crate::state_machine::ExecutionRunState;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::sqlite::SqliteRow;
use sqlx::{Row, SqlitePool};
use std::path::PathBuf;
use std::str::FromStr;

/// SQLite repository for ExecutionRun aggregates.
#[derive(Debug, Clone)]
pub struct SqliteExecutionRunRepository {
    pool: SqlitePool,
}

impl SqliteExecutionRunRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Update the state reconstruction watermark on an execution run.
    pub async fn update_watermark(
        &self,
        id: ExecutionRunId,
        sequence: u64,
    ) -> Result<(), M31AError> {
        let now = Utc::now().to_rfc3339();
        let rows_affected = sqlx::query(
            "UPDATE execution_runs SET last_applied_sequence = ?, updated_at = ? WHERE id = ?",
        )
        .bind(sequence as i64)
        .bind(&now)
        .bind(id.as_bytes().as_slice())
        .execute(&self.pool)
        .await?
        .rows_affected();

        if rows_affected == 0 {
            return Err(M31AError::not_found(format!("ExecutionRun {}", id)));
        }
        Ok(())
    }

    /// Retrieve all execution runs forked from a specific parent run.
    pub async fn get_by_parent(
        &self,
        parent_id: ExecutionRunId,
    ) -> Result<Vec<ExecutionRun>, M31AError> {
        let rows = sqlx::query(
            r#"
            SELECT id, objective, status, workspace_root, mode, policy_context,
                   constraints, requirements, success_criteria, started_at, completed_at,
                   parent_run_id, session_id, task_graph_id, last_applied_sequence, created_at, updated_at
            FROM execution_runs WHERE parent_run_id = ? ORDER BY created_at ASC
            "#,
        )
        .bind(parent_id.as_bytes().as_slice())
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter().map(map_row_to_execution_run).collect()
    }

    /// Retrieve an execution run by its ID.
    pub async fn get(&self, id: ExecutionRunId) -> Result<Option<ExecutionRun>, M31AError> {
        <Self as ExecutionRunRepository>::get(self, id).await
    }

    /// Insert a new execution run aggregate.
    pub async fn insert(&self, run: &ExecutionRun) -> Result<(), M31AError> {
        <Self as ExecutionRunRepository>::insert(self, run).await
    }

    /// Update status of an execution run.
    pub async fn update_status(
        &self,
        id: ExecutionRunId,
        status: ExecutionRunState,
    ) -> Result<(), M31AError> {
        <Self as ExecutionRunRepository>::update_status(self, id, status).await
    }

    /// List all execution runs.
    pub async fn list_all(&self) -> Result<Vec<ExecutionRun>, M31AError> {
        <Self as ExecutionRunRepository>::list_all(self).await
    }
}

#[async_trait]
impl ExecutionRunRepository for SqliteExecutionRunRepository {
    async fn insert(&self, run: &ExecutionRun) -> Result<(), M31AError> {
        let policy_context_json = serde_json::to_string(&run.policy_context)
            .map_err(|e| M31AError::validation(e.to_string()))?;
        let constraints_json = serde_json::to_string(&run.constraints)
            .map_err(|e| M31AError::validation(e.to_string()))?;
        let requirements_json = serde_json::to_string(&run.requirements)
            .map_err(|e| M31AError::validation(e.to_string()))?;
        let success_criteria_json = serde_json::to_string(&run.success_criteria)
            .map_err(|e| M31AError::validation(e.to_string()))?;

        // ExecutionRun is the SOLE canonical lifecycle authority: no Mission
        // aggregate is constructed, no mission lifecycle transition ever runs,
        // and child table foreign keys reference execution_runs directly (migration 031).
        sqlx::query(
            r#"
            INSERT INTO execution_runs (
                id, objective, status, workspace_root, mode, policy_context,
                constraints, requirements, success_criteria, started_at, completed_at,
                parent_run_id, session_id, task_graph_id, last_applied_sequence, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(run.id.as_bytes().as_slice())
        .bind(&run.objective)
        .bind(run.status.to_string())
        .bind(run.workspace_root.to_string_lossy().to_string())
        .bind(format!("{:?}", run.mode))
        .bind(policy_context_json)
        .bind(constraints_json)
        .bind(requirements_json)
        .bind(success_criteria_json)
        .bind(run.started_at.map(|t| t.to_rfc3339()))
        .bind(run.completed_at.map(|t| t.to_rfc3339()))
        .bind(run.parent_run_id.map(|p| p.as_bytes().to_vec()))
        .bind(run.session_id.map(|s| s.as_bytes().to_vec()))
        .bind(run.task_graph_id.map(|t| t.as_bytes().to_vec()))
        .bind(run.last_applied_sequence as i64)
        .bind(run.created_at.to_rfc3339())
        .bind(run.updated_at.to_rfc3339())
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn get(&self, id: ExecutionRunId) -> Result<Option<ExecutionRun>, M31AError> {
        let row = sqlx::query(
            r#"
            SELECT id, objective, status, workspace_root, mode, policy_context,
                   constraints, requirements, success_criteria, started_at, completed_at,
                   parent_run_id, session_id, task_graph_id, last_applied_sequence, created_at, updated_at
            FROM execution_runs WHERE id = ?
            "#,
        )
        .bind(id.as_bytes().as_slice())
        .fetch_optional(&self.pool)
        .await?;

        row.map(map_row_to_execution_run).transpose()
    }

    async fn update_status(
        &self,
        id: ExecutionRunId,
        status: ExecutionRunState,
    ) -> Result<(), M31AError> {
        let row = sqlx::query("SELECT status FROM execution_runs WHERE id = ?")
            .bind(id.as_bytes().as_slice())
            .fetch_optional(&self.pool)
            .await?;

        let current_status_str: String = match row {
            Some(r) => r.try_get("status")?,
            None => return Err(M31AError::not_found(format!("ExecutionRun {}", id))),
        };

        let current_status = ExecutionRunState::from_str(&current_status_str)
            .map_err(|e| M31AError::validation(format!("Invalid ExecutionRunState: {}", e)))?;

        if current_status == status {
            return Ok(());
        }

        // Invariant F-06: Canonical state machine validation before mutation
        ExecutionRunState::validate_transition(current_status, status)
            .map_err(|e| M31AError::transition(e.to_string()))?;

        let now = Utc::now().to_rfc3339();
        let rows_affected = sqlx::query(
            r#"
            UPDATE execution_runs
            SET status = ?,
                updated_at = ?,
                started_at = CASE WHEN started_at IS NULL AND ? != 'Created' THEN ? ELSE started_at END,
                completed_at = CASE WHEN ? IN ('Completed', 'Failed', 'Cancelled') THEN ? ELSE completed_at END
            WHERE id = ? AND status = ?
            "#,
        )
        .bind(status.to_string())
        .bind(&now)
        .bind(status.to_string())
        .bind(&now)
        .bind(status.to_string())
        .bind(&now)
        .bind(id.as_bytes().as_slice())
        .bind(&current_status_str)
        .execute(&self.pool)
        .await?
        .rows_affected();

        if rows_affected == 0 {
            return Err(M31AError::transition(format!(
                "Concurrent status modification on ExecutionRun {}",
                id
            )));
        }

        Ok(())
    }

    async fn list_all(&self) -> Result<Vec<ExecutionRun>, M31AError> {
        let rows = sqlx::query(
            r#"
            SELECT id, objective, status, workspace_root, mode, policy_context,
                   constraints, requirements, success_criteria, started_at, completed_at,
                   parent_run_id, session_id, task_graph_id, last_applied_sequence, created_at, updated_at
            FROM execution_runs ORDER BY created_at DESC
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter().map(map_row_to_execution_run).collect()
    }

    async fn update_watermark(&self, id: ExecutionRunId, sequence: u64) -> Result<(), M31AError> {
        SqliteExecutionRunRepository::update_watermark(self, id, sequence).await
    }
}

fn map_row_to_execution_run(row: SqliteRow) -> Result<ExecutionRun, M31AError> {
    let id_bytes: Vec<u8> = row.try_get("id")?;
    let id_array: [u8; 16] = id_bytes
        .try_into()
        .map_err(|_| M31AError::validation("Invalid UUID bytes for execution run id"))?;
    let id = ExecutionRunId::from_bytes(id_array);

    let objective: String = row.try_get("objective")?;
    let status_str: String = row.try_get("status")?;
    let status = ExecutionRunState::from_str(&status_str)
        .map_err(|e| M31AError::validation(format!("Invalid ExecutionRunState: {}", e)))?;

    let workspace_root_str: String = row.try_get("workspace_root")?;
    let workspace_root = PathBuf::from(workspace_root_str);

    let mode_str: String = row.try_get("mode")?;
    let mode = mode_str
        .parse::<AutonomyMode>()
        .unwrap_or(AutonomyMode::Safe);

    let policy_context_json: String = row.try_get("policy_context")?;
    let policy_context: PolicyContext = serde_json::from_str(&policy_context_json)
        .map_err(|e| M31AError::validation(format!("Invalid policy context JSON: {}", e)))?;

    let constraints_json: String = row.try_get("constraints")?;
    let constraints: Vec<String> = serde_json::from_str(&constraints_json)
        .map_err(|e| M31AError::validation(format!("Invalid constraints JSON: {}", e)))?;

    let requirements_json: String = row.try_get("requirements")?;
    let requirements: Vec<String> = serde_json::from_str(&requirements_json)
        .map_err(|e| M31AError::validation(format!("Invalid requirements JSON: {}", e)))?;

    let success_criteria_json: String = row.try_get("success_criteria")?;
    let success_criteria: Vec<String> = serde_json::from_str(&success_criteria_json)
        .map_err(|e| M31AError::validation(format!("Invalid success criteria JSON: {}", e)))?;

    let started_at: Option<DateTime<Utc>> = row
        .try_get::<Option<String>, _>("started_at")?
        .map(|s| {
            DateTime::parse_from_rfc3339(&s)
                .map(|dt| dt.with_timezone(&Utc))
                .map_err(|e| M31AError::validation(format!("Invalid started_at: {}", e)))
        })
        .transpose()?;

    let completed_at: Option<DateTime<Utc>> = row
        .try_get::<Option<String>, _>("completed_at")?
        .map(|s| {
            DateTime::parse_from_rfc3339(&s)
                .map(|dt| dt.with_timezone(&Utc))
                .map_err(|e| M31AError::validation(format!("Invalid completed_at: {}", e)))
        })
        .transpose()?;

    let parent_run_id: Option<ExecutionRunId> = row
        .try_get::<Option<Vec<u8>>, _>("parent_run_id")?
        .map(|bytes| -> Result<ExecutionRunId, M31AError> {
            let arr: [u8; 16] = bytes
                .try_into()
                .map_err(|_| M31AError::validation("Invalid parent execution run UUID bytes"))?;
            Ok(ExecutionRunId::from_bytes(arr))
        })
        .transpose()?;

    let session_id: Option<SessionId> = row
        .try_get::<Option<Vec<u8>>, _>("session_id")?
        .map(|bytes| -> Result<SessionId, M31AError> {
            let arr: [u8; 16] = bytes
                .try_into()
                .map_err(|_| M31AError::validation("Invalid session UUID bytes"))?;
            Ok(SessionId::from_bytes(arr))
        })
        .transpose()?;

    let task_graph_id: Option<TaskGraphId> = row
        .try_get::<Option<Vec<u8>>, _>("task_graph_id")?
        .map(|bytes| -> Result<TaskGraphId, M31AError> {
            let arr: [u8; 16] = bytes
                .try_into()
                .map_err(|_| M31AError::validation("Invalid task graph UUID bytes"))?;
            Ok(TaskGraphId::from_bytes(arr))
        })
        .transpose()?;

    let last_applied_sequence: i64 = row.try_get("last_applied_sequence")?;

    let created_at_str: String = row.try_get("created_at")?;
    let created_at = DateTime::parse_from_rfc3339(&created_at_str)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|e| M31AError::validation(format!("Invalid created_at: {}", e)))?;

    let updated_at_str: String = row.try_get("updated_at")?;
    let updated_at = DateTime::parse_from_rfc3339(&updated_at_str)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|e| M31AError::validation(format!("Invalid updated_at: {}", e)))?;

    Ok(ExecutionRun {
        id,
        objective,
        constraints,
        requirements,
        success_criteria,
        workspace_root,
        mode,
        policy_context,
        status,
        created_at,
        updated_at,
        started_at,
        completed_at,
        parent_run_id,
        session_id,
        task_graph_id,
        last_applied_sequence: last_applied_sequence as u64,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn setup_db() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();

        crate::persistence::sqlite::schema::run_migrations(&pool)
            .await
            .unwrap();

        pool
    }

    #[tokio::test]
    async fn test_execution_run_crud() {
        let pool = setup_db().await;
        let repo = SqliteExecutionRunRepository::new(pool);

        let id = ExecutionRunId::new();
        let run = ExecutionRun::new(id, "Test execution objective".to_string());

        repo.insert(&run).await.unwrap();

        let retrieved = repo.get(id).await.unwrap().expect("run must exist");
        assert_eq!(retrieved.id, id);
        assert_eq!(retrieved.objective, "Test execution objective");
        assert_eq!(retrieved.status, ExecutionRunState::Created);

        repo.update_status(id, ExecutionRunState::Executing)
            .await
            .unwrap();
        let updated = repo.get(id).await.unwrap().unwrap();
        assert_eq!(updated.status, ExecutionRunState::Executing);
        assert!(updated.started_at.is_some());

        let all = repo.list_all().await.unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].id, id);
    }

    #[tokio::test]
    async fn test_f06_terminal_resurrection_rejected() {
        let pool = setup_db().await;
        let repo = SqliteExecutionRunRepository::new(pool);

        for terminal_state in [
            ExecutionRunState::Completed,
            ExecutionRunState::Failed,
            ExecutionRunState::Cancelled,
        ] {
            let id = ExecutionRunId::new();
            let run = ExecutionRun::new(id, format!("Terminal test {:?}", terminal_state));
            repo.insert(&run).await.unwrap();

            // Progress to executing, then to terminal state
            repo.update_status(id, ExecutionRunState::Executing)
                .await
                .unwrap();
            repo.update_status(id, terminal_state).await.unwrap();

            // Verify terminal state
            let current = repo.get(id).await.unwrap().unwrap();
            assert_eq!(current.status, terminal_state);

            // Attempts to transition out of terminal state must fail closed
            for target in [
                ExecutionRunState::Created,
                ExecutionRunState::Understanding,
                ExecutionRunState::Planning,
                ExecutionRunState::Scheduled,
                ExecutionRunState::Executing,
                ExecutionRunState::Paused,
            ] {
                let res = repo.update_status(id, target).await;
                assert!(
                    res.is_err(),
                    "Transition from terminal state {:?} to {:?} should be rejected",
                    terminal_state,
                    target
                );
            }
        }
    }

    #[tokio::test]
    async fn test_f06_invalid_forward_leaps_rejected() {
        let pool = setup_db().await;
        let repo = SqliteExecutionRunRepository::new(pool);

        let id = ExecutionRunId::new();
        let run = ExecutionRun::new(id, "Invalid leap test".to_string());
        repo.insert(&run).await.unwrap();

        // Direct leap from Created to Completed must be rejected
        let res = repo.update_status(id, ExecutionRunState::Completed).await;
        assert!(res.is_err(), "Created -> Completed leap must fail");

        // Direct leap from Created to Paused must be rejected
        let res = repo.update_status(id, ExecutionRunState::Paused).await;
        assert!(res.is_err(), "Created -> Paused leap must fail");

        // Direct leap from Created to Verifying must be rejected
        let res = repo.update_status(id, ExecutionRunState::Verifying).await;
        assert!(res.is_err(), "Created -> Verifying leap must fail");
    }

    #[tokio::test]
    async fn test_f06_valid_lifecycle_progression() {
        let pool = setup_db().await;
        let repo = SqliteExecutionRunRepository::new(pool);

        let id = ExecutionRunId::new();
        let run = ExecutionRun::new(id, "Valid progression test".to_string());
        repo.insert(&run).await.unwrap();

        // Created -> Understanding -> Researching -> Planning -> Scheduled -> Executing -> Paused -> Executing -> Completed
        let stages = [
            ExecutionRunState::Understanding,
            ExecutionRunState::Researching,
            ExecutionRunState::Planning,
            ExecutionRunState::Scheduled,
            ExecutionRunState::Executing,
            ExecutionRunState::Paused,
            ExecutionRunState::Executing,
            ExecutionRunState::Completed,
        ];

        for stage in stages {
            repo.update_status(id, stage).await.unwrap();
            let current = repo.get(id).await.unwrap().unwrap();
            assert_eq!(current.status, stage);
        }
    }
}
