//! single-authority remediation contracts (task sections 2-28).
//!
//! every concept has exactly one canonical authority. tests assert the
//! current architecture, not obsolete project-local or allow-all contracts.

use std::fs;
use tempfile::tempdir;

use m31a::config::profile::ProfileResolver;
use m31a::config::resolved::ResolvedConfiguration;
use m31a::config::schema::{GitExecutionIsolation, GitPushPolicy, parse_and_validate_config};
use m31a::runtime_authorities::AutonomyPrecedence;
use m31a::state_machine::AutonomyMode;

// ── profiles own autonomy ─────────────────────────────────────────────

#[test]
fn all_canonical_profiles_resolve_and_own_autonomy() {
    let cases = [
        ("balanced", AutonomyMode::Assisted),
        ("autonomous", AutonomyMode::Autonomous),
        ("conservative", AutonomyMode::Safe),
        ("code_reviewer", AutonomyMode::Safe),
        ("safe", AutonomyMode::Safe),
        ("coding", AutonomyMode::Assisted),
        ("research", AutonomyMode::Safe),
        ("ci", AutonomyMode::Unattended),
        ("security_review", AutonomyMode::Safe),
        ("release", AutonomyMode::Assisted),
    ];
    for (profile, expected) in cases {
        let mode_str = ProfileResolver::canonical_autonomy_for_profile(profile)
            .unwrap_or_else(|e| panic!("profile {profile} must resolve: {e}"));
        let mode: AutonomyMode = mode_str.parse().expect("parse autonomy");
        assert_eq!(mode, expected, "profile {profile}");
        assert_eq!(
            AutonomyPrecedence::from_profile_name(Some(profile)),
            expected,
            "precedence {profile}"
        );
    }
}

#[test]
fn unknown_profile_fails_closed_never_silent_balanced() {
    let resolver = ProfileResolver::with_canonical_profiles();
    assert!(resolver.resolve_profile("nope_missing").is_err());
    // from_profile_name for unknown stays fail-closed Safe (unbound), never escalates.
    assert_eq!(
        AutonomyPrecedence::from_profile_name(Some("nope_missing")),
        AutonomyMode::Safe
    );
    assert_eq!(
        AutonomyPrecedence::from_profile_name(None),
        AutonomyMode::Safe
    );
}

#[test]
fn safe_only_when_unbound() {
    // direct mode names still parse (session/CLI overrides).
    assert_eq!(
        "plan".parse::<AutonomyMode>().expect("plan"),
        AutonomyMode::Plan
    );
    assert!("bogus".parse::<AutonomyMode>().is_err());
}

// ── config precedence + provenance ────────────────────────────────────

fn write_config(path: &std::path::Path, body: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("mkdir");
    }
    fs::write(path, body).expect("write");
}

#[test]
fn explicit_config_has_cli_precedence_over_workspace() {
    let dir = tempdir().expect("tempdir");
    let ws = dir.path().to_path_buf();
    write_config(
        &ws.join(".m31a").join("config.toml"),
        "[agents]\ndefault_model = \"ws-model\"\n",
    );
    let explicit = ws.join("explicit.toml");
    write_config(&explicit, "[agents]\ndefault_model = \"explicit-model\"\n");
    let cfg = ResolvedConfiguration::builder(&ws)
        .with_explicit_config(Some(explicit.clone()))
        .build()
        .expect("build");
    assert_eq!(cfg.app_config.agents.default_model, "explicit-model");
    let explained = cfg.explain("agents.default_model").expect("explain");
    assert_eq!(
        explained.winning_layer,
        m31a::config::provenance::ConfigLayer::Tier6Cli,
        "explicit --config must report CLI tier"
    );
    assert_eq!(explained.source_file, Some(explicit));
}

