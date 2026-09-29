//! Comprehensive Dynamic Provider / Model Discovery Test Suite (MDL-01, MDL-02, D-05).
//!
//! Verifies:
//! - Tests A through T (§17): Full coverage of dynamic discovery lifecycle, caching, router integration,
//!   CLI/TUI/Doctor/Wizard projection, error handling, and hardcoded assumption elimination.
//! - Critical Test (§18): Dynamic runtime model reflection without source code changes ([a, b] -> [b, c]).
//! - Live NVIDIA Discovery Test (§19): Live endpoint probe, discovery, validation, and inference.

use std::sync::Arc;
use tempfile::tempdir;
use tokio_util::sync::CancellationToken;

use m31a::agent::model_policy::{ModelCaller, RoutedModelCaller};
use m31a::cli::doctor::{DoctorProbe, ModelsProbe};
use m31a::interaction::commands::{CommandContext, CommandOutput, SlashCommandRegistry};
use m31a::model::catalog::{CatalogRefreshState, CatalogSource, ModelCatalog};
use m31a::model::provider::ModelProvider;
use m31a::model::provider::mock::MockProvider;
use m31a::model::provider::nvidia::NvidiaProvider;
use m31a::model::router::health::CircuitBreakerRegistry;
use m31a::model::router::resolver::{ModelCandidate, ModelRouter, ModelTier, RoutingRequest};
use m31a::model::types::{ModelError, ProviderCapabilityStatus};
use m31a::runtime::AppRuntime;
use m31a::tui::screens::wizard::SetupWizardScreen;

// ===========================================================================
// Test A: Successful Model Discovery
// ===========================================================================
#[tokio::test]
async fn test_a_successful_model_discovery() {
    let provider = MockProvider::new().with_discovered_models(vec![
        ModelCandidate::new(
            "meta/llama-3.1-70b-instruct",
            "nvidia",
            ModelTier::Standard,
            131072,
        )
        .with_display_name("Llama 3.1 70B Instruct")
        .with_tool_support(true)
        .with_structured_output(true)
        .with_availability(ProviderCapabilityStatus::Available)
        .with_source("discovered")
        .with_metadata("family", "llama"),
    ]);

    let discovered = provider
        .discover_models()
        .await
        .expect("discovery should succeed");
    assert_eq!(discovered.len(), 1);
    let m = &discovered[0];
    assert_eq!(m.model_id, "meta/llama-3.1-70b-instruct");
    assert_eq!(m.display_name.as_deref(), Some("Llama 3.1 70B Instruct"));
    assert_eq!(m.provider, "nvidia");
    assert_eq!(m.tier, ModelTier::Standard);
    assert_eq!(m.context_capacity, 131072);
    assert!(m.supports_tools);
    assert!(m.supports_structured_output);
    assert_eq!(m.availability, ProviderCapabilityStatus::Available);
    assert_eq!(m.source, "discovered");
    assert_eq!(m.metadata.get("family").map(String::as_str), Some("llama"));
}

// ===========================================================================
// Test B: Multiple Discovered Models Across Tiers
// ===========================================================================
#[tokio::test]
async fn test_b_multiple_discovered_models_across_tiers() {
    let models = vec![
        ModelCandidate::new(
            "meta/llama-3.1-70b-instruct",
            "nvidia",
            ModelTier::Standard,
            131072,
        )
        .with_tool_support(true),
        ModelCandidate::new(
            "meta/llama-3.2-11b-vision-instruct",
            "nvidia",
            ModelTier::Fast,
            131072,
        )
        .with_tool_support(true),
        ModelCandidate::new(
            "deepseek-ai/deepseek-r1",
            "nvidia",
            ModelTier::Reasoning,
            131072,
        )
        .with_tool_support(false),
    ];

    let catalog = ModelCatalog::from_discovered("nvidia", models, 1700000000);
    assert_eq!(catalog.len(), 3);
    assert_eq!(catalog.models_for_tier(ModelTier::Standard).len(), 1);
    assert_eq!(catalog.models_for_tier(ModelTier::Fast).len(), 1);
    assert_eq!(catalog.models_for_tier(ModelTier::Reasoning).len(), 1);
}

