//! Phase 24 — Closed-Loop Engineering: Verify → Diagnose → Repair → Replan Integration Tests
//!
//! Comprehensive test suite covering the 15 mandated scenarios:
//! 1. Missing import compilation failure repair
//! 2. API mismatch / caller argument repair
//! 3. Cascading compiler errors correlated to root cause
//! 4. Test regression detected vs pre-existing baseline test failures
//! 5. Pre-existing test failure not wrongly attributed as task regression
//! 6. Repeated identical failed repair rejected (AttemptStrategyClassification::RepeatedIdentical)
//! 7. Modified repair attempt permitted (AttemptStrategyClassification::ModifiedAttempt)
//! 8. Genuinely new repair attempt permitted (AttemptStrategyClassification::GenuinelyNew)
//! 9. Repair budget exhaustion triggering replan
//! 10. Deterministic non-retryable policy denial immediately halting
//! 11. Rollback action cleanly reverting tracked files to baseline
//! 12. AutonomyController closed loop: Verify -> Failure -> Honest Retry (no fabricated repair)
//! 13. Multi-file cascading error repair proposal synthesis
//! 14. Durable recovery attempt audit logging in SQLite recovery_attempts table
//! 15. Architectural single canonical ownership verification (no parallel duplicate engines)

use m31a::capability::providers::local_fs::LocalFileSystemProvider;
use m31a::capability::traits::FileSystemService;
use m31a::change::authority::ChangeAuthority;
use m31a::controller::AutonomyController;
use m31a::controller::dependencies::ControllerDependencies;
use m31a::controller::progress::LoopStage;
use m31a::controller::stage::StageOutcome;
use m31a::events::bus::{BroadcastEventBus, EventBus};
use m31a::ids::{MissionId, TaskId};
use m31a::kernel::change::{
    ChangeProposal, ChangeSurface, FileMutationOp, FileMutationProposal, ImplementationHypothesis,
};
use m31a::kernel::seams::recovery::{
    FailureClassification, RecoveryAction, RecoveryEngine, RecoveryStrategyRequest,
};
use m31a::kernel::seams::scheduler::WorkItem;
use m31a::kernel::seams::verifier::{
    CompletionGateOutcome, TaskVerificationRequest, VerificationEngine, VerificationError,
    VerificationOutcome,
};
use m31a::persistence::sqlite::schema::run_migrations;
use m31a::recovery::adapter::ProductionRecoveryEngine;
use m31a::recovery::budget::{
    AttemptStrategyClassification, BudgetEvaluation, RecoveryBudgetTracker,
    compute_mutation_fingerprint,
};
use m31a::state::Mission;
use m31a::state::intake::AutonomyMode;
use m31a::verification::diagnostician::{
    DiagnosticianContext, ModelDiagnostician, RecoveryRecommendation,
};
use m31a::verification::types::FailureEvidence;
use sqlx::sqlite::SqlitePoolOptions;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tempfile::tempdir;
use tokio_util::sync::CancellationToken;

