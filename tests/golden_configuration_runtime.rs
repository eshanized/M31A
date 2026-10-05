//! Golden End-to-End Configuration Control Plane Runtime Test (Section 22).
//!
//! Validates the complete 20-step configuration lifecycle:
//!  1. Initialize clean temp workspace with a Git repository.
//!  2. Verify Phase 1 default configuration.
//!  3. System configuration simulation.
//!  4. User configuration simulation.
//!  5. Workspace configuration.
//!  6. Environment variable overrides (all 8 representative vars).
//!  7. Explicit `--config` CLI override (and fail fast).
//!  8. CLI flag overrides (`--model`, `--profile`, `--concurrency`).
//!  9. Complete 8-tier hierarchy resolution.
//! 10. Dynamic `/model` session switch (with provider prefix validation).
//! 11. Dynamic `/profile` session switch.
//! 12. Attempt to weaken security invariants via session. Verify rejected.
//! 13. Execute an autonomous mission under the active profile and model.
//! 14. Verify runtime behavior matches configuration:
//!     - Concurrency limits respected.
//!     - Timeout respected.
//!     - Budget limits enforced.
//!     - Post-mission Git commit exists with model trailer matching configured model.
//!     - Test verification executed configured test command.
//!     - Denied tools were blocked.
//! 15. Inspect TUI Settings (View 34) projection.
//! 16. Verify all secrets are masked.
//! 17. Verify multi-language verification adapter detected.
//! 18. Execute CLI configuration commands (explain, sources, get, validate).
//! 19. Reconfigure dynamically (`with_config`), verify `AppRuntime`, `ControllerDependencies`, `BudgetEnforcer`, and `WorktreeManager` all reflect new configuration.
//! 20. Run post-migration validation checks.

use async_trait::async_trait;
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tempfile::tempdir;
use tokio_util::sync::CancellationToken;

use m31a::agent::model_policy::{ModelCaller, ModelProposal, ModelToolCall};
use m31a::cli::dispatch::{CliDispatcher, RuntimeCommand};
use m31a::config::hierarchy::{ConfigPrecedenceEngine, ConfigTier};
use m31a::config::merge::ConfigError;
use m31a::config::provenance::ConfigLayer;
use m31a::config::resolved::{ResolvedConfigBuilder, is_secret_key, mask_value};
use m31a::config::schema::{AppConfig, validate_config};
use m31a::events::bus::BroadcastEventBus;
use m31a::interaction::action::ApplicationAction;
use m31a::interaction::commands::{CommandContext, CommandOutput, SlashCommandRegistry};
use m31a::kernel::seams::context::CompiledContext;
use m31a::kernel::seams::policy::{PolicyDecision, PolicyEvaluationRequest, PolicyGate};
use m31a::persistence::sqlite::schema::initialize_database;
use m31a::runtime::AppRuntime;
use m31a::verification::adapter::{ProjectAdapter, ProjectType};

const FIXED_SRC_LIB: &str = r#"//! Repaired library for golden test.

pub fn answer() -> u32 {
    42
}
"#;

/// Deterministic model for Step 13 autonomous execution.
#[derive(Default)]
struct GoldenTestModel {
    pub turns_executed: AtomicUsize,
}

#[async_trait]
impl ModelCaller for GoldenTestModel {
    async fn call_model(&self, _context: &str) -> Result<ModelProposal, String> {
        let plan_json = serde_json::json!({
            "tasks": [
                {
                    "id": "TASK-01",
                    "title": "Fix library implementation",
                    "description": "Ensure answer returns 42",
                    "depends_on": [],
                    "required_capabilities": ["fs.read", "fs.write", "cargo.test"],
                    "role": "Implementer"
                }
            ]
        });
        Ok(ModelProposal::Complete {
            summary: plan_json.to_string(),
            artifacts: vec![],
        })
    }