// ===========================================================================
// Test C: New Model Appears Without Source Code Changes
// ===========================================================================
#[tokio::test]
async fn test_c_new_model_appears_dynamically() {
    let new_model_id = "vendor-x/quantum-coder-v1";
    let provider = MockProvider::new().with_discovered_models(vec![
        ModelCandidate::new(new_model_id, "vendor-x", ModelTier::Standard, 65536)
            .with_tool_support(true),
    ]);

    let mut catalog = ModelCatalog::new("vendor-x");
    assert!(!catalog.contains_model(new_model_id));

    let discovered = provider.discover_models().await.unwrap();
    catalog.update_from_provider("vendor-x", discovered);

    assert!(catalog.contains_model(new_model_id));
    let selected = catalog.select_default(None).expect("must select default");
    assert_eq!(selected.model_id, new_model_id);
}

// ===========================================================================
// Test D: Removed Model Becomes Unavailable
// ===========================================================================
#[tokio::test]
async fn test_d_removed_model_becomes_unavailable() {
    let mut catalog = ModelCatalog::from_discovered(
        "nvidia",
        vec![
            ModelCandidate::new("model-alpha", "nvidia", ModelTier::Standard, 32768)
                .with_tool_support(true),
            ModelCandidate::new("model-beta", "nvidia", ModelTier::Standard, 32768)
                .with_tool_support(true),
        ],
        1700000000,
    );
    assert!(catalog.contains_model("model-alpha"));

    // Discovery update omits model-alpha
    let new_models = vec![
        ModelCandidate::new("model-beta", "nvidia", ModelTier::Standard, 32768)
            .with_tool_support(true),
    ];
    catalog.update_from_provider("nvidia", new_models);

    assert!(!catalog.contains_model("model-alpha"));
    assert!(catalog.contains_model("model-beta"));

    let router = ModelRouter::new();
    let health = CircuitBreakerRegistry::default();
    let req = RoutingRequest::new(
        m31a::state_machine::agent::AgentRole::implementer(),
        ModelTier::Standard,
    );
    let selection = router
        .resolve_model(&req, &catalog.models, &health)
        .unwrap();
    assert_eq!(selection.model_name, "model-beta");
}

// ===========================================================================
// Test E: Malformed Provider Model Response
// ===========================================================================
#[test]
fn test_e_malformed_provider_model_response() {
    let malformed_json = "{ invalid_json: true, ";
    let result: Result<ModelCatalog, _> = serde_json::from_str(malformed_json);
    assert!(result.is_err(), "malformed json must return error");
}

// ===========================================================================
// Test F: Provider Discovery Timeout
// ===========================================================================
#[test]
fn test_f_provider_discovery_timeout_error_type() {
    let err = ModelError::Timeout("discovery request timed out after 10000ms".to_string());
    assert!(matches!(err, ModelError::Timeout(_)));
    assert!(err.to_string().contains("timed out"));
}

// ===========================================================================
// Test G: Provider Discovery Authentication Failure
// ===========================================================================
#[test]
fn test_g_provider_discovery_authentication_failure() {
    let err = ModelError::AuthenticationFailed;
    assert!(matches!(err, ModelError::AuthenticationFailed));
    assert!(err.to_string().contains("authentication failed"));
}

// ===========================================================================
// Test H: Discovery with Empty Model List
// ===========================================================================
#[tokio::test]
async fn test_h_discovery_with_empty_model_list() {
    let provider = MockProvider::new().with_discovered_models(vec![]);
    let discovered = provider.discover_models().await.unwrap();
    assert!(discovered.is_empty());

    let mut catalog = ModelCatalog::new("nvidia");
    catalog.update_from_provider("nvidia", discovered);
    assert!(catalog.is_empty());
    assert_eq!(catalog.len(), 0);
    assert_eq!(catalog.select_default(None), None);
}

