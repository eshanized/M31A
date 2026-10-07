//! Golden Test Suite for Setup Wizard Dynamic Model Selection (FRX-02, MDL-01, MDL-02).
//!
//! Verifies:
//! 1. Dynamic Discovery Loads Live/Cached Catalog (>2 models)
//! 2. Live Substring Search & Filter
//! 3. Keyboard Navigation (Up, Down, PageUp, PageDown)
//! 4. Primary Role Assignment ('1' / 'p')
//! 5. Fast Auxiliary Role Assignment ('2' / 'a')
//! 6. Disjoint Model Selection (Primary != Fast)
//! 7. Role Eligibility Enforcement (Tool Calling & Context Capacity)
//! 8. Rejection of Ineligible Models (Embeddings, Rewards, Guardrails)
//! 9. Configuration Persistence to `.m31a/config.toml`
//! 10. Credentials Persistence to `.m31a/credentials.json`
//! 11. Model Catalog Persistence to the channel-aware cache file
//! 12. Offline / Cached Catalog Startup
//! 13. Live Refresh Dynamic Triggering
//! 14. Full Ratatui Buffer Rendering & Inspection Panel
//! 15. TUI Composer /model Autocomplete from Dynamic Cache

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use std::fs;
use tempfile::tempdir;

use m31a::init::lifecycle::SetupStep;
use m31a::model::catalog::ModelCatalog;
use m31a::model::router::resolver::{ModelCandidate, ModelTier};
use m31a::model::types::ProviderCapabilityStatus;
use m31a::tui::composer::TuiComposer;
use m31a::tui::screens::wizard::SetupWizardScreen;

/// Helper to generate a realistic multi-model catalog fixture
fn create_test_catalog() -> ModelCatalog {
    let candidates = vec![
        ModelCandidate::new(
            "meta/llama-3.1-70b-instruct",
            "nvidia",
            ModelTier::Standard,
            131072,
        )
        .with_display_name("Llama 3.1 70B Instruct")
        .with_tool_support(true)
        .with_availability(ProviderCapabilityStatus::Available),
        ModelCandidate::new(
            "meta/llama-3.2-11b-vision-instruct",
            "nvidia",
            ModelTier::Fast,
            131072,
        )
        .with_display_name("Llama 3.2 11B Vision Instruct")
        .with_tool_support(true)
        .with_availability(ProviderCapabilityStatus::Available),
        ModelCandidate::new(
            "deepseek-ai/deepseek-r1",
            "nvidia",
            ModelTier::Reasoning,
            65536,
        )
        .with_display_name("DeepSeek R1 Reasoning")
        .with_tool_support(true)
        .with_availability(ProviderCapabilityStatus::Available),
        ModelCandidate::new(
            "mistralai/mixtral-8x22b-instruct-v0.1",
            "nvidia",
            ModelTier::Standard,
            65536,
        )
        .with_display_name("Mixtral 8x22B Instruct")
        .with_tool_support(true)
        .with_availability(ProviderCapabilityStatus::Available),
        ModelCandidate::new(
            "nvidia/llama-3.1-nemotron-70b-instruct",
            "nvidia",
            ModelTier::Standard,
            131072,
        )
        .with_display_name("Nemotron 70B Instruct")
        .with_tool_support(true)
        .with_availability(ProviderCapabilityStatus::Available),
        // Ineligible for primary: no tool support
        ModelCandidate::new("meta/llama-3-8b", "nvidia", ModelTier::Fast, 8192)
            .with_display_name("Llama 3 8B Base (No Tools)")
            .with_tool_support(false)
            .with_availability(ProviderCapabilityStatus::Available),
        // Ineligible for primary: context too small (<4096)
        ModelCandidate::new("legacy/tiny-model-2k", "nvidia", ModelTier::Fast, 2048)
            .with_display_name("Tiny Model 2k Context")
            .with_tool_support(true)
            .with_availability(ProviderCapabilityStatus::Available),
        // Ineligible: embedding model
        ModelCandidate::new("nvidia/nv-embedqa-e5-v5", "nvidia", ModelTier::Fast, 8192)
            .with_display_name("NV Embed QA E5")
            .with_tool_support(false)
            .with_availability(ProviderCapabilityStatus::Available),
    ];

    ModelCatalog::from_discovered("nvidia", candidates, 1700000000)
}

