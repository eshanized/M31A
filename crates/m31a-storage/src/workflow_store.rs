//! Persistent SQLite storage engine for autonomous workflows, task queues, lease recovery,
//! append-only audit events, and content-addressed artifacts (`m31a-storage::workflow_store`).

use anyhow::{Context, Result, anyhow, bail};
use chrono::Utc;
use m31a_core::workflow::{
    ConflictKind, ConflictReport, ConflictResolution, IntegrationLifecycleStatus,
    IntegrationRecord, WorkflowApprovalRequest, WorkflowApprovalStatus, WorkflowArtifact,
    WorkflowEvent, WorkflowEventType, WorkflowPlan, WorkflowRun, WorkflowState, WorkflowTask,
    WorkflowTaskStatus, WorkspaceIsolationStrategy, WorktreeRecord, WorktreeStatus,
};
use rusqlite::{Connection, params};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

/// Persistent workflow storage manager.
#[derive(Debug, Clone)]
pub struct WorkflowStore {
    conn: Arc<Mutex<Connection>>,
}

impl WorkflowStore {
    pub fn new(conn: Arc<Mutex<Connection>>) -> Self {
        Self { conn }
    }

    /// Persists a new workflow run.
    pub fn create_workflow(&self, run: &WorkflowRun) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            r#"
            INSERT INTO workflows (
                id, thread_id, workspace_root, objective, workflow_type,
                state, plan_revision, configuration_json, created_at_epoch_ms,
                updated_at_epoch_ms, started_at_epoch_ms, completed_at_epoch_ms,
                cancellation_reason, overall_result_json, resource_usage_json
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
            "#,
            params![
                run.id,
                run.thread_id,
                run.workspace_root.to_string_lossy().to_string(),
                run.objective,
                run.workflow_type,
                run.state.as_str(),
                run.plan_revision,
                run.configuration_json,
                run.created_at_epoch_ms,
                run.updated_at_epoch_ms,
                run.started_at_epoch_ms,
                run.completed_at_epoch_ms,
                run.cancellation_reason,
                run.overall_result_json,
                run.resource_usage_json,
            ],
        )
        .context("Failed inserting workflow into SQLite")?;

