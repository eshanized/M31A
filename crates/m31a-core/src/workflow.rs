//! Domain models, task DAG specifications, state machines, and lifecycle primitives
//! for autonomous workflow orchestration (`m31a-core::workflow`).

use anyhow::{Result, anyhow, bail};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

/// High-level lifecycle state of an autonomous workflow run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowState {
    Created,
    Analyzing,
    Planning,
    PlanReady,
    AwaitingApproval,
    Ready,
    Running,
    Verifying,
    IntegrationPending,
    Completed,
    Failed,
    Blocked,
    Cancelled,
}

impl WorkflowState {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Created => "created",
            Self::Analyzing => "analyzing",
            Self::Planning => "planning",
            Self::PlanReady => "plan_ready",
            Self::AwaitingApproval => "awaiting_approval",
            Self::Ready => "ready",
            Self::Running => "running",
            Self::Verifying => "verifying",
            Self::IntegrationPending => "integration_pending",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Blocked => "blocked",
            Self::Cancelled => "cancelled",
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "created" => Some(Self::Created),
            "analyzing" => Some(Self::Analyzing),
            "planning" => Some(Self::Planning),
            "plan_ready" => Some(Self::PlanReady),
            "awaiting_approval" => Some(Self::AwaitingApproval),
            "ready" => Some(Self::Ready),
            "running" => Some(Self::Running),
            "verifying" => Some(Self::Verifying),
            "integration_pending" => Some(Self::IntegrationPending),
            "completed" => Some(Self::Completed),
            "failed" => Some(Self::Failed),
            "blocked" => Some(Self::Blocked),
            "cancelled" => Some(Self::Cancelled),
            _ => None,
        }
    }

    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }

    /// Validates whether transitioning to `next` is structurally permitted.
    pub fn can_transition_to(&self, next: &WorkflowState) -> bool {
        if self == next {
            return true;
        }
        // Terminal states cannot transition to non-terminal states without explicit restart/resume
        if self.is_terminal() {
            return false;
        }
        match self {
            Self::Created => matches!(
                next,
                Self::Analyzing
                    | Self::Planning
                    | Self::Ready
                    | Self::Running
                    | Self::Failed
                    | Self::Cancelled
            ),
            Self::Analyzing => matches!(
                next,
                Self::Planning | Self::Failed | Self::Cancelled | Self::Blocked
            ),
            Self::Planning => matches!(
                next,
                Self::PlanReady | Self::Failed | Self::Cancelled | Self::Blocked
            ),
            Self::PlanReady => matches!(
                next,
                Self::AwaitingApproval
                    | Self::Ready
                    | Self::Running
                    | Self::Failed
                    | Self::Cancelled
            ),
            Self::AwaitingApproval => matches!(
                next,
                Self::Ready
                    | Self::Running
                    | Self::Blocked
                    | Self::Cancelled
                    | Self::Planning
                    | Self::Failed
            ),
            Self::Ready => matches!(
                next,
                Self::Running
                    | Self::AwaitingApproval
                    | Self::Blocked
                    | Self::Cancelled
                    | Self::Failed
            ),
            Self::Running => matches!(
                next,
                Self::Verifying
                    | Self::IntegrationPending
                    | Self::Completed
                    | Self::Failed
                    | Self::Blocked
                    | Self::Cancelled
                    | Self::Planning
            ),
            Self::Verifying => matches!(
                next,
                Self::IntegrationPending
                    | Self::Completed
                    | Self::Failed
                    | Self::Blocked
                    | Self::Cancelled
                    | Self::Running
                    | Self::Planning
            ),
            Self::IntegrationPending => matches!(
                next,
                Self::Running
                    | Self::Verifying
                    | Self::Completed
                    | Self::Failed
                    | Self::Blocked
                    | Self::Cancelled
            ),
            Self::Blocked => matches!(
                next,
                Self::Ready
                    | Self::Running
                    | Self::Planning
                    | Self::Failed
                    | Self::Cancelled
                    | Self::AwaitingApproval
            ),
            Self::Completed | Self::Failed | Self::Cancelled => false,
        }
    }
}

