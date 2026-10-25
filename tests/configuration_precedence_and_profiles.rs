//! Phase 11 Configuration Integration Tests (CFG-01..04).

use m31a::config::{ConfigError, ConfigPrecedenceEngine, ConfigTier};

#[test]
fn test_hierarchical_precedence_merge() {
    let mut engine = ConfigPrecedenceEngine::new();

    // 1. BuiltinDefaults (Tier 1)
    let builtin = serde_json::json!({
        "runtime": {
            "concurrency_limit": 2,
            "timeout_secs": 30,
            "sandbox_mode": "standard"
        },
        "policy": {
            "default_action": "ask",
            "interactive_approvals": true,
            "denied_tools": ["rm_rf"]
        },
        "tui": {
            "theme": "default",
            "fps": 30
        }
    });
    engine.set_layer_json(ConfigTier::BuiltinDefaults, builtin);

    // 2. System (Tier 2) - overrides timeout to 60, increases concurrency to 4
    let system = serde_json::json!({
        "runtime": {
            "concurrency_limit": 4,
            "timeout_secs": 60
        }
    });
    engine.set_layer_json(ConfigTier::System, system);

    // 3. User (Tier 3) - user customizes theme and adds "curl" to denied tools
    let user = serde_json::json!({
        "policy": {
            "denied_tools": ["rm_rf", "curl"]
        },
        "tui": {
            "theme": "catppuccin"
        }
    });
    engine.set_layer_json(ConfigTier::User, user);

    // 4. Workspace (Tier 4) - workspace overrides timeout to 120
    let workspace = serde_json::json!({
        "runtime": {
            "timeout_secs": 120
        },
        "policy": {
            "denied_tools": ["rm_rf", "curl", "raw_exec"]
        }
    });
    engine.set_layer_json(ConfigTier::Workspace, workspace);

    // 5. CliOverrides (Tier 7) - command-line flag --concurrency 8
    let cli = serde_json::json!({
        "runtime": {
            "concurrency_limit": 8
        }
    });
    engine.set_layer_json(ConfigTier::CliOverrides, cli);

    // Resolve merged configuration
    let merged = engine.resolve_json().expect("resolution should succeed");

    // Validate 7-tier precedence resolutions
    // - runtime.timeout_secs should be 120 (workspace overrode system and builtin)
    assert_eq!(merged["runtime"]["timeout_secs"].as_i64().unwrap(), 120);
    // - runtime.concurrency_limit should be 8 (CLI overrode all lower tiers)
    assert_eq!(merged["runtime"]["concurrency_limit"].as_i64().unwrap(), 8);
    // - runtime.sandbox_mode should remain "standard" from builtin defaults
    assert_eq!(
        merged["runtime"]["sandbox_mode"].as_str().unwrap(),
        "standard"
    );
    // - tui.theme should be "catppuccin" from user tier
    assert_eq!(merged["tui"]["theme"].as_str().unwrap(), "catppuccin");
    // - tui.fps should be 30 from builtin
    assert_eq!(merged["tui"]["fps"].as_i64().unwrap(), 30);
    // - policy.interactive_approvals should be true
    assert!(merged["policy"]["interactive_approvals"].as_bool().unwrap());
    // - policy.denied_tools should contain "rm_rf", "curl", "raw_exec"
    let denied = merged["policy"]["denied_tools"].as_array().unwrap();
    let denied_strs: Vec<&str> = denied.iter().filter_map(|v| v.as_str()).collect();
    assert_eq!(denied_strs, vec!["rm_rf", "curl", "raw_exec"]);

    // 6. Test Monotonic Security Enforcement:
    // A lower-tier config (workspace) attempting to relax interactive_approvals must fail
    let mut bad_engine = engine.clone();
    let bad_workspace = serde_json::json!({
        "policy": {
            "interactive_approvals": false
        }
    });
    bad_engine.set_layer_json(ConfigTier::Workspace, bad_workspace);
    let err = bad_engine.resolve_json().unwrap_err();
    assert!(
        matches!(err, ConfigError::SecurityDowngradeDenied { ref reason } if reason.contains("interactive_approvals"))
    );

    // A lower-tier config attempting to un-deny a tool must fail
    let mut bad_engine_tools = engine.clone();
    let bad_workspace_tools = serde_json::json!({
        "policy": {
            "denied_tools": ["only_one_tool"]
        }
    });
    bad_engine_tools.set_layer_json(ConfigTier::Workspace, bad_workspace_tools);
    let err_tools = bad_engine_tools.resolve_json().unwrap_err();
    assert!(
        matches!(err_tools, ConfigError::SecurityDowngradeDenied { ref reason } if reason.contains("denied_tools"))
    );
}

