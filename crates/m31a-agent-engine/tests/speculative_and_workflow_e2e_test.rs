//! Comprehensive end-to-end integration tests for Speculative Execution & Autonomous Workflow Orchestration.
//!
//! Validates:
//! 1. Genuine speculative execution: multi-candidate competition, hard gates (empty candidate rejection,
//!    syntax error rejection, working solution selection), deterministic scoring, and atomic promotion/rollback.
//! 2. Autonomous workflow orchestration: plan DAG generation, conflict-free wave partitioning, concurrent execution,
//!    worker claiming leases, stale task reconciliation, append-only audit trail, content-addressed artifacts,
//!    and crash/resume idempotence across orchestrator instances.

use anyhow::Result;
use futures::future::BoxFuture;
use m31a_agent_engine::agents::EngineAgentRuntime;
use m31a_agent_engine::speculative::{
    CandidateExecutionContext, CandidateExecutionResult, CandidateExecutor, SpeculateOptions,
    SpeculativeArena, SpeculativeBranchConfig, SpeculativeCandidateState, SpeculativeFailureReason,
};
use m31a_agent_engine::workflow::{WorkflowExecutionConfig, WorkflowOrchestrator};
use m31a_core::workflow::{WorkflowEventType, WorkflowState, WorkflowTaskStatus};
use m31a_execpolicy::ExecPolicy;
use m31a_provider::NimProvider;
use m31a_provider::ProviderClient;
use m31a_provider::wire_server::{LocalWireServer, WireServerResponse};
use m31a_storage::db::Storage;
use m31a_storage::worktree::{WorktreeManager, WorktreeSettings};
use m31a_tools::ToolRegistry;
use m31a_trust::{TrustLevel, TrustStore};
use std::fs;
use std::path::Path;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tempfile::tempdir;

fn init_git_repo(path: &Path, failing: bool) {
    Command::new("git")
        .args(["init", "-b", "master"])
        .current_dir(path)
        .output()
        .expect("git init");
    Command::new("git")
        .args(["config", "user.name", "M31A E2E Tester"])
        .current_dir(path)
        .output()
        .expect("git config name");
    Command::new("git")
        .args(["config", "user.email", "tester@m31a.local"])
        .current_dir(path)
        .output()
        .expect("git config email");

    // Create a minimal Cargo project
    let cargo_toml = r#"[package]
name = "speculative_e2e"
version = "0.1.0"
edition = "2021"
"#;
    fs::write(path.join("Cargo.toml"), cargo_toml).unwrap();
    fs::create_dir_all(path.join("src")).unwrap();

    let op = if failing { "-" } else { "+" };
    let lib_rs = format!(
        r#"pub fn add(a: i32, b: i32) -> i32 {{
    a {} b
}}

#[cfg(test)]
mod tests {{
    use super::*;

    #[test]
    fn test_add() {{
        assert_eq!(add(2, 2), 4);
    }}
}}
"#,
        op
    );
    fs::write(path.join("src").join("lib.rs"), lib_rs).unwrap();
    fs::write(path.join(".gitignore"), "target/\nartifacts/\n.m31a/\n").unwrap();

    // Generate Cargo.lock so it is committed
    Command::new("cargo")
        .args(["check", "--quiet"])
        .current_dir(path)
        .output()
        .ok();

    Command::new("git")
        .args(["add", "."])
        .current_dir(path)
        .output()
        .expect("git add");
    Command::new("git")
        .args(["commit", "-m", "Initial commit"])
        .current_dir(path)
        .output()
        .expect("git commit");
}

struct MultiHypothesisExecutor {
    candidate_code: std::collections::HashMap<String, Option<String>>,
}

impl CandidateExecutor for MultiHypothesisExecutor {
    fn execute<'a>(
        &'a self,
        ctx: &'a CandidateExecutionContext,
    ) -> BoxFuture<'a, Result<CandidateExecutionResult>> {
        Box::pin(async move {
            let code_opt = self
                .candidate_code
                .get(&ctx.candidate_id)
                .cloned()
                .flatten();
            if let Some(code) = code_opt {
                let target = ctx.workspace_root.join("src").join("lib.rs");
                fs::write(&target, code)?;

                let diff_out = Command::new("git")
                    .args(["diff", "HEAD"])
                    .current_dir(&ctx.workspace_root)
                    .output()?;
                let patch = String::from_utf8_lossy(&diff_out.stdout).to_string();

                Ok(CandidateExecutionResult {
                    success: true,
                    modified_files: vec!["src/lib.rs".to_string()],
                    patch_content: Some(patch),
                    git_commit_sha: None,
                    error: None,
                })
            } else {
                // Empty candidate - no code changes produced
                Ok(CandidateExecutionResult {
                    success: true,
                    modified_files: Vec::new(),
                    patch_content: Some(String::new()),
                    git_commit_sha: None,
                    error: None,
                })
            }
        })
    }
}