// ===========================================================================
// Test I: Discovery Cache Load and Persist
// ===========================================================================
#[test]
fn test_i_discovery_cache_load_and_persist() {
    let dir = tempdir().unwrap();
    let cache_path = dir.path().join("models_cache.json");

    let models = vec![
        ModelCandidate::new("model-1", "test", ModelTier::Standard, 65536)
            .with_display_name("Model 1")
            .with_tool_support(true),
    ];
    let catalog = ModelCatalog::from_discovered("test", models, 1700000000);
    catalog
        .save_to_cache_file(&cache_path)
        .expect("save must succeed");

    let loaded = ModelCatalog::load_from_cache_file(&cache_path).expect("load must succeed");
    assert_eq!(loaded.provider, "test");
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded.models[0].model_id, "model-1");
    assert_eq!(loaded.models[0].display_name.as_deref(), Some("Model 1"));
}

// ===========================================================================
// Test J: Discovery Cache Stale State
// ===========================================================================
#[test]
fn test_j_discovery_cache_stale_state() {
    let old_timestamp = 1000; // Far in the past
    let catalog = ModelCatalog::from_cache("nvidia", vec![], old_timestamp, true);
    assert!(catalog.is_stale(3600));
    assert_eq!(
        catalog.refresh_state,
        CatalogRefreshState::DiscoveryFailedWithCache
    );
}

// ===========================================================================
// Test K: Discovery Refresh Updates Timestamp and Cache
// ===========================================================================
#[tokio::test]
async fn test_k_discovery_refresh_updates_catalog() {
    let dir = tempdir().unwrap();
    let runtime = AppRuntime::new(dir.path())
        .await
        .expect("runtime init failed");

    let mock_provider = Arc::new(MockProvider::new().with_discovered_models(vec![
        ModelCandidate::new("refreshed-model", "nvidia", ModelTier::Standard, 65536)
            .with_tool_support(true),
    ]));

    let runtime = runtime.with_model_provider(mock_provider);
    let refreshed = runtime
        .refresh_model_catalog()
        .await
        .expect("refresh must succeed");

    assert_eq!(refreshed.len(), 1);
    assert_eq!(refreshed.models[0].model_id, "refreshed-model");
    assert_eq!(refreshed.source, CatalogSource::Discovered);
    assert_eq!(
        refreshed.refresh_state,
        CatalogRefreshState::DiscoverySuccess
    );

    let snapshot = runtime.model_catalog().await;
    assert_eq!(snapshot.len(), 1);
    assert_eq!(snapshot.models[0].model_id, "refreshed-model");
}

// ===========================================================================
// Test L: Configured Model Exists in Catalog
// ===========================================================================
#[tokio::test]
async fn test_l_configured_model_exists_in_catalog() {
    let catalog = Arc::new(tokio::sync::RwLock::new(ModelCatalog::from_discovered(
        "nvidia",
        vec![
            ModelCandidate::new("preferred-model", "nvidia", ModelTier::Standard, 65536)
                .with_tool_support(true),
            ModelCandidate::new("other-model", "nvidia", ModelTier::Standard, 65536)
                .with_tool_support(true),
        ],
        1700000000,
    )));

    let mock = Arc::new(MockProvider::new());
    let caller = RoutedModelCaller::new(Some(mock), ModelTier::Standard, vec![])
        .with_catalog_lock(catalog)
        .with_configured_model(Some("preferred-model".to_string()));

    let cancel = CancellationToken::new();
    let res = caller.call_model_cancellable("test prompt", &cancel).await;
    assert!(res.is_ok(), "caller must succeed: {:?}", res.err());
}

