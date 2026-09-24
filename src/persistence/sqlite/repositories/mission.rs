//! SQLite concrete implementation of MissionRepository (D-01, D-15, PST-01).

use crate::error::M31AError;
use crate::ids::{MissionId, TaskGraphId};
use crate::persistence::sqlite::repositories::MissionRepository;
use crate::state::budget::ResourceBudget;
use crate::state::intake::AutonomyMode;
use crate::state::mission::Mission;
use crate::state::policy_context::PolicyContext;
use crate::state_machine::MissionState;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::sqlite::SqliteRow;
use sqlx::{Row, SqlitePool};
use std::path::PathBuf;
use std::str::FromStr;

/// SQLite repository for Mission aggregates.
#[derive(Debug, Clone)]
pub struct SqliteMissionRepository {
    pool: SqlitePool,
}

impl SqliteMissionRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Update the state reconstruction watermark on a mission.
    pub async fn update_watermark(&self, id: MissionId, sequence: u64) -> Result<(), M31AError> {
        let now = Utc::now().to_rfc3339();
        let rows_affected = sqlx::query(
            "UPDATE missions SET last_applied_sequence = ?, updated_at = ? WHERE id = ?",
        )
        .bind(sequence as i64)
        .bind(&now)
        .bind(id.as_bytes().as_slice())
        .execute(&self.pool)
        .await?
        .rows_affected();

        if rows_affected == 0 {
            return Err(M31AError::not_found(format!("Mission {}", id)));
        }
        Ok(())
    }

    /// Retrieve all missions forked from a specific parent mission.
    pub async fn get_by_parent(&self, parent_id: MissionId) -> Result<Vec<Mission>, M31AError> {
        let rows = sqlx::query(
            r#"
            SELECT id, objective, status, workspace_root, mode, policy_context, budget,
                   constraints, requirements, success_criteria, started_at, completed_at,
                   parent_mission_id, task_graph_id, last_applied_sequence, created_at, updated_at
            FROM missions WHERE parent_mission_id = ? ORDER BY created_at ASC
            "#,
        )
        .bind(parent_id.as_bytes().as_slice())
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter().map(map_row_to_mission).collect()
    }

    /// Retrieve a mission by its ID.
    pub async fn get(&self, id: MissionId) -> Result<Option<Mission>, M31AError> {
        <Self as MissionRepository>::get(self, id).await
    }

    /// Insert a new mission aggregate.
    pub async fn insert(&self, mission: &Mission) -> Result<(), M31AError> {
        <Self as MissionRepository>::insert(self, mission).await
    }

    /// Update status of a mission.
    pub async fn update_status(
        &self,
        id: MissionId,
        status: MissionState,
    ) -> Result<(), M31AError> {
        <Self as MissionRepository>::update_status(self, id, status).await
    }

    /// List all missions.
    pub async fn list_all(&self) -> Result<Vec<Mission>, M31AError> {
        <Self as MissionRepository>::list_all(self).await
    }
}