#[tokio::test]
async fn test_genuine_speculative_execution_compete_and_promote() {
    let dir = tempdir().unwrap();
    let ws_root = dir.path();
    init_git_repo(ws_root, true);

    // Verify baseline test currently fails
    let initial_test = Command::new("cargo")
        .args(["test", "--quiet"])
        .current_dir(ws_root)
        .output()
        .expect("cargo test");
    assert!(!initial_test.status.success(), "Baseline test must fail");

    // Define 3 competing candidates:
    // 1. Unchanged workspace (empty patch) -> must be rejected by Hard Gate 1 (NoImplementation)
    // 2. Broken syntax -> must be rejected by Hard Gate 2 (BuildFailed)
    // 3. Correct solution -> passes all gates, selected as winner
    let mut code_map = std::collections::HashMap::new();
    code_map.insert("branch-empty".to_string(), None);
    code_map.insert(
        "branch-syntax-err".to_string(),
        Some(r#"pub fn add(a: i32, b: i32) -> i32 { INVALID SYNTAX }"#.to_string()),
    );
    code_map.insert(
        "branch-working".to_string(),
        Some(
            r#"pub fn add(a: i32, b: i32) -> i32 {
    a + b // Fixed implementation
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add() {
        assert_eq!(add(2, 2), 4);
    }
}
"#
            .to_string(),
        ),
    );

    let candidates = vec![
        SpeculativeBranchConfig {
            branch_id: "branch-empty".to_string(),
            name: "Empty Approach".to_string(),
            hypothesis_prompt: "Do nothing".to_string(),
            objective: Some("Fix add function".to_string()),
            model_override: None,
            patch_content: None,
        },
        SpeculativeBranchConfig {
            branch_id: "branch-syntax-err".to_string(),
            name: "Broken Approach".to_string(),
            hypothesis_prompt: "Bad syntax".to_string(),
            objective: Some("Fix add function".to_string()),
            model_override: None,
            patch_content: None,
        },
        SpeculativeBranchConfig {
            branch_id: "branch-working".to_string(),
            name: "Addition Approach".to_string(),
            hypothesis_prompt: "Use standard addition operator".to_string(),
            objective: Some("Fix add function".to_string()),
            model_override: None,
            patch_content: None,
        },
    ];

    let executor = Arc::new(MultiHypothesisExecutor {
        candidate_code: code_map,
    });

    let opts = SpeculateOptions {
        workspace_root: ws_root.to_path_buf(),
        mission: "Fix add function to make tests pass".to_string(),
        candidates,
        timeout_per_candidate: Duration::from_secs(60),
        auto_promote_winner: true,
        require_approval_for_promotion: false,
        max_concurrency: 3,
        executor: Some(executor),
    };

    let report = SpeculativeArena::compete_with_options(opts)
        .await
        .expect("Speculative arena should execute");

    assert_eq!(report.evaluations.len(), 3);

    // Hard Gate 1 Check: Empty candidate was rejected
    let empty_eval = report
        .evaluations
        .iter()
        .find(|e| e.branch_id == "branch-empty")
        .expect("empty eval present");
    assert_eq!(empty_eval.state, SpeculativeCandidateState::Rejected);
    assert_eq!(
        empty_eval.failure_reason,
        SpeculativeFailureReason::NoImplementation
    );
    assert_eq!(empty_eval.composite_score, 0.0);

    // Hard Gate 2 Check: Syntax error candidate failed build
    let broken_eval = report
        .evaluations
        .iter()
        .find(|e| e.branch_id == "branch-syntax-err")
        .expect("broken eval present");
    assert_eq!(broken_eval.state, SpeculativeCandidateState::Failed);
    assert!(
        matches!(
            broken_eval.failure_reason,
            SpeculativeFailureReason::BuildFailed(_)
        ),
        "Expected BuildFailed reason, got {:?}",
        broken_eval.failure_reason
    );
    assert_eq!(broken_eval.composite_score, 0.0);

    // Winning Candidate Check: Working candidate succeeded and was promoted
    let working_eval = report
        .evaluations
        .iter()
        .find(|e| e.branch_id == "branch-working")
        .expect("working eval present");
    assert_eq!(working_eval.state, SpeculativeCandidateState::Eligible);
    assert!(working_eval.hard_gates_passed);
    assert!(working_eval.build_passed);
    assert!(working_eval.tests_passed);
    assert!(working_eval.composite_score > 0.0);

    assert_eq!(report.winning_branch_id.as_deref(), Some("branch-working"));
    assert!(report.integration_report.is_some());
    let integration = report.integration_report.unwrap();
    assert!(integration.post_integration_build_passed);
    assert!(integration.post_integration_tests_passed);
    assert!(!integration.rollback_executed);

    // Verify baseline repository now passes `cargo test` after promotion!
    let post_test = Command::new("cargo")
        .args(["test", "--quiet"])
        .current_dir(ws_root)
        .output()
        .expect("cargo test");
    assert!(
        post_test.status.success(),
        "Promoted workspace tests must now pass!"
    );
}