/// Lifecycle status for an individual task within a workflow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowTaskStatus {
    Pending,
    Ready,
    Claimed,
    Running,
    Verifying,
    Completed,
    RetryPending,
    Failed,
    Blocked,
    Cancelled,
    NeedsApproval,
}

impl WorkflowTaskStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Ready => "ready",
            Self::Claimed => "claimed",
            Self::Running => "running",
            Self::Verifying => "verifying",
            Self::Completed => "completed",
            Self::RetryPending => "retry_pending",
            Self::Failed => "failed",
            Self::Blocked => "blocked",
            Self::Cancelled => "cancelled",
            Self::NeedsApproval => "needs_approval",
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "pending" => Some(Self::Pending),
            "ready" => Some(Self::Ready),
            "claimed" => Some(Self::Claimed),
            "running" => Some(Self::Running),
            "verifying" => Some(Self::Verifying),
            "completed" => Some(Self::Completed),
            "retry_pending" => Some(Self::RetryPending),
            "failed" => Some(Self::Failed),
            "blocked" => Some(Self::Blocked),
            "cancelled" => Some(Self::Cancelled),
            "needs_approval" => Some(Self::NeedsApproval),
            _ => None,
        }
    }

    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }

    pub fn can_transition_to(&self, next: &WorkflowTaskStatus) -> bool {
        if self == next {
            return true;
        }
        if self.is_terminal() {
            return false;
        }
        match self {
            Self::Pending => matches!(
                next,
                Self::Ready | Self::Blocked | Self::Cancelled | Self::Failed
            ),
            Self::Ready => matches!(
                next,
                Self::Claimed | Self::Running | Self::Blocked | Self::Cancelled | Self::Failed
            ),
            Self::Claimed => matches!(
                next,
                Self::Running
                    | Self::Ready
                    | Self::RetryPending
                    | Self::Failed
                    | Self::Cancelled
                    | Self::NeedsApproval
            ),
            Self::Running => matches!(
                next,
                Self::Verifying
                    | Self::Completed
                    | Self::RetryPending
                    | Self::Failed
                    | Self::Blocked
                    | Self::Cancelled
                    | Self::NeedsApproval
            ),
            Self::Verifying => matches!(
                next,
                Self::Completed
                    | Self::RetryPending
                    | Self::Failed
                    | Self::Cancelled
                    | Self::NeedsApproval
            ),
            Self::RetryPending => matches!(
                next,
                Self::Ready | Self::Claimed | Self::Running | Self::Failed | Self::Cancelled
            ),
            Self::Blocked => matches!(
                next,
                Self::Pending | Self::Ready | Self::Failed | Self::Cancelled
            ),
            Self::NeedsApproval => matches!(
                next,
                Self::Ready | Self::Running | Self::Blocked | Self::Cancelled | Self::Failed
            ),
            Self::Completed | Self::Failed | Self::Cancelled => false,
        }
    }
}

/// Workspace isolation strategy for task execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceIsolationStrategy {
    #[default]
    Worktree,
    SharedWorkspace,
    ReadOnlyCopy,
}

impl WorkspaceIsolationStrategy {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Worktree => "worktree",
            Self::SharedWorkspace => "shared_workspace",
            Self::ReadOnlyCopy => "read_only_copy",
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "worktree" => Some(Self::Worktree),
            "shared_workspace" => Some(Self::SharedWorkspace),
            "read_only_copy" => Some(Self::ReadOnlyCopy),
            _ => None,
        }
    }
}

