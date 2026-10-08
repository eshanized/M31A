//! Multi-agent delegation model-facing tool (Issue 13).

use crate::agent::delegation::{DelegationContract, DelegationError};
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

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DelegateTaskInput {
    /// Target role to delegate to (e.g. "architect", "tech_lead", "planner", "implementer", "reviewer", "tester", "researcher", "security_auditor")
    pub target_role: String,
    /// Detailed description of the subtask
    pub subtask: String,
    /// Detailed input specifications, constraints, and instructions for the child agent
    #[serde(default)]
    pub input_spec: Option<String>,
    /// Maximum delegation depth allowed (defaults to 3)
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
        RiskClass::ReadOnly
    }

    fn resource_limits(&self) -> ResourceLimits {
        ResourceLimits::new(30, 1024 * 1024)
    }

    async fn execute(
        &self,
        ctx: &ToolExecutionContext,
        input: Self::Input,
    ) -> Result<Self::Output, ToolError> {
        let parent_role_str = ctx
            .agent_role
            .as_ref()
            .map(|r| r.as_str().to_string())
            .unwrap_or_else(|| "implementer".to_string());

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

        let max_depth = input.max_depth.unwrap_or(3);
        let mut contract = DelegationContract::new(
            parent_role_str,
            target_role_norm.clone(),
            input.subtask.clone(),
            input.input_spec.as_deref().unwrap_or(""),
            0,
        )
        .with_max_depth(max_depth);

        if let Some(agent_id) = ctx.agent_id {
            contract = contract.with_parent_agent_id(agent_id);
        }

        contract.validate().map_err(|e| match e {
            DelegationError::DepthExceeded {
                current_depth,
                max_depth,
            } => ToolError::precondition_failed(
                format!("Delegation depth exceeded: current={current_depth}, max={max_depth}"),
                Some("Cannot delegate deeper than max_depth".to_string()),
            ),
            DelegationError::InvalidRole(r) => {
                ToolError::validation(format!("Invalid role '{r}'"), None)
            }
            DelegationError::SelfDelegationDisallowed(r) => ToolError::precondition_failed(
                format!("Self-delegation to same role '{r}' is disallowed"),
                Some(
                    "Delegate to a different specialized role or perform the task directly"
                        .to_string(),
                ),
            ),
            other => ToolError::execution_failed(other.to_string(), None, None),
        })?;

        let mut subtask_id_str = None;
        if let (Some(mission_id), Some(task_repo)) = (ctx.mission_id, &ctx.task_repo) {
            let child_task_id = TaskId::new();
            let task_title = format!("[Delegated: {}] {}", target_role_norm, input.subtask.trim());
            let child_task = Task::new(child_task_id, mission_id, task_title);
            if let Ok(()) = task_repo.insert(&child_task).await {
                subtask_id_str = Some(child_task_id.to_string());
            }
        }

        Ok(DelegateTaskOutput {
            delegated: true,
            target_role: target_role_norm.clone(),
            task_id: subtask_id_str.clone(),
            message: format!(
                "Subtask successfully delegated to role '{}' (subtask_id: {:?})",
                target_role_norm, subtask_id_str
            ),
        })
    }
}
