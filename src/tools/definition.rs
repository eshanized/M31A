//! Typed tool contracts, AnyTool object-safe adapter, and wire schema generator (TL-01, D-05).

use crate::agent::envelope::CapabilityEnvelope;
use crate::capability::family::CapabilityFamily;
use crate::capability::registry::CapabilityRegistry;
use crate::ids::{AgentId, MissionId, TaskId};
use crate::tools::error::ToolError;
use crate::tools::risk::RiskClass;
use async_trait::async_trait;
use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

/// Bounded resource execution limits for a tool.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ResourceLimits {
    pub timeout_secs: u64,
    pub max_output_bytes: usize,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            timeout_secs: crate::config::canonical::DEFAULT_TOOL_TIMEOUT_SECS,
            max_output_bytes: crate::config::canonical::DEFAULT_TOOL_MAX_OUTPUT_BYTES,
        }
    }
}

impl ResourceLimits {
    pub fn new(timeout_secs: u64, max_output_bytes: usize) -> Self {
        Self {
            timeout_secs,
            max_output_bytes,
        }
    }

    /// Immutable safety ceilings: no effective tool limit may exceed these.
    /// Configuration may tighten below them, never raise above them.
    pub const MAX_TIMEOUT_SECS: u64 = 600;
    pub const MAX_OUTPUT_BYTES: usize = 10 * 1024 * 1024;

    /// Resolve the effective execution limit for a tool.
    ///
    /// Authority: the tool's declared `ResourceLimits` is its intrinsic
    /// typed tool-specific policy; `[resources]` supplies the default for
    /// tools using `ResourceLimits::default()`; the ceilings above are
    /// immutable safety bounds. A tool declaration never exceeds the ceiling,
    /// and configuration never raises it.
    pub fn effective(
        declared: &ResourceLimits,
        _configured: &crate::config::ResourcesConfig,
    ) -> ResourceLimits {
        let output = declared
            .max_output_bytes
            .clamp(1024, Self::MAX_OUTPUT_BYTES);
        ResourceLimits::new(
            declared.timeout_secs.clamp(1, Self::MAX_TIMEOUT_SECS),
            output,
        )
    }

    /// Effective limits when the tool uses the canonical default declaration.
    /// In that case the operator-configured `[resources]` values govern
    /// (still clamped to immutable ceilings).
    pub fn effective_default(configured: &crate::config::ResourcesConfig) -> ResourceLimits {
        ResourceLimits::new(
            configured
                .tool_timeout_secs
                .clamp(1, Self::MAX_TIMEOUT_SECS),
            configured
                .tool_max_output_bytes
                .clamp(1024, Self::MAX_OUTPUT_BYTES),
        )
    }
}

/// Execution context passed to tool invocations.
#[derive(Clone)]
pub struct ToolExecutionContext {
    pub capability_registry: Arc<CapabilityRegistry>,
    pub workspace_root: PathBuf,
    pub cancellation_token: CancellationToken,
    pub role_envelope: Option<CapabilityEnvelope>,
    pub mission_id: Option<MissionId>,
    pub task_id: Option<TaskId>,
    pub agent_id: Option<AgentId>,
    pub agent_role: Option<crate::state_machine::agent::AgentRole>,
    pub autonomy_mode: Option<crate::state_machine::AutonomyMode>,
    /// Live policy generation hash observed when this context was bound.
    /// Model-facing tools mint scoped Git authorizations against this hash
    /// so a stale context cannot authorize mutations under a new policy.
    pub policy_hash: Option<String>,
}

impl ToolExecutionContext {
    pub fn new(
        capability_registry: Arc<CapabilityRegistry>,
        workspace_root: PathBuf,
        cancellation_token: CancellationToken,
    ) -> Self {
        Self {
            capability_registry,
            workspace_root,
            cancellation_token,
            role_envelope: None,
            mission_id: None,
            task_id: None,
            agent_id: None,
            agent_role: None,
            autonomy_mode: None,
            policy_hash: None,
        }
    }

