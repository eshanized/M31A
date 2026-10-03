use chrono::Utc;
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use std::collections::{BTreeMap, BTreeSet};

use crate::dag::graph::{DependencyEdge, DependencyKind, TaskGraph};
use crate::dag::validator::TaskGraphValidator;
use crate::error::M31AError;
use crate::ids::{MissionId, TaskGraphId, TaskId};
use crate::kernel::plan::{CandidatePlan, CandidateTask, CandidateTaskKey};
use crate::state::Task;
use crate::state_machine::TaskState;

/// Compute deterministic SHA-256 semantic task specification fingerprint (D-14).
pub fn compute_task_fingerprint(task: &CandidateTask) -> String {
    let mut hasher = Sha256::new();
    hasher.update(task.objective.as_bytes());
    hasher.update(task.role.to_string().as_bytes());
    let verif_json = serde_json::to_string(&task.verification).unwrap_or_default();
    hasher.update(verif_json.as_bytes());
    let est_json = serde_json::to_string(&task.estimates).unwrap_or_default();
    hasher.update(est_json.as_bytes());
    // The typed prompt binding is authority state and MUST participate in
    // identity exactly as in the materializer: tasks differing only in
    // their selected prompt are distinct, and identical bindings reuse.
    // (Must stay in lockstep with `TaskGraphMaterializer::materialize`.)
    if let Some(ref prompt_ref) = task.prompt_ref {
        hasher.update(prompt_ref.id.as_bytes());
        hasher.update(prompt_ref.version.to_be_bytes());
    }
    format!("{:x}", hasher.finalize())
}

/// Summary of mid-mission replan reconciliation outcomes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReconciliationResult {
    pub reused_tasks: Vec<(CandidateTaskKey, TaskId)>,
    pub new_tasks: Vec<(CandidateTaskKey, TaskId)>,
    pub superseded_tasks: Vec<TaskId>,
    pub running_tasks_to_cancel: Vec<TaskId>,
}

/// Versioned reconciler for mid-mission replans producing immutable revision N+1 (D-14).
#[derive(Debug, Clone)]
pub struct TaskGraphReconciler {
    pool: SqlitePool,
    validator: TaskGraphValidator,
}

impl TaskGraphReconciler {
    pub fn new(pool: SqlitePool) -> Self {
        Self {
            pool,
            validator: TaskGraphValidator::new(),
        }
    }

    /// Reconcile an existing TaskGraph with a new CandidatePlan, generating revision N+1.
    pub async fn reconcile(
        &self,
        mission_id: MissionId,
        old_graph: &TaskGraph,
        new_plan: &CandidatePlan,
    ) -> Result<TaskGraph, M31AError> {
        let (graph, _summary) = self
            .reconcile_with_summary(mission_id, old_graph, new_plan)
            .await?;
        Ok(graph)
    }

