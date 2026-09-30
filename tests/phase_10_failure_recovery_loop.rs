//! Phase 10 Integration Tests: Failure Classification, Recovery Budgets, Loop Detection & Differential Replanning (FLC-01–FLC-05).

use std::time::Duration;
use tempfile::tempdir;

use m31a::ids::{AgentId, MissionId, TaskId};
use m31a::kernel::seams::recovery::FailureClassification;
use m31a::persistence::sqlite::schema::initialize_database;
use m31a::recovery::budget::{BudgetEvaluation, RecoveryAttemptRecord, RecoveryBudgetTracker};
use m31a::recovery::classifier::FailureClassifier;

async fn seed_test_mission_and_task(pool: &sqlx::SqlitePool) -> (MissionId, TaskId) {
    let mission_id = MissionId::new();
    let task_id = TaskId::new();
    let agent_id = AgentId::new();
    let now = chrono::Utc::now().to_rfc3339();

    sqlx::query(
        "INSERT INTO missions (id, objective, status, created_at, updated_at) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(mission_id.as_bytes().as_slice())
    .bind("Failure recovery test mission")
    .bind("in_progress")
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO tasks (id, mission_id, title, status, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(task_id.as_bytes().as_slice())
    .bind(mission_id.as_bytes().as_slice())
    .bind("Task for failure recovery test")
    .bind("in_progress")
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO agents (id, mission_id, role, status, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(agent_id.as_bytes().as_slice())
    .bind(mission_id.as_bytes().as_slice())
    .bind("worker")
    .bind("active")
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await
    .unwrap();

    (mission_id, task_id)
}

#[tokio::test]
async fn test_failure_classification_and_budgets() {
    // 1. Verify deterministic classification for all 15 specification classes
    let test_cases = [
        (
            "HTTP 503 Service Unavailable: upstream service unreachable",
            FailureClassification::Transient,
        ),
        (
            "Execution aborted: task wall-clock timeout exceeded",
            FailureClassification::Timeout,
        ),
        (
            "POSIX error: EACCES permission denied to read /etc/shadow",
            FailureClassification::Permission,
        ),
        (
            "PolicyGate outcome DENY: unauthorized tool invocation forbidden by policy",
            FailureClassification::Policy,
        ),
        (
            "Environment error: missing compiler binary: cargo command not found",
            FailureClassification::Environment,
        ),
        (
            "Dependency error: crate resolution failure: failed to select a version for crate 'serde'",
            FailureClassification::Dependency,
        ),
        (
            "error[E0308]: mismatched types: expected `String`, found `&str`",
            FailureClassification::Compilation,
        ),
        (
            "assertion failed: `left == right` (left: 4, right: 5)",
            FailureClassification::Test,
        ),
        (
            "ToolContract validation failed: invalid JSON arguments: missing required field 'path'",
            FailureClassification::ToolContract,
        ),
        (
            "Model error: provider 500: stream disruption during generation",
            FailureClassification::Model,
        ),
        (
            "Context error: token window overflow: maximum context length exceeded",
            FailureClassification::Context,
        ),
        (
            "Kernel panic: out of memory (oom-killer invoked)",
            FailureClassification::ResourceLimit,
        ),
        (
            "Repository state error: unexpected drift: dirty working tree and merge conflict",
            FailureClassification::RepositoryState,
        ),
        (
            "Architecture error: cycle detected in task graph dependencies",
            FailureClassification::Architecture,
        ),
        (
            "Unexpected kernel trap code 0xdeadbeef",
            FailureClassification::Unknown,
        ),
    ];

    for (error_str, expected_class) in test_cases {
        let classified = FailureClassifier::classify_deterministic(None, error_str);
        assert_eq!(
            classified, expected_class,
            "Failed deterministic classification for '{}'",
            error_str
        );
    }

    // 2. Verify that Permission and Policy failures strictly evaluate to 0 retry budget (D-06)
    let budget_tracker = RecoveryBudgetTracker::new(3, 10);

    let perm_eval = budget_tracker.evaluate(FailureClassification::Permission, 0, 0, 0, None);
    assert!(
        matches!(perm_eval, BudgetEvaluation::NonRetryable { .. }),
        "Permission failures must be strictly non-retryable with 0 budget"
    );

    let policy_eval = budget_tracker.evaluate(FailureClassification::Policy, 0, 0, 0, None);
    assert!(
        matches!(policy_eval, BudgetEvaluation::NonRetryable { .. }),
        "Policy failures must be strictly non-retryable with 0 budget"
    );

    assert_eq!(FailureClassification::Permission.default_retry_limit(), 0);
    assert_eq!(FailureClassification::Policy.default_retry_limit(), 0);
    assert!(!FailureClassification::Permission.is_retryable());
    assert!(!FailureClassification::Policy.is_retryable());

    // 3. Verify exponential backoff with jitter calculation for Transient failures
    let b0 = budget_tracker.compute_backoff(0, None);
    let b1 = budget_tracker.compute_backoff(1, None);
    let b2 = budget_tracker.compute_backoff(2, None);

    assert!(
        b0 >= Duration::from_millis(400) && b0 <= Duration::from_millis(600),
        "Attempt 0 backoff should be around 500ms, got {:?}",
        b0
    );
    assert!(
        b1 >= Duration::from_millis(800) && b1 <= Duration::from_millis(1200),
        "Attempt 1 backoff should be around 1000ms, got {:?}",
        b1
    );
    assert!(
        b2 >= Duration::from_millis(1600) && b2 <= Duration::from_millis(2400),
        "Attempt 2 backoff should be around 2000ms, got {:?}",
        b2
    );

    // Honoring Retry-After header
    let b_retry_after = budget_tracker.compute_backoff(0, Some(8000));
    assert_eq!(b_retry_after, Duration::from_millis(8000));

    // 4. Verify that recovery attempts are durably logged to SQLite recovery_attempts (FLC-05)
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test_recovery.db");
    let pool = initialize_database(&db_path).await.unwrap();

    let (mission_id, task_id) = seed_test_mission_and_task(&pool).await;

    let record = RecoveryAttemptRecord {
        mission_id,
        task_id,
        failure_class: FailureClassification::Transient,
        strategy: "exponential_backoff".into(),
        attempt_number: 1,
        budget_consumed: 1,
        remaining_class_budget: 2,
        remaining_overall_budget: 2,
        backoff_delay_ms: 500,
        action_taken: "retry_with_delay".into(),
        result: "in_flight".into(),
        mutation_fingerprint: None,
        semantic_signature: None,
    };

    let attempt_id = budget_tracker
        .record_attempt(&pool, record)
        .await
        .expect("Failed to persist recovery attempt to SQLite");

    assert!(!attempt_id.is_nil());

    // Verify record exists in SQLite
    let count = RecoveryBudgetTracker::count_task_attempts(&pool, task_id)
        .await
        .unwrap();
    assert_eq!(count, 1, "Expected 1 recorded recovery attempt");

    let class_count = RecoveryBudgetTracker::count_class_attempts(
        &pool,
        task_id,
        FailureClassification::Transient,
    )
    .await
    .unwrap();
    assert_eq!(
        class_count, 1,
        "Expected 1 recorded Transient recovery attempt"
    );
}

#[tokio::test]
async fn test_differential_replan_preservation() {
    use m31a::dag::materializer::TaskGraphMaterializer;
    use m31a::kernel::plan::{
        CandidatePlan, CandidateTask, CandidateTaskKey, CapabilityAccessMode,
        CapabilityRequirement, ResourceEstimate, VerificationStrategy,
    };
    use m31a::persistence::sqlite::repositories::{SqliteTaskGraphRepository, TaskGraphRepository};
    use m31a::recovery::replan::{DifferentialReplanEngine, DifferentialReplanRequest};
    use m31a::state_machine::TaskState;
    use m31a::state_machine::agent::AgentRole;

    fn make_task(id: &str, objective: &str, deps: Vec<&str>) -> CandidateTask {
        let mut t = CandidateTask::new(
            id,
            objective.to_string(),
            AgentRole::implementer(),
            VerificationStrategy::Compilation,
            ResourceEstimate::default(),
        );
        for d in deps {
            t.depends_on.push(CandidateTaskKey::new(d));
        }
        t.capabilities.push(CapabilityRequirement::new(
            "fs.local",
            CapabilityAccessMode::ReadWrite,
        ));
        t
    }

    let dir = tempdir().unwrap();
    let db_path = dir.path().join("replan_preservation.db");
    let pool = initialize_database(&db_path).await.unwrap();

    let (mission_id, _) = seed_test_mission_and_task(&pool).await;
    let graph_repo = SqliteTaskGraphRepository::new(pool.clone());

    // 1. Initial 3-task DAG: Task 1 -> Task 2 -> Task 3
    let plan_1 = CandidatePlan::new(
        "plan-v1",
        "Initial Plan",
        vec![
            make_task("task_1", "Objective 1", vec![]),
            make_task("task_2", "Objective 2", vec!["task_1"]),
            make_task("task_3", "Objective 3", vec!["task_2"]),
        ],
    );

    let materializer = TaskGraphMaterializer::new(pool.clone());
    let graph_v1 = materializer.materialize(mission_id, &plan_1).await.unwrap();
    assert_eq!(graph_v1.revision, 1);

    let task_1_id_v1 = *graph_v1
        .candidate_to_task
        .get(&CandidateTaskKey::new("task_1"))
        .unwrap();
    let task_2_id_v1 = *graph_v1
        .candidate_to_task
        .get(&CandidateTaskKey::new("task_2"))
        .unwrap();

    // Mark Task 1 as Succeeded and Task 2 as Failed in SQLite
    graph_repo
        .update_task_status(task_1_id_v1, TaskState::Succeeded)
        .await
        .unwrap();
    graph_repo
        .update_task_status(task_2_id_v1, TaskState::Failed)
        .await
        .unwrap();

    // Reload graph_v1 to reflect updated statuses
    let updated_graph_v1 = graph_repo
        .get_graph_by_id(graph_v1.id)
        .await
        .unwrap()
        .unwrap();

    assert_eq!(
        updated_graph_v1.tasks.get(&task_1_id_v1).unwrap().status,
        TaskState::Succeeded
    );

    // 2. Differential replan proposing replacement Task 2b and updated Task 3 depending on Task 2b
    let plan_2 = CandidatePlan::new(
        "plan-v2",
        "Differential Recovery Plan",
        vec![
            make_task("task_1", "Objective 1", vec![]),
            make_task("task_2b", "Objective 2 Replacement", vec!["task_1"]),
            make_task("task_3", "Objective 3", vec!["task_2b"]),
        ],
    );

    let replan_engine = DifferentialReplanEngine::new(pool.clone());
    let req = DifferentialReplanRequest {
        mission_id,
        failed_task_id: Some(task_2_id_v1),
        failure_class: FailureClassification::Compilation,
        diagnosis_or_reason: "Task 2 failed compilation; superseded by Task 2b".into(),
        candidate_plan: plan_2,
        trigger: None,
    };

    let outcome = replan_engine
        .execute_replan(&updated_graph_v1, req)
        .await
        .unwrap();

    // 3. Assert completion preservation and DAG update
    assert_eq!(outcome.revision, 2);
    assert!(
        outcome.preserved_tasks.contains(&task_1_id_v1),
        "Task 1 must be preserved"
    );
    assert!(
        outcome.superseded_tasks.contains(&task_2_id_v1),
        "Task 2 must be superseded"
    );

    // Verify in updated graph from repository
    let graph_v2 = graph_repo
        .get_graph_by_id(outcome.new_graph_id)
        .await
        .unwrap()
        .unwrap();

    let task_1_id_v2 = *graph_v2
        .candidate_to_task
        .get(&CandidateTaskKey::new("task_1"))
        .unwrap();
    assert_eq!(
        task_1_id_v1, task_1_id_v2,
        "Task 1 ID must remain identical"
    );
    assert_eq!(
        graph_v2.tasks.get(&task_1_id_v2).unwrap().status,
        TaskState::Succeeded,
        "Task 1 must preserve Succeeded state without re-execution"
    );

    let task_2b_id = *graph_v2
        .candidate_to_task
        .get(&CandidateTaskKey::new("task_2b"))
        .unwrap();
    assert_ne!(task_2_id_v1, task_2b_id, "Task 2b must have a fresh TaskId");

    // 4. Verify durable audit trail in SQLite recovery_attempts
    let count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM recovery_attempts WHERE mission_id = ? AND strategy = 'differential_replan'",
    )
    .bind(mission_id.as_bytes().as_slice())
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(
        count.0, 1,
        "Expected 1 recorded differential replan in recovery_attempts"
    );
}
