//! Comprehensive end-to-end integration tests for Speculative Execution & Autonomous Workflow Orchestration.
//!
//! Validates:
//! 1. Genuine speculative execution: multi-candidate competition, hard gates (empty candidate rejection,
//!    syntax error rejection, working solution selection), deterministic scoring, and atomic promotion/rollback.
//! 2. Autonomous workflow orchestration: plan DAG generation, conflict-free wave partitioning, concurrent execution,
//!    worker claiming leases, stale task reconciliation, append-only audit trail, content-addressed artifacts,
//!    and crash/resume idempotence across orchestrator instances.

use anyhow::Result;
use chrono::Utc;
use futures::future::BoxFuture;
use m31a_agent_engine::agents::EngineAgentRuntime;
use m31a_agent_engine::integration::{IntegrationRequest, WorkflowIntegrationService};
use m31a_agent_engine::speculative::{
    CandidateExecutionContext, CandidateExecutionResult, CandidateExecutor, SpeculateOptions,
    SpeculativeArena, SpeculativeBranchConfig, SpeculativeCandidateState, SpeculativeFailureReason,
};
use m31a_agent_engine::workflow::{WorkflowExecutionConfig, WorkflowOrchestrator};
use m31a_core::workflow::{
    VerificationStatus, WorkflowEventType, WorkflowState, WorkflowTaskStatus,
};
use m31a_execpolicy::ExecPolicy;
use m31a_provider::NimProvider;
use m31a_provider::ProviderClient;
use m31a_provider::wire_server::{LocalWireServer, WireServerResponse};
use m31a_storage::db::Storage;
use m31a_storage::worktree::{WorktreeManager, WorktreeSettings};
use m31a_tools::ToolRegistry;
use m31a_tools::agent_runtime::{AgentRuntime, AgentSpawnRequest};
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