#[test]
fn test_01_wizard_loads_catalog_and_auto_assigns_defaults() {
    let dir = tempdir().unwrap();
    // Fixtures target the channel-aware cache so the wizard (which loads
    // the channel-aware store) observes them on every build channel.
    let cache_path = ModelCatalog::cache_path_for_channel(
        dir.path(),
        m31a::deployment::DeploymentChannel::current(),
    );
    let catalog = create_test_catalog();
    catalog.save_to_cache_file(&cache_path).unwrap();

    let wizard = SetupWizardScreen::new(dir.path().to_path_buf());
    assert_eq!(wizard.catalog.len(), 8);
    // Single model authority: the wizard defaults to the canonical default
    // model (config::canonical), not a competing wizard-local literal.
    // The fast auxiliary comes from the catalog's fast-default selection.
    assert_eq!(
        wizard.primary_model_input.text(),
        m31a::config::canonical::CANONICAL_DEFAULT_MODEL
    );
    assert!(
        wizard.catalog.models.iter().any(|c| c.model_id == wizard.fast_model_input.text()),
        "fast model must come from the catalog, got '{}'",
        wizard.fast_model_input.text()
    );
}

#[test]
fn test_02_search_and_filter_catalog() {
    let dir = tempdir().unwrap();
    // Fixtures target the channel-aware cache so the wizard (which loads
    // the channel-aware store) observes them on every build channel.
    let cache_path = ModelCatalog::cache_path_for_channel(
        dir.path(),
        m31a::deployment::DeploymentChannel::current(),
    );
    create_test_catalog()
        .save_to_cache_file(&cache_path)
        .unwrap();

    let mut wizard = SetupWizardScreen::new(dir.path().to_path_buf());

    // Initially all 8 models
    assert_eq!(wizard.filtered_models().len(), 8);

    // Search "deepseek"
    wizard.model_search_input.set_text("deepseek");
    let filtered = wizard.filtered_models();
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].model_id, "deepseek-ai/deepseek-r1");

    // Search "nemotron"
    wizard.model_search_input.set_text("nemotron");
    let filtered = wizard.filtered_models();
    assert_eq!(filtered.len(), 1);
    assert_eq!(
        filtered[0].model_id,
        "nvidia/llama-3.1-nemotron-70b-instruct"
    );

    // Clear search
    wizard.model_search_input.clear();
    assert_eq!(wizard.filtered_models().len(), 8);
}

fn advance_to_model_setup(wizard: &mut SetupWizardScreen) {
    let _ = fs::create_dir_all(wizard.workspace_path().join(".git"));
    wizard.refresh_diagnostics();
    wizard.trust_confirmed = true;
    let _ = wizard.advance(); // step 1 -> step 2: DoctorDiagnostics
    wizard.warnings_acknowledged = true;
    let _ = wizard.advance(); // step 2 -> step 3: ProviderSetup
    wizard.api_key_input.set_text("nvapi-test-dummy-key");
    let _ = wizard.advance(); // step 3 -> step 4: ModelSetup
    assert_eq!(wizard.current_step(), SetupStep::ModelSetup);
}

#[test]
fn test_03_keyboard_navigation_in_model_setup() {
    let dir = tempdir().unwrap();
    // Fixtures target the channel-aware cache so the wizard (which loads
    // the channel-aware store) observes them on every build channel.
    let cache_path = ModelCatalog::cache_path_for_channel(
        dir.path(),
        m31a::deployment::DeploymentChannel::current(),
    );
    create_test_catalog()
        .save_to_cache_file(&cache_path)
        .unwrap();

    let mut wizard = SetupWizardScreen::new(dir.path().to_path_buf());
    advance_to_model_setup(&mut wizard);

    assert_eq!(wizard.selected_model_index, 0);

    // Down arrow
    wizard.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::empty()));
    assert_eq!(wizard.selected_model_index, 1);

    // Down arrow again
    wizard.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::empty()));
    assert_eq!(wizard.selected_model_index, 2);

    // Up arrow
    wizard.handle_key(KeyEvent::new(KeyCode::Up, KeyModifiers::empty()));
    assert_eq!(wizard.selected_model_index, 1);

    // PageDown
    wizard.handle_key(KeyEvent::new(KeyCode::PageDown, KeyModifiers::empty()));
    assert_eq!(wizard.selected_model_index, 7); // capped at len - 1

    // PageUp
    wizard.handle_key(KeyEvent::new(KeyCode::PageUp, KeyModifiers::empty()));
    assert_eq!(wizard.selected_model_index, 1);
}

