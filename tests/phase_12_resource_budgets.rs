//! Phase 12 Resource Budgeting, Two-Phase Enforcement & Confinement Suite (BST-01, BST-02, SEC-03).

use tempfile::tempdir;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

use m31a::budget::{
    ActualUsage, BudgetEnforcer, BudgetExhaustionAction, BudgetExhaustionPolicy, BudgetGrant,
    TaskEstimates,
};
use m31a::controller::budget_tracker::BudgetKind;
use m31a::ids::{JobId, MissionId};
use m31a::persistence::artifacts::quota::{ArtifactExemption, QuotaEnforcer};
use m31a::persistence::sqlite::repositories::SqliteBudgetRepository;
use m31a::persistence::sqlite::schema::initialize_database;
use m31a::process::confinement::ConfinementManager;
use m31a::state::budget::ResourceBudget;

#[test]
fn test_ten_dimensional_budget_struct() {
    let budget = ResourceBudget {
        max_wall_clock_seconds: Some(7200),
        max_concurrent_agents: Some(8),
        max_agent_steps: Some(100),
        max_model_calls: Some(250),
        max_tokens: Some(5_000_000),
        max_cost_usd: Some(50.0),
        max_cpu_seconds: Some(1200),
        max_memory_bytes: Some(8 * 1024 * 1024 * 1024),
        max_artifact_bytes: Some(1024 * 1024 * 1024),
        max_retries: Some(5),
    };

    // JSON round-trip
    let json = serde_json::to_string(&budget).unwrap();
    let parsed_json: ResourceBudget = serde_json::from_str(&json).unwrap();
    assert_eq!(budget, parsed_json);

    // TOML round-trip
    let toml_str = toml::to_string(&budget).unwrap();
    let parsed_toml: ResourceBudget = toml::from_str(&toml_str).unwrap();
    assert_eq!(budget, parsed_toml);
}

#[test]
fn test_two_phase_reservation_and_settlement() {
    let limits = ResourceBudget {
        max_tokens: Some(2000),
        max_cost_usd: Some(0.10),
        max_concurrent_agents: Some(2),
        ..Default::default()
    };
    let enforcer = BudgetEnforcer::new(limits);

    // Reserve Task 1
    let est1 = TaskEstimates {
        estimated_tokens: 1200,
        estimated_cost_usd: 0.05,
        requires_worker: true,
        estimated_artifact_bytes: 500,
    };
    let receipt1 = enforcer.reserve(&est1, true).expect("Task 1 reserved");
    assert_eq!(enforcer.active_workers(), 1);

    // Task 2 requests 1000 tokens. 1200 reserved + 1000 = 2200 > 2000 limit -> Rejection!
    let est2 = TaskEstimates {
        estimated_tokens: 1000,
        estimated_cost_usd: 0.02,
        requires_worker: true,
        estimated_artifact_bytes: 200,
    };
    let err = enforcer.reserve(&est2, true).unwrap_err();
    assert_eq!(err, BudgetExhaustionAction::PauseForApproval);

    // Task 1 settles with actual usage of only 800 tokens and $0.03
    let actual1 = ActualUsage {
        steps: 2,
        calls: 1,
        tokens: 800,
        cost_usd: 0.03,
        artifact_bytes: 400,
        retries: 0,
    };
    enforcer.settle(&receipt1, &actual1);

    assert_eq!(enforcer.active_workers(), 0);
    assert_eq!(enforcer.total_tokens_consumed(), 800);
    assert!((enforcer.total_cost_usd_consumed() - 0.03).abs() < 1e-4);

    // Now Task 2 can be admitted because consumed (800) + reserved (0) + estimated (1000) = 1800 <= 2000!
    let receipt2 = enforcer
        .reserve(&est2, true)
        .expect("Task 2 reserved after settlement");
    assert_eq!(enforcer.active_workers(), 1);

    // Cleanly settle Task 2
    let actual2 = ActualUsage {
        tokens: 950,
        cost_usd: 0.02,
        ..Default::default()
    };
    enforcer.settle(&receipt2, &actual2);
    assert_eq!(enforcer.active_workers(), 0);
    assert_eq!(enforcer.total_tokens_consumed(), 1750);
}

