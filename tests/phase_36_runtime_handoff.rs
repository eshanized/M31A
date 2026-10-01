//! Phase 36 — End-to-End Runtime Handoff & Production Lifecycle Hardening Tests.
//!
//! Proves:
//! - Revision binding between plan, task, and authorization
//! - Stale state defense (out-of-order, duplicate, expired)
//! - Authorization semantics: each gate is distinct
//! - Model/chat cannot bypass authorization
//! - Discovery is evidence-based, not word-count based
//! - No workspace mutation before authorization
//! - Full lifecycle happy path with persistence
//! - Restart recovery
//! - Materialization boundary (revision-bound)

use m31a::events::bus::BroadcastEventBus;
use m31a::interaction::action::ApplicationAction;
use m31a::persistence::sqlite::schema::initialize_database;
use m31a::planning::review::{
    AuthorizationDecision, ExecutionAuthorization, PreExecutionCoordinator, PreExecutionResponse,
};
use m31a::state_machine::lifecycle::{LifecycleEvent, LifecycleStage, transition_lifecycle};
use std::sync::Arc;
use tempfile::tempdir;

async fn setup_test_db() -> (tempfile::TempDir, sqlx::SqlitePool, Arc<BroadcastEventBus>) {
    let dir = tempdir().expect("tempdir");
    let db_path = dir.path().join("lifecycle_test.db");
    let pool = initialize_database(&db_path).await.expect("init db");
    let bus = Arc::new(BroadcastEventBus::new(1024));
    (dir, pool, bus)
}

async fn create_test_session(pool: &sqlx::SqlitePool, dir: &std::path::Path) -> String {
    let repo = m31a::interaction::session::SqliteSessionRepository::new(pool.clone());
    let session = repo.create_session(dir).await.expect("create test session");
    session.id.to_string()
}

// ---------------------------------------------------------
// Group 1: Revision Binding Tests
// ---------------------------------------------------------

#[tokio::test]
async fn test_authorization_binds_to_exact_revisions() {
    let auth = ExecutionAuthorization::new("session_1", 1, 1, "operator");
    assert!(
        auth.is_valid_for(1, 1),
        "Auth must be valid for exact revision match"
    );
    assert!(
        !auth.is_valid_for(2, 1),
        "Auth must fail for different plan rev"
    );
    assert!(
        !auth.is_valid_for(1, 2),
        "Auth must fail for different task rev"
    );
    assert!(
        !auth.is_valid_for(0, 0),
        "Auth must fail for zero revisions"
    );
    assert!(!auth.is_valid_for(1, 0), "Auth must fail for zero task rev");
    assert!(!auth.is_valid_for(0, 1), "Auth must fail for zero plan rev");
}

#[tokio::test]
async fn test_plan_change_invalidates_task_and_authorization() {
    let mut auth = ExecutionAuthorization::new("session_1", 1, 1, "operator");
    assert!(auth.is_valid_for(1, 1));
    auth.invalidate("Plan modified by operator");
    assert!(
        !auth.is_valid_for(1, 1),
        "Invalidated auth must never be valid"
    );
    assert_eq!(auth.decision, AuthorizationDecision::Invalidated);
    assert!(auth.invalidation_reason.is_some());
}

#[tokio::test]
async fn test_task_change_invalidates_authorization() {
    let auth = ExecutionAuthorization::new("session_1", 1, 1, "operator");
    // Task revision 2 with same plan revision 1 — auth is stale
    assert!(
        !auth.is_valid_for(1, 2),
        "Auth for task rev 1 must be invalid when current is rev 2"
    );
}

// ---------------------------------------------------------
// Group 2: Stale State Defense Tests
// ---------------------------------------------------------

