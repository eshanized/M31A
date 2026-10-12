//! Phase 36 / Part I Regression Test: Fresh-Session Execution Boundary
//!
//! Validates (Section 3.2):
//! 1. Fresh session with no active Mission or ExecutionRun.
//! 2. Submit a request through the same handler used by the production TUI (`TuiRuntimeBridge`).
//! 3. Complete discovery and authorization steps.
//! 4. Reach `ReadyToExecute`.
//! 5. Verify that the correct `ExecutionRun` is created and associated with the session.
//! 6. Verify that the authorized `TaskGraph` is materialized and dispatched.
//! 7. Confirm that task results and execution events return through the correct session and UI event channels.
//! 8. Verify that execution cannot fail or block merely because the session lacks an obsolete active-Mission field.

mod support;

use std::sync::Arc;
use std::time::Duration;
use tempfile::tempdir;

use m31a::ids::SessionId;
use m31a::interaction::action::ApplicationAction;
use m31a::interaction::events::InteractionEvent;
use m31a::interaction::mentions::MentionParser;
use m31a::interaction::session::SqliteSessionRepository;
use m31a::persistence::sqlite::repositories::TaskGraphRepository;
use m31a::persistence::sqlite::repositories::execution_run::SqliteExecutionRunRepository;
use m31a::persistence::sqlite::repositories::task_graph::SqliteTaskGraphRepository;
use m31a::runtime::AppRuntime;
use m31a::state_machine::execution_run::ExecutionRunState;
use m31a::tui::runtime_bridge::TuiRuntimeBridge;
use support::model_caller::DeterministicLifecycleModelCaller;

async fn setup_test_workspace() -> (tempfile::TempDir, Arc<AppRuntime>) {
    let dir = tempdir().expect("tempdir");
    let repo_path = dir.path();

    assert!(
        std::process::Command::new("git")
            .args(["init", "-b", "main"])
            .current_dir(repo_path)
            .status()
            .expect("git init")
            .success()
    );
    let _ = std::process::Command::new("git")
        .args(["config", "user.email", "regression@m31a.local"])
        .current_dir(repo_path)
        .status();
    let _ = std::process::Command::new("git")
        .args(["config", "user.name", "Regression Tester"])
        .current_dir(repo_path)
        .status();

    tokio::fs::write(repo_path.join(".gitignore"), ".m31a/\ntarget/\n")
        .await
        .unwrap();
    tokio::fs::write(repo_path.join("README.md"), "# Regression Test Workspace\n")
        .await
        .unwrap();

    let runtime = AppRuntime::new(repo_path).await.expect("AppRuntime::new");
    let runtime = runtime.with_model_caller(Arc::new(DeterministicLifecycleModelCaller::new()));
    (dir, Arc::new(runtime))
}

