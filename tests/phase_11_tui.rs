//! Phase 11 TUI Cockpit, Layout, Navigation & Replay Tests (TUI-01..06).

use ratatui::Terminal;
use ratatui::backend::{Backend, TestBackend};

use m31a::events::envelope::EventEnvelope;
use m31a::events::types::EventType;
use m31a::ids::MissionId;
use m31a::interaction::events::InteractionEvent;
use m31a::tui::{LayoutTier, TuiApplication, classify_terminal_size, compute_layout};

#[test]
fn test_responsive_layout_degradation() {
    // 1. Verify exact layout tier classification across all 4 breakpoints (TUI-02)
    assert_eq!(classify_terminal_size(79, 24), LayoutTier::Compact);
    assert_eq!(classify_terminal_size(80, 24), LayoutTier::Compact);
    assert_eq!(classify_terminal_size(99, 30), LayoutTier::Compact);

    assert_eq!(classify_terminal_size(100, 30), LayoutTier::Standard);
    assert_eq!(classify_terminal_size(140, 40), LayoutTier::Standard);
    assert_eq!(classify_terminal_size(159, 40), LayoutTier::Standard);

    assert_eq!(classify_terminal_size(160, 45), LayoutTier::Large);
    assert_eq!(classify_terminal_size(200, 50), LayoutTier::Large);
    assert_eq!(classify_terminal_size(219, 50), LayoutTier::Large);

    assert_eq!(classify_terminal_size(220, 60), LayoutTier::UltraWide);
    assert_eq!(classify_terminal_size(300, 70), LayoutTier::UltraWide);

    // 2. Render at minimum viable resolution 80x24 (Compact tier)
    let backend_80x24 = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend_80x24).unwrap();
    let mut app = TuiApplication::new();

    let rendered = app.render_frame(&mut terminal).unwrap();
    assert!(rendered);

    let (_, areas) = compute_layout(terminal.backend().size().unwrap().into());
    assert_eq!(areas.header.height, 2);
    assert_eq!(areas.footer.height, 1);
    assert_eq!(areas.main.height, 21);
    assert!(areas.sidebar.is_none());
    assert!(areas.telemetry.is_none());

    // 3. Render at Standard resolution 120x35 (conversation-first: no permanent sidebar)
    let backend_standard = TestBackend::new(120, 35);
    let mut terminal_std = Terminal::new(backend_standard).unwrap();
    app.force_redraw = true;
    let rendered_std = app.render_frame(&mut terminal_std).unwrap();
    assert!(rendered_std);

    let (tier_std, areas_std) = compute_layout(terminal_std.backend().size().unwrap().into());
    assert_eq!(tier_std, LayoutTier::Standard);
    assert!(areas_std.sidebar.is_none());
    assert!(areas_std.telemetry.is_none());

    // 4. Render at Large resolution 180x45 (conversation remains dominant)
    let backend_large = TestBackend::new(180, 45);
    let mut terminal_large = Terminal::new(backend_large).unwrap();
    app.force_redraw = true;
    let rendered_large = app.render_frame(&mut terminal_large).unwrap();
    assert!(rendered_large);

    let (tier_large, areas_large) = compute_layout(terminal_large.backend().size().unwrap().into());
    assert_eq!(tier_large, LayoutTier::Large);
    assert!(areas_large.sidebar.is_none());
    assert!(areas_large.telemetry.is_none());
}

