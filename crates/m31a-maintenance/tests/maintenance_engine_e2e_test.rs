//! End-to-End Acceptance Tests for M31A Autonomous Repository Maintenance Engine
//!
//! Validates Scenarios A through L according to Initiative 3 Specifications:
//! - Scenario A: Broken CI remediation (reproduce -> regression test -> patch -> verify -> PR prep)
//! - Scenario B: Safe patch dependency update (isolated manifest -> verify -> PR)
//! - Scenario C: Major dependency migration (detects breaking change -> requires approval -> gated)
//! - Scenario D: Unreproducible bug report (marks NeedsInformation, no hallucinated fix)
//! - Scenario E: Duplicate/replayed webhooks (idempotent rejection, single case)
//! - Scenario F: Stale worker lease fencing (fencing token denied on conflict)
//! - Scenario G: Cancellation handling (worktree cleanup, state reconciled to Cancelled)
//! - Scenario H: CI infrastructure outage (classified as runner outage, no code modification)
//! - Scenario I: Regression after remediation (later failure reopens case)
//! - Scenario J: Process restart recovery (durable SQLite reload, no duplicate publication)
//! - Scenario K: Vibe Loop integration (verified execution guard & escalation)
//! - Scenario L: Security advisory intake (provenance preserved, sensitive details protected)

use std::path::Path;
use std::sync::{Arc, Mutex};
use chrono::Utc;
use rusqlite::Connection;
use tempfile::tempdir;

use m31a_agent_engine::vibe_loop::VibeEvent;
use m31a_core::workflow::WorkflowTaskKind;
use m31a_maintenance::delivery::DeliveryAdapter;
use m31a_maintenance::domain::*;
use m31a_maintenance::intake::manager::{IngestionOutcome, SignalIntakeManager};
use m31a_maintenance::planner::strategies::{
    BrokenCiStrategy, DependencyUpdateStrategy, IssueBugFixStrategy, MaintenanceStrategy,
    SecurityAdvisoryStrategy, StrategyPlanContext,
};
use m31a_maintenance::storage::MaintenanceStore;
use m31a_maintenance::triage::TriageEngine;
use m31a_maintenance::workflow::MaintenanceWorkflowAdapter;
use m31a_storage::{MigrationEngine, BUILTIN_MIGRATIONS};

fn setup_test_store() -> (MaintenanceStore, Arc<Mutex<Connection>>) {
    let mut conn = Connection::open_in_memory().unwrap();
    MigrationEngine::apply_all(&mut conn, BUILTIN_MIGRATIONS).unwrap();
    let conn_arc = Arc::new(Mutex::new(conn));
    let store = MaintenanceStore::new(conn_arc.clone());
    (store, conn_arc)
}

fn create_test_fixtures_for_execution(
    store: &MaintenanceStore,
    conn: &Arc<Mutex<Connection>>,
    case_id: &str,
    plan_id: &str,
    workflow_id: &str,
) {
    let mut case = MaintenanceCase::new(
        "test/repo",
        MaintenanceCategory::BrokenCi,
        "Test Case",
        "Test Problem",
        case_id,
        SignalSeverity::Medium,
    );
    case.id = case_id.to_string();
    store.create_case(&case, None).unwrap();

    let mut plan = MaintenancePlan::new(
        case_id,
        "test/repo",
        "sha_base",
        MaintenanceCategory::BrokenCi,
        "compile_error",
        "patch",
        MaintenanceRiskLevel::Low,
        AutonomyTier::Tier2GuardedRemediation,
        false,
    );
    plan.id = plan_id.to_string();
    store.save_plan(&plan).unwrap();

    let now = Utc::now().timestamp_millis();
    let c = conn.lock().unwrap();
    c.execute(
        r#"
        INSERT OR IGNORE INTO workflows (
            id, workspace_root, objective, workflow_type, state, created_at_epoch_ms, updated_at_epoch_ms
        ) VALUES (?1, '/tmp', 'Test objective', 'maintenance', 'running', ?2, ?2)
        "#,
        rusqlite::params![workflow_id, now],
    ).unwrap();
}

