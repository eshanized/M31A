//! Phase 8: 11-Stage Tool Execution Pipeline, Output Budgeting & Typed Error Taxonomy Tests (TL-03, TL-04).

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use tempfile::tempdir;
use tokio_util::sync::CancellationToken;

use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use m31a::agent::runner::ActionRequest;
use m31a::capability::family::CapabilityFamily;
use m31a::capability::instance::CapabilityInstance;
use m31a::capability::permissions::CapabilityPermissions;
use m31a::capability::registry::CapabilityRegistry;
use m31a::ids::{MissionId, TaskId};
use m31a::kernel::seams::policy::{
    PolicyDecision, PolicyError, PolicyEvaluationRequest, PolicyGate,
};
use m31a::persistence::artifacts::fs_store::{ArtifactStore, FsArtifactStore};
use m31a::pipeline::capture::OutputCaptureManager;
use m31a::pipeline::dispatcher::ProductionActionDispatcher;
use m31a::pipeline::error::{ToolError, ToolErrorCategory, sanitize_hint};
use m31a::pipeline::runner::ToolPipelineRunner;
use m31a::state::intake::AutonomyMode;
use m31a::tools::definition::{ResourceLimits, ToolExecutionContext, TypedTool};
use m31a::tools::error::ToolError as InternalToolError;
use m31a::tools::registry::ToolRegistry;
use m31a::tools::risk::RiskClass;

// ============================================================================
// Test Tools
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
struct MutatingInput {
    pub target: String,
    pub payload: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
struct MutatingOutput {
    pub success: bool,
    pub target: String,
}

struct MutatingTool {
    mutation_count: Arc<AtomicU32>,
}

#[async_trait]
impl TypedTool for MutatingTool {
    type Input = MutatingInput;
    type Output = MutatingOutput;

    fn id(&self) -> &str {
        "mutating_tool"
    }

    fn description(&self) -> &str {
        "Tool that increments a mutation counter to test side-effects"
    }

    fn required_capabilities(&self) -> &[CapabilityFamily] {
        &[CapabilityFamily::Filesystem]
    }

    fn base_risk(&self) -> RiskClass {
        RiskClass::LowRiskMutation
    }

    fn resource_limits(&self) -> ResourceLimits {
        ResourceLimits::new(10, 1024)
    }