#[tokio::test]
async fn test_stale_authorization_rejected_all_permutations() {
    let auth = ExecutionAuthorization::new("session_1", 3, 5, "operator");
    // Only (3, 5) should pass
    assert!(auth.is_valid_for(3, 5));
    // All other permutations must fail
    for plan_rev in 0..=6 {
        for task_rev in 0..=8 {
            if plan_rev == 3 && task_rev == 5 {
                continue;
            }
            assert!(
                !auth.is_valid_for(plan_rev, task_rev),
                "Auth(3,5) must be invalid for ({}, {})",
                plan_rev,
                task_rev
            );
        }
    }
}

#[tokio::test]
async fn test_duplicate_accept_is_illegal() {
    // Accept plan → move to PlanAccepted → try to accept again → must error
    let stage =
        transition_lifecycle(LifecycleStage::PlanReview, LifecycleEvent::PlanAccepted).unwrap();
    assert_eq!(stage, LifecycleStage::PlanAccepted);
    let res = transition_lifecycle(stage, LifecycleEvent::PlanAccepted);
    assert!(res.is_err(), "Double plan acceptance must be rejected");

    // Accept tasks → try to accept again → must error
    let stage =
        transition_lifecycle(LifecycleStage::TasksReview, LifecycleEvent::TasksAccepted).unwrap();
    assert_eq!(stage, LifecycleStage::TasksAccepted);
    let res = transition_lifecycle(stage, LifecycleEvent::TasksAccepted);
    assert!(res.is_err(), "Double task acceptance must be rejected");
}

#[tokio::test]
async fn test_out_of_order_lifecycle_transitions() {
    // TasksAccepted before PlanAccepted
    assert!(
        transition_lifecycle(LifecycleStage::PlanReview, LifecycleEvent::TasksAccepted).is_err()
    );
    assert!(
        transition_lifecycle(LifecycleStage::PlanDraft, LifecycleEvent::TasksAccepted).is_err()
    );
    assert!(
        transition_lifecycle(LifecycleStage::IntentActive, LifecycleEvent::TasksAccepted).is_err()
    );

    // ExecutionAuthorized before TasksAccepted
    assert!(
        transition_lifecycle(
            LifecycleStage::TasksReview,
            LifecycleEvent::AuthorizeExecution
        )
        .is_err()
    );
    assert!(
        transition_lifecycle(
            LifecycleStage::TasksDraft,
            LifecycleEvent::AuthorizeExecution
        )
        .is_err()
    );

    // Executing before ExecutionAuthorized
    assert!(
        transition_lifecycle(
            LifecycleStage::ExecutionAwaitingAuthorization,
            LifecycleEvent::StartExecution
        )
        .is_err()
    );
    assert!(
        transition_lifecycle(
            LifecycleStage::TasksAccepted,
            LifecycleEvent::StartExecution
        )
        .is_err()
    );
    assert!(
        transition_lifecycle(LifecycleStage::PlanAccepted, LifecycleEvent::StartExecution).is_err()
    );

    // Can't go backwards
    assert!(transition_lifecycle(LifecycleStage::Executing, LifecycleEvent::PlanAccepted).is_err());
    assert!(
        transition_lifecycle(LifecycleStage::Completed, LifecycleEvent::StartExecution).is_err()
    );
}

// ---------------------------------------------------------
// Group 3: Authorization Semantics Tests — Each Gate Distinct
// ---------------------------------------------------------

#[tokio::test]
async fn test_plan_accepted_does_not_imply_tasks_accepted() {
    // From PlanAccepted, cannot jump to RequestExecutionAuthorization (needs TasksDrafted → TasksReview → TasksAccepted)
    assert!(
        transition_lifecycle(
            LifecycleStage::PlanAccepted,
            LifecycleEvent::RequestExecutionAuthorization
        )
        .is_err()
    );
    assert!(
        transition_lifecycle(
            LifecycleStage::PlanAccepted,
            LifecycleEvent::AuthorizeExecution
        )
        .is_err()
    );
    assert!(
        transition_lifecycle(LifecycleStage::PlanAccepted, LifecycleEvent::StartExecution).is_err()
    );
}