#[test]
fn test_zero_sqlite_reads_in_render() {
    let backend = TestBackend::new(100, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut app = TuiApplication::new();

    // 1. Initial render frame executes with 0 SQLite operations
    let rendered = app.render_frame(&mut terminal).unwrap();
    assert!(rendered);
    assert_eq!(app.model.sqlite_render_access_count(), 0);

    // 2. Subsequent frame without changes skips draw via dirty-state scheduling
    let re_rendered = app.render_frame(&mut terminal).unwrap();
    assert!(
        !re_rendered,
        "Unchanged model must skip draw (dirty-state scheduling)"
    );
    assert_eq!(app.model.sqlite_render_access_count(), 0);

    // 3. Incoming bridge event reduces through the single TUI ingress and
    // marks the model dirty, triggering render on next tick.
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    let mut app_with_channel = TuiApplication::new().with_interaction_rx(rx);

    let mid = MissionId::new();
    tx.send(InteractionEvent::MissionStateChanged {
        mission_id: mid,
        status: "running".to_string(),
    })
    .unwrap();
    let count = app_with_channel.poll_updates();
    assert_eq!(count, 1);
    assert!(app_with_channel.model.is_dirty);
    assert_eq!(app_with_channel.model.mission_status, "running");

    let rendered2 = app_with_channel.render_frame(&mut terminal).unwrap();
    assert!(rendered2);
    assert_eq!(app_with_channel.model.sqlite_render_access_count(), 0);
    assert!(!app_with_channel.model.is_dirty);
}

#[test]
fn test_tui_screens_and_views() {
    use m31a::tui::ScreenId;

    let backend = TestBackend::new(120, 35);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut app = TuiApplication::new();

    let screens = [
        ScreenId::Dashboard,
        ScreenId::Mission,
        ScreenId::TaskGraph,
        ScreenId::Agents,
        ScreenId::Tools,
        ScreenId::Jobs,
        ScreenId::Verification,
        ScreenId::Git,
        ScreenId::Approvals,
        ScreenId::Doctor,
        ScreenId::Logs,
        ScreenId::ModelUsage,
        ScreenId::Artifacts,
        ScreenId::Replay,
        ScreenId::Help,
    ];

    assert_eq!(screens.len(), 15);

    for s in screens {
        app.navigation.navigate_to(s);
        app.force_redraw = true;
        let rendered = app.render_frame(&mut terminal).unwrap();
        assert!(rendered);
        assert_eq!(app.navigation.current_screen, s);
    }
}

#[test]
fn test_canonical_shortcuts_and_actions() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use m31a::cli::RuntimeCommand;
    use m31a::tui::ScreenId;
    use m31a::tui::navigation::NavigationAction;

    let mut app = TuiApplication::new();

    // 1. Primary numbers 1..0
    let key_3 = KeyEvent::new(KeyCode::Char('3'), KeyModifiers::empty());
    assert_eq!(
        app.navigation.handle_key(key_3),
        NavigationAction::ScreenChanged(ScreenId::TaskGraph)
    );
    assert_eq!(app.navigation.current_screen, ScreenId::TaskGraph);

    // Superseded Phase 11 contradiction eliminated per Remediation Spec §6:
    // Hotkey '0' canonical target is ViewId::ApprovalsQueue (ScreenId::Approvals),
    // and hotkey 'd' canonical target is ViewId::DoctorDiagnostics (ScreenId::Doctor).
    let key_0 = KeyEvent::new(KeyCode::Char('0'), KeyModifiers::empty());
    assert_eq!(
        app.navigation.handle_key(key_0),
        NavigationAction::ScreenChanged(ScreenId::Approvals)
    );
    assert_eq!(app.navigation.current_screen, ScreenId::Approvals);

    let key_d = KeyEvent::new(KeyCode::Char('d'), KeyModifiers::empty());
    assert_eq!(
        app.navigation.handle_key(key_d),
        NavigationAction::ScreenChanged(ScreenId::Doctor)
    );
    assert_eq!(app.navigation.current_screen, ScreenId::Doctor);

    // 2. Auxiliary letters l, m, a, r, ?
    let key_l = KeyEvent::new(KeyCode::Char('l'), KeyModifiers::empty());
    assert_eq!(
        app.navigation.handle_key(key_l),
        NavigationAction::ScreenChanged(ScreenId::Logs)
    );

    let key_qmark = KeyEvent::new(KeyCode::Char('?'), KeyModifiers::empty());
    assert_eq!(
        app.navigation.handle_key(key_qmark),
        NavigationAction::ScreenChanged(ScreenId::Help)
    );

    // 3. Command Palette trigger via Ctrl+P and ':'
    let key_ctrl_p = KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL);
    let action_palette = app.handle_key(key_ctrl_p);
    assert_eq!(action_palette, None);
    assert!(app.palette.is_open);

    // 4. Fuzzy search in Command Palette
    app.palette.query = "Emergency Cancel".to_string();
    let matches = app.palette.filtered_items();
    assert!(!matches.is_empty());
    assert!(matches[0].label.contains("Cancel"));

    // Enter executes selected action
    let key_enter = KeyEvent::new(KeyCode::Enter, KeyModifiers::empty());
    let cmd = app.handle_key(key_enter);
    assert!(!app.palette.is_open);
    assert!(matches!(cmd, Some(RuntimeCommand::CancelMission { .. })));

    // Test palette navigation
    app.palette.open();
    app.palette.query = "Overview".to_string();
    let matches_nav = app.palette.filtered_items();
    assert!(!matches_nav.is_empty());
    assert!(matches_nav[0].label.contains("Dashboard"));

    let key_enter_nav = KeyEvent::new(KeyCode::Enter, KeyModifiers::empty());
    let cmd_nav = app.handle_key(key_enter_nav);
    assert_eq!(cmd_nav, None);
    assert_eq!(app.navigation.current_screen, ScreenId::Dashboard);

    // 5. Space triggers pause/resume toggle in navigation mode — only with a real mission id
    app.unfocus_composer();
    let key_space = KeyEvent::new(KeyCode::Char(' '), KeyModifiers::empty());
    let pause_none = app.handle_key(key_space);
    assert_eq!(
        pause_none, None,
        "pause with no active mission must not emit an unresolvable command"
    );
    app.model.mission_id = Some("mission-01".to_string());
    let pause_cmd = app.handle_key(key_space);
    assert!(matches!(
        pause_cmd,
        Some(RuntimeCommand::PauseMission { .. })
    ));
}