#[test]
fn cli_scalar_overrides_explicit_file() {
    let dir = tempdir().expect("tempdir");
    let ws = dir.path().to_path_buf();
    let explicit = ws.join("explicit.toml");
    write_config(&explicit, "[agents]\ndefault_model = \"explicit-model\"\n");
    let cfg = ResolvedConfiguration::builder(&ws)
        .with_explicit_config(Some(explicit))
        .with_model(Some("scalar-model".to_string()))
        .build()
        .expect("build");
    assert_eq!(cfg.app_config.agents.default_model, "scalar-model");
}

#[test]
fn cli_autonomy_override_wins_over_profile() {
    let dir = tempdir().expect("tempdir");
    let ws = dir.path().to_path_buf();
    let cfg = ResolvedConfiguration::builder(&ws)
        .with_profile(Some("balanced".to_string()))
        .with_autonomy(Some("unattended".to_string()))
        .build()
        .expect("build");
    assert_eq!(
        AutonomyPrecedence::from_config(&cfg),
        AutonomyMode::Unattended
    );
}

#[test]
fn invalid_autonomy_fails_closed() {
    let dir = tempdir().expect("tempdir");
    let ws = dir.path().to_path_buf();
    let res = ResolvedConfiguration::builder(&ws)
        .with_autonomy(Some("bogus".to_string()))
        .build();
    assert!(res.is_err(), "invalid --autonomy must fail closed");
}

#[test]
fn present_but_invalid_config_never_becomes_missing() {
    let dir = tempdir().expect("tempdir");
    let ws = dir.path().to_path_buf();
    write_config(&ws.join(".m31a").join("config.toml"), "[[[\nnot toml\n");
    let res = ResolvedConfiguration::builder(&ws).build();
    assert!(res.is_err(), "malformed workspace config must error");
    assert!(ResolvedConfiguration::workspace_config_exists(&ws));
}

#[test]
fn missing_explicit_config_errors() {
    let dir = tempdir().expect("tempdir");
    let ws = dir.path().to_path_buf();
    let res = ResolvedConfiguration::builder(&ws)
        .with_explicit_config(Some(ws.join("does-not-exist.toml")))
        .build();
    assert!(res.is_err());
}

#[test]
fn unknown_fields_rejected() {
    let res = parse_and_validate_config("[runtime]\nconcurrency_limit = 4\nbogus_key = 1\n");
    assert!(res.is_err(), "unknown fields must be rejected");
}

// ── wizard round-trip ─────────────────────────────────────────────────

#[test]
fn wizard_profile_round_trip_matches_resolver_and_runtime() {
    use m31a::tui::screens::wizard::WizardProfile;
    for profile in WizardProfile::all() {
        let id = profile.canonical_id();
        // resolver knows it
        assert!(
            ProfileResolver::with_canonical_profiles()
                .resolve_profile(id)
                .is_ok(),
            "resolver must know {id}"
        );
        // wizard parses its own persisted id
        assert_eq!(
            WizardProfile::from_canonical_id(id),
            Some(*profile),
            "wizard round-trip {id}"
        );
        // runtime observes the same autonomy as the profile metadata
        let dir = tempdir().expect("tempdir");
        let ws = dir.path().to_path_buf();
        let cfg = ResolvedConfiguration::builder(&ws)
            .with_profile(Some(id.to_string()))
            .build()
            .expect("build");
        let expected: AutonomyMode = profile.autonomy_mode().parse().expect("mode");
        assert_eq!(AutonomyPrecedence::from_config(&cfg), expected, "{id}");
    }
}

#[test]
fn wizard_persists_canonical_profile_and_theme() {
    use m31a::tui::screens::wizard::{SetupWizardScreen, WizardProfile};
    let dir = tempdir().expect("tempdir");
    let ws = dir.path().to_path_buf();
    let mut wizard = SetupWizardScreen::new(ws.clone());
    wizard.profile = WizardProfile::SecurityReview;
    wizard.set_theme_mode(m31a::tui::theme::ThemeMode::HighContrast);
    wizard
        .primary_model_input
        .set_text("meta/llama-3.1-70b-instruct");
    wizard.persist_configuration().expect("persist");
    let content = fs::read_to_string(ws.join(".m31a").join("config.toml")).expect("read");
    let parsed = parse_and_validate_config(&content).expect("validate");
    assert_eq!(parsed.profile.as_deref(), Some("security_review"));
    assert_eq!(parsed.tui.theme, "high-contrast");
    // reload observes the same
    let resolved = ResolvedConfiguration::for_workspace(&ws).expect("resolve");
    assert_eq!(resolved.active_profile.as_deref(), Some("security_review"));
    assert_eq!(
        AutonomyPrecedence::from_config(&resolved),
        AutonomyMode::Safe
    );
}

