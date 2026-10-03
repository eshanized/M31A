//! Atomic transactional materialization of CandidatePlan into authoritative TaskGraph (D-13, D-16, DAG-01).

use crate::dag::graph::{DependencyEdge, TaskGraph};
use crate::dag::validator::TaskGraphValidator;
use crate::error::M31AError;
use crate::ids::{MissionId, TaskGraphId, TaskId};
use crate::kernel::plan::{CandidatePlan, CandidateTaskKey};
use crate::persistence::sqlite::repositories::{SqliteTaskGraphRepository, TaskGraphRepository};
use crate::planning::review::ExecutionAuthorization;
use crate::state::Task;
use crate::state_machine::TaskState;
use chrono::Utc;
use sha2::{Digest, Sha256};
use sqlx::{Row, SqlitePool};
use std::collections::{BTreeMap, BTreeSet};

/// Materializer that validates and atomically persists CandidatePlans as authoritative TaskGraphs in SQLite.
#[derive(Debug, Clone)]
pub struct TaskGraphMaterializer {
    pool: SqlitePool,
    validator: TaskGraphValidator,
}

/// Explicit materialization-lane ownership.
///
/// There is exactly one governed interactive lane and a small set of
/// explicitly classified non-interactive lanes. The lane is type-visible so
/// a governed caller cannot accidentally use the unauthenticated entrypoint:
///
/// - [`MaterializationLane::GovernedInteractive`]: the operator-governed
///   path (`interaction/runner.rs` ReadyToExecute arm,
///   `tui/runtime_bridge.rs`). MUST go through
///   [`TaskGraphMaterializer::materialize_authorized`] with exact
///   revision + content-hash binding. This lane is the ONLY path that may
///   produce a `TaskGraph` for interactive execution.
/// - [`MaterializationLane::HeadlessWorkflow`]: scheduler / workflow /
///   controller lanes (`scheduler/engine.rs::materialize_plan`,
///   `workflow/engine.rs`, `controller/mod.rs`) operating on
///   machine-generated plans outside the interactive authorization ritual.
///   Authorization is owned by the calling lane (workflow definition
///   acceptance, controller stage gating), never by the model.
/// - [`MaterializationLane::InternalTest`]: deterministic unit/integration
///   fixtures. Never reachable from production code.
///
/// No model path may reach any materializer to bypass authorization: the
/// model proposes candidate content; only the lanes above decide.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaterializationLane {
    GovernedInteractive,
    HeadlessWorkflow,
    InternalTest,
}

impl std::fmt::Display for MaterializationLane {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::GovernedInteractive => write!(f, "governed_interactive"),
            Self::HeadlessWorkflow => write!(f, "headless_workflow"),
            Self::InternalTest => write!(f, "internal_test"),
        }
    }
}

impl TaskGraphMaterializer {
    pub fn new(pool: SqlitePool) -> Self {
        Self {
            pool,
            validator: TaskGraphValidator::new(),
        }
    }

    /// Materializes a candidate plan but strictly requires an execution authorization
    /// that binds to the specified revisions.
    pub async fn materialize_authorized(
        &self,
        mission_id: MissionId,
        plan: &CandidatePlan,
        plan_revision: u32,
        task_revision: u32,
        authorization: &ExecutionAuthorization,
    ) -> Result<TaskGraph, M31AError> {
        if !authorization.is_valid_for(plan_revision, task_revision) {
            return Err(M31AError::validation(
                "Execution authorization is invalid for the given revisions",
            ));
        }

        if plan.revision != plan_revision {
            return Err(M31AError::validation(
                "Candidate plan revision does not match authorized plan revision",
            ));
        }

        // Exact content hash validation (INVARIANT C)
        if let Some(ref expected_plan_hash) = authorization.plan_content_hash {
            let actual_plan_hash =
                crate::planning::review::PlanRevision::compute_content_hash(plan);
            if expected_plan_hash != &actual_plan_hash {
                return Err(M31AError::validation(format!(
                    "Exact artifact integrity violation: authorized plan hash {} does not match supplied plan content hash {}",
                    expected_plan_hash, actual_plan_hash
                )));
            }
        }

        if let Some(ref expected_task_hash) = authorization.task_content_hash {
            let actual_task_hash =
                crate::planning::review::TaskRevision::compute_tasks_hash(&plan.tasks);
            if expected_task_hash != &actual_task_hash {
                return Err(M31AError::validation(format!(
                    "Exact artifact integrity violation: authorized tasks hash {} does not match supplied tasks content hash {}",
                    expected_task_hash, actual_task_hash
                )));
            }
        }

        self.materialize(mission_id, plan).await
    }

