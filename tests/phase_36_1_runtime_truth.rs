//! Phase 36.1 — Runtime Truth Reconciliation & Final Handoff Verification Tests.
//!
//! Validates:
//! 1. Exact user experience journey (coffee shop with Next.js) through intent, plan, tasks, auth, materialization
//! 2. Real execution handoff: Intent -> Plan -> Tasks -> Auth -> Materialize -> Scheduler -> Controller
//! 3. Provenance tracking: authorized revisions == materialized graph revision == executed tasks
//! 4. Pre-authorization side-effect prevention: workspace untouched before authorization
//! 5. Discovery authority: PromptSemanticAnalyzer correlates with semantic architecture clarity, not word count
//! 6. Dynamic question structure: inspectable question_id, target_unknown, reason, and UserProvided provenance
//! 7. Stale authorization defense: invalidated authorization cannot reach materializer or execution
//! 8. Direct materializer bypass prevention: mismatched revisions fail closed

use m31a::controller::AutonomyController;
use m31a::controller::dependencies::ControllerDependencies;
use m31a::controller::progress::LoopStage;
use m31a::dag::materializer::TaskGraphMaterializer;
use m31a::events::bus::{BroadcastEventBus, EventBus};
use m31a::ids::MissionId;
use m31a::interaction::action::ApplicationAction;
use m31a::kernel::plan::{
    CandidatePlan, CandidateTask, CandidateTaskKey, ResourceEstimate, VerificationStrategy,
};
use m31a::persistence::sqlite::schema::initialize_database;
use m31a::planning::review::{
    AuthorizationDecision, ExecutionAuthorization, PlanReviewStatus, PreExecutionCoordinator,
    PreExecutionResponse, TaskReviewStatus,
};
use m31a::state_machine::AutonomyMode;
use m31a::state_machine::agent::AgentRole;
use m31a::state_machine::lifecycle::LifecycleStage;
use std::sync::Arc;
use tempfile::tempdir;
use tokio_util::sync::CancellationToken;

async fn setup_test_env() -> (tempfile::TempDir, sqlx::SqlitePool, Arc<BroadcastEventBus>) {
    let dir = tempdir().expect("tempdir");
    let db_path = dir.path().join("truth_test.db");
    let pool = initialize_database(&db_path).await.expect("init db");
    let bus = Arc::new(BroadcastEventBus::new(1024));
    (dir, pool, bus)
}

async fn create_test_session(
    pool: &sqlx::SqlitePool,
    dir: &std::path::Path,
) -> (String, MissionId) {
    let repo = m31a::interaction::session::SqliteSessionRepository::new(pool.clone());
    let session = repo.create_session(dir).await.expect("create test session");
    (session.id.to_string(), session.active_mission_id.unwrap())
}

