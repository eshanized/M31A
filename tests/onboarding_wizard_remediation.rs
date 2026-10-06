//! Comprehensive Verification & Regression Suite for Onboarding Wizard Redesign
//! and Real NVIDIA Authentication Remediation (FRX-02, FRX-03, MDL-01).

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use m31a::config::provider_registry::ProviderRegistry;
use m31a::deployment::DeploymentChannel;
use m31a::init::lifecycle::SetupStep;
use m31a::model::catalog::ModelCatalog;
use m31a::model::router::resolver::{ModelCandidate, ModelTier};
use m31a::runtime_authorities::{CredentialSource, resolve_runtime_credentials};
use m31a::tui::screens::wizard::{
    ProviderVerificationState, SetupWizardScreen, WizardOutcome, WizardProfile,
};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tempfile::tempdir;

static ENV_MUTEX: Mutex<()> = Mutex::new(());

/// Guard to serialize tests that mutate process environment variables
/// and ensure clean state before and after each test.
struct EnvGuard {
    _lock: std::sync::MutexGuard<'static, ()>,
}

impl EnvGuard {
    fn lock() -> Self {
        let lock = ENV_MUTEX.lock().unwrap();
        unsafe {
            std::env::remove_var("NVIDIA_API_KEY");
            std::env::remove_var("API_KEY_NVIDIA");
        }
        Self { _lock: lock }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        unsafe {
            std::env::remove_var("NVIDIA_API_KEY");
            std::env::remove_var("API_KEY_NVIDIA");
        }
    }
}

/// Advance wizard through steps 1-6 up to Step 7 (FinalVerification).
fn advance_wizard_to_step_7(wizard: &mut SetupWizardScreen) {
    let _ = fs::create_dir_all(wizard.workspace_path().join(".git"));
    wizard.refresh_diagnostics();

    // Step 1: Trust
    wizard.trust_confirmed = true;
    assert!(wizard.advance());
    assert_eq!(wizard.current_step(), SetupStep::DoctorDiagnostics);

    // Step 2: Doctor
    wizard.warnings_acknowledged = true;
    assert!(wizard.advance());
    assert_eq!(wizard.current_step(), SetupStep::ProviderSetup);

    // Step 3: Provider
    wizard.api_key_input.set_text("nvapi-test-dummy-key-999");
    assert!(wizard.advance());
    assert_eq!(wizard.current_step(), SetupStep::ModelSetup);

    // Step 4: Model
    wizard
        .primary_model_input
        .set_text("meta/llama-3.1-70b-instruct");
    assert!(wizard.advance());
    assert_eq!(wizard.current_step(), SetupStep::ProfileSelection);

    // Step 5: Profile
    assert!(wizard.advance());
    assert_eq!(wizard.current_step(), SetupStep::AutonomySafety);

    // Step 6: Autonomy
    assert!(wizard.advance());
    assert_eq!(wizard.current_step(), SetupStep::FinalVerification);
}

