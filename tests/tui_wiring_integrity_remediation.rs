//! TUI Wiring Integrity & Runtime Consistency Remediation Tests (F01–F08).
//!
//! Authoritative regression suite verifying:
//! - F01: Command palette execution contract (governed dispatch, bridge absence, unsupported commands)
//! - F02: Replay isolation from live presentation model
//! - F03: Truthful task agent identity, role, and title projection
//! - F04: Session-scoped projection invalidation and delayed hydration rejection
//! - F05: Truthful Git branch projection (no fabricated master fallback)
//! - F06: Unicode-safe text truncation across wide/combining/emoji characters
//! - F07: Model usage token and cost hydration idempotency and deduplication
//! - F08: Modal keyboard ownership and composer insulation

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Terminal;
use ratatui::backend::TestBackend;

use m31a::cli::dispatch::RuntimeCommand;
use m31a::events::envelope::EventEnvelope;
use m31a::events::types::{EventType, TaskSummary};
use m31a::ids::{SessionId, TaskId};
use m31a::interaction::action::ApplicationAction;
use m31a::interaction::events::InteractionEvent;
use m31a::model::types::CostProvenance;
use m31a::state_machine::agent::AgentRole;
use m31a::state_machine::task::TaskState;
use m31a::tui::channel::TuiActionSender;
use m31a::tui::navigation::ViewId;
use m31a::tui::palette_v2::{PaletteActionV2, PaletteItemV2};
use m31a::tui::sanitizer::{truncate_str_safe, truncate_to_width};
use m31a::tui::{KeyAction, TuiApplication};

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn render_to_string(app: &mut TuiApplication, w: u16, h: u16) -> String {
    let backend = TestBackend::new(w, h);
    let mut terminal = Terminal::new(backend).unwrap();
    app.force_redraw = true;
    app.render_frame(&mut terminal).unwrap();
    terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|c| c.symbol())
        .collect()
}

// =========================================================================
// F01: Command palette command execution contract
// =========================================================================

#[test]
fn test_f01_supported_palette_commands_dispatch_through_governed_bridge() {
    let mut app = TuiApplication::new();
    let (tx, mut rx) = tokio::sync::mpsc::channel(32);
    app.bridge_tx = Some(TuiActionSender::Bounded(tx));
    app.model.mission_id = Some("mission-42".to_string());

    // 1. Pause mission
    app.palette.open();
    app.palette.clear_items();
    app.palette.add_item(PaletteItemV2 {
        id: "pause".to_string(),
        category: "Mission",
        label: "Pause".to_string(),
        detail: None,
        shortcut: None,
        action: PaletteActionV2::ExecuteCommand(RuntimeCommand::PauseMission {
            id: "current".to_string(),
        }),
    });
    app.palette.set_selected_index(0);
    let res = app.handle_key(key(KeyCode::Enter));
    assert!(
        res.is_none(),
        "Palette command must not return for second dispatch"
    );
    let dispatched = rx
        .try_recv()
        .expect("Action must be received on bridge channel");
    match dispatched {
        ApplicationAction::MissionPauseRequested { mission_id } => {
            assert_eq!(mission_id, "mission-42");
        }
        other => panic!("Unexpected action: {other:?}"),
    }

    // 2. Resume mission
    app.palette.open();
    app.palette.clear_items();
    app.palette.add_item(PaletteItemV2 {
        id: "resume".to_string(),
        category: "Mission",
        label: "Resume".to_string(),
        detail: None,
        shortcut: None,
        action: PaletteActionV2::ExecuteCommand(RuntimeCommand::ResumeMission {
            id: "current".to_string(),
        }),
    });
    app.palette.set_selected_index(0);
    let res = app.handle_key(key(KeyCode::Enter));
    assert!(res.is_none());
    let dispatched = rx
        .try_recv()
        .expect("Action must be received on bridge channel");
    match dispatched {
        ApplicationAction::MissionResumeRequested { mission_id } => {
            assert_eq!(mission_id, "mission-42");
        }
        other => panic!("Unexpected action: {other:?}"),
    }

    // 3. Cancel mission
    app.palette.open();
    app.palette.clear_items();
    app.palette.add_item(PaletteItemV2 {
        id: "cancel".to_string(),
        category: "Mission",
        label: "Cancel".to_string(),
        detail: None,
        shortcut: None,
        action: PaletteActionV2::ExecuteCommand(RuntimeCommand::CancelMission {
            id: "current".to_string(),
            reason: None,
        }),
    });
    app.palette.set_selected_index(0);
    let res = app.handle_key(key(KeyCode::Enter));
    assert!(res.is_none());
    let dispatched = rx
        .try_recv()
        .expect("Action must be received on bridge channel");
    assert!(matches!(dispatched, ApplicationAction::CancelRequested));
}

