//! Integration and adversarial test suite for Phase 35: Human-Governed Planning & Pre-Execution Lifecycle.
//!
//! Enforces:
//! - "The model proposes. The runtime decides."
//! - Three distinct authorization gates (Plan Acceptance, Task Acceptance, Execution Authorization)
//! - Strong typed state machine invariants and terminal outcomes
//! - Uncertainty-driven dynamic question generation and answer provenance
//! - Machine-readable plan/task editing, cycle detection, and dependent validation
//! - Execution authorization strictly bound to exact plan and task revisions
//! - Atomic invalidation of downstream approvals on upstream changes
//! - Crash/restart recovery preserving lifecycle position

use m31a::events::bus::BroadcastEventBus;
use m31a::interaction::action::ApplicationAction;
use m31a::kernel::plan::{CandidateTask, CandidateTaskKey, ResourceEstimate};
use m31a::persistence::sqlite::schema::initialize_database;
use m31a::planning::review::{
    AuthorizationDecision, PreExecutionCoordinator, PreExecutionResponse, RevisionAuthorType,
    TaskGraphValidationError, validate_candidate_tasks,
};
use m31a::state_machine::agent::AgentRole;
use m31a::state_machine::error::TransitionError;
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

fn sample_task(id: &str, deps: Vec<&str>) -> CandidateTask {
    let mut t = CandidateTask::new(
        id,
        format!("Task {}", id),
        AgentRole::implementer(),
        m31a::kernel::plan::VerificationStrategy::Compilation,
        ResourceEstimate::default(),
    );
    t.depends_on = deps.into_iter().map(CandidateTaskKey::new).collect();
    t
}

// ─────────────────────────────────────────────────────────────────────────────
// 1. State Machine Invariant Tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_state_machine_happy_path_transitions() {
    let mut stage = LifecycleStage::IntentActive;

    // Unknowns detected -> QuestionsRequired -> AwaitingInformation
    stage = transition_lifecycle(stage, LifecycleEvent::QuestionsRequired).unwrap();
    assert_eq!(stage, LifecycleStage::AwaitingInformation);

    // Questions answered / discovery converged -> PlanDraft
    stage = transition_lifecycle(stage, LifecycleEvent::DiscoveryConverged).unwrap();
    assert_eq!(stage, LifecycleStage::PlanDraft);

    // Enter plan review gate -> PlanReview
    stage = transition_lifecycle(stage, LifecycleEvent::EnterPlanReview).unwrap();
    assert_eq!(stage, LifecycleStage::PlanReview);

    // Plan accepted -> PlanAccepted
    stage = transition_lifecycle(stage, LifecycleEvent::PlanAccepted).unwrap();
    assert_eq!(stage, LifecycleStage::PlanAccepted);

    // Tasks drafted -> TasksDraft -> TasksReview
    stage = transition_lifecycle(stage, LifecycleEvent::TasksDrafted).unwrap();
    assert_eq!(stage, LifecycleStage::TasksDraft);
    stage = transition_lifecycle(stage, LifecycleEvent::EnterTasksReview).unwrap();
    assert_eq!(stage, LifecycleStage::TasksReview);

    // Tasks accepted -> TasksAccepted
    stage = transition_lifecycle(stage, LifecycleEvent::TasksAccepted).unwrap();
    assert_eq!(stage, LifecycleStage::TasksAccepted);

    // Execution launch requested -> ExecutionAwaitingAuthorization
    stage = transition_lifecycle(stage, LifecycleEvent::RequestExecutionAuthorization).unwrap();
    assert_eq!(stage, LifecycleStage::ExecutionAwaitingAuthorization);

    // Explicit operator authorization -> ExecutionAuthorized
    stage = transition_lifecycle(stage, LifecycleEvent::AuthorizeExecution).unwrap();
    assert_eq!(stage, LifecycleStage::ExecutionAuthorized);

    // Execution starts -> Executing
    stage = transition_lifecycle(stage, LifecycleEvent::StartExecution).unwrap();
    assert_eq!(stage, LifecycleStage::Executing);

    // Execution completes -> Completed
    stage = transition_lifecycle(stage, LifecycleEvent::Complete).unwrap();
    assert_eq!(stage, LifecycleStage::Completed);
}