// ---------------------------------------------------------------------------
// 1. Exact User Experience: "Eshan's Cafe" Coffee Shop with Next.js (PRD §27)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_exact_user_journey_coffee_shop_nextjs() {
    let (dir, pool, bus) = setup_test_env().await;
    let coordinator = PreExecutionCoordinator::deterministic_test(pool.clone(), Some(bus));
    let (session_id, mission_id) = create_test_session(&pool, dir.path()).await;

    // Step 1: User intent contains architecture ("next") so questions are not spurious
    let prompt = "I want to build a website with Next.js for my coffee shop named Eshan's Cafe.";
    let resp = coordinator
        .init_intent(&session_id, prompt, "operator_eshan")
        .await
        .expect("init intent");

    // Step 2: Plan is synthesized and presented for review
    let initial_revision = match resp {
        PreExecutionResponse::PlanForReview { revision, .. } => {
            assert_eq!(revision.revision, 1);
            assert_eq!(revision.status, PlanReviewStatus::Draft);
            assert!(revision.content.objective.contains("Next.js"));
            // Verify projections
            let md = revision.to_markdown();
            assert!(md.contains("Plan:"));
            let json = revision.to_json().expect("plan to json");
            assert!(json.contains("plan_id"));
            revision
        }
        other => panic!("Expected PlanForReview, got {:?}", other),
    };

    // Step 3: Operator reviews and accepts the plan
    let accept_plan_resp = coordinator
        .handle_action(
            ApplicationAction::PlanAcceptRequested {
                session_id: Some(session_id.clone()),
            },
            "operator_eshan",
        )
        .await
        .expect("accept plan");

    // Step 4: Tasks are lowered and presented for review
    let _task_revision = match accept_plan_resp {
        PreExecutionResponse::TasksForReview { revision, .. } => {
            assert_eq!(revision.revision, 1);
            assert_eq!(revision.plan_revision, initial_revision.revision);
            assert_eq!(revision.status, TaskReviewStatus::Draft);
            assert!(!revision.tasks.is_empty());
            revision
        }
        other => panic!("Expected TasksForReview, got {:?}", other),
    };

    // Step 5: Operator accepts the candidate tasks
    let accept_tasks_resp = coordinator
        .handle_action(
            ApplicationAction::TasksAcceptRequested {
                session_id: Some(session_id.clone()),
            },
            "operator_eshan",
        )
        .await
        .expect("accept tasks");

    // Step 6: Runtime demands explicit execution authorization
    match accept_tasks_resp {
        PreExecutionResponse::AuthorizationRequested {
            plan_revision,
            task_revision: t_rev,
            message,
            ..
        } => {
            assert_eq!(plan_revision, 1);
            assert_eq!(t_rev, 1);
            assert!(message.contains("Proceed with implementation?"));
        }
        other => panic!("Expected AuthorizationRequested, got {:?}", other),
    }

    // Step 7: Operator authorizes execution
    let auth_resp = coordinator
        .handle_action(
            ApplicationAction::ExecutionAuthorizationSubmitted {
                session_id: Some(session_id.clone()),
                decision: true,
                reason: None,
            },
            "operator_eshan",
        )
        .await
        .expect("authorize execution");

    let (plan, tasks, auth) = match auth_resp {
        PreExecutionResponse::ReadyToExecute {
            plan,
            tasks,
            authorization,
            ..
        } => {
            assert_eq!(authorization.plan_revision, 1);
            assert_eq!(authorization.task_revision, 1);
            assert_eq!(authorization.decision, AuthorizationDecision::Authorized);
            assert_eq!(authorization.authorized_by, "operator_eshan");
            (plan, tasks, authorization)
        }
        other => panic!("Expected ReadyToExecute, got {:?}", other),
    };

    // Step 8: Materialize with revision-binding gate
    let materializer = TaskGraphMaterializer::new(pool.clone());
    let mut plan_to_materialize = plan;
    plan_to_materialize.tasks = tasks;

    let graph = materializer
        .materialize_authorized(
            mission_id,
            &plan_to_materialize,
            auth.plan_revision,
            auth.task_revision,
            &auth,
        )
        .await
        .expect("materialize authorized graph");

    assert!(!graph.tasks.is_empty());
    assert_eq!(graph.tasks.len(), plan_to_materialize.tasks.len());
}