/// An individual verifiable task node in a workflow plan DAG.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowTask {
    pub id: String,
    pub workflow_id: String,
    pub plan_revision: u32,
    pub name: String,
    pub description: String,
    pub assigned_role: String,
    pub assigned_agent_id: Option<String>,
    pub priority: i32,
    pub wave_index: usize,
    pub dependencies: Vec<String>,
    pub acceptance_criteria: Vec<String>,
    pub required_verification: Vec<String>,
    pub declared_read_paths: Vec<PathBuf>,
    pub declared_write_paths: Vec<PathBuf>,
    pub workspace_isolation: WorkspaceIsolationStrategy,
    pub status: WorkflowTaskStatus,
    pub retry_count: u32,
    pub max_retries: u32,
    pub started_at_epoch_ms: Option<i64>,
    pub completed_at_epoch_ms: Option<i64>,
    pub worker_id: Option<String>,
    pub lease_expires_at_epoch_ms: Option<i64>,
    pub error_details: Option<String>,
    pub input_artifacts: Vec<String>,
    pub output_artifacts: Vec<String>,
    pub verification_result_json: Option<String>,
}

/// Validated directed acyclic graph (DAG) representing an execution plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowPlan {
    pub workflow_id: String,
    pub revision: u32,
    pub rationale: String,
    pub created_at_epoch_ms: i64,
    pub tasks: Vec<WorkflowTask>,
}

impl WorkflowPlan {
    /// Validates that the plan is structurally sound and satisfies all DAG constraints.
    pub fn validate(&self) -> Result<()> {
        if self.tasks.is_empty() {
            bail!("WorkflowPlan contains no tasks");
        }

        let mut seen_ids = HashSet::new();
        for task in &self.tasks {
            if task.id.trim().is_empty() {
                bail!("Task with empty ID found in plan");
            }
            if task.name.trim().is_empty() {
                bail!("Task '{}' has empty name", task.id);
            }
            if !seen_ids.insert(task.id.clone()) {
                bail!("Duplicate task ID '{}' detected in WorkflowPlan", task.id);
            }
        }

        // Dependency validity check
        for task in &self.tasks {
            for dep in &task.dependencies {
                if dep == &task.id {
                    bail!("Task '{}' has a self-dependency", task.id);
                }
                if !seen_ids.contains(dep) {
                    bail!("Task '{}' depends on unknown task ID '{}'", task.id, dep);
                }
            }
        }

        // Cycle detection via Kahn's algorithm / in-degree topological traversal
        let mut in_degrees: HashMap<String, usize> = HashMap::new();
        let mut adj_list: HashMap<String, Vec<String>> = HashMap::new();

        for task in &self.tasks {
            in_degrees.insert(task.id.clone(), task.dependencies.len());
            for dep in &task.dependencies {
                adj_list
                    .entry(dep.clone())
                    .or_default()
                    .push(task.id.clone());
            }
        }

        let mut ready_queue: Vec<String> = in_degrees
            .iter()
            .filter(|(_, deg)| **deg == 0)
            .map(|(id, _)| id.clone())
            .collect();

        let mut visited_count = 0;
        while let Some(curr) = ready_queue.pop() {
            visited_count += 1;
            if let Some(dependents) = adj_list.get(&curr) {
                for next in dependents {
                    if let Some(deg) = in_degrees.get_mut(next) {
                        *deg -= 1;
                        if *deg == 0 {
                            ready_queue.push(next.clone());
                        }
                    }
                }
            }
        }

        if visited_count != self.tasks.len() {
            bail!("Dependency cycle detected in WorkflowPlan task DAG");
        }

        Ok(())
    }

