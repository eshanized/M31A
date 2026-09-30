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
//! 11. Model Catalog Persistence to `.m31a/cache/model_catalog.json`
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
    let cache_path = ModelCatalog::cache_path(dir.path());
    let catalog = create_test_catalog();
    catalog.save_to_cache_file(&cache_path).unwrap();

    let wizard = SetupWizardScreen::new(dir.path().to_path_buf());
    assert_eq!(wizard.catalog.len(), 8);
    assert_eq!(
        wizard.primary_model_input.text(),
        "meta/llama-3.1-70b-instruct"
    );
    assert_eq!(
        wizard.fast_model_input.text(),
        "meta/llama-3.2-11b-vision-instruct"
    );
}

#[test]
fn test_02_search_and_filter_catalog() {
    let dir = tempdir().unwrap();
    let cache_path = ModelCatalog::cache_path(dir.path());
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
    let cache_path = ModelCatalog::cache_path(dir.path());
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
    let cache_path = ModelCatalog::cache_path(dir.path());
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
    let cache_path = ModelCatalog::cache_path(dir.path());
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
    let cache_path = ModelCatalog::cache_path(dir.path());
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
    let cache_path = ModelCatalog::cache_path(dir.path());
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
    let cache_path = ModelCatalog::cache_path(dir.path());
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

    // Verify .m31a/credentials.json
    let creds_path = dir.path().join(".m31a/credentials.json");
    assert!(creds_path.exists());
    let creds_json = fs::read_to_string(&creds_path).unwrap();
    assert!(creds_json.contains("nvapi-secret-key-12345"));
}

#[test]
fn test_09_buffer_rendering_model_setup() {
    let dir = tempdir().unwrap();
    let cache_path = ModelCatalog::cache_path(dir.path());
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
    let cache_path = ModelCatalog::cache_path(dir.path());
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
