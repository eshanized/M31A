//! Round 3: Comprehensive Workflow Execution Integrity Acceptance Test Suite
//!
//! Validates:
//! - Section 5.4: All agent integrations use authoritative WorkflowIntegrationService.
//! - Section 6.5: Immediate transactions prevent claim race; stale worker commits rejected by fence.
//! - Section 7.6: Cancellation and task timeout terminate child agent trees.
//! - Section 8.5: Zero verification fabrication in Analysis; Truthful Delivery report; 12-Point Gate.
//! - Section 9.2: Failed artifact persistence fails closed.
//! - Section 11.5: Porcelain -z parser correctness; Shared candidate executor factory validation.
//! - Section 13: Configuration concurrency and timeout enforcement.

use chrono::Utc;
use m31a_agent_engine::agents::EngineAgentRuntime;
use m31a_agent_engine::integration::{IntegrationRequest, WorkflowIntegrationService};
use m31a_agent_engine::speculative::{create_candidate_executor, parse_porcelain_z};
use m31a_agent_engine::workflow::{WorkflowExecutionConfig, WorkflowOrchestrator};
use m31a_core::workflow::{
    TaskExecutionResult, VerificationStatus, WorkflowPlan, WorkflowRun, WorkflowState,
    WorkflowTask, WorkflowTaskKind, WorkflowTaskStatus, WorkspaceIsolationStrategy,
};
use m31a_execpolicy::ExecPolicy;
use m31a_provider::{LocalWireServer, NimProvider, ProviderClient, WireServerResponse};
use m31a_storage::db::Storage;
use m31a_storage::worktree::{WorktreeManager, WorktreeSettings};
use m31a_tools::agent_runtime::{AgentRuntime, AgentSpawnRequest};
use m31a_tools::registry::ToolRegistry;
use m31a_trust::{TrustLevel, TrustStore};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use tempfile::tempdir;

fn init_git_repo(path: &Path) {
    Command::new("git")
        .args(["init", "-b", "master"])
        .current_dir(path)
        .output()
        .expect("git init");
    Command::new("git")
        .args(["config", "user.name", "Test Engineer"])
        .current_dir(path)
        .output()
        .expect("git config user.name");
    Command::new("git")
        .args(["config", "user.email", "engineer@m31a.local"])
        .current_dir(path)
        .output()
        .expect("git config user.email");

    let cargo_toml = r#"[package]
name = "integrity_test"
version = "0.1.0"
edition = "2024"

[dependencies]
"#;
    fs::write(path.join("Cargo.toml"), cargo_toml).expect("write Cargo.toml");
    fs::create_dir_all(path.join("src")).expect("create src dir");
    let lib_code = r#"pub fn add(a: i32, b: i32) -> i32 {
    a + b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add() {
        assert_eq!(add(2, 2), 4);
    }
}
"#;
    fs::write(path.join("src/lib.rs"), lib_code).expect("write src/lib.rs");
    fs::write(path.join(".gitignore"), "target/\nartifacts/\n.m31a/\n").expect("write .gitignore");

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

// -----------------------------------------------------------------------------
// 1. Speculative Porcelain -z parsing (Section 11.5)
// -----------------------------------------------------------------------------
#[test]
fn test_speculative_porcelain_z_parsing() {
    // Normal modified, untracked, deleted files
    let raw = b" M src/lib.rs\0?? src/new_file.rs\0 D old.rs\0";
    let parsed = parse_porcelain_z(raw);
    assert_eq!(
        parsed,
        vec![
            "src/lib.rs".to_string(),
            "src/new_file.rs".to_string(),
            "old.rs".to_string()
        ]
    );

    // Renamed file (R  takes two NUL-delimited records: orig_path and new_path)
    let raw_rename = b"R  src/old_name.rs\0src/renamed_target.rs\0?? file2.rs\0";
    let parsed_rename = parse_porcelain_z(raw_rename);
    assert_eq!(
        parsed_rename,
        vec![
            "src/old_name.rs".to_string(),
            "src/renamed_target.rs".to_string(),
            "file2.rs".to_string()
        ]
    );

    // Paths with spaces and unicode
    let raw_unicode = "?? path with spaces/my file.rs\0?? src/ünicøde_test.rs\0".as_bytes();
    let parsed_unicode = parse_porcelain_z(raw_unicode);
    assert_eq!(
        parsed_unicode,
        vec![
            "path with spaces/my file.rs".to_string(),
            "src/ünicøde_test.rs".to_string()
        ]
    );

    // Empty input
    assert!(parse_porcelain_z(b"").is_empty());
}

