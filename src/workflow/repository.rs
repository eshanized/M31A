//! SQLite persistence and repository trait for workflow runs, steps, and artifacts.

use crate::ids::{AgentId, ArtifactId, MissionId, WorkflowRunId, WorkflowStepRunId};
use crate::workflow::error::WorkflowError;
use crate::workflow::state::{
    WorkflowArtifact, WorkflowArtifactStatus, WorkflowMode, WorkflowRun, WorkflowRunState,
    WorkflowStepRun, WorkflowStepState,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::sqlite::SqliteRow;
use sqlx::{Row, SqlitePool};
use std::path::PathBuf;
use std::str::FromStr;

/// Abstract repository interface for durable workflow state persistence.
#[async_trait]
pub trait WorkflowRepository: Send + Sync {
    /// Persists a newly created workflow run.
    async fn create_run(&self, run: &WorkflowRun) -> Result<(), WorkflowError>;

    /// Retrieves a workflow run by its ID.
    async fn get_run(&self, id: WorkflowRunId) -> Result<Option<WorkflowRun>, WorkflowError>;

    /// Updates an existing workflow run.
    async fn update_run(&self, run: &WorkflowRun) -> Result<(), WorkflowError>;

    /// Persists a newly created workflow step run.
    async fn create_step_run(&self, step_run: &WorkflowStepRun) -> Result<(), WorkflowError>;

    /// Retrieves a workflow step run by its ID.
    async fn get_step_run(
        &self,
        id: WorkflowStepRunId,
    ) -> Result<Option<WorkflowStepRun>, WorkflowError>;

    /// Retrieves a workflow step run by workflow run ID and step key.
    async fn get_step_run_by_key(
        &self,
        run_id: WorkflowRunId,
        step_key: &str,
    ) -> Result<Option<WorkflowStepRun>, WorkflowError>;

    /// Updates an existing workflow step run.
    async fn update_step_run(&self, step_run: &WorkflowStepRun) -> Result<(), WorkflowError>;

    /// Lists all step runs belonging to a workflow run.
    async fn list_step_runs(
        &self,
        run_id: WorkflowRunId,
    ) -> Result<Vec<WorkflowStepRun>, WorkflowError>;

    /// Persists a newly produced workflow artifact.
    async fn record_artifact(&self, artifact: &WorkflowArtifact) -> Result<(), WorkflowError>;

    /// Retrieves an artifact by its ID.
    async fn get_artifact(&self, id: ArtifactId)
    -> Result<Option<WorkflowArtifact>, WorkflowError>;

    /// Lists all artifacts produced across a workflow run.
    async fn list_artifacts(
        &self,
        run_id: WorkflowRunId,
    ) -> Result<Vec<WorkflowArtifact>, WorkflowError>;

    /// Lists all artifacts produced by a specific step run.
    async fn list_step_artifacts(
        &self,
        step_run_id: WorkflowStepRunId,
    ) -> Result<Vec<WorkflowArtifact>, WorkflowError>;

    /// Updates the status of an artifact (e.g. marking it superseded).
    async fn update_artifact_status(
        &self,
        id: ArtifactId,
        status: WorkflowArtifactStatus,
    ) -> Result<(), WorkflowError>;

    /// Atomically updates a workflow run and advances/creates a step run in a single transaction.
    async fn advance_step_and_update_run(
        &self,
        run: &WorkflowRun,
        step_run: &WorkflowStepRun,
    ) -> Result<(), WorkflowError>;

    /// Atomically records an artifact and updates the associated step run in a single transaction.
    async fn record_artifact_and_update_step(
        &self,
        artifact: &WorkflowArtifact,
        step_run: &WorkflowStepRun,
    ) -> Result<(), WorkflowError>;

    /// Lists all workflow runs matching any of the specified states.
    async fn list_runs_by_states(
        &self,
        states: &[WorkflowRunState],
    ) -> Result<Vec<WorkflowRun>, WorkflowError>;

    /// Lists all incomplete workflow runs (running, awaiting_input, awaiting_approval, blocked).
    async fn list_incomplete_runs(&self) -> Result<Vec<WorkflowRun>, WorkflowError> {
        self.list_runs_by_states(&[
            WorkflowRunState::Running,
            WorkflowRunState::AwaitingInput,
            WorkflowRunState::AwaitingApproval,
            WorkflowRunState::Blocked,
        ])
        .await
    }

    /// Optional access to underlying SQLite pool if backed by SQLite.
    fn pool(&self) -> Option<&SqlitePool> {
        None
    }
}

/// SQLite-backed concrete implementation of `WorkflowRepository`.
#[derive(Debug, Clone)]
pub struct SqliteWorkflowRepository {
    pool: SqlitePool,
}

impl SqliteWorkflowRepository {
    /// Creates a new repository wrapping an existing SQLite connection pool.
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Access the underlying connection pool.
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }
}