/// Helper to render wizard to a string buffer.
fn render_wizard_to_string(wizard: &SetupWizardScreen, width: u16, height: u16) -> String {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("create test terminal");
    terminal
        .draw(|f| {
            wizard.render(f, f.area());
        })
        .expect("draw wizard");

    let buffer = terminal.backend().buffer().clone();
    (0..buffer.area.height)
        .map(|y| {
            (0..buffer.area.width)
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<String>>()
        .join("\n")
}

/// Lightweight mock HTTP server for hermetic testing of NVIDIA NIM provider probes.
fn spawn_mock_nvidia_server(
    status_code: u16,
    response_body: &'static str,
) -> (String, std::thread::JoinHandle<()>, Arc<AtomicBool>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock server");
    let port = listener.local_addr().expect("local addr").port();
    let auth_header_seen = Arc::new(AtomicBool::new(false));
    let auth_seen_clone = auth_header_seen.clone();

    let handle = std::thread::spawn(move || {
        // Accept requests in a bounded loop
        for _ in 0..5 {
            if let Ok((mut socket, _)) = listener.accept() {
                let mut buf = [0u8; 4096];
                if let Ok(n) = socket.read(&mut buf) {
                    let req_str = String::from_utf8_lossy(&buf[..n]);
                    if req_str.contains("authorization:") || req_str.contains("Authorization:") {
                        auth_seen_clone.store(true, Ordering::SeqCst);
                    }

                    let status_line = match status_code {
                        200 => "HTTP/1.1 200 OK",
                        401 => "HTTP/1.1 401 Unauthorized",
                        403 => "HTTP/1.1 403 Forbidden",
                        404 => "HTTP/1.1 404 Not Found",
                        429 => "HTTP/1.1 429 Too Many Requests",
                        _ => "HTTP/1.1 500 Internal Server Error",
                    };

                    let response = format!(
                        "{status_line}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{response_body}",
                        response_body.len()
                    );
                    let _ = socket.write_all(response.as_bytes());
                    let _ = socket.flush();
                }
            }
        }
    });

    let base_url = format!("http://127.0.0.1:{port}/v1");
    (base_url, handle, auth_header_seen)
}

// ===========================================================================
// Track B: Real NVIDIA Authentication & Credential Consistency Tests
// ===========================================================================

#[test]
fn test_credential_consistency_01_no_credential() {
    let _guard = EnvGuard::lock();
    let dir = tempdir().expect("tempdir");

    let resolution = resolve_runtime_credentials(dir.path(), DeploymentChannel::Production);
    assert_eq!(resolution.api_key, None);
    assert_eq!(resolution.source, CredentialSource::Absent);

    let wizard = SetupWizardScreen::new(dir.path().to_path_buf());
    assert_eq!(wizard.effective_api_key(), None);
    assert!(wizard.api_key_input.text().is_empty());
}

#[test]
fn test_credential_consistency_02_empty_or_whitespace_credential() {
    let _guard = EnvGuard::lock();
    let dir = tempdir().expect("tempdir");
    unsafe {
        std::env::set_var("NVIDIA_API_KEY", "   \t\n  ");
    }

    let resolution = resolve_runtime_credentials(dir.path(), DeploymentChannel::Production);
    assert_eq!(resolution.api_key, None);
    assert_eq!(resolution.source, CredentialSource::Absent);
}

#[test]
fn test_credential_consistency_03_credential_from_channel_file() {
    let _guard = EnvGuard::lock();
    let dir = tempdir().expect("tempdir");

    let creds_path = ProviderRegistry::channel_credentials_path(dir.path());
    let mut reg = ProviderRegistry::new();
    reg.set_credential("nvidia_nim", "nvapi-stored-in-channel-file-token");
    reg.save_credentials_to_file(&creds_path)
        .expect("save credentials");

    let resolution = resolve_runtime_credentials(dir.path(), DeploymentChannel::current());
    assert_eq!(
        resolution.api_key.as_deref(),
        Some("nvapi-stored-in-channel-file-token")
    );
    assert!(matches!(
        resolution.source,
        CredentialSource::ChannelFile(_)
    ));

    let wizard = SetupWizardScreen::new(dir.path().to_path_buf());
    assert_eq!(
        wizard.effective_api_key().as_deref(),
        Some("nvapi-stored-in-channel-file-token")
    );
    assert!(
        wizard
            .credential_source_label()
            .contains("Existing workspace credential store")
    );
}

#[test]
fn test_credential_consistency_04_credential_from_nvidia_api_key_env() {
    let _guard = EnvGuard::lock();
    let dir = tempdir().expect("tempdir");
    unsafe {
        std::env::set_var("NVIDIA_API_KEY", "nvapi-from-nvidia-api-key-env");
    }

    let resolution = resolve_runtime_credentials(dir.path(), DeploymentChannel::Production);
    assert_eq!(
        resolution.api_key.as_deref(),
        Some("nvapi-from-nvidia-api-key-env")
    );
    assert_eq!(
        resolution.source,
        CredentialSource::Environment("NVIDIA_API_KEY")
    );

    let wizard = SetupWizardScreen::new(dir.path().to_path_buf());
    assert_eq!(
        wizard.effective_api_key().as_deref(),
        Some("nvapi-from-nvidia-api-key-env")
    );
    assert!(
        wizard
            .credential_source_label()
            .contains("Environment variable (NVIDIA_API_KEY)")
    );
}

#[test]
fn test_credential_consistency_05_credential_from_api_key_nvidia_env() {
    let _guard = EnvGuard::lock();
    let dir = tempdir().expect("tempdir");
    unsafe {
        std::env::set_var("API_KEY_NVIDIA", "nvapi-from-api-key-nvidia-env");
    }

    let resolution = resolve_runtime_credentials(dir.path(), DeploymentChannel::Production);
    assert_eq!(
        resolution.api_key.as_deref(),
        Some("nvapi-from-api-key-nvidia-env")
    );
    assert_eq!(
        resolution.source,
        CredentialSource::Environment("API_KEY_NVIDIA")
    );

    let wizard = SetupWizardScreen::new(dir.path().to_path_buf());
    assert_eq!(
        wizard.effective_api_key().as_deref(),
        Some("nvapi-from-api-key-nvidia-env")
    );
    assert!(
        wizard
            .credential_source_label()
            .contains("Environment variable (API_KEY_NVIDIA)")
    );
}

#[test]
fn test_credential_consistency_06_channel_file_precedence_over_env() {
    let _guard = EnvGuard::lock();
    let dir = tempdir().expect("tempdir");
    unsafe {
        std::env::set_var("NVIDIA_API_KEY", "env-lower-priority-token");
    }

    let creds_path = ProviderRegistry::channel_credentials_path(dir.path());
    let mut reg = ProviderRegistry::new();
    reg.set_credential("nvidia_nim", "channel-file-winner-token");
    reg.save_credentials_to_file(&creds_path)
        .expect("save credentials");

    // Channel file MUST win over environment variable
    let resolution = resolve_runtime_credentials(dir.path(), DeploymentChannel::current());
    assert_eq!(
        resolution.api_key.as_deref(),
        Some("channel-file-winner-token")
    );
    assert!(matches!(
        resolution.source,
        CredentialSource::ChannelFile(_)
    ));

    let wizard = SetupWizardScreen::new(dir.path().to_path_buf());
    assert_eq!(
        wizard.effective_api_key().as_deref(),
        Some("channel-file-winner-token")
    );
}

#[test]
fn test_credential_consistency_07_changed_onboarding_credential_replaces_previous() {
    let _guard = EnvGuard::lock();
    let dir = tempdir().expect("tempdir");

    // Pre-existing credential in channel file
    let creds_path = ProviderRegistry::channel_credentials_path(dir.path());
    let mut reg = ProviderRegistry::new();
    reg.set_credential("nvidia_nim", "old-initial-token");
    reg.save_credentials_to_file(&creds_path)
        .expect("save credentials");

    let mut wizard = SetupWizardScreen::new(dir.path().to_path_buf());
    assert_eq!(
        wizard.effective_api_key().as_deref(),
        Some("old-initial-token")
    );

    // Operator enters a replacement credential in Step 3
    wizard
        .api_key_input
        .set_text("new-replacement-verified-token");
    assert_eq!(
        wizard.effective_api_key().as_deref(),
        Some("new-replacement-verified-token")
    );

    // Onboarding completes and persists
    wizard.persist_configuration().expect("persist config");

    // Now resolve runtime credentials again — must return the new replacement token!
    let resolution = resolve_runtime_credentials(dir.path(), DeploymentChannel::current());
    assert_eq!(
        resolution.api_key.as_deref(),
        Some("new-replacement-verified-token")
    );
}

#[test]
fn test_credential_consistency_08_runtime_and_wizard_resolve_same_source() {
    let _guard = EnvGuard::lock();
    let dir = tempdir().expect("tempdir");
    let mut wizard = SetupWizardScreen::new(dir.path().to_path_buf());
    wizard
        .api_key_input
        .set_text("nvapi-single-authority-key-42");
    wizard.persist_configuration().expect("persist config");

    // Runtime authority resolution
    let runtime_res = resolve_runtime_credentials(dir.path(), DeploymentChannel::current());
    assert_eq!(
        runtime_res.api_key.as_deref(),
        Some("nvapi-single-authority-key-42")
    );
    assert_eq!(
        wizard.effective_api_key().as_deref(),
        runtime_res.api_key.as_deref(),
        "Wizard and runtime must resolve the exact same credential material"
    );
}

// ===========================================================================
// Track B: Real Provider Probe & Prevention of False-Positive Bug
// ===========================================================================

#[test]
fn test_false_positive_remediation_09_invalid_credential_fails_verification() {
    let _guard = EnvGuard::lock();
    // Mock server returning HTTP 401 Unauthorized
    let (base_url, _handle, auth_seen) = spawn_mock_nvidia_server(
        401,
        r#"{"error":{"message":"Invalid API key provided","type":"invalid_request_error","code":"invalid_api_key"}}"#,
    );

    let dir = tempdir().expect("tempdir");
    let mut wizard = SetupWizardScreen::new(dir.path().to_path_buf()).with_base_url(&base_url);

    advance_wizard_to_step_7(&mut wizard);
    assert_eq!(wizard.current_step(), SetupStep::FinalVerification);

    // Before probe: unverified, cannot complete
    assert!(!wizard.can_advance());
    assert_eq!(
        wizard.verification_state,
        ProviderVerificationState::Unverified
    );

    // Press Enter to trigger probe on Step 7
    let outcome = wizard.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));
    assert!(auth_seen.load(Ordering::SeqCst), "Auth header must be sent");

    // Must fail with AuthenticationFailed
    assert!(matches!(
        wizard.verification_state,
        ProviderVerificationState::AuthenticationFailed {
            status_code: Some(401),
            ..
        }
    ));
    assert!(
        matches!(outcome, WizardOutcome::Error(_)),
        "Invalid key must yield error outcome on Step 7"
    );

    // Gate must remain closed: cannot complete or advance!
    assert!(!wizard.can_advance());
    let outcome_retry = wizard.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));
    assert_ne!(
        outcome_retry,
        WizardOutcome::Completed,
        "Failed verification must NEVER yield Completed outcome"
    );
}

