//! Integration Tests for Phase 13 Plan 03 (FRX-01–FRX-03, CFX-01, CFX-02, CFX-04).
//!
//! Verifies:
//! 1. Durable initialization lifecycle state machine, sentinel file, and SQLite migration.
//! 2. 7-step setup wizard navigation and doctor diagnostic probes.
//! 3. Multi-provider registry, masked secrets, and 7-layer configuration provenance.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use serde_json::json;
use std::fs;
use std::path::PathBuf;
use tempfile::tempdir;

use m31a::config::provenance::{ConfigLayer, ConfigurationService, ProvenanceError};
use m31a::config::provider_registry::{MaskedSecret, ProviderRegistry, ProviderType};
use m31a::init::doctor::{DiagnosticStatus, DoctorEngine};
use m31a::init::lifecycle::{InitError, InitManager, InitState, SetupStep};
use m31a::persistence::sqlite::initialize_database;
use m31a::tui::screens::wizard::{SetupWizardScreen, WizardOutcome};

#[tokio::test]
async fn test_initialization_lifecycle_and_sentinel() {
    let tmp = tempdir().expect("create tempdir");
    let ws_path = tmp.path().to_path_buf();

    // 1. New manager starts as Uninitialized
    let mut manager = InitManager::new(&ws_path).expect("init manager");
    assert_eq!(*manager.current_state(), InitState::Uninitialized);
    assert!(!manager.is_sentinel_present());

    // 2. Illegal jump: Uninitialized cannot jump directly to Onboarded
    let illegal = manager.transition_to(InitState::Onboarded);
    assert!(matches!(illegal, Err(InitError::InvalidTransition { .. })));

    // 3. Legal progression: Uninitialized -> Checking -> Configuring(WorkspaceTrust)
    manager
        .transition_to(InitState::Checking)
        .expect("checking");
    assert_eq!(*manager.current_state(), InitState::Checking);
    assert!(manager.is_sentinel_present());

    manager
        .transition_to(InitState::Configuring(SetupStep::WorkspaceTrust))
        .expect("step 1");
    assert_eq!(
        *manager.current_state(),
        InitState::Configuring(SetupStep::WorkspaceTrust)
    );

    // 4. Save step data
    manager
        .save_step_data(
            SetupStep::WorkspaceTrust,
            json!({ "trusted": true, "path": ws_path.to_str().unwrap() }),
        )
        .expect("save step data");

    let saved_trust = manager
        .get_step_data(SetupStep::WorkspaceTrust)
        .expect("get step data");
    assert_eq!(saved_trust["trusted"], true);

    // 5. Navigate through wizard steps
    let mut current_step = SetupStep::WorkspaceTrust;
    while let Some(next_step) = current_step.next() {
        manager
            .transition_to(InitState::Configuring(next_step))
            .expect("advance step");
        current_step = next_step;
    }
    assert_eq!(current_step, SetupStep::FinalVerification);

    // 6. FinalVerification -> Verifying -> Ready
    manager
        .transition_to(InitState::Verifying)
        .expect("verifying");
    manager.transition_to(InitState::Ready).expect("ready");
    assert_eq!(*manager.current_state(), InitState::Ready);

    // 7. Restart safety: reload InitManager from disk, verify state and data preserved
    let reloaded = InitManager::new(&ws_path).expect("reload manager");
    assert_eq!(*reloaded.current_state(), InitState::Ready);
    let reloaded_trust = reloaded
        .get_step_data(SetupStep::WorkspaceTrust)
        .expect("reloaded trust");
    assert_eq!(reloaded_trust["trusted"], true);

    // 8. Migration to SQLite database
    let db_path = ws_path.join(".m31a").join("runtime.db");
    fs::create_dir_all(ws_path.join(".m31a")).expect("create .m31a");
    let pool = initialize_database(&db_path)
        .await
        .expect("initialize schema");

    manager
        .complete_and_migrate_to_db(&pool)
        .await
        .expect("migrate to db");
    assert_eq!(*manager.current_state(), InitState::Onboarded);
    assert!(manager.is_onboarded());

    // Verify row in system_state table
    let row: (String, String) =
        sqlx::query_as("SELECT key, value FROM system_state WHERE key = 'init_state'")
            .fetch_one(&pool)
            .await
            .expect("fetch init_state from db");
    assert_eq!(row.0, "init_state");
    assert_eq!(row.1, "Onboarded");
}

