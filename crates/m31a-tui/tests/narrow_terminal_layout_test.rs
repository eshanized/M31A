//! Narrow Terminal Layout and Responsive UI Testing (`narrow_terminal_layout_test.rs`).

use m31a_core::Config;
use m31a_core::types::ChatMessage;
use m31a_tui::{App, ModalState, ThemeVariant, init_theme, renderer::render};
use ratatui::Terminal;
use ratatui::backend::TestBackend;

#[test]
fn test_responsive_layout_across_standard_and_narrow_viewports() {
    init_theme(ThemeVariant::Dark);
    let config = Config {
        model: "mock-model".to_string(),
        model_provider: Some("mock".to_string()),
        api_key: Some("test_api_key".to_string()),
        ..Default::default()
    };

    let mut app = App::new(config).expect("failed to create app");

    // Populate transcript with multiline content, code blocks, and notices
    app.chat_history.push(ChatMessage::user(
        "Show me the implementation of the fast path parser.",
    ));
    app.chat_history.push(ChatMessage::assistant(
        Some("```rust\npub fn parse_fast(input: &str) -> Result<Token> {\n    Ok(Token::Fast)\n}\n```".to_string()),
        None,
    ));
    app.transcript
        .push(m31a_tui::transcript::TranscriptItem::user(
            "Explain architecture",
        ));
    app.transcript
        .push(m31a_tui::transcript::TranscriptItem::assistant(
            "Here is the architectural overview.",
        ));

    let viewports = [
        (120, 40, "Standard 120x40"),
        (80, 24, "Standard 80x24 VT100"),
        (60, 20, "Compact 60x20"),
        (40, 15, "Ultra-narrow 40x15"),
    ];

    for (w, h, label) in viewports {
        let backend = TestBackend::new(w, h);
        let mut terminal = Terminal::new(backend).expect("failed to create terminal");

        // 1. Idle main screen rendering
        let res = terminal.draw(|f| {
            render(f, &mut app);
        });
        assert!(res.is_ok(), "Failed to render idle UI at {label} ({w}x{h})");

        // 2. Help Modal rendering
        app.push_modal(ModalState::help());
        let res = terminal.draw(|f| {
            render(f, &mut app);
        });
        assert!(
            res.is_ok(),
            "Failed to render Help modal at {label} ({w}x{h})"
        );

        // 3. Settings Modal rendering
        app.replace_modal(ModalState::settings());
        let res = terminal.draw(|f| {
            render(f, &mut app);
        });
        assert!(
            res.is_ok(),
            "Failed to render Settings modal at {label} ({w}x{h})"
        );

        // 4. Model Picker Modal rendering
        app.replace_modal(ModalState::model_picker(
            app.available_models.clone(),
            &app.active_model,
        ));
        let res = terminal.draw(|f| {
            render(f, &mut app);
        });
        assert!(
            res.is_ok(),
            "Failed to render Model Picker at {label} ({w}x{h})"
        );

        // 5. Analytics Dashboard rendering
        app.replace_modal(ModalState::AnalyticsDashboard);
        let res = terminal.draw(|f| {
            render(f, &mut app);
        });
        assert!(
            res.is_ok(),
            "Failed to render Analytics Dashboard at {label} ({w}x{h})"
        );

        app.clear_modals();
    }
}
