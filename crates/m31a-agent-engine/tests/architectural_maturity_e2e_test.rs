//! Architectural Maturity End-to-End Integration Suite.
//!
//! Validates Integration Paths A through F across the M31A autonomous runtime:
//! - Path A: Engineering execution loop (User Goal -> Engine -> AST -> Tools -> Sandbox -> Verify -> Review -> Complete)
//! - Path B: Model execution with actual NVIDIA NIM provider (Agent Engine -> ProviderClient -> NimProvider)
//! - Path C: Security enforcement (Request -> Authorization -> GitArmor -> ExecPolicy -> Sandbox Fail-Closed)
//! - Path D: Autonomous verification (Requirements -> Criteria -> Execution Evidence -> Independent Verification)
//! - Path E: Multi-agent coordination (Parent -> Child Agents -> Isolated Worktrees -> Merge -> Verification)
//! - Path F: Durable execution (Agent State -> Storage Migration 13 -> Checkpoint -> Interruption -> Recovery)

use anyhow::Result;
use m31a_agent_engine::{AgentEngine, EngineEvent};
use m31a_ast::AstCodebaseIndex;
use m31a_core::types::{ChatMessage, SessionMetadata};
use m31a_core::{ApprovalPolicy, SandboxPolicy};
use m31a_execpolicy::{Decision, ExecPolicy};
use m31a_provider::ProviderClient;
use m31a_review::{ReviewEngine, ReviewVerdict};
use m31a_sandbox::SandboxRunner;
use m31a_storage::{
    Storage, StoredEvidenceRecord, StoredSessionMemory, WorktreeManager, WorktreeSettings,
};
use m31a_tools::ToolRegistry;
use m31a_trust::GitArmor;
use m31a_verify::{
    AgentCompletionReport, AgentPlan, CompletionOutcome, CriterionEvidence, EvidenceRecord,
    EvidenceType, PlanCriterion, VerificationStatus, VerificationTimeline,
};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use tempfile::tempdir;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

// =============================================================================
// Path A: Engineering Execution Loop
// =============================================================================

