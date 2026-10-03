//! Task aggregate model

use crate::ids::{MissionId, TaskGraphId, TaskId};
pub use crate::kernel::plan::TaskResult;
use crate::kernel::plan::{CapabilityRequirement, ResourceEstimate, VerificationStrategy};
use crate::state_machine::TaskState;
use crate::state_machine::agent::AgentRole;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Detailed reason why a task is blocked in the scheduling lifecycle (D-05).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BlockingReason {
    PrerequisiteFailed {
        failed_task_id: TaskId,
        error_summary: String,
    },
    PolicyDenied {
        reason: String,
    },
    ResourceUnavailable {
        resource_key: String,
    },
}

/// Task aggregate containing authoritative task state.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Task {
    pub id: TaskId,
    pub mission_id: MissionId,
    pub task_graph_id: Option<TaskGraphId>,
    pub candidate_key: String,
    pub title: String,
    /// Target-specific work description. Flows from the
    /// candidate plan through materialization into worker context. `None`
    /// for legacy tasks; never fabricated by the runtime.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub role: AgentRole,
    pub status: TaskState,
    pub priority: u32,
    pub max_retries: u32,
    pub retry_count: u32,
    pub capabilities: Vec<CapabilityRequirement>,
    pub verification: VerificationStrategy,
    pub estimates: ResourceEstimate,
    /// Task-specific completion criteria. Rendered into
    /// worker context as the acceptance contract; empty when undeclared.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub completion_criteria: Vec<String>,
    /// Requirement keys this task satisfies. Traceability
    /// link from plan requirements to execution; used for memory recall.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requirement_keys: Vec<String>,
    /// Assumptions the task may rely on. Surfaced to the
    /// worker so model reasoning can challenge rather than assume silently.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub assumptions: Vec<String>,
    /// Typed prompt execution binding selected for this task.
    ///
    /// Flows from the workflow step through the candidate plan and
    /// materialization into worker context. `None` (legacy tasks) resolves
    /// through the role default at context compilation time. This MUST NOT
    /// be downgraded into [`Task::description`] text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt_ref: Option<crate::prompt::PromptReference>,
    pub blocking_reason: Option<BlockingReason>,
    pub fingerprint: String,
    pub result: Option<TaskResult>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
}

impl Task {
    pub fn new(id: TaskId, mission_id: MissionId, title: String) -> Self {
        let now = Utc::now();
        Self {
            id,
            mission_id,
            task_graph_id: None,
            candidate_key: String::new(),
            title,
            description: None,
            role: AgentRole::implementer(),
            status: TaskState::Pending,
            priority: 100,
            max_retries: 3,
            retry_count: 0,
            capabilities: Vec::new(),
            verification: VerificationStrategy::Compilation,
            estimates: ResourceEstimate::default(),
            completion_criteria: Vec::new(),
            requirement_keys: Vec::new(),
            assumptions: Vec::new(),
            prompt_ref: None,
            blocking_reason: None,
            fingerprint: String::new(),
            result: None,
            created_at: now,
            updated_at: now,
            started_at: None,
            completed_at: None,
        }
    }

    pub fn update_status(&mut self, new_status: TaskState) {
        self.status = new_status;
        self.updated_at = Utc::now();
    }

    pub fn can_retry(&self) -> bool {
        self.retry_count < self.max_retries
    }

    pub fn record_retry(&mut self) {
        self.retry_count = self.retry_count.saturating_add(1);
        self.updated_at = Utc::now();
    }

    pub fn set_blocked(&mut self, reason: BlockingReason) {
        self.status = TaskState::Blocked;
        self.blocking_reason = Some(reason);
        self.updated_at = Utc::now();
    }

    pub fn set_result(&mut self, result: TaskResult) {
        self.result = Some(result);
        self.updated_at = Utc::now();
    }

    pub fn mark_started(&mut self) {
        let now = Utc::now();
        self.status = TaskState::Running;
        self.started_at = Some(now);
        self.updated_at = now;
    }

