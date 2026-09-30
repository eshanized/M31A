//! Phase 05: Versioned Replan Reconciliation Integration Tests
//!
//! Verifies:
//! 1. Reconciling a candidate plan creates immutable revision N+1.
//! 2. Unmodified completed tasks preserve TaskId, outputs, and Succeeded status.
//! 3. Materially modified tasks are superseded and receive fresh TaskIds.
//! 4. Removed tasks are marked superseded.
//! 5. Newly added tasks receive fresh TaskIds.
//! 6. Old graph revision remains recorded in SQLite.

use m31a::dag::materializer::TaskGraphMaterializer;
use m31a::dag::reconciler::TaskGraphReconciler;
use m31a::ids::MissionId;
use m31a::kernel::plan::{
    CandidatePlan, CandidateTask, CandidateTaskKey, CapabilityAccessMode, CapabilityRequirement,
    ResourceEstimate, VerificationStrategy,
};
use m31a::persistence::initialize_database;
use m31a::persistence::sqlite::repositories::{
    SqliteMissionRepository, SqliteTaskGraphRepository, TaskGraphRepository,
};
use m31a::state::mission::Mission;
use m31a::state::task::TaskResult;
use m31a::state_machine::TaskState;
use m31a::state_machine::agent::AgentRole;
use tempfile::tempdir;

fn create_task(id: &str, objective: &str, deps: Vec<&str>) -> CandidateTask {
    let mut task = CandidateTask::new(
        id,
        objective.to_string(),
        AgentRole::implementer(),
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

#[tokio::test]
async fn test_replan_reconciliation_semantic_fingerprints() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("replan_test.db");
    let pool = initialize_database(&db_path).await.unwrap();

    let mission_repo = SqliteMissionRepository::new(pool.clone());
    let graph_repo = SqliteTaskGraphRepository::new(pool.clone());
    let mission_id = MissionId::new();
    let mission = Mission::new(mission_id, "Replan Test Mission".to_string());
    mission_repo.insert(&mission).await.unwrap();

    // 1. Materialize Plan 1 with Tasks A, B, C (A -> B -> C)
    let plan_1 = CandidatePlan::new(
        "plan-v1",
        "Initial Plan",
        vec![
            create_task("task_a", "Objective A", vec![]),
            create_task("task_b", "Objective B", vec!["task_a"]),
            create_task("task_c", "Objective C", vec!["task_b"]),
        ],
    );

    let materializer = TaskGraphMaterializer::new(pool.clone());
    let graph_v1 = materializer.materialize(mission_id, &plan_1).await.unwrap();
    assert_eq!(graph_v1.revision, 1);

    let task_a_id_v1 = *graph_v1
        .candidate_to_task
        .get(&CandidateTaskKey::new("task_a"))
        .unwrap();
    let task_b_id_v1 = *graph_v1
        .candidate_to_task
        .get(&CandidateTaskKey::new("task_b"))
        .unwrap();
    let task_c_id_v1 = *graph_v1
        .candidate_to_task
        .get(&CandidateTaskKey::new("task_c"))
        .unwrap();

    // 2. Complete Task A in SQLite
    graph_repo
        .update_task_status(task_a_id_v1, TaskState::Succeeded)
        .await
        .unwrap();
    let res = TaskResult {
        summary: "Task A succeeded".to_string(),
        output_artifacts: Vec::new(),
        metadata: std::collections::HashMap::new(),
    };
    let res_json = serde_json::to_string(&res).unwrap();
    sqlx::query("UPDATE tasks SET result = ? WHERE id = ?")
        .bind(&res_json)
        .bind(task_a_id_v1.as_bytes().as_slice())
        .execute(&pool)
        .await
        .unwrap();

    // Reload graph_v1 to have updated status
    let updated_graph_v1 = graph_repo
        .get_graph_by_id(graph_v1.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        updated_graph_v1.tasks.get(&task_a_id_v1).unwrap().status,
        TaskState::Succeeded
    );

    // 3. Propose Plan 2:
    // - Task A: identical (same objective) -> reused
    // - Task B: modified objective ("Objective B Modified") -> superseded, new TaskId
    // - Task C: removed -> superseded
    // - Task D: brand new ("Objective D", depends on A) -> new TaskId
    let plan_2 = CandidatePlan::new(
        "plan-v2",
        "Replanned Plan",
        vec![
            create_task("task_a", "Objective A", vec![]),
            create_task("task_b", "Objective B Modified", vec!["task_a"]),
            create_task("task_d", "Objective D", vec!["task_a"]),
        ],
    );

    let reconciler = TaskGraphReconciler::new(pool.clone());
    let (graph_v2, summary) = reconciler
        .reconcile_with_summary(mission_id, &updated_graph_v1, &plan_2)
        .await
        .unwrap();

    // Assert revision increment
    assert_eq!(graph_v2.revision, 2);

    // Assert Task A reused
    let task_a_id_v2 = *graph_v2
        .candidate_to_task
        .get(&CandidateTaskKey::new("task_a"))
        .unwrap();
    assert_eq!(task_a_id_v1, task_a_id_v2);
    assert_eq!(
        graph_v2.tasks.get(&task_a_id_v2).unwrap().status,
        TaskState::Succeeded
    );

    // Assert Task B is new ID and old ID is in superseded
    let task_b_id_v2 = *graph_v2
        .candidate_to_task
        .get(&CandidateTaskKey::new("task_b"))
        .unwrap();
    assert_ne!(task_b_id_v1, task_b_id_v2);
    assert!(summary.superseded_tasks.contains(&task_b_id_v1));

    // Assert Task C was superseded
    assert!(summary.superseded_tasks.contains(&task_c_id_v1));
    assert!(
        !graph_v2
            .candidate_to_task
            .contains_key(&CandidateTaskKey::new("task_c"))
    );

    // Assert Task D created fresh
    let task_d_id_v2 = *graph_v2
        .candidate_to_task
        .get(&CandidateTaskKey::new("task_d"))
        .unwrap();
    assert!(
        summary
            .new_tasks
            .iter()
            .any(|(k, id)| k.as_str() == "task_d" && *id == task_d_id_v2)
    );

    // Assert old graph record revision 1 still exists in SQLite
    use sqlx::Row;
    let old_graph_row = sqlx::query("SELECT revision, status FROM task_graphs WHERE id = ?")
        .bind(graph_v1.id.as_bytes().as_slice())
        .fetch_one(&pool)
        .await
        .unwrap();
    let old_rev: i64 = old_graph_row.get("revision");
    let old_status: String = old_graph_row.get("status");
    assert_eq!(old_rev, 1);
    assert_eq!(old_status, "superseded");

    // Assert new active graph is revision 2
    let active_graph = graph_repo
        .get_active_graph(mission_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(active_graph.revision, 2);
    assert_eq!(active_graph.id, graph_v2.id);
}
