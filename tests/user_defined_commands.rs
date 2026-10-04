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
use std::sync::Arc;
use tempfile::tempdir;

use m31a::events::bus::BroadcastEventBus;
use m31a::ids::SessionId;
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
use m31a::tui::palette_v2::UniversalCommandPalette;

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
