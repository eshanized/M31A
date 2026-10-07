//! Integration tests for Phase 6: Agent Runtime Subsystem.

use m31a::agent::envelope::{CapabilityIntersectionError, calculate_eligible_capabilities};
use m31a::agent::model_policy::{StepBudget, StepLimitExceeded};
use m31a::agent::profile::{AgentProfile, ProfileOverride, ProfileOverrideError};
use m31a::ids::ExecutionId;
use m31a::kernel::plan::{CapabilityAccessMode, CapabilityRequirement};
use m31a::state_machine::agent::AgentRole;

#[test]
fn test_all_eight_canonical_roles_exist_and_immutable() {
    let roles = [
        AgentRole::planner(),
        AgentRole::researcher(),
        AgentRole::architect(),
        AgentRole::implementer(),
        AgentRole::reviewer(),
        AgentRole::verifier(),
        AgentRole::diagnostician(),
        AgentRole::integrator(),
    ];

    for role in roles {
        let profile = AgentProfile::built_in(role.clone());
        assert_eq!(profile.role, role);
        assert!(!profile.id.is_empty());
        assert!(!profile.description.is_empty());
        assert!(!profile.sandbox_policy.is_empty());
        assert!(!profile.prompt_ref.id.is_empty());
        // Canonical generation per role (wiring remediation v0.1.1).
        let expected = m31a::prompt::canonical_version(&profile.prompt_ref.id).unwrap_or(1);
        assert_eq!(profile.prompt_ref.version, expected);
        assert!(profile.max_steps > 0);
        assert!(profile.context_policy.default_max_tokens > 0);
        assert!(profile.termination_policy.step_stall_timeout_secs > 0);
        assert!(!profile.capability_policy.allowed_capabilities.is_empty());

        let fp1 = profile.fingerprint();
        let fp2 = profile.fingerprint();
        assert_eq!(fp1, fp2);
        assert_eq!(fp1.len(), 64);
    }
}

#[test]
fn test_agent_profile_definition_ten_fields_and_fingerprint() {
    let profile = AgentProfile::built_in(AgentRole::implementer());

    // 10 explicit fields
    assert_eq!(profile.id, "builtin-implementer-v2");
    assert_eq!(profile.role, AgentRole::implementer());
    assert!(!profile.description.is_empty());
    // Single model authority: role model policy defaults to the canonical
    // default model (config::canonical), never a retired provider id.
    assert_eq!(
        profile.model_policy.preferred_model,
        m31a::config::canonical::CANONICAL_DEFAULT_MODEL
    );
    assert!(profile.capability_policy.allow_file_write);
    assert_eq!(profile.sandbox_policy, "workspace_write");
    assert_eq!(profile.context_policy.default_max_tokens, 8192);
    assert_eq!(profile.max_steps, 30);
    assert_eq!(profile.termination_policy.step_stall_timeout_secs, 60);
    assert!(profile.prompt_ref.id.contains("implementer"));

    // SHA-256 fingerprint collision-resistance & determinism
    let fp = profile.fingerprint();
    assert_eq!(fp.len(), 64);

    // Override ceiling validation
    let err = profile
        .apply_override(ProfileOverride {
            max_steps: Some(100),
            ..Default::default()
        })
        .unwrap_err();
    assert_eq!(
        err,
        ProfileOverrideError::StepCeilingExceeded {
            ceiling: 30,
            requested: 100,
        }
    );
}

#[test]
fn test_capability_intersection_enforces_envelope_boundaries() {
    let reviewer_profile = AgentProfile::built_in(AgentRole::reviewer());
    let allowed_read = vec![CapabilityRequirement::new(
        "repo.read",
        CapabilityAccessMode::Read,
    )];
    let res = calculate_eligible_capabilities(&allowed_read, &reviewer_profile.capability_policy);
    assert!(res.is_ok());

    let forbidden_write = vec![CapabilityRequirement::new(
        "repo.read",
        CapabilityAccessMode::Write,
    )];
    let err =
        calculate_eligible_capabilities(&forbidden_write, &reviewer_profile.capability_policy)
            .unwrap_err();
    assert_eq!(err, CapabilityIntersectionError::WriteAccessForbidden);

    let implementer_profile = AgentProfile::built_in(AgentRole::implementer());
    let allowed_write = vec![
        CapabilityRequirement::new("fs.write", CapabilityAccessMode::Write),
        CapabilityRequirement::new("shell.exec", CapabilityAccessMode::Read),
    ];
    let res =
        calculate_eligible_capabilities(&allowed_write, &implementer_profile.capability_policy);
    assert!(res.is_ok());
}

