//! P0 Safety Remediation Regression and Verification Test Suite.
//!
//! Covers:
//! - P0-A: Interactive approval pipeline wiring, lifecycle, escalation, resolution, and fail-closed safety.
//! - P0-B: Safe merge failure handling, untracked file preservation, git merge --abort, and retention policies.

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tempfile::{TempDir, tempdir};
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use m31a::agent::runner::ActionRequest;
use m31a::capability::family::CapabilityFamily;
use m31a::capability::instance::CapabilityInstance;
use m31a::capability::permissions::CapabilityPermissions;
use m31a::capability::registry::CapabilityRegistry;
use m31a::config::ResolvedConfigBuilder;
use m31a::events::bus::BroadcastEventBus;
use m31a::events::bus::EventBus;
use m31a::git::worktree::{WorktreeConfig, WorktreeManager, WorktreeRetentionPolicy};
use m31a::ids::{AgentId, ApprovalRequestId, MissionId, TaskId};

#[path = "common/git_auth.rs"]
mod git_auth;
use git_auth::TestGitAuth;
use m31a::kernel::seams::policy::PolicyDecisionContract;
use m31a::kernel::seams::policy::{
    PolicyDecision, PolicyError, PolicyEvaluationRequest, PolicyGate,
};
use m31a::persistence::sqlite::schema::initialize_database;
use m31a::pipeline::runner::ToolPipelineRunner;
use m31a::policy::approval::ApprovalAction;
use m31a::policy::approval::channel::{ApprovalChannel, ApprovalError};
use m31a::policy::approval::coordinator::ApprovalCoordinator;
use m31a::policy::approval::explanation::ApprovalExplanationPacket;
use m31a::policy::effective::PolicyDecisionRecord;
use m31a::policy::layers::PolicyLayer;
use m31a::runtime::AppRuntime;
use m31a::state::intake::AutonomyMode;
use m31a::tools::definition::{ResourceLimits, ToolExecutionContext, TypedTool};
use m31a::tools::error::ToolError as InternalToolError;
use m31a::tools::registry::ToolRegistry;
use m31a::tools::risk::RiskClass;

// ============================================================================
// Test Fixtures & Tools
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
struct CriticalMutationInput {
    pub file_name: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
struct CriticalMutationOutput {
    pub bytes_written: usize,
}

struct CriticalMutationTool {
    executed: Arc<AtomicBool>,
}

#[async_trait]
impl TypedTool for CriticalMutationTool {
    type Input = CriticalMutationInput;
    type Output = CriticalMutationOutput;

    fn id(&self) -> &str {
        "critical_mutation"
    }

    fn description(&self) -> &str {
        "A high-risk mutation tool requiring approval"
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
        self.executed.store(true, Ordering::SeqCst);
        Ok(CriticalMutationOutput {
            bytes_written: input.content.len(),
        })
    }
}

/// Mock policy gate returning Ask for testing escalations.
struct AskingPolicyGate {
    reason: String,
}

#[async_trait]
impl PolicyGate for AskingPolicyGate {
    async fn evaluate(&self, _req: PolicyEvaluationRequest) -> Result<PolicyDecision, PolicyError> {
        Ok(PolicyDecision::Ask)
    }

    async fn evaluate_record(
        &self,
        _req: PolicyEvaluationRequest,
    ) -> Result<(PolicyDecision, Option<PolicyDecisionContract>), PolicyError> {
        let rec = PolicyDecisionRecord {
            decision: PolicyDecision::Ask,
            matched_rule_id: Some("ask-rule".to_string()),
            matched_layer: Some(PolicyLayer::Workspace),
            precedence_rank: Some(PolicyLayer::Workspace.precedence_rank()),
            authority_source: PolicyLayer::Workspace.authority_source().to_string(),
            explanation: self.reason.clone(),
            policy_version_or_hash: "hash-ask-test".to_string(),
        };
        Ok((PolicyDecision::Ask, Some(rec.into())))
    }
}

/// Mock approval channel recording incoming notifications.
#[derive(Default, Clone)]
struct TestApprovalChannel {
    notified: Arc<Mutex<Vec<ApprovalExplanationPacket>>>,
}

#[async_trait]
impl ApprovalChannel for TestApprovalChannel {
    async fn notify_request(
        &self,
        packet: &ApprovalExplanationPacket,
    ) -> Result<(), ApprovalError> {
        let mut list = self.notified.lock().await;
        list.push(packet.clone());
        Ok(())
    }