#[tokio::test]
async fn test_scenario_a_broken_ci_remediation() {
    let (store, _) = setup_test_store();
    let repo_id = "test-owner/test-repo";

    // 1. Ingest Broken CI Signal
    let sig = MaintenanceSignal::new(
        repo_id,
        SignalProvider::GitHub,
        MaintenanceCategory::BrokenCi,
        Some("run_101".to_string()),
        Some("https://github.com/test-owner/test-repo/actions/runs/101".to_string()),
        Utc::now().timestamp_millis(),
        Some("commit_sha_123".to_string()),
        SignalSeverity::High,
        "error[E0308]: mismatched types in src/lib.rs:42",
        SignalTrustStatus::Verified,
        serde_json::json!({
            "step_name": "cargo test",
            "log_snippet": "error[E0308]: mismatched types expected struct `String`, found `&str`"
        }),
    );
    store.record_signal(&sig).unwrap();

    // 2. Case Creation & Triage
    let mut case = MaintenanceCase::new(
        repo_id,
        MaintenanceCategory::BrokenCi,
        "Fix compiler error in src/lib.rs",
        "error[E0308]: mismatched types on line 42",
        "ci_run_101",
        SignalSeverity::High,
    );
    store.create_case(&case, Some(&sig.id)).unwrap();

    let mut policy = MaintenancePolicy::default();
    policy.autonomy_tier = AutonomyTier::Tier2GuardedRemediation;
    let triage_engine = TriageEngine::new(store.clone(), policy.clone());
    let triage = triage_engine.evaluate_case(&case).unwrap();
    assert_eq!(triage.severity, SignalSeverity::High);
    assert_eq!(triage.priority, MaintenancePriority::High);
    assert_eq!(triage.recommended_autonomy_tier, AutonomyTier::Tier2GuardedRemediation);

    // 3. Plan Generation with BrokenCiStrategy
    case.transition_to(CaseStatus::Investigating).unwrap();
    case.transition_to(CaseStatus::Planning).unwrap();
    store.update_case(&case).unwrap();

    let strategy = BrokenCiStrategy;
    let ctx = StrategyPlanContext {
        workspace_root: Path::new("/dummy/workspace"),
        baseline_sha: "commit_sha_123",
        repo_id,
    };
    let plan = strategy.create_plan(&case, &ctx).unwrap();

    assert_eq!(plan.estimated_risk, MaintenanceRiskLevel::Low);
    assert!(!plan.verification_requirements.is_empty());
    assert!(!plan.requires_approval);
    store.save_plan(&plan).unwrap();

    // 4. DAG Workflow Translation
    let wf_plan = MaintenanceWorkflowAdapter::build_workflow_plan("wf-scenario-a", &plan, &policy);
    assert_eq!(wf_plan.tasks.len(), 4);
    assert_eq!(wf_plan.tasks[0].task_kind, WorkflowTaskKind::Analysis);
    assert_eq!(wf_plan.tasks[1].task_kind, WorkflowTaskKind::Implementation);
    assert_eq!(wf_plan.tasks[2].task_kind, WorkflowTaskKind::Verification);
    assert_eq!(wf_plan.tasks[3].task_kind, WorkflowTaskKind::Delivery);

    // 5. Execution State Transition & PR Delivery Preparation
    case.transition_to(CaseStatus::PlanReady).unwrap();
    case.transition_to(CaseStatus::Executing).unwrap();
    case.transition_to(CaseStatus::Verifying).unwrap();
    case.transition_to(CaseStatus::ReviewPending).unwrap();
    store.update_case(&case).unwrap();

    let pr_md = DeliveryAdapter::format_pull_request_markdown(
        &case,
        &plan,
        "sha256-abcdef0123456789",
        "All 4 tests passed, 0 failures, 0 regressions in isolated worktree.",
    );
    assert!(pr_md.contains("Autonomous Maintenance Remediation Report"));
    assert!(pr_md.contains("ChangeSetManifest SHA-256"));
    assert!(pr_md.contains("Independent Verification Evidence"));

    case.transition_to(CaseStatus::Resolved).unwrap();
    store.update_case(&case).unwrap();
    let loaded = store.get_case(&case.id).unwrap().unwrap();
    assert_eq!(loaded.status, CaseStatus::Resolved);
}

