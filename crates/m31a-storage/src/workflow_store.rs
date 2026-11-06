//! Persistent SQLite storage engine for autonomous workflows, task queues, lease recovery,
//! append-only audit events, and content-addressed artifacts (`m31a-storage::workflow_store`).

use anyhow::{Context, Result, anyhow};
use chrono::Utc;
use m31a_core::workflow::{
    WorkflowArtifact, WorkflowEvent, WorkflowEventType, WorkflowPlan, WorkflowRun, WorkflowState,
    WorkflowTask, WorkflowTaskStatus, WorkspaceIsolationStrategy,
};
use rusqlite::{Connection, params};
use sha2::{Digest, Sha256};
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
                    verification_result_json
                ) VALUES (
                    ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10,
                    ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20,
                    ?21, ?22, ?23, ?24, ?25, ?26
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
                   verification_result_json
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
                   verification_result_json
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

        Ok(WorkflowTask {
            id: row.get(0)?,
            workflow_id: row.get(1)?,
            plan_revision: row.get(2)?,
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
            started_at_epoch_ms: row.get(18)?,
            completed_at_epoch_ms: row.get(19)?,
            worker_id: row.get(20)?,
            lease_expires_at_epoch_ms: row.get(21)?,
            error_details: row.get(22)?,
            input_artifacts,
            output_artifacts,
            verification_result_json: row.get(25)?,
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

        tx.commit()?;
        Ok(())
    }

    /// Atomically claims the next ready or retry_pending task with worker lease semantics.
    pub fn claim_ready_task(
        &self,
        workflow_id: &str,
        worker_id: &str,
        lease_duration_ms: i64,
    ) -> Result<Option<WorkflowTask>> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        let now = Utc::now().timestamp_millis();
        let lease_expires = now + lease_duration_ms;

        // Find highest priority, lowest wave ready or retry_pending task
        let task_id: Option<String> = tx
            .query_row(
                r#"
                SELECT id
                FROM workflow_tasks
                WHERE workflow_id = ?1
                  AND status IN ('ready', 'retry_pending')
                ORDER BY wave_index ASC, priority DESC, id ASC
                LIMIT 1
                "#,
                params![workflow_id],
                |row| row.get(0),
            )
            .ok();

        if let Some(ref id) = task_id {
            tx.execute(
                r#"
                UPDATE workflow_tasks
                SET status = 'claimed',
                    worker_id = ?1,
                    lease_expires_at_epoch_ms = ?2,
                    started_at_epoch_ms = COALESCE(started_at_epoch_ms, ?3)
                WHERE workflow_id = ?4 AND id = ?5
                "#,
                params![worker_id, lease_expires, now, workflow_id, id],
            )?;
        }

        tx.commit()?;

        if let Some(id) = task_id {
            drop(conn);
            self.get_task(workflow_id, &id)
        } else {
            Ok(None)
        }
    }

    /// Extends a worker's lease on an active task.
    pub fn renew_task_lease(
        &self,
        workflow_id: &str,
        task_id: &str,
        worker_id: &str,
        lease_duration_ms: i64,
    ) -> Result<bool> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().timestamp_millis();
        let lease_expires = now + lease_duration_ms;

        let rows = conn.execute(
            r#"
            UPDATE workflow_tasks
            SET lease_expires_at_epoch_ms = ?1
            WHERE workflow_id = ?2
              AND id = ?3
              AND worker_id = ?4
              AND status IN ('claimed', 'running', 'verifying')
            "#,
            params![lease_expires, workflow_id, task_id, worker_id],
        )?;

        Ok(rows > 0)
    }

    /// Discovers tasks whose lease expired while in `claimed` or `running`,
    /// recovering them to `retry_pending` if within retry budget or `failed` if exceeded.
    pub fn reconcile_stale_tasks(&self, workflow_id: &str, now_epoch_ms: i64) -> Result<usize> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;

        // Find stale tasks
        let mut stmt = tx.prepare(
            r#"
            SELECT id, retry_count, max_retries
            FROM workflow_tasks
            WHERE workflow_id = ?1
              AND status IN ('claimed', 'running', 'verifying')
              AND lease_expires_at_epoch_ms IS NOT NULL
              AND lease_expires_at_epoch_ms < ?2
            "#,
        )?;

        let stale: Vec<(String, u32, u32)> = stmt
            .query_map(params![workflow_id, now_epoch_ms], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })?
            .filter_map(|r| r.ok())
            .collect();
        drop(stmt);

        let count = stale.len();
        for (id, retries, max_retries) in stale {
            if retries < max_retries {
                tx.execute(
                    r#"
                    UPDATE workflow_tasks
                    SET status = 'retry_pending',
                        worker_id = NULL,
                        retry_count = retry_count + 1,
                        lease_expires_at_epoch_ms = NULL,
                        error_details = 'Worker lease expired; task rescheduled for retry'
                    WHERE workflow_id = ?1 AND id = ?2
                    "#,
                    params![workflow_id, id],
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
                    WHERE workflow_id = ?1 AND id = ?2
                    "#,
                    params![workflow_id, id, now_epoch_ms],
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
            started_at_epoch_ms: None,
            completed_at_epoch_ms: None,
            worker_id: None,
            lease_expires_at_epoch_ms: None,
            error_details: None,
            input_artifacts: vec![],
            output_artifacts: vec![],
            verification_result_json: None,
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
}
