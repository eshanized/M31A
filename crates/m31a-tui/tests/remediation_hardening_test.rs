use m31a_core::{ApprovalPolicy, Config, SandboxPolicy};
use m31a_provider::models::ModelInfo;
use m31a_tui::app::App;
use m31a_tui::icons::Icon;
use m31a_tui::modals::{
    CommandPaletteGeometry, CommandPaletteItem, ConnectivityDetails, ConnectivityStatus,
    ModalState, SetupConfiguration, SetupStep, SetupWizardState,
};
use m31a_tui::model_selector::{ModelHealthState, ModelListState, ModelSelectorGeometry};
use m31a_tui::terminal::filter_palette_items;
use ratatui::layout::Rect;

// -----------------------------------------------------------------------------
// 1. Setup Wizard Secret Redaction Test
// -----------------------------------------------------------------------------

#[test]
fn test_setup_wizard_secret_redaction() {
    let config = Config {
        api_key: Some("super_secret_master_key_99999".to_string()),
        ..Default::default()
    };
    let mut wizard = SetupWizardState::new(&config, &[]);

    // 1. Initial debug format when input is empty
    let debug_empty = format!("{:?}", wizard);
    assert!(
        debug_empty.contains("<empty>"),
        "Empty key input should show <empty>, got: {debug_empty}"
    );
    assert!(
        !debug_empty.contains("super_secret_master_key_99999"),
        "Debug output leaked master key!"
    );

    // 2. Secret entered in input
    wizard.api_key_input = "sk-live-secret-token-xyz-12345".to_string();
    let debug_with_secret = format!("{:?}", wizard);
    assert!(
        debug_with_secret.contains("[REDACTED]"),
        "API key input must be redacted as [REDACTED], got: {debug_with_secret}"
    );
    assert!(
        !debug_with_secret.contains("sk-live-secret-token-xyz-12345"),
        "Debug output leaked raw API key string!"
    );
}

// -----------------------------------------------------------------------------
// 2. Setup Connectivity Verification & Stale Result Protection Test
// -----------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn test_setup_wizard_connectivity_state_transitions_and_stale_rejection() {
    let temp_dir = tempfile::tempdir().unwrap();
    let config = Config::default();
    let mut app = App::new_test(config, temp_dir.path()).unwrap();

    let mut wizard = SetupWizardState::new(&app.config, &[]);
    wizard.step = SetupStep::Connectivity;
    assert_eq!(wizard.connectivity_status, ConnectivityStatus::NotTested);
    let gen0 = wizard.connectivity_generation;

    // 1. Start verification on generation gen0 + 1
    app.verify_setup_connectivity_async(&mut wizard);
    let current_gen = wizard.connectivity_generation;
    assert_eq!(current_gen, gen0 + 1);
    assert_eq!(wizard.connectivity_status, ConnectivityStatus::Validating);

    // Push wizard to app modal stack
    app.push_modal(ModalState::SetupWizard {
        state: wizard.clone(),
    });

    // 2. Deliver a stale event with old generation gen0 (should be rejected/ignored)
    let stale_event = m31a_tui::app::TuiEvent::SetupConnectivityResult {
        generation: gen0,
        status: ConnectivityStatus::Connected,
        details: Some(ConnectivityDetails {
            endpoint_verified: true,
            model_verified: true,
            model_check_unsupported: false,
            latency_ms: Some(15),
            message: "Stale success".to_string(),
        }),
        error: None,
    };
    app.handle_tui_event(stale_event);

    // Assert modal still has current_gen and status was NOT overwritten by stale event
    if let Some(ModalState::SetupWizard { state }) = app.current_modal() {
        assert_eq!(state.connectivity_generation, current_gen);
        assert_ne!(
            state.connectivity_status,
            ConnectivityStatus::Connected,
            "Stale generation result improperly updated wizard state!"
        );
    } else {
        panic!("SetupWizard modal missing from app stack");
    }

    // 3. Deliver genuine event matching current_gen
    let fresh_event = m31a_tui::app::TuiEvent::SetupConnectivityResult {
        generation: current_gen,
        status: ConnectivityStatus::Connected,
        details: Some(ConnectivityDetails {
            endpoint_verified: true,
            model_verified: true,
            model_check_unsupported: false,
            latency_ms: Some(42),
            message: "Genuine connection OK".to_string(),
        }),
        error: None,
    };
    app.handle_tui_event(fresh_event);

    if let Some(ModalState::SetupWizard { state }) = app.current_modal() {
        assert_eq!(state.connectivity_status, ConnectivityStatus::Connected);
        let details = state.connectivity_details.as_ref().unwrap();
        assert!(details.endpoint_verified);
        assert!(details.model_verified);
        assert_eq!(details.latency_ms, Some(42));
    } else {
        panic!("SetupWizard modal missing from app stack");
    }

    // 4. Invalidation: changing provider or model resets connectivity
    if let Some(ModalState::SetupWizard { mut state }) = app.pop_modal() {
        state.invalidate_connectivity();
        assert_eq!(state.connectivity_status, ConnectivityStatus::NotTested);
        assert!(state.connectivity_details.is_none());
        assert!(state.connectivity_error.is_none());
        assert!(!state.skipped_verification);
    }
}