    pub fn with_role_envelope(mut self, envelope: CapabilityEnvelope) -> Self {
        self.role_envelope = Some(envelope);
        self
    }

    pub fn with_mission_id(mut self, mission_id: MissionId) -> Self {
        self.mission_id = Some(mission_id);
        self
    }

    pub fn with_task_id(mut self, task_id: TaskId) -> Self {
        self.task_id = Some(task_id);
        self
    }

    pub fn with_agent_id(mut self, agent_id: AgentId) -> Self {
        self.agent_id = Some(agent_id);
        self
    }

    pub fn with_agent_role(mut self, role: crate::state_machine::agent::AgentRole) -> Self {
        self.agent_role = Some(role);
        self
    }

    pub fn with_autonomy_mode(mut self, mode: crate::state_machine::AutonomyMode) -> Self {
        self.autonomy_mode = Some(mode);
        self
    }

    pub fn with_policy_hash(mut self, hash: impl Into<String>) -> Self {
        self.policy_hash = Some(hash.into());
        self
    }
}

/// Canonical strongly typed internal tool contract (TL-01, D-05).
#[async_trait]
pub trait TypedTool: Send + Sync {
    type Input: DeserializeOwned + JsonSchema + Send + Sync + 'static;
    type Output: Serialize + JsonSchema + Send + Sync + 'static;

    fn id(&self) -> &str;
    fn description(&self) -> &str;
    fn required_capabilities(&self) -> &[CapabilityFamily];
    fn base_risk(&self) -> RiskClass;
    fn resource_limits(&self) -> ResourceLimits {
        ResourceLimits::default()
    }
    async fn execute(
        &self,
        ctx: &ToolExecutionContext,
        input: Self::Input,
    ) -> Result<Self::Output, ToolError>;
}

/// Object-safe type-erased tool contract operating on `serde_json::Value`.
#[async_trait]
pub trait AnyTool: Send + Sync {
    fn id(&self) -> &str;
    fn description(&self) -> &str;
    fn parameter_schema(&self) -> serde_json::Value;
    fn result_schema(&self) -> serde_json::Value;
    fn required_capabilities(&self) -> &[CapabilityFamily];
    fn base_risk(&self) -> RiskClass;
    fn resource_limits(&self) -> ResourceLimits;
    async fn execute_raw(
        &self,
        ctx: &ToolExecutionContext,
        raw_args: serde_json::Value,
    ) -> Result<serde_json::Value, ToolError>;
}

/// Generic adapter bridging `TypedTool` to `AnyTool` with automatic `schemars` JSON schema generation.
pub struct ToolAdapter<T> {
    inner: T,
}

impl<T: TypedTool> ToolAdapter<T> {
    pub fn new(inner: T) -> Self {
        Self { inner }
    }

    pub fn inner(&self) -> &T {
        &self.inner
    }
}

#[async_trait]
impl<T: TypedTool + 'static> AnyTool for ToolAdapter<T> {
    fn id(&self) -> &str {
        self.inner.id()
    }

    fn description(&self) -> &str {
        self.inner.description()
    }

    fn parameter_schema(&self) -> serde_json::Value {
        let schema = schemars::schema_for!(T::Input);
        serde_json::to_value(&schema).unwrap_or_else(|_| serde_json::json!({}))
    }

    fn result_schema(&self) -> serde_json::Value {
        let schema = schemars::schema_for!(T::Output);
        serde_json::to_value(&schema).unwrap_or_else(|_| serde_json::json!({}))
    }

    fn required_capabilities(&self) -> &[CapabilityFamily] {
        self.inner.required_capabilities()
    }

    fn base_risk(&self) -> RiskClass {
        self.inner.base_risk()
    }

    fn resource_limits(&self) -> ResourceLimits {
        self.inner.resource_limits()
    }

    async fn execute_raw(
        &self,
        ctx: &ToolExecutionContext,
        raw_args: serde_json::Value,
    ) -> Result<serde_json::Value, ToolError> {
        let input: T::Input =
            serde_json::from_value(raw_args).map_err(|e| ToolError::Validation {
                message: format!("invalid arguments for tool '{}': {e}", self.inner.id()),
                hint: Some("Verify arguments match parameter schema".to_string()),
            })?;

        let output = self.inner.execute(ctx, input).await?;

        serde_json::to_value(output).map_err(|e| ToolError::ExecutionFailed {
            message: format!("failed to serialize tool output: {e}"),
            exit_code: None,
            hint: None,
        })
    }
}

