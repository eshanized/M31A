//! Comprehensive regression tests for Remediation Findings F-01, F-02, F-03, F-09.

use std::sync::Arc;
use tempfile::tempdir;
use tokio_util::sync::CancellationToken;

use m31a::agent::CapabilityEnvelope;
use m31a::agent::action::{AgentAction, AgentObservation};
use m31a::agent::handoff::{
    AgentHandoffArbiter, HandoffError, HandoffProposal, MAX_DELEGATION_DEPTH,
};
use m31a::agent::runner::{ActionDispatcher, ActionRequest};
use m31a::capability::family::CapabilityFamily;
use m31a::capability::permissions::CapabilityPermissions;
use m31a::capability::registry::CapabilityRegistry;
use m31a::ids::{AgentId, MissionId, TaskGraphId, TaskId};
use m31a::kernel::seams::policy::{
    PolicyDecision, PolicyError, PolicyEvaluationRequest, PolicyGate,
};
use m31a::persistence::sqlite::repositories::task_graph::SqliteTaskGraphRepository;
use m31a::persistence::sqlite::repositories::{SqliteTaskRepository, TaskGraphRepository};
use m31a::persistence::sqlite::schema::initialize_database;
use m31a::pipeline::dispatcher::ProductionActionDispatcher;
use m31a::pipeline::runner::ToolPipelineRunner;
use m31a::state::intake::AutonomyMode;
use m31a::state_machine::TaskState;
use m31a::state_machine::agent::AgentRole;
use m31a::tools::agent::{DelegateTaskInput, DelegateTaskTool};
use m31a::tools::definition::{ToolExecutionContext, TypedTool};
use m31a::tools::registry::ToolRegistry;
use m31a::tools::risk::RiskClass;

struct AllowAllTestPolicy;

#[async_trait::async_trait]
impl PolicyGate for AllowAllTestPolicy {
    async fn evaluate(&self, _req: PolicyEvaluationRequest) -> Result<PolicyDecision, PolicyError> {
        Ok(PolicyDecision::Allow)
    }
}

async fn seed_test_mission(pool: &sqlx::SqlitePool, mission_id: MissionId) {
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO missions (id, objective, status, created_at, updated_at) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(mission_id.as_bytes().as_slice())
    .bind("Remediation Test Mission")
    .bind("planning")
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO execution_runs (id, objective, status, created_at, updated_at) VALUES (?, ?, 'running', ?, ?)",
    )
    .bind(mission_id.as_bytes().as_slice())
    .bind("Remediation Test Mission")
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await
    .unwrap();
}

async fn seed_active_task_graph(pool: &sqlx::SqlitePool, mission_id: MissionId) -> TaskGraphId {
    let graph_id = TaskGraphId::new();
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        r#"
        INSERT INTO task_graphs (id, mission_id, run_id, revision, plan_id, status, created_at, updated_at)
        VALUES (?, ?, ?, 1, 'test-plan', 'active', ?, ?)
        "#,
    )
    .bind(graph_id.as_bytes().as_slice())
    .bind(mission_id.as_bytes().as_slice())
    .bind(mission_id.as_bytes().as_slice())
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await
    .unwrap();

    graph_id
}

// ============================================================================
// F-02: Task-Repository Wiring and Graph Persistence
// ============================================================================

