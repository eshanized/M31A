//! Comprehensive P1 Remediation Verification & Regression Test Suite.
//!
//! Validates all four authorized remediation areas from P1-REMEDIATION:
//! - P1-A: Configuration & Onboarding Integrity (BUG-CFG-01, BUG-CLI-01)
//!   - Dotted key insertion into hierarchical TOML tables
//!   - Pre-write schema deserialization validation against AppConfig preventing corrupted files
//!   - Bare CLI launch onboarding pre-flight detection and Doctor diagnostics
//! - P1-B: Interactive TUI Slash-Command Routing (BUG-TUI-01, BUG-SES-02)
//!   - TUI bridge action routing for ModelChangeRequested mutating runtime config
//!   - TUI bridge action routing for ProfileChangeRequested and ConfigOverrideRequested
//!   - Active profile propagated dynamically to missions instead of hardcoded "autonomous"
//!   - Session repository persistence of assistant confirmation messages
//! - P1-C: Dynamic NVIDIA NIM Model Catalog & Dynamic Selection (BUG-MOD-02)
//!   - Elimination of hardcoded provider whitelist permitting partner publishers
//!   - Validation rejecting malformed or malicious model identifiers
//!   - Dynamic `/model search` and `/model list [tier]` querying ModelCatalog
//! - P1-D: Unwire 22 Registered Capability Tools (BUG-MOD-01)
//!   - ToolFilter wiring for Implementer role admitting all 29 registered tools
//!   - CompleteTool universal lifecycle permission across all role envelopes
//!   - Planner role read-only envelope pruning out mutation tools
//!   - AppRuntime wiring of full tool schemas without 7-tool truncation

use clap::Parser;
use std::fs;
use std::sync::Arc;
use std::time::Duration;
use tempfile::tempdir;
use tokio::time::timeout;

use m31a::agent::profile::AgentProfile;
use m31a::capability::registry::CapabilityRegistry;
use m31a::cli::args::Cli;
use m31a::cli::dispatch::{CliDispatcher, insert_dotted_toml_value};
use m31a::cli::doctor::DoctorRunner;
use m31a::config::ResolvedConfiguration;
use m31a::config::schema::parse_and_validate_config;
use m31a::events::bus::BroadcastEventBus;
use m31a::init::lifecycle::{InitManager, InitState, SetupStep};
use m31a::interaction::action::ApplicationAction;
use m31a::interaction::commands::{CommandContext, CommandOutput, SlashCommandRegistry};
use m31a::interaction::events::InteractionEvent;
use m31a::interaction::session::{ConversationTurn, SqliteSessionRepository};
use m31a::model::catalog::ModelCatalog;
use m31a::model::router::resolver::{ModelCandidate, ModelTier};
use m31a::model::types::ProviderCapabilityStatus;
use m31a::persistence::sqlite::schema::initialize_database;
use m31a::runtime::AppRuntime;
use m31a::state_machine::agent::AgentRole;
use m31a::tools::definition::CompleteTool;
use m31a::tools::filter::{FilterCriteria, ToolFilter};
use m31a::tools::registry::ToolRegistry;
use m31a::tui::runtime_bridge::TuiRuntimeBridge;

// ============================================================================
// P1-A: Configuration & Onboarding Integrity
// ============================================================================