    /// Partitions the plan into sequential execution waves, resolving dependencies
    /// and separating tasks with conflicting write sets into separate waves.
    pub fn partition_into_waves(&self) -> Result<Vec<Vec<WorkflowTask>>> {
        self.validate()?;

        let mut task_map: HashMap<String, WorkflowTask> = HashMap::new();
        for t in &self.tasks {
            task_map.insert(t.id.clone(), t.clone());
        }

        // 1. Initial topological wave assignment: wave = max(dep_waves) + 1
        let mut wave_assignments: HashMap<String, usize> = HashMap::new();
        let task_ids: Vec<String> = task_map.keys().cloned().collect();

        for id in &task_ids {
            Self::compute_wave_index(id, &task_map, &mut wave_assignments, &mut HashSet::new())?;
        }

        // 2. Conflict isolation: tasks in the same wave that mutate identical write paths
        // and do not use isolated worktrees are separated into consecutive waves.
        let max_wave = *wave_assignments.values().max().unwrap_or(&1);
        for w in 1..=max_wave {
            let same_wave_ids: Vec<String> = wave_assignments
                .iter()
                .filter(|(_, wave)| **wave == w)
                .map(|(id, _)| id.clone())
                .collect();

            let mut touched_writes: HashSet<PathBuf> = HashSet::new();
            for id in same_wave_ids {
                if let Some(task) = task_map.get(&id)
                    && task.workspace_isolation == WorkspaceIsolationStrategy::SharedWorkspace
                {
                    let mut has_overlap = false;
                    for write_path in &task.declared_write_paths {
                        if touched_writes.contains(write_path) {
                            has_overlap = true;
                            break;
                        }
                    }

                    if has_overlap {
                        if let Some(curr_w) = wave_assignments.get_mut(&id) {
                            *curr_w += 1;
                        }
                    } else {
                        for write_path in &task.declared_write_paths {
                            touched_writes.insert(write_path.clone());
                        }
                    }
                }
            }
        }

        // 3. Bucket tasks by wave
        let mut wave_buckets: std::collections::BTreeMap<usize, Vec<WorkflowTask>> =
            std::collections::BTreeMap::new();
        for (id, wave) in wave_assignments {
            if let Some(mut task) = task_map.remove(&id) {
                task.wave_index = wave;
                wave_buckets.entry(wave).or_default().push(task);
            }
        }

        Ok(wave_buckets.into_values().collect())
    }

    fn compute_wave_index(
        task_id: &str,
        task_map: &HashMap<String, WorkflowTask>,
        wave_assignments: &mut HashMap<String, usize>,
        visiting: &mut HashSet<String>,
    ) -> Result<usize> {
        if let Some(&w) = wave_assignments.get(task_id) {
            return Ok(w);
        }

        if visiting.contains(task_id) {
            bail!("Cyclic dependency detected at task '{task_id}'");
        }

        visiting.insert(task_id.to_string());

        let task = task_map
            .get(task_id)
            .ok_or_else(|| anyhow!("Task '{task_id}' not found in task map"))?;

        let mut max_dep_wave = 0;
        for dep in &task.dependencies {
            let dep_wave = Self::compute_wave_index(dep, task_map, wave_assignments, visiting)?;
            max_dep_wave = max_dep_wave.max(dep_wave);
        }

        visiting.remove(task_id);

        let wave = (max_dep_wave + 1).max(task.wave_index);
        wave_assignments.insert(task_id.to_string(), wave);
        Ok(wave)
    }
}

/// Durable, top-level workflow run record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowRun {
    pub id: String,
    pub thread_id: Option<String>,
    pub workspace_root: PathBuf,
    pub objective: String,
    pub workflow_type: String,
    pub state: WorkflowState,
    pub plan_revision: u32,
    pub configuration_json: String,
    pub created_at_epoch_ms: i64,
    pub updated_at_epoch_ms: i64,
    pub started_at_epoch_ms: Option<i64>,
    pub completed_at_epoch_ms: Option<i64>,
    pub cancellation_reason: Option<String>,
    pub overall_result_json: Option<String>,
    pub resource_usage_json: Option<String>,
}

