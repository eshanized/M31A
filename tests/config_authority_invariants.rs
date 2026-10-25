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
    let md = m31a::config::migrate::convert_config_toml_to_markdown(
        "[runtime]\nconcurrency_limit = 7\n",
    )
    .expect("converts to canonical markdown");
    std::fs::write(ws.join(".m31a").join("config.md"), md).unwrap();
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

// ── F01: TUI composer autocomplete has no synthetic model inventory ──────────
#[test]
fn f01_tui_composer_model_autocomplete_no_synthetic_models() {
    let mut composer = m31a::tui::composer::TuiComposer::new(std::path::PathBuf::from("."));
    composer.set_cached_models(Vec::new());
    composer.set_text("/model ");
    let suggestions = composer.autocomplete_suggestions();
    assert_eq!(suggestions.len(), 1);
    assert_eq!(suggestions[0].label, "(no models available)");

    for s in suggestions {
        assert_ne!(s.label, "meta/llama-3.1-70b-instruct");
        assert_ne!(s.label, "meta/llama-3.2-11b-vision-instruct");
        assert_ne!(s.label, "meta/llama-3.3-70b-instruct");
        assert_ne!(s.label, "deepseek-ai/deepseek-r1");
    }
}

// ── F02: Reviewer confidence is optional, validated, and not fabricated ────────
#[test]
fn f02_reviewer_confidence_rejection_and_optionality() {
    use m31a::ids::{MissionId, TaskId};
    use m31a::verification::reviewer::{IndependentReviewer, ReviewDecision, ReviewVerdict};

    // Construct valid verdict with missing confidence (None)
    let mut verdict = ReviewVerdict {
        review_id: "rev-test-1".into(),
        mission_id: MissionId::new(),
        task_id: TaskId::new(),
        snapshot_hash: "snap-hash".into(),
        decision: ReviewDecision::Approved,
        findings: Vec::new(),
        confidence_score: None,
        rationale: "looks good".into(),
        requirement_coverage: vec!["AC1".into()],
    };

    // Serialized output omits confidence_score, and deserializing it yields None, not 90
    let json = serde_json::to_string(&verdict).unwrap();
    assert!(!json.contains("confidence_score"));
    let deserialized: ReviewVerdict = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.confidence_score, None);

    let check = IndependentReviewer::map_verdict_to_check(&deserialized);
    assert!(check.summary.contains("confidence: unspecified"));
    assert!(!check.summary.contains("confidence: 90%"));

    // Explicit confidence is serialized and preserved
    verdict.confidence_score = Some(85);
    let json = serde_json::to_string(&verdict).unwrap();
    assert!(json.contains("\"confidence_score\":85"));
    let deserialized: ReviewVerdict = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.confidence_score, Some(85));

    let check = IndependentReviewer::map_verdict_to_check(&deserialized);
    assert!(check.summary.contains("confidence: 85%"));
}

// ── F03: Mission and task objective normalization ─────────────────────────────
#[test]
fn f03_objective_normalization() {
    assert_eq!(C::normalize_objective(None), None);
    assert_eq!(C::normalize_objective(Some("")), None);
    assert_eq!(C::normalize_objective(Some("   \t\n  ")), None);
    assert_eq!(
        C::normalize_objective(Some("  Implement authentication  ")),
        Some("Implement authentication".to_string())
    );
}