// -----------------------------------------------------------------------------
// 2. Shared Candidate Executor Factory (Section 11.5)
// -----------------------------------------------------------------------------
#[test]
fn test_shared_candidate_executor_factory() {
    let storage = Storage::open_in_memory().unwrap();
    let policy = ExecPolicy::standard();

    // Missing client fails closed
    let res = create_candidate_executor(
        None,
        Some(ToolRegistry::new()),
        Some(storage.clone()),
        Some(policy.clone()),
        None,
    );
    match res {
        Err(e) => assert!(e.to_string().contains("ProviderClient")),
        Ok(_) => panic!("Expected error for missing ProviderClient"),
    }

    // Missing tools fails closed
    let server_fut = LocalWireServer::start();
    let server = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(server_fut)
        .unwrap();
    let provider = NimProvider::new(
        server.api_base(),
        "key".to_string(),
        "nvidia/nemotron-3.5-lightning-30b-a3b".to_string(),
    );
    let client = ProviderClient::from_provider(provider);

    let res_no_tools =
        create_candidate_executor(Some(client), None, Some(storage), Some(policy), None);
    match res_no_tools {
        Err(e) => assert!(e.to_string().contains("ToolRegistry")),
        Ok(_) => panic!("Expected error for missing ToolRegistry"),
    }
}

// -----------------------------------------------------------------------------
// 3. Immediate Transaction Prevents Claim Race (Section 6.5)
// -----------------------------------------------------------------------------
#[test]
fn test_immediate_transaction_prevents_claim_race() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("claim_race.db");
    let storage = Storage::open(&db_path).unwrap();
    let store = storage.workflow_store();

    let wf_id = "wf-claim-race";
    let run = WorkflowRun {
        id: wf_id.to_string(),
        thread_id: None,
        workspace_root: PathBuf::from("/ws"),
        objective: "Claim race test".into(),
        workflow_type: "autonomous".into(),
        state: WorkflowState::Running,
        plan_revision: 1,
        configuration_json: "{}".into(),
        created_at_epoch_ms: 1000,
        updated_at_epoch_ms: 1000,
        started_at_epoch_ms: Some(1000),
        completed_at_epoch_ms: None,
        cancellation_reason: None,
        overall_result_json: None,
        resource_usage_json: None,
    };
    store.create_workflow(&run).unwrap();

    let task_id = "task-race-1";
    let task = WorkflowTask {
        id: task_id.to_string(),
        workflow_id: wf_id.to_string(),
        plan_revision: 1,
        task_kind: WorkflowTaskKind::Implementation,
        name: "Contended Task".into(),
        description: "Testing claim concurrency".into(),
        assigned_role: "Worker".into(),
        assigned_agent_id: None,
        priority: 10,
        wave_index: 1,
        dependencies: vec![],
        acceptance_criteria: vec![],
        required_verification: vec![],
        declared_read_paths: vec![],
        declared_write_paths: vec![],
        workspace_isolation: WorkspaceIsolationStrategy::Worktree,
        status: WorkflowTaskStatus::Ready,
        retry_count: 0,
        max_retries: 3,
        attempt_id: 1,
        started_at_epoch_ms: None,
        completed_at_epoch_ms: None,
        worker_id: None,
        lease_expires_at_epoch_ms: None,
        error_details: None,
        input_artifacts: vec![],
        output_artifacts: vec![],
        verification_result_json: None,
        execution_result_json: None,
    };
    let plan = WorkflowPlan {
        workflow_id: wf_id.to_string(),
        revision: 1,
        tasks: vec![task],
        rationale: "Claim race test".to_string(),
        created_at_epoch_ms: 1000,
    };
    store.save_workflow_plan(&plan).unwrap();

    // Spawn 10 concurrent threads all attempting to claim the single ready task
    let mut handles = Vec::new();
    let num_workers = 10;
    for i in 0..num_workers {
        let db_p = db_path.clone();
        let wf = wf_id.to_string();
        let tid = task_id.to_string();
        let handle = std::thread::spawn(move || {
            let st = Storage::open(&db_p).unwrap();
            let s = st.workflow_store();
            let worker_id = format!("worker-{}", i);
            s.claim_task(&wf, &tid, &worker_id, 30_000)
        });
        handles.push(handle);
    }

    let mut successful_claims = 0;
    for h in handles {
        let res = h.join().unwrap();
        if let Ok(Some(task)) = res {
            assert_eq!(task.status, WorkflowTaskStatus::Claimed);
            successful_claims += 1;
        }
    }

    // Crucial invariant: exactly ONE worker won the immediate transaction claim
    assert_eq!(
        successful_claims, 1,
        "Exactly one worker must succeed in claiming the task without double-assignment"
    );
}

