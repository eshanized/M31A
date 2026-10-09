//! Differential DAG Replanning Engine with Task Completion Preservation (FLC-04, D-08).
//!
//! Triggered by `RecoveryAction::Replan` on invalidated assumptions, repeated failures,
//! architectural changes, or state drift. Preserves verified completed tasks (`TaskState::Completed`)
//! whose semantic fingerprints remain unchanged, retaining their success state without re-execution.
//! Cancels or supersedes invalidated tasks and applies validated differential DAG updates into
//! revision N+1 of the `TaskGraph`, recording the complete transaction in SQLite `recovery_attempts`.
//! Key capabilities:
//! - Accepts `ReplanningTrigger` in the request for provenance tracking.
//! - Classifies each replan as `Local`, `Partial`, or `Full` scope before execution.
//! - Exposes `classify_scope` as a pure function for pre-replan analysis.

use sqlx::SqlitePool;
use uuid::Uuid;

use crate::dag::graph::TaskGraph;
use crate::dag::reconciler::TaskGraphReconciler;
use crate::dag::validator::TaskGraphValidator;
use crate::error::M31AError;
use crate::events::envelope::EventEnvelope;
use crate::events::types::EventType;
use crate::ids::{MissionId, TaskGraphId, TaskId};
use crate::kernel::plan::{CandidatePlan, CandidateTask, CandidateTaskKey, ReplanningTrigger};
use crate::kernel::seams::recovery::FailureClassification;
use crate::persistence::sqlite::repositories::TaskGraphRepository;
use crate::persistence::sqlite::repositories::task_graph::SqliteTaskGraphRepository;
use std::sync::Arc;

/// Scope of a proposed plan revision — used for budget and risk classification (PLN-22).
///
/// Determined before execution from the size of the affected task set relative to
/// the total plan. Scope does NOT gate execution — it is metadata for audit and telemetry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReplanScope {
    /// Exactly one task is affected. All other tasks are preserved.
    Local,
    /// A contiguous subgraph of tasks is affected (more than one, less than all).
    Partial,
    /// All tasks in the plan are superseded or replaced. Full mission replanning.
    Full,
}

impl ReplanScope {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Partial => "partial",
            Self::Full => "full",
        }
    }
}

impl std::fmt::Display for ReplanScope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Classify the scope of a replan based on how many tasks are newly introduced
/// relative to the current graph task count. Pure — no side effects.
///
/// - `new_task_count`: tasks newly introduced in the candidate plan (not preserved).
/// - `old_task_count`: total tasks in the previous graph (0 means no history = Full).
///
/// Rules:
/// - If old_task_count == 0: always `Full` (no baseline to compare against).
/// - If new_task_count == 1: `Local`.
/// - If new_task_count >= old_task_count: `Full`.
/// - Otherwise: `Partial`.
pub fn classify_scope(new_task_count: usize, old_task_count: usize) -> ReplanScope {
    if old_task_count == 0 {
        return ReplanScope::Full;
    }
    match new_task_count {
        0 => ReplanScope::Local, // only preserved tasks, trivially local
        1 => ReplanScope::Local,
        n if n >= old_task_count => ReplanScope::Full,
        _ => ReplanScope::Partial,
    }
}

/// Request payload driving a differential replan transition (D-08, PLN-22).
#[derive(Debug, Clone)]
pub struct DifferentialReplanRequest {
    pub mission_id: MissionId,
    pub failed_task_id: Option<TaskId>,
    pub failure_class: FailureClassification,
    pub diagnosis_or_reason: String,
    pub candidate_plan: CandidatePlan,
    /// Why this replan was triggered. Used for provenance tracking and audit.
    pub trigger: Option<ReplanningTrigger>,
}

/// Outcome of a differential replan reconciliation (D-08, PLN-22).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DifferentialReplanOutcome {
    pub new_graph_id: TaskGraphId,
    pub revision: u32,
    pub preserved_tasks: Vec<TaskId>,
    pub new_tasks: Vec<TaskId>,
    pub superseded_tasks: Vec<TaskId>,
    pub cancelled_tasks: Vec<TaskId>,
    pub recovery_attempt_id: Uuid,
    /// Scope classification of this revision.
    pub scope: ReplanScope,
}

/// Differential DAG replanning engine (FLC-04, D-08, PLN-22).
#[derive(Debug, Clone)]
pub struct DifferentialReplanEngine {
    pool: SqlitePool,
    reconciler: TaskGraphReconciler,
    validator: TaskGraphValidator,
}

impl DifferentialReplanEngine {
    pub fn new(pool: SqlitePool) -> Self {
        Self {
            reconciler: TaskGraphReconciler::new(pool.clone()),
            validator: TaskGraphValidator::new(),
            pool,
        }
    }

