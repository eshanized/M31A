//! Genuine Demand-Driven Coordination & Worktree-First Integration Test Suite
//!
//! Exercises all 13 critical architectural guarantees specified for M31A's
//! autonomous multi-agent execution architecture:
//!
//! 1. Independent agents modifying identical relative paths execute concurrently in separate worktrees.
//! 2. Overlapping modifications are detected during integration.
//! 3. Shared-workspace overlapping writes are safely serialized.
//! 4. Agents discover conflicts and create dependent tasks via dynamic replanning.
//! 5. DAG revisions persist across process restarts in file-backed SQLite.
//! 6. Cyclic dependencies are rejected.
//! 7. Dynamically spawned agents cannot bypass ownership policies.
//! 8. Expired workers cannot publish stale results.
//! 9. Integration retries are idempotent.
//! 10. Interrupted worktrees recover without losing pending changes.
//! 11. Cancellation does not destroy unreconciled artifacts.
//! 12. Independent tasks are not unnecessarily serialized into consecutive waves.
//! 13. Verification failures prevent promotion.

use chrono::Utc;
use m31a_agent_engine::integration::{IntegrationRequest, WorkflowIntegrationService};
use m31a_agent_engine::workflow::{WorkflowExecutionConfig, WorkflowOrchestrator};
use m31a_core::workflow::{
    ConflictKind, ConflictReport, ConflictResolution, IntegrationLifecycleStatus, WorkflowPlan,
    WorkflowRun, WorkflowState, WorkflowTask, WorkflowTaskKind, WorkflowTaskStatus,
    WorkspaceIsolationStrategy, WorktreeStatus,
};
use m31a_execpolicy::ExecPolicy;
use m31a_storage::db::Storage;
use m31a_storage::worktree::{WorktreeManager, WorktreeSettings};
use m31a_tools::registry::ToolRegistry;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::Duration;
use tempfile::tempdir;
use uuid::Uuid;

/// Helper: Initializes a genuine Git repository with initial commit and returns HEAD SHA.
fn init_git_repo(path: &Path) -> String {
    let run_cmd = |args: &[&str]| {
        let out = Command::new("git")
            .args(args)
            .current_dir(path)
            .output()
            .expect("git execution");
        if !out.status.success() {
            panic!(
                "git command {:?} failed: {}",
                args,
                String::from_utf8_lossy(&out.stderr)
            );
        }
        out
    };

    run_cmd(&["init", "-b", "master"]);
    run_cmd(&["config", "user.name", "M31A Architect"]);
    run_cmd(&["config", "user.email", "architect@m31a.local"]);

    // Create minimal valid Rust workspace scaffold
    let cargo_toml = r#"[package]
name = "m31a_test_fixture"
version = "0.1.0"
edition = "2024"

[dependencies]
"#;
    fs::write(path.join("Cargo.toml"), cargo_toml).expect("write Cargo.toml");
    fs::create_dir_all(path.join("src")).expect("mkdir src");
    fs::write(
        path.join("src/lib.rs"),
        "pub fn base_function() -> i32 { 42 }\n",
    )
    .expect("write src/lib.rs");

    run_cmd(&["add", "."]);
    run_cmd(&["commit", "-m", "chore: initial scaffold"]);

    let head_out = run_cmd(&["rev-parse", "HEAD"]);
    String::from_utf8_lossy(&head_out.stdout).trim().to_string()
}

fn make_test_run(wf_id: &str, root: &Path, objective: &str) -> WorkflowRun {
    let now = Utc::now().timestamp_millis();
    WorkflowRun {
        id: wf_id.to_string(),
        thread_id: None,
        workspace_root: root.to_path_buf(),
        objective: objective.to_string(),
        workflow_type: "standard".to_string(),
        state: WorkflowState::Running,
        plan_revision: 1,
        configuration_json: serde_json::to_string(&WorkflowExecutionConfig::default()).unwrap(),
        created_at_epoch_ms: now,
        updated_at_epoch_ms: now,
        started_at_epoch_ms: Some(now),
        completed_at_epoch_ms: None,
        cancellation_reason: None,
        overall_result_json: None,
        resource_usage_json: None,
    }
}

fn make_test_task(
    wf_id: &str,
    task_id: &str,
    name: &str,
    isolation: WorkspaceIsolationStrategy,
    write_paths: Vec<PathBuf>,
) -> WorkflowTask {
    WorkflowTask {
        id: task_id.to_string(),
        workflow_id: wf_id.to_string(),
        plan_revision: 1,
        task_kind: WorkflowTaskKind::Implementation,
        name: name.to_string(),
        description: name.to_string(),
        assigned_role: "Engineer".to_string(),
        assigned_agent_id: None,
        priority: 10,
        wave_index: 1,
        dependencies: vec![],
        acceptance_criteria: vec![],
        required_verification: vec![],
        declared_read_paths: vec![],
        declared_write_paths: write_paths,
        workspace_isolation: isolation,
        status: WorkflowTaskStatus::Ready,
        retry_count: 0,
        max_retries: 3,
        attempt_id: 0,
        started_at_epoch_ms: None,
        completed_at_epoch_ms: None,
        worker_id: None,
        lease_expires_at_epoch_ms: None,
        error_details: None,
        input_artifacts: vec![],
        output_artifacts: vec![],
        verification_result_json: None,
        execution_result_json: None,
    }
}

