//! Phase 8: Tool Catalog, Schemars Schema Derivation & Model-Visible Filtering Tests (TL-01, TL-02, TL-05).

use async_trait::async_trait;
use m31a::agent::envelope::CapabilityEnvelope;
use m31a::capability::family::CapabilityFamily;
use m31a::capability::health::CapabilityHealthState;
use m31a::capability::instance::CapabilityInstance;
use m31a::capability::permissions::CapabilityPermissions;
use m31a::capability::registry::CapabilityRegistry;
use m31a::kernel::cancellation::MissionRuntime;
use m31a::state::intake::AutonomyMode;
use m31a::tools::definition::{
    AnyTool, ResourceLimits, ToolAdapter, ToolExecutionContext, TypedTool, to_openai_tool,
};
use m31a::tools::error::ToolError;
use m31a::tools::filter::{FilterCriteria, ToolFilter};
use m31a::tools::registry::ToolRegistry;
use m31a::tools::risk::{EffectiveRisk, RiskClass};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
struct EchoInput {
    pub message: String,
    pub repeat_count: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
struct EchoOutput {
    pub echoed: String,
    pub count: u32,
}

struct EchoTool;

#[async_trait]
impl TypedTool for EchoTool {
    type Input = EchoInput;
    type Output = EchoOutput;

    fn id(&self) -> &str {
        "test_echo"
    }

    fn description(&self) -> &str {
        "Test tool that echoes input message"
    }

    fn required_capabilities(&self) -> &[CapabilityFamily] {
        &[CapabilityFamily::Filesystem]
    }

    fn base_risk(&self) -> RiskClass {
        RiskClass::ReadOnly
    }

    fn resource_limits(&self) -> ResourceLimits {
        ResourceLimits::new(10, 512)
    }