#[test]
fn test_04_primary_role_assignment() {
    let dir = tempdir().unwrap();
    // Fixtures target the channel-aware cache so the wizard (which loads
    // the channel-aware store) observes them on every build channel.
    let cache_path = ModelCatalog::cache_path_for_channel(
        dir.path(),
        m31a::deployment::DeploymentChannel::current(),
    );
    create_test_catalog()
        .save_to_cache_file(&cache_path)
        .unwrap();

    let mut wizard = SetupWizardScreen::new(dir.path().to_path_buf());
    advance_to_model_setup(&mut wizard);

    // Select index 2: "deepseek-ai/deepseek-r1"
    wizard.selected_model_index = 2;
    wizard.handle_key(KeyEvent::new(KeyCode::Char('1'), KeyModifiers::empty()));

    assert_eq!(wizard.primary_model_input.text(), "deepseek-ai/deepseek-r1");
    assert!(
        wizard
            .status_message
            .as_ref()
            .unwrap()
            .contains("Selected 'deepseek-ai/deepseek-r1'")
    );
}

#[test]
fn test_05_fast_auxiliary_role_assignment() {
    let dir = tempdir().unwrap();
    // Fixtures target the channel-aware cache so the wizard (which loads
    // the channel-aware store) observes them on every build channel.
    let cache_path = ModelCatalog::cache_path_for_channel(
        dir.path(),
        m31a::deployment::DeploymentChannel::current(),
    );
    create_test_catalog()
        .save_to_cache_file(&cache_path)
        .unwrap();

    let mut wizard = SetupWizardScreen::new(dir.path().to_path_buf());
    advance_to_model_setup(&mut wizard);

    // Select index 3: "mistralai/mixtral-8x22b-instruct-v0.1"
    wizard.selected_model_index = 3;
    wizard.handle_key(KeyEvent::new(KeyCode::Char('2'), KeyModifiers::empty()));

    assert_eq!(
        wizard.fast_model_input.text(),
        "mistralai/mixtral-8x22b-instruct-v0.1"
    );
    assert!(
        wizard
            .status_message
            .as_ref()
            .unwrap()
            .contains("Selected 'mistralai/mixtral-8x22b-instruct-v0.1'")
    );
}

#[test]
fn test_06_disjoint_role_assignments() {
    let dir = tempdir().unwrap();
    // Fixtures target the channel-aware cache so the wizard (which loads
    // the channel-aware store) observes them on every build channel.
    let cache_path = ModelCatalog::cache_path_for_channel(
        dir.path(),
        m31a::deployment::DeploymentChannel::current(),
    );
    create_test_catalog()
        .save_to_cache_file(&cache_path)
        .unwrap();

    let mut wizard = SetupWizardScreen::new(dir.path().to_path_buf());
    advance_to_model_setup(&mut wizard);

    // Assign primary to Nemotron
    wizard.selected_model_index = 4;
    wizard.handle_key(KeyEvent::new(KeyCode::Char('1'), KeyModifiers::empty()));

    // Assign fast auxiliary to Mixtral
    wizard.selected_model_index = 3;
    wizard.handle_key(KeyEvent::new(KeyCode::Char('2'), KeyModifiers::empty()));

    assert_eq!(
        wizard.primary_model_input.text(),
        "nvidia/llama-3.1-nemotron-70b-instruct"
    );
    assert_eq!(
        wizard.fast_model_input.text(),
        "mistralai/mixtral-8x22b-instruct-v0.1"
    );
    assert_ne!(
        wizard.primary_model_input.text(),
        wizard.fast_model_input.text()
    );
}