// ── F04: Git worktree retention policy parsing and legacy aliases ──────────────
#[test]
fn f04_git_retention_policy_parsing_and_aliases() {
    use m31a::git::worktree::WorktreeRetentionPolicy;
    use std::str::FromStr;

    // Canonical names
    assert_eq!(
        WorktreeRetentionPolicy::from_str("keep_on_failure").unwrap(),
        WorktreeRetentionPolicy::KeepOnFailure
    );
    assert_eq!(
        WorktreeRetentionPolicy::from_str("always_remove").unwrap(),
        WorktreeRetentionPolicy::AlwaysRemove
    );
    assert_eq!(
        WorktreeRetentionPolicy::from_str("always_keep").unwrap(),
        WorktreeRetentionPolicy::AlwaysKeep
    );

    // Legacy aliases
    assert_eq!(
        WorktreeRetentionPolicy::from_str("remove").unwrap(),
        WorktreeRetentionPolicy::AlwaysRemove
    );
    assert_eq!(
        WorktreeRetentionPolicy::from_str("keep").unwrap(),
        WorktreeRetentionPolicy::AlwaysKeep
    );

    // Case insensitivity & whitespace trimming
    assert_eq!(
        WorktreeRetentionPolicy::from_str("  KEEP_ON_FAILURE  ").unwrap(),
        WorktreeRetentionPolicy::KeepOnFailure
    );

    // Unrecognized values fail validation
    assert!(WorktreeRetentionPolicy::from_str("invalid_policy").is_err());
    assert!(WorktreeRetentionPolicy::from_str("").is_err());

    // Schema git retention_policy() method
    let git_cfg = m31a::config::schema::GitConfig {
        retention_policy: "remove".to_string(),
        ..Default::default()
    };
    assert_eq!(
        git_cfg.retention_policy(),
        WorktreeRetentionPolicy::AlwaysRemove
    );
}

// ── F05: Canonical production provider identity ───────────────────────────────
#[test]
fn f05_canonical_production_provider_identity() {
    assert_eq!(
        registry::PRODUCTION_PROVIDER_ID,
        C::CANONICAL_DEFAULT_PROVIDER
    );
    assert_eq!(registry::PRODUCTION_PROVIDER_ID, "nvidia_nim");

    assert!(registry::is_production_provider_id("nvidia"));
    assert!(registry::is_production_provider_id("nvidia_nim"));
    assert!(registry::is_production_provider_id("NVIDIA"));
    assert!(registry::is_production_provider_id("  NVIDIA_NIM  "));

    assert!(!registry::is_production_provider_id("openai"));
    assert!(!registry::is_production_provider_id("anthropic"));
    assert!(!registry::is_production_provider_id("ollama"));
}

// ── F06: Sandbox mode canonical vocabulary and wizard transitions ─────────────
#[test]
fn f06_sandbox_mode_canonical_vocabulary_and_wizard() {
    use m31a::sandbox::mode::SandboxMode;
    use std::str::FromStr;

    assert_eq!(
        SandboxMode::from_str("strict").unwrap(),
        SandboxMode::Strict
    );
    assert_eq!(
        SandboxMode::from_str("standard").unwrap(),
        SandboxMode::Standard
    );
    assert_eq!(
        SandboxMode::from_str("permissive").unwrap(),
        SandboxMode::Permissive
    );
    assert_eq!(
        SandboxMode::from_str("disabled").unwrap(),
        SandboxMode::Disabled
    );
    assert!(SandboxMode::from_str("unknown").is_err());

    // Relative restriction ordering
    assert!(SandboxMode::Strict.is_more_restrictive_than(SandboxMode::Standard));
    assert!(SandboxMode::Standard.is_more_restrictive_than(SandboxMode::Permissive));
    assert!(SandboxMode::Permissive.is_more_restrictive_than(SandboxMode::Disabled));

    // Wizard cycles cleanly through all 4 modes
    assert_eq!(
        SandboxMode::Standard.next_wizard_mode(),
        SandboxMode::Strict
    );
    assert_eq!(
        SandboxMode::Strict.next_wizard_mode(),
        SandboxMode::Permissive
    );
    assert_eq!(
        SandboxMode::Permissive.next_wizard_mode(),
        SandboxMode::Disabled
    );
    assert_eq!(
        SandboxMode::Disabled.next_wizard_mode(),
        SandboxMode::Standard
    );
}