/// Converts an `AnyTool` to OpenAI/NVIDIA `tools[]` function calling declaration (TL-01, D-05).
pub fn to_openai_tool(tool: &dyn AnyTool) -> serde_json::Value {
    serde_json::json!({
        "type": "function",
        "function": {
            "name": tool.id(),
            "description": tool.description(),
            "parameters": tool.parameter_schema(),
        }
    })
}

/// Tool input for signaling task completion.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct CompleteInput {
    /// Summary of the work completed, changes made, and test verification results.
    pub summary: String,
}

/// Tool output for signaling task completion.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CompleteOutput {
    pub completed: bool,
    pub summary: String,
}

/// Core model-facing tool to signal that a task is finished and ready for verification.
#[derive(Debug, Clone, Copy, Default)]
pub struct CompleteTool;

#[async_trait]
impl TypedTool for CompleteTool {
    type Input = CompleteInput;
    type Output = CompleteOutput;

    fn id(&self) -> &str {
        "complete"
    }

    fn description(&self) -> &str {
        "Signal task completion after code changes are written and verified with tests."
    }

    fn required_capabilities(&self) -> &[CapabilityFamily] {
        &[]
    }

    fn base_risk(&self) -> RiskClass {
        RiskClass::ReadOnly
    }

    async fn execute(
        &self,
        _ctx: &ToolExecutionContext,
        input: Self::Input,
    ) -> Result<Self::Output, ToolError> {
        Ok(CompleteOutput {
            completed: true,
            summary: input.summary,
        })
    }
}

/// Tool input for adapting strategy, revising tasks, or replanning based on evidence.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AdaptStrategyInput {
    /// Desired execution strategy shape (e.g. "investigate_then_act", "research_then_act", "ask_user_then_act", "plan_then_execute", "recover", "direct_tool_execution").
    pub strategy: String,
    /// Explanation of why the strategy is being adapted based on execution evidence.
    pub reason: String,
    /// Optional list of task IDs that are now stale, invalid, or superseded.
    #[serde(default)]
    pub tasks_to_supersede: Vec<String>,
    /// Optional new tasks to introduce.
    #[serde(default)]
    pub new_tasks: Vec<String>,
}

/// Tool output for adapting strategy or replanning.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AdaptStrategyOutput {
    pub adapted: bool,
    pub new_strategy: String,
    pub reason: String,
    pub tasks_superseded: usize,
    pub new_tasks_added: usize,
}

/// Core model-facing tool to adapt execution strategy or revise plans.
#[derive(Debug, Clone, Copy, Default)]
pub struct AdaptStrategyTool;

#[async_trait]
impl TypedTool for AdaptStrategyTool {
    type Input = AdaptStrategyInput;
    type Output = AdaptStrategyOutput;

    fn id(&self) -> &str {
        "adapt_strategy"
    }

    fn description(&self) -> &str {
        "Adapt execution strategy or replan tasks when new execution evidence justifies changing approach."
    }

    fn required_capabilities(&self) -> &[CapabilityFamily] {
        &[]
    }

    fn base_risk(&self) -> RiskClass {
        RiskClass::ReadOnly
    }

    async fn execute(
        &self,
        _ctx: &ToolExecutionContext,
        input: Self::Input,
    ) -> Result<Self::Output, ToolError> {
        let superseded_len = input.tasks_to_supersede.len();
        let new_tasks_len = input.new_tasks.len();
        Ok(AdaptStrategyOutput {
            adapted: true,
            new_strategy: input.strategy,
            reason: input.reason,
            tasks_superseded: superseded_len,
            new_tasks_added: new_tasks_len,
        })
    }
}