#[test]
fn test_schema_validation_deny_unknown_fields() {
    use m31a::config::{ConfigValidationError, parse_and_validate_config};

    // 1. Valid config with namespaced plugins succeeds
    let valid_md = r#"---
kind: config
schema_version: 1
data:
  runtime:
    concurrency_limit: 6
    timeout_secs: 600
    sandbox_mode: strict
  git:
    auto_commit: true
    push_policy: ask
  policy:
    default_action: ask
    interactive_approvals: true
    denied_tools:
      - rm_rf
  agents:
    default_model: gpt-4o
    max_tokens: 4096
  tui:
    fps: 60
    theme: nord
    compact_mode: true
  plugins:
    custom_grep:
      regex_mode: pcre2
      max_depth: 5
---
"#;
    let config = parse_and_validate_config(valid_md).expect("valid config should parse");
    assert_eq!(config.runtime.concurrency_limit, 6);
    assert_eq!(config.runtime.sandbox_mode, "strict");
    assert_eq!(config.agents.default_model, "gpt-4o");
    assert_eq!(config.tui.fps, 60);
    assert!(config.plugins.contains_key("custom_grep"));
    assert_eq!(
        config.plugins["custom_grep"]["regex_mode"]
            .as_str()
            .unwrap(),
        "pcre2"
    );

    // 2. Misspelled field in core struct must be rejected by deny_unknown_fields
    let typo_md = r#"---
kind: config
schema_version: 1
data:
  runtime:
    concurency_limit: 4
---
"#;
    let err = parse_and_validate_config(typo_md).unwrap_err();
    assert!(
        matches!(err, ConfigValidationError::UnknownField { ref message, .. } if message.contains("unknown field `concurency_limit`"))
    );

    // 3. Unknown top-level field must be rejected
    let unknown_top_md = r#"---
kind: config
schema_version: 1
data:
  unknown_top_key: boom
---
"#;
    let err_top = parse_and_validate_config(unknown_top_md).unwrap_err();
    assert!(
        matches!(err_top, ConfigValidationError::UnknownField { ref message, .. } if message.contains("unknown field `unknown_top_key`"))
    );

    // 4. Invalid range fails validate_config
    let invalid_range_md = r#"---
kind: config
schema_version: 1
data:
  runtime:
    concurrency_limit: 0
---
"#;
    let err_range = parse_and_validate_config(invalid_range_md).unwrap_err();
    assert!(
        matches!(err_range, ConfigValidationError::InvalidField { ref field, .. } if field == "runtime.concurrency_limit")
    );

    // 5. Invalid sandbox enum value fails validate_config
    let invalid_sandbox_md = r#"---
kind: config
schema_version: 1
data:
  runtime:
    sandbox_mode: completely_open
---
"#;
    let err_sb = parse_and_validate_config(invalid_sandbox_md).unwrap_err();
    assert!(
        matches!(err_sb, ConfigValidationError::InvalidField { ref field, .. } if field == "runtime.sandbox_mode")
    );
}

