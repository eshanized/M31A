//! Phase 05: Task DAG Scheduler Concurrency, Limits, and Seam Integration Tests (Phase 5 Exit Gate)
//!
//! Verifies:
//! 1. Multi-dimensional concurrency limits (global workers and role limits).
//! 2. Resource conflict serialization (Subtree lock on src/** defers descendant src/dag/**).
//! 3. Downstream failure invalidation propagating Blocked status to transitive closure.
//! 4. Cooperative cancellation releasing locks and marking non-terminal tasks Cancelled.
//! 5. Dual-track CPM and atomic SchedulerSnapshot telemetry projection.

use m31a::ids::{AgentId, MissionId};
use m31a::kernel::plan::{
    CandidatePlan, CandidateTask, CandidateTaskKey, CapabilityAccessMode, CapabilityRequirement,
    ResourceEstimate, VerificationStrategy,
};
use m31a::kernel::seams::scheduler::WorkScheduler;
use m31a::persistence::initialize_database;
use m31a::persistence::sqlite::repositories::SqliteMissionRepository;
use m31a::scheduler::concurrency::ConcurrencyLimits;
use m31a::scheduler::engine::SchedulerEngine;
use m31a::scheduler::resources::{LockMode, PathScope, ResourceKey, ResourceManager};
use m31a::state::mission::Mission;
use m31a::state::task::TaskResult;
use m31a::state_machine::agent::AgentRole;
use std::sync::Arc;
use tempfile::tempdir;

fn build_task(id: &str, deps: Vec<&str>, role: AgentRole) -> CandidateTask {
    let mut task = CandidateTask::new(
        id,
        format!("Objective for {}", id),
        role,
        VerificationStrategy::Compilation,
        ResourceEstimate::default(),
    );
    task.depends_on = deps.into_iter().map(CandidateTaskKey::new).collect();
    task.capabilities = vec![CapabilityRequirement::new(
        "workspace",
        CapabilityAccessMode::Write,
    )];
    task
}

async fn setup_fixture() -> (SchedulerEngine, MissionId, CandidatePlan, tempfile::TempDir) {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("scheduler_concurrency_test.db");
    let pool = initialize_database(&db_path).await.unwrap();

    let mission_repo = SqliteMissionRepository::new(pool.clone());
    let mission_id = MissionId::new();
    let mission = Mission::new(mission_id, "Concurrency Fixture Mission".to_string());
    mission_repo.insert(&mission).await.unwrap();

    let limits = ConcurrencyLimits {
        max_global_workers: 2, // Strict bound for test
        ..Default::default()
    };

    let resource_mgr = Arc::new(ResourceManager::new(Some(pool.clone()), 1));
    let engine = SchedulerEngine::new(pool, resource_mgr, limits, None);

    // 5-task workflow:
    // T1, T2: parallel initial tasks
    // T3: depends on T1 (exclusive lock on src/**)
    // T4: depends on T2 (exclusive lock on src/dag/**)
    // T5: independent task
    let plan = CandidatePlan::new(
        "fixture-plan",
        "5-task concurrency fixture",
        vec![
            build_task("task_1", vec![], AgentRole::implementer()),
            build_task("task_2", vec![], AgentRole::implementer()),
            build_task("task_3", vec!["task_1"], AgentRole::architect()),
            build_task("task_4", vec!["task_2"], AgentRole::architect()),
            build_task("task_5", vec![], AgentRole::verifier()),
        ],
    );

    (engine, mission_id, plan, dir)
}

#[tokio::test]
async fn test_concurrency_limits_enforced() {
    let (engine, mission_id, plan, _dir) = setup_fixture().await;
    engine.materialize_plan(mission_id, &plan).await.unwrap();

    // With max_global_workers = 2, find_ready_work must return at most 2 tasks
    let resp = engine.find_ready_work(mission_id).await.unwrap();
    assert_eq!(resp.ready_tasks.len(), 2);

    let agent_id = AgentId::new();
    engine
        .mark_task_started(resp.ready_tasks[0].task_id, agent_id)
        .await
        .unwrap();
    engine
        .mark_task_started(resp.ready_tasks[1].task_id, agent_id)
        .await
        .unwrap();

    // Capacity is now saturated (2 active workers)
    let resp2 = engine.find_ready_work(mission_id).await.unwrap();
    assert_eq!(resp2.ready_tasks.len(), 0);

    // Complete one task to free capacity
    engine
        .mark_task_completed(resp.ready_tasks[0].task_id, TaskResult::new("done"))
        .await
        .unwrap();

    // Now capacity frees up, so remaining task (T5) can be dispatched
    let resp3 = engine.find_ready_work(mission_id).await.unwrap();
    assert_eq!(resp3.ready_tasks.len(), 1);
}

