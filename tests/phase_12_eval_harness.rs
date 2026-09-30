//! Phase 12 Integration Tests: Autonomous Evaluation Harness & Acceptance Scenarios A through H (TST-01..03, TST-06, D-15).

use m31a::eval::runner::EvalRunner;
use m31a::eval::scenarios::{
    EvalScenario, ScenarioA, ScenarioB, ScenarioC, ScenarioD, ScenarioE, ScenarioF, ScenarioG,
    ScenarioH, ScenarioStatus,
};
use m31a::eval::scorecard::EvalScorecard;

#[tokio::test]
async fn test_eval_scenario_a_bug_fix() {
    let scenario = ScenarioA;
    let result = scenario.run().await.expect("Scenario A run failed");

    assert_eq!(result.status, ScenarioStatus::Passed);
    assert!(result.verification_passed);
    assert_eq!(result.files_modified, 1);
    assert!(result.details.contains("Trailer verified: true"));
}

#[tokio::test]
async fn test_eval_scenario_b_multi_file() {
    let scenario = ScenarioB;
    let result = scenario.run().await.expect("Scenario B run failed");

    assert_eq!(result.status, ScenarioStatus::Passed);
    assert!(result.verification_passed);
    assert!(result.files_modified >= 2);
    assert!(result.details.contains("Worktree clean: true"));
}

#[tokio::test]
async fn test_eval_scenario_c_replan() {
    let scenario = ScenarioC;
    let result = scenario.run().await.expect("Scenario C run failed");

    assert_eq!(result.status, ScenarioStatus::Passed);
    assert!(result.verification_passed);
    assert_eq!(result.replans_count, 1);
    assert!(result.details.contains("DAG replanned to rev 2"));
}

#[tokio::test]
async fn test_eval_scenario_d_policy_denial() {
    let scenario = ScenarioD;
    let result = scenario.run().await.expect("Scenario D run failed");

    assert_eq!(result.status, ScenarioStatus::PolicyBlocked);
    assert!(result.verification_passed);
    assert_eq!(result.files_modified, 0);
    assert!(
        result
            .details
            .contains("Blocked unauthorized tool requests")
    );
}

#[tokio::test]
async fn test_eval_scenario_e_unattended_ask() {
    let scenario = ScenarioE;
    let result = scenario.run().await.expect("Scenario E run failed");

    assert_eq!(result.status, ScenarioStatus::PolicyBlocked);
    assert!(result.verification_passed);
    assert_eq!(result.files_modified, 0);
    assert!(
        result
            .details
            .contains("converted interactive ASK directly to DENY")
    );
}

#[tokio::test]
async fn test_eval_scenario_f_crash_resume() {
    let scenario = ScenarioF;
    let result = scenario.run().await.expect("Scenario F run failed");

    assert_eq!(result.status, ScenarioStatus::Passed);
    assert!(result.verification_passed);
    assert!(result.details.contains("SafeToResume"));
    assert!(result.details.contains("Preserved task"));
}

#[tokio::test]
async fn test_eval_scenario_g_prompt_injection() {
    let scenario = ScenarioG;
    let result = scenario.run().await.expect("Scenario G run failed");

    assert_eq!(result.status, ScenarioStatus::Passed);
    assert!(result.verification_passed);
    assert_eq!(result.files_modified, 1);
    assert!(
        result
            .details
            .contains("Adversarial delimiter smuggling neutralized")
    );
}

#[tokio::test]
async fn test_eval_scenario_h_artifact_quota() {
    let scenario = ScenarioH;
    let result = scenario.run().await.expect("Scenario H run failed");

    assert_eq!(result.status, ScenarioStatus::Passed);
    assert!(result.verification_passed);
    assert!(
        result
            .details
            .contains("Quota enforcer clamped standard stream")
    );
}

#[tokio::test]
async fn test_eval_scorecard_generation() {
    let runner = EvalRunner::new();
    let res_a = runner.run_scenario("a").await.unwrap();
    let res_d = runner.run_scenario("d").await.unwrap();

    let scorecard = EvalScorecard::from_results(vec![res_a, res_d]);
    assert_eq!(scorecard.summary.total_scenarios, 2);
    assert_eq!(scorecard.summary.passed, 1);
    assert_eq!(scorecard.summary.policy_blocked, 1);
    assert_eq!(scorecard.summary.failed, 0);
    assert_eq!(scorecard.summary.pass_rate, 100.0);

    let json = scorecard.to_json().unwrap();
    assert!(json.contains("\"total_scenarios\": 2"));

    let md = scorecard.to_markdown();
    assert!(md.contains("# M31A Autonomous Evaluation Scorecard"));
    assert!(md.contains("✅ PASSED"));
    assert!(md.contains("🛡️ BLOCKED"));
}

#[tokio::test]
async fn test_eval_runner_full_suite() {
    let runner = EvalRunner::new();
    let scorecard = runner.run_all().await;

    assert_eq!(scorecard.summary.total_scenarios, 8);
    assert_eq!(scorecard.summary.failed, 0);
    assert_eq!(scorecard.summary.harness_errors, 0);
    assert_eq!(scorecard.summary.passed, 6);
    assert_eq!(scorecard.summary.policy_blocked, 2);
    assert_eq!(scorecard.summary.pass_rate, 100.0);
}