#[test]
fn test_platform_aware_paths() {
    use m31a::config::PlatformPaths;
    use std::path::Path;

    let paths = PlatformPaths::new();

    // 1. Standard paths resolve to valid non-empty locations
    let config_dir = paths.config_dir();
    let data_dir = paths.data_dir();
    let cache_dir = paths.cache_dir();
    let state_dir = paths.state_dir();
    let system_config_dir = paths.system_config_dir();

    assert!(!config_dir.as_os_str().is_empty());
    assert!(!data_dir.as_os_str().is_empty());
    assert!(!cache_dir.as_os_str().is_empty());
    assert!(!state_dir.as_os_str().is_empty());
    assert!(!system_config_dir.as_os_str().is_empty());

    // User and system config files end in config.md (canonical)
    assert!(paths.user_config_file().ends_with("config.md"));
    assert!(paths.system_config_file().ends_with("config.md"));

    // 2. Environment variable overrides (M31A_CONFIG_DIR, M31A_DATA_DIR)
    unsafe {
        std::env::set_var("M31A_CONFIG_DIR", "/tmp/custom_m31a_config");
        std::env::set_var("M31A_DATA_DIR", "/tmp/custom_m31a_data");
    }

    let overridden_paths = PlatformPaths::new();
    assert_eq!(
        overridden_paths.config_dir(),
        Path::new("/tmp/custom_m31a_config")
    );
    assert_eq!(
        overridden_paths.data_dir(),
        Path::new("/tmp/custom_m31a_data")
    );

    unsafe {
        std::env::remove_var("M31A_CONFIG_DIR");
        std::env::remove_var("M31A_DATA_DIR");
    }

    // 3. Workspace and mission config path helpers
    let ws_path = PlatformPaths::workspace_config_file(Path::new("/test/repo"));
    assert_eq!(ws_path, Path::new("/test/repo/.m31a/config.md"));

    let mission_path = PlatformPaths::mission_config_file(Path::new("/test/worktree"));
    assert_eq!(mission_path, Path::new("/test/worktree/.m31a/mission.md"));
}

#[test]
fn test_composable_profile_safety_boundaries() {
    use m31a::config::{ConfigError, ProfileResolver};

    let mut resolver = ProfileResolver::new();

    // 1. Base profile
    let base_json = serde_json::json!({
        "runtime": {
            "concurrency_limit": 4,
            "timeout_secs": 300,
            "sandbox_mode": "strict"
        },
        "policy": {
            "interactive_approvals": true,
            "default_action": "ask",
            "denied_tools": ["rm_rf"]
        }
    });
    resolver.register_profile("base", base_json);

    // 2. Child profile extending base
    let dev_json = serde_json::json!({
        "extends": "base",
        "runtime": {
            "timeout_secs": 600
        },
        "policy": {
            "denied_tools": ["rm_rf", "raw_exec"]
        }
    });
    resolver.register_profile("dev", dev_json);

    // Resolve child profile
    let resolved = resolver
        .resolve_profile("dev")
        .expect("dev profile resolution should succeed");
    assert_eq!(
        resolved["runtime"]["concurrency_limit"].as_i64().unwrap(),
        4
    );
    assert_eq!(resolved["runtime"]["timeout_secs"].as_i64().unwrap(), 600);
    assert_eq!(
        resolved["runtime"]["sandbox_mode"].as_str().unwrap(),
        "strict"
    );
    assert!(
        resolved["policy"]["interactive_approvals"]
            .as_bool()
            .unwrap()
    );
    let denied = resolved["policy"]["denied_tools"].as_array().unwrap();
    let denied_strs: Vec<&str> = denied.iter().filter_map(|v| v.as_str()).collect();
    assert_eq!(denied_strs, vec!["rm_rf", "raw_exec"]);

    // 3. Safety violation: Child profile trying to disable interactive_approvals fails
    let unsafe_json = serde_json::json!({
        "extends": "base",
        "policy": {
            "interactive_approvals": false
        }
    });
    resolver.register_profile("unsafe_profile", unsafe_json);
    let err = resolver.resolve_profile("unsafe_profile").unwrap_err();
    assert!(
        matches!(err, ConfigError::SecurityDowngradeDenied { ref reason } if reason.contains("interactive_approvals"))
    );

    // 4. Cyclic inheritance detection
    resolver.register_profile("cycle_a", serde_json::json!({ "extends": "cycle_b" }));
    resolver.register_profile("cycle_b", serde_json::json!({ "extends": "cycle_a" }));
    let cycle_err = resolver.resolve_profile("cycle_a").unwrap_err();
    assert!(
        matches!(cycle_err, ConfigError::CyclicProfileInheritance(ref name) if name == "cycle_a")
    );

    // 5. Native JSON/Markdown Value profile resolution
    let md_json = serde_json::json!({
        "extends": "base",
        "runtime": {
            "timeout_secs": 900
        }
    });
    resolver.register_profile("md_profile", md_json);
    let resolved_md = resolver
        .resolve_profile("md_profile")
        .expect("md profile resolves");
    assert_eq!(
        resolved_md["runtime"]["timeout_secs"].as_i64().unwrap(),
        900
    );
    assert_eq!(
        resolved_md["runtime"]["concurrency_limit"]
            .as_i64()
            .unwrap(),
        4
    );
}