/// Event types emitted during workflow lifecycle for durable audit history.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowEventType {
    WorkflowCreated,
    PlanGenerated,
    PlanRevised,
    TaskScheduled,
    AgentAssigned,
    AgentSpawned,
    TaskStarted,
    TaskProgressUpdated,
    ApprovalRequested,
    ApprovalDecisionRecorded,
    VerificationStarted,
    VerificationCompleted,
    TaskFailed,
    TaskRetried,
    WorkflowResumed,
    WorkflowCancelled,
    CandidateIntegrated,
    WorkflowCompleted,
}

impl WorkflowEventType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::WorkflowCreated => "workflow_created",
            Self::PlanGenerated => "plan_generated",
            Self::PlanRevised => "plan_revised",
            Self::TaskScheduled => "task_scheduled",
            Self::AgentAssigned => "agent_assigned",
            Self::AgentSpawned => "agent_spawned",
            Self::TaskStarted => "task_started",
            Self::TaskProgressUpdated => "task_progress_updated",
            Self::ApprovalRequested => "approval_requested",
            Self::ApprovalDecisionRecorded => "approval_decision_recorded",
            Self::VerificationStarted => "verification_started",
            Self::VerificationCompleted => "verification_completed",
            Self::TaskFailed => "task_failed",
            Self::TaskRetried => "task_retried",
            Self::WorkflowResumed => "workflow_resumed",
            Self::WorkflowCancelled => "workflow_cancelled",
            Self::CandidateIntegrated => "candidate_integrated",
            Self::WorkflowCompleted => "workflow_completed",
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "workflow_created" => Some(Self::WorkflowCreated),
            "plan_generated" => Some(Self::PlanGenerated),
            "plan_revised" => Some(Self::PlanRevised),
            "task_scheduled" => Some(Self::TaskScheduled),
            "agent_assigned" => Some(Self::AgentAssigned),
            "agent_spawned" => Some(Self::AgentSpawned),
            "task_started" => Some(Self::TaskStarted),
            "task_progress_updated" => Some(Self::TaskProgressUpdated),
            "approval_requested" => Some(Self::ApprovalRequested),
            "approval_decision_recorded" => Some(Self::ApprovalDecisionRecorded),
            "verification_started" => Some(Self::VerificationStarted),
            "verification_completed" => Some(Self::VerificationCompleted),
            "task_failed" => Some(Self::TaskFailed),
            "task_retried" => Some(Self::TaskRetried),
            "workflow_resumed" => Some(Self::WorkflowResumed),
            "workflow_cancelled" => Some(Self::WorkflowCancelled),
            "candidate_integrated" => Some(Self::CandidateIntegrated),
            "workflow_completed" => Some(Self::WorkflowCompleted),
            _ => None,
        }
    }
}

/// Durable append-only event record for auditability.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowEvent {
    pub id: String,
    pub workflow_id: String,
    pub task_id: Option<String>,
    pub agent_id: Option<String>,
    pub event_type: WorkflowEventType,
    pub timestamp_epoch_ms: i64,
    pub correlation_id: Option<String>,
    pub payload_json: String,
    pub schema_version: u32,
}

