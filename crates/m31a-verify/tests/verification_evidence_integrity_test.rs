//! Adversarial verification evidence integrity and goal-backward tests.

use m31a_verify::{
    AgentCompletionReport, AgentPlan, CompletionOutcome, CriterionEvidence, EvidenceRecord,
    EvidenceType, GoalBackwardVerifier, PlanCriterion, TruthStatus, VerificationOutcome,
    VerificationTimeline,
};
use std::time::Duration;
use tempfile::tempdir;

#[test]
fn test_reject_foreign_criterion_evidence() {
    let mut timeline = VerificationTimeline::new();
    timeline.record_plan(AgentPlan {
        goal: "Implement secure authentication and rate limiting".to_string(),
        steps: vec!["Auth".to_string(), "Rate limit".to_string()],
        criteria: vec![
            PlanCriterion {
                id: "auth-crit".to_string(),
                description: "JWT tokens signed and verified".to_string(),
            },
            PlanCriterion {
                id: "rate-limit-crit".to_string(),
                description: "Leaky bucket rejects bursts".to_string(),
            },
        ],
        requires_preview: false,
    });

    timeline.record_edit(1);

    // Record valid evidence for auth-crit
    timeline.record_criterion_evidence(EvidenceRecord {
        requirement_id: "auth-crit".to_string(),
        evidence_id: "ev-auth-jwt".to_string(),
        evidence_type: EvidenceType::UnitTest,
        executed_operation: "cargo test test_jwt".to_string(),
        execution_timestamp: 1700000000,
        actual_result: "test result: ok. 4 passed; 0 failed".to_string(),
        success: true,
        relevant_artifacts: vec!["crates/auth/src/jwt.rs".to_string()],
        tool_execution_reference: Some("call-test-1".to_string()),
        test_and_diagnostic_references: vec!["test_jwt_signature".to_string()],
        environment_info: "linux-x86_64".to_string(),
        verification_limitations: None,
        execution_turn: Some(2),
        exit_code: Some(0),
        tests_passed: Some(4),
        tests_failed: Some(0),
        tests_total: Some(4),
        duration_ms: Some(100),
        status: Some(m31a_verify::VerificationStatus::Success),
    });

    // Adversarial report: tries to use ev-auth-jwt for rate-limit-crit!
    let adversarial_report = AgentCompletionReport {
        outcome: CompletionOutcome::Complete,
        summary: "Both features verified".to_string(),
        criteria: vec![
            CriterionEvidence {
                id: "auth-crit".to_string(),
                evidence: vec!["ev-auth-jwt".to_string()],
            },
            CriterionEvidence {
                id: "rate-limit-crit".to_string(),
                evidence: vec!["ev-auth-jwt".to_string()], // Foreign evidence!
            },
        ],
    };

    let res = timeline.evaluate_completion(&adversarial_report);
    assert!(
        res.is_err(),
        "Expected failure due to foreign criterion evidence substitution"
    );
    let problems = res.unwrap_err();
    assert!(
        problems.iter().any(|p| p
            .contains("foreign evidence ID 'ev-auth-jwt' belonging to requirement 'auth-crit'")),
        "Expected error diagnosing foreign evidence mismatch, got: {:?}",
        problems
    );
}

#[test]
fn test_reject_failed_evidence() {
    let mut timeline = VerificationTimeline::new();
    timeline.record_plan(AgentPlan {
        goal: "Sanitize SQL queries".to_string(),
        steps: vec!["Add prepared statement".to_string()],
        criteria: vec![PlanCriterion {
            id: "sql-injection-proof".to_string(),
            description: "No raw concatenation allowed".to_string(),
        }],
        requires_preview: false,
    });

    timeline.record_edit(1);

    // Record failed test evidence
    timeline.record_criterion_evidence(EvidenceRecord {
        requirement_id: "sql-injection-proof".to_string(),
        evidence_id: "ev-sql-failed".to_string(),
        evidence_type: EvidenceType::SecurityTest,
        executed_operation: "cargo test test_sql_injection".to_string(),
        execution_timestamp: 1700000000,
        actual_result: "FAILED: 1 test failed: test_sql_injection".to_string(),
        success: false, // Failed!
        relevant_artifacts: vec![],
        tool_execution_reference: None,
        test_and_diagnostic_references: vec![],
        environment_info: "linux".to_string(),
        verification_limitations: None,
        execution_turn: Some(2),
        exit_code: Some(1),
        tests_passed: Some(0),
        tests_failed: Some(1),
        tests_total: Some(1),
        duration_ms: Some(120),
        status: Some(m31a_verify::VerificationStatus::Failure {
            exit_code: Some(1),
            diagnostics: "1 test failed".to_string(),
        }),
    });

    let report = AgentCompletionReport {
        outcome: CompletionOutcome::Complete,
        summary: "Done".to_string(),
        criteria: vec![CriterionEvidence {
            id: "sql-injection-proof".to_string(),
            evidence: vec!["ev-sql-failed".to_string()],
        }],
    };

    let res = timeline.evaluate_completion(&report);
    assert!(res.is_err());
    let problems = res.unwrap_err();
    assert!(
        problems
            .iter()
            .any(|p| p.contains("references failed evidence ID 'ev-sql-failed'")),
        "Expected rejection of failed evidence, got: {:?}",
        problems
    );
}