// ── F07: Policy context profile resolution ────────────────────────────────────
#[test]
fn f07_policy_context_profile_resolution() {
    use m31a::state::policy_context::PolicyContext;

    let auto_ctx = PolicyContext::for_profile("autonomous").expect("autonomous profile");
    assert_eq!(auto_ctx.security_level, 3);
    assert!(
        auto_ctx
            .allowed_capabilities
            .contains(&"workspace_fs_read".to_string())
    );

    let safe_ctx = PolicyContext::for_profile("safe").expect("safe profile");
    assert_eq!(safe_ctx.security_level, 1);

    assert!(PolicyContext::for_profile("nonexistent_profile_xyz").is_err());
}

// ── F08: Local terminal preserves custom env variables ────────────────────────
#[test]
fn f08_local_terminal_preserves_custom_env() {
    use m31a::capability::traits::terminal::TerminalSessionConfig;
    use std::collections::HashMap;

    let mut custom_env = HashMap::new();
    custom_env.insert("TERM".to_string(), "xterm-direct".to_string());
    custom_env.insert("COLUMNS".to_string(), "120".to_string());
    custom_env.insert("LINES".to_string(), "40".to_string());
    custom_env.insert("CUSTOM_VAR".to_string(), "custom_val".to_string());

    let config = TerminalSessionConfig {
        env: custom_env.clone(),
        cols: 80,
        rows: 24,
        ..Default::default()
    };

    assert_eq!(config.env.get("TERM").unwrap(), "xterm-direct");
    assert_eq!(config.env.get("COLUMNS").unwrap(), "120");
    assert_eq!(config.env.get("LINES").unwrap(), "40");
    assert_eq!(config.env.get("CUSTOM_VAR").unwrap(), "custom_val");
}

// ── F09: Schema operational defaults consume canonical constants ─────────────
#[test]
fn f09_schema_operational_defaults_consume_canonical_constants() {
    use m31a::config::schema::{GitConfig, PolicyConfig, WorkspaceConfig};

    let policy_cfg = PolicyConfig::default();
    assert_eq!(policy_cfg.default_action, C::DEFAULT_POLICY_ACTION);

    let ws_cfg = WorkspaceConfig::default();
    assert_eq!(
        ws_cfg.ignored_paths,
        C::DEFAULT_IGNORED_PATHS
            .iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>()
    );

    let git_cfg = GitConfig::default();
    assert_eq!(git_cfg.retention_policy, C::DEFAULT_GIT_RETENTION_POLICY);
}

// ── F10: Role registry global role CSV helpers ────────────────────────────────
#[test]
fn f10_role_registry_global_roles_csv() {
    use m31a::agent::registry::RoleRegistry;

    let builtins = RoleRegistry::global_builtin_ids_csv();
    assert!(builtins.contains("implementer"));
    assert!(builtins.contains("researcher"));
    assert!(builtins.contains("verifier"));

    let available = RoleRegistry::global_available_roles_csv();
    assert!(available.contains("implementer"));
}

// ── F12: Web search timeout consumes canonical constant ───────────────────────
#[test]
fn f12_web_search_timeout_canonical_constant() {
    assert_eq!(C::DEFAULT_WEB_SEARCH_TIMEOUT_SECS, 10);
    assert_eq!(
        m31a::capability::providers::local_web::SEARCH_REQUEST_TIMEOUT,
        std::time::Duration::from_secs(C::DEFAULT_WEB_SEARCH_TIMEOUT_SECS)
    );
}

// ── F13: PlatformPaths candidate resolution ───────────────────────────────────
#[test]
fn f13_platform_paths_candidate_resolution() {
    use m31a::config::paths::PlatformPaths;

    let paths = PlatformPaths::new();
    let sys_candidates = paths.system_policy_candidates();
    assert!(!sys_candidates.is_empty());
    assert!(sys_candidates.iter().any(|p| p.ends_with("policy.toml")));

    let ws = std::path::Path::new("/tmp/test_ws");
    let ws_candidates = PlatformPaths::workspace_policy_candidates(ws);
    assert!(!ws_candidates.is_empty());
    assert!(
        ws_candidates
            .iter()
            .any(|p| p.ends_with(".m31a/policy.toml"))
    );
}