#[tokio::test]
async fn test_path_a_engineering_execution_pipeline() -> Result<()> {
    let tmp = tempdir()?;
    let workspace = tmp.path().to_path_buf();

    // 1. Initial codebase setup
    let src_dir = workspace.join("src");
    fs::create_dir_all(&src_dir)?;
    let calc_file = src_dir.join("calc.rs");
    fs::write(
        &calc_file,
        "pub fn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n",
    )?;

    // 2. AST Indexing
    let mut ast_index = AstCodebaseIndex::build(&workspace, 100);
    assert_eq!(ast_index.find_definition("add").len(), 1);

    // 3. Tool execution via ToolRegistry under Sandbox
    let registry = ToolRegistry::new();
    let updated_code = r#"
pub fn add(a: i32, b: i32) -> i32 {
    a + b
}

pub fn multiply(a: i32, b: i32) -> i32 {
    a * b
}
"#;
    let write_args = serde_json::json!({
        "path": "src/calc.rs",
        "content": updated_code.trim(),
        "replacement_reason": "Adding multiply operation per engineering task"
    });
    let tool_res = registry
        .execute(
            "write_file",
            &write_args.to_string(),
            &workspace,
            SandboxPolicy::WorkspaceWrite,
        )
        .await?;
    assert!(tool_res.contains("Successfully wrote"));

    // 4. Incremental AST index update
    ast_index.update_file(&calc_file, updated_code);
    assert_eq!(ast_index.find_definition("multiply").len(), 1);

    // 5. Verification Timeline & Evidence Ledger
    let mut timeline = VerificationTimeline::new();
    let plan = AgentPlan {
        goal: "Implement multiply and verify".to_string(),
        steps: vec!["Update code".to_string(), "Run tests".to_string()],
        criteria: vec![
            PlanCriterion {
                id: "CRIT-01".to_string(),
                description: "multiply function exists".to_string(),
            },
            PlanCriterion {
                id: "CRIT-02".to_string(),
                description: "calc tests pass".to_string(),
            },
        ],
        requires_preview: false,
    };
    timeline.record_plan(plan);
    timeline.record_edit(1);
    timeline.record_test_result(2, true, Some("tool-call-test-1"));

    let ev1 = EvidenceRecord {
        requirement_id: "CRIT-01".to_string(),
        evidence_id: "ev-1".to_string(),
        evidence_type: EvidenceType::FileInspection,
        executed_operation: "AST outline verified".to_string(),
        execution_timestamp: 1700000000,
        actual_result: "multiply function present".to_string(),
        success: true,
        relevant_artifacts: vec!["src/calc.rs".to_string()],
        tool_execution_reference: None,
        test_and_diagnostic_references: vec![],
        environment_info: "linux".to_string(),
        verification_limitations: None,
        execution_turn: Some(2),
        exit_code: Some(0),
        tests_passed: None,
        tests_failed: None,
        tests_total: None,
        duration_ms: Some(10),
        status: Some(VerificationStatus::Success),
    };
    let ev2 = EvidenceRecord {
        requirement_id: "CRIT-02".to_string(),
        evidence_id: "ev-2".to_string(),
        evidence_type: EvidenceType::TestExecution,
        executed_operation: "cargo test".to_string(),
        execution_timestamp: 1700000000,
        actual_result: "2 passed".to_string(),
        success: true,
        relevant_artifacts: vec![],
        tool_execution_reference: None,
        test_and_diagnostic_references: vec![],
        environment_info: "linux".to_string(),
        verification_limitations: None,
        execution_turn: Some(2),
        exit_code: Some(0),
        tests_passed: Some(2),
        tests_failed: Some(0),
        tests_total: Some(2),
        duration_ms: Some(100),
        status: Some(VerificationStatus::Success),
    };
    timeline.ledger.record(ev1);
    timeline.ledger.record(ev2);

    let report = AgentCompletionReport {
        outcome: CompletionOutcome::Complete,
        summary: "multiply added and verified".to_string(),
        criteria: vec![
            CriterionEvidence {
                id: "CRIT-01".to_string(),
                evidence: vec!["ev-1".to_string()],
            },
            CriterionEvidence {
                id: "CRIT-02".to_string(),
                evidence: vec!["ev-2".to_string()],
            },
        ],
    };
    assert!(
        timeline.evaluate_completion(&report).is_ok(),
        "Timeline completion must succeed with verified evidence"
    );

    // 6. Code Review against diff
    let diff = r#"diff --git a/src/calc.rs b/src/calc.rs
--- a/src/calc.rs
+++ b/src/calc.rs
@@ -3,1 +3,5 @@
 }
+
+pub fn multiply(a: i32, b: i32) -> i32 {
+    a * b
+}
"#;
    let review_report = ReviewEngine::analyze(diff);
    assert_eq!(review_report.verdict, ReviewVerdict::Approve);

    Ok(())
}

// =============================================================================
// Path B: Model Execution with NVIDIA Provider
// =============================================================================