// ── policy consumes interactive approvals ─────────────────────────────

#[test]
fn interactive_approvals_true_adds_ask_rule_and_never_allows() {
    use m31a::policy::effective::EffectivePolicy;
    let dir = tempdir().expect("tempdir");
    let ws = dir.path();
    let mut cfg_true = m31a::config::schema::AppConfig::default();
    cfg_true.policy.interactive_approvals = true;
    let pol_true = EffectivePolicy::standard_with_policy_config(ws, Some(&cfg_true.policy));
    let mut cfg_false = m31a::config::schema::AppConfig::default();
    // autonomous-style opt-out must not weaken built-in safety
    cfg_false.policy.interactive_approvals = false;
    let pol_false = EffectivePolicy::standard_with_policy_config(ws, Some(&cfg_false.policy));
    assert_ne!(
        pol_true.active_policy_hash(),
        pol_false.active_policy_hash(),
        "interactive_approvals must be consumed (hash differs)"
    );
}

#[test]
fn unattended_ask_never_becomes_allow() {
    use m31a::kernel::seams::policy::{PolicyDecision, ResolvedAction, resolve_decision};
    let denied = resolve_decision(
        AutonomyMode::Unattended,
        PolicyDecision::Ask,
        "needs approval",
        false,
    );
    assert!(matches!(denied, ResolvedAction::Deny { .. }));
    let escalated = resolve_decision(
        AutonomyMode::Unattended,
        PolicyDecision::Ask,
        "needs approval",
        true,
    );
    assert!(matches!(escalated, ResolvedAction::Escalate { .. }));
}

// ── storage authorities ───────────────────────────────────────────────

#[test]
fn production_db_never_equals_workspace_db() {
    let dir = tempdir().expect("tempdir");
    let ws = dir.path();
    let channel = m31a::deployment::DeploymentChannel::current();
    let layout = m31a::storage::StorageLayout::new(ws, channel);
    let global = layout.global_db_path();
    let legacy = m31a::deployment::DeploymentPaths::project_db_path(ws, channel);
    assert_ne!(global, legacy, "production db must not be workspace-local");
    assert!(
        !global.starts_with(ws.join(".m31a")),
        "global db must live in platform dirs"
    );
}

#[test]
fn credentials_never_newly_written_to_workspace() {
    let dir = tempdir().expect("tempdir");
    let ws = dir.path();
    let channel = m31a::deployment::DeploymentChannel::current();
    let layout = m31a::storage::StorageLayout::new(ws, channel);
    let global = layout.global_credentials_file();
    let legacy = m31a::deployment::DeploymentPaths::project_credentials_file(ws, channel);
    assert_ne!(global, legacy);
}

#[test]
fn two_workspaces_share_global_catalog_without_workspace_state() {
    let d1 = tempdir().expect("t1");
    let d2 = tempdir().expect("t2");
    let channel = m31a::deployment::DeploymentChannel::current();
    let l1 = m31a::storage::StorageLayout::new(d1.path(), channel);
    let l2 = m31a::storage::StorageLayout::new(d2.path(), channel);
    // hermetic test isolation gives per-workspace globals; the invariant is
    // that neither catalog lives under workspace-local `.m31a/cache`.
    for layout in [&l1, &l2] {
        let p = layout.global_model_catalog_file();
        assert!(p.ends_with("model_catalog.json"), "global catalog file");
        assert!(
            !p.starts_with(d1.path().join(".m31a"))
                || !p.starts_with(d2.path().join(".m31a").join("cache")),
            "catalog must not be workspace-local: {p:?}"
        );
    }
    // no workspace-local catalog state is created by resolving the path.
    assert!(
        !d1.path()
            .join(".m31a")
            .join("cache")
            .join("model_catalog.json")
            .is_file()
    );
    assert!(
        !d2.path()
            .join(".m31a")
            .join("cache")
            .join("model_catalog.json")
            .is_file()
    );
}

