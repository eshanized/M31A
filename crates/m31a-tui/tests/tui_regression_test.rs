use m31a_core::Config;
use m31a_provider::models::ModelInfo;
use m31a_tui::app::App;
use m31a_tui::modals::{ApprovalActionKind, ModalState, PendingApproval};
use m31a_tui::model_selector::ModelListState;
use m31a_tui::transcript_view::wrap_line_to_visual_rows;
use ratatui::layout::Rect;
use ratatui::text::Line;

// -----------------------------------------------------------------------------
// 7.3 TUI Transcript Rendering Regression Tests
// -----------------------------------------------------------------------------

#[test]
fn test_transcript_wrapping_and_measurement_across_varying_widths() {
    let long_text = "This is a long continuous line that must wrap predictably across multiple different terminal column widths without coordinate mismatches or line drift.";
    let widths = [20, 40, 80, 120, 200];

    for &width in &widths {
        let line = Line::from(long_text.to_string());
        let rows = wrap_line_to_visual_rows(line, width);
        assert!(!rows.is_empty());
        // Verify every wrapped row fits within the specified column width
        for row in &rows {
            assert!(
                row.width() <= width,
                "Row exceeds width {width}: width={}",
                row.width()
            );
        }
    }
}

#[test]
fn test_unbreakable_tokens_wrap_predictably() {
    let long_token = "A".repeat(150); // Unbreakable token of 150 chars
    let width = 40;
    let line = Line::from(long_token);
    let rows = wrap_line_to_visual_rows(line, width);

    // 150 chars at 40 cols must wrap into ceil(150/40) = 4 rows
    assert_eq!(rows.len(), 4);
    assert_eq!(rows[0].width(), 40);
    assert_eq!(rows[1].width(), 40);
    assert_eq!(rows[2].width(), 40);
    assert_eq!(rows[3].width(), 30);
}

#[test]
fn test_multibyte_cjk_width_handling() {
    let cjk_text = "你好世界，这是一个全角字符测试。"; // 16 CJK fullwidth chars = 32 columns
    let width = 20; // 20 columns can hold 10 CJK chars per row
    let line = Line::from(cjk_text.to_string());
    let rows = wrap_line_to_visual_rows(line, width);

    for row in &rows {
        assert!(row.width() <= width);
    }
}

// -----------------------------------------------------------------------------
// 7.4 Approval Interface Regression Tests
// -----------------------------------------------------------------------------

#[test]
fn test_approval_action_kinds_and_modal_geometry() {
    let tool_call = m31a_core::types::ToolCall {
        id: "call_1".to_string(),
        call_type: "function".to_string(),
        function: m31a_core::types::FunctionCall {
            name: "exec_command".to_string(),
            arguments: r#"{"command": "cargo test --all"}"#.to_string(),
        },
    };
    let approval = PendingApproval::new(
        tool_call,
        "Execute shell command",
        Some("cargo test --all".to_string()),
    );

    let actions = approval.action_kinds();
    assert!(actions.contains(&ApprovalActionKind::AllowOnce));
    assert!(actions.contains(&ApprovalActionKind::AllowRule));
    assert!(actions.contains(&ApprovalActionKind::DeclineWithFeedback));
    assert!(actions.contains(&ApprovalActionKind::Cancel));

    // Verify modal geometry calculation consistency
    let modal = ModalState::ApprovalDialog {
        approval,
        selected_option: 0,
    };
    let screen = Rect::new(0, 0, 160, 50);
    let popup = modal.calculate_rect(screen);

    assert!(popup.width > 0 && popup.width <= screen.width);
    assert!(popup.height > 0 && popup.height <= screen.height);
    assert_eq!(modal.dimensions(), (72, 56));
}

// -----------------------------------------------------------------------------
// 7.5 Model Selector Scrolling & Filter Regression Tests
// -----------------------------------------------------------------------------