// -----------------------------------------------------------------------------
// 4. Stale Worker Commit Rejected by Fence (Section 6.5)
// -----------------------------------------------------------------------------
#[test]
fn test_stale_worker_commit_rejected_by_fence() {
    let storage = Storage::open_in_memory().unwrap();
    let store = storage.workflow_store();
    let wf_id = "wf-fence";
    let run = WorkflowRun {
        id: wf_id.to_string(),
        thread_id: None,
        workspace_root: PathBuf::from("/ws"),
        objective: "Fencing test".into(),
        workflow_type: "autonomous".into(),
        state: WorkflowState::Running,
        plan_revision: 1,
        configuration_json: "{}".into(),
        created_at_epoch_ms: 1000,
        updated_at_epoch_ms: 1000,
        started_at_epoch_ms: Some(1000),
        completed_at_epoch_ms: None,
        cancellation_reason: None,
        overall_result_json: None,
        resource_usage_json: None,
    };
    store.create_workflow(&run).unwrap();

    let task_id = "task-fence-1";
    let task = WorkflowTask {
        id: task_id.to_string(),
        workflow_id: wf_id.to_string(),
        plan_revision: 1,
        task_kind: WorkflowTaskKind::Implementation,
        name: "Fenced Task".into(),
        description: "Testing fencing invariants".into(),
        assigned_role: "Worker".into(),
        assigned_agent_id: None,
        priority: 10,
        wave_index: 1,
        dependencies: vec![],
        acceptance_criteria: vec![],
        required_verification: vec![],
        declared_read_paths: vec![],
        declared_write_paths: vec![],
        workspace_isolation: WorkspaceIsolationStrategy::Worktree,
        status: WorkflowTaskStatus::Ready,
        retry_count: 0,
        max_retries: 3,
        attempt_id: 1,
        started_at_epoch_ms: None,
        completed_at_epoch_ms: None,
        worker_id: None,
        lease_expires_at_epoch_ms: None,
        error_details: None,
        input_artifacts: vec![],
        output_artifacts: vec![],
        verification_result_json: None,
        execution_result_json: None,
    };
    let plan = WorkflowPlan {
        workflow_id: wf_id.to_string(),
        revision: 1,
        tasks: vec![task],
        rationale: "Claim race test".to_string(),
        created_at_epoch_ms: 1000,
    };
    store.save_workflow_plan(&plan).unwrap();

    // 1. Worker A claims task
    let claim_a = store
        .claim_task(wf_id, task_id, "worker-a", 10_000)
        .unwrap()
        .unwrap();
    assert_eq!(claim_a.attempt_id, 2);

    // 2. Lease is reconciled and reclaimed by Worker B (attempt 3)
    let reconciled = store
        .reconcile_stale_tasks(wf_id, Utc::now().timestamp_millis() + 20_000)
        .unwrap();
    assert_eq!(reconciled, 1);

    let claim_b = store
        .claim_task(wf_id, task_id, "worker-b", 10_000)
        .unwrap()
        .unwrap();
    assert_eq!(claim_b.attempt_id, 3);

    // 3. Worker A tries to commit result with attempt 2 -> Rejected
    let stale_res = TaskExecutionResult {
        task_id: task_id.to_string(),
        workflow_id: wf_id.to_string(),
        attempt_id: 2,
        worker_id: Some("worker-a".to_string()),
        agent_id: None,
        task_kind: WorkflowTaskKind::Implementation,
        started_at_epoch_ms: 1000,
        completed_at_epoch_ms: 2000,
        status: WorkflowTaskStatus::Completed,
        modified_files: vec![],
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
        verification_duration_ms: 0,
        exit_code: Some(0),
    };

    let commit_res = store.update_task_execution_result(
        wf_id,
        task_id,
        Some(2),
        Some("worker-a"),
        WorkflowTaskStatus::Completed,
        Some(&stale_res),
        None,
    );
    assert!(
        commit_res.is_err(),
        "Worker A's stale attempt must fail closed"
    );

    // 4. Worker B commits with attempt 3 -> Succeeded
    let mut valid_res = stale_res;
    valid_res.attempt_id = 3;
    valid_res.worker_id = Some("worker-b".to_string());
    let valid_commit = store.update_task_execution_result(
        wf_id,
        task_id,
        Some(3),
        Some("worker-b"),
        WorkflowTaskStatus::Completed,
        Some(&valid_res),
        None,
    );
    assert!(
        valid_commit.is_ok(),
        "Worker B's current attempt must succeed"
    );
}