#[async_trait]
impl WorkflowRepository for SqliteWorkflowRepository {
    fn pool(&self) -> Option<&SqlitePool> {
        Some(&self.pool)
    }

    async fn create_run(&self, run: &WorkflowRun) -> Result<(), WorkflowError> {
        let started_at_str = run.started_at.to_rfc3339();
        let updated_at_str = run.updated_at.to_rfc3339();
        let completed_at_str = run.completed_at.map(|t| t.to_rfc3339());

        sqlx::query(
            r#"
            INSERT INTO workflow_runs (
                id, definition_id, definition_version, workspace_root,
                status, mode, current_step_key, started_at, updated_at,
                completed_at, error_summary
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(run.id.as_bytes().as_slice())
        .bind(&run.definition_id)
        .bind(run.definition_version as i64)
        .bind(run.workspace_root.to_string_lossy().as_ref())
        .bind(run.status.to_string())
        .bind(run.mode.to_string())
        .bind(&run.current_step_key)
        .bind(started_at_str)
        .bind(updated_at_str)
        .bind(completed_at_str)
        .bind(&run.error_summary)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn get_run(&self, id: WorkflowRunId) -> Result<Option<WorkflowRun>, WorkflowError> {
        let row_opt = sqlx::query(
            r#"
            SELECT id, definition_id, definition_version, workspace_root,
                   status, mode, current_step_key, started_at, updated_at,
                   completed_at, error_summary
            FROM workflow_runs
            WHERE id = ?
            "#,
        )
        .bind(id.as_bytes().as_slice())
        .fetch_optional(&self.pool)
        .await?;

        match row_opt {
            Some(row) => Ok(Some(map_row_to_workflow_run(row)?)),
            None => Ok(None),
        }
    }

    async fn update_run(&self, run: &WorkflowRun) -> Result<(), WorkflowError> {
        let updated_at_str = run.updated_at.to_rfc3339();
        let completed_at_str = run.completed_at.map(|t| t.to_rfc3339());

        let rows_affected = sqlx::query(
            r#"
            UPDATE workflow_runs
            SET definition_version = ?,
                workspace_root = ?,
                status = ?,
                mode = ?,
                current_step_key = ?,
                updated_at = ?,
                completed_at = ?,
                error_summary = ?
            WHERE id = ?
            "#,
        )
        .bind(run.definition_version as i64)
        .bind(run.workspace_root.to_string_lossy().as_ref())
        .bind(run.status.to_string())
        .bind(run.mode.to_string())
        .bind(&run.current_step_key)
        .bind(updated_at_str)
        .bind(completed_at_str)
        .bind(&run.error_summary)
        .bind(run.id.as_bytes().as_slice())
        .execute(&self.pool)
        .await?
        .rows_affected();

        if rows_affected == 0 {
            return Err(WorkflowError::UnknownWorkflow(run.id));
        }

        Ok(())
    }

    async fn create_step_run(&self, step_run: &WorkflowStepRun) -> Result<(), WorkflowError> {
        let started_at_str = step_run.started_at.to_rfc3339();
        let completed_at_str = step_run.completed_at.map(|t| t.to_rfc3339());
        let agent_bytes = step_run.assigned_agent_id.map(|id| *id.as_bytes());
        let mission_bytes = step_run.mission_id.map(|id| *id.as_bytes());

        sqlx::query(
            r#"
            INSERT INTO workflow_step_runs (
                id, workflow_run_id, step_key, status,
                assigned_agent_id, mission_id, attempt_count,
                started_at, completed_at, halt_reason
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(step_run.id.as_bytes().as_slice())
        .bind(step_run.workflow_run_id.as_bytes().as_slice())
        .bind(&step_run.step_key)
        .bind(step_run.status.to_string())
        .bind(agent_bytes.as_ref().map(|b| b.as_slice()))
        .bind(mission_bytes.as_ref().map(|b| b.as_slice()))
        .bind(step_run.attempt_count as i64)
        .bind(started_at_str)
        .bind(completed_at_str)
        .bind(&step_run.halt_reason)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn get_step_run(
        &self,
        id: WorkflowStepRunId,
    ) -> Result<Option<WorkflowStepRun>, WorkflowError> {
        let row_opt = sqlx::query(
            r#"
            SELECT id, workflow_run_id, step_key, status,
                   assigned_agent_id, mission_id, attempt_count,
                   started_at, completed_at, halt_reason
            FROM workflow_step_runs
            WHERE id = ?
            "#,
        )
        .bind(id.as_bytes().as_slice())
        .fetch_optional(&self.pool)
        .await?;

        match row_opt {
            Some(row) => Ok(Some(map_row_to_step_run(row)?)),
            None => Ok(None),
        }
    }

    async fn get_step_run_by_key(
        &self,
        run_id: WorkflowRunId,
        step_key: &str,
    ) -> Result<Option<WorkflowStepRun>, WorkflowError> {
        let row_opt = sqlx::query(
            r#"
            SELECT id, workflow_run_id, step_key, status,
                   assigned_agent_id, mission_id, attempt_count,
                   started_at, completed_at, halt_reason
            FROM workflow_step_runs
            WHERE workflow_run_id = ? AND step_key = ?
            "#,
        )
        .bind(run_id.as_bytes().as_slice())
        .bind(step_key)
        .fetch_optional(&self.pool)
        .await?;

        match row_opt {
            Some(row) => Ok(Some(map_row_to_step_run(row)?)),
            None => Ok(None),
        }
    }

    async fn update_step_run(&self, step_run: &WorkflowStepRun) -> Result<(), WorkflowError> {
        let completed_at_str = step_run.completed_at.map(|t| t.to_rfc3339());
        let agent_bytes = step_run.assigned_agent_id.map(|id| *id.as_bytes());
        let mission_bytes = step_run.mission_id.map(|id| *id.as_bytes());

        let rows_affected = sqlx::query(
            r#"
            UPDATE workflow_step_runs
            SET status = ?,
                assigned_agent_id = ?,
                mission_id = ?,
                attempt_count = ?,
                completed_at = ?,
                halt_reason = ?
            WHERE id = ?
            "#,
        )
        .bind(step_run.status.to_string())
        .bind(agent_bytes.as_ref().map(|b| b.as_slice()))
        .bind(mission_bytes.as_ref().map(|b| b.as_slice()))
        .bind(step_run.attempt_count as i64)
        .bind(completed_at_str)
        .bind(&step_run.halt_reason)
        .bind(step_run.id.as_bytes().as_slice())
        .execute(&self.pool)
        .await?
        .rows_affected();

        if rows_affected == 0 {
            return Err(WorkflowError::UnknownStep {
                run_id: step_run.workflow_run_id,
                step_key: step_run.step_key.clone(),
            });
        }

        Ok(())
    }

    async fn list_step_runs(
        &self,
        run_id: WorkflowRunId,
    ) -> Result<Vec<WorkflowStepRun>, WorkflowError> {
        let rows = sqlx::query(
            r#"
            SELECT id, workflow_run_id, step_key, status,
                   assigned_agent_id, mission_id, attempt_count,
                   started_at, completed_at, halt_reason
            FROM workflow_step_runs
            WHERE workflow_run_id = ?
            ORDER BY started_at ASC
            "#,
        )
        .bind(run_id.as_bytes().as_slice())
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter().map(map_row_to_step_run).collect()
    }

    async fn record_artifact(&self, artifact: &WorkflowArtifact) -> Result<(), WorkflowError> {
        let created_at_str = artifact.created_at.to_rfc3339();

        sqlx::query(
            r#"
            INSERT INTO workflow_artifacts (
                id, workflow_run_id, step_run_id, name,
                path, content_hash, version, status, created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET
                content_hash = excluded.content_hash,
                version = excluded.version,
                status = excluded.status
            "#,
        )
        .bind(artifact.id.as_bytes().as_slice())
        .bind(artifact.workflow_run_id.as_bytes().as_slice())
        .bind(artifact.step_run_id.as_bytes().as_slice())
        .bind(&artifact.name)
        .bind(artifact.path.to_string_lossy().as_ref())
        .bind(&artifact.content_hash)
        .bind(artifact.version as i64)
        .bind(artifact.status.to_string())
        .bind(created_at_str)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn get_artifact(
        &self,
        id: ArtifactId,
    ) -> Result<Option<WorkflowArtifact>, WorkflowError> {
        let row_opt = sqlx::query(
            r#"
            SELECT id, workflow_run_id, step_run_id, name,
                   path, content_hash, version, status, created_at
            FROM workflow_artifacts
            WHERE id = ?
            "#,
        )
        .bind(id.as_bytes().as_slice())
        .fetch_optional(&self.pool)
        .await?;

        match row_opt {
            Some(row) => Ok(Some(map_row_to_artifact(row)?)),
            None => Ok(None),
        }
    }

    async fn list_artifacts(
        &self,
        run_id: WorkflowRunId,
    ) -> Result<Vec<WorkflowArtifact>, WorkflowError> {
        let rows = sqlx::query(
            r#"
            SELECT id, workflow_run_id, step_run_id, name,
                   path, content_hash, version, status, created_at
            FROM workflow_artifacts
            WHERE workflow_run_id = ?
            ORDER BY created_at ASC
            "#,
        )
        .bind(run_id.as_bytes().as_slice())
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter().map(map_row_to_artifact).collect()
    }

    async fn list_step_artifacts(
        &self,
        step_run_id: WorkflowStepRunId,
    ) -> Result<Vec<WorkflowArtifact>, WorkflowError> {
        let rows = sqlx::query(
            r#"
            SELECT id, workflow_run_id, step_run_id, name,
                   path, content_hash, version, status, created_at
            FROM workflow_artifacts
            WHERE step_run_id = ?
            ORDER BY created_at ASC
            "#,
        )
        .bind(step_run_id.as_bytes().as_slice())
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter().map(map_row_to_artifact).collect()
    }

    async fn update_artifact_status(
        &self,
        id: ArtifactId,
        status: WorkflowArtifactStatus,
    ) -> Result<(), WorkflowError> {
        let rows_affected = sqlx::query(
            r#"
            UPDATE workflow_artifacts
            SET status = ?
            WHERE id = ?
            "#,
        )
        .bind(status.to_string())
        .bind(id.as_bytes().as_slice())
        .execute(&self.pool)
        .await?
        .rows_affected();

        if rows_affected == 0 {
            return Err(WorkflowError::PersistenceFailure(format!(
                "artifact {} not found for status update",
                id
            )));
        }

        Ok(())
    }

    async fn advance_step_and_update_run(
        &self,
        run: &WorkflowRun,
        step_run: &WorkflowStepRun,
    ) -> Result<(), WorkflowError> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;

        // 1. Update workflow run
        let updated_at_str = run.updated_at.to_rfc3339();
        let completed_at_str = run.completed_at.map(|t| t.to_rfc3339());

        let rows_affected = sqlx::query(
            r#"
            UPDATE workflow_runs
            SET definition_version = ?,
                workspace_root = ?,
                status = ?,
                mode = ?,
                current_step_key = ?,
                updated_at = ?,
                completed_at = ?,
                error_summary = ?
            WHERE id = ?
            "#,
        )
        .bind(run.definition_version as i64)
        .bind(run.workspace_root.to_string_lossy().as_ref())
        .bind(run.status.to_string())
        .bind(run.mode.to_string())
        .bind(&run.current_step_key)
        .bind(updated_at_str)
        .bind(completed_at_str)
        .bind(&run.error_summary)
        .bind(run.id.as_bytes().as_slice())
        .execute(&mut *tx)
        .await?
        .rows_affected();

        if rows_affected == 0 {
            return Err(WorkflowError::UnknownWorkflow(run.id));
        }

        // 2. Upsert step run
        let step_started_at_str = step_run.started_at.to_rfc3339();
        let step_completed_at_str = step_run.completed_at.map(|t| t.to_rfc3339());
        let agent_bytes = step_run.assigned_agent_id.map(|id| *id.as_bytes());
        let mission_bytes = step_run.mission_id.map(|id| *id.as_bytes());

        sqlx::query(
            r#"
            INSERT INTO workflow_step_runs (
                id, workflow_run_id, step_key, status,
                assigned_agent_id, mission_id, attempt_count,
                started_at, completed_at, halt_reason
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(workflow_run_id, step_key) DO UPDATE SET
                status = excluded.status,
                assigned_agent_id = excluded.assigned_agent_id,
                mission_id = excluded.mission_id,
                attempt_count = excluded.attempt_count,
                completed_at = excluded.completed_at,
                halt_reason = excluded.halt_reason
            "#,
        )
        .bind(step_run.id.as_bytes().as_slice())
        .bind(step_run.workflow_run_id.as_bytes().as_slice())
        .bind(&step_run.step_key)
        .bind(step_run.status.to_string())
        .bind(agent_bytes.as_ref().map(|b| b.as_slice()))
        .bind(mission_bytes.as_ref().map(|b| b.as_slice()))
        .bind(step_run.attempt_count as i64)
        .bind(step_started_at_str)
        .bind(step_completed_at_str)
        .bind(&step_run.halt_reason)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(())
    }

    async fn record_artifact_and_update_step(
        &self,
        artifact: &WorkflowArtifact,
        step_run: &WorkflowStepRun,
    ) -> Result<(), WorkflowError> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;

        // 1. Insert artifact
        let created_at_str = artifact.created_at.to_rfc3339();

        sqlx::query(
            r#"
            INSERT INTO workflow_artifacts (
                id, workflow_run_id, step_run_id, name,
                path, content_hash, version, status, created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(artifact.id.as_bytes().as_slice())
        .bind(artifact.workflow_run_id.as_bytes().as_slice())
        .bind(artifact.step_run_id.as_bytes().as_slice())
        .bind(&artifact.name)
        .bind(artifact.path.to_string_lossy().as_ref())
        .bind(&artifact.content_hash)
        .bind(artifact.version as i64)
        .bind(artifact.status.to_string())
        .bind(created_at_str)
        .execute(&mut *tx)
        .await?;

        // 2. Update step run
        let completed_at_str = step_run.completed_at.map(|t| t.to_rfc3339());
        let agent_bytes = step_run.assigned_agent_id.map(|id| *id.as_bytes());
        let mission_bytes = step_run.mission_id.map(|id| *id.as_bytes());

        let rows_affected = sqlx::query(
            r#"
            UPDATE workflow_step_runs
            SET status = ?,
                assigned_agent_id = ?,
                mission_id = ?,
                attempt_count = ?,
                completed_at = ?,
                halt_reason = ?
            WHERE id = ?
            "#,
        )
        .bind(step_run.status.to_string())
        .bind(agent_bytes.as_ref().map(|b| b.as_slice()))
        .bind(mission_bytes.as_ref().map(|b| b.as_slice()))
        .bind(step_run.attempt_count as i64)
        .bind(completed_at_str)
        .bind(&step_run.halt_reason)
        .bind(step_run.id.as_bytes().as_slice())
        .execute(&mut *tx)
        .await?
        .rows_affected();

        if rows_affected == 0 {
            return Err(WorkflowError::UnknownStep {
                run_id: step_run.workflow_run_id,
                step_key: step_run.step_key.clone(),
            });
        }

        tx.commit().await?;
        Ok(())
    }

    async fn list_runs_by_states(
        &self,
        states: &[WorkflowRunState],
    ) -> Result<Vec<WorkflowRun>, WorkflowError> {
        if states.is_empty() {
            return Ok(Vec::new());
        }

        let mut builder = sqlx::QueryBuilder::<sqlx::Sqlite>::new(
            r#"
            SELECT id, definition_id, definition_version, workspace_root,
                   status, mode, current_step_key, started_at, updated_at,
                   completed_at, error_summary
            FROM workflow_runs
            WHERE status IN (
            "#,
        );

        let mut separated = builder.separated(", ");
        for s in states {
            separated.push_bind(s.to_string());
        }
        separated.push_unseparated(") ORDER BY started_at ASC");

        let rows = builder.build().fetch_all(&self.pool).await.map_err(|e| {
            WorkflowError::PersistenceFailure(format!(
                "failed to query workflow runs by states: {}",
                e
            ))
        })?;

        rows.into_iter().map(map_row_to_workflow_run).collect()
    }
}

fn map_row_to_workflow_run(row: SqliteRow) -> Result<WorkflowRun, WorkflowError> {
    let id_raw: Vec<u8> = row.try_get("id")?;
    let id_bytes: [u8; 16] = id_raw.try_into().map_err(|_| {
        WorkflowError::PersistenceFailure("corrupt workflow run id BLOB".to_string())
    })?;
    let id = WorkflowRunId::from_bytes(id_bytes);

    let definition_id: String = row.try_get("definition_id")?;
    let definition_version: i64 = row.try_get("definition_version")?;
    let workspace_root_str: String = row.try_get("workspace_root")?;
    let status_str: String = row.try_get("status")?;
    let mode_str: String = row.try_get("mode")?;
    let current_step_key: Option<String> = row.try_get("current_step_key")?;

    let started_at_str: String = row.try_get("started_at")?;
    let started_at = DateTime::parse_from_rfc3339(&started_at_str)
        .map_err(|e| WorkflowError::PersistenceFailure(format!("invalid started_at: {}", e)))?
        .with_timezone(&Utc);

    let updated_at_str: String = row.try_get("updated_at")?;
    let updated_at = DateTime::parse_from_rfc3339(&updated_at_str)
        .map_err(|e| WorkflowError::PersistenceFailure(format!("invalid updated_at: {}", e)))?
        .with_timezone(&Utc);

    let completed_at_opt: Option<String> = row.try_get("completed_at")?;
    let completed_at = completed_at_opt
        .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
        .map(|dt| dt.with_timezone(&Utc));

    let error_summary: Option<String> = row.try_get("error_summary")?;

    Ok(WorkflowRun {
        id,
        definition_id,
        definition_version: definition_version as u32,
        workspace_root: PathBuf::from(workspace_root_str),
        status: WorkflowRunState::from_str(&status_str)?,
        mode: WorkflowMode::from_str(&mode_str)?,
        current_step_key,
        started_at,
        updated_at,
        completed_at,
        error_summary,
    })
}

fn map_row_to_step_run(row: SqliteRow) -> Result<WorkflowStepRun, WorkflowError> {
    let id_raw: Vec<u8> = row.try_get("id")?;
    let id_bytes: [u8; 16] = id_raw.try_into().map_err(|_| {
        WorkflowError::PersistenceFailure("corrupt workflow step run id BLOB".to_string())
    })?;
    let id = WorkflowStepRunId::from_bytes(id_bytes);

    let run_id_raw: Vec<u8> = row.try_get("workflow_run_id")?;
    let run_id_bytes: [u8; 16] = run_id_raw.try_into().map_err(|_| {
        WorkflowError::PersistenceFailure("corrupt workflow_run_id BLOB in step".to_string())
    })?;
    let workflow_run_id = WorkflowRunId::from_bytes(run_id_bytes);

    let step_key: String = row.try_get("step_key")?;
    let status_str: String = row.try_get("status")?;

    let agent_raw: Option<Vec<u8>> = row.try_get("assigned_agent_id")?;
    let assigned_agent_id = agent_raw.and_then(|b| {
        let arr: Result<[u8; 16], _> = b.try_into();
        arr.ok().map(AgentId::from_bytes)
    });

    let mission_raw: Option<Vec<u8>> = row.try_get("mission_id")?;
    let mission_id = mission_raw.and_then(|b| {
        let arr: Result<[u8; 16], _> = b.try_into();
        arr.ok().map(MissionId::from_bytes)
    });

    let attempt_count: i64 = row.try_get("attempt_count")?;

    let started_at_str: String = row.try_get("started_at")?;
    let started_at = DateTime::parse_from_rfc3339(&started_at_str)
        .map_err(|e| WorkflowError::PersistenceFailure(format!("invalid step started_at: {}", e)))?
        .with_timezone(&Utc);

    let completed_at_opt: Option<String> = row.try_get("completed_at")?;
    let completed_at = completed_at_opt
        .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
        .map(|dt| dt.with_timezone(&Utc));

    let halt_reason: Option<String> = row.try_get("halt_reason")?;

    Ok(WorkflowStepRun {
        id,
        workflow_run_id,
        step_key,
        status: WorkflowStepState::from_str(&status_str)?,
        assigned_agent_id,
        mission_id,
        attempt_count: attempt_count as u32,
        started_at,
        completed_at,
        halt_reason,
    })
}

fn map_row_to_artifact(row: SqliteRow) -> Result<WorkflowArtifact, WorkflowError> {
    let id_raw: Vec<u8> = row.try_get("id")?;
    let id_bytes: [u8; 16] = id_raw
        .try_into()
        .map_err(|_| WorkflowError::PersistenceFailure("corrupt artifact id BLOB".to_string()))?;
    let id = ArtifactId::from_bytes(id_bytes);

    let run_id_raw: Vec<u8> = row.try_get("workflow_run_id")?;
    let run_id_bytes: [u8; 16] = run_id_raw.try_into().map_err(|_| {
        WorkflowError::PersistenceFailure("corrupt workflow_run_id BLOB in artifact".to_string())
    })?;
    let workflow_run_id = WorkflowRunId::from_bytes(run_id_bytes);

    let step_id_raw: Vec<u8> = row.try_get("step_run_id")?;
    let step_id_bytes: [u8; 16] = step_id_raw.try_into().map_err(|_| {
        WorkflowError::PersistenceFailure("corrupt step_run_id BLOB in artifact".to_string())
    })?;
    let step_run_id = WorkflowStepRunId::from_bytes(step_id_bytes);

    let name: String = row.try_get("name")?;
    let path_str: String = row.try_get("path")?;
    let content_hash: String = row.try_get("content_hash")?;
    let version: i64 = row.try_get("version")?;
    let status_str: String = row.try_get("status")?;

    let created_at_str: String = row.try_get("created_at")?;
    let created_at = DateTime::parse_from_rfc3339(&created_at_str)
        .map_err(|e| {
            WorkflowError::PersistenceFailure(format!("invalid artifact created_at: {}", e))
        })?
        .with_timezone(&Utc);

    Ok(WorkflowArtifact {
        id,
        workflow_run_id,
        step_run_id,
        name,
        path: PathBuf::from(path_str),
        content_hash,
        version: version as u32,
        status: WorkflowArtifactStatus::from_str(&status_str)?,
        created_at,
    })
}