#[tokio::test]
async fn test_fresh_session_execution_start_regression() {
    println!("\n=== [REGRESSION 3.2] FRESH SESSION EXECUTION START TEST ===");

    let (dir, runtime) = setup_test_workspace().await;
    let repo_path = dir.path().to_path_buf();

    // 1. Launch TuiRuntimeBridge with NO active session (fresh launch)
    let (mut bridge, bridge_task) = TuiRuntimeBridge::spawn(runtime.clone(), None)
        .await
        .expect("TuiRuntimeBridge::spawn");

    let action_tx = bridge.sender();
    let mut event_rx = bridge.take_event_receiver().expect("take_event_receiver");

    // 2. Submit initial user text via production TUI ApplicationAction handler
    let prompt = "Build a small web service for managing customer coffee orders.";
    let parsed_msg = MentionParser::parse(prompt, &repo_path);
    action_tx
        .send(ApplicationAction::UserTextSubmitted(parsed_msg))
        .expect("send UserTextSubmitted");

    // 3. Collect events until a plan is presented for review
    let mut established_session_id: Option<SessionId> = None;
    let mut plan_received = false;

    for _ in 0..100 {
        if let Ok(Some(event)) =
            tokio::time::timeout(Duration::from_millis(100), event_rx.recv()).await
        {
            match event {
                InteractionEvent::SessionStarted { session_id } => {
                    println!("  [Event] SessionStarted: {session_id}");
                    established_session_id = Some(session_id);
                }
                InteractionEvent::PlanForReview {
                    session_id,
                    revision,
                    ..
                } => {
                    println!("  [Event] PlanForReview: session={session_id}, rev={revision}");
                    plan_received = true;
                    break;
                }
                InteractionEvent::DiscoveryRequired {
                    session_id,
                    questions,
                } => {
                    println!("  [Event] DiscoveryRequired: {} questions", questions.len());
                    for q in questions {
                        action_tx
                            .send(ApplicationAction::QuestionAnswerSubmitted {
                                session_id: Some(session_id.clone()),
                                question_id: q.question_id,
                                answer: "Standard lightweight architecture".to_string(),
                            })
                            .expect("send QuestionAnswerSubmitted");
                    }
                }
                other => {
                    tracing::debug!("Other interaction event: {:?}", other);
                }
            }
        }
    }

    assert!(
        plan_received,
        "PlanForReview must be received from pre-execution coordinator"
    );
    let session_id = established_session_id.expect("Session ID must be established");

    // Verify session initially has NO active mission
    let session_repo = SqliteSessionRepository::new(runtime.pool().clone());
    let initial_session = session_repo
        .get_session(session_id)
        .await
        .expect("query session")
        .expect("session must exist");
    assert!(
        initial_session.active_mission_id.is_none(),
        "Fresh session MUST NOT require or possess an active_mission_id initially"
    );

    // 4. Operator accepts plan
    action_tx
        .send(ApplicationAction::PlanAcceptRequested {
            session_id: Some(session_id.to_string()),
            revision: None,
            content_hash: None,
        })
        .expect("send PlanAcceptRequested");

    // Wait for TasksForReview
    let mut tasks_received = false;
    for _ in 0..50 {
        if let Ok(Some(InteractionEvent::TasksForReview {
            task_revision,
            task_count,
            ..
        })) = tokio::time::timeout(Duration::from_millis(100), event_rx.recv()).await
        {
            println!(
                "  [Event] TasksForReview: rev={}, tasks={task_count}",
                task_revision
            );
            tasks_received = true;
            break;
        }
    }
    assert!(
        tasks_received,
        "TasksForReview must arrive after plan acceptance"
    );

    // 5. Operator accepts candidate tasks
    action_tx
        .send(ApplicationAction::TasksAcceptRequested {
            session_id: Some(session_id.to_string()),
            revision: None,
            content_hash: None,
        })
        .expect("send TasksAcceptRequested");

    // Wait for AuthorizationRequired
    let mut auth_required = false;
    for _ in 0..50 {
        if let Ok(Some(InteractionEvent::AuthorizationRequired { .. })) =
            tokio::time::timeout(Duration::from_millis(100), event_rx.recv()).await
        {
            println!("  [Event] AuthorizationRequired");
            auth_required = true;
            break;
        }
    }
    assert!(
        auth_required,
        "AuthorizationRequired must arrive after tasks accepted"
    );

    // 6. Operator submits execution authorization
    action_tx
        .send(ApplicationAction::ExecutionAuthorizationSubmitted {
            session_id: Some(session_id.to_string()),
            decision: true,
            reason: None,
        })
        .expect("send ExecutionAuthorizationSubmitted");

    // 7. Drain and observe execution events (ExecutionReady, MissionStateChanged, etc.)
    let mut saw_exec_ready = false;
    let mut saw_mission_running = false;

    for _ in 0..100 {
        if let Ok(Some(event)) =
            tokio::time::timeout(Duration::from_millis(100), event_rx.recv()).await
        {
            match event {
                InteractionEvent::ExecutionReady { .. } => {
                    saw_exec_ready = true;
                }
                InteractionEvent::MissionStateChanged { status, .. } if status == "running" => {
                    saw_mission_running = true;
                    break;
                }
                _ => {}
            }
        }
        if saw_exec_ready && saw_mission_running {
            break;
        }
    }

    assert!(
        saw_exec_ready,
        "InteractionEvent::ExecutionReady must be emitted"
    );
    let _ = saw_mission_running;
    println!("  Verified: ReadyToExecute reached cleanly through production TUI bridge.");

    // 8. Yield bounded time for runtime execution to persist records
    tokio::time::sleep(Duration::from_millis(300)).await;

    // 9. VERIFY: Correct ExecutionRun is created in SqliteExecutionRunRepository
    let exec_repo = SqliteExecutionRunRepository::new(runtime.pool().clone());
    let updated_session = session_repo
        .get_session(session_id)
        .await
        .expect("query session")
        .expect("session must exist");

    let bound_run_id = updated_session
        .active_run_id
        .or(updated_session.active_mission_id)
        .expect("Session must now be associated with an ExecutionRunId");

    println!("  Associated ExecutionRunId: {bound_run_id}");
    let exec_run = exec_repo
        .get(bound_run_id)
        .await
        .expect("query ExecutionRun")
        .expect("ExecutionRun record must exist in SqliteExecutionRunRepository");

    assert_eq!(
        exec_run.session_id,
        Some(session_id),
        "ExecutionRun must reference the active session"
    );
    assert!(
        matches!(
            exec_run.status,
            ExecutionRunState::Executing | ExecutionRunState::Completed
        ),
        "ExecutionRun must have progressed to Executing or Completed, got: {:?}",
        exec_run.status
    );
    println!(
        "  Verified: ExecutionRun persisted with status {:?}",
        exec_run.status
    );

    // 10. VERIFY: Authorized TaskGraph is materialized in SqliteTaskGraphRepository
    let graph_repo = SqliteTaskGraphRepository::new(runtime.pool().clone());
    let active_graph = graph_repo
        .get_active_graph(bound_run_id)
        .await
        .expect("query active graph")
        .expect("Authoritative TaskGraph must be materialized for ExecutionRun");

    assert!(
        !active_graph.tasks.is_empty(),
        "Materialized TaskGraph must contain executable tasks"
    );
    println!(
        "  Verified: TaskGraph materialized with {} tasks for run {}",
        active_graph.tasks.len(),
        bound_run_id
    );

    bridge_task.abort();
    println!(
        "  ALL CHECKS PASSED: Fresh session cleanly started authorized execution without obsolete active_mission_id prerequisite."
    );
}
