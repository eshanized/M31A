//! SQLite implementation of TaskRepository (PST-01, D-13, DAG-05).

use crate::error::M31AError;
use crate::ids::{MissionId, TaskId};
use crate::persistence::sqlite::repositories::TaskRepository;
use crate::persistence::sqlite::repositories::task_graph::map_row_to_task;
use crate::state::Task;
use crate::state::task::{BlockingReason, TaskResult};
use crate::state_machine::TaskState;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{Row, SqlitePool};
use std::collections::BTreeMap;
use std::str::FromStr;

/// SQLite repository for Task persistence.
#[derive(Debug, Clone)]
pub struct SqliteTaskRepository {
    pool: SqlitePool,
}

impl SqliteTaskRepository {
    /// Create a new `SqliteTaskRepository`.
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Access the underlying connection pool.
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    /// Insert a new task.
    pub async fn insert(&self, task: &Task) -> Result<(), M31AError> {
        <Self as TaskRepository>::insert(self, task).await
    }

    /// Get a task by ID.
    pub async fn get(&self, id: TaskId) -> Result<Option<Task>, M31AError> {
        <Self as TaskRepository>::get(self, id).await
    }

    /// Update task status.
    pub async fn update_status(&self, id: TaskId, status: TaskState) -> Result<(), M31AError> {
        <Self as TaskRepository>::update_status(self, id, status).await
    }

    /// List all tasks for a mission.
    pub async fn list_by_mission(&self, mission_id: MissionId) -> Result<Vec<Task>, M31AError> {
        <Self as TaskRepository>::list_by_mission(self, mission_id).await
    }

    /// Count total tasks for a mission.
    pub async fn count_by_mission(&self, mission_id: MissionId) -> Result<usize, M31AError> {
        <Self as TaskRepository>::count_by_mission(self, mission_id).await
    }

    /// Count completed/succeeded tasks for a mission.
    pub async fn count_completed_by_mission(
        &self,
        mission_id: MissionId,
    ) -> Result<usize, M31AError> {
        <Self as TaskRepository>::count_completed_by_mission(self, mission_id).await
    }

    /// Retrieve task states map for a mission.
    pub async fn get_task_states_by_mission(
        &self,
        mission_id: MissionId,
    ) -> Result<BTreeMap<TaskId, TaskState>, M31AError> {
        <Self as TaskRepository>::get_task_states_by_mission(self, mission_id).await
    }

    /// Update max_retries for a task by mission_id and candidate_key.
    pub async fn update_max_retries_by_candidate_key(
        &self,
        mission_id: MissionId,
        candidate_key: &str,
        max_retries: u32,
    ) -> Result<(), M31AError> {
        <Self as TaskRepository>::update_max_retries_by_candidate_key(
            self,
            mission_id,
            candidate_key,
            max_retries,
        )
        .await
    }

    /// List candidate key summaries (candidate_key, status, retry_count) for a mission.
    pub async fn list_candidate_summaries_by_mission(
        &self,
        mission_id: MissionId,
    ) -> Result<Vec<(String, String, i64)>, M31AError> {
        <Self as TaskRepository>::list_candidate_summaries_by_mission(self, mission_id).await
    }

    /// Mark task succeeded with result.
    pub async fn mark_succeeded(&self, id: TaskId, result: &TaskResult) -> Result<(), M31AError> {
        <Self as TaskRepository>::mark_succeeded(self, id, result).await
    }

    /// Mark task ready.
    pub async fn mark_ready(&self, id: TaskId) -> Result<(), M31AError> {
        <Self as TaskRepository>::mark_ready(self, id).await
    }

    /// Mark task for retry with updated count.
    pub async fn mark_retry(&self, id: TaskId, retry_count: u32) -> Result<(), M31AError> {
        <Self as TaskRepository>::mark_retry(self, id, retry_count).await
    }

    /// Mark task failed with timestamp.
    pub async fn mark_failed(
        &self,
        id: TaskId,
        completed_at: DateTime<Utc>,
    ) -> Result<(), M31AError> {
        <Self as TaskRepository>::mark_failed(self, id, completed_at).await
    }

    /// Mark task blocked with reason.
    pub async fn mark_blocked(&self, id: TaskId, reason: &BlockingReason) -> Result<(), M31AError> {
        <Self as TaskRepository>::mark_blocked(self, id, reason).await
    }