// ---------------------------------------------------------------------------
// 2. Real Execution Handoff: Intent -> Auth -> Materialize -> Scheduler (PRD §12)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_real_execution_handoff_reaches_scheduler_and_controller() {
    let (dir, pool, bus) = setup_test_env().await;
    let coordinator = PreExecutionCoordinator::deterministic_test(pool.clone(), Some(bus.clone()));
    let (session_id, mission_id) = create_test_session(&pool, dir.path()).await;

    // 1. Advance through full governance
    let _ = coordinator
        .init_intent(&session_id, "Build a microservice API in Rust", "operator")
        .await
        .expect("init");

    let _ = coordinator
        .handle_action(
            ApplicationAction::PlanAcceptRequested {
                session_id: Some(session_id.clone()),
            },
            "operator",
        )
        .await
        .expect("accept plan");

    let _ = coordinator
        .handle_action(
            ApplicationAction::TasksAcceptRequested {
                session_id: Some(session_id.clone()),
            },
            "operator",
        )
        .await
        .expect("accept tasks");

    let auth_resp = coordinator
        .handle_action(
            ApplicationAction::ExecutionAuthorizationSubmitted {
                session_id: Some(session_id.clone()),
                decision: true,
                reason: None,
            },
            "operator",
        )
        .await
        .expect("authorize");

    let (mut plan, tasks, auth) = match auth_resp {
        PreExecutionResponse::ReadyToExecute {
            plan,
            tasks,
            authorization,
            ..
        } => (plan, tasks, authorization),
        other => panic!("Expected ReadyToExecute, got {:?}", other),
    };

    plan.tasks = tasks;
    let materializer = TaskGraphMaterializer::new(pool.clone());

    // 2. Materialize authorized graph
    let graph = materializer
        .materialize_authorized(
            mission_id,
            &plan,
            auth.plan_revision,
            auth.task_revision,
            &auth,
        )
        .await
        .expect("materialize");

    // 3. Connect to real SchedulerEngine
    let deps = ControllerDependencies::production(
        pool.clone(),
        dir.path().to_path_buf(),
        dir.path().join(".m31a"),
        Some(bus.clone()),
    );

    let expected_task_id = *graph.tasks.keys().next().unwrap();

    // Verify SchedulerEngine directly discovers ready tasks from the materialized TaskGraph in SQLite
    let ready_work = deps
        .scheduler()
        .find_ready_work(mission_id)
        .await
        .expect("scheduler find ready work");

    assert!(
        !ready_work.ready_tasks.is_empty(),
        "Scheduler must immediately discover ready work from materialized graph"
    );
    assert_eq!(ready_work.ready_tasks[0].task_id, expected_task_id);

    // 4. Initialize AutonomyController on the mission
    let cancel = CancellationToken::new();
    let mut controller = AutonomyController::new(
        mission_id,
        AutonomyMode::Safe,
        deps,
        bus as Arc<dyn EventBus>,
        cancel,
    );

    // Step 1: Observe -> checks planner seam, detects ready work in scheduler, bypasses replanning!
    assert_eq!(controller.progress.current_stage, LoopStage::Observe);
    let outcome = controller.step().await.expect("step observe");
    assert_eq!(
        outcome,
        m31a::controller::StageOutcome::Advance(LoopStage::IdentifyReadyWork)
    );

    // Step 2: IdentifyReadyWork -> grabs the ready task from the materialized graph!
    let outcome2 = controller.step().await.expect("step identify ready work");
    assert_eq!(
        outcome2,
        m31a::controller::StageOutcome::Advance(LoopStage::ValidatePolicyAndResources)
    );
    assert!(controller.active_task.is_some());
    assert_eq!(
        controller.active_task.as_ref().unwrap().task_id,
        expected_task_id
    );
}

// ---------------------------------------------------------------------------
// 3. Provenance Tracking (PRD §13)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_provenance_binding_across_layers() {
    let (dir, pool, bus) = setup_test_env().await;
    let coordinator = PreExecutionCoordinator::deterministic_test(pool.clone(), Some(bus));
    let (session_id, _mission_id) = create_test_session(&pool, dir.path()).await;

    let _ = coordinator
        .init_intent(&session_id, "Build a Docker container cluster", "dev_alice")
        .await
        .unwrap();

    let _ = coordinator
        .handle_action(
            ApplicationAction::PlanAcceptRequested {
                session_id: Some(session_id.clone()),
            },
            "dev_alice",
        )
        .await
        .unwrap();

    let _ = coordinator
        .handle_action(
            ApplicationAction::TasksAcceptRequested {
                session_id: Some(session_id.clone()),
            },
            "dev_alice",
        )
        .await
        .unwrap();

    let auth_resp = coordinator
        .handle_action(
            ApplicationAction::ExecutionAuthorizationSubmitted {
                session_id: Some(session_id.clone()),
                decision: true,
                reason: None,
            },
            "dev_alice",
        )
        .await
        .unwrap();

    let (plan, tasks, auth) = match auth_resp {
        PreExecutionResponse::ReadyToExecute {
            plan,
            tasks,
            authorization,
            ..
        } => (plan, tasks, authorization),
        other => panic!("Expected ReadyToExecute, got {:?}", other),
    };

    // Verify cryptographic and aggregate provenance
    assert_eq!(auth.plan_revision, 1);
    assert_eq!(auth.task_revision, 1);
    assert_eq!(auth.authorized_by, "dev_alice");
    assert_eq!(plan.revision, 1);
    assert_eq!(tasks.len(), 1);

    // Verify persisted state matches provenance exactly
    let repo = coordinator.lifecycle_repo();
    let persisted = repo
        .load_lifecycle_state(&session_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(persisted.stage, LifecycleStage::ExecutionAuthorized);
    assert_eq!(persisted.plan_revision, 1);
    assert_eq!(persisted.task_revision, 1);
    assert_eq!(persisted.authorization_id, Some(auth.id));
}

