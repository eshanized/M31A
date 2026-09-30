//! Phase 9: Multi-Layer Policy Precedence, Invariants & Stage 7 Tracer Tests (POL-01..POL-04, POL-06).

use std::path::{Path, PathBuf};
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
use m31a::kernel::seams::policy::PolicyDecision;
use m31a::pipeline::runner::ToolPipelineRunner;
use m31a::policy::defaults::{built_in_safety_rules, developer_defaults};
use m31a::policy::effective::EffectivePolicy;
use m31a::policy::file::PolicyFileError;
use m31a::policy::layers::{PolicyLayer, merge_preliminary_decision};
use m31a::policy::matcher::{PolicyEvaluationContext, PolicyMatcher};
use m31a::policy::rule::{CURRENT_POLICY_SCHEMA_VERSION, PolicyDocument, PolicyRule};
use m31a::state::intake::AutonomyMode;
use m31a::state_machine::agent::AgentRole;
use m31a::tools::definition::{ResourceLimits, ToolExecutionContext, TypedTool};
use m31a::tools::error::ToolError as InternalToolError;
use m31a::tools::registry::ToolRegistry;
use m31a::tools::risk::RiskClass;

// ============================================================================
// Test Mock Tools
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
struct FileWriteInput {
    pub path: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
struct FileWriteOutput {
    pub written: bool,
    pub path: String,
}

struct TestWriteFileTool {
    counter: Arc<AtomicU32>,
}

#[async_trait]
impl TypedTool for TestWriteFileTool {
    type Input = FileWriteInput;
    type Output = FileWriteOutput;

    fn id(&self) -> &str {
        "write_file"
    }

    fn description(&self) -> &str {
        "Writes a file in the workspace"
    }

    fn required_capabilities(&self) -> &[CapabilityFamily] {
        &[CapabilityFamily::Filesystem]
    }

    fn base_risk(&self) -> RiskClass {
        RiskClass::HighRiskMutation
    }

    fn resource_limits(&self) -> ResourceLimits {
        ResourceLimits::new(10, 1024)
    }

