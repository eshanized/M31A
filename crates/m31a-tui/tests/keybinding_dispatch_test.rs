//! Keybinding Dispatch and Precedence Integration Tests (`keybinding_dispatch_test.rs`).

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use m31a_core::Config;
use m31a_tui::{App, ModalState, ThemeVariant, handle_key_event, init_theme};

#[test]
fn test_modal_precedence_and_escape_dismissal() {
    init_theme(ThemeVariant::Dark);
    let temp = tempfile::tempdir().unwrap();
    let config = Config {
        model: "meta/llama-3.3-70b-instruct".to_string(),
        model_provider: Some("nim".to_string()),
        api_key: Some(
            m31a_core::resolve_stored_api_key()
                .unwrap_or_else(|| "nvapi-runtime-integration-token".to_string()),
        ),
        ..Default::default()
    };

    let mut app = App::new_test(config, temp.path()).expect("failed to create app");

    // 1. Open Help modal via F1
    handle_key_event(&mut app, KeyEvent::new(KeyCode::F(1), KeyModifiers::NONE));
    assert!(app.has_active_modal());

    // 2. Escape dismisses the active modal
    handle_key_event(&mut app, KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(!app.has_active_modal());

    // 3. Open Settings modal via Ctrl+S
    handle_key_event(
        &mut app,
        KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL),
    );
    assert!(matches!(
        app.current_modal(),
        Some(ModalState::SettingsModal { .. })
    ));

    // 4. Escape closes Settings
    handle_key_event(&mut app, KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(!app.has_active_modal());
}

#[test]
fn test_model_picker_shortcuts_and_ctrl_enter_separation() {
    init_theme(ThemeVariant::Dark);
    let temp = tempfile::tempdir().unwrap();
    let config = Config {
        model: "meta/llama-3.3-70b-instruct".to_string(),
        model_provider: Some("nim".to_string()),
        api_key: Some(
            m31a_core::resolve_stored_api_key()
                .unwrap_or_else(|| "nvapi-runtime-integration-token".to_string()),
        ),
        ..Default::default()
    };

    let mut app = App::new_test(config, temp.path()).expect("failed to create app");

    // Ctrl+M opens model picker
    handle_key_event(
        &mut app,
        KeyEvent::new(KeyCode::Char('m'), KeyModifiers::CONTROL),
    );
    assert!(matches!(
        app.current_modal(),
        Some(ModalState::ModelPicker { .. })
    ));
    handle_key_event(&mut app, KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(!app.has_active_modal());

    // Alt+M opens model picker (legacy terminal fallback)
    handle_key_event(
        &mut app,
        KeyEvent::new(KeyCode::Char('m'), KeyModifiers::ALT),
    );
    assert!(matches!(
        app.current_modal(),
        Some(ModalState::ModelPicker { .. })
    ));
    handle_key_event(&mut app, KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(!app.has_active_modal());

    // F2 opens model picker
    handle_key_event(&mut app, KeyEvent::new(KeyCode::F(2), KeyModifiers::NONE));
    assert!(matches!(
        app.current_modal(),
        Some(ModalState::ModelPicker { .. })
    ));
    handle_key_event(&mut app, KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(!app.has_active_modal());

    // Ctrl+Enter MUST NOT open model picker!
    handle_key_event(
        &mut app,
        KeyEvent::new(KeyCode::Enter, KeyModifiers::CONTROL),
    );
    assert!(
        !app.has_active_modal(),
        "Ctrl+Enter must never open the model picker modal"
    );
}

#[test]
fn test_ctrl_enter_prompt_queuing_during_execution() {
    init_theme(ThemeVariant::Dark);
    let temp = tempfile::tempdir().unwrap();
    let config = Config {
        model: "meta/llama-3.3-70b-instruct".to_string(),
        model_provider: Some("nim".to_string()),
        api_key: Some(
            m31a_core::resolve_stored_api_key()
                .unwrap_or_else(|| "nvapi-runtime-integration-token".to_string()),
        ),
        ..Default::default()
    };

    let mut app = App::new_test(config, temp.path()).expect("failed to create app");

    // Simulate active execution state
    app.turn_state.transition_submitted("test-turn").unwrap();
    app.turn_state.transition_streaming("test-turn").unwrap();
    app.composer.set_text("Follow-up question while busy");

    // Ctrl+Enter should queue prompt
    handle_key_event(
        &mut app,
        KeyEvent::new(KeyCode::Enter, KeyModifiers::CONTROL),
    );

    assert_eq!(app.queued_prompts.len(), 1);
    assert_eq!(
        app.queued_prompts.front().map(|s| s.as_str()),
        Some("Follow-up question while busy")
    );
    assert!(app.composer.is_empty());
}

#[test]
fn test_ctrl_c_interrupt_lifecycle() {
    init_theme(ThemeVariant::Dark);
    let temp = tempfile::tempdir().unwrap();
    let config = Config {
        model: "meta/llama-3.3-70b-instruct".to_string(),
        model_provider: Some("nim".to_string()),
        api_key: Some(
            m31a_core::resolve_stored_api_key()
                .unwrap_or_else(|| "nvapi-runtime-integration-token".to_string()),
        ),
        ..Default::default()
    };

    let mut app = App::new_test(config, temp.path()).expect("failed to create app");

    // Type text into composer
    app.composer.set_text("In-progress draft prompt");
    assert!(!app.composer.is_empty());

    // Ctrl+C clears prompt buffer when not streaming
    handle_key_event(
        &mut app,
        KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
    );
    assert!(app.composer.is_empty());
}