// =========================================================================
// Scenario 1: Missing import compilation failure — honest repair boundary
// =========================================================================
#[tokio::test]
async fn test_scenario_1_missing_import_compilation_repair() {
    let dir = tempdir().unwrap();
    let ws = dir.path();
    let fs: Arc<dyn FileSystemService> = Arc::new(LocalFileSystemProvider::new(ws).unwrap());

    // Setup initial source missing an import
    let file_path = ws.join("src/lib.rs");
    tokio::fs::create_dir_all(ws.join("src")).await.unwrap();
    tokio::fs::write(&file_path, "pub fn run() -> i32 { helper() }\n")
        .await
        .unwrap();

    let rustc_error =
        "error[E0425]: cannot find function `helper` in this scope\n --> src/lib.rs:1:23";
    let ctx =
        DiagnosticianContext::new(rustc_error, Some(101), None, Some(rustc_error.to_string()));
    let ev = FailureEvidence::new(MissionId::new(), TaskId::new(), rustc_error)
        .with_process_output(Some(101), None, Some(rustc_error.to_string()));
    let ctx = ctx.with_failure_evidence(ev);

    let hypothesis = ModelDiagnostician::heuristic_hypothesis(&ctx);
    assert_eq!(
        hypothesis.recommended_action,
        RecoveryRecommendation::Repair
    );
    // Phase 27: the heuristic classifier must NOT fabricate file content.
    // No structured fix operation exists, so no repair proposal is produced;
    // recovery escalates to retry/replan instead of writing stub code.
    assert!(hypothesis.suggested_fix.is_none());
    assert!(hypothesis.repair_proposal.is_none());

    // A structured, evidence-derived fix operation DOES yield a proposal.
    let mut structured = hypothesis.clone();
    structured.suggested_fix = Some("insert: pub fn helper() -> i32 { 1 }".to_string());
    let diagnostician = ModelDiagnostician::new();
    let proposal = diagnostician
        .propose_repair(&ctx, &structured)
        .expect("structured fix operation yields a proposal");
    assert_eq!(proposal.mutations.len(), 1);
    assert_eq!(proposal.mutations[0].path, "src/lib.rs");

    // ChangeAuthority applies the repair
    let authority = ChangeAuthority::new();
    let outcome = authority
        .execute_change_proposal(ws, &proposal, &fs, None, None)
        .await
        .expect("repair proposal executed successfully");

    assert_eq!(outcome.files_modified, vec!["src/lib.rs"]);
    let updated_content = tokio::fs::read_to_string(&file_path).await.unwrap();
    assert!(updated_content.contains("pub fn helper() -> i32 { 1 }"));
}

// =========================================================================
// Scenario 2: API mismatch / caller argument repair
// =========================================================================
#[tokio::test]
async fn test_scenario_2_api_mismatch_caller_repair() {
    let rustc_error = r#"
error[E0061]: this function takes 2 arguments but 1 argument was supplied
 --> src/service.rs:10:5
  |
10|     compute(val);
  |     ^^^^^^^ --- supplied 1 argument
"#;
    let mut ctx =
        DiagnosticianContext::new(rustc_error, Some(101), None, Some(rustc_error.to_string()));
    let ev = FailureEvidence::new(MissionId::new(), TaskId::new(), rustc_error)
        .with_process_output(Some(101), None, Some(rustc_error.to_string()));
    ctx.failure_evidence = Some(ev);

    let hypothesis = ModelDiagnostician::heuristic_hypothesis(&ctx);
    assert_eq!(
        hypothesis.recommended_action,
        RecoveryRecommendation::Repair
    );
    assert_eq!(
        hypothesis.semantic_subclass,
        Some("api_mismatch".to_string())
    );
    // Phase 27: classification without a structured operation yields no
    // fabricated repair. A structured substitution yields a real proposal.
    assert!(hypothesis.repair_proposal.is_none());

    let mut structured = hypothesis.clone();
    structured.suggested_fix =
        Some("replace: compute(val); with: compute(val, DEFAULT_TIMEOUT);".to_string());
    let diagnostician = ModelDiagnostician::new();
    let proposal = diagnostician
        .propose_repair(&ctx, &structured)
        .expect("structured substitution yields a proposal");
    assert_eq!(proposal.mutations[0].path, "src/service.rs");
}

