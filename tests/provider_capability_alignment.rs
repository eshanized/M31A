//! Workstream I: Provider / Model Capability Alignment Canonical Test Suite.
//!
//! Verifies:
//! - 5 Truthful Capability States: AVAILABLE, UNAVAILABLE, MISCONFIGURED, MOCK / TEST-ONLY, UNKNOWN
//! - Conditions A through N (WS-I §18)
//! - Live NVIDIA NIM Execution & Token Telemetry (WS-I §17)
//! - Fail-Closed Invariants: Zero silent fallback to mock in production
//! - Secret Redaction and Masking Boundaries

use tokio_util::sync::CancellationToken;

use m31a::agent::model_policy::{ModelCaller, RoutedModelCaller};
use m31a::config::env::{SafeEnvironmentStatus, load_dotenv};
use m31a::config::provider_registry::{MaskedSecret, ProviderRegistry};
use m31a::interaction::commands::{CommandContext, CommandOutput, SlashCommandRegistry};
use m31a::model::provider::ModelProvider;
use m31a::model::provider::nvidia::NvidiaProvider;
use m31a::model::router::health::{FailureKind, ModelCircuitBreaker};
use m31a::model::router::resolver::{ModelCandidate, ModelTier};
use m31a::model::types::{ModelError, ProviderCapabilityStatus};
use m31a::runtime::AppRuntime;
use m31a::tui::screens::wizard::SetupWizardScreen;

// ===========================================================================
// Condition A: Missing Credentials
// ===========================================================================
#[tokio::test]
async fn test_condition_a_missing_credentials() {
    // 1. Instantiating NvidiaProvider with explicitly missing API key must fail immediately
    let result = NvidiaProvider::new(None, None);
    assert!(
        result.is_err(),
        "NvidiaProvider must fail closed without API key"
    );
    let err = result.err().unwrap();
    assert!(
        matches!(
            err,
            ModelError::MissingCredentials(_)
                | ModelError::MissingConfiguration(_)
                | ModelError::AuthenticationFailed
        ),
        "Expected MissingCredentials, MissingConfiguration, or AuthenticationFailed, got: {:?}",
        err
    );

    // 2. Status check on registry without credentials must report MISCONFIGURED
    let reg = ProviderRegistry::new();
    let status = reg.get_status("nvidia_nim");
    if std::env::var("NVIDIA_API_KEY").is_err() && std::env::var("API_KEY_NVIDIA").is_err() {
        assert_eq!(
            status,
            ProviderCapabilityStatus::Misconfigured,
            "Provider requiring key without credentials must be MISCONFIGURED"
        );
    }

    // 3. RoutedModelCaller with Misconfigured status must fail closed without network call
    let caller = RoutedModelCaller::new(None, ModelTier::Standard, vec![])
        .with_provider_status("nvidia_nim", ProviderCapabilityStatus::Misconfigured);

    let call_res = caller.call_model("Test prompt").await;
    assert!(
        call_res.is_err(),
        "RoutedModelCaller must fail closed on Misconfigured"
    );
    let msg = call_res.err().unwrap();
    assert!(
        msg.contains("MISCONFIGURED"),
        "Error message must indicate MISCONFIGURED, got: {}",
        msg
    );
}

// ===========================================================================
// Condition B: Invalid Credentials
// ===========================================================================
#[tokio::test]
async fn test_condition_b_invalid_credentials() {
    let mut reg = ProviderRegistry::new();
    reg.set_credential("nvidia_nim", "nvapi-invalid-bogus-key-12345");

    // Probing with bogus key must fail with authentication error or misconfigured status
    let report = reg.probe_provider("nvidia_nim").await;
    assert!(
        !report.authentication_accepted,
        "Invalid key must not be accepted"
    );
    assert_eq!(report.status, ProviderCapabilityStatus::Misconfigured);
    assert!(
        report.details.contains("Authentication failure")
            || report.details.contains("invalid")
            || report.details.contains("probe failed")
            || report.details.contains("HTTP"),
        "Details must explain failure truthfully: {}",
        report.details
    );
}

