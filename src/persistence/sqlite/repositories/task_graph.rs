//! SQLite implementation of TaskGraphRepository (D-13, DAG-01, DAG-05).

use crate::dag::graph::{DependencyEdge, DependencyKind, TaskGraph};
use crate::error::M31AError;
use crate::ids::{MissionId, TaskGraphId, TaskId};
use crate::kernel::plan::{CapabilityRequirement, ResourceEstimate, VerificationStrategy};
use crate::persistence::sqlite::repositories::TaskGraphRepository;
use crate::state::Task;
use crate::state::task::{BlockingReason, TaskResult};
use crate::state_machine::TaskState;
use crate::state_machine::agent::AgentRole;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::sqlite::SqliteRow;
use sqlx::{Row, SqlitePool};
use std::collections::{BTreeMap, BTreeSet};
use std::str::FromStr;

/// SQLite repository for TaskGraph aggregates and task state updates.
#[derive(Debug, Clone)]
pub struct SqliteTaskGraphRepository {
    pool: SqlitePool,
}

impl SqliteTaskGraphRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl TaskGraphRepository for SqliteTaskGraphRepository {
    async fn get_active_graph(
        &self,
        mission_id: MissionId,
    ) -> Result<Option<TaskGraph>, M31AError> {
        let graph_row = sqlx::query(
            r#"
            SELECT id, mission_id, revision, plan_id, status, created_at, updated_at
            FROM task_graphs
            WHERE mission_id = ? AND status = 'active'
            ORDER BY revision DESC
            LIMIT 1
            "#,
        )
        .bind(mission_id.as_bytes().as_slice())
        .fetch_optional(&self.pool)
        .await?;

        let row = match graph_row {
            Some(r) => r,
            None => return Ok(None),
        };

        let graph_id_bytes: Vec<u8> = row.try_get("id")?;
        let graph_id = TaskGraphId::from_bytes(
            graph_id_bytes
                .try_into()
                .map_err(|_| M31AError::persistence("Invalid TaskGraphId bytes in database"))?,
        );

        let revision: i64 = row.try_get("revision")?;
        let plan_id: String = row.try_get("plan_id")?;
        let status: String = row.try_get("status")?;
        let created_at_str: String = row.try_get("created_at")?;
        let updated_at_str: String = row.try_get("updated_at")?;

        let created_at = DateTime::parse_from_rfc3339(&created_at_str)
            .map_err(|e| M31AError::persistence(e.to_string()))?
            .with_timezone(&Utc);
        let updated_at = DateTime::parse_from_rfc3339(&updated_at_str)
            .map_err(|e| M31AError::persistence(e.to_string()))?
            .with_timezone(&Utc);

        let tasks = self.load_tasks_for_graph(graph_id).await?;
        let edges = self.load_edges_for_graph(graph_id).await?;

        let graph = TaskGraph::build_from_records(
            graph_id,
            mission_id,
            revision as u32,
            plan_id,
            status,
            tasks,
            edges,
            created_at,
            updated_at,
        );

        Ok(Some(graph))
    }

    async fn get_graph_by_id(&self, graph_id: TaskGraphId) -> Result<Option<TaskGraph>, M31AError> {
        let graph_row = sqlx::query(
            r#"
            SELECT id, mission_id, revision, plan_id, status, created_at, updated_at
            FROM task_graphs
            WHERE id = ?
            LIMIT 1
            "#,
        )
        .bind(graph_id.as_bytes().as_slice())
        .fetch_optional(&self.pool)
        .await?;

        let row = match graph_row {
            Some(r) => r,
            None => return Ok(None),
        };

        let mission_id_bytes: Vec<u8> = row.try_get("mission_id")?;
        let mission_id = MissionId::from_bytes(
            mission_id_bytes
                .try_into()
                .map_err(|_| M31AError::persistence("Invalid MissionId bytes in database"))?,
        );

        let revision: i64 = row.try_get("revision")?;
        let plan_id: String = row.try_get("plan_id")?;
        let status: String = row.try_get("status")?;
        let created_at_str: String = row.try_get("created_at")?;
        let updated_at_str: String = row.try_get("updated_at")?;

        let created_at = DateTime::parse_from_rfc3339(&created_at_str)
            .map_err(|e| M31AError::persistence(e.to_string()))?
            .with_timezone(&Utc);
        let updated_at = DateTime::parse_from_rfc3339(&updated_at_str)
            .map_err(|e| M31AError::persistence(e.to_string()))?
            .with_timezone(&Utc);

        let tasks = self.load_tasks_for_graph(graph_id).await?;
        let edges = self.load_edges_for_graph(graph_id).await?;

        let graph = TaskGraph::build_from_records(
            graph_id,
            mission_id,
            revision as u32,
            plan_id,
            status,
            tasks,
            edges,
            created_at,
            updated_at,
        );

        Ok(Some(graph))
    }