// ---------------------------------------------------------------------------
// 4. Pre-Authorization Side Effects Prevention (PRD §14)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_pre_authorization_guarantees_zero_workspace_mutations() {
    let (dir, pool, bus) = setup_test_env().await;
    let workspace = dir.path().join("workspace");
    std::fs::create_dir_all(&workspace).unwrap();

    let sentinel_file = workspace.join("source.rs");
    std::fs::write(&sentinel_file, "fn main() { println!(\"original\"); }").unwrap();
    let initial_modified = std::fs::metadata(&sentinel_file)
        .unwrap()
        .modified()
        .unwrap();

    let coordinator = PreExecutionCoordinator::deterministic_test(pool.clone(), Some(bus));
    let (session_id, _mission_id) = create_test_session(&pool, &workspace).await;

    // Advance to ExecutionAwaitingAuthorization with a prompt containing architecture
    let _ = coordinator
        .init_intent(
            &session_id,
            "Refactor and clean up Rust backend source code",
            "attacker",
        )
        .await
        .unwrap();

    let _ = coordinator
        .handle_action(
            ApplicationAction::PlanAcceptRequested {
                session_id: Some(session_id.clone()),
            },
            "attacker",
        )
        .await
        .unwrap();

    let _ = coordinator
        .handle_action(
            ApplicationAction::TasksAcceptRequested {
                session_id: Some(session_id.clone()),
            },
            "attacker",
        )
        .await
        .unwrap();

    // Verify sentinel file is completely untouched
    let content = std::fs::read_to_string(&sentinel_file).unwrap();
    assert_eq!(content, "fn main() { println!(\"original\"); }");
    let current_modified = std::fs::metadata(&sentinel_file)
        .unwrap()
        .modified()
        .unwrap();
    assert_eq!(initial_modified, current_modified);
}

// ---------------------------------------------------------------------------
// 5. Discovery Authority: Architecture Semantics vs Word Count (PRD §18)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_discovery_semantics_uncorrelated_with_word_count() {
    let (dir, pool, bus) = setup_test_env().await;
    let coordinator = PreExecutionCoordinator::deterministic_test(pool.clone(), Some(bus));

    // Case 1: Short (4 words) but ambiguous -> requires questions
    let (sid1, _) = create_test_session(&pool, dir.path()).await;
    let resp1 = coordinator
        .init_intent(&sid1, "Build a web app", "user")
        .await
        .unwrap();
    assert!(
        matches!(resp1, PreExecutionResponse::QuestionsRequired { .. }),
        "Short ambiguous prompt must require questions"
    );

    // Case 2: Short (4 words) and specific with architecture -> skips questions directly to PlanReview
    let (sid2, _) = create_test_session(&pool, dir.path()).await;
    let resp2 = coordinator
        .init_intent(&sid2, "Build a React API", "user")
        .await
        .unwrap();
    assert!(
        matches!(resp2, PreExecutionResponse::PlanForReview { .. }),
        "Short prompt with architecture keywords must bypass questions"
    );

    // Case 3: Long (25 words) but ambiguous -> requires questions
    let (sid3, _) = create_test_session(&pool, dir.path()).await;
    let long_ambiguous = "I need an application for my organization that helps people connect \
                          and do important work together across different locations efficiently \
                          without any issues or problems";
    let resp3 = coordinator
        .init_intent(&sid3, long_ambiguous, "user")
        .await
        .unwrap();
    assert!(
        matches!(resp3, PreExecutionResponse::QuestionsRequired { .. }),
        "Long vague prompt without architecture must require questions"
    );

    // Case 4: Long (20 words) and specific with architecture -> skips questions
    let (sid4, _) = create_test_session(&pool, dir.path()).await;
    let long_specific = "Deploy a high-availability Kubernetes container cluster with an ingress \
                         controller and Prometheus monitoring stack for production services";
    let resp4 = coordinator
        .init_intent(&sid4, long_specific, "user")
        .await
        .unwrap();
    assert!(
        matches!(resp4, PreExecutionResponse::PlanForReview { .. }),
        "Long specific prompt with architecture must bypass questions"
    );
}

