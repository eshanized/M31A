//! Deterministic regression tests for Phase 29.6 Gap B: Recovery Circuit Breaker & Unrepairable State.
//!
//! Validates:
//! 1. One recovery attempt succeeds.
//! 2. First repair fails but a second materially different repair succeeds.
//! 3. Identical recovery candidate repeatedly fails and is rejected.
//! 4. Semantically equivalent failures trigger the circuit breaker across replans.
//! 5. Recovery budget (task ceiling and mission ceiling) is strictly respected.
//! 6. Exhausted recovery becomes an explicit unrecoverable mission state (`AbortMission`).
//! 7. No false-success handoff occurs; failure evidence is preserved.
//! 8. Existing successful recovery paths remain intact.

use m31a::ids::{MissionId, TaskId};
use m31a::kernel::change::{
    ChangeProposal, ChangeProposalId, ChangeSurface, FileMutationOp, FileMutationProposal,
    ImplementationHypothesis,
};
use m31a::kernel::seams::recovery::{
    FailureClassification, RecoveryAction, RecoveryEngine, RecoveryStrategyRequest,
};
use m31a::recovery::adapter::ProductionRecoveryEngine;
use m31a::recovery::budget::{
    AttemptStrategyClassification, BudgetEvaluation, RecoveryBudgetTracker,
    SemanticFailureSignature, compute_mutation_fingerprint,
};

use m31a::verification::diagnostician::{
    DiagnosticHypothesis, ModelDiagnostician, RecoveryRecommendation,
};
use std::sync::Arc;

#[tokio::test]
async fn test_1_one_recovery_attempt_succeeds() {
    let mission_id = MissionId::new();
    let task_id = TaskId::new();

    let proposal = ChangeProposal {
        id: ChangeProposalId::new(),
        task_id,
        mission_id,
        intent: ImplementationHypothesis::new("prob", "cause", "change", "res", "ver"),
        change_surface: ChangeSurface::new(vec!["src/lib.rs".to_string()]),
        preconditions: vec![],
        mutations: vec![FileMutationProposal::new(
            "src/lib.rs",
            FileMutationOp::CreateNew {
                content: "pub fn fix() {}".to_string(),
            },
            "attempt 1",
        )],
        assumptions: vec![],
        verification_plan: vec![],
        risk_level: None,
        timestamp: chrono::Utc::now(),
    };

    let simulated = DiagnosticHypothesis {
        failure_class: FailureClassification::Compilation,
        semantic_subclass: Some("missing_import".to_string()),
        root_cause: "Missing import in src/lib.rs".to_string(),
        cascading_symptoms: Vec::new(),
        affected_files: vec!["src/lib.rs".to_string()],
        supporting_evidence: Vec::new(),
        contradicting_evidence: Vec::new(),
        confidence_score: 95,
        uncertainty_notes: None,
        recommended_action: RecoveryRecommendation::Repair,
        suggested_fix: Some("Add missing import".to_string()),
        repair_proposal: Some(proposal),
        invalidating_conditions: Vec::new(),
    };

    let diagnostician = ModelDiagnostician::new().with_simulated_hypothesis(simulated);
    let engine = ProductionRecoveryEngine::default().with_diagnostician(Arc::new(diagnostician));

    let err_msg = "error[E0432]: unresolved import `m31a::util`\n --> src/lib.rs:2:5";
    let req =
        RecoveryStrategyRequest::new(mission_id, task_id, FailureClassification::Compilation, 0)
            .with_error_message(err_msg);

    let action = engine
        .determine_recovery(req)
        .await
        .expect("determine recovery");

    match action {
        RecoveryAction::Repair { proposal, reason } => {
            assert!(!reason.is_empty());
            assert!(!proposal.mutations.is_empty());
            assert_eq!(proposal.mutations[0].path, "src/lib.rs");
        }
        other => panic!(
            "Expected Repair action for first import error, got {:?}",
            other
        ),
    }
}