    async fn update_task_status(
        &self,
        task_id: TaskId,
        status: TaskState,
    ) -> Result<(), M31AError> {
        let now = Utc::now().to_rfc3339();
        let rows_affected = sqlx::query("UPDATE tasks SET status = ?, updated_at = ? WHERE id = ?")
            .bind(status.to_string())
            .bind(&now)
            .bind(task_id.as_bytes().as_slice())
            .execute(&self.pool)
            .await?
            .rows_affected();

        if rows_affected == 0 {
            return Err(M31AError::not_found(format!("Task {}", task_id)));
        }

        Ok(())
    }

    async fn update_task_result(
        &self,
        task_id: TaskId,
        status: &str,
        result_json: Option<&str>,
    ) -> Result<(), M31AError> {
        let now = Utc::now().to_rfc3339();
        let rows_affected =
            sqlx::query("UPDATE tasks SET status = ?, result = ?, updated_at = ? WHERE id = ?")
                .bind(status)
                .bind(result_json)
                .bind(&now)
                .bind(task_id.as_bytes().as_slice())
                .execute(&self.pool)
                .await?
                .rows_affected();

        if rows_affected == 0 {
            return Err(M31AError::not_found(format!("Task {}", task_id)));
        }

        Ok(())
    }
}

impl SqliteTaskGraphRepository {
    /// Retrieve the most recently updated active TaskGraph across all missions.
    pub async fn find_latest_active_graph(&self) -> Result<Option<TaskGraph>, M31AError> {
        let graph_row = sqlx::query(
            r#"
            SELECT id, mission_id, revision, plan_id, status, created_at, updated_at
            FROM task_graphs
            WHERE status = 'active'
            ORDER BY updated_at DESC
            LIMIT 1
            "#,
        )
        .fetch_optional(&self.pool)
        .await?;

        let row = match graph_row {
            Some(r) => r,
            None => return Ok(None),
        };

        let graph_id_bytes: Vec<u8> = row.try_get("id")?;
        let graph_id = TaskGraphId::from_bytes(
            graph_id_bytes
                .try_into()
                .map_err(|_| M31AError::persistence("Invalid TaskGraphId bytes in database"))?,
        );

        let mission_id_bytes: Vec<u8> = row.try_get("mission_id")?;
        let mission_id = MissionId::from_bytes(
            mission_id_bytes
                .try_into()
                .map_err(|_| M31AError::persistence("Invalid MissionId bytes in database"))?,
        );

        let revision: i64 = row.try_get("revision")?;
        let plan_id: String = row.try_get("plan_id")?;
        let status: String = row.try_get("status")?;
        let created_at_str: String = row.try_get("created_at")?;
        let updated_at_str: String = row.try_get("updated_at")?;

        let created_at = DateTime::parse_from_rfc3339(&created_at_str)
            .map_err(|e| M31AError::persistence(e.to_string()))?
            .with_timezone(&Utc);
        let updated_at = DateTime::parse_from_rfc3339(&updated_at_str)
            .map_err(|e| M31AError::persistence(e.to_string()))?
            .with_timezone(&Utc);

        let tasks = self.load_tasks_for_graph(graph_id).await?;
        let edges = self.load_edges_for_graph(graph_id).await?;

        let graph = TaskGraph::build_from_records(
            graph_id,
            mission_id,
            revision as u32,
            plan_id,
            status,
            tasks,
            edges,
            created_at,
            updated_at,
        );

        Ok(Some(graph))
    }

