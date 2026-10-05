//! Transactional persistence boundaries for atomic aggregate and event commits (PST-06, D-10, D-11).

use crate::error::M31AError;
use crate::events::envelope::EventEnvelope;
use crate::ids::{AgentId, MissionId, TaskId};
use crate::state::mission::Mission;
use sqlx::sqlite::SqlitePool;

/// Coordinator for atomic SQLite transactions spanning multiple entities and event logs.
#[derive(Debug, Clone)]
pub struct SqliteTransactionManager {
    pool: SqlitePool,
}

impl SqliteTransactionManager {
    /// Create a new transaction manager wrapping an initialized SQLite connection pool.
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Access the underlying connection pool.
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    /// Atomically persist or update a Mission aggregate and append its lifecycle event to event_log.
    ///
    /// Per PST-06 and D-10: Guarantees that the state mutation and durable event write are
    /// committed within a single SQLite ACID transaction. If either operation fails, both are rolled back.
    pub async fn commit_mission_with_event(
        &self,
        mission: &Mission,
        envelope: &EventEnvelope,
    ) -> Result<(), M31AError> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;

        // 1. Serialize complex fields to JSON strings
        let policy_context_json = serde_json::to_string(&mission.policy_context)
            .map_err(|e| M31AError::validation(e.to_string()))?;
        let budget_json = serde_json::to_string(&mission.budget)
            .map_err(|e| M31AError::validation(e.to_string()))?;
        let constraints_json = serde_json::to_string(&mission.constraints)
            .map_err(|e| M31AError::validation(e.to_string()))?;
        let requirements_json = serde_json::to_string(&mission.requirements)
            .map_err(|e| M31AError::validation(e.to_string()))?;
        let success_criteria_json = serde_json::to_string(&mission.success_criteria)
            .map_err(|e| M31AError::validation(e.to_string()))?;

        let started_at_str = mission.started_at.map(|t| t.to_rfc3339());
        let completed_at_str = mission.completed_at.map(|t| t.to_rfc3339());
        let parent_mission_bytes = mission.parent_mission.map(|p| *p.as_bytes());
        let task_graph_bytes = mission.task_graph_id.map(|t| *t.as_bytes());

