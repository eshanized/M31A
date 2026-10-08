//! TUI runtime/state/scrolling repair acceptance suite (§23–§28, §35).
//!
//! Every test exercises real state machines and the real bridge path —
//! responses are never mocked so aggressively that the bridge is bypassed.
//! Timeouts exist only to fail deterministically, never to fake success.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use m31a::agent::model_policy::DeterministicLifecycleModelCaller;
use m31a::interaction::action::ApplicationAction;
use m31a::interaction::events::InteractionEvent;
use m31a::interaction::parser::InteractionParser;
use m31a::interaction::state::SessionPromptState;
use m31a::runtime::AppRuntime;
use m31a::tui::conversation::TuiConversationItem;
use m31a::tui::model::{ActivityKind, TuiViewModel, UiOperationState};
use m31a::tui::{TuiApplication, TuiRuntimeBridge};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use std::sync::Arc;
use std::time::Duration;

fn enter_key() -> KeyEvent {
    KeyEvent::new(KeyCode::Enter, KeyModifiers::empty())
}

fn buffer_text(app: &mut TuiApplication, width: u16, height: u16) -> String {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    app.force_redraw = true;
    app.render_frame(&mut terminal).expect("render frame");
    let buffer = terminal.backend().buffer().clone();
    (0..buffer.area.height)
        .map(|y| {
            (0..buffer.area.width)
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<String>>()
        .join("\n")
}

fn header_text(model: &TuiViewModel, width: u16, height: u16) -> String {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();
    let tokens = m31a::tui::theme::ThemeTokens::resolve(m31a::tui::theme::ThemeMode::Default);
    let replay = m31a::tui::replay::ReplayController::new();
    terminal
        .draw(|f| {
            m31a::tui::shell::header::render_header(
                f,
                f.area(),
                model,
                m31a::tui::navigation::ScreenId::Dashboard,
                &replay,
                &tokens,
            );
        })
        .unwrap();
    let buffer = terminal.backend().buffer().clone();
    (0..buffer.area.height)
        .map(|y| {
            (0..buffer.area.width)
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<String>>()
        .join("\n")
}

async fn setup_runtime(deterministic_model: bool) -> (tempfile::TempDir, Arc<AppRuntime>) {
    let dir = tempfile::tempdir().expect("fixture tempdir");
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
        .args(["config", "user.email", "tui-test@m31a.local"])
        .current_dir(repo_path)
        .status();
    let _ = std::process::Command::new("git")
        .args(["config", "user.name", "M31A TUI Test"])
        .current_dir(repo_path)
        .status();
    let runtime = AppRuntime::new(repo_path).await.expect("AppRuntime::new");
    let runtime = if deterministic_model {
        runtime.with_model_caller(Arc::new(DeterministicLifecycleModelCaller::new()))
    } else {
        runtime
    };
    (dir, Arc::new(runtime))
}

/// Attach a live bridge to a composer-focused app; drain startup chatter.
async fn bridge_app(runtime: Arc<AppRuntime>) -> (TuiApplication, tokio::task::JoinHandle<()>) {
    let (mut bridge, handle) = TuiRuntimeBridge::spawn(runtime, None).await.unwrap();
    let sender = bridge.sender();
    let irx = bridge.take_event_receiver().expect("event receiver");
    let mut app = TuiApplication::new()
        .with_bridge_tx(sender)
        .with_interaction_rx(irx)
        .with_composer_focused(true);
    // Drain SessionStarted / GitStateChanged startup events.
    let deadline = tokio::time::Instant::now() + Duration::from_millis(800);
    while tokio::time::Instant::now() < deadline {
        app.poll_updates();
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    app.poll_updates();
    (app, handle)
}

fn submit(app: &mut TuiApplication, text: &str) {
    app.composer.set_text(text);
    app.handle_key(enter_key());
}

/// Pump the TUI event loop until `pred` holds or the timeout expires.
/// Asserts on every iteration that slash-command handling never entered
/// model-thinking state when `forbid_thinking` is set.
async fn pump_until(
    app: &mut TuiApplication,
    timeout: Duration,
    forbid_thinking: bool,
    mut pred: impl FnMut(&TuiApplication) -> bool,
) -> bool {
    let deadline = tokio::time::Instant::now() + timeout;
    while tokio::time::Instant::now() < deadline {
        app.poll_updates();
        if forbid_thinking {
            assert_ne!(
                app.model.activity_kind,
                ActivityKind::Thinking,
                "slash command must never enter Thinking"
            );
            assert!(
                app.model
                    .activity_message
                    .as_deref()
                    .is_none_or(|m| !m.contains("Reasoning on task plan")),
                "slash command must never show model reasoning text"
            );
        }
        if pred(app) {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    app.poll_updates();
    pred(app)
}

fn has_command_output(app: &TuiApplication, needle: &str) -> bool {
    app.model.conversation.iter().any(|item| match item {
        TuiConversationItem::System { text, .. } => text.contains(needle),
        TuiConversationItem::Assistant { text, .. } => text.contains(needle),
        _ => false,
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// §26: initial session state
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_initial_session_state_is_idle_ready() {
    let model = TuiViewModel::new();
    assert_eq!(model.session_status, "idle");
    assert_eq!(model.operation_state(), UiOperationState::Idle);
    assert!(!model.is_semantically_active());
    assert_eq!(model.execution_summary(), None);
    assert_eq!(model.activity_message, None);
    assert_eq!(model.live_activity, None);
    assert_eq!(model.active_stream_message_id, None);
    assert_eq!(model.active_request_id, None);

    let header = header_text(&model, 100, 3);
    assert!(
        header.contains("Ready"),
        "fresh header must say Ready: {header}"
    );
    assert!(
        !header.contains("Working"),
        "fresh header must not say Working"
    );
    assert!(
        !header.contains("Thinking"),
        "fresh header must not say Thinking"
    );
}

#[test]
fn test_session_started_does_not_mean_working() {
    let mut model = TuiViewModel::new();
    model.apply_interaction_event(&InteractionEvent::SessionStarted {
        session_id: m31a::ids::SessionId::from(uuid::Uuid::now_v7()),
    });
    assert_eq!(model.session_status, "active");
    // A live session with no in-flight operation is Idle — never Working.
    assert_eq!(model.operation_state(), UiOperationState::Idle);
    assert!(!model.is_semantically_active());
    assert_eq!(model.execution_summary(), None);

    let header = header_text(&model, 100, 3);
    assert!(header.contains("Ready"), "header must say Ready: {header}");
    assert!(!header.contains("Working"));
}

// ─────────────────────────────────────────────────────────────────────────────
// §27: commands do not enter thinking state (TUI-level matrix)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_readonly_slash_commands_never_enter_thinking() {
    // Commands servable without a bridge stay Idle; commands requiring the
    // governed runtime fail EXPLICITLY — but none may enter Thinking.
    let idle_locally = ["/help", "/tools", "/tasks", "/agents", "/doctor"];
    for cmd in [
        "/help", "/status", "/config", "/model", "/profile", "/tools", "/skills", "/doctor",
        "/diff", "/tasks", "/agents",
    ] {
        let mut app = TuiApplication::new().with_composer_focused(true);
        submit(&mut app, cmd);
        assert_ne!(
            app.model.activity_kind,
            ActivityKind::Thinking,
            "{cmd} must not enter Thinking on submit"
        );
        assert_eq!(
            app.model.live_activity, None,
            "{cmd} must not set live activity"
        );
        assert!(
            app.model
                .activity_message
                .as_deref()
                .is_none_or(|m| !m.contains("Reasoning")),
            "{cmd} must not show reasoning text"
        );
        if idle_locally.contains(&cmd) {
            assert_eq!(
                app.model.operation_state(),
                UiOperationState::Idle,
                "{cmd} must leave operation state Idle"
            );
        } else {
            // Bridge-dependent commands without a bridge: explicit error card,
            // settled request, Failed activity — never silent Thinking.
            assert_eq!(
                app.model.activity_kind,
                ActivityKind::Failed,
                "{cmd} without bridge must fail explicitly"
            );
            assert!(
                app.model
                    .conversation
                    .iter()
                    .any(|i| matches!(i, TuiConversationItem::Error { .. }))
            );
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// §28: input routing parity matrix
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_input_routing_parity_slash_vs_text() {
    let parser = InteractionParser::default();
    let ws = std::path::Path::new(".");
    let route = |input: &str| {
        parser
            .parse(input, ws, SessionPromptState::Idle, None, false)
            .expect("parse must succeed")
    };
    // Slash commands route to command actions — never to UserTextSubmitted.
    for cmd in [
        "/help", "/config", "/model", "/profile", "/doctor", "/tasks", "/agents", "/tools",
        "/skills",
    ] {
        assert!(
            !matches!(route(cmd), ApplicationAction::UserTextSubmitted(_)),
            "{cmd} must not route as autonomous coding prompt"
        );
    }
    assert_eq!(route("/status"), ApplicationAction::StatusRequested);
    assert_eq!(route("/diff"), ApplicationAction::DiffRequested);
    // Natural language routes to UserTextSubmitted (with mention resolution).
    match route("study the codebase") {
        ApplicationAction::UserTextSubmitted(parsed) => {
            assert_eq!(parsed.raw_text, "study the codebase");
        }
        other => panic!("plain text must be UserTextSubmitted, got {other:?}"),
    }
    match route("fix the parser in @src/parser.rs") {
        ApplicationAction::UserTextSubmitted(parsed) => {
            assert_eq!(parsed.mentions.len(), 1);
        }
        other => panic!("mention text must be UserTextSubmitted, got {other:?}"),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Request correlation ids (§9)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_request_ids_open_and_settle() {
    let mut model = TuiViewModel::new();
    let id = model.begin_request("Working on your request…");
    assert_eq!(model.active_request_id.as_deref(), Some(id.as_str()));
    assert_eq!(model.activity_kind, ActivityKind::Thinking);
    model.settle_request();
    assert_eq!(model.active_request_id, None);
    assert_eq!(model.last_settled_request_id.as_deref(), Some(id.as_str()));
    assert_eq!(model.operation_state(), UiOperationState::Idle);

    let silent = model.begin_request_silent();
    assert_eq!(model.active_request_id.as_deref(), Some(silent.as_str()));
    assert_eq!(model.operation_state(), UiOperationState::Idle);
    model.fail_request("boom");
    assert_eq!(model.active_request_id, None);
    assert_eq!(model.operation_state(), UiOperationState::Failed);
}

#[test]
fn test_command_lifecycle_dispatched_running_output_idle() {
    // Command execution is its own state machine, distinct from the
    // model-thinking lifecycle: dispatched → running → output → idle.
    let mut model = TuiViewModel::new();
    let id = model.begin_request_silent();
    assert_eq!(model.active_request_id.as_deref(), Some(id.as_str()));
    assert_eq!(model.operation_state(), UiOperationState::Idle);

    // Only genuinely long-running commands opt into visible activity, and
    // even then as command-specific state — never model reasoning.
    model.begin_command_running("doctor");
    assert_eq!(
        model.operation_state(),
        UiOperationState::CommandRunning {
            command: "doctor".to_string()
        }
    );
    assert_ne!(model.activity_kind, ActivityKind::Thinking);

    model.apply_interaction_event(&InteractionEvent::CommandOutput {
        text: "diagnostics ok".to_string(),
    });
    assert_eq!(model.operation_state(), UiOperationState::Idle);
    assert_eq!(model.active_request_id, None);
    assert_eq!(model.active_command, None);
}

#[tokio::test]
async fn test_nl_submit_stamps_request_id() {
    let (_dir, runtime) = setup_runtime(false).await;
    let (mut app, _handle) = bridge_app(runtime).await;
    submit(&mut app, "study the codebase");
    assert_eq!(app.model.activity_kind, ActivityKind::Thinking);
    assert!(
        app.model.active_request_id.is_some(),
        "NL submit must open a correlated request"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// §23: end-to-end slash commands through the real bridge
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_e2e_slash_help_behaves_like_a_command() {
    let (_dir, runtime) = setup_runtime(false).await;
    let (mut app, _handle) = bridge_app(runtime).await;

    submit(&mut app, "/help");
    // Immediately after key handling: no thinking state.
    assert_eq!(app.model.activity_kind, ActivityKind::Idle);
    assert_eq!(app.model.live_activity, None);

    let arrived = pump_until(&mut app, Duration::from_secs(10), true, |a| {
        has_command_output(a, "Available commands")
    })
    .await;
    assert!(arrived, "/help must produce command output");
    // No stale model-thinking state remains once the result arrived.
    assert_eq!(app.model.activity_kind, ActivityKind::Idle);
    assert_eq!(app.model.live_activity, None);
    assert_eq!(app.model.operation_state(), UiOperationState::Idle);
    assert_eq!(app.model.active_request_id, None);
    assert!(!app.model.is_semantically_active());

    // Command output is a first-class scrollable conversation event: the head
    // is reachable via Home, the tail sits at the followed bottom.
    // (Viewport Home/End live in navigation mode; Esc leaves the composer.
    // A second Esc is harmless if the first only closed autocomplete.
    // One frame renders first so viewport geometry is measured — exactly
    // like the production tick loop, which always renders before polling.)
    let _ = buffer_text(&mut app, 100, 30);
    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::empty()));
    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::empty()));
    assert!(!app.is_composer_focused);
    app.handle_key(KeyEvent::from(KeyCode::Home));
    let top = buffer_text(&mut app, 100, 30);
    assert!(
        top.contains("Available commands"),
        "head scrollable:\n{top}"
    );
    app.handle_key(KeyEvent::from(KeyCode::End));
    assert!(app.model.follow);
    let bottom = buffer_text(&mut app, 100, 30);
    assert!(!bottom.contains("Reasoning on task plan"));
}

#[tokio::test]
async fn test_e2e_slash_status_and_config_settle_to_idle() {
    let (_dir, runtime) = setup_runtime(false).await;
    let (mut app, _handle) = bridge_app(runtime).await;

    submit(&mut app, "/status");
    let arrived = pump_until(&mut app, Duration::from_secs(10), true, |a| {
        has_command_output(a, "Session")
    })
    .await;
    assert!(arrived, "/status must produce command output");
    assert_eq!(app.model.operation_state(), UiOperationState::Idle);

    submit(&mut app, "/config");
    let arrived = pump_until(&mut app, Duration::from_secs(10), true, |a| {
        has_command_output(a, "Workspace") || has_command_output(a, "Model")
    })
    .await;
    assert!(arrived, "/config must produce command output");
    assert_eq!(app.model.operation_state(), UiOperationState::Idle);
    assert_eq!(app.model.active_request_id, None);
}

// ─────────────────────────────────────────────────────────────────────────────
// §24: end-to-end natural-language prompt through the real bridge
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_e2e_nl_prompt_produces_real_runtime_outcome() {
    let (_dir, runtime) = setup_runtime(true).await;
    let (mut app, _handle) = bridge_app(runtime).await;

    submit(&mut app, "study the codebase");
    assert_eq!(app.model.activity_kind, ActivityKind::Thinking);

    // One valid terminal outcome must arrive through the real bridge path.
    let arrived = pump_until(&mut app, Duration::from_secs(60), false, |a| {
        a.model.conversation.iter().any(|item| {
            matches!(
                item,
                TuiConversationItem::Discovery { .. }
                    | TuiConversationItem::PlanReview { .. }
                    | TuiConversationItem::TaskReview { .. }
                    | TuiConversationItem::AuthRequired { .. }
                    | TuiConversationItem::Assistant {
                        streaming: false,
                        ..
                    }
                    | TuiConversationItem::Error { .. }
                    | TuiConversationItem::Failure { .. }
            )
        })
    })
    .await;
    assert!(
        arrived,
        "NL prompt must produce a real runtime outcome (discovery/plan/response/error)"
    );
    // The UI must NOT be permanently stuck in Thinking.
    assert_ne!(
        app.model.activity_kind,
        ActivityKind::Thinking,
        "activity must reconcile with the returned outcome"
    );
    assert_eq!(
        app.model.operation_state(),
        UiOperationState::AwaitingInput,
        "deterministic discovery must surface as awaiting input"
    );
    let screen = buffer_text(&mut app, 100, 30);
    assert!(
        screen.contains("Input needed") || screen.contains("M31A"),
        "lifecycle result must render, got:\n{screen}"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// §30: every terminal event reconciles activity
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_terminal_events_clear_stale_activity() {
    let mut model = TuiViewModel::new();

    model.begin_request("Working on your request…");
    model.apply_interaction_event(&InteractionEvent::CommandOutput {
        text: "Session\n  active".to_string(),
    });
    assert_eq!(model.operation_state(), UiOperationState::Idle);
    assert_eq!(model.live_activity, None);

    // A completed tool must not leave "Running tool…" behind.
    model.begin_request("Working on your request…");
    model.apply_interaction_event(&InteractionEvent::ToolStarted {
        call_id: "c1".to_string(),
        tool_name: "fs_read".to_string(),
        parameters: serde_json::json!({}),
    });
    assert_eq!(model.operation_state(), UiOperationState::RunningTool);
    model.apply_interaction_event(&InteractionEvent::ToolCompleted {
        call_id: "c1".to_string(),
        tool_name: "fs_read".to_string(),
        success: true,
        output_preview: "ok".to_string(),
    });
    assert_eq!(model.live_activity, None);
    assert!(!model.operation_state().is_working());

    // Errors fail explicitly and settle the request.
    model.begin_request("Working on your request…");
    model.apply_interaction_event(&InteractionEvent::Error {
        message: "boom".to_string(),
    });
    assert_eq!(model.operation_state(), UiOperationState::Failed);
    assert_eq!(model.active_request_id, None);
    assert!(model.conversation.iter().any(|i| matches!(
        i,
        TuiConversationItem::Error { message, .. } if message == "boom"
    )));
}

// ─────────────────────────────────────────────────────────────────────────────
// §11: assistant streaming lifecycle
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_assistant_streaming_lifecycle_no_duplicates() {
    let mut model = TuiViewModel::new();
    let mid = "m1".to_string();

    model.apply_interaction_event(&InteractionEvent::AssistantStarted {
        message_id: mid.clone(),
    });
    assert_eq!(model.operation_state(), UiOperationState::Thinking);
    let items_after_start = model.conversation.len();

    for delta in ["hello", " world"] {
        model.apply_interaction_event(&InteractionEvent::AssistantDelta {
            message_id: mid.clone(),
            delta: delta.to_string(),
        });
    }
    // Deltas append to the same item — no new items.
    assert_eq!(model.conversation.len(), items_after_start);

    model.apply_interaction_event(&InteractionEvent::AssistantFinished {
        message_id: mid.clone(),
    });
    assert_eq!(model.operation_state(), UiOperationState::Idle);
    assert_eq!(model.active_stream_message_id, None);

    // A final AssistantOutput reconciles with the streamed item (no dup).
    model.apply_interaction_event(&InteractionEvent::AssistantOutput {
        text: "hello world".to_string(),
    });
    let assistant_items = model
        .conversation
        .iter()
        .filter(|i| matches!(i, TuiConversationItem::Assistant { .. }))
        .count();
    assert_eq!(assistant_items, 1);

    // Failure without an item surfaces an explicit error, clears streaming.
    model.apply_interaction_event(&InteractionEvent::AssistantFailed {
        message_id: "m2".to_string(),
        error: "stream cut".to_string(),
    });
    assert_eq!(model.operation_state(), UiOperationState::Failed);
    assert_eq!(model.active_stream_message_id, None);
    assert!(model.conversation.iter().any(|i| matches!(
        i,
        TuiConversationItem::Error { message, .. } if message.contains("stream cut")
    )));
}

// ─────────────────────────────────────────────────────────────────────────────
// No-bridge submissions fail explicitly (§8)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_submit_without_bridge_fails_explicitly_not_silently() {
    let mut app = TuiApplication::new().with_composer_focused(true);
    assert!(app.bridge_tx.is_none());

    submit(&mut app, "study the codebase");
    // The optimistic Thinking state is revoked; the error card is visible.
    assert_ne!(app.model.activity_kind, ActivityKind::Thinking);
    assert!(
        app.model
            .conversation
            .iter()
            .any(|i| matches!(i, TuiConversationItem::Error { .. }))
    );

    submit(&mut app, "/config");
    // Bridge-dependent command without a bridge: explicit failure, not Idle
    // pretending, and never Thinking.
    assert_eq!(app.model.activity_kind, ActivityKind::Failed);
    assert!(app.model.conversation.iter().any(|i| matches!(
        i,
        TuiConversationItem::Error { message, .. } if message.contains("/config")
    )));
}

// ─────────────────────────────────────────────────────────────────────────────
// §25: scrolling viewport end to end
// ─────────────────────────────────────────────────────────────────────────────

fn conversation_app(items: usize) -> TuiApplication {
    let mut app = TuiApplication::new();
    for i in 0..items {
        app.model.add_conversation_item(TuiConversationItem::User {
            id: format!("u{i}"),
            sequence: i as u64 * 2,
            text: format!("user message number {i} with enough words to wrap lines"),
            mentions: Vec::new(),
            timestamp: chrono::Utc::now(),
        });
        app.model
            .add_conversation_item(TuiConversationItem::Assistant {
                id: format!("a{i}"),
                sequence: i as u64 * 2 + 1,
                text: format!("assistant response number {i} also carrying enough words to wrap"),
                streaming: false,
                timestamp: chrono::Utc::now(),
            });
    }
    app
}

#[test]
fn test_scrolling_viewport_follow_and_clamping() {
    let mut app = conversation_app(30);
    // Establish real viewport geometry at a small terminal.
    let _ = buffer_text(&mut app, 100, 20);
    assert!(app.model.follow, "initial state follows");
    assert_eq!(app.model.scroll_offset, 0);
    assert!(app.model.max_scroll() > 0, "content must overflow 20 rows");

    let page = app.model.page_step();
    assert!(page > 1, "page step must be viewport-sized, got {page}");

    // PageUp leaves follow mode and moves by a viewport amount.
    app.handle_key(KeyEvent::from(KeyCode::PageUp));
    assert_eq!(app.model.scroll_offset, page.min(app.model.max_scroll()));
    assert!(!app.model.follow);

    // Home jumps to the top; End jumps back to the bottom + follow.
    app.handle_key(KeyEvent::from(KeyCode::Home));
    assert_eq!(app.model.scroll_offset, app.model.max_scroll());
    app.handle_key(KeyEvent::from(KeyCode::End));
    assert_eq!(app.model.scroll_offset, 0);
    assert!(app.model.follow);
    assert_eq!(app.model.unseen_count, 0);

    // New messages while reading old content do NOT yank the viewport:
    // the offset-from-top position is preserved and unseen counts grow.
    app.handle_key(KeyEvent::from(KeyCode::PageUp));
    let _ = buffer_text(&mut app, 100, 20);
    let top_before = app.model.max_scroll() - app.model.scroll_offset;
    app.model
        .add_conversation_item(TuiConversationItem::Assistant {
            id: "new1".to_string(),
            sequence: 999,
            text: "fresh arrival while reading history".to_string(),
            streaming: false,
            timestamp: chrono::Utc::now(),
        });
    let _ = buffer_text(&mut app, 100, 20);
    let top_after = app.model.max_scroll() - app.model.scroll_offset;
    assert_eq!(top_before, top_after, "viewport must be preserved");
    assert!(!app.model.follow);
    assert_eq!(app.model.unseen_count, 1);
    // A subtle indicator surfaces the arrival without yanking the viewport.
    let reading = buffer_text(&mut app, 100, 20);
    assert!(
        reading.contains("new · End to follow"),
        "unseen indicator required, got:\n{reading}"
    );

    // Returning to bottom restores follow mode.
    app.handle_key(KeyEvent::from(KeyCode::End));
    assert!(app.model.follow);
    assert_eq!(app.model.unseen_count, 0);

    // Offset can never exceed the real content height.
    for _ in 0..200 {
        app.model.scroll_up(50);
    }
    assert!(app.model.scroll_offset <= app.model.max_scroll());

    // Scrollbar + position feedback render from real geometry.
    app.handle_key(KeyEvent::from(KeyCode::PageUp));
    let screen = buffer_text(&mut app, 100, 20);
    assert!(
        screen.contains("█") || screen.contains("│"),
        "scrollbar must render when overflowing"
    );
}

#[test]
fn test_scrolling_framebuffer_top_vs_bottom_differ() {
    let mut app = conversation_app(30);
    app.handle_key(KeyEvent::from(KeyCode::End));
    let bottom = buffer_text(&mut app, 100, 20);
    app.handle_key(KeyEvent::from(KeyCode::Home));
    let top = buffer_text(&mut app, 100, 20);
    assert_ne!(top, bottom, "top and bottom viewports must differ");
    assert!(
        top.contains("user message number 0"),
        "top must show oldest content"
    );
    assert!(
        bottom.contains("assistant response number 29"),
        "bottom must show newest content"
    );
}

#[test]
fn test_mouse_wheel_scrolls_viewport() {
    let mut app = conversation_app(30);
    let _ = buffer_text(&mut app, 100, 20);
    assert!(app.model.follow);
    app.handle_mouse_scroll(true);
    assert!(!app.model.follow || app.model.max_scroll() == 0);
    assert!(app.model.scroll_offset > 0);
    app.handle_mouse_scroll(false);
    app.handle_mouse_scroll(false);
    app.handle_mouse_scroll(false);
}

// ─────────────────────────────────────────────────────────────────────────────
// §20: command output is width-safe at every required size
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_command_output_width_safe_at_all_sizes() {
    let long_token = "x".repeat(200);
    let long_path =
        format!("/very/long/workspace/path/segment/{long_token}/model.nemotron-3-instruct");
    let mut model = TuiViewModel::new();
    model.add_conversation_item(TuiConversationItem::System {
        text: format!(
            "Available commands\n\n/help\n  Display available commands and detailed usage.\n  Model\n  {long_path}\n  Token\n  {long_token}"
        ),
        timestamp: chrono::Utc::now(),
    });
    for (w, h) in [(80u16, 24u16), (96, 24), (100, 30), (120, 30), (160, 40)] {
        let backend = TestBackend::new(w, h);
        let mut terminal = Terminal::new(backend).unwrap();
        let tokens = m31a::tui::theme::ThemeTokens::resolve(m31a::tui::theme::ThemeMode::Default);
        terminal
            .draw(|f| {
                m31a::tui::surface::render_conversation_surface(
                    f,
                    f.area(),
                    &mut model,
                    &tokens,
                    false,
                );
            })
            .unwrap();
        let buffer = terminal.backend().buffer().clone();
        for y in 0..buffer.area.height {
            let width: usize = (0..buffer.area.width)
                .map(|x| buffer[(x, y)].symbol().chars().count())
                .sum();
            assert!(
                width <= w as usize,
                "row {y} overflows {w} columns at {w}x{h}"
            );
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Final acceptance journey (§35): header + slash + prompt + scroll
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_acceptance_journey_ready_slash_prompt_scroll() {
    let (_dir, runtime) = setup_runtime(true).await;
    let (mut app, _handle) = bridge_app(runtime).await;

    // START: Ready, not Working.
    let header = header_text(&app.model, 100, 3);
    assert!(header.contains("Ready"), "start header: {header}");
    assert!(!header.contains("Working"));

    // /help → command output, no reasoning.
    submit(&mut app, "/help");
    let ok = pump_until(&mut app, Duration::from_secs(10), true, |a| {
        has_command_output(a, "Available commands")
    })
    .await;
    assert!(ok);

    // NL prompt → real lifecycle outcome, never stuck reasoning.
    submit(&mut app, "study the codebase");
    let ok = pump_until(&mut app, Duration::from_secs(60), false, |a| {
        a.model.conversation.iter().any(|item| {
            matches!(
                item,
                TuiConversationItem::Discovery { .. }
                    | TuiConversationItem::PlanReview { .. }
                    | TuiConversationItem::Assistant { .. }
                    | TuiConversationItem::Error { .. }
                    | TuiConversationItem::Failure { .. }
            )
        })
    })
    .await;
    assert!(ok, "prompt must yield a visible response");
    assert_ne!(app.model.activity_kind, ActivityKind::Thinking);

    // Overflow the viewport → scroll system works with visible feedback.
    for i in 0..25 {
        app.model
            .add_conversation_item(TuiConversationItem::Assistant {
                id: format!("fill{i}"),
                sequence: 1000 + i,
                text: format!(
                    "filler response {i} padding the conversation past the viewport edge"
                ),
                streaming: false,
                timestamp: chrono::Utc::now(),
            });
    }
    let _ = buffer_text(&mut app, 100, 20);
    assert!(app.model.max_scroll() > 0);
    app.handle_key(KeyEvent::from(KeyCode::PageUp));
    assert!(!app.model.follow);
    let screen = buffer_text(&mut app, 100, 20);
    assert!(
        screen.contains("█") || screen.contains("│"),
        "scrollbar feedback required"
    );
    // Viewport Home/End live in navigation mode (composer Home/End keep
    // editor line semantics per §16): Esc out, End returns to follow.
    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::empty()));
    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::empty()));
    app.handle_key(KeyEvent::from(KeyCode::End));
    assert!(app.model.follow);
}
