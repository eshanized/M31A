//! Phase 05: Task DAG & Scheduler Persistence Integration Tests
//!
//! Verifies atomic transactional materialization of CandidatePlan into SQLite,
//! authoritative graph reconstruction with in-memory derived index caches,
//! idempotency, and transaction rollback on corrupted inputs.

use m31a::dag::materializer::TaskGraphMaterializer;
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
use m31a::state_machine::TaskState;
use m31a::state_machine::agent::AgentRole;
use tempfile::tempdir;

fn sample_candidate(id: &str, depends_on: Vec<&str>, role: AgentRole) -> CandidateTask {
    let mut task = CandidateTask::new(
        id,
        format!("Execute {}", id),
        role,
        VerificationStrategy::Compilation,
        ResourceEstimate::default(),
    );
    task.depends_on = depends_on.into_iter().map(CandidateTaskKey::new).collect();
    task.capabilities = vec![CapabilityRequirement::new(
        "workspace",
        CapabilityAccessMode::Write,
    )];
    task
}

#[tokio::test]
async fn test_materialize_candidate_plan_happy_path() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("phase_05_test.db");
    let pool = initialize_database(&db_path).await.unwrap();

    // 1. Create mission
    let mission_repo = SqliteMissionRepository::new(pool.clone());
    let mission_id = MissionId::new();
    let mission = Mission::new(mission_id, "Test Phase 5 mission".to_string());
    mission_repo.insert(&mission).await.unwrap();

    // 2. Build 4-task diamond candidate plan
    // T1 -> T2, T1 -> T3, T2 -> T4, T3 -> T4
    let plan = CandidatePlan::new(
        "plan-diamond-01",
        "Diamond workflow plan",
        vec![
            sample_candidate("step_a", vec![], AgentRole::architect()),
            sample_candidate("step_b", vec!["step_a"], AgentRole::implementer()),
            sample_candidate("step_c", vec!["step_a"], AgentRole::implementer()),
            sample_candidate("step_d", vec!["step_b", "step_c"], AgentRole::verifier()),
        ],
    );

    // 3. Materialize plan
    let materializer = TaskGraphMaterializer::new(pool.clone());
    let graph = materializer.materialize(mission_id, &plan).await.unwrap();

    // 4. Assert aggregate properties
    assert_eq!(graph.mission_id, mission_id);
    assert_eq!(graph.revision, 1);
    assert_eq!(graph.plan_id, "plan-diamond-01");
    assert_eq!(graph.status, "active");
    assert_eq!(graph.task_count(), 4);
    assert_eq!(graph.edge_count(), 4);

    // 5. Assert wave tiers derived index
    assert_eq!(graph.wave_tiers.len(), 3);
    assert_eq!(graph.wave_tiers.get(&0).unwrap().len(), 1); // step_a
    assert_eq!(graph.wave_tiers.get(&1).unwrap().len(), 2); // step_b, step_c
    assert_eq!(graph.wave_tiers.get(&2).unwrap().len(), 1); // step_d

    // 6. Assert database rows directly
    let graph_count: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM task_graphs WHERE mission_id = ?")
            .bind(mission_id.as_bytes().as_slice())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(graph_count.0, 1);

    let task_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM tasks WHERE task_graph_id = ?")
        .bind(graph.id.as_bytes().as_slice())
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(task_count.0, 4);

    let dep_count: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM task_dependencies WHERE task_graph_id = ?")
            .bind(graph.id.as_bytes().as_slice())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(dep_count.0, 4);

    // 7. Verify reload parity via repository
    let graph_repo = SqliteTaskGraphRepository::new(pool.clone());
    let reloaded = graph_repo
        .get_active_graph(mission_id)
        .await
        .unwrap()
        .expect("Active graph exists");

    assert_eq!(reloaded.id, graph.id);
    assert_eq!(reloaded.revision, graph.revision);
    assert_eq!(reloaded.task_count(), graph.task_count());
    assert_eq!(reloaded.edge_count(), graph.edge_count());
    assert_eq!(reloaded.wave_tiers, graph.wave_tiers);
    assert_eq!(reloaded.in_degrees, graph.in_degrees);

    // 8. Verify reload by ID
    let by_id = graph_repo
        .get_graph_by_id(graph.id)
        .await
        .unwrap()
        .expect("Graph by id exists");
    assert_eq!(by_id.id, graph.id);
}