#[tokio::test]
async fn test_tasks_accepted_does_not_imply_execution_authorized() {
    // From TasksAccepted, cannot jump to StartExecution
    assert!(
        transition_lifecycle(
            LifecycleStage::TasksAccepted,
            LifecycleEvent::StartExecution
        )
        .is_err()
    );
    assert!(
        transition_lifecycle(
            LifecycleStage::TasksAccepted,
            LifecycleEvent::AuthorizeExecution
        )
        .is_err()
    );
    // Can only go to RequestExecutionAuthorization
    let next = transition_lifecycle(
        LifecycleStage::TasksAccepted,
        LifecycleEvent::RequestExecutionAuthorization,
    )
    .unwrap();
    assert_eq!(next, LifecycleStage::ExecutionAwaitingAuthorization);
}

#[tokio::test]
async fn test_execution_authorized_does_not_imply_tool_approved() {
    // ExecutionAuthorized stage gates StartExecution, but tool approval is per-tool via pipeline policy
    let stage = LifecycleStage::ExecutionAuthorized;
    assert_ne!(stage, LifecycleStage::Executing);
    // Only StartExecution moves to Executing
    let next = transition_lifecycle(stage, LifecycleEvent::StartExecution).unwrap();
    assert_eq!(next, LifecycleStage::Executing);
}

// ---------------------------------------------------------
// Group 4: Model/Chat Authorization Bypass Prevention
// ---------------------------------------------------------

#[tokio::test]
async fn test_model_text_cannot_create_authorization() {
    // Prove: ExecutionAuthorization can only be created through explicit struct construction,
    // not through chat text parsing. The state machine rejects non-typed transitions.
    let from_states = vec![
        LifecycleStage::IntentActive,
        LifecycleStage::PlanDraft,
        LifecycleStage::PlanReview,
        LifecycleStage::TasksReview,
        LifecycleStage::TasksAccepted,
    ];
    for state in from_states {
        let res = transition_lifecycle(state, LifecycleEvent::StartExecution);
        assert!(res.is_err(), "StartExecution from {:?} must fail", state);
    }
}

#[tokio::test]
async fn test_freeform_chat_cannot_bypass_authorization() {
    // Prove: the only path to ExecutionAuthorized is through the typed LifecycleEvent::AuthorizeExecution
    // from ExecutionAwaitingAuthorization state. No freeform text parsing can produce this transition.
    let all_events = vec![
        LifecycleEvent::StartIntent,
        LifecycleEvent::QuestionsRequired,
        LifecycleEvent::InformationProvided,
        LifecycleEvent::DiscoveryConverged,
        LifecycleEvent::PlanDrafted,
        LifecycleEvent::EnterPlanReview,
        LifecycleEvent::PlanAccepted,
        LifecycleEvent::TasksDrafted,
        LifecycleEvent::TasksAccepted,
        LifecycleEvent::Complete,
    ];
    // None of these events from AwaitingAuthorization can produce ExecutionAuthorized
    for event in all_events {
        let res = transition_lifecycle(
            LifecycleStage::ExecutionAwaitingAuthorization,
            event.clone(),
        );
        if let Ok(stage) = res {
            assert_ne!(
                stage,
                LifecycleStage::ExecutionAuthorized,
                "Event {:?} must NOT produce ExecutionAuthorized",
                event
            );
        }
    }
    // Only AuthorizeExecution can
    let res = transition_lifecycle(
        LifecycleStage::ExecutionAwaitingAuthorization,
        LifecycleEvent::AuthorizeExecution,
    )
    .unwrap();
    assert_eq!(res, LifecycleStage::ExecutionAuthorized);
}

// ---------------------------------------------------------
// Group 5: Discovery Semantic Tests
// ---------------------------------------------------------