    /// Mark task cancelled with timestamp.
    pub async fn mark_cancelled(
        &self,
        id: TaskId,
        completed_at: DateTime<Utc>,
    ) -> Result<(), M31AError> {
        <Self as TaskRepository>::mark_cancelled(self, id, completed_at).await
    }

    /// Mark task skipped with reason.
    pub async fn mark_skipped(&self, id: TaskId, reason: &str) -> Result<(), M31AError> {
        <Self as TaskRepository>::mark_skipped(self, id, reason).await
    }

    /// Mark task running with start timestamp.
    pub async fn mark_running(
        &self,
        id: TaskId,
        started_at: DateTime<Utc>,
    ) -> Result<(), M31AError> {
        <Self as TaskRepository>::mark_running(self, id, started_at).await
    }

    /// Mark task as needs review.
    pub async fn mark_needs_review(&self, id: TaskId) -> Result<(), M31AError> {
        <Self as TaskRepository>::mark_needs_review(self, id).await
    }

    /// Reset any running tasks for a mission to pending (for crash recovery).
    pub async fn reset_running_tasks_to_pending(
        &self,
        mission_id: MissionId,
    ) -> Result<usize, M31AError> {
        <Self as TaskRepository>::reset_running_tasks_to_pending(self, mission_id).await
    }

    /// Reset a single task to pending status, clearing completion timestamp (for resume/invalidation).
    pub async fn reset_task_to_pending(&self, id: TaskId) -> Result<(), M31AError> {
        <Self as TaskRepository>::reset_task_to_pending(self, id).await
    }

    /// Get max_retries for a task by ID.
    pub async fn get_max_retries(&self, id: TaskId) -> Result<Option<u32>, M31AError> {
        <Self as TaskRepository>::get_max_retries(self, id).await
    }
}

#[async_trait]
impl TaskRepository for SqliteTaskRepository {
    async fn insert(&self, task: &Task) -> Result<(), M31AError> {
        let capabilities_json = serde_json::to_string(&task.capabilities)
            .map_err(|e| M31AError::persistence(e.to_string()))?;
        let verification_json = serde_json::to_string(&task.verification)
            .map_err(|e| M31AError::persistence(e.to_string()))?;
        let estimates_json = serde_json::to_string(&task.estimates)
            .map_err(|e| M31AError::persistence(e.to_string()))?;
        let blocking_json = task
            .blocking_reason
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(|e| M31AError::persistence(e.to_string()))?;
        let result_json = task
            .result
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(|e| M31AError::persistence(e.to_string()))?;
        let graph_id_bytes = task.task_graph_id.map(|g| g.as_bytes().to_vec());
        let started_at_str = task.started_at.map(|t| t.to_rfc3339());
        let completed_at_str = task.completed_at.map(|t| t.to_rfc3339());
        let criteria_json = serde_json::to_string(&task.completion_criteria).unwrap_or_default();
        let req_keys_json = serde_json::to_string(&task.requirement_keys).unwrap_or_default();
        let assumptions_json = serde_json::to_string(&task.assumptions).unwrap_or_default();

        sqlx::query(
            r#"
            INSERT INTO tasks (
                id, mission_id, task_graph_id, candidate_key, title, role, status,
                priority, max_retries, retry_count, capabilities, verification,
                estimates, blocking_reason, fingerprint, result, created_at, updated_at,
                started_at, completed_at,
                description, completion_criteria, requirement_keys, assumptions,
                prompt_ref_id, prompt_ref_version
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET
                task_graph_id = excluded.task_graph_id,
                candidate_key = excluded.candidate_key,
                title = excluded.title,
                role = excluded.role,
                status = excluded.status,
                priority = excluded.priority,
                max_retries = excluded.max_retries,
                retry_count = excluded.retry_count,
                capabilities = excluded.capabilities,
                verification = excluded.verification,
                estimates = excluded.estimates,
                blocking_reason = excluded.blocking_reason,
                fingerprint = excluded.fingerprint,
                result = excluded.result,
                updated_at = excluded.updated_at,
                started_at = excluded.started_at,
                completed_at = excluded.completed_at,
                description = excluded.description,
                completion_criteria = excluded.completion_criteria,
                requirement_keys = excluded.requirement_keys,
                assumptions = excluded.assumptions,
                prompt_ref_id = excluded.prompt_ref_id,
                prompt_ref_version = excluded.prompt_ref_version
            "#,
        )
        .bind(task.id.as_bytes().as_slice())
        .bind(task.mission_id.as_bytes().as_slice())
        .bind(graph_id_bytes.as_deref())
        .bind(&task.candidate_key)
        .bind(&task.title)
        .bind(task.role.as_str())
        .bind(task.status.to_string().to_lowercase())
        .bind(task.priority as i64)
        .bind(task.max_retries as i64)
        .bind(task.retry_count as i64)
        .bind(&capabilities_json)
        .bind(&verification_json)
        .bind(&estimates_json)
        .bind(blocking_json.as_deref())
        .bind(&task.fingerprint)
        .bind(result_json.as_deref())
        .bind(task.created_at.to_rfc3339())
        .bind(task.updated_at.to_rfc3339())
        .bind(started_at_str.as_deref())
        .bind(completed_at_str.as_deref())
        .bind(task.description.as_deref())
        .bind(&criteria_json)
        .bind(&req_keys_json)
        .bind(&assumptions_json)
        .bind(task.prompt_ref.as_ref().map(|r| r.id.clone()))
        .bind(task.prompt_ref.as_ref().map(|r| r.version as i64))
        .execute(&self.pool)
        .await
        .map_err(|e| M31AError::persistence(e.to_string()))?;

        Ok(())
    }

    async fn get(&self, id: TaskId) -> Result<Option<Task>, M31AError> {
        let row = sqlx::query(
            r#"
            SELECT id, mission_id, task_graph_id, candidate_key, title, role, status,
                   priority, max_retries, retry_count, capabilities, verification,
                   estimates, blocking_reason, fingerprint, result, created_at, updated_at,
                   started_at, completed_at, description, completion_criteria,
                   requirement_keys, assumptions, prompt_ref_id,
                   prompt_ref_version
            FROM tasks
            WHERE id = ?
            "#,
        )
        .bind(id.as_bytes().as_slice())
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| M31AError::persistence(e.to_string()))?;

        row.map(map_row_to_task).transpose()
    }

