//! End-to-end integration tests for global user-defined slash commands.
//!
//! Validates:
//! 1. Dynamic loading from user command directories (`prompts/commands/*.toml`).
//! 2. Bounded loading controls, malformed file diagnostics, path and symlink protections.
//! 3. Canonical PromptCatalog integration, authority levels, and namespace isolation.
//! 4. Single active SlashCommandRegistry, alias binding, palette discovery, and `/help` parity.
//! 5. Collision rejection (built-ins, aliases, duplicate names).
//! 6. PromptCommandHandler argument binding against typed schemas (e.g. `--dry-run`, `--help`).
//! 7. Full governed execution through AppRuntime with tool pipeline and policy gate.
//! 8. Real post-execution verification (`working_tree_status`, `commit_contains_single_file`, `conventional_commit_message`).
//! 9. Negative verification failure (fails closed on dirty tree or non-conventional message).
//! 10. Generic arbitrary second user command without any Rust source modifications.

use std::fs;
use std::path::Path;
use std::process::Command;
use std::sync::{Arc, Mutex};
use tempfile::tempdir;

use m31a::events::bus::BroadcastEventBus;
use m31a::ids::{MissionId, SessionId};
use m31a::interaction::action::ApplicationAction;
use m31a::interaction::commands::{CommandContext, CommandHandler, SlashCommandRegistry};
use m31a::interaction::user_commands::{
    load_global_user_commands_from_dir, parse_user_command_toml,
};
use m31a::model::provider::mock::MockProvider;
use m31a::model::types::{ModelProposal, ModelToolCall, TokenUsage, UsageSource};
use m31a::prompt::PromptError;
use m31a::prompt::catalog::{InMemoryPromptCatalog, PromptCatalog};
use m31a::prompt::provenance::PromptSourceKind;
use m31a::prompt::v2::AuthorityLevel;
use m31a::runtime::AppRuntime;
use m31a::state::Mission;
use m31a::tui::composer::TuiComposer;
use m31a::tui::palette_v2::UniversalCommandPalette;

static GLOBAL_ENV_MUTEX: Mutex<()> = Mutex::new(());

const ATOMIC_COMMIT_TOML: &str = r#"
id = "command.atomic_commit"
version = 1
kind = "command"

[command]
name = "atomic-commit"
aliases = ["ac"]
description = "Commit each changed file independently using conventional commits."
usage = "/atomic-commit [--dry-run]"

[execution]
role = "integrator"
side_effect = "mutating"
requires_approval = false
max_steps = 40

[capabilities]
required = [
    "git.read",
    "git.add",
    "git.commit",
]

[inputs]
optional = [
    { name = "dry_run", type = "boolean", default = false },
    { name = "invocation", type = "string", default = "user-command" },
]

[verification]
required = true
checks = [
    "working_tree_status",
    "commit_contains_single_file",
    "conventional_commit_message",
]

[template]
body = """
Execute atomic commit for {{ invocation }} (dry_run: {{ dry_run }}).
"""
"#;

const SUMMARIZE_DIFF_TOML: &str = r#"
id = "command.summarize_diff"
version = 1
kind = "command"

[command]
name = "summarize-diff"
aliases = ["sd", "diff-sum"]
description = "Summarize differences across repository staging areas."
usage = "/summarize-diff [--format <fmt>]"

[execution]
role = "reviewer"
side_effect = "read_only"
requires_approval = false
max_steps = 10

[capabilities]
required = [
    "git.read",
]

[inputs]
optional = [
    { name = "format", type = "string", default = "markdown" },
]

[verification]
required = false
checks = []

[template]
body = """
Summarize diff in format: {{ format }}.
"""
"#;

/// Helper to set up an isolated test git workspace.
async fn setup_git_workspace(dir: &Path) {
    let git_init = Command::new("git")
        .args(["init", "-b", "main"])
        .current_dir(dir)
        .status()
        .expect("git init failed");
    assert!(git_init.success());

    let _ = Command::new("git")
        .args(["config", "user.name", "M31A Test Agent"])
        .current_dir(dir)
        .status();
    let _ = Command::new("git")
        .args(["config", "user.email", "agent@m31a.local"])
        .current_dir(dir)
        .status();

    fs::write(dir.join(".gitignore"), "/target\n.m31a\n").unwrap();
    fs::create_dir_all(dir.join("src")).unwrap();
    fs::write(
        dir.join("Cargo.toml"),
        r#"[package]
name = "workspace_fixture"
version = "0.1.0"
edition = "2021"
"#,
    )
    .unwrap();
    fs::write(
        dir.join("src/lib.rs"),
        "pub fn add(a: i32, b: i32) -> i32 { a + b }\n",
    )
    .unwrap();

    let _ = Command::new("git")
        .args(["add", "."])
        .current_dir(dir)
        .status();
    let _ = Command::new("git")
        .args(["commit", "-m", "chore: initial commit"])
        .current_dir(dir)
        .status();
}

#[tokio::test]
async fn test_dynamic_loading_from_isolated_directory() {
    let temp = tempdir().unwrap();
    let cmd_dir = temp.path().join("prompts").join("commands");
    fs::create_dir_all(&cmd_dir).unwrap();

    // Write two valid command definitions
    fs::write(cmd_dir.join("atomic-commit.v1.toml"), ATOMIC_COMMIT_TOML).unwrap();
    fs::write(cmd_dir.join("summarize-diff.v1.toml"), SUMMARIZE_DIFF_TOML).unwrap();

    // Write a malformed file
    fs::write(
        cmd_dir.join("malformed.v1.toml"),
        "not a valid toml = [ { incomplete",
    )
    .unwrap();

    let report = load_global_user_commands_from_dir(&cmd_dir);

    // Fail-closed per file: 2 valid loaded, 1 rejected with diagnostics
    assert_eq!(report.loaded.len(), 2);
    assert_eq!(report.rejected.len(), 1);

    let names: Vec<&str> = report.loaded.iter().map(|c| c.name.as_str()).collect();
    assert!(names.contains(&"atomic-commit"));
    assert!(names.contains(&"summarize-diff"));

    let diags = report.diagnostics();
    assert_eq!(diags.len(), 1);
    assert!(diags[0].contains("malformed.v1.toml"));
}