#[test]
fn test_p1_a_dotted_key_nested_table_insertion() {
    let mut root = toml::Table::new();

    // 1. Single-level insertion
    insert_dotted_toml_value(&mut root, "runtime.timeout_secs", toml::Value::Integer(120))
        .expect("single dotted key should succeed");

    assert_eq!(
        root.get("runtime")
            .and_then(|v| v.as_table())
            .and_then(|t| t.get("timeout_secs"))
            .and_then(|v| v.as_integer()),
        Some(120)
    );

    // 2. Multi-level deep insertion: agent.roles.planner.max_steps
    insert_dotted_toml_value(
        &mut root,
        "agent.roles.planner.max_steps",
        toml::Value::Integer(25),
    )
    .expect("deep nested dotted key should succeed");

    assert_eq!(
        root.get("agent")
            .and_then(|v| v.as_table())
            .and_then(|t| t.get("roles"))
            .and_then(|v| v.as_table())
            .and_then(|t| t.get("planner"))
            .and_then(|v| v.as_table())
            .and_then(|t| t.get("max_steps"))
            .and_then(|v| v.as_integer()),
        Some(25)
    );

    // 3. Sibling insertion in intermediate table: agent.roles.reviewer.max_steps
    insert_dotted_toml_value(
        &mut root,
        "agent.roles.reviewer.max_steps",
        toml::Value::Integer(15),
    )
    .expect("sibling nested insertion should succeed");

    assert_eq!(
        root.get("agent")
            .and_then(|v| v.as_table())
            .and_then(|t| t.get("roles"))
            .and_then(|v| v.as_table())
            .and_then(|t| t.get("reviewer"))
            .and_then(|v| v.as_table())
            .and_then(|t| t.get("max_steps"))
            .and_then(|v| v.as_integer()),
        Some(15)
    );

    // Planner should still be intact
    assert_eq!(
        root.get("agent")
            .and_then(|v| v.as_table())
            .and_then(|t| t.get("roles"))
            .and_then(|v| v.as_table())
            .and_then(|t| t.get("planner"))
            .and_then(|v| v.as_table())
            .and_then(|t| t.get("max_steps"))
            .and_then(|v| v.as_integer()),
        Some(25)
    );

    // 4. Incompatible intermediate type returns error
    let conflict_err = insert_dotted_toml_value(
        &mut root,
        "runtime.timeout_secs.invalid_child",
        toml::Value::Boolean(true),
    );
    assert!(
        conflict_err.is_err(),
        "inserting child under primitive value must error"
    );
    assert!(
        conflict_err
            .unwrap_err()
            .contains("intermediate key 'timeout_secs' is not a table")
    );
}

#[tokio::test]
async fn test_p1_a_cli_config_set_dotted_key_and_validation() {
    let tmp = tempdir().expect("create temp dir");
    let ws = tmp.path().to_path_buf();
    let config_dir = ws.join(".m31a");
    fs::create_dir_all(&config_dir).unwrap();

    let initial_toml = r#"
[runtime]
concurrency_limit = 2
timeout_secs = 30
"#;
    fs::write(config_dir.join("config.toml"), initial_toml).unwrap();

    let dispatcher = CliDispatcher::new().with_workspace_root(ws.clone());

    // 1. Dispatch valid dotted config set: runtime.timeout_secs = 60
    let cli_valid =
        Cli::try_parse_from(["m31a", "config", "set", "runtime.timeout_secs", "60"]).unwrap();

    let cmd_valid = dispatcher.parse_command(&cli_valid).unwrap();
    let out_valid = dispatcher.dispatch(cmd_valid).await.unwrap();
    assert_eq!(out_valid.exit_code, 0);

    // Check disk content was updated with valid structure
    let updated_content = fs::read_to_string(config_dir.join("config.toml")).unwrap();
    let parsed_config = parse_and_validate_config(&updated_content)
        .expect("config must remain valid and deserializable");
    assert_eq!(parsed_config.runtime.timeout_secs, 60);

    // 2. Dispatch invalid config key/type: runtime.timeout_secs = "not_a_number"
    let cli_invalid = Cli::try_parse_from([
        "m31a",
        "config",
        "set",
        "runtime.timeout_secs",
        "not_a_number",
    ])
    .unwrap();

    let cmd_invalid = dispatcher.parse_command(&cli_invalid).unwrap();
    let res_invalid = dispatcher.dispatch(cmd_invalid).await;
    assert!(
        res_invalid.is_err(),
        "invalid config mutation must be rejected by pre-write validation"
    );

    // Ensure disk file was NOT corrupted and retains previous valid timeout_secs = 60
    let post_err_content = fs::read_to_string(config_dir.join("config.toml")).unwrap();
    let post_err_config = parse_and_validate_config(&post_err_content)
        .expect("config file must not be corrupted on validation failure");
    assert_eq!(
        post_err_config.runtime.timeout_secs, 60,
        "original valid config must be preserved"
    );
}