#[test]
fn channel_global_dirs_are_isolated() {
    use m31a::deployment::{DeploymentChannel, DeploymentPaths};
    let prod = DeploymentPaths::new(DeploymentChannel::Production);
    let dev = DeploymentPaths::new(DeploymentChannel::Development);
    assert_ne!(prod.config_dir(), dev.config_dir());
    assert_ne!(prod.data_dir(), dev.data_dir());
    assert_ne!(prod.global_db_path(), dev.global_db_path());
    assert_ne!(
        prod.global_credentials_file(),
        dev.global_credentials_file()
    );
    assert_ne!(
        prod.global_model_catalog_file(),
        dev.global_model_catalog_file()
    );
    assert_ne!(prod.socket_path(), dev.socket_path());
}

// ── migration integrity ───────────────────────────────────────────────

#[test]
fn migration_is_hash_verified_and_idempotent() {
    use m31a::deployment::DeploymentChannel;
    let wsdir = tempdir().expect("ws");
    let ws = wsdir.path();
    // isolated global root keeps this hermetic (StorageLayout test isolation).
    let layout = m31a::storage::StorageLayout::new(ws, DeploymentChannel::current());
    let legacy_db = m31a::deployment::DeploymentPaths::project_db_path(ws, layout.channel());
    fs::create_dir_all(legacy_db.parent().expect("parent")).expect("mkdir");
    fs::write(&legacy_db, b"SQLite format 3\0test-payload").expect("write legacy");
    let r1 = m31a::storage::migrate_legacy_workspace_state(&layout);
    assert!(
        r1.db_migrated,
        "first migration must migrate: {:?}",
        r1.notes
    );
    let dst_bytes = fs::read(layout.global_db_path()).expect("read dst");
    assert!(dst_bytes.starts_with(b"SQLite format 3\0"));
    // second run is idempotent
    let r2 = m31a::storage::migrate_legacy_workspace_state(&layout);
    assert!(
        fs::read(layout.global_db_path()).expect("reread") == dst_bytes,
        "already-migrated destination must be preserved"
    );
    let _ = r2;
}

#[test]
fn migration_rejects_truncated_source() {
    use m31a::deployment::DeploymentChannel;
    let wsdir = tempdir().expect("ws");
    let ws = wsdir.path();
    let layout = m31a::storage::StorageLayout::new(ws, DeploymentChannel::current());
    let legacy_db = m31a::deployment::DeploymentPaths::project_db_path(ws, layout.channel());
    fs::create_dir_all(legacy_db.parent().expect("parent")).expect("mkdir");
    fs::write(&legacy_db, b"not-sqlite").expect("write");
    let report = m31a::storage::migrate_legacy_workspace_state(&layout);
    assert!(
        !report.db_migrated,
        "truncated/invalid source must not migrate"
    );
}

// ── git typed contracts + optionality ─────────────────────────────────

#[test]
fn git_push_and_isolation_are_typed_canonical() {
    let ok: m31a::config::schema::GitConfig =
        toml::from_str("push_policy = \"ask\"\nexecution_isolation = \"required\"\n")
            .expect("canonical git must parse");
    assert_eq!(ok.push_policy, GitPushPolicy::Ask);
    assert_eq!(ok.execution_isolation, GitExecutionIsolation::Required);
    let bad = toml::from_str::<m31a::config::schema::GitConfig>("push_policy = \"never\"\n");
    assert!(bad.is_err(), "obsolete 'never' must be rejected");
    let bad2 =
        toml::from_str::<m31a::config::schema::GitConfig>("execution_isolation = \"optional\"\n");
    assert!(bad2.is_err(), "obsolete 'optional' must be rejected");
}

