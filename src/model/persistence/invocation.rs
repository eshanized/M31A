//! Durable SQLite repository for ModelInvocation records and token usage telemetry (D-08, MDL-05).
//!
//! Maps to the `model_invocations` table created in Migration 005.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::sqlite::SqliteRow;
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

use crate::ids::{AgentId, MissionId, TaskId};
use crate::model::types::{TokenUsage, UsageSource};

/// Durable record of an authoritative model invocation attempt (D-08, MDL-05).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelInvocationRecord {
    pub id: Uuid,
    pub mission_id: MissionId,
    pub task_id: TaskId,
    pub agent_id: AgentId,
    pub step_number: u32,
    pub provider: String,
    pub model_name: String,
    pub attempt_number: u32,
    pub outcome: String,
    pub prompt_tokens: usize,
    pub completion_tokens: usize,
    pub total_tokens: usize,
    pub usage_source: String,
    pub routing_reason: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt_provenance: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl ModelInvocationRecord {
    /// Construct a new ModelInvocationRecord with UUIDv7 identity and current timestamp.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        mission_id: MissionId,
        task_id: TaskId,
        agent_id: AgentId,
        step_number: u32,
        provider: impl Into<String>,
        model_name: impl Into<String>,
        attempt_number: u32,
        outcome: impl Into<String>,
        usage: &TokenUsage,
        routing_reason: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::now_v7(),
            mission_id,
            task_id,
            agent_id,
            step_number,
            provider: provider.into(),
            model_name: model_name.into(),
            attempt_number,
            outcome: outcome.into(),
            prompt_tokens: usage.prompt_tokens,
            completion_tokens: usage.completion_tokens,
            total_tokens: usage.total_tokens,
            usage_source: match usage.source {
                UsageSource::AuthoritativeProvider => "authoritative_provider".to_string(),
                UsageSource::Estimated => "estimated".to_string(),
            },
            routing_reason: routing_reason.into(),
            prompt_provenance: None,
            created_at: Utc::now(),
        }
    }

    /// Builder method to attach prompt provenance JSON metadata.
    pub fn with_prompt_provenance(mut self, provenance: impl Into<String>) -> Self {
        self.prompt_provenance = Some(provenance.into());
        self
    }
}

/// Transactional SQLite repository for model invocation telemetry.
#[derive(Debug, Clone)]
pub struct SqliteModelInvocationRepository {
    pool: SqlitePool,
}