// =========================================================================
// Scenario 3: Cascading compiler errors correlated to root cause
// =========================================================================
#[tokio::test]
async fn test_scenario_3_cascading_compiler_errors_correlated() {
    let rustc_cascading = r#"
error[E0432]: unresolved import `m31a::database`
 --> src/main.rs:1:5
  |
1 | use m31a::database;
  |     ^^^^^^^^^^^^^^ no `database` in root

error[E0425]: cannot find type `DatabaseConnection` in this scope
 --> src/main.rs:4:12
  |
4 |     let c: DatabaseConnection = connect();
  |            ^^^^^^^^^^^^^^^^^^ not found

error[E0425]: cannot find function `connect` in this scope
 --> src/main.rs:4:33
  |
4 |     let c: DatabaseConnection = connect();
  |                                 ^^^^^^^ not found
"#;
    let ev = FailureEvidence::new(MissionId::new(), TaskId::new(), rustc_cascading)
        .with_process_output(Some(101), None, Some(rustc_cascading.to_string()));

    assert_eq!(ev.compiler_diagnostics.len(), 3);

    let (root_cause, cascading, subclass, class, rec) = ModelDiagnostician::correlate_root_cause(
        Some(&ev),
        rustc_cascading,
        Some(rustc_cascading),
        None,
    );

    assert!(root_cause.contains("unresolved import"));
    assert_eq!(cascading.len(), 2);
    assert_eq!(subclass, Some("unresolved_import".to_string()));
    assert_eq!(class, FailureClassification::Compilation);
    assert_eq!(rec, RecoveryRecommendation::Repair);
}

// =========================================================================
// Scenario 4 & 5: Baseline test failure vs test regression distinction
// =========================================================================
#[tokio::test]
async fn test_scenario_4_and_5_baseline_vs_regressions() {
    let test_output = r#"
running 3 tests
test suite::test_legacy_failing ... FAILED
test suite::test_user_auth ... FAILED
test suite::test_ping ... ok

failures:
---- suite::test_legacy_failing stdout ----
panicked at 'known upstream bug'
---- suite::test_user_auth stdout ----
panicked at 'regression: signature changed'
"#;
    let mut ev = FailureEvidence::new(MissionId::new(), TaskId::new(), test_output)
        .with_process_output(Some(101), None, Some(test_output.to_string()));

    assert_eq!(ev.test_failures.len(), 2);

    // Baseline failures passed: legacy test is known pre-existing
    ev.correlate_baseline_failures(&["suite::test_legacy_failing".to_string()]);

    assert!(ev.test_failures[0].is_pre_existing);
    assert!(!ev.test_failures[1].is_pre_existing);

    // Has new regressions because test_user_auth is newly broken
    assert!(ev.has_new_regressions());

    // If only legacy test failed, it would not have new regressions
    let mut ev_clean = FailureEvidence::new(
        MissionId::new(),
        TaskId::new(),
        "test suite::test_legacy_failing ... FAILED",
    )
    .with_process_output(
        Some(101),
        None,
        Some("test suite::test_legacy_failing ... FAILED".to_string()),
    );
    ev_clean.correlate_baseline_failures(&["suite::test_legacy_failing".to_string()]);
    assert!(!ev_clean.has_new_regressions());
}