/// Helper: Creates standard test orchestrator over a file-backed SQLite storage.
fn create_test_orchestrator(
    storage: Storage,
    wt_mgr: Arc<WorktreeManager>,
) -> WorkflowOrchestrator {
    let tools = ToolRegistry::new();
    let exec_policy = ExecPolicy::standard();
    WorkflowOrchestrator::new(storage, None, None, Some(tools), Some(exec_policy))
        .with_worktree_manager(wt_mgr)
}

// =========================================================================
// TEST 1: Independent agents modifying identical relative paths execute concurrently in worktrees
// =========================================================================
#[tokio::test]
async fn test_1_independent_agents_modifying_identical_paths_execute_concurrently_in_worktrees() {
    let repo_temp = tempdir().unwrap();
    let db_temp = tempdir().unwrap();
    let db_path = db_temp.path().join("m31a_test1.db");

    let base_sha = init_git_repo(repo_temp.path());
    let storage = Storage::open(&db_path).unwrap();
    let store = storage.workflow_store();

    let wt_mgr = Arc::new(
        WorktreeManager::new(WorktreeSettings::new(repo_temp.path())).with_store(store.clone()),
    );

    let wf_id = format!("wf-{}", Uuid::new_v4().simple());
    let run = make_test_run(&wf_id, repo_temp.path(), "Concurrent worktree execution");
    store.create_workflow(&run).unwrap();

    let task_a = make_test_task(
        &wf_id,
        "task-feat-a",
        "Feature A",
        WorkspaceIsolationStrategy::Worktree,
        vec![PathBuf::from("src/lib.rs")],
    );

    let task_b = make_test_task(
        &wf_id,
        "task-feat-b",
        "Feature B",
        WorkspaceIsolationStrategy::Worktree,
        vec![PathBuf::from("src/lib.rs")],
    );

    let plan = WorkflowPlan {
        workflow_id: wf_id.clone(),
        revision: 1,
        rationale: "Concurrent worktrees".into(),
        tasks: vec![task_a.clone(), task_b.clone()],
        created_at_epoch_ms: Utc::now().timestamp_millis(),
    };
    store.save_workflow_plan(&plan).unwrap();

    // Both tasks can be claimed CONCURRENTLY because isolation is Worktree!
    let claimed_a = store
        .claim_task(&wf_id, &task_a.id, "worker-a", 30_000)
        .unwrap();
    assert!(claimed_a.is_some(), "Task A should be claimed");

    let claimed_b = store
        .claim_task(&wf_id, &task_b.id, "worker-b", 30_000)
        .unwrap();
    assert!(
        claimed_b.is_some(),
        "Task B should also be claimed concurrently despite overlapping declared path"
    );

    // Spawn real isolated worktrees on filesystem
    let wt_a = wt_mgr
        .create_task_worktree(Some(&wf_id), Some(&task_a.id), "worker-a", Some(&base_sha))
        .unwrap();

    let wt_b = wt_mgr
        .create_task_worktree(Some(&wf_id), Some(&task_b.id), "worker-b", Some(&base_sha))
        .unwrap();

    // Modify identical relative paths concurrently in isolated worktrees
    let lib_a = wt_a.worktree_path.join("src/lib.rs");
    let lib_b = wt_b.worktree_path.join("src/lib.rs");

    fs::write(
        &lib_a,
        "pub fn base_function() -> i32 { 42 }\npub fn feat_a() -> i32 { 100 }\n",
    )
    .unwrap();
    fs::write(
        &lib_b,
        "pub fn base_function() -> i32 { 42 }\npub fn feat_b() -> i32 { 200 }\n",
    )
    .unwrap();

    // Both export valid diffs against base commit
    let diff_a = wt_mgr
        .export_worktree_diff(&wt_a.worktree_path, &base_sha)
        .unwrap();
    let diff_b = wt_mgr
        .export_worktree_diff(&wt_b.worktree_path, &base_sha)
        .unwrap();

    assert!(diff_a.contains("feat_a"));
    assert!(diff_b.contains("feat_b"));

    // Cleanup worktrees
    let _ = wt_mgr.cleanup_worktree("worker-a");
    let _ = wt_mgr.cleanup_worktree("worker-b");
}