#[test]
fn test_model_router_deterministic_selection() {
    use m31a::model::router::health::CircuitBreakerRegistry;
    use m31a::model::router::resolver::{ModelCandidate, ModelRouter, ModelTier, RoutingRequest};

    let router = ModelRouter::new();
    let health = CircuitBreakerRegistry::new();
    let request = RoutingRequest::new(AgentRole::implementer(), ModelTier::Standard);
    let candidates = vec![
        ModelCandidate::new("candidate-fast", "builtin", ModelTier::Fast, 8192),
        ModelCandidate::new("candidate-standard", "builtin", ModelTier::Standard, 8192),
    ];
    let selection = router
        .resolve_model(&request, &candidates, &health)
        .unwrap();
    assert_eq!(selection.model_name, "candidate-standard");
}

#[test]
fn test_step_budget_metering_separates_steps_from_tool_calls() {
    let mut budget = StepBudget::new(5);

    // Record 10 tool calls - steps should remain 0
    for _ in 0..10 {
        budget.record_tool_call();
    }
    assert_eq!(budget.tool_calls_consumed(), 10);
    assert_eq!(budget.steps_consumed(), 0);
    assert!(budget.check_admission().is_ok());

    // Record 5 steps - steps should reach limit
    for _ in 0..5 {
        assert!(budget.check_admission().is_ok());
        budget.record_step();
    }
    assert_eq!(budget.steps_consumed(), 5);
    let err = budget.check_admission().unwrap_err();
    assert_eq!(
        err,
        StepLimitExceeded {
            limit: 5,
            consumed: 5
        }
    );
}

#[test]
fn test_execution_id_tracer() {
    let id1 = ExecutionId::new();
    let id2 = ExecutionId::new();
    assert_ne!(id1, id2);
    let serialized = serde_json::to_string(&id1).unwrap();
    let deserialized: ExecutionId = serde_json::from_str(&serialized).unwrap();
    assert_eq!(id1, deserialized);
}

#[tokio::test]
async fn test_worker_dispatcher_lifecycle() {
    use m31a::agent::{ProductionWorkerDispatcher, TestModelCaller};
    use m31a::ids::{MissionId, TaskId};
    use m31a::kernel::seams::execution::{WorkExecutionRequest, WorkerDispatcher};
    use std::sync::Arc;
    use std::time::Duration;

    let dispatcher = ProductionWorkerDispatcher::new()
        .with_context_compiler(Arc::new(
            m31a::context::compiler::ProductionContextCompiler::new(),
        ))
        .with_model_caller(Arc::new(TestModelCaller::new(
            "task executed under worker supervision",
        )));
    let task_id = TaskId::new();
    let mission_id = MissionId::new();

    let agent_id = dispatcher
        .allocate_worker(
            task_id,
            mission_id,
            &["role:researcher".to_string(), "fs.read".to_string()],
        )
        .await
        .unwrap();

    let handle = dispatcher
        .dispatch_work(WorkExecutionRequest::new(
            mission_id,
            task_id,
            agent_id,
            "ctx-lifecycle",
        ))
        .await
        .unwrap();

    let mut result = None;
    for _ in 0..50 {
        tokio::time::sleep(Duration::from_millis(20)).await;
        if let Ok(res) = dispatcher.collect_result(&handle).await {
            result = Some(res);
            break;
        }
    }

    let res = result.expect("dispatched worker should complete");
    assert!(res.success);
    assert!(
        res.output
            .contains("task executed under worker supervision")
    );
    assert!(res.error_detail.is_none());
}