#[test]
fn test_budget_exhaustion_policy_transitions() {
    // Soft limits: interactive pauses, unattended blocks
    assert_eq!(
        BudgetExhaustionPolicy::determine_transition(BudgetKind::Tokens, true),
        BudgetExhaustionAction::PauseForApproval
    );
    assert_eq!(
        BudgetExhaustionPolicy::determine_transition(BudgetKind::Tokens, false),
        BudgetExhaustionAction::Block
    );
    assert_eq!(
        BudgetExhaustionPolicy::determine_transition(BudgetKind::CostUsd, false),
        BudgetExhaustionAction::Block
    );

    // Hard limits: always fails closed
    assert_eq!(
        BudgetExhaustionPolicy::determine_transition(BudgetKind::WallClock, true),
        BudgetExhaustionAction::FailClosed
    );
    assert_eq!(
        BudgetExhaustionPolicy::determine_transition(BudgetKind::CpuSeconds, false),
        BudgetExhaustionAction::FailClosed
    );
    assert_eq!(
        BudgetExhaustionPolicy::determine_transition(BudgetKind::MemoryBytes, true),
        BudgetExhaustionAction::FailClosed
    );
    assert_eq!(
        BudgetExhaustionPolicy::determine_transition(BudgetKind::ArtifactBytes, true),
        BudgetExhaustionAction::FailClosed
    );
    assert_eq!(
        BudgetExhaustionPolicy::determine_transition(BudgetKind::ConcurrentAgents, true),
        BudgetExhaustionAction::FailClosed
    );
}

#[tokio::test]
async fn test_process_confinement_rlimits() {
    let manager = ConfinementManager::new();
    let mut cmd = Command::new("true");
    let limits = ResourceBudget {
        max_cpu_seconds: Some(5),
        max_memory_bytes: Some(1024 * 1024 * 256), // 256 MB
        ..Default::default()
    };
    let job_id = JobId::new();

    let mut handle = manager.apply_confinement(&mut cmd, &limits, &job_id);

    // Spawn and run process
    let mut child = cmd.spawn().expect("Spawn confined command");
    if let Some(pid) = child.id() {
        handle.attach_pid(pid);
    }
    let status = child.wait().await.expect("Wait on child");
    assert!(status.success());
    handle.cleanup();
}

#[tokio::test]
async fn test_streaming_artifact_quota_enforcement() {
    let enforcer = QuotaEnforcer::new(Some(50), Some(200));

    // 1. Standard write within limits succeeds
    let mut buffer = Vec::new();
    let mut writer = enforcer.wrap_writer(&mut buffer, ArtifactExemption::Standard);
    writer
        .write_all(b"small payload under 50 bytes")
        .await
        .unwrap();
    assert_eq!(writer.bytes_written(), 28);
    assert_eq!(enforcer.current_cumulative_bytes(), 28);

    // 2. Standard write exceeding single artifact limit is aborted immediately
    let mut big_buffer = Vec::new();
    let mut big_writer = enforcer.wrap_writer(&mut big_buffer, ArtifactExemption::Standard);
    let chunk = [1u8; 30];
    big_writer.write_all(&chunk).await.unwrap();
    let err = big_writer.write_all(&chunk).await.unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::FileTooLarge);

    // 3. Protected artifact (verification / report) is exempt from limit
    let mut protected_buffer = Vec::new();
    let mut protected_writer =
        enforcer.wrap_writer(&mut protected_buffer, ArtifactExemption::Protected);
    let large_evidence = [7u8; 150];
    assert!(protected_writer.write_all(&large_evidence).await.is_ok());
    assert!(!enforcer.can_evict(ArtifactExemption::Protected));
}

#[tokio::test]
async fn test_durable_budget_grants() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("grants_test.db");
    let pool = initialize_database(&db_path).await.unwrap();

    let repo = SqliteBudgetRepository::new(pool);
    let mission_id = MissionId::new();

    let top_up = ResourceBudget {
        max_tokens: Some(2_000_000),
        max_cost_usd: Some(30.0),
        ..Default::default()
    };

    let grant = BudgetGrant::new(
        mission_id,
        "operator@example.com",
        "Top up budget for large refactor",
        top_up.clone(),
    );

    repo.record_grant(&grant).await.unwrap();

    let retrieved_grants = repo.get_grants(&mission_id).await.unwrap();
    assert_eq!(retrieved_grants.len(), 1);
    assert_eq!(retrieved_grants[0].actor, "operator@example.com");
    assert_eq!(
        retrieved_grants[0].reason,
        "Top up budget for large refactor"
    );
    assert_eq!(retrieved_grants[0].limits, top_up);

    let latest = repo
        .get_latest_grant(&mission_id)
        .await
        .unwrap()
        .expect("latest grant");
    assert_eq!(latest.grant_id, grant.grant_id);
}