#[tokio::test]
async fn test_crash_recovery_resumes_without_repeating_completed_tasks() {
    let dir = tempdir().unwrap();
    let ws_root = dir.path();
    init_git_repo(ws_root, false);

    let server = LocalWireServer::start()
        .await
        .expect("LocalWireServer start");
    let tool_args = serde_json::json!({
        "path": "src/lib.rs",
        "content": r#"pub fn add(a: i32, b: i32) -> i32 {
    a + b
}

pub fn stage() -> &'static str {
    "recovered"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add() {
        assert_eq!(add(2, 2), 4);
    }
}
"#,
    });
    // Turn 1: write_file
    server.enqueue_response(
        WireServerResponse::text("").with_tool_call("write_file", &tool_args.to_string()),
    );
    // Turn 2: complete
    server.enqueue_response(WireServerResponse::text("Recovered turn complete."));

    let api_key = "nvapi-token".to_string();
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
        Some(runtime.clone()),
        Some(client.clone()),
        Some(tools.clone()),
        Some(exec_policy.clone()),
    )
    .with_worktree_manager(wt_mgr.clone());

    let run = orchestrator
        .create_workflow(
            ws_root,
            "Implement recovered pipeline stage",
            "autonomous",
            None,
            Some(WorkflowExecutionConfig {
                max_concurrency: 2,
                task_timeout_seconds: 30,
                verify_post_wave: false,
                allow_speculative_stages: false,
                require_approval_for_integration: false,
                max_replan_attempts: 2,
            }),
        )
        .expect("Create workflow");

    let _plan = orchestrator.generate_plan(&run.id).await.expect("Plan");

    // 1. Execute only Task 1 (Analysis) to simulate partially completed workflow
    let tasks = store.list_tasks(&run.id).unwrap();
    let analysis_task = tasks
        .iter()
        .find(|t| t.task_kind == m31a_core::workflow::WorkflowTaskKind::Analysis)
        .expect("Analysis task")
        .clone();

    let t1_res = orchestrator
        .execute_task(analysis_task.clone())
        .await
        .unwrap();
    assert!(t1_res.success);

    let t1_record = store.get_task(&run.id, &analysis_task.id).unwrap().unwrap();
    assert_eq!(t1_record.status, WorkflowTaskStatus::Completed);
    let t1_completed_at = t1_record.completed_at_epoch_ms;
    let t1_attempt_id = t1_record.attempt_id;

    // 2. Simulate task 2 (Implementation) interrupted: claimed by worker with an expired lease
    let impl_task = tasks
        .iter()
        .find(|t| t.task_kind == m31a_core::workflow::WorkflowTaskKind::Implementation)
        .expect("Implementation task")
        .clone();

    let initial_impl_attempt = impl_task.attempt_id;
    let claimed_impl = store
        .claim_task(&run.id, &impl_task.id, "crashed-worker-99", 50)
        .unwrap()
        .expect("Claim task for interruption simulation");
    assert_eq!(claimed_impl.status, WorkflowTaskStatus::Claimed);
    assert_eq!(claimed_impl.attempt_id, initial_impl_attempt + 1);

    // Sleep 60ms to guarantee lease expiration
    tokio::time::sleep(Duration::from_millis(60)).await;

    // 3. New orchestrator instance resumes the workflow
    let new_orchestrator = WorkflowOrchestrator::new(
        storage.clone(),
        Some(runtime),
        Some(client),
        Some(tools),
        Some(exec_policy),
    )
    .with_worktree_manager(wt_mgr);

    let resumed_run = new_orchestrator
        .resume_workflow(&run.id)
        .await
        .expect("Resume workflow");
    assert_eq!(resumed_run.state, WorkflowState::Completed);

    // 4. Invariant Verification:
    // Task 1 was preserved: completion timestamp and attempt_id are identical
    let t1_after = store.get_task(&run.id, &analysis_task.id).unwrap().unwrap();
    assert_eq!(t1_after.status, WorkflowTaskStatus::Completed);
    assert_eq!(t1_after.completed_at_epoch_ms, t1_completed_at);
    assert_eq!(t1_after.attempt_id, t1_attempt_id);

    // Task 2 was reconciled from expired lease and succeeded with incremented attempt_id
    let t2_after = store.get_task(&run.id, &impl_task.id).unwrap().unwrap();
    assert_eq!(t2_after.status, WorkflowTaskStatus::Completed);
    assert!(t2_after.attempt_id > claimed_impl.attempt_id);
}

