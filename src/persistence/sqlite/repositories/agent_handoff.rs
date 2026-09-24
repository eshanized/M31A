//! SQLite concrete implementation of AgentHandoffRepository (AGT-06, D-14).

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::sqlite::SqliteRow;
use sqlx::{Row, SqlitePool};
use std::str::FromStr;

use crate::agent::handoff::{AgentHandoffRepository, HandoffError, HandoffRecord};
use crate::ids::{AgentId, ArtifactId, HandoffId, MissionId, TaskId};
use crate::state_machine::agent::AgentRole;

/// SQLite repository for AGT-06 durable agent handoff records.
#[derive(Debug, Clone)]
pub struct SqliteAgentHandoffRepository {
    pool: SqlitePool,
}

impl SqliteAgentHandoffRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Insert a new agent handoff record into SQLite (D-14).
    pub async fn insert_handoff(&self, record: &HandoffRecord) -> Result<(), HandoffError> {
        let required_inputs_json = serde_json::to_string(&record.required_inputs)?;
        let artifacts_json = serde_json::to_string(&record.artifacts)?;
        let unresolved_questions_json = serde_json::to_string(&record.unresolved_questions)?;
        let target_task_id_bytes = record.target_task_id.map(|t| *t.as_bytes());
        let target_agent_id_bytes = record.target_agent_id.map(|a| *a.as_bytes());

        sqlx::query(
            r#"
            INSERT INTO agent_handoffs (
                id, mission_id, source_task_id, source_agent_id, source_role,
                target_task_id, target_role, target_agent_id, reason,
                required_inputs, artifacts, unresolved_questions, task_state, created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(record.id.as_bytes().as_slice())
        .bind(record.mission_id.as_bytes().as_slice())
        .bind(record.source_task_id.as_bytes().as_slice())
        .bind(record.source_agent_id.as_bytes().as_slice())
        .bind(record.source_role.to_string())
        .bind(target_task_id_bytes.as_ref().map(|b| b.as_slice()))
        .bind(record.target_role.to_string())
        .bind(target_agent_id_bytes.as_ref().map(|b| b.as_slice()))
        .bind(&record.reason)
        .bind(&required_inputs_json)
        .bind(&artifacts_json)
        .bind(&unresolved_questions_json)
        .bind(&record.task_state)
        .bind(record.created_at.to_rfc3339())
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Query all handoff records associated with a mission, ordered chronologically.
    pub async fn find_by_mission(
        &self,
        mission_id: MissionId,
    ) -> Result<Vec<HandoffRecord>, HandoffError> {
        let rows = sqlx::query(
            r#"
            SELECT id, mission_id, source_task_id, source_agent_id, source_role,
                   target_task_id, target_role, target_agent_id, reason,
                   required_inputs, artifacts, unresolved_questions, task_state, created_at
            FROM agent_handoffs
            WHERE mission_id = ?
            ORDER BY created_at ASC
            "#,
        )
        .bind(mission_id.as_bytes().as_slice())
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter().map(map_row_to_handoff).collect()
    }

    /// Query all handoff records for a specific source or target task.
    pub async fn find_by_task(&self, task_id: TaskId) -> Result<Vec<HandoffRecord>, HandoffError> {
        let rows = sqlx::query(
            r#"
            SELECT id, mission_id, source_task_id, source_agent_id, source_role,
                   target_task_id, target_role, target_agent_id, reason,
                   required_inputs, artifacts, unresolved_questions, task_state, created_at
            FROM agent_handoffs
            WHERE source_task_id = ? OR target_task_id = ?
            ORDER BY created_at ASC
            "#,
        )
        .bind(task_id.as_bytes().as_slice())
        .bind(task_id.as_bytes().as_slice())
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter().map(map_row_to_handoff).collect()
    }
}

#[async_trait]
impl AgentHandoffRepository for SqliteAgentHandoffRepository {
    async fn insert_handoff(&self, record: &HandoffRecord) -> Result<(), HandoffError> {
        self.insert_handoff(record).await
    }

    async fn find_by_mission(
        &self,
        mission_id: MissionId,
    ) -> Result<Vec<HandoffRecord>, HandoffError> {
        self.find_by_mission(mission_id).await
    }

    async fn find_by_task(&self, task_id: TaskId) -> Result<Vec<HandoffRecord>, HandoffError> {
        self.find_by_task(task_id).await
    }
}

fn map_row_to_handoff(row: SqliteRow) -> Result<HandoffRecord, HandoffError> {
    let id_raw: Vec<u8> = row
        .try_get("id")
        .map_err(|e| HandoffError::DatabaseError(e.to_string()))?;
    let id_bytes: [u8; 16] = id_raw
        .try_into()
        .map_err(|_| HandoffError::DatabaseError("corrupt handoff id BLOB length".to_string()))?;
    let id = HandoffId::from_bytes(id_bytes);

    let mission_id_raw: Vec<u8> = row
        .try_get("mission_id")
        .map_err(|e| HandoffError::DatabaseError(e.to_string()))?;
    let mission_id_bytes: [u8; 16] = mission_id_raw
        .try_into()
        .map_err(|_| HandoffError::DatabaseError("corrupt mission_id BLOB length".to_string()))?;
    let mission_id = MissionId::from_bytes(mission_id_bytes);

    let source_task_id_raw: Vec<u8> = row
        .try_get("source_task_id")
        .map_err(|e| HandoffError::DatabaseError(e.to_string()))?;
    let source_task_id_bytes: [u8; 16] = source_task_id_raw.try_into().map_err(|_| {
        HandoffError::DatabaseError("corrupt source_task_id BLOB length".to_string())
    })?;
    let source_task_id = TaskId::from_bytes(source_task_id_bytes);

    let source_agent_id_raw: Vec<u8> = row
        .try_get("source_agent_id")
        .map_err(|e| HandoffError::DatabaseError(e.to_string()))?;
    let source_agent_id_bytes: [u8; 16] = source_agent_id_raw.try_into().map_err(|_| {
        HandoffError::DatabaseError("corrupt source_agent_id BLOB length".to_string())
    })?;
    let source_agent_id = AgentId::from_bytes(source_agent_id_bytes);

    let source_role_str: String = row
        .try_get("source_role")
        .map_err(|e| HandoffError::DatabaseError(e.to_string()))?;
    let source_role = AgentRole::from_str(&source_role_str)
        .map_err(|e| HandoffError::DatabaseError(format!("invalid source_role: {}", e)))?;

    let target_task_id: Option<TaskId> = match row
        .try_get::<Option<Vec<u8>>, _>("target_task_id")
        .map_err(|e| {
        HandoffError::DatabaseError(e.to_string())
    })? {
        Some(bytes) => {
            let b: [u8; 16] = bytes.try_into().map_err(|_| {
                HandoffError::DatabaseError("corrupt target_task_id BLOB length".to_string())
            })?;
            Some(TaskId::from_bytes(b))
        }
        None => None,
    };

    let target_role_str: String = row
        .try_get("target_role")
        .map_err(|e| HandoffError::DatabaseError(e.to_string()))?;
    let target_role = AgentRole::from_str(&target_role_str)
        .map_err(|e| HandoffError::DatabaseError(format!("invalid target_role: {}", e)))?;

    let target_agent_id: Option<AgentId> = match row
        .try_get::<Option<Vec<u8>>, _>("target_agent_id")
        .map_err(|e| HandoffError::DatabaseError(e.to_string()))?
    {
        Some(bytes) => {
            let b: [u8; 16] = bytes.try_into().map_err(|_| {
                HandoffError::DatabaseError("corrupt target_agent_id BLOB length".to_string())
            })?;
            Some(AgentId::from_bytes(b))
        }
        None => None,
    };

    let reason: String = row
        .try_get("reason")
        .map_err(|e| HandoffError::DatabaseError(e.to_string()))?;

    let required_inputs_json: String = row
        .try_get("required_inputs")
        .map_err(|e| HandoffError::DatabaseError(e.to_string()))?;
    let required_inputs: Vec<String> = serde_json::from_str(&required_inputs_json)?;

    let artifacts_json: String = row
        .try_get("artifacts")
        .map_err(|e| HandoffError::DatabaseError(e.to_string()))?;
    let artifacts: Vec<ArtifactId> = serde_json::from_str(&artifacts_json)?;

    let unresolved_questions_json: String = row
        .try_get("unresolved_questions")
        .map_err(|e| HandoffError::DatabaseError(e.to_string()))?;
    let unresolved_questions: Vec<String> = serde_json::from_str(&unresolved_questions_json)?;

    let task_state: String = row
        .try_get("task_state")
        .map_err(|e| HandoffError::DatabaseError(e.to_string()))?;

    let created_at_str: String = row
        .try_get("created_at")
        .map_err(|e| HandoffError::DatabaseError(e.to_string()))?;
    let created_at = DateTime::parse_from_rfc3339(&created_at_str)
        .map_err(|e| HandoffError::DatabaseError(format!("invalid created_at timestamp: {}", e)))?
        .with_timezone(&Utc);

    Ok(HandoffRecord {
        id,
        mission_id,
        source_task_id,
        source_agent_id,
        source_role,
        target_task_id,
        target_role,
        target_agent_id,
        reason,
        required_inputs,
        artifacts,
        unresolved_questions,
        task_state,
        created_at,
    })
}