    pub fn mark_completed(&mut self, result: TaskResult) {
        let now = Utc::now();
        self.status = TaskState::Succeeded;
        self.result = Some(result);
        self.completed_at = Some(now);
        self.updated_at = now;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::{ArtifactId, MissionId, TaskGraphId, TaskId};
    use crate::kernel::plan::VerificationStrategy;
    use crate::state_machine::agent::AgentRole;

    #[test]
    fn test_task_construction() {
        let id = TaskId::new();
        let mission_id = MissionId::new();
        let task = Task::new(id, mission_id, "Test task".to_string());
        assert_eq!(task.id, id);
        assert_eq!(task.mission_id, mission_id);
        assert_eq!(task.title, "Test task");
        assert_eq!(task.status, TaskState::Pending);
        assert_eq!(task.role, AgentRole::implementer());
        assert_eq!(task.priority, 100);
        assert_eq!(task.max_retries, 3);
        assert_eq!(task.retry_count, 0);
        assert!(task.can_retry());
        assert_eq!(task.task_graph_id, None);
        assert_eq!(task.blocking_reason, None);
    }

    #[test]
    fn test_task_status_update() {
        let id = TaskId::new();
        let mission_id = MissionId::new();
        let mut task = Task::new(id, mission_id, "Test".to_string());
        task.update_status(TaskState::Running);
        assert_eq!(task.status, TaskState::Running);
    }

    #[test]
    fn test_task_retry_lifecycle() {
        let id = TaskId::new();
        let mission_id = MissionId::new();
        let mut task = Task::new(id, mission_id, "Retry test".to_string());
        assert!(task.can_retry());
        task.record_retry();
        assert_eq!(task.retry_count, 1);
        task.record_retry();
        assert_eq!(task.retry_count, 2);
        task.record_retry();
        assert_eq!(task.retry_count, 3);
        assert!(!task.can_retry());
    }

    #[test]
    fn test_task_blocking_and_results() {
        let id = TaskId::new();
        let mission_id = MissionId::new();
        let mut task = Task::new(id, mission_id, "Block test".to_string());
        let failed_task = TaskId::new();
        task.set_blocked(BlockingReason::PrerequisiteFailed {
            failed_task_id: failed_task,
            error_summary: "Dependency failed".to_string(),
        });
        assert_eq!(task.status, TaskState::Blocked);
        assert_eq!(
            task.blocking_reason,
            Some(BlockingReason::PrerequisiteFailed {
                failed_task_id: failed_task,
                error_summary: "Dependency failed".to_string()
            })
        );

        let mut result = TaskResult::new("Completed with artifact");
        let art_id = ArtifactId::new();
        result.output_artifacts.push(art_id);
        result
            .metadata
            .insert("exit_code".to_string(), "0".to_string());
        task.set_result(result.clone());
        assert_eq!(task.result, Some(result));
    }

    #[test]
    fn test_task_serde_roundtrip() {
        let id = TaskId::new();
        let mission_id = MissionId::new();
        let graph_id = TaskGraphId::new();
        let mut task = Task::new(id, mission_id, "Test".to_string());
        task.task_graph_id = Some(graph_id);
        task.candidate_key = "step_1".to_string();
        task.fingerprint = "abc123sha".to_string();
        task.verification = VerificationStrategy::AutomatedTest {
            command: Some("cargo test".to_string()),
        };

        let json = serde_json::to_string(&task).unwrap();
        let parsed: Task = serde_json::from_str(&json).unwrap();
        assert_eq!(task.id, parsed.id);
        assert_eq!(task.mission_id, parsed.mission_id);
        assert_eq!(task.task_graph_id, parsed.task_graph_id);
        assert_eq!(task.candidate_key, parsed.candidate_key);
        assert_eq!(task.fingerprint, parsed.fingerprint);
        assert_eq!(task.title, parsed.title);
        assert_eq!(task.status, parsed.status);
        assert_eq!(task.role, parsed.role);
        assert_eq!(task.verification, parsed.verification);
    }
}
