use m31a_core::{ApprovalPolicy, Config, SandboxPolicy};
use m31a_provider::models::ModelInfo;
use m31a_tui::app::App;
use m31a_tui::icons::{Icon, IconMode};
use m31a_tui::modals::{ModalState, SetupStep, SetupWizardState};
use m31a_tui::model_selector::{ModelListState, render_model_list};
use m31a_tui::theme::Theme;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;

// -----------------------------------------------------------------------------
// 1. Dynamic Context Silence & World State Regression Tests
// -----------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn test_dynamic_context_silence_and_digest_tracking() {
    let temp_dir = tempfile::tempdir().unwrap();
    let config = Config {
        api_key: Some("test_api_key_12345".to_string()),
        ..Default::default()
    };
    let mut app = App::new_test(config, temp_dir.path()).unwrap();

    // Verify initial transcript does not contain internal dynamic context noise
    for item in &app.transcript {
        if let m31a_tui::transcript::TranscriptItem::Notice { text, .. } = item {
            assert!(
                !text.contains("Dynamic context updated"),
                "Startup transcript contained context leak: {text}"
            );
            assert!(
                !text.contains("~workspace_status"),
                "Startup transcript contained internal section id: {text}"
            );
        }
    }

    let initial_count = app.transcript.len();

    // Repeatedly refresh system prompt
    app.refresh_system_prompt();
    app.refresh_system_prompt();

    // Must be completely silent - no new transcript items
    assert_eq!(
        app.transcript.len(),
        initial_count,
        "System prompt refresh appended noisy notices to transcript"
    );

    // Verify world-state digests are maintained
    assert!(
        !app.world_state_digests.is_empty(),
        "World state digests should be tracked"
    );

    // Modify a file in workspace
    let test_file = temp_dir.path().join("test_file.rs");
    std::fs::write(&test_file, b"pub fn hello() {}\n").unwrap();

    // Refresh again with workspace changes
    app.refresh_system_prompt();

    // Still completely silent
    assert_eq!(
        app.transcript.len(),
        initial_count,
        "Workspace change refresh leaked dynamic context notice to transcript"
    );
}

// -----------------------------------------------------------------------------
// 2. Centralized Lucide Icon System Tests
// -----------------------------------------------------------------------------

#[test]
fn test_lucide_icon_system_and_fallbacks() {
    let theme_lucide = Theme::everblush().with_icon_mode(IconMode::Lucide);
    let theme_unicode = Theme::everblush().with_icon_mode(IconMode::Unicode);
    let theme_ascii = Theme::everblush().with_icon_mode(IconMode::Ascii);

    let test_icons = [
        Icon::MessageSquare,
        Icon::Cpu,
        Icon::Folder,
        Icon::Terminal,
        Icon::CircleCheck,
        Icon::AlertTriangle,
        Icon::Sparkles,
        Icon::Sliders,
        Icon::Command,
    ];

    for icon in test_icons {
        // Lucide mode returns valid glyph
        let lucide_str = icon.render(&theme_lucide);
        assert!(!lucide_str.is_empty());

        // Unicode fallback mode returns non-empty clean symbol
        let unicode_str = icon.render(&theme_unicode);
        assert!(!unicode_str.is_empty());

        // ASCII mode returns ASCII string
        let ascii_str = icon.render(&theme_ascii);
        assert!(!ascii_str.is_empty());
        assert!(
            ascii_str.is_ascii(),
            "Ascii mode returned non-ascii: {ascii_str}"
        );
    }
}

#[test]
fn test_theme_typography_icon_mode_propagation() {
    let mut theme = Theme::everblush();
    theme.typography.use_icons = false;
    // When use_icons is false, effective mode must be Ascii
    assert_eq!(theme.effective_icon_mode(), IconMode::Ascii);

    theme.typography.use_icons = true;
    theme.icon_mode = IconMode::Lucide;
    assert_eq!(theme.effective_icon_mode(), IconMode::Lucide);
}

