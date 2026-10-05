//! Phase Remediation Regression Test Suite: Task State Machine, Scheduler Correctness, Cancellation, and Review

use futures::StreamExt;
use m31a::events::EventBus;
use m31a::events::bus::{BroadcastEventBus, EventFilter};
use m31a::events::types::EventType;
use m31a::ids::{AgentId, MissionId};
use m31a::kernel::plan::{
    CandidatePlan, CandidateTask, CandidateTaskKey, CapabilityAccessMode, CapabilityRequirement,
    ResourceEstimate, VerificationStrategy,
};
use m31a::kernel::seams::scheduler::WorkScheduler;
use m31a::persistence::initialize_database;
use m31a::persistence::sqlite::repositories::{SqliteMissionRepository, SqliteTaskRepository};
use m31a::scheduler::concurrency::ConcurrencyLimits;
use m31a::scheduler::engine::SchedulerEngine;
use m31a::scheduler::resources::ResourceManager;
use m31a::state::mission::Mission;
use m31a::state_machine::TaskState;
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

async fn setup_test_engine(
    limits: ConcurrencyLimits,
    bus: Arc<dyn EventBus>,
) -> (
    SchedulerEngine,
    MissionId,
    SqliteTaskRepository,
    sqlx::SqlitePool,
    tempfile::TempDir,
) {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("scheduler_test.db");
    let pool = initialize_database(&db_path).await.unwrap();

    let mission_repo = SqliteMissionRepository::new(pool.clone());
    let mission_id = MissionId::new();
    let mission = Mission::new(mission_id, "Scheduler Test Mission".to_string());
    mission_repo.insert(&mission).await.unwrap();

    let task_repo = SqliteTaskRepository::new(pool.clone());
    let resource_mgr = Arc::new(ResourceManager::new(Some(pool.clone()), 2));
    let engine = SchedulerEngine::new(pool.clone(), resource_mgr, limits, Some(bus));

    (engine, mission_id, task_repo, pool, dir)
}

#[tokio::test]
async fn test_retryable_failure_emits_attempt_failed_not_terminal_failed() {
    let bus: Arc<dyn EventBus> = Arc::new(BroadcastEventBus::new(64));
    let limits = ConcurrencyLimits {
        max_global_workers: 2,
        ..Default::default()
    };
    let (engine, mission_id, task_repo, _pool, _dir) = setup_test_engine(limits, bus.clone()).await;

    let mut event_rx = bus.subscribe(EventFilter::all()).await;

    let plan = CandidatePlan::new(
        "plan_retry",
        "Retryable test plan",
        vec![build_task("task_a", vec![], AgentRole::implementer())],
    );

    let _graph_id = engine.materialize_plan(mission_id, &plan).await.unwrap();
    let ready = engine.find_ready_work(mission_id).await.unwrap();
    assert_eq!(ready.ready_tasks.len(), 1);
    let task_a_id = ready.ready_tasks[0].task_id;

    // Mark task started
    let agent_id = AgentId::new();
    engine.mark_task_started(task_a_id, agent_id).await.unwrap();

    // Mark task failed with retryable = true
    engine
        .mark_task_failed(task_a_id, "Compiler error".into(), true)
        .await
        .unwrap();

    // Collect emitted events
    let mut attempt_failed = false;
    let mut retry_scheduled = false;
    let mut task_failed = false;

    while let Ok(Some(Ok(envelope))) =
        tokio::time::timeout(std::time::Duration::from_millis(50), event_rx.next()).await
    {
        match envelope.event_type {
            EventType::TaskAttemptFailed {
                task_id, attempt, ..
            } => {
                assert_eq!(task_id, task_a_id);
                assert_eq!(attempt, 1);
                attempt_failed = true;
            }
            EventType::TaskRetryScheduled {
                task_id, attempt, ..
            } => {
                assert_eq!(task_id, task_a_id);
                assert_eq!(attempt, 1);
                retry_scheduled = true;
            }
            EventType::TaskFailed { task_id, .. } if task_id == task_a_id => {
                task_failed = true;
            }
            _ => {}
        }
    }

    assert!(attempt_failed, "TaskAttemptFailed event must be emitted");
    assert!(retry_scheduled, "TaskRetryScheduled event must be emitted");
    assert!(
        !task_failed,
        "TaskFailed must NEVER be emitted for a retryable attempt failure"
    );

    // Task must be Ready in memory and in DB with retry_count = 1
    let task_in_db = task_repo.get(task_a_id).await.unwrap().unwrap();
    assert_eq!(task_in_db.status, TaskState::Ready);
    assert_eq!(task_in_db.retry_count, 1);
}