        // 2. Upsert mission record
        sqlx::query(
            r#"
            INSERT INTO missions (
                id, objective, status, workspace_root, mode, policy_context, budget,
                constraints, requirements, success_criteria, started_at, completed_at,
                parent_mission_id, task_graph_id, last_applied_sequence, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET
                objective = excluded.objective,
                status = excluded.status,
                workspace_root = excluded.workspace_root,
                mode = excluded.mode,
                policy_context = excluded.policy_context,
                budget = excluded.budget,
                constraints = excluded.constraints,
                requirements = excluded.requirements,
                success_criteria = excluded.success_criteria,
                started_at = excluded.started_at,
                completed_at = excluded.completed_at,
                parent_mission_id = excluded.parent_mission_id,
                task_graph_id = excluded.task_graph_id,
                last_applied_sequence = excluded.last_applied_sequence,
                updated_at = excluded.updated_at
            "#,
        )
        .bind(mission.id.as_bytes().as_slice())
        .bind(&mission.objective)
        .bind(mission.status.to_string())
        .bind(mission.workspace_root.to_string_lossy().as_ref())
        .bind(mission.mode.to_string())
        .bind(&policy_context_json)
        .bind(&budget_json)
        .bind(&constraints_json)
        .bind(&requirements_json)
        .bind(&success_criteria_json)
        .bind(started_at_str)
        .bind(completed_at_str)
        .bind(parent_mission_bytes.as_ref().map(|b| b.as_slice()))
        .bind(task_graph_bytes.as_ref().map(|b| b.as_slice()))
        .bind(mission.last_applied_sequence as i64)
        .bind(mission.created_at.to_rfc3339())
        .bind(mission.updated_at.to_rfc3339())
        .execute(&mut *tx)
        .await?;

        // 3. Serialize event payload
        let payload_json = serde_json::to_string(&envelope.event_type)
            .map_err(|e| M31AError::validation(e.to_string()))?;
        let category_str = format!("{:?}", envelope.category).to_lowercase();
        let session_bytes = envelope.session_id.map(|s| *s.as_bytes());
        let causation_bytes = envelope.causation_id.map(|c| *c.as_bytes());

        // 4. Insert into event_log
        sqlx::query(
            r#"
            INSERT INTO event_log (
                id, sequence, mission_id, session_id, actor, category,
                correlation_id, causation_id, event_type, payload, schema_version, created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(envelope.id.as_bytes().as_slice())
        .bind(envelope.sequence as i64)
        .bind(
            envelope
                .mission_id
                .map(|m| *m.as_bytes())
                .as_ref()
                .map(|b| b.as_slice()),
        )
        .bind(session_bytes.as_ref().map(|b| b.as_slice()))
        .bind(&envelope.actor)
        .bind(category_str)
        .bind(envelope.correlation_id.as_deref())
        .bind(causation_bytes.as_ref().map(|b| b.as_slice()))
        .bind(envelope.event_type.name())
        .bind(&payload_json)
        .bind(envelope.schema_version as i64)
        .bind(envelope.timestamp.to_rfc3339())
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(())
    }
    /// Begin a new transaction on the underlying connection pool.
    pub async fn begin(&self) -> Result<sqlx::Transaction<'_, sqlx::Sqlite>, M31AError> {
        self.pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(M31AError::from)
    }

    /// Atomically commit one task-execution side-effect bundle.
    ///
    /// The task result, agent linkage, invocation telemetry, and
    /// budget-ledger snapshot form ONE logical state transition: either all
    /// four are durable or none are. A missing task fails the transition
    /// loudly (`not_found`) instead of leaving partial telemetry behind.
    /// Callers surface the error (SeamError) so the runtime knows persistence
    /// failed — errors here are never swallowed.
    #[allow(clippy::too_many_arguments)]
    pub async fn commit_task_execution_side_effects(
        &self,
        mission_id: MissionId,
        task_id: TaskId,
        agent_id: AgentId,
        status: &str,
        result_json: Option<&str>,
        telemetry: Option<TaskExecutionTelemetry>,
        budget: Option<BudgetConsumption>,
    ) -> Result<(), M31AError> {
        let now = chrono::Utc::now().to_rfc3339();
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;

        // 1. Task result (missing task fails loudly: no partial state).
        let rows =
            sqlx::query("UPDATE tasks SET status = ?, result = ?, updated_at = ? WHERE id = ?")
                .bind(status)
                .bind(result_json)
                .bind(&now)
                .bind(task_id.as_bytes().as_slice())
                .execute(&mut *tx)
                .await?
                .rows_affected();
        if rows == 0 {
            return Err(M31AError::not_found(format!("Task {task_id}")));
        }

        // 2. Agent linkage for foreign-key integrity.
        sqlx::query(
            r#"
            INSERT INTO agents (
                id, mission_id, task_id, role, status,
                profile_fingerprint, max_steps, steps_consumed, model_name,
                created_at, updated_at
            ) VALUES (?, ?, ?, 'implementer', 'active', 'production', 30, 0, 'production_model', ?, ?)
            ON CONFLICT(id) DO NOTHING
            "#,
        )
        .bind(agent_id.as_bytes().as_slice())
        .bind(mission_id.as_bytes().as_slice())
        .bind(task_id.as_bytes().as_slice())
        .bind(&now)
        .bind(&now)
        .execute(&mut *tx)
        .await?;

        // 3. Authoritative invocation telemetry (same row shape the telemetry
        // repository writes; no second identity).
        if let Some(t) = telemetry {
            sqlx::query(
                r#"
                INSERT INTO model_invocations (
                    id, mission_id, task_id, agent_id, step_number, provider,
                    model_name, attempt_number, outcome, prompt_tokens, completion_tokens,
                    total_tokens, usage_source, routing_reason, prompt_provenance, created_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                "#,
            )
            .bind(t.id.as_bytes().as_slice())
            .bind(mission_id.as_bytes().as_slice())
            .bind(task_id.as_bytes().as_slice())
            .bind(agent_id.as_bytes().as_slice())
            .bind(t.step_number as i64)
            .bind(&t.provider)
            .bind(&t.model_name)
            .bind(t.attempt_number as i64)
            .bind(&t.outcome)
            .bind(t.prompt_tokens as i64)
            .bind(t.completion_tokens as i64)
            .bind(t.total_tokens as i64)
            .bind(&t.usage_source)
            .bind(&t.routing_reason)
            .bind(t.prompt_provenance.as_deref())
            .bind(&now)
            .execute(&mut *tx)
            .await?;
        }

        // 4. Budget-ledger snapshot for restart durability (authoritative
        // AND estimated: a restart must restore full accounted totals).
        if let Some(b) = budget {
            sqlx::query(
                r#"
                INSERT INTO budget_ledger (
                    mission_id, consumed_tokens, consumed_cost_microcents,
                    consumed_artifact_bytes, steps_consumed, calls_consumed,
                    retries_consumed, estimated_tokens, estimated_cost_microcents,
                    updated_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                ON CONFLICT(mission_id) DO UPDATE SET
                    consumed_tokens = excluded.consumed_tokens,
                    consumed_cost_microcents = excluded.consumed_cost_microcents,
                    consumed_artifact_bytes = excluded.consumed_artifact_bytes,
                    steps_consumed = excluded.steps_consumed,
                    calls_consumed = excluded.calls_consumed,
                    retries_consumed = excluded.retries_consumed,
                    estimated_tokens = excluded.estimated_tokens,
                    estimated_cost_microcents = excluded.estimated_cost_microcents,
                    updated_at = excluded.updated_at
                "#,
            )
            .bind(mission_id.as_bytes().as_slice())
            .bind(b.tokens as i64)
            .bind(b.cost_microcents as i64)
            .bind(b.artifact_bytes as i64)
            .bind(b.steps as i64)
            .bind(b.calls as i64)
            .bind(b.retries as i64)
            .bind(b.estimated_tokens as i64)
            .bind(b.estimated_cost_microcents as i64)
            .bind(&now)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }
}

/// Invocation telemetry payload for [`SqliteTransactionManager::commit_task_execution_side_effects`].
#[derive(Debug, Clone)]
pub struct TaskExecutionTelemetry {
    pub id: uuid::Uuid,
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
    pub prompt_provenance: Option<String>,
}

/// Admission-critical consumption snapshot for [`SqliteTransactionManager::commit_task_execution_side_effects`].
#[derive(Debug, Clone, Copy)]
pub struct BudgetConsumption {
    pub tokens: u64,
    pub cost_microcents: u64,
    pub artifact_bytes: u64,
    pub steps: usize,
    pub calls: usize,
    pub retries: usize,
    pub estimated_tokens: u64,
    pub estimated_cost_microcents: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::types::EventType;
    use crate::ids::MissionId;
    use crate::persistence::sqlite::schema::initialize_database;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_commit_mission_with_event_atomic_success() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let pool = initialize_database(&db_path).await.unwrap();
        let tx_mgr = SqliteTransactionManager::new(pool.clone());

        let mission_id = MissionId::new();
        let mut mission = Mission::new(mission_id, "Test Atomic Mission".to_string());
        mission.last_applied_sequence = 1;

        let envelope = EventEnvelope::new(
            1,
            Some(mission_id),
            None,
            "test-actor".to_string(),
            EventType::MissionStarted {
                mission_id,
                objective: "Test Atomic Mission".to_string(),
            },
        );

        let result = tx_mgr.commit_mission_with_event(&mission, &envelope).await;
        assert!(result.is_ok());

        // Verify mission exists in database
        let count_missions: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM missions WHERE id = ?")
            .bind(mission_id.as_bytes().as_slice())
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count_missions.0, 1);

        // Verify event exists in database
        let count_events: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM event_log WHERE id = ?")
            .bind(envelope.id.as_bytes().as_slice())
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count_events.0, 1);
    }

    #[tokio::test]
    async fn test_commit_mission_with_event_rollback_on_failure() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let pool = initialize_database(&db_path).await.unwrap();
        let tx_mgr = SqliteTransactionManager::new(pool.clone());

        // First, commit a valid event so that envelope.id is taken
        let mission1_id = MissionId::new();
        let mission1 = Mission::new(mission1_id, "Mission 1".to_string());
        let envelope1 = EventEnvelope::new(
            1,
            Some(mission1_id),
            None,
            "test-actor".to_string(),
            EventType::MissionStarted {
                mission_id: mission1_id,
                objective: "Mission 1".to_string(),
            },
        );
        tx_mgr
            .commit_mission_with_event(&mission1, &envelope1)
            .await
            .unwrap();

        // Now attempt to commit a NEW mission with a duplicate event ID
        let mission2_id = MissionId::new();
        let mission2 = Mission::new(mission2_id, "Mission 2 (Rollback Candidate)".to_string());
        let mut duplicate_envelope = EventEnvelope::new(
            2,
            Some(mission2_id),
            None,
            "test-actor".to_string(),
            EventType::MissionStarted {
                mission_id: mission2_id,
                objective: "Mission 2".to_string(),
            },
        );
        // Force the duplicate primary key on event_log
        duplicate_envelope.id = envelope1.id;

        let result = tx_mgr
            .commit_mission_with_event(&mission2, &duplicate_envelope)
            .await;
        assert!(
            result.is_err(),
            "Transaction must fail on duplicate event id"
        );

        // Verify mission2 was rolled back and does NOT exist in the database
        let count_missions: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM missions WHERE id = ?")
            .bind(mission2_id.as_bytes().as_slice())
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(
            count_missions.0, 0,
            "Mission 2 must not be persisted after rollback"
        );
    }
}