#[test]
fn test_large_model_catalog_scrolling_and_navigation() {
    let count = 150;
    let models: Vec<ModelInfo> = (0..count)
        .map(|i| ModelInfo {
            slug: format!("provider/model-{i:03}"),
            display_name: format!("Model {i:03}"),
            description: format!("Description for model {i:03}"),
            context_window: 128_000,
            max_output_tokens: 4096,
            supports_tools: true,
            supports_thinking: i % 2 == 0,
            ..Default::default()
        })
        .collect();

    let mut state = ModelListState::new(models, "provider/model-000".to_string());
    let viewport_h = 15;
    state.set_viewport_height(viewport_h);

    assert_eq!(state.total_filtered(), 150);
    assert_eq!(state.selected_index, 0);
    assert_eq!(state.scroll_offset, 0);

    // PageDown moves by viewport_height
    state.select_page_down();
    assert_eq!(state.selected_index, viewport_h);
    assert_eq!(state.scroll_offset, 1);

    // End jumps to last item
    state.select_end();
    assert_eq!(state.selected_index, 149);
    assert_eq!(state.scroll_offset, 150 - viewport_h);

    // Home jumps to first item
    state.select_home();
    assert_eq!(state.selected_index, 0);
    assert_eq!(state.scroll_offset, 0);

    // Filter restricts list
    state.set_search_query("model-04".to_string());
    assert_eq!(state.total_filtered(), 10); // 040 to 049
    assert_eq!(state.selected_index, 0);

    // Clear filter restores 150 items
    state.set_search_query("".to_string());
    assert_eq!(state.total_filtered(), 150);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_transactional_model_switching_updates_agent_runtime() {
    let wire_server = m31a_provider::wire_server::LocalWireServer::start()
        .await
        .expect("wire server start");
    let temp = tempfile::tempdir().unwrap();

    let config = Config {
        model: "meta/llama-3.3-70b-instruct".to_string(),
        api_base: wire_server.api_base(),
        api_key: Some(
            m31a_core::resolve_stored_api_key()
                .unwrap_or_else(|| "nvapi-runtime-integration-token".to_string()),
        ),
        ..Default::default()
    };
    let mut app = App::new_test(config, temp.path()).unwrap();

    assert_eq!(app.active_model, "meta/llama-3.3-70b-instruct");

    // Transactionally switch model
    let res = app.switch_model("deepseek-ai/deepseek-r1");
    assert!(res.is_ok());
    assert_eq!(app.active_model, "deepseek-ai/deepseek-r1");
    assert_eq!(app.nim_client.model(), "deepseek-ai/deepseek-r1");

    // Failed switch (missing credentials) preserves previous active model
    app.config.api_key = None;
    let fail_res = app.switch_model("mistralai/mistral-large-2407");
    assert!(fail_res.is_err());
    assert_eq!(app.active_model, "deepseek-ai/deepseek-r1");
    assert_eq!(app.nim_client.model(), "deepseek-ai/deepseek-r1");
}

#[test]
fn test_production_missing_credentials_fails_honestly() {
    let config = Config {
        model: "meta/llama-3.3-70b-instruct".to_string(),
        api_key: None,
        ..Default::default()
    };
    let app_res = App::new(config);
    assert!(app_res.is_err());
    let err_str = app_res.err().unwrap().to_string();
    assert!(err_str.contains("Missing API key") || err_str.contains("API key required"));
}

#[test]
fn test_model_list_rendering_single_row_invariant_and_scrollbar() {
    use m31a_tui::model_selector::{render_model_details, render_model_list};
    use m31a_tui::theme::Theme;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    let models: Vec<ModelInfo> = (0..50)
        .map(|i| ModelInfo {
            slug: format!("provider/model-{:03}", i),
            display_name: format!("Model {:03}", i),
            description: format!("Description for model {:03}", i),
            context_window: 131072,
            max_output_tokens: 4096,
            supports_tools: true,
            supports_thinking: i % 2 == 0,
            supports_vision: i % 3 == 0,
            originator: "NVIDIA".to_string(),
            ..Default::default()
        })
        .collect();

    let mut state = ModelListState::new(models.clone(), "provider/model-000".to_string());
    let area = Rect::new(0, 0, 70, 12);
    let inner_h = area.height.saturating_sub(2) as usize; // 10 rows
    state.set_viewport_height(inner_h);

    let backend = TestBackend::new(70, 12);
    let mut terminal = Terminal::new(backend).unwrap();
    let theme = Theme::everblush();

    terminal
        .draw(|f| {
            render_model_list(f, &state, area, &theme, "Model Registry");
        })
        .unwrap();

    let buffer = terminal.backend().buffer();

    // Line 0 is top border with title
    let line0: String = (0..70).map(|x| buffer[(x, 0)].symbol()).collect();
    assert!(line0.contains("Model Registry"));

    // Lines 1 through 10 are exactly 10 distinct models (1 terminal row per model)
    for y in 1..=inner_h as u16 {
        let row_text: String = (0..69).map(|x| buffer[(x, y)].symbol()).collect();
        let idx = (y - 1) as usize;
        assert!(
            row_text.contains(&format!("Model {:03}", idx)),
            "Row {} must contain Model {:03}, got: {}",
            y,
            idx,
            row_text
        );
        assert!(row_text.contains("[128k]"));
    }

    // Rightmost column of inner rows contains scrollbar symbols because 50 > 10
    let scroll_sym = buffer[(69, 1)].symbol();
    assert!(
        scroll_sym == "▲"
            || scroll_sym == "█"
            || scroll_sym == "│"
            || scroll_sym == "┐"
            || scroll_sym == "│",
        "Scrollbar indicator should appear on right border column: {}",
        scroll_sym
    );

    // Verify detail inspector rendering
    let detail_backend = TestBackend::new(60, 20);
    let mut detail_term = Terminal::new(detail_backend).unwrap();
    let sel_model = state.get_selected_model();
    detail_term
        .draw(|f| {
            render_model_details(f, sel_model, true, Rect::new(0, 0, 60, 20), &theme);
        })
        .unwrap();

    let detail_buf = detail_term.backend().buffer();
    let mut all_detail_text = String::new();
    for y in 0..20 {
        let row_text: String = (0..60).map(|x| detail_buf[(x, y)].symbol()).collect();
        all_detail_text.push_str(&row_text);
        all_detail_text.push('\n');
    }

    assert!(all_detail_text.contains("Model Specification"));
    assert!(all_detail_text.contains("Not Probed"));
    assert!(all_detail_text.contains("Model 000"));
    assert!(all_detail_text.contains("provider/model-000"));
    assert!(all_detail_text.contains("131072 tokens"));
    assert!(all_detail_text.contains("Tool Calling"));
    assert!(all_detail_text.contains("Reasoning / Thinking"));
    assert!(all_detail_text.contains("Vision"));
}

#[test]
fn test_modal_model_picker_keyboard_interaction_lifecycle() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use m31a_tui::modals::ModalState;
    use m31a_tui::terminal::handle_key_event;

    let temp = tempfile::tempdir().unwrap();
    let config = Config {
        model: "meta/llama-3.3-70b-instruct".to_string(),
        api_key: Some(
            m31a_core::resolve_stored_api_key()
                .unwrap_or_else(|| "nvapi-runtime-integration-token".to_string()),
        ),
        ..Default::default()
    };
    let mut app = App::new_test(config, temp.path()).expect("app init");

    // Open model picker
    handle_key_event(
        &mut app,
        KeyEvent::new(KeyCode::Char('m'), KeyModifiers::CONTROL),
    );
    assert!(matches!(
        app.current_modal(),
        Some(ModalState::ModelPicker { .. })
    ));

    // Navigate Down and Up
    handle_key_event(&mut app, KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    if let Some(ModalState::ModelPicker { state }) = app.current_modal() {
        assert_eq!(state.selected_index, 1);
    } else {
        panic!("Modal should still be ModelPicker");
    }

    handle_key_event(&mut app, KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));
    if let Some(ModalState::ModelPicker { state }) = app.current_modal() {
        assert_eq!(state.selected_index, 0);
    } else {
        panic!("Modal should still be ModelPicker");
    }

    // PageDown
    handle_key_event(
        &mut app,
        KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE),
    );
    if let Some(ModalState::ModelPicker { state }) = app.current_modal() {
        assert!(state.selected_index > 0);
    }

    // Home jumps back to top
    handle_key_event(&mut app, KeyEvent::new(KeyCode::Home, KeyModifiers::NONE));
    if let Some(ModalState::ModelPicker { state }) = app.current_modal() {
        assert_eq!(state.selected_index, 0);
    }

    // Typing characters updates search filter
    handle_key_event(
        &mut app,
        KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE),
    );
    handle_key_event(
        &mut app,
        KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE),
    );
    handle_key_event(
        &mut app,
        KeyEvent::new(KeyCode::Char('m'), KeyModifiers::NONE),
    );
    handle_key_event(
        &mut app,
        KeyEvent::new(KeyCode::Char('o'), KeyModifiers::NONE),
    );

    if let Some(ModalState::ModelPicker { state }) = app.current_modal() {
        assert!(state.search_query.contains("nemo"));
        assert!(state.total_filtered() > 0);
    }

    // Backspace removes char
    handle_key_event(
        &mut app,
        KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE),
    );
    if let Some(ModalState::ModelPicker { state }) = app.current_modal() {
        assert_eq!(state.search_query, "nem");
    }

    // Esc closes modal
    handle_key_event(&mut app, KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(!app.has_active_modal());
}