#[tokio::test]
async fn test_terminal_failure_propagates_transitive_blocked() {
    let bus: Arc<dyn EventBus> = Arc::new(BroadcastEventBus::new(64));
    let limits = ConcurrencyLimits {
        max_global_workers: 2,
        ..Default::default()
    };
    let (engine, mission_id, task_repo, _pool, _dir) = setup_test_engine(limits, bus.clone()).await;

    let mut event_rx = bus.subscribe(EventFilter::all()).await;

    // Linear DAG: task_1 -> task_2 -> task_3
    let plan = CandidatePlan::new(
        "plan_transitive",
        "Transitive failure plan",
        vec![
            build_task("task_1", vec![], AgentRole::implementer()),
            build_task("task_2", vec!["task_1"], AgentRole::implementer()),
            build_task("task_3", vec!["task_2"], AgentRole::implementer()),
        ],
    );

    engine.materialize_plan(mission_id, &plan).await.unwrap();
    let _ = engine.find_ready_work(mission_id).await.unwrap();

    let tasks = task_repo.list_by_mission(mission_id).await.unwrap();
    let t1 = tasks
        .iter()
        .find(|t| t.candidate_key == "task_1")
        .unwrap()
        .id;
    let t2 = tasks
        .iter()
        .find(|t| t.candidate_key == "task_2")
        .unwrap()
        .id;
    let t3 = tasks
        .iter()
        .find(|t| t.candidate_key == "task_3")
        .unwrap()
        .id;

    engine.mark_task_started(t1, AgentId::new()).await.unwrap();

    // Terminal failure on t1
    engine
        .mark_task_failed(t1, "Fatal panic".into(), false)
        .await
        .unwrap();

    // Verify events
    let mut failed_emitted = false;
    let mut blocked_t2 = false;
    let mut blocked_t3 = false;

    while let Ok(Some(Ok(envelope))) =
        tokio::time::timeout(std::time::Duration::from_millis(50), event_rx.next()).await
    {
        match envelope.event_type {
            EventType::TaskFailed { task_id, .. } if task_id == t1 => {
                failed_emitted = true;
            }
            EventType::TaskBlocked { task_id, .. } if task_id == t2 => {
                blocked_t2 = true;
            }
            EventType::TaskBlocked { task_id, .. } if task_id == t3 => {
                blocked_t3 = true;
            }
            _ => {}
        }
    }

    assert!(failed_emitted, "TaskFailed must be emitted for t1");
    assert!(
        blocked_t2,
        "TaskBlocked must be emitted for direct dependent t2"
    );
    assert!(
        blocked_t3,
        "TaskBlocked must be emitted for transitive dependent t3"
    );

    // DB state verification
    assert_eq!(
        task_repo.get(t1).await.unwrap().unwrap().status,
        TaskState::Failed
    );
    assert_eq!(
        task_repo.get(t2).await.unwrap().unwrap().status,
        TaskState::Blocked
    );
    assert_eq!(
        task_repo.get(t3).await.unwrap().unwrap().status,
        TaskState::Blocked
    );
}