#[test]
fn test_false_positive_remediation_10_valid_credential_succeeds_verification() {
    let _guard = EnvGuard::lock();
    // Mock server returning HTTP 200 OK
    let (base_url, _handle, auth_seen) = spawn_mock_nvidia_server(
        200,
        r#"{"id":"chatcmpl-test","choices":[{"message":{"content":"pong"}}],"usage":{"total_tokens":2}}"#,
    );

    let dir = tempdir().expect("tempdir");
    let mut wizard = SetupWizardScreen::new(dir.path().to_path_buf()).with_base_url(&base_url);

    advance_wizard_to_step_7(&mut wizard);
    assert_eq!(wizard.current_step(), SetupStep::FinalVerification);

    // Press Enter to run live verification
    let outcome = wizard.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));
    assert!(auth_seen.load(Ordering::SeqCst), "Auth header must be sent");

    assert!(
        wizard.verification_state.is_success(),
        "Valid credential must produce successful verification state"
    );
    assert_eq!(outcome, WizardOutcome::None); // Prompted to confirm completion

    // Gate is now satisfied!
    assert!(wizard.can_advance());

    // Press Enter again to complete onboarding
    let completion_outcome =
        wizard.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));
    assert_eq!(
        completion_outcome,
        WizardOutcome::Completed,
        "Verified provider must permit completion"
    );
}