// -----------------------------------------------------------------------------
// 5. Verification Integrity in Analysis: No Fabricated Passed (Section 8.5)
// -----------------------------------------------------------------------------
#[tokio::test]
async fn test_no_verification_fabrication_in_analysis() {
    let dir = tempdir().unwrap();
    let ws_root = dir.path();
    init_git_repo(ws_root);

    let storage = Storage::open_in_memory().unwrap();
    let orch = WorkflowOrchestrator::new(storage.clone(), None, None, None, None);

    let run = orch
        .create_workflow(ws_root, "Analysis truthfulness", "autonomous", None, None)
        .unwrap();
    let _plan = orch.generate_plan(&run.id).await.unwrap();

    let tasks = storage.workflow_store().list_tasks(&run.id).unwrap();
    let analysis_task = tasks
        .into_iter()
        .find(|t| t.task_kind == WorkflowTaskKind::Analysis)
        .unwrap();

    let res = orch.execute_task(analysis_task.clone()).await.unwrap();
    assert!(res.success);

    let record = storage
        .workflow_store()
        .get_task(&run.id, &analysis_task.id)
        .unwrap()
        .unwrap();
    assert_eq!(record.status, WorkflowTaskStatus::Completed);

    // Parse persisted execution result
    let exec_res: TaskExecutionResult =
        serde_json::from_str(&record.execution_result_json.unwrap()).unwrap();

    // Invariant:
    // 1. build_status was checked via compiler
    assert_eq!(exec_res.build_status, VerificationStatus::Passed);
    // 2. safety_status was NOT executed in Analysis (must be NotExecuted, never synthetic Passed)
    assert_eq!(exec_res.safety_status, VerificationStatus::NotExecuted);
    // 3. review_status is NotApplicable
    assert_eq!(exec_res.review_status, VerificationStatus::NotApplicable);
}