#[test]
fn test_f01_palette_command_without_bridge_fails_closed_visibly() {
    let mut app = TuiApplication::new();
    app.bridge_tx = None; // No runtime bridge

    app.palette.open();
    app.palette.clear_items();
    app.palette.add_item(PaletteItemV2 {
        id: "pause".to_string(),
        category: "Mission",
        label: "Pause".to_string(),
        detail: None,
        shortcut: None,
        action: PaletteActionV2::ExecuteCommand(RuntimeCommand::PauseMission {
            id: "m-1".to_string(),
        }),
    });
    app.palette.set_selected_index(0);
    let res = app.handle_key(key(KeyCode::Enter));
    assert!(res.is_none(), "Must not return command when bridge absent");

    // Must visibly fail into conversation items and error state
    let has_error = app.model.conversation.iter().any(|item| match item {
        m31a::tui::conversation::TuiConversationItem::Error { message, .. } => {
            message.contains("governed runtime bridge unavailable")
        }
        _ => false,
    });
    assert!(
        has_error,
        "Must surface bridge unavailability as a visible conversation error"
    );
}

#[test]
fn test_f01_unsupported_palette_command_produces_explicit_error() {
    let mut app = TuiApplication::new();
    let (tx, _rx) = tokio::sync::mpsc::channel(32);
    app.bridge_tx = Some(TuiActionSender::Bounded(tx));

    app.palette.open();
    app.palette.clear_items();
    app.palette.add_item(PaletteItemV2 {
        id: "unsupported".to_string(),
        category: "Test",
        label: "Unsupported".to_string(),
        detail: None,
        shortcut: None,
        action: PaletteActionV2::ExecuteCommand(RuntimeCommand::ListCheckpoints {
            mission_id: None,
        }),
    });
    app.palette.set_selected_index(0);
    let res = app.handle_key(key(KeyCode::Enter));
    assert!(
        res.is_none(),
        "Must not discard command to unhandled return"
    );

    let has_error = app.model.conversation.iter().any(|item| match item {
        m31a::tui::conversation::TuiConversationItem::Error { message, .. } => {
            message.contains("not supported from the cockpit palette")
        }
        _ => false,
    });
    assert!(
        has_error,
        "Unsupported palette command must produce an explicit visible error"
    );
}

// =========================================================================
// F02: Replay isolation from live presentation model
// =========================================================================