#[tokio::test]
async fn test_2_first_repair_fails_second_different_repair_succeeds() {
    let tracker = RecoveryBudgetTracker::default();
    let mission_id = MissionId::new();
    let task_id = TaskId::new();

    let proposal_1 = ChangeProposal {
        id: ChangeProposalId::new(),
        task_id,
        mission_id,
        intent: ImplementationHypothesis::new("prob", "cause", "change 1", "res", "ver"),
        change_surface: ChangeSurface::new(vec!["src/lib.rs".to_string()]),
        preconditions: vec![],
        mutations: vec![FileMutationProposal::new(
            "src/lib.rs",
            FileMutationOp::CreateNew {
                content: "use m31a::util_v1;".to_string(),
            },
            "attempt 1",
        )],
        assumptions: vec![],
        verification_plan: vec![],
        risk_level: None,
        timestamp: chrono::Utc::now(),
    };

    let fp_1 = compute_mutation_fingerprint(&proposal_1);
    tracker.record_mission_attempt_fingerprint(mission_id, task_id, &fp_1);

    // Second proposal is materially different
    let proposal_2 = ChangeProposal {
        id: ChangeProposalId::new(),
        task_id,
        mission_id,
        intent: ImplementationHypothesis::new("prob", "cause", "change 2", "res", "ver"),
        change_surface: ChangeSurface::new(vec!["src/lib.rs".to_string()]),
        preconditions: vec![],
        mutations: vec![FileMutationProposal::new(
            "src/lib.rs",
            FileMutationOp::CreateNew {
                content: "use m31a::util_v2;".to_string(),
            },
            "attempt 2",
        )],
        assumptions: vec![],
        verification_plan: vec![],
        risk_level: None,
        timestamp: chrono::Utc::now(),
    };

    let fp_2 = compute_mutation_fingerprint(&proposal_2);
    assert_ne!(fp_1, fp_2);

    let classification = tracker.classify_attempt_with_mission(Some(mission_id), task_id, &fp_2);
    assert_eq!(
        classification,
        AttemptStrategyClassification::ModifiedAttempt
    );

    let eval = tracker.evaluate_proposal_for_mission(
        Some(mission_id),
        task_id,
        &proposal_2,
        FailureClassification::Compilation,
        1,
        1,
        1,
    );
    assert!(matches!(eval, BudgetEvaluation::Permitted { .. }));
}

#[tokio::test]
async fn test_3_identical_recovery_candidate_repeatedly_fails() {
    let tracker = RecoveryBudgetTracker::default();
    let mission_id = MissionId::new();
    let task_id = TaskId::new();

    let proposal = ChangeProposal {
        id: ChangeProposalId::new(),
        task_id,
        mission_id,
        intent: ImplementationHypothesis::new("prob", "cause", "change", "res", "ver"),
        change_surface: ChangeSurface::new(vec!["src/lib.rs".to_string()]),
        preconditions: vec![],
        mutations: vec![FileMutationProposal::new(
            "src/lib.rs",
            FileMutationOp::CreateNew {
                content: "use m31a::util;".to_string(),
            },
            "identical attempt",
        )],
        assumptions: vec![],
        verification_plan: vec![],
        risk_level: None,
        timestamp: chrono::Utc::now(),
    };

    let fp = compute_mutation_fingerprint(&proposal);
    tracker.record_mission_attempt_fingerprint(mission_id, task_id, &fp);

    // Attempting same proposal again even with a different TaskId in the same mission
    let new_task_id = TaskId::new();
    let classification = tracker.classify_attempt_with_mission(Some(mission_id), new_task_id, &fp);
    assert_eq!(
        classification,
        AttemptStrategyClassification::RepeatedIdentical
    );

    let eval = tracker.evaluate_proposal_for_mission(
        Some(mission_id),
        new_task_id,
        &proposal,
        FailureClassification::Compilation,
        1,
        1,
        1,
    );
    assert!(matches!(eval, BudgetEvaluation::RepeatedIdentical { .. }));
}

#[tokio::test]
async fn test_4_semantically_equivalent_failures_trigger_circuit_breaker() {
    let tracker = RecoveryBudgetTracker::new(5, 10).with_circuit_breaker_threshold(3);
    let engine = ProductionRecoveryEngine::with_components(
        m31a::recovery::classifier::FailureClassifier::new(),
        tracker,
        None,
    );

    let mission_id = MissionId::new();
    let err_msg = "error[E0308]: mismatched types expected struct `String`, found `&str`\n --> src/main.rs:10:15";

    // Attempt 1
    let req1 = RecoveryStrategyRequest::new(
        mission_id,
        TaskId::new(),
        FailureClassification::Compilation,
        0,
    )
    .with_error_message(err_msg);
    let action1 = engine.determine_recovery(req1).await.unwrap();
    assert!(!matches!(action1, RecoveryAction::AbortMission { .. }));

    // Attempt 2 (re-plan task with different task ID, but same semantic failure)
    let err_msg_attempt2 = "error[E0308]: mismatched types expected struct `String`, found `&str`\n --> src/main.rs:10:15\n (timestamp 2026-09-27T00:00:00Z)";
    let req2 = RecoveryStrategyRequest::new(
        mission_id,
        TaskId::new(),
        FailureClassification::Compilation,
        1,
    )
    .with_error_message(err_msg_attempt2);
    let action2 = engine.determine_recovery(req2).await.unwrap();
    assert!(!matches!(action2, RecoveryAction::AbortMission { .. }));

    // Attempt 3 reaches circuit breaker threshold -> must trip and abort!
    let req3 = RecoveryStrategyRequest::new(
        mission_id,
        TaskId::new(),
        FailureClassification::Compilation,
        2,
    )
    .with_error_message(err_msg);
    let action3 = engine.determine_recovery(req3).await.unwrap();

    match action3 {
        RecoveryAction::AbortMission { reason } => {
            assert!(
                reason.contains("Recovery circuit breaker tripped"),
                "Expected circuit breaker reason, got: {}",
                reason
            );
        }
        other => panic!(
            "Expected AbortMission from tripped circuit breaker, got: {:?}",
            other
        ),
    }
}