#[test]
fn test_reconnect_and_deterministic_replay() {
    use m31a::ids::{AgentId, TaskId};

    let mut app = TuiApplication::new();
    assert_eq!(app.model.mission_status, "idle");
    assert_eq!(app.model.tasks.len(), 0);

    let mid = MissionId::new();
    let aid = AgentId::new();
    let tid1 = TaskId::new();
    let tid2 = TaskId::new();

    let history = vec![
        EventEnvelope::new(
            1,
            Some(mid),
            None,
            "orchestrator".to_string(),
            EventType::MissionStarted {
                mission_id: mid,
                objective: "Deploy autonomous satellite cluster".to_string(),
            },
        ),
        EventEnvelope::new(
            2,
            Some(mid),
            None,
            "scheduler".to_string(),
            EventType::AgentSpawned {
                agent_id: aid,
                mission_id: mid,
                role: "architect".to_string(),
            },
        ),
        EventEnvelope::new(
            3,
            Some(mid),
            None,
            "scheduler".to_string(),
            EventType::TaskStarted {
                task_id: tid1,
                mission_id: mid,
                agent_id: aid,
            },
        ),
        EventEnvelope::new(
            4,
            Some(mid),
            None,
            "scheduler".to_string(),
            EventType::TaskCompleted {
                task_id: tid1,
                mission_id: mid,
                result: "Architecture document generated".to_string(),
            },
        ),
        EventEnvelope::new(
            5,
            Some(mid),
            None,
            "scheduler".to_string(),
            EventType::TaskStarted {
                task_id: tid2,
                mission_id: mid,
                agent_id: aid,
            },
        ),
    ];

    // Reconnect and reconstruct state (TUI-05)
    app.reconnect(&history);

    assert_eq!(app.model.mission_status, "running");
    assert_eq!(app.model.objective, "Deploy autonomous satellite cluster");
    assert_eq!(app.model.agents.len(), 1);
    assert_eq!(app.model.agents[0].role, "architect");
    assert_eq!(app.model.tasks.len(), 2);
    assert_eq!(app.model.tasks[0].status, "completed");
    assert_eq!(app.model.tasks[1].status, "running");
    assert!(app.force_redraw);
}

#[test]
fn test_ansi_escape_sanitization() {
    use m31a::tui::{sanitize_diff, sanitize_terminal_text, sanitize_tool_spool};

    // 1. Color escape sequences and formatting codes stripped (TUI-06, T-11-18)
    let dirty_color = "\x1b[31;1mCRITICAL ERROR:\x1b[0m \x1b[33mWarning threshold exceeded\x1b[0m";
    let clean_color = sanitize_terminal_text(dirty_color);
    assert_eq!(clean_color, "CRITICAL ERROR: Warning threshold exceeded");

    // 2. Dangerous terminal cursor manipulation and clearing codes stripped
    let dirty_cursor = "Prefix\x1b[2J\x1b[H\x1b[20;30H\x1b[?25lInjected Text\x1b[?25h";
    let clean_cursor = sanitize_terminal_text(dirty_cursor);
    assert_eq!(clean_cursor, "PrefixInjected Text");

    // 3. OSC terminal title spoofing and bell characters stripped
    let dirty_osc = "Compiling...\x1b]0;MALICIOUS TITLE HIJACK\x07Finished\x07";
    let clean_osc = sanitize_terminal_text(dirty_osc);
    assert!(!clean_osc.contains("MALICIOUS TITLE HIJACK"));
    assert_eq!(clean_osc, "Compiling...Finished");

    // 4. Git diff and tool spool helper verification
    let diff = "--- a/src/main.rs\n+++ b/src/main.rs\n@@ -1,3 +1,3 @@\n-\x1b[31m-old_fn()\x1b[0m\n+\x1b[32m+new_fn()\x1b[0m\n";
    let clean_diff = sanitize_diff(diff);
    assert_eq!(
        clean_diff,
        "--- a/src/main.rs\n+++ b/src/main.rs\n@@ -1,3 +1,3 @@\n--old_fn()\n++new_fn()\n"
    );

    let spool = "cargo check \x1b[32m[ok]\x1b[0m\n";
    let clean_spool = sanitize_tool_spool(spool);
    assert_eq!(clean_spool, "cargo check [ok]\n");
}