#[tokio::test]
async fn test_path_b_model_execution_with_nvidia_provider() -> Result<()> {
    let tmp = tempdir()?;
    let workspace = tmp.path().to_path_buf();

    // Start local TCP test service implementing provider wire protocol
    let server = m31a_provider::LocalWireServer::start().await?;
    server
        .enqueue_text("I have inspected the engineering requirements and completed the analysis.");

    let provider = m31a_provider::NimProvider::new(
        server.api_base(),
        "local-test-key".to_string(),
        "meta/llama-3.3-70b-instruct".to_string(),
    );

    let client = ProviderClient::from_provider(provider);
    let storage = Storage::open_in_memory()?;
    let registry = ToolRegistry::new();

    let mut engine = AgentEngine::new(
        client,
        registry,
        storage,
        ApprovalPolicy::Never,
        ExecPolicy::standard(),
        SandboxPolicy::WorkspaceWrite,
        workspace,
    );

    let history = vec![ChatMessage::user("Audit the engineering architecture.")];
    let (event_tx, mut event_rx) = mpsc::unbounded_channel();
    let (_steer_tx, steer_rx) = mpsc::unbounded_channel();
    let cancel = CancellationToken::new();

    let result = engine
        .run_turn(
            "session-model-indep-1",
            history,
            event_tx,
            steer_rx,
            cancel,
            2,
        )
        .await;

    assert!(
        result.is_ok(),
        "Engine failed on generic ProviderClient: {:?}",
        result.err()
    );

    let mut response_text = String::new();
    while let Ok(ev) = event_rx.try_recv() {
        if let EngineEvent::ContentChunk(chunk) = &ev {
            response_text.push_str(chunk);
        }
    }

    assert!(response_text.contains("inspected the engineering requirements"));
    assert!(
        !server.requests().is_empty(),
        "Must exercise actual HTTP transport to wire server"
    );
    Ok(())
}

// =============================================================================
// Path C: Security Enforcement
// =============================================================================

#[tokio::test]
async fn test_path_c_security_enforcement_pipeline() -> Result<()> {
    let tmp = tempdir()?;
    let repo_dir = tmp.path().join("secure_repo");
    fs::create_dir_all(&repo_dir)?;

    // 1. GitArmor repository audit
    let git_dir = repo_dir.join(".git");
    fs::create_dir_all(git_dir.join("hooks"))?;
    let config_file = git_dir.join("config");
    fs::write(&config_file, "[core]\n\thooksPath = /tmp/malicious/hooks\n")?;

    let armor = GitArmor::audit(&repo_dir);
    assert!(
        !armor.is_safe,
        "GitArmor must detect dangerous core.hooksPath override"
    );

    // 2. ExecPolicy command evaluation
    let exec_policy = ExecPolicy::standard();
    assert_eq!(
        exec_policy.evaluate("rm -rf /"),
        Decision::Deny,
        "Critical destructive command must be denied"
    );
    assert_eq!(
        exec_policy.evaluate("mkfs /dev/sda1"),
        Decision::Deny,
        "Critical disk formatter must be denied"
    );
    assert_eq!(
        exec_policy.evaluate_network("169.254.169.254", "http"),
        Decision::Deny,
        "Cloud metadata network probe must be denied"
    );

    // 3. Sandbox Fail-Closed enforcement
    let runner = SandboxRunner::new();
    let write_blocked = runner
        .execute(
            "echo 'forbidden' > /tmp/test_blocked.txt",
            Path::new("/tmp"),
            SandboxPolicy::ReadOnly,
            std::time::Duration::from_secs(5),
        )
        .await;
    // Under ReadOnly, write operations must be blocked by sandbox
    if let Ok(out) = write_blocked {
        assert_ne!(out.exit_code, 0, "Write in ReadOnly sandbox must fail");
    }

    Ok(())
}

// =============================================================================
// Path D: Autonomous Verification & Evidence Ledger
// =============================================================================