// =========================================================================
// TEST 2: Overlapping modifications are detected during integration
// =========================================================================
#[tokio::test]
async fn test_2_overlapping_modifications_detected_during_integration() {
    let repo_temp = tempdir().unwrap();
    let db_temp = tempdir().unwrap();
    let db_path = db_temp.path().join("m31a_test2.db");

    let base_sha = init_git_repo(repo_temp.path());
    let storage = Storage::open(&db_path).unwrap();
    let store = storage.workflow_store();

    let wf_id = format!("wf-{}", Uuid::new_v4().simple());
    let run = make_test_run(&wf_id, repo_temp.path(), "Overlap detection");
    store.create_workflow(&run).unwrap();

    let svc = WorkflowIntegrationService::new().with_store(store.clone());

    // Task 1 modifies line 1 of src/lib.rs
    let patch_1 = r#"*** Begin Patch
*** Update File: src/lib.rs
-pub fn base_function() -> i32 { 42 }
+pub fn base_function() -> i32 { 100 }
*** End Patch
"#;

    let req_1 = IntegrationRequest {
        workspace_root: repo_temp.path().to_path_buf(),
        patch_content: patch_1.into(),
        source_label: "task-1".into(),
        commit_message: Some("feat: task 1 changes".into()),
        require_approval: false,
        is_approved: true,
        author_role: Some("Engineer".into()),
        skip_dirty_check: false,
        skip_verification: true,
        verification_timeout: Some(Duration::from_secs(30)),
        workflow_id: Some(wf_id.clone()),
        task_id: Some("task-1".into()),
        attempt_id: Some(1),
        expected_change_set_hash: None,
        source_branch: Some("branch-task-1".into()),
        source_base_sha: Some(base_sha.clone()),
        target_base_sha: Some(base_sha.clone()),
    };

    let outcome_1 = svc.integrate(&req_1).await.unwrap();
    assert!(
        outcome_1.success,
        "Task 1 integration must succeed: {:?}",
        outcome_1.error
    );

    // Now target workspace HEAD has advanced.
    // Task 2 was developed against stale base_sha and conflicts on the exact same line!
    let patch_2 = r#"*** Begin Patch
*** Update File: src/lib.rs
-pub fn base_function() -> i32 { 42 }
+pub fn base_function() -> i32 { 999 }
*** End Patch
"#;

    let req_2 = IntegrationRequest {
        workspace_root: repo_temp.path().to_path_buf(),
        patch_content: patch_2.into(),
        source_label: "task-2".into(),
        commit_message: Some("feat: task 2 changes".into()),
        require_approval: false,
        is_approved: true,
        author_role: Some("Engineer".into()),
        skip_dirty_check: false,
        skip_verification: true,
        verification_timeout: Some(Duration::from_secs(30)),
        workflow_id: Some(wf_id.clone()),
        task_id: Some("task-2".into()),
        attempt_id: Some(1),
        expected_change_set_hash: None,
        source_branch: Some("branch-task-2".into()),
        source_base_sha: Some(base_sha.clone()), // Stale base!
        target_base_sha: Some(base_sha.clone()),
    };

    let outcome_2 = svc.integrate(&req_2).await.unwrap();
    assert!(
        !outcome_2.success,
        "Task 2 integration must fail due to conflicting overlap on stale base"
    );

    // Verify durable record status is Rejected
    let records = store.list_integrations(&wf_id).unwrap();
    let rec_2 = records
        .iter()
        .find(|r| r.task_id == "task-2")
        .expect("record for task-2");
    assert_eq!(rec_2.status, IntegrationLifecycleStatus::Rejected);

    // Verify failed integration patch artifact was preserved
    let artifacts = store.list_artifacts(&wf_id).unwrap();
    assert!(
        artifacts
            .iter()
            .any(|a| a.name.starts_with("failed_integration_task-2"))
    );
}