    async fn load_tasks_for_graph(
        &self,
        graph_id: TaskGraphId,
    ) -> Result<BTreeMap<TaskId, Task>, M31AError> {
        let rows = sqlx::query(
            r#"
            SELECT id, mission_id, task_graph_id, candidate_key, title, role, status,
                   priority, max_retries, retry_count, capabilities, verification,
                   estimates, blocking_reason, fingerprint, result, created_at, updated_at,
                   started_at, completed_at, description, completion_criteria,
                   requirement_keys, assumptions, prompt_ref_id,
                   prompt_ref_version
            FROM tasks
            WHERE task_graph_id = ?
            ORDER BY priority DESC, id ASC
            "#,
        )
        .bind(graph_id.as_bytes().as_slice())
        .fetch_all(&self.pool)
        .await?;

        let mut tasks = BTreeMap::new();
        for row in rows {
            let task = map_row_to_task(row)?;
            tasks.insert(task.id, task);
        }

        Ok(tasks)
    }

    async fn load_edges_for_graph(
        &self,
        graph_id: TaskGraphId,
    ) -> Result<BTreeSet<DependencyEdge>, M31AError> {
        let rows = sqlx::query(
            r#"
            SELECT prerequisite_task_id, dependent_task_id, kind
            FROM task_dependencies
            WHERE task_graph_id = ?
            "#,
        )
        .bind(graph_id.as_bytes().as_slice())
        .fetch_all(&self.pool)
        .await?;

        let mut edges = BTreeSet::new();
        for row in rows {
            let prereq_bytes: Vec<u8> = row.try_get("prerequisite_task_id")?;
            let dep_bytes: Vec<u8> = row.try_get("dependent_task_id")?;
            let kind_str: String = row.try_get("kind")?;

            let prerequisite_id = TaskId::from_bytes(
                prereq_bytes
                    .try_into()
                    .map_err(|_| M31AError::persistence("Invalid prerequisite TaskId bytes"))?,
            );
            let dependent_id = TaskId::from_bytes(
                dep_bytes
                    .try_into()
                    .map_err(|_| M31AError::persistence("Invalid dependent TaskId bytes"))?,
            );
            let kind = DependencyKind::from_str(&kind_str)
                .map_err(|e| M31AError::persistence(e.to_string()))?;

            edges.insert(DependencyEdge {
                prerequisite_id,
                dependent_id,
                kind,
            });
        }

        Ok(edges)
    }
}