#[test]
fn test_path_d_autonomous_verification_and_evidence_ledger() {
    let mut timeline = VerificationTimeline::new();
    let plan = AgentPlan {
        goal: "Test autonomous verification".to_string(),
        steps: vec!["Step 1".to_string()],
        criteria: vec![
            PlanCriterion {
                id: "CRIT-01".to_string(),
                description: "Unit tests pass".to_string(),
            },
            PlanCriterion {
                id: "CRIT-02".to_string(),
                description: "Zero compiler warnings".to_string(),
            },
        ],
        requires_preview: false,
    };
    timeline.record_plan(plan);
    timeline.record_test_result(1, true, Some("tool-call-1"));

    // Fabrication prevention test: Empty evidence must be rejected
    let report_empty = AgentCompletionReport {
        outcome: CompletionOutcome::Complete,
        summary: "done".to_string(),
        criteria: vec![CriterionEvidence {
            id: "CRIT-01".to_string(),
            evidence: vec![],
        }],
    };
    assert!(
        timeline.evaluate_completion(&report_empty).is_err(),
        "Empty evidence must be rejected"
    );

    // Fabricated IDs must be rejected
    let report_fabricated = AgentCompletionReport {
        outcome: CompletionOutcome::Complete,
        summary: "done".to_string(),
        criteria: vec![CriterionEvidence {
            id: "CRIT-01".to_string(),
            evidence: vec!["fabricated_ev_123".to_string()],
        }],
    };
    assert!(
        timeline.evaluate_completion(&report_fabricated).is_err(),
        "Fabricated evidence IDs must be rejected"
    );

    // Legitimate evidence recorded through ledger from actual process executions
    let cargo_proc = std::process::Command::new("cargo")
        .arg("--version")
        .output()
        .expect("cargo --version should execute");
    let cargo_out = String::from_utf8_lossy(&cargo_proc.stdout).to_string();

    let rustc_proc = std::process::Command::new("rustc")
        .arg("--version")
        .output()
        .expect("rustc --version should execute");
    let rustc_out = String::from_utf8_lossy(&rustc_proc.stdout).to_string();

    let ev1 = EvidenceRecord {
        requirement_id: "CRIT-01".to_string(),
        evidence_id: "ev-real-1".to_string(),
        evidence_type: EvidenceType::TestExecution,
        executed_operation: "cargo --version".to_string(),
        execution_timestamp: chrono::Utc::now().timestamp(),
        actual_result: cargo_out,
        success: cargo_proc.status.success(),
        relevant_artifacts: vec![],
        tool_execution_reference: None,
        test_and_diagnostic_references: vec![],
        environment_info: "linux".to_string(),
        verification_limitations: None,
        execution_turn: Some(2),
        exit_code: cargo_proc.status.code(),
        tests_passed: Some(1),
        tests_failed: Some(0),
        tests_total: Some(1),
        duration_ms: Some(10),
        status: Some(VerificationStatus::Success),
    };
    let ev2 = EvidenceRecord {
        requirement_id: "CRIT-02".to_string(),
        evidence_id: "ev-real-2".to_string(),
        evidence_type: EvidenceType::CompilerCheck,
        executed_operation: "rustc --version".to_string(),
        execution_timestamp: chrono::Utc::now().timestamp(),
        actual_result: rustc_out,
        success: rustc_proc.status.success(),
        relevant_artifacts: vec![],
        tool_execution_reference: None,
        test_and_diagnostic_references: vec![],
        environment_info: "linux".to_string(),
        verification_limitations: None,
        execution_turn: Some(2),
        exit_code: rustc_proc.status.code(),
        tests_passed: None,
        tests_failed: None,
        tests_total: None,
        duration_ms: Some(10),
        status: Some(VerificationStatus::Success),
    };
    timeline.ledger.record(ev1);
    timeline.ledger.record(ev2);

    let report_valid = AgentCompletionReport {
        outcome: CompletionOutcome::Complete,
        summary: "verified".to_string(),
        criteria: vec![
            CriterionEvidence {
                id: "CRIT-01".to_string(),
                evidence: vec!["ev-real-1".to_string()],
            },
            CriterionEvidence {
                id: "CRIT-02".to_string(),
                evidence: vec!["ev-real-2".to_string()],
            },
        ],
    };
    assert!(
        timeline.evaluate_completion(&report_valid).is_ok(),
        "Timeline must be complete with legitimate evidence"
    );
}

// =============================================================================
// Path E: Multi-Agent Worktree Isolation
// =============================================================================

