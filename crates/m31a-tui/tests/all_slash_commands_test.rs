use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use m31a_core::Config;
use m31a_tui::{App, ThemeVariant, handle_key_event, handle_mouse_event, init_theme};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use std::path::PathBuf;

#[tokio::test(flavor = "multi_thread")]
async fn test_all_slash_commands_execution_and_rendering() {
    init_theme(ThemeVariant::Dark);
    let config = Config {
        api_key: Some("test_api_key".to_string()),
        ..Default::default()
    };
    let mut app = App::new(config).expect("failed to create app");

    // Test Unicode resilience in CWD and session ID
    app.cwd = PathBuf::from("/tmp/m31a_test_🦀_привет_世界_español");
    app.session_id = "🦀_session_привет_12345".to_string();

    let commands = [
        // Base / Empty / Unknown
        "",
        "   ",
        "/",
        "/unknown_cmd_xyz",
        // Help / Settings / Setup
        "/help",
        "/settings",
        "/config",
        "/setup",
        // Model
        "/model",
        "/models",
        "/model route",
        "/model route on",
        "/model route off",
        "/model refresh",
        "/model fast",
        "/model coder",
        "/model reasoning",
        "/model flagship",
        "/model custom-vendor/custom-model-id",
        // Context
        "/context",
        "/context refresh",
        "/context pin",
        "/context pin Cargo.toml",
        "/context unpin",
        "/context unpin Cargo.toml",
        "/context clear",
        "/context compact",
        "/context invalid_subcommand",
        // Modes & Policies
        "/palette",
        "/build",
        "/plan",
        "/sandbox",
        "/approval",
        // Effort
        "/effort",
        "/reasoning-effort",
        "/effort low",
        "/effort med",
        "/effort medium",
        "/effort high",
        "/effort none",
        "/effort invalid_effort",
        // Status & Doctor & Analytics & Timing
        "/status",
        "/doctor",
        "/analytics",
        "/timing",
        "/critical-path",
        "/locks",
        "/active-locks",
        "/file-locks",
        "/path-locks",
        // Dials & QR
        "/dial",
        "/dials",
        "/quick",
        "/dial 1",
        "/dial 99",
        "/qr",
        // Skills, MCP, Templates, Personas, Agents
        "/skills",
        "/skills test_query",
        "/mcp",
        "/hub",
        "/template",
        "/templates",
        "/persona",
        "/personas",
        "/agents",
        "/subagents",
        // Sound / Bell
        "/bell",
        "/sound",
        "/bell on",
        "/bell off",
        // Goal
        "/goal",
        "/goal status",
        "/goal pause",
        "/goal resume",
        "/goal clear",
        "/goal autonomous objective test",
        // Diff, Verify, Test, Outline, Def, Review, Apply
        "/diff",
        "/verify",
        "/check",
        "/test",
        "/outline",
        "/outline Cargo.toml",
        "/outline nonexistent_file_xyz.rs",
        "/symbols",
        "/def",
        "/def my_test_symbol",
        "/review",
        "/review HEAD",
        "/apply",
        "/apply nonexistent.patch",
        // Processes
        "/ps",
        "/processes",
        "/procs",
        "/kill",
        "/kill 99999",
        "/logs",
        "/logs 99999",
        // Trust & Armor
        "/trust",
        "/trust add",
        "/trust session",
        "/trust audit",
        "/trust revoke",
        "/trust status",
        // Worktree
        "/worktree",
        "/worktree list",
        // Hooks & Memories & Recap & Export
        "/hooks",
        "/recap",
        "/experimental",
        "/memories",
        "/export",
        // Navigation & Workspace
        "/cd",
        "/cd .",
        "/cd /nonexistent_dir_xyz",
        "/pwd",
        "/cwd",
        "/lock",
        "/lock Cargo.toml",
        "/unlock",
        "/unlock Cargo.toml",
        // Session / Threads / Checkpoints
        "/checkpoints",
        "/undo",
        "/revert",
        "/revert chk_nonexistent",
        "/resume",
        "/fork",
        "/board",
        // Search
        "/search",
        "/search rust async pattern",
        // Theme
        "/theme",
        "/theme dark",
        "/theme light",
        "/theme contrast",
        "/theme invalid_theme",
        // Clear & Exit
        "/clear",
        "/compact",
        "/exit",
        "/quit",
    ];

    let viewports = [(120, 40), (80, 24), (40, 15)];

    for (w, h) in viewports {
        let backend = TestBackend::new(w, h);
        let mut terminal = Terminal::new(backend).expect("failed to create terminal");

        for cmd in &commands {
            app.execute_slash_command(cmd);

            // Test rendering with terminal
            terminal
                .draw(|f| {
                    m31a_tui::renderer::render(f, &mut app);
                })
                .unwrap_or_else(|e| {
                    panic!(
                        "Failed to render frame after slash command {:?}: {}",
                        cmd, e
                    )
                });

            // If a modal dialog opened, test interactive navigation and keyboard events
            if app.has_active_modal() {
                let keys = [
                    KeyCode::Up,
                    KeyCode::Down,
                    KeyCode::Tab,
                    KeyCode::BackTab,
                    KeyCode::PageUp,
                    KeyCode::PageDown,
                    KeyCode::Char(' '),
                    KeyCode::Char('1'),
                    KeyCode::Char('2'),
                    KeyCode::Char('3'),
                    KeyCode::Enter,
                    KeyCode::Esc,
                ];

                for keycode in keys {
                    handle_key_event(&mut app, KeyEvent::new(keycode, KeyModifiers::NONE));
                    terminal
                        .draw(|f| {
                            m31a_tui::renderer::render(f, &mut app);
                        })
                        .unwrap_or_else(|e| {
                            panic!(
                                "Failed rendering after key {:?} on modal from {:?}: {}",
                                keycode, cmd, e
                            )
                        });
                }
            }
        }
    }

    // Clean up any export files created during test
    if let Ok(entries) = std::fs::read_dir(".") {
        for entry in entries.flatten() {
            if let Some(name) = entry.file_name().to_str()
                && name.starts_with("m31a-session-")
                && name.ends_with(".md")
            {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }
}

#[test]
fn test_slash_commands_in_synchronous_context() {
    // Tests that executing slash commands in a purely sync thread (no tokio reactor running) never panics
    let config = Config {
        api_key: Some("test_api_key".to_string()),
        ..Default::default()
    };
    let mut app = App::new(config).expect("failed to create app");

    let sync_cmds = [
        "",
        "   ",
        "/help",
        "/setup",
        "/settings",
        "/config",
        "/model",
        "/model refresh",
        "/model route on",
        "/model route off",
        "/context",
        "/context refresh",
        "/context compact",
        "/palette",
        "/build",
        "/plan",
        "/effort",
        "/status",
        "/doctor",
        "/diff",
        "/verify",
        "/test",
        "/ps",
        "/procs",
        "/kill 123",
        "/logs 123",
        "/goal",
        "/goal resume",
        "/goal clear",
        "/recap",
        "/search hello",
        "/exit",
    ];

    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).expect("failed to create terminal");

    for cmd in sync_cmds {
        app.execute_slash_command(cmd);
        terminal
            .draw(|f| {
                m31a_tui::renderer::render(f, &mut app);
            })
            .unwrap_or_else(|e| panic!("Failed to render in sync context for {:?}: {}", cmd, e));
    }
}

#[test]
fn test_mouse_interactions_on_slash_modals() {
    init_theme(ThemeVariant::Dark);
    let config = Config {
        api_key: Some("test_api_key".to_string()),
        ..Default::default()
    };
    let mut app = App::new(config).expect("failed to create app");

    // Trigger settings modal via slash command
    app.execute_slash_command("/settings");
    assert!(app.has_active_modal());

    use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
    let mouse_down = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 40,
        row: 15,
        modifiers: KeyModifiers::NONE,
    };
    let area = ratatui::layout::Rect::new(0, 0, 80, 24);
    handle_mouse_event(&mut app, mouse_down, area);

    // Click outside to dismiss
    let backdrop_click = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 2,
        row: 2,
        modifiers: KeyModifiers::NONE,
    };
    handle_mouse_event(&mut app, backdrop_click, area);
    assert!(!app.has_active_modal());
}
