//! Comprehensive Acceptance Test Suite for Configuration Control Plane (CFG-01–CFG-04, CFX-04, Phase V).
//!
//! Enforces:
//! 1. 8-Tier resolution precedence (Tier 7 > Tier 6 > Tier 5 > Tier 4 > Tier 3 > Tier 2 > Tier 1 > Tier 0).
//! 2. Monotonic security constraint (Tier 0 invariants cannot be weakened by workspace/session).
//! 3. Credential masking (API keys and secrets are masked in explain/sources/provenance).
//! 4. Dynamic session mutation (`with_session_model`, `with_session_profile`, `with_session_override`).
//! 5. Verification project adapter auto-detection and custom override.
//! 6. CLI dispatcher configuration commands (Get, Set, Sources, Explain, Validate).

use std::path::PathBuf;
use std::sync::Arc;
use tempfile::TempDir;

use m31a::cli::dispatch::{CliDispatcher, RuntimeCommand};
use m31a::config::hierarchy::{ConfigPrecedenceEngine, ConfigTier};
use m31a::config::provenance::{ConfigLayer, ConfigurationService, ProvenanceError};
use m31a::config::resolved::{ResolvedConfigBuilder, is_secret_key, mask_value};
use m31a::config::schema::AppConfig;
use m31a::verification::adapter::{ProjectAdapter, ProjectType};

#[test]
fn test_8_tier_precedence_engine() {
    let mut engine = ConfigPrecedenceEngine::new();

    // Tier 0: Built-in defaults
    let t0 = toml::from_str::<toml::Value>(
        r#"
        [runtime]
        concurrency_limit = 1
        timeout_secs = 300
        [agents]
        default_model = "builtin-model"
        "#,
    )
    .unwrap();
    engine.set_layer(ConfigTier::Tier0SecurityInvariants, t0);

    // Tier 1: System
    let t1 = toml::from_str::<toml::Value>(
        r#"
        [runtime]
        concurrency_limit = 2
        "#,
    )
    .unwrap();
    engine.set_layer(ConfigTier::Tier1System, t1);

    // Tier 2: User
    let t2 = toml::from_str::<toml::Value>(
        r#"
        [runtime]
        concurrency_limit = 3
        "#,
    )
    .unwrap();
    engine.set_layer(ConfigTier::Tier2User, t2);

    // Tier 3: Workspace
    let t3 = toml::from_str::<toml::Value>(
        r#"
        [runtime]
        concurrency_limit = 4
        "#,
    )
    .unwrap();
    engine.set_layer(ConfigTier::Tier3Workspace, t3);

    // Tier 4: Profile
    let t4 = toml::from_str::<toml::Value>(
        r#"
        [runtime]
        concurrency_limit = 5
        "#,
    )
    .unwrap();
    engine.set_layer(ConfigTier::Tier4Profile, t4);

    // Tier 5: Environment
    let t5 = toml::from_str::<toml::Value>(
        r#"
        [runtime]
        concurrency_limit = 6
        "#,
    )
    .unwrap();
    engine.set_layer(ConfigTier::Tier5Environment, t5);

    // Tier 6: CLI
    let t6 = toml::from_str::<toml::Value>(
        r#"
        [runtime]
        concurrency_limit = 7
        "#,
    )
    .unwrap();
    engine.set_layer(ConfigTier::Tier6Cli, t6);

    // Tier 7: Session
    let t7 = toml::from_str::<toml::Value>(
        r#"
        [runtime]
        concurrency_limit = 8
        "#,
    )
    .unwrap();
    engine.set_layer(ConfigTier::Tier7Session, t7);

    // At this point, Tier 7 should win: concurrency_limit = 8
    let app_cfg: AppConfig = engine.resolve().unwrap();
    assert_eq!(app_cfg.runtime.concurrency_limit, 8);
    assert_eq!(app_cfg.runtime.timeout_secs, 300); // from Tier 0
    assert_eq!(app_cfg.agents.default_model, "builtin-model"); // from Tier 0

    // Remove Tier 7: Tier 6 should win: concurrency_limit = 7
    engine.remove_layer(ConfigTier::Tier7Session);
    let app_cfg: AppConfig = engine.resolve().unwrap();
    assert_eq!(app_cfg.runtime.concurrency_limit, 7);

    // Remove Tier 6: Tier 5 should win: concurrency_limit = 6
    engine.remove_layer(ConfigTier::Tier6Cli);
    let app_cfg: AppConfig = engine.resolve().unwrap();
    assert_eq!(app_cfg.runtime.concurrency_limit, 6);

    // Remove Tier 5: Tier 4 should win: concurrency_limit = 5
    engine.remove_layer(ConfigTier::Tier5Environment);
    let app_cfg: AppConfig = engine.resolve().unwrap();
    assert_eq!(app_cfg.runtime.concurrency_limit, 5);

    // Remove Tier 4: Tier 3 should win: concurrency_limit = 4
    engine.remove_layer(ConfigTier::Tier4Profile);
    let app_cfg: AppConfig = engine.resolve().unwrap();
    assert_eq!(app_cfg.runtime.concurrency_limit, 4);

    // Remove Tier 3: Tier 2 should win: concurrency_limit = 3
    engine.remove_layer(ConfigTier::Tier3Workspace);
    let app_cfg: AppConfig = engine.resolve().unwrap();
    assert_eq!(app_cfg.runtime.concurrency_limit, 3);

    // Remove Tier 2: Tier 1 should win: concurrency_limit = 2
    engine.remove_layer(ConfigTier::Tier2User);
    let app_cfg: AppConfig = engine.resolve().unwrap();
    assert_eq!(app_cfg.runtime.concurrency_limit, 2);

    // Remove Tier 1: Tier 0 should win: concurrency_limit = 1
    engine.remove_layer(ConfigTier::Tier1System);
    let app_cfg: AppConfig = engine.resolve().unwrap();
    assert_eq!(app_cfg.runtime.concurrency_limit, 1);
}