// =========================================================================
// TEST 3: Shared-workspace overlapping writes are safely serialized
// =========================================================================
#[tokio::test]
async fn test_3_shared_workspace_overlapping_writes_safely_serialized() {
    let db_temp = tempdir().unwrap();
    let db_path = db_temp.path().join("m31a_test3.db");
    let storage = Storage::open(&db_path).unwrap();
    let store = storage.workflow_store();

    let wf_id = format!("wf-{}", Uuid::new_v4().simple());
    let run = make_test_run(
        &wf_id,
        Path::new("/tmp/dummy"),
        "Shared workspace serialization",
    );
    store.create_workflow(&run).unwrap();

    let task_shared_1 = make_test_task(
        &wf_id,
        "shared-1",
        "Shared 1",
        WorkspaceIsolationStrategy::SharedWorkspace,
        vec![PathBuf::from("src/shared.rs")],
    );

    let task_shared_2 = make_test_task(
        &wf_id,
        "shared-2",
        "Shared 2",
        WorkspaceIsolationStrategy::SharedWorkspace,
        vec![PathBuf::from("src/shared.rs")],
    );

    let plan = WorkflowPlan {
        workflow_id: wf_id.clone(),
        revision: 1,
        rationale: "Shared workspace".into(),
        tasks: vec![task_shared_1.clone(), task_shared_2.clone()],
        created_at_epoch_ms: Utc::now().timestamp_millis(),
    };
    store.save_workflow_plan(&plan).unwrap();

    // 1. Worker 1 claims shared-1
    let claimed_1 = store
        .claim_task(&wf_id, &task_shared_1.id, "worker-1", 30_000)
        .unwrap();
    assert!(claimed_1.is_some());

    // 2. Worker 2 attempts to claim shared-2 concurrently while shared-1 is claimed on same path
    let claimed_2 = store
        .claim_task(&wf_id, &task_shared_2.id, "worker-2", 30_000)
        .unwrap();
    assert!(
        claimed_2.is_none(),
        "Shared workspace overlapping write must NOT be claimed concurrently"
    );

    // 3. Worker 1 completes shared-1
    store
        .update_task_status(
            &wf_id,
            &task_shared_1.id,
            WorkflowTaskStatus::Completed,
            None,
        )
        .unwrap();

    // 4. Now Worker 2 can safely claim shared-2
    let claimed_2_retry = store
        .claim_task(&wf_id, &task_shared_2.id, "worker-2", 30_000)
        .unwrap();
    assert!(
        claimed_2_retry.is_some(),
        "Once shared-1 completes, shared-2 can claim cleanly"
    );
}

// =========================================================================
// TEST 4: Agents discover conflicts and create dependent tasks
// =========================================================================
#[tokio::test]
async fn test_4_agents_discover_conflicts_and_create_dependent_tasks() {
    let repo_temp = tempdir().unwrap();
    let db_temp = tempdir().unwrap();
    let db_path = db_temp.path().join("m31a_test4.db");

    let _base_sha = init_git_repo(repo_temp.path());
    let storage = Storage::open(&db_path).unwrap();
    let store = storage.workflow_store();
    let wt_mgr = Arc::new(WorktreeManager::new(WorktreeSettings::new(
        repo_temp.path(),
    )));
    let orch = create_test_orchestrator(storage.clone(), wt_mgr);

    let wf_id = format!("wf-{}", Uuid::new_v4().simple());
    let run = make_test_run(&wf_id, repo_temp.path(), "Dynamic replanning");
    store.create_workflow(&run).unwrap();

    let task_schema = make_test_task(
        &wf_id,
        "task-schema",
        "Create Schema",
        WorkspaceIsolationStrategy::Worktree,
        vec![PathBuf::from("schema.sql")],
    );

    let task_populate = make_test_task(
        &wf_id,
        "task-populate",
        "Populate Data",
        WorkspaceIsolationStrategy::Worktree,
        vec![PathBuf::from("data.sql")],
    );

    let plan = WorkflowPlan {
        workflow_id: wf_id.clone(),
        revision: 1,
        rationale: "Initial uncoordinated tasks".into(),
        tasks: vec![task_schema.clone(), task_populate.clone()],
        created_at_epoch_ms: Utc::now().timestamp_millis(),
    };
    store.save_workflow_plan(&plan).unwrap();

    // Populate task claims and discovers schema is missing
    let _ = store
        .claim_task(&wf_id, &task_populate.id, "worker-pop", 30_000)
        .unwrap();

    let conflict = ConflictReport {
        id: format!("conf-{}", Uuid::new_v4().simple()),
        workflow_id: wf_id.clone(),
        reporting_task_id: "task-populate".into(),
        conflicting_task_id: Some("task-schema".into()),
        resource_path: PathBuf::from("schema.sql"),
        conflict_kind: ConflictKind::GenuineDependency,
        description: "Populate task requires table schema to be created first".into(),
        suggested_dependency: Some("task-schema".into()),
        reported_at_epoch_ms: Utc::now().timestamp_millis(),
    };

    let resolution = orch.handle_task_conflict(&conflict).unwrap();
    match resolution {
        ConflictResolution::AddDependency {
            task_id,
            depends_on,
        } => {
            assert_eq!(task_id, "task-populate");
            assert_eq!(depends_on, "task-schema");
        }
        other => panic!("Expected AddDependency, got {:?}", other),
    }

    // Verify task-populate yielded and is now Blocked
    let t_pop = store.get_task(&wf_id, "task-populate").unwrap().unwrap();
    assert_eq!(t_pop.status, WorkflowTaskStatus::Blocked);
    assert!(
        t_pop.worker_id.is_none(),
        "Worker lease should be cleared on yield"
    );

    // Verify plan revision was incremented
    let updated_run = store.get_workflow(&wf_id).unwrap().unwrap();
    assert_eq!(updated_run.plan_revision, 2);

    // Complete task-schema
    let _ = store
        .claim_task(&wf_id, "task-schema", "worker-sch", 30_000)
        .unwrap();
    store
        .update_task_status(&wf_id, "task-schema", WorkflowTaskStatus::Completed, None)
        .unwrap();

    // Now task-populate is unblocked and promoted to Ready
    let t_pop_after = store.get_task(&wf_id, "task-populate").unwrap().unwrap();
    assert_eq!(t_pop_after.status, WorkflowTaskStatus::Ready);
}

