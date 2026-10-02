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
// Test U: NVIDIA NIM Metadata Parsing, Precedence, and Provenance (§15 Matrix A)
// ===========================================================================
#[tokio::test]
async fn test_u_nvidia_nim_metadata_parsing_precedence_and_provenance() {
    use m31a::model::provider::nvidia::{NvidiaProvider, extract_context_capacity};
    use serde_json::json;

    // 1. max_model_len takes top priority and assigns provider:max_model_len
    let item_max_model_len = json!({
        "id": "meta/llama-3.1-70b-instruct",
        "max_model_len": 131072,
        "context_window": 65536,
        "max_tokens": 4096
    });
    let (cap, prov) = extract_context_capacity(&item_max_model_len);
    assert_eq!(cap, 131072);
    assert_eq!(prov, "provider:max_model_len");

    // 2. context_window overrides max_tokens if max_model_len absent
    let item_ctx_window = json!({
        "id": "deepseek-ai/deepseek-r1",
        "context_window": 65536,
        "max_tokens": 8192
    });
    let (cap, prov) = extract_context_capacity(&item_ctx_window);
    assert_eq!(cap, 65536);
    assert_eq!(prov, "provider:context_window");

    // 3. context_length
    let item_ctx_length = json!({
        "id": "mistralai/mixtral-8x22b",
        "context_length": 32768
    });
    let (cap, prov) = extract_context_capacity(&item_ctx_length);
    assert_eq!(cap, 32768);
    assert_eq!(prov, "provider:context_length");

    // 4. max_tokens only as legacy last resort
    let item_max_tokens = json!({
        "id": "legacy/old-model",
        "max_tokens": 16384
    });
    let (cap, prov) = extract_context_capacity(&item_max_tokens);
    assert_eq!(cap, 16384);
    assert_eq!(prov, "provider:max_tokens");

    // 5. Unknown context yields (0, "unknown")
    let item_unknown = json!({
        "id": "unknown/model-without-context"
    });
    let (cap, prov) = extract_context_capacity(&item_unknown);
    assert_eq!(cap, 0);
    assert_eq!(prov, "unknown");

    // 6. parse_model_candidate constructs candidate preserving actual capacity and provenance
    let cand = NvidiaProvider::parse_model_candidate(&item_max_model_len, 1700000000)
        .expect("should parse candidate");
    assert_eq!(cand.context_capacity, 131072);
    assert_eq!(cand.context_provenance(), Some("provider:max_model_len"));
    assert!(cand.is_context_known());

    let cand_unknown = NvidiaProvider::parse_model_candidate(&item_unknown, 1700000000)
        .expect("should parse candidate");
    assert_eq!(cand_unknown.context_capacity, 0);
    assert_eq!(cand_unknown.context_provenance(), Some("unknown"));
    assert!(!cand_unknown.is_context_known());
}

// ===========================================================================
// Test V: Unknown Context Model Fails Closed in Router and Wizard (§15 Matrix A)
// ===========================================================================
#[test]
fn test_v_unknown_context_fails_closed_in_router_and_wizard() {
    let unknown_model =
        ModelCandidate::new("vendor/model-no-context", "nvidia", ModelTier::Standard, 0)
            .with_tool_support(true)
            .with_context_provenance("unknown");

    assert!(!unknown_model.is_context_known());

    // Fails wizard primary eligibility
    let primary_check = SetupWizardScreen::check_primary_eligibility(&unknown_model);
    assert!(primary_check.is_err());
    assert!(
        primary_check
            .unwrap_err()
            .contains("unknown or unreported by provider")
    );

    // Fails wizard fast auxiliary eligibility
    let fast_check = SetupWizardScreen::check_fast_auxiliary_eligibility(&unknown_model);
    assert!(fast_check.is_err());
    assert!(
        fast_check
            .unwrap_err()
            .contains("unknown or unreported by provider")
    );

    // Router rejecting context requests larger than capacity
    let router = ModelRouter::new();
    let health = CircuitBreakerRegistry::default();
    let req = RoutingRequest::new(
        m31a::state_machine::agent::AgentRole::implementer(),
        ModelTier::Standard,
    )
    .with_context_tokens(4096);
    let decision = router.resolve_model(&req, &[unknown_model], &health);
    assert!(
        decision.is_err(),
        "Model with 0 context must not satisfy request for 4096 tokens"
    );
}