#[tokio::test]
async fn test_cancel_task_transitive_cancellation_and_resource_release() {
    let bus: Arc<dyn EventBus> = Arc::new(BroadcastEventBus::new(64));
    let limits = ConcurrencyLimits {
        max_global_workers: 2,
        ..Default::default()
    };
    let (engine, mission_id, task_repo, _pool, _dir) = setup_test_engine(limits, bus.clone()).await;

    let mut event_rx = bus.subscribe(EventFilter::all()).await;

    // Linear DAG: task_1 -> task_2 -> task_3
    let plan = CandidatePlan::new(
        "plan_cancel",
        "Cancellation plan",
        vec![
            build_task("task_1", vec![], AgentRole::implementer()),
            build_task("task_2", vec!["task_1"], AgentRole::implementer()),
            build_task("task_3", vec!["task_2"], AgentRole::implementer()),
        ],
    );

    engine.materialize_plan(mission_id, &plan).await.unwrap();
    let _ = engine.find_ready_work(mission_id).await.unwrap();

    let tasks = task_repo.list_by_mission(mission_id).await.unwrap();
    let t1 = tasks
        .iter()
        .find(|t| t.candidate_key == "task_1")
        .unwrap()
        .id;
    let t2 = tasks
        .iter()
        .find(|t| t.candidate_key == "task_2")
        .unwrap()
        .id;
    let t3 = tasks
        .iter()
        .find(|t| t.candidate_key == "task_3")
        .unwrap()
        .id;

    engine.mark_task_started(t1, AgentId::new()).await.unwrap();

    // Cancel t1
    engine.cancel_task(t1).await.unwrap();

    // Verify events
    let mut cancelled_t1 = false;
    let mut blocked_t2 = false;
    let mut blocked_t3 = false;

    while let Ok(Some(Ok(envelope))) =
        tokio::time::timeout(std::time::Duration::from_millis(50), event_rx.next()).await
    {
        match envelope.event_type {
            EventType::TaskCancelled { task_id, .. } if task_id == t1 => {
                cancelled_t1 = true;
            }
            EventType::TaskBlocked { task_id, .. } if task_id == t2 => {
                blocked_t2 = true;
            }
            EventType::TaskBlocked { task_id, .. } if task_id == t3 => {
                blocked_t3 = true;
            }
            _ => {}
        }
    }

    assert!(cancelled_t1, "TaskCancelled must be emitted for t1");
    assert!(blocked_t2, "TaskBlocked must be emitted for dependent t2");
    assert!(
        blocked_t3,
        "TaskBlocked must be emitted for transitive dependent t3"
    );

    // DB state
    assert_eq!(
        task_repo.get(t1).await.unwrap().unwrap().status,
        TaskState::Cancelled
    );
    assert_eq!(
        task_repo.get(t2).await.unwrap().unwrap().status,
        TaskState::Blocked
    );
    assert_eq!(
        task_repo.get(t3).await.unwrap().unwrap().status,
        TaskState::Blocked
    );
}