// -----------------------------------------------------------------------------
// 3. Command Palette Responsive Layout & Scrolling Tests
// -----------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn test_command_palette_rendering_across_viewports() {
    let temp = tempfile::tempdir().unwrap();
    let config = Config {
        api_key: Some("test_key".to_string()),
        ..Default::default()
    };
    let mut app = App::new_test(config, temp.path()).unwrap();
    let items = ModalState::default_command_palette_items();
    app.push_modal(ModalState::command_palette(items));

    let viewports = [(80, 24), (100, 30), (120, 36), (160, 45)];

    for &(w, h) in &viewports {
        let backend = TestBackend::new(w, h);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                m31a_tui::renderer::render(f, &mut app);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let mut found_title = false;
        let mut found_search = false;

        for y in 0..h {
            let row_str: String = (0..w).map(|x| buffer[(x, y)].symbol()).collect();
            if row_str.contains("Command Palette") {
                found_title = true;
            }
            if row_str.contains("search commands") || row_str.contains("Type to search") {
                found_search = true;
            }
        }

        assert!(found_title, "Command palette title missing at {w}x{h}");
        assert!(found_search, "Search bar missing at {w}x{h}");
    }
}

// -----------------------------------------------------------------------------
// 4. Model Selector Two-Pane Layout Tests
// -----------------------------------------------------------------------------

#[test]
fn test_model_selector_two_pane_and_narrow_fallback() {
    let models = vec![
        ModelInfo {
            slug: "meta/llama-3.3-70b-instruct".to_string(),
            display_name: "Llama 3.3 70B Instruct".to_string(),
            originator: "Meta".to_string(),
            description: "High performance open weights model".to_string(),
            context_window: 131_072,
            max_output_tokens: 4_096,
            supports_tools: true,
            supports_thinking: true,
            supports_vision: false,
            ..Default::default()
        },
        ModelInfo {
            slug: "nvidia/llama-3.1-nemotron-70b-instruct".to_string(),
            display_name: "Nemotron 70B".to_string(),
            originator: "NVIDIA".to_string(),
            description: "Refined reasoning and coding model".to_string(),
            context_window: 131_072,
            max_output_tokens: 4_096,
            supports_tools: true,
            supports_thinking: false,
            supports_vision: true,
            ..Default::default()
        },
    ];

    let state = ModelListState::new(models, "meta/llama-3.3-70b-instruct".to_string());
    let theme = Theme::everblush();

    // 1. Wide viewport (100x30) -> Two panes
    let backend_wide = TestBackend::new(100, 30);
    let mut term_wide = Terminal::new(backend_wide).unwrap();
    term_wide
        .draw(|f| {
            render_model_list(f, &state, Rect::new(0, 0, 100, 30), &theme, "Models");
        })
        .unwrap();

    let buf_wide = term_wide.backend().buffer();
    let mut found_details = false;
    for y in 0..30 {
        let row_str: String = (0..100).map(|x| buf_wide[(x, y)].symbol()).collect();
        if row_str.contains("SPECIFICATIONS")
            || row_str.contains("CAPABILITIES")
            || row_str.contains("Context Window")
        {
            found_details = true;
            break;
        }
    }
    assert!(
        found_details,
        "Wide layout should render specifications/details pane"
    );

    // 2. Narrow viewport (60x20) -> Single pane fallback
    let backend_narrow = TestBackend::new(60, 20);
    let mut term_narrow = Terminal::new(backend_narrow).unwrap();
    term_narrow
        .draw(|f| {
            render_model_list(f, &state, Rect::new(0, 0, 60, 20), &theme, "Models");
        })
        .unwrap();

    let buf_narrow = term_narrow.backend().buffer();
    let mut found_list = false;
    for y in 0..20 {
        let row_str: String = (0..60).map(|x| buf_narrow[(x, y)].symbol()).collect();
        if row_str.contains("Llama 3.3") {
            found_list = true;
            break;
        }
    }
    assert!(found_list, "Narrow layout should render model row");
}

