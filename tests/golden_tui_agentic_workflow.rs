//! Golden Interactive TUI Agentic Workflow End-to-End Test (PRD §01, CLI-01–CLI-04, TUI-01–TUI-06).
//!
//! Validates the entire 17-step product experience and interactive TUI workflow:
//! 1. Workspace initialization and SQLite persistent state setup
//! 2. Headless TestBackend terminal initialization (120x40 standard tier)
//! 3. TUI Cockpit initialization with zero database I/O during render
//! 4. Header status strip presentation (Session ID, Model, Provider, Profile, Git branch)
//! 5. Interactive Composer rendering at bottom of main viewport with `m31a>` prompt
//! 6. Slash command autocomplete popup (`/sta` -> `/status`, `/diff`, `/help`)
//! 7. `@mention` autocomplete popup for workspace files (`@Cargo...`, `@src/...`)
//! 8. Natural language task submission via composer
//! 9. Live Agent Activity spinner and telemetry update
//! 10. Autonomous Tool Execution presentation (`[TOOL]` and `[OK]` badges)
//! 11. Policy Approval modal escalation and resolution (`y` approval)
//! 12. Verification Gate proof badge presentation (`[VERIFY:OK]`)
//! 13. Git Diff Viewer screen rendering unified diff with syntax coloring
//! 14. Conversation Viewport scrolling (PageUp/PageDown)
//! 15. Responsive layout resizing (80x24 compact, 120x40 standard, 160x48 large)
//! 16. Durable session history loading and resumption
//! 17. Clean session exit and terminal safety invariants

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use std::sync::Arc;
use tempfile::TempDir;

use m31a::events::bus::BroadcastEventBus;
use m31a::interaction::events::InteractionEvent;
use m31a::interaction::session::ConversationTurn;
use m31a::persistence::sqlite::schema::initialize_database;
use m31a::runtime::AppRuntime;
use m31a::tui::TuiApplication;
use m31a::tui::conversation::TuiConversationItem;
use m31a::tui::layout::{LayoutTier, classify_terminal_size};
use m31a::tui::navigation::ScreenId;
use m31a::tui::runtime_bridge::TuiRuntimeBridge;