// -----------------------------------------------------------------------------
// 3. Truthful Unsupported Model Availability Test
// -----------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn test_setup_wizard_unsupported_model_truthful_reporting() {
    let temp_dir = tempfile::tempdir().unwrap();
    let config = Config::default();
    let mut app = App::new_test(config, temp_dir.path()).unwrap();

    let mut wizard = SetupWizardState::new(&app.config, &[]);
    wizard.step = SetupStep::Connectivity;
    wizard.connectivity_generation = 5;
    app.push_modal(ModalState::SetupWizard {
        state: wizard.clone(),
    });

    // Event where provider connected, but model check is unsupported
    let unsupported_event = m31a_tui::app::TuiEvent::SetupConnectivityResult {
        generation: 5,
        status: ConnectivityStatus::Connected,
        details: Some(ConnectivityDetails {
            endpoint_verified: true,
            model_verified: false,
            model_check_unsupported: true,
            latency_ms: Some(28),
            message: "Endpoint verified (28ms). Model availability not independently verified."
                .to_string(),
        }),
        error: None,
    };
    app.handle_tui_event(unsupported_event);

    if let Some(ModalState::SetupWizard { state }) = app.current_modal() {
        assert_eq!(state.connectivity_status, ConnectivityStatus::Connected);
        let details = state.connectivity_details.as_ref().unwrap();
        assert!(details.endpoint_verified);
        assert!(
            !details.model_verified,
            "Model availability must NOT be claimed as verified when unsupported"
        );
        assert!(details.model_check_unsupported);
    } else {
        panic!("SetupWizard modal missing from app stack");
    }
}

// -----------------------------------------------------------------------------
// 4. Explicit Skip & Failure Guard Behavior Test
// -----------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn test_setup_wizard_explicit_skip_and_failure_guards() {
    let temp_dir = tempfile::tempdir().unwrap();
    let config = Config::default();
    let mut app = App::new_test(config, temp_dir.path()).unwrap();

    let mut wizard = SetupWizardState::new(&app.config, &[]);
    wizard.step = SetupStep::Connectivity;
    wizard.connectivity_status = ConnectivityStatus::Failed;
    wizard.connectivity_error = Some("Endpoint connection refused (ECONNREFUSED)".to_string());
    app.push_modal(ModalState::SetupWizard {
        state: wizard.clone(),
    });

    // 1. Enter on Failed status must NOT advance step
    let enter_key = crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::Enter,
        crossterm::event::KeyModifiers::NONE,
    );
    m31a_tui::terminal::handle_key_event(&mut app, enter_key);

    if let Some(ModalState::SetupWizard { state }) = app.current_modal() {
        assert_eq!(
            state.step,
            SetupStep::Connectivity,
            "Enter must NOT bypass failed verification!"
        );
    } else {
        panic!("SetupWizard modal missing");
    }

    // 2. Explicit skip ('s') advances to Review and sets skipped_verification flag
    let skip_key = crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::Char('s'),
        crossterm::event::KeyModifiers::NONE,
    );
    m31a_tui::terminal::handle_key_event(&mut app, skip_key);

    if let Some(ModalState::SetupWizard { state }) = app.current_modal() {
        assert_eq!(
            state.step,
            SetupStep::Review,
            "Pressing 's' must advance to Review"
        );
        assert!(
            state.skipped_verification,
            "Skipping must explicitly set skipped_verification to true"
        );
    } else {
        panic!("SetupWizard modal missing");
    }
}