    async fn update_status(&self, id: TaskId, status: TaskState) -> Result<(), M31AError> {
        let now = Utc::now().to_rfc3339();
        let rows_affected = sqlx::query("UPDATE tasks SET status = ?, updated_at = ? WHERE id = ?")
            .bind(status.to_string().to_lowercase())
            .bind(&now)
            .bind(id.as_bytes().as_slice())
            .execute(&self.pool)
            .await
            .map_err(|e| M31AError::persistence(e.to_string()))?
            .rows_affected();

        if rows_affected == 0 {
            return Err(M31AError::not_found(format!("Task {id}")));
        }
        Ok(())
    }

    async fn list_by_mission(&self, mission_id: MissionId) -> Result<Vec<Task>, M31AError> {
        let rows = sqlx::query(
            r#"
            SELECT id, mission_id, task_graph_id, candidate_key, title, role, status,
                   priority, max_retries, retry_count, capabilities, verification,
                   estimates, blocking_reason, fingerprint, result, created_at, updated_at,
                   started_at, completed_at, description, completion_criteria,
                   requirement_keys, assumptions, prompt_ref_id,
                   prompt_ref_version
            FROM tasks
            WHERE mission_id = ?
            ORDER BY priority DESC, id ASC
            "#,
        )
        .bind(mission_id.as_bytes().as_slice())
        .fetch_all(&self.pool)
        .await
        .map_err(|e| M31AError::persistence(e.to_string()))?;

        rows.into_iter().map(map_row_to_task).collect()
    }

    async fn count_by_mission(&self, mission_id: MissionId) -> Result<usize, M31AError> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tasks WHERE mission_id = ?")
            .bind(mission_id.as_bytes().as_slice())
            .fetch_one(&self.pool)
            .await
            .map_err(|e| M31AError::persistence(e.to_string()))?;