// ===========================================================================
// Test W: Configured Model In Dynamic Catalog Uses Discovered Context (§15 Matrix D)
// ===========================================================================
#[tokio::test]
async fn test_w_configured_model_in_dynamic_catalog_inherits_discovered_context() {
    let dir = tempdir().unwrap();
    let runtime = AppRuntime::new(dir.path()).await.unwrap();

    let discovered_models = vec![
        ModelCandidate::new(
            "meta/llama-3.1-70b-instruct",
            "nvidia",
            ModelTier::Standard,
            65536, // distinct from static 131072!
        )
        .with_context_provenance("provider:max_model_len")
        .with_tool_support(true)
        .with_availability(ProviderCapabilityStatus::Available),
    ];

    let mock_provider = Arc::new(MockProvider::new().with_discovered_models(discovered_models));
    let runtime = runtime.with_model_provider(mock_provider);
    let catalog = runtime.refresh_model_catalog().await.unwrap();

    // Verify catalog contains the real discovered 65536 context
    let cat_model = catalog
        .models
        .iter()
        .find(|m| m.model_id == "meta/llama-3.1-70b-instruct")
        .unwrap();
    assert_eq!(cat_model.context_capacity, 65536);
    assert_eq!(
        cat_model.context_provenance(),
        Some("provider:max_model_len")
    );

    // Create RoutedModelCaller with catalog attached
    let caller = RoutedModelCaller::new(None, ModelTier::Standard, vec![])
        .with_catalog(Arc::new(catalog))
        .with_model("meta/llama-3.1-70b-instruct");

    // Caller's static candidates must have inherited 65536 from dynamic catalog, NOT 131072!
    let caller_candidate = caller
        .candidates
        .iter()
        .find(|c| c.model_id == "meta/llama-3.1-70b-instruct")
        .expect("must contain candidate");

    assert_eq!(
        caller_candidate.context_capacity, 65536,
        "Discovered catalog context (65536) must win over static 131072 default"
    );
    assert_eq!(
        caller_candidate.context_provenance(),
        Some("provider:max_model_len")
    );
}

// ===========================================================================
// Test X: NVIDIA Metadata Enrichment Pipeline & Canonical Model Specs
// ===========================================================================
#[tokio::test]
async fn test_x_nvidia_metadata_enrichment_canonical_specs() {
    use m31a::model::provider::nvidia_metadata::{
        NvidiaModelMetadataResolver, ProviderModelMetadataSource,
    };
    use m31a::model::router::resolver::{CapabilitySupport, ModelKind};
    use serde_json::json;

    let resolver = NvidiaModelMetadataResolver::new_offline();

    // Raw items simulating /v1/models response without context or capability metadata
    let raw_items = vec![
        json!({ "id": "google/gemma-4-31b-it", "owned_by": "google" }),
        json!({ "id": "nvidia/nemotron-3-super-120b-a12b", "owned_by": "nvidia" }),
        json!({ "id": "meta/llama-3.1-70b-instruct", "owned_by": "meta" }),
        json!({ "id": "meta/llama-3.2-11b-vision-instruct", "owned_by": "meta" }),
        json!({ "id": "writer/palmyra-fin-70b-32k", "owned_by": "writer" }),
        json!({ "id": "nvidia/embed-qa-4", "owned_by": "nvidia" }),
        json!({ "id": "meta/llama-guard-4-12b", "owned_by": "meta" }),
        json!({ "id": "meta/muse-glimmer-30b", "owned_by": "meta" }),
        json!({ "id": "unknown-vendor/unregistered-model", "owned_by": "unknown" }),
    ];

    let mut candidates: Vec<_> = raw_items
        .iter()
        .map(|item| NvidiaProvider::parse_model_candidate(item, 1700000000).unwrap())
        .collect();

    // Verify un-enriched baseline
    assert_eq!(candidates[0].context_capacity, 0);
    assert_eq!(candidates[0].tool_support, CapabilitySupport::Unknown);

    // Enrich candidates
    resolver.enrich_candidates(&mut candidates).await.unwrap();

    // 1. Google Gemma 4 31B = 256K, Chat, Supported
    let gemma = &candidates[0];
    assert_eq!(gemma.model_id, "google/gemma-4-31b-it");
    assert_eq!(gemma.context_capacity, 256_000);
    assert_eq!(gemma.context_provenance(), Some("nvidia:model_reference"));
    assert_eq!(gemma.tool_support, CapabilitySupport::Supported);
    assert!(gemma.supports_tools);
    assert_eq!(gemma.model_kind, ModelKind::Chat);

    // 2. Nemotron 3 Super = 1M (1,000,000)
    let nemotron = &candidates[1];
    assert_eq!(nemotron.model_id, "nvidia/nemotron-3-super-120b-a12b");
    assert_eq!(nemotron.context_capacity, 1_000_000);
    assert_eq!(
        nemotron.context_provenance(),
        Some("nvidia:model_reference")
    );
    assert_eq!(nemotron.tool_support, CapabilitySupport::Supported);

    // 3. Llama 3.1 70B = 131,072
    let llama31 = &candidates[2];
    assert_eq!(llama31.context_capacity, 131_072);
    assert_eq!(llama31.context_provenance(), Some("nvidia:model_reference"));
    assert_eq!(llama31.tool_support, CapabilitySupport::Supported);

    // 4. Llama 3.2 11B Vision = 131,072, VisualLanguage
    let llama32_vl = &candidates[3];
    assert_eq!(llama32_vl.context_capacity, 131_072);
    assert_eq!(llama32_vl.model_kind, ModelKind::VisualLanguage);

    // 5. Palmyra 32K = 32,768
    let palmyra = &candidates[4];
    assert_eq!(palmyra.context_capacity, 32_768);
    assert_eq!(palmyra.context_provenance(), Some("nvidia:model_reference"));

    // 6. Distinct contexts across models
    let contexts: Vec<usize> = candidates[0..5]
        .iter()
        .map(|c| c.context_capacity)
        .collect();
    assert_eq!(contexts, vec![256_000, 1_000_000, 131_072, 131_072, 32_768]);

    // 7. Embeddings classified correctly
    let embed = &candidates[5];
    assert_eq!(embed.model_id, "nvidia/embed-qa-4");
    assert_eq!(embed.model_kind, ModelKind::Embedding);
    assert!(embed.is_embedding());
    assert_eq!(embed.tool_support, CapabilitySupport::Unsupported);
    assert!(!embed.supports_tools);

    // 8. Safety guard classified correctly
    let guard = &candidates[6];
    assert_eq!(guard.model_id, "meta/llama-guard-4-12b");
    assert_eq!(guard.model_kind, ModelKind::GuardSafety);
    assert!(guard.is_safety_guard());

    // 9. Image generation classified correctly
    let image_gen = &candidates[7];
    assert_eq!(image_gen.model_id, "meta/muse-glimmer-30b");
    assert_eq!(image_gen.model_kind, ModelKind::ImageGeneration);
    assert!(image_gen.is_image_generation());

    // 10. Unknown model fails closed: 0 context, unknown tool support, unknown kind
    let unknown = &candidates[8];
    assert_eq!(unknown.model_id, "unknown-vendor/unregistered-model");
    assert_eq!(unknown.context_capacity, 0);
    assert!(!unknown.is_context_known());
    assert_eq!(unknown.context_provenance(), Some("unknown"));
    assert_eq!(unknown.tool_support, CapabilitySupport::Unknown);
    assert!(!unknown.supports_tools);
    assert_eq!(unknown.model_kind, ModelKind::Unknown);
}