    async fn execute(
        &self,
        _ctx: &ToolExecutionContext,
        input: Self::Input,
    ) -> Result<Self::Output, InternalToolError> {
        self.counter.fetch_add(1, Ordering::SeqCst);
        Ok(FileWriteOutput {
            written: true,
            path: input.path,
        })
    }
}

// ============================================================================
// Task 09-01-01: Rule Matching, TOML AST & Canonical Path Matching
// ============================================================================

#[test]
fn test_rule_matching() {
    let temp = tempdir().unwrap();
    let workspace = temp.path().to_path_buf();

    // 1. Tool identifier wildcard matching
    assert!(PolicyMatcher::matches_tool("write_file", "write_file"));
    assert!(!PolicyMatcher::matches_tool("write_file", "read_file"));
    assert!(PolicyMatcher::matches_tool("git_*", "git_commit"));
    assert!(PolicyMatcher::matches_tool("git_*", "git_push"));
    assert!(!PolicyMatcher::matches_tool("git_*", "run_command"));
    assert!(PolicyMatcher::matches_tool("fs:*", "fs:read"));
    assert!(PolicyMatcher::matches_tool("fs:*", "fs:write"));
    assert!(!PolicyMatcher::matches_tool("fs:*", "shell:run"));
    assert!(PolicyMatcher::matches_tool("*", "anything"));

    // 2. Canonical path matching and directory traversal prevention
    let safe_file = workspace.join("src/main.rs");
    std::fs::create_dir_all(safe_file.parent().unwrap()).unwrap();
    std::fs::write(&safe_file, "fn main() {}").unwrap();

    // Safe path matches workspace pattern
    assert!(PolicyMatcher::matches_path("./**", &safe_file, &workspace));
    assert!(PolicyMatcher::matches_path(
        "./src/**", &safe_file, &workspace
    ));

    // Traversal escape attempt '../' must fail closed and NOT match ./**
    let escape_traversal = workspace.join("../secret.txt");
    assert!(!PolicyMatcher::matches_path(
        "./**",
        &escape_traversal,
        &workspace
    ));

    let nested_traversal = workspace.join("src/../../secret.txt");
    assert!(!PolicyMatcher::matches_path(
        "./**",
        &nested_traversal,
        &workspace
    ));

    // Non-workspace path
    let etc_passwd = Path::new("/etc/passwd");
    assert!(!PolicyMatcher::matches_path("./**", etc_passwd, &workspace));

    // Sensitive global pattern matching
    assert!(PolicyMatcher::matches_path(
        "**/.ssh/**",
        Path::new("/home/user/.ssh/id_rsa"),
        &workspace
    ));
    assert!(PolicyMatcher::matches_path(
        "**/.env*",
        Path::new("/workspace/.env.production"),
        &workspace
    ));
    assert!(PolicyMatcher::matches_path(
        "**/*id_rsa*",
        Path::new("/opt/keys/id_rsa.pub"),
        &workspace
    ));
    assert!(PolicyMatcher::matches_path(
        "/etc/sudoers*",
        Path::new("/etc/sudoers.d/custom"),
        &workspace
    ));

    // 3. Argument predicate matching
    let commit_pat = serde_json::json!({ "subcommand": "commit" });
    let push_args = serde_json::json!({ "subcommand": "push" });
    let commit_args = serde_json::json!({ "subcommand": "commit", "message": "feat: init" });

    assert!(PolicyMatcher::matches_args(Some(&commit_pat), &commit_args));
    assert!(!PolicyMatcher::matches_args(Some(&commit_pat), &push_args));

    // Destructive shell command predicates
    let destructive_pat = serde_json::json!({
        "command": ["*rm -rf /*", "*git reset --hard*", "*git clean -fdx*"]
    });
    let bad_cmd1 = serde_json::json!({ "command": "sudo rm -rf / --no-preserve-root" });
    let bad_cmd2 = serde_json::json!({ "command": ["bash", "-c", "git reset --hard HEAD~1"] });
    let good_cmd = serde_json::json!({ "command": "cargo check" });

    assert!(PolicyMatcher::matches_args(
        Some(&destructive_pat),
        &bad_cmd1
    ));
    assert!(PolicyMatcher::matches_args(
        Some(&destructive_pat),
        &bad_cmd2
    ));
    assert!(!PolicyMatcher::matches_args(
        Some(&destructive_pat),
        &good_cmd
    ));

    // Null argument handling
    assert!(PolicyMatcher::matches_args(None, &serde_json::Value::Null));
    assert!(PolicyMatcher::matches_args(
        Some(&serde_json::Value::Null),
        &serde_json::Value::Null
    ));

    // 4. TOML serialization & parsing with deny_unknown_fields
    let valid_toml = r#"
version = "1.0"
name = "workspace-policy"

[[rules]]
id = "allow-src-edits"
description = "Allow editing source files"
decision = "allow"
tools = ["write_file", "replace_file_content"]
paths = ["./src/**"]
roles = ["implementer"]
modes = ["autonomous", "assisted"]
"#;

    let doc = PolicyDocument::from_toml_str(valid_toml).expect("valid TOML should parse");
    assert_eq!(doc.version, CURRENT_POLICY_SCHEMA_VERSION);
    assert_eq!(doc.rules.len(), 1);
    assert_eq!(doc.rules[0].id, "allow-src-edits");
    assert_eq!(doc.rules[0].decision, PolicyDecision::Allow);
    assert_eq!(doc.rules[0].roles, vec![AgentRole::implementer()]);
    assert_eq!(
        doc.rules[0].modes,
        vec![AutonomyMode::Autonomous, AutonomyMode::Assisted]
    );

    // Unknown fields must fail closed immediately
    let invalid_toml = r#"
version = "1.0"
unknown_field = "attacker-injected"

[[rules]]
id = "rule-1"
decision = "allow"
"#;
    let err = PolicyDocument::from_toml_str(invalid_toml);
    assert!(err.is_err());
    match err.unwrap_err() {
        PolicyFileError::ParseFailed(msg) => {
            assert!(
                msg.contains("unknown field"),
                "error should report unknown field: {msg}"
            );
        }
        other => panic!("expected ParseFailed, got: {:?}", other),
    }

    // Invalid schema version must fail closed
    let wrong_ver_toml = r#"
version = "99.0"

[[rules]]
id = "rule-1"
decision = "allow"
"#;
    let ver_err = PolicyDocument::from_toml_str(wrong_ver_toml);
    assert!(ver_err.is_err());
    match ver_err.unwrap_err() {
        PolicyFileError::UnsupportedVersion(v) => assert_eq!(v, "99.0"),
        other => panic!("expected UnsupportedVersion, got: {:?}", other),
    }
}

// ============================================================================
// Task 09-01-02: 9-Layer Precedence Hierarchy & Immutable Safety Invariants
// ============================================================================

#[test]
fn test_layer_precedence_and_invariants() {
    let temp = tempdir().unwrap();
    let workspace = temp.path().to_path_buf();

    // Invariant 1 (POL-06, Law 3): BuiltInSafety DENY cannot be weakened by any lower layer
    let built_ins = built_in_safety_rules();
    let cred_rule = built_ins
        .iter()
        .find(|r| r.id == "veto-credential-access")
        .expect("credential veto must exist");

    let context_ssh = PolicyEvaluationContext::new("read_file", workspace.clone())
        .with_target_paths(vec![PathBuf::from("/home/user/.ssh/id_rsa")]);

    assert!(PolicyMatcher::matches_rule(cred_rule, &context_ssh));

    // Construct EffectivePolicy with Layer 0 BuiltInSafety and Layer 3 Workspace trying to ALLOW ssh access
    let workspace_override_rule =
        PolicyRule::new("rogue-allow-ssh", PolicyDecision::Allow).with_paths(["**/.ssh/**"]);

    let policy = EffectivePolicy::builder()
        .with_layer(PolicyLayer::BuiltInSafety, built_in_safety_rules())
        .with_layer(PolicyLayer::Workspace, vec![workspace_override_rule])
        .build();

    let (decision, record) = policy.evaluate_request(&context_ssh);
    assert_eq!(
        decision,
        PolicyDecision::Deny,
        "Built-in safety veto must completely override workspace allow"
    );
    assert_eq!(record.matched_layer, Some(PolicyLayer::BuiltInSafety));
    assert_eq!(
        record.matched_rule_id.as_deref(),
        Some("veto-credential-access")
    );

    // Invariant 2: Mathematical property of merge_preliminary_decision
    // A DENY from any higher layer beats ALLOW or ASK from any lower layer
    let deny_higher = (
        PolicyDecision::Deny,
        PolicyLayer::SystemAdmin,
        "sys-deny".to_string(),
    );
    let allow_lower = (
        PolicyDecision::Allow,
        PolicyLayer::Workspace,
        "ws-allow".to_string(),
    );
    let merged = merge_preliminary_decision(deny_higher.clone(), allow_lower);
    assert_eq!(merged.0, PolicyDecision::Deny);
    assert_eq!(merged.1, PolicyLayer::SystemAdmin);

    // Invariant 3: Lower layer may further restrict (ALLOW -> ASK -> DENY)
    let allow_sys = (
        PolicyDecision::Allow,
        PolicyLayer::SystemAdmin,
        "sys-allow".to_string(),
    );
    let ask_ws = (
        PolicyDecision::Ask,
        PolicyLayer::Workspace,
        "ws-ask".to_string(),
    );
    let merged_restrict = merge_preliminary_decision(allow_sys.clone(), ask_ws);
    assert_eq!(merged_restrict.0, PolicyDecision::Ask);
    assert_eq!(merged_restrict.1, PolicyLayer::Workspace);

    let deny_ws = (
        PolicyDecision::Deny,
        PolicyLayer::Workspace,
        "ws-deny".to_string(),
    );
    let merged_deny = merge_preliminary_decision(allow_sys, deny_ws);
    assert_eq!(merged_deny.0, PolicyDecision::Deny);
    assert_eq!(merged_deny.1, PolicyLayer::Workspace);

    // Invariant 4: SessionApproval (Layer 8) can resolve an ASK
    let ask_higher = (
        PolicyDecision::Ask,
        PolicyLayer::User,
        "user-ask".to_string(),
    );
    let session_grant = (
        PolicyDecision::Allow,
        PolicyLayer::SessionApproval,
        "grant-1".to_string(),
    );
    let resolved = merge_preliminary_decision(ask_higher, session_grant);
    assert_eq!(resolved.0, PolicyDecision::Allow);
    assert_eq!(resolved.1, PolicyLayer::SessionApproval);

    // Invariant 5: SessionApproval (Layer 8) CANNOT override a higher DENY
    let session_grant_vs_deny = merge_preliminary_decision(
        (
            PolicyDecision::Deny,
            PolicyLayer::User,
            "user-deny".to_string(),
        ),
        (
            PolicyDecision::Allow,
            PolicyLayer::SessionApproval,
            "grant-1".to_string(),
        ),
    );
    assert_eq!(session_grant_vs_deny.0, PolicyDecision::Deny);
    assert_eq!(session_grant_vs_deny.1, PolicyLayer::User);
}

// ============================================================================
// Task 09-01-03: POL-04 Developer Defaults
// ============================================================================

#[test]
fn test_developer_defaults() {
    let temp = tempdir().unwrap();
    let workspace = temp.path().to_path_buf();
    let policy = EffectivePolicy::builder()
        .with_layer(PolicyLayer::BuiltInSafety, built_in_safety_rules())
        .with_layer(PolicyLayer::DeveloperDefault, developer_defaults())
        .build();

    // 1. Workspace file write -> ALLOW
    let target_file = workspace.join("src/lib.rs");
    std::fs::create_dir_all(target_file.parent().unwrap()).unwrap();
    std::fs::write(&target_file, "// code").unwrap();

    let ctx_file = PolicyEvaluationContext::new("write_file", workspace.clone())
        .with_target_paths(vec![target_file]);
    let (dec, rec) = policy.evaluate_request(&ctx_file);
    assert_eq!(dec, PolicyDecision::Allow);
    assert_eq!(rec.matched_layer, Some(PolicyLayer::DeveloperDefault));
    assert_eq!(
        rec.matched_rule_id.as_deref(),
        Some("dev-default-workspace-files")
    );

    // 2. Git commit -> ALLOW
    let ctx_commit = PolicyEvaluationContext::new("git", workspace.clone())
        .with_args(serde_json::json!({ "subcommand": "commit", "message": "feat: test" }));
    let (dec, rec) = policy.evaluate_request(&ctx_commit);
    assert_eq!(dec, PolicyDecision::Allow);
    assert_eq!(
        rec.matched_rule_id.as_deref(),
        Some("dev-default-git-commit")
    );

    // 3. Git push -> ASK
    let ctx_push = PolicyEvaluationContext::new("git", workspace.clone())
        .with_args(serde_json::json!({ "subcommand": "push", "remote": "origin" }));
    let (dec, rec) = policy.evaluate_request(&ctx_push);
    assert_eq!(dec, PolicyDecision::Ask);
    assert_eq!(rec.matched_rule_id.as_deref(), Some("dev-default-git-push"));

    // 4. Sandboxed shell -> ALLOW
    let ctx_shell = PolicyEvaluationContext::new("run_command", workspace.clone())
        .with_args(serde_json::json!({ "command": "cargo test" }));
    let (dec, rec) = policy.evaluate_request(&ctx_shell);
    assert_eq!(dec, PolicyDecision::Allow);
    assert_eq!(
        rec.matched_rule_id.as_deref(),
        Some("dev-default-sandboxed-shell")
    );

    // 5. Destructive shell command -> DENY
    let ctx_destructive = PolicyEvaluationContext::new("run_command", workspace.clone())
        .with_args(serde_json::json!({ "command": "rm -rf /" }));
    let (dec, rec) = policy.evaluate_request(&ctx_destructive);
    assert_eq!(dec, PolicyDecision::Deny);
    assert_eq!(
        rec.matched_rule_id.as_deref(),
        Some("dev-default-destructive-ops")
    );

    // 6. Network access -> ASK
    let ctx_network = PolicyEvaluationContext::new("fetch_url", workspace.clone())
        .with_args(serde_json::json!({ "url": "https://api.github.com" }));
    let (dec, rec) = policy.evaluate_request(&ctx_network);
    assert_eq!(dec, PolicyDecision::Ask);
    assert_eq!(rec.matched_rule_id.as_deref(), Some("dev-default-network"));
}

// ============================================================================
// Tracer Slice: Pipeline Stage 7 Integration
// ============================================================================

#[tokio::test]
async fn test_pipeline_stage7_policy_tracer() {
    let temp = tempdir().unwrap();
    let workspace = temp.path().to_path_buf();

    let registry = Arc::new(CapabilityRegistry::new());
    registry.register_instance(CapabilityInstance::new(
        "fs.test",
        "Test Filesystem",
        "1.0.0",
        CapabilityFamily::Filesystem,
        "test_fs",
        CapabilityPermissions::full_access(),
    ));

    let counter = Arc::new(AtomicU32::new(0));
    let mut tool_reg = ToolRegistry::new_default(Arc::clone(&registry));
    tool_reg.register(TestWriteFileTool {
        counter: Arc::clone(&counter),
    });
    let tool_registry = Arc::new(tool_reg);
    let runner = ToolPipelineRunner::new(tool_registry);

    let token = CancellationToken::new();
    let context = ToolExecutionContext::new(registry, workspace.clone(), token)
        .with_mission_id(MissionId::new())
        .with_task_id(TaskId::new());

    let policy_gate = EffectivePolicy::standard(&workspace);

    // 1. Tracer Slice Case A: Valid workspace file write passes Stage 7 with ALLOW
    let safe_file = workspace.join("output.txt");
    let safe_req = ActionRequest {
        id: "act-safe".to_string(),
        tool_name: "write_file".to_string(),
        parameters: serde_json::json!({
            "path": safe_file.to_string_lossy().to_string(),
            "content": "hello world"
        }),
    };

    let result = runner
        .execute_action(&safe_req, &context, &policy_gate, AutonomyMode::Autonomous)
        .await;

    assert!(
        result.success,
        "Safe workspace file write should succeed: {:?}",
        result.error
    );
    assert_eq!(counter.load(Ordering::SeqCst), 1);

    // 2. Tracer Slice Case B: Credential file access inside workspace passes Stage 6 and is DENIED by BuiltInSafety at Stage 7
    let sensitive_file = workspace.join(".ssh/id_rsa");
    let bad_req = ActionRequest {
        id: "act-bad".to_string(),
        tool_name: "write_file".to_string(),
        parameters: serde_json::json!({
            "path": sensitive_file.to_string_lossy().to_string(),
            "content": "tampering with ssh keys"
        }),
    };

    let bad_result = runner
        .execute_action(&bad_req, &context, &policy_gate, AutonomyMode::Autonomous)
        .await;

    assert!(!bad_result.success, "Access to .ssh keys must be denied");
    let err_str = bad_result.error.expect("error should exist");
    assert!(
        err_str.contains("POLICY_DENIED"),
        "error should be POLICY_DENIED: {err_str}"
    );
    // Tool execution stage was never reached for bad_req
    assert_eq!(counter.load(Ordering::SeqCst), 1);
}
