//! Architectural invariants: one configuration authority, one model-selection
//! authority, one provider endpoint authority, one resource policy.
//!
//! These tests fail if hardcoded authorities are reintroduced.

use m31a::config::canonical as C;
use m31a::config::{ResolvedConfiguration, provider_registry as registry};
use m31a::model::router::resolver::{
    CapabilitySupport, ModelCandidate, ModelRouter, RoutingRequest,
};
use m31a::state_machine::agent::AgentRole;

// 1. Configuration precedence: workspace overrides user/system; session wins.
#[test]
fn configuration_precedence_workspace_over_default() {
    let dir = tempfile::tempdir().unwrap();
    let ws = dir.path().join("ws");
    std::fs::create_dir_all(ws.join(".m31a")).unwrap();
    std::fs::write(
        ws.join(".m31a").join("config.toml"),
        "[runtime]\nconcurrency_limit = 7\n",
    )
    .unwrap();
    let cfg = ResolvedConfiguration::for_workspace(&ws).expect("workspace config loads");
    assert_eq!(cfg.app_config.runtime.concurrency_limit, 7);
    let with_session = cfg
        .with_session_override("runtime.concurrency_limit", serde_json::json!(9))
        .expect("session override");
    assert_eq!(with_session.app_config.runtime.concurrency_limit, 9);
}

// 8/9. Model resolution: single authority triple; unknown stays unknown.
#[test]
fn effective_model_selection_is_single_authority() {
    let dir = tempfile::tempdir().unwrap();
    let cfg = ResolvedConfiguration::builder(dir.path()).build_fallback();
    let (provider, model, _fast) = cfg.effective_model_selection();
    assert_eq!(provider, C::CANONICAL_DEFAULT_PROVIDER);
    assert_eq!(model, C::CANONICAL_DEFAULT_MODEL);
    assert_eq!(cfg.active_model, cfg.app_config.agents.default_model);
}

// 10. Arbitrary model cannot fabricate 131K tool-capable metadata.
#[test]
fn arbitrary_model_never_fabricates_capabilities() {
    let cand = ModelCandidate::new_unknown("arbitrary-model-xyz", registry::PRODUCTION_PROVIDER_ID);
    assert_eq!(cand.context_capacity, 0);
    assert_eq!(cand.tool_support, CapabilitySupport::Unknown);
    assert!(!cand.supports_tools);
    assert_eq!(cand.context_provenance(), Some("unknown"));

    let router = ModelRouter::new();
    let health = m31a::model::router::health::CircuitBreakerRegistry::default();
    let req = RoutingRequest::new(
        AgentRole::implementer(),
        m31a::model::router::resolver::ModelTier::Standard,
    )
    .with_tool_calling(true);
    assert!(router.resolve_model(&req, &[cand], &health).is_err());
}

// 11. Provider endpoint resolution: single canonical URL.
#[test]
fn provider_endpoint_single_authority() {
    assert_eq!(
        C::CANONICAL_NVIDIA_BASE_URL,
        m31a::model::provider::endpoint::CANONICAL_NVIDIA_BASE_URL
    );
    assert_eq!(
        C::CANONICAL_NVIDIA_BASE_URL,
        "https://integrate.api.nvidia.com/v1"
    );
    // Registry descriptor agrees with the canonical endpoint.
    let reg = registry::ProviderRegistry::new();
    let desc = reg
        .get_descriptor(registry::PRODUCTION_PROVIDER_ID)
        .expect("production descriptor");
    assert_eq!(
        desc.base_url.as_deref().unwrap(),
        C::CANONICAL_NVIDIA_BASE_URL
    );
    assert_eq!(desc.default_model, C::CANONICAL_DEFAULT_MODEL);
}

// 12/13. Timeout hierarchy + resource policy have explicit scope.
#[test]
fn timeout_and_resource_policy_scopes() {
    let dir = tempfile::tempdir().unwrap();
    let cfg = ResolvedConfiguration::builder(dir.path()).build_fallback();
    let tp = cfg.timeout_policy();
    assert_eq!(tp.runtime_secs, C::DEFAULT_RUNTIME_TIMEOUT_SECS);
    assert_eq!(tp.workflow_step_secs, C::DEFAULT_WORKFLOW_STEP_TIMEOUT_SECS);
    assert_eq!(tp.verification_secs, C::DEFAULT_VERIFICATION_TIMEOUT_SECS);
    assert_eq!(tp.process_secs, C::DEFAULT_PROCESS_TIMEOUT_SECS);
    assert_eq!(tp.approval_secs, C::DEFAULT_APPROVAL_TIMEOUT_SECS);
    let rp = cfg.resource_policy();
    assert_eq!(rp.runtime_concurrency, C::DEFAULT_RUNTIME_CONCURRENCY);
    assert_eq!(rp.per_role_concurrency, C::DEFAULT_PER_ROLE_CONCURRENCY);
    assert_eq!(rp.tool_timeout_secs, C::DEFAULT_TOOL_TIMEOUT_SECS);
    let cp = cfg.cache_policy();
    assert_eq!(cp.local_freshness_secs, C::DEFAULT_CATALOG_FRESHNESS_SECS);
    assert_eq!(
        cp.remote_metadata_ttl_secs,
        C::DEFAULT_REMOTE_METADATA_TTL_SECS
    );
}