        Ok(())
    }

    /// Fetches a workflow run by ID.
    pub fn get_workflow(&self, workflow_id: &str) -> Result<Option<WorkflowRun>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            r#"
            SELECT id, thread_id, workspace_root, objective, workflow_type,
                   state, plan_revision, configuration_json, created_at_epoch_ms,
                   updated_at_epoch_ms, started_at_epoch_ms, completed_at_epoch_ms,
                   cancellation_reason, overall_result_json, resource_usage_json
            FROM workflows
            WHERE id = ?1
            "#,
        )?;

        let mut rows = stmt.query(params![workflow_id])?;
        if let Some(row) = rows.next()? {
            let state_str: String = row.get(5)?;
            let ws_str: String = row.get(2)?;
            Ok(Some(WorkflowRun {
                id: row.get(0)?,
                thread_id: row.get(1)?,
                workspace_root: PathBuf::from(ws_str),
                objective: row.get(3)?,
                workflow_type: row.get(4)?,
                state: WorkflowState::from_str(&state_str).unwrap_or(WorkflowState::Created),
                plan_revision: row.get(6)?,
                configuration_json: row.get(7)?,
                created_at_epoch_ms: row.get(8)?,
                updated_at_epoch_ms: row.get(9)?,
                started_at_epoch_ms: row.get(10)?,
                completed_at_epoch_ms: row.get(11)?,
                cancellation_reason: row.get(12)?,
                overall_result_json: row.get(13)?,
                resource_usage_json: row.get(14)?,
            }))
        } else {
            Ok(None)
        }
    }

    /// Lists recent workflow runs.
    pub fn list_workflows(&self, limit: usize) -> Result<Vec<WorkflowRun>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            r#"
            SELECT id, thread_id, workspace_root, objective, workflow_type,
                   state, plan_revision, configuration_json, created_at_epoch_ms,
                   updated_at_epoch_ms, started_at_epoch_ms, completed_at_epoch_ms,
                   cancellation_reason, overall_result_json, resource_usage_json
            FROM workflows
            ORDER BY created_at_epoch_ms DESC
            LIMIT ?1
            "#,
        )?;

        let rows = stmt.query_map(params![limit as i64], |row| {
            let state_str: String = row.get(5)?;
            let ws_str: String = row.get(2)?;
            Ok(WorkflowRun {
                id: row.get(0)?,
                thread_id: row.get(1)?,
                workspace_root: PathBuf::from(ws_str),
                objective: row.get(3)?,
                workflow_type: row.get(4)?,
                state: WorkflowState::from_str(&state_str).unwrap_or(WorkflowState::Created),
                plan_revision: row.get(6)?,
                configuration_json: row.get(7)?,
                created_at_epoch_ms: row.get(8)?,
                updated_at_epoch_ms: row.get(9)?,
                started_at_epoch_ms: row.get(10)?,
                completed_at_epoch_ms: row.get(11)?,
                cancellation_reason: row.get(12)?,
                overall_result_json: row.get(13)?,
                resource_usage_json: row.get(14)?,
            })
        })?;

        let mut list = Vec::new();
        for r in rows {
            list.push(r?);
        }
        Ok(list)
    }

    /// Atomically updates a workflow's state with transition validation.
    pub fn update_workflow_state(&self, workflow_id: &str, state: WorkflowState) -> Result<()> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        let now = Utc::now().timestamp_millis();

        // 1. Fetch current state to validate transition
        let curr_state_str: Option<String> = tx
            .query_row(
                "SELECT state FROM workflows WHERE id = ?1",
                params![workflow_id],
                |row| row.get(0),
            )
            .ok();

        if let Some(curr_str) = curr_state_str
            && let Some(curr) = WorkflowState::from_str(&curr_str)
            && !curr.can_transition_to(&state)
        {
            return Err(anyhow!(
                "Invalid workflow state transition: '{}' cannot transition to '{}'",
                curr.as_str(),
                state.as_str()
            ));
        }

        let started_at: Option<i64> = if state == WorkflowState::Running {
            Some(now)
        } else {
            None
        };
        let completed_at: Option<i64> = if state.is_terminal() { Some(now) } else { None };

        tx.execute(
            r#"
            UPDATE workflows
            SET state = ?1,
                updated_at_epoch_ms = ?2,
                started_at_epoch_ms = COALESCE(started_at_epoch_ms, ?3),
                completed_at_epoch_ms = COALESCE(completed_at_epoch_ms, ?4)
            WHERE id = ?5
            "#,
            params![state.as_str(), now, started_at, completed_at, workflow_id],
        )?;

        tx.commit()?;
        Ok(())
    }

    /// Atomically persists a workflow plan revision along with all its tasks and dependencies.
    pub fn save_workflow_plan(&self, plan: &WorkflowPlan) -> Result<()> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;

        // 1. Insert plan revision
        tx.execute(
            r#"
            INSERT OR REPLACE INTO workflow_plans (workflow_id, revision, rationale, created_at_epoch_ms)
            VALUES (?1, ?2, ?3, ?4)
            "#,
            params![
                plan.workflow_id,
                plan.revision,
                plan.rationale,
                plan.created_at_epoch_ms,
            ],
        )?;

        // Update workflow's plan_revision pointer
        tx.execute(
            "UPDATE workflows SET plan_revision = ?1, updated_at_epoch_ms = ?2 WHERE id = ?3",
            params![
                plan.revision,
                Utc::now().timestamp_millis(),
                plan.workflow_id
            ],
        )?;

        // 2. Insert tasks and dependencies
        for task in &plan.tasks {
            let deps_json = serde_json::to_string(&task.dependencies)?;
            let criteria_json = serde_json::to_string(&task.acceptance_criteria)?;
            let req_verif_json = serde_json::to_string(&task.required_verification)?;
            let read_paths_json = serde_json::to_string(&task.declared_read_paths)?;
            let write_paths_json = serde_json::to_string(&task.declared_write_paths)?;
            let in_arts_json = serde_json::to_string(&task.input_artifacts)?;
            let out_arts_json = serde_json::to_string(&task.output_artifacts)?;

            tx.execute(
                r#"
                INSERT OR REPLACE INTO workflow_tasks (
                    id, workflow_id, plan_revision, name, description, assigned_role,
                    assigned_agent_id, priority, wave_index, dependencies_json,
                    acceptance_criteria_json, required_verification_json,
                    declared_read_paths_json, declared_write_paths_json, workspace_isolation,
                    status, retry_count, max_retries, started_at_epoch_ms,
                    completed_at_epoch_ms, worker_id, lease_expires_at_epoch_ms,
                    error_details, input_artifacts_json, output_artifacts_json,
                    verification_result_json, task_kind, attempt_id, execution_result_json
                ) VALUES (
                    ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10,
                    ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20,
                    ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28, ?29
                )
                "#,
                params![
                    task.id,
                    task.workflow_id,
                    task.plan_revision,
                    task.name,
                    task.description,
                    task.assigned_role,
                    task.assigned_agent_id,
                    task.priority,
                    task.wave_index as i64,
                    deps_json,
                    criteria_json,
                    req_verif_json,
                    read_paths_json,
                    write_paths_json,
                    task.workspace_isolation.as_str(),
                    task.status.as_str(),
                    task.retry_count,
                    task.max_retries,
                    task.started_at_epoch_ms,
                    task.completed_at_epoch_ms,
                    task.worker_id,
                    task.lease_expires_at_epoch_ms,
                    task.error_details,
                    in_arts_json,
                    out_arts_json,
                    task.verification_result_json,
                    task.task_kind.as_str(),
                    task.attempt_id,
                    task.execution_result_json,
                ],
            )?;

            // Insert dependency relationships
            for dep in &task.dependencies {
                tx.execute(
                    r#"
                    INSERT OR IGNORE INTO workflow_task_dependencies (workflow_id, task_id, depends_on_task_id)
                    VALUES (?1, ?2, ?3)
                    "#,
                    params![task.workflow_id, task.id, dep],
                )?;
            }
        }

        // Promote any pending tasks whose dependencies are already satisfied to 'ready'
        tx.execute(
            r#"
            UPDATE workflow_tasks
            SET status = 'ready'
            WHERE workflow_id = ?1
              AND status = 'pending'
              AND NOT EXISTS (
                  SELECT 1
                  FROM workflow_task_dependencies d
                  JOIN workflow_tasks dep ON dep.workflow_id = d.workflow_id AND dep.id = d.depends_on_task_id
                  WHERE d.workflow_id = ?1
                    AND d.task_id = workflow_tasks.id
                    AND dep.status != 'completed'
              )
            "#,
            params![plan.workflow_id],
        )?;

        tx.commit()?;
        Ok(())
    }

    /// Retrieves a workflow plan and all its tasks by revision (or latest revision if None).
    pub fn get_workflow_plan(
        &self,
        workflow_id: &str,
        revision: Option<u32>,
    ) -> Result<Option<WorkflowPlan>> {
        let conn = self.conn.lock().unwrap();

        let rev: u32 = match revision {
            Some(r) => r,
            None => {
                let mut stmt = conn.prepare(
                    "SELECT COALESCE(MAX(revision), 0) FROM workflow_plans WHERE workflow_id = ?1",
                )?;
                let max_rev: u32 = stmt.query_row(params![workflow_id], |row| row.get(0))?;
                if max_rev == 0 {
                    return Ok(None);
                }
                max_rev
            }
        };

        let mut stmt = conn.prepare(
            r#"
            SELECT workflow_id, revision, rationale, created_at_epoch_ms
            FROM workflow_plans
            WHERE workflow_id = ?1 AND revision = ?2
            "#,
        )?;

        let plan_header = stmt
            .query_row(params![workflow_id, rev], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, u32>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                ))
            })
            .ok();

        if let Some((wf_id, revision, rationale, created_at)) = plan_header {
            drop(stmt);
            drop(conn);
            let tasks = self.list_tasks(workflow_id)?;
            let filtered_tasks: Vec<WorkflowTask> = tasks
                .into_iter()
                .filter(|t| t.plan_revision == revision)
                .collect();

            Ok(Some(WorkflowPlan {
                workflow_id: wf_id,
                revision,
                rationale,
                created_at_epoch_ms: created_at,
                tasks: filtered_tasks,
            }))
        } else {
            Ok(None)
        }
    }

    /// Retrieves an individual task by workflow ID and task ID.
    pub fn get_task(&self, workflow_id: &str, task_id: &str) -> Result<Option<WorkflowTask>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            r#"
            SELECT id, workflow_id, plan_revision, name, description, assigned_role,
                   assigned_agent_id, priority, wave_index, dependencies_json,
                   acceptance_criteria_json, required_verification_json,
                   declared_read_paths_json, declared_write_paths_json, workspace_isolation,
                   status, retry_count, max_retries, started_at_epoch_ms,
                   completed_at_epoch_ms, worker_id, lease_expires_at_epoch_ms,
                   error_details, input_artifacts_json, output_artifacts_json,
                   verification_result_json, task_kind, attempt_id, execution_result_json
            FROM workflow_tasks
            WHERE workflow_id = ?1 AND id = ?2
            "#,
        )?;

        let mut rows = stmt.query(params![workflow_id, task_id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(Self::parse_task_row(row)?))
        } else {
            Ok(None)
        }
    }

    /// Lists all tasks for a workflow.
    pub fn list_tasks(&self, workflow_id: &str) -> Result<Vec<WorkflowTask>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            r#"
            SELECT id, workflow_id, plan_revision, name, description, assigned_role,
                   assigned_agent_id, priority, wave_index, dependencies_json,
                   acceptance_criteria_json, required_verification_json,
                   declared_read_paths_json, declared_write_paths_json, workspace_isolation,
                   status, retry_count, max_retries, started_at_epoch_ms,
                   completed_at_epoch_ms, worker_id, lease_expires_at_epoch_ms,
                   error_details, input_artifacts_json, output_artifacts_json,
                   verification_result_json, task_kind, attempt_id, execution_result_json
            FROM workflow_tasks
            WHERE workflow_id = ?1
            ORDER BY wave_index ASC, priority DESC, id ASC
            "#,
        )?;

        let rows = stmt.query_map(params![workflow_id], |row| {
            Self::parse_task_row(row).map_err(|e| rusqlite::Error::ToSqlConversionFailure(e.into()))
        })?;

        let mut list = Vec::new();
        for r in rows {
            list.push(r?);
        }
        Ok(list)
    }

    fn parse_task_row(row: &rusqlite::Row) -> Result<WorkflowTask> {
        let deps_str: String = row.get(9)?;
        let crit_str: String = row.get(10)?;
        let verif_str: String = row.get(11)?;
        let read_str: String = row.get(12)?;
        let write_str: String = row.get(13)?;
        let isol_str: String = row.get(14)?;
        let status_str: String = row.get(15)?;
        let in_art_str: String = row.get(23)?;
        let out_art_str: String = row.get(24)?;

        let dependencies: Vec<String> = serde_json::from_str(&deps_str).unwrap_or_default();
        let acceptance_criteria: Vec<String> = serde_json::from_str(&crit_str).unwrap_or_default();
        let required_verification: Vec<String> =
            serde_json::from_str(&verif_str).unwrap_or_default();
        let declared_read_paths: Vec<PathBuf> = serde_json::from_str(&read_str).unwrap_or_default();
        let declared_write_paths: Vec<PathBuf> =
            serde_json::from_str(&write_str).unwrap_or_default();
        let input_artifacts: Vec<String> = serde_json::from_str(&in_art_str).unwrap_or_default();
        let output_artifacts: Vec<String> = serde_json::from_str(&out_art_str).unwrap_or_default();

        let wave_idx: i64 = row.get(8)?;

        let kind_str: Option<String> = row.get(26).ok();
        let task_kind = kind_str
            .as_deref()
            .and_then(m31a_core::workflow::WorkflowTaskKind::from_str)
            .unwrap_or(m31a_core::workflow::WorkflowTaskKind::Implementation);
        let attempt_id: u32 = row.get::<_, Option<u32>>(27).ok().flatten().unwrap_or(1);
        let execution_result_json: Option<String> = row.get(28).ok().flatten();

        Ok(WorkflowTask {
            id: row.get(0)?,
            workflow_id: row.get(1)?,
            plan_revision: row.get(2)?,
            task_kind,
            name: row.get(3)?,
            description: row.get(4)?,
            assigned_role: row.get(5)?,
            assigned_agent_id: row.get(6)?,
            priority: row.get(7)?,
            wave_index: wave_idx as usize,
            dependencies,
            acceptance_criteria,
            required_verification,
            declared_read_paths,
            declared_write_paths,
            workspace_isolation: WorkspaceIsolationStrategy::from_str(&isol_str)
                .unwrap_or(WorkspaceIsolationStrategy::Worktree),
            status: WorkflowTaskStatus::from_str(&status_str)
                .unwrap_or(WorkflowTaskStatus::Pending),
            retry_count: row.get(16)?,
            max_retries: row.get(17)?,
            attempt_id,
            started_at_epoch_ms: row.get(18)?,
            completed_at_epoch_ms: row.get(19)?,
            worker_id: row.get(20)?,
            lease_expires_at_epoch_ms: row.get(21)?,
            error_details: row.get(22)?,
            input_artifacts,
            output_artifacts,
            verification_result_json: row.get(25)?,
            execution_result_json,
        })
    }

    /// Atomically updates a task's status, error, and timestamps.
    pub fn update_task_status(
        &self,
        workflow_id: &str,
        task_id: &str,
        status: WorkflowTaskStatus,
        error: Option<&str>,
    ) -> Result<()> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        let now = Utc::now().timestamp_millis();

        let started_at: Option<i64> = if status == WorkflowTaskStatus::Running {
            Some(now)
        } else {
            None
        };
        let completed_at: Option<i64> = if status.is_terminal() {
            Some(now)
        } else {
            None
        };

        tx.execute(
            r#"
            UPDATE workflow_tasks
            SET status = ?1,
                error_details = COALESCE(?2, error_details),
                started_at_epoch_ms = COALESCE(started_at_epoch_ms, ?3),
                completed_at_epoch_ms = COALESCE(completed_at_epoch_ms, ?4)
            WHERE workflow_id = ?5 AND id = ?6
            "#,
            params![
                status.as_str(),
                error,
                started_at,
                completed_at,
                workflow_id,
                task_id
            ],
        )?;

        // When a task completes, promote any pending or blocked tasks whose dependencies are now satisfied to 'ready'
        if status == WorkflowTaskStatus::Completed {
            tx.execute(
                r#"
                UPDATE workflow_tasks
                SET status = 'ready'
                WHERE workflow_id = ?1
                  AND status IN ('pending', 'blocked')
                  AND NOT EXISTS (
                      SELECT 1
                      FROM workflow_task_dependencies d
                      JOIN workflow_tasks dep ON dep.workflow_id = d.workflow_id AND dep.id = d.depends_on_task_id
                      WHERE d.workflow_id = ?1
                        AND d.task_id = workflow_tasks.id
                        AND dep.status != 'completed'
                  )
                "#,
                params![workflow_id],
            )?;
        }

        tx.commit()?;
        Ok(())
    }

    /// Atomically updates a task's status, error, execution result, and timestamps, enforcing worker and attempt fencing.
    #[allow(clippy::too_many_arguments)]
    pub fn update_task_execution_result(
        &self,
        workflow_id: &str,
        task_id: &str,
        expected_attempt_id: Option<u32>,
        expected_worker_id: Option<&str>,
        status: WorkflowTaskStatus,
        result: Option<&m31a_core::workflow::TaskExecutionResult>,
        error: Option<&str>,
    ) -> Result<()> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let now = Utc::now().timestamp_millis();

        // 1. Verify workflow exists and is active
        let wf_state: String = match tx.query_row(
            "SELECT state FROM workflows WHERE id = ?1",
            params![workflow_id],
            |row| row.get(0),
        ) {
            Ok(s) => s,
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                bail!("Workflow '{workflow_id}' not found");
            }
            Err(e) => return Err(e.into()),
        };

        if matches!(wf_state.as_str(), "completed" | "failed" | "cancelled") {
            bail!(
                "Cannot update task '{task_id}' result: workflow '{workflow_id}' is in terminal state '{wf_state}'"
            );
        }

        let started_at: Option<i64> = if status == WorkflowTaskStatus::Running {
            Some(now)
        } else {
            None
        };
        let completed_at: Option<i64> = if status.is_terminal() {
            Some(now)
        } else {
            None
        };
        let res_json = result.map(serde_json::to_string).transpose()?;

        let rows = match (expected_attempt_id, expected_worker_id) {
            (Some(att), Some(worker)) => tx.execute(
                r#"
                UPDATE workflow_tasks
                SET status = ?1,
                    error_details = COALESCE(?2, error_details),
                    execution_result_json = COALESCE(?3, execution_result_json),
                    started_at_epoch_ms = COALESCE(started_at_epoch_ms, ?4),
                    completed_at_epoch_ms = COALESCE(completed_at_epoch_ms, ?5)
                WHERE workflow_id = ?6
                  AND id = ?7
                  AND attempt_id = ?8
                  AND worker_id = ?9
                  AND (lease_expires_at_epoch_ms IS NULL OR lease_expires_at_epoch_ms > ?10)
                "#,
                params![
                    status.as_str(),
                    error,
                    res_json,
                    started_at,
                    completed_at,
                    workflow_id,
                    task_id,
                    att,
                    worker,
                    now
                ],
            )?,
            (Some(att), None) => tx.execute(
                r#"
                UPDATE workflow_tasks
                SET status = ?1,
                    error_details = COALESCE(?2, error_details),
                    execution_result_json = COALESCE(?3, execution_result_json),
                    started_at_epoch_ms = COALESCE(started_at_epoch_ms, ?4),
                    completed_at_epoch_ms = COALESCE(completed_at_epoch_ms, ?5)
                WHERE workflow_id = ?6 AND id = ?7 AND attempt_id = ?8
                "#,
                params![
                    status.as_str(),
                    error,
                    res_json,
                    started_at,
                    completed_at,
                    workflow_id,
                    task_id,
                    att
                ],
            )?,
            (None, Some(worker)) => tx.execute(
                r#"
                UPDATE workflow_tasks
                SET status = ?1,
                    error_details = COALESCE(?2, error_details),
                    execution_result_json = COALESCE(?3, execution_result_json),
                    started_at_epoch_ms = COALESCE(started_at_epoch_ms, ?4),
                    completed_at_epoch_ms = COALESCE(completed_at_epoch_ms, ?5)
                WHERE workflow_id = ?6 AND id = ?7 AND worker_id = ?8
                "#,
                params![
                    status.as_str(),
                    error,
                    res_json,
                    started_at,
                    completed_at,
                    workflow_id,
                    task_id,
                    worker
                ],
            )?,
            (None, None) => tx.execute(
                r#"
                UPDATE workflow_tasks
                SET status = ?1,
                    error_details = COALESCE(?2, error_details),
                    execution_result_json = COALESCE(?3, execution_result_json),
                    started_at_epoch_ms = COALESCE(started_at_epoch_ms, ?4),
                    completed_at_epoch_ms = COALESCE(completed_at_epoch_ms, ?5)
                WHERE workflow_id = ?6 AND id = ?7
                "#,
                params![
                    status.as_str(),
                    error,
                    res_json,
                    started_at,
                    completed_at,
                    workflow_id,
                    task_id
                ],
            )?,
        };

        if rows == 0 && (expected_attempt_id.is_some() || expected_worker_id.is_some()) {
            return Err(anyhow!(
                "Fencing violation or stale lease: task '{task_id}' was not updated (attempt {:?}, worker {:?})",
                expected_attempt_id,
                expected_worker_id
            ));
        }

        // When a task completes, promote any pending or blocked tasks whose dependencies are now satisfied to 'ready'
        if status == WorkflowTaskStatus::Completed {
            tx.execute(
                r#"
                UPDATE workflow_tasks
                SET status = 'ready'
                WHERE workflow_id = ?1
                  AND status IN ('pending', 'blocked')
                  AND NOT EXISTS (
                      SELECT 1
                      FROM workflow_task_dependencies d
                      JOIN workflow_tasks dep ON dep.workflow_id = d.workflow_id AND dep.id = d.depends_on_task_id
                      WHERE d.workflow_id = ?1
                        AND d.task_id = workflow_tasks.id
                        AND dep.status != 'completed'
                  )
                "#,
                params![workflow_id],
            )?;
        }

        tx.commit()?;
        Ok(())
    }

    /// Atomically claims the next ready or retry_pending task with worker lease semantics, dependency validation, and write-path conflict checks.
    pub fn claim_ready_task(
        &self,
        workflow_id: &str,
        worker_id: &str,
        lease_duration_ms: i64,
    ) -> Result<Option<WorkflowTask>> {
        self.claim_task_internal(workflow_id, None, worker_id, lease_duration_ms)
    }

    /// Atomically claims a specific task (if ready/retry_pending and dependencies satisfied) for a worker.
    pub fn claim_task(
        &self,
        workflow_id: &str,
        task_id: &str,
        worker_id: &str,
        lease_duration_ms: i64,
    ) -> Result<Option<WorkflowTask>> {
        self.claim_task_internal(workflow_id, Some(task_id), worker_id, lease_duration_ms)
    }

    fn claim_task_internal(
        &self,
        workflow_id: &str,
        target_task_id: Option<&str>,
        worker_id: &str,
        lease_duration_ms: i64,
    ) -> Result<Option<WorkflowTask>> {
        if worker_id.trim().is_empty() {
            bail!("Worker ID must not be empty");
        }
        if lease_duration_ms <= 0 {
            bail!("Lease duration must be positive (got {lease_duration_ms}ms)");
        }

        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let now = Utc::now().timestamp_millis();
        let lease_expires = now
            .checked_add(lease_duration_ms)
            .ok_or_else(|| anyhow!("Timestamp overflow for lease expiration"))?;

        // 1. Verify workflow exists and is active
        let wf_state: String = match tx.query_row(
            "SELECT state FROM workflows WHERE id = ?1",
            params![workflow_id],
            |row| row.get(0),
        ) {
            Ok(s) => s,
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                bail!("Workflow '{workflow_id}' not found");
            }
            Err(e) => return Err(e.into()),
        };

        if matches!(wf_state.as_str(), "completed" | "failed" | "cancelled") {
            return Ok(None);
        }

        // 2. Fetch declared write paths of currently active tasks to prevent write-path conflicts
        let mut active_write_paths: HashSet<PathBuf> = HashSet::new();
        {
            let mut active_stmt = tx.prepare(
                r#"
                SELECT declared_write_paths_json, workspace_isolation
                FROM workflow_tasks
                WHERE workflow_id = ?1
                  AND status IN ('claimed', 'running', 'verifying')
                "#,
            )?;
            let active_rows = active_stmt.query_map(params![workflow_id], |row| {
                let paths_json: String = row.get(0)?;
                let isolation: String = row.get(1)?;
                Ok((paths_json, isolation))
            })?;
            for r in active_rows {
                let (paths_json, isolation) = r?;
                if isolation != "worktree" {
                    let paths: Vec<PathBuf> = serde_json::from_str(&paths_json).map_err(|e| {
                        anyhow!("Failed to parse active task declared_write_paths_json: {e}")
                    })?;
                    for p in paths {
                        active_write_paths.insert(p);
                    }
                }
            }
        }

        // 3. Find candidates whose dependencies are all completed
        let query_sql = if target_task_id.is_some() {
            r#"
            SELECT t.id, t.attempt_id, t.declared_write_paths_json, t.workspace_isolation
            FROM workflow_tasks t
            WHERE t.workflow_id = ?1
              AND t.id = ?2
              AND t.status IN ('ready', 'retry_pending')
              AND NOT EXISTS (
                  SELECT 1
                  FROM workflow_task_dependencies d
                  JOIN workflow_tasks dep ON dep.workflow_id = d.workflow_id AND dep.id = d.depends_on_task_id
                  WHERE d.workflow_id = ?1
                    AND d.task_id = t.id
                    AND dep.status != 'completed'
              )
            LIMIT 1
            "#
        } else {
            r#"
            SELECT t.id, t.attempt_id, t.declared_write_paths_json, t.workspace_isolation
            FROM workflow_tasks t
            WHERE t.workflow_id = ?1
              AND t.status IN ('ready', 'retry_pending')
              AND NOT EXISTS (
                  SELECT 1
                  FROM workflow_task_dependencies d
                  JOIN workflow_tasks dep ON dep.workflow_id = d.workflow_id AND dep.id = d.depends_on_task_id
                  WHERE d.workflow_id = ?1
                    AND d.task_id = t.id
                    AND dep.status != 'completed'
              )
            ORDER BY t.wave_index ASC, t.priority DESC, t.id ASC
            "#
        };

        let candidates: Vec<(String, u32, String, String)> = {
            let mut candidate_stmt = tx.prepare(query_sql)?;
            let parse_row = |row: &rusqlite::Row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, u32>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            };
            if let Some(target_id) = target_task_id {
                candidate_stmt
                    .query_map(params![workflow_id, target_id], parse_row)?
                    .collect::<Result<Vec<_>, _>>()?
            } else {
                candidate_stmt
                    .query_map(params![workflow_id], parse_row)?
                    .collect::<Result<Vec<_>, _>>()?
            }
        };

        let mut selected = None;
        for (id, attempt, write_paths_json, isolation) in candidates {
            if isolation == "worktree" || active_write_paths.is_empty() {
                selected = Some((id, attempt));
                break;
            }
            let paths: Vec<PathBuf> = serde_json::from_str(&write_paths_json)
                .map_err(|e| anyhow!("Failed to parse candidate write_paths_json: {e}"))?;
            let has_conflict = paths.iter().any(|p| active_write_paths.contains(p));
            if !has_conflict {
                selected = Some((id, attempt));
                break;
            }
        }

        let mut claimed_task_id = None;
        if let Some((ref id, current_attempt)) = selected {
            let next_attempt = current_attempt
                .checked_add(1)
                .ok_or_else(|| anyhow!("Attempt ID overflow"))?;
            let rows_updated = tx.execute(
                r#"
                UPDATE workflow_tasks
                SET status = 'claimed',
                    worker_id = ?1,
                    lease_expires_at_epoch_ms = ?2,
                    attempt_id = ?3,
                    started_at_epoch_ms = COALESCE(started_at_epoch_ms, ?4)
                WHERE workflow_id = ?5
                  AND id = ?6
                  AND status IN ('ready', 'retry_pending')
                  AND attempt_id = ?7
                "#,
                params![
                    worker_id,
                    lease_expires,
                    next_attempt,
                    now,
                    workflow_id,
                    id,
                    current_attempt
                ],
            )?;

            if rows_updated > 0 {
                let event_id = format!("evt-claim-{}", Uuid::new_v4().simple());
                let payload = serde_json::json!({
                    "worker_id": worker_id,
                    "attempt_id": next_attempt,
                    "lease_expires_at_epoch_ms": lease_expires,
                })
                .to_string();
                tx.execute(
                    r#"
                    INSERT INTO workflow_events (
                        id, workflow_id, task_id, agent_id, event_type,
                        timestamp_epoch_ms, correlation_id, payload_json, schema_version
                    ) VALUES (?1, ?2, ?3, NULL, 'task_claimed', ?4, NULL, ?5, 1)
                    "#,
                    params![event_id, workflow_id, id, now, payload],
                )?;
                claimed_task_id = Some(id.clone());
            }
        }

        tx.commit()?;

        if let Some(id) = claimed_task_id {
            drop(conn);
            self.get_task(workflow_id, &id)
        } else {
            Ok(None)
        }
    }

    /// Extends a worker's lease on an active task attempt.
    /// Fails closed if lease expired, attempt_id was fenced, or workflow is terminal.
    pub fn renew_task_lease(
        &self,
        workflow_id: &str,
        task_id: &str,
        worker_id: &str,
        expected_attempt_id: u32,
        lease_duration_ms: i64,
    ) -> Result<bool> {
        if lease_duration_ms <= 0 {
            bail!("Lease duration must be positive (got {lease_duration_ms}ms)");
        }
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let now = Utc::now().timestamp_millis();
        let lease_expires = now
            .checked_add(lease_duration_ms)
            .ok_or_else(|| anyhow!("Timestamp overflow for lease expiration"))?;

        // Verify workflow exists and is active
        let wf_state: String = match tx.query_row(
            "SELECT state FROM workflows WHERE id = ?1",
            params![workflow_id],
            |row| row.get(0),
        ) {
            Ok(s) => s,
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                bail!("Workflow '{workflow_id}' not found");
            }
            Err(e) => return Err(e.into()),
        };

        if matches!(wf_state.as_str(), "completed" | "failed" | "cancelled") {
            return Ok(false);
        }

        let rows = tx.execute(
            r#"
            UPDATE workflow_tasks
            SET lease_expires_at_epoch_ms = ?1
            WHERE workflow_id = ?2
              AND id = ?3
              AND worker_id = ?4
              AND attempt_id = ?5
              AND status IN ('claimed', 'running', 'verifying')
              AND lease_expires_at_epoch_ms > ?6
            "#,
            params![
                lease_expires,
                workflow_id,
                task_id,
                worker_id,
                expected_attempt_id,
                now
            ],
        )?;

        tx.commit()?;
        Ok(rows > 0)
    }

    /// Discovers tasks whose lease expired while in `claimed` or `running`,
    /// recovering them to `retry_pending` if within retry budget or `failed` if exceeded.
    pub fn reconcile_stale_tasks(&self, workflow_id: &str, now_epoch_ms: i64) -> Result<usize> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;

        // Find stale tasks
        let mut stmt = tx.prepare(
            r#"
            SELECT id, retry_count, max_retries, attempt_id, worker_id
            FROM workflow_tasks
            WHERE workflow_id = ?1
              AND status IN ('claimed', 'running', 'verifying')
              AND lease_expires_at_epoch_ms IS NOT NULL
              AND lease_expires_at_epoch_ms < ?2
            "#,
        )?;

        let stale: Vec<(String, u32, u32, u32, Option<String>)> = stmt
            .query_map(params![workflow_id, now_epoch_ms], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        drop(stmt);

        let count = stale.len();
        for (id, retries, max_retries, attempt_id, worker_id) in stale {
            if retries < max_retries {
                let next_retries = retries.checked_add(1).unwrap_or(retries + 1);
                tx.execute(
                    r#"
                    UPDATE workflow_tasks
                    SET status = 'retry_pending',
                        worker_id = NULL,
                        retry_count = ?3,
                        lease_expires_at_epoch_ms = NULL,
                        error_details = 'Worker lease expired; task rescheduled for retry'
                    WHERE workflow_id = ?1 AND id = ?2 AND attempt_id = ?4
                    "#,
                    params![workflow_id, id, next_retries, attempt_id],
                )?;

                let event_id = format!("evt-stale-retry-{}", Uuid::new_v4().simple());
                let payload = serde_json::json!({
                    "expired_worker_id": worker_id,
                    "attempt_id": attempt_id,
                    "retry_count": next_retries,
                    "max_retries": max_retries,
                })
                .to_string();
                tx.execute(
                    r#"
                    INSERT INTO workflow_events (
                        id, workflow_id, task_id, agent_id, event_type,
                        timestamp_epoch_ms, correlation_id, payload_json, schema_version
                    ) VALUES (?1, ?2, ?3, NULL, 'task_retried', ?4, NULL, ?5, 1)
                    "#,
                    params![event_id, workflow_id, id, now_epoch_ms, payload],
                )?;
            } else {
                tx.execute(
                    r#"
                    UPDATE workflow_tasks
                    SET status = 'failed',
                        worker_id = NULL,
                        lease_expires_at_epoch_ms = NULL,
                        completed_at_epoch_ms = ?3,
                        error_details = 'Worker lease expired and max retries exceeded'
                    WHERE workflow_id = ?1 AND id = ?2 AND attempt_id = ?4
                    "#,
                    params![workflow_id, id, now_epoch_ms, attempt_id],
                )?;

                let event_id = format!("evt-stale-failed-{}", Uuid::new_v4().simple());
                let payload = serde_json::json!({
                    "expired_worker_id": worker_id,
                    "attempt_id": attempt_id,
                    "retry_count": retries,
                    "max_retries": max_retries,
                    "reason": "Worker lease expired and max retries exceeded",
                })
                .to_string();
                tx.execute(
                    r#"
                    INSERT INTO workflow_events (
                        id, workflow_id, task_id, agent_id, event_type,
                        timestamp_epoch_ms, correlation_id, payload_json, schema_version
                    ) VALUES (?1, ?2, ?3, NULL, 'task_failed', ?4, NULL, ?5, 1)
                    "#,
                    params![event_id, workflow_id, id, now_epoch_ms, payload],
                )?;
            }
        }

        tx.commit()?;
        Ok(count)
    }

    /// Appends an event to the workflow audit log.
    pub fn record_event(&self, event: &WorkflowEvent) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            r#"
            INSERT INTO workflow_events (
                id, workflow_id, task_id, agent_id, event_type,
                timestamp_epoch_ms, correlation_id, payload_json, schema_version
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            "#,
            params![
                event.id,
                event.workflow_id,
                event.task_id,
                event.agent_id,
                event.event_type.as_str(),
                event.timestamp_epoch_ms,
                event.correlation_id,
                event.payload_json,
                event.schema_version,
            ],
        )
        .context("Failed recording workflow event into SQLite")?;

        Ok(())
    }

    /// Lists audit events for a workflow.
    pub fn list_events(
        &self,
        workflow_id: &str,
        since_epoch_ms: Option<i64>,
    ) -> Result<Vec<WorkflowEvent>> {
        let conn = self.conn.lock().unwrap();
        let since = since_epoch_ms.unwrap_or(0);
        let mut stmt = conn.prepare(
            r#"
            SELECT id, workflow_id, task_id, agent_id, event_type,
                   timestamp_epoch_ms, correlation_id, payload_json, schema_version
            FROM workflow_events
            WHERE workflow_id = ?1 AND timestamp_epoch_ms >= ?2
            ORDER BY timestamp_epoch_ms ASC, id ASC
            "#,
        )?;

        let rows = stmt.query_map(params![workflow_id, since], |row| {
            let event_type_str: String = row.get(4)?;
            Ok(WorkflowEvent {
                id: row.get(0)?,
                workflow_id: row.get(1)?,
                task_id: row.get(2)?,
                agent_id: row.get(3)?,
                event_type: WorkflowEventType::from_str(&event_type_str)
                    .unwrap_or(WorkflowEventType::WorkflowCreated),
                timestamp_epoch_ms: row.get(5)?,
                correlation_id: row.get(6)?,
                payload_json: row.get(7)?,
                schema_version: row.get(8)?,
            })
        })?;

        let mut list = Vec::new();
        for r in rows {
            list.push(r?);
        }
        Ok(list)
    }

    /// Persists an artifact metadata record.
    pub fn save_artifact(&self, artifact: &WorkflowArtifact) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            r#"
            INSERT OR REPLACE INTO workflow_artifacts (
                id, workflow_id, task_id, name, artifact_type,
                file_path, content_sha256, size_bytes, mime_type, created_at_epoch_ms
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
            "#,
            params![
                artifact.id,
                artifact.workflow_id,
                artifact.task_id,
                artifact.name,
                artifact.artifact_type,
                artifact.file_path.to_string_lossy().to_string(),
                artifact.content_sha256,
                artifact.size_bytes as i64,
                artifact.mime_type,
                artifact.created_at_epoch_ms,
            ],
        )
        .context("Failed saving workflow artifact into SQLite")?;

        Ok(())
    }

    /// Lists all artifacts for a workflow.
    pub fn list_artifacts(&self, workflow_id: &str) -> Result<Vec<WorkflowArtifact>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            r#"
            SELECT id, workflow_id, task_id, name, artifact_type,
                   file_path, content_sha256, size_bytes, mime_type, created_at_epoch_ms
            FROM workflow_artifacts
            WHERE workflow_id = ?1
            ORDER BY created_at_epoch_ms ASC, name ASC
            "#,
        )?;

        let rows = stmt.query_map(params![workflow_id], |row| {
            let fp: String = row.get(5)?;
            let sz: i64 = row.get(7)?;
            Ok(WorkflowArtifact {
                id: row.get(0)?,
                workflow_id: row.get(1)?,
                task_id: row.get(2)?,
                name: row.get(3)?,
                artifact_type: row.get(4)?,
                file_path: PathBuf::from(fp),
                content_sha256: row.get(6)?,
                size_bytes: sz as u64,
                mime_type: row.get(8)?,
                created_at_epoch_ms: row.get(9)?,
            })
        })?;

        let mut list = Vec::new();
        for r in rows {
            list.push(r?);
        }
        Ok(list)
    }

    /// Persists a new approval request.
    pub fn create_approval_request(&self, req: &WorkflowApprovalRequest) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            r#"
            INSERT INTO workflow_approvals (
                id, workflow_id, task_id, attempt_id, change_set_hash,
                patch_preview, status, requested_at_epoch_ms,
                decided_at_epoch_ms, decision_actor, decision_reason
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
            "#,
            params![
                req.id,
                req.workflow_id,
                req.task_id,
                req.attempt_id,
                req.change_set_hash,
                req.patch_preview,
                req.status.as_str(),
                req.requested_at_epoch_ms,
                req.decided_at_epoch_ms,
                req.decision_actor,
                req.decision_reason,
            ],
        )
        .context("Failed inserting workflow approval into SQLite")?;
        Ok(())
    }

    /// Fetches an approval request by ID.
    pub fn get_approval_request(
        &self,
        approval_id: &str,
    ) -> Result<Option<WorkflowApprovalRequest>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            r#"
            SELECT id, workflow_id, task_id, attempt_id, change_set_hash,
                   patch_preview, status, requested_at_epoch_ms,
                   decided_at_epoch_ms, decision_actor, decision_reason
            FROM workflow_approvals
            WHERE id = ?1
            "#,
        )?;
        let mut rows = stmt.query(params![approval_id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(Self::parse_approval_row(row)?))
        } else {
            Ok(None)
        }
    }

    /// Fetches the pending approval request for a workflow task attempt.
    pub fn get_pending_approval(
        &self,
        workflow_id: &str,
        task_id: &str,
        attempt_id: u32,
    ) -> Result<Option<WorkflowApprovalRequest>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            r#"
            SELECT id, workflow_id, task_id, attempt_id, change_set_hash,
                   patch_preview, status, requested_at_epoch_ms,
                   decided_at_epoch_ms, decision_actor, decision_reason
            FROM workflow_approvals
            WHERE workflow_id = ?1 AND task_id = ?2 AND attempt_id = ?3 AND status = 'pending'
            ORDER BY requested_at_epoch_ms DESC
            LIMIT 1
            "#,
        )?;
        let mut rows = stmt.query(params![workflow_id, task_id, attempt_id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(Self::parse_approval_row(row)?))
        } else {
            Ok(None)
        }
    }

    /// Lists all approvals for a workflow.
    pub fn list_approvals(&self, workflow_id: &str) -> Result<Vec<WorkflowApprovalRequest>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            r#"
            SELECT id, workflow_id, task_id, attempt_id, change_set_hash,
                   patch_preview, status, requested_at_epoch_ms,
                   decided_at_epoch_ms, decision_actor, decision_reason
            FROM workflow_approvals
            WHERE workflow_id = ?1
            ORDER BY requested_at_epoch_ms ASC
            "#,
        )?;
        let rows = stmt.query_map(params![workflow_id], |row| {
            Self::parse_approval_row(row)
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(e.into()))
        })?;
        let mut list = Vec::new();
        for r in rows {
            list.push(r?);
        }
        Ok(list)
    }

    /// Records an approval decision for an approval request.
    /// Returns true if updated, false if not found or already decided.
    pub fn record_approval_decision(
        &self,
        approval_id: &str,
        status: WorkflowApprovalStatus,
        actor: &str,
        reason: Option<&str>,
    ) -> Result<bool> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().timestamp_millis();
        let rows = conn.execute(
            r#"
            UPDATE workflow_approvals
            SET status = ?1,
                decided_at_epoch_ms = ?2,
                decision_actor = ?3,
                decision_reason = ?4
            WHERE id = ?5 AND status = 'pending'
            "#,
            params![status.as_str(), now, actor, reason, approval_id],
        )?;
        Ok(rows > 0)
    }

    /// Verifies if a patch change-set hash is approved for the current workflow, task, and attempt.
    pub fn is_patch_approved(
        &self,
        workflow_id: &str,
        task_id: &str,
        attempt_id: u32,
        change_set_hash: &str,
    ) -> Result<bool> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            r#"
            SELECT 1 FROM workflow_approvals
            WHERE workflow_id = ?1
              AND task_id = ?2
              AND attempt_id = ?3
              AND change_set_hash = ?4
              AND status = 'approved'
            LIMIT 1
            "#,
        )?;
        let mut rows = stmt.query(params![workflow_id, task_id, attempt_id, change_set_hash])?;
        Ok(rows.next()?.is_some())
    }

    fn parse_approval_row(row: &rusqlite::Row) -> Result<WorkflowApprovalRequest> {
        let status_str: String = row.get(6)?;
        let status = WorkflowApprovalStatus::from_str(&status_str)
            .unwrap_or(WorkflowApprovalStatus::Pending);
        Ok(WorkflowApprovalRequest {
            id: row.get(0)?,
            workflow_id: row.get(1)?,
            task_id: row.get(2)?,
            attempt_id: row.get(3)?,
            change_set_hash: row.get(4)?,
            patch_preview: row.get(5)?,
            status,
            requested_at_epoch_ms: row.get(7)?,
            decided_at_epoch_ms: row.get(8)?,
            decision_actor: row.get(9)?,
            decision_reason: row.get(10)?,
        })
    }

    /// Stores artifact content into an immutable content-addressed file in `base_dir` and saves its metadata.
    #[allow(clippy::too_many_arguments)]
    pub fn store_artifact_file(
        &self,
        base_dir: &Path,
        workflow_id: &str,
        task_id: Option<&str>,
        name: &str,
        artifact_type: &str,
        content: &[u8],
        mime_type: &str,
    ) -> Result<WorkflowArtifact> {
        let mut hasher = Sha256::new();
        hasher.update(content);
        let content_sha256 = format!("{:x}", hasher.finalize());

        let wf_artifact_dir = base_dir.join("artifacts").join(workflow_id);
        fs::create_dir_all(&wf_artifact_dir)?;

        let safe_name = name.replace(['/', '\\', ' '], "_");
        let filename = format!("{}_{}", &content_sha256[..12], safe_name);
        let final_path = wf_artifact_dir.join(&filename);

        // Atomic write via temp file
        let tmp_path = wf_artifact_dir.join(format!(".tmp_{}", Uuid::new_v4().simple()));
        fs::write(&tmp_path, content)?;
        fs::rename(&tmp_path, &final_path)?;

        let artifact_id = format!("art-{}", Uuid::new_v4().simple());
        let artifact = WorkflowArtifact {
            id: artifact_id,
            workflow_id: workflow_id.to_string(),
            task_id: task_id.map(String::from),
            name: name.to_string(),
            artifact_type: artifact_type.to_string(),
            file_path: final_path,
            content_sha256,
            size_bytes: content.len() as u64,
            mime_type: mime_type.to_string(),
            created_at_epoch_ms: Utc::now().timestamp_millis(),
        };

        self.save_artifact(&artifact)?;
        Ok(artifact)
    }

    /// Persists or updates metadata for a managed Git worktree.
    pub fn save_worktree(&self, record: &WorktreeRecord) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            r#"
            INSERT INTO managed_worktrees (
                id, thread_id, workflow_id, task_id, worktree_path,
                branch, base_sha, head_sha, source_root, status,
                unintegrated_diff, created_at_epoch_ms, updated_at_epoch_ms
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
            ON CONFLICT(id) DO UPDATE SET
                thread_id = excluded.thread_id,
                workflow_id = COALESCE(excluded.workflow_id, managed_worktrees.workflow_id),
                task_id = COALESCE(excluded.task_id, managed_worktrees.task_id),
                worktree_path = excluded.worktree_path,
                branch = excluded.branch,
                base_sha = excluded.base_sha,
                head_sha = excluded.head_sha,
                source_root = excluded.source_root,
                status = excluded.status,
                unintegrated_diff = COALESCE(excluded.unintegrated_diff, managed_worktrees.unintegrated_diff),
                updated_at_epoch_ms = excluded.updated_at_epoch_ms
            "#,
            params![
                record.id,
                record.thread_id,
                record.workflow_id,
                record.task_id,
                record.worktree_path.to_string_lossy().to_string(),
                record.branch,
                record.base_sha,
                record.head_sha,
                record.source_root.to_string_lossy().to_string(),
                record.status.as_str(),
                record.unintegrated_diff,
                record.created_at_epoch_ms,
                record.updated_at_epoch_ms,
            ],
        )
        .context("Failed saving managed worktree to SQLite")?;
        Ok(())
    }

    /// Fetches a managed worktree record by thread ID or worktree ID.
    pub fn get_worktree(&self, id_or_thread_id: &str) -> Result<Option<WorktreeRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            r#"
            SELECT id, thread_id, workflow_id, task_id, worktree_path,
                   branch, base_sha, head_sha, source_root, status,
                   unintegrated_diff, created_at_epoch_ms, updated_at_epoch_ms
            FROM managed_worktrees
            WHERE id = ?1 OR thread_id = ?1
            LIMIT 1
            "#,
        )?;
        let mut rows = stmt.query(params![id_or_thread_id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(Self::parse_worktree_row(row)?))
        } else {
            Ok(None)
        }
    }

    /// Lists active or preserved managed worktrees for a workflow.
    pub fn list_worktrees_for_workflow(&self, workflow_id: &str) -> Result<Vec<WorktreeRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            r#"
            SELECT id, thread_id, workflow_id, task_id, worktree_path,
                   branch, base_sha, head_sha, source_root, status,
                   unintegrated_diff, created_at_epoch_ms, updated_at_epoch_ms
            FROM managed_worktrees
            WHERE workflow_id = ?1
            ORDER BY created_at_epoch_ms ASC
            "#,
        )?;
        let rows = stmt.query_map(params![workflow_id], |row| {
            Self::parse_worktree_row(row)
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(e.into()))
        })?;
        let mut list = Vec::new();
        for r in rows {
            list.push(r?);
        }
        Ok(list)
    }

    /// Updates worktree status and optional unintegrated diff.
    pub fn update_worktree_status(
        &self,
        thread_id: &str,
        status: WorktreeStatus,
        unintegrated_diff: Option<&str>,
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().timestamp_millis();
        conn.execute(
            r#"
            UPDATE managed_worktrees
            SET status = ?1,
                unintegrated_diff = COALESCE(?2, unintegrated_diff),
                updated_at_epoch_ms = ?3
            WHERE id = ?4 OR thread_id = ?4
            "#,
            params![status.as_str(), unintegrated_diff, now, thread_id],
        )?;
        Ok(())
    }

    /// Recovers all active or preserved worktrees for a workflow after process interruption.
    pub fn recover_interrupted_worktrees(&self, workflow_id: &str) -> Result<Vec<WorktreeRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            r#"
            SELECT id, thread_id, workflow_id, task_id, worktree_path,
                   branch, base_sha, head_sha, source_root, status,
                   unintegrated_diff, created_at_epoch_ms, updated_at_epoch_ms
            FROM managed_worktrees
            WHERE workflow_id = ?1 AND status IN ('active', 'preserved')
            ORDER BY created_at_epoch_ms ASC
            "#,
        )?;
        let rows = stmt.query_map(params![workflow_id], |row| {
            Self::parse_worktree_row(row)
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(e.into()))
        })?;
        let mut list = Vec::new();
        for r in rows {
            list.push(r?);
        }
        Ok(list)
    }

    fn parse_worktree_row(row: &rusqlite::Row) -> Result<WorktreeRecord> {
        let path_str: String = row.get(4)?;
        let root_str: String = row.get(8)?;
        let status_str: String = row.get(9)?;
        let status = WorktreeStatus::from_str(&status_str).unwrap_or(WorktreeStatus::Active);
        Ok(WorktreeRecord {
            id: row.get(0)?,
            thread_id: row.get(1)?,
            workflow_id: row.get(2)?,
            task_id: row.get(3)?,
            worktree_path: PathBuf::from(path_str),
            branch: row.get(5)?,
            base_sha: row.get(6)?,
            head_sha: row.get(7)?,
            source_root: PathBuf::from(root_str),
            status,
            unintegrated_diff: row.get(10)?,
            created_at_epoch_ms: row.get(11)?,
            updated_at_epoch_ms: row.get(12)?,
        })
    }

    /// Persists a new integration record in the durable integration pipeline.
    pub fn create_integration_record(&self, record: &IntegrationRecord) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let files_json = serde_json::to_string(&record.modified_files)?;
        let path_str = record
            .patch_artifact_path
            .as_ref()
            .map(|p| p.to_string_lossy().to_string());
        conn.execute(
            r#"
            INSERT INTO workflow_integrations (
                id, workflow_id, task_id, attempt_id, status,
                source_branch, source_base_sha, target_base_sha, current_target_sha,
                change_set_hash, patch_artifact_path, modified_files_json,
                verification_passed, error_details, created_at_epoch_ms, updated_at_epoch_ms
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)
            ON CONFLICT(id) DO UPDATE SET
                status = excluded.status,
                current_target_sha = COALESCE(excluded.current_target_sha, workflow_integrations.current_target_sha),
                verification_passed = excluded.verification_passed,
                error_details = excluded.error_details,
                updated_at_epoch_ms = excluded.updated_at_epoch_ms
            "#,
            params![
                record.id,
                record.workflow_id,
                record.task_id,
                record.attempt_id,
                record.status.as_str(),
                record.source_branch,
                record.source_base_sha,
                record.target_base_sha,
                record.current_target_sha,
                record.change_set_hash,
                path_str,
                files_json,
                if record.verification_passed { 1 } else { 0 },
                record.error,
                record.created_at_epoch_ms,
                record.updated_at_epoch_ms,
            ],
        )
        .context("Failed inserting workflow integration record into SQLite")?;
        Ok(())
    }

    /// Fetches an integration record by ID.
    pub fn get_integration_record(&self, id: &str) -> Result<Option<IntegrationRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            r#"
            SELECT id, workflow_id, task_id, attempt_id, status,
                   source_branch, source_base_sha, target_base_sha, current_target_sha,
                   change_set_hash, patch_artifact_path, modified_files_json,
                   verification_passed, error_details, created_at_epoch_ms, updated_at_epoch_ms
            FROM workflow_integrations
            WHERE id = ?1
            "#,
        )?;
        let mut rows = stmt.query(params![id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(Self::parse_integration_row(row)?))
        } else {
            Ok(None)
        }
    }

    /// Fetches an integration record for a specific task attempt.
    pub fn get_integration_by_task(
        &self,
        workflow_id: &str,
        task_id: &str,
        attempt_id: u32,
    ) -> Result<Option<IntegrationRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            r#"
            SELECT id, workflow_id, task_id, attempt_id, status,
                   source_branch, source_base_sha, target_base_sha, current_target_sha,
                   change_set_hash, patch_artifact_path, modified_files_json,
                   verification_passed, error_details, created_at_epoch_ms, updated_at_epoch_ms
            FROM workflow_integrations
            WHERE workflow_id = ?1 AND task_id = ?2 AND attempt_id = ?3
            ORDER BY created_at_epoch_ms DESC
            LIMIT 1
            "#,
        )?;
        let mut rows = stmt.query(params![workflow_id, task_id, attempt_id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(Self::parse_integration_row(row)?))
        } else {
            Ok(None)
        }
    }

    /// Finds any previously PROMOTED integration with identical change-set hash for idempotent deduplication.
    pub fn find_promoted_integration_by_hash(
        &self,
        workflow_id: &str,
        change_set_hash: &str,
    ) -> Result<Option<IntegrationRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            r#"
            SELECT id, workflow_id, task_id, attempt_id, status,
                   source_branch, source_base_sha, target_base_sha, current_target_sha,
                   change_set_hash, patch_artifact_path, modified_files_json,
                   verification_passed, error_details, created_at_epoch_ms, updated_at_epoch_ms
            FROM workflow_integrations
            WHERE workflow_id = ?1 AND change_set_hash = ?2 AND status = 'promoted'
            LIMIT 1
            "#,
        )?;
        let mut rows = stmt.query(params![workflow_id, change_set_hash])?;
        if let Some(row) = rows.next()? {
            Ok(Some(Self::parse_integration_row(row)?))
        } else {
            Ok(None)
        }
    }

    /// Updates integration lifecycle status, target sha, and error details.
    pub fn update_integration_status(
        &self,
        id: &str,
        status: IntegrationLifecycleStatus,
        current_target_sha: Option<&str>,
        error: Option<&str>,
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().timestamp_millis();
        let verif_passed = if status == IntegrationLifecycleStatus::Verified
            || status == IntegrationLifecycleStatus::Promoted
        {
            1
        } else {
            0
        };
        conn.execute(
            r#"
            UPDATE workflow_integrations
            SET status = ?1,
                current_target_sha = COALESCE(?2, current_target_sha),
                verification_passed = CASE WHEN ?3 = 1 THEN 1 ELSE verification_passed END,
                error_details = COALESCE(?4, error_details),
                updated_at_epoch_ms = ?5
            WHERE id = ?6
            "#,
            params![
                status.as_str(),
                current_target_sha,
                verif_passed,
                error,
                now,
                id
            ],
        )?;
        Ok(())
    }

    /// Lists all integrations for a workflow.
    pub fn list_integrations(&self, workflow_id: &str) -> Result<Vec<IntegrationRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            r#"
            SELECT id, workflow_id, task_id, attempt_id, status,
                   source_branch, source_base_sha, target_base_sha, current_target_sha,
                   change_set_hash, patch_artifact_path, modified_files_json,
                   verification_passed, error_details, created_at_epoch_ms, updated_at_epoch_ms
            FROM workflow_integrations
            WHERE workflow_id = ?1
            ORDER BY created_at_epoch_ms ASC
            "#,
        )?;
        let rows = stmt.query_map(params![workflow_id], |row| {
            Self::parse_integration_row(row)
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(e.into()))
        })?;
        let mut list = Vec::new();
        for r in rows {
            list.push(r?);
        }
        Ok(list)
    }

    fn parse_integration_row(row: &rusqlite::Row) -> Result<IntegrationRecord> {
        let status_str: String = row.get(4)?;
        let status = IntegrationLifecycleStatus::from_str(&status_str)
            .unwrap_or(IntegrationLifecycleStatus::Created);
        let path_str: Option<String> = row.get(10)?;
        let files_str: String = row.get(11)?;
        let files: Vec<PathBuf> = serde_json::from_str(&files_str).unwrap_or_default();
        let verif_int: i64 = row.get(12)?;
        Ok(IntegrationRecord {
            id: row.get(0)?,
            workflow_id: row.get(1)?,
            task_id: row.get(2)?,
            attempt_id: row.get(3)?,
            status,
            source_branch: row.get(5)?,
            source_base_sha: row.get(6)?,
            target_base_sha: row.get(7)?,
            current_target_sha: row.get(8)?,
            change_set_hash: row.get(9)?,
            patch_artifact_path: path_str.map(PathBuf::from),
            modified_files: files,
            verification_passed: verif_int != 0,
            error: row.get(13)?,
            created_at_epoch_ms: row.get(14)?,
            updated_at_epoch_ms: row.get(15)?,
        })
    }

    /// Records a newly discovered execution or resource conflict.
    pub fn record_conflict(&self, report: &ConflictReport) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            r#"
            INSERT INTO workflow_conflicts (
                id, workflow_id, reporting_task_id, conflicting_task_id,
                resource_path, conflict_kind, description, suggested_dependency,
                resolution_json, reported_at_epoch_ms, resolved_at_epoch_ms
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
            "#,
            params![
                report.id,
                report.workflow_id,
                report.reporting_task_id,
                report.conflicting_task_id,
                report.resource_path.to_string_lossy().to_string(),
                report.conflict_kind.as_str(),
                report.description,
                report.suggested_dependency,
                None::<String>,
                report.reported_at_epoch_ms,
                None::<i64>,
            ],
        )
        .context("Failed inserting workflow conflict record into SQLite")?;
        Ok(())
    }

    /// Fetches a conflict report by ID.
    pub fn get_conflict(&self, id: &str) -> Result<Option<ConflictReport>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            r#"
            SELECT id, workflow_id, reporting_task_id, conflicting_task_id,
                   resource_path, conflict_kind, description, suggested_dependency,
                   reported_at_epoch_ms
            FROM workflow_conflicts
            WHERE id = ?1
            "#,
        )?;
        let mut rows = stmt.query(params![id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(Self::parse_conflict_row(row)?))
        } else {
            Ok(None)
        }
    }

    /// Lists all conflicts reported for a workflow.
    pub fn list_conflicts(&self, workflow_id: &str) -> Result<Vec<ConflictReport>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            r#"
            SELECT id, workflow_id, reporting_task_id, conflicting_task_id,
                   resource_path, conflict_kind, description, suggested_dependency,
                   reported_at_epoch_ms
            FROM workflow_conflicts
            WHERE workflow_id = ?1
            ORDER BY reported_at_epoch_ms ASC
            "#,
        )?;
        let rows = stmt.query_map(params![workflow_id], |row| {
            Self::parse_conflict_row(row)
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(e.into()))
        })?;
        let mut list = Vec::new();
        for r in rows {
            list.push(r?);
        }
        Ok(list)
    }

    /// Resolves a conflict report with structured resolution.
    pub fn resolve_conflict(
        &self,
        conflict_id: &str,
        resolution: &ConflictResolution,
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().timestamp_millis();
        let res_json = serde_json::to_string(resolution)?;
        conn.execute(
            r#"
            UPDATE workflow_conflicts
            SET resolution_json = ?1,
                resolved_at_epoch_ms = ?2
            WHERE id = ?3
            "#,
            params![res_json, now, conflict_id],
        )?;
        Ok(())
    }

    /// Transactionally updates the task DAG when a genuine dependency is discovered,
    /// validating cycles, persisting the new revision, updating task dependencies,
    /// yielding the dependent task, and recording audit events.
    pub fn replan_with_dependency(
        &self,
        workflow_id: &str,
        task_id: &str,
        depends_on_task_id: &str,
        rationale: &str,
    ) -> Result<WorkflowPlan> {
        let current_plan = self
            .get_workflow_plan(workflow_id, None)?
            .ok_or_else(|| anyhow!("Workflow '{workflow_id}' has no plan"))?;

        // 1. Cycle validation
        if current_plan.would_create_cycle(task_id, depends_on_task_id) {
            bail!(
                "Cannot add dependency from '{task_id}' to '{depends_on_task_id}': cyclic dependency detected in task DAG"
            );
        }

        // 2. Generate new revision
        let new_plan =
            current_plan.with_added_dependency(task_id, depends_on_task_id, rationale)?;

        // 3. Transactionally persist new plan and update task state
        {
            let mut conn = self.conn.lock().unwrap();
            let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
            let now = Utc::now().timestamp_millis();

            // Insert new plan header
            tx.execute(
                r#"
                INSERT INTO workflow_plans (workflow_id, revision, rationale, created_at_epoch_ms)
                VALUES (?1, ?2, ?3, ?4)
                "#,
                params![
                    new_plan.workflow_id,
                    new_plan.revision,
                    new_plan.rationale,
                    now
                ],
            )?;

            // Update workflow run plan revision
            tx.execute(
                r#"
                UPDATE workflows
                SET plan_revision = ?1, updated_at_epoch_ms = ?2
                WHERE id = ?3
                "#,
                params![new_plan.revision, now, workflow_id],
            )?;

            // Insert new/updated dependency
            tx.execute(
                r#"
                INSERT OR IGNORE INTO workflow_task_dependencies (workflow_id, task_id, depends_on_task_id)
                VALUES (?1, ?2, ?3)
                "#,
                params![workflow_id, task_id, depends_on_task_id],
            )?;

            // Update all tasks in this workflow to the new plan revision
            tx.execute(
                "UPDATE workflow_tasks SET plan_revision = ?1 WHERE workflow_id = ?2",
                params![new_plan.revision, workflow_id],
            )?;

            // Yield current task: transition to 'blocked' if it was running/claimed/ready
            tx.execute(
                r#"
                UPDATE workflow_tasks
                SET status = 'blocked',
                    plan_revision = ?1,
                    worker_id = NULL,
                    lease_expires_at_epoch_ms = NULL,
                    dependencies_json = (
                        SELECT json_group_array(depends_on_task_id)
                        FROM workflow_task_dependencies
                        WHERE workflow_id = ?2 AND task_id = ?3
                    )
                WHERE workflow_id = ?2 AND id = ?3
                "#,
                params![new_plan.revision, workflow_id, task_id],
            )?;

            // Promote any pending tasks whose dependencies are satisfied
            tx.execute(
                r#"
                UPDATE workflow_tasks
                SET status = 'ready'
                WHERE workflow_id = ?1
                  AND status IN ('pending', 'blocked')
                  AND NOT EXISTS (
                      SELECT 1
                      FROM workflow_task_dependencies d
                      JOIN workflow_tasks dep ON dep.workflow_id = d.workflow_id AND dep.id = d.depends_on_task_id
                      WHERE d.workflow_id = ?1
                        AND d.task_id = workflow_tasks.id
                        AND dep.status != 'completed'
                  )
                "#,
                params![workflow_id],
            )?;

            tx.commit()?;
        }

        // Record plan revised event
        self.record_event(&WorkflowEvent {
            id: format!("evt-replan-{}", Uuid::new_v4().simple()),
            workflow_id: workflow_id.to_string(),
            task_id: Some(task_id.to_string()),
            agent_id: None,
            event_type: WorkflowEventType::PlanRevised,
            timestamp_epoch_ms: Utc::now().timestamp_millis(),
            correlation_id: None,
            payload_json: serde_json::json!({
                "revision": new_plan.revision,
                "task_id": task_id,
                "added_dependency": depends_on_task_id,
                "rationale": rationale,
            })
            .to_string(),
            schema_version: 1,
        })?;

        Ok(new_plan)
    }

    fn parse_conflict_row(row: &rusqlite::Row) -> Result<ConflictReport> {
        let path_str: String = row.get(4)?;
        let kind_str: String = row.get(5)?;
        let kind = ConflictKind::from_str(&kind_str).unwrap_or(ConflictKind::GenuineDependency);
        Ok(ConflictReport {
            id: row.get(0)?,
            workflow_id: row.get(1)?,
            reporting_task_id: row.get(2)?,
            conflicting_task_id: row.get(3)?,
            resource_path: PathBuf::from(path_str),
            conflict_kind: kind,
            description: row.get(6)?,
            suggested_dependency: row.get(7)?,
            reported_at_epoch_ms: row.get(8)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn setup_test_db() -> (WorkflowStore, Arc<Mutex<Connection>>) {
        let conn = Connection::open_in_memory().unwrap();
        let conn = Arc::new(Mutex::new(conn));
        {
            let mut c = conn.lock().unwrap();
            crate::migrations::MigrationEngine::apply_all(
                &mut c,
                crate::migrations::BUILTIN_MIGRATIONS,
            )
            .unwrap();
        }
        (WorkflowStore::new(conn.clone()), conn)
    }

    #[test]
    fn test_workflow_lifecycle_persistence_and_recovery() {
        let (store, _) = setup_test_db();
        let wf_id = "wf-test-1";

        let run = WorkflowRun {
            id: wf_id.to_string(),
            thread_id: None,
            workspace_root: PathBuf::from("/workspace"),
            objective: "Implement resilient backup".into(),
            workflow_type: "autonomous".into(),
            state: WorkflowState::Created,
            plan_revision: 1,
            configuration_json: "{}".into(),
            created_at_epoch_ms: 1000,
            updated_at_epoch_ms: 1000,
            started_at_epoch_ms: None,
            completed_at_epoch_ms: None,
            cancellation_reason: None,
            overall_result_json: None,
            resource_usage_json: None,
        };

        store.create_workflow(&run).unwrap();
        let fetched = store.get_workflow(wf_id).unwrap().unwrap();
        assert_eq!(fetched.objective, "Implement resilient backup");
        assert_eq!(fetched.state, WorkflowState::Created);

        // Update state
        store
            .update_workflow_state(wf_id, WorkflowState::Running)
            .unwrap();
        let updated = store.get_workflow(wf_id).unwrap().unwrap();
        assert_eq!(updated.state, WorkflowState::Running);
        assert!(updated.started_at_epoch_ms.is_some());
    }

    #[test]
    fn test_task_claiming_and_lease_expiry_reconciliation() {
        let (store, _) = setup_test_db();
        let wf_id = "wf-test-2";

        let run = WorkflowRun {
            id: wf_id.to_string(),
            thread_id: None,
            workspace_root: PathBuf::from("/workspace"),
            objective: "Test lease recovery".into(),
            workflow_type: "autonomous".into(),
            state: WorkflowState::Running,
            plan_revision: 1,
            configuration_json: "{}".into(),
            created_at_epoch_ms: 1000,
            updated_at_epoch_ms: 1000,
            started_at_epoch_ms: Some(1000),
            completed_at_epoch_ms: None,
            cancellation_reason: None,
            overall_result_json: None,
            resource_usage_json: None,
        };
        store.create_workflow(&run).unwrap();

        let task = WorkflowTask {
            id: "task-1".into(),
            workflow_id: wf_id.into(),
            plan_revision: 1,
            task_kind: m31a_core::workflow::WorkflowTaskKind::Implementation,
            name: "Initial task".into(),
            description: "Do something".into(),
            assigned_role: "Implementer".into(),
            assigned_agent_id: None,
            priority: 10,
            wave_index: 1,
            dependencies: vec![],
            acceptance_criteria: vec![],
            required_verification: vec![],
            declared_read_paths: vec![],
            declared_write_paths: vec![],
            workspace_isolation: WorkspaceIsolationStrategy::Worktree,
            status: WorkflowTaskStatus::Ready,
            retry_count: 0,
            max_retries: 2,
            attempt_id: 1,
            started_at_epoch_ms: None,
            completed_at_epoch_ms: None,
            worker_id: None,
            lease_expires_at_epoch_ms: None,
            error_details: None,
            input_artifacts: vec![],
            output_artifacts: vec![],
            verification_result_json: None,
            execution_result_json: None,
        };

        let plan = WorkflowPlan {
            workflow_id: wf_id.into(),
            revision: 1,
            rationale: "Test plan".into(),
            created_at_epoch_ms: 1000,
            tasks: vec![task],
        };
        store.save_workflow_plan(&plan).unwrap();

        // 1. Worker 1 claims task with 500ms lease
        let claimed = store
            .claim_ready_task(wf_id, "worker-alpha", 500)
            .unwrap()
            .unwrap();
        assert_eq!(claimed.id, "task-1");
        assert_eq!(claimed.status, WorkflowTaskStatus::Claimed);
        assert_eq!(claimed.worker_id.as_deref(), Some("worker-alpha"));

        // No other worker can claim it now
        let second_claim = store.claim_ready_task(wf_id, "worker-beta", 500).unwrap();
        assert!(second_claim.is_none());

        // 2. Simulate lease expiration after 1000ms
        let future_time = Utc::now().timestamp_millis() + 1000;
        let recovered = store.reconcile_stale_tasks(wf_id, future_time).unwrap();
        assert_eq!(recovered, 1);

        // Task should now be in retry_pending with retry_count = 1
        let recovered_task = store.get_task(wf_id, "task-1").unwrap().unwrap();
        assert_eq!(recovered_task.status, WorkflowTaskStatus::RetryPending);
        assert_eq!(recovered_task.retry_count, 1);
        assert!(recovered_task.worker_id.is_none());

        // 3. Worker beta can now claim it
        let beta_claim = store
            .claim_ready_task(wf_id, "worker-beta", 500)
            .unwrap()
            .unwrap();
        assert_eq!(beta_claim.worker_id.as_deref(), Some("worker-beta"));
    }

    #[test]
    fn test_artifact_file_persistence_and_hashing() {
        let (store, _) = setup_test_db();
        let temp_dir = tempdir().unwrap();
        let wf_id = "wf-test-3";

        let run = WorkflowRun {
            id: wf_id.to_string(),
            thread_id: None,
            workspace_root: PathBuf::from("/workspace"),
            objective: "Test artifacts".into(),
            workflow_type: "autonomous".into(),
            state: WorkflowState::Created,
            plan_revision: 1,
            configuration_json: "{}".into(),
            created_at_epoch_ms: 1000,
            updated_at_epoch_ms: 1000,
            started_at_epoch_ms: None,
            completed_at_epoch_ms: None,
            cancellation_reason: None,
            overall_result_json: None,
            resource_usage_json: None,
        };
        store.create_workflow(&run).unwrap();

        let artifact = store
            .store_artifact_file(
                temp_dir.path(),
                wf_id,
                Some("task-1"),
                "architecture_plan.md",
                "plan",
                b"# Engineering Plan\nStep 1: Build\nStep 2: Verify",
                "text/markdown",
            )
            .unwrap();

        assert_eq!(artifact.name, "architecture_plan.md");
        assert!(artifact.file_path.exists());
        assert_eq!(
            fs::read(&artifact.file_path).unwrap(),
            b"# Engineering Plan\nStep 1: Build\nStep 2: Verify"
        );

        let list = store.list_artifacts(wf_id).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].content_sha256, artifact.content_sha256);
    }

    #[test]
    fn test_task_dependency_claiming_and_fencing() {
        let (store, _) = setup_test_db();
        let wf_id = "wf-dep-test";

        let run = WorkflowRun {
            id: wf_id.to_string(),
            thread_id: None,
            workspace_root: PathBuf::from("/workspace"),
            objective: "Dependency and fencing test".into(),
            workflow_type: "autonomous".into(),
            state: WorkflowState::Running,
            plan_revision: 1,
            configuration_json: "{}".into(),
            created_at_epoch_ms: 1000,
            updated_at_epoch_ms: 1000,
            started_at_epoch_ms: Some(1000),
            completed_at_epoch_ms: None,
            cancellation_reason: None,
            overall_result_json: None,
            resource_usage_json: None,
        };
        store.create_workflow(&run).unwrap();

        let task_a = WorkflowTask {
            id: "task-A".into(),
            workflow_id: wf_id.into(),
            plan_revision: 1,
            task_kind: m31a_core::workflow::WorkflowTaskKind::Implementation,
            name: "Task A".into(),
            description: "First task".into(),
            assigned_role: "Dev".into(),
            assigned_agent_id: None,
            priority: 10,
            wave_index: 1,
            dependencies: vec![],
            acceptance_criteria: vec![],
            required_verification: vec![],
            declared_read_paths: vec![],
            declared_write_paths: vec![],
            workspace_isolation: WorkspaceIsolationStrategy::Worktree,
            status: WorkflowTaskStatus::Ready,
            retry_count: 0,
            max_retries: 2,
            attempt_id: 1,
            started_at_epoch_ms: None,
            completed_at_epoch_ms: None,
            worker_id: None,
            lease_expires_at_epoch_ms: None,
            error_details: None,
            input_artifacts: vec![],
            output_artifacts: vec![],
            verification_result_json: None,
            execution_result_json: None,
        };

        let task_b = WorkflowTask {
            id: "task-B".into(),
            workflow_id: wf_id.into(),
            plan_revision: 1,
            task_kind: m31a_core::workflow::WorkflowTaskKind::Verification,
            name: "Task B (depends on A)".into(),
            description: "Second task".into(),
            assigned_role: "QA".into(),
            assigned_agent_id: None,
            priority: 20, // Higher priority than A, but depends on A!
            wave_index: 2,
            dependencies: vec!["task-A".into()],
            acceptance_criteria: vec![],
            required_verification: vec![],
            declared_read_paths: vec![],
            declared_write_paths: vec![],
            workspace_isolation: WorkspaceIsolationStrategy::Worktree,
            status: WorkflowTaskStatus::Ready,
            retry_count: 0,
            max_retries: 2,
            attempt_id: 1,
            started_at_epoch_ms: None,
            completed_at_epoch_ms: None,
            worker_id: None,
            lease_expires_at_epoch_ms: None,
            error_details: None,
            input_artifacts: vec![],
            output_artifacts: vec![],
            verification_result_json: None,
            execution_result_json: None,
        };

        let plan = WorkflowPlan {
            workflow_id: wf_id.into(),
            revision: 1,
            rationale: "Dependency test".into(),
            created_at_epoch_ms: 1000,
            tasks: vec![task_a, task_b],
        };
        store.save_workflow_plan(&plan).unwrap();

        // 1. Claim: Even though Task B has higher priority (20 > 10), Task A must be claimed
        // because Task B has an uncompleted dependency on Task A!
        let claimed = store
            .claim_ready_task(wf_id, "worker-1", 10_000)
            .unwrap()
            .unwrap();
        assert_eq!(claimed.id, "task-A");
        assert_eq!(claimed.attempt_id, 2);
        assert_eq!(claimed.worker_id.as_deref(), Some("worker-1"));

        // Second claim right now returns None, because Task B is blocked on Task A!
        let second_claim = store.claim_ready_task(wf_id, "worker-2", 10_000).unwrap();
        assert!(second_claim.is_none());

        // 2. Fencing test: Stale worker (attempt 1 or wrong worker) tries to update Task A
        let stale_res = store.update_task_execution_result(
            wf_id,
            "task-A",
            Some(1), // Stale attempt ID!
            Some("worker-1"),
            WorkflowTaskStatus::Completed,
            None,
            None,
        );
        assert!(stale_res.is_err());

        // Wrong worker ID fails
        let wrong_worker_res = store.update_task_execution_result(
            wf_id,
            "task-A",
            Some(2),
            Some("worker-stale"),
            WorkflowTaskStatus::Completed,
            None,
            None,
        );
        assert!(wrong_worker_res.is_err());

        // Valid update by worker-1 with attempt 2 completes Task A
        store
            .update_task_execution_result(
                wf_id,
                "task-A",
                Some(2),
                Some("worker-1"),
                WorkflowTaskStatus::Completed,
                None,
                None,
            )
            .unwrap();

        // 3. Now that Task A is completed, Task B is immediately claimable!
        let claimed_b = store
            .claim_ready_task(wf_id, "worker-2", 10_000)
            .unwrap()
            .unwrap();
        assert_eq!(claimed_b.id, "task-B");
        assert_eq!(claimed_b.worker_id.as_deref(), Some("worker-2"));
    }

    #[test]
    fn test_renew_task_lease_and_fencing() {
        let (store, _) = setup_test_db();
        let wf_id = "wf-lease-test";
        let run = WorkflowRun {
            id: wf_id.to_string(),
            thread_id: None,
            workspace_root: PathBuf::from("/workspace"),
            objective: "Lease test".into(),
            workflow_type: "autonomous".into(),
            state: WorkflowState::Running,
            plan_revision: 1,
            configuration_json: "{}".into(),
            created_at_epoch_ms: 1000,
            updated_at_epoch_ms: 1000,
            started_at_epoch_ms: Some(1000),
            completed_at_epoch_ms: None,
            cancellation_reason: None,
            overall_result_json: None,
            resource_usage_json: None,
        };
        store.create_workflow(&run).unwrap();

        let task = WorkflowTask {
            id: "task-lease".into(),
            workflow_id: wf_id.into(),
            plan_revision: 1,
            task_kind: m31a_core::workflow::WorkflowTaskKind::Implementation,
            name: "Task Lease".into(),
            description: "Test".into(),
            assigned_role: "Dev".into(),
            assigned_agent_id: None,
            priority: 10,
            wave_index: 0,
            dependencies: vec![],
            acceptance_criteria: vec![],
            required_verification: vec![],
            declared_read_paths: vec![],
            declared_write_paths: vec![],
            workspace_isolation: WorkspaceIsolationStrategy::Worktree,
            status: WorkflowTaskStatus::Ready,
            retry_count: 0,
            max_retries: 2,
            attempt_id: 0,
            started_at_epoch_ms: None,
            completed_at_epoch_ms: None,
            worker_id: None,
            lease_expires_at_epoch_ms: None,
            error_details: None,
            input_artifacts: vec![],
            output_artifacts: vec![],
            verification_result_json: None,
            execution_result_json: None,
        };

        let plan = WorkflowPlan {
            workflow_id: wf_id.into(),
            revision: 1,
            rationale: "Lease test".into(),
            created_at_epoch_ms: 1000,
            tasks: vec![task],
        };
        store.save_workflow_plan(&plan).unwrap();

        let claimed = store
            .claim_ready_task(wf_id, "worker-lease-1", 5_000)
            .unwrap()
            .unwrap();
        assert_eq!(claimed.attempt_id, 1);

        // Stale attempt renewal fails
        let stale_renew = store
            .renew_task_lease(wf_id, "task-lease", "worker-lease-1", 0, 5_000)
            .unwrap();
        assert!(!stale_renew);

        // Wrong worker renewal fails
        let wrong_worker_renew = store
            .renew_task_lease(wf_id, "task-lease", "worker-other", 1, 5_000)
            .unwrap();
        assert!(!wrong_worker_renew);

        // Non-positive lease renewal returns error
        let non_pos_renew = store.renew_task_lease(wf_id, "task-lease", "worker-lease-1", 1, 0);
        assert!(non_pos_renew.is_err());

        // Valid renewal succeeds
        let valid_renew = store
            .renew_task_lease(wf_id, "task-lease", "worker-lease-1", 1, 10_000)
            .unwrap();
        assert!(valid_renew);
    }

    #[test]
    fn test_shared_workspace_write_conflict_isolation() {
        let (store, _) = setup_test_db();
        let wf_id = "wf-conflict-test";
        let run = WorkflowRun {
            id: wf_id.to_string(),
            thread_id: None,
            workspace_root: PathBuf::from("/workspace"),
            objective: "Conflict test".into(),
            workflow_type: "autonomous".into(),
            state: WorkflowState::Running,
            plan_revision: 1,
            configuration_json: "{}".into(),
            created_at_epoch_ms: 1000,
            updated_at_epoch_ms: 1000,
            started_at_epoch_ms: Some(1000),
            completed_at_epoch_ms: None,
            cancellation_reason: None,
            overall_result_json: None,
            resource_usage_json: None,
        };
        store.create_workflow(&run).unwrap();

        let task_shared_1 = WorkflowTask {
            id: "task-s1".into(),
            workflow_id: wf_id.into(),
            plan_revision: 1,
            task_kind: m31a_core::workflow::WorkflowTaskKind::Implementation,
            name: "Task S1".into(),
            description: "Test".into(),
            assigned_role: "Dev".into(),
            assigned_agent_id: None,
            priority: 20,
            wave_index: 0,
            dependencies: vec![],
            acceptance_criteria: vec![],
            required_verification: vec![],
            declared_read_paths: vec![],
            declared_write_paths: vec![PathBuf::from("src/conflict.rs")],
            workspace_isolation: WorkspaceIsolationStrategy::SharedWorkspace,
            status: WorkflowTaskStatus::Ready,
            retry_count: 0,
            max_retries: 2,
            attempt_id: 0,
            started_at_epoch_ms: None,
            completed_at_epoch_ms: None,
            worker_id: None,
            lease_expires_at_epoch_ms: None,
            error_details: None,
            input_artifacts: vec![],
            output_artifacts: vec![],
            verification_result_json: None,
            execution_result_json: None,
        };

        let task_shared_2 = WorkflowTask {
            id: "task-s2".into(),
            workflow_id: wf_id.into(),
            plan_revision: 1,
            task_kind: m31a_core::workflow::WorkflowTaskKind::Implementation,
            name: "Task S2".into(),
            description: "Test".into(),
            assigned_role: "Dev".into(),
            assigned_agent_id: None,
            priority: 15,
            wave_index: 0,
            dependencies: vec![],
            acceptance_criteria: vec![],
            required_verification: vec![],
            declared_read_paths: vec![],
            declared_write_paths: vec![PathBuf::from("src/conflict.rs")],
            workspace_isolation: WorkspaceIsolationStrategy::SharedWorkspace,
            status: WorkflowTaskStatus::Ready,
            retry_count: 0,
            max_retries: 2,
            attempt_id: 0,
            started_at_epoch_ms: None,
            completed_at_epoch_ms: None,
            worker_id: None,
            lease_expires_at_epoch_ms: None,
            error_details: None,
            input_artifacts: vec![],
            output_artifacts: vec![],
            verification_result_json: None,
            execution_result_json: None,
        };

        let task_worktree = WorkflowTask {
            id: "task-wt".into(),
            workflow_id: wf_id.into(),
            plan_revision: 1,
            task_kind: m31a_core::workflow::WorkflowTaskKind::Implementation,
            name: "Task WT".into(),
            description: "Test".into(),
            assigned_role: "Dev".into(),
            assigned_agent_id: None,
            priority: 10,
            wave_index: 0,
            dependencies: vec![],
            acceptance_criteria: vec![],
            required_verification: vec![],
            declared_read_paths: vec![],
            declared_write_paths: vec![PathBuf::from("src/conflict.rs")],
            workspace_isolation: WorkspaceIsolationStrategy::Worktree,
            status: WorkflowTaskStatus::Ready,
            retry_count: 0,
            max_retries: 2,
            attempt_id: 0,
            started_at_epoch_ms: None,
            completed_at_epoch_ms: None,
            worker_id: None,
            lease_expires_at_epoch_ms: None,
            error_details: None,
            input_artifacts: vec![],
            output_artifacts: vec![],
            verification_result_json: None,
            execution_result_json: None,
        };

        let plan = WorkflowPlan {
            workflow_id: wf_id.into(),
            revision: 1,
            rationale: "Conflict test".into(),
            created_at_epoch_ms: 1000,
            tasks: vec![task_shared_1, task_shared_2, task_worktree],
        };
        store.save_workflow_plan(&plan).unwrap();

        // 1. Claim task-s1 (highest priority 20)
        let claim1 = store
            .claim_ready_task(wf_id, "worker-1", 10_000)
            .unwrap()
            .unwrap();
        assert_eq!(claim1.id, "task-s1");

        // 2. Next claim should skip task-s2 (which conflicts on src/conflict.rs in Shared mode)
        // and instead claim task-wt which has Worktree isolation!
        let claim2 = store
            .claim_ready_task(wf_id, "worker-2", 10_000)
            .unwrap()
            .unwrap();
        assert_eq!(claim2.id, "task-wt");

        // 3. Third claim returns None because task-s2 is still in write conflict with task-s1
        let claim3 = store.claim_ready_task(wf_id, "worker-3", 10_000).unwrap();
        assert!(claim3.is_none());
    }

    #[test]
    fn test_managed_worktree_persistence_and_recovery() {
        let (store, _) = setup_test_db();
        let wt = WorktreeRecord {
            id: "wt-agent-1".into(),
            thread_id: "agent-1".into(),
            workflow_id: Some("wf-rec".into()),
            task_id: Some("task-1".into()),
            worktree_path: PathBuf::from("/tmp/wt1"),
            branch: "m31a/thread-agent-1".into(),
            base_sha: "abc1234".into(),
            head_sha: "abc1234".into(),
            source_root: PathBuf::from("/workspace"),
            status: WorktreeStatus::Active,
            unintegrated_diff: None,
            created_at_epoch_ms: 1000,
            updated_at_epoch_ms: 1000,
        };

        store.save_worktree(&wt).unwrap();

        let fetched = store.get_worktree("agent-1").unwrap().unwrap();
        assert_eq!(fetched.id, "wt-agent-1");
        assert_eq!(fetched.status, WorktreeStatus::Active);

        let list = store.list_worktrees_for_workflow("wf-rec").unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].thread_id, "agent-1");

        // Update status and save unintegrated diff
        store
            .update_worktree_status(
                "agent-1",
                WorktreeStatus::Preserved,
                Some("diff --git a/file.rs b/file.rs"),
            )
            .unwrap();

        let recovered = store.recover_interrupted_worktrees("wf-rec").unwrap();
        assert_eq!(recovered.len(), 1);
        assert_eq!(recovered[0].status, WorktreeStatus::Preserved);
        assert_eq!(
            recovered[0].unintegrated_diff.as_deref(),
            Some("diff --git a/file.rs b/file.rs")
        );
    }

    #[test]
    fn test_integration_record_lifecycle_and_deduplication() {
        let (store, _) = setup_test_db();
        let wf_id = "wf-integ";
        let run = WorkflowRun {
            id: wf_id.to_string(),
            thread_id: None,
            workspace_root: PathBuf::from("/workspace"),
            objective: "Integration pipeline test".into(),
            workflow_type: "autonomous".into(),
            state: WorkflowState::Running,
            plan_revision: 1,
            configuration_json: "{}".into(),
            created_at_epoch_ms: 1000,
            updated_at_epoch_ms: 1000,
            started_at_epoch_ms: Some(1000),
            completed_at_epoch_ms: None,
            cancellation_reason: None,
            overall_result_json: None,
            resource_usage_json: None,
        };
        store.create_workflow(&run).unwrap();

        let rec = IntegrationRecord {
            id: "integ-1".into(),
            workflow_id: wf_id.into(),
            task_id: "task-integ".into(),
            attempt_id: 1,
            status: IntegrationLifecycleStatus::Created,
            source_branch: "m31a/thread-1".into(),
            source_base_sha: "base111".into(),
            target_base_sha: "base111".into(),
            current_target_sha: None,
            change_set_hash: "hash_abcdef".into(),
            patch_artifact_path: Some(PathBuf::from("/artifacts/patch.diff")),
            modified_files: vec![PathBuf::from("src/lib.rs")],
            verification_passed: false,
            error: None,
            created_at_epoch_ms: 1000,
            updated_at_epoch_ms: 1000,
        };

        store.create_integration_record(&rec).unwrap();

        let fetched = store.get_integration_record("integ-1").unwrap().unwrap();
        assert_eq!(fetched.status, IntegrationLifecycleStatus::Created);

        // Progress status
        store
            .update_integration_status(
                "integ-1",
                IntegrationLifecycleStatus::Verified,
                Some("base222"),
                None,
            )
            .unwrap();

        let updated = store
            .get_integration_by_task(wf_id, "task-integ", 1)
            .unwrap()
            .unwrap();
        assert_eq!(updated.status, IntegrationLifecycleStatus::Verified);
        assert!(updated.verification_passed);

        // Progress to Promoted
        store
            .update_integration_status(
                "integ-1",
                IntegrationLifecycleStatus::Promoted,
                Some("commit_333"),
                None,
            )
            .unwrap();

        let deduplicated = store
            .find_promoted_integration_by_hash(wf_id, "hash_abcdef")
            .unwrap();
        assert!(deduplicated.is_some());
        assert_eq!(deduplicated.unwrap().id, "integ-1");
    }

    #[test]
    fn test_conflict_recording_and_dynamic_replanning() {
        let (store, _) = setup_test_db();
        let wf_id = "wf-dyn";
        let run = WorkflowRun {
            id: wf_id.to_string(),
            thread_id: None,
            workspace_root: PathBuf::from("/workspace"),
            objective: "Dynamic replan test".into(),
            workflow_type: "autonomous".into(),
            state: WorkflowState::Running,
            plan_revision: 1,
            configuration_json: "{}".into(),
            created_at_epoch_ms: 1000,
            updated_at_epoch_ms: 1000,
            started_at_epoch_ms: Some(1000),
            completed_at_epoch_ms: None,
            cancellation_reason: None,
            overall_result_json: None,
            resource_usage_json: None,
        };
        store.create_workflow(&run).unwrap();

        let task_1 = WorkflowTask {
            id: "t1".into(),
            workflow_id: wf_id.into(),
            plan_revision: 1,
            task_kind: m31a_core::workflow::WorkflowTaskKind::Implementation,
            name: "Task 1".into(),
            description: "Test".into(),
            assigned_role: "Dev".into(),
            assigned_agent_id: None,
            priority: 10,
            wave_index: 1,
            dependencies: vec![],
            acceptance_criteria: vec![],
            required_verification: vec![],
            declared_read_paths: vec![],
            declared_write_paths: vec![PathBuf::from("src/a.rs")],
            workspace_isolation: WorkspaceIsolationStrategy::Worktree,
            status: WorkflowTaskStatus::Ready,
            retry_count: 0,
            max_retries: 2,
            attempt_id: 1,
            started_at_epoch_ms: None,
            completed_at_epoch_ms: None,
            worker_id: None,
            lease_expires_at_epoch_ms: None,
            error_details: None,
            input_artifacts: vec![],
            output_artifacts: vec![],
            verification_result_json: None,
            execution_result_json: None,
        };

        let task_2 = WorkflowTask {
            id: "t2".into(),
            workflow_id: wf_id.into(),
            plan_revision: 1,
            task_kind: m31a_core::workflow::WorkflowTaskKind::Implementation,
            name: "Task 2".into(),
            description: "Test".into(),
            assigned_role: "Dev".into(),
            assigned_agent_id: None,
            priority: 10,
            wave_index: 1,
            dependencies: vec![],
            acceptance_criteria: vec![],
            required_verification: vec![],
            declared_read_paths: vec![],
            declared_write_paths: vec![PathBuf::from("src/b.rs")],
            workspace_isolation: WorkspaceIsolationStrategy::Worktree,
            status: WorkflowTaskStatus::Ready,
            retry_count: 0,
            max_retries: 2,
            attempt_id: 1,
            started_at_epoch_ms: None,
            completed_at_epoch_ms: None,
            worker_id: None,
            lease_expires_at_epoch_ms: None,
            error_details: None,
            input_artifacts: vec![],
            output_artifacts: vec![],
            verification_result_json: None,
            execution_result_json: None,
        };

        let plan = WorkflowPlan {
            workflow_id: wf_id.into(),
            revision: 1,
            rationale: "Initial plan".into(),
            created_at_epoch_ms: 1000,
            tasks: vec![task_1, task_2],
        };
        store.save_workflow_plan(&plan).unwrap();

        // Agent executing t2 discovers that it requires unfinished output from t1!
        let conflict = ConflictReport {
            id: "cnf-1".into(),
            workflow_id: wf_id.into(),
            reporting_task_id: "t2".into(),
            conflicting_task_id: Some("t1".into()),
            resource_path: PathBuf::from("src/shared.rs"),
            conflict_kind: ConflictKind::GenuineDependency,
            description: "t2 depends on types exported by t1".into(),
            suggested_dependency: Some("t1".into()),
            reported_at_epoch_ms: 2000,
        };
        store.record_conflict(&conflict).unwrap();

        let conflict_list = store.list_conflicts(wf_id).unwrap();
        assert_eq!(conflict_list.len(), 1);

        // Orchestrator resolves genuine dependency by adding explicit DAG dependency: t2 depends on t1
        let new_plan = store
            .replan_with_dependency(
                wf_id,
                "t2",
                "t1",
                "t2 dynamically yielded to wait for t1 completion",
            )
            .unwrap();

        assert_eq!(new_plan.revision, 2);
        let t2_updated = new_plan.tasks.iter().find(|t| t.id == "t2").unwrap();
        assert_eq!(t2_updated.dependencies, vec!["t1".to_string()]);

        // In storage, t2 is now blocked waiting for t1!
        let t2_db = store.get_task(wf_id, "t2").unwrap().unwrap();
        assert_eq!(t2_db.status, WorkflowTaskStatus::Blocked);

        // Attempting to add a cyclic dependency (t1 depends on t2) is rejected transactionally
        let cyclic_res = store.replan_with_dependency(wf_id, "t1", "t2", "invalid cycle addition");
        assert!(cyclic_res.is_err());
    }
}
