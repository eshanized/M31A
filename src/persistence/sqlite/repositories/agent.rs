//! SQLite concrete implementation of AgentRepository (AGT-05, PST-01).

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::sqlite::SqliteRow;
use sqlx::{Row, SqlitePool};
use std::str::FromStr;

use crate::error::M31AError;
use crate::ids::{AgentId, MissionId, TaskId};
use crate::persistence::sqlite::repositories::AgentRepository;
use crate::state::agent::Agent;
use crate::state_machine::AgentState;

/// SQLite repository for Agent aggregates.
#[derive(Debug, Clone)]
pub struct SqliteAgentRepository {
    pool: SqlitePool,
}

impl SqliteAgentRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Insert a new or updated agent aggregate.
    pub async fn insert(&self, agent: &Agent) -> Result<(), M31AError> {
        <Self as AgentRepository>::insert(self, agent).await
    }

    /// Get an agent aggregate by ID.
    pub async fn get(&self, id: AgentId) -> Result<Option<Agent>, M31AError> {
        <Self as AgentRepository>::get(self, id).await
    }
}

#[async_trait]
impl AgentRepository for SqliteAgentRepository {
    async fn insert(&self, agent: &Agent) -> Result<(), M31AError> {
        let task_id_bytes = agent.task_id.map(|t| *t.as_bytes());
        let completed_at_str = agent.completed_at.map(|t| t.to_rfc3339());

        sqlx::query(
            r#"
            INSERT INTO agents (
                id, mission_id, task_id, role, status,
                profile_fingerprint, max_steps, steps_consumed, model_name,
                created_at, updated_at, completed_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET
                task_id = excluded.task_id,
                role = excluded.role,
                status = excluded.status,
                profile_fingerprint = excluded.profile_fingerprint,
                max_steps = excluded.max_steps,
                steps_consumed = excluded.steps_consumed,
                model_name = excluded.model_name,
                updated_at = excluded.updated_at,
                completed_at = excluded.completed_at
            "#,
        )
        .bind(agent.id.as_bytes().as_slice())
        .bind(agent.mission_id.as_bytes().as_slice())
        .bind(task_id_bytes.as_ref().map(|b| b.as_slice()))
        .bind(&agent.role)
        .bind(agent.status.to_string())
        .bind(&agent.profile_fingerprint)
        .bind(agent.max_steps as i64)
        .bind(agent.steps_consumed as i64)
        .bind(&agent.model_name)
        .bind(agent.created_at.to_rfc3339())
        .bind(agent.updated_at.to_rfc3339())
        .bind(completed_at_str)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn get(&self, id: AgentId) -> Result<Option<Agent>, M31AError> {
        let maybe_row = sqlx::query(
            r#"
            SELECT id, mission_id, task_id, role, status,
                   profile_fingerprint, max_steps, steps_consumed, model_name,
                   created_at, updated_at, completed_at
            FROM agents WHERE id = ?
            "#,
        )
        .bind(id.as_bytes().as_slice())
        .fetch_optional(&self.pool)
        .await?;

        match maybe_row {
            Some(row) => Ok(Some(map_row_to_agent(row)?)),
            None => Ok(None),
        }
    }

    async fn update_status(&self, id: AgentId, status: AgentState) -> Result<(), M31AError> {
        let now = Utc::now();
        let now_str = now.to_rfc3339();
        let completed_at_str = if status.is_terminal() {
            Some(now_str.clone())
        } else {
            None
        };

        let rows_affected = sqlx::query(
            r#"
            UPDATE agents
            SET status = ?, updated_at = ?, completed_at = COALESCE(completed_at, ?)
            WHERE id = ?
            "#,
        )
        .bind(status.to_string())
        .bind(&now_str)
        .bind(completed_at_str)
        .bind(id.as_bytes().as_slice())
        .execute(&self.pool)
        .await?
        .rows_affected();

        if rows_affected == 0 {
            return Err(M31AError::not_found(format!("Agent {}", id)));
        }

        Ok(())
    }

    async fn list_by_mission(&self, mission_id: MissionId) -> Result<Vec<Agent>, M31AError> {
        let rows = sqlx::query(
            r#"
            SELECT id, mission_id, task_id, role, status,
                   profile_fingerprint, max_steps, steps_consumed, model_name,
                   created_at, updated_at, completed_at
            FROM agents WHERE mission_id = ? ORDER BY created_at ASC
            "#,
        )
        .bind(mission_id.as_bytes().as_slice())
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter().map(map_row_to_agent).collect()
    }
}

fn map_row_to_agent(row: SqliteRow) -> Result<Agent, M31AError> {
    let id_raw: Vec<u8> = row.try_get("id")?;
    let id_bytes: [u8; 16] = id_raw
        .try_into()
        .map_err(|_| M31AError::persistence("corrupt agent id BLOB length"))?;
    let id = AgentId::from_bytes(id_bytes);

    let mission_id_raw: Vec<u8> = row.try_get("mission_id")?;
    let mission_id_bytes: [u8; 16] = mission_id_raw
        .try_into()
        .map_err(|_| M31AError::persistence("corrupt mission_id BLOB length in agents"))?;
    let mission_id = MissionId::from_bytes(mission_id_bytes);

    let task_id: Option<TaskId> = match row.try_get::<Option<Vec<u8>>, _>("task_id")? {
        Some(bytes) => {
            let b: [u8; 16] = bytes
                .try_into()
                .map_err(|_| M31AError::persistence("corrupt task_id BLOB length in agents"))?;
            Some(TaskId::from_bytes(b))
        }
        None => None,
    };

    let role: String = row.try_get("role")?;
    let status_str: String = row.try_get("status")?;
    let status = AgentState::from_str(&status_str)
        .map_err(|e| M31AError::persistence(format!("invalid agent status in DB: {}", e)))?;

    let profile_fingerprint: String = row.try_get("profile_fingerprint").unwrap_or_default();
    let max_steps_i64: i64 = row.try_get("max_steps").unwrap_or(50);
    let max_steps = max_steps_i64 as u32;

    let steps_consumed_i64: i64 = row.try_get("steps_consumed").unwrap_or(0);
    let steps_consumed = steps_consumed_i64 as u32;

    let model_name: String = row.try_get("model_name").unwrap_or_default();

    let created_at_str: String = row.try_get("created_at")?;
    let created_at = DateTime::parse_from_rfc3339(&created_at_str)
        .map_err(|e| M31AError::persistence(format!("invalid created_at timestamp: {}", e)))?
        .with_timezone(&Utc);

    let updated_at_str: String = row.try_get("updated_at")?;
    let updated_at = DateTime::parse_from_rfc3339(&updated_at_str)
        .map_err(|e| M31AError::persistence(format!("invalid updated_at timestamp: {}", e)))?
        .with_timezone(&Utc);

    let completed_at: Option<DateTime<Utc>> =
        match row.try_get::<Option<String>, _>("completed_at")? {
            Some(s) => Some(
                DateTime::parse_from_rfc3339(&s)
                    .map_err(|e| {
                        M31AError::persistence(format!("invalid completed_at timestamp: {}", e))
                    })?
                    .with_timezone(&Utc),
            ),
            None => None,
        };

    Ok(Agent {
        id,
        mission_id,
        task_id,
        role,
        status,
        profile_fingerprint,
        max_steps,
        steps_consumed,
        model_name,
        created_at,
        updated_at,
        completed_at,
    })
}