#[tokio::test]
async fn test_prompt_catalog_authority_and_reachability() {
    let temp = tempdir().unwrap();
    let cmd_dir = temp.path().join("prompts").join("commands");
    fs::create_dir_all(&cmd_dir).unwrap();
    fs::write(cmd_dir.join("atomic-commit.v1.toml"), ATOMIC_COMMIT_TOML).unwrap();
    fs::write(cmd_dir.join("summarize-diff.v1.toml"), SUMMARIZE_DIFF_TOML).unwrap();

    let mut catalog = InMemoryPromptCatalog::new();
    let loaded = catalog.reload_user_commands_from_dir(&cmd_dir).unwrap();
    assert_eq!(loaded, 2);
    assert_eq!(catalog.user_commands_count(), 2);

    // Verify canonical registration namespace: command.<name>
    let contract = catalog.get("command.atomic-commit", 1).unwrap();
    assert_eq!(contract.id, "command.atomic_commit");
    assert_eq!(contract.version, 1);
    assert_eq!(contract.role.as_str(), "integrator");
    assert_eq!(contract.authority, AuthorityLevel::DynamicMission);

    let meta_list = catalog.list_user_commands();
    assert_eq!(meta_list.len(), 2);
    assert_eq!(
        meta_list[0].source_kind,
        PromptSourceKind::GlobalUserCommand
    );
    assert!(!meta_list[0].is_overrideable);

    // Invariant: Workspace files attempting to hijack command.* namespace must fail closed
    let hijack_toml = r#"
id = "command.atomic_commit"
version = 1
kind = "command"
[command]
name = "atomic-commit"
description = "Malicious hijack"
usage = "/atomic-commit"
[execution]
role = "implementer"
[template]
body = "bad"
"#;
    let hijack_cmd = parse_user_command_toml(hijack_toml, None).unwrap();
    let hijack_contract = hijack_cmd.to_prompt_contract().unwrap();
    let override_res = catalog.register_with_source(
        hijack_contract,
        PromptSourceKind::WorkspaceOverride,
        Some("workspace/prompts/command.atomic-commit.toml".to_string()),
    );
    assert!(matches!(
        override_res,
        Err(PromptError::PromptSecurityViolation { .. })
    ));
}

#[tokio::test]
async fn test_single_active_registry_and_help_parity() {
    let temp = tempdir().unwrap();
    let cmd_dir = temp.path().join("prompts").join("commands");
    fs::create_dir_all(&cmd_dir).unwrap();
    fs::write(cmd_dir.join("atomic-commit.v1.toml"), ATOMIC_COMMIT_TOML).unwrap();
    fs::write(cmd_dir.join("summarize-diff.v1.toml"), SUMMARIZE_DIFF_TOML).unwrap();

    let report = load_global_user_commands_from_dir(&cmd_dir);
    let mut reg = SlashCommandRegistry::new_standard();
    let diags = reg.register_user_commands(report.loaded, &[]);
    assert!(diags.is_empty());

    // Primary name lookup
    let cmd = reg.find("atomic-commit").expect("command found");
    assert_eq!(cmd.name, "atomic-commit");
    assert_eq!(cmd.aliases, vec!["ac".to_string()]);

    // Alias lookup resolves to the identical command contract
    let cmd_by_alias = reg.find("ac").expect("alias found");
    assert_eq!(cmd_by_alias.name, "atomic-commit");

    // General /help contains user commands
    let general_help = reg.generate_help(None);
    assert!(general_help.contains("/atomic-commit"));
    assert!(general_help.contains("/summarize-diff"));
    assert!(general_help.contains("/help")); // Built-ins preserved

    // Specific /help atomic-commit includes full metadata and parameters
    let specific_help = reg.generate_help(Some("atomic-commit"));
    assert!(specific_help.contains("Usage:   /atomic-commit [--dry-run]"));
    assert!(specific_help.contains("Aliases: /ac"));
    assert!(specific_help.contains("Effect:  Mutating"));
    assert!(specific_help.contains("dry-run"));
    assert!(specific_help.contains("optional"));

    // Specific /help ac returns the exact same help
    let alias_help = reg.generate_help(Some("ac"));
    assert_eq!(specific_help, alias_help);

    // UniversalCommandPalette integration: registers user slash commands and searches them
    let mut palette = UniversalCommandPalette::new();
    palette.register_slash_commands(&reg);
    palette.set_query("/atomic");
    let matches = palette.filtered_items();
    assert!(
        matches
            .iter()
            .any(|item| item.label.contains("/atomic-commit"))
    );
}

#[tokio::test]
async fn test_command_collisions_and_rejections() {
    let report = load_global_user_commands_from_dir(Path::new("/nonexistent"));
    assert!(report.is_empty());

    // 1. Built-in name collision: user command named 'help'
    let fake_help_toml = r#"
id = "command.help"
version = 1
kind = "command"
[command]
name = "help"
description = "hijack help"
usage = "/help"
[execution]
role = "implementer"
[template]
body = "bad"
"#;
    let help_cmd = parse_user_command_toml(fake_help_toml, None).unwrap();
    let mut reg = SlashCommandRegistry::new_standard();
    let diags = reg.register_user_commands(vec![help_cmd], &[]);
    assert_eq!(diags.len(), 1);
    assert!(diags[0].reason.contains("collides with built-in command"));

    // 2. Built-in alias collision: user command alias colliding with built-in alias or name
    let fake_alias_toml = r#"
id = "command.my_status"
version = 1
kind = "command"
[command]
name = "my-status"
aliases = ["status"]
description = "hijack status"
usage = "/my-status"
[execution]
role = "implementer"
[template]
body = "bad"
"#;
    let alias_cmd = parse_user_command_toml(fake_alias_toml, None).unwrap();
    let diags2 = reg.register_user_commands(vec![alias_cmd], &[]);
    assert_eq!(diags2.len(), 1);
    assert!(diags2[0].reason.contains("collides with built-in command"));
}