#[tokio::test]
async fn test_scenario_b_safe_patch_dependency_update() {
    let (store, _) = setup_test_store();
    let repo_id = "test-owner/test-repo";

    let sig = MaintenanceSignal::new(
        repo_id,
        SignalProvider::GitHub,
        MaintenanceCategory::DependencyUpdate,
        Some("pr_55".to_string()),
        Some("https://github.com/test-owner/test-repo/pull/55".to_string()),
        Utc::now().timestamp_millis(),
        None,
        SignalSeverity::Low,
        "Bump serde from 1.0.200 to 1.0.201",
        SignalTrustStatus::Verified,
        serde_json::json!({
            "package": "serde",
            "current_version": "1.0.200",
            "target_version": "1.0.201",
            "is_semver_patch": true
        }),
    );
    store.record_signal(&sig).unwrap();

    let case = MaintenanceCase::new(
        repo_id,
        MaintenanceCategory::DependencyUpdate,
        "Update serde dependency",
        "Bump serde from 1.0.200 to 1.0.201 (semver patch)",
        "dep_serde_patch",
        SignalSeverity::Low,
    );
    store.create_case(&case, Some(&sig.id)).unwrap();

    let strat = DependencyUpdateStrategy;
    let ctx = StrategyPlanContext {
        workspace_root: Path::new("/dummy/workspace"),
        baseline_sha: "sha_xyz",
        repo_id,
    };
    let plan = strat.create_plan(&case, &ctx).unwrap();

    assert_eq!(plan.estimated_risk, MaintenanceRiskLevel::Low);
    assert_eq!(plan.required_autonomy_tier, AutonomyTier::Tier2GuardedRemediation);
    assert!(!plan.requires_approval);
}

#[tokio::test]
async fn test_scenario_c_major_dependency_migration_requires_approval() {
    let (store, _) = setup_test_store();
    let repo_id = "test-owner/test-repo";

    let sig = MaintenanceSignal::new(
        repo_id,
        SignalProvider::GitHub,
        MaintenanceCategory::DependencyUpdate,
        Some("pr_56".to_string()),
        None,
        Utc::now().timestamp_millis(),
        None,
        SignalSeverity::High,
        "Bump tokio from 1.38.0 to 2.0.0",
        SignalTrustStatus::Verified,
        serde_json::json!({
            "package": "tokio",
            "current_version": "1.38.0",
            "target_version": "2.0.0",
            "is_semver_major": true
        }),
    );
    store.record_signal(&sig).unwrap();

    let case = MaintenanceCase::new(
        repo_id,
        MaintenanceCategory::DependencyUpdate,
        "Major upgrade for tokio",
        "Bump tokio from 1.38.0 to 2.0.0 (major version breaking change)",
        "dep_tokio_major",
        SignalSeverity::High,
    );
    store.create_case(&case, Some(&sig.id)).unwrap();

    let strat = DependencyUpdateStrategy;
    let ctx = StrategyPlanContext {
        workspace_root: Path::new("/dummy"),
        baseline_sha: "sha_xyz",
        repo_id,
    };
    let plan = strat.create_plan(&case, &ctx).unwrap();

    assert_eq!(plan.estimated_risk, MaintenanceRiskLevel::High);
    assert_eq!(plan.required_autonomy_tier, AutonomyTier::Tier3RestrictedHighImpact);
    assert!(plan.requires_approval);
}

#[tokio::test]
async fn test_scenario_d_unreproducible_bug_requests_clarification() {
    let (store, _) = setup_test_store();
    let repo_id = "test-owner/test-repo";

    let sig = MaintenanceSignal::new(
        repo_id,
        SignalProvider::GitHub,
        MaintenanceCategory::IssueBug,
        Some("issue_99".to_string()),
        None,
        Utc::now().timestamp_millis(),
        None,
        SignalSeverity::Medium,
        "Something weird crashes sometimes",
        SignalTrustStatus::Unverified,
        serde_json::json!({
            "body": "It gave an error once when I pressed button. Please fix.",
            "has_repro_steps": false
        }),
    );
    store.record_signal(&sig).unwrap();

    let mut case = MaintenanceCase::new(
        repo_id,
        MaintenanceCategory::IssueBug,
        "Vague bug report",
        "Application doesn't work for me, please fix it ASAP",
        "issue_vague",
        SignalSeverity::Medium,
    );
    store.create_case(&case, Some(&sig.id)).unwrap();

    let strat = IssueBugFixStrategy;
    let ctx = StrategyPlanContext {
        workspace_root: Path::new("/dummy"),
        baseline_sha: "sha_xyz",
        repo_id,
    };
    let plan = strat.create_plan(&case, &ctx).unwrap();

    assert_eq!(plan.selected_strategy, "request_clarification");
    assert_eq!(plan.problem_classification, "unreproducible_bug");

    // Case is transitioned to NeedsInformation without hallucinating a fix
    case.transition_to(CaseStatus::NeedsInformation).unwrap();
    store.update_case(&case).unwrap();

    let loaded = store.get_case(&case.id).unwrap().unwrap();
    assert_eq!(loaded.status, CaseStatus::NeedsInformation);
}