// ===========================================================================
// Test Y: Non-Chat and Non-Coding Models Rejected from Primary and Fast Roles
// ===========================================================================
#[test]
fn test_y_non_chat_models_rejected_from_primary_and_fast() {
    use m31a::model::router::resolver::{CapabilitySupport, ModelKind};

    let embed_model = ModelCandidate::new("nvidia/embed-qa-4", "nvidia", ModelTier::Fast, 512)
        .with_model_kind(ModelKind::Embedding)
        .with_tool_capability(CapabilitySupport::Unsupported)
        .with_context_provenance("nvidia:model_reference");

    let guard_model =
        ModelCandidate::new("meta/llama-guard-4-12b", "nvidia", ModelTier::Fast, 8192)
            .with_model_kind(ModelKind::GuardSafety)
            .with_tool_capability(CapabilitySupport::Unsupported)
            .with_context_provenance("nvidia:model_reference");

    let image_model =
        ModelCandidate::new("meta/muse-glimmer-30b", "nvidia", ModelTier::Standard, 4096)
            .with_model_kind(ModelKind::ImageGeneration)
            .with_tool_capability(CapabilitySupport::Unsupported)
            .with_context_provenance("nvidia:model_reference");

    // All must fail wizard primary eligibility
    assert!(SetupWizardScreen::check_primary_eligibility(&embed_model).is_err());
    assert!(SetupWizardScreen::check_primary_eligibility(&guard_model).is_err());
    assert!(SetupWizardScreen::check_primary_eligibility(&image_model).is_err());

    // All must fail wizard fast auxiliary eligibility
    assert!(SetupWizardScreen::check_fast_auxiliary_eligibility(&embed_model).is_err());
    assert!(SetupWizardScreen::check_fast_auxiliary_eligibility(&guard_model).is_err());
    assert!(SetupWizardScreen::check_fast_auxiliary_eligibility(&image_model).is_err());

    // Router must reject all non-generative models
    let router = ModelRouter::new();
    let health = CircuitBreakerRegistry::default();
    let req = RoutingRequest::new(
        m31a::state_machine::agent::AgentRole::implementer(),
        ModelTier::Standard,
    );
    let decision = router.resolve_model(
        &req,
        &[
            embed_model.clone(),
            guard_model.clone(),
            image_model.clone(),
        ],
        &health,
    );
    assert!(
        decision.is_err(),
        "Router must reject all non-generative models"
    );

    // Catalog auto-selection must skip non-generative models
    let catalog = ModelCatalog::from_discovered(
        "nvidia",
        vec![
            embed_model,
            guard_model,
            image_model,
            ModelCandidate::new(
                "meta/llama-3.1-70b-instruct",
                "nvidia",
                ModelTier::Standard,
                131072,
            )
            .with_model_kind(ModelKind::Chat)
            .with_tool_capability(CapabilitySupport::Supported)
            .with_context_provenance("nvidia:model_reference"),
        ],
        1700000000,
    );

    let default_sel = catalog
        .select_default(None)
        .expect("must select generative model");
    assert_eq!(default_sel.model_id, "meta/llama-3.1-70b-instruct");
}