impl SqliteModelInvocationRepository {
    /// Create a new repository bound to a SQLite pool.
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Insert an authoritative model invocation record transactionally (D-08).
    pub async fn insert_invocation(
        &self,
        record: &ModelInvocationRecord,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            INSERT INTO model_invocations (
                id, mission_id, task_id, agent_id, step_number, provider,
                model_name, attempt_number, outcome, prompt_tokens, completion_tokens,
                total_tokens, usage_source, routing_reason, prompt_provenance, created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(record.id.as_bytes().as_slice())
        .bind(record.mission_id.as_bytes().as_slice())
        .bind(record.task_id.as_bytes().as_slice())
        .bind(record.agent_id.as_bytes().as_slice())
        .bind(record.step_number as i64)
        .bind(&record.provider)
        .bind(&record.model_name)
        .bind(record.attempt_number as i64)
        .bind(&record.outcome)
        .bind(record.prompt_tokens as i64)
        .bind(record.completion_tokens as i64)
        .bind(record.total_tokens as i64)
        .bind(&record.usage_source)
        .bind(&record.routing_reason)
        .bind(record.prompt_provenance.as_deref())
        .bind(record.created_at.to_rfc3339())
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Retrieve all invocation records recorded for a given task, ordered by step and attempt.
    pub async fn get_invocations_for_task(
        &self,
        task_id: &TaskId,
    ) -> Result<Vec<ModelInvocationRecord>, sqlx::Error> {
        let rows = sqlx::query(
            r#"
            SELECT id, mission_id, task_id, agent_id, step_number, provider,
                   model_name, attempt_number, outcome, prompt_tokens, completion_tokens,
                   total_tokens, usage_source, routing_reason, prompt_provenance, created_at
            FROM model_invocations
            WHERE task_id = ?
            ORDER BY step_number ASC, attempt_number ASC, created_at ASC
            "#,
        )
        .bind(task_id.as_bytes().as_slice())
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter().map(map_row_to_invocation).collect()
    }

    /// Retrieve all invocation records recorded for a given mission.
    pub async fn get_invocations_for_mission(
        &self,
        mission_id: &MissionId,
    ) -> Result<Vec<ModelInvocationRecord>, sqlx::Error> {
        let rows = sqlx::query(
            r#"
            SELECT id, mission_id, task_id, agent_id, step_number, provider,
                   model_name, attempt_number, outcome, prompt_tokens, completion_tokens,
                   total_tokens, usage_source, routing_reason, prompt_provenance, created_at
            FROM model_invocations
            WHERE mission_id = ?
            ORDER BY created_at ASC
            "#,
        )
        .bind(mission_id.as_bytes().as_slice())
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter().map(map_row_to_invocation).collect()
    }

    /// Aggregate total token usage across all invocations for a mission.
    pub async fn get_total_token_usage_for_mission(
        &self,
        mission_id: &MissionId,
    ) -> Result<TokenUsage, sqlx::Error> {
        let row = sqlx::query(
            r#"
            SELECT 
                COALESCE(SUM(prompt_tokens), 0) AS total_prompt,
                COALESCE(SUM(completion_tokens), 0) AS total_completion,
                COALESCE(SUM(total_tokens), 0) AS total_all
            FROM model_invocations
            WHERE mission_id = ?
            "#,
        )
        .bind(mission_id.as_bytes().as_slice())
        .fetch_one(&self.pool)
        .await?;

        let prompt_tokens: i64 = row.try_get("total_prompt")?;
        let completion_tokens: i64 = row.try_get("total_completion")?;
        let total_tokens: i64 = row.try_get("total_all")?;

        Ok(TokenUsage::new(
            prompt_tokens.max(0) as usize,
            completion_tokens.max(0) as usize,
            total_tokens.max(0) as usize,
            0,
            UsageSource::AuthoritativeProvider,
        ))
    }

    /// Retrieve all invocation records recorded across all missions/sessions.
    pub async fn get_all_invocations(&self) -> Result<Vec<ModelInvocationRecord>, sqlx::Error> {
        let rows = sqlx::query(
            r#"
            SELECT id, mission_id, task_id, agent_id, step_number, provider,
                   model_name, attempt_number, outcome, prompt_tokens, completion_tokens,
                   total_tokens, usage_source, routing_reason, prompt_provenance, created_at
            FROM model_invocations
            ORDER BY created_at ASC
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter().map(map_row_to_invocation).collect()
    }
}

fn map_row_to_invocation(row: SqliteRow) -> Result<ModelInvocationRecord, sqlx::Error> {
    let id_raw: Vec<u8> = row.try_get("id")?;
    let id_bytes: [u8; 16] = id_raw
        .try_into()
        .map_err(|_| sqlx::Error::Decode("corrupt id BLOB length".into()))?;
    let id = Uuid::from_bytes(id_bytes);

    let mission_id_raw: Vec<u8> = row.try_get("mission_id")?;
    let mission_id_bytes: [u8; 16] = mission_id_raw
        .try_into()
        .map_err(|_| sqlx::Error::Decode("corrupt mission_id BLOB length".into()))?;
    let mission_id = MissionId::from_bytes(mission_id_bytes);

    let task_id_raw: Vec<u8> = row.try_get("task_id")?;
    let task_id_bytes: [u8; 16] = task_id_raw
        .try_into()
        .map_err(|_| sqlx::Error::Decode("corrupt task_id BLOB length".into()))?;
    let task_id = TaskId::from_bytes(task_id_bytes);

    let agent_id_raw: Vec<u8> = row.try_get("agent_id")?;
    let agent_id_bytes: [u8; 16] = agent_id_raw
        .try_into()
        .map_err(|_| sqlx::Error::Decode("corrupt agent_id BLOB length".into()))?;
    let agent_id = AgentId::from_bytes(agent_id_bytes);

    let step_number: i64 = row.try_get("step_number")?;
    let provider: String = row.try_get("provider")?;
    let model_name: String = row.try_get("model_name")?;
    let attempt_number: i64 = row.try_get("attempt_number")?;
    let outcome: String = row.try_get("outcome")?;
    let prompt_tokens: i64 = row.try_get("prompt_tokens")?;
    let completion_tokens: i64 = row.try_get("completion_tokens")?;
    let total_tokens: i64 = row.try_get("total_tokens")?;
    let usage_source: String = row.try_get("usage_source")?;
    let routing_reason: String = row.try_get("routing_reason")?;
    let prompt_provenance: Option<String> = row.try_get("prompt_provenance").unwrap_or(None);
    let created_at_str: String = row.try_get("created_at")?;

    let created_at = DateTime::parse_from_rfc3339(&created_at_str)
        .map_err(|e| sqlx::Error::Decode(format!("invalid created_at timestamp: {}", e).into()))?
        .with_timezone(&Utc);

    Ok(ModelInvocationRecord {
        id,
        mission_id,
        task_id,
        agent_id,
        step_number: step_number.max(0) as u32,
        provider,
        model_name,
        attempt_number: attempt_number.max(1) as u32,
        outcome,
        prompt_tokens: prompt_tokens.max(0) as usize,
        completion_tokens: completion_tokens.max(0) as usize,
        total_tokens: total_tokens.max(0) as usize,
        usage_source,
        routing_reason,
        prompt_provenance,
        created_at,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    async fn seed_test_parents(
        pool: &SqlitePool,
        mission_id: &MissionId,
        task_id: &TaskId,
        agent_id: &AgentId,
    ) {
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO missions (id, objective, status, created_at, updated_at) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(mission_id.as_bytes().as_slice())
        .bind("test mission")
        .bind("active")
        .bind(&now)
        .bind(&now)
        .execute(pool)
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO tasks (id, mission_id, title, status, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(task_id.as_bytes().as_slice())
        .bind(mission_id.as_bytes().as_slice())
        .bind("test task")
        .bind("active")
        .bind(&now)
        .bind(&now)
        .execute(pool)
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO agents (id, mission_id, role, status, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(agent_id.as_bytes().as_slice())
        .bind(mission_id.as_bytes().as_slice())
        .bind("implementer")
        .bind("active")
        .bind(&now)
        .bind(&now)
        .execute(pool)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn test_sqlite_model_invocation_crud() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test_telemetry.db");
        let pool = crate::persistence::sqlite::schema::initialize_database(&db_path)
            .await
            .unwrap();

        let mission_id = MissionId::new();
        let task_id = TaskId::new();
        let agent_id = AgentId::new();

        seed_test_parents(&pool, &mission_id, &task_id, &agent_id).await;

        let repo = SqliteModelInvocationRepository::new(pool);
        let usage = TokenUsage::new(120, 80, 200, 0, UsageSource::AuthoritativeProvider);

        let record1 = ModelInvocationRecord::new(
            mission_id,
            task_id,
            agent_id,
            1,
            "nvidia",
            "meta/llama-3.1-8b-instruct",
            1,
            "succeeded",
            &usage,
            "tier=Fast, health=Healthy",
        );

        let record2 = ModelInvocationRecord::new(
            mission_id,
            task_id,
            agent_id,
            2,
            "nvidia",
            "meta/llama-3.3-70b-instruct",
            1,
            "succeeded",
            &TokenUsage::new(500, 250, 750, 0, UsageSource::AuthoritativeProvider),
            "tier=Standard, health=Healthy",
        )
        .with_prompt_provenance(r#"{"prompt_id":"agent.implementer","prompt_version":1}"#);

        repo.insert_invocation(&record1).await.unwrap();
        repo.insert_invocation(&record2).await.unwrap();

        let task_invocations = repo.get_invocations_for_task(&task_id).await.unwrap();
        assert_eq!(task_invocations.len(), 2);
        assert_eq!(task_invocations[0].id, record1.id);
        assert_eq!(task_invocations[0].step_number, 1);
        assert_eq!(task_invocations[0].prompt_tokens, 120);
        assert_eq!(task_invocations[0].prompt_provenance, None);
        assert_eq!(task_invocations[1].id, record2.id);
        assert_eq!(task_invocations[1].step_number, 2);
        assert_eq!(task_invocations[1].prompt_tokens, 500);
        assert_eq!(
            task_invocations[1].prompt_provenance.as_deref(),
            Some(r#"{"prompt_id":"agent.implementer","prompt_version":1}"#)
        );

        let total_usage = repo
            .get_total_token_usage_for_mission(&mission_id)
            .await
            .unwrap();
        assert_eq!(total_usage.prompt_tokens, 620);
        assert_eq!(total_usage.completion_tokens, 330);
        assert_eq!(total_usage.total_tokens, 950);
        assert_eq!(total_usage.source, UsageSource::AuthoritativeProvider);
    }
}