#[tokio::test]
async fn test_p1_a_onboarding_preflight_sentinel_and_doctor() {
    let tmp = tempdir().expect("create temp dir");
    let ws = tmp.path().to_path_buf();

    // 1. Fresh directory is NOT onboarded
    let init_mgr = InitManager::new(&ws).expect("init manager init");
    assert!(
        !init_mgr.is_onboarded(),
        "fresh directory must not be detected as onboarded"
    );

    // 2. Run Doctor diagnostics on fresh directory
    let doctor = DoctorRunner::with_default_probes();
    let report = doctor.run(None).await;
    let report_text = report.format_text();
    assert!(
        report_text.contains("M31A Doctor Diagnostics Report"),
        "doctor report must format correctly"
    );
    assert!(
        !report.results.is_empty(),
        "default probes should be executed"
    );

    // 3. Progress through InitManager state machine to Onboarded
    let mut mgr = InitManager::new(&ws).expect("reload init manager");
    mgr.transition_to(InitState::Checking)
        .expect("transition to checking");
    mgr.transition_to(InitState::Configuring(SetupStep::WorkspaceTrust))
        .expect("transition to configuring");
    let mut current_step = SetupStep::WorkspaceTrust;
    while let Some(next_step) = current_step.next() {
        mgr.transition_to(InitState::Configuring(next_step))
            .expect("advance step");
        current_step = next_step;
    }
    mgr.transition_to(InitState::Verifying)
        .expect("transition to verifying");
    mgr.transition_to(InitState::Ready)
        .expect("transition to ready");
    mgr.transition_to(InitState::Onboarded)
        .expect("transition to onboarded");
    mgr.persist_sentinel().expect("persist sentinel");

    // 4. Reload manager from disk and verify it is now onboarded
    let reloaded = InitManager::new(&ws).expect("reload persisted manager");
    assert!(
        reloaded.is_onboarded(),
        "directory must be detected as onboarded after sentinel persistence"
    );
}

// ============================================================================
// P1-B: Interactive TUI Slash-Command Routing
// ============================================================================