// ===========================================================================
// Condition C: Unsupported Provider (OpenAI, Anthropic, Gemini, Local)
// ===========================================================================
#[tokio::test]
async fn test_condition_c_unsupported_provider() {
    let reg = ProviderRegistry::new();

    // 1. Registry status checks
    assert_eq!(
        reg.get_status("openai"),
        ProviderCapabilityStatus::Unavailable
    );
    assert_eq!(
        reg.get_status("anthropic"),
        ProviderCapabilityStatus::Unavailable
    );
    assert_eq!(
        reg.get_status("gemini"),
        ProviderCapabilityStatus::Unavailable
    );
    assert_eq!(
        reg.get_status("openai_compatible"),
        ProviderCapabilityStatus::Unavailable
    );
    assert_eq!(
        reg.get_status("local"),
        ProviderCapabilityStatus::Unavailable
    );
    assert_eq!(
        reg.get_status("ollama"),
        ProviderCapabilityStatus::Unavailable
    );

    // 2. Probe checks
    let report = reg.probe_provider("openai").await;
    assert_eq!(report.status, ProviderCapabilityStatus::Unavailable);
    assert!(!report.is_production_supported);
    assert!(report.details.contains("UNAVAILABLE"));

    // 3. RoutedModelCaller configured with unsupported provider fails closed
    let caller = RoutedModelCaller::new(None, ModelTier::Standard, vec![])
        .with_provider_status("anthropic", ProviderCapabilityStatus::Unavailable);

    let err = caller.call_model("Test context").await.err().unwrap();
    assert!(
        err.contains("UNAVAILABLE"),
        "Expected UNAVAILABLE in error, got: {}",
        err
    );
    assert!(
        err.contains("only NVIDIA NIM is production-supported"),
        "Must clarify that only NVIDIA NIM is production-supported, got: {}",
        err
    );
}

// ===========================================================================
// Condition D: Unavailable Model
// ===========================================================================
#[test]
fn test_condition_d_unavailable_model() {
    let provider = NvidiaProvider::new(None, Some("nvapi-test-key".to_string())).unwrap();

    // Candidate validation should reject models with empty or malformed strings
    let invalid_candidate = ModelCandidate::new("", "nvidia", ModelTier::Standard, 128000);
    assert!(
        !provider.validate_candidate(&invalid_candidate),
        "Empty model candidate must be rejected"
    );

    let valid_candidate = ModelCandidate::new(
        "meta/llama-3.2-11b-vision-instruct",
        "nvidia",
        ModelTier::Standard,
        131072,
    );
    assert!(
        provider.validate_candidate(&valid_candidate),
        "Valid candidate must be accepted"
    );
}

// ===========================================================================
// Condition E: Invalid Endpoint
// ===========================================================================
#[tokio::test]
async fn test_condition_e_invalid_endpoint() {
    // Port 1 is unassigned and immediately refused on localhost
    let provider = NvidiaProvider::new(
        Some("http://127.0.0.1:1/v1".to_string()),
        Some("nvapi-test-key".to_string()),
    )
    .unwrap();

    let err = provider.probe().await.expect_err("Probe must fail");
    assert!(
        matches!(
            err,
            ModelError::EndpointUnavailable(_) | ModelError::Network(_) | ModelError::Timeout(_)
        ),
        "Expected network/endpoint error, got: {:?}",
        err
    );

    // Circuit breaker classification
    let failure_kind = FailureKind::from_model_error(&err);
    assert!(
        matches!(
            failure_kind,
            FailureKind::ServerOverload | FailureKind::TransientNetwork
        ),
        "Endpoint failure must degrade circuit health appropriately"
    );
}

// ===========================================================================
// Condition F: Timeout & Clean Cancellation
// ===========================================================================
#[tokio::test]
async fn test_condition_f_timeout_cancellation() {
    let cancel = CancellationToken::new();
    cancel.cancel(); // Cancel immediately

    let provider = NvidiaProvider::new(None, Some("nvapi-test-key".to_string())).unwrap();
    let err = provider
        .call_model(
            "meta/llama-3.2-11b-vision-instruct",
            "hello",
            vec![],
            &cancel,
        )
        .await
        .expect_err("Must fail on pre-cancelled token");

    assert!(
        matches!(err, ModelError::Cancelled),
        "Expected ModelError::Cancelled, got: {:?}",
        err
    );
}

// ===========================================================================
// Condition G: Rate Limit Handling
// ===========================================================================
#[test]
fn test_condition_g_rate_limit() {
    let err = ModelError::RateLimited { cooldown_secs: 45 };
    let kind = FailureKind::from_model_error(&err);
    assert_eq!(kind, FailureKind::RateLimit { cooldown_secs: 45 });

    let mut breaker =
        ModelCircuitBreaker::new("https://integrate.api.nvidia.com", "meta/llama-3.1-70b");
    breaker.record_failure(&err);
    assert_eq!(
        breaker.state,
        m31a::model::router::health::CircuitState::Open,
        "Rate limit must transition circuit to open"
    );
}