#[tokio::test]
async fn test_worker_dispatcher_executes_tool_pipeline() {
    use m31a::agent::{ModelProposal, ProductionWorkerDispatcher, TestModelCaller};
    use m31a::capability::providers::LocalFileSystemProvider;
    use m31a::capability::registry::CapabilityRegistry;
    use m31a::ids::{MissionId, TaskId};
    use m31a::kernel::seams::execution::{WorkExecutionRequest, WorkerDispatcher};
    use m31a::model::types::ModelToolCall;
    use std::sync::Arc;
    use std::time::Duration;

    let caps = Arc::new(CapabilityRegistry::new());
    let fs_service =
        Arc::new(LocalFileSystemProvider::new(std::env::current_dir().unwrap()).unwrap());
    caps.register_filesystem(fs_service);

    let model = Arc::new(TestModelCaller::from_proposals(vec![
        Ok(ModelProposal::ToolCalls {
            calls: vec![ModelToolCall::new(
                "read_file",
                serde_json::json!({"path": "Cargo.toml"}),
            )],
        }),
        Ok(ModelProposal::Complete {
            summary: "read Cargo.toml via tool pipeline successfully".to_string(),
            artifacts: Vec::new(),
        }),
    ]));

    let dispatcher = ProductionWorkerDispatcher::new()
        .with_capability_registry(caps)
        .with_context_compiler(Arc::new(
            m31a::context::compiler::ProductionContextCompiler::new(),
        ))
        .with_model_caller(model);

    let task_id = TaskId::new();
    let mission_id = MissionId::new();
    let agent_id = dispatcher
        .allocate_worker(
            task_id,
            mission_id,
            &["role:researcher".to_string(), "fs.read".to_string()],
        )
        .await
        .unwrap();

    let handle = dispatcher
        .dispatch_work(WorkExecutionRequest::new(
            mission_id,
            task_id,
            agent_id,
            "ctx-tool-pipeline",
        ))
        .await
        .unwrap();

    let mut result = None;
    for _ in 0..50 {
        tokio::time::sleep(Duration::from_millis(20)).await;
        if let Ok(res) = dispatcher.collect_result(&handle).await {
            result = Some(res);
            break;
        }
    }

    let res = result.expect("dispatched worker should complete");
    assert!(res.success);
    assert!(
        res.output
            .contains("read Cargo.toml via tool pipeline successfully")
    );
}

#[tokio::test]
async fn test_production_dispatcher_fails_closed_without_provider() {
    use m31a::agent::ProductionWorkerDispatcher;
    use m31a::ids::{MissionId, TaskId};
    use m31a::kernel::seams::execution::{WorkExecutionRequest, WorkerDispatcher};
    use std::time::Duration;

    if std::env::var("NVIDIA_API_KEY").is_err() {
        let dispatcher = ProductionWorkerDispatcher::new();
        let task_id = TaskId::new();
        let mission_id = MissionId::new();
        let agent_id = dispatcher
            .allocate_worker(
                task_id,
                mission_id,
                &["fs.write".to_string(), "fs.read".to_string()],
            )
            .await
            .unwrap();

        let handle = dispatcher
            .dispatch_work(WorkExecutionRequest::new(
                mission_id,
                task_id,
                agent_id,
                "ctx-fail-closed",
            ))
            .await
            .unwrap();

        let mut result = None;
        for _ in 0..50 {
            tokio::time::sleep(Duration::from_millis(20)).await;
            if let Ok(res) = dispatcher.collect_result(&handle).await {
                result = Some(res);
                break;
            }
        }

        let res = result.expect("dispatched worker should complete");
        assert!(
            !res.success,
            "production dispatcher must not fake success when provider is missing"
        );
        assert!(res.error_detail.is_some());
    }
}

#[tokio::test]
async fn test_worker_dispatcher_rejects_out_of_envelope_allocation() {
    use m31a::agent::ProductionWorkerDispatcher;
    use m31a::ids::{MissionId, TaskId};
    use m31a::kernel::seams::execution::{ExecutionError, WorkerDispatcher};

    let dispatcher = ProductionWorkerDispatcher::new();
    let err = dispatcher
        .allocate_worker(
            TaskId::new(),
            MissionId::new(),
            &["plan.propose".to_string(), "unauthorized.cap".to_string()],
        )
        .await
        .unwrap_err();

    assert!(matches!(err, ExecutionError::AllocationFailed(_)));
}