#[test]
fn test_approval_modal_resolution() {
    use chrono::Utc;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use m31a::tui::approval::{ApprovalDecision, ApprovalModal};
    use m31a::tui::model::TuiApprovalRequest;

    let mut modal = ApprovalModal::new();
    assert!(!modal.is_open);

    let req = TuiApprovalRequest {
        id: "pol-001".to_string(),
        tool_name: "shell:execute_script".to_string(),
        agent_role: "executor".to_string(),
        justification: "Run build script in isolated environment".to_string(),
        parameters_summary: "{\"command\": \"./build.sh --release\"}".to_string(),
        risk_tier: "high".to_string(),
        timestamp: Utc::now(),
    };

    // Open modal
    modal.open(req);
    assert!(modal.is_open);

    // Pressing 'y' approves once
    let key_y = KeyEvent::new(KeyCode::Char('y'), KeyModifiers::empty());
    let dec = modal.handle_key(key_y);
    assert_eq!(dec, Some(ApprovalDecision::ApproveOnce));
    assert!(!modal.is_open);

    // Open and reject with 'n'
    let req2 = TuiApprovalRequest {
        id: "pol-002".to_string(),
        tool_name: "fs:delete_dir".to_string(),
        agent_role: "cleaner".to_string(),
        justification: "Purge cache directory".to_string(),
        parameters_summary: "{\"path\": \"/tmp/m31a\"}".to_string(),
        risk_tier: "critical".to_string(),
        timestamp: Utc::now(),
    };
    modal.open(req2);
    let key_n = KeyEvent::new(KeyCode::Char('n'), KeyModifiers::empty());
    let dec_reject = modal.handle_key(key_n);
    assert_eq!(dec_reject, Some(ApprovalDecision::Reject));
    assert!(!modal.is_open);
}

#[test]
fn test_post_mortem_replay_scrubber() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use m31a::ids::MissionId;
    use m31a::tui::ReplaySpeed;

    let mut app = TuiApplication::new();
    let mid = MissionId::new();

    let history = vec![
        EventEnvelope::new(
            1,
            Some(mid),
            None,
            "test".to_string(),
            EventType::MissionStarted {
                mission_id: mid,
                objective: "Step 1: Init".to_string(),
            },
        ),
        EventEnvelope::new(
            2,
            Some(mid),
            None,
            "test".to_string(),
            EventType::MissionPaused {
                mission_id: mid,
                reason: "Step 2: Pause".to_string(),
            },
        ),
        EventEnvelope::new(
            3,
            Some(mid),
            None,
            "test".to_string(),
            EventType::MissionResumed { mission_id: mid },
        ),
    ];

    // Enter replay mode
    app.enter_replay(history);
    assert!(app.replay.is_active);
    assert_eq!(app.replay.cursor, 2); // starts at latest
    assert_eq!(app.model.mission_status, "running");

    // Step backward with '['
    let key_back = KeyEvent::new(KeyCode::Char('['), KeyModifiers::empty());
    app.handle_key(key_back);
    assert_eq!(app.replay.cursor, 1);
    assert_eq!(app.model.mission_status, "paused");

    // Step backward to beginning
    app.handle_key(key_back);
    assert_eq!(app.replay.cursor, 0);
    assert_eq!(app.model.mission_status, "running");
    assert_eq!(app.model.objective, "Step 1: Init");

    // Replay mode blocks mutating commands (D-19, T-11-20)
    assert!(!app.replay.is_mutation_allowed());
    let key_space = KeyEvent::new(KeyCode::Char(' '), KeyModifiers::empty());
    let cmd = app.handle_key(key_space);
    assert_eq!(cmd, None, "Mutating command must be blocked in replay mode");

    // Toggle speed
    let key_4 = KeyEvent::new(KeyCode::Char('4'), KeyModifiers::empty());
    app.handle_key(key_4);
    assert_eq!(app.replay.speed, ReplaySpeed::Hyper);

    // Step forward with ']'
    let key_forward = KeyEvent::new(KeyCode::Char(']'), KeyModifiers::empty());
    app.handle_key(key_forward);
    assert_eq!(app.replay.cursor, 1);

    // Exit replay mode with Esc
    let key_esc = KeyEvent::new(KeyCode::Esc, KeyModifiers::empty());
    app.handle_key(key_esc);
    assert!(!app.replay.is_active);
    assert!(app.replay.is_mutation_allowed());
}
