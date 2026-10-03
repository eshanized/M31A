use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use std::path::PathBuf;

use crate::ids::{AgentId, JobId, MissionId, TaskId};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkExecutionRequest {
    pub mission_id: MissionId,
    pub task_id: TaskId,
    pub agent_id: AgentId,
    pub context_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mission_objective: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_objective: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_root: Option<PathBuf>,
    /// Target-specific work description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_description: Option<String>,
    /// Task-specific completion criteria.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub task_criteria: Vec<String>,
    /// Requirement keys this work satisfies.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requirement_keys: Vec<String>,
    /// Assumptions the work may rely on.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub task_assumptions: Vec<String>,
    /// Upstream project charter markdown.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upstream_charter: Option<String>,
    /// Upstream target architecture markdown.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upstream_architecture: Option<String>,
    /// Upstream requirement statements.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub upstream_requirements: Vec<String>,
    /// Upstream assumption statements.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub upstream_assumptions: Vec<String>,
    /// Upstream decision records.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub upstream_decisions: Vec<String>,
    /// Upstream research summary markdown.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upstream_research_summary: Option<String>,
    /// Declared verification strategy. `None`
    /// preserves the legacy strict test-evidence gate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verification: Option<crate::kernel::plan::VerificationStrategy>,
    /// Typed prompt execution binding selected for this work.
    ///
    /// Carried from the durable task record into the worker. `None`
    /// (legacy requests) resolves through the role default at context
    /// compilation time. The worker MUST NOT silently substitute the role
    /// default when this binding is present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt_ref: Option<crate::prompt::PromptReference>,
}

impl WorkExecutionRequest {
    pub fn new(
        mission_id: MissionId,
        task_id: TaskId,
        agent_id: AgentId,
        context_id: impl Into<String>,
    ) -> Self {
        Self {
            mission_id,
            task_id,
            agent_id,
            context_id: context_id.into(),
            mission_objective: None,
            task_objective: None,
            workspace_root: None,
            task_description: None,
            task_criteria: Vec::new(),
            requirement_keys: Vec::new(),
            task_assumptions: Vec::new(),
            upstream_charter: None,
            upstream_architecture: None,
            upstream_requirements: Vec::new(),
            upstream_assumptions: Vec::new(),
            upstream_decisions: Vec::new(),
            upstream_research_summary: None,
            verification: None,
            prompt_ref: None,
        }
    }

    pub fn with_objectives(
        mut self,
        mission_obj: Option<String>,
        task_obj: Option<String>,
    ) -> Self {
        self.mission_objective = mission_obj;
        self.task_objective = task_obj;
        self
    }

    pub fn with_mission_objective(mut self, obj: impl Into<String>) -> Self {
        self.mission_objective = Some(obj.into());
        self
    }

    pub fn with_task_objective(mut self, obj: impl Into<String>) -> Self {
        self.task_objective = Some(obj.into());
        self
    }

    pub fn with_workspace_root(mut self, root: impl Into<PathBuf>) -> Self {
        self.workspace_root = Some(root.into());
        self
    }

    pub fn with_task_description(mut self, desc: impl Into<String>) -> Self {
        self.task_description = Some(desc.into());
        self
    }

    pub fn with_task_description_opt(mut self, desc: Option<String>) -> Self {
        self.task_description = desc;
        self
    }

    pub fn with_task_criteria(mut self, criteria: Vec<String>) -> Self {
        self.task_criteria = criteria;
        self
    }

    pub fn with_requirement_keys(mut self, keys: Vec<String>) -> Self {
        self.requirement_keys = keys;
        self
    }

    pub fn with_task_assumptions(mut self, assumptions: Vec<String>) -> Self {
        self.task_assumptions = assumptions;
        self
    }

    pub fn with_upstream_charter_opt(mut self, charter: Option<String>) -> Self {
        self.upstream_charter = charter;
        self
    }

    pub fn with_upstream_architecture_opt(mut self, architecture: Option<String>) -> Self {
        self.upstream_architecture = architecture;
        self
    }

    pub fn with_upstream_requirements(mut self, requirements: Vec<String>) -> Self {
        self.upstream_requirements = requirements;
        self
    }

    pub fn with_upstream_assumptions(mut self, assumptions: Vec<String>) -> Self {
        self.upstream_assumptions = assumptions;
        self
    }

    pub fn with_upstream_decisions(mut self, decisions: Vec<String>) -> Self {
        self.upstream_decisions = decisions;
        self
    }

    pub fn with_upstream_research_summary_opt(mut self, summary: Option<String>) -> Self {
        self.upstream_research_summary = summary;
        self
    }

    pub fn with_verification_opt(
        mut self,
        verification: Option<crate::kernel::plan::VerificationStrategy>,
    ) -> Self {
        self.verification = verification;
        self
    }

    /// Attach the typed prompt execution binding for this work.
    pub fn with_prompt_ref(mut self, prompt_ref: crate::prompt::PromptReference) -> Self {
        self.prompt_ref = Some(prompt_ref);
        self
    }

    /// Attach the optional typed prompt execution binding for this work.
    pub fn with_prompt_ref_opt(
        mut self,
        prompt_ref: Option<crate::prompt::PromptReference>,
    ) -> Self {
        self.prompt_ref = prompt_ref;
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkExecutionHandle {
    pub job_id: JobId,
    pub task_id: TaskId,
    pub agent_id: AgentId,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkExecutionResult {
    pub task_id: TaskId,
    pub success: bool,
    pub output: String,
    pub error_detail: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token_usage: Option<crate::model::types::TokenUsage>,
}

impl WorkExecutionResult {
    pub fn new(
        task_id: TaskId,
        success: bool,
        output: impl Into<String>,
        error_detail: Option<String>,
    ) -> Self {
        Self {
            task_id,
            success,
            output: output.into(),
            error_detail,
            token_usage: None,
        }
    }

    pub fn with_token_usage(mut self, usage: crate::model::types::TokenUsage) -> Self {
        self.token_usage = Some(usage);
        self
    }
}

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum ExecutionError {
    #[error("worker allocation failed: {0}")]
    AllocationFailed(String),

    #[error("dispatch failed: {0}")]
    DispatchFailed(String),

    #[error("execution error: {0}")]
    ExecutionFailed(String),
}

#[async_trait]
pub trait WorkerDispatcher: Send + Sync {
    async fn allocate_worker(
        &self,
        task_id: TaskId,
        mission_id: MissionId,
        capabilities: &[String],
    ) -> Result<AgentId, ExecutionError>;

    async fn dispatch_work(
        &self,
        req: WorkExecutionRequest,
    ) -> Result<WorkExecutionHandle, ExecutionError>;

    async fn collect_result(
        &self,
        handle: &WorkExecutionHandle,
    ) -> Result<WorkExecutionResult, ExecutionError>;

    async fn cancel_job(&self, _job_id: &JobId) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_execution_dto_serde() {
        let handle = WorkExecutionHandle {
            job_id: JobId::new(),
            task_id: TaskId::new(),
            agent_id: AgentId::new(),
        };
        let serialized = serde_json::to_string(&handle).unwrap();
        let deserialized: WorkExecutionHandle = serde_json::from_str(&serialized).unwrap();
        assert_eq!(handle, deserialized);
    }
}