#[test]
fn test_f02_replay_isolation_preserves_live_state_and_restores_on_exit() {
    let mut app = TuiApplication::new();
    app.model.session_id = Some("live-session-001".to_string());
    app.model.git_branch = "feature/live-branch".to_string();
    app.model.model_usage.prompt_tokens = 4500;
    app.model.model_usage.completion_tokens = 1500;
    app.model.model_usage.api_calls = 5;
    app.model.model_usage.total_cost_cents = Some(80);
    app.model
        .add_conversation_item(m31a::tui::conversation::TuiConversationItem::User {
            id: uuid::Uuid::now_v7().to_string(),
            sequence: 1,
            text: "Live query".to_string(),
            mentions: Vec::new(),
            timestamp: chrono::Utc::now(),
        });

    // Create historical event timeline
    let historical_events = vec![EventEnvelope::new(
        1,
        None,
        None,
        "test".to_string(),
        EventType::MissionStarted {
            mission_id: m31a::ids::MissionId::new(),
            objective: "Historical Mission 1".to_string(),
        },
    )];

    // Enter replay mode
    app.enter_replay(historical_events);
    assert!(app.replay.is_active, "Replay must be active");
    assert!(app.replay_model.is_some(), "Replay model must be populated");

    // Verify live model is completely preserved
    assert_eq!(app.model.session_id.as_deref(), Some("live-session-001"));
    assert_eq!(app.model.git_branch, "feature/live-branch");
    assert_eq!(app.model.model_usage.prompt_tokens, 4500);

    // Render during replay: must show replay banner
    let frame = render_to_string(&mut app, 120, 30);
    assert!(
        frame.contains("Space play/pause · Esc exit"),
        "Must display replay banner during replay"
    );

    // Scrub navigation
    app.handle_key(key(KeyCode::Char('['))); // Step back
    app.handle_key(key(KeyCode::Char(']'))); // Step forward
    app.handle_key(key(KeyCode::Char('2'))); // Fast speed
    assert!(app.replay.is_active);

    // Deliver a live interaction event while in replay
    app.apply_interaction_event(&InteractionEvent::ModelActivity {
        text: "Background live model activity".to_string(),
    });
    // The live model received the event!
    assert_eq!(
        app.model.live_activity.as_deref(),
        Some("Background live model activity")
    );

    // Exit replay mode on Escape
    app.handle_key(key(KeyCode::Esc));
    assert!(!app.replay.is_active, "Replay must be inactive after exit");
    assert!(app.replay_model.is_none(), "Replay model must be cleared");

    // Confirm live model values are intact
    assert_eq!(app.model.session_id.as_deref(), Some("live-session-001"));
    assert_eq!(app.model.git_branch, "feature/live-branch");
    assert_eq!(app.model.model_usage.prompt_tokens, 4500);
    assert_eq!(app.model.model_usage.total_cost_cents, Some(80));
    assert_eq!(
        app.model.live_activity.as_deref(),
        Some("Background live model activity")
    );

    // Render after exit: must NOT have replay banner
    let frame_after = render_to_string(&mut app, 120, 30);
    assert!(
        !frame_after.contains("Space play/pause · Esc exit"),
        "Must not display replay banner after exit"
    );
}

#[test]
fn test_f02_empty_history_replay_lifecycle() {
    let mut app = TuiApplication::new();
    app.enter_replay(Vec::new());
    assert!(app.replay.is_active);
    assert!(app.replay_model.is_some());

    // Scrubbing empty history does not panic
    app.handle_key(key(KeyCode::Char('[')));
    app.handle_key(key(KeyCode::Char(']')));

    app.exit_replay();
    assert!(!app.replay.is_active);
    assert!(app.replay_model.is_none());
}

// =========================================================================
// F03: Truthful task agent identity, role, and title projection
// =========================================================================

#[test]
fn test_f03_task_started_before_materialization_truthful_role() {
    let mut app = TuiApplication::new();
    let task_id = TaskId::new();
    let task_id_str = task_id.to_string();

    // Task starts before graph materialization
    app.apply_interaction_event(&InteractionEvent::TaskStarted {
        mission_id: "m-1".to_string(),
        task_id: task_id_str.clone(),
        agent_id: "agent-special-id".to_string(),
    });

    let task = app
        .model
        .tasks
        .iter()
        .find(|t| t.id == task_id_str)
        .unwrap();
    assert_eq!(task.assigned_agent_id.as_deref(), Some("agent-special-id"));
    assert_eq!(
        task.agent_role, None,
        "Task agent_role must NOT be fabricated from agent_id"
    );
    assert_eq!(task.status, "running");

    // Authoritative materialization arrives
    let summary = TaskSummary {
        id: task_id,
        title: "Build release binary".to_string(),
        role: AgentRole::implementer(),
        status: TaskState::Running,
        dependencies: vec![],
    };
    app.apply_interaction_event(&InteractionEvent::TasksMaterialized {
        graph_id: "graph-1".to_string(),
        revision: 1,
        tasks: vec![summary],
        session_id: None,
    });

    let updated = app
        .model
        .tasks
        .iter()
        .find(|t| t.id == task_id_str)
        .unwrap();
    assert_eq!(
        updated.title, "Build release binary",
        "Title must update to authoritative title"
    );
    assert_eq!(
        updated.agent_role.as_deref(),
        Some("implementer"),
        "Role must update to authoritative role"
    );
    assert_eq!(
        updated.assigned_agent_id.as_deref(),
        Some("agent-special-id"),
        "Assigned agent must be preserved"
    );
    assert_eq!(
        updated.status, "running",
        "Live running status must be preserved"
    );
}