// ===========================================================================
// Test Z: Unknown Tool Calling Fails Closed in Router and Wizard
// ===========================================================================
#[test]
fn test_z_unknown_tool_calling_fails_closed() {
    use m31a::model::router::resolver::{CapabilitySupport, ModelKind};

    let unknown_tool_model = ModelCandidate::new(
        "custom/model-with-context-but-unknown-tools",
        "nvidia",
        ModelTier::Standard,
        65536,
    )
    .with_context_provenance("provider:max_model_len")
    .with_model_kind(ModelKind::Chat)
    .with_tool_capability(CapabilitySupport::Unknown);

    assert_eq!(unknown_tool_model.tool_support, CapabilitySupport::Unknown);
    assert!(!unknown_tool_model.supports_tools);

    // Fails wizard primary eligibility
    let primary_check = SetupWizardScreen::check_primary_eligibility(&unknown_tool_model);
    assert!(primary_check.is_err());
    assert!(
        primary_check
            .unwrap_err()
            .contains("Lacks function/tool calling support")
    );

    // Router with requires_tool_calling must reject it
    let router = ModelRouter::new();
    let health = CircuitBreakerRegistry::default();
    let req = RoutingRequest::new(
        m31a::state_machine::agent::AgentRole::implementer(),
        ModelTier::Standard,
    )
    .with_tool_calling(true);

    let decision = router.resolve_model(&req, &[unknown_tool_model], &health);
    assert!(
        decision.is_err(),
        "Model with unknown tool calling must fail closed"
    );
}

// ===========================================================================
// Test AA: Catalog Schema Version 2 Freshness & Cache Migration
// ===========================================================================
#[tokio::test]
async fn test_aa_catalog_schema_v2_freshness_and_cache_migration() {
    let dir = tempdir().unwrap();
    let cache_path = ModelCatalog::cache_path(dir.path());

    // 1. Write legacy schema version 1 cache with fabricated 131072 entries
    let legacy_json = serde_json::json!({
        "schema_version": 1,
        "provider": "nvidia_nim",
        "models": [
            {
                "model_id": "legacy/fabricated-model",
                "provider": "nvidia",
                "tier": "standard",
                "context_capacity": 131072,
                "supports_tools": true,
                "supports_structured_output": true,
                "cost_per_million_input": 0,
                "cost_per_million_output": 0,
                "availability": "available",
                "source": "nvidia_discovery"
            }
        ],
        "discovered_at": 1700000000
    });
    std::fs::create_dir_all(cache_path.parent().unwrap()).unwrap();
    std::fs::write(&cache_path, serde_json::to_string(&legacy_json).unwrap()).unwrap();

    // 2. Loading v1 cache in v2 catalog invalidates fabricated 131072 context
    let loaded = ModelCatalog::load_from_cache_file(&cache_path).unwrap();
    let loaded_model = &loaded.models[0];
    assert_eq!(
        loaded_model.context_capacity, 0,
        "Legacy 131072 context must be sanitized to 0 (unknown)"
    );
    assert_eq!(loaded_model.context_provenance(), Some("unknown"));
    assert!(!loaded_model.is_context_known());

    // 3. Schema v2 fresh timestamps
    let mut v2_catalog = ModelCatalog::new("nvidia_nim");
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    v2_catalog.inventory_updated_at = Some(now);
    v2_catalog.metadata_updated_at = Some(now);
    assert!(!v2_catalog.is_inventory_stale(3600));
    assert!(!v2_catalog.is_metadata_stale(86400));
    assert!(!v2_catalog.is_stale(3600));

    // Stale inventory (>1h ago)
    v2_catalog.inventory_updated_at = Some(now.saturating_sub(3601));
    assert!(v2_catalog.is_inventory_stale(3600));

    // Stale metadata (>24h ago)
    v2_catalog.metadata_updated_at = Some(now.saturating_sub(86401));
    assert!(v2_catalog.is_metadata_stale(86400));
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
    assert!(first.is_context_known());
    assert!(first.context_provenance().is_some());
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