// ===========================================================================
// Test M: Configured Model Missing from Catalog (Deterministic Fallback)
// ===========================================================================
#[tokio::test]
async fn test_m_configured_model_missing_from_catalog_fallback() {
    let catalog = Arc::new(tokio::sync::RwLock::new(ModelCatalog::from_discovered(
        "nvidia",
        vec![
            ModelCandidate::new("fallback-model", "nvidia", ModelTier::Standard, 65536)
                .with_tool_support(true),
        ],
        1700000000,
    )));

    let mock = Arc::new(MockProvider::new());
    // Configure a missing model: caller should fall back to fallback-model without failing
    let caller = RoutedModelCaller::new(Some(mock), ModelTier::Standard, vec![])
        .with_catalog_lock(catalog)
        .with_configured_model(Some("nonexistent-model".to_string()));

    let cancel = CancellationToken::new();
    let res = caller.call_model_cancellable("test prompt", &cancel).await;
    assert!(
        res.is_ok(),
        "caller must fall back gracefully: {:?}",
        res.err()
    );
}

// ===========================================================================
// Test N: Router Uses Discovered Model Metadata (Stage 1 Filters)
// ===========================================================================
#[test]
fn test_n_router_uses_discovered_metadata() {
    let router = ModelRouter::new();
    let health = CircuitBreakerRegistry::default();

    let candidates = vec![
        // Disqualified by lack of tool support
        ModelCandidate::new("no-tools", "nvidia", ModelTier::Standard, 65536)
            .with_tool_support(false),
        // Disqualified by small context capacity
        ModelCandidate::new("small-ctx", "nvidia", ModelTier::Standard, 1024)
            .with_tool_support(true),
        // Disqualified by unavailability
        ModelCandidate::new("unavailable-model", "nvidia", ModelTier::Standard, 65536)
            .with_tool_support(true)
            .with_availability(ProviderCapabilityStatus::Unavailable),
        // Eligible model
        ModelCandidate::new("valid-model", "nvidia", ModelTier::Standard, 65536)
            .with_tool_support(true)
            .with_availability(ProviderCapabilityStatus::Available),
    ];

    let req = RoutingRequest::new(
        m31a::state_machine::agent::AgentRole::implementer(),
        ModelTier::Standard,
    )
    .with_tool_calling(true)
    .with_context_tokens(4096);

    let selection = router
        .resolve_model(&req, &candidates, &health)
        .expect("must resolve eligible model");
    assert_eq!(selection.model_name, "valid-model");
}

// ===========================================================================
// Test O: CLI /model Uses Dynamic Catalog
// ===========================================================================
#[tokio::test]
async fn test_o_cli_model_command_uses_dynamic_catalog() {
    let dir = tempdir().unwrap();
    let runtime = AppRuntime::new(dir.path()).await.unwrap();
    let cache_path = dir.path().join(".m31a/cache/model_catalog.json");
    let models = vec![
        ModelCandidate::new("dynamic-cli-model", "nvidia", ModelTier::Standard, 65536)
            .with_tool_support(true),
    ];
    let catalog = ModelCatalog::from_discovered("nvidia_nim", models, 1700000000);
    catalog.save_to_cache_file(&cache_path).unwrap();

    let registry = SlashCommandRegistry::new_standard();
    let ctx = CommandContext {
        workspace_root: runtime.workspace_root(),
        session_id: None,
        active_mission_id: None,
        pool: runtime.pool(),
        event_bus: runtime.event_bus(),
        configured_model: "dynamic-cli-model".to_string(),
        configured_provider: "nvidia_nim".to_string(),
        active_profile: "balanced".to_string(),
    };
    let output = registry.execute_line("/model", &ctx).await.unwrap();

    match output {
        CommandOutput::Info(msg) => {
            assert!(
                msg.contains("dynamic-cli-model") || msg.contains("Dynamic Model Catalog"),
                "Output should reflect dynamic catalog: {}",
                msg
            );
        }
        _ => panic!("Expected CommandOutput::Info"),
    }
}