#[test]
fn test_all_8_representative_settings_precedence_matrix() {
    let mut engine = ConfigPrecedenceEngine::new();

    // Configure all 8 tiers for all 8 representative settings
    // 1. agents.default_model
    // 2. runtime.concurrency_limit
    // 3. budget.max_agent_steps
    // 4. runtime.timeout_secs
    // 5. tui.theme
    // 6. git.auto_commit
    // 7. workspace.verification.test_command
    // 8. policy.denied_tools (accumulates monotonically)

    let tier_configs = [
        (
            ConfigTier::Tier0SecurityInvariants,
            r#"
            [agents]
            default_model = "model-tier0"
            [runtime]
            concurrency_limit = 1
            timeout_secs = 100
            [budget]
            max_agent_steps = 10
            [tui]
            theme = "theme-tier0"
            [git]
            auto_commit = false
            [workspace.verification]
            test_command = "test-tier0"
            [policy]
            denied_tools = ["tool-0"]
        "#,
        ),
        (
            ConfigTier::Tier1System,
            r#"
            [agents]
            default_model = "model-tier1"
            [runtime]
            concurrency_limit = 2
            timeout_secs = 200
            [budget]
            max_agent_steps = 20
            [tui]
            theme = "theme-tier1"
            [git]
            auto_commit = true
            [workspace.verification]
            test_command = "test-tier1"
            [policy]
            denied_tools = ["tool-0", "tool-1"]
        "#,
        ),
        (
            ConfigTier::Tier2User,
            r#"
            [agents]
            default_model = "model-tier2"
            [runtime]
            concurrency_limit = 3
            timeout_secs = 300
            [budget]
            max_agent_steps = 30
            [tui]
            theme = "theme-tier2"
            [git]
            auto_commit = false
            [workspace.verification]
            test_command = "test-tier2"
            [policy]
            denied_tools = ["tool-0", "tool-1", "tool-2"]
        "#,
        ),
        (
            ConfigTier::Tier3Workspace,
            r#"
            [agents]
            default_model = "model-tier3"
            [runtime]
            concurrency_limit = 4
            timeout_secs = 400
            [budget]
            max_agent_steps = 40
            [tui]
            theme = "theme-tier3"
            [git]
            auto_commit = true
            [workspace.verification]
            test_command = "test-tier3"
            [policy]
            denied_tools = ["tool-0", "tool-1", "tool-2", "tool-3"]
        "#,
        ),
        (
            ConfigTier::Tier4Profile,
            r#"
            [agents]
            default_model = "model-tier4"
            [runtime]
            concurrency_limit = 5
            timeout_secs = 500
            [budget]
            max_agent_steps = 50
            [tui]
            theme = "theme-tier4"
            [git]
            auto_commit = false
            [workspace.verification]
            test_command = "test-tier4"
            [policy]
            denied_tools = ["tool-0", "tool-1", "tool-2", "tool-3", "tool-4"]
        "#,
        ),
        (
            ConfigTier::Tier5Environment,
            r#"
            [agents]
            default_model = "model-tier5"
            [runtime]
            concurrency_limit = 6
            timeout_secs = 600
            [budget]
            max_agent_steps = 60
            [tui]
            theme = "theme-tier5"
            [git]
            auto_commit = true
            [workspace.verification]
            test_command = "test-tier5"
            [policy]
            denied_tools = ["tool-0", "tool-1", "tool-2", "tool-3", "tool-4", "tool-5"]
        "#,
        ),
        (
            ConfigTier::Tier6Cli,
            r#"
            [agents]
            default_model = "model-tier6"
            [runtime]
            concurrency_limit = 7
            timeout_secs = 700
            [budget]
            max_agent_steps = 70
            [tui]
            theme = "theme-tier6"
            [git]
            auto_commit = false
            [workspace.verification]
            test_command = "test-tier6"
            [policy]
            denied_tools = ["tool-0", "tool-1", "tool-2", "tool-3", "tool-4", "tool-5", "tool-6"]
        "#,
        ),
        (
            ConfigTier::Tier7Session,
            r#"
            [agents]
            default_model = "model-tier7"
            [runtime]
            concurrency_limit = 8
            timeout_secs = 800
            [budget]
            max_agent_steps = 80
            [tui]
            theme = "theme-tier7"
            [git]
            auto_commit = true
            [workspace.verification]
            test_command = "test-tier7"
            [policy]
            denied_tools = ["tool-0", "tool-1", "tool-2", "tool-3", "tool-4", "tool-5", "tool-6", "tool-7"]
        "#,
        ),
    ];

    for (tier, toml_str) in &tier_configs {
        engine.set_layer_from_str(*tier, toml_str).unwrap();
    }

    // Expected values at each tier from 7 down to 0 (8 tiers * 8 settings = 64 test points)
    let tiers_desc = [
        (
            ConfigTier::Tier7Session,
            "model-tier7",
            8,
            800,
            Some(80),
            "theme-tier7",
            true,
            "test-tier7",
            8,
        ),
        (
            ConfigTier::Tier6Cli,
            "model-tier6",
            7,
            700,
            Some(70),
            "theme-tier6",
            false,
            "test-tier6",
            7,
        ),
        (
            ConfigTier::Tier5Environment,
            "model-tier5",
            6,
            600,
            Some(60),
            "theme-tier5",
            true,
            "test-tier5",
            6,
        ),
        (
            ConfigTier::Tier4Profile,
            "model-tier4",
            5,
            500,
            Some(50),
            "theme-tier4",
            false,
            "test-tier4",
            5,
        ),
        (
            ConfigTier::Tier3Workspace,
            "model-tier3",
            4,
            400,
            Some(40),
            "theme-tier3",
            true,
            "test-tier3",
            4,
        ),
        (
            ConfigTier::Tier2User,
            "model-tier2",
            3,
            300,
            Some(30),
            "theme-tier2",
            false,
            "test-tier2",
            3,
        ),
        (
            ConfigTier::Tier1System,
            "model-tier1",
            2,
            200,
            Some(20),
            "theme-tier1",
            true,
            "test-tier1",
            2,
        ),
        (
            ConfigTier::Tier0SecurityInvariants,
            "model-tier0",
            1,
            100,
            Some(10),
            "theme-tier0",
            false,
            "test-tier0",
            1,
        ),
    ];

    for (
        tier_to_check,
        exp_model,
        exp_conc,
        exp_timeout,
        exp_steps,
        exp_theme,
        exp_commit,
        exp_test_cmd,
        exp_denied_count,
    ) in tiers_desc
    {
        let app_cfg: AppConfig = engine.resolve().unwrap();
        // 1. agents.default_model
        assert_eq!(
            app_cfg.agents.default_model, exp_model,
            "Failed default_model at {:?}",
            tier_to_check
        );
        // 2. runtime.concurrency_limit
        assert_eq!(
            app_cfg.runtime.concurrency_limit, exp_conc,
            "Failed concurrency_limit at {:?}",
            tier_to_check
        );
        // 3. runtime.timeout_secs
        assert_eq!(
            app_cfg.runtime.timeout_secs, exp_timeout,
            "Failed timeout_secs at {:?}",
            tier_to_check
        );
        // 4. budget.max_agent_steps
        assert_eq!(
            app_cfg.budget.max_agent_steps, exp_steps,
            "Failed max_agent_steps at {:?}",
            tier_to_check
        );
        // 5. tui.theme
        assert_eq!(
            app_cfg.tui.theme, exp_theme,
            "Failed theme at {:?}",
            tier_to_check
        );
        // 6. git.auto_commit
        assert_eq!(
            app_cfg.git.auto_commit, exp_commit,
            "Failed auto_commit at {:?}",
            tier_to_check
        );
        // 7. workspace.verification.test_command
        assert_eq!(
            app_cfg
                .workspace
                .verification
                .as_ref()
                .and_then(|v| v.test_command()),
            Some(exp_test_cmd),
            "Failed test_command at {:?}",
            tier_to_check
        );
        // 8. policy.denied_tools
        assert_eq!(
            app_cfg.policy.denied_tools.len(),
            exp_denied_count,
            "Failed denied_tools count at {:?}",
            tier_to_check
        );

        // Remove the winning tier for the next iteration
        engine.remove_layer(tier_to_check);
    }
}

