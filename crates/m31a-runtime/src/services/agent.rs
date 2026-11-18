//! Agent engine initialization, runtime delegation, and prompt construction.

use crate::services::RuntimeServiceState;
use async_trait::async_trait;
use m31a_agent_engine::agents::EngineAgentRuntime;
use m31a_agent_engine::AgentEngine;
use m31a_core::prompts::{
    CollaborationCapabilitiesSnapshot, CollaborationMode, ModelMetadata, PromptRuntimeSnapshot,
    SecurityPolicySnapshot, SystemPromptBuilder,
};
use m31a_execpolicy::ExecPolicy;
use m31a_guardian::GuardianReviewer;
use m31a_hooks::HookRuntime;
use m31a_provider::ProviderClient;
use m31a_storage::Storage;
use m31a_tools::ToolRegistry;
use m31a_tools::agent_runtime::{
    AgentRuntime, AgentSpawnRequest, AgentWaitOutcome, LiveAgentInfo, SpawnedAgent,
};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};

/// Build the canonical system instructions for a given model, tools, and execution context.
pub fn build_system_prompt(
    cwd: &Path,
    model: &str,
    model_provider: Option<&str>,
    sandbox: m31a_core::SandboxPolicy,
    approval: m31a_core::ApprovalPolicy,
    collaboration_mode: CollaborationMode,
    user_system_prompt: Option<String>,
    tools: &ToolRegistry,
    code_mode: Option<&m31a_code_mode::CodeModeOrchestrator>,
    max_spawn_depth: usize,
) -> String {
    let model_meta = if let Some(info) = m31a_provider::find_model(model) {
        ModelMetadata {
            provider: info.originator.clone(),
            model_id: info.slug.clone(),
            model_family: m31a_core::prompts::ModelFamily::from_model_slug(&info.slug),
            context_window: info.context_window as usize,
            max_output_tokens: Some(info.max_output_tokens as usize),
            supports_tools: info.supports_tools,
            supports_reasoning: info.supports_thinking,
            supports_vision: info.supports_vision,
            supports_parallel_tools: info.supports_tools,
        }
    } else {
        ModelMetadata::conservative(
            model_provider.unwrap_or("NVIDIA NIM / Upstream Provider"),
            model,
        )
    };

    let mut tool_defs = tools.definitions();
    if let Some(cm) = code_mode {
        tool_defs.extend(cm.tool_definitions(&tool_defs));
    }

    let snapshot = PromptRuntimeSnapshot::gather_from_environment(
        cwd,
        model,
        &format!("{:?}", sandbox),
        collaboration_mode,
    )
    .with_user_custom_instructions(user_system_prompt)
    .with_provider(model_provider.unwrap_or("NVIDIA NIM / Upstream Provider"))
    .with_model_metadata(model_meta)
    .with_tool_definitions(&tool_defs)
    .with_security_policy(SecurityPolicySnapshot {
        sandbox_policy: format!("{:?}", sandbox),
        approval_policy: format!("{:?}", approval),
        execution_mode: "runtime".to_string(),
        workspace_root: cwd.to_path_buf(),
        allows_escalation: false,
    })
    .with_collaboration(CollaborationCapabilitiesSnapshot {
        max_spawn_depth,
        current_spawn_depth: 0,
        can_spawn_subagents: max_spawn_depth > 0 && tools.is_allowed("spawn_agent"),
        available_roles: Vec::new(),
        worktree_isolation_supported: true,
        inter_agent_channels_enabled: tools.is_allowed("post_message"),
    });

    SystemPromptBuilder::build_canonical(&snapshot)
}

/// Build a runtime-owned child-agent runtime from canonical shared services.
pub(crate) fn build_engine_runtime(
    client: ProviderClient,
    tools: &ToolRegistry,
    storage: &Storage,
    exec_policy: Arc<RwLock<ExecPolicy>>,
    cwd: PathBuf,
    hooks: Option<Arc<HookRuntime>>,
    context_window: usize,
    max_depth: u32,
    trust_store: Arc<Mutex<m31a_trust::TrustStore>>,
) -> Arc<EngineAgentRuntime> {
    let policy_snapshot = exec_policy
        .read()
        .map(|policy| policy.clone())
        .unwrap_or_else(|_| ExecPolicy::standard());
    Arc::new(
        EngineAgentRuntime::new(
            client,
            tools.clone(),
            storage.clone(),
            policy_snapshot,
            cwd,
            hooks,
            context_window,
            max_depth,
        )
        .with_trust_store(trust_store)
        .with_exec_policy_handle(exec_policy),
    )
}