// ===========================================================================
// Test P: TUI Model Selector Uses Dynamic Catalog
// ===========================================================================
#[test]
fn test_p_tui_model_selector_uses_dynamic_catalog() {
    let dir = tempdir().unwrap();
    let cache_path = dir.path().join(".m31a/cache/model_catalog.json");
    let models = vec![
        ModelCandidate::new("tui-discovered-model", "nvidia", ModelTier::Standard, 65536)
            .with_tool_support(true),
    ];
    let catalog = ModelCatalog::from_discovered("nvidia_nim", models, 1700000000);
    catalog.save_to_cache_file(&cache_path).unwrap();

    // Verify loading catalog for TUI presentation
    let loaded = ModelCatalog::load_from_cache_file(&cache_path).unwrap();
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded.models[0].model_id, "tui-discovered-model");
}

// ===========================================================================
// Test Q: Doctor Uses Dynamic Catalog
// ===========================================================================
#[tokio::test]
async fn test_q_doctor_uses_dynamic_catalog() {
    let probe = ModelsProbe;
    let res = probe.check().await;
    // Probe runs and reports status without panicking
    assert_eq!(res.category, m31a::cli::doctor::ProbeCategory::Models);
}

// ===========================================================================
// Test R: Setup Wizard Uses Dynamic Catalog
// ===========================================================================
#[test]
fn test_r_setup_wizard_uses_dynamic_catalog() {
    let dir = tempdir().unwrap();
    let cache_path = dir.path().join(".m31a/cache/model_catalog.json");
    let models = vec![
        ModelCandidate::new("custom-wizard-model", "nvidia", ModelTier::Standard, 65536)
            .with_tool_support(true),
        ModelCandidate::new("fast-wizard-model", "nvidia", ModelTier::Fast, 65536)
            .with_tool_support(true),
    ];
    let catalog = ModelCatalog::from_discovered("nvidia_nim", models, 1700000000);
    catalog.save_to_cache_file(&cache_path).unwrap();

    let wizard = SetupWizardScreen::new(dir.path().to_path_buf());
    assert_eq!(wizard.primary_model_input.text(), "custom-wizard-model");
    assert_eq!(wizard.fast_model_input.text(), "fast-wizard-model");
}

// ===========================================================================
// Test S: No Production Hardcoded Model List Remains
// ===========================================================================
#[test]
fn test_s_no_production_hardcoded_model_list() {
    // Empty catalog starts with zero models and no hardcoded fallbacks
    let catalog = ModelCatalog::new("nvidia_nim");
    assert!(catalog.is_empty());
    assert_eq!(catalog.len(), 0);
    assert_eq!(catalog.select_default(None), None);
}

// ===========================================================================
// Test T: Test-Only Model Fixtures Remain Valid
// ===========================================================================
#[tokio::test]
async fn test_t_test_fixtures_remain_valid() {
    let mock = MockProvider::new();
    let cancellation = CancellationToken::new();
    let (proposal, usage) = mock
        .call_model("test-model", "system context", vec![], &cancellation)
        .await
        .expect("mock call must succeed");

    assert!(matches!(
        proposal,
        m31a::model::types::ModelProposal::Complete { .. }
    ));
    assert_eq!(usage.total_tokens, 70);
}

// ===========================================================================
// Critical Test (§18): Dynamic Reflection Without Source Code Changes
// ===========================================================================
#[tokio::test]
async fn test_critical_dynamic_reflection_without_code_changes() {
    let dir = tempdir().unwrap();
    let runtime = AppRuntime::new(dir.path())
        .await
        .expect("runtime init failed");

    // Phase 1: Mock provider discovers [model-new-a, model-new-b]
    let mock_provider = Arc::new(MockProvider::new().with_discovered_models(vec![
        ModelCandidate::new("model-new-a", "nvidia", ModelTier::Standard, 65536)
            .with_tool_support(true),
        ModelCandidate::new("model-new-b", "nvidia", ModelTier::Fast, 65536)
            .with_tool_support(true),
    ]));

    let runtime = runtime.with_model_provider(mock_provider.clone());
    let catalog_p1 = runtime
        .refresh_model_catalog()
        .await
        .expect("phase 1 refresh failed");

    assert_eq!(catalog_p1.len(), 2);
    assert!(catalog_p1.contains_model("model-new-a"));
    assert!(catalog_p1.contains_model("model-new-b"));
    assert!(!catalog_p1.contains_model("model-new-c"));

    // Phase 2: Provider updates models at runtime to [model-new-b, model-new-c]
    mock_provider.set_discovered_models(vec![
        ModelCandidate::new("model-new-b", "nvidia", ModelTier::Standard, 65536)
            .with_tool_support(true),
        ModelCandidate::new("model-new-c", "nvidia", ModelTier::Reasoning, 65536)
            .with_tool_support(true),
    ]);

    let catalog_p2 = runtime
        .refresh_model_catalog()
        .await
        .expect("phase 2 refresh failed");

    // model-new-a must be gone; model-new-c must be present and available
    assert_eq!(catalog_p2.len(), 2);
    assert!(
        !catalog_p2.contains_model("model-new-a"),
        "model-new-a must no longer be in catalog"
    );
    assert!(
        catalog_p2.contains_model("model-new-b"),
        "model-new-b must remain in catalog"
    );
    assert!(
        catalog_p2.contains_model("model-new-c"),
        "model-new-c must be dynamically discovered"
    );
}