#[test]
fn test_state_machine_blocks_illegal_bypass_attempts() {
    // Model cannot jump directly from PlanReview to Executing
    let err = transition_lifecycle(LifecycleStage::PlanReview, LifecycleEvent::StartExecution)
        .unwrap_err();
    assert!(matches!(err, TransitionError::InvalidTransition { .. }));

    // Cannot authorize execution directly from PlanAccepted without Task Review & Acceptance
    let err = transition_lifecycle(
        LifecycleStage::PlanAccepted,
        LifecycleEvent::AuthorizeExecution,
    )
    .unwrap_err();
    assert!(matches!(err, TransitionError::InvalidTransition { .. }));

    // Cannot start execution from TasksAccepted without explicit operator authorization
    let err = transition_lifecycle(
        LifecycleStage::TasksAccepted,
        LifecycleEvent::StartExecution,
    )
    .unwrap_err();
    assert!(matches!(err, TransitionError::InvalidTransition { .. }));

    // Cannot start execution from ExecutionAwaitingAuthorization without authorization
    let err = transition_lifecycle(
        LifecycleStage::ExecutionAwaitingAuthorization,
        LifecycleEvent::StartExecution,
    )
    .unwrap_err();
    assert!(matches!(err, TransitionError::InvalidTransition { .. }));
}

#[test]
fn test_state_machine_upstream_invalidation_cascades() {
    // Invalidate from TasksReview to PlanReview
    let stage = transition_lifecycle(
        LifecycleStage::TasksReview,
        LifecycleEvent::InvalidateToPlanReview,
    )
    .unwrap();
    assert_eq!(stage, LifecycleStage::PlanReview);

    // Invalidate from ExecutionAwaitingAuthorization to TasksReview
    let stage = transition_lifecycle(
        LifecycleStage::ExecutionAwaitingAuthorization,
        LifecycleEvent::InvalidateToTasksReview,
    )
    .unwrap();
    assert_eq!(stage, LifecycleStage::TasksReview);

    // Rejection results in terminal state
    let stage =
        transition_lifecycle(LifecycleStage::PlanReview, LifecycleEvent::PlanRejected).unwrap();
    assert_eq!(stage, LifecycleStage::Rejected);
}