#[test]
fn test_f03_materialization_before_task_started() {
    let mut app = TuiApplication::new();
    let task_id = TaskId::new();
    let task_id_str = task_id.to_string();

    let summary = TaskSummary {
        id: task_id,
        title: "Run security audit".to_string(),
        role: AgentRole::auditor(),
        status: TaskState::Ready,
        dependencies: vec![],
    };
    app.apply_interaction_event(&InteractionEvent::TasksMaterialized {
        graph_id: "graph-1".to_string(),
        revision: 1,
        tasks: vec![summary],
        session_id: None,
    });

    let task = &app.model.tasks[0];
    assert_eq!(task.agent_role.as_deref(), Some("auditor"));
    assert_eq!(task.assigned_agent_id, None);

    // Task starts
    app.apply_interaction_event(&InteractionEvent::TaskStarted {
        mission_id: "m-1".to_string(),
        task_id: task_id_str,
        agent_id: "agent-sec-1".to_string(),
    });

    let started = &app.model.tasks[0];
    assert_eq!(
        started.agent_role.as_deref(),
        Some("auditor"),
        "Role must remain auditor"
    );
    assert_eq!(
        started.assigned_agent_id.as_deref(),
        Some("agent-sec-1"),
        "Assigned agent must be recorded"
    );
    assert_eq!(started.status, "running");
}

// =========================================================================
// F04: Reset session-scoped projection on session change
// =========================================================================

#[test]
fn test_f04_session_change_resets_tasks_and_rejects_delayed_hydration() {
    let mut app = TuiApplication::new();
    let sid_a = SessionId::new();
    let sid_b = SessionId::new();
    let task_a1 = TaskId::new();
    let task_a2 = TaskId::new();
    let task_b1 = TaskId::new();

    // 1. Session A starts and receives tasks
    app.apply_interaction_event(&InteractionEvent::SessionStarted { session_id: sid_a });
    app.apply_interaction_event(&InteractionEvent::TasksMaterialized {
        graph_id: "graph-a".to_string(),
        revision: 1,
        tasks: vec![TaskSummary {
            id: task_a1,
            title: "Task in Session A".to_string(),
            role: AgentRole::architect(),
            status: TaskState::Ready,
            dependencies: vec![],
        }],
        session_id: Some(sid_a.to_string()),
    });
    assert_eq!(app.model.tasks.len(), 1);
    assert_eq!(app.model.tasks[0].id, task_a1.to_string());

    // 2. Switch to Session B
    app.apply_interaction_event(&InteractionEvent::SessionResumed { session_id: sid_b });
    assert!(
        app.model.tasks.is_empty(),
        "Switching to Session B must immediately clear Session A's tasks"
    );
    assert_eq!(app.model.active_graph_id, None);

    // 3. Delayed hydration from Session A arrives while Session B is active
    app.apply_interaction_event(&InteractionEvent::TasksMaterialized {
        graph_id: "graph-a".to_string(),
        revision: 2,
        tasks: vec![TaskSummary {
            id: task_a2,
            title: "Delayed Task from Session A".to_string(),
            role: AgentRole::architect(),
            status: TaskState::Ready,
            dependencies: vec![],
        }],
        session_id: Some(sid_a.to_string()),
    });
    assert!(
        app.model.tasks.is_empty(),
        "Delayed hydration from Session A must be rejected while Session B is active"
    );

    // 4. Session B materializes its own authoritative tasks
    app.apply_interaction_event(&InteractionEvent::TasksMaterialized {
        graph_id: "graph-b".to_string(),
        revision: 1,
        tasks: vec![TaskSummary {
            id: task_b1,
            title: "Task in Session B".to_string(),
            role: AgentRole::implementer(),
            status: TaskState::Running,
            dependencies: vec![],
        }],
        session_id: Some(sid_b.to_string()),
    });
    assert_eq!(app.model.tasks.len(), 1);
    assert_eq!(app.model.tasks[0].id, task_b1.to_string());
}