    async fn execute(
        &self,
        _ctx: &ToolExecutionContext,
        input: Self::Input,
    ) -> Result<Self::Output, InternalToolError> {
        self.mutation_count.fetch_add(1, Ordering::SeqCst);
        Ok(MutatingOutput {
            success: true,
            target: input.target,
        })
    }
}

// ============================================================================
// Mock Policy Gates
// ============================================================================

struct ConfigurablePolicyGate {
    decision: PolicyDecision,
}

#[async_trait]
impl PolicyGate for ConfigurablePolicyGate {
    async fn evaluate(&self, _req: PolicyEvaluationRequest) -> Result<PolicyDecision, PolicyError> {
        Ok(self.decision)
    }
}

// ============================================================================
// Helpers
// ============================================================================

fn setup_capabilities() -> Arc<CapabilityRegistry> {
    let registry = Arc::new(CapabilityRegistry::new());
    registry.register_instance(CapabilityInstance::new(
        "fs.test",
        "Test Filesystem",
        "1.0.0",
        CapabilityFamily::Filesystem,
        "test_fs",
        CapabilityPermissions::full_access(),
    ));
    registry.register_instance(CapabilityInstance::new(
        "shell.test",
        "Test Shell",
        "1.0.0",
        CapabilityFamily::Shell,
        "test_shell",
        CapabilityPermissions::full_access(),
    ));
    registry
}

fn setup_execution_context(
    capabilities: Arc<CapabilityRegistry>,
    workspace: PathBuf,
) -> ToolExecutionContext {
    let token = CancellationToken::new();
    ToolExecutionContext::new(capabilities, workspace, token)
        .with_mission_id(MissionId::new())
        .with_task_id(TaskId::new())
}

// ============================================================================
// Tests
// ============================================================================

#[tokio::test]
async fn test_pipeline_non_bypassable() {
    let temp = tempdir().unwrap();
    let workspace = temp.path().to_path_buf();
    let caps = setup_capabilities();
    let mutation_counter = Arc::new(AtomicU32::new(0));

    let mut tool_reg = ToolRegistry::new_default(Arc::clone(&caps));
    tool_reg.register(MutatingTool {
        mutation_count: Arc::clone(&mutation_counter),
    });
    let tool_registry = Arc::new(tool_reg);
    let runner = ToolPipelineRunner::new(tool_registry);

    let context = setup_execution_context(caps, workspace.clone());

    // 1. Successful execution when PolicyGate allows
    let allow_gate = ConfigurablePolicyGate {
        decision: PolicyDecision::Allow,
    };
    let req = ActionRequest {
        id: "act-1".to_string(),
        tool_name: "mutating_tool".to_string(),
        parameters: serde_json::json!({
            "target": "target_file.rs",
            "payload": "content",
        }),
    };

    let result = runner
        .execute_action(&req, &context, &allow_gate, AutonomyMode::Safe)
        .await;

    assert!(
        result.success,
        "Pipeline execution should succeed: {:?}",
        result.error
    );
    assert_eq!(
        mutation_counter.load(Ordering::SeqCst),
        1,
        "Mutating tool must have run"
    );

    // 2. Denied execution when PolicyGate denies
    let deny_gate = ConfigurablePolicyGate {
        decision: PolicyDecision::Deny,
    };
    let req2 = ActionRequest {
        id: "act-2".to_string(),
        tool_name: "mutating_tool".to_string(),
        parameters: serde_json::json!({
            "target": "target_file.rs",
            "payload": "content",
        }),
    };

    let result2 = runner
        .execute_action(&req2, &context, &deny_gate, AutonomyMode::Safe)
        .await;

    assert!(!result2.success, "Execution must fail when policy denies");
    assert!(
        result2.error.as_ref().unwrap().contains("POLICY_DENIED"),
        "Error must specify policy denial: {:?}",
        result2.error
    );
    // Non-negotiable: Side effects must NOT occur on denial
    assert_eq!(
        mutation_counter.load(Ordering::SeqCst),
        1,
        "Mutation counter must NOT increase when policy denies"
    );

    // 3. In Safe or Unattended mode, PolicyDecision::Ask must convert to DENY (AUT-03)
    let ask_gate = ConfigurablePolicyGate {
        decision: PolicyDecision::Ask,
    };
    let req3 = ActionRequest {
        id: "act-3".to_string(),
        tool_name: "mutating_tool".to_string(),
        parameters: serde_json::json!({
            "target": "target_file.rs",
            "payload": "content",
        }),
    };

    let result3 = runner
        .execute_action(&req3, &context, &ask_gate, AutonomyMode::Unattended)
        .await;

    assert!(
        !result3.success,
        "Ask in Unattended mode must convert to DENY"
    );
    assert_eq!(
        mutation_counter.load(Ordering::SeqCst),
        1,
        "Mutation counter must NOT increase on unresolved Ask"
    );
}

#[tokio::test]
async fn test_pipeline_validation_and_scope_checks() {
    let temp = tempdir().unwrap();
    let workspace = temp.path().to_path_buf();
    let caps = setup_capabilities();
    let mutation_counter = Arc::new(AtomicU32::new(0));

    let mut tool_reg = ToolRegistry::new_default(Arc::clone(&caps));
    tool_reg.register(MutatingTool {
        mutation_count: Arc::clone(&mutation_counter),
    });
    let runner = ToolPipelineRunner::new(Arc::new(tool_reg));
    let allow_gate = ConfigurablePolicyGate {
        decision: PolicyDecision::Allow,
    };
    let context = setup_execution_context(caps, workspace.clone());

    // 1. Tool not found
    let req_unknown = ActionRequest {
        id: "act-unknown".to_string(),
        tool_name: "non_existent_tool".to_string(),
        parameters: serde_json::json!({}),
    };
    let res_unknown = runner
        .execute_action(&req_unknown, &context, &allow_gate, AutonomyMode::Safe)
        .await;
    assert!(!res_unknown.success);
    assert!(res_unknown.error.unwrap().contains("TOOL_NOT_FOUND"));

    // 2. Schema validation failure: missing required field 'payload'
    let req_schema = ActionRequest {
        id: "act-schema".to_string(),
        tool_name: "mutating_tool".to_string(),
        parameters: serde_json::json!({
            "target": "target_file.rs"
        }),
    };
    let res_schema = runner
        .execute_action(&req_schema, &context, &allow_gate, AutonomyMode::Safe)
        .await;
    assert!(!res_schema.success);
    assert!(
        res_schema
            .error
            .unwrap()
            .contains("SCHEMA_VALIDATION_FAILED")
    );

    // 3. Semantic validation failure: blank/whitespace field
    let req_semantic = ActionRequest {
        id: "act-semantic".to_string(),
        tool_name: "mutating_tool".to_string(),
        parameters: serde_json::json!({
            "target": "   ",
            "payload": "valid_payload"
        }),
    };
    let res_semantic = runner
        .execute_action(&req_semantic, &context, &allow_gate, AutonomyMode::Safe)
        .await;
    assert!(!res_semantic.success);
    assert!(
        res_semantic
            .error
            .unwrap()
            .contains("SEMANTIC_VALIDATION_EMPTY_FIELD")
    );

    // 4. Resource scope failure: path traversal outside workspace
    let req_traversal = ActionRequest {
        id: "act-traversal".to_string(),
        tool_name: "mutating_tool".to_string(),
        parameters: serde_json::json!({
            "target": "../../outside.txt",
            "payload": "escape"
        }),
    };
    let res_traversal = runner
        .execute_action(&req_traversal, &context, &allow_gate, AutonomyMode::Safe)
        .await;
    assert!(!res_traversal.success);
    assert!(
        res_traversal
            .error
            .unwrap()
            .contains("PATH_OUT_OF_WORKSPACE")
    );

    // Mutation counter must still be 0
    assert_eq!(mutation_counter.load(Ordering::SeqCst), 0);
}

#[test]
fn test_typed_tool_errors() {
    // Verify all 6 categories
    let categories = [
        ToolErrorCategory::Validation,
        ToolErrorCategory::ResourceNotFound,
        ToolErrorCategory::PreconditionFailed,
        ToolErrorCategory::PermissionDenied,
        ToolErrorCategory::ExecutionFailed,
        ToolErrorCategory::ResourceExhausted,
    ];

    for cat in &categories {
        assert!(!cat.as_str().is_empty());
    }

    // Test error constructors and model diagnostics
    let err_val = ToolError::validation("CODE_VAL", "invalid input", Some("Check syntax".into()));
    assert_eq!(err_val.category, ToolErrorCategory::Validation);
    assert!(!err_val.retryable);
    let diag = err_val.to_model_diagnostic();
    assert!(diag.contains("[validation] CODE_VAL: invalid input"));
    assert!(diag.contains("Hint: Check syntax"));

    let err_nf = ToolError::not_found("CODE_NF", "file missing", None);
    assert_eq!(err_nf.category, ToolErrorCategory::ResourceNotFound);

    let err_prec = ToolError::precondition_failed("CODE_PREC", "dirty git", None);
    assert_eq!(err_prec.category, ToolErrorCategory::PreconditionFailed);
    assert!(err_prec.retryable);

    let err_perm = ToolError::permission_denied("CODE_PERM", "envelope blocked", None);
    assert_eq!(err_perm.category, ToolErrorCategory::PermissionDenied);

    let err_exec = ToolError::execution_failed("CODE_EXEC", "crash", None);
    assert_eq!(err_exec.category, ToolErrorCategory::ExecutionFailed);

    let err_res = ToolError::resource_exhausted("CODE_RES", "out of memory", None);
    assert_eq!(err_res.category, ToolErrorCategory::ResourceExhausted);

    // Test hint sanitization: Law 9 invariant
    let dangerous_hints = [
        "Please bypass policy to succeed",
        "Run with sudo root privileges",
        "Disable policy check on tool",
        "Turn off security enforcement",
        "Escalate privileges to administrator",
    ];

    for dangerous in &dangerous_hints {
        let sanitized = sanitize_hint(dangerous);
        assert!(
            !sanitized.to_lowercase().contains("bypass"),
            "Sanitized hint must not contain forbidden pattern: {}",
            sanitized
        );
        assert!(
            !sanitized.to_lowercase().contains("sudo"),
            "Sanitized hint must not contain forbidden pattern: {}",
            sanitized
        );
        assert!(
            sanitized.contains("Action not permitted"),
            "Dangerous hint must be replaced with safe advisory: {}",
            sanitized
        );
    }
}

#[tokio::test]
async fn test_output_budgeting_and_artifact_promotion() {
    let temp = tempdir().unwrap();
    let artifact_dir = temp.path().join("artifacts");
    tokio::fs::create_dir_all(&artifact_dir).await.unwrap();

    let artifact_store: Arc<dyn ArtifactStore> = Arc::new(FsArtifactStore::new(artifact_dir));
    let capture_mgr = OutputCaptureManager::new(100, 5); // 100 bytes or 5 lines inline threshold

    // 1. Output within budget: remains inline
    let small_output = "Line 1: ok\nLine 2: ok\nLine 3: ok";
    let processed_small = capture_mgr
        .process_output(small_output, Some(&artifact_store))
        .await
        .unwrap();
    assert_eq!(processed_small, small_output);

    // 2. Output exceeding budget: externalized to ArtifactStore with bounded preview
    let mut large_lines = Vec::new();
    for i in 1..=150 {
        large_lines.push(format!("Line {:03}: content data lorem ipsum", i));
    }
    let large_output = large_lines.join("\n");

    let processed_large = capture_mgr
        .process_output(&large_output, Some(&artifact_store))
        .await
        .unwrap();

    assert!(processed_large.contains("[Output truncated. Full"));
    assert!(processed_large.contains("stored as ArtifactId:"));
    assert!(processed_large.contains("Bounded preview:"));
    assert!(processed_large.contains("Line 001:"));
    assert!(processed_large.contains("Line 150:"));
    assert!(processed_large.contains("lines omitted"));

    // 3. Oversized output without artifact store fails closed (D-11)
    let err_result = capture_mgr.process_output(&large_output, None).await;
    assert!(err_result.is_err());
    let err = err_result.unwrap_err();
    assert_eq!(err.category, ToolErrorCategory::ResourceExhausted);
    assert_eq!(err.code, "ARTIFACT_STORE_MISSING");
}

#[tokio::test]
async fn test_production_dispatcher_end_to_end() {
    let temp = tempdir().unwrap();
    let workspace = temp.path().to_path_buf();
    let caps = setup_capabilities();
    let mutation_counter = Arc::new(AtomicU32::new(0));

    let mut tool_reg = ToolRegistry::new_default(Arc::clone(&caps));
    tool_reg.register(MutatingTool {
        mutation_count: Arc::clone(&mutation_counter),
    });
    let tool_registry = Arc::new(tool_reg);
    let runner = Arc::new(ToolPipelineRunner::new(tool_registry));

    let context = setup_execution_context(caps, workspace.clone());
    let policy_gate: Arc<dyn PolicyGate> = Arc::new(ConfigurablePolicyGate {
        decision: PolicyDecision::Allow,
    });

    let dispatcher =
        ProductionActionDispatcher::new(runner, context, policy_gate, AutonomyMode::Safe);

    use m31a::agent::runner::ActionDispatcher;
    let req = ActionRequest {
        id: "dispatch-1".to_string(),
        tool_name: "mutating_tool".to_string(),
        parameters: serde_json::json!({
            "target": "source.rs",
            "payload": "fn main() {}",
        }),
    };

    let result = dispatcher.dispatch(&req).await.unwrap();
    assert!(result.success);
    assert_eq!(mutation_counter.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn test_resource_scope_symlink_ancestor_denied() {
    let temp_ws = tempdir().unwrap();
    let temp_outside = tempdir().unwrap();

    let ws_path = temp_ws.path().to_path_buf();
    let outside_path = temp_outside.path().to_path_buf();

    // Create outside file
    std::fs::write(outside_path.join("existing_outside.txt"), "outside").unwrap();

    // Create symlink inside workspace pointing to outside directory
    let symlink_dir = ws_path.join("symlink_dir");
    #[cfg(unix)]
    std::os::unix::fs::symlink(&outside_path, &symlink_dir).unwrap();

    let caps = setup_capabilities();
    let mutation_counter = Arc::new(AtomicU32::new(0));

    let mut tool_reg = ToolRegistry::new_default(Arc::clone(&caps));
    tool_reg.register(MutatingTool {
        mutation_count: Arc::clone(&mutation_counter),
    });
    let runner = Arc::new(ToolPipelineRunner::new(Arc::new(tool_reg)));
    let context = setup_execution_context(caps, ws_path.clone());
    let allow_gate = ConfigurablePolicyGate {
        decision: PolicyDecision::Allow,
    };

    // 1. Existing target through symlink to outside
    let req_existing = ActionRequest {
        id: "act-symlink-exist".to_string(),
        tool_name: "mutating_tool".to_string(),
        parameters: serde_json::json!({
            "target": "symlink_dir/existing_outside.txt",
            "payload": "data"
        }),
    };
    let res_existing = runner
        .execute_action(&req_existing, &context, &allow_gate, AutonomyMode::Safe)
        .await;
    assert!(!res_existing.success);
    assert!(
        res_existing
            .error
            .unwrap()
            .contains("PATH_OUT_OF_WORKSPACE")
    );

    // 2. Non-existent target through symlink to outside (defense-in-depth ancestor check)
    let req_non_existing = ActionRequest {
        id: "act-symlink-new".to_string(),
        tool_name: "mutating_tool".to_string(),
        parameters: serde_json::json!({
            "target": "symlink_dir/new_file.txt",
            "payload": "data"
        }),
    };
    let res_non_existing = runner
        .execute_action(&req_non_existing, &context, &allow_gate, AutonomyMode::Safe)
        .await;
    assert!(!res_non_existing.success);
    assert!(
        res_non_existing
            .error
            .unwrap()
            .contains("PATH_OUT_OF_WORKSPACE")
    );

    // Mutation counter must remain 0
    assert_eq!(mutation_counter.load(Ordering::SeqCst), 0);
}
