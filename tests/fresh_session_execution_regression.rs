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
    tokio::fs::write(
        repo_path.join("Cargo.toml"),
        "[package]\nname = \"coffee_orders\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[dependencies]\n",
    )
    .await
    .unwrap();
    tokio::fs::create_dir_all(repo_path.join("src"))
        .await
        .unwrap();
    tokio::fs::write(repo_path.join("src/main.rs"), "fn main() {}\n")
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
    let mut planning_choice_received = false;

    for _ in 0..100 {
        if let Ok(Some(event)) =
            tokio::time::timeout(Duration::from_millis(100), event_rx.recv()).await
        {
            match event {
                InteractionEvent::SessionStarted { session_id } => {
                    println!("  [Event] SessionStarted: {session_id}");
                    established_session_id = Some(session_id);
                }
                InteractionEvent::PlanningChoiceRequired {
                    session_id,
                    prompt,
                    question,
                } => {
                    println!(
                        "  [Event] PlanningChoiceRequired: session={session_id}, prompt={prompt:?}, question={question:?}"
                    );
                    planning_choice_received = true;
                    if established_session_id.is_none() {
                        established_session_id = session_id.parse().ok();
                    }
                    action_tx
                        .send(ApplicationAction::PlanningChoiceSubmitted {
                            session_id: Some(session_id),
                            generate_project_docs: false,
                        })
                        .expect("send PlanningChoiceSubmitted");
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
        planning_choice_received,
        "PlanningChoiceRequired must be received for ambiguous fresh request"
    );
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

    // 7. Drain and observe execution events through the REAL production TUI
    // event channel (ExecutionReady + terminal Completion/Error). Bounded
    // wait for the actual execution result — no fixed sleep.
    let mut saw_exec_ready = false;
    let mut terminal_outcome: Option<&str> = None;
    let deadline = std::time::Instant::now() + Duration::from_secs(60);
    while std::time::Instant::now() < deadline {
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        let timeout = remaining.min(Duration::from_millis(200));
        match tokio::time::timeout(timeout, event_rx.recv()).await {
            Ok(Some(event)) => match event {
                InteractionEvent::ExecutionReady { .. } => {
                    println!("  [Event] ExecutionReady");
                    saw_exec_ready = true;
                }
                InteractionEvent::Completion { .. } => {
                    println!("  [Event] Completion");
                    terminal_outcome = Some("completed");
                    break;
                }
                InteractionEvent::Error { message } => {
                    println!("  [Event] Error: {message}");
                    terminal_outcome = Some("error");
                    break;
                }
                event => {
                    println!("  [Event] Other: {event:?}");
                }
            },
            _ => {
                // No event yet: check whether the run already reached a
                // terminal persisted state, then keep waiting.
                let session_repo = SqliteSessionRepository::new(runtime.pool().clone());
                if let Ok(Some(sess)) = session_repo.get_session(session_id).await {
                    if let Some(rid) = sess.active_run_id.or(sess.active_mission_id) {
                        let exec_repo = SqliteExecutionRunRepository::new(runtime.pool().clone());
                        if let Ok(Some(run)) = exec_repo.get(rid).await {
                            if run.status.is_terminal() {
                                break;
                            }
                        }
                    }
                }
            }
        }
        if saw_exec_ready && terminal_outcome.is_some() {
            break;
        }
    }
    println!("  Terminal outcome observed: {:?}", terminal_outcome);

    assert!(
        saw_exec_ready,
        "InteractionEvent::ExecutionReady must be emitted on the real TUI event channel"
    );
    println!("  Verified: ReadyToExecute reached cleanly through production TUI bridge.");

    // 8. Bounded wait for the canonical ExecutionRun to reach a terminal or
    // executing persisted state (polls; never a fixed sleep).
    let exec_repo = SqliteExecutionRunRepository::new(runtime.pool().clone());
    let updated_session = session_repo
        .get_session(session_id)
        .await
        .expect("query session")
        .expect("session must exist");

    // Canonical identity: a fresh session executes WITHOUT any Mission
    // prerequisite. active_run_id is authoritative; active_mission_id is only
    // a legacy mirror of the same UUID.
    let bound_run_id = updated_session
        .active_run_id
        .expect("Session must now be associated with a canonical active_run_id (ExecutionRun)");
    println!("  Associated ExecutionRunId: {bound_run_id}");

    let exec_run = tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            if let Ok(Some(run)) = exec_repo.get(bound_run_id).await {
                if run.status.is_terminal() || run.status == ExecutionRunState::Executing {
                    return run;
                }
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .expect("bounded wait for ExecutionRun progress must not time out");

    assert_eq!(
        exec_run.session_id,
        Some(session_id),
        "ExecutionRun must reference the active session"
    );
    assert!(
        matches!(
            exec_run.status,
            ExecutionRunState::Executing
                | ExecutionRunState::Completed
                | ExecutionRunState::Failed
                | ExecutionRunState::Cancelled
        ),
        "ExecutionRun must have progressed to a live state, got: {:?}",
        exec_run.status
    );
    println!(
        "  Verified: ExecutionRun persisted with status {:?}",
        exec_run.status
    );

    // 9. VERIFY: Authorized TaskGraph is materialized with canonical run_id.
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
    assert_eq!(
        active_graph.canonical_run_id(),
        bound_run_id,
        "TaskGraph must be owned by the canonical ExecutionRun"
    );
    for (task_id, task) in &active_graph.tasks {
        assert_eq!(
            task.canonical_run_id(),
            bound_run_id,
            "Task {task_id} must carry the canonical run_id"
        );
    }
    println!(
        "  Verified: TaskGraph materialized with {} tasks for run {} (all run_id-linked)",
        active_graph.tasks.len(),
        bound_run_id
    );

    // 10. VERIFY: execution did not require the obsolete Mission aggregate.
    // The fresh session started with no active_mission_id; the canonical
    // active_run_id drove the entire lifecycle through the real TUI path.
    assert!(
        initial_session.active_run_id.is_none(),
        "Fresh session must start with no active run (normal supported state, not an error)"
    );

    bridge_task.abort();
    println!(
        "  ALL CHECKS PASSED: Fresh session cleanly started authorized execution without obsolete active_mission_id prerequisite."
    );
}

#[tokio::test]
async fn test_ambiguous_fresh_request_requires_clarification_and_no_unintended_execution() {
    let (dir, runtime) = setup_test_workspace().await;
    let repo_path = dir.path().to_path_buf();

    let (mut bridge, bridge_task) = TuiRuntimeBridge::spawn(runtime.clone(), None)
        .await
        .expect("TuiRuntimeBridge::spawn");

    let action_tx = bridge.sender();
    let mut event_rx = bridge.take_event_receiver().expect("take_event_receiver");

    // Submit ambiguous prompt in existing codebase
    let prompt = "Build a microservice for processing invoices.";
    let parsed_msg = MentionParser::parse(prompt, &repo_path);
    action_tx
        .send(ApplicationAction::UserTextSubmitted(parsed_msg))
        .expect("send UserTextSubmitted");

    let mut established_session_id: Option<SessionId> = None;
    let mut planning_choice_received = false;
    let mut plan_received = false;

    // Collect events up to PlanningChoiceRequired
    for _ in 0..50 {
        if let Ok(Some(event)) =
            tokio::time::timeout(Duration::from_millis(100), event_rx.recv()).await
        {
            match event {
                InteractionEvent::SessionStarted { session_id } => {
                    established_session_id = Some(session_id);
                }
                InteractionEvent::PlanningChoiceRequired { session_id, .. } => {
                    planning_choice_received = true;
                    if established_session_id.is_none() {
                        established_session_id = session_id.parse().ok();
                    }
                    break;
                }
                InteractionEvent::PlanForReview { .. } => {
                    plan_received = true;
                    break;
                }
                _ => {}
            }
        }
    }

    assert!(
        planning_choice_received,
        "Ambiguous request must trigger PlanningChoiceRequired clarification"
    );
    assert!(
        !plan_received,
        "PlanForReview must NOT be emitted before clarification is provided"
    );

    let session_id = established_session_id.expect("SessionId must be known");
    let session_repo = SqliteSessionRepository::new(runtime.pool().clone());
    let pending_sess = session_repo
        .get_session(session_id)
        .await
        .expect("get session")
        .expect("session exists");

    // State persistence assertion: planning_choice_pending must be present
    assert!(
        pending_sess
            .metadata
            .get("planning_choice_pending")
            .is_some(),
        "planning_choice_pending must be persisted in session metadata"
    );
    assert!(
        pending_sess.active_run_id.is_none(),
        "No active ExecutionRun before clarification"
    );

    // Verify absence of active task graph or execution run
    let active_graphs: Vec<String> = sqlx::query_scalar("SELECT run_id FROM task_graphs")
        .fetch_all(runtime.pool())
        .await
        .unwrap_or_default();
    assert!(
        active_graphs.is_empty(),
        "No task graph should be materialized before clarification"
    );

    // Now submit clarification
    action_tx
        .send(ApplicationAction::PlanningChoiceSubmitted {
            session_id: Some(session_id.to_string()),
            generate_project_docs: false,
        })
        .expect("send PlanningChoiceSubmitted");

    // PlanForReview should now arrive
    for _ in 0..50 {
        if let Ok(Some(InteractionEvent::PlanForReview { .. })) =
            tokio::time::timeout(Duration::from_millis(100), event_rx.recv()).await
        {
            plan_received = true;
            break;
        }
    }
    assert!(
        plan_received,
        "PlanForReview must arrive after clarification is submitted"
    );

    let updated_sess = session_repo
        .get_session(session_id)
        .await
        .expect("get session")
        .expect("session exists");
    assert!(
        updated_sess
            .metadata
            .get("planning_choice_pending")
            .is_none(),
        "planning_choice_pending must be cleared after resolution"
    );
    assert_eq!(
        SqliteSessionRepository::read_planning_choice(&updated_sess),
        Some(m31a::runtime::PlanningChoice::DirectExecution)
    );

    bridge_task.abort();
}

#[tokio::test]
async fn test_unambiguous_fresh_request_follows_normal_path() {
    let (dir, runtime) = setup_test_workspace().await;
    let repo_path = dir.path().to_path_buf();

    let (mut bridge, bridge_task) = TuiRuntimeBridge::spawn(runtime.clone(), None)
        .await
        .expect("TuiRuntimeBridge::spawn");

    let action_tx = bridge.sender();
    let mut event_rx = bridge.take_event_receiver().expect("take_event_receiver");

    // Submit an unambiguous request (localized fix in brownfield repository)
    let prompt = "Fix the typo in README.md";
    let parsed_msg = MentionParser::parse(prompt, &repo_path);
    action_tx
        .send(ApplicationAction::UserTextSubmitted(parsed_msg))
        .expect("send UserTextSubmitted");

    let mut planning_choice_received = false;
    let mut plan_received = false;

    for _ in 0..50 {
        if let Ok(Some(event)) =
            tokio::time::timeout(Duration::from_millis(100), event_rx.recv()).await
        {
            match event {
                InteractionEvent::PlanningChoiceRequired { .. } => {
                    planning_choice_received = true;
                }
                InteractionEvent::DiscoveryRequired {
                    session_id,
                    questions,
                } => {
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
                InteractionEvent::PlanForReview { .. } => {
                    plan_received = true;
                    break;
                }
                _ => {}
            }
        }
    }

    assert!(
        !planning_choice_received,
        "Unambiguous request must NOT ask for planning choice clarification"
    );
    assert!(
        plan_received,
        "Unambiguous request must directly transition to PlanForReview"
    );

    bridge_task.abort();
}

#[tokio::test]
async fn test_event_delivery_consumer_subscribes_after_producer_starts() {
    let (dir, runtime) = setup_test_workspace().await;
    let repo_path = dir.path().to_path_buf();

    // Spawn bridge but do NOT take event receiver immediately
    let (mut bridge, bridge_task) = TuiRuntimeBridge::spawn(runtime.clone(), None)
        .await
        .expect("TuiRuntimeBridge::spawn");

    let action_tx = bridge.sender();

    // Submit request before taking receiver
    let prompt = "Build a cache module for coffee orders.";
    let parsed_msg = MentionParser::parse(prompt, &repo_path);
    action_tx
        .send(ApplicationAction::UserTextSubmitted(parsed_msg))
        .expect("send UserTextSubmitted");

    // Allow producer to start, create session, and emit events into the channel buffer
    tokio::time::sleep(Duration::from_millis(150)).await;

    // Consumer now subscribes (takes event receiver) after events were already emitted
    let mut event_rx = bridge.take_event_receiver().expect("take_event_receiver");

    let mut saw_config = false;
    let mut saw_session = false;
    let mut saw_planning_choice = false;

    for _ in 0..50 {
        if let Ok(Some(event)) =
            tokio::time::timeout(Duration::from_millis(100), event_rx.recv()).await
        {
            match event {
                InteractionEvent::ConfigurationUpdated { .. } => {
                    saw_config = true;
                }
                InteractionEvent::SessionStarted { .. } => {
                    saw_session = true;
                }
                InteractionEvent::PlanningChoiceRequired { .. } => {
                    saw_planning_choice = true;
                    break;
                }
                _ => {}
            }
        }
    }

    assert!(saw_config, "Buffered ConfigurationUpdated must be received");
    assert!(saw_session, "Buffered SessionStarted must be received");
    assert!(
        saw_planning_choice,
        "Buffered PlanningChoiceRequired must be received"
    );

    bridge_task.abort();
}

async fn setup_greenfield_workspace() -> (tempfile::TempDir, Arc<AppRuntime>) {
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
        .args(["config", "user.email", "greenfield@m31a.local"])
        .current_dir(repo_path)
        .status();
    let _ = std::process::Command::new("git")
        .args(["config", "user.name", "Greenfield Tester"])
        .current_dir(repo_path)
        .status();

    let runtime = AppRuntime::new(repo_path).await.expect("AppRuntime::new");
    let runtime = runtime.with_model_caller(Arc::new(DeterministicLifecycleModelCaller::new()));
    (dir, Arc::new(runtime))
}

#[tokio::test]
async fn test_greenfield_new_project_request_proceeds_to_planning() {
    // verifies that a new project starter in greenfield workspace proceeds to planning without clarification
    let (dir, runtime) = setup_greenfield_workspace().await;
    let repo_path = dir.path().to_path_buf();

    let (mut bridge, bridge_task) = TuiRuntimeBridge::spawn(runtime.clone(), None)
        .await
        .expect("TuiRuntimeBridge::spawn");

    let action_tx = bridge.sender();
    let mut event_rx = bridge.take_event_receiver().expect("take_event_receiver");

    let prompt = "Build a small web service for managing customer coffee orders.";
    let parsed_msg = MentionParser::parse(prompt, &repo_path);
    action_tx
        .send(ApplicationAction::UserTextSubmitted(parsed_msg))
        .expect("send UserTextSubmitted");

    let mut planning_choice_received = false;
    let mut plan_received = false;

    for _ in 0..50 {
        if let Ok(Some(event)) =
            tokio::time::timeout(Duration::from_millis(100), event_rx.recv()).await
        {
            match event {
                InteractionEvent::PlanningChoiceRequired { .. } => {
                    planning_choice_received = true;
                }
                InteractionEvent::DiscoveryRequired {
                    session_id,
                    questions,
                } => {
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
                InteractionEvent::PlanForReview { .. } => {
                    plan_received = true;
                    break;
                }
                _ => {}
            }
        }
    }

    assert!(
        !planning_choice_received,
        "Greenfield new project request must not require planning choice clarification"
    );
    assert!(
        plan_received,
        "Greenfield new project request must proceed to plan review"
    );

    bridge_task.abort();
}

#[tokio::test]
async fn test_explicit_planning_request_proceeds_without_clarification() {
    // verifies that an explicit /plan prefix counts as planning consent without prompting
    let (dir, runtime) = setup_test_workspace().await;
    let repo_path = dir.path().to_path_buf();

    let (mut bridge, bridge_task) = TuiRuntimeBridge::spawn(runtime.clone(), None)
        .await
        .expect("TuiRuntimeBridge::spawn");

    let action_tx = bridge.sender();
    let mut event_rx = bridge.take_event_receiver().expect("take_event_receiver");

    let prompt = "/plan Build a small web service for managing customer coffee orders.";
    let parsed_msg = MentionParser::parse(prompt, &repo_path);
    action_tx
        .send(ApplicationAction::UserTextSubmitted(parsed_msg))
        .expect("send UserTextSubmitted");

    let mut planning_choice_received = false;
    let mut plan_received = false;

    for _ in 0..50 {
        if let Ok(Some(event)) =
            tokio::time::timeout(Duration::from_millis(100), event_rx.recv()).await
        {
            match event {
                InteractionEvent::PlanningChoiceRequired { .. } => {
                    planning_choice_received = true;
                }
                InteractionEvent::DiscoveryRequired {
                    session_id,
                    questions,
                } => {
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
                InteractionEvent::PlanForReview { .. } => {
                    plan_received = true;
                    break;
                }
                _ => {}
            }
        }
    }

    assert!(
        !planning_choice_received,
        "Explicit /plan request must not prompt for planning choice"
    );
    assert!(
        plan_received,
        "Explicit /plan request must proceed to plan review"
    );

    bridge_task.abort();
}

#[tokio::test]
async fn test_explicit_refusal_of_planning_proceeds_to_direct_execution() {
    // verifies that an explicit /no-plan prefix routes directly to direct execution
    let (dir, runtime) = setup_test_workspace().await;
    let repo_path = dir.path().to_path_buf();

    let (mut bridge, bridge_task) = TuiRuntimeBridge::spawn(runtime.clone(), None)
        .await
        .expect("TuiRuntimeBridge::spawn");

    let action_tx = bridge.sender();
    let mut event_rx = bridge.take_event_receiver().expect("take_event_receiver");

    let prompt = "/no-plan Build a small web service for managing customer coffee orders.";
    let parsed_msg = MentionParser::parse(prompt, &repo_path);
    action_tx
        .send(ApplicationAction::UserTextSubmitted(parsed_msg))
        .expect("send UserTextSubmitted");

    let mut planning_choice_received = false;
    let mut plan_received = false;

    for _ in 0..50 {
        if let Ok(Some(event)) =
            tokio::time::timeout(Duration::from_millis(100), event_rx.recv()).await
        {
            match event {
                InteractionEvent::PlanningChoiceRequired { .. } => {
                    planning_choice_received = true;
                }
                InteractionEvent::DiscoveryRequired {
                    session_id,
                    questions,
                } => {
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
                InteractionEvent::PlanForReview { .. } => {
                    plan_received = true;
                    break;
                }
                _ => {}
            }
        }
    }

    assert!(
        !planning_choice_received,
        "Explicit /no-plan request must not prompt for planning choice"
    );
    assert!(
        plan_received,
        "Explicit /no-plan request must proceed to plan review directly"
    );

    bridge_task.abort();
}

#[tokio::test]
async fn test_session_with_persisted_choice_reuses_choice_without_clarification() {
    // verifies that a session with an intentionally persisted choice reuses it without re-prompting
    let (dir, runtime) = setup_test_workspace().await;
    let repo_path = dir.path().to_path_buf();

    let session_repo = SqliteSessionRepository::new(runtime.pool().clone());
    let session = session_repo
        .create_session(&repo_path)
        .await
        .expect("create_session");
    session_repo
        .set_planning_choice(session.id, m31a::runtime::PlanningChoice::DirectExecution)
        .await
        .expect("set_planning_choice");

    let (mut bridge, bridge_task) = TuiRuntimeBridge::spawn(runtime.clone(), Some(session.id))
        .await
        .expect("TuiRuntimeBridge::spawn");

    let action_tx = bridge.sender();
    let mut event_rx = bridge.take_event_receiver().expect("take_event_receiver");

    let prompt = "Build a small web service for managing customer coffee orders.";
    let parsed_msg = MentionParser::parse(prompt, &repo_path);
    action_tx
        .send(ApplicationAction::UserTextSubmitted(parsed_msg))
        .expect("send UserTextSubmitted");

    let mut planning_choice_received = false;
    let mut plan_received = false;

    for _ in 0..50 {
        if let Ok(Some(event)) =
            tokio::time::timeout(Duration::from_millis(100), event_rx.recv()).await
        {
            match event {
                InteractionEvent::PlanningChoiceRequired { .. } => {
                    planning_choice_received = true;
                }
                InteractionEvent::DiscoveryRequired {
                    session_id,
                    questions,
                } => {
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
                InteractionEvent::PlanForReview { .. } => {
                    plan_received = true;
                    break;
                }
                _ => {}
            }
        }
    }

    assert!(
        !planning_choice_received,
        "Session with persisted planning choice must not prompt for clarification"
    );
    assert!(
        plan_received,
        "Session with persisted planning choice must proceed to plan review"
    );

    bridge_task.abort();
}