#[test]
fn test_concurrency_ceiling_enforcement() {
    let mut cfg = AppConfig::default();
    cfg.runtime.concurrency_limit = 0;
    assert!(
        m31a::config::schema::validate_config(&cfg).is_err(),
        "Concurrency limit 0 must be rejected"
    );

    cfg.runtime.concurrency_limit = 32;
    assert!(
        m31a::config::schema::validate_config(&cfg).is_ok(),
        "Concurrency limit 32 must be valid"
    );

    cfg.runtime.concurrency_limit = 33;
    assert!(
        m31a::config::schema::validate_config(&cfg).is_err(),
        "Concurrency limit > 32 must be rejected"
    );
}

#[test]
fn test_monotonic_security_approval_and_sandbox_invariants() {
    // 1. interactive_approvals cannot be disabled
    let base = toml::from_str::<toml::Value>(
        r#"
        [policy]
        interactive_approvals = true
    "#,
    )
    .unwrap();
    let overlay_bad = toml::from_str::<toml::Value>(
        r#"
        [policy]
        interactive_approvals = false
    "#,
    )
    .unwrap();
    assert!(m31a::config::merge::MonotonicSecurityMerger::check(&base, &overlay_bad).is_err());

    // 2. require_approval_for_destructive alias cannot be disabled
    let overlay_bad_alias = toml::from_str::<toml::Value>(
        r#"
        [policy]
        require_approval_for_destructive = false
    "#,
    )
    .unwrap();
    assert!(
        m31a::config::merge::MonotonicSecurityMerger::check(&base, &overlay_bad_alias).is_err()
    );

    // 3. sandbox_mode cannot be relaxed from strict to standard/permissive/disabled/none
    let base_strict = toml::from_str::<toml::Value>(
        r#"
        [policy]
        sandbox_mode = "strict"
    "#,
    )
    .unwrap();
    let overlay_none = toml::from_str::<toml::Value>(
        r#"
        [policy]
        sandbox_mode = "none"
    "#,
    )
    .unwrap();
    assert!(
        m31a::config::merge::MonotonicSecurityMerger::check(&base_strict, &overlay_none).is_err()
    );

    // 4. denied_tools cannot drop a denied tool
    let base_tools = toml::from_str::<toml::Value>(
        r#"
        [policy]
        denied_tools = ["tool_a", "tool_b"]
    "#,
    )
    .unwrap();
    let overlay_tools_bad = toml::from_str::<toml::Value>(
        r#"
        [policy]
        denied_tools = ["tool_a"]
    "#,
    )
    .unwrap();
    assert!(
        m31a::config::merge::MonotonicSecurityMerger::check(&base_tools, &overlay_tools_bad)
            .is_err()
    );
}