// =========================================================================
// Scenario 6, 7 & 8: Attempt classification (RepeatedIdentical, Modified, GenuinelyNew)
// =========================================================================
#[tokio::test]
async fn test_scenario_6_7_8_attempt_strategy_classification() {
    let tracker = RecoveryBudgetTracker::default();
    let task_id = TaskId::new();
    let mission_id = MissionId::new();

    let proposal_a = ChangeProposal {
        id: m31a::kernel::change::ChangeProposalId::new(),
        task_id,
        mission_id,
        intent: ImplementationHypothesis::new("prob", "cause", "change", "res", "ver"),
        change_surface: ChangeSurface::new(vec!["src/mod.rs".to_string()]),
        preconditions: vec![],
        mutations: vec![FileMutationProposal::new(
            "src/mod.rs",
            FileMutationOp::CreateNew {
                content: "pub fn fix_a() {}".to_string(),
            },
            "attempt a",
        )],
        assumptions: vec![],
        verification_plan: vec![],
        risk_level: None,
        timestamp: chrono::Utc::now(),
    };

    let fp_a = compute_mutation_fingerprint(&proposal_a);

    // 8. GenuinelyNew on first proposal
    assert_eq!(
        tracker.classify_attempt(task_id, &fp_a),
        AttemptStrategyClassification::GenuinelyNew
    );
    let eval_a = tracker.evaluate_proposal(
        task_id,
        &proposal_a,
        FailureClassification::Compilation,
        0,
        0,
        0,
    );
    assert!(matches!(eval_a, BudgetEvaluation::Permitted { .. }));

    // Record attempt A
    tracker.record_attempt_fingerprint(task_id, &fp_a);

    // 6. RepeatedIdentical on second identical proposal
    assert_eq!(
        tracker.classify_attempt(task_id, &fp_a),
        AttemptStrategyClassification::RepeatedIdentical
    );
    let eval_a2 = tracker.evaluate_proposal(
        task_id,
        &proposal_a,
        FailureClassification::Compilation,
        1,
        1,
        1,
    );
    assert!(matches!(
        eval_a2,
        BudgetEvaluation::RepeatedIdentical { .. }
    ));

    // 7. ModifiedAttempt on different proposal for same task
    let mut proposal_b = proposal_a.clone();
    proposal_b.mutations[0].operation = FileMutationOp::CreateNew {
        content: "pub fn fix_b() {}".to_string(),
    };
    let fp_b = compute_mutation_fingerprint(&proposal_b);
    assert_ne!(fp_a, fp_b);

    assert_eq!(
        tracker.classify_attempt(task_id, &fp_b),
        AttemptStrategyClassification::ModifiedAttempt
    );
    let eval_b = tracker.evaluate_proposal(
        task_id,
        &proposal_b,
        FailureClassification::Compilation,
        1,
        1,
        1,
    );
    assert!(matches!(eval_b, BudgetEvaluation::Permitted { .. }));
}

// =========================================================================
// Scenario 9: Repair budget exhaustion triggering replan
// =========================================================================
#[tokio::test]
async fn test_scenario_9_repair_budget_exhaustion_triggers_replan() {
    let engine = ProductionRecoveryEngine::default();
    let mission_id = MissionId::new();
    let task_id = TaskId::new();

    // With retry_count = 3 (the default class ceiling for compilation), budget is exhausted
    let req =
        RecoveryStrategyRequest::new(mission_id, task_id, FailureClassification::Compilation, 3)
            .with_error_message(
                "error[E0432]: unresolved import `m31a::util`\n --> src/lib.rs:2:5",
            );

    let action = engine
        .determine_recovery(req)
        .await
        .expect("determine recovery");
    match action {
        RecoveryAction::Replan { reason } => {
            assert!(
                reason.contains("Repair budget exhausted") || reason.contains("unresolved import")
            );
        }
        other => panic!(
            "Expected Replan action on exhausted budget, got {:?}",
            other
        ),
    }
}

// =========================================================================
// Scenario 10: Deterministic non-retryable policy denial immediately halts
// =========================================================================
#[tokio::test]
async fn test_scenario_10_deterministic_policy_denial_aborts() {
    let engine = ProductionRecoveryEngine::default();
    let mission_id = MissionId::new();
    let task_id = TaskId::new();

    let req = RecoveryStrategyRequest::new(mission_id, task_id, FailureClassification::Policy, 0)
        .with_error_message("SEC-01 Policy Violation: illegal network socket opened");

    let action = engine
        .determine_recovery(req)
        .await
        .expect("determine recovery");
    match action {
        RecoveryAction::AbortMission { reason } => {
            assert!(reason.contains("non-retryable"));
        }
        other => panic!("Expected AbortMission on policy denial, got {:?}", other),
    }
}