// ===========================================================================
// Condition H: Malformed Response
// ===========================================================================
#[test]
fn test_condition_h_malformed_response() {
    let err = ModelError::InvalidResponse("missing 'choices' array in JSON".to_string());
    let kind = FailureKind::from_model_error(&err);
    assert_eq!(
        kind,
        FailureKind::NonDegrading,
        "Client protocol/malformed errors must not degrade endpoint circuit"
    );
}

// ===========================================================================
// Condition I: Provider Internal Failure
// ===========================================================================
#[test]
fn test_condition_i_provider_internal_failure() {
    let err = ModelError::ProviderInternalFailure("HTTP 503 Service Unavailable".to_string());
    let kind = FailureKind::from_model_error(&err);
    assert_eq!(
        kind,
        FailureKind::ServerOverload,
        "Provider 5xx must classify as ServerOverload"
    );
}

// ===========================================================================
// Condition J: No Fallback to Mock in Production
// ===========================================================================
#[tokio::test]
async fn test_condition_j_no_fallback_to_mock_in_production() {
    // In production, when the active provider fails, the system must FAIL.
    // It must NOT silently substitute a mock provider or return canned success.
    let caller = RoutedModelCaller::new(None, ModelTier::Standard, vec![])
        .with_provider_status("nvidia_nim", ProviderCapabilityStatus::Misconfigured);

    let result = caller.call_model("Write production code").await;
    assert!(
        result.is_err(),
        "Production caller must fail closed when misconfigured, never fallback to mock"
    );

    // Verify Mock provider descriptor is strictly tagged MockOnly
    let reg = ProviderRegistry::new();
    let mock_desc = reg
        .get_descriptor("mock")
        .expect("mock descriptor must exist");
    assert!(
        mock_desc.is_mock,
        "Mock descriptor must be flagged is_mock: true"
    );
    assert!(
        !mock_desc.is_production_supported,
        "Mock descriptor must NOT be production supported"
    );
    assert_eq!(
        reg.get_status("mock"),
        ProviderCapabilityStatus::MockOnly,
        "Mock descriptor must report MOCK / TEST-ONLY status"
    );
}

// ===========================================================================
// Condition K: Secret Redaction
// ===========================================================================
#[test]
fn test_condition_k_secret_redaction() {
    let raw_key = "nvapi-test-super-secret-key-99887766";
    let secret = MaskedSecret::new(raw_key);

    // 1. Masked string
    let masked = secret.masked();
    assert!(!masked.contains("super-secret"));
    assert!(masked.contains("nvap...[MASKED]"));

    // 2. Display trait
    let display = format!("{}", secret);
    assert!(!display.contains("super-secret"));
    assert_eq!(display, "nvap...[MASKED]");

    // 3. Debug trait
    let debug = format!("{:?}", secret);
    assert!(!debug.contains("super-secret"));
    assert_eq!(debug, "nvap...[MASKED]");

    // 4. Serialization
    let json = serde_json::to_string(&secret).unwrap();
    assert!(!json.contains("super-secret"));
    assert!(json.contains("nvap...[MASKED]"));

    // 5. Expose secret only via explicit method
    assert_eq!(secret.expose_secret(), raw_key);
}

// ===========================================================================
// Condition L: /model Command Truthful Reporting
// ===========================================================================
#[tokio::test]
async fn test_condition_l_model_command_truthful_reporting() {
    let tmp = tempfile::tempdir().unwrap();
    let runtime = AppRuntime::new(tmp.path()).await.unwrap();
    let registry = SlashCommandRegistry::new_standard();

    // 1. Query current model with nvidia_nim
    let ctx_available = CommandContext {
        workspace_root: runtime.workspace_root(),
        session_id: None,
        active_mission_id: None,
        pool: runtime.pool(),
        event_bus: runtime.event_bus(),
        configured_model: "meta/llama-3.2-11b-vision-instruct".to_string(),
        configured_provider: "nvidia_nim".to_string(),
        active_profile: "balanced".to_string(),
    };

    let out = registry
        .execute_line("/model", &ctx_available)
        .await
        .unwrap();
    if let CommandOutput::Info(msg) = out {
        assert!(msg.contains("Configured model:    meta/llama-3.2-11b-vision-instruct"));
        assert!(msg.contains("Provider:            nvidia_nim"));
        assert!(msg.contains("Status:"));
    } else {
        panic!("Expected CommandOutput::Info");
    }

    // 2. Query with an unavailable provider
    let ctx_unavailable = CommandContext {
        workspace_root: runtime.workspace_root(),
        session_id: None,
        active_mission_id: None,
        pool: runtime.pool(),
        event_bus: runtime.event_bus(),
        configured_model: "gpt-4o".to_string(),
        configured_provider: "openai".to_string(),
        active_profile: "balanced".to_string(),
    };

    let out = registry
        .execute_line("/model", &ctx_unavailable)
        .await
        .unwrap();
    if let CommandOutput::Info(msg) = out {
        assert!(msg.contains("Status:              UNAVAILABLE"));
        assert!(msg.contains("NVIDIA NIM is the production model provider"));
    } else {
        panic!("Expected CommandOutput::Info");
    }
}