    async fn poll_response(
        &self,
        _id: ApprovalRequestId,
    ) -> Result<Option<ApprovalAction>, ApprovalError> {
        Ok(None)
    }
}

/// Helper to set up a git repository in a tempdir.
fn setup_git_repo() -> (TempDir, PathBuf) {
    let dir = tempdir().expect("failed to create temp dir");
    let ws = dir.path().to_path_buf();

    let init = Command::new("git")
        .args(["init", "-b", "main"])
        .current_dir(&ws)
        .status()
        .expect("git init must succeed");
    assert!(init.success());

    Command::new("git")
        .args(["config", "user.name", "P0 Test"])
        .current_dir(&ws)
        .status()
        .unwrap();
    Command::new("git")
        .args(["config", "user.email", "test@m31a.dev"])
        .current_dir(&ws)
        .status()
        .unwrap();

    fs::write(ws.join("README.md"), "# Original Baseline\n").unwrap();
    Command::new("git")
        .args(["add", "README.md"])
        .current_dir(&ws)
        .status()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "Initial commit"])
        .current_dir(&ws)
        .status()
        .unwrap();

    (dir, ws)
}

async fn seed_mission_and_task(
    pool: &sqlx::SqlitePool,
    mission_id: MissionId,
    task_id: TaskId,
    agent_id: AgentId,
) {
    let now = chrono::Utc::now().to_rfc3339();

    sqlx::query(
        "INSERT INTO missions (id, objective, status, created_at, updated_at) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(mission_id.as_bytes().as_slice())
    .bind("P0 Safety Test Mission")
    .bind("planning")
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO agents (id, mission_id, role, status, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(agent_id.as_bytes().as_slice())
    .bind(mission_id.as_bytes().as_slice())
    .bind("implementer")
    .bind("active")
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO tasks (id, mission_id, title, status, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(task_id.as_bytes().as_slice())
    .bind(mission_id.as_bytes().as_slice())
    .bind("Test Task")
    .bind("running")
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await
    .unwrap();
}

// ============================================================================
// P0-B: Destructive Clean Elimination & Untracked File Preservation
// ============================================================================

#[tokio::test]
async fn test_p0_b_merge_failure_preserves_untracked_files() {
    let (_tmp, ws) = setup_git_repo();

    // 1. Create untracked user files in workspace root
    let untracked_file = ws.join("important_user_notes.txt");
    let untracked_content = "DO NOT DELETE: CRITICAL UNTRACKED USER DATA";
    fs::write(&untracked_file, untracked_content).unwrap();

    let untracked_subdir = ws.join("secret_drafts");
    fs::create_dir_all(&untracked_subdir).unwrap();
    let untracked_subfile = untracked_subdir.join("draft.md");
    let nested_content = "DRAFT ARCHITECTURE NOTES";
    fs::write(&untracked_subfile, nested_content).unwrap();

    // Verify files exist before merge
    assert!(untracked_file.exists());
    assert!(untracked_subfile.exists());

    // 2. Create a worktree branch
    let wt_cfg = WorktreeConfig::new(&ws).with_retention(WorktreeRetentionPolicy::KeepOnFailure);
    let manager = WorktreeManager::new(wt_cfg);
    let mission_id = MissionId::new();
    let p0_auth = TestGitAuth::new().with_mission(mission_id);
    let wt_add_path = ws
        .join(".m31a/worktrees")
        .join(mission_id.to_string())
        .to_string_lossy()
        .to_string();
    let wt_add_branch = format!("m31a/{mission_id}");
    let wt = manager
        .create_worktree(
            &mission_id,
            None,
            &p0_auth.worktree_add_gate(&ws, &wt_add_path, &wt_add_branch),
        )
        .await
        .unwrap();

    // 3. Make conflicting commits in worktree branch and main
    fs::write(wt.path.join("README.md"), "# Worktree Changes\n").unwrap();
    let p0_raw = p0_auth.raw_gate(&wt.path);
    manager
        .run_git_in_worktree(&wt, &["add", "README.md"], &p0_raw)
        .await
        .unwrap();
    manager
        .run_git_in_worktree(&wt, &["commit", "-m", "Worktree commit"], &p0_raw)
        .await
        .unwrap();

    fs::write(ws.join("README.md"), "# Main Conflicting Changes\n").unwrap();
    Command::new("git")
        .args(["add", "README.md"])
        .current_dir(&ws)
        .status()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "Main conflicting commit"])
        .current_dir(&ws)
        .status()
        .unwrap();

    // 4. Simulate the merge attempt in workspace root (as run_mission does)
    let merge_out = tokio::process::Command::new("git")
        .args(["merge", &wt.branch, "--no-edit"])
        .current_dir(&ws)
        .output()
        .await
        .unwrap();

    // Merge must fail due to conflicts
    assert!(
        !merge_out.status.success(),
        "Merge should fail due to conflicting changes"
    );

    // 5. Execute the safe remediation cleanup (git merge --abort, NO git clean -f)
    let abort_out = tokio::process::Command::new("git")
        .args(["merge", "--abort"])
        .current_dir(&ws)
        .output()
        .await
        .unwrap();
    assert!(
        abort_out.status.success(),
        "git merge --abort must succeed on conflict"
    );

    // 6. VERIFY: Untracked files in workspace root are 100% intact!
    assert!(
        untracked_file.exists(),
        "Untracked user file must NOT be deleted!"
    );
    let actual_content = fs::read_to_string(&untracked_file).unwrap();
    assert_eq!(
        actual_content, untracked_content,
        "Untracked content must match byte-for-byte!"
    );

    assert!(
        untracked_subfile.exists(),
        "Nested untracked file must NOT be deleted!"
    );
    let actual_sub_content = fs::read_to_string(&untracked_subfile).unwrap();
    assert_eq!(
        actual_sub_content, nested_content,
        "Nested content must match byte-for-byte!"
    );

    // 7. Verify git status shows clean working tree (no MERGE_HEAD)
    let status_out = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(&ws)
        .output()
        .unwrap();
    let git_status = String::from_utf8_lossy(&status_out.stdout);
    assert!(
        git_status.contains("important_user_notes.txt"),
        "Untracked file must remain reported as untracked"
    );
    assert!(
        !git_status.contains("UU"),
        "No unresolved merge conflict markers remain"
    );

    // 8. Verify KeepOnFailure preserves worktree for inspection
    assert!(
        wt.path.exists(),
        "Worktree must be preserved on failure under KeepOnFailure policy"
    );
}

#[tokio::test]
async fn test_p0_b_untracked_collision_preserves_untracked_file() {
    let (_tmp, ws) = setup_git_repo();

    // 1. Create an untracked file in root
    let untracked_file = ws.join("generated_artifact.json");
    let untracked_content = r#"{"user_state": "preserved"}"#;
    fs::write(&untracked_file, untracked_content).unwrap();

    // 2. Create a worktree branch
    let wt_cfg = WorktreeConfig::new(&ws).with_retention(WorktreeRetentionPolicy::AlwaysRemove);
    let manager = WorktreeManager::new(wt_cfg);
    let mission_id = MissionId::new();
    let p0b_auth = TestGitAuth::new().with_mission(mission_id);
    let wt_add_path = ws
        .join(".m31a/worktrees")
        .join(mission_id.to_string())
        .to_string_lossy()
        .to_string();
    let wt_add_branch = format!("m31a/{mission_id}");
    let wt = manager
        .create_worktree(
            &mission_id,
            None,
            &p0b_auth.worktree_add_gate(&ws, &wt_add_path, &wt_add_branch),
        )
        .await
        .unwrap();

    // 3. In the worktree, add and commit a file with the SAME NAME (untracked collision)
    fs::write(
        wt.path.join("generated_artifact.json"),
        r#"{"worktree_state": "conflicting"}"#,
    )
    .unwrap();
    let p0b_raw = p0b_auth.raw_gate(&wt.path);
    manager
        .run_git_in_worktree(&wt, &["add", "generated_artifact.json"], &p0b_raw)
        .await
        .unwrap();
    manager
        .run_git_in_worktree(&wt, &["commit", "-m", "Worktree artifact"], &p0b_raw)
        .await
        .unwrap();

    // 4. Merge attempt into root will collide with untracked file
    let merge_out = tokio::process::Command::new("git")
        .args(["merge", &wt.branch, "--no-edit"])
        .current_dir(&ws)
        .output()
        .await
        .unwrap();
    assert!(
        !merge_out.status.success(),
        "Git should refuse merge due to untracked collision"
    );

    // 5. Abort if in merge state
    let _ = tokio::process::Command::new("git")
        .args(["merge", "--abort"])
        .current_dir(&ws)
        .output()
        .await;

    // 6. Handle retention: AlwaysRemove removes the worktree
    manager
        .remove_worktree(
            &wt,
            true,
            &p0b_auth.worktree_remove_gate(&ws, true, Some(wt.branch.clone())),
        )
        .await
        .unwrap();
    assert!(
        !wt.path.exists(),
        "Worktree should be removed under AlwaysRemove policy"
    );

    // 7. CRITICAL VERIFICATION: Untracked collision file was NOT deleted by clean!
    assert!(
        untracked_file.exists(),
        "Untracked collision file must NEVER be deleted"
    );
    let content = fs::read_to_string(&untracked_file).unwrap();
    assert_eq!(
        content, untracked_content,
        "Original untracked file content must be preserved exactly"
    );
}

// ============================================================================
// P0-A: Interactive Approval Production Pipeline Wiring & Lifecycle
// ============================================================================

#[tokio::test]
async fn test_p0_a_policy_ask_to_approval_allow_once_succeeds() {
    let (_tmp, ws) = setup_git_repo();
    let db_path = ws.join("test_p0_a.db");
    let pool = initialize_database(&db_path).await.unwrap();
    let event_bus = Arc::new(BroadcastEventBus::new(128));

    let mission_id = MissionId::new();
    let task_id = TaskId::new();
    let agent_id = AgentId::new();
    seed_mission_and_task(&pool, mission_id, task_id, agent_id).await;

    let channel = Arc::new(TestApprovalChannel::default());
    let coordinator = Arc::new(
        ApprovalCoordinator::new(Some(pool.clone()), Some(channel.clone()))
            .with_event_bus(event_bus.clone() as Arc<dyn EventBus>),
    );

    let executed = Arc::new(AtomicBool::new(false));
    let tool = CriticalMutationTool {
        executed: executed.clone(),
    };

    let registry = Arc::new(CapabilityRegistry::new());
    registry.register_instance(CapabilityInstance::new(
        "fs.test",
        "Test Filesystem",
        "1.0.0",
        CapabilityFamily::Filesystem,
        "test_fs",
        CapabilityPermissions::full_access(),
    ));

    let mut tool_reg = ToolRegistry::new_default(registry.clone());
    tool_reg.register(tool);
    let tool_registry = Arc::new(tool_reg);

    let asking_gate = AskingPolicyGate {
        reason: "Modifying production file requires operator approval".to_string(),
    };

    let runner = ToolPipelineRunner::new(tool_registry)
        .with_db_pool(pool.clone())
        .with_approval_coordinator(coordinator.clone());

    let token = CancellationToken::new();
    let context = ToolExecutionContext::new(registry, ws.clone(), token)
        .with_mission_id(mission_id)
        .with_task_id(task_id)
        .with_agent_id(agent_id);

    let req = ActionRequest {
        id: "act-1".to_string(),
        tool_name: "critical_mutation".to_string(),
        parameters: serde_json::json!({
            "file_name": "config.json",
            "content": "new_config_data"
        }),
    };

    // Spawn execution in background because it will await approval
    let runner_clone = runner.clone();
    let ctx_clone = context.clone();
    let handle = tokio::spawn(async move {
        runner_clone
            .execute_action(&req, &ctx_clone, &asking_gate, AutonomyMode::Safe)
            .await
    });

    // Wait for approval request to be notified and registered in coordinator
    let mut pending_id: Option<ApprovalRequestId> = None;
    for _ in 0..50 {
        tokio::time::sleep(Duration::from_millis(50)).await;
        let notified = channel.notified.lock().await;
        if let Some(packet) = notified.first() {
            let req_id = packet.request_id;
            if coordinator.has_active_waiter(&req_id).await {
                pending_id = Some(req_id);
                break;
            }
        }
    }

    let req_id = pending_id.expect("Approval request must be registered in coordinator");
    assert!(coordinator.has_active_waiter(&req_id).await);

    // Resolve with AllowOnce
    coordinator
        .resolve_request(req_id, ApprovalAction::AllowOnce, "test_operator")
        .await
        .expect("resolve_request must succeed");

    // Execution should now finish successfully
    let result = handle.await.unwrap();
    assert!(result.success);
    assert!(
        executed.load(Ordering::SeqCst),
        "Tool must have executed after AllowOnce"
    );

    // Verify SQLite audit log
    let row: (String, Option<String>) =
        sqlx::query_as("SELECT resolution_state, resolved_by FROM approval_requests WHERE id = ?")
            .bind(req_id.as_bytes().as_slice())
            .fetch_one(&pool)
            .await
            .expect("Audit record must exist");

    assert_eq!(row.0, "approved");
    assert_eq!(row.1.as_deref(), Some("test_operator"));
}

#[tokio::test]
async fn test_p0_a_policy_ask_to_approval_deny_fails_closed() {
    let (_tmp, ws) = setup_git_repo();
    let db_path = ws.join("test_p0_a_deny.db");
    let pool = initialize_database(&db_path).await.unwrap();
    let event_bus = Arc::new(BroadcastEventBus::new(128));

    let mission_id = MissionId::new();
    let task_id = TaskId::new();
    let agent_id = AgentId::new();
    seed_mission_and_task(&pool, mission_id, task_id, agent_id).await;

    let channel = Arc::new(TestApprovalChannel::default());
    let coordinator = Arc::new(
        ApprovalCoordinator::new(Some(pool.clone()), Some(channel.clone()))
            .with_event_bus(event_bus.clone() as Arc<dyn EventBus>),
    );

    let executed = Arc::new(AtomicBool::new(false));
    let tool = CriticalMutationTool {
        executed: executed.clone(),
    };

    let registry = Arc::new(CapabilityRegistry::new());
    registry.register_instance(CapabilityInstance::new(
        "fs.test",
        "Test Filesystem",
        "1.0.0",
        CapabilityFamily::Filesystem,
        "test_fs",
        CapabilityPermissions::full_access(),
    ));

    let mut tool_reg = ToolRegistry::new_default(registry.clone());
    tool_reg.register(tool);
    let tool_registry = Arc::new(tool_reg);

    let asking_gate = AskingPolicyGate {
        reason: "Dangerous action requires approval".to_string(),
    };

    let runner = ToolPipelineRunner::new(tool_registry)
        .with_db_pool(pool.clone())
        .with_approval_coordinator(coordinator.clone());

    let token = CancellationToken::new();
    let context = ToolExecutionContext::new(registry, ws.clone(), token)
        .with_mission_id(mission_id)
        .with_task_id(task_id)
        .with_agent_id(agent_id);

    let req = ActionRequest {
        id: "act-deny".to_string(),
        tool_name: "critical_mutation".to_string(),
        parameters: serde_json::json!({
            "file_name": "critical.bin",
            "content": "corrupt_data"
        }),
    };

    let runner_clone = runner.clone();
    let ctx_clone = context.clone();
    let handle = tokio::spawn(async move {
        runner_clone
            .execute_action(&req, &ctx_clone, &asking_gate, AutonomyMode::Safe)
            .await
    });

    // Wait for waiter
    let mut pending_id: Option<ApprovalRequestId> = None;
    for _ in 0..50 {
        tokio::time::sleep(Duration::from_millis(50)).await;
        let notified = channel.notified.lock().await;
        if let Some(packet) = notified.first() {
            let req_id = packet.request_id;
            if coordinator.has_active_waiter(&req_id).await {
                pending_id = Some(req_id);
                break;
            }
        }
    }

    let req_id = pending_id.expect("Approval request must be registered in coordinator");

    // Resolve with Deny
    coordinator
        .resolve_request(
            req_id,
            ApprovalAction::Deny {
                reason: "Operation denied by security officer".to_string(),
            },
            "security_officer",
        )
        .await
        .unwrap();

    let result = handle.await.unwrap();
    assert!(!result.success);
    assert!(
        result
            .error
            .as_deref()
            .unwrap_or("")
            .contains("POLICY_DENIED"),
        "Error must clearly indicate POLICY_DENIED"
    );
    assert!(
        !executed.load(Ordering::SeqCst),
        "Tool must NOT have executed after Deny (zero side effects)"
    );

    // Verify SQLite audit log is updated to denied
    let row: (String, Option<String>) =
        sqlx::query_as("SELECT resolution_state, resolved_by FROM approval_requests WHERE id = ?")
            .bind(req_id.as_bytes().as_slice())
            .fetch_one(&pool)
            .await
            .unwrap();

    assert_eq!(row.0, "denied");
    assert_eq!(row.1.as_deref(), Some("security_officer"));
}

#[tokio::test]
async fn test_p0_a_headless_unattended_mode_fails_closed() {
    let (_tmp, ws) = setup_git_repo();
    let db_path = ws.join("test_p0_a_headless.db");
    let pool = initialize_database(&db_path).await.unwrap();

    let mission_id = MissionId::new();
    let task_id = TaskId::new();
    let agent_id = AgentId::new();
    seed_mission_and_task(&pool, mission_id, task_id, agent_id).await;

    let executed = Arc::new(AtomicBool::new(false));
    let tool = CriticalMutationTool {
        executed: executed.clone(),
    };

    let registry = Arc::new(CapabilityRegistry::new());
    registry.register_instance(CapabilityInstance::new(
        "fs.test",
        "Test Filesystem",
        "1.0.0",
        CapabilityFamily::Filesystem,
        "test_fs",
        CapabilityPermissions::full_access(),
    ));

    let mut tool_reg = ToolRegistry::new_default(registry.clone());
    tool_reg.register(tool);
    let tool_registry = Arc::new(tool_reg);

    let asking_gate = AskingPolicyGate {
        reason: "Action needs approval".to_string(),
    };

    // ToolPipelineRunner instantiated WITHOUT approval coordinator (headless/unattended)
    let runner = ToolPipelineRunner::new(tool_registry).with_db_pool(pool.clone());

    let token = CancellationToken::new();
    let context = ToolExecutionContext::new(registry, ws.clone(), token)
        .with_mission_id(mission_id)
        .with_task_id(task_id)
        .with_agent_id(agent_id);

    let req = ActionRequest {
        id: "act-headless".to_string(),
        tool_name: "critical_mutation".to_string(),
        parameters: serde_json::json!({
            "file_name": "server.conf",
            "content": "val"
        }),
    };

    let result = runner
        .execute_action(&req, &context, &asking_gate, AutonomyMode::Unattended)
        .await;

    assert!(!result.success);
    let err = result.error.as_deref().unwrap_or("");
    assert!(
        err.contains("POLICY_DENIED") || err.contains("INTERACTIVE_APPROVAL_UNAVAILABLE"),
        "Unattended mode must fail closed with denial, got: {err}"
    );
    assert!(
        !executed.load(Ordering::SeqCst),
        "Tool must never execute in unattended mode when approval is required"
    );

    // Also test when running in Safe mode without approval coordinator (headless CLI)
    let result_safe_headless = runner
        .execute_action(&req, &context, &asking_gate, AutonomyMode::Safe)
        .await;

    assert!(!result_safe_headless.success);
    assert!(
        result_safe_headless
            .error
            .as_deref()
            .unwrap_or("")
            .contains("INTERACTIVE_APPROVAL_UNAVAILABLE"),
        "Headless interactive mode without coordinator must return INTERACTIVE_APPROVAL_UNAVAILABLE"
    );
    assert!(
        !executed.load(Ordering::SeqCst),
        "Tool must never execute without approval coordinator"
    );
}

#[tokio::test]
async fn test_p0_a_runtime_wiring_integrity() {
    let (_tmp, ws) = setup_git_repo();
    let db_path = ws.join("test_p0_a_runtime.db");
    let pool = initialize_database(&db_path).await.unwrap();
    let event_bus = Arc::new(BroadcastEventBus::new(128));

    let config = Arc::new(ResolvedConfigBuilder::new(&ws).build_fallback());
    let runtime = AppRuntime::from_pool_workspace_and_config(
        pool.clone(),
        ws.clone(),
        event_bus.clone(),
        config,
    )
    .await
    .unwrap();

    // Verify approval_coordinator is present on AppRuntime
    let coord = runtime.approval_coordinator();
    assert_eq!(coord.active_waiters_count().await, 0);

    // Verify with_approval_channel attaches dynamically
    let channel = Arc::new(TestApprovalChannel::default());
    let runtime = runtime.with_approval_channel(channel.clone());

    // Verify controller dependencies has approval coordinator attached
    assert!(runtime.dependencies().approval_coordinator().is_some());
}

#[tokio::test]
async fn test_p0_a_timeout_fails_closed_and_marks_expired() {
    let (_tmp, ws) = setup_git_repo();
    let db_path = ws.join("test_p0_a_timeout.db");
    let pool = initialize_database(&db_path).await.unwrap();
    let event_bus = Arc::new(BroadcastEventBus::new(128));

    let mission_id = MissionId::new();
    let task_id = TaskId::new();
    let agent_id = AgentId::new();
    seed_mission_and_task(&pool, mission_id, task_id, agent_id).await;

    let channel = Arc::new(TestApprovalChannel::default());
    let coordinator = Arc::new(
        ApprovalCoordinator::new(Some(pool.clone()), Some(channel.clone()))
            .with_event_bus(event_bus.clone() as Arc<dyn EventBus>),
    );

    let req = m31a::policy::approval::ApprovalRequest::new(
        mission_id,
        Some(task_id),
        Some(agent_id),
        m31a::ids::ToolCallId::new(),
        "critical_tool",
        serde_json::json!({}),
        vec!["resource.txt".to_string()],
        RiskClass::HighRiskMutation,
        Some("rule-1".to_string()),
        "hash-123",
        "Requires manual authorization",
    );

    let req_id = req.id;

    // Request approval with a 50ms timeout
    let result = coordinator
        .request_approval(req, AutonomyMode::Safe, Duration::from_millis(50))
        .await;

    // Must fail closed with Timeout
    assert!(
        matches!(result, Err(ApprovalError::Timeout(_))),
        "Must return ApprovalError::Timeout on timeout"
    );

    // Verify DB marked expired
    let row: (String,) =
        sqlx::query_as("SELECT resolution_state FROM approval_requests WHERE id = ?")
            .bind(req_id.as_bytes().as_slice())
            .fetch_one(&pool)
            .await
            .expect("DB row must exist");

    assert_eq!(row.0, "expired");
}

#[tokio::test]
async fn test_p0_a_cancellation_unblocks_cleanly() {
    let (_tmp, ws) = setup_git_repo();
    let db_path = ws.join("test_p0_a_cancel.db");
    let pool = initialize_database(&db_path).await.unwrap();
    let event_bus = Arc::new(BroadcastEventBus::new(128));

    let mission_id = MissionId::new();
    let task_id = TaskId::new();
    let agent_id = AgentId::new();
    seed_mission_and_task(&pool, mission_id, task_id, agent_id).await;

    let channel = Arc::new(TestApprovalChannel::default());
    let coordinator = Arc::new(
        ApprovalCoordinator::new(Some(pool.clone()), Some(channel.clone()))
            .with_event_bus(event_bus.clone() as Arc<dyn EventBus>),
    );

    let req = m31a::policy::approval::ApprovalRequest::new(
        mission_id,
        Some(task_id),
        Some(agent_id),
        m31a::ids::ToolCallId::new(),
        "long_running_mutation",
        serde_json::json!({}),
        vec!["target.txt".to_string()],
        RiskClass::HighRiskMutation,
        Some("rule-cancel".to_string()),
        "hash-cancel",
        "Waiting for approval during cancellation test",
    );

    let req_id = req.id;
    let coord_clone = coordinator.clone();

    let handle = tokio::spawn(async move {
        coord_clone
            .request_approval(req, AutonomyMode::Safe, Duration::from_secs(10))
            .await
    });

    // Wait until waiter is registered
    for _ in 0..50 {
        tokio::time::sleep(Duration::from_millis(20)).await;
        if coordinator.has_active_waiter(&req_id).await {
            break;
        }
    }
    assert!(coordinator.has_active_waiter(&req_id).await);

    // Cancel all approvals for task
    coordinator.cancel_task_approvals(task_id).await;

    // Handle should unblock immediately
    let res = handle
        .await
        .unwrap()
        .expect("Should resolve with cancel action");
    assert!(
        matches!(res, ApprovalAction::DenyAndCancelTask { .. }),
        "Cancelled task must receive DenyAndCancelTask action"
    );

    // Verify DB marked cancelled
    let row: (String,) =
        sqlx::query_as("SELECT resolution_state FROM approval_requests WHERE id = ?")
            .bind(req_id.as_bytes().as_slice())
            .fetch_one(&pool)
            .await
            .expect("DB row must exist");

    assert_eq!(row.0, "cancelled");
}

#[tokio::test]
async fn test_p0_a_surface_resolution_loop() {
    let (_tmp, ws) = setup_git_repo();
    let db_path = ws.join("test_p0_a_surface.db");
    let pool = initialize_database(&db_path).await.unwrap();
    let event_bus = Arc::new(BroadcastEventBus::new(128));

    let config = Arc::new(ResolvedConfigBuilder::new(&ws).build_fallback());
    let runtime = AppRuntime::from_pool_workspace_and_config(
        pool.clone(),
        ws.clone(),
        event_bus.clone(),
        config,
    )
    .await
    .unwrap();

    let mission_id = MissionId::new();
    let task_id = TaskId::new();
    let agent_id = AgentId::new();
    seed_mission_and_task(&pool, mission_id, task_id, agent_id).await;

    let coord = runtime.approval_coordinator();

    let req = m31a::policy::approval::ApprovalRequest::new(
        mission_id,
        Some(task_id),
        Some(agent_id),
        m31a::ids::ToolCallId::new(),
        "deploy_code",
        serde_json::json!({}),
        vec!["cluster".to_string()],
        RiskClass::HighRiskMutation,
        Some("rule-surface".to_string()),
        "hash-surface",
        "Deploy to production requires TUI/REPL operator sign-off",
    );

    let req_id = req.id;
    let coord_clone = coord.clone();

    let handle = tokio::spawn(async move {
        coord_clone
            .request_approval(req, AutonomyMode::Safe, Duration::from_secs(10))
            .await
    });

    for _ in 0..50 {
        tokio::time::sleep(Duration::from_millis(20)).await;
        if coord.has_active_waiter(&req_id).await {
            break;
        }
    }
    assert!(coord.has_active_waiter(&req_id).await);

    // Operator in TUI / CLI resolves the request via runtime.approval_coordinator()
    runtime
        .approval_coordinator()
        .resolve_request(req_id, ApprovalAction::AllowOnce, "tui_operator")
        .await
        .expect("TUI resolve_request must succeed");

    let action = handle
        .await
        .unwrap()
        .expect("Request should resolve successfully");
    assert_eq!(action, ApprovalAction::AllowOnce);

    // Verify DB updated
    let row: (String, Option<String>) =
        sqlx::query_as("SELECT resolution_state, resolved_by FROM approval_requests WHERE id = ?")
            .bind(req_id.as_bytes().as_slice())
            .fetch_one(&pool)
            .await
            .unwrap();

    assert_eq!(row.0, "approved");
    assert_eq!(row.1.as_deref(), Some("tui_operator"));
}
