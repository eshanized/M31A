//! Agent engine initialization, system prompt generation, and turn execution loop.

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
use std::path::Path;
use std::sync::Arc;

/// Build the canonical system instructions for a given model, tools, and execution context.
#[allow(clippy::too_many_arguments)]
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

/// Construct an `AgentEngine` with shared runtime services.
#[allow(clippy::too_many_arguments)]
pub fn create_agent_engine(
    client: ProviderClient,
    tools: ToolRegistry,
    storage: Storage,
    approval: m31a_core::ApprovalPolicy,
    exec_policy: ExecPolicy,
    sandbox: m31a_core::SandboxPolicy,
    cwd: std::path::PathBuf,
    guardian: Option<Arc<GuardianReviewer>>,
    hooks: Option<Arc<HookRuntime>>,
    code_mode: Option<m31a_code_mode::CodeModeOrchestrator>,
    routing_enabled: bool,
    tool_output_token_limit: usize,
    collaboration_mode: CollaborationMode,
    dangerously_bypass: bool,
) -> AgentEngine {
    let mut engine = AgentEngine::new(client, tools, storage, approval, exec_policy, sandbox, cwd);
    engine.guardian = guardian;
    engine.hooks = hooks;
    engine.code_mode = code_mode;
    engine.routing_enabled = routing_enabled;
    engine.tool_output_token_limit = tool_output_token_limit;
    engine.collaboration_mode = collaboration_mode;
    engine.dangerously_bypass = dangerously_bypass;
    engine
}