#[test]
fn test_false_positive_remediation_11_no_fake_42ms_or_hardcoded_success() {
    let _guard = EnvGuard::lock();
    let (base_url, _handle, _) = spawn_mock_nvidia_server(
        200,
        r#"{"id":"chatcmpl-test","choices":[{"message":{"content":"ok"}}]}"#,
    );

    let dir = tempdir().expect("tempdir");
    let mut wizard = SetupWizardScreen::new(dir.path().to_path_buf()).with_base_url(&base_url);

    advance_wizard_to_step_7(&mut wizard);

    // Press 'T' to run connection probe
    wizard.handle_key(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::empty()));

    if let ProviderVerificationState::Success { latency, .. } = wizard.verification_state {
        // Truthful latency must be real elapsed time, never fixed 42ms
        assert!(latency > Duration::ZERO);
        assert_ne!(
            latency,
            Duration::from_millis(42),
            "Must never produce fabricated 42ms latency"
        );
    } else {
        panic!("Expected successful probe");
    }

    let status_str = wizard.connection_status.expect("status string");
    assert!(
        !status_str.contains("42ms"),
        "Status string must not contain hardcoded 42ms"
    );
}

#[test]
fn test_false_positive_remediation_12_network_error_reporting() {
    let _guard = EnvGuard::lock();
    // Non-existent unreachable localhost port
    let base_url = "http://127.0.0.1:1/v1";

    let dir = tempdir().expect("tempdir");
    let mut wizard = SetupWizardScreen::new(dir.path().to_path_buf()).with_base_url(base_url);

    advance_wizard_to_step_7(&mut wizard);

    // Run verification against dead endpoint
    wizard.verify_provider();

    assert!(
        matches!(
            wizard.verification_state,
            ProviderVerificationState::NetworkError(_)
        ),
        "Dead endpoint must produce NetworkError state"
    );
    assert!(!wizard.can_advance(), "Network error must block completion");
}