#[tokio::test]
async fn test_needs_review_lifecycle_and_approval() {
    let bus: Arc<dyn EventBus> = Arc::new(BroadcastEventBus::new(64));
    let limits = ConcurrencyLimits {
        max_global_workers: 2,
        ..Default::default()
    };
    let (engine, mission_id, task_repo, _pool, _dir) = setup_test_engine(limits, bus.clone()).await;

    let mut event_rx = bus.subscribe(EventFilter::all()).await;

    let plan = CandidatePlan::new(
        "plan_review",
        "Review plan",
        vec![
            build_task("task_a", vec![], AgentRole::implementer()),
            build_task("task_b", vec!["task_a"], AgentRole::verifier()),
        ],
    );

    engine.materialize_plan(mission_id, &plan).await.unwrap();
    let _ = engine.find_ready_work(mission_id).await.unwrap();

    let tasks = task_repo.list_by_mission(mission_id).await.unwrap();
    let ta = tasks
        .iter()
        .find(|t| t.candidate_key == "task_a")
        .unwrap()
        .id;
    let tb = tasks
        .iter()
        .find(|t| t.candidate_key == "task_b")
        .unwrap()
        .id;

    engine.mark_task_started(ta, AgentId::new()).await.unwrap();

    // Mark needs review
    engine.mark_task_needs_review(ta).await.unwrap();

    // Verify state & DB
    assert_eq!(
        task_repo.get(ta).await.unwrap().unwrap().status,
        TaskState::NeedsReview
    );

    // Approve the review
    engine
        .review_task(ta, true, "security_lead".into())
        .await
        .unwrap();

    // Verify events: TaskNeedsReview, TaskCompleted, TaskReviewed
    let mut needs_review_emitted = false;
    let mut reviewed_emitted = false;
    let mut completed_emitted = false;

    while let Ok(Some(Ok(envelope))) =
        tokio::time::timeout(std::time::Duration::from_millis(50), event_rx.next()).await
    {
        match envelope.event_type {
            EventType::TaskNeedsReview { task_id, .. } if task_id == ta => {
                needs_review_emitted = true;
            }
            EventType::TaskReviewed {
                task_id, approved, ..
            } if task_id == ta && approved => {
                reviewed_emitted = true;
            }
            EventType::TaskCompleted { task_id, .. } if task_id == ta => {
                completed_emitted = true;
            }
            _ => {}
        }
    }

    assert!(needs_review_emitted, "TaskNeedsReview must be emitted");
    assert!(reviewed_emitted, "TaskReviewed must be emitted");
    assert!(
        completed_emitted,
        "TaskCompleted must be emitted upon approval"
    );

    // Dependent task_b should now be Ready
    assert_eq!(
        task_repo.get(ta).await.unwrap().unwrap().status,
        TaskState::Succeeded
    );
    assert_eq!(
        task_repo.get(tb).await.unwrap().unwrap().status,
        TaskState::Ready
    );
}

#[tokio::test]
async fn test_needs_review_rejection_is_terminal_failure() {
    let bus: Arc<dyn EventBus> = Arc::new(BroadcastEventBus::new(64));
    let limits = ConcurrencyLimits {
        max_global_workers: 2,
        ..Default::default()
    };
    let (engine, mission_id, task_repo, _pool, _dir) = setup_test_engine(limits, bus.clone()).await;

    let plan = CandidatePlan::new(
        "plan_review_reject",
        "Review reject plan",
        vec![
            build_task("task_a", vec![], AgentRole::implementer()),
            build_task("task_b", vec!["task_a"], AgentRole::verifier()),
        ],
    );

    engine.materialize_plan(mission_id, &plan).await.unwrap();
    let _ = engine.find_ready_work(mission_id).await.unwrap();

    let tasks = task_repo.list_by_mission(mission_id).await.unwrap();
    let ta = tasks
        .iter()
        .find(|t| t.candidate_key == "task_a")
        .unwrap()
        .id;
    let tb = tasks
        .iter()
        .find(|t| t.candidate_key == "task_b")
        .unwrap()
        .id;

    engine.mark_task_started(ta, AgentId::new()).await.unwrap();
    engine.mark_task_needs_review(ta).await.unwrap();

    // Reject the review
    engine
        .review_task(ta, false, "security_lead".into())
        .await
        .unwrap();

    // ta is Failed, tb is Blocked
    assert_eq!(
        task_repo.get(ta).await.unwrap().unwrap().status,
        TaskState::Failed
    );
    assert_eq!(
        task_repo.get(tb).await.unwrap().unwrap().status,
        TaskState::Blocked
    );
}