// ---------------------------------------------------------------------------
// 6. Dynamic Question Inspection & Provenance (PRD §19)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_dynamic_questions_are_inspectable_and_record_user_provenance() {
    let (dir, pool, bus) = setup_test_env().await;
    let coordinator = PreExecutionCoordinator::deterministic_test(pool.clone(), Some(bus));
    let (session_id, _mission_id) = create_test_session(&pool, dir.path()).await;

    let resp = coordinator
        .init_intent(&session_id, "Build a web app", "user")
        .await
        .unwrap();

    let questions = match resp {
        PreExecutionResponse::QuestionsRequired { questions, .. } => {
            assert!(!questions.is_empty());
            let q = &questions[0];
            // Inspectable fields
            assert_eq!(q.target_unknown, "unk_arch_choice");
            assert!(q.reason.contains("Application architecture"));
            assert!(q.text.contains("intended preference"));
            assert!(!q.options.is_empty());
            assert!(q.blocking);
            questions
        }
        other => panic!("Expected QuestionsRequired, got {:?}", other),
    };

    // Operator answers with custom answer
    let answer_resp = coordinator
        .submit_answer(
            &session_id,
            &questions[0].question_id,
            "Use SvelteKit frontend with SQLite and TailwindCSS",
            "operator_sarah",
        )
        .await
        .unwrap();

    // After answering, discovery converges to PlanReview
    assert!(matches!(
        answer_resp,
        PreExecutionResponse::PlanForReview { .. }
    ));

    // Verify stored answer has UserProvided provenance in IntentState
    let repo = coordinator.lifecycle_repo();
    let stored_questions = repo.load_discovery_questions(&session_id).await.unwrap();
    let q_record = stored_questions
        .iter()
        .find(|q| q.question_id == questions[0].question_id)
        .unwrap();
    assert_eq!(q_record.status, "answered");
    assert_eq!(
        q_record.answer.as_deref(),
        Some("Use SvelteKit frontend with SQLite and TailwindCSS")
    );
    assert_eq!(q_record.answered_by.as_deref(), Some("operator_sarah"));
}

// ---------------------------------------------------------------------------
// 7. Stale Authorization Defense (PRD §16)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_stale_authorization_fails_closed_before_execution() {
    let (_dir, pool, _bus) = setup_test_env().await;
    let materializer = TaskGraphMaterializer::new(pool.clone());
    let mission_id = MissionId::new();

    let plan = CandidatePlan::new(
        "plan-1",
        "Test Stale Defense",
        vec![CandidateTask::new(
            CandidateTaskKey::new("TASK-01"),
            "Execute Task",
            AgentRole::implementer(),
            VerificationStrategy::Compilation,
            ResourceEstimate::default(),
        )],
    );

    // Stale authorization: plan is revision 0, auth was issued for revision 2
    let auth = ExecutionAuthorization::new("session_stale", 2, 1, "operator");
    let result = materializer
        .materialize_authorized(mission_id, &plan, 2, 1, &auth)
        .await;

    assert!(
        result.is_err(),
        "Must reject authorization when plan.revision != authorized revision"
    );
    let err_str = result.err().unwrap().to_string();
    assert!(err_str.contains("Candidate plan revision does not match authorized"));
}

// ---------------------------------------------------------------------------
// 8. Direct Materializer Bypass Defense (PRD §17)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_invalidated_authorization_fails_materialization() {
    let (_dir, pool, _bus) = setup_test_env().await;
    let materializer = TaskGraphMaterializer::new(pool.clone());
    let mission_id = MissionId::new();

    let mut plan = CandidatePlan::new(
        "plan-1",
        "Test Invalidation",
        vec![CandidateTask::new(
            CandidateTaskKey::new("TASK-01"),
            "Execute Task",
            AgentRole::implementer(),
            VerificationStrategy::Compilation,
            ResourceEstimate::default(),
        )],
    );
    plan.revision = 1;

    let mut auth = ExecutionAuthorization::new("session_inv", 1, 1, "operator");
    auth.invalidate("Plan modified by upstream user");

    let result = materializer
        .materialize_authorized(mission_id, &plan, 1, 1, &auth)
        .await;

    assert!(
        result.is_err(),
        "Invalidated authorization must fail closed"
    );
    let err_str = result.err().unwrap().to_string();
    assert!(err_str.contains("authorization is invalid"));
}