#[tokio::test]
async fn test_resource_serialization_conflicts() {
    let (engine, mission_id, plan, _dir) = setup_fixture().await;
    engine.materialize_plan(mission_id, &plan).await.unwrap();

    // Complete T1 and T2 so T3 and T4 become ready
    let agent_id = AgentId::new();
    let resp1 = engine.find_ready_work(mission_id).await.unwrap();
    for item in &resp1.ready_tasks {
        engine
            .mark_task_started(item.task_id, agent_id)
            .await
            .unwrap();
        engine
            .mark_task_completed(item.task_id, TaskResult::new("done"))
            .await
            .unwrap();
    }

    // Explicitly set conflicting resource requirements: T3 requests src/**, T4 requests src/dag/**
    let res_t3 = ResourceKey::workspace("src", PathScope::Subtree);
    let res_t4 = ResourceKey::workspace("src/dag", PathScope::Subtree);

    let resp2 = engine.find_ready_work(mission_id).await.unwrap();
    assert!(resp2.ready_tasks.len() >= 2);

    let t3 = resp2.ready_tasks[0].task_id;
    let t4 = resp2.ready_tasks[1].task_id;

    engine
        .set_task_resources(t3, vec![(res_t3, LockMode::Exclusive)])
        .await;
    engine
        .set_task_resources(t4, vec![(res_t4, LockMode::Exclusive)])
        .await;

    // Start T3 (holds exclusive lock on src/**)
    engine.mark_task_started(t3, agent_id).await.unwrap();

    // T4 now conflicts with T3 on workspace path; cannot be started concurrently
    let start_t4 = engine.mark_task_started(t4, agent_id).await;
    assert!(
        start_t4.is_err(),
        "T4 must be rejected due to resource conflict with T3"
    );

    // Complete T3 (releases lock on src/**)
    engine
        .mark_task_completed(t3, TaskResult::new("T3 finished"))
        .await
        .unwrap();

    // T4 can now acquire its locks and start
    assert!(engine.mark_task_started(t4, agent_id).await.is_ok());
}

#[tokio::test]
async fn test_downstream_failure_propagation() {
    let (engine, mission_id, plan, _dir) = setup_fixture().await;
    engine.materialize_plan(mission_id, &plan).await.unwrap();

    let resp = engine.find_ready_work(mission_id).await.unwrap();
    let agent_id = AgentId::new();

    // Start T1 and fail it permanently (retryable = false)
    let t1_id = resp.ready_tasks[0].task_id;
    engine.mark_task_started(t1_id, agent_id).await.unwrap();
    engine
        .mark_task_failed(t1_id, "Fatal compiler error".into(), false)
        .await
        .unwrap();

    // Snapshot verifies T3 (dependent on T1) is now Blocked
    let snapshot = engine.get_snapshot(mission_id).await.unwrap();
    assert!(snapshot.queue_stats.blocked_count >= 1);

    // Independent tasks (T2, T5) can still proceed and complete
    let resp2 = engine.find_ready_work(mission_id).await.unwrap();
    assert!(!resp2.ready_tasks.is_empty());
    for item in resp2.ready_tasks {
        engine
            .mark_task_started(item.task_id, agent_id)
            .await
            .unwrap();
        engine
            .mark_task_completed(item.task_id, TaskResult::new("independent done"))
            .await
            .unwrap();
    }
}

#[tokio::test]
async fn test_cooperative_cancellation() {
    let (engine, mission_id, plan, _dir) = setup_fixture().await;
    engine.materialize_plan(mission_id, &plan).await.unwrap();

    let resp = engine.find_ready_work(mission_id).await.unwrap();
    let agent_id = AgentId::new();
    let t1_id = resp.ready_tasks[0].task_id;
    engine.mark_task_started(t1_id, agent_id).await.unwrap();

    // Cancel mission cooperatively
    engine.cancel_mission(mission_id).await.unwrap();

    // Leases must be fully cleared
    assert_eq!(engine.resource_manager().active_leases_count(), 0);

    // Snapshot verifies all non-terminal tasks reached Cancelled
    let snapshot = engine.get_snapshot(mission_id).await.unwrap();
    assert_eq!(snapshot.queue_stats.running_count, 0);
    assert_eq!(snapshot.queue_stats.ready_count, 0);
    assert_eq!(snapshot.queue_stats.cancelled_count, 5);

    // Work is complete because all tasks reached terminal states
    assert!(engine.is_work_complete(mission_id).await.unwrap());
}

#[tokio::test]
async fn test_cpm_and_snapshot_telemetry() {
    let (engine, mission_id, plan, _dir) = setup_fixture().await;
    engine.materialize_plan(mission_id, &plan).await.unwrap();

    let snapshot = engine.get_snapshot(mission_id).await.unwrap();
    assert_eq!(snapshot.revision, 1);
    assert_eq!(snapshot.max_workers, 2);
    assert!(!snapshot.critical_path.is_empty());
    assert!(!snapshot.wave_tiers.is_empty());
}