#[test]
fn test_false_positive_remediation_13_step3_connection_probe() {
    let _guard = EnvGuard::lock();
    let (base_url, _handle, _) = spawn_mock_nvidia_server(
        200,
        r#"{"id":"chatcmpl-test","choices":[{"message":{"content":"ok"}}]}"#,
    );

    let dir = tempdir().expect("tempdir");
    let mut wizard = SetupWizardScreen::new(dir.path().to_path_buf()).with_base_url(&base_url);

    // Advance to Step 3
    let _ = fs::create_dir_all(wizard.workspace_path().join(".git"));
    wizard.refresh_diagnostics();
    wizard.trust_confirmed = true;
    wizard.advance();
    wizard.warnings_acknowledged = true;
    wizard.advance();
    assert_eq!(wizard.current_step(), SetupStep::ProviderSetup);

    wizard.api_key_input.set_text("nvapi-test-key-step3");

    // Press 'T' on Step 3
    wizard.handle_key(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::empty()));

    assert!(
        wizard.step3_connection_state.is_success(),
        "Step 3 connection test must succeed against mock server"
    );
}

// ===========================================================================
// Track A: UI, UX, Information Hierarchy & Rendering Tests
// ===========================================================================

#[test]
fn test_ui_01_secret_leak_regression_across_all_steps() {
    let sensitive_key = "nvapi-realistic-secret-for-test-xyz987654321";

    let dir = tempdir().expect("tempdir");
    let mut wizard = SetupWizardScreen::new(dir.path().to_path_buf());
    wizard.api_key_input.set_text(sensitive_key);

    let steps = [
        SetupStep::WorkspaceTrust,
        SetupStep::DoctorDiagnostics,
        SetupStep::ProviderSetup,
        SetupStep::ModelSetup,
        SetupStep::ProfileSelection,
        SetupStep::AutonomySafety,
        SetupStep::FinalVerification,
    ];

    for step in steps {
        wizard.trust_confirmed = true;
        wizard.warnings_acknowledged = true;
        let _ = wizard.advance();

        let buffer_content = render_wizard_to_string(&wizard, 100, 30);
        assert!(
            !buffer_content.contains(sensitive_key),
            "Sensitive API key must NEVER appear in cleartext on step {:?}",
            step
        );
    }
}