#[tokio::test]
async fn test_worker_panic_containment_in_dispatcher() {
    use m31a::agent::ProductionWorkerDispatcher;
    use m31a::agent::supervisor::{AgentOutcome, FailureClass, FailureEvidence};
    use m31a::ids::{AgentId, JobId, TaskId};
    use m31a::kernel::seams::execution::{WorkExecutionHandle, WorkerDispatcher};
    use std::collections::HashMap;

    let dispatcher = ProductionWorkerDispatcher::new();
    let job_id = JobId::new();
    let task_id = TaskId::new();
    let agent_id = AgentId::new();

    let panic_outcome = AgentOutcome::Failed(FailureEvidence {
        failure_class: FailureClass::WorkerPanic,
        message: "isolated worker panic: simulated worker unwind".to_string(),
        step_number: 1,
        occurred_at: chrono::Utc::now(),
        is_panic: true,
        diagnostics: HashMap::new(),
    });

    dispatcher.set_job_outcome(job_id, panic_outcome).await;

    let handle = WorkExecutionHandle {
        job_id,
        task_id,
        agent_id,
    };

    let res = dispatcher.collect_result(&handle).await.unwrap();
    assert!(!res.success);
    assert!(res.error_detail.unwrap().contains("WorkerPanic"));
}

#[tokio::test]
async fn test_worker_cooperative_cancellation_lifecycle() {
    use m31a::agent::ProductionWorkerDispatcher;
    use m31a::agent::supervisor::AgentOutcome;
    use m31a::ids::{AgentId, JobId, TaskId};
    use m31a::kernel::seams::execution::{WorkExecutionHandle, WorkerDispatcher};

    let dispatcher = ProductionWorkerDispatcher::new();
    let job_id = JobId::new();
    let task_id = TaskId::new();
    let agent_id = AgentId::new();

    let cancelled_outcome = AgentOutcome::Cancelled {
        reason: "operator abort requested".to_string(),
        steps_consumed: 3,
    };

    dispatcher.set_job_outcome(job_id, cancelled_outcome).await;

    let handle = WorkExecutionHandle {
        job_id,
        task_id,
        agent_id,
    };

    let res = dispatcher.collect_result(&handle).await.unwrap();
    assert!(!res.success);
    assert!(
        res.error_detail
            .unwrap()
            .contains("cancelled: operator abort requested")
    );
}

// ============================================================================
// Wave 4: Agent Persistence, Handoff Arbitration, and Completion Gate Tests
// ============================================================================

use futures::StreamExt;
use m31a::agent::handoff::{AgentHandoffArbiter, HandoffError, HandoffProposal};
use m31a::events::bus::{BroadcastEventBus, EventBus, EventFilter};
use m31a::events::types::EventType;
use m31a::ids::{AgentId, ArtifactId, MissionId, TaskId};
use m31a::persistence::initialize_database;
use m31a::persistence::sqlite::repositories::{
    AgentRepository, SqliteAgentHandoffRepository, SqliteAgentRepository,
};
use m31a::state::agent::Agent;
use m31a::state_machine::AgentState;
use std::sync::Arc;
use tempfile::tempdir;

async fn setup_test_entities(pool: &sqlx::SqlitePool) -> (MissionId, TaskId, AgentId) {
    let mission_id = MissionId::new();
    let task_id = TaskId::new();
    let agent_id = AgentId::new();
    let now = chrono::Utc::now().to_rfc3339();

    sqlx::query(
        "INSERT INTO missions (id, objective, status, created_at, updated_at) VALUES (?, 'Test Mission', 'Running', ?, ?)"
    )
    .bind(mission_id.as_bytes().as_slice())
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO tasks (id, mission_id, title, status, created_at, updated_at) VALUES (?, ?, 'Test Task', 'Running', ?, ?)"
    )
    .bind(task_id.as_bytes().as_slice())
    .bind(mission_id.as_bytes().as_slice())
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO agents (id, mission_id, task_id, role, status, created_at, updated_at) VALUES (?, ?, ?, 'implementer', 'Running', ?, ?)"
    )
    .bind(agent_id.as_bytes().as_slice())
    .bind(mission_id.as_bytes().as_slice())
    .bind(task_id.as_bytes().as_slice())
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await
    .unwrap();

    (mission_id, task_id, agent_id)
}

