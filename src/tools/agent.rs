//! Multi-agent delegation model-facing tool (Issue 13).

use crate::agent::handoff::{
    AgentHandoffArbiter, HandoffError, HandoffProposal, MAX_DELEGATION_DEPTH,
};
use crate::capability::family::CapabilityFamily;
use crate::ids::TaskId;
use crate::state::Task;
use crate::state_machine::agent::AgentRole;
use crate::tools::definition::{ResourceLimits, ToolExecutionContext, TypedTool};
use crate::tools::error::ToolError;
use crate::tools::risk::RiskClass;
use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DelegateTaskInput {
    /// Target role to delegate to (e.g. "architect", "tech_lead", "planner", "implementer", "reviewer", "tester", "researcher", "security_auditor")
    pub target_role: String,
    /// Detailed description of the subtask
    pub subtask: String,
    /// Detailed input specifications, constraints, and instructions for the child agent
    #[serde(default)]
    pub input_spec: Option<String>,
    /// Maximum delegation depth allowed (defaults to 2)
    #[serde(default)]
    pub max_depth: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DelegateTaskOutput {
    pub delegated: bool,
    pub target_role: String,
    pub task_id: Option<String>,
    pub message: String,
}

pub struct DelegateTaskTool;

#[async_trait]
impl TypedTool for DelegateTaskTool {
    type Input = DelegateTaskInput;
    type Output = DelegateTaskOutput;

    fn id(&self) -> &str {
        "delegate_task"
    }

    fn description(&self) -> &str {
        "Delegate an isolated subtask to another specialized agent role with bounded recursion depth."
    }

    fn required_capabilities(&self) -> &[CapabilityFamily] {
        &[]
    }

    fn base_risk(&self) -> RiskClass {
        RiskClass::LowRiskMutation
    }

    fn resource_limits(&self) -> ResourceLimits {
        ResourceLimits::new(30, 1024 * 1024)
    }

    async fn execute(
        &self,
        ctx: &ToolExecutionContext,
        input: Self::Input,
    ) -> Result<Self::Output, ToolError> {
        let mission_id = ctx.mission_id.ok_or_else(|| {
            ToolError::precondition_failed(
                "Missing mission_id in execution context for delegation",
                Some("A valid mission is required to delegate tasks".to_string()),
            )
        })?;

        let task_repo = ctx.task_repo.as_ref().ok_or_else(|| {
            ToolError::execution_failed(
                "DELEGATION_TASK_REPO_MISSING: Cannot delegate task: durable task repository is not configured in execution context",
                None,
                None,
            )
        })?;

        let parent_role_str = ctx
            .agent_role
            .as_ref()
            .map(|r| r.as_str().to_string())
            .unwrap_or_else(|| "implementer".to_string());
        let source_role = AgentRole::new(&parent_role_str);

        let target_role_norm = input.target_role.trim().to_lowercase();
        let target_role = AgentRole::new(&target_role_norm);
        if !target_role.is_well_formed() {
            return Err(ToolError::validation(
                format!("Invalid target role '{}'", input.target_role),
                Some(
                    "Role must be non-empty and contain only letters, numbers, and underscores"
                        .to_string(),
                ),
            ));
        }

        let max_depth_bound = input
            .max_depth
            .map(|d| (d as u32).min(MAX_DELEGATION_DEPTH))
            .unwrap_or(MAX_DELEGATION_DEPTH);

        let mut arbiter =
            AgentHandoffArbiter::new(Arc::new(crate::events::bus::BroadcastEventBus::new(10)));
        arbiter.max_delegation_depth = max_depth_bound;

        let proposal = HandoffProposal {
            source_task_id: ctx.task_id.unwrap_or_default(),
            source_agent_id: ctx.agent_id.unwrap_or_default(),
            source_role,
            target_role: target_role.clone(),
            reason: input.subtask.clone(),
            required_inputs: Vec::new(),
            artifacts: Vec::new(),
            unresolved_questions: Vec::new(),
            current_depth: ctx.delegation_depth,
        };

        arbiter.validate_proposal(&proposal).map_err(|e| match e {
            HandoffError::MaxDelegationDepthExceeded(current) => ToolError::precondition_failed(
                format!("Delegation depth exceeded: current={current}, max={max_depth_bound}"),
                Some("Cannot delegate deeper than max_depth".to_string()),
            ),
            HandoffError::InvalidTargetRole(r) => ToolError::validation(
                format!("Invalid role '{r}': not registered in RoleRegistry"),
                None,
            ),
            HandoffError::SelfDelegationDisallowed(r) => ToolError::precondition_failed(
                format!("Self-delegation to same role '{r}' is disallowed"),
                Some(
                    "Delegate to a different specialized role or perform the task directly"
                        .to_string(),
                ),
            ),
            other => ToolError::execution_failed(
                format!("DELEGATION_VALIDATION_FAILED: {other}"),
                None,
                None,
            ),
        })?;

        let child_task_id = TaskId::new();
        let task_title = format!("[Delegated: {}] {}", target_role_norm, input.subtask.trim());
        let mut child_task = Task::new(child_task_id, mission_id, task_title);
        child_task.role = target_role;
        child_task.description = input.input_spec.or_else(|| Some(input.subtask.clone()));

        task_repo.insert(&child_task).await.map_err(|e| {
            ToolError::execution_failed(
                format!("DELEGATED_TASK_PERSISTENCE_FAILED: Failed to persist delegated child task in database: {e}"),
                None,
                None,
            )
        })?;

        Ok(DelegateTaskOutput {
            delegated: true,
            target_role: target_role_norm.clone(),
            task_id: Some(child_task_id.to_string()),
            message: format!(
                "Subtask successfully delegated to role '{}' (subtask_id: {})",
                target_role_norm, child_task_id
            ),
        })
    }
}