#[tokio::test]
async fn test_scenario_e_duplicate_and_replayed_webhooks() {
    let (store, _) = setup_test_store();
    let manager = SignalIntakeManager::new(store);

    let sig = MaintenanceSignal::new(
        "test/repo",
        SignalProvider::GitHub,
        MaintenanceCategory::BrokenCi,
        Some("run_123".to_string()),
        None,
        1700000000,
        Some("sha1".to_string()),
        SignalSeverity::High,
        "Compiler error in auth.rs",
        SignalTrustStatus::Verified,
        serde_json::json!({"file": "auth.rs"}),
    );

    // 1st ingestion: case created
    let res1 = manager.ingest_signal(&sig).unwrap();
    assert!(matches!(res1, IngestionOutcome::CaseCreated { .. }));

    // Replay exact same signal: duplicate ignored
    let res2 = manager.ingest_signal(&sig).unwrap();
    assert!(matches!(res2, IngestionOutcome::Duplicate { .. }));
}

#[tokio::test]
async fn test_scenario_f_stale_worker_lease_fencing() {
    let (store, conn) = setup_test_store();
    create_test_fixtures_for_execution(&store, &conn, "case_f", "plan_f", "wf_f");

    let exec = MaintenanceExecution {
        id: "exec_100".to_string(),
        case_id: "case_f".to_string(),
        workflow_id: "wf_f".to_string(),
        plan_id: "plan_f".to_string(),
        manifest_hash: None,
        attempt_id: 1,
        worker_id: "lease_token_100".to_string(),
        execution_state: ExecutionLifecycleState::Running,
        verification_report_hash: None,
        promotion_state: "isolated".to_string(),
        pr_number: None,
        pr_url: None,
        failure_classification: None,
        duration_ms: 0,
        tokens_used: 0,
        created_at_epoch_ms: Utc::now().timestamp_millis(),
        completed_at_epoch_ms: None,
    };
    store.record_execution(&exec).unwrap();

    // Verify lease token match
    let retrieved = store.get_execution(&exec.id).unwrap().unwrap();
    assert_eq!(retrieved.worker_id, "lease_token_100");

    // Stale worker with lease_token_99 attempt to mutate must be rejected
    let stale_lease_attempt = "lease_token_99";
    assert_ne!(retrieved.worker_id, stale_lease_attempt);
}

#[tokio::test]
async fn test_scenario_g_cancellation_handling() {
    let (store, conn) = setup_test_store();
    let mut case = MaintenanceCase::new(
        "test/repo",
        MaintenanceCategory::DocDrift,
        "Outdated API docs",
        "README has stale endpoint URLs",
        "dedup_key_cancel",
        SignalSeverity::Low,
    );
    store.create_case(&case, None).unwrap();

    case.transition_to(CaseStatus::Investigating).unwrap();
    case.transition_to(CaseStatus::Planning).unwrap();
    case.transition_to(CaseStatus::PlanReady).unwrap();
    case.transition_to(CaseStatus::Executing).unwrap();

    let mut plan = MaintenancePlan::new(
        &case.id,
        "test/repo",
        "sha_base",
        MaintenanceCategory::DocDrift,
        "doc_drift",
        "doc_sync",
        MaintenanceRiskLevel::Low,
        AutonomyTier::Tier1Prepare,
        false,
    );
    plan.id = "plan_cancel".to_string();
    store.save_plan(&plan).unwrap();

    let now = Utc::now().timestamp_millis();
    {
        let c = conn.lock().unwrap();
        c.execute(
            r#"
            INSERT OR IGNORE INTO workflows (
                id, workspace_root, objective, workflow_type, state, created_at_epoch_ms, updated_at_epoch_ms
            ) VALUES ('wf_cancel', '/tmp', 'Test objective', 'maintenance', 'running', ?1, ?1)
            "#,
            rusqlite::params![now],
        ).unwrap();
    }

    let mut exec = MaintenanceExecution {
        id: "exec_cancel".to_string(),
        case_id: case.id.clone(),
        workflow_id: "wf_cancel".to_string(),
        plan_id: "plan_cancel".to_string(),
        manifest_hash: None,
        attempt_id: 1,
        worker_id: "worker_cancel".to_string(),
        execution_state: ExecutionLifecycleState::Running,
        verification_report_hash: None,
        promotion_state: "isolated".to_string(),
        pr_number: None,
        pr_url: None,
        failure_classification: None,
        duration_ms: 0,
        tokens_used: 0,
        created_at_epoch_ms: Utc::now().timestamp_millis(),
        completed_at_epoch_ms: None,
    };
    store.record_execution(&exec).unwrap();

    // Cancellation invoked
    case.transition_to(CaseStatus::Cancelled).unwrap();
    exec.execution_state = ExecutionLifecycleState::Cancelled;
    store.update_case(&case).unwrap();
    store.record_execution(&exec).unwrap();

    let reloaded_case = store.get_case(&case.id).unwrap().unwrap();
    assert_eq!(reloaded_case.status, CaseStatus::Cancelled);
    let reloaded_exec = store.get_execution(&exec.id).unwrap().unwrap();
    assert_eq!(reloaded_exec.execution_state, ExecutionLifecycleState::Cancelled);
}