// =========================================================================
// Scenario 11: Rollback action cleanly reverting tracked files
// =========================================================================
#[tokio::test]
async fn test_scenario_11_rollback_action() {
    let dir = tempdir().unwrap();
    let ws = dir.path();
    let fs: Arc<dyn FileSystemService> = Arc::new(LocalFileSystemProvider::new(ws).unwrap());

    // Create file
    let file = ws.join("src/state.rs");
    tokio::fs::create_dir_all(ws.join("src")).await.unwrap();
    tokio::fs::write(&file, "original content\n").await.unwrap();

    let proposal = ChangeProposal {
        id: m31a::kernel::change::ChangeProposalId::new(),
        task_id: TaskId::new(),
        mission_id: MissionId::new(),
        intent: ImplementationHypothesis::new("prob", "cause", "change", "res", "ver"),
        change_surface: ChangeSurface::new(vec!["src/state.rs".to_string()]),
        preconditions: vec![],
        mutations: vec![FileMutationProposal::new(
            "src/state.rs",
            FileMutationOp::Substring {
                old_content: "original content\n".to_string(),
                new_content: "broken mutated content\n".to_string(),
            },
            "mutate",
        )],
        assumptions: vec![],
        verification_plan: vec![],
        risk_level: None,
        timestamp: chrono::Utc::now(),
    };

    let authority = ChangeAuthority::new();
    let outcome = authority
        .execute_change_proposal(ws, &proposal, &fs, None, None)
        .await
        .unwrap();

    assert_eq!(
        tokio::fs::read_to_string(&file).await.unwrap(),
        "broken mutated content\n"
    );

    // Execute rollback using the applied change set
    if let Some(applied_set) = outcome.applied_set {
        applied_set.rollback(&fs).await.expect("rollback succeeds");
    }

    assert_eq!(
        tokio::fs::read_to_string(&file).await.unwrap(),
        "original content\n"
    );
}

// =========================================================================
// Scenario 12: AutonomyController closed loop: Verify -> Retry (honest)
// =========================================================================
struct MockVerifierForClosedLoop {
    verify_call_count: Arc<AtomicUsize>,
}

#[async_trait::async_trait]
impl VerificationEngine for MockVerifierForClosedLoop {
    async fn verify_task(
        &self,
        _req: TaskVerificationRequest,
    ) -> Result<VerificationOutcome, VerificationError> {
        let count = self.verify_call_count.fetch_add(1, Ordering::SeqCst);
        if count == 0 {
            // First verification fails with compiler error
            Ok(VerificationOutcome::Failed {
                reason: "error[E0432]: unresolved import `m31a::util`\n --> src/lib.rs:2:5"
                    .to_string(),
            })
        } else {
            // Re-verification after repair passes!
            Ok(VerificationOutcome::Passed)
        }
    }

    async fn verify_completion_gate(
        &self,
        _id: MissionId,
    ) -> Result<CompletionGateOutcome, VerificationError> {
        Ok(CompletionGateOutcome::Satisfied)
    }
}

struct MockSchedulerForClosedLoop;

#[async_trait::async_trait]
impl m31a::kernel::seams::WorkScheduler for MockSchedulerForClosedLoop {
    async fn find_ready_work(
        &self,
        _mission_id: MissionId,
    ) -> Result<
        m31a::kernel::seams::scheduler::ReadyWorkResponse,
        m31a::kernel::seams::scheduler::SchedulerError,
    > {
        Ok(m31a::kernel::seams::scheduler::ReadyWorkResponse {
            ready_tasks: vec![],
            blocked_tasks_count: 0,
        })
    }

    async fn is_work_complete(
        &self,
        _mission_id: MissionId,
    ) -> Result<bool, m31a::kernel::seams::scheduler::SchedulerError> {
        Ok(true)
    }

    async fn mark_task_started(
        &self,
        _task_id: TaskId,
        _agent_id: m31a::ids::AgentId,
    ) -> Result<(), m31a::kernel::seams::scheduler::SchedulerError> {
        Ok(())
    }

    async fn mark_task_completed(
        &self,
        _task_id: TaskId,
        _result: Option<m31a::kernel::plan::TaskResult>,
    ) -> Result<(), m31a::kernel::seams::scheduler::SchedulerError> {
        Ok(())
    }

    async fn mark_task_failed(
        &self,
        _task_id: TaskId,
        _error_message: String,
        _is_recoverable: bool,
    ) -> Result<(), m31a::kernel::seams::scheduler::SchedulerError> {
        Ok(())
    }
}

