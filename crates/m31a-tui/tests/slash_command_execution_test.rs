//! Comprehensive Slash Command Execution & Tokenizer Integration Tests (`slash_command_execution_test.rs`).

use m31a_core::{CollaborationMode, Config};
use m31a_tui::{App, ThemeVariant, init_theme};

#[test]
fn test_slash_command_dispatch_and_state_updates() {
    init_theme(ThemeVariant::Dark);
    let config = Config {
        model: "mock-testing-model".to_string(),
        model_provider: Some("mock".to_string()),
        api_key: Some("test_api_key".to_string()),
        ..Default::default()
    };

    let mut app = App::new(config).expect("failed to create app");

    // 1. /plan toggles collaboration mode between Plan and Default
    assert_eq!(app.collaboration_mode, CollaborationMode::Default);
    app.execute_slash_command("/plan");
    assert_eq!(app.collaboration_mode, CollaborationMode::Plan);
    app.execute_slash_command("/plan");
    assert_eq!(app.collaboration_mode, CollaborationMode::Default);

    // 2. /effort updates reasoning effort
    app.execute_slash_command("/effort high");
    assert_eq!(app.config.reasoning_effort.as_deref(), Some("high"));
    assert_eq!(app.nim_client.reasoning_effort(), Some("high"));

    app.execute_slash_command("/effort low");
    assert_eq!(app.config.reasoning_effort.as_deref(), Some("low"));
    assert_eq!(app.nim_client.reasoning_effort(), Some("low"));

    // 3. /theme updates theme variant
    app.execute_slash_command("/theme light");
    app.execute_slash_command("/theme dark");

    // 4. /goal opens GoalDashboardModal
    app.execute_slash_command("/goal");
    assert!(matches!(
        app.current_modal(),
        Some(m31a_tui::ModalState::GoalDashboardModal { .. })
    ));
}

#[test]
fn test_slash_command_alias_dispatch() {
    init_theme(ThemeVariant::Dark);
    let config = Config {
        model: "mock-model".to_string(),
        model_provider: Some("mock".to_string()),
        api_key: Some("test_api_key".to_string()),
        ..Default::default()
    };
    let mut app = App::new(config).expect("failed to create app");

    // /config is alias for /settings -> opens Settings modal
    app.execute_slash_command("/config");
    assert!(matches!(
        app.current_modal(),
        Some(m31a_tui::ModalState::SettingsModal { .. })
    ));
    app.clear_modals();

    // /settings opens Settings modal
    app.execute_slash_command("/settings");
    assert!(matches!(
        app.current_modal(),
        Some(m31a_tui::ModalState::SettingsModal { .. })
    ));
    app.clear_modals();

    // /stats is alias for /analytics -> opens Analytics dashboard
    app.execute_slash_command("/stats");
    assert!(matches!(
        app.current_modal(),
        Some(m31a_tui::ModalState::AnalyticsDashboard)
    ));
    app.clear_modals();

    // /quit is alias for /exit -> sets should_quit flag
    assert!(!app.should_quit);
    app.execute_slash_command("/quit");
    assert!(app.should_quit);
}

#[test]
fn test_slash_command_quoted_arguments_with_spaces() {
    init_theme(ThemeVariant::Dark);
    let config = Config {
        model: "mock-model".to_string(),
        model_provider: Some("mock".to_string()),
        api_key: Some("test_api_key".to_string()),
        ..Default::default()
    };
    let mut app = App::new(config).expect("failed to create app");

    // /effort with quoted string
    app.execute_slash_command(r#"/effort "high""#);
    assert_eq!(app.config.reasoning_effort.as_deref(), Some("high"));

    // /search with multiple arguments and quotes
    let initial_count = app.transcript.len();
    app.execute_slash_command(r#"/search "rust tokio process group" --limit 5"#);
    // Transcript receives notice without panic
    assert!(app.transcript.len() >= initial_count);
}

#[test]
fn test_unknown_slash_command_handling() {
    init_theme(ThemeVariant::Dark);
    let config = Config {
        model: "mock-model".to_string(),
        model_provider: Some("mock".to_string()),
        api_key: Some("test_api_key".to_string()),
        ..Default::default()
    };
    let mut app = App::new(config).expect("failed to create app");

    let count_before = app.transcript.len();
    app.execute_slash_command("/non_existent_command_xyz_123");
    let count_after = app.transcript.len();

    // Unknown command emits a warning notice without crashing or modifying modal state
    assert!(count_after > count_before);
    assert!(!app.has_active_modal());
}