#[async_trait]
impl MissionRepository for SqliteMissionRepository {
    async fn insert(&self, mission: &Mission) -> Result<(), M31AError> {
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
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn get(&self, id: MissionId) -> Result<Option<Mission>, M31AError> {
        let maybe_row = sqlx::query(
            r#"
            SELECT id, objective, status, workspace_root, mode, policy_context, budget,
                   constraints, requirements, success_criteria, started_at, completed_at,
                   parent_mission_id, task_graph_id, last_applied_sequence, created_at, updated_at
            FROM missions WHERE id = ?
            "#,
        )
        .bind(id.as_bytes().as_slice())
        .fetch_optional(&self.pool)
        .await?;

        match maybe_row {
            Some(row) => Ok(Some(map_row_to_mission(row)?)),
            None => Ok(None),
        }
    }

    async fn update_status(&self, id: MissionId, status: MissionState) -> Result<(), M31AError> {
        let now = Utc::now().to_rfc3339();
        let rows_affected =
            sqlx::query("UPDATE missions SET status = ?, updated_at = ? WHERE id = ?")
                .bind(status.to_string())
                .bind(&now)
                .bind(id.as_bytes().as_slice())
                .execute(&self.pool)
                .await?
                .rows_affected();

        if rows_affected == 0 {
            return Err(M31AError::not_found(format!("Mission {}", id)));
        }
        Ok(())
    }

    async fn list_all(&self) -> Result<Vec<Mission>, M31AError> {
        let rows = sqlx::query(
            r#"
            SELECT id, objective, status, workspace_root, mode, policy_context, budget,
                   constraints, requirements, success_criteria, started_at, completed_at,
                   parent_mission_id, task_graph_id, last_applied_sequence, created_at, updated_at
            FROM missions ORDER BY created_at ASC
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter().map(map_row_to_mission).collect()
    }

    async fn update_watermark(&self, id: MissionId, sequence: u64) -> Result<(), M31AError> {
        Self::update_watermark(self, id, sequence).await
    }
}

fn map_row_to_mission(row: SqliteRow) -> Result<Mission, M31AError> {
    let id_raw: Vec<u8> = row.try_get("id")?;
    let id_bytes: [u8; 16] = id_raw
        .try_into()
        .map_err(|_| M31AError::persistence("corrupt mission id BLOB length"))?;
    let id = MissionId::from_bytes(id_bytes);

    let objective: String = row.try_get("objective")?;
    let status_str: String = row.try_get("status")?;
    let status = MissionState::from_str(&status_str)
        .map_err(|e| M31AError::persistence(format!("invalid status in DB: {}", e)))?;

    let workspace_root_str: String = row.try_get("workspace_root")?;
    let workspace_root = PathBuf::from(workspace_root_str);

    let mode_str: String = row.try_get("mode")?;
    // Autonomy mode is trust-relevant; malformed values fail closed instead
    // of silently defaulting to Safe.
    let mode = serde_json::from_str::<AutonomyMode>(&format!("\"{}\"", mode_str))
        .map_err(|e| M31AError::persistence(format!("corrupt mission mode '{mode_str}': {e}")))?;

    let policy_context_str: String = row.try_get("policy_context")?;
    // Policy, budget, and requirement payloads fail closed. A silent default
    // here would be a policy or budget security downgrade.
    let policy_context: PolicyContext = serde_json::from_str(&policy_context_str)
        .map_err(|e| M31AError::persistence(format!("corrupt mission policy_context: {e}")))?;

    let budget_str: String = row.try_get("budget")?;
    let budget: ResourceBudget = serde_json::from_str(&budget_str)
        .map_err(|e| M31AError::persistence(format!("corrupt mission budget: {e}")))?;

    let constraints_str: String = row.try_get("constraints")?;
    let constraints: Vec<String> = serde_json::from_str(&constraints_str)
        .map_err(|e| M31AError::persistence(format!("corrupt mission constraints: {e}")))?;

    let requirements_str: String = row.try_get("requirements")?;
    let requirements: Vec<String> = serde_json::from_str(&requirements_str)
        .map_err(|e| M31AError::persistence(format!("corrupt mission requirements: {e}")))?;

    let success_criteria_str: String = row.try_get("success_criteria")?;
    let success_criteria: Vec<String> = serde_json::from_str(&success_criteria_str)
        .map_err(|e| M31AError::persistence(format!("corrupt mission success_criteria: {e}")))?;

    let started_at_opt: Option<String> = row.try_get("started_at")?;
    let started_at = started_at_opt
        .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
        .map(|dt| dt.with_timezone(&Utc));

    let completed_at_opt: Option<String> = row.try_get("completed_at")?;
    let completed_at = completed_at_opt
        .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
        .map(|dt| dt.with_timezone(&Utc));

    let parent_id_raw: Option<Vec<u8>> = row.try_get("parent_mission_id")?;
    // Present-but-malformed identities fail closed; NULL stays absent
    // (genuinely optional).
    let parent_mission = parent_id_raw
        .map(|bytes| {
            <[u8; 16]>::try_from(bytes)
                .map(MissionId::from_bytes)
                .map_err(|_| M31AError::persistence("corrupt parent_mission_id BLOB length"))
        })
        .transpose()?;

    let task_graph_raw: Option<Vec<u8>> = row.try_get("task_graph_id")?;
    let task_graph_id = task_graph_raw
        .map(|bytes| {
            <[u8; 16]>::try_from(bytes)
                .map(TaskGraphId::from_bytes)
                .map_err(|_| M31AError::persistence("corrupt task_graph_id BLOB length"))
        })
        .transpose()?;

    let last_applied_sequence: i64 = row.try_get("last_applied_sequence")?;

    let created_at_str: String = row.try_get("created_at")?;
    let created_at = DateTime::parse_from_rfc3339(&created_at_str)
        .map_err(|e| M31AError::persistence(format!("invalid created_at: {}", e)))?
        .with_timezone(&Utc);

    let updated_at_str: String = row.try_get("updated_at")?;
    let updated_at = DateTime::parse_from_rfc3339(&updated_at_str)
        .map_err(|e| M31AError::persistence(format!("invalid updated_at: {}", e)))?
        .with_timezone(&Utc);

    Ok(Mission {
        id,
        objective,
        constraints,
        requirements,
        success_criteria,
        workspace_root,
        mode,
        policy_context,
        budget,
        status,
        created_at,
        updated_at,
        started_at,
        completed_at,
        parent_mission,
        task_graph_id,
        last_applied_sequence: last_applied_sequence as u64,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::persistence::sqlite::schema::initialize_database;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_sqlite_mission_repository_crud() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let pool = initialize_database(&db_path).await.unwrap();
        let repo = SqliteMissionRepository::new(pool);

        let id = MissionId::new();
        let mut mission = Mission::new(id, "Test Objective".to_string());
        mission.constraints.push("No GPU".to_string());
        mission.workspace_root = PathBuf::from("/test/path");
        mission.mode = AutonomyMode::Assisted;
        mission.last_applied_sequence = 5;

        // Insert
        repo.insert(&mission).await.unwrap();

        // Get
        let retrieved = repo.get(id).await.unwrap().expect("Mission should exist");
        assert_eq!(retrieved.id, id);
        assert_eq!(retrieved.objective, "Test Objective");
        assert_eq!(retrieved.status, MissionState::Created);
        assert_eq!(retrieved.mode, AutonomyMode::Assisted);
        assert_eq!(retrieved.last_applied_sequence, 5);
        assert_eq!(retrieved.constraints, vec!["No GPU".to_string()]);

        // Update status
        repo.update_status(id, MissionState::Executing)
            .await
            .unwrap();
        let updated = repo.get(id).await.unwrap().unwrap();
        assert_eq!(updated.status, MissionState::Executing);

        // Update watermark
        repo.update_watermark(id, 42).await.unwrap();
        let updated = repo.get(id).await.unwrap().unwrap();
        assert_eq!(updated.last_applied_sequence, 42);

        // List
        let list = repo.list_all().await.unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, id);
    }

    #[tokio::test]
    async fn test_sqlite_mission_repository_fork_queries() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let pool = initialize_database(&db_path).await.unwrap();
        let repo = SqliteMissionRepository::new(pool);

        let parent_id = MissionId::new();
        let parent = Mission::new(parent_id, "Parent".to_string());
        repo.insert(&parent).await.unwrap();

        let child1_id = MissionId::new();
        let mut child1 = Mission::new(child1_id, "Child 1".to_string());
        child1.parent_mission = Some(parent_id);
        repo.insert(&child1).await.unwrap();

        let child2_id = MissionId::new();
        let mut child2 = Mission::new(child2_id, "Child 2".to_string());
        child2.parent_mission = Some(parent_id);
        repo.insert(&child2).await.unwrap();

        let forks = repo.get_by_parent(parent_id).await.unwrap();
        assert_eq!(forks.len(), 2);
    }
}