    /// Reconciles an existing `TaskGraph` with a new `CandidatePlan`, preserving completed work.
    pub async fn execute_replan(
        &self,
        old_graph: &TaskGraph,
        req: DifferentialReplanRequest,
    ) -> Result<DifferentialReplanOutcome, M31AError> {
        // 1. Defensively validate candidate plan for acyclicity and constraints
        self.validator
            .validate_candidate_plan(&req.candidate_plan)
            .map_err(|e| {
                M31AError::validation(format!("Replan candidate validation failed: {}", e))
            })?;

        // 2. Classify scope before reconciliation (pure, no side effects)
        let old_task_count = old_graph.tasks.len();
        let new_plan_task_count = req.candidate_plan.tasks.len();

        // 3. Reconcile with task completion preservation via TaskGraphReconciler (D-08)
        let (new_graph, summary) = self
            .reconciler
            .reconcile_with_summary(req.mission_id, old_graph, &req.candidate_plan)
            .await?;

        // 4. Extract preserved and newly allocated tasks
        let preserved_tasks: Vec<TaskId> = summary.reused_tasks.iter().map(|(_, id)| *id).collect();
        let new_tasks: Vec<TaskId> = summary.new_tasks.iter().map(|(_, id)| *id).collect();
        let superseded_tasks = summary.superseded_tasks.clone();
        let cancelled_tasks = summary.running_tasks_to_cancel.clone();

        // 5. Determine scope from post-reconciliation evidence
        let scope = classify_scope(new_tasks.len(), old_task_count);

        // 6. Build action record with trigger provenance and scope
        let trigger_str = req
            .trigger
            .as_ref()
            .map(|t| format!(" trigger={}", t))
            .unwrap_or_default();

        let target_task_id = req.failed_task_id.unwrap_or_default();
        let action_taken = format!(
            "differential_replan[{}]: rev {} -> {}; preserved={}, new={}, superseded={}, total_plan_tasks={}{}",
            scope,
            old_graph.revision,
            new_graph.revision,
            preserved_tasks.len(),
            new_tasks.len(),
            superseded_tasks.len(),
            new_plan_task_count,
            trigger_str,
        );

        // 7. Durably record replan in recovery_attempts table (FLC-05)
        let record = crate::recovery::budget::RecoveryAttemptRecord {
            mission_id: req.mission_id,
            task_id: target_task_id,
            failure_class: req.failure_class,
            strategy: "differential_replan".to_string(),
            attempt_number: 1,
            budget_consumed: 1,
            remaining_class_budget: 0,
            remaining_overall_budget: 0,
            backoff_delay_ms: 0,
            action_taken,
            result: "reconciled".to_string(),
            mutation_fingerprint: None,
            semantic_signature: None,
        };
        let budget_tracker = crate::recovery::budget::RecoveryBudgetTracker::default();
        let attempt_id = budget_tracker
            .record_attempt(&self.pool, record)
            .await
            .map_err(|e| {
                M31AError::persistence(format!(
                    "Failed to record replan in recovery_attempts: {}",
                    e
                ))
            })?;

        Ok(DifferentialReplanOutcome {
            new_graph_id: new_graph.id,
            revision: new_graph.revision,
            preserved_tasks,
            new_tasks,
            superseded_tasks,
            cancelled_tasks,
            recovery_attempt_id: attempt_id,
            scope,
        })
    }
}

/// canonical replan authority coordinating strategy adaptation, task supersession,
/// task creation, dag reconciliation, revision increment, and replan events.
#[derive(Clone)]
pub struct ReplanAuthority {
    pool: SqlitePool,
    engine: DifferentialReplanEngine,
    event_bus: Option<Arc<dyn crate::events::EventBus>>,
}

impl ReplanAuthority {
    pub fn new(pool: SqlitePool, event_bus: Option<Arc<dyn crate::events::EventBus>>) -> Self {
        Self {
            engine: DifferentialReplanEngine::new(pool.clone()),
            pool,
            event_bus,
        }
    }

    /// authoritative execution of a differential replan request.
    pub async fn execute_replan(
        &self,
        req: DifferentialReplanRequest,
    ) -> Result<DifferentialReplanOutcome, M31AError> {
        let mission_id = req.mission_id;
        let graph_repo = SqliteTaskGraphRepository::new(self.pool.clone());
        let old_graph = graph_repo
            .get_active_graph(mission_id)
            .await?
            .ok_or_else(|| {
                M31AError::validation(format!(
                    "no active task graph found for mission {mission_id}"
                ))
            })?;

        let outcome = self.engine.execute_replan(&old_graph, req).await?;

        if let Some(ref bus) = self.event_bus {
            for sup_id in &outcome.superseded_tasks {
                let env = EventEnvelope::new(
                    0,
                    Some(mission_id),
                    None,
                    "replan_authority".to_string(),
                    EventType::TaskSuperseded {
                        task_id: *sup_id,
                        mission_id,
                        replacement_task_id: outcome.new_tasks.first().copied(),
                    },
                );
                let _ = bus.publish(env).await;
            }
            let env = EventEnvelope::new(
                0,
                Some(mission_id),
                None,
                "replan_authority".to_string(),
                EventType::TaskRevisionCreated {
                    session_id: format!("mission:{}", mission_id),
                    task_revision: outcome.revision,
                    plan_revision: outcome.revision,
                    author: "replan_authority".to_string(),
                },
            );
            let _ = bus.publish(env).await;
        }

        Ok(outcome)
    }