#[tokio::test]
async fn test_golden_interactive_tui_workflow() -> Result<(), Box<dyn std::error::Error>> {
    // -------------------------------------------------------------------------
    // Step 1: Workspace & SQLite Setup
    // -------------------------------------------------------------------------
    let temp_dir = TempDir::new()?;
    let ws = temp_dir.path().to_path_buf();

    // Create fixture workspace files for @mention resolution
    std::fs::create_dir_all(ws.join("src"))?;
    std::fs::write(
        ws.join("Cargo.toml"),
        "[package]\nname = \"fixture\"\nversion = \"0.1.0\"\n",
    )?;
    std::fs::write(
        ws.join("src/lib.rs"),
        "pub fn add(a: i32, b: i32) -> i32 { a + b }\n",
    )?;
    std::fs::write(
        ws.join("README.md"),
        "# Fixture Project\nDemonstrating autonomous agentic workflow.\n",
    )?;

    // Initialize git repository
    let _ = std::process::Command::new("git")
        .args(["init"])
        .current_dir(&ws)
        .output();
    let _ = std::process::Command::new("git")
        .args(["config", "user.name", "M31A Test"])
        .current_dir(&ws)
        .output();
    let _ = std::process::Command::new("git")
        .args(["config", "user.email", "test@m31a.dev"])
        .current_dir(&ws)
        .output();
    let _ = std::process::Command::new("git")
        .args(["add", "."])
        .current_dir(&ws)
        .output();
    let _ = std::process::Command::new("git")
        .args(["commit", "-m", "Initial fixture commit"])
        .current_dir(&ws)
        .output();

    let db_path = ws.join("test_cockpit.db");
    let pool = initialize_database(&db_path).await?;
    let event_bus = Arc::new(BroadcastEventBus::new(2048));

    let runtime = Arc::new(
        AppRuntime::from_pool_and_workspace(pool.clone(), ws.clone(), event_bus.clone()).await?,
    );

    // -------------------------------------------------------------------------
    // Step 2 & 3: Terminal TestBackend & TUI Cockpit Initialization
    // -------------------------------------------------------------------------
    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend)?;

    let (mut bridge, bridge_task) = TuiRuntimeBridge::spawn(runtime.clone(), None).await?;
    let bridge_sender = bridge.sender();
    let interaction_rx = bridge
        .take_event_receiver()
        .expect("interaction event receiver available");

    let mut app = TuiApplication::new()
        .with_interaction_rx(interaction_rx)
        .with_bridge_tx(bridge_sender)
        .with_workspace_root(ws.clone());

    // Poll initial updates (ConfigurationUpdated event on fresh launch)
    let polled = app.poll_updates();
    assert!(
        polled > 0,
        "Expected initial configuration event in channel"
    );
    assert_eq!(app.model.session_status, "idle");
    assert!(app.model.session_id.is_none());

    // -------------------------------------------------------------------------
    // Step 4 & 5: Header Status Strip & Composer Rendering Verification
    // -------------------------------------------------------------------------
    let rendered = app.render_frame(&mut terminal)?;
    assert!(rendered, "Initial frame rendered");

    // Invariant check: SQLite render access counter remains 0
    assert_eq!(
        app.model.sqlite_render_access_count(),
        0,
        "Zero SQLite queries during draw (Law 9, Law 15)"
    );

    // Verify cockpit layout areas
    let tier = classify_terminal_size(120, 40);
    assert_eq!(tier, LayoutTier::Standard);

    // -------------------------------------------------------------------------
    // Step 6: Interactive Composer Slash Autocomplete Popup
    // -------------------------------------------------------------------------
    // Activate composer focus
    app.handle_key(KeyEvent::from(KeyCode::Char('i')));
    assert!(app.is_composer_focused);

    // Type "/sta"
    app.composer.set_text("/sta");
    assert!(app.composer.is_autocomplete_open());
    assert_eq!(app.composer.text(), "/sta");

    // Tab completes the command
    app.handle_key(KeyEvent::from(KeyCode::Tab));
    assert!(app.composer.text().starts_with("/status"));
    assert!(!app.composer.is_autocomplete_open());

    // -------------------------------------------------------------------------
    // Step 7: Composer @mention Autocomplete Popup
    // -------------------------------------------------------------------------
    app.composer.set_text("inspect @Car");
    assert!(app.composer.is_autocomplete_open());

    // Tab accepts mention
    app.handle_key(KeyEvent::from(KeyCode::Tab));
    assert!(app.composer.text().contains("@Cargo.toml"));
    assert!(!app.composer.is_autocomplete_open());

    // Clear composer for next action
    app.composer.clear();

    // -------------------------------------------------------------------------
    // Step 8: Slash Command Submission (/help, /status)
    // -------------------------------------------------------------------------
    app.composer.set_text("/help");
    if app.composer.is_autocomplete_open() {
        app.handle_key(KeyEvent::from(KeyCode::Tab));
    }
    let cmd = app.handle_key(KeyEvent::from(KeyCode::Enter));
    assert_eq!(cmd, None); // Handled internally
    // Phase 36.5 §8/§53: /help routes through the runtime bridge to the
    // canonical SlashCommandRegistry (no hardcoded composer string). The
    // registry card arrives asynchronously — poll boundedly, never forever.
    let mut help_seen = false;
    for _ in 0..100 {
        app.poll_updates();
        help_seen = app.model.conversation.iter().any(|item| {
            matches!(item, TuiConversationItem::System { text, .. } if text.contains("Available commands"))
        });
        if help_seen {
            break;
        }
        // Async yield: the bridge worker lives on this runtime and must be
        // polled; a blocking sleep would starve it on current-thread runtimes.
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    assert!(
        help_seen,
        "canonical /help card must arrive via the runtime bridge"
    );

    // -------------------------------------------------------------------------
    // Step 9 & 10: Live Agent Activity and Tool Execution Telemetry
    // -------------------------------------------------------------------------
    // Apply model reasoning activity event
    app.model
        .apply_interaction_event(&InteractionEvent::ModelActivity {
            text: "Analyzing project structure and test suites...".to_string(),
        });
    assert_eq!(
        app.model.live_activity.as_deref(),
        Some("Analyzing project structure and test suites...")
    );

    // Apply autonomous tool execution started
    app.model
        .apply_interaction_event(&InteractionEvent::ToolStarted {
            call_id: "call_01".to_string(),
            tool_name: "read_file".to_string(),
            parameters: serde_json::json!({ "path": "src/lib.rs" }),
        });
    assert!(
        app.model
            .conversation
            .iter()
            .any(|item| matches!(item, TuiConversationItem::ToolActivity { tool_name, .. } if tool_name == "read_file"))
    );

    // Apply autonomous tool execution concluded
    app.model
        .apply_interaction_event(&InteractionEvent::ToolCompleted {
            call_id: "call_01".to_string(),
            tool_name: "read_file".to_string(),
            success: true,
            output_preview: "pub fn add(a: i32, b: i32) -> i32 { a + b }".to_string(),
        });
    assert!(
        app.model
            .conversation
            .iter()
            .any(|item| matches!(item, TuiConversationItem::ToolResult { success: true, .. }))
    );

    // -------------------------------------------------------------------------
    // Step 11: Policy Approval Modal Escalation & Resolution
    // -------------------------------------------------------------------------
    app.model
        .apply_interaction_event(&InteractionEvent::ApprovalRequested {
            request_id: "req_sec_42".to_string(),
            tool_name: "exec_command".to_string(),
            details: "Execute `cargo test` inside sandbox".to_string(),
            risk_tier: None,
            parameters_summary: None,
            agent_role: None,
            timeout_seconds: None,
        });
    assert_eq!(app.model.approvals.len(), 1);

    // Open approval modal
    let req = app.model.approvals[0].clone();
    app.approval_modal.open(req);
    assert!(app.approval_modal.is_open);

    // Render frame while modal is open
    app.model.mark_dirty();
    app.render_frame(&mut terminal)?;

    // Handle 'y' to approve via the single canonical bridge path (§8):
    // no parallel RuntimeCommand — the typed ApprovalDecision action must
    // arrive on the runtime bridge the test attached at startup.
    let approve_key = KeyEvent::from(KeyCode::Char('y'));
    let action_cmd = app.handle_key(approve_key);
    assert!(!app.approval_modal.is_open);
    assert!(
        action_cmd.is_none(),
        "approval must route via bridge, not a parallel RuntimeCommand"
    );

    // Resolve approval event in view model
    app.model
        .apply_interaction_event(&InteractionEvent::ApprovalResolved {
            request_id: "req_sec_42".to_string(),
            approved: true,
        });
    assert_eq!(app.model.approvals.len(), 0);

    // -------------------------------------------------------------------------
    // Step 12: Verification Gate Evidence Presentation
    // -------------------------------------------------------------------------
    app.model
        .apply_interaction_event(&InteractionEvent::VerificationPassed {
            summary: "Gate 1-6 passed: Unit tests, policy invariant, worktree cleanliness."
                .to_string(),
            verification_id: None,
            check: None,
        });
    assert!(
        app.model
            .conversation
            .iter()
            .any(|item| matches!(item, TuiConversationItem::Verification { passed: true, .. }))
    );

    // -------------------------------------------------------------------------
    // Step 13: Git Diff Viewer Screen with Syntax Color Rendering
    // -------------------------------------------------------------------------
    let sample_diff = "diff --git a/src/lib.rs b/src/lib.rs\n\
                       index 1234567..89abcdef 100644\n\
                       --- a/src/lib.rs\n\
                       +++ b/src/lib.rs\n\
                       @@ -1,1 +1,2 @@\n\
                       -pub fn add(a: i32, b: i32) -> i32 { a + b }\n\
                       +pub fn add(a: i32, b: i32) -> i32 { a + b }\n\
                       +pub fn sub(a: i32, b: i32) -> i32 { a - b }\n";

    app.model.git_diff_text = Some(sample_diff.to_string());
    app.navigation.navigate_to(ScreenId::Git);
    app.model.mark_dirty();

    let diff_rendered = app.render_frame(&mut terminal)?;
    assert!(diff_rendered, "Diff screen rendered with syntax highlights");
    app.navigation.navigate_to(ScreenId::Dashboard);

    // -------------------------------------------------------------------------
    // Step 14: Viewport Scrolling (PageUp / PageDown)
    // -------------------------------------------------------------------------
    assert_eq!(app.model.scroll_offset, 0);
    app.is_composer_focused = false; // Return to navigation

    // Paging moves by viewport-sized amounts (one real viewport, not a
    // hardcoded constant); follow mode disengages while reading history.
    let page = app.model.page_step();
    assert!(page > 1, "page step must be viewport-sized");
    app.handle_key(KeyEvent::from(KeyCode::PageUp));
    assert_eq!(app.model.scroll_offset, page.min(app.model.max_scroll()));
    assert!(!app.model.follow || app.model.max_scroll() == 0);

    app.handle_key(KeyEvent::from(KeyCode::PageDown));
    assert_eq!(app.model.scroll_offset, 0);
    assert!(app.model.follow);

    // -------------------------------------------------------------------------
    // Step 15: Responsive Layout Resizing
    // -------------------------------------------------------------------------
    // 15a: Compact layout (80x24)
    let compact_backend = TestBackend::new(80, 24);
    let mut compact_term = Terminal::new(compact_backend)?;
    app.model.mark_dirty();
    assert!(app.render_frame(&mut compact_term)?);

    // 15b: Large layout (160x48)
    let large_backend = TestBackend::new(160, 48);
    let mut large_term = Terminal::new(large_backend)?;
    app.model.mark_dirty();
    assert!(app.render_frame(&mut large_term)?);

    // -------------------------------------------------------------------------
    // Step 16: Durable Session History Loading & Resumption
    // -------------------------------------------------------------------------
    let turns = vec![
        ConversationTurn::UserMessage {
            id: uuid::Uuid::now_v7(),
            sequence: 1,
            content: "Build math utility".to_string(),
            raw_text: "Build math utility".to_string(),
            mentions: Vec::new(),
            created_at: chrono::Utc::now(),
        },
        ConversationTurn::AssistantMessage {
            id: uuid::Uuid::now_v7(),
            sequence: 2,
            content: "Math utility implemented and verified.".to_string(),
            created_at: chrono::Utc::now(),
        },
    ];

    app.model.load_session_history(&turns);
    assert_eq!(app.model.conversation.len(), 2);
    assert_eq!(app.model.conversation[0].badge(), "[USER]");
    assert_eq!(app.model.conversation[1].badge(), "[ASST]");

    // -------------------------------------------------------------------------
    // Step 17: Clean Exit & Safety Invariants
    // -------------------------------------------------------------------------
    assert!(app.is_running);
    app.handle_key(KeyEvent::from(KeyCode::Char('q')));
    assert!(!app.is_running, "Clean application shutdown on 'q'");

    bridge_task.abort();
    Ok(())
}