// -----------------------------------------------------------------------------
// 5. Interactive Setup Wizard Workflow Tests
// -----------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn test_setup_wizard_lifecycle_navigation_and_persistence() {
    let temp_dir = tempfile::tempdir().unwrap();
    let config = Config {
        api_key: Some("orig_key".to_string()),
        model: "meta/llama-3.3-70b-instruct".to_string(),
        api_base: "https://integrate.api.nvidia.com/v1".to_string(),
        sandbox: SandboxPolicy::WorkspaceWrite,
        approval: ApprovalPolicy::OnRequest,
        ..Default::default()
    };
    let mut app = App::new_test(config, temp_dir.path()).unwrap();

    // Create setup wizard state
    let mut wizard = SetupWizardState::new(&app.config, &app.available_models);

    // Initial step is Welcome
    assert_eq!(wizard.step, SetupStep::Welcome);

    // Step navigation
    wizard.next_step();
    assert_eq!(wizard.step, SetupStep::Provider);

    wizard.next_step();
    assert_eq!(wizard.step, SetupStep::Credentials);

    // Key input
    wizard.preserve_existing_key = false;
    wizard.api_key_input = "sk-new-super-secret-key".to_string();

    wizard.next_step();
    assert_eq!(wizard.step, SetupStep::Model);

    wizard.next_step();
    assert_eq!(wizard.step, SetupStep::Security);
    wizard.sandbox_policy = SandboxPolicy::ReadOnly;
    wizard.approval_policy = ApprovalPolicy::Never;

    wizard.next_step();
    assert_eq!(wizard.step, SetupStep::Connectivity);

    wizard.next_step();
    assert_eq!(wizard.step, SetupStep::Review);

    // Backwards navigation
    assert!(wizard.can_go_back());
    wizard.previous_step();
    assert_eq!(wizard.step, SetupStep::Connectivity);
    wizard.next_step();
    assert_eq!(wizard.step, SetupStep::Review);

    // Persist configuration
    let persist_res = app.persist_setup_wizard_config(&wizard);
    assert!(
        persist_res.is_ok(),
        "Failed to persist wizard config: {:?}",
        persist_res.err()
    );

    assert_eq!(
        app.config.api_key.as_deref(),
        Some("sk-new-super-secret-key")
    );
    assert_eq!(app.config.sandbox, SandboxPolicy::ReadOnly);
    assert_eq!(app.config.approval, ApprovalPolicy::Never);
    assert_eq!(app.bottom_pane.sandbox_policy, "ReadOnly");
}

// -----------------------------------------------------------------------------
// 6. Interactive Reasoning Effort Dialog Tests
// -----------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn test_reasoning_effort_dialog_and_application() {
    let temp_dir = tempfile::tempdir().unwrap();
    let config = Config {
        reasoning_effort: None,
        ..Default::default()
    };
    let mut app = App::new_test(config, temp_dir.path()).unwrap();

    // Initial state is None
    assert_eq!(app.config.reasoning_effort, None);
    assert_eq!(app.nim_client.reasoning_effort(), None);
    assert_eq!(app.bottom_pane.reasoning_effort, None);

    // 1. Direct command override
    app.execute_slash_command("/effort high");
    assert_eq!(app.config.reasoning_effort.as_deref(), Some("high"));
    assert_eq!(app.nim_client.reasoning_effort(), Some("high"));
    assert_eq!(app.bottom_pane.reasoning_effort.as_deref(), Some("high"));

    // 2. /effort without args opens modal dialog
    app.execute_slash_command("/effort");
    assert!(matches!(
        app.current_modal(),
        Some(ModalState::ReasoningEffort { .. })
    ));

    // Cancel modal without changes
    app.pop_modal();
    assert_eq!(app.config.reasoning_effort.as_deref(), Some("high"));

    // 3. Apply via helper (simulating Enter in dialog)
    app.apply_reasoning_effort(Some("low".to_string()));
    assert_eq!(app.config.reasoning_effort.as_deref(), Some("low"));
    assert_eq!(app.nim_client.reasoning_effort(), Some("low"));
    assert_eq!(app.bottom_pane.reasoning_effort.as_deref(), Some("low"));

    // 4. Reset to None
    app.execute_slash_command("/effort none");
    assert_eq!(app.config.reasoning_effort, None);
    assert_eq!(app.nim_client.reasoning_effort(), None);
    assert_eq!(app.bottom_pane.reasoning_effort, None);
}