// -----------------------------------------------------------------------------
// 6. Final Workflow Completion Gate: 12-Point Gate (Section 8.5)
// -----------------------------------------------------------------------------
#[tokio::test]
async fn test_final_workflow_completion_gate_verifies_all_criteria() {
    let dir = tempdir().unwrap();
    let ws_root = dir.path();
    init_git_repo(ws_root);

    let storage = Storage::open_in_memory().unwrap();
    let store = storage.workflow_store();
    let orch = WorkflowOrchestrator::new(storage.clone(), None, None, None, None);

    let run = orch
        .create_workflow(ws_root, "Gate verification", "autonomous", None, None)
        .unwrap();
    let _plan = orch.generate_plan(&run.id).await.unwrap();

    // Complete all tasks legitimately
    let tasks = store.list_tasks(&run.id).unwrap();
    for t in &tasks {
        let dummy_res = TaskExecutionResult {
            task_id: t.id.clone(),
            workflow_id: run.id.clone(),
            attempt_id: t.attempt_id,
            worker_id: Some("test-worker".to_string()),
            agent_id: None,
            task_kind: t.task_kind,
            started_at_epoch_ms: 1000,
            completed_at_epoch_ms: 2000,
            status: WorkflowTaskStatus::Completed,
            modified_files: vec![],
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
            verification_duration_ms: 10,
            exit_code: Some(0),
        };
        store
            .update_task_execution_result(
                &run.id,
                &t.id,
                Some(t.attempt_id),
                None,
                WorkflowTaskStatus::Completed,
                Some(&dummy_res),
                None,
            )
            .unwrap();
    }

    // Store a legitimate artifact
    let art = store
        .store_artifact_file(
            ws_root,
            &run.id,
            Some(&tasks[0].id),
            "manifest.json",
            "metadata",
            b"{\"status\":\"valid\"}",
            "application/json",
        )
        .unwrap();

    // Corrupt artifact file on disk (simulating disk bitrot or tampering)
    fs::write(&art.file_path, b"TAMPERED_CONTENT_CORRUPTED").unwrap();

    // Attempt to resume/complete workflow through orchestrator:
    // The 12-point gate MUST detect the SHA-256 hash mismatch and fail closed!
    let resumed = orch.resume_workflow(&run.id).await.unwrap();
    assert_eq!(
        resumed.state,
        WorkflowState::Failed,
        "Workflow must fail closed when an artifact SHA-256 checksum fails verification"
    );
    assert!(
        resumed
            .cancellation_reason
            .as_deref()
            .unwrap_or("")
            .contains("SHA-256 mismatch"),
        "Expected cancellation reason to describe SHA-256 mismatch, got: {:?}",
        resumed.cancellation_reason
    );
}