// =========================================================================
// TEST 5: DAG revisions persist across process restarts
// =========================================================================
#[tokio::test]
async fn test_5_dag_revisions_persist_across_process_restarts() {
    let db_temp = tempdir().unwrap();
    let db_path = db_temp.path().join("m31a_test5.db");
    let wf_id = format!("wf-{}", Uuid::new_v4().simple());

    // INSTANCE A: Creates plan, dynamically adds dependency, writes revision 2
    {
        let storage_a = Storage::open(&db_path).unwrap();
        let store_a = storage_a.workflow_store();

        let run = make_test_run(&wf_id, Path::new("/tmp"), "Persistence across restarts");
        store_a.create_workflow(&run).unwrap();

        let t1 = make_test_task(
            &wf_id,
            "t1",
            "T1",
            WorkspaceIsolationStrategy::Worktree,
            vec![],
        );
        let t2 = make_test_task(
            &wf_id,
            "t2",
            "T2",
            WorkspaceIsolationStrategy::Worktree,
            vec![],
        );

        let plan_v1 = WorkflowPlan {
            workflow_id: wf_id.clone(),
            revision: 1,
            rationale: "V1".into(),
            tasks: vec![t1, t2],
            created_at_epoch_ms: Utc::now().timestamp_millis(),
        };
        store_a.save_workflow_plan(&plan_v1).unwrap();

        // Transactionally replan with dependency: t2 depends on t1
        store_a
            .replan_with_dependency(&wf_id, "t2", "t1", "t2 requires t1 output")
            .unwrap();
    } // Instance A dropped

    // INSTANCE B: Opens same file-backed SQLite database from disk
    {
        let storage_b = Storage::open(&db_path).unwrap();
        let store_b = storage_b.workflow_store();

        let run_b = store_b.get_workflow(&wf_id).unwrap().unwrap();
        assert_eq!(
            run_b.plan_revision, 2,
            "Workflow plan revision must be 2 after restart"
        );

        let plan_v2 = store_b.get_workflow_plan(&wf_id, Some(2)).unwrap().unwrap();
        assert_eq!(plan_v2.revision, 2);
        assert!(plan_v2.rationale.contains("t2 requires t1 output"));

        let t2_b = store_b.get_task(&wf_id, "t2").unwrap().unwrap();
        assert_eq!(t2_b.status, WorkflowTaskStatus::Blocked);
    }
}

// =========================================================================
// TEST 6: Cyclic dependencies are rejected
// =========================================================================
#[tokio::test]
async fn test_6_cyclic_dependencies_rejected() {
    let db_temp = tempdir().unwrap();
    let db_path = db_temp.path().join("m31a_test6.db");
    let storage = Storage::open(&db_path).unwrap();
    let store = storage.workflow_store();
    let wf_id = format!("wf-{}", Uuid::new_v4().simple());

    let run = make_test_run(&wf_id, Path::new("/tmp"), "Cycle rejection");
    store.create_workflow(&run).unwrap();

    let plan = WorkflowPlan {
        workflow_id: wf_id.clone(),
        revision: 1,
        rationale: "Initial A -> B -> C".into(),
        tasks: vec![
            make_test_task(
                &wf_id,
                "A",
                "A",
                WorkspaceIsolationStrategy::Worktree,
                vec![],
            ),
            make_test_task(
                &wf_id,
                "B",
                "B",
                WorkspaceIsolationStrategy::Worktree,
                vec![],
            ),
            make_test_task(
                &wf_id,
                "C",
                "C",
                WorkspaceIsolationStrategy::Worktree,
                vec![],
            ),
        ],
        created_at_epoch_ms: Utc::now().timestamp_millis(),
    };
    store.save_workflow_plan(&plan).unwrap();

    // Establish A -> B -> C
    store
        .replan_with_dependency(&wf_id, "B", "A", "B depends on A")
        .unwrap();
    store
        .replan_with_dependency(&wf_id, "C", "B", "C depends on B")
        .unwrap();

    // Attempting C -> A would create a cycle A -> B -> C -> A
    let cycle_err = store.replan_with_dependency(&wf_id, "A", "C", "Cycle attempt");
    assert!(cycle_err.is_err(), "Cyclic dependency must be rejected");
    let err_str = cycle_err.err().unwrap().to_string();
    assert!(
        err_str.contains("cyclic dependency detected"),
        "Error should explain cycle: {err_str}"
    );

    // Verify plan revision was not incremented by failed replanning
    let current_run = store.get_workflow(&wf_id).unwrap().unwrap();
    assert_eq!(current_run.plan_revision, 3);
}