#[test]
fn test_f04_session_b_without_graph_materializes_empty() {
    let mut app = TuiApplication::new();
    let sid_a = SessionId::new();
    let sid_b = SessionId::new();
    let task_a1 = TaskId::new();

    // Session A has tasks
    app.apply_interaction_event(&InteractionEvent::SessionStarted { session_id: sid_a });
    app.apply_interaction_event(&InteractionEvent::TasksMaterialized {
        graph_id: "graph-a".to_string(),
        revision: 1,
        tasks: vec![TaskSummary {
            id: task_a1,
            title: "Task in Session A".to_string(),
            role: AgentRole::architect(),
            status: TaskState::Ready,
            dependencies: vec![],
        }],
        session_id: Some(sid_a.to_string()),
    });

    // Session B resumed, has no graph
    app.apply_interaction_event(&InteractionEvent::SessionResumed { session_id: sid_b });
    app.apply_interaction_event(&InteractionEvent::TasksMaterialized {
        graph_id: String::new(),
        revision: 0,
        tasks: vec![],
        session_id: Some(sid_b.to_string()),
    });
    assert!(app.model.tasks.is_empty(), "Session B must have zero tasks");
}

// =========================================================================
// F05: Truthful Git branch projection (no fabricated master fallback)
// =========================================================================

#[test]
fn test_f05_git_branch_truthful_rendering() {
    let mut app = TuiApplication::new();

    // 1. Empty / unavailable branch renders N/A, never "master"
    app.model.git_branch = "N/A".to_string();
    let out = render_to_string(&mut app, 120, 30);
    assert!(
        !out.contains("Branch: master"),
        "Unavailable Git must NOT fabricate master"
    );

    app.model.git_branch = "".to_string();
    let out_empty = render_to_string(&mut app, 120, 30);
    assert!(
        !out_empty.contains("Branch: master"),
        "Empty branch must NOT fabricate master"
    );

    // 2. Real master branch renders master
    app.model.git_branch = "master".to_string();
    let out_master = render_to_string(&mut app, 120, 30);
    assert!(
        out_master.contains("master"),
        "Authentic master branch must be rendered"
    );

    // 3. Real main branch renders main
    app.model.git_branch = "main".to_string();
    let out_main = render_to_string(&mut app, 120, 30);
    assert!(
        out_main.contains("main"),
        "Authentic main branch must be rendered"
    );

    // 4. Feature branch renders feature branch
    app.model.git_branch = "feature/agent-core".to_string();
    let out_feat = render_to_string(&mut app, 120, 30);
    assert!(out_feat.contains("feature/agent-core"));

    // 5. Execution worktree branch takes precedence if present
    app.model.execution_worktree_branch = Some("m31a/mid-99".to_string());
    let out_exec = render_to_string(&mut app, 120, 30);
    assert!(out_exec.contains("m31a/mid-99"));
}

// =========================================================================
// F06: Unicode-safe text truncation
// =========================================================================

#[test]
fn test_f06_unicode_safe_truncation_no_byte_boundary_panic() {
    // 1. Multibyte characters where arbitrary byte index lands inside character
    let s = "こんにちは世界"; // 3 bytes per Japanese char
    for max in 0..15 {
        let truncated = truncate_str_safe(s, max);
        // Valid UTF-8 string that does not panic
        assert!(std::str::from_utf8(truncated.as_bytes()).is_ok());
    }

    // 2. Emoji with multi-byte representations
    let emoji_s = "🦀🚀✨🎉🔥";
    for max in 0..15 {
        let truncated = truncate_str_safe(emoji_s, max);
        assert!(std::str::from_utf8(truncated.as_bytes()).is_ok());
    }

    // 3. Exact limit behaviors
    assert_eq!(truncate_to_width("abcde", 5), "abcde");
    assert_eq!(truncate_to_width("abcdef", 5), "abcd…");
    assert_eq!(truncate_to_width("a", 1), "a");
    assert_eq!(truncate_to_width("ab", 1), "…");
    assert_eq!(truncate_to_width("abc", 0), "");

    // 4. Wide CJK display columns
    let cjk = "東京"; // 2 cols each = 4 cols
    assert_eq!(truncate_to_width(cjk, 4), "東京");
    assert_eq!(truncate_to_width(cjk, 3), "東…"); // 2 + 1 = 3 cols
}