#[tokio::test]
async fn test_p1_b_tui_bridge_model_change_routing_and_session_persistence() {
    let tmp = tempdir().expect("create temp dir");
    let ws = tmp.path().to_path_buf();
    let db_path = ws.join("test_p1_b.db");
    let pool = initialize_database(&db_path)
        .await
        .expect("initialize sqlite database");
    let event_bus = Arc::new(BroadcastEventBus::new(1024));

    let runtime = Arc::new(
        AppRuntime::from_pool_and_workspace(pool.clone(), ws.clone(), event_bus.clone())
            .await
            .expect("initialize app runtime"),
    );

    let (mut bridge, worker_handle) = TuiRuntimeBridge::spawn(runtime.clone(), None)
        .await
        .expect("spawn tui bridge");

    let action_tx = bridge.sender();
    let mut event_rx = bridge
        .take_event_receiver()
        .expect("event receiver must be present");

    // Consume initial SessionStarted event
    let first_ev = timeout(Duration::from_secs(2), event_rx.recv())
        .await
        .expect("should receive event in time")
        .expect("channel should not close");
    let session_id = match first_ev {
        InteractionEvent::SessionStarted { session_id } => session_id,
        other => panic!("expected SessionStarted, got {:?}", other),
    };

    // 1. Dispatch ModelChangeRequested action
    action_tx
        .send(ApplicationAction::ModelChangeRequested {
            model: "meta/llama-3.3-70b-instruct".to_string(),
        })
        .expect("send model change action");

    // Expect CommandOutput event confirmation (skipping background domain events like GitStateChanged)
    let confirm_ev = loop {
        let ev = timeout(Duration::from_secs(2), event_rx.recv())
            .await
            .expect("should receive model change confirmation")
            .expect("channel active");
        if matches!(ev, InteractionEvent::CommandOutput { .. }) {
            break ev;
        }
    };
    match confirm_ev {
        InteractionEvent::CommandOutput { text } => {
            assert!(
                text.contains("meta/llama-3.3-70b-instruct"),
                "expected model confirmation, got: {text}"
            );
        }
        other => panic!("expected CommandOutput, got {:?}", other),
    }

    // Verify session persistence in SQLite
    let session_repo = SqliteSessionRepository::new(pool.clone());
    let turns = session_repo
        .get_conversation(session_id)
        .await
        .expect("fetch turns");
    assert!(
        turns.iter().any(|turn| matches!(turn, ConversationTurn::AssistantMessage { content, .. } if content.contains("meta/llama-3.3-70b-instruct"))),
        "assistant confirmation message must be durably stored in session turns"
    );

    // 2. Dispatch ProfileChangeRequested action with valid canonical profile ("coding")
    action_tx
        .send(ApplicationAction::ProfileChangeRequested {
            profile: "coding".to_string(),
        })
        .expect("send profile change action");

    let profile_ev = loop {
        let ev = timeout(Duration::from_secs(2), event_rx.recv())
            .await
            .expect("should receive profile change confirmation")
            .expect("channel active");
        if matches!(ev, InteractionEvent::CommandOutput { .. }) {
            break ev;
        }
    };
    match profile_ev {
        InteractionEvent::CommandOutput { text } => {
            assert!(
                text.contains("coding"),
                "expected profile confirmation, got: {text}"
            );
        }
        other => panic!("expected CommandOutput, got {:?}", other),
    }

    let turns2 = session_repo
        .get_conversation(session_id)
        .await
        .expect("fetch turns");
    assert!(
        turns2.iter().any(|turn| matches!(turn, ConversationTurn::AssistantMessage { content, .. } if content.contains("coding"))),
        "assistant profile confirmation must be stored in session turns"
    );

    // 3. Dispatch ConfigOverrideRequested action
    action_tx
        .send(ApplicationAction::ConfigOverrideRequested {
            key: "runtime.timeout_secs".to_string(),
            value: "90".to_string(),
        })
        .expect("send config override action");

    let cfg_ev = loop {
        let ev = timeout(Duration::from_secs(2), event_rx.recv())
            .await
            .expect("should receive config override confirmation")
            .expect("channel active");
        if matches!(ev, InteractionEvent::CommandOutput { .. }) {
            break ev;
        }
    };
    match cfg_ev {
        InteractionEvent::CommandOutput { text } => {
            assert!(
                text.contains("runtime.timeout_secs = 90"),
                "expected config override confirmation, got: {text}"
            );
        }
        other => panic!("expected CommandOutput, got {:?}", other),
    }

    // Cleanly abort bridge worker
    worker_handle.abort();
}

#[tokio::test]
async fn test_p1_b_active_profile_dynamic_mutation_and_fallback() {
    let tmp = tempdir().expect("create temp dir");
    let ws = tmp.path().to_path_buf();
    let base_cfg = ResolvedConfiguration::build_fallback(&ws);
    assert_eq!(
        base_cfg.active_profile, None,
        "default configuration has no profile override"
    );

    // Mutate session profile using valid canonical profile ("coding")
    let updated_cfg = base_cfg.with_session_profile("coding").unwrap();
    assert_eq!(
        updated_cfg.active_profile.as_deref(),
        Some("coding"),
        "active profile should be mutated to coding"
    );

    // Fallback logic check: active_profile.as_deref().unwrap_or("autonomous")
    assert_eq!(
        updated_cfg
            .active_profile
            .as_deref()
            .unwrap_or("autonomous"),
        "coding"
    );
    assert_eq!(
        base_cfg.active_profile.as_deref().unwrap_or("autonomous"),
        "autonomous"
    );
}

// ============================================================================
// P1-C: Dynamic NVIDIA NIM Model Catalog & Selection
// ============================================================================