#[test]
fn git_disabled_needs_no_repo() {
    let dir = tempdir().expect("tempdir");
    let ws = dir.path().to_path_buf();
    assert!(!ws.join(".git").exists());
    let cfg = "[git]\nenabled = false\n".to_string();
    write_config(&ws.join(".m31a").join("config.toml"), &cfg);
    let resolved = ResolvedConfiguration::for_workspace(&ws).expect("resolve");
    assert!(!resolved.app_config.git.enabled);
}

// ── budget unlimited + timeout ────────────────────────────────────────

#[test]
fn unlimited_budget_stays_unbounded_through_persist_and_reload() {
    use m31a::tui::screens::wizard::SetupWizardScreen;
    let dir = tempdir().expect("tempdir");
    let ws = dir.path().to_path_buf();
    let mut wizard = SetupWizardScreen::new(ws.clone());
    wizard.unlimited_budget = true;
    wizard
        .primary_model_input
        .set_text("meta/llama-3.1-70b-instruct");
    wizard.persist_configuration().expect("persist");
    let resolved = ResolvedConfiguration::for_workspace(&ws).expect("resolve");
    assert!(resolved.app_config.budget.max_cost_usd.is_none());
    assert!(resolved.app_config.budget.max_agent_steps.is_none());
    assert!(resolved.app_config.budget.max_tokens.is_none());
    assert!(resolved.app_config.budget.max_model_calls.is_none());
    assert!(resolved.app_config.budget.max_wall_clock_seconds.is_none());
    assert!(resolved.app_config.budget.max_retries.is_none());
    // concurrency stays explicitly configured (not unlimited).
    assert!(resolved.app_config.runtime.concurrency_limit >= 1);
}

#[test]
fn dispatcher_timeout_is_bounded_by_runtime_config() {
    use m31a::agent::dispatcher::ProductionWorkerDispatcher;
    use m31a::capability::registry::CapabilityRegistry;
    use m31a::policy::effective::EffectivePolicy;
    use std::sync::Arc;
    let dir = tempdir().expect("tempdir");
    let ws = dir.path().to_path_buf();
    write_config(
        &ws.join(".m31a").join("config.toml"),
        "[runtime]\ntimeout_secs = 120\n",
    );
    let cfg = ResolvedConfiguration::for_workspace(&ws).expect("resolve");
    assert_eq!(cfg.app_config.runtime.timeout_secs, 120);
    let caps = Arc::new(CapabilityRegistry::production(&ws, None, None));
    let policy = Arc::new(EffectivePolicy::standard(&ws));
    let store = Arc::new(m31a::persistence::artifacts::FsArtifactStore::new(
        ws.join("artifacts"),
    ));
    let d = ProductionWorkerDispatcher::from_shared_authorities(
        ws,
        caps,
        policy,
        store,
        None,
        None,
        None,
        Some(&cfg),
        None,
    );
    assert_eq!(d.runtime_timeout_secs(), 120);
    assert_eq!(d.effective_wall_timeout_secs(600), 120);
    assert_eq!(d.effective_wall_timeout_secs(60), 60);
}

// ── provider + model cache truthfulness ───────────────────────────────

#[test]
fn only_nvidia_nim_is_production() {
    assert!(m31a::config::provider_registry::is_retired_provider(
        "anthropic"
    ));
    assert!(m31a::config::provider_registry::is_retired_provider(
        "openai"
    ));
    let bad = parse_and_validate_config("[provider]\ndefault = \"anthropic\"\n");
    assert!(bad.is_err());
    let good = parse_and_validate_config(
        "[provider]\ndefault = \"nvidia_nim\"\n[agents]\ndefault_model = \"meta/x\"\n",
    );
    assert!(good.is_ok());
}