        Ok(count as usize)
    }

    async fn count_completed_by_mission(&self, mission_id: MissionId) -> Result<usize, M31AError> {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM tasks WHERE mission_id = ? AND LOWER(status) IN ('completed', 'succeeded')",
        )
        .bind(mission_id.as_bytes().as_slice())
        .fetch_one(&self.pool)
        .await
        .map_err(|e| M31AError::persistence(e.to_string()))?;

        Ok(count as usize)
    }

    async fn get_task_states_by_mission(
        &self,
        mission_id: MissionId,
    ) -> Result<BTreeMap<TaskId, TaskState>, M31AError> {
        let rows = sqlx::query("SELECT id, status FROM tasks WHERE mission_id = ?")
            .bind(mission_id.as_bytes().as_slice())
            .fetch_all(&self.pool)
            .await
            .map_err(|e| M31AError::persistence(e.to_string()))?;

        let mut map = BTreeMap::new();
        for row in rows {
            let id_bytes: Vec<u8> = row
                .try_get("id")
                .map_err(|e| M31AError::persistence(e.to_string()))?;
            if id_bytes.len() != 16 {
                return Err(M31AError::persistence(format!(
                    "corrupt task identity: expected 16 bytes, got {}",
                    id_bytes.len()
                )));
            }
            let mut arr = [0u8; 16];
            arr.copy_from_slice(&id_bytes);
            let tid = TaskId::from_bytes(arr);
            let status_str: String = row
                .try_get("status")
                .map_err(|e| M31AError::persistence(e.to_string()))?;
            // Corrupt status fails closed with a typed persistence error
            // instead of silently omitting the task from checkpoint snapshots
            // (which would let resume resurrect a false-coherent task set).
            let state = TaskState::from_str(&status_str).map_err(|e| {
                M31AError::persistence(format!("corrupt task status '{status_str}': {e}"))
            })?;
            map.insert(tid, state);
        }
        Ok(map)
    }

    async fn update_max_retries_by_candidate_key(
        &self,
        mission_id: MissionId,
        candidate_key: &str,
        max_retries: u32,
    ) -> Result<(), M31AError> {
        sqlx::query("UPDATE tasks SET max_retries = ? WHERE mission_id = ? AND candidate_key = ?")
            .bind(max_retries as i64)
            .bind(mission_id.as_bytes().as_slice())
            .bind(candidate_key)
            .execute(&self.pool)
            .await
            .map_err(|e| M31AError::persistence(e.to_string()))?;

        Ok(())
    }

    async fn list_candidate_summaries_by_mission(
        &self,
        mission_id: MissionId,
    ) -> Result<Vec<(String, String, i64)>, M31AError> {
        let rows = sqlx::query_as::<_, (String, String, i64)>(
            "SELECT candidate_key, status, retry_count FROM tasks WHERE mission_id = ?",
        )
        .bind(mission_id.as_bytes().as_slice())
        .fetch_all(&self.pool)
        .await
        .map_err(|e| M31AError::persistence(e.to_string()))?;

        Ok(rows)
    }

    async fn mark_succeeded(&self, id: TaskId, result: &TaskResult) -> Result<(), M31AError> {
        let now = Utc::now().to_rfc3339();
        let result_json = serde_json::to_string(result).unwrap_or_default();
        sqlx::query(
            "UPDATE tasks SET status = 'succeeded', completed_at = ?, result = ?, updated_at = ? WHERE id = ?",
        )
        .bind(&now)
        .bind(&result_json)
        .bind(&now)
        .bind(id.as_bytes().as_slice())
        .execute(&self.pool)
        .await
        .map_err(|e| M31AError::persistence(e.to_string()))?;

        Ok(())
    }

    async fn mark_ready(&self, id: TaskId) -> Result<(), M31AError> {
        let now = Utc::now().to_rfc3339();
        sqlx::query("UPDATE tasks SET status = 'ready', updated_at = ? WHERE id = ?")
            .bind(&now)
            .bind(id.as_bytes().as_slice())
            .execute(&self.pool)
            .await
            .map_err(|e| M31AError::persistence(e.to_string()))?;

        Ok(())
    }

    async fn mark_retry(&self, id: TaskId, retry_count: u32) -> Result<(), M31AError> {
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "UPDATE tasks SET status = 'ready', retry_count = ?, updated_at = ? WHERE id = ?",
        )
        .bind(retry_count as i64)
        .bind(&now)
        .bind(id.as_bytes().as_slice())
        .execute(&self.pool)
        .await
        .map_err(|e| M31AError::persistence(e.to_string()))?;

        Ok(())
    }

    async fn mark_failed(&self, id: TaskId, completed_at: DateTime<Utc>) -> Result<(), M31AError> {
        let now = Utc::now().to_rfc3339();
        let completed_str = completed_at.to_rfc3339();
        sqlx::query(
            "UPDATE tasks SET status = 'failed', completed_at = ?, updated_at = ? WHERE id = ?",
        )
        .bind(&completed_str)
        .bind(&now)
        .bind(id.as_bytes().as_slice())
        .execute(&self.pool)
        .await
        .map_err(|e| M31AError::persistence(e.to_string()))?;

        Ok(())
    }

    async fn mark_blocked(&self, id: TaskId, reason: &BlockingReason) -> Result<(), M31AError> {
        let now = Utc::now().to_rfc3339();
        let reason_json = serde_json::to_string(reason).unwrap_or_default();
        sqlx::query(
            "UPDATE tasks SET status = 'blocked', blocking_reason = ?, updated_at = ? WHERE id = ?",
        )
        .bind(&reason_json)
        .bind(&now)
        .bind(id.as_bytes().as_slice())
        .execute(&self.pool)
        .await
        .map_err(|e| M31AError::persistence(e.to_string()))?;

        Ok(())
    }

    async fn mark_cancelled(
        &self,
        id: TaskId,
        completed_at: DateTime<Utc>,
    ) -> Result<(), M31AError> {
        let now = Utc::now().to_rfc3339();
        let completed_str = completed_at.to_rfc3339();
        sqlx::query(
            "UPDATE tasks SET status = 'cancelled', completed_at = ?, updated_at = ? WHERE id = ?",
        )
        .bind(&completed_str)
        .bind(&now)
        .bind(id.as_bytes().as_slice())
        .execute(&self.pool)
        .await
        .map_err(|e| M31AError::persistence(e.to_string()))?;

        Ok(())
    }

    async fn mark_skipped(&self, id: TaskId, _reason: &str) -> Result<(), M31AError> {
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "UPDATE tasks SET status = 'skipped', completed_at = ?, updated_at = ? WHERE id = ?",
        )
        .bind(&now)
        .bind(&now)
        .bind(id.as_bytes().as_slice())
        .execute(&self.pool)
        .await
        .map_err(|e| M31AError::persistence(e.to_string()))?;

        Ok(())
    }

    async fn mark_running(&self, id: TaskId, started_at: DateTime<Utc>) -> Result<(), M31AError> {
        let now = Utc::now().to_rfc3339();
        let started_str = started_at.to_rfc3339();
        sqlx::query(
            "UPDATE tasks SET status = 'running', started_at = ?, updated_at = ? WHERE id = ?",
        )
        .bind(&started_str)
        .bind(&now)
        .bind(id.as_bytes().as_slice())
        .execute(&self.pool)
        .await
        .map_err(|e| M31AError::persistence(e.to_string()))?;

        Ok(())
    }

    async fn mark_needs_review(&self, id: TaskId) -> Result<(), M31AError> {
        let now = Utc::now().to_rfc3339();
        sqlx::query("UPDATE tasks SET status = 'needs_review', updated_at = ? WHERE id = ?")
            .bind(&now)
            .bind(id.as_bytes().as_slice())
            .execute(&self.pool)
            .await
            .map_err(|e| M31AError::persistence(e.to_string()))?;

        Ok(())
    }

    async fn reset_running_tasks_to_pending(
        &self,
        mission_id: MissionId,
    ) -> Result<usize, M31AError> {
        let res = sqlx::query(
            "UPDATE tasks SET status = 'pending' WHERE mission_id = ? AND LOWER(status) = 'running'",
        )
        .bind(mission_id.as_bytes().as_slice())
        .execute(&self.pool)
        .await
        .map_err(|e| M31AError::persistence(e.to_string()))?;

        Ok(res.rows_affected() as usize)
    }

    async fn reset_task_to_pending(&self, id: TaskId) -> Result<(), M31AError> {
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "UPDATE tasks SET status = 'pending', completed_at = NULL, updated_at = ? WHERE id = ?",
        )
        .bind(&now)
        .bind(id.as_bytes().as_slice())
        .execute(&self.pool)
        .await
        .map_err(|e| M31AError::persistence(e.to_string()))?;

        Ok(())
    }

    async fn get_max_retries(&self, id: TaskId) -> Result<Option<u32>, M31AError> {
        let row: Option<i64> = sqlx::query_scalar("SELECT max_retries FROM tasks WHERE id = ?")
            .bind(id.as_bytes().as_slice())
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| M31AError::persistence(e.to_string()))?;

        Ok(row.map(|r| r as u32))
    }
}