// ===========================================================================
// Condition M: TUI Setup Wizard Truthful Reporting
// ===========================================================================
#[test]
fn test_condition_m_tui_wizard_truthful_reporting() {
    // 1. Verify provider list clearly designates NVIDIA NIM as production provider
    let providers = SetupWizardScreen::PROVIDERS;
    assert_eq!(providers.len(), 1);
    assert!(
        providers[0].contains("NVIDIA NIM") && providers[0].contains("PRODUCTION PROVIDER"),
        "NVIDIA NIM must be designated PRODUCTION PROVIDER: {}",
        providers[0]
    );
}

// ===========================================================================
// Condition N: Live NVIDIA NIM Execution & Telemetry (WS-I §17)
// ===========================================================================
#[tokio::test]
#[ignore = "requires live external LLM API credentials and WAN connection"]
async fn test_condition_n_live_provider_execution() {
    load_dotenv();

    let env_status = SafeEnvironmentStatus::probe();
    assert!(
        env_status.api_key_configured,
        "NVIDIA_API_KEY must be configured to run live provider tests (invoke with: cargo test --ignored)"
    );

    println!("Executing live NVIDIA NIM request verification...");
    let api_key = std::env::var("NVIDIA_API_KEY")
        .or_else(|_| std::env::var("API_KEY_NVIDIA"))
        .ok();
    let provider =
        NvidiaProvider::new(None, api_key).expect("Failed to initialize NvidiaProvider from env");

    // 1. Live probe verification
    let latency = provider
        .probe()
        .await
        .expect("Live NVIDIA NIM probe failed");
    println!("NVIDIA NIM live probe latency: {:?}", latency);
    assert!(latency.as_millis() > 0);

    // 2. Discover models verification
    let models = provider
        .discover_models()
        .await
        .expect("Model discovery failed");
    assert!(
        !models.is_empty(),
        "Must discover at least one model candidate"
    );
    println!("Discovered {} NVIDIA NIM models", models.len());

    // 3. Real live turn through RoutedModelCaller
    let cancel = CancellationToken::new();
    let model_name = std::env::var("M31A_MODEL")
        .or_else(|_| std::env::var("NVIDIA_MODEL"))
        .unwrap_or_else(|_| "meta/llama-3.2-11b-vision-instruct".to_string());

    let (proposal, usage) = provider
        .call_model(
            &model_name,
            "Respond strictly with the single word: OK",
            vec![],
            &cancel,
        )
        .await
        .expect("Live call_model failed");

    println!("Live model proposal: {:?}", proposal);
    println!("Live model usage: {:?}", usage);

    // 4. Verify usage telemetry
    assert!(
        usage.prompt_tokens > 0,
        "Prompt tokens must be > 0, got {}",
        usage.prompt_tokens
    );
    assert!(
        usage.completion_tokens > 0,
        "Completion tokens must be > 0, got {}",
        usage.completion_tokens
    );
    assert!(
        usage.total_tokens == usage.prompt_tokens + usage.completion_tokens,
        "Total tokens must equal prompt + completion"
    );

    // 5. Verify fail-closed behavior with corrupted credentials
    let bad_provider =
        NvidiaProvider::new(None, Some("nvapi-invalid-key-for-test-failure".to_string())).unwrap();
    let bad_res = bad_provider
        .call_model(&model_name, "Hello", vec![], &cancel)
        .await;
    assert!(
        bad_res.is_err(),
        "Must fail closed with invalid API credentials"
    );
    println!("Verified fail-closed with bad key: {:?}", bad_res.err());
}