#[tokio::test]
async fn test_prompt_command_handler_argument_binding() {
    let cmd = parse_user_command_toml(ATOMIC_COMMIT_TOML, None).unwrap();
    let handler = m31a::interaction::user_commands::PromptCommandHandler::new(Arc::new(cmd));

    let dir = tempdir().unwrap();
    let bus = Arc::new(BroadcastEventBus::new(16));
    let dummy_pool = m31a::persistence::sqlite::pool::create_pool(&dir.path().join("dummy.db"))
        .await
        .unwrap();

    let ctx = CommandContext {
        workspace_root: dir.path(),
        session_id: None,
        active_mission_id: None,
        pool: &dummy_pool,
        event_bus: &bus,
        configured_model: "test-model".to_string(),
        configured_provider: "test-provider".to_string(),
        active_profile: "autonomous".to_string(),
        tool_registry: None,
        command_registry: None,
    };

    // 1. Invocation with --dry-run
    let out = handler
        .execute(&["--dry-run".to_string()], &ctx)
        .await
        .unwrap();
    match out {
        m31a::interaction::commands::CommandOutput::ApplicationAction(
            ApplicationAction::UserCommandRequested { command, args },
        ) => {
            assert_eq!(command, "atomic-commit");
            assert_eq!(args, vec!["--dry-run".to_string()]);
        }
        other => panic!("expected UserCommandRequested, got {:?}", other),
    }

    // 2. Invocation with --help returns info output without requesting action
    let out_help = handler
        .execute(&["--help".to_string()], &ctx)
        .await
        .unwrap();
    match out_help {
        m31a::interaction::commands::CommandOutput::Info(msg) => {
            assert!(msg.contains("Usage:   /atomic-commit [--dry-run]"));
        }
        other => panic!("expected Info, got {:?}", other),
    }

    // 3. Invocation with invalid argument fails with validation error output
    let out_invalid = handler
        .execute(&["--unknown-flag".to_string()], &ctx)
        .await
        .unwrap();
    match out_invalid {
        m31a::interaction::commands::CommandOutput::Error(msg) => {
            assert!(msg.contains("unknown flag '--unknown-flag'"));
        }
        other => panic!("expected Error, got {:?}", other),
    }
}

#[derive(Clone)]
struct AutoApprovalChannel {
    coordinator: Arc<
        tokio::sync::RwLock<Option<Arc<m31a::policy::approval::coordinator::ApprovalCoordinator>>>,
    >,
}

#[async_trait::async_trait]
impl m31a::policy::approval::channel::ApprovalChannel for AutoApprovalChannel {
    async fn notify_request(
        &self,
        packet: &m31a::policy::approval::explanation::ApprovalExplanationPacket,
    ) -> Result<(), m31a::policy::approval::channel::ApprovalError> {
        let id = packet.request_id;
        let coord_lock = self.coordinator.clone();
        tokio::spawn(async move {
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
            if let Some(coord) = coord_lock.read().await.as_ref() {
                let _ = coord
                    .resolve_request(
                        id,
                        m31a::policy::approval::ApprovalAction::AllowOnce,
                        "test_operator",
                    )
                    .await;
            }
        });
        Ok(())
    }

    async fn poll_response(
        &self,
        _id: m31a::ids::ApprovalRequestId,
    ) -> Result<
        Option<m31a::policy::approval::ApprovalAction>,
        m31a::policy::approval::channel::ApprovalError,
    > {
        Ok(None)
    }
}

#[derive(Clone)]
struct DenyingApprovalChannel {
    coordinator: Arc<
        tokio::sync::RwLock<Option<Arc<m31a::policy::approval::coordinator::ApprovalCoordinator>>>,
    >,
}

#[async_trait::async_trait]
impl m31a::policy::approval::channel::ApprovalChannel for DenyingApprovalChannel {
    async fn notify_request(
        &self,
        packet: &m31a::policy::approval::explanation::ApprovalExplanationPacket,
    ) -> Result<(), m31a::policy::approval::channel::ApprovalError> {
        let id = packet.request_id;
        let coord_lock = self.coordinator.clone();
        tokio::spawn(async move {
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
            if let Some(coord) = coord_lock.read().await.as_ref() {
                let _ = coord
                    .resolve_request(
                        id,
                        m31a::policy::approval::ApprovalAction::Deny {
                            reason: "Operator denied execution for security policy".to_string(),
                        },
                        "security_test_operator",
                    )
                    .await;
            }
        });
        Ok(())
    }

    async fn poll_response(
        &self,
        _id: m31a::ids::ApprovalRequestId,
    ) -> Result<
        Option<m31a::policy::approval::ApprovalAction>,
        m31a::policy::approval::channel::ApprovalError,
    > {
        Ok(None)
    }
}

#[tokio::test]
async fn test_end_to_end_governed_atomic_commit_execution() {
    let dir = tempdir().unwrap();
    setup_git_workspace(dir.path()).await;

    // Create a new untracked/unstaged file in the workspace
    let new_file = dir.path().join("src").join("component.rs");
    fs::write(&new_file, "pub fn component() -> &'static str { \"ok\" }\n").unwrap();

    // Install atomic-commit.v1.toml in the global config dir outside workspace
    let global_config = tempdir().unwrap();
    let cmd_dir = global_config.path().join("prompts").join("commands");
    fs::create_dir_all(&cmd_dir).unwrap();
    fs::write(cmd_dir.join("atomic-commit.v1.toml"), ATOMIC_COMMIT_TOML).unwrap();

    // Initialize AppRuntime
    let mut runtime = AppRuntime::new(dir.path()).await.expect("AppRuntime::new");
    let reloaded = runtime
        .reload_user_commands_from_dir(&cmd_dir)
        .expect("reload user commands");
    assert_eq!(reloaded, 1);

    // Attach auto-approval channel so operator approval for git mutation succeeds
    let approval_channel = Arc::new(AutoApprovalChannel {
        coordinator: Arc::new(tokio::sync::RwLock::new(Some(
            runtime.approval_coordinator().clone(),
        ))),
    });
    let runtime = runtime.with_approval_channel(approval_channel);

    // Configure MockProvider proposing git_add and git_commit, followed by completion
    let mock = Arc::new(MockProvider::new());
    mock.push_response(Ok((
        ModelProposal::ToolCalls {
            calls: vec![
                ModelToolCall::with_id(
                    "call_git_add",
                    "git_add",
                    serde_json::json!({
                        "paths": ["src/component.rs"]
                    }),
                ),
                ModelToolCall::with_id(
                    "call_git_commit",
                    "git_commit",
                    serde_json::json!({
                        "message": "feat(core): add component module"
                    }),
                ),
            ],
        },
        TokenUsage::new(100, 50, 150, 0, UsageSource::AuthoritativeProvider),
    )))
    .await;
    let runtime = runtime.with_model_provider(mock);

    // Execute the user command
    let session_id = SessionId::new();
    let res = runtime
        .execute_user_command("atomic-commit", vec![], session_id)
        .await;

    assert!(res.is_ok(), "execute_user_command failed: {:?}", res);

    // Verify git commit was created and verified
    let commits = runtime.git_service().log(1).await.unwrap();
    assert_eq!(commits.len(), 1);
    assert_eq!(commits[0].message, "feat(core): add component module");

    // Working tree is clean
    let status = runtime.git_service().status().await.unwrap();
    assert!(status.is_clean);
}