// ─────────────────────────────────────────────────────────────────────────────
// 2. Dynamic Discovery & Answer Provenance
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_minimal_intent_enters_discovery_and_records_provenance() {
    let (dir, pool, bus) = setup_test_db().await;
    let coordinator = PreExecutionCoordinator::deterministic_test(pool.clone(), Some(bus.clone()))
        .with_workspace_root(dir.path().to_path_buf());
    let session_id = create_test_session(&pool, dir.path()).await;

    // 1. Submit minimal prompt ("Build a web app")
    let resp = coordinator
        .init_intent(&session_id, "Build a web app", "test_user")
        .await
        .expect("init intent");

    // 2. Expect dynamic questions required
    match resp {
        PreExecutionResponse::QuestionsRequired {
            session_id: sid,
            questions,
        } => {
            assert_eq!(sid, session_id);
            assert!(
                !questions.is_empty(),
                "questions should be generated for minimal prompt"
            );

            let first_q = &questions[0];
            assert!(!first_q.question_id.is_empty());
            assert!(!first_q.text.is_empty());

            // 3. Submit operator answer to the question
            let answer_resp = coordinator
                .submit_answer(
                    &session_id,
                    &first_q.question_id,
                    "Use React frontend with SQLite database",
                    "operator_bob",
                )
                .await
                .expect("submit answer");

            // 4. Verify question recorded in SQLite repository with answered status
            let repo = coordinator.lifecycle_repo();
            let questions_in_db = repo
                .load_discovery_questions(&session_id)
                .await
                .expect("load questions");
            let db_q = questions_in_db
                .iter()
                .find(|q| q.question_id == first_q.question_id)
                .expect("found answered question");
            assert_eq!(db_q.status, "answered");
            assert_eq!(
                db_q.answer.as_deref(),
                Some("Use React frontend with SQLite database")
            );
            assert_eq!(db_q.answered_by.as_deref(), Some("operator_bob"));

            // 5. Answer remaining questions if any until discovery converges
            let mut current_resp = answer_resp;
            while let PreExecutionResponse::QuestionsRequired {
                questions: ref remaining,
                ..
            } = current_resp
            {
                if let Some(next_q) = remaining
                    .iter()
                    .find(|q| q.question_id != first_q.question_id)
                {
                    let next_qid = next_q.question_id.clone();
                    current_resp = coordinator
                        .submit_answer(
                            &session_id,
                            &next_qid,
                            "Standard single-tenant architecture",
                            "operator_bob",
                        )
                        .await
                        .expect("submit remaining answer");
                } else {
                    break;
                }
            }

            // 6. Once questions answered, coordinator produces PlanForReview
            match current_resp {
                PreExecutionResponse::PlanForReview { revision, .. } => {
                    assert_eq!(revision.revision, 1);
                    assert_eq!(revision.author_type, RevisionAuthorType::Model);
                }
                other => panic!("expected PlanForReview, got {:?}", other),
            }
        }
        other => panic!("expected QuestionsRequired, got {:?}", other),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 3. Plan Review: Manual Edit, Model Revision, and Acceptance Gates
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_plan_review_manual_edit_and_validation() {
    let (dir, pool, bus) = setup_test_db().await;
    let coordinator = PreExecutionCoordinator::deterministic_test(pool.clone(), Some(bus.clone()))
        .with_workspace_root(dir.path().to_path_buf());
    let session_id = create_test_session(&pool, dir.path()).await;

    // Initialize with a fully-specified prompt so questions are bypassed directly into PlanReview
    let full_prompt = "Build a comprehensive authenticated payment gateway with Stripe integration, webhook handling, and audit logging.";
    let resp = coordinator
        .init_intent(&session_id, full_prompt, "operator")
        .await
        .expect("init intent");

    let initial_plan_rev = match resp {
        PreExecutionResponse::PlanForReview { revision, .. } => revision,
        PreExecutionResponse::QuestionsRequired { questions, .. } => {
            // Answer all to get to PlanForReview
            let mut last_resp = PreExecutionResponse::QuestionsRequired {
                session_id: session_id.clone(),
                questions: questions.clone(),
            };
            for q in questions {
                last_resp = coordinator
                    .submit_answer(
                        &session_id,
                        &q.question_id,
                        "Confirmed requirement",
                        "operator",
                    )
                    .await
                    .expect("submit");
            }
            match last_resp {
                PreExecutionResponse::PlanForReview { revision, .. } => revision,
                other => panic!("expected PlanForReview, got {:?}", other),
            }
        }
        other => panic!("expected PlanForReview, got {:?}", other),
    };

    assert_eq!(initial_plan_rev.revision, 1);

    // A. Invalid manual edit (malformed JSON) fails closed
    let bad_json_action = ApplicationAction::PlanEditRequested {
        session_id: Some(session_id.clone()),
        plan_json: "{ malformed json ...".to_string(),
    };
    let err = coordinator
        .handle_action(bad_json_action, "operator")
        .await
        .unwrap_err();
    assert!(err.contains("Invalid plan JSON schema"));

    // Previous valid plan remains authoritative
    let repo = coordinator.lifecycle_repo();
    let current = repo
        .load_latest_plan_revision(&session_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(current.revision, 1);

    // B. Valid manual edit increments revision and records author
    let mut modified_plan = initial_plan_rev.content.clone();
    modified_plan.objective = "Updated Payment Gateway Objective with Idempotency Keys".to_string();
    let valid_json = serde_json::to_string_pretty(&modified_plan).unwrap();

    let edit_action = ApplicationAction::PlanEditRequested {
        session_id: Some(session_id.clone()),
        plan_json: valid_json,
    };
    let resp = coordinator
        .handle_action(edit_action, "operator_alice")
        .await
        .expect("valid edit");

    match resp {
        PreExecutionResponse::PlanForReview { revision, .. } => {
            assert_eq!(revision.revision, 2);
            assert_eq!(revision.author_type, RevisionAuthorType::User);
            assert_eq!(revision.created_by, "operator_alice");
            assert_eq!(revision.supersedes_revision, Some(1));
            assert_eq!(
                revision.content.objective,
                "Updated Payment Gateway Objective with Idempotency Keys"
            );
        }
        other => panic!("expected PlanForReview after edit, got {:?}", other),
    }

    // C. Model Revision request
    let model_rev_action = ApplicationAction::PlanRevisionRequested {
        session_id: Some(session_id.clone()),
        feedback: "Add rate limiting on checkout webhook endpoints".to_string(),
    };
    let resp = coordinator
        .handle_action(model_rev_action, "operator")
        .await
        .expect("model revision");

    match resp {
        PreExecutionResponse::PlanForReview { revision, .. } => {
            assert_eq!(revision.revision, 3);
            assert_eq!(revision.author_type, RevisionAuthorType::Model);
            assert!(revision.content.objective.contains("rate limiting"));
        }
        other => panic!("expected PlanForReview after revision, got {:?}", other),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 4. Plan Acceptance -> Task Generation & Review
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_plan_acceptance_unlocks_task_review_and_task_operations() {
    let (dir, pool, bus) = setup_test_db().await;
    let coordinator = PreExecutionCoordinator::deterministic_test(pool.clone(), Some(bus.clone()))
        .with_workspace_root(dir.path().to_path_buf());
    let session_id = create_test_session(&pool, dir.path()).await;

    // Setup plan in PlanReview
    let _ = coordinator
        .init_intent(
            &session_id,
            "Build a comprehensive API microservice with health checks, structured logging, and metrics endpoints",
            "operator",
        )
        .await
        .expect("init");

    // 1. Accept Plan
    let accept_action = ApplicationAction::PlanAcceptRequested {
        session_id: Some(session_id.clone()),
        revision: None,
        content_hash: None,
    };
    let resp = coordinator
        .handle_action(accept_action, "operator")
        .await
        .expect("accept plan");

    // 2. Acceptance must trigger task generation and lower into TasksForReview
    let initial_task_rev = match resp {
        PreExecutionResponse::TasksForReview { revision, .. } => {
            assert_eq!(revision.revision, 1);
            assert!(
                !revision.tasks.is_empty(),
                "tasks must be generated from plan"
            );
            revision
        }
        other => panic!("expected TasksForReview, got {:?}", other),
    };

    // 3. Task Edit
    let mut updated_task = initial_task_rev.tasks[0].clone();
    updated_task.objective = "Updated Task Objective: Implement hardened JWT auth".to_string();
    let task_json = serde_json::to_string(&updated_task).unwrap();

    let edit_task_action = ApplicationAction::TaskEditRequested {
        session_id: Some(session_id.clone()),
        task_json,
    };
    let resp = coordinator
        .handle_action(edit_task_action, "operator")
        .await
        .expect("edit task");

    match resp {
        PreExecutionResponse::TasksForReview { revision, .. } => {
            assert_eq!(revision.revision, 2);
            assert_eq!(revision.author_type, RevisionAuthorType::User);
            assert_eq!(
                revision.tasks[0].objective,
                "Updated Task Objective: Implement hardened JWT auth"
            );
        }
        other => panic!("expected TasksForReview after task edit, got {:?}", other),
    }

    // 4. Task Add
    let new_task = sample_task("custom_metric_task", vec![]);
    let new_task_json = serde_json::to_string(&new_task).unwrap();
    let add_action = ApplicationAction::TaskAddRequested {
        session_id: Some(session_id.clone()),
        task_json: new_task_json,
    };
    let resp = coordinator
        .handle_action(add_action, "operator")
        .await
        .expect("add task");

    match resp {
        PreExecutionResponse::TasksForReview { revision, .. } => {
            assert_eq!(revision.revision, 3);
            assert!(
                revision
                    .tasks
                    .iter()
                    .any(|t| t.id.0 == "custom_metric_task")
            );
        }
        other => panic!("expected TasksForReview after task add, got {:?}", other),
    }

    // 5. Task Remove (with dependent check)
    let current_tasks = coordinator
        .lifecycle_repo()
        .load_latest_task_revision(&session_id)
        .await
        .unwrap()
        .unwrap();

    if let Some(parent) = current_tasks.tasks.iter().find(|p| {
        current_tasks
            .tasks
            .iter()
            .any(|c| c.depends_on.contains(&p.id))
    }) {
        let bad_remove = ApplicationAction::TaskRemoveRequested {
            session_id: Some(session_id.clone()),
            task_id: parent.id.0.clone(),
        };
        let err = coordinator
            .handle_action(bad_remove, "operator")
            .await
            .unwrap_err();
        assert!(
            err.contains("dependent tasks"),
            "cannot remove prerequisite task: {}",
            err
        );
    }

    // Removing independent task succeeds
    let rm_action = ApplicationAction::TaskRemoveRequested {
        session_id: Some(session_id.clone()),
        task_id: "custom_metric_task".to_string(),
    };
    let resp = coordinator
        .handle_action(rm_action, "operator")
        .await
        .expect("remove task");

    match resp {
        PreExecutionResponse::TasksForReview { revision, .. } => {
            assert_eq!(revision.revision, 4);
            assert!(
                !revision
                    .tasks
                    .iter()
                    .any(|t| t.id.0 == "custom_metric_task")
            );
        }
        other => panic!("expected TasksForReview after task remove, got {:?}", other),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 5. Task Graph Validation: Cycles and Missing Dependencies Rejected
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_task_graph_cycle_and_missing_dep_detection() {
    // A. Missing dependency
    let t1 = sample_task("task_a", vec!["non_existent_task"]);
    let err = validate_candidate_tasks(&[t1]).unwrap_err();
    assert!(
        matches!(err, TaskGraphValidationError::MissingDependency { .. }),
        "expected missing dep error, got: {:?}",
        err
    );

    // B. Direct cycle
    let t_c1 = sample_task("task_c1", vec!["task_c2"]);
    let t_c2 = sample_task("task_c2", vec!["task_c1"]);
    let err = validate_candidate_tasks(&[t_c1, t_c2]).unwrap_err();
    assert!(
        matches!(err, TaskGraphValidationError::CycleDetected { .. }),
        "expected cycle error, got: {:?}",
        err
    );

    // C. Indirect cycle
    let t_x = sample_task("x", vec!["z"]);
    let t_y = sample_task("y", vec!["x"]);
    let t_z = sample_task("z", vec!["y"]);
    let err = validate_candidate_tasks(&[t_x, t_y, t_z]).unwrap_err();
    assert!(
        matches!(err, TaskGraphValidationError::CycleDetected { .. }),
        "expected indirect cycle error, got: {:?}",
        err
    );

    // D. Valid DAG passes
    let t_root = sample_task("root", vec![]);
    let t_child1 = sample_task("c1", vec!["root"]);
    let t_child2 = sample_task("c2", vec!["root"]);
    let t_sink = sample_task("sink", vec!["c1", "c2"]);
    assert!(validate_candidate_tasks(&[t_root, t_child1, t_child2, t_sink]).is_ok());
}

// ─────────────────────────────────────────────────────────────────────────────
// 6. Execution Authorization Gate & Invalidation
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_execution_authorization_and_invalidation() {
    let (dir, pool, bus) = setup_test_db().await;
    let coordinator = PreExecutionCoordinator::deterministic_test(pool.clone(), Some(bus.clone()))
        .with_workspace_root(dir.path().to_path_buf());
    let session_id = create_test_session(&pool, dir.path()).await;

    // Setup: Intent -> Plan -> Accept Plan -> Tasks
    let _ = coordinator
        .init_intent(
            &session_id,
            "Deploy a high-availability container cluster with ingress controller and monitoring stack",
            "operator",
        )
        .await
        .expect("init");

    let _ = coordinator
        .handle_action(
            ApplicationAction::PlanAcceptRequested {
                session_id: Some(session_id.clone()),
                revision: None,
                content_hash: None,
            },
            "operator",
        )
        .await
        .expect("accept plan");

    // 1. Accept Tasks
    let resp = coordinator
        .handle_action(
            ApplicationAction::TasksAcceptRequested {
                session_id: Some(session_id.clone()),
                revision: None,
                content_hash: None,
            },
            "operator",
        )
        .await
        .expect("accept tasks");

    // Must reach AuthorizationRequested
    match resp {
        PreExecutionResponse::AuthorizationRequested {
            session_id: sid,
            plan_revision,
            task_revision,
            message,
        } => {
            assert_eq!(sid, session_id);
            assert_eq!(plan_revision, 1);
            assert_eq!(task_revision, 1);
            assert!(message.contains("Proceed with implementation?"));
        }
        other => panic!("expected AuthorizationRequested, got {:?}", other),
    }

    // 2. Rejecting Authorization terminates session
    let reject_auth_action = ApplicationAction::ExecutionAuthorizationSubmitted {
        session_id: Some(session_id.clone()),
        decision: false,
        reason: Some("Not ready for production deploy".to_string()),
    };
    let term_resp = coordinator
        .handle_action(reject_auth_action, "operator")
        .await
        .expect("reject auth");
    match term_resp {
        PreExecutionResponse::Terminated { stage, reason, .. } => {
            assert_eq!(stage, LifecycleStage::Rejected);
            assert!(reason.contains("Not ready for production"));
        }
        other => panic!("expected Terminated, got {:?}", other),
    }

    // 3. Clean test for successful authorization
    let sess2 = create_test_session(&pool, dir.path()).await;
    let _ = coordinator
        .init_intent(
            &sess2,
            "Deploy a secondary high-availability container cluster with ingress controller and monitoring stack",
            "operator",
        )
        .await
        .expect("init");
    let _ = coordinator
        .handle_action(
            ApplicationAction::PlanAcceptRequested {
                session_id: Some(sess2.clone()),
                revision: None,
                content_hash: None,
            },
            "operator",
        )
        .await
        .expect("accept plan");
    let _ = coordinator
        .handle_action(
            ApplicationAction::TasksAcceptRequested {
                session_id: Some(sess2.clone()),
                revision: None,
                content_hash: None,
            },
            "operator",
        )
        .await
        .expect("accept tasks");

    // Submit explicit YES authorization
    let auth_action = ApplicationAction::ExecutionAuthorizationSubmitted {
        session_id: Some(sess2.clone()),
        decision: true,
        reason: None,
    };
    let ready_resp = coordinator
        .handle_action(auth_action, "operator_lead")
        .await
        .expect("authorize execution");

    match ready_resp {
        PreExecutionResponse::ReadyToExecute {
            authorization,
            tasks,
            plan,
            ..
        } => {
            assert_eq!(authorization.plan_revision, 1);
            assert_eq!(authorization.task_revision, 1);
            assert_eq!(authorization.authorized_by, "operator_lead");
            assert_eq!(authorization.decision, AuthorizationDecision::Authorized);
            assert!(!tasks.is_empty());
            assert_eq!(plan.revision, 1);
        }
        other => panic!("expected ReadyToExecute, got {:?}", other),
    }

    // 4. Verify Authorization Invalidation on upstream plan change
    let repo = coordinator.lifecycle_repo();
    repo.invalidate_authorizations(&sess2, "Plan modified after launch")
        .await
        .expect("invalidate");

    let auth = repo
        .load_latest_execution_authorization(&sess2)
        .await
        .expect("load auth")
        .expect("auth exists");
    assert!(
        !auth.is_valid_for(1, 1),
        "authorization must be invalidated"
    );
    assert_eq!(
        auth.invalidation_reason.as_deref(),
        Some("Plan modified after launch")
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// 7. Crash/Restart Recovery
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_crash_restart_recovery_restores_exact_lifecycle_stage() {
    let (dir, pool, bus) = setup_test_db().await;
    let session_id = create_test_session(&pool, dir.path()).await;

    {
        // First instance of coordinator advances to TasksReview
        let coordinator =
            PreExecutionCoordinator::deterministic_test(pool.clone(), Some(bus.clone()));
        let _ = coordinator
            .init_intent(
                &session_id,
                "Build a comprehensive microservice architecture with service discovery and health monitoring",
                "operator",
            )
            .await
            .expect("init");
        let _ = coordinator
            .handle_action(
                ApplicationAction::PlanAcceptRequested {
                    session_id: Some(session_id.clone()),
                    revision: None,
                    content_hash: None,
                },
                "operator",
            )
            .await
            .expect("accept plan");
    }

    // "Crash" and create new coordinator instance attached to same SQLite database
    let coordinator2 = PreExecutionCoordinator::deterministic_test(pool.clone(), Some(bus.clone()));
    let recovered_resp = coordinator2
        .resume_session(&session_id)
        .await
        .expect("resume session");

    // Coordinator accurately recovers into TasksForReview
    match recovered_resp {
        PreExecutionResponse::TasksForReview { revision, .. } => {
            assert_eq!(revision.plan_revision, 1);
            assert_eq!(revision.revision, 1);
        }
        other => panic!("expected TasksForReview on recovery, got {:?}", other),
    }
}