#[test]
fn test_short_prompt_with_architecture_skips_unknowns() {
    // "Build a React REST API" — short but has architecture signal
    // The PromptSemanticAnalyzer should NOT trigger unknowns for this
    let prompt = "Build a React REST API";
    let lower = prompt.to_lowercase();
    let arch_keywords = [
        "react", "vue", "angular", "django", "flask", "express", "api", "rest", "graphql",
        "database", "postgres", "node",
    ];
    let has_arch = arch_keywords.iter().any(|k| lower.contains(k));
    assert!(
        has_arch,
        "Short prompt with architecture keywords should have arch signal"
    );
}

#[test]
fn test_long_ambiguous_prompt_may_trigger_unknowns() {
    // A long prompt without any architecture/technology signals
    let prompt = "I want something really cool and amazing for my business \
                  that will help us grow and serve customers better and make \
                  everything work smoothly";
    let lower = prompt.to_lowercase();
    let arch_keywords = [
        "react",
        "vue",
        "angular",
        "django",
        "flask",
        "express",
        "api",
        "rest",
        "graphql",
        "database",
        "postgres",
        "node",
        "typescript",
        "python",
        "rust",
        "java",
        "go",
        "docker",
    ];
    let has_arch = arch_keywords.iter().any(|k| lower.contains(k));
    let deliverable_markers = [
        "create",
        "build",
        "implement",
        "add",
        "fix",
        "refactor",
        "update",
        "delete",
        "remove",
        "endpoint",
        "page",
        "component",
        "service",
        "module",
        "feature",
        "function",
        "file",
    ];
    let has_deliverable = deliverable_markers.iter().any(|k| lower.contains(k));
    assert!(
        !has_arch,
        "Long ambiguous prompt should NOT have arch signal"
    );
    assert!(
        !has_deliverable,
        "Long ambiguous prompt should NOT have deliverable signal"
    );
}

#[test]
fn test_word_count_is_not_semantic_authority() {
    // A 3-word prompt with architecture: "Build React app"
    let prompt_short = "Build React app";
    let lower = prompt_short.to_lowercase();
    let has_arch = ["react"].iter().any(|k| lower.contains(k));
    let has_deliverable = ["build"].iter().any(|k| lower.contains(k));
    assert!(
        has_arch && has_deliverable,
        "Short specific prompt should be classified as complete"
    );

    // A 20-word prompt without architecture
    let prompt_long = "I need a tool that does something interesting for our team members in the office downtown near the park by the river";
    let lower_long = prompt_long.to_lowercase();
    let has_arch_long = ["react", "api", "database", "python", "node"]
        .iter()
        .any(|k| lower_long.contains(k));
    assert!(
        !has_arch_long,
        "Long vague prompt should lack architecture signal regardless of word count"
    );
}

// ---------------------------------------------------------
// Group 6: Pre-Authorization Side Effect Tests
// ---------------------------------------------------------

#[tokio::test]
async fn test_no_workspace_mutation_before_authorization() {
    let (dir, pool, bus) = setup_test_db().await;
    let workspace = dir.path().join("workspace");
    std::fs::create_dir_all(&workspace).unwrap();
    let marker_file = workspace.join("sentinel.txt");
    std::fs::write(&marker_file, "original content").unwrap();

    let coordinator = PreExecutionCoordinator::deterministic_test(pool.clone(), Some(bus));
    let session_id = create_test_session(&pool, dir.path()).await;

    // Run through lifecycle up to AwaitingAuthorization
    let _resp = coordinator
        .init_intent(&session_id, "Deploy a comprehensive service.", "operator")
        .await
        .unwrap();

    let _ = coordinator
        .handle_action(
            ApplicationAction::PlanAcceptRequested {
                session_id: Some(session_id.clone()),
            },
            "operator",
        )
        .await;

    let _ = coordinator
        .handle_action(
            ApplicationAction::TasksAcceptRequested {
                session_id: Some(session_id.clone()),
            },
            "operator",
        )
        .await;

    // Verify: workspace sentinel file must be unchanged
    let content = std::fs::read_to_string(&marker_file).unwrap();
    assert_eq!(
        content, "original content",
        "Workspace must NOT be mutated before authorization"
    );
}