    /// Lane-tagged materialization entrypoint.
    ///
    /// The [`MaterializationLane::GovernedInteractive`] lane is REJECTED
    /// here by construction: governed interactive callers must use
    /// [`TaskGraphMaterializer::materialize_authorized`] so the exact
    /// authorization binding cannot be bypassed accidentally. Headless,
    /// workflow, and test lanes declare themselves explicitly.
    pub async fn materialize_in_lane(
        &self,
        mission_id: MissionId,
        plan: &CandidatePlan,
        lane: MaterializationLane,
    ) -> Result<TaskGraph, M31AError> {
        if lane == MaterializationLane::GovernedInteractive {
            return Err(M31AError::validation(
                "GovernedInteractive lane must use materialize_authorized with exact authorization binding; materialize_in_lane rejects it by construction",
            ));
        }
        self.materialize(mission_id, plan).await
    }

    /// Atomically materialize a candidate plan into an authoritative TaskGraph in SQLite.
    ///
    /// Lane contract: this unauthenticated entrypoint serves
    /// the [`MaterializationLane::HeadlessWorkflow`] and
    /// [`MaterializationLane::InternalTest`] lanes ONLY. Governed
    /// interactive callers (TUI / runner ReadyToExecute) MUST use
    /// [`TaskGraphMaterializer::materialize_authorized`]; see
    /// [`TaskGraphMaterializer::materialize_in_lane`], which rejects the
    /// governed lane by construction.
    ///
    /// 1. Re-validates the candidate plan DAG structure.
    /// 2. Checks idempotency: if an active graph already exists for this (mission_id, plan_id), returns it.
    /// 3. In an ACID SQLite transaction, allocates fresh TaskGraphId and TaskIds, and persists:
    ///    - task_graphs record
    ///    - tasks records with SHA-256 semantic fingerprints
    ///    - task_dependencies records
    /// 4. Rebuilds and returns the authoritative TaskGraph with derived in-memory cache indexes.
    pub async fn materialize(
        &self,
        mission_id: MissionId,
        plan: &CandidatePlan,
    ) -> Result<TaskGraph, M31AError> {
        // 1. Defensive DAG validation
        self.validator
            .validate_candidate_plan(plan)
            .map_err(|e| M31AError::validation(format!("DAG validation failed: {}", e)))?;

        // 2. Check idempotency: if active graph exists with identical plan_id, return it
        let repo = SqliteTaskGraphRepository::new(self.pool.clone());
        if let Some(existing) = repo
            .get_active_graph(mission_id)
            .await?
            .filter(|g| g.plan_id == plan.plan_id)
        {
            return Ok(existing);
        }

        // 3. Begin atomic transaction with immediate write lock to prevent SQLITE_BUSY_SNAPSHOT
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;

        // 4. Compute next revision number
        let max_rev_row =
            sqlx::query("SELECT COALESCE(MAX(revision), 0) FROM task_graphs WHERE mission_id = ?")
                .bind(mission_id.as_bytes().as_slice())
                .fetch_one(&mut *tx)
                .await?;
        let current_rev: i64 = max_rev_row.try_get(0)?;
        let revision = (current_rev as u32) + 1;

        // 5. Insert task_graphs record
        let graph_id = TaskGraphId::new();
        let now = Utc::now();
        let now_str = now.to_rfc3339();

        // Mark any prior active graph for this mission as superseded
        sqlx::query(
            "UPDATE task_graphs SET status = 'superseded', updated_at = ? WHERE mission_id = ? AND status = 'active'",
        )
        .bind(&now_str)
        .bind(mission_id.as_bytes().as_slice())
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            r#"
            INSERT INTO task_graphs (id, mission_id, revision, plan_id, status, created_at, updated_at)
            VALUES (?, ?, ?, ?, 'active', ?, ?)
            "#,
        )
        .bind(graph_id.as_bytes().as_slice())
        .bind(mission_id.as_bytes().as_slice())
        .bind(revision as i64)
        .bind(&plan.plan_id)
        .bind(&now_str)
        .bind(&now_str)
        .execute(&mut *tx)
        .await?;

        // 6. Allocate fresh TaskIds for each candidate key
        let mut candidate_to_id: BTreeMap<CandidateTaskKey, TaskId> = BTreeMap::new();
        for candidate in &plan.tasks {
            candidate_to_id.insert(candidate.id.clone(), TaskId::new());
        }