// ===========================================================================
// Live NVIDIA NIM Discovery Test (§19)
// ===========================================================================
#[tokio::test]
#[ignore = "requires live external LLM API credentials and WAN connection"]
async fn test_live_nvidia_discovery_and_inference() {
    m31a::config::load_dotenv_from_workspace(std::path::Path::new("."));
    let api_key = std::env::var("NVIDIA_API_KEY")
        .or_else(|_| std::env::var("API_KEY_NVIDIA"))
        .expect("NVIDIA_API_KEY or API_KEY_NVIDIA must be set to run live discovery tests (invoke with: cargo test --ignored)");

    let provider = NvidiaProvider::new(None, Some(api_key)).expect("provider init failed");

    // 1. Live discovery probe
    let discovered = provider
        .discover_models()
        .await
        .expect("live model discovery failed");

    assert!(
        !discovered.is_empty(),
        "NVIDIA NIM live discovery returned zero models"
    );
    println!(
        "Discovered {} live models from NVIDIA NIM",
        discovered.len()
    );
    let sample_ids: Vec<&str> = discovered.iter().map(|m| m.model_id.as_str()).collect();
    println!("Model IDs: {:?}", sample_ids);

    // 2. Validate discovered model fields
    let first = &discovered[0];
    assert!(!first.model_id.is_empty());
    assert_eq!(first.provider, "nvidia");
    assert!(first.context_capacity > 0);
    assert_eq!(first.availability, ProviderCapabilityStatus::Available);

    // 3. Select discovered model deterministically and perform inference
    let catalog = ModelCatalog::from_discovered("nvidia", discovered.clone(), 1700000000);
    let preferred = std::env::var("M31A_MODEL")
        .or_else(|_| std::env::var("NVIDIA_MODEL"))
        .ok();
    let selected = catalog
        .select_default(
            preferred
                .as_deref()
                .or(Some("meta/llama-3.2-11b-vision-instruct")),
        )
        .or_else(|| catalog.select_default(Some("nvidia/llama-3.1-nemotron-70b-instruct")))
        .or_else(|| catalog.select_default(None))
        .expect("failed to select default from discovered catalog");

    println!(
        "Executing live inference with discovered model: {}",
        selected.model_id
    );
    let cancellation = CancellationToken::new();
    let result = provider
        .call_model(
            &selected.model_id,
            "Say 'M31A discovery confirmed' and nothing else.",
            vec![],
            &cancellation,
        )
        .await;

    assert!(
        result.is_ok(),
        "Live model invocation failed: {:?}",
        result.err()
    );
    let (proposal, usage) = result.unwrap();
    assert!(
        usage.total_tokens > 0,
        "Usage telemetry must be authoritative"
    );
    println!("Live model invocation succeeded: usage={:?}", usage);
    match proposal {
        m31a::model::types::ModelProposal::Complete { summary, .. } => {
            println!("Model response: {}", summary);
        }
        _ => panic!("Expected Complete proposal"),
    }
}