// =========================================================================
// F07: Model usage token and cost hydration idempotency and deduplication
// =========================================================================

#[test]
fn test_f07_model_usage_and_cost_idempotency() {
    let mut app = TuiApplication::new();

    // 1. One priced invocation
    app.apply_interaction_event(&InteractionEvent::ModelUsageUpdated {
        invocation_id: Some("inv-101".to_string()),
        prompt_tokens: 100,
        completion_tokens: 50,
        total_tokens: 150,
        cost_cents: Some(25),
        cost_usd: Some(0.25),
        cost_provenance: CostProvenance::Authoritative,
    });
    assert_eq!(app.model.model_usage.prompt_tokens, 100);
    assert_eq!(app.model.model_usage.completion_tokens, 50);
    assert_eq!(app.model.model_usage.api_calls, 1);
    assert_eq!(app.model.model_usage.total_cost_cents, Some(25));

    // 2. Duplicate event arrives for inv-101: must NOT double count
    app.apply_interaction_event(&InteractionEvent::ModelUsageUpdated {
        invocation_id: Some("inv-101".to_string()),
        prompt_tokens: 100,
        completion_tokens: 50,
        total_tokens: 150,
        cost_cents: Some(25),
        cost_usd: Some(0.25),
        cost_provenance: CostProvenance::Authoritative,
    });
    assert_eq!(app.model.model_usage.prompt_tokens, 100);
    assert_eq!(app.model.model_usage.completion_tokens, 50);
    assert_eq!(app.model.model_usage.api_calls, 1);
    assert_eq!(app.model.model_usage.total_cost_cents, Some(25));

    // 3. Unpriced invocation arrives (cost_cents: None)
    app.apply_interaction_event(&InteractionEvent::ModelUsageUpdated {
        invocation_id: Some("inv-102".to_string()),
        prompt_tokens: 200,
        completion_tokens: 100,
        total_tokens: 300,
        cost_cents: None,
        cost_usd: None,
        cost_provenance: CostProvenance::Unknown,
    });
    assert_eq!(app.model.model_usage.prompt_tokens, 300);
    assert_eq!(app.model.model_usage.completion_tokens, 150);
    assert_eq!(app.model.model_usage.api_calls, 2);
    // Cost must NOT be reset or converted to 0
    assert_eq!(app.model.model_usage.total_cost_cents, Some(25));

    // 4. Late pricing arrives for inv-102
    app.apply_interaction_event(&InteractionEvent::ModelUsageUpdated {
        invocation_id: Some("inv-102".to_string()),
        prompt_tokens: 200,
        completion_tokens: 100,
        total_tokens: 300,
        cost_cents: Some(30),
        cost_usd: Some(0.30),
        cost_provenance: CostProvenance::Authoritative,
    });
    // Tokens must NOT be double counted, but cost must be applied!
    assert_eq!(app.model.model_usage.prompt_tokens, 300);
    assert_eq!(app.model.model_usage.completion_tokens, 150);
    assert_eq!(app.model.model_usage.api_calls, 2);
    assert_eq!(app.model.model_usage.total_cost_cents, Some(55)); // 25 + 30

    // 5. Zero-cost invocation
    app.apply_interaction_event(&InteractionEvent::ModelUsageUpdated {
        invocation_id: Some("inv-103".to_string()),
        prompt_tokens: 50,
        completion_tokens: 25,
        total_tokens: 75,
        cost_cents: Some(0),
        cost_usd: Some(0.0),
        cost_provenance: CostProvenance::Authoritative,
    });
    assert_eq!(app.model.model_usage.prompt_tokens, 350);
    assert_eq!(app.model.model_usage.completion_tokens, 175);
    assert_eq!(app.model.model_usage.api_calls, 3);
    assert_eq!(app.model.model_usage.total_cost_cents, Some(55));

    // 6. Streaming in-flight chunk then terminal record
    app.apply_interaction_event(&InteractionEvent::ModelUsageUpdated {
        invocation_id: None,
        prompt_tokens: 40,
        completion_tokens: 20,
        total_tokens: 60,
        cost_cents: None,
        cost_usd: None,
        cost_provenance: CostProvenance::Unknown,
    });
    assert_eq!(app.model.model_usage.effective_prompt_tokens(), 390); // 350 + 40
    assert_eq!(app.model.model_usage.effective_completion_tokens(), 195); // 175 + 20

    // Terminal record clears in-flight
    app.apply_interaction_event(&InteractionEvent::ModelUsageUpdated {
        invocation_id: Some("inv-104".to_string()),
        prompt_tokens: 40,
        completion_tokens: 20,
        total_tokens: 60,
        cost_cents: Some(10),
        cost_usd: Some(0.10),
        cost_provenance: CostProvenance::Authoritative,
    });
    assert_eq!(app.model.model_usage.prompt_tokens, 390);
    assert_eq!(app.model.model_usage.completion_tokens, 195);
    assert_eq!(app.model.model_usage.in_flight_prompt_tokens, 0);
    assert_eq!(app.model.model_usage.in_flight_completion_tokens, 0);
    assert_eq!(app.model.model_usage.total_cost_cents, Some(65));
}

