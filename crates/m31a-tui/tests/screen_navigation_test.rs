//! Integration tests for M31A 10-Screen Navigation, Application Shell & Contextual Inspector.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use m31a_core::Config;
use m31a_tui::{
    App, ModalState, Screen, ThemeVariant, execute_palette_action, handle_key_event, init_theme,
    renderer,
};
use ratatui::{Terminal, backend::TestBackend};
use tempfile::tempdir;

#[test]
fn test_screen_navigation_and_state_preservation() {
    init_theme(ThemeVariant::Dark);
    let temp = tempdir().unwrap();
    let config = Config {
        model: "meta/llama-3.3-70b-instruct".to_string(),
        api_key: None,
        ..Default::default()
    };

    let mut app = App::new_test(config, temp.path()).expect("failed to create app");

    // 1. Initial screen is Chat
    assert_eq!(app.current_screen, Screen::Chat);

    // 2. Cycle screens via next_screen() and prev_screen()
    app.next_screen();
    assert_eq!(app.current_screen, Screen::Sessions);

    app.next_screen();
    assert_eq!(app.current_screen, Screen::Models);

    app.prev_screen();
    assert_eq!(app.current_screen, Screen::Sessions);

    // 3. Navigate via Alt+1 .. Alt+0 key events
    handle_key_event(
        &mut app,
        KeyEvent::new(KeyCode::Char('1'), KeyModifiers::ALT),
    );
    assert_eq!(app.current_screen, Screen::Chat);

    handle_key_event(
        &mut app,
        KeyEvent::new(KeyCode::Char('3'), KeyModifiers::ALT),
    );
    assert_eq!(app.current_screen, Screen::Models);

    handle_key_event(
        &mut app,
        KeyEvent::new(KeyCode::Char('4'), KeyModifiers::ALT),
    );
    assert_eq!(app.current_screen, Screen::Providers);

    handle_key_event(
        &mut app,
        KeyEvent::new(KeyCode::Char('5'), KeyModifiers::ALT),
    );
    assert_eq!(app.current_screen, Screen::Agents);

    handle_key_event(
        &mut app,
        KeyEvent::new(KeyCode::Char('6'), KeyModifiers::ALT),
    );
    assert_eq!(app.current_screen, Screen::Workspace);

    handle_key_event(
        &mut app,
        KeyEvent::new(KeyCode::Char('7'), KeyModifiers::ALT),
    );
    assert_eq!(app.current_screen, Screen::SourceControl);

    handle_key_event(
        &mut app,
        KeyEvent::new(KeyCode::Char('8'), KeyModifiers::ALT),
    );
    assert_eq!(app.current_screen, Screen::ToolsIntegrations);

    handle_key_event(
        &mut app,
        KeyEvent::new(KeyCode::Char('9'), KeyModifiers::ALT),
    );
    assert_eq!(app.current_screen, Screen::Settings);

    handle_key_event(
        &mut app,
        KeyEvent::new(KeyCode::Char('0'), KeyModifiers::ALT),
    );
    assert_eq!(app.current_screen, Screen::Diagnostics);

    // 4. Pressing Esc on non-Chat screen returns directly to Chat
    handle_key_event(&mut app, KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert_eq!(app.current_screen, Screen::Chat);
}

#[test]
fn test_contextual_inspector_toggle() {
    init_theme(ThemeVariant::Dark);
    let temp = tempdir().unwrap();
    let config = Config {
        model: "meta/llama-3.3-70b-instruct".to_string(),
        api_key: None,
        ..Default::default()
    };

    let mut app = App::new_test(config, temp.path()).expect("failed to create app");

    // Initially inspector is closed
    assert!(!app.inspector_open);

    // Toggle via F12
    handle_key_event(&mut app, KeyEvent::new(KeyCode::F(12), KeyModifiers::NONE));
    assert!(app.inspector_open);

    // Toggle via Ctrl+B
    handle_key_event(
        &mut app,
        KeyEvent::new(KeyCode::Char('b'), KeyModifiers::CONTROL),
    );
    assert!(!app.inspector_open);

    // Direct toggle method
    app.toggle_inspector();
    assert!(app.inspector_open);
}

#[test]
fn test_command_palette_screen_switching() {
    init_theme(ThemeVariant::Dark);
    let temp = tempdir().unwrap();
    let config = Config {
        model: "meta/llama-3.3-70b-instruct".to_string(),
        api_key: None,
        ..Default::default()
    };

    let mut app = App::new_test(config, temp.path()).expect("failed to create app");

    // Open palette via Ctrl+P
    handle_key_event(
        &mut app,
        KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL),
    );
    assert!(matches!(
        app.current_modal(),
        Some(ModalState::CommandPalette { .. })
    ));

    // Execute nav_source_control action
    execute_palette_action(&mut app, "nav_source_control");
    assert_eq!(app.current_screen, Screen::SourceControl);

    // Execute nav_diagnostics action
    execute_palette_action(&mut app, "nav_diagnostics");
    assert_eq!(app.current_screen, Screen::Diagnostics);

    // Execute nav_chat action
    execute_palette_action(&mut app, "nav_chat");
    assert_eq!(app.current_screen, Screen::Chat);
}

#[test]
fn test_render_all_10_screens_without_panic() {
    init_theme(ThemeVariant::Dark);
    let temp = tempdir().unwrap();
    let config = Config {
        model: "meta/llama-3.3-70b-instruct".to_string(),
        api_key: None,
        ..Default::default()
    };

    let mut app = App::new_test(config, temp.path()).expect("failed to create app");
    let backend = TestBackend::new(120, 36);
    let mut terminal = Terminal::new(backend).unwrap();

    // Render every screen with inspector open and closed
    for &screen in &Screen::ALL {
        app.switch_screen(screen);

        // Render with inspector closed
        app.inspector_open = false;
        terminal
            .draw(|f| renderer::render(f, &mut app))
            .expect("render failed");

        // Render with inspector open
        app.inspector_open = true;
        terminal
            .draw(|f| renderer::render(f, &mut app))
            .expect("render with inspector failed");
    }
}

#[test]
fn test_active_turn_preservation_during_screen_navigation() {
    init_theme(ThemeVariant::Dark);
    let temp = tempdir().unwrap();
    let config = Config {
        model: "meta/llama-3.3-70b-instruct".to_string(),
        api_key: None,
        ..Default::default()
    };

    let mut app = App::new_test(config, temp.path()).expect("failed to create app");

    // Activate turn
    let turn_id = "turn-screen-preserve";
    app.current_turn_id = Some(turn_id.to_string());
    app.turn_state.transition_submitted(turn_id).unwrap();
    app.turn_state.transition_streaming(turn_id).unwrap();
    assert!(app.turn_state.is_active());

    // Switch to Sessions, Models, Settings, Diagnostics
    app.switch_screen(Screen::Sessions);
    assert_eq!(app.current_screen, Screen::Sessions);
    assert!(app.turn_state.is_active());
    assert_eq!(app.current_turn_id.as_deref(), Some(turn_id));

    app.switch_screen(Screen::Models);
    assert!(app.turn_state.is_active());

    app.switch_screen(Screen::Diagnostics);
    assert!(app.turn_state.is_active());

    // Return to Chat - turn state is still intact and active
    app.switch_screen(Screen::Chat);
    assert_eq!(app.current_screen, Screen::Chat);
    assert!(app.turn_state.is_active());
    assert_eq!(app.current_turn_id.as_deref(), Some(turn_id));
}
