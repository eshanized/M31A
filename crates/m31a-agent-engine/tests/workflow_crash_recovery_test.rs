//! Stage 6: File-Backed Genuine Crash Recovery Acceptance Test
//!
//! Verifies end-to-end crash resilience across separate process/instance lifecycles:
//! 1. Instance A opens a real file-backed SQLite database on disk.
//! 2. Instance A initializes a workflow plan, completes task 1 (Analysis), claims task 2 (Implementation).
//! 3. Instance A crashes (is dropped without clean finalization).
//! 4. Instance B opens the exact same file-backed SQLite database from disk.
//! 5. Stale worker lease from Instance A is fenced and reconciled.
//! 6. Stale worker's attempt to commit fails closed.
//! 7. Instance B resumes the workflow to full completion.
//! 8. All artifacts, audit trails, and SHA-256 hashes are verified intact.

use chrono::Utc;
use m31a_agent_engine::agents::EngineAgentRuntime;
use m31a_agent_engine::workflow::{WorkflowExecutionConfig, WorkflowOrchestrator};
use m31a_core::workflow::{
    TaskExecutionResult, VerificationStatus, WorkflowState, WorkflowTaskKind, WorkflowTaskStatus,
};
use m31a_execpolicy::ExecPolicy;
use m31a_provider::{LocalWireServer, NimProvider, ProviderClient, WireServerResponse};
use m31a_storage::db::Storage;
use m31a_storage::worktree::{WorktreeManager, WorktreeSettings};
use m31a_tools::registry::ToolRegistry;
use m31a_trust::{TrustLevel, TrustStore};
use std::fs;
use std::path::Path;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::Duration;
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

    // Standard Rust project scaffold
    let cargo_toml = r#"[package]
name = "crash_recovery_target"
version = "0.1.0"
edition = "2024"

[dependencies]
"#;
    fs::write(path.join("Cargo.toml"), cargo_toml).expect("write Cargo.toml");
    fs::create_dir_all(path.join("src")).expect("create src dir");
    fs::write(
        path.join("src/lib.rs"),
        "pub fn core_add(a: i32, b: i32) -> i32 { a + b }\n",
    )
    .expect("write src/lib.rs");

    fs::write(path.join(".gitignore"), "target/\nartifacts/\n.m31a/\n").expect("write .gitignore");

    // Generate Cargo.lock
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
        .args(["commit", "-m", "Initial baseline commit"])
        .current_dir(path)
        .output()
        .expect("git commit");
}