#[tokio::test]
async fn test_f02_create_task_and_graph_hydration() {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("task_graph_f02.db");
    let pool = initialize_database(&db_path).await.unwrap();

    let mission_id = MissionId::new();
    seed_test_mission(&pool, mission_id).await;
    let graph_id = seed_active_task_graph(&pool, mission_id).await;

    let tool_reg = Arc::new(ToolRegistry::new_default(Arc::new(
        CapabilityRegistry::new(),
    )));
    let runner = Arc::new(ToolPipelineRunner::new(tool_reg).with_db_pool(pool.clone()));

    let context = ToolExecutionContext::new(
        Arc::new(CapabilityRegistry::new()),
        temp.path().to_path_buf(),
        CancellationToken::new(),
    )
    .with_mission_id(mission_id)
    .with_task_id(TaskId::new())
    .with_task_repo(SqliteTaskRepository::new(pool.clone()));

    let dispatcher = ProductionActionDispatcher::new(
        runner,
        context,
        Arc::new(AllowAllTestPolicy),
        AutonomyMode::Safe,
    )
    .with_db_pool(pool.clone());

    // 1. Create Task A (Prerequisite)
    let action_a = AgentAction::CreateTask {
        title: "Task A: Setup Infrastructure".to_string(),
        description: "Initial database and server setup".to_string(),
        role: Some(AgentRole::implementer()),
        dependencies: vec![],
    };
    let obs_a = dispatcher.dispatch_action(&action_a).await.unwrap();
    let task_a_id = match obs_a {
        AgentObservation::TaskCreated { task_id } => task_id,
        other => panic!("Expected TaskCreated, got {:?}", other),
    };

    // 2. Create Task B (Dependent on Task A)
    let action_b = AgentAction::CreateTask {
        title: "Task B: Run Migrations".to_string(),
        description: "Apply pending schema migrations".to_string(),
        role: Some(AgentRole::implementer()),
        dependencies: vec![task_a_id],
    };
    let obs_b = dispatcher.dispatch_action(&action_b).await.unwrap();
    let task_b_id = match obs_b {
        AgentObservation::TaskCreated { task_id } => task_id,
        other => panic!("Expected TaskCreated, got {:?}", other),
    };

    // 3. Hydrate graph via TaskGraphRepository
    let graph_repo = SqliteTaskGraphRepository::new(pool.clone());
    let hydrated_graph = graph_repo
        .get_active_graph(mission_id)
        .await
        .unwrap()
        .expect("Active graph must exist");

    assert_eq!(hydrated_graph.id, graph_id);
    assert_eq!(hydrated_graph.tasks.len(), 2);
    assert!(hydrated_graph.tasks.contains_key(&task_a_id));
    assert!(hydrated_graph.tasks.contains_key(&task_b_id));

    // Verify task attributes
    let loaded_a = hydrated_graph.tasks.get(&task_a_id).unwrap();
    assert_eq!(loaded_a.title, "Task A: Setup Infrastructure");
    assert_eq!(loaded_a.task_graph_id, Some(graph_id));

    let loaded_b = hydrated_graph.tasks.get(&task_b_id).unwrap();
    assert_eq!(loaded_b.title, "Task B: Run Migrations");
    assert_eq!(loaded_b.task_graph_id, Some(graph_id));

    // Verify dependency edge: A -> B
    assert_eq!(hydrated_graph.edges.len(), 1);
    let edge = hydrated_graph.edges.iter().next().unwrap();
    assert_eq!(edge.prerequisite_id, task_a_id);
    assert_eq!(edge.dependent_id, task_b_id);

    // Verify derived graph indexes
    assert_eq!(
        hydrated_graph.prerequisites_of.get(&task_b_id).unwrap(),
        &std::collections::BTreeSet::from([task_a_id])
    );
    assert_eq!(
        hydrated_graph.dependents_of.get(&task_a_id).unwrap(),
        &std::collections::BTreeSet::from([task_b_id])
    );
}

