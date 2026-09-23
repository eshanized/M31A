//! SQLite-backed Git commit attribution persistence (GST-03, D-03).

use chrono::{DateTime, Utc};
use sqlx::{Row, SqlitePool};

use crate::git::GitError;
use crate::git::trailers::CommitTrailers;
use crate::ids::{MissionId, TaskId};

/// Entity representing a persisted Git commit attribution record in SQLite.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CommitAttributionRecord {
    pub commit_hash: String,
    pub mission_id: String,
    pub task_id: String,
    pub agent_role: String,
    pub model_id: String,
    pub verification_run_id: Option<String>,
    pub created_at: String,
}

/// Store for recording and querying Git commit provenance.
#[derive(Clone)]
pub struct GitAttributionStore {
    pool: SqlitePool,
}

impl GitAttributionStore {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    /// Record a commit's attribution metadata into SQLite.
    pub async fn record_commit(
        &self,
        commit_hash: &str,
        trailers: &CommitTrailers,
        created_at: Option<DateTime<Utc>>,
    ) -> Result<CommitAttributionRecord, GitError> {
        let ts = created_at.unwrap_or_else(Utc::now).to_rfc3339();
        let record = CommitAttributionRecord {
            commit_hash: commit_hash.to_string(),
            mission_id: trailers.mission_id.to_string(),
            task_id: trailers.task_id.to_string(),
            agent_role: trailers.agent_role.to_string(),
            model_id: trailers.model_id.clone(),
            verification_run_id: trailers.verification_run_id.clone(),
            created_at: ts,
        };

        sqlx::query(
            r#"
            INSERT INTO git_commits (
                commit_hash,
                mission_id,
                task_id,
                agent_role,
                model_id,
                verification_run_id,
                created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            ON CONFLICT(commit_hash) DO UPDATE SET
                mission_id = excluded.mission_id,
                task_id = excluded.task_id,
                agent_role = excluded.agent_role,
                model_id = excluded.model_id,
                verification_run_id = excluded.verification_run_id,
                created_at = excluded.created_at
            "#,
        )
        .bind(&record.commit_hash)
        .bind(&record.mission_id)
        .bind(&record.task_id)
        .bind(&record.agent_role)
        .bind(&record.model_id)
        .bind(&record.verification_run_id)
        .bind(&record.created_at)
        .execute(&self.pool)
        .await
        .map_err(|e| GitError::Database(format!("Failed to record git commit attribution: {e}")))?;

        Ok(record)
    }

    /// Query attribution record by commit hash.
    pub async fn find_by_commit(
        &self,
        commit_hash: &str,
    ) -> Result<Option<CommitAttributionRecord>, GitError> {
        let row_opt = sqlx::query(
            r#"
            SELECT
                commit_hash,
                mission_id,
                task_id,
                agent_role,
                model_id,
                verification_run_id,
                created_at
            FROM git_commits
            WHERE commit_hash = ?1
            "#,
        )
        .bind(commit_hash)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| GitError::Database(format!("Failed to query git commit: {e}")))?;

        Ok(row_opt.map(|row| CommitAttributionRecord {
            commit_hash: row.get("commit_hash"),
            mission_id: row.get("mission_id"),
            task_id: row.get("task_id"),
            agent_role: row.get("agent_role"),
            model_id: row.get("model_id"),
            verification_run_id: row.get("verification_run_id"),
            created_at: row.get("created_at"),
        }))
    }

    /// Query all commits attributed to a specific task.
    pub async fn find_by_task(
        &self,
        task_id: &TaskId,
    ) -> Result<Vec<CommitAttributionRecord>, GitError> {
        let task_str = task_id.to_string();
        let rows = sqlx::query(
            r#"
            SELECT
                commit_hash,
                mission_id,
                task_id,
                agent_role,
                model_id,
                verification_run_id,
                created_at
            FROM git_commits
            WHERE task_id = ?1
            ORDER BY created_at ASC
            "#,
        )
        .bind(task_str)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| GitError::Database(format!("Failed to query git commits by task: {e}")))?;

        Ok(rows
            .into_iter()
            .map(|row| CommitAttributionRecord {
                commit_hash: row.get("commit_hash"),
                mission_id: row.get("mission_id"),
                task_id: row.get("task_id"),
                agent_role: row.get("agent_role"),
                model_id: row.get("model_id"),
                verification_run_id: row.get("verification_run_id"),
                created_at: row.get("created_at"),
            })
            .collect())
    }

    /// Query all commits attributed to a specific mission.
    pub async fn find_by_mission(
        &self,
        mission_id: &MissionId,
    ) -> Result<Vec<CommitAttributionRecord>, GitError> {
        let mission_str = mission_id.to_string();
        let rows = sqlx::query(
            r#"
            SELECT
                commit_hash,
                mission_id,
                task_id,
                agent_role,
                model_id,
                verification_run_id,
                created_at
            FROM git_commits
            WHERE mission_id = ?1
            ORDER BY created_at ASC
            "#,
        )
        .bind(mission_str)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| GitError::Database(format!("Failed to query git commits by mission: {e}")))?;

        Ok(rows
            .into_iter()
            .map(|row| CommitAttributionRecord {
                commit_hash: row.get("commit_hash"),
                mission_id: row.get("mission_id"),
                task_id: row.get("task_id"),
                agent_role: row.get("agent_role"),
                model_id: row.get("model_id"),
                verification_run_id: row.get("verification_run_id"),
                created_at: row.get("created_at"),
            })
            .collect())
    }

    /// Query all commits attributed to a specific model.
    pub async fn find_by_model(
        &self,
        model_id: &str,
    ) -> Result<Vec<CommitAttributionRecord>, GitError> {
        let rows = sqlx::query(
            r#"
            SELECT
                commit_hash,
                mission_id,
                task_id,
                agent_role,
                model_id,
                verification_run_id,
                created_at
            FROM git_commits
            WHERE model_id = ?1
            ORDER BY created_at ASC
            "#,
        )
        .bind(model_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| GitError::Database(format!("Failed to query git commits by model: {e}")))?;

        Ok(rows
            .into_iter()
            .map(|row| CommitAttributionRecord {
                commit_hash: row.get("commit_hash"),
                mission_id: row.get("mission_id"),
                task_id: row.get("task_id"),
                agent_role: row.get("agent_role"),
                model_id: row.get("model_id"),
                verification_run_id: row.get("verification_run_id"),
                created_at: row.get("created_at"),
            })
            .collect())
    }
}