    /// Reconcile an existing TaskGraph with a new CandidatePlan and return detailed reconciliation summary.
    pub async fn reconcile_with_summary(
        &self,
        mission_id: MissionId,
        old_graph: &TaskGraph,
        new_plan: &CandidatePlan,
    ) -> Result<(TaskGraph, ReconciliationResult), M31AError> {
        // 1. Defensive DAG re-validation on proposed plan
        self.validator
            .validate_candidate_plan(new_plan)
            .map_err(|e| M31AError::validation(format!("Replan DAG validation failed: {}", e)))?;

        let mut reused_tasks = Vec::new();
        let mut new_tasks = Vec::new();
        let mut superseded_tasks = Vec::new();
        let mut running_tasks_to_cancel = Vec::new();

        let mut candidate_to_id: BTreeMap<CandidateTaskKey, TaskId> = BTreeMap::new();
        let mut tasks_for_new_graph: BTreeMap<TaskId, Task> = BTreeMap::new();

        // 2. Classify tasks in proposed plan
        for candidate in &new_plan.tasks {
            let fingerprint = compute_task_fingerprint(candidate);
            let maybe_old_id = old_graph.candidate_to_task.get(&candidate.id);
            let maybe_old_task = maybe_old_id.and_then(|id| old_graph.tasks.get(id));

            if let Some(old_task) = maybe_old_task {
                if old_task.fingerprint == fingerprint {
                    // Exact fingerprint match: reuse task
                    reused_tasks.push((candidate.id.clone(), old_task.id));
                    candidate_to_id.insert(candidate.id.clone(), old_task.id);
                    let mut task_to_reuse = old_task.clone();
                    // Refresh non-identity context from the candidate so a
                    // re-resolved plan can never leave stale descriptions,
                    // criteria, or prompt bindings behind (fingerprint
                    // covers identity; these travel with it).
                    task_to_reuse.description = candidate.description.clone();
                    task_to_reuse.completion_criteria = candidate.completion_criteria.clone();
                    task_to_reuse.requirement_keys = candidate.requirement_keys.clone();
                    task_to_reuse.assumptions = candidate.assumptions.clone();
                    task_to_reuse.prompt_ref = candidate.prompt_ref.clone();
                    if task_to_reuse.status == TaskState::Failed {
                        task_to_reuse.status = TaskState::Pending;
                        task_to_reuse.result = None;
                        task_to_reuse.blocking_reason = None;
                    }
                    tasks_for_new_graph.insert(old_task.id, task_to_reuse);
                    continue;
                } else {
                    // Fingerprint changed materially: supersede old task
                    superseded_tasks.push(old_task.id);
                    if old_task.status == TaskState::Running {
                        running_tasks_to_cancel.push(old_task.id);
                    }
                }
            }

            // Fresh task allocation
            let fresh_id = TaskId::new();
            new_tasks.push((candidate.id.clone(), fresh_id));
            candidate_to_id.insert(candidate.id.clone(), fresh_id);

            let mut fresh_task = Task::new(fresh_id, mission_id, candidate.objective.clone());
            fresh_task.candidate_key = candidate.id.to_string();
            fresh_task.role = candidate.role.clone();
            fresh_task.capabilities = candidate.capabilities.clone();
            fresh_task.verification = candidate.verification.clone();
            fresh_task.estimates = candidate.estimates.clone();
            // Rich context + typed prompt binding survive reconciliation
            // exactly as in initial materialization (never downgraded,
            // never dropped on replan/resume).
            fresh_task.description = candidate.description.clone();
            fresh_task.completion_criteria = candidate.completion_criteria.clone();
            fresh_task.requirement_keys = candidate.requirement_keys.clone();
            fresh_task.assumptions = candidate.assumptions.clone();
            fresh_task.prompt_ref = candidate.prompt_ref.clone();
            fresh_task.fingerprint = fingerprint;

            tasks_for_new_graph.insert(fresh_id, fresh_task);
        }

        // 3. Identify old tasks completely removed from new plan
        for (old_key, old_id) in &old_graph.candidate_to_task {
            if !new_plan.tasks.iter().any(|c| c.id == *old_key) {
                superseded_tasks.push(*old_id);
                if old_graph
                    .tasks
                    .get(old_id)
                    .is_some_and(|t| t.status == TaskState::Running)
                {
                    running_tasks_to_cancel.push(*old_id);
                }
            }
        }

        // 4. ACID SQLite transaction
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let now = Utc::now();
        let now_str = now.to_rfc3339();

        let new_graph_id = TaskGraphId::new();
        let new_revision = old_graph.revision + 1;

        // Mark old graph as superseded
        sqlx::query("UPDATE task_graphs SET status = 'superseded', updated_at = ? WHERE id = ?")
            .bind(&now_str)
            .bind(old_graph.id.as_bytes().as_slice())
            .execute(&mut *tx)
            .await?;

        // Insert new task_graphs row
        sqlx::query(
            r#"
            INSERT INTO task_graphs (id, mission_id, revision, plan_id, status, created_at, updated_at)
            VALUES (?, ?, ?, ?, 'active', ?, ?)
            "#,
        )
        .bind(new_graph_id.as_bytes().as_slice())
        .bind(mission_id.as_bytes().as_slice())
        .bind(new_revision as i64)
        .bind(&new_plan.plan_id)
        .bind(&now_str)
        .bind(&now_str)
        .execute(&mut *tx)
        .await?;

        // Update superseded non-terminal tasks to skipped
        for &sup_id in &superseded_tasks {
            sqlx::query(
                r#"
                UPDATE tasks
                SET status = 'skipped',
                    blocking_reason = '{"type":"superseded"}',
                    updated_at = ?
                WHERE id = ? AND status IN ('pending', 'ready', 'blocked')
                "#,
            )
            .bind(&now_str)
            .bind(sup_id.as_bytes().as_slice())
            .execute(&mut *tx)
            .await?;
        }

        // Update reused tasks to point to new graph
        for (_key, reused_id) in &reused_tasks {
            let t = tasks_for_new_graph.get_mut(reused_id).unwrap();
            t.task_graph_id = Some(new_graph_id);
            t.updated_at = now;
            // Persist refreshed context + prompt binding (see reuse above).
            let criteria_json = serde_json::to_string(&t.completion_criteria).unwrap_or_default();
            let req_keys_json = serde_json::to_string(&t.requirement_keys).unwrap_or_default();
            let assumptions_json = serde_json::to_string(&t.assumptions).unwrap_or_default();
            let prompt_ref_id = t.prompt_ref.as_ref().map(|r| r.id.clone());
            let prompt_ref_version = t.prompt_ref.as_ref().map(|r| r.version as i64);

            if t.status == TaskState::Pending {
                sqlx::query(
                    "UPDATE tasks SET task_graph_id = ?, status = 'pending', blocking_reason = NULL, result = NULL, updated_at = ?, description = ?, completion_criteria = ?, requirement_keys = ?, assumptions = ?, prompt_ref_id = ?, prompt_ref_version = ? WHERE id = ?",
                )
                .bind(new_graph_id.as_bytes().as_slice())
                .bind(&now_str)
                .bind(t.description.as_deref())
                .bind(&criteria_json)
                .bind(&req_keys_json)
                .bind(&assumptions_json)
                .bind(prompt_ref_id)
                .bind(prompt_ref_version)
                .bind(reused_id.as_bytes().as_slice())
                .execute(&mut *tx)
                .await?;
            } else {
                sqlx::query("UPDATE tasks SET task_graph_id = ?, updated_at = ?, description = ?, completion_criteria = ?, requirement_keys = ?, assumptions = ?, prompt_ref_id = ?, prompt_ref_version = ? WHERE id = ?")
                    .bind(new_graph_id.as_bytes().as_slice())
                    .bind(&now_str)
                    .bind(t.description.as_deref())
                    .bind(&criteria_json)
                    .bind(&req_keys_json)
                    .bind(&assumptions_json)
                    .bind(prompt_ref_id)
                    .bind(prompt_ref_version)
                    .bind(reused_id.as_bytes().as_slice())
                    .execute(&mut *tx)
                    .await?;
            }
        }

        // Insert newly allocated tasks
        for (_key, fresh_id) in &new_tasks {
            let task = tasks_for_new_graph.get_mut(fresh_id).unwrap();
            task.task_graph_id = Some(new_graph_id);

            let cap_json = serde_json::to_string(&task.capabilities)
                .map_err(|e| M31AError::validation(e.to_string()))?;
            let verif_json = serde_json::to_string(&task.verification)
                .map_err(|e| M31AError::validation(e.to_string()))?;
            let est_json = serde_json::to_string(&task.estimates)
                .map_err(|e| M31AError::validation(e.to_string()))?;
            let criteria_json =
                serde_json::to_string(&task.completion_criteria).unwrap_or_default();
            let req_keys_json = serde_json::to_string(&task.requirement_keys).unwrap_or_default();
            let assumptions_json = serde_json::to_string(&task.assumptions).unwrap_or_default();

            sqlx::query(
                r#"
                INSERT INTO tasks (
                    id, mission_id, task_graph_id, candidate_key, title, role, status,
                    priority, max_retries, retry_count, capabilities, verification,
                    estimates, required_resources, blocking_reason, fingerprint, result,
                    created_at, updated_at, started_at, completed_at,
                    description, completion_criteria, requirement_keys, assumptions,
                    prompt_ref_id, prompt_ref_version
                ) VALUES (
                    ?, ?, ?, ?, ?, ?, ?,
                    ?, ?, ?, ?, ?,
                    ?, '[]', NULL, ?, NULL,
                    ?, ?, NULL, NULL,
                    ?, ?, ?, ?,
                    ?, ?
                )
                "#,
            )
            .bind(task.id.as_bytes().as_slice())
            .bind(mission_id.as_bytes().as_slice())
            .bind(new_graph_id.as_bytes().as_slice())
            .bind(&task.candidate_key)
            .bind(&task.title)
            .bind(task.role.to_string())
            .bind(task.status.to_string())
            .bind(task.priority as i64)
            .bind(task.max_retries as i64)
            .bind(task.retry_count as i64)
            .bind(&cap_json)
            .bind(&verif_json)
            .bind(&est_json)
            .bind(&task.fingerprint)
            .bind(&now_str)
            .bind(&now_str)
            .bind(task.description.as_deref())
            .bind(&criteria_json)
            .bind(&req_keys_json)
            .bind(&assumptions_json)
            .bind(task.prompt_ref.as_ref().map(|r| r.id.clone()))
            .bind(task.prompt_ref.as_ref().map(|r| r.version as i64))
            .execute(&mut *tx)
            .await?;
        }

        // Insert new dependencies
        let mut edges = BTreeSet::new();
        for candidate in &new_plan.tasks {
            let dep_task_id = *candidate_to_id.get(&candidate.id).unwrap();
            for prereq_key in &candidate.depends_on {
                let prereq_task_id = *candidate_to_id.get(prereq_key).unwrap();

                sqlx::query(
                    r#"
                    INSERT INTO task_dependencies (task_graph_id, prerequisite_task_id, dependent_task_id, kind)
                    VALUES (?, ?, ?, 'hard_prerequisite')
                    "#,
                )
                .bind(new_graph_id.as_bytes().as_slice())
                .bind(prereq_task_id.as_bytes().as_slice())
                .bind(dep_task_id.as_bytes().as_slice())
                .execute(&mut *tx)
                .await?;

                edges.insert(DependencyEdge {
                    prerequisite_id: prereq_task_id,
                    dependent_id: dep_task_id,
                    kind: DependencyKind::HardPrerequisite,
                });
            }
        }

        tx.commit().await?;

        // 5. Construct authoritative TaskGraph aggregate
        let new_graph = TaskGraph::build_from_records(
            new_graph_id,
            mission_id,
            new_revision,
            new_plan.plan_id.clone(),
            "active".to_string(),
            tasks_for_new_graph,
            edges,
            now,
            now,
        );

        let summary = ReconciliationResult {
            reused_tasks,
            new_tasks,
            superseded_tasks,
            running_tasks_to_cancel,
        };

        Ok((new_graph, summary))
    }
}