#[tokio::test]
async fn test_task_skipped_lifecycle() {
    let bus: Arc<dyn EventBus> = Arc::new(BroadcastEventBus::new(64));
    let limits = ConcurrencyLimits {
        max_global_workers: 2,
        ..Default::default()
    };
    let (engine, mission_id, task_repo, _pool, _dir) = setup_test_engine(limits, bus.clone()).await;

    let mut event_rx = bus.subscribe(EventFilter::all()).await;

    let plan = CandidatePlan::new(
        "plan_skip",
        "Skip plan",
        vec![
            build_task("task_a", vec![], AgentRole::implementer()),
            build_task("task_b", vec!["task_a"], AgentRole::verifier()),
        ],
    );

    engine.materialize_plan(mission_id, &plan).await.unwrap();

    let tasks = task_repo.list_by_mission(mission_id).await.unwrap();
    let ta = tasks
        .iter()
        .find(|t| t.candidate_key == "task_a")
        .unwrap()
        .id;
    let tb = tasks
        .iter()
        .find(|t| t.candidate_key == "task_b")
        .unwrap()
        .id;

    // Skip task_a
    engine
        .mark_task_skipped(ta, "Feature not needed".into())
        .await
        .unwrap();

    let mut skipped_emitted = false;
    let mut blocked_emitted = false;

    while let Ok(Some(Ok(envelope))) =
        tokio::time::timeout(std::time::Duration::from_millis(50), event_rx.next()).await
    {
        match envelope.event_type {
            EventType::TaskSkipped { task_id, .. } if task_id == ta => {
                skipped_emitted = true;
            }
            EventType::TaskBlocked { task_id, .. } if task_id == tb => {
                blocked_emitted = true;
            }
            _ => {}
        }
    }

    assert!(skipped_emitted, "TaskSkipped must be emitted");
    assert!(blocked_emitted, "TaskBlocked must be emitted for dependent");
    assert_eq!(
        task_repo.get(ta).await.unwrap().unwrap().status,
        TaskState::Skipped
    );
    assert_eq!(
        task_repo.get(tb).await.unwrap().unwrap().status,
        TaskState::Blocked
    );
}

#[tokio::test]
async fn test_cancel_mission_scoped_to_mission_only() {
    let bus: Arc<dyn EventBus> = Arc::new(BroadcastEventBus::new(64));
    let limits = ConcurrencyLimits {
        max_global_workers: 4,
        ..Default::default()
    };
    let (engine, mission_1, task_repo, pool, _dir) = setup_test_engine(limits, bus.clone()).await;

    // Create a second mission
    let mission_repo = SqliteMissionRepository::new(pool.clone());
    let mission_2 = MissionId::new();
    let m2 = Mission::new(mission_2, "Mission 2".to_string());
    mission_repo.insert(&m2).await.unwrap();

    let plan_1 = CandidatePlan::new(
        "plan_m1",
        "Plan 1",
        vec![build_task("m1_task", vec![], AgentRole::implementer())],
    );
    let plan_2 = CandidatePlan::new(
        "plan_m2",
        "Plan 2",
        vec![build_task("m2_task", vec![], AgentRole::implementer())],
    );

    engine.materialize_plan(mission_1, &plan_1).await.unwrap();
    let tasks_1 = task_repo.list_by_mission(mission_1).await.unwrap();
    let m1_task_id = tasks_1[0].id;

    engine.materialize_plan(mission_2, &plan_2).await.unwrap();
    let _ = engine.find_ready_work(mission_2).await.unwrap();
    let tasks_2 = task_repo.list_by_mission(mission_2).await.unwrap();
    let m2_task_id = tasks_2[0].id;

    // Start m2_task
    engine
        .mark_task_started(m2_task_id, AgentId::new())
        .await
        .unwrap();

    // Cancel Mission 1
    engine.cancel_mission(mission_1).await.unwrap();

    // Mission 1 task is cancelled
    assert_eq!(
        task_repo.get(m1_task_id).await.unwrap().unwrap().status,
        TaskState::Cancelled
    );

    // Mission 2 task remains Running!
    assert_eq!(
        task_repo.get(m2_task_id).await.unwrap().unwrap().status,
        TaskState::Running
    );
}