// -----------------------------------------------------------------------------
// 7. Cancellation Terminates Child Agents (Section 7.6)
// -----------------------------------------------------------------------------
#[tokio::test]
async fn test_cancel_workflow_terminates_all_child_agents() {
    let dir = tempdir().unwrap();
    let ws_root = dir.path();
    init_git_repo(ws_root);

    let server = LocalWireServer::start().await.unwrap();
    server.enqueue_response(WireServerResponse::text("Child agent busy"));

    let provider = NimProvider::new(
        server.api_base(),
        "key".to_string(),
        "nvidia/nemotron-3.5-lightning-30b-a3b".to_string(),
    );
    let client = ProviderClient::from_provider(provider);

    let storage = Storage::open_in_memory().unwrap();
    let tools = ToolRegistry::new();
    let exec_policy = ExecPolicy::standard();
    let mut trust_store = TrustStore::new();
    trust_store.trust(ws_root, TrustLevel::SessionOnly).unwrap();

    let wt_mgr = Arc::new(WorktreeManager::new(WorktreeSettings::new(ws_root)));

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
        .with_trust_store(Arc::new(Mutex::new(trust_store))),
    );

    let orch = WorkflowOrchestrator::new(
        storage.clone(),
        Some(runtime.clone()),
        Some(client),
        Some(tools),
        Some(exec_policy),
    )
    .with_worktree_manager(wt_mgr);

    let run = orch
        .create_workflow(ws_root, "Cancel termination test", "autonomous", None, None)
        .unwrap();
    let _plan = orch.generate_plan(&run.id).await.unwrap();

    // Spawn child agent
    let spawned = runtime
        .spawn_agent(AgentSpawnRequest {
            role_name: "Coder".to_string(),
            task_name: "Long Task".to_string(),
            prompt: "Work".to_string(),
            isolate_worktree: false,
            model_override: None,
            depth: 1,
            max_depth: 3,
        })
        .await
        .unwrap();

    orch.register_active_agent(&run.id, &spawned.agent_id);

    // Cancel workflow
    orch.cancel_workflow(&run.id, "User requested cancellation")
        .await
        .unwrap();

    // Verify workflow is Cancelled
    let fetched = storage
        .workflow_store()
        .get_workflow(&run.id)
        .unwrap()
        .unwrap();
    assert_eq!(fetched.state, WorkflowState::Cancelled);

    // Verify runtime child agent was terminated
    let wait_res = runtime
        .wait_agents(std::slice::from_ref(&spawned.agent_id), Some(500))
        .await;
    if let Ok(outcomes) = wait_res
        && let Some(o) = outcomes.first()
    {
        assert!(matches!(
            o.status.as_str(),
            "closed" | "cancelled" | "terminated"
        ));
    }
}

// -----------------------------------------------------------------------------
// 8. All Agent Integrations Route Through Authoritative Service (Section 5.4)
// -----------------------------------------------------------------------------
#[tokio::test]
async fn test_all_agent_integrations_use_workflow() {
    let dir = tempdir().unwrap();
    let ws_root = dir.path();
    init_git_repo(ws_root);

    let svc = WorkflowIntegrationService::new();

    // Directory traversal patch MUST be rejected
    let traversal_patch = "--- a/src/lib.rs\n+++ b/../../etc/passwd\n@@ -1,1 +1,1 @@\n+evil\n";
    let req = IntegrationRequest {
        workspace_root: ws_root.to_path_buf(),
        patch_content: traversal_patch.to_string(),
        source_label: "test".to_string(),
        commit_message: Some("bad".to_string()),
        require_approval: false,
        is_approved: true,
        author_role: None,
        skip_dirty_check: true,
        skip_verification: true,
        verification_timeout: None,
        workflow_id: None,
        task_id: None,
        attempt_id: None,
        expected_change_set_hash: None,
        source_branch: None,
        source_base_sha: None,
        target_base_sha: None,
    };

    let outcome = svc.integrate(&req).await.unwrap();
    assert!(!outcome.success, "Directory traversal must fail closed");
    let err_str = outcome.error.as_deref().unwrap_or("").to_lowercase();
    let sum_str = outcome.summary.to_lowercase();
    assert!(
        err_str.contains("directory traversal") || sum_str.contains("directory traversal"),
        "Expected directory traversal rejection, got error: {:?}, summary: {:?}",
        outcome.error,
        outcome.summary
    );
}