#[tokio::test]
async fn test_f02_create_task_validation_failures() {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("task_validation_f02.db");
    let pool = initialize_database(&db_path).await.unwrap();

    let mission_id = MissionId::new();
    seed_test_mission(&pool, mission_id).await;
    seed_active_task_graph(&pool, mission_id).await;

    let graph_repo = SqliteTaskGraphRepository::new(pool.clone());

    // 1. Empty title fails
    let err_empty = graph_repo
        .create_task_in_graph(mission_id, "   ", None, None, &[])
        .await
        .unwrap_err();
    assert!(err_empty.to_string().contains("title cannot be empty"));

    // 2. Non-existent dependency fails
    let fake_dep = TaskId::new();
    let err_missing_dep = graph_repo
        .create_task_in_graph(mission_id, "Valid Title", None, None, &[fake_dep])
        .await
        .unwrap_err();
    assert!(
        err_missing_dep
            .to_string()
            .contains("does not exist in active graph")
    );

    // 3. Cross-mission dependency fails
    let other_mission = MissionId::new();
    seed_test_mission(&pool, other_mission).await;
    seed_active_task_graph(&pool, other_mission).await;

    let other_task = graph_repo
        .create_task_in_graph(other_mission, "Other Mission Task", None, None, &[])
        .await
        .unwrap();

    let err_cross_mission = graph_repo
        .create_task_in_graph(mission_id, "My Task", None, None, &[other_task.id])
        .await
        .unwrap_err();
    assert!(
        err_cross_mission
            .to_string()
            .contains("does not exist in active graph")
    );
}

#[tokio::test]
async fn test_f02_supersede_task_lifecycle() {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("task_supersede_f02.db");
    let pool = initialize_database(&db_path).await.unwrap();

    let mission_id = MissionId::new();
    seed_test_mission(&pool, mission_id).await;
    let _ = seed_active_task_graph(&pool, mission_id).await;

    let graph_repo = SqliteTaskGraphRepository::new(pool.clone());

    // Create a task
    let task = graph_repo
        .create_task_in_graph(mission_id, "Task to be superseded", None, None, &[])
        .await
        .unwrap();

    // 1. Supersede valid task
    let superseded = graph_repo
        .supersede_task_in_graph(mission_id, task.id, "Specification changed upstream")
        .await
        .unwrap();
    assert_eq!(superseded.status, TaskState::Cancelled);
    assert!(superseded.blocking_reason.is_some());

    // 2. Cannot supersede already cancelled/superseded task
    let err_already = graph_repo
        .supersede_task_in_graph(mission_id, task.id, "Try again")
        .await
        .unwrap_err();
    assert!(
        err_already
            .to_string()
            .contains("already cancelled/superseded")
    );

    // 3. Cannot supersede completed/succeeded task
    let succeeded_task = graph_repo
        .create_task_in_graph(mission_id, "Completed Task", None, None, &[])
        .await
        .unwrap();
    // Mark it succeeded directly
    graph_repo
        .update_task_status(succeeded_task.id, TaskState::Succeeded)
        .await
        .unwrap();

    let err_succeeded = graph_repo
        .supersede_task_in_graph(mission_id, succeeded_task.id, "Cannot supersede completed")
        .await
        .unwrap_err();
    assert!(err_succeeded.to_string().contains("has already succeeded"));

    // 4. Empty reason fails
    let err_empty = graph_repo
        .supersede_task_in_graph(mission_id, TaskId::new(), "   ")
        .await
        .unwrap_err();
    assert!(err_empty.to_string().contains("reason cannot be empty"));
}

// ============================================================================
// F-03 & F-09: Multi-Agent Delegation, Risk Class & Arbiter Bounds
// ============================================================================