#[test]
fn test_monotonic_security_constraint_protection() {
    let mut svc = ConfigurationService::new();

    // Register immutable constraint at Tier 0
    svc.set_value(
        ConfigLayer::Tier0SecurityInvariants,
        "security.non_bypassable_policy",
        serde_json::json!(true),
        None,
        true, // is_immutable
    )
    .expect("Registering invariant must succeed");

    // Attempt to weaken constraint at Tier 3 (Workspace)
    let res = svc.set_value(
        ConfigLayer::Tier3Workspace,
        "security.non_bypassable_policy",
        serde_json::json!(false),
        Some(PathBuf::from(".m31a/config.toml")),
        false,
    );

    assert!(
        res.is_err(),
        "Workspace layer must not weaken Tier 0 invariant"
    );
    match res.unwrap_err() {
        ProvenanceError::ImmutableConstraintViolation {
            key,
            attempted_layer,
            ..
        } => {
            assert_eq!(key, "security.non_bypassable_policy");
            assert_eq!(attempted_layer, ConfigLayer::Tier3Workspace);
        }
    }

    // Attempt to weaken constraint at Tier 7 (Session)
    let res_session = svc.set_value(
        ConfigLayer::Tier7Session,
        "security.non_bypassable_policy",
        serde_json::json!(false),
        None,
        false,
    );
    assert!(
        res_session.is_err(),
        "Session layer must not weaken Tier 0 invariant"
    );
}