#[tokio::test]
async fn test_p1_c_partner_publishers_accepted_without_whitelist() {
    let tmp = tempdir().expect("create temp dir");
    let ws = tmp.path().to_path_buf();
    let db_path = ws.join("test_p1_c.db");
    let pool = initialize_database(&db_path).await.unwrap();
    let event_bus = Arc::new(BroadcastEventBus::new(1024));

    let registry = SlashCommandRegistry::new_standard();
    let ctx = CommandContext {
        workspace_root: &ws,
        session_id: None,
        active_mission_id: None,
        pool: &pool,
        event_bus: &event_bus,
        configured_model: "meta/llama-3.1-70b-instruct".to_string(),
        configured_provider: "nvidia_nim".to_string(),
        active_profile: "autonomous".to_string(),
    };

    // 1. Partner publisher: snowflake
    let out_snowflake = registry
        .execute_line("/model snowflake/arctic-embed-l", &ctx)
        .await
        .expect("execute should succeed");
    assert!(
        matches!(out_snowflake, CommandOutput::ApplicationAction(ApplicationAction::ModelChangeRequested { ref model }) if model == "snowflake/arctic-embed-l"),
        "snowflake partner publisher must be accepted"
    );

    // 2. Partner publisher: 01-ai
    let out_01ai = registry
        .execute_line("/model 01-ai/yi-large", &ctx)
        .await
        .expect("execute should succeed");
    assert!(
        matches!(out_01ai, CommandOutput::ApplicationAction(ApplicationAction::ModelChangeRequested { ref model }) if model == "01-ai/yi-large"),
        "01-ai partner publisher must be accepted"
    );

    // 3. DeepSeek publisher
    let out_deepseek = registry
        .execute_line("/model deepseek-ai/deepseek-r1", &ctx)
        .await
        .expect("execute should succeed");
    assert!(
        matches!(out_deepseek, CommandOutput::ApplicationAction(ApplicationAction::ModelChangeRequested { ref model }) if model == "deepseek-ai/deepseek-r1"),
        "deepseek partner publisher must be accepted"
    );

    // 4. Meta publisher
    let out_meta = registry
        .execute_line("/model meta/llama-3.3-70b-instruct", &ctx)
        .await
        .expect("execute should succeed");
    assert!(
        matches!(out_meta, CommandOutput::ApplicationAction(ApplicationAction::ModelChangeRequested { ref model }) if model == "meta/llama-3.3-70b-instruct"),
        "meta publisher must be accepted"
    );
}

#[tokio::test]
async fn test_p1_c_malicious_and_invalid_model_names_rejected() {
    let tmp = tempdir().expect("create temp dir");
    let ws = tmp.path().to_path_buf();
    let db_path = ws.join("test_p1_c_invalid.db");
    let pool = initialize_database(&db_path).await.unwrap();
    let event_bus = Arc::new(BroadcastEventBus::new(1024));

    let registry = SlashCommandRegistry::new_standard();
    let ctx = CommandContext {
        workspace_root: &ws,
        session_id: None,
        active_mission_id: None,
        pool: &pool,
        event_bus: &event_bus,
        configured_model: "meta/llama-3.1-70b-instruct".to_string(),
        configured_provider: "nvidia_nim".to_string(),
        active_profile: "autonomous".to_string(),
    };

    // 1. Path traversal injection
    let out_traversal = registry
        .execute_line("/model ../../../etc/passwd", &ctx)
        .await
        .expect("execute returns command output");
    assert!(
        matches!(out_traversal, CommandOutput::Error(msg) if msg.contains("Invalid model identifier")),
        "path traversal must be rejected with Invalid model identifier error"
    );

    // 2. Command injection with semicolons
    let out_injection = registry
        .execute_line("/model evil;rm -rf /", &ctx)
        .await
        .expect("execute returns command output");
    assert!(
        matches!(out_injection, CommandOutput::Error(msg) if msg.contains("Invalid model identifier")),
        "semicolon injection must be rejected"
    );

    // 3. Shell variable expansion
    let out_var = registry
        .execute_line("/model foo$bar", &ctx)
        .await
        .expect("execute returns command output");
    assert!(
        matches!(out_var, CommandOutput::Error(msg) if msg.contains("Invalid model identifier")),
        "dollar sign injection must be rejected"
    );

    // 4. Leading slash rejected
    let out_slash = registry
        .execute_line("/model /absolute/path", &ctx)
        .await
        .expect("execute returns command output");
    assert!(
        matches!(out_slash, CommandOutput::Error(msg) if msg.contains("Invalid model identifier")),
        "leading slash must be rejected"
    );
}