#[tokio::test]
async fn test_scenario_12_controller_closed_loop_verify_repair_reverify() {
    let dir = tempdir().unwrap();
    let ws = dir.path().to_path_buf();

    // Create source file to be repaired
    tokio::fs::create_dir_all(ws.join("src")).await.unwrap();
    tokio::fs::write(ws.join("src/lib.rs"), "pub fn test() {}\n")
        .await
        .unwrap();

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    run_migrations(&pool).await.unwrap();

    let bus = Arc::new(BroadcastEventBus::new(64));
    let verify_call_count = Arc::new(AtomicUsize::new(0));

    let deps =
        ControllerDependencies::production(pool.clone(), ws.clone(), ws.clone(), Some(bus.clone()))
            .with_verifier(Arc::new(MockVerifierForClosedLoop {
                verify_call_count: verify_call_count.clone(),
            }))
            .with_scheduler(Arc::new(MockSchedulerForClosedLoop));

    let mission_id = MissionId::new();
    let mission = Mission::new(mission_id, "Test closed loop recovery".to_string());
    deps.mission_repo().unwrap().insert(&mission).await.unwrap();

    let cancel = CancellationToken::new();
    let mut controller = AutonomyController::new(
        mission_id,
        AutonomyMode::Autonomous,
        deps,
        bus as Arc<dyn EventBus>,
        cancel,
    )
    .with_workspace_root(&ws);

    // Set active task and failed execution result
    let task_id = TaskId::new();
    controller.active_task = Some(WorkItem {
        task_id,
        title: "Closed Loop Task".into(),
        estimated_tokens: 100,
        required_capabilities: vec!["fs.write".into()],
        description: None,
        completion_criteria: Vec::new(),
        requirement_keys: Vec::new(),
        assumptions: Vec::new(),
        verification: None,

        prompt_ref: None,
    });

    // 1. Controller is at Verify stage
    controller.progress.current_stage = LoopStage::Verify;
    let outcome1 = controller.step().await.expect("step verify");
    // Verification failed -> advances to ClassifyFailure
    assert_eq!(outcome1, StageOutcome::Advance(LoopStage::ClassifyFailure));

    // 2. ClassifyFailure stage
    controller.progress.current_stage = LoopStage::ClassifyFailure;
    let outcome2 = controller.step().await.expect("step classify");
    assert_eq!(outcome2, StageOutcome::Advance(LoopStage::RecoverOrReplan));

    // 3. RecoverOrReplan stage: no structured repair exists without model
    // intelligence, so the engine honestly retries (SkipTo Observe) instead
    // of applying a fabricated repair and skipping to Verify (Phase 27).
    controller.progress.current_stage = LoopStage::RecoverOrReplan;
    let outcome3 = controller.step().await.expect("step recover_or_replan");
    assert_eq!(outcome3, StageOutcome::SkipTo(LoopStage::Observe));

    // No re-verification happened: the mock verifier observed exactly the
    // initial failing call (second call would require a real repair).
    assert_eq!(verify_call_count.load(Ordering::SeqCst), 1);
}

// =========================================================================
// Scenario 13: Multi-file cascading error repair proposal synthesis
// =========================================================================
#[tokio::test]
async fn test_scenario_13_multi_file_repair_proposal() {
    let proposal = ChangeProposal {
        id: m31a::kernel::change::ChangeProposalId::new(),
        task_id: TaskId::new(),
        mission_id: MissionId::new(),
        intent: ImplementationHypothesis::new(
            "multi-file",
            "api change",
            "update caller and callee",
            "ok",
            "test",
        ),
        change_surface: ChangeSurface::new(vec!["src/api.rs".into(), "src/caller.rs".into()]),
        preconditions: vec![],
        mutations: vec![
            FileMutationProposal::new(
                "src/api.rs",
                FileMutationOp::CreateNew {
                    content: "pub fn compute(a: i32, b: i32) -> i32 { a + b }".into(),
                },
                "update signature",
            ),
            FileMutationProposal::new(
                "src/caller.rs",
                FileMutationOp::CreateNew {
                    content: "pub fn call() -> i32 { crate::api::compute(1, 2) }".into(),
                },
                "update callsite",
            ),
        ],
        assumptions: vec![],
        verification_plan: vec![],
        risk_level: None,
        timestamp: chrono::Utc::now(),
    };

    let dir = tempdir().unwrap();
    let ws = dir.path();
    let fs: Arc<dyn FileSystemService> = Arc::new(LocalFileSystemProvider::new(ws).unwrap());
    let authority = ChangeAuthority::new();

    let outcome = authority
        .execute_change_proposal(ws, &proposal, &fs, None, None)
        .await
        .expect("multi-file repair applied");

    assert_eq!(outcome.files_modified.len(), 2);
    assert!(ws.join("src/api.rs").exists());
    assert!(ws.join("src/caller.rs").exists());
}