#[tokio::test]
async fn test_f03_delegate_task_tool_risk_and_fail_closed_persistence() {
    let tool = DelegateTaskTool;
    // F-03: Must be mutating risk class
    assert_eq!(tool.base_risk(), RiskClass::LowRiskMutation);

    let temp = tempdir().unwrap();
    let db_path = temp.path().join("delegate_f03.db");
    let pool = initialize_database(&db_path).await.unwrap();

    let mission_id = MissionId::new();
    seed_test_mission(&pool, mission_id).await;

    // 1. Fails closed when task_repo is None
    let ctx_no_repo = ToolExecutionContext::new(
        Arc::new(CapabilityRegistry::new()),
        temp.path().to_path_buf(),
        CancellationToken::new(),
    )
    .with_mission_id(mission_id);

    let input = DelegateTaskInput {
        target_role: "researcher".to_string(),
        subtask: "Investigate database schema".to_string(),
        input_spec: None,
        max_depth: None,
    };

    let err_no_repo = tool.execute(&ctx_no_repo, input.clone()).await.unwrap_err();
    assert!(
        err_no_repo
            .to_string()
            .contains("DELEGATION_TASK_REPO_MISSING")
    );

    // 2. Fails closed when mission_id is None
    let ctx_no_mission = ToolExecutionContext::new(
        Arc::new(CapabilityRegistry::new()),
        temp.path().to_path_buf(),
        CancellationToken::new(),
    )
    .with_task_repo(SqliteTaskRepository::new(pool.clone()));

    let err_no_mission = tool
        .execute(&ctx_no_mission, input.clone())
        .await
        .unwrap_err();
    assert!(err_no_mission.to_string().contains("Missing mission_id"));

    // 3. Succeeds when properly configured with valid repository and role
    let ctx_valid = ToolExecutionContext::new(
        Arc::new(CapabilityRegistry::new()),
        temp.path().to_path_buf(),
        CancellationToken::new(),
    )
    .with_mission_id(mission_id)
    .with_task_repo(SqliteTaskRepository::new(pool.clone()))
    .with_agent_role(AgentRole::implementer());

    let output = tool.execute(&ctx_valid, input).await.unwrap();
    assert!(output.delegated);
    assert_eq!(output.target_role, "researcher");
    assert!(output.task_id.is_some());

    // Verify persisted task in SQLite
    let child_task_id = output.task_id.unwrap();
    let repo = SqliteTaskRepository::new(pool);
    let tasks = repo.list_by_mission(mission_id).await.unwrap();
    let found = tasks.iter().find(|t| t.id.to_string() == child_task_id);
    assert!(found.is_some());
    let child_task = found.unwrap();
    assert_eq!(child_task.role, AgentRole::researcher());
    assert!(child_task.title.contains("Investigate database schema"));
}

#[tokio::test]
async fn test_f09_arbiter_enforces_bounds_and_role_boundaries() {
    let bus = Arc::new(m31a::events::bus::BroadcastEventBus::new(10));
    let arbiter = AgentHandoffArbiter::new(bus);
    assert_eq!(arbiter.max_delegation_depth, MAX_DELEGATION_DEPTH);
    assert_eq!(MAX_DELEGATION_DEPTH, 2);

    // 1. Self-delegation rejected
    let self_proposal = HandoffProposal {
        source_task_id: TaskId::new(),
        source_agent_id: AgentId::new(),
        source_role: AgentRole::implementer(),
        target_role: AgentRole::implementer(),
        reason: "Self delegation".to_string(),
        required_inputs: vec![],
        artifacts: vec![],
        unresolved_questions: vec![],
        current_depth: 0,
    };
    let err_self = arbiter.validate_proposal(&self_proposal).unwrap_err();
    assert_eq!(
        err_self,
        HandoffError::SelfDelegationDisallowed("implementer".to_string())
    );

    // 2. Unregistered role rejected
    let bad_role_proposal = HandoffProposal {
        source_task_id: TaskId::new(),
        source_agent_id: AgentId::new(),
        source_role: AgentRole::implementer(),
        target_role: AgentRole::new("super_admin_unregistered"),
        reason: "Privilege escalation attempt".to_string(),
        required_inputs: vec![],
        artifacts: vec![],
        unresolved_questions: vec![],
        current_depth: 0,
    };
    let err_bad_role = arbiter.validate_proposal(&bad_role_proposal).unwrap_err();
    assert_eq!(
        err_bad_role,
        HandoffError::InvalidTargetRole("super_admin_unregistered".to_string())
    );

    // 3. Max delegation depth exceeded rejected
    let deep_proposal = HandoffProposal {
        source_task_id: TaskId::new(),
        source_agent_id: AgentId::new(),
        source_role: AgentRole::implementer(),
        target_role: AgentRole::researcher(),
        reason: "Deep handoff".to_string(),
        required_inputs: vec![],
        artifacts: vec![],
        unresolved_questions: vec![],
        current_depth: 2, // At maximum allowed depth (2)
    };
    let err_deep = arbiter.validate_proposal(&deep_proposal).unwrap_err();
    assert_eq!(err_deep, HandoffError::MaxDelegationDepthExceeded(2));
}