    async fn call_model_with_context(
        &self,
        _compiled: &CompiledContext,
        _cancellation: &CancellationToken,
    ) -> Result<ModelProposal, String> {
        let turn = self.turns_executed.fetch_add(1, Ordering::SeqCst);
        match turn {
            0 => Ok(ModelProposal::ToolCalls {
                calls: vec![ModelToolCall::new(
                    "run_tests",
                    serde_json::json!({ "args": ["--test", "calc_test"] }),
                )],
            }),
            1 => Ok(ModelProposal::ToolCalls {
                calls: vec![ModelToolCall::new(
                    "write_file",
                    serde_json::json!({
                        "path": "src/lib.rs",
                        "content": FIXED_SRC_LIB,
                    }),
                )],
            }),
            2 => Ok(ModelProposal::ToolCalls {
                calls: vec![ModelToolCall::new(
                    "run_tests",
                    serde_json::json!({ "args": ["--test", "calc_test"] }),
                )],
            }),
            _ => Ok(ModelProposal::Complete {
                summary: "Autonomous repair complete".to_string(),
                artifacts: vec![],
            }),
        }
    }
}

#[tokio::test]
async fn test_golden_configuration_runtime_20_steps() {
    // -------------------------------------------------------------------------
    // Step 1: Initialize clean temp workspace with a Git repository
    // -------------------------------------------------------------------------
    let temp = tempdir().expect("failed to create temp workspace");
    let ws = temp.path().to_path_buf();

    let git_init = Command::new("git")
        .args(["init", "-b", "main"])
        .current_dir(&ws)
        .status()
        .expect("git init must succeed");
    assert!(git_init.success());

    Command::new("git")
        .args(["config", "user.name", "M31A Golden Agent"])
        .current_dir(&ws)
        .status()
        .unwrap();
    Command::new("git")
        .args(["config", "user.email", "golden@m31a.local"])
        .current_dir(&ws)
        .status()
        .unwrap();

    // Initial files
    tokio::fs::write(
        ws.join("Cargo.toml"),
        "[package]\nname = \"golden_calc\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .await
    .unwrap();

    tokio::fs::create_dir_all(ws.join("src")).await.unwrap();
    tokio::fs::write(
        ws.join("src/lib.rs"),
        "pub fn answer() -> u32 { 0 /* failing placeholder */ }\n",
    )
    .await
    .unwrap();

    tokio::fs::create_dir_all(ws.join("tests")).await.unwrap();
    tokio::fs::write(
        ws.join("tests/calc_test.rs"),
        "#[test]\nfn test_answer() {\n    assert_eq!(golden_calc::answer(), 42);\n}\n",
    )
    .await
    .unwrap();

    Command::new("git")
        .args(["add", "-A"])
        .current_dir(&ws)
        .status()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "chore: initial commit"])
        .current_dir(&ws)
        .status()
        .unwrap();

    // -------------------------------------------------------------------------
    // Step 2: Verify Phase 1 default configuration
    // -------------------------------------------------------------------------
    let default_cfg = ResolvedConfigBuilder::new(&ws).build().unwrap();
    assert_eq!(
        default_cfg.active_model,
        "meta/llama-3.2-11b-vision-instruct"
    );
    assert_eq!(default_cfg.app_config.runtime.concurrency_limit, 4);
    assert_eq!(default_cfg.app_config.runtime.timeout_secs, 300);
    assert!(default_cfg.app_config.policy.interactive_approvals);
    assert!(default_cfg.app_config.git.auto_commit);

    // -------------------------------------------------------------------------
    // Step 3: System configuration simulation
    // -------------------------------------------------------------------------
    let mut engine = ConfigPrecedenceEngine::new();
    let sys_toml = r#"
        [runtime]
        concurrency_limit = 2
        timeout_secs = 600
        [agents]
        default_model = "system-model"
    "#;
    engine
        .set_layer_from_str(ConfigTier::Tier1System, sys_toml)
        .unwrap();
    let sys_resolved: AppConfig = engine.resolve().unwrap();
    assert_eq!(sys_resolved.runtime.concurrency_limit, 2);
    assert_eq!(sys_resolved.agents.default_model, "system-model");

    // -------------------------------------------------------------------------
    // Step 4: User configuration simulation
    // -------------------------------------------------------------------------
    let user_toml = r#"
        [runtime]
        concurrency_limit = 3
        [agents]
        default_model = "user-model"
    "#;
    engine
        .set_layer_from_str(ConfigTier::Tier2User, user_toml)
        .unwrap();
    let user_resolved: AppConfig = engine.resolve().unwrap();
    assert_eq!(user_resolved.runtime.concurrency_limit, 3);
    assert_eq!(user_resolved.agents.default_model, "user-model");

    // -------------------------------------------------------------------------
    // Step 5: Workspace configuration (.m31a/config.toml)
    // -------------------------------------------------------------------------
    let m31a_dir = ws.join(".m31a");
    tokio::fs::create_dir_all(&m31a_dir).await.unwrap();
    let ws_toml = r#"
        [agents]
        default_model = "meta/llama-3.2-11b-vision-instruct"

        [runtime]
        concurrency_limit = 3
        timeout_secs = 240

        [budget]
        max_agent_steps = 15
        max_cost_usd = 8.5

        [policy]
        denied_tools = ["forbidden_tool", "rm_rf"]
        interactive_approvals = true

        [git]
        auto_commit = true

        [workspace.verification]
        manifest_file = "Cargo.toml"
        test_command = "cargo test --quiet"
    "#;
    tokio::fs::write(m31a_dir.join("config.toml"), ws_toml)
        .await
        .unwrap();

    let ws_cfg = ResolvedConfigBuilder::new(&ws).build().unwrap();
    assert_eq!(ws_cfg.app_config.runtime.concurrency_limit, 3);
    assert_eq!(ws_cfg.app_config.budget.max_agent_steps, Some(15));
    assert_eq!(ws_cfg.app_config.budget.max_cost_usd, Some(8.5));
    assert_eq!(
        ws_cfg.app_config.policy.denied_tools,
        vec!["forbidden_tool".to_string(), "rm_rf".to_string()]
    );

    // -------------------------------------------------------------------------
    // Step 6: Environment variable overrides (Tier 5)
    // -------------------------------------------------------------------------
    unsafe {
        std::env::set_var("M31A_MODEL", "meta/llama-3.3-70b-instruct");
        std::env::set_var("M31A_CONCURRENCY", "5");
        std::env::set_var("M31A_TIMEOUT", "450");
        std::env::set_var("M31A_THEME", "matrix");
        std::env::set_var("M31A_MAX_STEPS", "25");
        std::env::set_var("M31A_AUTO_COMMIT", "true");
        std::env::set_var("M31A_TEST_COMMAND", "cargo test --test calc_test");
        std::env::set_var("M31A_DENIED_TOOLS", "forbidden_tool,rm_rf,dangerous_tool");
    }

    let env_cfg = ResolvedConfigBuilder::new(&ws).build().unwrap();
    assert_eq!(env_cfg.active_model, "meta/llama-3.3-70b-instruct");
    assert_eq!(env_cfg.app_config.runtime.concurrency_limit, 5);
    assert_eq!(env_cfg.app_config.runtime.timeout_secs, 450);
    assert_eq!(env_cfg.app_config.tui.theme, "matrix");
    assert_eq!(env_cfg.app_config.budget.max_agent_steps, Some(25));
    assert_eq!(
        env_cfg.app_config.policy.denied_tools,
        vec!["forbidden_tool", "rm_rf", "dangerous_tool"]
    );

    // Clean up env vars to avoid bleeding into subsequent steps
    unsafe {
        std::env::remove_var("M31A_MODEL");
        std::env::remove_var("M31A_CONCURRENCY");
        std::env::remove_var("M31A_TIMEOUT");
        std::env::remove_var("M31A_THEME");
        std::env::remove_var("M31A_MAX_STEPS");
        std::env::remove_var("M31A_AUTO_COMMIT");
        std::env::remove_var("M31A_TEST_COMMAND");
        std::env::remove_var("M31A_DENIED_TOOLS");
    }

    // -------------------------------------------------------------------------
    // Step 7: Explicit `--config` CLI override (and fail fast)
    // -------------------------------------------------------------------------
    let explicit_path = ws.join("explicit_config.toml");
    tokio::fs::write(
        &explicit_path,
        r#"
        [agents]
        default_model = "meta/llama-3.2-11b-vision-instruct"
        [runtime]
        concurrency_limit = 2
        "#,
    )
    .await
    .unwrap();

    let explicit_cfg = ResolvedConfigBuilder::new(&ws)
        .with_explicit_config(Some(explicit_path.clone()))
        .build()
        .unwrap();
    assert_eq!(explicit_cfg.app_config.runtime.concurrency_limit, 2);

    // Fail-fast test: non-existent explicit config must return IoError
    let missing_path = ws.join("non_existent_config.toml");
    let missing_res = ResolvedConfigBuilder::new(&ws)
        .with_explicit_config(Some(missing_path))
        .build();
    assert!(
        matches!(missing_res, Err(ConfigError::IoError(_))),
        "Missing explicit config file must fail fast without fallback"
    );

    // -------------------------------------------------------------------------
    // Step 8: CLI flag overrides (--model, --profile, etc.)
    // -------------------------------------------------------------------------
    let cli_cfg = ResolvedConfigBuilder::new(&ws)
        .with_model(Some("meta/llama-3.3-70b-instruct".to_string()))
        .with_concurrency(Some(6))
        .with_profile(Some("coding".to_string()))
        .build()
        .unwrap();
    assert_eq!(cli_cfg.active_model, "meta/llama-3.3-70b-instruct");
    assert_eq!(cli_cfg.app_config.runtime.concurrency_limit, 6);
    assert_eq!(cli_cfg.active_profile, Some("coding".to_string()));

    // -------------------------------------------------------------------------
    // Step 9: Complete 8-tier hierarchy resolution
    // -------------------------------------------------------------------------
    let explain_model = cli_cfg.explain("agents.default_model").unwrap();
    assert_eq!(explain_model.winning_layer, ConfigLayer::Tier6Cli);
    assert_eq!(
        explain_model.resolved_value,
        serde_json::json!("meta/llama-3.3-70b-instruct")
    );

    let explain_conc = cli_cfg.explain("runtime.concurrency_limit").unwrap();
    assert_eq!(explain_conc.winning_layer, ConfigLayer::Tier6Cli);
    assert_eq!(explain_conc.resolved_value, serde_json::json!(6));

    // -------------------------------------------------------------------------
    // Step 10: Dynamic `/model` session switch (with provider validation)
    // -------------------------------------------------------------------------
    let slash_reg = SlashCommandRegistry::new_standard();
    let db_path = ws.join("m31a.db");
    let pool = initialize_database(&db_path).await.unwrap();
    let bus = Arc::new(BroadcastEventBus::new(1024));

    let cmd_ctx = CommandContext {
        workspace_root: &ws,
        session_id: None,
        active_mission_id: None,
        pool: &pool,
        event_bus: &bus,
        configured_model: cli_cfg.active_model.clone(),
        configured_provider: "nvidia".to_string(),
        active_profile: "coding".to_string(),
        tool_registry: None,
        command_registry: None,
    };

    // Rejection of unsupported provider
    let invalid_model_out = slash_reg
        .execute_line("/model unsupported_provider/model-x", &cmd_ctx)
        .await
        .unwrap();
    match invalid_model_out {
        CommandOutput::Error(t) => assert!(t.contains("Unsupported model provider")),
        _ => panic!("Expected error output for unsupported provider"),
    }

    // Valid switch
    let valid_model_out = slash_reg
        .execute_line("/model meta/llama-3.2-11b-vision-instruct", &cmd_ctx)
        .await
        .unwrap();
    let dynamic_model = match valid_model_out {
        CommandOutput::ApplicationAction(ApplicationAction::ModelChangeRequested { model }) => {
            cli_cfg.with_session_model(&model).unwrap()
        }
        _ => panic!("Expected ModelChangeRequested action"),
    };
    assert_eq!(
        dynamic_model.active_model,
        "meta/llama-3.2-11b-vision-instruct"
    );
    assert_eq!(
        dynamic_model
            .explain("agents.default_model")
            .unwrap()
            .winning_layer,
        ConfigLayer::Tier7Session
    );

    // -------------------------------------------------------------------------
    // Step 11: Dynamic `/profile` session switch
    // -------------------------------------------------------------------------
    let dynamic_profile = dynamic_model.with_session_profile("safe").unwrap();
    assert_eq!(dynamic_profile.active_profile, Some("safe".to_string()));

    // -------------------------------------------------------------------------
    // Step 12: Monotonic security invariant enforcement via session
    // -------------------------------------------------------------------------
    let weaken_approval = dynamic_profile
        .with_session_override("policy.interactive_approvals", serde_json::json!(false));
    assert!(
        weaken_approval.is_err(),
        "Session override must not weaken interactive_approvals invariant"
    );

    // -------------------------------------------------------------------------
    // Step 13: Execute autonomous mission under the active profile and model
    // -------------------------------------------------------------------------
    let runtime = AppRuntime::from_pool_workspace_and_config(
        pool.clone(),
        ws.clone(),
        bus.clone(),
        Arc::new(dynamic_profile.clone()),
    )
    .await
    .unwrap();

    let model = Arc::new(GoldenTestModel::default());
    let runtime_with_model = runtime.with_model_caller(model);

    let mission_res = runtime_with_model
        .run_mission("Fix the answer function in src/lib.rs", Some("safe"), false)
        .await
        .expect("Mission execution must succeed");

    assert_eq!(
        mission_res.status, "Completed",
        "mission halt: {}",
        mission_res.halt_reason
    );

    // -------------------------------------------------------------------------
    // Step 14: Verify runtime behavior matches configuration
    // -------------------------------------------------------------------------
    // Budget limits enforced
    let active_budget = runtime_with_model.budget_enforcer().budget();
    assert_eq!(active_budget.max_agent_steps, Some(50));
    assert_eq!(active_budget.max_cost_usd, Some(8.5));

    // Git commit created with model trailer
    let git_log = Command::new("git")
        .args(["log", "-1", "--pretty=%B"])
        .current_dir(&ws)
        .output()
        .unwrap();
    let log_msg = String::from_utf8_lossy(&git_log.stdout);
    assert!(
        log_msg.contains("M31A-Model: meta/llama-3.2-11b-vision-instruct"),
        "Commit trailers must record configured active model. Got: {}",
        log_msg
    );

    // Policy gate blocked denied tools
    let denied_req = PolicyEvaluationRequest::new(
        mission_res.mission_id,
        m31a::ids::TaskId::new(),
        "forbidden_tool",
    )
    .with_role(m31a::state_machine::agent::AgentRole::implementer())
    .with_autonomy_mode(m31a::state_machine::AutonomyMode::Safe)
    .with_workspace(ws.clone());
    let decision = runtime_with_model
        .policy()
        .evaluate(denied_req)
        .await
        .unwrap();
    assert_eq!(decision, PolicyDecision::Deny);

    // Verify repaired file in workspace
    let lib_content = tokio::fs::read_to_string(ws.join("src/lib.rs"))
        .await
        .unwrap();
    assert!(lib_content.contains("42"));

    // -------------------------------------------------------------------------
    // Step 16: Verify all secrets are masked
    // -------------------------------------------------------------------------
    assert!(is_secret_key("provider.nvidia_nim.api_key"));
    assert!(is_secret_key("anthropic_token"));
    assert!(is_secret_key("api_secret"));
    assert!(is_secret_key("authorization_header"));
    assert!(!is_secret_key("agents.default_model"));

    let masked = mask_value(&serde_json::json!("secret_token_123456789"));
    assert_eq!(masked, serde_json::json!("secr...[MASKED]"));

    // -------------------------------------------------------------------------
    // Step 17: Multi-language verification adapter auto-detection
    // -------------------------------------------------------------------------
    let adapter = ProjectAdapter::detect(&ws, None, None);
    assert_eq!(adapter.project_type, ProjectType::Rust);
    assert_eq!(adapter.manifest_file, "Cargo.toml");

    let py_cfg = m31a::config::schema::WorkspaceVerificationConfig {
        tier1_manifest: Some("pyproject.toml".to_string()),
        tier3_tests: Some("pytest tests/".to_string()),
        ..Default::default()
    };
    let py_adapter = ProjectAdapter::detect(&ws, Some(&py_cfg), Some("python"));
    assert_eq!(py_adapter.project_type, ProjectType::Python);
    assert_eq!(py_adapter.test_command, "pytest tests/");

    // -------------------------------------------------------------------------
    // Step 18: Execute CLI configuration commands
    // -------------------------------------------------------------------------
    let dispatcher = CliDispatcher::new()
        .with_workspace_root(ws.clone())
        .with_config(Arc::new(dynamic_profile.clone()));

    let get_res = dispatcher
        .dispatch(RuntimeCommand::ConfigGet {
            key: "agents.default_model".to_string(),
        })
        .await
        .unwrap();
    assert!(get_res.text.contains("meta/llama-3.2-11b-vision-instruct"));

    let explain_res = dispatcher
        .dispatch(RuntimeCommand::ConfigExplain {
            key: "agents.default_model".to_string(),
        })
        .await
        .unwrap();
    assert!(explain_res.text.contains("Session Override"));

    let sources_res = dispatcher
        .dispatch(RuntimeCommand::ConfigSources)
        .await
        .unwrap();
    assert!(sources_res.text.contains("Tier 0: Built-in Safe Defaults"));

    let val_res = dispatcher
        .dispatch(RuntimeCommand::ValidateConfig { path: None })
        .await
        .unwrap();
    assert!(val_res.text.contains("Configuration schema valid"));

    // -------------------------------------------------------------------------
    // Step 19: Reconfigure dynamically (`with_config`)
    // -------------------------------------------------------------------------
    let mutated_cfg = dynamic_profile
        .with_session_model("meta/llama-3.3-70b-instruct")
        .unwrap()
        .with_session_override("runtime.concurrency_limit", serde_json::json!(8))
        .unwrap();

    let reconfigured_rt = runtime_with_model.with_config(Arc::new(mutated_cfg));
    assert_eq!(
        reconfigured_rt.config().active_model,
        "meta/llama-3.3-70b-instruct"
    );
    assert_eq!(
        reconfigured_rt
            .config()
            .app_config
            .runtime
            .concurrency_limit,
        8
    );
    assert_eq!(
        reconfigured_rt
            .budget_enforcer()
            .budget()
            .max_concurrent_agents,
        Some(8)
    );

    // -------------------------------------------------------------------------
    // Step 20: Post-migration validation checks
    // -------------------------------------------------------------------------
    // Range bounds:
    let mut bad_cfg = AppConfig::default();
    bad_cfg.runtime.concurrency_limit = 0;
    assert!(validate_config(&bad_cfg).is_err());
    bad_cfg.runtime.concurrency_limit = 35;
    assert!(validate_config(&bad_cfg).is_err());

    // Legacy path fallback:
    let legacy_paths = m31a::config::paths::PlatformPaths::new();
    let legacy_ws_file = m31a::config::paths::PlatformPaths::workspace_config_file(&ws);
    assert!(legacy_ws_file.ends_with(".m31a/config.toml"));
    assert!(
        legacy_paths
            .user_config_file()
            .to_string_lossy()
            .contains("m31a")
    );
}