pub(crate) fn map_row_to_task(row: SqliteRow) -> Result<Task, M31AError> {
    let id_bytes: Vec<u8> = row.try_get("id")?;
    let mission_id_bytes: Vec<u8> = row.try_get("mission_id")?;
    let task_graph_id_bytes: Option<Vec<u8>> = row.try_get("task_graph_id")?;

    let id = TaskId::from_bytes(
        id_bytes
            .try_into()
            .map_err(|_| M31AError::persistence("Invalid TaskId bytes"))?,
    );
    let mission_id = MissionId::from_bytes(
        mission_id_bytes
            .try_into()
            .map_err(|_| M31AError::persistence("Invalid MissionId bytes"))?,
    );
    let task_graph_id = task_graph_id_bytes
        .map(|b| {
            b.try_into()
                .map(TaskGraphId::from_bytes)
                .map_err(|_| M31AError::persistence("Invalid TaskGraphId bytes"))
        })
        .transpose()?;

    let candidate_key: String = row.try_get("candidate_key").unwrap_or_default();
    let title: String = row.try_get("title")?;
    let role_str: String = row
        .try_get("role")
        .unwrap_or_else(|_| "implementer".to_string());
    let status_str: String = row.try_get("status")?;
    let priority: i64 = row.try_get("priority").unwrap_or(100);
    let max_retries: i64 = row.try_get("max_retries").unwrap_or(3);
    let retry_count: i64 = row.try_get("retry_count").unwrap_or(0);

    let capabilities_json: String = row
        .try_get("capabilities")
        .unwrap_or_else(|_| "[]".to_string());
    let verification_json: String = row
        .try_get("verification")
        .unwrap_or_else(|_| "{}".to_string());
    let estimates_json: String = row
        .try_get("estimates")
        .unwrap_or_else(|_| "{}".to_string());
    let blocking_reason_json: Option<String> = row.try_get("blocking_reason")?;
    let fingerprint: String = row.try_get("fingerprint").unwrap_or_default();
    let result_json: Option<String> = row.try_get("result")?;
    // Rich context columns (migration 020); absent on legacy rows.
    let description: Option<String> = row.try_get("description").unwrap_or(None);
    let criteria_json: String = row
        .try_get("completion_criteria")
        .unwrap_or_else(|_| "[]".to_string());
    let req_keys_json: String = row
        .try_get("requirement_keys")
        .unwrap_or_else(|_| "[]".to_string());
    let assumptions_json: String = row
        .try_get("assumptions")
        .unwrap_or_else(|_| "[]".to_string());

    let created_at_str: String = row.try_get("created_at")?;
    let updated_at_str: String = row.try_get("updated_at")?;
    let started_at_str: Option<String> = row.try_get("started_at")?;
    let completed_at_str: Option<String> = row.try_get("completed_at")?;

    let role = AgentRole::from_str(&role_str).map_err(|e| M31AError::persistence(e.to_string()))?;
    let status =
        TaskState::from_str(&status_str).map_err(|e| M31AError::persistence(e.to_string()))?;

    let capabilities: Vec<CapabilityRequirement> = serde_json::from_str(&capabilities_json)
        .map_err(|e| M31AError::persistence(format!("corrupt task capabilities: {e}")))?;
    // Present-but-malformed scheduling inputs fail closed instead of silently
    // downgrading verification or estimates. The string-level defaults above
    // only tolerate absent legacy columns.
    let verification: VerificationStrategy = serde_json::from_str(&verification_json)
        .map_err(|e| M31AError::persistence(format!("corrupt task verification: {e}")))?;
    let estimates: ResourceEstimate = serde_json::from_str(&estimates_json)
        .map_err(|e| M31AError::persistence(format!("corrupt task estimates: {e}")))?;
    let blocking_reason: Option<BlockingReason> = blocking_reason_json
        .map(|s| {
            serde_json::from_str(&s)
                .map_err(|e| M31AError::persistence(format!("corrupt task blocking_reason: {e}")))
        })
        .transpose()?;
    let completion_criteria: Vec<String> = serde_json::from_str(&criteria_json)
        .map_err(|e| M31AError::persistence(format!("corrupt task completion_criteria: {e}")))?;
    let requirement_keys: Vec<String> = serde_json::from_str(&req_keys_json)
        .map_err(|e| M31AError::persistence(format!("corrupt task requirement_keys: {e}")))?;
    let assumptions: Vec<String> = serde_json::from_str(&assumptions_json)
        .map_err(|e| M31AError::persistence(format!("corrupt task assumptions: {e}")))?;
    // Typed prompt execution binding (migration 025); NULL on legacy rows.
    // A present-but-malformed binding fails closed: silently dropping the
    // workflow-selected prompt would fork prompt authority.
    let prompt_ref_id: Option<String> = row.try_get("prompt_ref_id").unwrap_or(None);
    let prompt_ref_version: Option<i64> = row.try_get("prompt_ref_version").unwrap_or(None);
    let prompt_ref = match (prompt_ref_id, prompt_ref_version) {
        (Some(id), Some(version)) if !id.trim().is_empty() && version > 0 => Some(
            crate::prompt::PromptReference::new(id, version as u32),
        ),
        (Some(id), _) if !id.trim().is_empty() => {
            return Err(M31AError::persistence(format!(
                "corrupt task prompt_ref binding for id '{id}': missing version"
            )));
        }
        _ => None,
    };
    let result: Option<TaskResult> = result_json
        .map(|s| {
            serde_json::from_str(&s)
                .map_err(|e| M31AError::persistence(format!("corrupt task result: {e}")))
        })
        .transpose()?;

    let created_at = DateTime::parse_from_rfc3339(&created_at_str)
        .map_err(|e| M31AError::persistence(e.to_string()))?
        .with_timezone(&Utc);
    let updated_at = DateTime::parse_from_rfc3339(&updated_at_str)
        .map_err(|e| M31AError::persistence(e.to_string()))?
        .with_timezone(&Utc);
    let started_at = started_at_str
        .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
        .map(|dt| dt.with_timezone(&Utc));
    let completed_at = completed_at_str
        .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
        .map(|dt| dt.with_timezone(&Utc));

    Ok(Task {
        id,
        mission_id,
        task_graph_id,
        candidate_key,
        title,
        description,
        role,
        status,
        priority: priority as u32,
        max_retries: max_retries as u32,
        retry_count: retry_count as u32,
        capabilities,
        verification,
        estimates,
        completion_criteria,
        requirement_keys,
        assumptions,
        prompt_ref,
        blocking_reason,
        fingerprint,
        result,
        created_at,
        updated_at,
        started_at,
        completed_at,
    })
}