#[tokio::test]
async fn test_sqlite_agent_repository_lifecycle_and_runtime_fields() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("agent_repo_test.db");
    let pool = initialize_database(&db_path).await.unwrap();

    let mission_id = MissionId::new();
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO missions (id, objective, status, created_at, updated_at) VALUES (?, 'Agent Mission', 'Running', ?, ?)"
    )
    .bind(mission_id.as_bytes().as_slice())
    .bind(&now)
    .bind(&now)
    .execute(&pool)
    .await
    .unwrap();

    let agent_repo = SqliteAgentRepository::new(pool.clone());
    let agent_id = AgentId::new();
    let task_id = TaskId::new();

    sqlx::query(
        "INSERT INTO tasks (id, mission_id, title, status, created_at, updated_at) VALUES (?, ?, 'Agent Task', 'Running', ?, ?)"
    )
    .bind(task_id.as_bytes().as_slice())
    .bind(mission_id.as_bytes().as_slice())
    .bind(&now)
    .bind(&now)
    .execute(&pool)
    .await
    .unwrap();

    let agent = Agent::new(agent_id, mission_id, "implementer".to_string()).with_runtime_details(
        Some(task_id),
        "fp_sample_sha256".to_string(),
        45,
        "claude-3-7-sonnet".to_string(),
    );

    // Insert
    agent_repo.insert(&agent).await.unwrap();

    // Get
    let loaded = agent_repo
        .get(agent_id)
        .await
        .unwrap()
        .expect("agent found");
    assert_eq!(loaded.id, agent_id);
    assert_eq!(loaded.mission_id, mission_id);
    assert_eq!(loaded.task_id, Some(task_id));
    assert_eq!(loaded.role, "implementer");
    assert_eq!(loaded.status, AgentState::Starting);
    assert_eq!(loaded.profile_fingerprint, "fp_sample_sha256");
    assert_eq!(loaded.max_steps, 45);
    assert_eq!(loaded.steps_consumed, 0);
    assert_eq!(loaded.model_name, "claude-3-7-sonnet");
    assert!(loaded.completed_at.is_none());

    // Update status to terminal
    agent_repo
        .update_status(agent_id, AgentState::Completed)
        .await
        .unwrap();
    let completed = agent_repo
        .get(agent_id)
        .await
        .unwrap()
        .expect("agent found");
    assert_eq!(completed.status, AgentState::Completed);
    assert!(completed.completed_at.is_some());

    // List by mission
    let list = agent_repo.list_by_mission(mission_id).await.unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].id, agent_id);
}

#[tokio::test]
async fn test_handoff_persistence_and_event_bus_emission() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("handoff_test.db");
    let pool = initialize_database(&db_path).await.unwrap();

    let (mission_id, task_id, agent_id) = setup_test_entities(&pool).await;

    let bus = Arc::new(BroadcastEventBus::new(100));
    let mut rx = bus.subscribe(EventFilter::all()).await;

    let handoff_repo = SqliteAgentHandoffRepository::new(pool.clone());
    let arbiter = AgentHandoffArbiter::new(bus.clone());

    let artifact_id = ArtifactId::new();
    let proposal = HandoffProposal {
        source_task_id: task_id,
        source_agent_id: agent_id,
        source_role: AgentRole::implementer(),
        target_role: AgentRole::reviewer(),
        reason: "code changes ready for review".to_string(),
        required_inputs: vec!["git diff".to_string(), "unit test logs".to_string()],
        artifacts: vec![artifact_id],
        unresolved_questions: vec!["edge case in negative index".to_string()],
        current_depth: 0,
    };

    let record = arbiter
        .evaluate_and_record_handoff(&handoff_repo, mission_id, proposal)
        .await
        .unwrap();

    // Verify durable persistence in SQLite (AGT-06, D-14)
    let mission_handoffs = handoff_repo.find_by_mission(mission_id).await.unwrap();
    assert_eq!(mission_handoffs.len(), 1);
    assert_eq!(mission_handoffs[0].id, record.id);
    assert_eq!(mission_handoffs[0].source_role, AgentRole::implementer());
    assert_eq!(mission_handoffs[0].target_role, AgentRole::reviewer());
    assert_eq!(mission_handoffs[0].artifacts, vec![artifact_id]);
    assert_eq!(mission_handoffs[0].required_inputs.len(), 2);
    assert_eq!(mission_handoffs[0].unresolved_questions.len(), 1);

    let task_handoffs = handoff_repo.find_by_task(task_id).await.unwrap();
    assert_eq!(task_handoffs.len(), 1);
    assert_eq!(task_handoffs[0].id, record.id);

    // Verify EventBus emission (D-14, D-15)
    let event = rx.next().await.unwrap().unwrap();
    match event.event_type {
        EventType::AgentHandoffRecorded {
            handoff_id,
            mission_id: ev_m_id,
            source_agent_id,
            target_role,
            reason,
        } => {
            assert_eq!(handoff_id, record.id);
            assert_eq!(ev_m_id, mission_id);
            assert_eq!(source_agent_id, agent_id);
            assert_eq!(target_role, "reviewer");
            assert_eq!(reason, "code changes ready for review");
        }
        other => panic!("expected AgentHandoffRecorded, got {:?}", other),
    }
}