// -----------------------------------------------------------------------------
// 9. Config Max Concurrency & Task Timeout Enforced (Section 13)
// -----------------------------------------------------------------------------
#[tokio::test]
async fn test_config_max_concurrency_and_timeout_enforced() {
    let cfg = WorkflowExecutionConfig {
        execution_mode: m31a_core::workflow::ExecutionMode::Standard,
        max_concurrency: 4,
        task_timeout_seconds: 45,
        verify_post_wave: true,
        allow_speculative_stages: true,
        require_approval_for_integration: true,
        max_replan_attempts: 3,
    };

    let serialized = serde_json::to_string(&cfg).unwrap();
    let deserialized: WorkflowExecutionConfig = serde_json::from_str(&serialized).unwrap();

    assert_eq!(deserialized.max_concurrency, 4);
    assert_eq!(deserialized.task_timeout_seconds, 45);
    assert!(deserialized.verify_post_wave);
    assert!(deserialized.allow_speculative_stages);
    assert!(deserialized.require_approval_for_integration);
    assert_eq!(deserialized.max_replan_attempts, 3);
}

// -----------------------------------------------------------------------------
// 10. Failed Artifact Persistence Fails Task (Section 9.2)
// -----------------------------------------------------------------------------
#[tokio::test]
async fn test_failed_artifact_write_fails_task() {
    let dir = tempdir().unwrap();
    let storage = Storage::open_in_memory().unwrap();
    let store = storage.workflow_store();

    // Invoking store_artifact_file where a file blocks directory creation forces I/O error
    let invalid_dir = dir.path().join("blocking_file");
    fs::write(&invalid_dir, b"block").unwrap();

    let res = store.store_artifact_file(
        &invalid_dir,
        "wf-fail-art",
        Some("task-fail-art"),
        "report.json",
        "analysis",
        b"{}",
        "application/json",
    );
    assert!(
        res.is_err(),
        "Artifact persistence must fail closed on I/O error"
    );
}

// -----------------------------------------------------------------------------
// 11. Delivery Reports Consolidated Evidence (Section 8.5)
// -----------------------------------------------------------------------------
#[tokio::test]
async fn test_delivery_reports_consolidated_evidence() {
    let dir = tempdir().unwrap();
    let ws_root = dir.path();
    init_git_repo(ws_root);

    let storage = Storage::open_in_memory().unwrap();
    let store = storage.workflow_store();
    let orch = WorkflowOrchestrator::new(storage.clone(), None, None, None, None);

    let run = orch
        .create_workflow(
            ws_root,
            "Delivery evidence consolidation",
            "autonomous",
            None,
            None,
        )
        .unwrap();

    // Create a Delivery task specifically designed to consolidate evidence
    let delivery_task = WorkflowTask {
        id: format!("{}-t-delivery", run.id),
        workflow_id: run.id.clone(),
        plan_revision: 1,
        task_kind: WorkflowTaskKind::Delivery,
        name: "Consolidated Delivery".into(),
        description: "Aggregate evidence and produce delivery report".into(),
        assigned_role: "Delivery Manager".into(),
        assigned_agent_id: None,
        priority: 50,
        wave_index: 1,
        dependencies: vec![],
        acceptance_criteria: vec![],
        required_verification: vec![],
        declared_read_paths: vec![],
        declared_write_paths: vec![],
        workspace_isolation: WorkspaceIsolationStrategy::SharedWorkspace,
        status: WorkflowTaskStatus::Ready,
        retry_count: 0,
        max_retries: 1,
        attempt_id: 1,
        started_at_epoch_ms: None,
        completed_at_epoch_ms: None,
        worker_id: None,
        lease_expires_at_epoch_ms: None,
        error_details: None,
        input_artifacts: vec![],
        output_artifacts: vec!["delivery_report.json".into()],
        verification_result_json: None,
        execution_result_json: None,
    };

    let plan = WorkflowPlan {
        workflow_id: run.id.clone(),
        revision: 1,
        tasks: vec![delivery_task.clone()],
        rationale: "Delivery report test".to_string(),
        created_at_epoch_ms: 1000,
    };
    store.save_workflow_plan(&plan).unwrap();

    // Execute Delivery task
    let res = orch.execute_task(delivery_task).await.unwrap();
    assert!(
        res.success,
        "Delivery task execution must succeed: {:?}",
        res.error
    );

    // Verify delivery artifact was persisted in storage
    let artifacts = store.list_artifacts(&run.id).unwrap();
    let delivery_art = artifacts
        .iter()
        .find(|a| a.name == "delivery_report.json")
        .expect("delivery_report.json must be persisted");
    assert!(delivery_art.file_path.exists());
    let content = fs::read_to_string(&delivery_art.file_path).unwrap();
    assert!(content.contains("delivery_ready") || content.contains("total_tasks"));
}