    /// authoritative execution of a candidate plan replan (recovery / controller loop).
    pub async fn execute_candidate_plan_replan(
        &self,
        mission_id: MissionId,
        failed_task_id: Option<TaskId>,
        reason: &str,
        candidate_plan: CandidatePlan,
        trigger: Option<ReplanningTrigger>,
    ) -> Result<DifferentialReplanOutcome, M31AError> {
        let req = DifferentialReplanRequest {
            mission_id,
            failed_task_id,
            failure_class: FailureClassification::Architecture,
            diagnosis_or_reason: reason.to_string(),
            candidate_plan,
            trigger,
        };
        self.execute_replan(req).await
    }

    /// authoritative execution of an agent_action::replan proposal
    pub async fn execute_replan_action(
        &self,
        mission_id: MissionId,
        reason: &str,
        tasks_to_supersede: &[TaskId],
        new_tasks: &[String],
    ) -> Result<DifferentialReplanOutcome, M31AError> {
        let graph_repo = SqliteTaskGraphRepository::new(self.pool.clone());
        let old_graph = graph_repo
            .get_active_graph(mission_id)
            .await?
            .ok_or_else(|| {
                M31AError::validation(format!(
                    "no active task graph found for mission {mission_id}"
                ))
            })?;

        let supersede_set: std::collections::HashSet<TaskId> =
            tasks_to_supersede.iter().copied().collect();
        let mut candidate_tasks = Vec::new();

        for (task_id, task) in &old_graph.tasks {
            if !supersede_set.contains(task_id) {
                let key = CandidateTaskKey::new(if task.candidate_key.is_empty() {
                    task.id.to_string()
                } else {
                    task.candidate_key.clone()
                });
                let mut ct = CandidateTask::new(
                    key,
                    &task.title,
                    task.role.clone(),
                    task.verification.clone(),
                    task.estimates.clone(),
                );
                ct.capabilities = task.capabilities.clone();
                ct.verification = task.verification.clone();
                ct.estimates = task.estimates.clone();
                ct.description = task.description.clone();
                ct.completion_criteria = task.completion_criteria.clone();
                ct.requirement_keys = task.requirement_keys.clone();
                ct.assumptions = task.assumptions.clone();
                ct.prompt_ref = task.prompt_ref.clone();

                for edge in &old_graph.edges {
                    if edge.dependent_id == *task_id
                        && !supersede_set.contains(&edge.prerequisite_id)
                    {
                        if let Some(dep_task) = old_graph.tasks.get(&edge.prerequisite_id) {
                            let dep_key =
                                CandidateTaskKey::new(if dep_task.candidate_key.is_empty() {
                                    dep_task.id.to_string()
                                } else {
                                    dep_task.candidate_key.clone()
                                });
                            ct.depends_on.push(dep_key);
                        }
                    }
                }
                candidate_tasks.push(ct);
            }
        }

        for (idx, task_desc) in new_tasks.iter().enumerate() {
            if task_desc.trim().is_empty() {
                continue;
            }
            let key = CandidateTaskKey::new(format!("replan-task-{}", idx + 1));
            let mut ct = CandidateTask::new(
                key,
                task_desc,
                crate::state_machine::agent::AgentRole::implementer(),
                crate::kernel::plan::VerificationStrategy::Compilation,
                crate::kernel::plan::ResourceEstimate::default(),
            );
            ct.description = Some(task_desc.clone());
            candidate_tasks.push(ct);
        }

        let candidate_plan = CandidatePlan {
            tasks: candidate_tasks,
            objective: reason.to_string(),
            ..Default::default()
        };

        let req = DifferentialReplanRequest {
            mission_id,
            failed_task_id: tasks_to_supersede.first().copied(),
            failure_class: FailureClassification::Architecture,
            diagnosis_or_reason: reason.to_string(),
            candidate_plan,
            trigger: Some(ReplanningTrigger::EvidenceDiscovery {
                unexpected_finding: reason.to_string(),
            }),
        };

        self.execute_replan(req).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_scope_no_baseline() {
        assert_eq!(classify_scope(3, 0), ReplanScope::Full);
    }

    #[test]
    fn test_classify_scope_zero_new() {
        assert_eq!(classify_scope(0, 5), ReplanScope::Local);
    }

    #[test]
    fn test_classify_scope_one_new() {
        assert_eq!(classify_scope(1, 5), ReplanScope::Local);
    }

    #[test]
    fn test_classify_scope_partial() {
        assert_eq!(classify_scope(3, 7), ReplanScope::Partial);
    }

    #[test]
    fn test_classify_scope_full_equal() {
        assert_eq!(classify_scope(5, 5), ReplanScope::Full);
    }

    #[test]
    fn test_classify_scope_full_exceeds() {
        assert_eq!(classify_scope(8, 5), ReplanScope::Full);
    }

    #[test]
    fn test_scope_display() {
        assert_eq!(ReplanScope::Local.to_string(), "local");
        assert_eq!(ReplanScope::Partial.to_string(), "partial");
        assert_eq!(ReplanScope::Full.to_string(), "full");
    }
}