#[tokio::test]
async fn test_mid_task_delegation_recursion_depth_limiting() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("delegation_limit_test.db");
    let pool = initialize_database(&db_path).await.unwrap();

    let (mission_id, task_id, agent_id) = setup_test_entities(&pool).await;

    let bus = Arc::new(BroadcastEventBus::new(100));
    let handoff_repo = SqliteAgentHandoffRepository::new(pool.clone());
    let arbiter = AgentHandoffArbiter::new(bus.clone());

    // Depth 0 succeeds
    let p0 = HandoffProposal {
        source_task_id: task_id,
        source_agent_id: agent_id,
        source_role: AgentRole::implementer(),
        target_role: AgentRole::researcher(),
        reason: "research question 1".to_string(),
        required_inputs: vec![],
        artifacts: vec![],
        unresolved_questions: vec![],
        current_depth: 0,
    };
    assert!(
        arbiter
            .evaluate_and_record_handoff(&handoff_repo, mission_id, p0)
            .await
            .is_ok()
    );

    // Depth 1 succeeds
    let p1 = HandoffProposal {
        source_task_id: task_id,
        source_agent_id: agent_id,
        source_role: AgentRole::researcher(),
        target_role: AgentRole::diagnostician(),
        reason: "diagnostic question 2".to_string(),
        required_inputs: vec![],
        artifacts: vec![],
        unresolved_questions: vec![],
        current_depth: 1,
    };
    assert!(
        arbiter
            .evaluate_and_record_handoff(&handoff_repo, mission_id, p1)
            .await
            .is_ok()
    );

    // Depth 2 is rejected by MAX_DELEGATION_DEPTH = 2 (T-06-09, D-13)
    let p2 = HandoffProposal {
        source_task_id: task_id,
        source_agent_id: agent_id,
        source_role: AgentRole::diagnostician(),
        target_role: AgentRole::planner(),
        reason: "attempt recursive replan".to_string(),
        required_inputs: vec![],
        artifacts: vec![],
        unresolved_questions: vec![],
        current_depth: 2,
    };
    let err = arbiter
        .evaluate_and_record_handoff(&handoff_repo, mission_id, p2)
        .await
        .unwrap_err();
    assert_eq!(err, HandoffError::MaxDelegationDepthExceeded(2));
}

#[tokio::test]
async fn test_task_boundary_handoff_derives_clean_record() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("task_boundary_test.db");
    let pool = initialize_database(&db_path).await.unwrap();

    let (mission_id, task_id, agent_id) = setup_test_entities(&pool).await;

    let bus = Arc::new(BroadcastEventBus::new(100));
    let handoff_repo = SqliteAgentHandoffRepository::new(pool.clone());
    let arbiter = AgentHandoffArbiter::new(bus.clone());

    let proposal = HandoffProposal {
        source_task_id: task_id,
        source_agent_id: agent_id,
        source_role: AgentRole::implementer(),
        target_role: AgentRole::reviewer(),
        reason: "task boundary reached, handoff to reviewer".to_string(),
        required_inputs: vec!["patch.diff".to_string()],
        artifacts: vec![],
        unresolved_questions: vec![],
        current_depth: 0,
    };

    let record = arbiter
        .evaluate_and_record_handoff(&handoff_repo, mission_id, proposal)
        .await
        .unwrap();

    assert_eq!(record.source_role, AgentRole::implementer());
    assert_eq!(record.target_role, AgentRole::reviewer());
    assert_eq!(record.task_state, "delegated");
    assert_eq!(record.reason, "task boundary reached, handoff to reviewer");
}