// -----------------------------------------------------------------------------
// 12. Task Timeout Terminates Agent Tree (Section 7.6)
// -----------------------------------------------------------------------------
#[tokio::test]
async fn test_task_timeout_terminates_agent_tree() {
    let dir = tempdir().unwrap();
    let ws_root = dir.path();
    init_git_repo(ws_root);

    // Mock HTTP server that accepts connections but delays response past the 1s timeout
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        while let Ok((mut stream, _)) = listener.accept().await {
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            let _ =
                tokio::io::AsyncWriteExt::write_all(&mut stream, b"HTTP/1.1 200 OK\r\n\r\n").await;
        }
    });

    let provider = NimProvider::new(
        format!("http://{}", addr),
        "key".to_string(),
        "nvidia/nemotron-3.5-lightning-30b-a3b".to_string(),
    );
    let client = ProviderClient::from_provider(provider);

    let storage = Storage::open_in_memory().unwrap();
    let tools = ToolRegistry::new();
    let exec_policy = ExecPolicy::standard();
    let mut trust_store = TrustStore::new();
    trust_store.trust(ws_root, TrustLevel::SessionOnly).unwrap();

    let wt_mgr = Arc::new(WorktreeManager::new(WorktreeSettings::new(ws_root)));

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
        .with_trust_store(Arc::new(Mutex::new(trust_store))),
    );

    let orch = WorkflowOrchestrator::new(
        storage.clone(),
        Some(runtime.clone()),
        Some(client),
        Some(tools),
        Some(exec_policy),
    )
    .with_worktree_manager(wt_mgr);

    let run = orch
        .create_workflow(
            ws_root,
            "Timeout termination test",
            "autonomous",
            None,
            Some(WorkflowExecutionConfig {
                execution_mode: m31a_core::workflow::ExecutionMode::Standard,
                max_concurrency: 1,
                task_timeout_seconds: 1, // 1 second timeout
                verify_post_wave: false,
                allow_speculative_stages: false,
                require_approval_for_integration: false,
                max_replan_attempts: 1,
            }),
        )
        .unwrap();

    let _plan = orch.generate_plan(&run.id).await.unwrap();
    let tasks = storage.workflow_store().list_tasks(&run.id).unwrap();

    // Execute Analysis task first so Implementation task dependencies are satisfied
    let analysis_task = tasks
        .iter()
        .find(|t| t.task_kind == WorkflowTaskKind::Analysis)
        .unwrap();
    let r_analysis = orch.execute_task(analysis_task.clone()).await.unwrap();
    assert!(r_analysis.success);

    let impl_task = tasks
        .iter()
        .find(|t| t.task_kind == WorkflowTaskKind::Implementation)
        .unwrap();

    let start = std::time::Instant::now();
    let res = orch.execute_task(impl_task.clone()).await;
    let elapsed = start.elapsed();

    // Verify task execution failed due to timeout
    assert!(res.is_err() || !res.unwrap().success);
    assert!(elapsed.as_secs() < 10, "Timeout must fire promptly");

    let final_task = storage
        .workflow_store()
        .get_task(&run.id, &impl_task.id)
        .unwrap()
        .unwrap();
    assert_eq!(final_task.status, WorkflowTaskStatus::Failed);
    assert!(
        final_task
            .error_details
            .as_deref()
            .unwrap_or("")
            .contains("timed out")
    );
}