#[tokio::test]
async fn test_path_e_multi_agent_worktree_isolation() -> Result<()> {
    let tmp = tempdir()?;
    let workspace = tmp.path().to_path_buf();

    // Initialize real git repo
    std::process::Command::new("git")
        .args(["init"])
        .current_dir(&workspace)
        .output()?;
    std::process::Command::new("git")
        .args(["config", "user.name", "M31A Engineer"])
        .current_dir(&workspace)
        .output()?;
    std::process::Command::new("git")
        .args(["config", "user.email", "m31a@engineer.internal"])
        .current_dir(&workspace)
        .output()?;

    let readme = workspace.join("README.md");
    fs::write(&readme, "# M31A Mainline\n")?;
    std::process::Command::new("git")
        .args(["add", "README.md"])
        .current_dir(&workspace)
        .output()?;
    std::process::Command::new("git")
        .args(["commit", "-m", "Initial commit"])
        .current_dir(&workspace)
        .output()?;

    // Create WorktreeManager
    let wt_manager = Arc::new(WorktreeManager::new(WorktreeSettings::new(&workspace)));
    let child_wt = wt_manager.create_worktree("child-agent-task-1", None)?;
    assert!(child_wt.worktree_path.exists());
    assert_ne!(child_wt.worktree_path, workspace);

    // Child writes in isolated worktree
    let child_file = child_wt.worktree_path.join("CHILD_TASK.txt");
    fs::write(&child_file, "Child Agent Output")?;
    assert!(child_file.exists());

    // Verify parent workspace is unpolluted
    assert!(!workspace.join("CHILD_TASK.txt").exists());

    // Cleanup worktree
    wt_manager.cleanup_worktree("child-agent-task-1")?;
    assert!(!child_wt.worktree_path.exists());

    Ok(())
}

// =============================================================================
// Path F: Durable Execution, Relational Memories & Recovery
// =============================================================================

#[tokio::test]
async fn test_path_f_durable_execution_relational_memories_and_recovery() -> Result<()> {
    let storage = Storage::open_in_memory()?;

    // 1. Migration 13 Evidence Ledger persistence
    let ev = StoredEvidenceRecord {
        id: "ev-persistent-1".to_string(),
        requirement_id: "REQ-DURABLE-01".to_string(),
        evidence_type: "RegressionTest".to_string(),
        description: "Regression suite passed without flakes".to_string(),
        verified: true,
        turn_index: Some(1),
        timestamp_epoch_ms: 1700000000,
        details_json: Some(r#"{"status": "passed"}"#.to_string()),
    };
    storage.store_evidence_record(&ev)?;

    let queried_ev = storage.query_evidence_by_requirement("REQ-DURABLE-01")?;
    assert_eq!(queried_ev.len(), 1);
    assert_eq!(queried_ev[0].id, "ev-persistent-1");
    assert!(queried_ev[0].verified);

    // 2. Migration 13 Session Memories relational persistence
    let mem = StoredSessionMemory {
        id: "mem-relational-1".to_string(),
        topic: "Concurrency Architecture".to_string(),
        detail: "Use advisory flock on memory lockfiles for atomic safety.".to_string(),
        citation_count: 3,
        last_used_at_epoch_ms: Some(1700001000),
        created_at_epoch_ms: 1700000000,
    };
    storage.store_memory(&mem)?;

    let queried_mem = storage.query_memories("advisory flock")?;
    assert_eq!(queried_mem.len(), 1);
    assert_eq!(queried_mem[0].id, "mem-relational-1");

    // 3. Execution claims and crash recovery
    let meta = SessionMetadata {
        id: "session-recovery-test".to_string(),
        title: "Durable Session".to_string(),
        model: "mock".to_string(),
        cwd: "/tmp".to_string(),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        extra: HashMap::new(),
    };
    storage.save_thread(&meta, "/tmp/rollout", "workspace_write", "never")?;

    let claim = storage.create_execution_claim(&meta.id, 1, 99999999, Some("Active turn"))?;
    assert_eq!(claim.status, "running");

    // Simulate crash recovery
    let recovered = storage.recover_crashed_claims()?;
    assert_eq!(recovered.len(), 1);
    assert_eq!(recovered[0].status, "crashed");
    assert!(storage.get_active_claim(&meta.id)?.is_none());

    Ok(())
}