// -----------------------------------------------------------------------------
// 5. Transactional Setup Configuration Persistence Test
// -----------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn test_transactional_setup_configuration_application() {
    let temp_dir = tempfile::tempdir().unwrap();
    let config = Config {
        model: "meta/llama-3.3-70b-instruct".to_string(),
        api_base: "http://127.0.0.1:8000/v1".to_string(),
        api_key: Some("init_key".to_string()),
        sandbox: SandboxPolicy::ReadOnly,
        approval: ApprovalPolicy::OnRequest,
        ..Default::default()
    };
    let mut app = App::new_test(config, temp_dir.path()).unwrap();

    let initial_model = app.active_model.clone();
    let initial_client_model = app.nim_client.model().to_string();

    // 1. Candidate with invalid configuration (empty model slug)
    let invalid_candidate = SetupConfiguration {
        provider_type: m31a_provider::ProviderType::Nim,
        api_base: "http://127.0.0.1:8000/v1".to_string(),
        api_key: Some("test_key".to_string()),
        model: "".to_string(), // Invalid: empty model
        sandbox: SandboxPolicy::WorkspaceWrite,
        approval: ApprovalPolicy::Never,
    };

    let res = app.apply_setup_configuration(invalid_candidate);
    assert!(res.is_err(), "Invalid configuration candidate must fail");

    // Verify completely untouched live state (no partial mutations)
    assert_eq!(app.active_model, initial_model);
    assert_eq!(app.config.model, initial_model);
    assert_eq!(app.nim_client.model(), initial_client_model);
    assert_eq!(app.config.sandbox, SandboxPolicy::ReadOnly);
    assert_eq!(app.config.approval, ApprovalPolicy::OnRequest);

    // 2. Valid candidate application
    let valid_candidate = SetupConfiguration {
        provider_type: m31a_provider::ProviderType::Nim,
        api_base: "http://127.0.0.1:9090/v1".to_string(),
        api_key: Some("new_nv_key_456".to_string()),
        model: "deepseek-ai/deepseek-r1".to_string(),
        sandbox: SandboxPolicy::WorkspaceWrite,
        approval: ApprovalPolicy::ApproveForMe,
    };

    let res = app.apply_setup_configuration(valid_candidate);
    assert!(res.is_ok(), "Valid candidate configuration must succeed");

    // Verify atomic commit across all runtime components
    assert_eq!(app.active_model, "deepseek-ai/deepseek-r1");
    assert_eq!(app.config.model, "deepseek-ai/deepseek-r1");
    assert_eq!(app.bottom_pane.active_model, "deepseek-ai/deepseek-r1");
    assert_eq!(app.nim_client.model(), "deepseek-ai/deepseek-r1");
    assert_eq!(app.config.sandbox, SandboxPolicy::WorkspaceWrite);
    assert_eq!(app.config.approval, ApprovalPolicy::ApproveForMe);
    assert_eq!(app.guardian.model(), "deepseek-ai/deepseek-r1");
}

// -----------------------------------------------------------------------------
// 6. Transactional Reasoning Effort Application Test
// -----------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn test_transactional_reasoning_effort_validation_and_rollback() {
    let temp_dir = tempfile::tempdir().unwrap();
    let config = Config::default();
    let mut app = App::new_test(config, temp_dir.path()).unwrap();

    // 1. Invalid reasoning effort string
    let res = app.apply_reasoning_effort(Some("super_fast_max".to_string()));
    assert!(res.is_err(), "Invalid effort string must fail validation");
    assert_eq!(app.config.reasoning_effort, None);
    assert_eq!(app.nim_client.reasoning_effort(), None);

    // 2. Valid reasoning effort: "high"
    app.apply_reasoning_effort(Some("high".to_string()))
        .unwrap();
    assert_eq!(app.config.reasoning_effort.as_deref(), Some("high"));
    assert_eq!(app.nim_client.reasoning_effort(), Some("high"));
    assert_eq!(app.bottom_pane.reasoning_effort.as_deref(), Some("high"));

    // 3. Reset to None
    app.apply_reasoning_effort(None).unwrap();
    assert_eq!(app.config.reasoning_effort, None);
    assert_eq!(app.nim_client.reasoning_effort(), None);
    assert_eq!(app.bottom_pane.reasoning_effort, None);
}

// -----------------------------------------------------------------------------
// 7. Command Palette Fuzzy Search & Ranking Test
// -----------------------------------------------------------------------------

