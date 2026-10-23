//! SQLite concrete implementation of MissionRepository (D-01, D-15, PST-01).

use crate::error::M31AError;
use crate::ids::{MissionId, TaskGraphId};
use crate::persistence::sqlite::repositories::MissionRepository;
use crate::persistence::sqlite::repositories::row::{
    decode_aggregate_status, decode_autonomy_mode, decode_json_column, decode_optional_timestamp,
    decode_required_timestamp, encode_aggregate_fields,
};
use crate::state::mission::Mission;
use crate::state_machine::MissionState;
use async_trait::async_trait;
use chrono::Utc;
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

    /// Update the state reconstruction watermark on a mission (and its canonical execution run).
    pub async fn update_watermark(&self, id: MissionId, sequence: u64) -> Result<(), M31AError> {
        let now = Utc::now().to_rfc3339();
        let run_rows = sqlx::query(
            "UPDATE execution_runs SET last_applied_sequence = ?, updated_at = ? WHERE id = ?",
        )
        .bind(sequence as i64)
        .bind(&now)
        .bind(id.as_bytes().as_slice())
        .execute(&self.pool)
        .await?
        .rows_affected();

        let mission_rows = sqlx::query(
            "UPDATE missions SET last_applied_sequence = ?, updated_at = ? WHERE id = ?",
        )
        .bind(sequence as i64)
        .bind(&now)
        .bind(id.as_bytes().as_slice())
        .execute(&self.pool)
        .await?
        .rows_affected();

        if run_rows == 0 && mission_rows == 0 {
            return Err(M31AError::not_found(format!("Mission/ExecutionRun {}", id)));
        }
        Ok(())
    }

    /// Retrieve all missions forked from a specific parent mission.
    pub async fn get_by_parent(&self, parent_id: MissionId) -> Result<Vec<Mission>, M31AError> {
        let rows = sqlx::query(
            r#"
            SELECT id, objective, status, workspace_root, mode, policy_context,
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
        let encoded = encode_aggregate_fields(
            &mission.policy_context,
            &mission.constraints,
            &mission.requirements,
            &mission.success_criteria,
        )?;

        let started_at_str = mission.started_at.map(|t| t.to_rfc3339());
        let completed_at_str = mission.completed_at.map(|t| t.to_rfc3339());
        let parent_mission_bytes = mission.parent_mission.map(|p| *p.as_bytes());
        let task_graph_bytes = mission.task_graph_id.map(|t| *t.as_bytes());

        sqlx::query(
            r#"
            INSERT INTO missions (
                id, objective, status, workspace_root, mode, policy_context,
                constraints, requirements, success_criteria, started_at, completed_at,
                parent_mission_id, task_graph_id, last_applied_sequence, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET
                objective = excluded.objective,
                status = excluded.status,
                workspace_root = excluded.workspace_root,
                mode = excluded.mode,
                policy_context = excluded.policy_context,
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
        .bind(&encoded.policy_context)
        .bind(&encoded.constraints)
        .bind(&encoded.requirements)
        .bind(&encoded.success_criteria)
        .bind(started_at_str)
        .bind(completed_at_str)
        .bind(parent_mission_bytes.as_ref().map(|b| b.as_slice()))
        .bind(task_graph_bytes.as_ref().map(|b| b.as_slice()))
        .bind(mission.last_applied_sequence as i64)
        .bind(mission.created_at.to_rfc3339())
        .bind(mission.updated_at.to_rfc3339())
        .execute(&self.pool)
        .await?;

        // F-02/F-06: NO dual-write into `execution_runs` here. The
        // canonical execution lifecycle is owned exclusively by
        // `SqliteExecutionRunRepository` (validated state-machine
        // transitions) plus the one-time migration in migration 030 and
        // the idempotent `reconcile_legacy_missions_once` scanner step.
        // A legacy `missions` write must never overwrite canonical run
        // status (it could resurrect terminal runs past the state machine
        // with swallowed errors). This repository is migration-read-only.
        Ok(())
    }

    async fn get(&self, id: MissionId) -> Result<Option<Mission>, M31AError> {
        let maybe_run = sqlx::query(
            r#"
            SELECT id, objective, status, workspace_root, mode, policy_context,
                   constraints, requirements, success_criteria, started_at, completed_at,
                   parent_run_id as parent_mission_id, task_graph_id, last_applied_sequence, created_at, updated_at
            FROM execution_runs WHERE id = ?
            "#,
        )
        .bind(id.as_bytes().as_slice())
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = maybe_run {
            return Ok(Some(map_row_to_mission(row)?));
        }

        let maybe_row = sqlx::query(
            r#"
            SELECT id, objective, status, workspace_root, mode, policy_context,
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
        let run_rows =
            sqlx::query("UPDATE execution_runs SET status = ?, updated_at = ? WHERE id = ?")
                .bind(status.to_string())
                .bind(&now)
                .bind(id.as_bytes().as_slice())
                .execute(&self.pool)
                .await?
                .rows_affected();

        let mission_rows =
            sqlx::query("UPDATE missions SET status = ?, updated_at = ? WHERE id = ?")
                .bind(status.to_string())
                .bind(&now)
                .bind(id.as_bytes().as_slice())
                .execute(&self.pool)
                .await?
                .rows_affected();

        if run_rows == 0 && mission_rows == 0 {
            return Err(M31AError::not_found(format!("Mission/ExecutionRun {}", id)));
        }
        Ok(())
    }

    async fn list_all(&self) -> Result<Vec<Mission>, M31AError> {
        let rows = sqlx::query(
            r#"
            SELECT id, objective, status, workspace_root, mode, policy_context,
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
    let status = decode_aggregate_status(&row, "mission")?;

    let workspace_root_str: String = row.try_get("workspace_root")?;
    let workspace_root = PathBuf::from(workspace_root_str);

    let mode = decode_autonomy_mode(&row, "mission")?;
    // Corrupt policy and requirement payloads must fail closed.
    let policy_context = decode_json_column(&row, "policy_context", "mission")?;
    let constraints = decode_json_column(&row, "constraints", "mission")?;
    let requirements = decode_json_column(&row, "requirements", "mission")?;
    let success_criteria = decode_json_column(&row, "success_criteria", "mission")?;
    let started_at = decode_optional_timestamp(&row, "started_at", "mission")?;
    let completed_at = decode_optional_timestamp(&row, "completed_at", "mission")?;

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

    let created_at = decode_required_timestamp(&row, "created_at", "mission")?;
    let updated_at = decode_required_timestamp(&row, "updated_at", "mission")?;

    Ok(Mission {
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
        parent_mission,
        task_graph_id,
        last_applied_sequence: last_applied_sequence as u64,
    })
}

/// Map a legacy `missions.status` spelling onto the canonical
/// `execution_runs` status vocabulary (F-02).
///
/// This is the SINGLE Rust-side definition of the legacy mapping. The
/// startup scanner's bulk SQL (`reconcile_legacy_missions_once`) carries an
/// equivalent `CASE` for set-based migration; keep the two in sync.
/// Unknown spellings are rejected (fail closed) rather than stored as
/// canonical state.
pub fn map_legacy_status_to_canonical(status: &str) -> Result<&'static str, M31AError> {
    Ok(match status.to_lowercase().as_str() {
        "running" | "in_progress" | "inprogress" | "active" | "executing" => "Executing",
        "succeeded" | "completed" => "Completed",
        "failed" => "Failed",
        "cancelled" | "canceled" => "Cancelled",
        "paused" => "Paused",
        "created" => "Created",
        "blocked" => "Blocked",
        // Full canonical vocabulary passes through (case-insensitive parse
        // validates below); anything else is rejected.
        other => {
            crate::state_machine::ExecutionRunState::from_str(other).map_err(|_| {
                M31AError::validation(format!(
                    "legacy missions row carries unknown status '{status}'; refusing to mint canonical state"
                ))
            })?;
            // Return the canonical display spelling for the validated state.
            match other {
                "understanding" => "Understanding",
                "researching" => "Researching",
                "planning" => "Planning",
                "scheduled" => "Scheduled",
                "verifying" => "Verifying",
                "reviewing" => "Reviewing",
                "integrating" => "Integrating",
                "shipping" => "Shipping",
                _ => {
                    return Err(M31AError::validation(format!(
                        "legacy missions row carries unknown status '{status}'; refusing to mint canonical state"
                    )));
                }
            }
        }
    })
}

/// One-time deterministic reconciliation of a single legacy `missions` row
/// into the canonical `execution_runs` table (F-02).
///
/// Create-if-absent ONLY: rows that already have a canonical counterpart
/// are never touched (canonical terminal state always wins), and unknown
/// legacy statuses fail closed. Returns `true` when a canonical row was
/// minted, `false` when there was nothing to reconcile (canonical row
/// already exists, or no legacy row exists — the latter lets a dangling
/// foreign key fail closed at the caller instead of fabricating state).
pub async fn ensure_canonical_row_for_legacy(
    pool: &SqlitePool,
    id: MissionId,
) -> Result<bool, M31AError> {
    let canonical_exists: bool =
        sqlx::query_scalar("SELECT COUNT(*) FROM execution_runs WHERE id = ?")
            .bind(id.as_bytes().as_slice())
            .fetch_one(pool)
            .await
            .map(|c: i64| c > 0)
            .map_err(M31AError::from)?;
    if canonical_exists {
        return Ok(false);
    }
    let row = sqlx::query(
        r#"
        SELECT objective, status, workspace_root, mode, policy_context,
               constraints, requirements, success_criteria, started_at, completed_at,
               parent_mission_id, task_graph_id, last_applied_sequence, created_at, updated_at
        FROM missions WHERE id = ?
        "#,
    )
    .bind(id.as_bytes().as_slice())
    .fetch_optional(pool)
    .await
    .map_err(M31AError::from)?;
    let Some(r) = row else {
        return Ok(false);
    };
    let legacy_status: String = r.try_get("status").map_err(M31AError::from)?;
    let canonical_status = map_legacy_status_to_canonical(&legacy_status)?;
    let get_str =
        |col: &str| -> Result<String, M31AError> { r.try_get(col).map_err(M31AError::from) };
    let get_opt_str = |col: &str| -> Result<Option<String>, M31AError> {
        r.try_get(col).map_err(M31AError::from)
    };
    sqlx::query(
        r#"
        INSERT OR IGNORE INTO execution_runs (
            id, objective, status, workspace_root, mode, policy_context,
            constraints, requirements, success_criteria, started_at, completed_at,
            parent_run_id, task_graph_id, last_applied_sequence, created_at, updated_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(id.as_bytes().as_slice())
    .bind(get_str("objective")?)
    .bind(canonical_status)
    .bind(get_str("workspace_root")?)
    .bind(get_str("mode")?)
    .bind(get_str("policy_context")?)
    .bind(get_str("constraints")?)
    .bind(get_str("requirements")?)
    .bind(get_str("success_criteria")?)
    .bind(get_opt_str("started_at")?)
    .bind(get_opt_str("completed_at")?)
    .bind(
        r.try_get::<Option<Vec<u8>>, _>("parent_mission_id")
            .map_err(M31AError::from)?,
    )
    .bind(
        r.try_get::<Option<Vec<u8>>, _>("task_graph_id")
            .map_err(M31AError::from)?,
    )
    .bind(
        r.try_get::<i64, _>("last_applied_sequence")
            .map_err(M31AError::from)?,
    )
    .bind(get_str("created_at")?)
    .bind(get_str("updated_at")?)
    .execute(pool)
    .await
    .map_err(M31AError::from)?;
    tracing::info!(
        mission = %id,
        status = canonical_status,
        "reconciled single legacy missions row into canonical execution_runs"
    );
    Ok(true)
}

/// Ensure a mirror row in the legacy `missions` table exists for an ExecutionRun.
///
/// Many legacy tables (such as `approval_requests`, `policy_grants`, etc.) enforce
/// foreign key constraints against `missions(id)`. When an execution run is created
/// without a corresponding legacy row, inserting into those tables triggers
/// SQLITE_CONSTRAINT_FOREIGNKEY (code 787). This reconciles the legacy table
/// idempotently.
pub async fn ensure_legacy_row_for_run(
    pool: &SqlitePool,
    id: MissionId,
    objective: &str,
) -> Result<bool, M31AError> {
    let legacy_exists: bool = sqlx::query_scalar("SELECT COUNT(*) FROM missions WHERE id = ?")
        .bind(id.as_bytes().as_slice())
        .fetch_one(pool)
        .await
        .map(|c: i64| c > 0)
        .map_err(M31AError::from)?;
    if legacy_exists {
        return Ok(false);
    }
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT OR IGNORE INTO missions (id, objective, status, created_at, updated_at) VALUES (?, ?, 'Created', ?, ?)"
    )
    .bind(id.as_bytes().as_slice())
    .bind(objective)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await
    .map_err(M31AError::from)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::persistence::sqlite::schema::initialize_database;
    use crate::state::intake::AutonomyMode;
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