#[tokio::test]
async fn test_end_to_end_agent_runtime_supervision_and_completion() {
    use m31a::agent::{ProductionWorkerDispatcher, TestModelCaller};
    use m31a::context::compiler::ProductionContextCompiler;
    use m31a::kernel::seams::context::{ContextCompilationRequest, ContextCompiler};
    use m31a::kernel::seams::execution::{WorkExecutionRequest, WorkerDispatcher};
    use std::time::Duration;

    // 1. Set up SQLite, EventBus, and Handoff infrastructure
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("e2e_runtime_test.db");
    let pool = initialize_database(&db_path).await.unwrap();
    let (mission_id, task_id, _seed_agent_id) = setup_test_entities(&pool).await;

    let bus = Arc::new(BroadcastEventBus::new(100));
    let handoff_repo = SqliteAgentHandoffRepository::new(pool.clone());
    let arbiter = AgentHandoffArbiter::new(bus.clone());

    // 2. Compile fresh context using production ContextCompiler
    let compiler = ProductionContextCompiler::new();
    let req = ContextCompilationRequest::new(mission_id, task_id, 4096)
        .with_mission_objective("Law 1: Model proposes, runtime decides.")
        .with_task_objective("Implement binary search routine");
    let compiled = compiler.compile_context(req).await.unwrap();
    assert!(compiled.system_prompt.contains("binary search"));

    // 3. Dispatch worker with bounded turns (explicitly isolated test
    // compiler; production binds the runtime-shared compiler).
    let dispatcher = ProductionWorkerDispatcher::new()
        .with_context_compiler(Arc::new(
            m31a::context::compiler::ProductionContextCompiler::new(),
        ))
        .with_model_caller(Arc::new(TestModelCaller::new(
            "task executed under worker supervision",
        )));
    let allocated_agent_id = dispatcher
        .allocate_worker(
            task_id,
            mission_id,
            &["role:researcher".to_string(), "fs.read".to_string()],
        )
        .await
        .unwrap();

    let handle = dispatcher
        .dispatch_work(WorkExecutionRequest::new(
            mission_id,
            task_id,
            allocated_agent_id,
            "ctx-e2e-search",
        ))
        .await
        .unwrap();

    let mut result = None;
    for _ in 0..50 {
        tokio::time::sleep(Duration::from_millis(20)).await;
        if let Ok(res) = dispatcher.collect_result(&handle).await {
            result = Some(res);
            break;
        }
    }
    let res = result.expect("dispatched worker should complete");
    assert!(res.success);

    // Persist allocated agent aggregate in SQLite (AGT-05)
    let agent_repo = SqliteAgentRepository::new(pool.clone());
    let agent_record = Agent::new(allocated_agent_id, mission_id, "implementer".to_string())
        .with_runtime_details(
            Some(task_id),
            "fp_e2e_implementer".to_string(),
            50,
            "claude-3-7-sonnet".to_string(),
        );
    agent_repo.insert(&agent_record).await.unwrap();

    // 4. Runtime arbitrates handoff from Implementer to Reviewer
    let produced_artifact = ArtifactId::new();
    let handoff_prop = HandoffProposal {
        source_task_id: task_id,
        source_agent_id: allocated_agent_id,
        source_role: AgentRole::implementer(),
        target_role: AgentRole::reviewer(),
        reason: "implementation complete, requesting code review".to_string(),
        required_inputs: vec!["git diff".to_string()],
        artifacts: vec![produced_artifact],
        unresolved_questions: vec![],
        current_depth: 0,
    };

    let handoff_record = arbiter
        .evaluate_and_record_handoff(&handoff_repo, mission_id, handoff_prop)
        .await
        .unwrap();
    assert_eq!(handoff_record.target_role, AgentRole::reviewer());
}