#[test]
fn test_reject_empty_plan_criteria_and_duplicate_reporting() {
    let mut timeline = VerificationTimeline::new();
    timeline.record_plan(AgentPlan {
        goal: "Empty plan".to_string(),
        steps: vec!["Step 1".to_string()],
        criteria: vec![], // Empty criteria!
        requires_preview: false,
    });

    let report = AgentCompletionReport {
        outcome: CompletionOutcome::Complete,
        summary: "Done without criteria".to_string(),
        criteria: vec![],
    };

    let res = timeline.evaluate_completion(&report);
    assert!(res.is_err());
    let problems = res.unwrap_err();
    assert!(
        problems
            .iter()
            .any(|p| p.contains("Empty plan criteria rejected")),
        "Expected rejection of empty plan criteria, got: {:?}",
        problems
    );

    // Duplicate criteria test
    let mut timeline2 = VerificationTimeline::new();
    timeline2.record_plan(AgentPlan {
        goal: "Plan with one criteria".to_string(),
        steps: vec!["Step 1".to_string()],
        criteria: vec![PlanCriterion {
            id: "c1".to_string(),
            description: "Build ok".to_string(),
        }],
        requires_preview: false,
    });

    let duplicate_report = AgentCompletionReport {
        outcome: CompletionOutcome::Complete,
        summary: "Duplicate".to_string(),
        criteria: vec![
            CriterionEvidence {
                id: "c1".to_string(),
                evidence: vec!["ev-1".to_string()],
            },
            CriterionEvidence {
                id: "c1".to_string(),
                evidence: vec!["ev-1".to_string()],
            },
        ],
    };

    let res2 = timeline2.evaluate_completion(&duplicate_report);
    assert!(res2.is_err());
    let problems2 = res2.unwrap_err();
    assert!(
        problems2
            .iter()
            .any(|p| p.contains("Duplicate criterion reported: 'c1'")),
        "Expected duplicate criterion rejection, got: {:?}",
        problems2
    );
}

#[test]
fn test_reject_stale_evidence_prior_to_last_edit() {
    let mut timeline = VerificationTimeline::new();
    timeline.record_plan(AgentPlan {
        goal: "Update parser".to_string(),
        steps: vec!["Run test".to_string(), "Modify parser".to_string()],
        criteria: vec![PlanCriterion {
            id: "c1".to_string(),
            description: "Parser unit tests".to_string(),
        }],
        requires_preview: false,
    });

    // Evidence recorded at turn 1
    timeline.record_criterion_evidence(EvidenceRecord {
        requirement_id: "c1".to_string(),
        evidence_id: "ev-turn-1".to_string(),
        evidence_type: EvidenceType::UnitTest,
        executed_operation: "cargo test".to_string(),
        execution_timestamp: 1700000000,
        actual_result: "test result: ok. 5 passed; 0 failed".to_string(),
        success: true,
        relevant_artifacts: vec![],
        tool_execution_reference: None,
        test_and_diagnostic_references: vec![],
        environment_info: "linux".to_string(),
        verification_limitations: None,
        execution_turn: Some(1),
        exit_code: Some(0),
        tests_passed: Some(5),
        tests_failed: Some(0),
        tests_total: Some(5),
        duration_ms: Some(150),
        status: Some(m31a_verify::VerificationStatus::Success),
    });

    // Modification happened at turn 2 (AFTER evidence was collected)
    timeline.record_edit(2);

    let report = AgentCompletionReport {
        outcome: CompletionOutcome::Complete,
        summary: "Parser modified".to_string(),
        criteria: vec![CriterionEvidence {
            id: "c1".to_string(),
            evidence: vec!["ev-turn-1".to_string()],
        }],
    };

    let res = timeline.evaluate_completion(&report);
    assert!(res.is_err());
    let problems = res.unwrap_err();
    assert!(
        problems
            .iter()
            .any(|p| p.contains("recorded at turn 1 before latest edit at turn 2")),
        "Expected rejection of stale pre-edit evidence, got: {:?}",
        problems
    );
}

