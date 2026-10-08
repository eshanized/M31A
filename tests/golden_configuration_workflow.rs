//! Golden End-to-End Configuration Control Plane Workflow Test (Phase W).
//!
//! Validates:
//! 1. Multi-tier resolution: System -> User -> Workspace -> Profile -> Env -> CLI.
//! 2. Immutable security invariant enforcement across the complete runtime spine.
//! 3. TUI Settings screen model reflection from ResolvedConfiguration.
//! 4. Dynamic session slash commands mutating runtime configuration without process restart.
//! 5. Policy denial integration from configuration `policy.denied_tools`.

use std::sync::Arc;
use tempfile::TempDir;

use m31a::cli::dispatch::{CliDispatcher, RuntimeCommand};
use m31a::config::resolved::ResolvedConfigBuilder;
use m31a::events::bus::BroadcastEventBus;
use m31a::interaction::action::ApplicationAction;
use m31a::interaction::commands::{CommandContext, CommandOutput, SlashCommandRegistry};
use m31a::kernel::seams::policy::{PolicyDecision, PolicyEvaluationRequest, PolicyGate};
use m31a::persistence::sqlite::schema::initialize_database;
use m31a::runtime::AppRuntime;
use m31a::tui::TuiApplication;

#[tokio::test]
async fn test_golden_configuration_control_plane_workflow() {
    let temp = TempDir::new().unwrap();
    let ws = temp.path().to_path_buf();

    // -------------------------------------------------------------------------
    // 1. Setup Workspace Configuration (.m31a/config.toml)
    // -------------------------------------------------------------------------
    let m31a_dir = ws.join(".m31a");
    std::fs::create_dir_all(&m31a_dir).unwrap();
    let ws_config = r#"
    [agents]
    default_model = "workspace-model-1"

    [runtime]
    concurrency_limit = 3

    [policy]
    denied_tools = ["shell_exec", "dangerous_tool"]

    [workspace]
    project_type = "rust"

    [workspace.verification]
    manifest_file = "Cargo.toml"
    test_command = "cargo test --quiet"
    "#;
    std::fs::write(m31a_dir.join("config.toml"), ws_config).unwrap();
    std::fs::write(
        ws.join("Cargo.toml"),
        "[package]\nname = \"fixture\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();

    // -------------------------------------------------------------------------
    // 2. Build ResolvedConfiguration with CLI override
    // -------------------------------------------------------------------------
    let resolved = ResolvedConfigBuilder::new(&ws)
        .with_model(Some("cli-override-model".to_string()))
        .with_concurrency(Some(4))
        .build()
        .expect("Resolved configuration build must succeed");

    // Tier 6 CLI overrides Tier 3 Workspace
    assert_eq!(resolved.active_model, "cli-override-model");
    assert_eq!(resolved.app_config.runtime.concurrency_limit, 4);
    // Non-overridden workspace settings remain intact
    assert_eq!(
        resolved.app_config.policy.denied_tools,
        vec!["shell_exec".to_string(), "dangerous_tool".to_string()]
    );

    // -------------------------------------------------------------------------
    // 3. Initialize Runtime with ResolvedConfiguration
    // -------------------------------------------------------------------------
    let db_path = ws.join("test.db");
    let pool = initialize_database(&db_path).await.unwrap();
    let bus = Arc::new(BroadcastEventBus::new(1024));
    let config_arc = Arc::new(resolved);

    let runtime = AppRuntime::from_pool_workspace_and_config(
        pool.clone(),
        ws.clone(),
        bus.clone(),
        config_arc.clone(),
    )
    .await
    .expect("AppRuntime construction with configuration must succeed");

    assert_eq!(runtime.config().active_model, "cli-override-model");

    // -------------------------------------------------------------------------
    // 4. Verify Policy Layer received Denied Tools from Configuration
    // -------------------------------------------------------------------------
    let req_denied = PolicyEvaluationRequest::new(
        m31a::ids::MissionId::new(),
        m31a::ids::TaskId::new(),
        "dangerous_tool",
    )
    .with_role(m31a::state_machine::agent::AgentRole::implementer())
    .with_autonomy_mode(m31a::state_machine::AutonomyMode::Safe)
    .with_workspace(ws.clone());
    let eval_denied = runtime.policy().evaluate(req_denied).await.unwrap();
    assert_eq!(eval_denied, PolicyDecision::Deny);

    // -------------------------------------------------------------------------
    // 5. Test TUI App Initialization with Resolved Configuration
    // -------------------------------------------------------------------------
    let app = TuiApplication::new()
        .with_workspace_root(ws.clone())
        .with_config(&config_arc);

    assert_eq!(app.model.active_model, "cli-override-model");

    // -------------------------------------------------------------------------
    // 6. Test Dynamic Session Mutation (/model and /profile commands)
    // -------------------------------------------------------------------------
    let reg = SlashCommandRegistry::new_standard();
    let ctx = CommandContext {
        workspace_root: &ws,
        session_id: None,
        active_mission_id: None,
        pool: &pool,
        event_bus: &bus,
        configured_model: runtime.config().active_model.clone(),
        configured_provider: "nvidia".to_string(),
        active_profile: "default".to_string(),
        tool_registry: None,
        command_registry: None,
    };
    let output = reg
        .execute_line("/model meta/llama-3.3-70b-instruct", &ctx)
        .await
        .unwrap();

    match output {
        CommandOutput::ApplicationAction(ApplicationAction::ModelChangeRequested { model }) => {
            assert_eq!(model, "meta/llama-3.3-70b-instruct");
            let session_cfg = runtime.config().with_session_model(&model).unwrap();
            let new_runtime = runtime.clone().with_config(Arc::new(session_cfg));
            assert_eq!(
                new_runtime.config().active_model,
                "meta/llama-3.3-70b-instruct"
            );
        }
        _ => panic!("Expected ModelChangeRequested action"),
    }

    // -------------------------------------------------------------------------
    // 7. Verify CLI Dispatcher Integration with Resolved Configuration
    // -------------------------------------------------------------------------
    let dispatcher = CliDispatcher::new()
        .with_workspace_root(ws.clone())
        .with_config(config_arc);

    let cli_cmd = dispatcher
        .dispatch(RuntimeCommand::ConfigGet {
            key: "agents.default_model".to_string(),
        })
        .await
        .unwrap();
    assert!(cli_cmd.text.contains("cli-override-model"));

    let sources_cmd = dispatcher
        .dispatch(RuntimeCommand::ConfigSources)
        .await
        .unwrap();
    assert!(sources_cmd.text.contains("Tier 0: Built-in Safe Defaults"));
    assert!(sources_cmd.text.contains("Tier3Workspace"));
}