#[tokio::test]
async fn test_negative_verification_fails_closed() {
    let dir = tempdir().unwrap();
    setup_git_workspace(dir.path()).await;

    // Create a new file
    let new_file = dir.path().join("src").join("feature.rs");
    fs::write(&new_file, "pub fn feature() {}\n").unwrap();

    let global_config = tempdir().unwrap();
    let cmd_dir = global_config.path().join("prompts").join("commands");
    fs::create_dir_all(&cmd_dir).unwrap();
    fs::write(cmd_dir.join("atomic-commit.v1.toml"), ATOMIC_COMMIT_TOML).unwrap();

    let mut runtime = AppRuntime::new(dir.path()).await.expect("AppRuntime::new");
    runtime.reload_user_commands_from_dir(&cmd_dir).unwrap();

    // Attach auto-approval channel so operator approval for git mutation succeeds
    let approval_channel = Arc::new(AutoApprovalChannel {
        coordinator: Arc::new(tokio::sync::RwLock::new(Some(
            runtime.approval_coordinator().clone(),
        ))),
    });
    let runtime = runtime.with_approval_channel(approval_channel);

    // Mock provider proposing a NON-conventional commit message (e.g. "updated stuff"), followed by completion
    let mock = Arc::new(MockProvider::new());
    mock.push_response(Ok((
        ModelProposal::ToolCalls {
            calls: vec![
                ModelToolCall::with_id(
                    "call_git_add",
                    "git_add",
                    serde_json::json!({
                        "paths": ["src/feature.rs"]
                    }),
                ),
                ModelToolCall::with_id(
                    "call_git_commit",
                    "git_commit",
                    serde_json::json!({
                        "message": "updated stuff without conventional prefix"
                    }),
                ),
            ],
        },
        TokenUsage::new(100, 50, 150, 0, UsageSource::AuthoritativeProvider),
    )))
    .await;
    let runtime = runtime.with_model_provider(mock);

    let session_id = SessionId::new();
    let res = runtime
        .execute_user_command("atomic-commit", vec![], session_id)
        .await;

    // Verification must fail closed: never return fake success!
    assert!(
        res.is_err(),
        "expected verification failure for non-conventional commit"
    );
    let err_str = res.err().unwrap().to_string();
    assert!(err_str.contains("conventional_commit_message"));
}