/// Content-addressed immutable artifact tracking.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowArtifact {
    pub id: String,
    pub workflow_id: String,
    pub task_id: Option<String>,
    pub name: String,
    pub artifact_type: String,
    pub file_path: PathBuf,
    pub content_sha256: String,
    pub size_bytes: u64,
    pub mime_type: String,
    pub created_at_epoch_ms: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_dag_partitioning() {
        let t1 = WorkflowTask {
            id: "t1".into(),
            workflow_id: "wf-1".into(),
            plan_revision: 1,
            name: "Architecture Analysis".into(),
            description: "Analyze code".into(),
            assigned_role: "Architect".into(),
            assigned_agent_id: None,
            priority: 10,
            wave_index: 0,
            dependencies: vec![],
            acceptance_criteria: vec![],
            required_verification: vec![],
            declared_read_paths: vec![],
            declared_write_paths: vec![],
            workspace_isolation: WorkspaceIsolationStrategy::Worktree,
            status: WorkflowTaskStatus::Pending,
            retry_count: 0,
            max_retries: 3,
            started_at_epoch_ms: None,
            completed_at_epoch_ms: None,
            worker_id: None,
            lease_expires_at_epoch_ms: None,
            error_details: None,
            input_artifacts: vec![],
            output_artifacts: vec![],
            verification_result_json: None,
        };

        let t2 = WorkflowTask {
            id: "t2".into(),
            workflow_id: "wf-1".into(),
            plan_revision: 1,
            name: "Implementation".into(),
            description: "Write code".into(),
            assigned_role: "Implementer".into(),
            assigned_agent_id: None,
            priority: 10,
            wave_index: 0,
            dependencies: vec!["t1".into()],
            acceptance_criteria: vec![],
            required_verification: vec![],
            declared_read_paths: vec![],
            declared_write_paths: vec![],
            workspace_isolation: WorkspaceIsolationStrategy::Worktree,
            status: WorkflowTaskStatus::Pending,
            retry_count: 0,
            max_retries: 3,
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
            workflow_id: "wf-1".into(),
            revision: 1,
            rationale: "Sequential two-stage plan".into(),
            created_at_epoch_ms: 1000,
            tasks: vec![t1, t2],
        };

        assert!(plan.validate().is_ok());
        let waves = plan.partition_into_waves().unwrap();
        assert_eq!(waves.len(), 2);
        assert_eq!(waves[0][0].id, "t1");
        assert_eq!(waves[1][0].id, "t2");
    }

    #[test]
    fn test_dag_cycle_detection() {
        let t1 = WorkflowTask {
            id: "t1".into(),
            workflow_id: "wf-1".into(),
            plan_revision: 1,
            name: "Task 1".into(),
            description: "Desc".into(),
            assigned_role: "Role".into(),
            assigned_agent_id: None,
            priority: 0,
            wave_index: 0,
            dependencies: vec!["t2".into()],
            acceptance_criteria: vec![],
            required_verification: vec![],
            declared_read_paths: vec![],
            declared_write_paths: vec![],
            workspace_isolation: WorkspaceIsolationStrategy::Worktree,
            status: WorkflowTaskStatus::Pending,
            retry_count: 0,
            max_retries: 3,
            started_at_epoch_ms: None,
            completed_at_epoch_ms: None,
            worker_id: None,
            lease_expires_at_epoch_ms: None,
            error_details: None,
            input_artifacts: vec![],
            output_artifacts: vec![],
            verification_result_json: None,
        };

        let t2 = WorkflowTask {
            id: "t2".into(),
            workflow_id: "wf-1".into(),
            plan_revision: 1,
            name: "Task 2".into(),
            description: "Desc".into(),
            assigned_role: "Role".into(),
            assigned_agent_id: None,
            priority: 0,
            wave_index: 0,
            dependencies: vec!["t1".into()],
            acceptance_criteria: vec![],
            required_verification: vec![],
            declared_read_paths: vec![],
            declared_write_paths: vec![],
            workspace_isolation: WorkspaceIsolationStrategy::Worktree,
            status: WorkflowTaskStatus::Pending,
            retry_count: 0,
            max_retries: 3,
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
            workflow_id: "wf-1".into(),
            revision: 1,
            rationale: "Cyclic plan".into(),
            created_at_epoch_ms: 1000,
            tasks: vec![t1, t2],
        };

        assert!(plan.validate().is_err());
    }

    #[test]
    fn test_workflow_state_transitions() {
        assert!(WorkflowState::Created.can_transition_to(&WorkflowState::Analyzing));
        assert!(WorkflowState::Planning.can_transition_to(&WorkflowState::PlanReady));
        assert!(WorkflowState::Ready.can_transition_to(&WorkflowState::Running));
        assert!(WorkflowState::Running.can_transition_to(&WorkflowState::Completed));
        // Completed cannot transition
        assert!(!WorkflowState::Completed.can_transition_to(&WorkflowState::Running));
    }
}
