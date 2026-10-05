use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::ids::{AgentId, MissionId, TaskGraphId, TaskId};
use crate::kernel::plan::{CandidatePlan, TaskResult, VerificationStrategy};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkItem {
    pub task_id: TaskId,
    pub title: String,
    pub estimated_tokens: u64,
    pub required_capabilities: Vec<String>,
    /// Target-specific work description. Populated by the
    /// scheduler from the durable task record; empty for legacy items.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Task-specific completion criteria carried to the worker.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub completion_criteria: Vec<String>,
    /// Requirement keys this work satisfies for end-to-end traceability.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requirement_keys: Vec<String>,
    /// Assumptions the work may rely on.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub assumptions: Vec<String>,
    /// Declared verification strategy. `None` for legacy items means "unknown":
    /// the runner applies the strict test-evidence gate by default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verification: Option<VerificationStrategy>,
    /// Typed prompt execution binding selected for this work item.
    ///
    /// Carried from the candidate plan through materialization into worker
    /// context. `None` (legacy items) resolves through the role default at
    /// context compilation time. This MUST NOT be downgraded into
    /// [`WorkItem::description`] text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt_ref: Option<crate::prompt::PromptReference>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReadyWorkResponse {
    pub ready_tasks: Vec<WorkItem>,
    pub blocked_tasks_count: usize,
}

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum SchedulerError {
    #[error("scheduling error: {0}")]
    Failed(String),
}

#[async_trait]
pub trait WorkScheduler: Send + Sync {
    async fn materialize_plan(
        &self,
        _mission_id: MissionId,
        _plan: &CandidatePlan,
    ) -> Result<TaskGraphId, SchedulerError> {
        Ok(TaskGraphId::new())
    }

    async fn reconcile_plan(
        &self,
        mission_id: MissionId,
        plan: &CandidatePlan,
    ) -> Result<TaskGraphId, SchedulerError> {
        self.materialize_plan(mission_id, plan).await
    }

    async fn find_ready_work(
        &self,
        mission_id: MissionId,
    ) -> Result<ReadyWorkResponse, SchedulerError>;

    async fn is_work_complete(&self, mission_id: MissionId) -> Result<bool, SchedulerError>;

    async fn mark_task_started(
        &self,
        task_id: TaskId,
        agent_id: AgentId,
    ) -> Result<(), SchedulerError>;

    async fn mark_task_completed(
        &self,
        _task_id: TaskId,
        _result: Option<TaskResult>,
    ) -> Result<(), SchedulerError> {
        Ok(())
    }

    async fn mark_task_failed(
        &self,
        _task_id: TaskId,
        _error_message: String,
        _is_recoverable: bool,
    ) -> Result<(), SchedulerError> {
        Ok(())
    }

    async fn mark_task_skipped(
        &self,
        _task_id: TaskId,
        _reason: String,
    ) -> Result<(), SchedulerError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_work_item_serde() {
        let item = WorkItem {
            task_id: TaskId::new(),
            title: "Task 1".to_string(),
            estimated_tokens: 1000,
            required_capabilities: vec!["rust".into()],
            description: Some("Migrate the parser".to_string()),
            completion_criteria: vec!["cargo test passes".to_string()],
            requirement_keys: vec!["REQ-FUNC-01".to_string()],
            assumptions: vec!["stable API".to_string()],
            verification: None,

            prompt_ref: None,
        };
        let serialized = serde_json::to_string(&item).unwrap();
        let deserialized: WorkItem = serde_json::from_str(&serialized).unwrap();
        assert_eq!(item, deserialized);
    }
}