// =========================================================================
// Scenario 14: Durable recovery attempt audit logging in SQLite
// =========================================================================
#[tokio::test]
async fn test_scenario_14_durable_recovery_attempt_logging() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();

    run_migrations(&pool).await.unwrap();

    let mission_id = MissionId::new();
    let task_id = TaskId::new();

    // Insert dummy mission and task so foreign keys are satisfied
    sqlx::query("INSERT INTO missions (id, objective, status, created_at, updated_at) VALUES (?, 'test', 'running', datetime('now'), datetime('now'))")
        .bind(mission_id.as_bytes().as_slice())
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query("INSERT INTO tasks (id, mission_id, title, status, created_at, updated_at) VALUES (?, ?, 'task', 'pending', datetime('now'), datetime('now'))")
        .bind(task_id.as_bytes().as_slice())
        .bind(mission_id.as_bytes().as_slice())
        .execute(&pool)
        .await
        .unwrap();

    let tracker = RecoveryBudgetTracker::default();
    let engine = ProductionRecoveryEngine::with_components(
        m31a::recovery::classifier::FailureClassifier::new(),
        tracker.clone(),
        Some(pool.clone()),
    );

    let req =
        RecoveryStrategyRequest::new(mission_id, task_id, FailureClassification::Compilation, 0)
            .with_error_message(
                "error[E0432]: unresolved import `m31a::util`\n --> src/lib.rs:2:5",
            );

    let action = engine.determine_recovery(req).await.unwrap();
    // Phase 27: without a structured, evidence-derived fix operation there
    // is no Repair proposal; the engine honestly retries within budget.
    assert!(matches!(action, RecoveryAction::Retry { .. }));

    // Verify the honest retry attempt was durably recorded
    let records = RecoveryBudgetTracker::get_task_attempt_records(&pool, task_id)
        .await
        .expect("fetch attempts");

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].task_id, task_id);
    assert_eq!(records[0].failure_class, FailureClassification::Compilation);
    assert_eq!(records[0].action_taken, "retry");
}

// =========================================================================
// Scenario 15: Architectural single canonical ownership verification
// =========================================================================
#[test]
fn test_scenario_15_architectural_single_canonical_ownership() {
    // Verify there is only one canonical implementation of each recovery subsystem
    // Failure Classification: FailureClassifier
    let class = m31a::recovery::classifier::FailureClassifier::classify_deterministic(
        Some(101),
        "error[E0432]: unresolved import",
    );
    assert_eq!(class, FailureClassification::Compilation);

    // Diagnostician: ModelDiagnostician
    let env = ModelDiagnostician::capability_envelope();
    assert!(env.allowed_capabilities.contains("repo.read"));
    assert!(env.allowed_capabilities.contains("artifacts.read"));
    assert!(env.allowed_capabilities.contains("diagnosis.submit"));
    assert!(!env.allow_file_write);

    // Change Authority: ChangeAuthority
    let auth = ChangeAuthority::new();
    assert!(auth.db_pool.is_none());

    // Budget Tracker: RecoveryBudgetTracker
    let tracker = RecoveryBudgetTracker::default();
    assert_eq!(tracker.task_retry_ceiling, 3);
    assert_eq!(tracker.mission_retry_ceiling, 10);
}