#[test]
fn test_credential_masking() {
    assert!(is_secret_key("provider.nvidia.api_key"));
    assert!(is_secret_key("NVIDIA_API_KEY"));
    assert!(is_secret_key("anthropic.token"));
    assert!(is_secret_key("aws.secret_access_key"));
    assert!(!is_secret_key("agents.default_model"));
    assert!(!is_secret_key("runtime.concurrency_limit"));

    let masked = mask_value(&serde_json::json!("nvapi-1234567890abcdef"));
    assert_eq!(masked, serde_json::json!("nvap...[MASKED]"));

    let short_masked = mask_value(&serde_json::json!("short"));
    assert_eq!(short_masked, serde_json::json!("********"));

    let non_string = mask_value(&serde_json::json!(42));
    assert_eq!(non_string, serde_json::json!(42));
}

#[test]
fn test_dynamic_session_mutation() {
    let temp = TempDir::new().unwrap();
    let ws = temp.path();

    let config = ResolvedConfigBuilder::new(ws).build().unwrap();
    assert_eq!(config.active_model, "meta/llama-3.2-11b-vision-instruct");

    // Mutate model
    let updated_model = config
        .with_session_model("meta/llama-3.3-70b-instruct")
        .unwrap();
    assert_eq!(updated_model.active_model, "meta/llama-3.3-70b-instruct");
    assert_eq!(
        updated_model.app_config.agents.default_model,
        "meta/llama-3.3-70b-instruct"
    );

    // Explain model to verify Tier 7 Session won
    let explain = updated_model.explain("agents.default_model").unwrap();
    assert_eq!(explain.winning_layer, ConfigLayer::Tier7Session);
    assert_eq!(
        explain.resolved_value,
        serde_json::json!("meta/llama-3.3-70b-instruct")
    );

    // Mutate profile
    let updated_profile = updated_model.with_session_profile("safe").unwrap();
    assert_eq!(updated_profile.active_profile, Some("safe".to_string()));

    // Mutate arbitrary config override
    let updated_override = updated_profile
        .with_session_override("runtime.concurrency_limit", serde_json::json!(8))
        .unwrap();
    assert_eq!(updated_override.app_config.runtime.concurrency_limit, 8);

    // Verify original config was not mutated (immutability)
    assert_eq!(config.active_model, "meta/llama-3.2-11b-vision-instruct");
    assert_eq!(config.active_profile, None);
}