#[tokio::test]
async fn test_p1_c_model_search_and_list_subcommands() {
    let tmp = tempdir().expect("create temp dir");
    let ws = tmp.path().to_path_buf();
    let db_path = ws.join("test_p1_c_search.db");
    let pool = initialize_database(&db_path).await.unwrap();
    let event_bus = Arc::new(BroadcastEventBus::new(1024));

    // Populate catalog cache
    let mut catalog = ModelCatalog::new("nvidia_nim");
    catalog.update_from_provider(
        "nvidia_nim",
        vec![
            ModelCandidate::new(
                "meta/llama-3.3-70b-instruct",
                "nvidia_nim",
                ModelTier::Standard,
                131072,
            )
            .with_display_name("Llama 3.3 70B Instruct")
            .with_availability(ProviderCapabilityStatus::Available),
            ModelCandidate::new(
                "meta/llama-3.1-8b-instruct",
                "nvidia_nim",
                ModelTier::Fast,
                131072,
            )
            .with_display_name("Llama 3.1 8B Instruct")
            .with_availability(ProviderCapabilityStatus::Available),
            ModelCandidate::new(
                "deepseek-ai/deepseek-r1",
                "nvidia_nim",
                ModelTier::Reasoning,
                131072,
            )
            .with_display_name("DeepSeek R1")
            .with_availability(ProviderCapabilityStatus::Available),
        ],
    );
    let cache_path = ModelCatalog::cache_path(&ws);
    catalog
        .save_to_cache_file(&cache_path)
        .expect("save catalog cache");

    let registry = SlashCommandRegistry::new_standard();
    let ctx = CommandContext {
        workspace_root: &ws,
        session_id: None,
        active_mission_id: None,
        pool: &pool,
        event_bus: &event_bus,
        configured_model: "meta/llama-3.3-70b-instruct".to_string(),
        configured_provider: "nvidia_nim".to_string(),
        active_profile: "autonomous".to_string(),
    };

    // 1. Search query: "llama"
    let out_search = registry
        .execute_line("/model search llama", &ctx)
        .await
        .unwrap();
    match out_search {
        CommandOutput::Info(txt) => {
            assert!(txt.contains("meta/llama-3.3-70b-instruct"));
            assert!(txt.contains("meta/llama-3.1-8b-instruct"));
            assert!(!txt.contains("deepseek-r1"));
        }
        other => panic!("expected Info, got {:?}", other),
    }

    // 2. List query: "fast"
    let out_list_fast = registry
        .execute_line("/model list fast", &ctx)
        .await
        .unwrap();
    match out_list_fast {
        CommandOutput::Info(txt) => {
            assert!(txt.contains("meta/llama-3.1-8b-instruct"));
            assert!(!txt.contains("meta/llama-3.3-70b-instruct"));
        }
        other => panic!("expected Info, got {:?}", other),
    }

    // 3. List query: "reasoning"
    let out_list_reasoning = registry
        .execute_line("/model list reasoning", &ctx)
        .await
        .unwrap();
    match out_list_reasoning {
        CommandOutput::Info(txt) => {
            assert!(txt.contains("deepseek-ai/deepseek-r1"));
            assert!(!txt.contains("meta/llama-3.1-8b-instruct"));
        }
        other => panic!("expected Info, got {:?}", other),
    }
}

// ============================================================================
// P1-D: Unwire 22 Registered Capability Tools
// ============================================================================