#[test]
fn test_07_role_eligibility_enforcement() {
    let dir = tempdir().unwrap();
    // Fixtures target the channel-aware cache so the wizard (which loads
    // the channel-aware store) observes them on every build channel.
    let cache_path = ModelCatalog::cache_path_for_channel(
        dir.path(),
        m31a::deployment::DeploymentChannel::current(),
    );
    create_test_catalog()
        .save_to_cache_file(&cache_path)
        .unwrap();

    let mut wizard = SetupWizardScreen::new(dir.path().to_path_buf());
    advance_to_model_setup(&mut wizard);

    let original_primary = wizard.primary_model_input.text().to_string();

    // Index 5: "meta/llama-3-8b" lacks tool support -> cannot be Primary!
    wizard.selected_model_index = 5;
    wizard.handle_key(KeyEvent::new(KeyCode::Char('1'), KeyModifiers::empty()));
    assert_eq!(wizard.primary_model_input.text(), original_primary);
    assert!(
        wizard
            .status_message
            .as_ref()
            .unwrap()
            .contains("Ineligible for Primary")
    );

    // Index 6: "legacy/tiny-model-2k" context < 4096 -> cannot be Primary!
    wizard.selected_model_index = 6;
    wizard.handle_key(KeyEvent::new(KeyCode::Char('1'), KeyModifiers::empty()));
    assert_eq!(wizard.primary_model_input.text(), original_primary);
    assert!(
        wizard
            .status_message
            .as_ref()
            .unwrap()
            .contains("Ineligible for Primary")
    );

    // Index 7: "nvidia/nv-embedqa-e5-v5" embedding -> cannot be Primary or Fast!
    wizard.selected_model_index = 7;
    wizard.handle_key(KeyEvent::new(KeyCode::Char('1'), KeyModifiers::empty()));
    assert_eq!(wizard.primary_model_input.text(), original_primary);
    assert!(
        wizard
            .status_message
            .as_ref()
            .unwrap()
            .contains("Ineligible for Primary")
    );

    let original_fast = wizard.fast_model_input.text().to_string();
    wizard.handle_key(KeyEvent::new(KeyCode::Char('2'), KeyModifiers::empty()));
    assert_eq!(wizard.fast_model_input.text(), original_fast);
    assert!(
        wizard
            .status_message
            .as_ref()
            .unwrap()
            .contains("Ineligible for Fast Auxiliary")
    );
}

#[test]
fn test_08_configuration_and_credentials_persistence() {
    let dir = tempdir().unwrap();
    // Fixtures target the channel-aware cache so the wizard (which loads
    // the channel-aware store) observes them on every build channel.
    let cache_path = ModelCatalog::cache_path_for_channel(
        dir.path(),
        m31a::deployment::DeploymentChannel::current(),
    );
    create_test_catalog()
        .save_to_cache_file(&cache_path)
        .unwrap();

    let mut wizard = SetupWizardScreen::new(dir.path().to_path_buf());
    wizard.api_key_input.set_text("nvapi-secret-key-12345");
    wizard
        .primary_model_input
        .set_text("meta/llama-3.1-70b-instruct");
    wizard
        .fast_model_input
        .set_text("meta/llama-3.2-11b-vision-instruct");

    // Persist configuration
    wizard
        .persist_configuration()
        .expect("persistence must succeed");

    // Verify .m31a/config.toml
    let config_path = dir.path().join(".m31a/config.toml");
    assert!(config_path.exists());
    let config_toml = fs::read_to_string(&config_path).unwrap();
    assert!(config_toml.contains("default_model = \"meta/llama-3.1-70b-instruct\""));
    assert!(config_toml.contains("fast_auxiliary_model = \"meta/llama-3.2-11b-vision-instruct\""));
    assert!(config_toml.contains("default = \"nvidia_nim\""));

    // Verify channel-aware credentials store: canonical GLOBAL user store
    // (platform config, channel-isolated). The workspace must never receive
    // secrets from onboarding. The wizard persists to the canonical store
    // for the artifact channel under test — never cross-channel.
    let global_creds_path =
        m31a::config::provider_registry::ProviderRegistry::global_credentials_path_for_workspace(
            dir.path(),
        );
    assert!(global_creds_path.exists());
    let creds_json = fs::read_to_string(&global_creds_path).unwrap();
    assert!(creds_json.contains("nvapi-secret-key-12345"));
    // No project pollution: workspace must not contain credentials.
    assert!(!dir.path().join(".m31a").join("credentials.json").exists());
    assert!(
        !dir.path()
            .join(".m31a")
            .join("credentials-dev.json")
            .exists()
    );
}