// =========================================================================
// F08: Modal keyboard ownership and composer insulation
// =========================================================================

#[test]
fn test_f08_modal_insulates_underlying_composer_and_escape_dismisses() {
    let mut app = TuiApplication::new();
    app.is_composer_focused = true;
    app.composer.set_text("pre-existing draft");

    // Open a registered ModalDialog (MissionCreation = View 40)
    app.navigation.navigate_to_view(ViewId::MissionCreation);
    assert_eq!(app.navigation.active_modal, Some(ViewId::MissionCreation));

    // 1. Classification must report Dialog, never Composer
    let classified = app.classify_key(key(KeyCode::Char('x')));
    assert_eq!(
        classified,
        KeyAction::Dialog,
        "Active modal dialog must own keyboard classification"
    );

    // 2. Typing characters must NOT reach composer
    app.handle_key(key(KeyCode::Char('x')));
    app.handle_key(key(KeyCode::Char('y')));
    app.handle_key(key(KeyCode::Enter));
    assert_eq!(
        app.composer.text(),
        "pre-existing draft",
        "Keystrokes while modal is open must not leak into composer"
    );

    // 3. Global hotkeys (e.g. '0' for approvals, '?' for help) must not navigate away
    app.handle_key(key(KeyCode::Char('?')));
    assert_eq!(
        app.navigation.active_modal,
        Some(ViewId::MissionCreation),
        "Modal must not be navigated away by global hotkeys"
    );

    // 4. Escape dismisses modal and restores focus
    app.handle_key(key(KeyCode::Esc));
    assert_eq!(
        app.navigation.active_modal, None,
        "Escape must dismiss active modal dialog"
    );
    assert_eq!(
        app.composer.text(),
        "pre-existing draft",
        "Composer draft must be intact after modal dismissal"
    );
}

#[test]
fn test_f08_all_registered_modal_dialogs_insulate_composer() {
    let modals = [
        ViewId::ApprovalsQueue,
        ViewId::SetupWizard,
        ViewId::RollbackInspector,
        ViewId::StartupRecovery,
        ViewId::MissionCreation,
    ];

    for modal_id in modals {
        let mut app = TuiApplication::new();
        app.is_composer_focused = true;
        app.composer.set_text("clean composer");

        app.navigation.navigate_to_view(modal_id);
        assert_eq!(
            app.classify_key(key(KeyCode::Char('z'))),
            KeyAction::Dialog,
            "Modal {modal_id:?} must own keyboard input"
        );

        app.handle_key(key(KeyCode::Char('z')));
        assert_eq!(
            app.composer.text(),
            "clean composer",
            "Modal {modal_id:?} must insulate composer"
        );

        app.handle_key(key(KeyCode::Esc));
        assert_eq!(
            app.navigation.active_modal, None,
            "Escape must dismiss modal {modal_id:?}"
        );
    }
}