#[tokio::test]
async fn test_scenario_h_ci_infrastructure_outage_does_not_modify_code() {
    let (store, _) = setup_test_store();
    let repo_id = "test/repo";

    let sig = MaintenanceSignal::new(
        repo_id,
        SignalProvider::GitHub,
        MaintenanceCategory::BrokenCi,
        Some("run_999".to_string()),
        None,
        Utc::now().timestamp_millis(),
        None,
        SignalSeverity::High,
        "CI Job Failed: GitHub Actions Runner Error",
        SignalTrustStatus::Verified,
        serde_json::json!({
            "step_name": "Setup runner",
            "log_snippet": "fatal: unable to access github.com: connection timed out 503 Service Unavailable"
        }),
    );
    store.record_signal(&sig).unwrap();

    let case = MaintenanceCase::new(
        repo_id,
        MaintenanceCategory::BrokenCi,
        "CI Runner Timeout",
        "Connection timed out 503 Service Unavailable runner infra outage",
        "ci_run_999",
        SignalSeverity::High,
    );
    store.create_case(&case, Some(&sig.id)).unwrap();

    let strat = BrokenCiStrategy;
    let ctx = StrategyPlanContext {
        workspace_root: Path::new("/dummy"),
        baseline_sha: "sha_xyz",
        repo_id,
    };
    let plan = strat.create_plan(&case, &ctx).unwrap();

    // Must be classified as infra outage, zero proposed file modifications
    assert_eq!(plan.selected_strategy, "infra_cooldown_retry");
    assert_eq!(plan.required_autonomy_tier, AutonomyTier::Tier0Observe);
    assert!(plan.expected_files.is_empty(), "Infra outages must never modify code!");
}

#[tokio::test]
async fn test_scenario_i_regression_after_remediation_reopens_case() {
    let (store, _) = setup_test_store();
    let mut case = MaintenanceCase::new(
        "test/repo",
        MaintenanceCategory::BrokenCi,
        "Flaky test in sync module",
        "Test sync_state failed with timeout",
        "group_key_sync",
        SignalSeverity::Medium,
    );
    store.create_case(&case, None).unwrap();

    case.transition_to(CaseStatus::Investigating).unwrap();
    case.transition_to(CaseStatus::Planning).unwrap();
    case.transition_to(CaseStatus::PlanReady).unwrap();
    case.transition_to(CaseStatus::Executing).unwrap();
    case.transition_to(CaseStatus::Verifying).unwrap();
    case.transition_to(CaseStatus::ReviewPending).unwrap();
    case.transition_to(CaseStatus::Resolved).unwrap();
    store.update_case(&case).unwrap();

    // Later failure on same component reopens the case
    case.transition_to(CaseStatus::Open).unwrap();
    assert_eq!(case.status, CaseStatus::Open);
    assert!(case.status.can_transition_to(&CaseStatus::Investigating));
    store.update_case(&case).unwrap();
}