#[tokio::test]
async fn test_autonomous_workflow_orchestration_full_lifecycle_and_resume() {
    let dir = tempdir().unwrap();
    let ws_root = dir.path();
    init_git_repo(ws_root, false);

    // 0. Spin up LocalWireServer to mock NIM/OpenAI wire protocol for real turn loop
    let server = LocalWireServer::start()
        .await
        .expect("LocalWireServer start");
    let tool_args = serde_json::json!({
        "path": "src/lib.rs",
        "content": r#"pub fn add(a: i32, b: i32) -> i32 {
    a + b
}

pub fn pipeline_stage() -> &'static str {
    "active"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add() {
        assert_eq!(add(2, 2), 4);
    }

    #[test]
    fn test_stage() {
        assert_eq!(pipeline_stage(), "active");
    }
}
"#,
    });
    // Turn 1: Child agent calls write_file to implement the objective
    server.enqueue_response(
        WireServerResponse::text("").with_tool_call("write_file", &tool_args.to_string()),
    );
    // Turn 2: Child agent confirms completion
    server.enqueue_response(WireServerResponse::text(
        "Pipeline implementation complete.",
    ));

    let api_key = "nvapi-runtime-integration-token".to_string();
    let provider = NimProvider::new(
        server.api_base(),
        api_key,
        "nvidia/nemotron-3.5-lightning-30b-a3b".to_string(),
    );
    let client = ProviderClient::from_provider(provider);

    let storage = Storage::open_in_memory().unwrap();
    let store = storage.workflow_store();

    let tools = ToolRegistry::new();
    let exec_policy = ExecPolicy::standard();
    let wt_settings = WorktreeSettings::new(ws_root);
    let wt_mgr = Arc::new(WorktreeManager::new(wt_settings));

    let mut trust_store = TrustStore::new();
    trust_store
        .trust(ws_root, TrustLevel::SessionOnly)
        .expect("trust workspace");

    let runtime = Arc::new(
        EngineAgentRuntime::new(
            client.clone(),
            tools.clone(),
            storage.clone(),
            exec_policy.clone(),
            ws_root.to_path_buf(),
            None,
            128_000,
            3,
        )
        .with_worktree_manager(wt_mgr.clone())
        .with_trust_store(Arc::new(Mutex::new(trust_store)))
        .with_approval_mediator(Arc::new(|_agent_id, _approval| {
            Some(m31a_elicitation::ApprovalDecision::AllowOnce)
        })),
    );

    let orchestrator = WorkflowOrchestrator::new(
        storage.clone(),
        Some(runtime),
        Some(client),
        Some(tools),
        Some(exec_policy),
    )
    .with_worktree_manager(wt_mgr);

    // 1. Create Workflow
    let run = orchestrator
        .create_workflow(
            ws_root,
            "Implement high-performance concurrent processing pipeline",
            "autonomous",
            None,
            Some(WorkflowExecutionConfig {
                max_concurrency: 4,
                task_timeout_seconds: 60,
                verify_post_wave: false,
                allow_speculative_stages: false,
                require_approval_for_integration: false,
                max_replan_attempts: 2,
            }),
        )
        .expect("Create workflow");

    assert_eq!(run.state, WorkflowState::Created);
    let wf_id = run.id.clone();

    // Verify initial audit event
    let events = store.list_events(&wf_id, None).unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].event_type, WorkflowEventType::WorkflowCreated);

    // 2. Generate Plan with Task DAG
    let plan = orchestrator
        .generate_plan(&wf_id)
        .await
        .expect("Plan generation");

    assert!(!plan.tasks.is_empty());
    let waves = plan.partition_into_waves().expect("Wave partitioning");
    assert!(!waves.is_empty(), "Plan must be partitioned into waves");

    // Verify tasks are persisted in store
    let persisted_tasks = store.list_tasks(&wf_id).unwrap();
    assert_eq!(persisted_tasks.len(), plan.tasks.len());

    // 3. Test Worker Lease Claiming & Reconcile Stale Tasks
    let ready_task = store
        .claim_ready_task(&wf_id, "worker-alpha", 5000)
        .unwrap();
    assert!(ready_task.is_some(), "Worker should claim a ready task");
    let claimed = ready_task.unwrap();
    assert_eq!(claimed.status, WorkflowTaskStatus::Claimed);
    assert_eq!(claimed.worker_id.as_deref(), Some("worker-alpha"));
    assert!(claimed.lease_expires_at_epoch_ms.is_some());

    // Test stale lease reconciliation after expiration
    let future_epoch_ms = chrono::Utc::now().timestamp_millis() + 10_000;
    let stale_count = store
        .reconcile_stale_tasks(&wf_id, future_epoch_ms)
        .expect("Reconcile stale tasks");
    assert_eq!(stale_count, 1, "Expired task lease should be reconciled");

    // Task is reset to RetryPending
    let task_after = store.get_task(&wf_id, &claimed.id).unwrap().unwrap();
    assert_eq!(task_after.status, WorkflowTaskStatus::RetryPending);
    assert_eq!(task_after.retry_count, 1);

    // 4. Test Content-Addressed Artifact Storage
    let artifact_data = b"{\"benchmark\": \"simulated_results\", \"latency_p99_ms\": 1.24}";
    let artifact = store
        .store_artifact_file(
            ws_root,
            &wf_id,
            Some(&claimed.id),
            "bench",
            "benchmark_data",
            artifact_data,
            "application/json",
        )
        .expect("Store content-addressed artifact");

    assert_eq!(artifact.artifact_type, "benchmark_data");
    assert_eq!(artifact.size_bytes, artifact_data.len() as u64);
    assert!(!artifact.content_sha256.is_empty());
    assert!(artifact.file_path.exists());

    // 5. Execute Full Workflow with Genuine Child Agent and Verification
    let executed_run = orchestrator
        .execute_workflow(&wf_id)
        .await
        .expect("Execute workflow");
    assert_eq!(
        executed_run.state,
        WorkflowState::Completed,
        "Workflow must complete successfully"
    );

    // Verify all tasks succeeded with actual execution evidence
    let final_tasks = store.list_tasks(&wf_id).unwrap();
    for task in final_tasks {
        assert_eq!(
            task.status,
            WorkflowTaskStatus::Completed,
            "Task {} failed: {:?}",
            task.id,
            task.error_details
        );
        assert!(
            task.execution_result_json.is_some(),
            "Task {} must persist execution evidence",
            task.id
        );
    }

    // 6. Test Crash Resilience & Resume Idempotence across new Orchestrator instance
    let new_orchestrator = WorkflowOrchestrator::new(storage.clone(), None, None, None, None);
    let resumed_run = new_orchestrator
        .resume_workflow(&wf_id)
        .await
        .expect("Resume completed workflow");
    assert_eq!(resumed_run.state, WorkflowState::Completed);

    // Verify final state in database
    let stored_wf = store.get_workflow(&wf_id).unwrap().unwrap();
    assert_eq!(stored_wf.state, WorkflowState::Completed);
    assert!(stored_wf.completed_at_epoch_ms.is_some());
}