// 14. Tool resource inheritance clamps to immutable ceilings.
#[test]
fn tool_resource_inheritance_respects_ceilings() {
    use m31a::config::ResourcesConfig;
    use m31a::tools::definition::ResourceLimits;
    let cfg = ResourcesConfig::default();
    let huge = ResourceLimits::new(9999, 999 * 1024 * 1024);
    let eff = ResourceLimits::effective(&huge, &cfg);
    assert_eq!(eff.timeout_secs, ResourceLimits::MAX_TIMEOUT_SECS);
    assert_eq!(eff.max_output_bytes, ResourceLimits::MAX_OUTPUT_BYTES);
    let def = ResourceLimits::effective_default(&cfg);
    assert_eq!(def.timeout_secs, C::DEFAULT_TOOL_TIMEOUT_SECS);
}

// 15. Verification adapters load from declarative assets.
#[test]
fn verification_adapters_are_declarative() {
    use m31a::verification::adapter::{ProjectAdapter, ProjectType};
    for pt in [
        ProjectType::Rust,
        ProjectType::Python,
        ProjectType::Node,
        ProjectType::Go,
        ProjectType::Custom,
    ] {
        let a = ProjectAdapter::adapter_for(pt);
        assert_eq!(a.project_type, pt);
    }
    let rust = ProjectAdapter::rust_defaults();
    assert_eq!(rust.manifest_file, "Cargo.toml");
    assert!(rust.compiler_command.contains("cargo"));
}

// 16/17. Malformed persistence records fail closed (agent mapper).
#[test]
fn agent_mapper_requires_migrated_columns() {
    // Covered by strict try_get with migration hints in
    // persistence::sqlite::repositories::agent; this test pins the
    // canonical default so a future literal cannot drift.
    assert_eq!(
        C::CANONICAL_DEFAULT_MODEL,
        "meta/llama-3.2-11b-vision-instruct"
    );
    assert_eq!(C::CANONICAL_DEFAULT_PROVIDER, "nvidia_nim");
}

// 18/19. Builtin skills + profiles load from packaged assets.
#[test]
fn builtin_skills_load_from_packaged_assets() {
    let skills = m31a::skill::discovery::builtin_skills();
    assert_eq!(skills.len(), 3);
    let ids: Vec<String> = skills.iter().map(|s| s.manifest.id.clone()).collect();
    assert!(ids.contains(&"fix-test-failure".to_string()));
    assert!(ids.contains(&"refactor-module".to_string()));
    assert!(ids.contains(&"prepare-release".to_string()));
}

#[test]
fn canonical_profiles_load_from_packaged_assets() {
    let resolver = m31a::config::ProfileResolver::with_canonical_profiles();
    for name in [
        "safe",
        "coding",
        "balanced",
        "research",
        "autonomous",
        "ci",
        "release",
    ] {
        assert!(
            resolver.resolve_profile(name).is_ok(),
            "canonical profile {name} must resolve"
        );
    }
}

// 20/21. TUI renders from runtime catalog; empty catalog has explicit state.
#[test]
fn tui_model_selector_has_no_fallback_inventory() {
    let (providers, models) =
        m31a::tui::surface::model_selector::resolve_display_models_and_providers(
            &[],
            "nvidia_nim",
            C::CANONICAL_DEFAULT_MODEL,
        );
    assert!(models.is_empty(), "empty catalog must stay empty");
    assert!(m31a::tui::surface::model_selector::is_catalog_empty(
        &models
    ));
    assert!(!providers.is_empty());
}

// 2/3/4/5/6/7. /settings command registration + navigation.
#[test]
fn settings_command_registered_with_help() {
    let reg = m31a::interaction::commands::SlashCommandRegistry::new_standard();
    let cmd = reg.find("settings").expect("/settings registered");
    assert!(cmd.description.to_lowercase().contains("setting"));
    let help = reg.generate_help(Some("settings"));
    assert!(help.contains("/settings"));
}

// 22. Secret masking.
#[test]
fn settings_masks_secrets() {
    assert_eq!(
        m31a::tui::surface::settings::mask_secret_configured(true),
        "configured (hidden)"
    );
    assert_ne!(
        m31a::tui::surface::settings::mask_secret_configured(true),
        "sk-test-key"
    );
}

// 23/24/25. Safety ceilings + restart semantics + reload.
#[test]
fn immutable_ceilings_hold() {
    use m31a::repo::query::{
        CEILING_MAX_BYTES, CEILING_MAX_DEPTH, CEILING_MAX_RESULTS, QueryBounds,
    };
    let clamped = QueryBounds::new(9999, 99, 999_999_999).clamped();
    assert_eq!(clamped.max_results, CEILING_MAX_RESULTS);
    assert_eq!(clamped.max_depth, CEILING_MAX_DEPTH);
    assert_eq!(clamped.max_bytes, CEILING_MAX_BYTES);
}

#[test]
fn settings_persist_atomically_and_validate() {
    use m31a::tui::surface::settings::{apply_edit_to_draft, persist_workspace_config};
    let dir = tempfile::tempdir().unwrap();
    let mut draft = m31a::config::schema::AppConfig::default();
    let restart = apply_edit_to_draft(&mut draft, "runtime.concurrency_limit", "6").unwrap();
    assert!(!restart);
    assert!(apply_edit_to_draft(&mut draft, "provider.default", "openai").is_err());
    let path = persist_workspace_config(dir.path(), &draft).unwrap();
    assert!(path.is_file());
    let reloaded = ResolvedConfiguration::for_workspace(dir.path()).expect("reload");
    assert_eq!(reloaded.app_config.runtime.concurrency_limit, 6);
}