#[tokio::test]
async fn test_materialize_idempotency() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("idempotency_test.db");
    let pool = initialize_database(&db_path).await.unwrap();

    let mission_repo = SqliteMissionRepository::new(pool.clone());
    let mission_id = MissionId::new();
    let mission = Mission::new(mission_id, "Idempotency test".to_string());
    mission_repo.insert(&mission).await.unwrap();

    let plan = CandidatePlan::new(
        "plan-idemp-01",
        "Idempotent test plan",
        vec![sample_candidate("step_1", vec![], AgentRole::implementer())],
    );

    let materializer = TaskGraphMaterializer::new(pool.clone());

    let graph1 = materializer.materialize(mission_id, &plan).await.unwrap();
    let graph2 = materializer.materialize(mission_id, &plan).await.unwrap();

    assert_eq!(graph1.id, graph2.id);
    assert_eq!(graph1.revision, graph2.revision);

    let total_graphs: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM task_graphs WHERE mission_id = ?")
            .bind(mission_id.as_bytes().as_slice())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(total_graphs.0, 1);
}

#[tokio::test]
async fn test_materialize_rollback_on_cycle() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("rollback_test.db");
    let pool = initialize_database(&db_path).await.unwrap();

    let mission_repo = SqliteMissionRepository::new(pool.clone());
    let mission_id = MissionId::new();
    let mission = Mission::new(mission_id, "Rollback test".to_string());
    mission_repo.insert(&mission).await.unwrap();

    // Cyclic plan: A -> B -> A
    let plan = CandidatePlan::new(
        "plan-cyclic-01",
        "Cyclic plan",
        vec![
            sample_candidate("step_a", vec!["step_b"], AgentRole::implementer()),
            sample_candidate("step_b", vec!["step_a"], AgentRole::implementer()),
        ],
    );

    let materializer = TaskGraphMaterializer::new(pool.clone());
    let result = materializer.materialize(mission_id, &plan).await;

    assert!(result.is_err());

    let total_graphs: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM task_graphs WHERE mission_id = ?")
            .bind(mission_id.as_bytes().as_slice())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(total_graphs.0, 0);
}

#[tokio::test]
async fn test_task_status_persistence_and_update() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("task_status_test.db");
    let pool = initialize_database(&db_path).await.unwrap();

    let mission_repo = SqliteMissionRepository::new(pool.clone());
    let mission_id = MissionId::new();
    let mission = Mission::new(mission_id, "Status test".to_string());
    mission_repo.insert(&mission).await.unwrap();

    let plan = CandidatePlan::new(
        "plan-status-01",
        "Status plan",
        vec![sample_candidate("step_1", vec![], AgentRole::implementer())],
    );

    let materializer = TaskGraphMaterializer::new(pool.clone());
    let graph = materializer.materialize(mission_id, &plan).await.unwrap();

    let task_id = *graph.tasks.keys().next().unwrap();

    let graph_repo = SqliteTaskGraphRepository::new(pool.clone());

    // Update status to Running
    graph_repo
        .update_task_status(task_id, TaskState::Running)
        .await
        .unwrap();
    let reloaded = graph_repo
        .get_active_graph(mission_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        reloaded.get_task(task_id).unwrap().status,
        TaskState::Running
    );

    // Update status to Succeeded
    graph_repo
        .update_task_status(task_id, TaskState::Succeeded)
        .await
        .unwrap();
    let reloaded = graph_repo
        .get_active_graph(mission_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        reloaded.get_task(task_id).unwrap().status,
        TaskState::Succeeded
    );
}