#[tokio::test]
async fn test_autonomous_workflow_orchestration_fails_closed_without_runtime() {
    let dir = tempdir().unwrap();
    let ws_root = dir.path();
    init_git_repo(ws_root, false);

    let storage = Storage::open_in_memory().unwrap();
    let orchestrator = WorkflowOrchestrator::new(storage.clone(), None, None, None, None);

    let run = orchestrator
        .create_workflow(
            ws_root,
            "Implement high-performance concurrent processing pipeline",
            "autonomous",
            None,
            Some(WorkflowExecutionConfig {
                max_concurrency: 2,
                task_timeout_seconds: 30,
                verify_post_wave: false,
                allow_speculative_stages: false,
                require_approval_for_integration: false,
                max_replan_attempts: 1,
            }),
        )
        .expect("Create workflow");

    let _plan = orchestrator
        .generate_plan(&run.id)
        .await
        .expect("Plan generation");

    let executed_run = orchestrator
        .execute_workflow(&run.id)
        .await
        .expect("Execute workflow returns result");

    assert_eq!(
        executed_run.state,
        WorkflowState::Failed,
        "Workflow without runtime MUST fail closed"
    );

    // Verify tasks did not falsely succeed
    let tasks = storage.workflow_store().list_tasks(&run.id).unwrap();
    let impl_task = tasks
        .iter()
        .find(|t| t.task_kind == m31a_core::workflow::WorkflowTaskKind::Implementation)
        .expect("Implementation task exists");
    assert_eq!(
        impl_task.status,
        WorkflowTaskStatus::Failed,
        "Implementation task without runtime MUST fail"
    );
    assert!(
        impl_task
            .error_details
            .as_deref()
            .unwrap_or("")
            .contains("Execution runtime (EngineAgentRuntime) is not configured"),
        "Failure reason must explain missing runtime"
    );
}