#[test]
fn test_ui_02_model_catalog_distinguishes_cached_vs_live() {
    let dir = tempdir().expect("tempdir");
    let cache_path = ModelCatalog::cache_path_for_channel(dir.path(), DeploymentChannel::current());

    let candidates = vec![
        ModelCandidate::new(
            "meta/llama-3.1-70b-instruct",
            "nvidia",
            ModelTier::Standard,
            131072,
        )
        .with_tool_support(true),
    ];
    let catalog = ModelCatalog::from_discovered("nvidia", candidates, 1700000000);
    catalog.save_to_cache_file(&cache_path).expect("save cache");

    let mut wizard = SetupWizardScreen::new(dir.path().to_path_buf());
    advance_wizard_to_step_7(&mut wizard);
    wizard.back(); // Step 6
    wizard.back(); // Step 5
    wizard.back(); // Step 4 (ModelSetup)
    assert_eq!(wizard.current_step(), SetupStep::ModelSetup);

    // Initial state: loaded from disk cache -> not verified live
    assert!(!wizard.catalog_verified_live);
    let content = render_wizard_to_string(&wizard, 120, 30);
    assert!(
        content.contains("CACHED"),
        "Buffer must indicate cached catalog"
    );
    assert!(
        content.contains("Not verified against current credentials"),
        "Buffer must explicitly note lack of live verification"
    );
}

#[test]
fn test_ui_03_step1_workspace_trust_renders_git_info() {
    let dir = tempdir().expect("tempdir");
    let _ = fs::create_dir_all(dir.path().join(".git"));

    let wizard = SetupWizardScreen::new(dir.path().to_path_buf());
    let content = render_wizard_to_string(&wizard, 100, 30);

    assert!(content.contains("Workspace Directory"));
    assert!(content.contains("Repository Status"));
    assert!(content.contains("Git repository detected"));
    assert!(content.contains("I trust this repository"));
    assert!(content.contains("Policy Gate"));
    assert!(content.contains("Container Sandbox"));
}

#[test]
fn test_ui_04_step2_doctor_diagnostics_renders_checks_and_badges() {
    let dir = tempdir().expect("tempdir");
    let mut wizard = SetupWizardScreen::new(dir.path().to_path_buf());
    wizard.trust_confirmed = true;
    wizard.advance();

    let content = render_wizard_to_string(&wizard, 100, 30);
    assert!(content.contains("Step 2/7: Doctor Diagnostics"));
    assert!(content.contains("System Readiness"));
    assert!(content.contains("CHECKS PASSED"));
}

#[test]
fn test_ui_05_step5_profile_selection_cycles_cleanly() {
    let dir = tempdir().expect("tempdir");
    let mut wizard = SetupWizardScreen::new(dir.path().to_path_buf());
    advance_wizard_to_step_7(&mut wizard);
    wizard.back(); // Step 6
    wizard.back(); // Step 5 (ProfileSelection)
    assert_eq!(wizard.current_step(), SetupStep::ProfileSelection);

    assert_eq!(wizard.profile, WizardProfile::Balanced);

    // Down arrow -> Autonomous
    wizard.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::empty()));
    assert_eq!(wizard.profile, WizardProfile::Autonomous);

    // Down arrow -> Conservative
    wizard.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::empty()));
    assert_eq!(wizard.profile, WizardProfile::Conservative);

    // Down arrow -> CodeReviewer
    wizard.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::empty()));
    assert_eq!(wizard.profile, WizardProfile::CodeReviewer);

    let content = render_wizard_to_string(&wizard, 100, 30);
    assert!(content.contains("BALANCED"));
    assert!(content.contains("AUTONOMOUS"));
    assert!(content.contains("CONSERVATIVE"));
    assert!(content.contains("CODE REVIEWER"));
}