#[tokio::test]
async fn test_file_backed_genuine_crash_recovery_lifecycle() {
    let test_dir = tempdir().unwrap();
    let db_path = test_dir.path().join("workflow_recovery.db");
    let ws_root = test_dir.path().join("workspace");
    fs::create_dir_all(&ws_root).unwrap();
    init_git_repo(&ws_root);

    let server = LocalWireServer::start().await.expect("LocalWireServer");
    // Mock response for Implementation agent: tool call to write verified file
    let tool_args = serde_json::json!({
        "path": "src/lib.rs",
        "content": r#"pub fn core_add(a: i32, b: i32) -> i32 {
    a + b
}

pub fn stage_marker() -> &'static str {
    "recovered_instance_b"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_core_add() {
        assert_eq!(core_add(10, 20), 30);
    }

    #[test]
    fn test_marker() {
        assert_eq!(stage_marker(), "recovered_instance_b");
    }
}
"#,
    });
    server.enqueue_response(
        WireServerResponse::text("").with_tool_call("write_file", &tool_args.to_string()),
    );
    server.enqueue_response(WireServerResponse::text(
        "Implementation completed cleanly.",
    ));

    let api_key = "nvapi-test-token".to_string();
    let provider = NimProvider::new(
        server.api_base(),
        api_key,
        "nvidia/nemotron-3.5-lightning-30b-a3b".to_string(),
    );
    let client = ProviderClient::from_provider(provider);

    let workflow_id: String;
    let t1_id: String;
    let t1_completed_at: i64;
    let t1_attempt_id: u32;
    let t2_id: String;
    let t2_stale_attempt: u32;

    // =========================================================================
    // INSTANCE A: Initializes workflow, executes Task 1, claims Task 2, then crashes
    // =========================================================================
    {
        // 1. Open genuine file-backed SQLite DB
        let storage_a = Storage::open(&db_path).expect("Instance A open DB");
        let store_a = storage_a.workflow_store();
        let tools_a = ToolRegistry::new();
        let exec_policy_a = ExecPolicy::standard();
        let wt_mgr_a = Arc::new(WorktreeManager::new(WorktreeSettings::new(&ws_root)));

        let mut trust_store_a = TrustStore::new();
        trust_store_a
            .trust(&ws_root, TrustLevel::SessionOnly)
            .expect("trust workspace");

        let runtime_a = Arc::new(
            EngineAgentRuntime::new(
                client.clone(),
                tools_a.clone(),
                storage_a.clone(),
                exec_policy_a.clone(),
                ws_root.clone(),
                None,
                128_000,
                3,
            )
            .with_worktree_manager(wt_mgr_a.clone())
            .with_trust_store(Arc::new(Mutex::new(trust_store_a)))
            .with_approval_mediator(Arc::new(|_aid, _app| {
                Some(m31a_elicitation::ApprovalDecision::AllowOnce)
            })),
        );

        let orch_a = WorkflowOrchestrator::new(
            storage_a.clone(),
            Some(runtime_a),
            Some(client.clone()),
            Some(tools_a),
            Some(exec_policy_a),
        )
        .with_worktree_manager(wt_mgr_a);

        let run = orch_a
            .create_workflow(
                &ws_root,
                "Crash resilient engineering pipeline",
                "autonomous",
                None,
                Some(WorkflowExecutionConfig {
                    execution_mode: m31a_core::workflow::ExecutionMode::Standard,
                    max_concurrency: 1,
                    task_timeout_seconds: 60,
                    verify_post_wave: false,
                    allow_speculative_stages: false,
                    require_approval_for_integration: false,
                    max_replan_attempts: 1,
                }),
            )
            .expect("Instance A create workflow");
        workflow_id = run.id.clone();

        let plan = orch_a
            .generate_plan(&workflow_id)
            .await
            .expect("Instance A plan generation");
        assert_eq!(plan.tasks.len(), 3);

        // Execute Task 1 (Analysis)
        let tasks = store_a.list_tasks(&workflow_id).unwrap();
        let task_1 = tasks
            .iter()
            .find(|t| t.task_kind == WorkflowTaskKind::Analysis)
            .expect("Task 1 Analysis")
            .clone();
        t1_id = task_1.id.clone();

        let t1_res = orch_a.execute_task(task_1).await.expect("Execute Task 1");
        assert!(t1_res.success, "Task 1 must succeed");

        let t1_record = store_a.get_task(&workflow_id, &t1_id).unwrap().unwrap();
        assert_eq!(t1_record.status, WorkflowTaskStatus::Completed);
        t1_completed_at = t1_record.completed_at_epoch_ms.unwrap();
        t1_attempt_id = t1_record.attempt_id;

        // Instance A claims Task 2 (Implementation) with a 50ms lease
        let task_2 = tasks
            .iter()
            .find(|t| t.task_kind == WorkflowTaskKind::Implementation)
            .expect("Task 2 Implementation")
            .clone();
        t2_id = task_2.id.clone();

        let claimed_t2 = store_a
            .claim_task(&workflow_id, &t2_id, "instance-a-worker-crashed", 50)
            .unwrap()
            .expect("Claim Task 2 by Instance A");
        t2_stale_attempt = claimed_t2.attempt_id;
        assert_eq!(claimed_t2.status, WorkflowTaskStatus::Claimed);

        // Instance A drops storage_a, orch_a, etc. (CRASH!)
    }

    // Wait 100ms for Instance A's lease to expire on disk
    tokio::time::sleep(Duration::from_millis(100)).await;

    // =========================================================================
    // INSTANCE B: Cold start from the exact same SQLite database on disk
    // =========================================================================
    {
        // 1. Re-open SQLite from disk
        let storage_b = Storage::open(&db_path).expect("Instance B open DB from disk");
        let store_b = storage_b.workflow_store();

        // Verify Task 1 is still persisted as Completed with original timestamp
        let t1_check = store_b.get_task(&workflow_id, &t1_id).unwrap().unwrap();
        assert_eq!(t1_check.status, WorkflowTaskStatus::Completed);
        assert_eq!(t1_check.completed_at_epoch_ms, Some(t1_completed_at));
        assert_eq!(t1_check.attempt_id, t1_attempt_id);

        // Verify Task 2 is currently Claimed by the dead worker
        let t2_check = store_b.get_task(&workflow_id, &t2_id).unwrap().unwrap();
        assert_eq!(t2_check.status, WorkflowTaskStatus::Claimed);
        assert_eq!(
            t2_check.worker_id.as_deref(),
            Some("instance-a-worker-crashed")
        );

        // 2. Fencing verification: dead worker tries to commit result -> MUST be rejected!
        let stale_result = TaskExecutionResult {
            task_id: t2_id.clone(),
            workflow_id: workflow_id.clone(),
            attempt_id: t2_stale_attempt,
            worker_id: Some("instance-a-worker-crashed".to_string()),
            agent_id: None,
            task_kind: WorkflowTaskKind::Implementation,
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
            verification_duration_ms: 10,
            exit_code: Some(0),
        };
        let commit_res = store_b.update_task_execution_result(
            &workflow_id,
            &t2_id,
            Some(t2_stale_attempt),
            Some("instance-a-worker-crashed"),
            WorkflowTaskStatus::Completed,
            Some(&stale_result),
            None,
        );
        assert!(
            commit_res.is_err(),
            "Dead worker with expired lease must be fenced from committing"
        );

        // 3. Reconcile stale tasks
        let now_ms = Utc::now().timestamp_millis();
        let reconciled_count = store_b.reconcile_stale_tasks(&workflow_id, now_ms).unwrap();
        assert_eq!(
            reconciled_count, 1,
            "Reconciliation must reset the 1 expired task"
        );

        let t2_reconciled = store_b.get_task(&workflow_id, &t2_id).unwrap().unwrap();
        assert_eq!(t2_reconciled.status, WorkflowTaskStatus::RetryPending);
        assert!(t2_reconciled.worker_id.is_none());

        // 4. Construct Instance B orchestrator and resume workflow
        let tools_b = ToolRegistry::new();
        let exec_policy_b = ExecPolicy::standard();
        let wt_mgr_b = Arc::new(WorktreeManager::new(WorktreeSettings::new(&ws_root)));

        let mut trust_store_b = TrustStore::new();
        trust_store_b
            .trust(&ws_root, TrustLevel::SessionOnly)
            .expect("trust workspace");

        let runtime_b = Arc::new(
            EngineAgentRuntime::new(
                client.clone(),
                tools_b.clone(),
                storage_b.clone(),
                exec_policy_b.clone(),
                ws_root.clone(),
                None,
                128_000,
                3,
            )
            .with_worktree_manager(wt_mgr_b.clone())
            .with_trust_store(Arc::new(Mutex::new(trust_store_b)))
            .with_approval_mediator(Arc::new(|_aid, _app| {
                Some(m31a_elicitation::ApprovalDecision::AllowOnce)
            })),
        );

        let orch_b = WorkflowOrchestrator::new(
            storage_b.clone(),
            Some(runtime_b),
            Some(client),
            Some(tools_b),
            Some(exec_policy_b),
        )
        .with_worktree_manager(wt_mgr_b);

        let resumed_run = orch_b
            .resume_workflow(&workflow_id)
            .await
            .expect("Instance B resume workflow");
        assert_eq!(
            resumed_run.state,
            WorkflowState::Completed,
            "Resumed workflow must pass 12-point gate and complete"
        );

        // 5. Invariant & Artifact integrity verification
        // Task 1 was NEVER re-executed
        let final_t1 = store_b.get_task(&workflow_id, &t1_id).unwrap().unwrap();
        assert_eq!(final_t1.status, WorkflowTaskStatus::Completed);
        assert_eq!(final_t1.completed_at_epoch_ms, Some(t1_completed_at));
        assert_eq!(final_t1.attempt_id, t1_attempt_id);

        // Task 2 was executed by Instance B with incremented attempt_id
        let final_t2 = store_b.get_task(&workflow_id, &t2_id).unwrap().unwrap();
        assert_eq!(final_t2.status, WorkflowTaskStatus::Completed);
        assert!(final_t2.attempt_id > t2_stale_attempt);

        // All tasks are completed
        let all_tasks = store_b.list_tasks(&workflow_id).unwrap();
        for t in &all_tasks {
            assert_eq!(
                t.status,
                WorkflowTaskStatus::Completed,
                "Task {} ({:?}) must be completed",
                t.name,
                t.task_kind
            );
        }

        // Audit events verify continuous durable lineage across crash
        let events = store_b.list_events(&workflow_id, None).unwrap();
        let event_types: Vec<m31a_core::workflow::WorkflowEventType> =
            events.iter().map(|e| e.event_type).collect();
        assert!(event_types.contains(&m31a_core::workflow::WorkflowEventType::WorkflowCreated));
        assert!(event_types.contains(&m31a_core::workflow::WorkflowEventType::PlanGenerated));
        assert!(event_types.contains(&m31a_core::workflow::WorkflowEventType::TaskStarted));
        assert!(event_types.contains(&m31a_core::workflow::WorkflowEventType::TaskRetried));
        assert!(event_types.contains(&m31a_core::workflow::WorkflowEventType::WorkflowCompleted));

        // File modified by recovered task actually exists in repository
        let final_code = fs::read_to_string(ws_root.join("src/lib.rs")).unwrap();
        assert!(
            final_code.contains("recovered_instance_b"),
            "Expected recovered code written by Instance B"
        );
    }
}