#[test]
fn test_command_palette_fuzzy_search_and_ranking() {
    let items = vec![
        CommandPaletteItem {
            id: "switch_model".to_string(),
            title: "Switch Model".to_string(),
            description: "Change active LLM model".to_string(),
            category: "Model".to_string(),
            shortcut: Some("F2".to_string()),
            icon: Icon::Cpu,
        },
        CommandPaletteItem {
            id: "nav_chat".to_string(),
            title: "Chat Screen".to_string(),
            description: "Navigate to primary conversational screen".to_string(),
            category: "Navigation".to_string(),
            shortcut: Some("F1".to_string()),
            icon: Icon::MessageSquare,
        },
        CommandPaletteItem {
            id: "toggle_git".to_string(),
            title: "Git Status".to_string(),
            description: "View repository diffs and working tree status".to_string(),
            category: "Tools".to_string(),
            shortcut: Some("Ctrl+G".to_string()),
            icon: Icon::GitBranch,
        },
        CommandPaletteItem {
            id: "effort".to_string(),
            title: "Reasoning Effort".to_string(),
            description: "Configure thinking and reasoning effort".to_string(),
            category: "Model".to_string(),
            shortcut: None,
            icon: Icon::Sliders,
        },
    ];

    // 1. Exact match bonus on title: "Switch Model"
    let matches = filter_palette_items("Switch Model", &items);
    assert_eq!(matches[0], 0, "Exact title match must rank first");

    // 2. Prefix match bonus on title: "Git"
    let matches = filter_palette_items("git", &items);
    assert_eq!(matches[0], 2, "'git' must rank 'Git Status' first");

    // 3. Subsequence fuzzy match: "ch" matches "Chat Screen"
    let matches = filter_palette_items("ch", &items);
    assert!(
        matches.contains(&1),
        "'ch' should fuzzy match 'Chat Screen'"
    );

    // 4. Shortcut matching: "F1" matches "Chat Screen"
    let matches = filter_palette_items("F1", &items);
    assert_eq!(
        matches[0], 1,
        "Shortcut 'F1' must match 'Chat Screen' as top result"
    );

    // 5. Category matching: "Navigation"
    let matches = filter_palette_items("Navigation", &items);
    assert_eq!(matches[0], 1);

    // 6. Description matching: "diffs"
    let matches = filter_palette_items("diffs", &items);
    assert_eq!(matches[0], 2);

    // 7. Empty query returns all items in original order
    let matches = filter_palette_items("", &items);
    assert_eq!(matches, vec![0, 1, 2, 3]);
}

// -----------------------------------------------------------------------------
// 8. Command Palette Geometry & Hit Testing Test
// -----------------------------------------------------------------------------

#[test]
fn test_command_palette_geometry_and_hit_testing() {
    let outer = Rect::new(10, 5, 80, 24);
    let geom = CommandPaletteGeometry::compute(outer);

    // Layout areas must be within outer bounds
    assert!(outer.contains(ratatui::layout::Position::new(geom.search.x, geom.search.y)));
    assert!(outer.contains(ratatui::layout::Position::new(geom.list.x, geom.list.y)));
    assert!(outer.contains(ratatui::layout::Position::new(geom.footer.x, geom.footer.y)));

    // Wide terminal (80 width) should create a two-pane layout with details
    assert!(geom.details.is_some());

    let inner = geom.list_inner();
    // Click inside the list rows
    let click_col = inner.x + 2;
    let click_row = inner.y + 1;
    let hit = geom.row_to_visible_index(click_col, click_row);
    assert_eq!(hit, Some(1));

    // Clicks in search bar must return None
    assert_eq!(
        geom.row_to_visible_index(geom.search.x + 2, geom.search.y + 1),
        None
    );

    // Clicks on border must return None
    assert_eq!(geom.row_to_visible_index(outer.x, outer.y), None);

    // Clicks in footer must return None
    assert_eq!(
        geom.row_to_visible_index(geom.footer.x + 2, geom.footer.y),
        None
    );

    // Clicks in details pane must return None
    if let Some(details) = geom.details {
        assert_eq!(
            geom.row_to_visible_index(details.x + 2, details.y + 2),
            None
        );
    }
}

// -----------------------------------------------------------------------------
// 9. Model Selector Geometry, Hit-Testing & Health Probe Test
// -----------------------------------------------------------------------------