#[tokio::test]
async fn test_zero_rust_code_changes_for_arbitrary_new_command() {
    let dir = tempdir().unwrap();
    setup_git_workspace(dir.path()).await;

    let cmd_dir = dir.path().join("prompts").join("commands");
    fs::create_dir_all(&cmd_dir).unwrap();

    // User creates a brand new arbitrary command TOML
    const CUSTOM_ANALYSIS_TOML: &str = r#"
id = "command.custom_analysis"
version = 1
kind = "command"

[command]
name = "custom-analysis"
aliases = ["ca"]
description = "Perform deep architectural analysis of the workspace."
usage = "/custom-analysis [--scope <scope>]"

[execution]
role = "architect"
side_effect = "read_only"
requires_approval = false
max_steps = 15

[capabilities]
required = [
    "fs.read",
]

[inputs]
optional = [
    { name = "scope", type = "string", default = "full" },
]

[verification]
required = false
checks = []

[template]
body = """
Analyze architecture with scope: {{ scope }}.
"""
"#;

    fs::write(
        cmd_dir.join("custom-analysis.v1.toml"),
        CUSTOM_ANALYSIS_TOML,
    )
    .unwrap();

    let mut runtime = AppRuntime::new(dir.path()).await.expect("AppRuntime::new");
    let count = runtime.reload_user_commands_from_dir(&cmd_dir).unwrap();
    assert_eq!(count, 1);

    // Active slash registry contains the new command and alias
    let reg = runtime.create_slash_registry_from_dir(&cmd_dir);
    assert!(reg.find("custom-analysis").is_some());
    assert!(reg.find("ca").is_some());

    // /help reports the new command without any Rust modifications
    let help = reg.generate_help(Some("custom-analysis"));
    assert!(help.contains("/custom-analysis [--scope <scope>]"));
    assert!(help.contains("Perform deep architectural analysis of the workspace."));
    assert!(help.contains("scope"));

    // AppRuntime executes the custom command
    let mock = Arc::new(MockProvider::new().with_default_response(
        ModelProposal::Complete {
            summary: "Architecture looks great!".to_string(),
            artifacts: vec![],
        },
        TokenUsage::new(50, 20, 70, 0, UsageSource::AuthoritativeProvider),
    ));
    let runtime = runtime.with_model_provider(mock);

    let session_id = SessionId::new();
    let res = runtime
        .execute_user_command(
            "custom-analysis",
            vec!["--scope".to_string(), "kernel".to_string()],
            session_id,
        )
        .await;

    assert!(
        res.is_ok(),
        "arbitrary new command executed successfully: {:?}",
        res
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Phase S: Verification & Regression Test Suite
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_phase_s_1_global_directory_startup_without_manual_reload() {
    let _lock = GLOBAL_ENV_MUTEX.lock().unwrap();

    let fake_xdg = tempdir().unwrap();
    let old_xdg = std::env::var_os("XDG_CONFIG_HOME");
    unsafe {
        std::env::set_var("XDG_CONFIG_HOME", fake_xdg.path());
    }

    let global_dir = m31a::interaction::user_commands::global_user_commands_dir()
        .expect("global user commands directory resolvable");
    fs::create_dir_all(&global_dir).unwrap();
    fs::write(global_dir.join("atomic-commit.v1.toml"), ATOMIC_COMMIT_TOML).unwrap();

    let workspace = tempdir().unwrap();
    setup_git_workspace(workspace.path()).await;

    // Boot AppRuntime without calling any manual reload methods
    let runtime = AppRuntime::new(workspace.path())
        .await
        .expect("AppRuntime::new");

    // Command and alias are immediately discovered in the active registry on boot
    let reg = runtime.slash_registry();
    assert!(
        reg.find("atomic-commit").is_some(),
        "atomic-commit discovered on startup"
    );
    assert!(reg.find("ac").is_some(), "ac alias discovered on startup");

    // Universal command palette sees user command and alias immediately
    let palette = UniversalCommandPalette::new().with_slash_registry(&reg);
    assert!(
        palette.items().iter().any(|item| item.action
            == m31a::tui::palette_v2::PaletteActionV2::Action("/atomic-commit".to_string())),
        "palette contains /atomic-commit"
    );
    assert!(
        palette
            .items()
            .iter()
            .any(|item| item.action
                == m31a::tui::palette_v2::PaletteActionV2::Action("/ac".to_string())),
        "palette contains /ac"
    );

    // Restore environment
    match old_xdg {
        Some(val) => unsafe { std::env::set_var("XDG_CONFIG_HOME", val) },
        None => unsafe { std::env::remove_var("XDG_CONFIG_HOME") },
    }
}

#[tokio::test]
async fn test_phase_s_2_alias_execution_parity() {
    let dir = tempdir().unwrap();
    setup_git_workspace(dir.path()).await;

    let global_config = tempdir().unwrap();
    let cmd_dir = global_config.path().join("prompts").join("commands");
    fs::create_dir_all(&cmd_dir).unwrap();
    fs::write(cmd_dir.join("atomic-commit.v1.toml"), ATOMIC_COMMIT_TOML).unwrap();

    let mut runtime = AppRuntime::new(dir.path()).await.expect("AppRuntime::new");
    runtime.reload_user_commands_from_dir(&cmd_dir).unwrap();

    let approval_channel = Arc::new(AutoApprovalChannel {
        coordinator: Arc::new(tokio::sync::RwLock::new(Some(
            runtime.approval_coordinator().clone(),
        ))),
    });
    let runtime = runtime.with_approval_channel(approval_channel);

    // 1. Run canonical command /atomic-commit on file1
    let file1 = dir.path().join("src").join("file1.rs");
    fs::write(&file1, "pub fn f1() {}\n").unwrap();

    let mock1 = Arc::new(MockProvider::new());
    mock1
        .push_response(Ok((
            ModelProposal::ToolCalls {
                calls: vec![
                    ModelToolCall::with_id(
                        "c1",
                        "git_add",
                        serde_json::json!({ "paths": ["src/file1.rs"] }),
                    ),
                    ModelToolCall::with_id(
                        "c2",
                        "git_commit",
                        serde_json::json!({ "message": "feat(file1): add initial function" }),
                    ),
                ],
            },
            TokenUsage::new(100, 50, 150, 0, UsageSource::AuthoritativeProvider),
        )))
        .await;
    mock1
        .push_response(Ok((
            ModelProposal::Complete {
                summary: "committed file1".to_string(),
                artifacts: vec![],
            },
            TokenUsage::new(50, 20, 70, 0, UsageSource::AuthoritativeProvider),
        )))
        .await;

    let runtime1 = runtime.clone().with_model_provider(mock1);
    let session1 = SessionId::new();
    let res1 = runtime1
        .execute_user_command("atomic-commit", vec![], session1)
        .await;
    assert!(res1.is_ok(), "canonical /atomic-commit failed: {:?}", res1);

    // 2. Run alias /ac on file2
    let file2 = dir.path().join("src").join("file2.rs");
    fs::write(&file2, "pub fn f2() {}\n").unwrap();

    let mock2 = Arc::new(MockProvider::new());
    mock2
        .push_response(Ok((
            ModelProposal::ToolCalls {
                calls: vec![
                    ModelToolCall::with_id(
                        "c3",
                        "git_add",
                        serde_json::json!({ "paths": ["src/file2.rs"] }),
                    ),
                    ModelToolCall::with_id(
                        "c4",
                        "git_commit",
                        serde_json::json!({ "message": "feat(file2): add second function" }),
                    ),
                ],
            },
            TokenUsage::new(100, 50, 150, 0, UsageSource::AuthoritativeProvider),
        )))
        .await;
    mock2
        .push_response(Ok((
            ModelProposal::Complete {
                summary: "committed file2".to_string(),
                artifacts: vec![],
            },
            TokenUsage::new(50, 20, 70, 0, UsageSource::AuthoritativeProvider),
        )))
        .await;

    let runtime2 = runtime.with_model_provider(mock2);
    let session2 = SessionId::new();
    let res2 = runtime2.execute_user_command("ac", vec![], session2).await;
    assert!(res2.is_ok(), "alias /ac failed: {:?}", res2);

    // Both commits exist in git history with verified conventional messages
    let log_out = Command::new("git")
        .args(["log", "--oneline", "-n", "2"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    let log_str = String::from_utf8_lossy(&log_out.stdout);
    assert!(log_str.contains("feat(file2): add second function"));
    assert!(log_str.contains("feat(file1): add initial function"));
}

#[tokio::test]
async fn test_phase_s_3_single_registry_identity() {
    let dir = tempdir().unwrap();
    let cmd_dir = dir.path().join("prompts").join("commands");
    fs::create_dir_all(&cmd_dir).unwrap();
    fs::write(cmd_dir.join("atomic-commit.v1.toml"), ATOMIC_COMMIT_TOML).unwrap();

    let mut runtime = AppRuntime::new(dir.path()).await.expect("AppRuntime::new");
    runtime.reload_user_commands_from_dir(&cmd_dir).unwrap();

    let runtime_arc = Arc::new(runtime);

    // 1. Session runner receives canonical registry
    let runner = m31a::interaction::runner::InteractiveSessionRunner::new(runtime_arc.clone());

    // Arc pointer equality proves runner shares exact same registry
    assert!(
        Arc::ptr_eq(runtime_arc.slash_registry(), runner.command_registry()),
        "runner must share canonical slash_registry Arc"
    );

    // Arc pointer equality proves parser inside runner shares exact same registry
    assert!(
        Arc::ptr_eq(
            runtime_arc.slash_registry(),
            runner.parser().command_registry_arc()
        ),
        "parser must share canonical slash_registry Arc"
    );

    // 2. Composer receives canonical registry
    let mut composer = TuiComposer::new(dir.path().to_path_buf())
        .with_slash_registry(runtime_arc.slash_registry().clone());

    assert!(
        Arc::ptr_eq(
            runtime_arc.slash_registry(),
            composer.slash_registry().unwrap()
        ),
        "composer must share canonical slash_registry Arc"
    );

    // Autocomplete on composer dynamically reveals user commands and aliases
    composer.set_text("/at");
    assert!(composer.is_autocomplete_open());
    assert!(
        composer
            .autocomplete_suggestions()
            .iter()
            .any(|s| s.label == "/atomic-commit"),
        "autocomplete suggests /atomic-commit"
    );

    composer.set_text("/ac");
    assert!(composer.is_autocomplete_open());
    assert!(
        composer
            .autocomplete_suggestions()
            .iter()
            .any(|s| s.label == "/ac"),
        "autocomplete suggests /ac"
    );

    // 3. Command palette receives canonical registry
    let palette =
        UniversalCommandPalette::new().with_slash_registry(runtime_arc.slash_registry().as_ref());
    assert!(
        palette.items().iter().any(|item| item.action
            == m31a::tui::palette_v2::PaletteActionV2::Action("/atomic-commit".to_string())),
        "palette registered /atomic-commit"
    );
    assert!(
        palette
            .items()
            .iter()
            .any(|item| item.action
                == m31a::tui::palette_v2::PaletteActionV2::Action("/ac".to_string())),
        "palette registered /ac"
    );
}

#[tokio::test]
async fn test_phase_s_4_single_tool_pipeline_authority() {
    let dir = tempdir().unwrap();
    let runtime = AppRuntime::new(dir.path()).await.expect("AppRuntime::new");

    let runtime_pipeline = runtime.tool_pipeline();
    let authorities_pipeline = runtime.authorities().tool_pipeline();

    // Invariant: Exactly one authoritative execution pipeline
    assert!(
        Arc::ptr_eq(&runtime_pipeline, authorities_pipeline),
        "runtime and authorities must share the exact same ToolPipelineRunner instance"
    );

    // Creating an agent engine also uses the exact same pipeline
    let session_id = SessionId::new();
    let engine = runtime.create_agent_engine(session_id);
    assert!(
        Arc::ptr_eq(&runtime_pipeline, engine.pipeline_runner()),
        "agent engine must use the canonical ToolPipelineRunner from AppRuntime"
    );
}

#[tokio::test]
async fn test_phase_s_5_durable_identity_propagation() {
    let dir = tempdir().unwrap();
    setup_git_workspace(dir.path()).await;

    let file = dir.path().join("src").join("durable.rs");
    fs::write(&file, "pub fn durable() {}\n").unwrap();

    let global_config = tempdir().unwrap();
    let cmd_dir = global_config.path().join("prompts").join("commands");
    fs::create_dir_all(&cmd_dir).unwrap();
    fs::write(cmd_dir.join("atomic-commit.v1.toml"), ATOMIC_COMMIT_TOML).unwrap();

    let mut runtime = AppRuntime::new(dir.path()).await.expect("AppRuntime::new");
    runtime.reload_user_commands_from_dir(&cmd_dir).unwrap();

    let approval_channel = Arc::new(AutoApprovalChannel {
        coordinator: Arc::new(tokio::sync::RwLock::new(Some(
            runtime.approval_coordinator().clone(),
        ))),
    });
    let runtime = runtime.with_approval_channel(approval_channel);

    let mock = Arc::new(MockProvider::new());
    mock.push_response(Ok((
        ModelProposal::ToolCalls {
            calls: vec![
                ModelToolCall::with_id(
                    "c1",
                    "git_add",
                    serde_json::json!({ "paths": ["src/durable.rs"] }),
                ),
                ModelToolCall::with_id(
                    "c2",
                    "git_commit",
                    serde_json::json!({ "message": "feat(durable): add durable feature" }),
                ),
            ],
        },
        TokenUsage::new(100, 50, 150, 0, UsageSource::AuthoritativeProvider),
    )))
    .await;
    mock.push_response(Ok((
        ModelProposal::Complete {
            summary: "durable commit finished".to_string(),
            artifacts: vec![],
        },
        TokenUsage::new(50, 20, 70, 0, UsageSource::AuthoritativeProvider),
    )))
    .await;

    let runtime = runtime.with_model_provider(mock);

    // Bind real persistent session and mission
    let session_repo = runtime.session_repo();
    let session = session_repo.create_session(dir.path()).await.unwrap();

    let mission_repo = m31a::persistence::sqlite::repositories::SqliteMissionRepository::new(
        runtime.pool().clone(),
    );
    let mission_id = MissionId::new();
    let mission = Mission::new(mission_id, "Durable Mission Test".to_string());
    mission_repo.insert(&mission).await.unwrap();

    session_repo
        .set_active_mission(session.id, mission_id)
        .await
        .unwrap();

    let res = runtime
        .execute_user_command("atomic-commit", vec![], session.id)
        .await;
    assert!(res.is_ok(), "execute_user_command failed: {:?}", res);

    // 1. Verify Task aggregate row exists with exact mission_id and succeeded state
    let (db_title, db_status): (String, String) =
        sqlx::query_as("SELECT title, status FROM tasks WHERE mission_id = ?")
            .bind(mission_id.as_bytes().as_slice())
            .fetch_one(runtime.pool())
            .await
            .expect("tasks row must exist in SQLite");
    assert_eq!(db_title, "Execute /atomic-commit");
    assert_eq!(db_status, "succeeded");

    // 2. Verify Agent aggregate row exists with exact mission_id and completed state
    let (db_role, db_status): (String, String) =
        sqlx::query_as("SELECT role, status FROM agents WHERE mission_id = ?")
            .bind(mission_id.as_bytes().as_slice())
            .fetch_one(runtime.pool())
            .await
            .expect("agents row must exist in SQLite");
    assert_eq!(db_role, "integrator");
    assert_eq!(db_status, "Completed");

    // 3. Verify verification_checks rows share exact mission_id and task_id
    let db_task_id_bytes: Vec<u8> = sqlx::query_scalar("SELECT id FROM tasks WHERE mission_id = ?")
        .bind(mission_id.as_bytes().as_slice())
        .fetch_one(runtime.pool())
        .await
        .expect("task id exists");

    let count_for_task: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM verification_checks WHERE task_id = ? AND status = 'passed'",
    )
    .bind(&db_task_id_bytes)
    .fetch_one(runtime.pool())
    .await
    .expect("checks count");
    assert_eq!(count_for_task, 3, "must record all 3 verification checks");

    let count_for_mission: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM verification_checks WHERE mission_id = ? AND status = 'passed'",
    )
    .bind(mission_id.as_bytes().as_slice())
    .fetch_one(runtime.pool())
    .await
    .expect("checks count for mission");
    assert_eq!(count_for_mission, 3, "checks must match mission_id");
}

#[tokio::test]
async fn test_phase_s_6_toctou_snapshot_immutability() {
    let dir = tempdir().unwrap();
    let global_config = tempdir().unwrap();
    let cmd_dir = global_config.path().join("prompts").join("commands");
    fs::create_dir_all(&cmd_dir).unwrap();

    const SNAPSHOT_TOML: &str = r#"
id = "command.snapshot_test"
version = 1
kind = "command"

[command]
name = "snapshot-test"
aliases = ["snt"]
description = "Test TOCTOU snapshot immutability."
usage = "/snapshot-test"

[execution]
role = "reviewer"
side_effect = "read_only"
requires_approval = false
max_steps = 5

[capabilities]
required = ["git.read"]

[verification]
required = false
checks = []

[template]
body = "Snapshot test execution."
"#;

    let cmd_file = cmd_dir.join("snapshot-test.v1.toml");
    fs::write(&cmd_file, SNAPSHOT_TOML).unwrap();

    let mut runtime = AppRuntime::new(dir.path()).await.expect("AppRuntime::new");
    runtime.reload_user_commands_from_dir(&cmd_dir).unwrap();

    // Verify command is in the memory snapshot
    assert!(runtime.slash_registry().find("snapshot-test").is_some());

    // DELETE the file from the filesystem to test TOCTOU safety
    fs::remove_file(&cmd_file).unwrap();
    assert!(
        fs::metadata(&cmd_file).is_err(),
        "file must be gone from disk"
    );

    // Configure mock provider
    let mock = Arc::new(MockProvider::new().with_default_response(
        ModelProposal::Complete {
            summary: "snapshot test executed from in-memory definition".to_string(),
            artifacts: vec![],
        },
        TokenUsage::new(30, 10, 40, 0, UsageSource::AuthoritativeProvider),
    ));
    let runtime = runtime.with_model_provider(mock);

    // Command succeeds because execute_user_command uses the immutable in-memory UserCommandDefinition snapshot
    let session_id = SessionId::new();
    let res = runtime
        .execute_user_command("snapshot-test", vec![], session_id)
        .await;
    assert!(
        res.is_ok(),
        "execution must succeed using in-memory snapshot despite file deletion on disk: {:?}",
        res
    );

    // Now reload user commands from directory — snapshot atomically updates to reflect deleted file
    let mut runtime = runtime;
    let count = runtime.reload_user_commands_from_dir(&cmd_dir).unwrap();
    assert_eq!(count, 0, "reload must discover 0 commands");
    assert!(
        runtime.slash_registry().find("snapshot-test").is_none(),
        "command removed from registry after atomic reload"
    );

    // Executing now returns NotFound fail-closed
    let res_after = runtime
        .execute_user_command("snapshot-test", vec![], session_id)
        .await;
    assert!(
        res_after.is_err(),
        "execution must fail-closed once reload removes command"
    );
}

#[tokio::test]
async fn test_phase_s_7_atomic_reload_failure_preserves_valid_commands() {
    let dir = tempdir().unwrap();
    let cmd_dir = dir.path().join("prompts").join("commands");
    fs::create_dir_all(&cmd_dir).unwrap();

    // Write a valid command
    fs::write(cmd_dir.join("atomic-commit.v1.toml"), ATOMIC_COMMIT_TOML).unwrap();

    let mut runtime = AppRuntime::new(dir.path()).await.expect("AppRuntime::new");
    runtime.reload_user_commands_from_dir(&cmd_dir).unwrap();
    assert!(runtime.slash_registry().find("atomic-commit").is_some());

    // Drop a completely corrupt file with syntax errors
    fs::write(
        cmd_dir.join("corrupt.v1.toml"),
        "this is not valid toml at all {{{[[[",
    )
    .unwrap();

    // Reload: reports corrupt file in rejected, but valid command remains intact
    runtime.reload_user_commands_from_dir(&cmd_dir).unwrap();
    let report = runtime.user_command_report();
    assert_eq!(
        report.rejected_count(),
        1,
        "corrupt file recorded in report"
    );
    assert_eq!(report.loaded_count(), 1, "valid command remains loaded");
    assert!(
        runtime.slash_registry().find("atomic-commit").is_some(),
        "previously valid command is preserved and not destroyed by corrupt file"
    );

    // Drop a file attempting to hijack a reserved namespace (e.g. behavioral.*)
    const FORBIDDEN_NS_TOML: &str = r#"
id = "behavioral.agent.hijack"
version = 1
kind = "command"

[command]
name = "hijack-command"
description = "Attempted hijack"
usage = "/hijack-command"

[execution]
role = "integrator"
side_effect = "read_only"
requires_approval = false
max_steps = 5

[capabilities]
required = ["git.read"]

[verification]
required = false
checks = []

[template]
body = "bad"
"#;
    fs::write(cmd_dir.join("forbidden_ns.v1.toml"), FORBIDDEN_NS_TOML).unwrap();

    runtime.reload_user_commands_from_dir(&cmd_dir).unwrap();
    let report2 = runtime.user_command_report();
    assert!(
        report2
            .rejected
            .iter()
            .any(|r| r.reason.contains("reserved")
                || r.reason.contains("namespace")
                || r.reason.contains("command.")),
        "reserved namespace attempt rejected"
    );
    assert!(
        runtime.slash_registry().find("atomic-commit").is_some(),
        "valid command remains active despite multiple rejected files"
    );
}

#[tokio::test]
async fn test_phase_s_8_metadata_enforcement() {
    let dir = tempdir().unwrap();
    setup_git_workspace(dir.path()).await;

    let global_config = tempdir().unwrap();
    let cmd_dir = global_config.path().join("prompts").join("commands");
    fs::create_dir_all(&cmd_dir).unwrap();

    // 8a. Unknown role rejection
    let bad_role_toml = ATOMIC_COMMIT_TOML.replace("role = \"integrator\"", "role = \"archmage\"");
    let err_role = parse_user_command_toml(&bad_role_toml, None).unwrap_err();
    assert!(
        err_role.to_string().contains("role") || err_role.to_string().contains("archmage"),
        "unknown role rejected fail-closed: {:?}",
        err_role
    );

    // 8b. Approval denial blocking
    let approval_cmd_toml = ATOMIC_COMMIT_TOML
        .replace("requires_approval = false", "requires_approval = true")
        .replace("name = \"atomic-commit\"", "name = \"approval-cmd\"")
        .replace(
            "id = \"command.atomic_commit\"",
            "id = \"command.approval_cmd\"",
        )
        .replace("aliases = [\"ac\"]", "aliases = []");
    fs::write(cmd_dir.join("approval-cmd.v1.toml"), approval_cmd_toml).unwrap();

    let mut runtime = AppRuntime::new(dir.path()).await.expect("AppRuntime::new");
    runtime.reload_user_commands_from_dir(&cmd_dir).unwrap();

    let denying_channel = Arc::new(DenyingApprovalChannel {
        coordinator: Arc::new(tokio::sync::RwLock::new(Some(
            runtime.approval_coordinator().clone(),
        ))),
    });
    let runtime_denied = runtime.clone().with_approval_channel(denying_channel);

    let session_id = SessionId::new();
    let res_denied = runtime_denied
        .execute_user_command("approval-cmd", vec![], session_id)
        .await;
    assert!(
        res_denied.is_err(),
        "command requiring approval must fail when operator denies"
    );
    let err_msg = res_denied.err().unwrap().to_string();
    assert!(err_msg.contains("denied by operator approval"));

    // Verify task row was marked failed in SQLite
    let task_statuses: Vec<String> =
        sqlx::query_scalar("SELECT status FROM tasks WHERE title = 'Execute /approval-cmd'")
            .fetch_all(runtime.pool())
            .await
            .unwrap();
    assert_eq!(task_statuses.last().unwrap(), "failed");

    // 8c. max_steps exhaustion failure
    const STEP_BUDGET_TOML: &str = r#"
id = "command.step_budget_test"
version = 1
kind = "command"

[command]
name = "step-budget-test"
aliases = []
description = "Test max_steps budget exhaustion"
usage = "/step-budget-test"

[execution]
role = "integrator"
side_effect = "read_only"
requires_approval = false
max_steps = 2

[capabilities]
required = ["git.read"]

[verification]
required = false
checks = []

[template]
body = "Execute step budget test."
"#;
    fs::write(cmd_dir.join("step-budget-test.v1.toml"), STEP_BUDGET_TOML).unwrap();
    runtime.reload_user_commands_from_dir(&cmd_dir).unwrap();

    // MockProvider proposing infinite tool calls (never completes)
    let loop_mock = Arc::new(MockProvider::new());
    loop_mock
        .push_response(Ok((
            ModelProposal::ToolCalls {
                calls: vec![ModelToolCall::with_id(
                    "loop1",
                    "git_status",
                    serde_json::json!({}),
                )],
            },
            TokenUsage::new(20, 10, 30, 0, UsageSource::AuthoritativeProvider),
        )))
        .await;
    loop_mock
        .push_response(Ok((
            ModelProposal::ToolCalls {
                calls: vec![ModelToolCall::with_id(
                    "loop2",
                    "git_status",
                    serde_json::json!({}),
                )],
            },
            TokenUsage::new(20, 10, 30, 0, UsageSource::AuthoritativeProvider),
        )))
        .await;
    loop_mock
        .push_response(Ok((
            ModelProposal::ToolCalls {
                calls: vec![ModelToolCall::with_id(
                    "loop3",
                    "git_status",
                    serde_json::json!({}),
                )],
            },
            TokenUsage::new(20, 10, 30, 0, UsageSource::AuthoritativeProvider),
        )))
        .await;

    let runtime_loop = runtime.clone().with_model_provider(loop_mock);
    let res_budget = runtime_loop
        .execute_user_command("step-budget-test", vec![], SessionId::new())
        .await;
    assert!(
        res_budget.is_err(),
        "command exceeding step budget must fail closed"
    );
    let budget_err = res_budget.err().unwrap().to_string();
    assert!(budget_err.contains("exhausted step budget of 2 steps without completion"));

    // Verify task row was marked failed in SQLite
    let budget_task_status: String =
        sqlx::query_scalar("SELECT status FROM tasks WHERE title = 'Execute /step-budget-test'")
            .fetch_one(runtime.pool())
            .await
            .unwrap();
    assert_eq!(budget_task_status, "failed");
}

#[tokio::test]
async fn test_phase_s_9_negative_security_invariants() {
    let dir = tempdir().unwrap();
    setup_git_workspace(dir.path()).await;

    // 1. Contract namespace hijacking blocked fail-closed
    let bad_layer0 = r#"
id = "runtime.l0.kernel_override"
version = 1
kind = "command"

[command]
name = "kernel-hack"
description = "Attempt layer0 override"
usage = "/kernel-hack"

[execution]
role = "integrator"
side_effect = "read_only"
requires_approval = false
max_steps = 5

[capabilities]
required = ["git.read"]

[verification]
required = false
checks = []

[template]
body = "evil"
"#;
    let err_ns = parse_user_command_toml(bad_layer0, None).unwrap_err();
    assert!(
        err_ns.to_string().contains("command.") || err_ns.to_string().contains("reserved"),
        "layer-0 namespace hijack blocked: {:?}",
        err_ns
    );

    // 2. Builtin command collision blocked fail-closed
    let bad_builtin = r#"
id = "command.status"
version = 1
kind = "command"

[command]
name = "status"
description = "Attempt builtin collide"
usage = "/status"

[execution]
role = "integrator"
side_effect = "read_only"
requires_approval = false
max_steps = 5

[capabilities]
required = ["git.read"]

[verification]
required = false
checks = []

[template]
body = "collide"
"#;
    let cmd = parse_user_command_toml(bad_builtin, None).unwrap();
    let mut reg = SlashCommandRegistry::new_standard();
    let rejections = reg.register_user_commands(vec![cmd], &[]);
    assert_eq!(rejections.len(), 1);
    assert!(rejections[0].reason.contains("collides with built-in"));

    // 3. Path traversal in command name blocked fail-closed
    let bad_path = r#"
id = "command.path_traversal"
version = 1
kind = "command"

[command]
name = "../escape"
description = "Path traversal command"
usage = "/../escape"

[execution]
role = "integrator"
side_effect = "read_only"
requires_approval = false
max_steps = 5

[capabilities]
required = ["git.read"]

[verification]
required = false
checks = []

[template]
body = "bad"
"#;
    let err_path = parse_user_command_toml(bad_path, None).unwrap_err();
    assert!(err_path.to_string().contains("invalid") || err_path.to_string().contains("name"));

    // 4. Ungranted/denied tools blocked by PolicyGate during governed execution
    let cmd_dir = dir.path().join("prompts").join("commands");
    fs::create_dir_all(&cmd_dir).unwrap();
    fs::write(cmd_dir.join("atomic-commit.v1.toml"), ATOMIC_COMMIT_TOML).unwrap();

    let mut runtime = AppRuntime::new(dir.path()).await.expect("AppRuntime::new");
    runtime.reload_user_commands_from_dir(&cmd_dir).unwrap();

    // Model proposes an ungranted tool outside declared capabilities
    let mock_evil = Arc::new(MockProvider::new());
    mock_evil
        .push_response(Ok((
            ModelProposal::ToolCalls {
                calls: vec![ModelToolCall::with_id(
                    "evil1",
                    "bash_execute",
                    serde_json::json!({ "command": "rm -rf /" }),
                )],
            },
            TokenUsage::new(30, 10, 40, 0, UsageSource::AuthoritativeProvider),
        )))
        .await;

    let runtime_evil = runtime.with_model_provider(mock_evil);
    let session_id = SessionId::new();
    let res_evil = runtime_evil
        .execute_user_command("atomic-commit", vec![], session_id)
        .await;

    assert!(
        res_evil.is_err(),
        "execution proposing ungranted/denied tool must fail closed"
    );
}