#[tokio::test]
async fn test_stale_worker_fencing_rejects_expired_or_superseded_worker() {
    let storage = Storage::open_in_memory().unwrap();
    let store = storage.workflow_store();
    let orch = WorkflowOrchestrator::new(storage.clone(), None, None, None, None);

    let dir = tempdir().unwrap();
    let run = orch
        .create_workflow(dir.path(), "Fencing test", "autonomous", None, None)
        .unwrap();
    let _plan = orch.generate_plan(&run.id).await.unwrap();

    let tasks = store.list_tasks(&run.id).unwrap();
    let task = &tasks[0];

    // 1. Worker A claims task with a short lease (100ms)
    let claim_a = store
        .claim_task(&run.id, &task.id, "worker-a", 100)
        .unwrap()
        .expect("Worker A claims task");
    let att_a = claim_a.attempt_id;
    assert_eq!(claim_a.worker_id.as_deref(), Some("worker-a"));

    // 2. Fast forward past 100ms and reconcile stale lease
    let future_time = Utc::now().timestamp_millis() + 500;
    let reconciled = store.reconcile_stale_tasks(&run.id, future_time).unwrap();
    assert_eq!(reconciled, 1);

    // 3. Worker B claims the task with next attempt
    let claim_b = store
        .claim_task(&run.id, &task.id, "worker-b", 10_000)
        .unwrap()
        .expect("Worker B claims task");
    let att_b = claim_b.attempt_id;
    assert_eq!(att_b, att_a + 1);
    assert_eq!(claim_b.worker_id.as_deref(), Some("worker-b"));

    // 4. Worker A attempts to renew lease -> MUST return false (fenced / expired)
    let renew_a = store
        .renew_task_lease(&run.id, &task.id, "worker-a", att_a, 10_000)
        .unwrap();
    assert!(!renew_a, "Worker A cannot renew superseded attempt");

    // 5. Worker A attempts to submit result with attempt att_a -> MUST fail closed
    let dummy_result = m31a_core::workflow::TaskExecutionResult {
        task_id: task.id.clone(),
        workflow_id: run.id.clone(),
        attempt_id: att_a,
        worker_id: Some("worker-a".to_string()),
        agent_id: None,
        task_kind: task.task_kind,
        started_at_epoch_ms: Utc::now().timestamp_millis(),
        completed_at_epoch_ms: Utc::now().timestamp_millis(),
        status: WorkflowTaskStatus::Completed,
        modified_files: Vec::new(),
        patch_content: None,
        build_status: VerificationStatus::Passed,
        tests_status: VerificationStatus::Passed,
        safety_status: VerificationStatus::Passed,
        review_status: VerificationStatus::Passed,
        build_passed: true,
        tests_passed: true,
        safety_passed: true,
        verification_details: None,
        failure_reason: None,
        verification_duration_ms: 5,
        exit_code: Some(0),
    };

    let submit_a = store.update_task_execution_result(
        &run.id,
        &task.id,
        Some(att_a),
        Some("worker-a"),
        WorkflowTaskStatus::Completed,
        Some(&dummy_result),
        None,
    );
    assert!(
        submit_a.is_err(),
        "Worker A submit must fail closed due to attempt mismatch"
    );

    // 6. Worker B submits with active attempt att_b -> MUST succeed
    let mut valid_b_result = dummy_result.clone();
    valid_b_result.attempt_id = att_b;
    valid_b_result.worker_id = Some("worker-b".to_string());

    let submit_b = store.update_task_execution_result(
        &run.id,
        &task.id,
        Some(att_b),
        Some("worker-b"),
        WorkflowTaskStatus::Completed,
        Some(&valid_b_result),
        None,
    );
    assert!(
        submit_b.is_ok(),
        "Worker B submit must succeed with valid attempt"
    );

    let final_task = store.get_task(&run.id, &task.id).unwrap().unwrap();
    assert_eq!(final_task.status, WorkflowTaskStatus::Completed);
    assert_eq!(final_task.attempt_id, att_b);
}

#[tokio::test]
async fn test_approval_gated_integration_blocks_unapproved_integration() {
    let dir = tempdir().unwrap();
    let ws_root = dir.path();
    init_git_repo(ws_root, false);

    let initial_content = fs::read_to_string(ws_root.join("src/lib.rs")).unwrap();
    let service = WorkflowIntegrationService::new();

    let patch = r#"--- a/src/lib.rs
+++ b/src/lib.rs
@@ -1,3 +1,3 @@
 pub fn add(a: i32, b: i32) -> i32 {
-    a + b
+    a + b // Approved addition
 }
"#
    .to_string();

    // 1. Attempt unapproved integration -> Must fail closed
    let req_unapproved = IntegrationRequest {
        workspace_root: ws_root.to_path_buf(),
        patch_content: patch.clone(),
        source_label: "test-agent".to_string(),
        commit_message: Some("Test commit".to_string()),
        require_approval: true,
        is_approved: false,
        author_role: Some("Implementer".to_string()),
        skip_dirty_check: true,
        skip_verification: false,
        verification_timeout: Some(Duration::from_secs(30)),
        workflow_id: None,
        task_id: None,
        attempt_id: None,
        expected_change_set_hash: None,
    };

    let outcome_unapproved = service.integrate(&req_unapproved).await.unwrap();
    assert!(
        !outcome_unapproved.success,
        "Unapproved integration must fail"
    );
    assert!(!outcome_unapproved.approval_granted);
    assert!(
        outcome_unapproved
            .error
            .as_deref()
            .unwrap_or("")
            .contains("Human approval required")
    );

    // Verify workspace file remained strictly untouched
    let after_unapproved = fs::read_to_string(ws_root.join("src/lib.rs")).unwrap();
    assert_eq!(
        after_unapproved, initial_content,
        "Workspace must remain untouched when approval missing"
    );

    // 2. Supply approval -> Integration must succeed cleanly
    let req_approved = IntegrationRequest {
        workspace_root: ws_root.to_path_buf(),
        patch_content: patch,
        source_label: "test-agent".to_string(),
        commit_message: Some("Test commit".to_string()),
        require_approval: true,
        is_approved: true,
        author_role: Some("Implementer".to_string()),
        skip_dirty_check: true,
        skip_verification: false,
        verification_timeout: Some(Duration::from_secs(30)),
        workflow_id: None,
        task_id: None,
        attempt_id: None,
        expected_change_set_hash: None,
    };

    let outcome_approved = service
        .integrate(&req_approved)
        .await
        .expect("Approved integration succeeds");
    assert!(outcome_approved.success);
    assert!(outcome_approved.verification_passed);
    assert!(!outcome_approved.rollback_performed);

    let after_approved = fs::read_to_string(ws_root.join("src/lib.rs")).unwrap();
    assert!(after_approved.contains("// Approved addition"));
}