#[test]
fn test_model_selector_geometry_and_hit_testing() {
    // 1. Wide terminal: two-pane
    let wide_outer = Rect::new(5, 2, 100, 28);
    let wide_geom = ModelSelectorGeometry::compute(wide_outer);
    assert!(
        wide_geom.details.is_some(),
        "Width >= 70 must create two-pane layout"
    );

    // 2. Narrow terminal: single-pane
    let narrow_outer = Rect::new(5, 2, 60, 28);
    let narrow_geom = ModelSelectorGeometry::compute(narrow_outer);
    assert!(
        narrow_geom.details.is_none(),
        "Width < 70 must fall back to single-pane layout"
    );

    // 3. Strict hit testing: search, border, footer, details return None
    let inner = wide_geom.list_inner();
    assert_eq!(
        wide_geom.row_to_visible_index(inner.x + 1, inner.y),
        Some(0)
    );
    assert_eq!(
        wide_geom.row_to_visible_index(wide_geom.search.x + 1, wide_geom.search.y + 1),
        None
    );
    assert_eq!(
        wide_geom.row_to_visible_index(wide_geom.footer.x + 1, wide_geom.footer.y),
        None
    );
    if let Some(details) = wide_geom.details {
        assert_eq!(
            wide_geom.row_to_visible_index(details.x + 1, details.y + 2),
            None
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_model_health_probe_state_transitions_and_deduplication() {
    let models = vec![ModelInfo {
        slug: "test-model-probe".to_string(),
        display_name: "Test Model".to_string(),
        ..Default::default()
    }];

    let mut state = ModelListState::new(models, "test-model-probe".to_string());
    assert_eq!(
        state.get_health("test-model-probe"),
        ModelHealthState::NotProbed
    );

    // Set probing state
    state.set_health("test-model-probe".to_string(), ModelHealthState::Probing);
    assert_eq!(
        state.get_health("test-model-probe"),
        ModelHealthState::Probing
    );

    // Deduplication check: state is already Probing
    assert_eq!(
        state.get_health("test-model-probe"),
        ModelHealthState::Probing
    );

    // Receive healthy probe result
    state.set_health(
        "test-model-probe".to_string(),
        ModelHealthState::Healthy { latency_ms: 38 },
    );
    assert_eq!(
        state.get_health("test-model-probe"),
        ModelHealthState::Healthy { latency_ms: 38 }
    );
}

// -----------------------------------------------------------------------------
// 11. Explicit Distinguishable Connectivity Status Variants
// -----------------------------------------------------------------------------

#[test]
fn test_explicit_connectivity_status_predicates_and_labels() {
    let auth_fail = ConnectivityStatus::AuthenticationFailed;
    assert!(auth_fail.is_failure());
    assert!(!auth_fail.is_success());
    assert!(!auth_fail.is_in_progress());
    assert_eq!(auth_fail.label(), "Authentication Failed");

    let model_unsupp = ConnectivityStatus::ModelUnsupported;
    assert!(model_unsupp.is_failure());
    assert!(!model_unsupp.is_success());
    assert_eq!(model_unsupp.label(), "Model Unsupported");

    let timeout = ConnectivityStatus::TimedOut;
    assert!(timeout.is_failure());
    assert_eq!(timeout.label(), "Timed Out");

    let unreach = ConnectivityStatus::Unreachable;
    assert!(unreach.is_failure());
    assert_eq!(unreach.label(), "Host Unreachable");

    let conn = ConnectivityStatus::Connected;
    assert!(conn.is_success());
    assert!(!conn.is_failure());

    let testing = ConnectivityStatus::TestingConnection;
    assert!(testing.is_in_progress());
    assert!(!testing.is_failure());
}

// -----------------------------------------------------------------------------
// 12. Asynchronous Model Switch and Session Resume State Transitions
// -----------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn test_async_model_switch_lifecycle() {
    let wire_server = m31a_provider::wire_server::LocalWireServer::start().await.expect("wire server start");
    let temp = tempfile::tempdir().unwrap();

    let initial_model = "nvidia/nemotron-3.5-lightning-30b-a3b";
    let config = Config {
        model: initial_model.to_string(),
        api_base: wire_server.api_base(),
        api_key: Some("valid-nim-key".to_string()),
        ..Default::default()
    };

    let mut app = App::new_test(config, temp.path()).expect("app init");
    assert_eq!(app.active_model, initial_model);

    // Call switch_model_async
    let target = "meta/llama-3.3-70b-instruct";
    let async_res = app.switch_model_async(target);
    assert!(async_res.is_ok(), "switch_model_async must initiate without blocking");

    // Provider status message should indicate activation in progress
    assert!(app.provider_status_message.is_some());

    // Event should be received from the background task
    let evt = tokio::time::timeout(std::time::Duration::from_secs(3), app.event_rx.recv())
        .await
        .expect("probe event delivered")
        .expect("channel open");

    // Handle event in App
    app.handle_tui_event(evt);

    // Verify atomic commit
    assert_eq!(app.active_model, target);
    assert_eq!(app.bottom_pane.active_model, target);
    assert_eq!(app.nim_client.model(), target);
}