/// Construct an `AgentEngine` with shared runtime services.
#[allow(clippy::too_many_arguments)]
pub(crate) fn create_agent_engine(
    client: ProviderClient,
    tools: ToolRegistry,
    storage: Storage,
    approval: m31a_core::ApprovalPolicy,
    exec_policy: Arc<RwLock<ExecPolicy>>,
    sandbox: m31a_core::SandboxPolicy,
    cwd: PathBuf,
    guardian: Option<Arc<GuardianReviewer>>,
    hooks: Option<Arc<HookRuntime>>,
    code_mode: Option<m31a_code_mode::CodeModeOrchestrator>,
    routing_enabled: bool,
    tool_output_token_limit: usize,
    collaboration_mode: CollaborationMode,
    dangerously_bypass: bool,
    session_allowed_files: Arc<RwLock<HashSet<PathBuf>>>,
) -> AgentEngine {
    let policy_snapshot = exec_policy
        .read()
        .map(|policy| policy.clone())
        .unwrap_or_else(|_| ExecPolicy::standard());
    let mut engine = AgentEngine::new(
        client,
        tools,
        storage,
        approval,
        policy_snapshot,
        sandbox,
        cwd,
    )
    .with_exec_policy_handle(exec_policy);
    engine.guardian = guardian;
    engine.hooks = hooks;
    engine.code_mode = code_mode;
    engine.routing_enabled = routing_enabled;
    engine.tool_output_token_limit = tool_output_token_limit;
    engine.collaboration_mode = collaboration_mode;
    engine.dangerously_bypass = dangerously_bypass;
    engine.session_allowed_files = session_allowed_files;
    engine
}

/// Stable tool-registry adapter that delegates child-agent operations to the
/// runtime's currently active child-agent runtime.
///
/// Keeping this adapter installed in `ToolRegistry` avoids a race where a
/// model switch replaces the concrete runtime but a tool registry clone still
/// points at the previous provider.
pub(crate) struct RuntimeAgentRuntime {
    state: Arc<RwLock<RuntimeServiceState>>,
}

impl RuntimeAgentRuntime {
    pub(crate) fn new(state: Arc<RwLock<RuntimeServiceState>>) -> Self {
        Self { state }
    }

    fn current(&self) -> Arc<EngineAgentRuntime> {
        Arc::clone(
            &self
                .state
                .read()
                .unwrap_or_else(|e| e.into_inner())
                .engine_runtime,
        )
    }
}

#[async_trait]
impl AgentRuntime for RuntimeAgentRuntime {
    async fn spawn_agent(&self, req: AgentSpawnRequest) -> anyhow::Result<SpawnedAgent> {
        AgentRuntime::spawn_agent(self.current().as_ref(), req).await
    }

    async fn send_input(
        &self,
        agent_id: &str,
        message: &str,
        interrupt: bool,
    ) -> anyhow::Result<String> {
        AgentRuntime::send_input(self.current().as_ref(), agent_id, message, interrupt).await
    }

    async fn wait_agents(
        &self,
        agent_ids: &[String],
        timeout_ms: Option<u64>,
    ) -> anyhow::Result<Vec<AgentWaitOutcome>> {
        AgentRuntime::wait_agents(self.current().as_ref(), agent_ids, timeout_ms).await
    }

    async fn close_agent(&self, agent_id: &str) -> anyhow::Result<String> {
        AgentRuntime::close_agent(self.current().as_ref(), agent_id).await
    }

    async fn terminate_tree(
        &self,
        agent_id: &str,
    ) -> anyhow::Result<m31a_core::types::TreeTerminationReport> {
        AgentRuntime::terminate_tree(self.current().as_ref(), agent_id).await
    }

    async fn list_live(&self) -> anyhow::Result<Vec<LiveAgentInfo>> {
        AgentRuntime::list_live(self.current().as_ref()).await
    }

    async fn integrate_agent(
        &self,
        agent_id: &str,
        commit_message: Option<&str>,
    ) -> anyhow::Result<String> {
        AgentRuntime::integrate_agent(self.current().as_ref(), agent_id, commit_message).await
    }

    fn max_depth(&self) -> u32 {
        self.current().max_depth()
    }

    async fn get_status(&self, agent_id: &str) -> Option<String> {
        AgentRuntime::get_status(self.current().as_ref(), agent_id).await
    }

    async fn get_transcript(&self, agent_id: &str) -> Option<Vec<String>> {
        AgentRuntime::get_transcript(self.current().as_ref(), agent_id).await
    }
}