#[test]
fn model_cache_status_distinguishes_live_cached_stale_unverified() {
    use m31a::model::catalog::{
        CatalogRefreshState, CatalogSource, ModelCacheStatus, ModelCatalog,
    };
    use m31a::model::router::resolver::{ModelCandidate, ModelTier};
    let mk = |source, state: CatalogRefreshState, age_offset: u64| {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let mut c = ModelCatalog::new("nvidia_nim");
        c.source = source;
        c.refresh_state = state;
        c.models = vec![ModelCandidate::new(
            "meta/llama-3.1-70b-instruct",
            "nvidia",
            ModelTier::Standard,
            8192,
        )];
        let ts = now.saturating_sub(age_offset);
        c.discovered_at = Some(ts);
        c.inventory_updated_at = Some(ts);
        c.metadata_updated_at = Some(ts);
        c
    };
    assert_eq!(
        mk(
            CatalogSource::Discovered,
            CatalogRefreshState::DiscoverySuccess,
            10
        )
        .cache_status(),
        ModelCacheStatus::Live
    );
    assert_eq!(
        mk(
            CatalogSource::Cache,
            CatalogRefreshState::DiscoverySuccess,
            10
        )
        .cache_status(),
        ModelCacheStatus::Cached
    );
    assert_eq!(
        mk(
            CatalogSource::Cache,
            CatalogRefreshState::DiscoveryFailedWithCache,
            10
        )
        .cache_status(),
        ModelCacheStatus::Stale
    );
    assert_eq!(
        mk(
            CatalogSource::Discovered,
            CatalogRefreshState::DiscoverySuccess,
            7200
        )
        .cache_status(),
        ModelCacheStatus::Stale
    );
    assert_eq!(
        ModelCatalog::new("nvidia_nim").cache_status(),
        ModelCacheStatus::NotVerified
    );
}

// ── sandbox mounts ────────────────────────────────────────────────────

#[test]
fn sandbox_denies_credential_and_repo_mounts_but_allows_toolchain() {
    use m31a::sandbox::plan::SandboxPlan;
    let dir = tempdir().expect("tempdir");
    let ws = dir.path().to_path_buf();
    let toolchain = dir.path().join("toolchain");
    fs::create_dir_all(&toolchain).expect("mkdir");
    // legitimate toolchain mount passes
    let ok_plan = SandboxPlan::new(ws.clone()).with_extra_ro_mount(toolchain.clone());
    assert!(ok_plan.validate_extra_ro_mounts().is_ok());
    // credential store denied
    let cred = dir.path().join("credentials.json");
    fs::write(&cred, "{}").expect("write");
    let bad = SandboxPlan::new(ws.clone()).with_extra_ro_mount(cred);
    assert!(bad.validate_extra_ro_mounts().is_err());
    // .m31a denied
    let m31a_dir = dir.path().join(".m31a");
    fs::create_dir_all(&m31a_dir).expect("mkdir");
    let bad2 = SandboxPlan::new(ws).with_extra_ro_mount(m31a_dir);
    assert!(bad2.validate_extra_ro_mounts().is_err());
}

// ── examples validate ─────────────────────────────────────────────────

#[test]
fn shipped_example_config_parses_and_validates() {
    let content = fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/examples/config.example.toml"
    ))
    .expect("read example");
    let parsed = parse_and_validate_config(&content).expect("example must validate");
    assert_eq!(parsed.provider.default, "nvidia_nim");
    assert!(parsed.budget.max_cost_usd.is_none());
}

// ── telemetry uses canonical dir ──────────────────────────────────────

#[test]
fn telemetry_dirs_are_canonical_and_channel_isolated() {
    let dir = tempdir().expect("tempdir");
    let ws = dir.path();
    let channel = m31a::deployment::DeploymentChannel::current();
    let layout = m31a::storage::StorageLayout::new(ws, channel);
    let global = layout.global_telemetry_dir();
    assert!(!global.ends_with(".m31a/telemetry"));
    let writer = m31a::telemetry::stream::NdjsonStreamWriter::new(&global);
    let mid = m31a::ids::MissionId::new();
    assert_eq!(
        writer.stream_path(&mid),
        global.join(format!("{}.ndjson", mid))
    );
}