#[test]
fn test_wizard_and_doctor_diagnostics() {
    let tmp = tempdir().expect("tempdir");
    let ws_path = tmp.path().to_path_buf();

    // 1. Doctor diagnostics probes
    let doctor = DoctorEngine::new();
    let git_probe = doctor.check_git_installed();
    // In standard linux environments git is installed
    assert!(
        git_probe.status == DiagnosticStatus::Pass || git_probe.status == DiagnosticStatus::Fail
    );
    assert!(git_probe.is_mandatory);

    // Directory is not a git repo initially -> should fail repository probe
    let repo_probe = doctor.check_git_repository(&ws_path);
    assert_eq!(repo_probe.status, DiagnosticStatus::Fail);
    assert!(repo_probe.is_mandatory);

    // Initialize mock doctor to test deterministic gate evaluation
    let mock_doctor = DoctorEngine::new().with_mock_mode(true);
    let mock_probes = mock_doctor.run_all(&ws_path);
    assert_eq!(mock_probes.len(), 6);
    assert!(!mock_doctor.has_blocking_failures(&mock_probes));

    // 2. SetupWizardScreen navigation and validation gates
    let mut wizard = SetupWizardScreen::new(ws_path.clone());
    assert_eq!(wizard.current_step(), SetupStep::WorkspaceTrust);

    // Attempting to advance without confirming trust must fail
    assert!(!wizard.can_advance());
    let outcome = wizard.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));
    assert!(matches!(outcome, WizardOutcome::Error(_)));
    assert_eq!(wizard.current_step(), SetupStep::WorkspaceTrust);

    // Toggle trust confirmation
    let _ = wizard.handle_key(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::empty()));
    assert!(wizard.trust_confirmed);
    assert!(wizard.can_advance());

    // Advance to Step 2: Doctor Diagnostics
    let outcome = wizard.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));
    assert_eq!(
        outcome,
        WizardOutcome::Advanced(SetupStep::DoctorDiagnostics)
    );
    assert_eq!(wizard.current_step(), SetupStep::DoctorDiagnostics);

    // If warnings exist, test acknowledgement toggle
    if wizard.doctor.has_warnings(wizard.probes()) {
        assert!(!wizard.warnings_acknowledged);
        let _ = wizard.handle_key(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::empty()));
        assert!(wizard.warnings_acknowledged);
    }

    // Step backward test
    let outcome = wizard.handle_key(KeyEvent::new(KeyCode::Char('b'), KeyModifiers::empty()));
    assert_eq!(outcome, WizardOutcome::Back(SetupStep::WorkspaceTrust));
    assert_eq!(wizard.current_step(), SetupStep::WorkspaceTrust);

    // Step forward again
    let outcome = wizard.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));
    assert_eq!(
        outcome,
        WizardOutcome::Advanced(SetupStep::DoctorDiagnostics)
    );

    // 3. Test Ratatui buffer rendering of Wizard across multiple steps
    let backend = TestBackend::new(100, 30);
    let mut terminal = Terminal::new(backend).expect("create test terminal");

    terminal
        .draw(|f| {
            wizard.render(f, f.area());
        })
        .expect("draw wizard step 2");

    let buffer = terminal.backend().buffer().clone();
    let content = (0..buffer.area.height)
        .map(|y| {
            (0..buffer.area.width)
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<String>>()
        .join("\n");

    assert!(content.contains("M31A FIRST-RUN SETUP WIZARD"));
    assert!(content.contains("Step 2/7: Doctor Diagnostics"));
}

#[test]
fn test_provider_registry_and_provenance() {
    let tmp = tempdir().expect("tempdir");
    let creds_path = tmp.path().join(".m31a").join("credentials.json");

    // 1. Multi-Provider Registry Initialization
    let mut registry = ProviderRegistry::new();
    let descriptors = registry.all_descriptors();
    assert_eq!(descriptors.len(), 5);

    let openai = registry.get_descriptor("openai").expect("openai");
    assert_eq!(openai.provider_type, ProviderType::OpenAI);
    assert!(openai.requires_api_key);

    let anthropic = registry.get_descriptor("anthropic").expect("anthropic");
    assert_eq!(anthropic.provider_type, ProviderType::Anthropic);

    let gemini = registry.get_descriptor("gemini").expect("gemini");
    assert_eq!(gemini.provider_type, ProviderType::Gemini);

    let nim = registry.get_descriptor("nvidia_nim").expect("nvidia_nim");
    assert_eq!(nim.provider_type, ProviderType::NvidiaNim);

    let local = registry.get_descriptor("openai_compatible").expect("local");
    assert_eq!(local.provider_type, ProviderType::OpenAICompatible);
    assert!(!local.requires_api_key);

    // 2. MaskedSecret behavior (D-08, T-13-08)
    let secret = MaskedSecret::new("sk-ant-api03-abcdef1234567890");
    assert_eq!(secret.expose_secret(), "sk-ant-api03-abcdef1234567890");
    // Display and Debug must mask the secret
    assert_eq!(format!("{}", secret), "sk-a...[MASKED]");
    assert_eq!(format!("{:?}", secret), "sk-a...[MASKED]");
    // Serialization must NOT expose raw token
    let serialized = serde_json::to_string(&secret).expect("serialize secret");
    assert!(!serialized.contains("abcdef1234567890"));
    assert!(serialized.contains("sk-a...[MASKED]"));

    // 3. Credential storage and isolation (0600 permissions)
    registry.set_credential("anthropic", "sk-ant-secret-token-value");
    registry.set_credential("openai", "sk-openai-secret-token-value");
    registry
        .save_credentials_to_file(&creds_path)
        .expect("save credentials");
    assert!(creds_path.exists());

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = fs::metadata(&creds_path).expect("metadata").permissions();
        assert_eq!(perms.mode() & 0o777, 0o600);
    }

    // Load credentials into fresh registry
    let mut fresh_registry = ProviderRegistry::new();
    assert!(!fresh_registry.has_credential("anthropic"));
    fresh_registry
        .load_credentials_from_file(&creds_path)
        .expect("load credentials");
    assert!(fresh_registry.has_credential("anthropic"));
    assert_eq!(
        fresh_registry
            .get_credential("anthropic")
            .unwrap()
            .expose_secret(),
        "sk-ant-secret-token-value"
    );

    // 4. Test connection probe
    let latency = fresh_registry.test_connection("anthropic").expect("probe");
    assert!(latency.as_millis() > 0);

    // 5. 7-Layer Configuration Provenance (D-07, CFX-04)
    let mut config_svc = ConfigurationService::new();

    // BuiltIn layer defines defaults
    config_svc
        .set_value(
            ConfigLayer::BuiltIn,
            "autonomy.budget_limit_usd",
            json!(10.0),
            None,
            false,
        )
        .expect("set builtin");

    config_svc
        .set_value(
            ConfigLayer::BuiltIn,
            "security.fail_closed",
            json!(true),
            None,
            true, // Immutable safety constraint
        )
        .expect("set immutable builtin");

    // Workspace layer sets a higher budget
    config_svc
        .set_value(
            ConfigLayer::Workspace,
            "autonomy.budget_limit_usd",
            json!(25.0),
            Some(PathBuf::from("/repo/.m31a/config.toml")),
            false,
        )
        .expect("set workspace");

    // Resolving budget_limit_usd: Workspace overrides BuiltIn
    let resolved_budget = config_svc
        .resolve("autonomy.budget_limit_usd")
        .expect("resolve budget");
    assert_eq!(resolved_budget.layer, ConfigLayer::Workspace);
    assert_eq!(resolved_budget.value, json!(25.0));

    // CLI override wins over Workspace
    config_svc
        .set_value(
            ConfigLayer::Cli,
            "autonomy.budget_limit_usd",
            json!(50.0),
            None,
            false,
        )
        .expect("set cli");
    let resolved_cli_budget = config_svc
        .resolve("autonomy.budget_limit_usd")
        .expect("resolve cli budget");
    assert_eq!(resolved_cli_budget.layer, ConfigLayer::Cli);
    assert_eq!(resolved_cli_budget.value, json!(50.0));

    // 6. Immutable constraint protection (T-13-09):
    // Attempting to override security.fail_closed from lower layer must be rejected
    let attempt = config_svc.set_value(
        ConfigLayer::Workspace,
        "security.fail_closed",
        json!(false),
        Some(PathBuf::from("/repo/.m31a/config.toml")),
        false,
    );
    assert!(matches!(
        attempt,
        Err(ProvenanceError::ImmutableConstraintViolation { .. })
    ));
}