#[tokio::test]
async fn test_p1_d_implementer_receives_all_29_tools() {
    let tmp = tempdir().expect("create temp dir");
    let ws = tmp.path().to_path_buf();
    let capabilities = Arc::new(CapabilityRegistry::production(&ws, None, None));

    let mut reg = ToolRegistry::new_default(capabilities.clone());
    reg.register(CompleteTool);
    reg.register_agentic_tools();
    let tool_registry = Arc::new(reg);

    assert_eq!(
        tool_registry.len(),
        32,
        "base registry with CompleteTool must have exactly 32 tools"
    );

    let profile = AgentProfile::built_in(AgentRole::implementer());
    let criteria = FilterCriteria::new(capabilities).with_role_envelope(&profile.capability_policy);
    let filter = ToolFilter::new(tool_registry);
    let wire_schemas = filter.filter_to_wire_format(&criteria);

    // Core Invariant: Implementer receives all 29 tools, eliminating the hardcoded 7-tool truncation
    assert_eq!(
        wire_schemas.len(),
        29,
        "Implementer wire format must contain all 29 tools (was truncated to 7)"
    );

    let tool_names: Vec<String> = wire_schemas
        .iter()
        .filter_map(|s| {
            s.get("function")
                .and_then(|f| f.get("name"))
                .and_then(|n| n.as_str())
                .map(ToString::to_string)
        })
        .collect();

    let expected_tools = [
        "read_file",
        "write_file",
        "edit_file",
        "apply_patch",
        "list_files",
        "glob",
        "grep",
        "repo_search",
        "repo_symbols",
        "repo_dependencies",
        "run_command",
        "start_job",
        "job_status",
        "job_output",
        "job_stop",
        "git_status",
        "git_diff",
        "git_log",
        "git_show",
        "git_branch",
        "git_checkout",
        "git_add",
        "git_commit",
        "run_tests",
        "run_formatter",
        "run_linter",
        "create_artifact",
        "read_artifact",
        "complete",
    ];

    for expected in expected_tools {
        assert!(
            tool_names.contains(&expected.to_string()),
            "expected tool '{}' must be present in Implementer wire format",
            expected
        );
    }
}

#[tokio::test]
async fn test_p1_d_complete_tool_universally_permitted_across_all_roles() {
    let tmp = tempdir().expect("create temp dir");
    let ws = tmp.path().to_path_buf();
    let capabilities = Arc::new(CapabilityRegistry::production(&ws, None, None));

    let mut reg = ToolRegistry::new_default(capabilities.clone());
    reg.register(CompleteTool);
    let tool_registry = Arc::new(reg);

    let roles = [
        AgentRole::planner(),
        AgentRole::researcher(),
        AgentRole::architect(),
        AgentRole::implementer(),
        AgentRole::reviewer(),
    ];

    for role in roles {
        let profile = AgentProfile::built_in(role.clone());
        let criteria = FilterCriteria::new(capabilities.clone())
            .with_role_envelope(&profile.capability_policy);
        let filter = ToolFilter::new(tool_registry.clone());
        let filtered_tools = filter.filter_tools(&criteria);

        assert!(
            filtered_tools.iter().any(|t| t.id() == "complete"),
            "CompleteTool must be universally available for role {:?}",
            role
        );
    }
}

#[tokio::test]
async fn test_p1_d_planner_restricted_from_mutation_tools() {
    let tmp = tempdir().expect("create temp dir");
    let ws = tmp.path().to_path_buf();
    let capabilities = Arc::new(CapabilityRegistry::production(&ws, None, None));

    let mut reg = ToolRegistry::new_default(capabilities.clone());
    reg.register(CompleteTool);
    let tool_registry = Arc::new(reg);

    let profile = AgentProfile::built_in(AgentRole::planner());
    let criteria = FilterCriteria::new(capabilities).with_role_envelope(&profile.capability_policy);
    let filter = ToolFilter::new(tool_registry);
    let filtered_tools = filter.filter_tools(&criteria);

    let ids: Vec<&str> = filtered_tools.iter().map(|t| t.id()).collect();

    // Read tools and complete must be available
    assert!(ids.contains(&"read_file"));
    assert!(ids.contains(&"list_files"));
    assert!(ids.contains(&"glob"));
    assert!(ids.contains(&"grep"));
    assert!(ids.contains(&"repo_search"));
    assert!(ids.contains(&"complete"));

    // Mutation tools must NOT be available for Planner
    assert!(!ids.contains(&"write_file"));
    assert!(!ids.contains(&"edit_file"));
    assert!(!ids.contains(&"apply_patch"));
    assert!(!ids.contains(&"run_command"));
    assert!(!ids.contains(&"git_commit"));
    assert!(!ids.contains(&"create_artifact"));
}