// ============================================================================
// F-01: Child-Agent Execution Authority Isolation
// ============================================================================

#[tokio::test]
async fn test_f01_child_dispatcher_isolation_and_write_denial() {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("child_isolation_f01.db");
    let pool = initialize_database(&db_path).await.unwrap();

    let mission_id = MissionId::new();
    let parent_agent_id = AgentId::new();
    let parent_task_id = TaskId::new();
    seed_test_mission(&pool, mission_id).await;

    let registry = Arc::new(CapabilityRegistry::new());
    registry.register_instance(m31a::capability::instance::CapabilityInstance::new(
        "fs.local",
        "Local FS",
        "1.0.0",
        CapabilityFamily::Filesystem,
        "test_fs",
        CapabilityPermissions::full_access(),
    ));

    let tool_reg = Arc::new(ToolRegistry::new_default(Arc::clone(&registry)));
    let runner = Arc::new(ToolPipelineRunner::new(tool_reg).with_db_pool(pool.clone()));

    // Parent context: implementer with full write permissions
    let parent_envelope = CapabilityEnvelope::workspace_write(["file_system", "process"]);
    let parent_context = ToolExecutionContext::new(
        Arc::clone(&registry),
        temp.path().to_path_buf(),
        CancellationToken::new(),
    )
    .with_mission_id(mission_id)
    .with_task_id(parent_task_id)
    .with_agent_id(parent_agent_id)
    .with_agent_role(AgentRole::implementer())
    .with_role_envelope(parent_envelope);

    let parent_dispatcher = ProductionActionDispatcher::new(
        runner,
        parent_context,
        Arc::new(AllowAllTestPolicy),
        AutonomyMode::Safe,
    )
    .with_db_pool(pool.clone());

    // Construct isolated child dispatcher for researcher (read-only)
    let child_agent_id = AgentId::new();
    let child_task_id = TaskId::new();
    let child_envelope = CapabilityEnvelope::read_only(["file_system"]);
    assert!(
        !child_envelope.allow_file_write,
        "Researcher role envelope must be read-only"
    );

    let child_dispatcher = parent_dispatcher.create_child_dispatcher(
        child_agent_id,
        child_task_id,
        AgentRole::researcher(),
        child_envelope,
        1,
    );

    // Verify child context isolation:
    assert_eq!(child_dispatcher.context().agent_id, Some(child_agent_id));
    assert_eq!(child_dispatcher.context().task_id, Some(child_task_id));
    assert_eq!(
        child_dispatcher.context().agent_role,
        Some(AgentRole::researcher())
    );
    assert_eq!(child_dispatcher.context().delegation_depth, 1);
    assert!(
        !child_dispatcher
            .context()
            .role_envelope
            .as_ref()
            .unwrap()
            .allow_file_write
    );

    // Parent identity must NOT be in child context
    assert_ne!(child_dispatcher.context().agent_id, Some(parent_agent_id));
    assert_ne!(child_dispatcher.context().task_id, Some(parent_task_id));

    // Child attempting mutating action (write_file) must be rejected by CapabilityCheckStage
    let write_req = ActionRequest {
        id: "act-child-write-1".to_string(),
        tool_name: "write_file".to_string(),
        parameters: serde_json::json!({
            "path": "test.txt",
            "content": "illegal child mutation"
        }),
    };
    let res = child_dispatcher.dispatch(&write_req).await.unwrap();
    assert!(
        !res.success,
        "Child researcher must NOT be able to execute mutating tool"
    );
    let err_msg = res.error.unwrap();
    assert!(
        err_msg.contains("WRITE_ACCESS_FORBIDDEN"),
        "Expected WRITE_ACCESS_FORBIDDEN error, got: {}",
        err_msg
    );
}