#[test]
fn test_verification_project_adapter() {
    let temp = TempDir::new().unwrap();
    let ws = temp.path();

    // 1. Rust project auto-detection
    std::fs::write(
        ws.join("Cargo.toml"),
        "[package]\nname = \"test\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    let adapter = ProjectAdapter::detect(ws, None, None);
    assert_eq!(adapter.project_type, ProjectType::Rust);
    assert_eq!(adapter.manifest_file, "Cargo.toml");
    assert_eq!(adapter.source_dir, Some("src".to_string()));

    // 2. Override with custom verification config
    let custom_cfg = m31a::config::schema::WorkspaceVerificationConfig {
        tier1_manifest: Some("pyproject.toml".to_string()),
        tier3_tests: Some("pytest tests/".to_string()),
        ..Default::default()
    };

    let adapter_custom = ProjectAdapter::detect(ws, Some(&custom_cfg), Some("python"));
    assert_eq!(adapter_custom.project_type, ProjectType::Python);
    assert_eq!(adapter_custom.manifest_file, "pyproject.toml");
    assert_eq!(adapter_custom.test_command, "pytest tests/");
}

#[tokio::test]
async fn test_cli_dispatch_config_commands() {
    let temp = TempDir::new().unwrap();
    let ws = temp.path().to_path_buf();

    // Write workspace config
    let m31a_dir = ws.join(".m31a");
    std::fs::create_dir_all(&m31a_dir).unwrap();
    std::fs::write(
        m31a_dir.join("config.toml"),
        r#"
        [agents]
        default_model = "test-model-42"
        [runtime]
        concurrency_limit = 5
        "#,
    )
    .unwrap();

    let resolved = Arc::new(ResolvedConfigBuilder::new(&ws).build().unwrap());
    let dispatcher = CliDispatcher::new()
        .with_workspace_root(ws.clone())
        .with_config(resolved);

    // 1. ConfigGet
    let get_out = dispatcher
        .dispatch(RuntimeCommand::ConfigGet {
            key: "agents.default_model".to_string(),
        })
        .await
        .unwrap();
    assert!(get_out.text.contains("test-model-42"));

    // 2. ConfigSources
    let sources_out = dispatcher
        .dispatch(RuntimeCommand::ConfigSources)
        .await
        .unwrap();
    assert!(sources_out.text.contains("Tier 0: Built-in Safe Defaults"));
    assert!(sources_out.text.contains("Tier3Workspace"));

    // 3. ConfigExplain
    let explain_out = dispatcher
        .dispatch(RuntimeCommand::ConfigExplain {
            key: "agents.default_model".to_string(),
        })
        .await
        .unwrap();
    assert!(explain_out.text.contains("Key: agents.default_model"));
    assert!(explain_out.text.contains("test-model-42"));

    // 4. ValidateConfig
    let validate_out = dispatcher
        .dispatch(RuntimeCommand::ValidateConfig { path: None })
        .await
        .unwrap();
    assert!(validate_out.text.contains("Configuration schema valid"));

    // 5. ConfigSet
    let set_out = dispatcher
        .dispatch(RuntimeCommand::ConfigSet {
            key: "runtime.timeout_secs".to_string(),
            value: "60".to_string(),
        })
        .await
        .unwrap();
    assert!(
        set_out
            .text
            .contains("Configuration updated: runtime.timeout_secs = 60")
    );
}