        // 7. Insert tasks records
        let mut tasks = BTreeMap::new();
        for candidate in &plan.tasks {
            let task_id = *candidate_to_id.get(&candidate.id).unwrap();

            // Compute SHA-256 semantic fingerprint per D-14
            let mut hasher = Sha256::new();
            hasher.update(candidate.objective.as_bytes());
            hasher.update(candidate.role.to_string().as_bytes());
            let verif_json = serde_json::to_string(&candidate.verification).unwrap_or_default();
            hasher.update(verif_json.as_bytes());
            let est_json = serde_json::to_string(&candidate.estimates).unwrap_or_default();
            hasher.update(est_json.as_bytes());
            // The typed prompt binding is authority state: two tasks that
            // differ only in their selected prompt must not share a fingerprint.
            if let Some(ref prompt_ref) = candidate.prompt_ref {
                hasher.update(prompt_ref.id.as_bytes());
                hasher.update(prompt_ref.version.to_be_bytes());
            }
            let fingerprint = format!("{:x}", hasher.finalize());

            let cap_json = serde_json::to_string(&candidate.capabilities)
                .map_err(|e| M31AError::validation(e.to_string()))?;

            let mut task = Task::new(task_id, mission_id, candidate.objective.clone());
            task.task_graph_id = Some(graph_id);
            task.candidate_key = candidate.id.to_string();
            task.description = candidate.description.clone();
            task.role = candidate.role.clone();
            task.capabilities = candidate.capabilities.clone();
            task.verification = candidate.verification.clone();
            task.estimates = candidate.estimates.clone();
            task.completion_criteria = candidate.completion_criteria.clone();
            task.requirement_keys = candidate.requirement_keys.clone();
            task.assumptions = candidate.assumptions.clone();
            // Typed prompt execution binding: the workflow-selected prompt
            // survives materialization into the durable task record and is
            // NEVER downgraded into description text.
            task.prompt_ref = candidate.prompt_ref.clone();
            task.fingerprint = fingerprint.clone();
            task.status = TaskState::Pending;

            let criteria_json =
                serde_json::to_string(&task.completion_criteria).unwrap_or_default();
            let req_keys_json = serde_json::to_string(&task.requirement_keys).unwrap_or_default();
            let assumptions_json = serde_json::to_string(&task.assumptions).unwrap_or_default();

            sqlx::query(
                r#"
                INSERT INTO tasks (
                    id, mission_id, task_graph_id, candidate_key, title, role, status,
                    priority, max_retries, retry_count, capabilities, verification,
                    estimates, blocking_reason, fingerprint, result, created_at, updated_at,
                    description, completion_criteria, requirement_keys, assumptions,
                    prompt_ref_id, prompt_ref_version
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL, ?, NULL, ?, ?, ?, ?, ?, ?, ?, ?)
                "#,
            )
            .bind(task_id.as_bytes().as_slice())
            .bind(mission_id.as_bytes().as_slice())
            .bind(graph_id.as_bytes().as_slice())
            .bind(candidate.id.as_str())
            .bind(&candidate.objective)
            .bind(candidate.role.to_string())
            .bind(TaskState::Pending.to_string())
            .bind(task.priority as i64)
            .bind(task.max_retries as i64)
            .bind(task.retry_count as i64)
            .bind(&cap_json)
            .bind(&verif_json)
            .bind(&est_json)
            .bind(&fingerprint)
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

            tasks.insert(task_id, task);
        }

        // 8. Insert task_dependencies records
        let mut edges = BTreeSet::new();
        for candidate in &plan.tasks {
            let dependent_id = *candidate_to_id.get(&candidate.id).unwrap();
            for prereq_key in &candidate.depends_on {
                let prereq_id = *candidate_to_id.get(prereq_key).unwrap();

                sqlx::query(
                    r#"
                    INSERT INTO task_dependencies (task_graph_id, prerequisite_task_id, dependent_task_id, kind)
                    VALUES (?, ?, ?, 'hard_prerequisite')
                    "#,
                )
                .bind(graph_id.as_bytes().as_slice())
                .bind(prereq_id.as_bytes().as_slice())
                .bind(dependent_id.as_bytes().as_slice())
                .execute(&mut *tx)
                .await?;

                edges.insert(DependencyEdge::hard(prereq_id, dependent_id));
            }
        }

        // 9. Commit atomic transaction
        tx.commit().await?;

        // 10. Construct TaskGraph aggregate with all derived in-memory caches
        let graph = TaskGraph::build_from_records(
            graph_id,
            mission_id,
            revision,
            plan.plan_id.clone(),
            "active".to_string(),
            tasks,
            edges,
            now,
            now,
        );

        Ok(graph)
    }
}