    async fn execute(
        &self,
        _ctx: &ToolExecutionContext,
        input: Self::Input,
    ) -> Result<Self::Output, ToolError> {
        let count = input.repeat_count.unwrap_or(1);
        let echoed = input.message.repeat(count as usize);
        Ok(EchoOutput { echoed, count })
    }
}

#[tokio::test]
async fn test_tool_schemas() {
    let tool = EchoTool;
    let adapter = ToolAdapter::new(tool);

    // 1. Parameter and result JSON schemas derived via schemars
    let param_schema = adapter.parameter_schema();
    assert!(param_schema.is_object());
    assert_eq!(param_schema["type"], "object");
    assert!(param_schema["properties"]["message"].is_object());
    assert_eq!(param_schema["properties"]["message"]["type"], "string");

    let result_schema = adapter.result_schema();
    assert!(result_schema.is_object());
    assert_eq!(result_schema["type"], "object");
    assert!(result_schema["properties"]["echoed"].is_object());

    // 2. OpenAI wire format translation
    let openai_val = to_openai_tool(&adapter);
    assert_eq!(openai_val["type"], "function");
    assert_eq!(openai_val["function"]["name"], "test_echo");
    assert_eq!(
        openai_val["function"]["description"],
        "Test tool that echoes input message"
    );
    assert_eq!(openai_val["function"]["parameters"]["type"], "object");

    // 3. Object-safe execution via AnyTool execute_raw
    let runtime = MissionRuntime::new();
    let ctx = ToolExecutionContext::new(
        Arc::new(CapabilityRegistry::new()),
        PathBuf::from("/tmp"),
        runtime.spawn_child(),
    );

    let raw_args = serde_json::json!({
        "message": "hello ",
        "repeat_count": 3
    });
    let raw_res = adapter.execute_raw(&ctx, raw_args).await.unwrap();
    assert_eq!(raw_res["echoed"], "hello hello hello ");
    assert_eq!(raw_res["count"], 3);

    // 4. Argument validation failure handling
    let bad_args = serde_json::json!({
        "message": 12345
    });
    let err = adapter.execute_raw(&ctx, bad_args).await.unwrap_err();
    match err {
        ToolError::Validation { message, hint } => {
            assert!(message.contains("test_echo"));
            assert!(hint.is_some());
        }
        other => panic!("expected Validation error, got: {other:?}"),
    }

    // 5. EffectiveRisk calculation rules and non-bypassable floor
    let mut risk = EffectiveRisk::new(RiskClass::HighRiskMutation);
    assert_eq!(risk.effective_risk(), RiskClass::HighRiskMutation);

    // Cannot downgrade below base risk
    risk.escalate(RiskClass::ReadOnly, "attempted reduction");
    assert_eq!(risk.effective_risk(), RiskClass::HighRiskMutation);
    assert!(risk.reasons().is_empty());

    // Escalating to higher risk succeeds
    risk.escalate(RiskClass::ExternalCommunication, "network call");
    assert_eq!(risk.effective_risk(), RiskClass::ExternalCommunication);
    assert_eq!(risk.reasons().len(), 1);

    // Clamping on from_candidate
    let clamped = EffectiveRisk::from_candidate(
        RiskClass::ProcessExecution,
        RiskClass::LowRiskMutation,
        "downgrade",
    );
    assert_eq!(clamped.effective_risk(), RiskClass::ProcessExecution);

    // Path evaluation escalation
    let normal_path =
        EffectiveRisk::evaluate_file_path(RiskClass::LowRiskMutation, Path::new("normal_file.txt"));
    assert_eq!(normal_path.effective_risk(), RiskClass::LowRiskMutation);

    let sensitive_path =
        EffectiveRisk::evaluate_file_path(RiskClass::LowRiskMutation, Path::new(".env"));
    assert_eq!(sensitive_path.effective_risk(), RiskClass::HighRiskMutation);

    let git_path =
        EffectiveRisk::evaluate_file_path(RiskClass::LowRiskMutation, Path::new(".git/HEAD"));
    assert_eq!(git_path.effective_risk(), RiskClass::HighRiskMutation);
}

#[tokio::test]
async fn test_28_core_tools() {
    let capabilities = Arc::new(CapabilityRegistry::new());
    let registry = ToolRegistry::new_default(capabilities.clone());

    // Verify exactly 28 tools
    assert_eq!(
        registry.len(),
        28,
        "Expected exactly 28 core tools in ToolRegistry"
    );

    let expected_tools = [
        // fs (7)
        "read_file",
        "write_file",
        "edit_file",
        "apply_patch",
        "list_files",
        "glob",
        "grep",
        // repo (3)
        "repo_search",
        "repo_symbols",
        "repo_dependencies",
        // process (5)
        "run_command",
        "start_job",
        "job_status",
        "job_output",
        "job_stop",
        // git (8)
        "git_status",
        "git_diff",
        "git_log",
        "git_show",
        "git_branch",
        "git_checkout",
        "git_add",
        "git_commit",
        // qa (3)
        "run_tests",
        "run_formatter",
        "run_linter",
        // artifact (2)
        "create_artifact",
        "read_artifact",
    ];

    assert_eq!(expected_tools.len(), 28);

    for tool_id in expected_tools {
        let tool = registry.get(tool_id);
        assert!(tool.is_some(), "Tool '{tool_id}' not found in registry");
        let tool = tool.unwrap();

        assert_eq!(tool.id(), tool_id);
        assert!(!tool.description().is_empty());
        assert!(!tool.required_capabilities().is_empty());

        let param_schema = tool.parameter_schema();
        assert!(
            param_schema.is_object(),
            "Parameter schema for '{tool_id}' must be an object"
        );

        let result_schema = tool.result_schema();
        assert!(
            result_schema.is_object(),
            "Result schema for '{tool_id}' must be an object"
        );

        let openai_decl = to_openai_tool(&*tool);
        assert_eq!(openai_decl["type"], "function");
        assert_eq!(openai_decl["function"]["name"], tool_id);
    }
}

#[tokio::test]
async fn test_tool_filter_denied_tools() {
    let capabilities = Arc::new(CapabilityRegistry::new());
    let registry = Arc::new(ToolRegistry::new_default(capabilities.clone()));
    let filter = ToolFilter::new(registry);

    let criteria = FilterCriteria::new(capabilities).with_denied_tools(["read_file", "git_status"]);
    let tools = filter.filter_tools(&criteria);

    assert_eq!(tools.len(), 26);
    assert!(!tools.iter().any(|t| t.id() == "read_file"));
    assert!(!tools.iter().any(|t| t.id() == "git_status"));
    assert!(tools.iter().any(|t| t.id() == "write_file"));
}

#[tokio::test]
async fn test_tool_filter_autonomy_mode_plan() {
    let capabilities = Arc::new(CapabilityRegistry::new());
    let registry = Arc::new(ToolRegistry::new_default(capabilities.clone()));
    let filter = ToolFilter::new(registry);

    let criteria = FilterCriteria::new(capabilities).with_autonomy_mode(AutonomyMode::Plan);
    let tools = filter.filter_tools(&criteria);

    // All returned tools MUST be ReadOnly
    for tool in &tools {
        assert_eq!(
            tool.base_risk(),
            RiskClass::ReadOnly,
            "Tool '{}' has base risk {:?} but was admitted under Plan mode",
            tool.id(),
            tool.base_risk()
        );
    }

    // Must NOT contain mutating tools
    let mutating_tools = [
        "write_file",
        "edit_file",
        "apply_patch",
        "git_commit",
        "run_command",
        "create_artifact",
    ];
    for tool_id in mutating_tools {
        assert!(
            !tools.iter().any(|t| t.id() == tool_id),
            "Mutating tool '{tool_id}' should be pruned in Plan mode"
        );
    }

    // Must contain read-only tools
    assert!(tools.iter().any(|t| t.id() == "read_file"));
    assert!(tools.iter().any(|t| t.id() == "git_status"));
    assert!(tools.iter().any(|t| t.id() == "repo_search"));
}

#[tokio::test]
async fn test_tool_filter_role_envelope() {
    let capabilities = Arc::new(CapabilityRegistry::new());
    let registry = Arc::new(ToolRegistry::new_default(capabilities.clone()));
    let filter = ToolFilter::new(registry);

    // Planner role: strictly read-only, repo and artifacts
    let planner_envelope =
        CapabilityEnvelope::read_only(["repo.read", "artifacts.read", "plan.propose"]);

    let criteria = FilterCriteria::new(capabilities).with_role_envelope(&planner_envelope);
    let tools = filter.filter_tools(&criteria);

    // Mutating and shell tools must be excluded
    assert!(!tools.iter().any(|t| t.id() == "write_file"));
    assert!(!tools.iter().any(|t| t.id() == "run_command"));
    assert!(!tools.iter().any(|t| t.id() == "git_commit"));

    // Permitted read tools must be present
    assert!(tools.iter().any(|t| t.id() == "repo_search"));
    assert!(tools.iter().any(|t| t.id() == "repo_symbols"));
    assert!(tools.iter().any(|t| t.id() == "read_artifact"));
}

#[tokio::test]
async fn test_tool_filter_deterministic_sorting() {
    let capabilities = Arc::new(CapabilityRegistry::new());
    let registry = Arc::new(ToolRegistry::new_default(capabilities.clone()));
    let filter = ToolFilter::new(registry);

    let criteria = FilterCriteria::new(capabilities);
    let tools = filter.filter_tools(&criteria);

    let ids: Vec<&str> = tools.iter().map(|t| t.id()).collect();
    let mut sorted_ids = ids.clone();
    sorted_ids.sort();

    assert_eq!(
        ids, sorted_ids,
        "Tools must be sorted lexicographically by ToolId"
    );
}

#[tokio::test]
async fn test_tool_filter_context_budget() {
    let capabilities = Arc::new(CapabilityRegistry::new());
    let registry = Arc::new(ToolRegistry::new_default(capabilities.clone()));
    let filter = ToolFilter::new(registry);

    // Get size of a single tool wire format
    let all_tools = filter.filter_tools(&FilterCriteria::new(capabilities.clone()));
    assert_eq!(all_tools.len(), 28);

    let first_tool_size = serde_json::to_vec(&to_openai_tool(&*all_tools[0]))
        .unwrap()
        .len();

    // Set budget just enough for ~2 tools
    let tight_budget = first_tool_size * 2 + 50;
    let criteria = FilterCriteria::new(capabilities).with_context_budget(tight_budget);
    let budgeted_tools = filter.filter_tools(&criteria);

    assert!(!budgeted_tools.is_empty() && budgeted_tools.len() <= 3);

    // Verify wire format output is valid JSON
    let wire = filter.filter_to_wire_format(&criteria);
    assert_eq!(wire.len(), budgeted_tools.len());
    for item in wire {
        assert_eq!(item["type"], "function");
        assert!(item["function"]["parameters"].is_object());
    }
}

#[tokio::test]
async fn test_tool_filter_health_check() {
    let config = m31a::capability::health::HealthConfig {
        cooldown_duration: std::time::Duration::from_secs(60),
        ..Default::default()
    };
    let capabilities = Arc::new(CapabilityRegistry::with_health_config(config));
    // Register an unavailable Process capability instance
    let instance = CapabilityInstance::new(
        "native-process",
        "Local Process",
        "1.0",
        CapabilityFamily::Process,
        "tokio_process",
        CapabilityPermissions::full_access(),
    )
    .with_health(CapabilityHealthState::Unavailable);

    capabilities.register_instance(instance);

    let registry = Arc::new(ToolRegistry::new_default(capabilities.clone()));
    let filter = ToolFilter::new(registry);

    let criteria = FilterCriteria::new(capabilities);
    let tools = filter.filter_tools(&criteria);

    // run_command requires Process, so it must be dropped
    assert!(!tools.iter().any(|t| t.id() == "run_command"));
    // Other tools (e.g. read_file) remain
    assert!(tools.iter().any(|t| t.id() == "read_file"));
}
