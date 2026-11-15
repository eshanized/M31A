//! Production Fault Injection and Acceptance Test Suite for Initiative 2: Verified Execution Engine.
//!
//! Exercises the 6 critical invariants:
//! 1. Crash during patch application with restart recovery via durable SQLite snapshots.
//! 2. Stale worker fencing across storage, mutation, and external effect gateway boundaries.
//! 3. External API lost response -> uncertain outcome -> reconciliation -> idempotency skip on retry.
//! 4. Target revision CAS rejection when branch advances post-verification.
//! 5. Zero-test false positive prevention (exit 0 with 0 tests rejected as inconclusive/failed).
//! 6. Dirty workspace pre-check fail-closed preservation of untracked/uncommitted changes.

use m31a_agent_engine::effect_gateway::{
    EffectAdmissionOutcome, EffectDispatchRequest, ExternalEffectGateway,
};
use m31a_agent_engine::integration::{IntegrationRequest, WorkflowIntegrationService};
use m31a_core::workflow::{
    ChangeSetManifest, DurableVerificationRecord, ExternalEffectLifecycleStatus,
    PreIntegrationSnapshotRecord, PreIntegrationSnapshotStatus, WorkflowPlan, WorkflowRun,
    WorkflowState, WorkflowTask, WorkflowTaskKind, WorkflowTaskStatus, WorkspaceIsolationStrategy,
};
use m31a_storage::db::Storage;
use m31a_verify::{
    IndependentVerificationEngine, VerificationCategory, VerificationPlan, VerificationRequirement,
};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
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

    run_cmd(&["init"]);
    run_cmd(&["config", "user.name", "M31A Test"]);
    run_cmd(&["config", "user.email", "m31a@test.internal"]);

    fs::create_dir_all(path.join("src")).unwrap();
    fs::write(
        path.join("src/lib.rs"),
        "pub fn core_logic() -> &'static str { \"initial\" }\n",
    )
    .unwrap();
    fs::write(
        path.join("Cargo.toml"),
        "[package]\nname = \"dummy\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .unwrap();

    run_cmd(&["add", "."]);
    run_cmd(&["commit", "-m", "Initial commit"]);

    let out = run_cmd(&["rev-parse", "HEAD"]);
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

#[tokio::test]
async fn test_fault_injection_crash_during_patch_application_and_restart_recovery() {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("test.db");
    let storage = Storage::open(&db_path).unwrap();
    let store = storage.workflow_store();

    let wf_id = "wf-crash-recovery-1";
    let run = WorkflowRun {
        id: wf_id.to_string(),
        thread_id: None,
        workspace_root: temp.path().to_path_buf(),
        objective: "Crash recovery test".into(),
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

    let integration_id = "integ-crash-999";
    let file1_rel = PathBuf::from("src/lib.rs");
    let file2_rel = PathBuf::from("src/new_module.rs");

    let file1_abs = temp.path().join(&file1_rel);
    let file2_abs = temp.path().join(&file2_rel);
    fs::create_dir_all(file1_abs.parent().unwrap()).unwrap();

    let initial_content = b"pub fn pristine() -> bool { true }\n";
    fs::write(&file1_abs, initial_content).unwrap();

    let mut hasher = Sha256::new();
    hasher.update(initial_content);
    let original_hash = format!("{:x}", hasher.finalize());

    // 1. Stage durable pre-integration snapshots in SQLite (as taken immediately prior to patch application)
    let rec1 = PreIntegrationSnapshotRecord {
        id: Uuid::new_v4().to_string(),
        integration_id: integration_id.to_string(),
        workflow_id: wf_id.into(),
        task_id: "task-crash".into(),
        attempt_id: 1,
        relative_path: file1_rel.clone(),
        original_exists: true,
        original_content_hash: Some(original_hash),
        original_mode: None,
        preimage_storage_path: None,
        status: PreIntegrationSnapshotStatus::Prepared,
        created_at_epoch_ms: 1000,
    };
    store
        .save_pre_integration_snapshot(&rec1, Some(initial_content))
        .unwrap();

    let rec2 = PreIntegrationSnapshotRecord {
        id: Uuid::new_v4().to_string(),
        integration_id: integration_id.to_string(),
        workflow_id: wf_id.into(),
        task_id: "task-crash".into(),
        attempt_id: 1,
        relative_path: file2_rel.clone(),
        original_exists: false,
        original_content_hash: None,
        original_mode: None,
        preimage_storage_path: None,
        status: PreIntegrationSnapshotStatus::Prepared,
        created_at_epoch_ms: 1000,
    };
    store.save_pre_integration_snapshot(&rec2, None).unwrap();

    // 2. Simulate abrupt process termination during patch application:
    // File 1 is partially mutated/corrupted, File 2 was created before the crash occurred.
    fs::write(&file1_abs, "BROKEN PARTIAL MUTATION").unwrap();
    fs::write(&file2_abs, "UNWANTED UNCOMMITTED CREATION").unwrap();

    // 3. New engine process starts up after crash and calls resume_incomplete_rollbacks()
    let svc = WorkflowIntegrationService::new().with_store(store.clone());
    let incomplete = store.find_incomplete_rollbacks().unwrap();
    assert!(incomplete.contains(&integration_id.to_string()));

    let rollback_result = svc
        .resume_incomplete_rollbacks(temp.path(), integration_id)
        .unwrap();
    assert_eq!(rollback_result.restored_files.len(), 1);
    assert_eq!(rollback_result.removed_created_files.len(), 1);
    assert!(rollback_result.failed_files.is_empty());

    // 4. Verify disk state has returned completely to pristine initial state
    assert_eq!(fs::read(&file1_abs).unwrap(), initial_content);
    assert!(!file2_abs.exists());

    // 5. Verify snapshot entries in SQLite are all marked Restored
    let snapshots = store
        .list_pre_integration_snapshots(integration_id)
        .unwrap();
    for (s, _) in snapshots {
        assert_eq!(s.status, PreIntegrationSnapshotStatus::Restored);
    }
}

#[tokio::test]
async fn test_fault_injection_worker_fencing_at_storage_mutation_and_gateway_boundaries() {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("test.db");
    let storage = Storage::open(&db_path).unwrap();
    let store = storage.workflow_store();

    let wf_id = "wf-fence-test";
    let task_id = "task-fenced-worker";
    let run = WorkflowRun {
        id: wf_id.to_string(),
        thread_id: None,
        workspace_root: temp.path().to_path_buf(),
        objective: "Worker fencing test".into(),
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

    let plan = WorkflowPlan {
        workflow_id: wf_id.to_string(),
        revision: 1,
        rationale: "Fencing test plan".into(),
        tasks: vec![WorkflowTask {
            id: task_id.into(),
            workflow_id: wf_id.into(),
            plan_revision: 1,
            task_kind: WorkflowTaskKind::Implementation,
            name: "Fenced Task".into(),
            description: "Testing worker fencing".into(),
            assigned_role: "coder".into(),
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
        }],
        created_at_epoch_ms: 1000,
    };
    store.save_workflow_plan(&plan).unwrap();

    // Worker 1 claims Attempt 1
    let claimed_1 = store
        .claim_task(wf_id, task_id, "worker-1", 60)
        .unwrap()
        .unwrap();
    assert_eq!(claimed_1.attempt_id, 1);
    assert_eq!(claimed_1.worker_id.as_deref(), Some("worker-1"));

    // Worker 1 lease expires or reconciliation resets task -> Worker 2 claims Attempt 2
    store
        .update_task_status(wf_id, task_id, WorkflowTaskStatus::Ready, None)
        .unwrap();
    let claimed_2 = store
        .claim_task(wf_id, task_id, "worker-2", 60)
        .unwrap()
        .unwrap();
    assert_eq!(claimed_2.attempt_id, 2);
    assert_eq!(claimed_2.worker_id.as_deref(), Some("worker-2"));

    // Boundary 1: Storage update by stale Worker 1 must fail fencing check
    let stale_update_res = store.renew_task_lease(wf_id, task_id, "worker-1", 1, 60);
    assert!(
        stale_update_res.is_err() || !stale_update_res.unwrap(),
        "Stale worker lease renewal must fail"
    );

    // Boundary 2: Promotion via WorkflowIntegrationService by stale Worker 1 must be rejected
    let svc = WorkflowIntegrationService::new().with_store(store.clone());
    let promo_req = IntegrationRequest {
        workspace_root: temp.path().to_path_buf(),
        patch_content: "*** Begin Patch\n*** End Patch\n".into(),
        source_label: "worker-1".into(),
        workflow_id: Some(wf_id.to_string()),
        task_id: Some(task_id.to_string()),
        attempt_id: Some(1),
        worker_id: Some("worker-1".into()), // Stale worker!
        skip_dirty_check: true,
        skip_verification: true,
        is_approved: true,
        ..Default::default()
    };
    let promo_res = svc.integrate(&promo_req).await.unwrap();
    assert!(!promo_res.success);
    assert!(promo_res.error.unwrap().contains("Fencing violation"));

    // Boundary 3: External side effect gateway must reject stale Worker 1
    let gateway = ExternalEffectGateway::new(store.clone());
    let dispatch_req = EffectDispatchRequest {
        workflow_id: wf_id.to_string(),
        task_id: task_id.to_string(),
        attempt_id: 1,                     // Stale attempt!
        worker_id: "worker-1".to_string(), // Stale worker!
        tool_name: "deploy_service".to_string(),
        target_service: "aws_ecs".to_string(),
        authorized_destination: "https://api.ecs.us-east-1.amazonaws.com".to_string(),
        payload: "{\"cluster\":\"prod\",\"service\":\"web\"}".to_string(),
        logical_operation_id: Some("deploy-prod-op-1".to_string()),
        custom_idempotency_key: Some("deploy-prod-v1".to_string()),
        is_reversible: false,
        compensation_procedure: None,
    };
    let outcome = gateway.admit_effect(&dispatch_req).unwrap();
    match outcome {
        EffectAdmissionOutcome::FencingDenied { reason } => {
            assert!(reason.contains("Stale worker attempt"));
        }
        other => panic!("Expected FencingDenied, got {:?}", other),
    }
}

#[tokio::test]
async fn test_fault_injection_external_effect_uncertain_outcome_reconciliation_and_idempotent_retry()
 {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("test.db");
    let storage = Storage::open(&db_path).unwrap();
    let store = storage.workflow_store();

    let wf_id = "wf-effect-reconcile";
    let task_id = "task-deploy";
    let run = WorkflowRun {
        id: wf_id.to_string(),
        thread_id: None,
        workspace_root: temp.path().to_path_buf(),
        objective: "External effect reconciliation test".into(),
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

    let plan = WorkflowPlan {
        workflow_id: wf_id.to_string(),
        revision: 1,
        rationale: "External effect test".into(),
        tasks: vec![WorkflowTask {
            id: task_id.into(),
            workflow_id: wf_id.into(),
            plan_revision: 1,
            task_kind: WorkflowTaskKind::Implementation,
            name: "Deploy Task".into(),
            description: "Testing external side effects".into(),
            assigned_role: "devops".into(),
            assigned_agent_id: None,
            priority: 10,
            wave_index: 1,
            dependencies: vec![],
            acceptance_criteria: vec![],
            required_verification: vec![],
            declared_read_paths: vec![],
            declared_write_paths: vec![],
            workspace_isolation: WorkspaceIsolationStrategy::SharedWorkspace,
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
        }],
        created_at_epoch_ms: 1000,
    };
    store.save_workflow_plan(&plan).unwrap();

    // Worker 1 claims Attempt 1
    store.claim_task(wf_id, task_id, "worker-1", 60).unwrap();

    let gateway = ExternalEffectGateway::new(store.clone());
    let logical_op_id = "op-deploy-release-42";
    let idempotency_key = "idemp-deploy-release-42";

    // 1. Admit effect under attempt 1
    let dispatch_req1 = EffectDispatchRequest {
        workflow_id: wf_id.to_string(),
        task_id: task_id.to_string(),
        attempt_id: 1,
        worker_id: "worker-1".to_string(),
        tool_name: "deploy_release".to_string(),
        target_service: "production_cluster".to_string(),
        authorized_destination: "https://k8s.prod.internal/api/v1/namespaces/default/deployments"
            .to_string(),
        payload: "{\"tag\":\"v2.4.0\"}".to_string(),
        logical_operation_id: Some(logical_op_id.to_string()),
        custom_idempotency_key: Some(idempotency_key.to_string()),
        is_reversible: true,
        compensation_procedure: Some("kubectl rollout undo deployment/web".to_string()),
    };

    let admit_1 = gateway.admit_effect(&dispatch_req1).unwrap();
    let intent = match admit_1 {
        EffectAdmissionOutcome::Admitted { intent, .. } => intent,
        _ => panic!("Expected effect to be admitted"),
    };
    assert_eq!(
        intent.lifecycle_status,
        ExternalEffectLifecycleStatus::Authorized
    );

    // 2. Dispatch effect, simulate connection drop / timeout resulting in UnknownOutcome
    gateway.mark_dispatching(&intent.id).unwrap();
    gateway
        .record_uncertain(&intent.id, "Network connection timed out after dispatch")
        .unwrap();

    // Check that engine detects uncertain external effect
    let uncertain = store.list_uncertain_external_effects(wf_id).unwrap();
    assert_eq!(uncertain.len(), 1);
    assert_eq!(
        uncertain[0].lifecycle_status,
        ExternalEffectLifecycleStatus::UnknownOutcome
    );

    // 3. Independent reconciliation confirms provider successfully applied the effect
    let rec_status = gateway
        .reconcile(
            &intent.id,
            Some(true), // Confirmed applied!
            "Provider query confirmed deployment revision 42 is active",
        )
        .unwrap();
    assert_eq!(rec_status, ExternalEffectLifecycleStatus::Confirmed);

    // 4. Retry occurs under Attempt 2 by Worker 2 for the same logical operation
    store
        .update_task_status(wf_id, task_id, WorkflowTaskStatus::Ready, None)
        .unwrap();
    store.claim_task(wf_id, task_id, "worker-2", 60).unwrap();

    let dispatch_req2 = EffectDispatchRequest {
        workflow_id: wf_id.to_string(),
        task_id: task_id.to_string(),
        attempt_id: 2, // Attempt 2
        worker_id: "worker-2".to_string(),
        tool_name: "deploy_release".to_string(),
        target_service: "production_cluster".to_string(),
        authorized_destination: "https://k8s.prod.internal/api/v1/namespaces/default/deployments"
            .to_string(),
        payload: "{\"tag\":\"v2.4.0\"}".to_string(),
        logical_operation_id: Some(logical_op_id.to_string()), // Same logical operation!
        custom_idempotency_key: Some(idempotency_key.to_string()),
        is_reversible: true,
        compensation_procedure: Some("kubectl rollout undo deployment/web".to_string()),
    };

    let admit_retry = gateway.admit_effect(&dispatch_req2).unwrap();

    // Gateway MUST detect already completed/confirmed effect and return IdempotentSkip!
    match admit_retry {
        EffectAdmissionOutcome::IdempotentSkip {
            intent: skipped_intent,
        } => {
            assert_eq!(
                skipped_intent.lifecycle_status,
                ExternalEffectLifecycleStatus::Confirmed
            );
            assert_eq!(skipped_intent.logical_operation_id, logical_op_id);
        }
        other => panic!("Expected IdempotentSkip on retry, got {:?}", other),
    }
}

#[tokio::test]
async fn test_fault_injection_target_head_cas_rejection_when_branch_advances() {
    let temp = tempdir().unwrap();
    let repo_path = temp.path().join("repo");
    fs::create_dir_all(&repo_path).unwrap();

    let head_sha1 = init_git_repo(&repo_path);

    let svc = WorkflowIntegrationService::new();

    // Advance HEAD by another commit (simulating concurrent landing or external push)
    fs::write(
        repo_path.join("src/lib.rs"),
        "pub fn advanced() -> bool { true }\n",
    )
    .unwrap();
    let _ = Command::new("git")
        .args(["add", "."])
        .current_dir(&repo_path)
        .output()
        .unwrap();
    let _ = Command::new("git")
        .args(["commit", "-m", "Advance HEAD"])
        .current_dir(&repo_path)
        .output()
        .unwrap();

    let patch = r#"*** Begin Patch
*** Update File: src/lib.rs
-pub fn core_logic() -> &'static str { "initial" }
+pub fn core_logic() -> &'static str { "candidate" }
*** End Patch
"#;

    // Promotion request expects target HEAD to be head_sha1 (CAS check)
    let req = IntegrationRequest {
        workspace_root: repo_path.clone(),
        patch_content: patch.into(),
        source_label: "candidate-1".into(),
        expected_target_head_sha: Some(head_sha1), // Stale expected HEAD!
        skip_dirty_check: false,
        skip_verification: true,
        is_approved: true,
        ..Default::default()
    };

    let outcome = svc.integrate(&req).await.unwrap();
    assert!(
        !outcome.success,
        "Promotion must fail closed when target HEAD advanced"
    );
    assert!(outcome.error.unwrap().contains("CAS check failed"));
}

#[test]
fn test_fault_injection_zero_test_false_positive_rejected() {
    let plan = VerificationPlan {
        plan_id: "vplan-1".into(),
        workflow_id: "wf-1".into(),
        task_id: "task-1".into(),
        attempt_id: 1,
        manifest_hash: "man-hash".into(),
        requirements: vec![VerificationRequirement {
            id: "req-tests".into(),
            category: VerificationCategory::UnitTest,
            description: "Unit tests must pass with positive count".into(),
            target_paths: vec![],
            target_symbols: vec![],
            mandatory: true,
        }],
        related_tests: vec![],
        impacted_symbols: vec![],
        plan_hash: "plan-hash".into(),
        created_at_epoch_ms: 1000,
    };

    let manifest = ChangeSetManifest {
        manifest_version: 1,
        id: "manifest-1".into(),
        workflow_id: "wf-1".into(),
        plan_revision: 1,
        task_id: "task-1".into(),
        attempt_id: 1,
        worker_id: "worker-1".into(),
        agent_id: None,
        repository_root: PathBuf::from("/repo"),
        canonical_workspace_id: "ws-1".into(),
        source_base_sha: "base".into(),
        target_commit_sha: None,
        files: vec![],
        isolation_strategy: WorkspaceIsolationStrategy::Worktree,
        created_at_epoch_ms: 1000,
        frozen_at_epoch_ms: Some(1000),
        is_frozen: true,
        manifest_hash: "man-hash".into(),
    };

    // Record indicates command exited successfully (code 0) but executed ZERO tests!
    let zero_test_record = DurableVerificationRecord {
        id: "vrec-zero".into(),
        workflow_id: "wf-1".into(),
        task_id: "task-1".into(),
        attempt_id: 1,
        manifest_hash: "man-hash".into(),
        source_base_sha: "base".into(),
        target_commit_sha: None,
        verification_plan_hash: "plan-hash".into(),
        requirement_id: "req-tests".into(),
        category: "unit_test".into(),
        operation: "cargo test".into(),
        exit_code: Some(0),
        status: "Passed".into(),
        tests_total: Some(0), // ZERO tests!
        tests_passed: Some(0),
        tests_failed: Some(0),
        tests_skipped: Some(0),
        duration_ms: 120,
        diagnostics: None,
        artifact_refs: vec![],
        timestamp_epoch_ms: 1000,
    };

    let report =
        IndependentVerificationEngine::evaluate_evidence(&plan, &manifest, &[zero_test_record]);

    // Invariant: Zero tests executed MUST NOT be accepted as passing evidence!
    assert!(
        !report.success,
        "Zero-test execution must NOT be accepted as success"
    );
    assert_eq!(report.zero_test_false_positives_detected, 1);
    assert!(!report.failed_requirements.is_empty());
}

#[tokio::test]
async fn test_fault_injection_dirty_workspace_fails_closed_preserving_user_work() {
    let temp = tempdir().unwrap();
    let repo_path = temp.path().join("repo");
    fs::create_dir_all(&repo_path).unwrap();

    init_git_repo(&repo_path);

    // User has uncommitted work in the workspace
    let user_uncommitted_work = "pub fn user_uncommitted_feature() -> i32 { 1337 }\n";
    let target_file = repo_path.join("src/lib.rs");
    fs::write(&target_file, user_uncommitted_work).unwrap();

    let patch = r#"*** Begin Patch
*** Update File: src/lib.rs
-pub fn core_logic() -> &'static str { "initial" }
+pub fn core_logic() -> &'static str { "candidate" }
*** End Patch
"#;

    let svc = WorkflowIntegrationService::new();
    let req = IntegrationRequest {
        workspace_root: repo_path.clone(),
        patch_content: patch.into(),
        source_label: "candidate-1".into(),
        skip_dirty_check: false, // Strict cleanliness check!
        skip_verification: true,
        is_approved: true,
        ..Default::default()
    };

    let outcome = svc.integrate(&req).await.unwrap();
    assert!(
        !outcome.success,
        "Integration on dirty workspace must fail closed"
    );
    assert!(!outcome.pre_check_clean);
    assert!(outcome.summary.contains("dirty"));

    // Verify user's uncommitted work is strictly untouched and intact
    assert_eq!(
        fs::read_to_string(&target_file).unwrap(),
        user_uncommitted_work,
        "User uncommitted work must be strictly preserved"
    );
}