#[test]
fn test_reject_zero_test_execution_proof() {
    let mut timeline = VerificationTimeline::new();
    timeline.record_plan(AgentPlan {
        goal: "Write tests".to_string(),
        steps: vec!["Add test file".to_string()],
        criteria: vec![PlanCriterion {
            id: "c1".to_string(),
            description: "Tests must execute".to_string(),
        }],
        requires_preview: false,
    });

    timeline.record_edit(1);

    // Record evidence where command succeeded exit code 0, but 0 tests were executed
    timeline.record_criterion_evidence(EvidenceRecord {
        requirement_id: "c1".to_string(),
        evidence_id: "ev-empty-runner".to_string(),
        evidence_type: EvidenceType::UnitTest,
        executed_operation: "cargo test non_existent_filter".to_string(),
        execution_timestamp: 1700000000,
        actual_result: "running 0 tests\n\ntest result: ok. 0 passed; 0 failed; 0 ignored"
            .to_string(),
        success: true,
        relevant_artifacts: vec![],
        tool_execution_reference: None,
        test_and_diagnostic_references: vec![],
        environment_info: "linux".to_string(),
        verification_limitations: None,
        execution_turn: Some(2),
        exit_code: Some(0),
        tests_passed: Some(0),
        tests_failed: Some(0),
        tests_total: Some(0),
        duration_ms: Some(20),
        status: Some(m31a_verify::VerificationStatus::Success),
    });

    let report = AgentCompletionReport {
        outcome: CompletionOutcome::Complete,
        summary: "Empty tests passed".to_string(),
        criteria: vec![CriterionEvidence {
            id: "c1".to_string(),
            evidence: vec!["ev-empty-runner".to_string()],
        }],
    };

    let res = timeline.evaluate_completion(&report);
    assert!(res.is_err());
    let problems = res.unwrap_err();
    assert!(
        problems
            .iter()
            .any(|p| p.contains("produced no test execution proof (0 tests executed)")),
        "Expected rejection of empty 0-test execution proof, got: {:?}",
        problems
    );
}

#[tokio::test]
async fn test_goal_backward_uncertain_on_empty_observable_inputs() {
    let dir = tempdir().unwrap();

    let report = GoalBackwardVerifier::verify_phase(
        dir.path(),
        "Phase 1",
        &["Truth 1: Feature XYZ works flawlessly".to_string()],
        &[], // No artifacts
        &[], // No wiring
        &[], // No test commands
        Duration::from_secs(5),
    )
    .await
    .unwrap();

    assert_eq!(report.outcome, VerificationOutcome::GapsFound);
    assert_eq!(report.truths.len(), 1);
    match &report.truths[0].status {
        TruthStatus::Uncertain { reason } => {
            assert!(reason.contains("No observable artifacts or automated test commands"));
        }
        other => panic!("Expected TruthStatus::Uncertain, got: {:?}", other),
    }
    assert!(
        report
            .gaps
            .iter()
            .any(|g| g.contains("No observable validation artifacts or automated test checks"))
    );
}

#[tokio::test]
async fn test_goal_backward_timeout_reaps_process_group() {
    let dir = tempdir().unwrap();

    let report = GoalBackwardVerifier::verify_phase(
        dir.path(),
        "Phase 2",
        &["Must not hang".to_string()],
        &[],
        &[],
        &["sleep 10".to_string()], // Will time out
        Duration::from_millis(200),
    )
    .await
    .unwrap();

    assert_eq!(report.outcome, VerificationOutcome::Failed);
    assert_eq!(report.test_results.len(), 1);
    assert!(!report.test_results[0].passed);
    assert_eq!(report.test_results[0].exit_code, 124);
    assert!(report.gaps.iter().any(|g| g.contains("timed out")));
}

#[test]
fn test_mutation_audit_distinguishes_status() {
    let dir = tempdir().unwrap();
    let src_file = dir.path().join("logic.rs");
    std::fs::write(
        &src_file,
        "pub fn is_positive(x: i32) -> bool {\n    x > 0\n}\n",
    )
    .unwrap();

    // 1. Mutant killed when test fails on mutant
    // Command 'false' always fails, meaning the mutant was killed by failing test
    let res_killed = m31a_verify::integrity::TestIntegrityAuditor::evaluate_mutation(
        dir.path(),
        &src_file,
        "false",
    )
    .unwrap();

    assert!(res_killed.killed);
    assert_eq!(res_killed.status, Some(m31a_verify::MutationStatus::Killed));

    // 2. Mutant survived when test passes on mutant
    // Command 'true' always passes, meaning the test failed to detect the mutant
    let res_survived = m31a_verify::integrity::TestIntegrityAuditor::evaluate_mutation(
        dir.path(),
        &src_file,
        "true",
    )
    .unwrap();

    assert!(!res_survived.killed);
    assert_eq!(
        res_survived.status,
        Some(m31a_verify::MutationStatus::Survived)
    );
}