#[tokio::test]
async fn test_5_recovery_budget_is_respected() {
    let tracker = RecoveryBudgetTracker::new(2, 4);
    let engine = ProductionRecoveryEngine::with_components(
        m31a::recovery::classifier::FailureClassifier::new(),
        tracker,
        None,
    );

    let mission_id = MissionId::new();

    // 1. Task retry limit check (retry_count >= task_ceiling)
    let req_task_exhausted = RecoveryStrategyRequest::new(
        mission_id,
        TaskId::new(),
        FailureClassification::Compilation,
        2,
    )
    .with_error_message("error[E0432]: unresolved import");

    let action = engine.determine_recovery(req_task_exhausted).await.unwrap();
    // With retry_count = 2 and task ceiling = 2, class limit or repair budget replans or aborts
    assert!(matches!(
        action,
        RecoveryAction::Replan { .. } | RecoveryAction::AbortMission { .. }
    ));

    // 2. Mission retry ceiling check (mission attempts >= mission_retry_ceiling = 4)
    let req_mission_exhausted = RecoveryStrategyRequest::new(
        mission_id,
        TaskId::new(),
        FailureClassification::Compilation,
        4, // simulates mission_retries >= 4
    )
    .with_error_message("error[E0432]: unresolved import");

    let action_mission = engine
        .determine_recovery(req_mission_exhausted)
        .await
        .unwrap();
    match action_mission {
        RecoveryAction::AbortMission { reason } => {
            assert!(
                reason.contains("Mission recovery ceiling exhausted")
                    || reason.contains("Recovery circuit breaker tripped"),
                "Expected mission ceiling or circuit breaker exhaustion, got: {}",
                reason
            );
        }
        other => panic!(
            "Expected AbortMission on mission budget exhaustion, got {:?}",
            other
        ),
    }
}

#[tokio::test]
async fn test_6_exhausted_recovery_becomes_explicit_unrecoverable_mission_state() {
    let tracker = RecoveryBudgetTracker::new(1, 1);
    let engine = ProductionRecoveryEngine::with_components(
        m31a::recovery::classifier::FailureClassifier::new(),
        tracker,
        None,
    );

    let mission_id = MissionId::new();
    let task_id = TaskId::new();

    let req =
        RecoveryStrategyRequest::new(mission_id, task_id, FailureClassification::Compilation, 1)
            .with_error_message("error[E0432]: unresolved import");

    let action = engine.determine_recovery(req).await.unwrap();
    match action {
        RecoveryAction::AbortMission { reason } => {
            assert!(!reason.is_empty());
        }
        other => panic!(
            "Expected explicit AbortMission on exhausted recovery, got {:?}",
            other
        ),
    }
}

#[tokio::test]
async fn test_7_no_false_success_handoff_occurs() {
    let tracker = RecoveryBudgetTracker::new(1, 1);
    let engine = ProductionRecoveryEngine::with_components(
        m31a::recovery::classifier::FailureClassifier::new(),
        tracker,
        None,
    );

    let mission_id = MissionId::new();
    let task_id = TaskId::new();

    let req =
        RecoveryStrategyRequest::new(mission_id, task_id, FailureClassification::Compilation, 1)
            .with_error_message("error[E0432]: cannot find module");

    let action = engine.determine_recovery(req).await.unwrap();
    // Must NOT be SkipTask, Retry, or any action that pretends success
    assert!(!matches!(action, RecoveryAction::SkipTask));
    assert!(matches!(action, RecoveryAction::AbortMission { .. }));

    // Verify SemanticFailureSignature preserves failure evidence
    let norm = m31a::recovery::budget::normalize_error_message("error[E0432]: cannot find module");
    let sig =
        SemanticFailureSignature::new(FailureClassification::Compilation, "src/lib.rs", norm, None);
    assert_eq!(sig.failure_class, FailureClassification::Compilation);
    assert!(sig.normalized_error.contains("error[e0432]"));
}

#[tokio::test]
async fn test_8_existing_successful_recovery_paths_remain_intact() {
    let engine = ProductionRecoveryEngine::default();
    let mission_id = MissionId::new();
    let task_id = TaskId::new();

    // Transient failure with empty error detail still calculates backoff retry
    let req =
        RecoveryStrategyRequest::new(mission_id, task_id, FailureClassification::Transient, 0);

    let action = engine.determine_recovery(req).await.unwrap();
    match action {
        RecoveryAction::Retry { delay_ms } => {
            assert!(delay_ms > 0);
        }
        other => panic!(
            "Expected Retry action for transient failure, got {:?}",
            other
        ),
    }
}