// =========================================================================
// TEST 7: Dynamically spawned agents cannot bypass ownership policies
// =========================================================================
#[tokio::test]
async fn test_7_dynamically_spawned_agents_cannot_bypass_ownership_policies() {
    let repo_temp = tempdir().unwrap();
    let db_temp = tempdir().unwrap();
    let db_path = db_temp.path().join("m31a_test7.db");

    let _base_sha = init_git_repo(repo_temp.path());
    let storage = Storage::open(&db_path).unwrap();
    let store = storage.workflow_store();

    let svc = WorkflowIntegrationService::new().with_store(store);

    // Malicious or unauthorized patch attempting to traverse outside workspace root
    let malicious_patch = r#"*** Begin Patch
*** Update File: ../outside_secret.txt
+stolen_data
*** End Patch
"#;

    let req = IntegrationRequest {
        workspace_root: repo_temp.path().to_path_buf(),
        patch_content: malicious_patch.into(),
        source_label: "unauthorized-child-agent".into(),
        commit_message: None,
        require_approval: false,
        is_approved: true,
        author_role: Some("UntrustedWorker".into()),
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
    assert!(
        !outcome.success,
        "Unauthorized traversal attempt must be rejected"
    );
    assert!(outcome.error.unwrap().contains("traversal"));
}

// =========================================================================
// TEST 8: Expired workers cannot publish stale results
// =========================================================================
#[tokio::test]
async fn test_8_expired_workers_cannot_publish_stale_results() {
    let db_temp = tempdir().unwrap();
    let db_path = db_temp.path().join("m31a_test8.db");
    let storage = Storage::open(&db_path).unwrap();
    let store = storage.workflow_store();
    let wf_id = format!("wf-{}", Uuid::new_v4().simple());

    let run = make_test_run(&wf_id, Path::new("/tmp"), "Lease expiration and fencing");
    store.create_workflow(&run).unwrap();

    let task = make_test_task(
        &wf_id,
        "task-fenced",
        "Fenced Task",
        WorkspaceIsolationStrategy::Worktree,
        vec![],
    );

    let plan = WorkflowPlan {
        workflow_id: wf_id.clone(),
        revision: 1,
        rationale: "Fencing test".into(),
        tasks: vec![task],
        created_at_epoch_ms: Utc::now().timestamp_millis(),
    };
    store.save_workflow_plan(&plan).unwrap();

    // 1. Worker 1 claims task with 1ms lease
    let claimed_1 = store
        .claim_task(&wf_id, "task-fenced", "worker-1", 1)
        .unwrap()
        .unwrap();
    assert_eq!(claimed_1.attempt_id, 1);

    // Sleep to ensure lease expires
    tokio::time::sleep(Duration::from_millis(10)).await;

    // 2. Reconcile stale tasks -> Task returned to ready
    let now = Utc::now().timestamp_millis();
    let reconciled = store.reconcile_stale_tasks(&wf_id, now).unwrap();
    assert_eq!(reconciled, 1);

    // 3. Worker 2 claims the task -> attempt_id becomes 2
    let claimed_2 = store
        .claim_task(&wf_id, "task-fenced", "worker-2", 30_000)
        .unwrap()
        .unwrap();
    assert_eq!(claimed_2.attempt_id, 2);

    // 4. Stale Worker 1 attempts to publish completion using attempt_id 1
    let stale_err = store.update_task_execution_result(
        &wf_id,
        "task-fenced",
        Some(1),
        Some("worker-1"),
        WorkflowTaskStatus::Completed,
        None,
        None,
    );
    assert!(
        stale_err.is_err(),
        "Stale worker completion must fail closed"
    );
    let err_str = stale_err.err().unwrap().to_string();
    assert!(
        err_str.contains("Fencing violation") || err_str.contains("stale lease"),
        "Should report fencing token mismatch: {err_str}"
    );
}

// =========================================================================
// TEST 9: Integration retries are idempotent
// =========================================================================
#[tokio::test]
async fn test_9_integration_retries_are_idempotent() {
    let repo_temp = tempdir().unwrap();
    let db_temp = tempdir().unwrap();
    let db_path = db_temp.path().join("m31a_test9.db");

    let base_sha = init_git_repo(repo_temp.path());
    let storage = Storage::open(&db_path).unwrap();
    let store = storage.workflow_store();
    let wf_id = format!("wf-{}", Uuid::new_v4().simple());

    let run = make_test_run(&wf_id, repo_temp.path(), "Idempotency");
    store.create_workflow(&run).unwrap();

    let svc = WorkflowIntegrationService::new().with_store(store);

    let patch = r#"*** Begin Patch
*** Update File: src/lib.rs
-pub fn base_function() -> i32 { 42 }
+pub fn base_function() -> i32 { 777 }
*** End Patch
"#;

    let req = IntegrationRequest {
        workspace_root: repo_temp.path().to_path_buf(),
        patch_content: patch.into(),
        source_label: "idempotent-task".into(),
        commit_message: Some("feat: idempotent change".into()),
        require_approval: false,
        is_approved: true,
        author_role: Some("Engineer".into()),
        skip_dirty_check: false,
        skip_verification: true,
        verification_timeout: Some(Duration::from_secs(30)),
        workflow_id: Some(wf_id.clone()),
        task_id: Some("idempotent-task".into()),
        attempt_id: Some(1),
        expected_change_set_hash: None,
        source_branch: Some("branch-idemp".into()),
        source_base_sha: Some(base_sha.clone()),
        target_base_sha: Some(base_sha.clone()),
    };

    // First attempt: integrates and commits
    let outcome_1 = svc.integrate(&req).await.unwrap();
    assert!(outcome_1.success);

    // Second attempt (exact same request): detects already promoted change-set hash and returns success cleanly
    let outcome_2 = svc.integrate(&req).await.unwrap();
    assert!(outcome_2.success);
    assert!(
        outcome_2.summary.contains("Idempotent skip"),
        "Summary should indicate idempotent skip: {}",
        outcome_2.summary
    );
}

// =========================================================================
// TEST 10: Interrupted worktrees recover without losing pending changes
// =========================================================================
#[tokio::test]
async fn test_10_interrupted_worktrees_recover_without_losing_pending_changes() {
    let repo_temp = tempdir().unwrap();
    let db_temp = tempdir().unwrap();
    let db_path = db_temp.path().join("m31a_test10.db");

    let base_sha = init_git_repo(repo_temp.path());
    let storage = Storage::open(&db_path).unwrap();
    let store = storage.workflow_store();
    let wf_id = format!("wf-{}", Uuid::new_v4().simple());

    let run = make_test_run(&wf_id, repo_temp.path(), "Crash recovery");
    store.create_workflow(&run).unwrap();

    let wt_mgr = Arc::new(
        WorktreeManager::new(WorktreeSettings::new(repo_temp.path())).with_store(store.clone()),
    );

    // Create a worktree for task-crash
    let wt = wt_mgr
        .create_task_worktree(
            Some(&wf_id),
            Some("task-crash"),
            "thread-crash-1",
            Some(&base_sha),
        )
        .unwrap();

    // Agent writes uncommitted file in worktree
    let uncommitted_file = wt.worktree_path.join("src/recovery.rs");
    fs::write(
        &uncommitted_file,
        "pub fn recovered_work() -> &'static str { \"safe\" }\n",
    )
    .unwrap();

    // Process crashes and restarts -> invoke recover_interrupted_worktrees
    let recovered = wt_mgr.recover_interrupted_worktrees(&wf_id).unwrap();
    assert_eq!(recovered.len(), 1);
    assert_eq!(recovered[0].status, WorktreeStatus::Preserved);

    // Verify unintegrated diff was captured and persisted in SQLite
    let db_records = store.list_worktrees_for_workflow(&wf_id).unwrap();
    assert_eq!(db_records.len(), 1);
    assert_eq!(db_records[0].status, WorktreeStatus::Preserved);
    let diff = db_records[0]
        .unintegrated_diff
        .as_ref()
        .expect("unintegrated diff must be preserved");
    assert!(
        diff.contains("recovered_work"),
        "Diff must capture uncommitted changes: {diff}"
    );
}

// =========================================================================
// TEST 11: Cancellation does not destroy unreconciled artifacts
// =========================================================================
#[tokio::test]
async fn test_11_cancellation_does_not_destroy_unreconciled_artifacts() {
    let repo_temp = tempdir().unwrap();
    let db_temp = tempdir().unwrap();
    let db_path = db_temp.path().join("m31a_test11.db");

    let base_sha = init_git_repo(repo_temp.path());
    let storage = Storage::open(&db_path).unwrap();
    let store = storage.workflow_store();
    let wf_id = format!("wf-{}", Uuid::new_v4().simple());

    let run = make_test_run(&wf_id, repo_temp.path(), "Cancellation safety");
    store.create_workflow(&run).unwrap();

    let wt_mgr = Arc::new(
        WorktreeManager::new(WorktreeSettings::new(repo_temp.path())).with_store(store.clone()),
    );
    let orch = create_test_orchestrator(storage.clone(), wt_mgr.clone());

    // Create an active worktree with unreconciled work
    let wt = wt_mgr
        .create_task_worktree(
            Some(&wf_id),
            Some("task-cancel"),
            "thread-cancel-1",
            Some(&base_sha),
        )
        .unwrap();

    fs::write(
        wt.worktree_path.join("src/cancel_artifact.rs"),
        "// forensic evidence",
    )
    .unwrap();

    // Cancel workflow
    orch.cancel_workflow(&wf_id, "User aborted operation")
        .await
        .unwrap();

    // Verify workflow state is Cancelled
    let updated_run = store.get_workflow(&wf_id).unwrap().unwrap();
    assert_eq!(updated_run.state, WorkflowState::Cancelled);

    // Verify unreconciled worktree artifact still exists on disk for forensic inspection
    assert!(
        wt.worktree_path.exists(),
        "Worktree directory must NOT be deleted upon cancellation"
    );
    assert!(wt.worktree_path.join("src/cancel_artifact.rs").exists());

    // Verify worktree record in SQLite was marked Preserved
    let db_records = store.list_worktrees_for_workflow(&wf_id).unwrap();
    assert_eq!(db_records[0].status, WorktreeStatus::Preserved);
}

// =========================================================================
// TEST 12: Independent tasks are not unnecessarily serialized
// =========================================================================
#[test]
fn test_12_independent_tasks_not_unnecessarily_serialized() {
    let t1 = make_test_task(
        "wf-1",
        "t1",
        "T1",
        WorkspaceIsolationStrategy::Worktree,
        vec![PathBuf::from("src/lib.rs")],
    );

    let t2 = make_test_task(
        "wf-1",
        "t2",
        "T2",
        WorkspaceIsolationStrategy::Worktree,
        vec![PathBuf::from("src/lib.rs")],
    );

    let plan = WorkflowPlan {
        workflow_id: "wf-1".into(),
        revision: 1,
        rationale: "Parallel worktree wave partitioning".into(),
        tasks: vec![t1, t2],
        created_at_epoch_ms: Utc::now().timestamp_millis(),
    };

    let waves = plan.partition_into_waves().unwrap();
    // Both independent tasks with Worktree isolation MUST stay in Wave 1 together
    assert_eq!(
        waves.len(),
        1,
        "Worktree isolated tasks must NOT be split into separate waves"
    );
    assert_eq!(
        waves[0].len(),
        2,
        "Both tasks must execute concurrently in Wave 1"
    );
}

// =========================================================================
// TEST 13: Verification failures prevent promotion
// =========================================================================
#[tokio::test]
async fn test_13_verification_failures_prevent_promotion() {
    let repo_temp = tempdir().unwrap();
    let db_temp = tempdir().unwrap();
    let db_path = db_temp.path().join("m31a_test13.db");

    let base_sha = init_git_repo(repo_temp.path());
    let storage = Storage::open(&db_path).unwrap();
    let store = storage.workflow_store();
    let wf_id = format!("wf-{}", Uuid::new_v4().simple());

    let run = make_test_run(&wf_id, repo_temp.path(), "Verification gate");
    store.create_workflow(&run).unwrap();

    let svc = WorkflowIntegrationService::new().with_store(store.clone());

    // Patch introduces a syntax error that will fail Rust compiler verification
    let breaking_patch = r#"*** Begin Patch
*** Update File: src/lib.rs
-pub fn base_function() -> i32 { 42 }
+pub fn base_function() -> i32 { this is a broken syntax error }
*** End Patch
"#;

    let req = IntegrationRequest {
        workspace_root: repo_temp.path().to_path_buf(),
        patch_content: breaking_patch.into(),
        source_label: "breaking-task".into(),
        commit_message: Some("feat: broken code".into()),
        require_approval: false,
        is_approved: true,
        author_role: Some("Engineer".into()),
        skip_dirty_check: false,
        skip_verification: false, // MANDATORY verification
        verification_timeout: Some(Duration::from_secs(30)),
        workflow_id: Some(wf_id.clone()),
        task_id: Some("breaking-task".into()),
        attempt_id: Some(1),
        expected_change_set_hash: None,
        source_branch: Some("branch-broken".into()),
        source_base_sha: Some(base_sha.clone()),
        target_base_sha: Some(base_sha.clone()),
    };

    let outcome = svc.integrate(&req).await.unwrap();
    assert!(
        !outcome.success,
        "Breaking patch must fail verification and reject integration"
    );
    assert!(!outcome.verification_passed);
    assert!(
        outcome.rollback_performed,
        "Exact rollback must be executed on verification failure"
    );

    // Primary workspace content must be restored exactly to original base state
    let restored_content = fs::read_to_string(repo_temp.path().join("src/lib.rs")).unwrap();
    assert_eq!(restored_content, "pub fn base_function() -> i32 { 42 }\n");

    // Primary git HEAD must not have changed
    let head_out = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(repo_temp.path())
        .output()
        .unwrap();
    let current_sha = String::from_utf8_lossy(&head_out.stdout).trim().to_string();
    assert_eq!(
        current_sha, base_sha,
        "HEAD commit must not advance when verification fails"
    );

    // Integration status in SQLite must be Failed
    let records = store.list_integrations(&wf_id).unwrap();
    let rec = records
        .iter()
        .find(|r| r.task_id == "breaking-task")
        .unwrap();
    assert_eq!(rec.status, IntegrationLifecycleStatus::Failed);
}