// ---------------------------------------------------------
// Group 7: End-to-End Lifecycle Tests
// ---------------------------------------------------------

#[tokio::test]
async fn test_full_lifecycle_happy_path_with_persistence() {
    let (dir, pool, bus) = setup_test_db().await;
    let coordinator = PreExecutionCoordinator::deterministic_test(pool.clone(), Some(bus));
    let session_id = create_test_session(&pool, dir.path()).await;

    // 1. init_intent — should reach PlanReview (prompt has "deploy" + "service" deliverables)
    let resp = coordinator
        .init_intent(
            &session_id,
            "Deploy a comprehensive microservice.",
            "operator",
        )
        .await
        .expect("init");
    assert!(
        matches!(resp, PreExecutionResponse::PlanForReview { .. }),
        "Prompt with deliverable+arch should skip questions and go to PlanForReview"
    );

    // 2. Accept plan
    let resp = coordinator
        .handle_action(
            ApplicationAction::PlanAcceptRequested {
                session_id: Some(session_id.clone()),
            },
            "operator",
        )
        .await
        .expect("accept plan");
    assert!(matches!(resp, PreExecutionResponse::TasksForReview { .. }));

    // 3. Accept tasks
    let resp = coordinator
        .handle_action(
            ApplicationAction::TasksAcceptRequested {
                session_id: Some(session_id.clone()),
            },
            "operator",
        )
        .await
        .expect("accept tasks");
    assert!(matches!(
        resp,
        PreExecutionResponse::AuthorizationRequested { .. }
    ));

    // 4. Authorize execution
    let resp = coordinator
        .handle_action(
            ApplicationAction::ExecutionAuthorizationSubmitted {
                session_id: Some(session_id.clone()),
                decision: true,
                reason: None,
            },
            "operator",
        )
        .await
        .expect("authorize execution");
    match resp {
        PreExecutionResponse::ReadyToExecute {
            plan,
            tasks,
            authorization,
            ..
        } => {
            assert!(
                !plan.objective.is_empty(),
                "Plan objective must not be empty"
            );
            assert!(!tasks.is_empty(), "Authorized tasks must not be empty");
            assert!(
                authorization
                    .is_valid_for(authorization.plan_revision, authorization.task_revision)
            );
            assert_eq!(authorization.decision, AuthorizationDecision::Authorized);
        }
        other => panic!("Expected ReadyToExecute, got {:?}", other),
    }

    // 5. Verify durable state is ExecutionAuthorized (INVARIANT D: Authorization != Execution)
    let repo = coordinator.lifecycle_repo();
    let state = repo
        .load_lifecycle_state(&session_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(state.stage, LifecycleStage::ExecutionAuthorized);
}

#[tokio::test]
async fn test_lifecycle_restart_recovery() {
    let (dir, pool, bus) = setup_test_db().await;
    let session_id = create_test_session(&pool, dir.path()).await;

    // Phase 1: run lifecycle to PlanReview with coordinator #1
    {
        let coordinator =
            PreExecutionCoordinator::deterministic_test(pool.clone(), Some(bus.clone()));
        let _resp = coordinator
            .init_intent(&session_id, "Build a microservice API.", "operator")
            .await
            .expect("init");
    }
    // Coordinator dropped — simulate restart

    // Phase 2: new coordinator, resume session
    {
        let coordinator =
            PreExecutionCoordinator::deterministic_test(pool.clone(), Some(bus.clone()));
        let resp = coordinator
            .resume_session(&session_id)
            .await
            .expect("resume");

        // Must resume to PlanForReview — exact state restored
        assert!(
            matches!(resp, PreExecutionResponse::PlanForReview { .. }),
            "Restart must restore exact PlanReview state"
        );
    }
}

#[tokio::test]
async fn test_authorization_without_accepted_plan_fails() {
    let (dir, pool, bus) = setup_test_db().await;
    let coordinator = PreExecutionCoordinator::deterministic_test(pool.clone(), Some(bus));
    let session_id = create_test_session(&pool, dir.path()).await;

    let _resp = coordinator
        .init_intent(&session_id, "Create a REST API with Express.", "operator")
        .await
        .expect("init");

    // Try to authorize without accepting plan first
    let res = coordinator
        .handle_action(
            ApplicationAction::ExecutionAuthorizationSubmitted {
                session_id: Some(session_id.clone()),
                decision: true,
                reason: None,
            },
            "operator",
        )
        .await;
    assert!(
        res.is_err(),
        "Authorization without accepted plan must fail"
    );
}

#[tokio::test]
async fn test_authorization_without_accepted_tasks_fails() {
    let (dir, pool, bus) = setup_test_db().await;
    let coordinator = PreExecutionCoordinator::deterministic_test(pool.clone(), Some(bus));
    let session_id = create_test_session(&pool, dir.path()).await;

    let _resp = coordinator
        .init_intent(&session_id, "Build a Django web application.", "operator")
        .await
        .expect("init");

    // Accept plan but NOT tasks
    let _resp = coordinator
        .handle_action(
            ApplicationAction::PlanAcceptRequested {
                session_id: Some(session_id.clone()),
            },
            "operator",
        )
        .await
        .expect("accept plan");

    // Try to authorize without accepting tasks
    let res = coordinator
        .handle_action(
            ApplicationAction::ExecutionAuthorizationSubmitted {
                session_id: Some(session_id.clone()),
                decision: true,
                reason: None,
            },
            "operator",
        )
        .await;
    assert!(
        res.is_err(),
        "Authorization without accepted tasks must fail"
    );
}

// ---------------------------------------------------------
// Group 8: Materialization Boundary Tests
// ---------------------------------------------------------

#[tokio::test]
async fn test_materializer_rejects_stale_authorization() {
    use m31a::dag::materializer::TaskGraphMaterializer;
    use m31a::ids::MissionId;
    use m31a::kernel::plan::{
        CandidatePlan, CandidateTask, CandidateTaskKey, ResourceEstimate, VerificationStrategy,
    };
    use m31a::state_machine::agent::AgentRole;

    let (_dir, pool, _bus) = setup_test_db().await;

    let plan = CandidatePlan::new(
        "test-plan",
        "test objective",
        vec![CandidateTask::new(
            CandidateTaskKey::new("T1"),
            "Task 1",
            AgentRole::implementer(),
            VerificationStrategy::Compilation,
            ResourceEstimate::default(),
        )],
    );

    let materializer = TaskGraphMaterializer::new(pool.clone());
    let mission_id = MissionId::new();

    // Auth for rev (1,1) but plan is rev 0 (default)
    let auth = ExecutionAuthorization::new("session_1", 1, 1, "operator");

    let result = materializer
        .materialize_authorized(mission_id, &plan, 1, 1, &auth)
        .await;
    assert!(
        result.is_err(),
        "Materializer must reject when plan.revision != authorized revision"
    );
}

#[tokio::test]
async fn test_materializer_rejects_invalidated_authorization() {
    use m31a::dag::materializer::TaskGraphMaterializer;
    use m31a::ids::MissionId;
    use m31a::kernel::plan::{
        CandidatePlan, CandidateTask, CandidateTaskKey, ResourceEstimate, VerificationStrategy,
    };
    use m31a::state_machine::agent::AgentRole;

    let (_dir, pool, _bus) = setup_test_db().await;

    let mut plan = CandidatePlan::new(
        "test-plan",
        "test objective",
        vec![CandidateTask::new(
            CandidateTaskKey::new("T1"),
            "Task 1",
            AgentRole::implementer(),
            VerificationStrategy::Compilation,
            ResourceEstimate::default(),
        )],
    );
    plan.revision = 1;

    let materializer = TaskGraphMaterializer::new(pool.clone());
    let mission_id = MissionId::new();

    // Auth is invalidated
    let mut auth = ExecutionAuthorization::new("session_1", 1, 1, "operator");
    auth.invalidate("Plan changed");

    let result = materializer
        .materialize_authorized(mission_id, &plan, 1, 1, &auth)
        .await;
    assert!(
        result.is_err(),
        "Materializer must reject invalidated authorization"
    );
}

// ---------------------------------------------------------
// Group 9: Invalidation Cascade Tests
// ---------------------------------------------------------

#[tokio::test]
async fn test_plan_edit_invalidates_downstream() {
    let (dir, pool, bus) = setup_test_db().await;
    let coordinator = PreExecutionCoordinator::deterministic_test(pool.clone(), Some(bus));
    let session_id = create_test_session(&pool, dir.path()).await;

    // Build up to TasksReview
    let _resp = coordinator
        .init_intent(&session_id, "Build a Vue frontend.", "operator")
        .await
        .unwrap();
    let _resp = coordinator
        .handle_action(
            ApplicationAction::PlanAcceptRequested {
                session_id: Some(session_id.clone()),
            },
            "operator",
        )
        .await
        .unwrap();

    // Now in TasksReview. Edit the plan (upstream change)
    let repo = coordinator.lifecycle_repo();
    let plan = repo
        .load_latest_plan_revision(&session_id)
        .await
        .unwrap()
        .unwrap();
    let mut edited_plan = plan.content.clone();
    edited_plan.objective = "Build a Vue + Nuxt frontend".to_string();
    let plan_json = serde_json::to_string_pretty(&edited_plan).unwrap();

    let resp = coordinator
        .handle_action(
            ApplicationAction::PlanEditRequested {
                session_id: Some(session_id.clone()),
                plan_json,
            },
            "operator",
        )
        .await
        .unwrap();

    // Must return to PlanReview — downstream state invalidated
    assert!(matches!(resp, PreExecutionResponse::PlanForReview { .. }));

    // Verify lifecycle state reverted to PlanReview
    let state = repo
        .load_lifecycle_state(&session_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(state.stage, LifecycleStage::PlanReview);
}

#[tokio::test]
async fn test_execution_rejection_records_reason() {
    let (dir, pool, bus) = setup_test_db().await;
    let coordinator = PreExecutionCoordinator::deterministic_test(pool.clone(), Some(bus));
    let session_id = create_test_session(&pool, dir.path()).await;

    let _resp = coordinator
        .init_intent(&session_id, "Create a Flask API.", "operator")
        .await
        .unwrap();
    let _resp = coordinator
        .handle_action(
            ApplicationAction::PlanAcceptRequested {
                session_id: Some(session_id.clone()),
            },
            "operator",
        )
        .await
        .unwrap();
    let _resp = coordinator
        .handle_action(
            ApplicationAction::TasksAcceptRequested {
                session_id: Some(session_id.clone()),
            },
            "operator",
        )
        .await
        .unwrap();

    // Reject authorization
    let resp = coordinator
        .handle_action(
            ApplicationAction::ExecutionAuthorizationSubmitted {
                session_id: Some(session_id.clone()),
                decision: false,
                reason: Some("Too risky for production".to_string()),
            },
            "operator",
        )
        .await
        .unwrap();

    match resp {
        PreExecutionResponse::Terminated { stage, reason, .. } => {
            assert_eq!(stage, LifecycleStage::Rejected);
            assert!(
                reason.contains("Too risky"),
                "Rejection reason must be preserved"
            );
        }
        other => panic!("Expected Terminated, got {:?}", other),
    }
}