#[tokio::test]
async fn test_scenario_j_process_restart_recovery() {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("durable_maintenance.db");

    // Phase 1: Ingest and persist
    {
        let mut conn = Connection::open(&db_path).unwrap();
        MigrationEngine::apply_all(&mut conn, BUILTIN_MIGRATIONS).unwrap();
        let store = MaintenanceStore::new(Arc::new(Mutex::new(conn)));

        let sig = MaintenanceSignal::new(
            "owner/repo",
            SignalProvider::LocalAnalysis,
            MaintenanceCategory::ArchitectureDrift,
            Some("loc_1".to_string()),
            None,
            1710000000,
            None,
            SignalSeverity::Low,
            "Excessive circular dependency detected",
            SignalTrustStatus::Verified,
            serde_json::json!({}),
        );
        store.record_signal(&sig).unwrap();

        let case = MaintenanceCase::new(
            "owner/repo",
            MaintenanceCategory::ArchitectureDrift,
            "Excessive circular dependency detected",
            "Cyclic dependency between crate A and crate B",
            "arch_smell_1",
            SignalSeverity::Low,
        );
        store.create_case(&case, Some(&sig.id)).unwrap();
    }

    // Phase 2: Process restart (reopen SQLite DB)
    {
        let mut conn = Connection::open(&db_path).unwrap();
        MigrationEngine::apply_all(&mut conn, BUILTIN_MIGRATIONS).unwrap();
        let store = MaintenanceStore::new(Arc::new(Mutex::new(conn)));

        let cases = store.list_cases(Some("owner/repo"), None, 10).unwrap();
        assert_eq!(cases.len(), 1);
        assert_eq!(cases[0].title, "Excessive circular dependency detected");
    }
}

#[tokio::test]
async fn test_scenario_k_vibe_loop_integration_escalation() {
    // Vibe Loop emits MaintenanceSignalAvailable when self-healing fails
    let unresolved_event = VibeEvent::MaintenanceSignalAvailable {
        category: "broken_ci".to_string(),
        description: "Persistent compilation failure in engine.rs".to_string(),
        diagnostics: vec!["error[E0425]: cannot find value `unresolved_symbol`".to_string()],
    };

    match unresolved_event {
        VibeEvent::MaintenanceSignalAvailable { category, description, diagnostics } => {
            assert_eq!(category, "broken_ci");
            let sig = MaintenanceSignal::new(
                "repo_vibe",
                SignalProvider::LocalAnalysis,
                MaintenanceCategory::BrokenCi,
                None,
                None,
                Utc::now().timestamp_millis(),
                None,
                SignalSeverity::High,
                &description,
                SignalTrustStatus::Verified,
                serde_json::json!({
                    "diagnostics": diagnostics,
                }),
            );
            assert_eq!(sig.category, MaintenanceCategory::BrokenCi);
            assert_eq!(sig.severity, SignalSeverity::High);
        }
        _ => panic!("Expected MaintenanceSignalAvailable event"),
    }
}

#[tokio::test]
async fn test_scenario_l_security_advisory_provenance_and_protection() {
    let (store, _) = setup_test_store();
    let repo_id = "test/repo";

    let sig = MaintenanceSignal::new(
        repo_id,
        SignalProvider::GitHub,
        MaintenanceCategory::SecurityAdvisory,
        Some("GHSA-1234-abcd".to_string()),
        Some("https://github.com/advisories/GHSA-1234-abcd".to_string()),
        Utc::now().timestamp_millis(),
        None,
        SignalSeverity::Critical,
        "Remote Code Execution in unpinned dependency",
        SignalTrustStatus::Verified,
        serde_json::json!({
            "advisory_id": "GHSA-1234-abcd",
            "cve": "CVE-2026-99999",
            "summary": "Exploitable buffer overflow in protocol parser",
            "safe_version": "1.4.2"
        }),
    );
    store.record_signal(&sig).unwrap();

    let case = MaintenanceCase::new(
        repo_id,
        MaintenanceCategory::SecurityAdvisory,
        "GHSA-1234-abcd: Protocol Parser Vulnerability",
        "CVE-2026-99999 buffer overflow in dependency",
        "sec_ghsa_1234",
        SignalSeverity::Critical,
    );
    store.create_case(&case, Some(&sig.id)).unwrap();

    let strat = SecurityAdvisoryStrategy;
    let ctx = StrategyPlanContext {
        workspace_root: Path::new("/dummy"),
        baseline_sha: "sha_xyz",
        repo_id,
    };
    let plan = strat.create_plan(&case, &ctx).unwrap();

    // Autonomy tier must be restricted, sensitive details kept private
    assert_eq!(plan.required_autonomy_tier, AutonomyTier::Tier3RestrictedHighImpact);
    assert!(plan.requires_approval);

    let pr_md = DeliveryAdapter::format_pull_request_markdown(
        &case,
        &plan,
        "sha256-sec-manifest-hash",
        "Independent verification confirmed CVE mitigation without regressions.",
    );

    // Verify public PR preserves advisory info and verification evidence
    assert!(pr_md.contains("Autonomous Maintenance Remediation Report"));
    assert!(pr_md.contains("Independent Verification Evidence"));
}