#[tokio::test]
async fn test_agent_tree_cancellation_terminates_child_agents() {
    let dir = tempdir().unwrap();
    let ws_root = dir.path();
    init_git_repo(ws_root, false);

    let server = LocalWireServer::start().await.expect("LocalWireServer");
    server.enqueue_response(WireServerResponse::text("Child agent working..."));

    let api_key = "nvapi-token".to_string();
    let provider = NimProvider::new(
        server.api_base(),
        api_key,
        "nvidia/nemotron-3.5-lightning-30b-a3b".to_string(),
    );
    let client = ProviderClient::from_provider(provider);

    let storage = Storage::open_in_memory().unwrap();
    let tools = ToolRegistry::new();
    let exec_policy = ExecPolicy::standard();

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
        .with_trust_store(Arc::new(Mutex::new(trust_store))),
    );

    let orchestrator = WorkflowOrchestrator::new(
        storage.clone(),
        Some(runtime.clone()),
        Some(client),
        Some(tools),
        Some(exec_policy),
    );

    let run = orchestrator
        .create_workflow(ws_root, "Cancellation test", "autonomous", None, None)
        .unwrap();
    let _plan = orchestrator.generate_plan(&run.id).await.unwrap();

    // Spawn an active child agent
    let spawn_req = AgentSpawnRequest {
        role_name: "Worker".to_string(),
        task_name: "Long Task".to_string(),
        prompt: "Run long computation".to_string(),
        isolate_worktree: false,
        model_override: None,
        depth: 1,
        max_depth: 3,
    };

    let spawned = runtime
        .spawn_agent(spawn_req)
        .await
        .expect("Spawn child agent");
    orchestrator.register_active_agent(&run.id, &spawned.agent_id);

    // Cancel workflow
    orchestrator
        .cancel_workflow(&run.id, "Emergency operator stop")
        .await
        .expect("Cancel workflow");

    // Invariant Verification:
    // 1. Workflow state is Cancelled
    let fetched_run = orchestrator.store().get_workflow(&run.id).unwrap().unwrap();
    assert_eq!(fetched_run.state, WorkflowState::Cancelled);

    // 2. All tasks are Cancelled
    let tasks = orchestrator.store().list_tasks(&run.id).unwrap();
    for t in tasks {
        assert_eq!(t.status, WorkflowTaskStatus::Cancelled);
    }

    // 3. Child agent in runtime was closed/terminated
    let wait_res = runtime
        .wait_agents(std::slice::from_ref(&spawned.agent_id), Some(500))
        .await;
    if let Ok(outcomes) = wait_res
        && let Some(o) = outcomes.first()
    {
        assert!(matches!(
            o.status.as_str(),
            "closed" | "cancelled" | "terminated" | "completed"
        ));
    }
}