#[test]
fn test_09_buffer_rendering_model_setup() {
    let dir = tempdir().unwrap();
    // Fixtures target the channel-aware cache so the wizard (which loads
    // the channel-aware store) observes them on every build channel.
    let cache_path = ModelCatalog::cache_path_for_channel(
        dir.path(),
        m31a::deployment::DeploymentChannel::current(),
    );
    create_test_catalog()
        .save_to_cache_file(&cache_path)
        .unwrap();

    let mut wizard = SetupWizardScreen::new(dir.path().to_path_buf());
    advance_to_model_setup(&mut wizard);

    let backend = TestBackend::new(120, 36);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
        .draw(|f| {
            wizard.render(f, f.area());
        })
        .expect("render must not panic");

    let buffer = terminal.backend().buffer().clone();
    let content = (0..buffer.area.height)
        .map(|y| {
            (0..buffer.area.width)
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<String>>()
        .join("\n");

    // Telemetry and UI elements
    assert!(content.contains("Provider: NVIDIA NIM"));
    assert!(content.contains("Catalog: 8 discovered"));
    assert!(content.contains("Model Metadata & Eligibility"));
    assert!(content.contains("[PRIMARY]"));
    assert!(content.contains("[FAST]"));
    assert!(content.contains("Active Role Assignments"));
    assert!(content.contains("Tab/") || content.contains("Tab"));
}

#[test]
fn test_10_composer_autocomplete_from_cached_catalog() {
    let dir = tempdir().unwrap();
    // Fixtures target the channel-aware cache so the wizard (which loads
    // the channel-aware store) observes them on every build channel.
    let cache_path = ModelCatalog::cache_path_for_channel(
        dir.path(),
        m31a::deployment::DeploymentChannel::current(),
    );
    create_test_catalog()
        .save_to_cache_file(&cache_path)
        .unwrap();

    let mut composer = TuiComposer::new(dir.path().to_path_buf());

    // Type "/model " to open model autocomplete
    composer.set_text("/model ");
    assert!(composer.is_autocomplete_open());

    // Type "/model deepseek"
    composer.set_text("/model deepseek");
    assert!(composer.is_autocomplete_open());

    // Type "/model llama"
    composer.set_text("/model llama");
    assert!(composer.is_autocomplete_open());
}

#[test]
fn test_11_buffer_rendering_distinct_context_windows() {
    let dir = tempdir().unwrap();
    // Fixtures target the channel-aware cache so the wizard (which loads
    // the channel-aware store) observes them on every build channel.
    let cache_path = ModelCatalog::cache_path_for_channel(
        dir.path(),
        m31a::deployment::DeploymentChannel::current(),
    );

    let candidates = vec![
        ModelCandidate::new("test/model-131k", "nvidia", ModelTier::Standard, 131072)
            .with_display_name("Model 131K")
            .with_tool_support(true)
            .with_context_provenance("provider:max_model_len"),
        ModelCandidate::new("test/model-65k", "nvidia", ModelTier::Standard, 65536)
            .with_display_name("Model 65K")
            .with_tool_support(true)
            .with_context_provenance("provider:context_window"),
        ModelCandidate::new("test/model-32k", "nvidia", ModelTier::Standard, 32768)
            .with_display_name("Model 32K")
            .with_tool_support(true)
            .with_context_provenance("provider:context_length"),
        ModelCandidate::new("test/model-16k", "nvidia", ModelTier::Standard, 16384)
            .with_display_name("Model 16K")
            .with_tool_support(true)
            .with_context_provenance("provider:max_sequence_length"),
        ModelCandidate::new("test/model-8k", "nvidia", ModelTier::Fast, 8192)
            .with_display_name("Model 8K")
            .with_tool_support(true)
            .with_context_provenance("provider:max_position_embeddings"),
        ModelCandidate::new("test/model-2k", "nvidia", ModelTier::Fast, 2048)
            .with_display_name("Model 2K")
            .with_tool_support(true)
            .with_context_provenance("provider:max_tokens"),
        ModelCandidate::new("test/model-unknown", "nvidia", ModelTier::Fast, 0)
            .with_display_name("Model Unknown")
            .with_tool_support(true)
            .with_context_provenance("unknown"),
    ];

    let catalog = ModelCatalog::from_discovered("nvidia", candidates, 1700000000);
    catalog.save_to_cache_file(&cache_path).unwrap();

    let mut wizard = SetupWizardScreen::new(dir.path().to_path_buf());
    advance_to_model_setup(&mut wizard);

    let backend = TestBackend::new(120, 36);
    let mut terminal = Terminal::new(backend).unwrap();

    // 1. Initial render - Model 131K selected (index 0)
    terminal
        .draw(|f| {
            wizard.render(f, f.area());
        })
        .unwrap();

    let render_to_string = |term: &Terminal<TestBackend>| -> String {
        let buf = term.backend().buffer().clone();
        (0..buf.area.height)
            .map(|y| {
                (0..buf.area.width)
                    .map(|x| buf[(x, y)].symbol())
                    .collect::<String>()
            })
            .collect::<Vec<String>>()
            .join("\n")
    };

    let content_0 = render_to_string(&terminal);

    // Verify distinct capacity indicators in the list column
    assert!(content_0.contains("131k"), "Buffer should show 131k");
    assert!(content_0.contains("65k"), "Buffer should show 65k");
    assert!(content_0.contains("32k"), "Buffer should show 32k");
    assert!(content_0.contains("16k"), "Buffer should show 16k");
    assert!(content_0.contains("8k"), "Buffer should show 8k");
    assert!(content_0.contains("2k"), "Buffer should show 2k");
    assert!(
        content_0.contains("unk"),
        "Buffer should show unk for unknown context"
    );

    // Details panel for Model 131K (index 0) must show exact token count
    assert!(
        content_0.contains("131072 tokens"),
        "Details panel must display exact token count 131072 tokens"
    );

    // 2. Select Model 65K (index 1)
    wizard.selected_model_index = 1;
    terminal.draw(|f| wizard.render(f, f.area())).unwrap();
    let content_1 = render_to_string(&terminal);
    assert!(
        content_1.contains("65536 tokens"),
        "Details panel must display exact token count 65536 tokens"
    );

    // 3. Select Model 32K (index 2)
    wizard.selected_model_index = 2;
    terminal.draw(|f| wizard.render(f, f.area())).unwrap();
    let content_2 = render_to_string(&terminal);
    assert!(
        content_2.contains("32768 tokens"),
        "Details panel must display exact token count 32768 tokens"
    );

    // 4. Select Model 16K (index 3)
    wizard.selected_model_index = 3;
    terminal.draw(|f| wizard.render(f, f.area())).unwrap();
    let content_3 = render_to_string(&terminal);
    assert!(
        content_3.contains("16384 tokens"),
        "Details panel must display exact token count 16384 tokens"
    );

    // 5. Select Model Unknown (index 6)
    wizard.selected_model_index = 6;
    terminal.draw(|f| wizard.render(f, f.area())).unwrap();
    let content_6 = render_to_string(&terminal);
    assert!(
        content_6.contains("Unknown (unreported by provider)"),
        "Details panel must display explicit unknown notice"
    );
    assert!(
        content_6.contains("Context window is") && content_6.contains("unreported by provider"),
        "Eligibility evaluation must fail closed on unknown context"
    );

    // Ensure unknown model cannot be assigned to roles
    let orig_primary = wizard.primary_model_input.text().to_string();
    wizard.handle_key(KeyEvent::new(KeyCode::Char('1'), KeyModifiers::empty()));
    assert_eq!(wizard.primary_model_input.text(), orig_primary);
    assert!(
        wizard
            .status_message
            .as_ref()
            .unwrap()
            .contains("Ineligible for Primary")
    );
}

#[test]
fn test_12_catalog_exact_context_persistence_matrix() {
    let dir = tempdir().unwrap();
    // Fixtures target the channel-aware cache so the wizard (which loads
    // the channel-aware store) observes them on every build channel.
    let cache_path = ModelCatalog::cache_path_for_channel(
        dir.path(),
        m31a::deployment::DeploymentChannel::current(),
    );

    let candidates = vec![
        ModelCandidate::new("vendor/model-a", "nvidia", ModelTier::Standard, 131072)
            .with_context_provenance("provider:max_model_len"),
        ModelCandidate::new("vendor/model-b", "nvidia", ModelTier::Standard, 65536)
            .with_context_provenance("provider:max_model_len"),
        ModelCandidate::new("vendor/model-c", "nvidia", ModelTier::Standard, 32768)
            .with_context_provenance("provider:context_window"),
        ModelCandidate::new("vendor/model-d", "nvidia", ModelTier::Standard, 16384)
            .with_context_provenance("provider:context_length"),
    ];

    let catalog = ModelCatalog::from_discovered("nvidia", candidates, 1700000000);
    catalog.save_to_cache_file(&cache_path).unwrap();

    let loaded = ModelCatalog::load_from_cache_file(&cache_path).unwrap();
    assert_eq!(loaded.models.len(), 4);
    assert_eq!(loaded.models[0].context_capacity, 131072);
    assert_eq!(loaded.models[1].context_capacity, 65536);
    assert_eq!(loaded.models[2].context_capacity, 32768);
    assert_eq!(loaded.models[3].context_capacity, 16384);
}