#[test]
fn test_ui_06_step6_autonomy_safety_renders_budget_and_guardrails() {
    let dir = tempdir().expect("tempdir");
    let mut wizard = SetupWizardScreen::new(dir.path().to_path_buf());
    advance_wizard_to_step_7(&mut wizard);
    wizard.back(); // Step 6 (AutonomySafety)
    assert_eq!(wizard.current_step(), SetupStep::AutonomySafety);

    let content = render_wizard_to_string(&wizard, 100, 30);
    assert!(content.contains("Execution Policy Guardrails"));
    assert!(content.contains("Require human authorization"));
    assert!(content.contains("$25.00 USD"));
    assert!(content.contains("Security Model Pipeline"));
}

#[test]
fn test_ui_07_step7_final_verification_renders_configuration_summary() {
    let dir = tempdir().expect("tempdir");
    let mut wizard = SetupWizardScreen::new(dir.path().to_path_buf());
    advance_wizard_to_step_7(&mut wizard);
    assert_eq!(wizard.current_step(), SetupStep::FinalVerification);

    let content = render_wizard_to_string(&wizard, 100, 30);
    assert!(content.contains("Step 7/7: Final Verification"));
    assert!(content.contains("Onboarding Configuration Summary"));
    assert!(content.contains("Provider Authentication & Readiness"));
    assert!(content.contains("NVIDIA NIM"));
    assert!(content.contains("UNVERIFIED"));
}

#[test]
fn test_ui_08_multi_resolution_responsiveness() {
    let dir = tempdir().expect("tempdir");
    let mut wizard = SetupWizardScreen::new(dir.path().to_path_buf());
    advance_wizard_to_step_7(&mut wizard);

    let resolutions = [(80, 24), (100, 30), (120, 36), (160, 48)];

    for (w, h) in resolutions {
        let content = render_wizard_to_string(&wizard, w, h);
        // Header title must be present
        assert!(
            content.contains("M31A FIRST-RUN SETUP WIZARD"),
            "Resolution {w}x{h} must contain header title"
        );
        // Step number must be present
        assert!(
            content.contains("7"),
            "Resolution {w}x{h} must contain step 7 indicator"
        );
    }
}

#[test]
#[ignore = "requires live external LLM API credentials and WAN connection"]
fn test_live_nvidia_provider_verification_with_env_key() {
    let _guard = EnvGuard::lock();
    m31a::config::load_dotenv_from_workspace(std::path::Path::new("."));
    let api_key = std::env::var("NVIDIA_API_KEY")
        .or_else(|_| std::env::var("API_KEY_NVIDIA"))
        .expect("NVIDIA_API_KEY or API_KEY_NVIDIA must be set to run live discovery tests");

    let dir = tempdir().expect("tempdir");
    let mut wizard = SetupWizardScreen::new(dir.path().to_path_buf());
    advance_wizard_to_step_7(&mut wizard);

    // Set the real API key on wizard and an active NVIDIA NIM model
    wizard.api_key_input.set_text(&api_key);
    wizard
        .primary_model_input
        .set_text("meta/llama-3.2-11b-vision-instruct");

    // Execute real probe on Step 7
    let outcome = wizard.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));

    assert!(
        wizard.verification_state.is_success(),
        "Live provider verification must succeed with real credentials: {:?}",
        wizard.verification_state
    );
    assert_eq!(outcome, WizardOutcome::None);

    // Gate satisfied, completing onboarding succeeds
    assert!(wizard.can_advance());
    let outcome_complete = wizard.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));
    assert_eq!(outcome_complete, WizardOutcome::Completed);
}
