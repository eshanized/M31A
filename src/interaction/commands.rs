//! Centralized Slash Command Subsystem and Command Registry (PRD §01, CLI-01).
//!
//! "Do NOT use scattered match statements throughout the CLI/TUI. Create a typed command abstraction."
//!
//! Minimum commands:
//! - `/help` [command]
//! - `/status`
//! - `/model` [name]
//! - `/profile` [name]
//! - `/config`
//! - `/tools`
//! - `/skills`
//! - `/diff`
//! - `/commit` [message]
//! - `/cancel`
//! - `/resume` [session-id]
//! - `/clear`
//! - `/clear-session`
//! - `/exit`

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use crate::capability::traits::git::GitService;

use crate::error::M31AError;

/// Deterministic slash command argument tokenizer.
///
/// Handles:
/// - Double-quoted strings with escaped quotes: `"hello \"world\""`
/// - Single-quoted strings: `'hello world'`
/// - JSON objects/arrays as single tokens: `{"key": "value"}` or `[1, 2, 3]`
/// - Unquoted words split on whitespace
fn parse_slash_command(input: &str) -> (String, Vec<String>) {
    let trimmed = input.trim();
    if !trimmed.starts_with('/') {
        return (String::new(), Vec::new());
    }

    let rest = &trimmed[1..].trim_start();
    if rest.is_empty() {
        return (String::new(), Vec::new());
    }

    // First token is the command name (stops at first whitespace)
    let (cmd_name, args_str) = match rest.split_once(char::is_whitespace) {
        Some((name, args)) => (name.to_string(), args.trim_start()),
        None => (rest.to_string(), ""),
    };

    let mut args = Vec::new();
    let mut chars = args_str.chars().peekable();
    let mut current = String::new();
    let mut in_double_quote = false;
    let mut in_single_quote = false;
    let mut in_json = false;
    let mut json_depth = 0;

    while let Some(c) = chars.next() {
        match c {
            '"' if !in_single_quote && !in_json => {
                in_double_quote = !in_double_quote;
                current.push(c);
            }
            '\'' if !in_double_quote && !in_json => {
                in_single_quote = !in_single_quote;
                current.push(c);
            }
            '{' | '[' if !in_double_quote && !in_single_quote => {
                in_json = true;
                json_depth += 1;
                current.push(c);
            }
            '}' | ']' if in_json && !in_double_quote && !in_single_quote => {
                json_depth -= 1;
                current.push(c);
                if json_depth == 0 {
                    in_json = false;
                }
            }
            '\\' if in_double_quote => {
                // Handle escaped characters in double quotes
                if let Some(&next_c) = chars.peek() {
                    if next_c == '"' || next_c == '\\' {
                        chars.next(); // consume the escaped char
                        current.push(next_c); // Just push the escaped character (quote or backslash)
                    } else {
                        current.push(c);
                    }
                } else {
                    current.push(c);
                }
            }
            '\\' if in_single_quote => {
                // In single quotes, only \\ and \' are special
                if let Some(&next_c) = chars.peek() {
                    if next_c == '\'' || next_c == '\\' {
                        chars.next();
                        current.push(next_c);
                    } else {
                        current.push(c);
                    }
                } else {
                    current.push(c);
                }
            }
            c if c.is_whitespace() && !in_double_quote && !in_single_quote && !in_json => {
                if !current.is_empty() {
                    args.push(strip_quotes(&current));
                    current = String::new();
                }
                // Skip remaining whitespace
                while chars.peek().is_some_and(|c| c.is_whitespace()) {
                    chars.next();
                }
            }
            _ => {
                current.push(c);
            }
        }
    }

    if !current.is_empty() {
        args.push(strip_quotes(&current));
    }

    (cmd_name, args)
}

/// Strip outer quotes from a string if present.
/// Handles both single and double quotes, preserving escaped quotes inside.
fn strip_quotes(s: &str) -> String {
    let s = s.trim();
    if s.len() >= 2
        && ((s.starts_with('"') && s.ends_with('"')) || (s.starts_with('\'') && s.ends_with('\'')))
    {
        let inner = &s[1..s.len() - 1];
        // Unescape escaped quotes
        return inner.replace("\\\"", "\"").replace("\\'", "'");
    }
    s.to_string()
}
use crate::events::bus::BroadcastEventBus;
use crate::ids::{MissionId, SessionId};
use crate::interaction::action::ApplicationAction;

/// Side-effect classification for slash commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CommandSideEffect {
    /// Pure inspection, no mutations.
    ReadOnly,
    /// Mutates configuration, git repository, or database state.
    Mutating,
    /// Controls session lifecycle (clear, resume).
    SessionControl,
    /// Exits the interactive shell.
    Terminal,
}

/// Execution outcome returned by a slash command handler.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CommandOutput {
    /// Informational text to display to the user.
    Info(String),
    /// Typed application action to be processed by interaction layer.
    ApplicationAction(ApplicationAction),
    /// Structured error message.
    Error(String),
}

impl CommandOutput {
    pub fn info(text: impl Into<String>) -> Self {
        Self::Info(text.into())
    }

    pub fn error(msg: impl Into<String>) -> Self {
        Self::Error(msg.into())
    }
}

/// Execution context passed to slash command handlers.
pub struct CommandContext<'a> {
    pub workspace_root: &'a Path,
    pub session_id: Option<SessionId>,
    pub active_mission_id: Option<MissionId>,
    pub pool: &'a SqlitePool,
    pub event_bus: &'a Arc<BroadcastEventBus>,
    pub configured_model: String,
    pub configured_provider: String,
    pub active_profile: String,
    /// Canonical runtime tool registry for inventory display (`/tools`).
    ///
    /// Display commands MUST inspect this authoritative registry — never
    /// construct a second registry just to display tools (Invariant 2).
    /// `None` (offline/test contexts without a runtime) fails `/tools`
    /// with an explicit error instead of a forked snapshot.
    pub tool_registry: Option<Arc<crate::tools::registry::ToolRegistry>>,
    /// Active slash command registry, if available to this execution context.
    pub command_registry: Option<&'a SlashCommandRegistry>,
}

/// Asynchronous execution handler trait for slash commands.
#[async_trait]
pub trait CommandHandler: Send + Sync {
    async fn execute(
        &self,
        args: &[String],
        ctx: &CommandContext<'_>,
    ) -> Result<CommandOutput, M31AError>;
}

/// Blanket implementation for Arc<T> where T: CommandHandler.
#[async_trait]
impl<T: CommandHandler + ?Sized> CommandHandler for Arc<T> {
    async fn execute(
        &self,
        args: &[String],
        ctx: &CommandContext<'_>,
    ) -> Result<CommandOutput, M31AError> {
        (**self).execute(args, ctx).await
    }
}

/// Strongly typed definition of a slash command.
///
/// Built-in commands use static string references for zero-cost
/// construction. User-defined commands are registered dynamically with
/// owned strings via [`SlashCommand::dynamic`].
pub struct SlashCommand {
    pub name: String,
    pub aliases: Vec<String>,
    pub description: String,
    pub usage: String,
    pub side_effect: CommandSideEffect,
    pub handler: Arc<dyn CommandHandler>,
    /// Whether this is a built-in (static) or user-defined (dynamic) command.
    /// Built-in commands are reserved and cannot be shadowed.
    pub is_builtin: bool,
    /// Detailed help text (e.g. from PromptCommand::describe()), if available.
    pub help_details: Option<String>,
}

impl SlashCommand {
    /// Create a built-in command with static string references (zero-cost).
    pub fn builtin(
        name: &'static str,
        description: &'static str,
        usage: &'static str,
        side_effect: CommandSideEffect,
        handler: impl CommandHandler + 'static,
    ) -> Self {
        Self {
            name: name.to_string(),
            aliases: Vec::new(),
            description: description.to_string(),
            usage: usage.to_string(),
            side_effect,
            handler: Arc::new(handler),
            is_builtin: true,
            help_details: None,
        }
    }

    /// Create a dynamic (user-defined) command with owned strings.
    pub fn dynamic(
        name: String,
        description: String,
        usage: String,
        side_effect: CommandSideEffect,
        handler: impl CommandHandler + 'static,
    ) -> Self {
        Self {
            name,
            aliases: Vec::new(),
            description,
            usage,
            side_effect,
            handler: Arc::new(handler),
            is_builtin: false,
            help_details: None,
        }
    }

    pub fn with_alias(mut self, alias: impl Into<String>) -> Self {
        self.aliases.push(alias.into());
        self
    }

    pub fn with_help_details(mut self, details: impl Into<String>) -> Self {
        self.help_details = Some(details.into());
        self
    }
}

/// Centralized slash command registry.
pub struct SlashCommandRegistry {
    commands: Vec<SlashCommand>,
    lookup: HashMap<String, usize>,
}

impl Default for SlashCommandRegistry {
    fn default() -> Self {
        Self::new_standard()
    }
}

impl SlashCommandRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self {
            commands: Vec::new(),
            lookup: HashMap::new(),
        }
    }

    /// Register a command.
    pub fn register(&mut self, cmd: SlashCommand) {
        let idx = self.commands.len();
        self.lookup.insert(cmd.name.to_lowercase(), idx);
        for alias in &cmd.aliases {
            self.lookup.insert(alias.to_lowercase(), idx);
        }
        self.commands.push(cmd);
    }

    /// Access all registered commands.
    pub fn commands(&self) -> &[SlashCommand] {
        &self.commands
    }

    /// Look up a command by name or alias (with or without leading slash).
    pub fn find(&self, name: &str) -> Option<&SlashCommand> {
        let clean = name.trim_start_matches('/').to_lowercase();
        self.lookup.get(&clean).map(|&idx| &self.commands[idx])
    }

    /// Parses an input line into command name and arguments.
    ///
    /// Supports:
    /// - Quoted strings: `"hello world"` or `'hello world'`
    /// - Escaped quotes inside quotes: `"he said \"hello\""`
    /// - JSON objects/arrays as single arguments: `{"key": "value"}`
    /// - Unquoted words split on whitespace
    pub fn parse_input(&self, input: &str) -> Option<(String, Vec<String>)> {
        let trimmed = input.trim();
        if !trimmed.starts_with('/') {
            return None;
        }

        let (cmd_name, args) = parse_slash_command(trimmed);
        if cmd_name.is_empty() {
            return None;
        }
        Some((cmd_name, args))
    }

    /// Execute a command line string.
    pub async fn execute_line(
        &self,
        input: &str,
        ctx: &CommandContext<'_>,
    ) -> Result<CommandOutput, M31AError> {
        let (cmd_name, args) = match self.parse_input(input) {
            Some(res) => res,
            None => {
                return Ok(CommandOutput::error(
                    "Invalid command format. Commands must begin with '/'",
                ));
            }
        };

        let enriched_ctx = CommandContext {
            workspace_root: ctx.workspace_root,
            session_id: ctx.session_id,
            active_mission_id: ctx.active_mission_id,
            pool: ctx.pool,
            event_bus: ctx.event_bus,
            configured_model: ctx.configured_model.clone(),
            configured_provider: ctx.configured_provider.clone(),
            active_profile: ctx.active_profile.clone(),
            tool_registry: ctx.tool_registry.clone(),
            command_registry: Some(self),
        };

        if let Some(cmd) = self.find(&cmd_name) {
            cmd.handler.execute(&args, &enriched_ctx).await
        } else {
            Ok(CommandOutput::error(format!(
                "Unknown command '/{cmd_name}'. Type /help for available commands."
            )))
        }
    }

    /// Dynamically generate /help text from command metadata.
    pub fn generate_help(&self, command_filter: Option<&str>) -> String {
        if let Some(name) = command_filter {
            if let Some(cmd) = self.find(name) {
                if let Some(details) = &cmd.help_details {
                    return details.clone();
                }
                let aliases_str = if cmd.aliases.is_empty() {
                    "none".to_string()
                } else {
                    cmd.aliases
                        .iter()
                        .map(|a| format!("/{a}"))
                        .collect::<Vec<_>>()
                        .join(", ")
                };
                return format!(
                    "Command: /{name}\nUsage:   {}\nAliases: {}\nEffect:  {:?}\n\n{}",
                    cmd.usage, aliases_str, cmd.side_effect, cmd.description
                );
            } else {
                return format!(
                    "Unknown command '/{name}'. Type /help to see all available commands."
                );
            }
        }

        let mut out = String::from("Available Slash Commands:\n\n");
        for cmd in &self.commands {
            out.push_str(&format!(
                "  {:<16} {:<24} {}\n",
                format!("/{}", cmd.name),
                cmd.usage,
                cmd.description
            ));
        }
        out.push_str("\nType /help <command> for detailed usage.");
        out
    }

    /// Construct standard registry with all 21 mandatory built-in commands.
    ///
    /// This registry is the authoritative source for built-in commands.
    /// User-defined global commands are loaded separately and registered
    /// via [`Self::register_user_commands`].
    pub fn new_standard() -> Self {
        let mut reg = Self::new();

        // 1. /help
        reg.register(
            SlashCommand::builtin(
                "help",
                "Display available commands and detailed usage.",
                "/help [command]",
                CommandSideEffect::ReadOnly,
                HelpHandler,
            )
            .with_alias("?"),
        );

        // 2. /status
        reg.register(
            SlashCommand::builtin(
                "status",
                "Show session, active mission, tasks, model, and git status.",
                "/status",
                CommandSideEffect::ReadOnly,
                StatusHandler,
            )
            .with_alias("st"),
        );

        // 3. /model
        reg.register(SlashCommand::builtin(
            "model",
            "Inspect or change the configured model for this session.",
            "/model [model-name]",
            CommandSideEffect::Mutating,
            ModelHandler,
        ));

        // 4. /profile
        reg.register(SlashCommand::builtin(
            "profile",
            "Inspect or change active autonomy profile (autonomous, guided, safe, coding).",
            "/profile [profile-name]",
            CommandSideEffect::Mutating,
            ProfileHandler,
        ));

        // 5. /config
        reg.register(SlashCommand::builtin(
            "config",
            "Display active configuration, explain keys, or set session overrides.",
            "/config [key] [val]",
            CommandSideEffect::Mutating,
            ConfigHandler,
        ));

        // 6. /tools
        reg.register(SlashCommand::builtin(
            "tools",
            "List all registered tools and their capability requirements.",
            "/tools",
            CommandSideEffect::ReadOnly,
            ToolsHandler,
        ));

        // 7. /skills
        reg.register(SlashCommand::builtin(
            "skills",
            "List all available skills and sub-DAG capabilities.",
            "/skills",
            CommandSideEffect::ReadOnly,
            SkillsHandler,
        ));

        // 8. /diff
        reg.register(
            SlashCommand::builtin(
                "diff",
                "Inspect uncommitted or worktree Git changes safely.",
                "/diff",
                CommandSideEffect::ReadOnly,
                DiffHandler,
            )
            .with_alias("d"),
        );

        // 9. /commit
        reg.register(SlashCommand::builtin(
            "commit",
            "Commit verified changes with M31A attribution trailers.",
            "/commit [message]",
            CommandSideEffect::Mutating,
            CommitHandler,
        ));

        // 10. /cancel
        reg.register(SlashCommand::builtin(
            "cancel",
            "Cancel currently executing task or model request.",
            "/cancel",
            CommandSideEffect::SessionControl,
            CancelHandler,
        ));

        // 11. /resume
        reg.register(SlashCommand::builtin(
            "resume",
            "Resume an existing session or continue current mission.",
            "/resume [session-id]",
            CommandSideEffect::SessionControl,
            ResumeHandler,
        ));

        // 12. /clear
        reg.register(SlashCommand::builtin(
            "clear",
            "Clear active prompt input or terminal screen.",
            "/clear",
            CommandSideEffect::SessionControl,
            ClearHandler,
        ));

        // 13. /clear-session
        reg.register(SlashCommand::builtin(
            "clear-session",
            "Clear durable conversation history for this session.",
            "/clear-session",
            CommandSideEffect::SessionControl,
            ClearSessionHandler,
        ));

        // 14. /exit
        reg.register(
            SlashCommand::builtin(
                "exit",
                "Close interactive session and exit M31A.",
                "/exit",
                CommandSideEffect::Terminal,
                ExitHandler,
            )
            .with_alias("quit"),
        );

        // 15. /genesis
        reg.register(SlashCommand::builtin(
            "genesis",
            "Initialize Project Genesis intake and planning workflow from idea or repository.",
            "/genesis <idea or project prompt>",
            CommandSideEffect::Mutating,
            GenesisHandler,
        ));

        // 16. /roadmap
        reg.register(SlashCommand::builtin(
            "roadmap",
            "Inspect current Project Genesis engineering roadmap and phase status.",
            "/roadmap",
            CommandSideEffect::ReadOnly,
            RoadmapHandler,
        ));

        // 17. /plan
        reg.register(SlashCommand::builtin(
            "plan",
            "Review, edit, revise, regenerate, accept, or reject candidate project plans.",
            "/plan <accept|edit <json>|revise <prompt>|regen|reject [reason]>",
            CommandSideEffect::Mutating,
            PlanCommandHandler,
        ));

        // 18. /tasks
        reg.register(SlashCommand::builtin(
            "tasks",
            "Review, accept, or regenerate candidate task set.",
            "/tasks <accept|regen>",
            CommandSideEffect::Mutating,
            TasksCommandHandler,
        ));

        // 19. /task
        reg.register(SlashCommand::builtin(
            "task",
            "Inspect, edit, add, or remove candidate tasks.",
            "/task <edit <id> <json>|add <json>|remove <id>>",
            CommandSideEffect::Mutating,
            TaskCommandHandler,
        ));

        // 20. /authorize
        reg.register(SlashCommand::builtin(
            "authorize",
            "Explicitly authorize or reject autonomous execution of approved tasks.",
            "/authorize <yes|no> [notes]",
            CommandSideEffect::Mutating,
            AuthorizeCommandHandler,
        ));

        // 21. /workflow
        reg.register(SlashCommand::builtin(
            "workflow",
            "Inspect, pause, resume, approve, or cancel a durable workflow run.",
            "/workflow <inspect|pause|cancel|resume> <run-id> [args]",
            CommandSideEffect::Mutating,
            WorkflowCommandHandler,
        ));

        reg
    }

    /// Register global user-defined commands from the global command directory.
    ///
    /// Built-in commands are RESERVED and cannot be shadowed: if a user command
    /// attempts to register a name or alias that collides with a built-in, it
    /// is rejected with a diagnostic (returned in `rejected`).
    ///
    /// Returns the list of rejected commands (collisions, etc.) so callers can
    /// surface them as warnings without crashing the session.
    pub fn register_user_commands(
        &mut self,
        commands: Vec<crate::interaction::user_commands::PromptCommand>,
        builtin_names: &[&str],
    ) -> Vec<crate::interaction::user_commands::UserCommandRejection> {
        let mut rejected = Vec::new();
        // Precompute built-in name/alias set for O(1) collision detection.
        let mut builtin_set: std::collections::HashSet<String> = builtin_names
            .iter()
            .map(|s| s.trim_start_matches('/').to_lowercase())
            .collect();
        for cmd in &self.commands {
            builtin_set.insert(cmd.name.to_lowercase());
            for alias in &cmd.aliases {
                builtin_set.insert(alias.to_lowercase());
            }
        }
        for k in self.lookup.keys() {
            builtin_set.insert(k.clone());
        }

        let mut user_set: std::collections::HashSet<String> = std::collections::HashSet::new();

        for cmd in commands {
            let cmd_name = cmd.name.trim().trim_start_matches('/').to_lowercase();
            if cmd_name.is_empty() {
                rejected.push(crate::interaction::user_commands::UserCommandRejection {
                    file: cmd.source_path.clone().unwrap_or_default(),
                    reason: "command name cannot be empty".to_string(),
                });
                continue;
            }

            // 1. Collision check against built-ins.
            if builtin_set.contains(&cmd_name) {
                rejected.push(crate::interaction::user_commands::UserCommandRejection {
                    file: cmd.source_path.clone().unwrap_or_default(),
                    reason: format!(
                        "command name '{}' collides with built-in command '/{}'",
                        cmd.name, cmd_name
                    ),
                });
                continue;
            }

            // 2. Collision check against already registered user commands.
            if user_set.contains(&cmd_name) {
                rejected.push(crate::interaction::user_commands::UserCommandRejection {
                    file: cmd.source_path.clone().unwrap_or_default(),
                    reason: format!(
                        "command name '{}' collides with previously registered user command or alias",
                        cmd.name
                    ),
                });
                continue;
            }

            let mut alias_rejected = false;
            let mut normalized_aliases = Vec::new();
            let mut seen_in_this_cmd = std::collections::HashSet::new();

            for alias in &cmd.aliases {
                let alias_norm = alias.trim().trim_start_matches('/').to_lowercase();
                if alias_norm.is_empty() {
                    rejected.push(crate::interaction::user_commands::UserCommandRejection {
                        file: cmd.source_path.clone().unwrap_or_default(),
                        reason: format!("empty alias declared for command '/{}'", cmd.name),
                    });
                    alias_rejected = true;
                    break;
                }
                if alias_norm == cmd_name {
                    rejected.push(crate::interaction::user_commands::UserCommandRejection {
                        file: cmd.source_path.clone().unwrap_or_default(),
                        reason: format!(
                            "alias '/{}' is identical to command name '/{}'",
                            alias, cmd.name
                        ),
                    });
                    alias_rejected = true;
                    break;
                }
                if !seen_in_this_cmd.insert(alias_norm.clone()) {
                    rejected.push(crate::interaction::user_commands::UserCommandRejection {
                        file: cmd.source_path.clone().unwrap_or_default(),
                        reason: format!(
                            "duplicate alias '/{}' declared for command '/{}'",
                            alias, cmd.name
                        ),
                    });
                    alias_rejected = true;
                    break;
                }
                if builtin_set.contains(&alias_norm) {
                    rejected.push(crate::interaction::user_commands::UserCommandRejection {
                        file: cmd.source_path.clone().unwrap_or_default(),
                        reason: format!(
                            "alias '/{}' collides with built-in command '/{}'",
                            alias, alias_norm
                        ),
                    });
                    alias_rejected = true;
                    break;
                }
                if user_set.contains(&alias_norm) {
                    rejected.push(crate::interaction::user_commands::UserCommandRejection {
                        file: cmd.source_path.clone().unwrap_or_default(),
                        reason: format!(
                            "alias '/{}' collides with previously registered user command or alias",
                            alias
                        ),
                    });
                    alias_rejected = true;
                    break;
                }
                normalized_aliases.push(alias_norm);
            }

            if alias_rejected {
                continue;
            }

            // Create command-specific handler holding Arc<PromptCommand>
            let cmd_arc = Arc::new(cmd.clone());
            let handler = crate::interaction::user_commands::PromptCommandHandler::new(cmd_arc);

            let mut dynamic_cmd = SlashCommand::dynamic(
                cmd.name.clone(),
                cmd.description.clone(),
                if cmd.usage.trim().is_empty() {
                    cmd.generated_usage()
                } else {
                    cmd.usage.clone()
                },
                cmd.side_effect,
                handler,
            )
            .with_help_details(cmd.describe());

            for alias in &cmd.aliases {
                dynamic_cmd = dynamic_cmd.with_alias(alias.clone());
            }

            // Register the command and its aliases.
            self.register(dynamic_cmd);

            // Track names so later user commands don't collide with earlier ones.
            user_set.insert(cmd_name);
            for a in normalized_aliases {
                user_set.insert(a);
            }
        }
        rejected
    }
}

// ===========================================================================
// Command Handlers
// ===========================================================================

struct HelpHandler;
#[async_trait]
impl CommandHandler for HelpHandler {
    async fn execute(
        &self,
        args: &[String],
        ctx: &CommandContext<'_>,
    ) -> Result<CommandOutput, M31AError> {
        let filter = args.first().map(|s| s.as_str());
        if let Some(registry) = ctx.command_registry {
            Ok(CommandOutput::info(registry.generate_help(filter)))
        } else {
            let registry = SlashCommandRegistry::new_standard();
            Ok(CommandOutput::info(registry.generate_help(filter)))
        }
    }
}

struct StatusHandler;
#[async_trait]
impl CommandHandler for StatusHandler {
    async fn execute(
        &self,
        _args: &[String],
        ctx: &CommandContext<'_>,
    ) -> Result<CommandOutput, M31AError> {
        let session_str = ctx
            .session_id
            .map(|id| id.to_string())
            .unwrap_or_else(|| "none (ephemeral)".to_string());

        let (mission_str, mission_status, mission_obj) = if let Some(mid) = ctx.active_mission_id {
            let mission_repo =
                crate::persistence::sqlite::repositories::SqliteMissionRepository::new(
                    ctx.pool.clone(),
                );
            match crate::persistence::sqlite::repositories::MissionRepository::get(
                &mission_repo,
                mid,
            )
            .await
            .unwrap_or(None)
            {
                Some(m) => (mid.to_string(), m.status.to_string(), m.objective),
                None => (mid.to_string(), "Unknown".to_string(), "".to_string()),
            }
        } else {
            ("none".to_string(), "none".to_string(), "".to_string())
        };

        let tasks_count = if let Some(mid) = ctx.active_mission_id {
            let task_repo = crate::persistence::sqlite::repositories::SqliteTaskRepository::new(
                ctx.pool.clone(),
            );
            task_repo.count_by_mission(mid).await.unwrap_or(0) as i64
        } else {
            0
        };

        let tasks_done = if let Some(mid) = ctx.active_mission_id {
            let task_repo = crate::persistence::sqlite::repositories::SqliteTaskRepository::new(
                ctx.pool.clone(),
            );
            task_repo.count_completed_by_mission(mid).await.unwrap_or(0) as i64
        } else {
            0
        };

        let git_status = if ctx.workspace_root.join(".git").exists() {
            let git = crate::capability::providers::CliGitProvider::new(ctx.workspace_root);
            match git.status_porcelain().await {
                Ok(s) => {
                    let trimmed = s.trim();
                    if trimmed.is_empty() {
                        "clean".to_string()
                    } else {
                        format!("{} modified file(s)", trimmed.lines().count())
                    }
                }
                Err(_) => "git status error".to_string(),
            }
        } else {
            "not a git repository".to_string()
        };

        let info = format!(
            "Session:    {}\n\
             Workspace:  {}\n\
             Model:      {} ({})\n\
             Profile:    {}\n\
             Mission:    {} [{}]\n\
             Objective:  {}\n\
             Tasks:      {}/{} completed\n\
             Git State:  {}",
            session_str,
            ctx.workspace_root.display(),
            ctx.configured_model,
            ctx.configured_provider,
            ctx.active_profile,
            mission_str,
            mission_status,
            if mission_obj.is_empty() {
                "none"
            } else {
                &mission_obj
            },
            tasks_done,
            tasks_count,
            git_status
        );

        Ok(CommandOutput::info(info))
    }
}

struct ModelHandler;
#[async_trait]
impl CommandHandler for ModelHandler {
    async fn execute(
        &self,
        args: &[String],
        ctx: &CommandContext<'_>,
    ) -> Result<CommandOutput, M31AError> {
        if let Some(arg) = args.first() {
            let trimmed = arg.trim();
            if trimmed.eq_ignore_ascii_case("search") {
                let query = args.get(1).map(|s| s.trim()).unwrap_or("");
                if query.is_empty() {
                    return Ok(CommandOutput::error("Usage: /model search <query>"));
                }
                let cache_path = crate::model::catalog::ModelCatalog::cache_path_for_channel(
                    ctx.workspace_root,
                    crate::deployment::DeploymentChannel::current(),
                );
                let catalog =
                    crate::model::catalog::ModelCatalog::load_from_cache_file(&cache_path).ok();
                if let Some(cat) = catalog {
                    let query_lower = query.to_lowercase();
                    let matches: Vec<_> = cat
                        .models
                        .iter()
                        .filter(|m| {
                            m.model_id.to_lowercase().contains(&query_lower)
                                || m.display_name
                                    .as_deref()
                                    .unwrap_or("")
                                    .to_lowercase()
                                    .contains(&query_lower)
                        })
                        .collect();
                    if matches.is_empty() {
                        return Ok(CommandOutput::info(format!(
                            "No models matching '{query}' found in discovered catalog (total {} models).",
                            cat.len()
                        )));
                    }
                    let mut out = format!(
                        "Search results for '{query}' ({} models found):\n",
                        matches.len()
                    );
                    for m in matches {
                        out.push_str(&format!(
                            "  - {:<42} [tier: {:<9}, ctx: {:>4}k, tools: {}, status: {}]\n",
                            m.model_id,
                            m.tier,
                            m.context_capacity / 1024,
                            if m.supports_tools { "yes" } else { "no" },
                            m.availability
                        ));
                    }
                    out.push_str("\nUse `/model <name>` to select a model.");
                    return Ok(CommandOutput::info(out));
                } else {
                    return Ok(CommandOutput::error(
                        "Model catalog not initialized. Run discovery or doctor to probe catalog.",
                    ));
                }
            }

            if trimmed.eq_ignore_ascii_case("list") {
                let filter_tier = args.get(1).map(|s| s.trim().to_lowercase());
                let cache_path = crate::model::catalog::ModelCatalog::cache_path_for_channel(
                    ctx.workspace_root,
                    crate::deployment::DeploymentChannel::current(),
                );
                let catalog =
                    crate::model::catalog::ModelCatalog::load_from_cache_file(&cache_path).ok();
                if let Some(cat) = catalog {
                    let models: Vec<_> = if let Some(ref t) = filter_tier {
                        cat.models
                            .iter()
                            .filter(|m| m.tier.to_string().to_lowercase() == *t)
                            .collect()
                    } else {
                        cat.models.iter().collect()
                    };
                    let mut out = format!("Discovered models ({} total):\n", models.len());
                    for m in models {
                        out.push_str(&format!(
                            "  - {:<42} [tier: {:<9}, ctx: {:>4}k, tools: {}, status: {}]\n",
                            m.model_id,
                            m.tier,
                            m.context_capacity / 1024,
                            if m.supports_tools { "yes" } else { "no" },
                            m.availability
                        ));
                    }
                    out.push_str("\nUse `/model <name>` to select a model.");
                    return Ok(CommandOutput::info(out));
                } else {
                    return Ok(CommandOutput::error(
                        "Model catalog not initialized. Run discovery or doctor to probe catalog.",
                    ));
                }
            }

            let model_name = trimmed;
            if model_name.is_empty() {
                return Ok(CommandOutput::error("Model name cannot be empty."));
            }

            if model_name.contains("..")
                || model_name.starts_with('/')
                || model_name.ends_with('/')
                || !model_name
                    .chars()
                    .all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == '.' || c == '/')
            {
                return Ok(CommandOutput::error(format!(
                    "Invalid model identifier '{model_name}'. Model names may only contain letters, numbers, '-', '_', '.', and '/' and cannot contain path traversal."
                )));
            }

            // NVIDIA NIM is the production model provider in this release.
            // A bare NVIDIA-hosted model ID (`meta/llama-...`) or an explicit
            // `nvidia/` qualifier is valid; retired provider qualifiers are
            // rejected deterministically and never become active.
            if let Some((prefix, _)) = model_name.split_once('/') {
                let p_lower = prefix.to_lowercase();
                if p_lower == "nvidia" || p_lower == "nvidia_nim" {
                    // Valid: normalized by ResolvedConfiguration::with_session_model.
                } else if crate::config::provider_registry::is_retired_provider(&p_lower) {
                    return Ok(CommandOutput::error(format!(
                        "Unsupported model provider '{prefix}'. {}",
                        crate::config::provider_registry::NVIDIA_ONLY_ERROR
                    )));
                } else if p_lower == "mock" {
                    return Ok(CommandOutput::error(
                        "Mock provider is test-only and cannot be selected in normal interaction. Only NVIDIA NIM models are supported in this release.",
                    ));
                } else {
                    let known_nim_publishers = [
                        "meta",
                        "mistralai",
                        "google",
                        "deepseek-ai",
                        "deepseek",
                        "qwen",
                        "snowflake",
                        "01-ai",
                        "baichuan-inc",
                        "microsoft",
                    ];
                    if !known_nim_publishers.contains(&p_lower.as_str()) {
                        return Ok(CommandOutput::error(format!(
                            "Unsupported model provider '{prefix}'. {}",
                            crate::config::provider_registry::NVIDIA_ONLY_ERROR
                        )));
                    }
                }
            }

            Ok(CommandOutput::ApplicationAction(
                ApplicationAction::ModelChangeRequested {
                    model: model_name.to_string(),
                },
            ))
        } else {
            // Status reflects the channel-aware credential store (same
            // file+env precedence as the runtime authority), not ambient
            // environment alone.
            let mut registry = crate::config::provider_registry::ProviderRegistry::new();
            let creds_path =
                crate::config::provider_registry::ProviderRegistry::channel_credentials_path(
                    ctx.workspace_root,
                );
            if creds_path.is_file() {
                let _ = registry.load_credentials_from_file(&creds_path);
            }
            let status = registry.get_status(&ctx.configured_provider);
            let status_note = match status {
                crate::model::types::ProviderCapabilityStatus::Available => {
                    "Operational (NVIDIA NIM production endpoint ready)"
                }
                crate::model::types::ProviderCapabilityStatus::Unavailable => {
                    "Unavailable (Deferred in v1; only NVIDIA NIM is production-supported)"
                }
                crate::model::types::ProviderCapabilityStatus::Misconfigured => {
                    "Misconfigured (Missing or invalid API credentials)"
                }
                crate::model::types::ProviderCapabilityStatus::MockOnly => {
                    "Mock / Test-Only (Offline simulation, not for production)"
                }
                crate::model::types::ProviderCapabilityStatus::Unknown => {
                    "Unknown (Provider not recognized in architecture)"
                }
            };

            let cache_path = crate::model::catalog::ModelCatalog::cache_path_for_channel(
                ctx.workspace_root,
                crate::deployment::DeploymentChannel::current(),
            );
            let catalog =
                crate::model::catalog::ModelCatalog::load_from_cache_file(&cache_path).ok();

            let mut info = format!(
                "Configured model:    {}\nProvider:            {}\nStatus:              {} ({})\n",
                ctx.configured_model, ctx.configured_provider, status, status_note
            );

            if let Some(cat) = catalog {
                let freshness =
                    if cat.is_stale(crate::model::catalog::ModelCatalog::DEFAULT_MAX_AGE_SECS) {
                        "Cached (STALE)"
                    } else {
                        "Current (discovered)"
                    };
                let std_count = cat
                    .models_for_tier(crate::model::router::resolver::ModelTier::Standard)
                    .len();
                let fast_count = cat
                    .models_for_tier(crate::model::router::resolver::ModelTier::Fast)
                    .len();
                let reasoning_count = cat
                    .models_for_tier(crate::model::router::resolver::ModelTier::Reasoning)
                    .len();
                info.push_str(&format!(
                    "Catalog freshness:   {} ({} models discovered: {} standard, {} fast, {} reasoning)\n",
                    freshness,
                    cat.len(),
                    std_count,
                    fast_count,
                    reasoning_count
                ));
                if !cat.models.is_empty() {
                    info.push_str("Representative models:\n");
                    for m in cat.models.iter().take(10) {
                        info.push_str(&format!(
                            "  - {:<42} [tier: {:<9}, ctx: {:>4}k, tools: {}, status: {}]\n",
                            m.model_id,
                            m.tier,
                            m.context_capacity / 1024,
                            if m.supports_tools { "yes" } else { "no" },
                            m.availability
                        ));
                    }
                    if cat.models.len() > 10 {
                        info.push_str(&format!(
                            "  ... (use `/model list` to view all {} models or `/model search <query>`)\n",
                            cat.models.len()
                        ));
                    }
                }
            } else {
                info.push_str("Catalog freshness:   Uninitialized (run discovery or doctor to probe catalog)\n");
            }
            info.push_str("\nCommands:\n  /model <name>          Switch active model for this session\n  /model list [tier]     List all discovered models (optional: standard, fast, reasoning)\n  /model search <query>  Search models by name or publisher\n");

            Ok(CommandOutput::info(info))
        }
    }
}

struct ProfileHandler;
#[async_trait]
impl CommandHandler for ProfileHandler {
    async fn execute(
        &self,
        args: &[String],
        ctx: &CommandContext<'_>,
    ) -> Result<CommandOutput, M31AError> {
        if let Some(new_profile) = args.first() {
            let p_name = new_profile.trim().to_lowercase();
            let valid_profiles = [
                "autonomous",
                "assisted",
                "guided",
                "safe",
                "plan",
                "coding",
                "ci",
                "unattended",
            ];
            if !valid_profiles.contains(&p_name.as_str()) {
                return Ok(CommandOutput::error(format!(
                    "Unknown profile '{}'. Available profiles: {}",
                    p_name,
                    valid_profiles.join(", ")
                )));
            }
            Ok(CommandOutput::ApplicationAction(
                ApplicationAction::ProfileChangeRequested { profile: p_name },
            ))
        } else {
            Ok(CommandOutput::info(format!(
                "Active profile: {}\nAvailable: autonomous, assisted, guided, safe, plan, coding, ci, unattended\nUse `/profile <name>` to switch profile for this session.",
                ctx.active_profile
            )))
        }
    }
}

struct ConfigHandler;
#[async_trait]
impl CommandHandler for ConfigHandler {
    async fn execute(
        &self,
        args: &[String],
        ctx: &CommandContext<'_>,
    ) -> Result<CommandOutput, M31AError> {
        let resolved = crate::config::ResolvedConfiguration::for_workspace(ctx.workspace_root)
            .unwrap_or_else(|_| {
                crate::config::ResolvedConfigBuilder::new(ctx.workspace_root).build_fallback()
            });

        match args.len() {
            0 => {
                let storage_dir = ctx.workspace_root.join(".m31a");
                let db_path = storage_dir.join("m31a.db");
                let artifacts_dir = storage_dir.join("artifacts");

                let out = format!(
                    "Workspace:   {}\n\
                     Storage:     {}\n\
                     Database:    {}\n\
                     Artifacts:   {}\n\
                     Model:       {}\n\
                     Provider:    {}\n\
                     Profile:     {}\n\
                     Sources ({}):\n{}",
                    ctx.workspace_root.display(),
                    storage_dir.display(),
                    db_path.display(),
                    artifacts_dir.display(),
                    resolved.active_model,
                    resolved.active_provider,
                    resolved.active_profile.as_deref().unwrap_or("none"),
                    resolved.loaded_sources.len(),
                    resolved.sources().join("\n")
                );
                Ok(CommandOutput::info(out))
            }
            1 => {
                let key = &args[0];
                if let Some(explain) = resolved.explain(key) {
                    let out = format!(
                        "Key:           {}\n\
                         Resolved:      {}\n\
                         Winning Layer: {}\n\
                         Locked:        {}\n\
                         Source File:   {}",
                        explain.key,
                        explain.resolved_value,
                        explain.winning_tier,
                        explain.is_immutable,
                        explain
                            .source_file
                            .map(|p| p.display().to_string())
                            .unwrap_or_else(|| "built-in".to_string())
                    );
                    Ok(CommandOutput::info(out))
                } else {
                    Ok(CommandOutput::error(format!(
                        "Configuration key '{}' not found.",
                        key
                    )))
                }
            }
            _ => {
                let key = args[0].clone();
                let val = args[1..].join(" ");
                Ok(CommandOutput::ApplicationAction(
                    ApplicationAction::ConfigOverrideRequested { key, value: val },
                ))
            }
        }
    }
}

struct ToolsHandler;
#[async_trait]
impl CommandHandler for ToolsHandler {
    async fn execute(
        &self,
        _args: &[String],
        ctx: &CommandContext<'_>,
    ) -> Result<CommandOutput, M31AError> {
        // Canonical source: the runtime-shared ToolRegistry carried by the
        // command context. A display command inspects the authoritative
        // registry; it MUST NOT construct a second registry (Invariant 2).
        let registry = ctx.tool_registry.clone().ok_or_else(|| {
            M31AError::internal(
                "tool inventory unavailable: no runtime tool registry attached to command context",
            )
        })?;
        let mut out = String::from("Registered Tools:\n\n");
        let mut tools = registry.list_tools();
        tools.sort_by(|a, b| a.id().cmp(b.id()));
        for tool in &tools {
            out.push_str(&format!("  {:<16} {}\n", tool.id(), tool.description()));
        }
        if tools.is_empty() {
            return Ok(CommandOutput::error(
                "ToolRegistry is empty: no tools registered for this workspace",
            ));
        }
        Ok(CommandOutput::info(out))
    }
}

struct SkillsHandler;
#[async_trait]
impl CommandHandler for SkillsHandler {
    async fn execute(
        &self,
        _args: &[String],
        ctx: &CommandContext<'_>,
    ) -> Result<CommandOutput, M31AError> {
        // Canonical source: SkillRegistry via SkillDiscovery. No hardcoded
        // inventory — discovery is the single authority.
        let registry =
            crate::skill::registry::SkillRegistry::load_discovered(Some(ctx.workspace_root));
        let mut out = String::from("Available Skills:\n\n");
        match registry {
            Ok(registry) => {
                let mut skills = registry.list();
                skills.sort_by(|a, b| a.manifest.id.cmp(&b.manifest.id));
                for s in &skills {
                    out.push_str(&format!(
                        "  {:<16} {}\n",
                        s.manifest.id, s.manifest.description
                    ));
                }
                if skills.is_empty() {
                    out.push_str("  (no skills discovered in this workspace)\n");
                }
                Ok(CommandOutput::info(out))
            }
            Err(e) => Ok(CommandOutput::error(format!("Skill discovery failed: {e}"))),
        }
    }
}

struct DiffHandler;
#[async_trait]
impl CommandHandler for DiffHandler {
    async fn execute(
        &self,
        _args: &[String],
        _ctx: &CommandContext<'_>,
    ) -> Result<CommandOutput, M31AError> {
        Ok(CommandOutput::ApplicationAction(
            ApplicationAction::DiffRequested,
        ))
    }
}

struct CommitHandler;
#[async_trait]
impl CommandHandler for CommitHandler {
    async fn execute(
        &self,
        args: &[String],
        _ctx: &CommandContext<'_>,
    ) -> Result<CommandOutput, M31AError> {
        let msg = if args.is_empty() {
            None
        } else {
            Some(args.join(" "))
        };
        Ok(CommandOutput::ApplicationAction(
            ApplicationAction::CommitRequested { message: msg },
        ))
    }
}

struct CancelHandler;
#[async_trait]
impl CommandHandler for CancelHandler {
    async fn execute(
        &self,
        _args: &[String],
        _ctx: &CommandContext<'_>,
    ) -> Result<CommandOutput, M31AError> {
        Ok(CommandOutput::ApplicationAction(
            ApplicationAction::CancelRequested,
        ))
    }
}

struct ResumeHandler;
#[async_trait]
impl CommandHandler for ResumeHandler {
    async fn execute(
        &self,
        args: &[String],
        ctx: &CommandContext<'_>,
    ) -> Result<CommandOutput, M31AError> {
        let sid = if let Some(arg) = args.first() {
            arg.clone()
        } else if let Some(sid) = ctx.session_id {
            sid.to_string()
        } else {
            return Ok(CommandOutput::error(
                "No session ID specified. Usage: /resume <session-id>",
            ));
        };
        Ok(CommandOutput::ApplicationAction(
            ApplicationAction::SessionResumeRequested { session_id: sid },
        ))
    }
}

struct ClearHandler;
#[async_trait]
impl CommandHandler for ClearHandler {
    async fn execute(
        &self,
        _args: &[String],
        _ctx: &CommandContext<'_>,
    ) -> Result<CommandOutput, M31AError> {
        Ok(CommandOutput::ApplicationAction(
            ApplicationAction::ClearRequested,
        ))
    }
}

struct ClearSessionHandler;
#[async_trait]
impl CommandHandler for ClearSessionHandler {
    async fn execute(
        &self,
        _args: &[String],
        _ctx: &CommandContext<'_>,
    ) -> Result<CommandOutput, M31AError> {
        Ok(CommandOutput::ApplicationAction(
            ApplicationAction::ClearSessionRequested,
        ))
    }
}

struct ExitHandler;
#[async_trait]
impl CommandHandler for ExitHandler {
    async fn execute(
        &self,
        _args: &[String],
        _ctx: &CommandContext<'_>,
    ) -> Result<CommandOutput, M31AError> {
        Ok(CommandOutput::ApplicationAction(
            ApplicationAction::ExitRequested,
        ))
    }
}

struct GenesisHandler;
#[async_trait]
impl CommandHandler for GenesisHandler {
    async fn execute(
        &self,
        args: &[String],
        _ctx: &CommandContext<'_>,
    ) -> Result<CommandOutput, M31AError> {
        if args.is_empty() {
            return Ok(CommandOutput::error(
                "Usage: /genesis <project concept or feature request>",
            ));
        }
        let prompt = args.join(" ");
        Ok(CommandOutput::ApplicationAction(
            ApplicationAction::GenesisRequested { prompt, mode: None },
        ))
    }
}

struct RoadmapHandler;
#[async_trait]
impl CommandHandler for RoadmapHandler {
    async fn execute(
        &self,
        _args: &[String],
        ctx: &CommandContext<'_>,
    ) -> Result<CommandOutput, M31AError> {
        let roadmap_path = ctx.workspace_root.join(".planning").join("ROADMAP.md");
        if roadmap_path.exists()
            && let Ok(content) = tokio::fs::read_to_string(&roadmap_path).await
        {
            return Ok(CommandOutput::info(content));
        }
        Ok(CommandOutput::ApplicationAction(
            ApplicationAction::GenesisStateRequested,
        ))
    }
}

struct PlanCommandHandler;
#[async_trait]
impl CommandHandler for PlanCommandHandler {
    async fn execute(
        &self,
        args: &[String],
        ctx: &CommandContext<'_>,
    ) -> Result<CommandOutput, M31AError> {
        let sid = ctx.session_id.map(|s| s.to_string());
        let subcmd = args.first().map(|s| s.to_lowercase()).unwrap_or_default();
        match subcmd.as_str() {
            "accept" => Ok(CommandOutput::ApplicationAction(
                ApplicationAction::PlanAcceptRequested { session_id: sid },
            )),
            "edit" => {
                let json = args[1..].join(" ");
                if json.trim().is_empty() {
                    return Ok(CommandOutput::error(
                        "Usage: /plan edit <json-plan-content>",
                    ));
                }
                Ok(CommandOutput::ApplicationAction(
                    ApplicationAction::PlanEditRequested {
                        session_id: sid,
                        plan_json: json,
                    },
                ))
            }
            "revise" => {
                let feedback = args[1..].join(" ");
                if feedback.trim().is_empty() {
                    return Ok(CommandOutput::error("Usage: /plan revise <feedback-text>"));
                }
                Ok(CommandOutput::ApplicationAction(
                    ApplicationAction::PlanRevisionRequested {
                        session_id: sid,
                        feedback,
                    },
                ))
            }
            "regen" | "regenerate" => Ok(CommandOutput::ApplicationAction(
                ApplicationAction::PlanRegenerateRequested { session_id: sid },
            )),
            "reject" => {
                let reason = args[1..].join(" ");
                let r = if reason.trim().is_empty() {
                    "Operator rejected plan".to_string()
                } else {
                    reason
                };
                Ok(CommandOutput::ApplicationAction(
                    ApplicationAction::PlanRejectRequested {
                        session_id: sid,
                        reason: r,
                    },
                ))
            }
            _ => Ok(CommandOutput::info(
                "Usage: /plan <accept|edit <json>|revise <feedback>|regen|reject [reason]>",
            )),
        }
    }
}

struct TasksCommandHandler;
#[async_trait]
impl CommandHandler for TasksCommandHandler {
    async fn execute(
        &self,
        args: &[String],
        ctx: &CommandContext<'_>,
    ) -> Result<CommandOutput, M31AError> {
        let sid = ctx.session_id.map(|s| s.to_string());
        let subcmd = args.first().map(|s| s.to_lowercase()).unwrap_or_default();
        match subcmd.as_str() {
            "accept" => Ok(CommandOutput::ApplicationAction(
                ApplicationAction::TasksAcceptRequested { session_id: sid },
            )),
            "regen" | "regenerate" => {
                let feedback = if args.len() > 1 {
                    Some(args[1..].join(" "))
                } else {
                    None
                };
                Ok(CommandOutput::ApplicationAction(
                    ApplicationAction::TaskRegenerateRequested {
                        session_id: sid,
                        feedback,
                    },
                ))
            }
            _ => Ok(CommandOutput::info(
                "Usage: /tasks <accept|regen [feedback]>",
            )),
        }
    }
}

struct TaskCommandHandler;
#[async_trait]
impl CommandHandler for TaskCommandHandler {
    async fn execute(
        &self,
        args: &[String],
        ctx: &CommandContext<'_>,
    ) -> Result<CommandOutput, M31AError> {
        let sid = ctx.session_id.map(|s| s.to_string());
        let subcmd = args.first().map(|s| s.to_lowercase()).unwrap_or_default();
        match subcmd.as_str() {
            "edit" => {
                let json = args[1..].join(" ");
                if json.trim().is_empty() {
                    return Ok(CommandOutput::error(
                        "Usage: /task edit <json-task-content>",
                    ));
                }
                Ok(CommandOutput::ApplicationAction(
                    ApplicationAction::TaskEditRequested {
                        session_id: sid,
                        task_json: json,
                    },
                ))
            }
            "add" => {
                let json = args[1..].join(" ");
                if json.trim().is_empty() {
                    return Ok(CommandOutput::error("Usage: /task add <json-task-content>"));
                }
                Ok(CommandOutput::ApplicationAction(
                    ApplicationAction::TaskAddRequested {
                        session_id: sid,
                        task_json: json,
                    },
                ))
            }
            "remove" | "rm" => {
                let task_id = args.get(1).cloned().unwrap_or_default();
                if task_id.is_empty() {
                    return Ok(CommandOutput::error("Usage: /task remove <task-id>"));
                }
                Ok(CommandOutput::ApplicationAction(
                    ApplicationAction::TaskRemoveRequested {
                        session_id: sid,
                        task_id,
                    },
                ))
            }
            _ => Ok(CommandOutput::info(
                "Usage: /task <edit <json>|add <json>|remove <id>>",
            )),
        }
    }
}

struct AuthorizeCommandHandler;
#[async_trait]
impl CommandHandler for AuthorizeCommandHandler {
    async fn execute(
        &self,
        args: &[String],
        ctx: &CommandContext<'_>,
    ) -> Result<CommandOutput, M31AError> {
        let sid = ctx.session_id.map(|s| s.to_string());
        let subcmd = args.first().map(|s| s.to_lowercase()).unwrap_or_default();
        match subcmd.as_str() {
            "yes" | "y" | "approve" => Ok(CommandOutput::ApplicationAction(
                ApplicationAction::ExecutionAuthorizationSubmitted {
                    session_id: sid,
                    decision: true,
                    reason: None,
                },
            )),
            "no" | "n" | "reject" | "deny" => {
                let reason = if args.len() > 1 {
                    Some(args[1..].join(" "))
                } else {
                    None
                };
                Ok(CommandOutput::ApplicationAction(
                    ApplicationAction::ExecutionAuthorizationSubmitted {
                        session_id: sid,
                        decision: false,
                        reason,
                    },
                ))
            }
            _ => Ok(CommandOutput::info("Usage: /authorize <yes|no> [reason]")),
        }
    }
}

struct WorkflowCommandHandler;
#[async_trait]
impl CommandHandler for WorkflowCommandHandler {
    async fn execute(
        &self,
        args: &[String],
        _ctx: &CommandContext<'_>,
    ) -> Result<CommandOutput, M31AError> {
        // Route every workflow mutation through the single canonical owner
        // (AppRuntime::*_workflow_run). This handler only builds the typed
        // action; the runtime decides.
        let subcmd = args.first().map(|s| s.to_lowercase()).unwrap_or_default();
        match subcmd.as_str() {
            "inspect" | "show" | "status" => {
                let run_id = args.get(1).cloned().unwrap_or_default();
                if run_id.trim().is_empty() {
                    return Ok(CommandOutput::error("Usage: /workflow inspect <run-id>"));
                }
                Ok(CommandOutput::ApplicationAction(
                    ApplicationAction::WorkflowInspectRequested { run_id },
                ))
            }
            "pause" => {
                let run_id = args.get(1).cloned().unwrap_or_default();
                if run_id.trim().is_empty() {
                    return Ok(CommandOutput::error(
                        "Usage: /workflow pause <run-id> [reason]",
                    ));
                }
                let reason = if args.len() > 2 {
                    args[2..].join(" ")
                } else {
                    "paused by operator".to_string()
                };
                Ok(CommandOutput::ApplicationAction(
                    ApplicationAction::WorkflowPauseRequested { run_id, reason },
                ))
            }
            "cancel" => {
                let run_id = args.get(1).cloned().unwrap_or_default();
                if run_id.trim().is_empty() {
                    return Ok(CommandOutput::error(
                        "Usage: /workflow cancel <run-id> [reason]",
                    ));
                }
                let reason = if args.len() > 2 {
                    args[2..].join(" ")
                } else {
                    "cancelled by operator".to_string()
                };
                Ok(CommandOutput::ApplicationAction(
                    ApplicationAction::WorkflowCancelRequested { run_id, reason },
                ))
            }
            "resume" => {
                let run_id = args.get(1).cloned().unwrap_or_default();
                if run_id.trim().is_empty() {
                    return Ok(CommandOutput::error("Usage: /workflow resume <run-id>"));
                }
                Ok(CommandOutput::ApplicationAction(
                    ApplicationAction::WorkflowResumeRequested { run_id },
                ))
            }
            "approve" => {
                if args.len() < 3 {
                    return Ok(CommandOutput::error(
                        "Usage: /workflow approve <run-id> <step-key> [reason]",
                    ));
                }
                Ok(CommandOutput::ApplicationAction(
                    ApplicationAction::WorkflowApprovalSubmitted {
                        run_id: args[1].clone(),
                        step_key: args[2].clone(),
                        approved: true,
                        reason: if args.len() > 3 {
                            Some(args[3..].join(" "))
                        } else {
                            None
                        },
                    },
                ))
            }
            _ => Ok(CommandOutput::info(
                "Usage: /workflow <inspect|pause|cancel|resume|approve> <run-id> [args]",
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_slash_command_discovery_and_help() {
        let reg = SlashCommandRegistry::new_standard();
        assert!(reg.find("help").is_some());
        assert!(reg.find("diff").is_some());
        assert!(reg.find("status").is_some());
        assert!(reg.find("exit").is_some());
        assert!(reg.find("quit").is_some()); // alias

        let general_help = reg.generate_help(None);
        assert!(general_help.contains("/help"));
        assert!(general_help.contains("/diff"));
        assert!(general_help.contains("/status"));

        let diff_help = reg.generate_help(Some("diff"));
        assert!(diff_help.contains("Usage:   /diff"));
        assert!(diff_help.contains("Inspect uncommitted"));
    }

    #[tokio::test]
    async fn test_unknown_command_produces_structured_error() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("cmd_test.db");
        let pool = crate::persistence::sqlite::schema::initialize_database(&db_path)
            .await
            .unwrap();
        let bus = Arc::new(BroadcastEventBus::new(16));

        let reg = SlashCommandRegistry::new_standard();
        let ctx = CommandContext {
            workspace_root: dir.path(),
            session_id: None,
            active_mission_id: None,
            pool: &pool,
            event_bus: &bus,
            configured_model: "test-model".to_string(),
            configured_provider: "test-provider".to_string(),
            active_profile: "autonomous".to_string(),
            tool_registry: None,
            command_registry: None,
        };

        let result = reg.execute_line("/nonexistent_foo", &ctx).await.unwrap();
        match result {
            CommandOutput::Error(msg) => {
                assert!(msg.contains("Unknown command '/nonexistent_foo'"));
            }
            other => panic!("expected CommandOutput::Error, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_command_argument_parsing() {
        let reg = SlashCommandRegistry::new_standard();
        let (cmd, args) = reg.parse_input("/model meta/llama-3.2-11b").unwrap();
        assert_eq!(cmd, "model");
        assert_eq!(args, vec!["meta/llama-3.2-11b"]);

        let (cmd2, args2) = reg.parse_input("/commit fix parser bug").unwrap();
        assert_eq!(cmd2, "commit");
        assert_eq!(args2, vec!["fix", "parser", "bug"]);
    }

    #[test]
    fn test_parse_slash_command_quoted_strings() {
        // Double quoted string
        let (cmd, args) = parse_slash_command("/plan revise \"make this more detailed\"");
        assert_eq!(cmd, "plan");
        assert_eq!(args, vec!["revise", "make this more detailed"]);

        // Single quoted string
        let (cmd, args) = parse_slash_command("/plan revise 'make this more detailed'");
        assert_eq!(cmd, "plan");
        assert_eq!(args, vec!["revise", "make this more detailed"]);

        // Escaped quotes inside double quotes
        let (cmd, args) = parse_slash_command(r#"/plan revise "he said \"hello\"""#);
        assert_eq!(cmd, "plan");
        assert_eq!(args, vec!["revise", "he said \"hello\""]);

        // Multiple quoted arguments
        let (cmd, args) = parse_slash_command("/task add \"Task 1\" \"Task 2\"");
        assert_eq!(cmd, "task");
        assert_eq!(args, vec!["add", "Task 1", "Task 2"]);
    }

    #[test]
    fn test_parse_slash_command_json() {
        // JSON object
        let (cmd, args) = parse_slash_command(r#"/plan edit {"tasks":[{"id":"T1"}]}"#);
        assert_eq!(cmd, "plan");
        assert_eq!(args, vec!["edit", r#"{"tasks":[{"id":"T1"}]}"#]);

        // JSON array
        let (cmd, args) = parse_slash_command("/task add [\"a\", \"b\"]");
        assert_eq!(cmd, "task");
        assert_eq!(args, vec!["add", "[\"a\", \"b\"]"]);

        // JSON with whitespace
        let (cmd, args) = parse_slash_command(r#"/plan edit { "key": "value" }"#);
        assert_eq!(cmd, "plan");
        assert_eq!(args, vec!["edit", r#"{ "key": "value" }"#]);
    }

    #[test]
    fn test_parse_slash_command_unquoted() {
        let (cmd, args) = parse_slash_command("/plan accept");
        assert_eq!(cmd, "plan");
        assert_eq!(args, vec!["accept"]);

        let (cmd, args) = parse_slash_command("/tasks regen feedback here");
        assert_eq!(cmd, "tasks");
        assert_eq!(args, vec!["regen", "feedback", "here"]);

        // Multiple spaces collapsed
        let (cmd, args) = parse_slash_command("/plan   accept  ");
        assert_eq!(cmd, "plan");
        assert_eq!(args, vec!["accept"]);
    }

    #[test]
    fn test_parse_slash_command_mixed() {
        // Mix of quoted, JSON, and unquoted
        let (cmd, args) = parse_slash_command(r#"/task add "quoted arg" {"json": true} unquoted"#);
        assert_eq!(cmd, "task");
        assert_eq!(
            args,
            vec!["add", "quoted arg", r#"{"json": true}"#, "unquoted"]
        );
    }

    #[test]
    fn test_parse_slash_command_edge_cases() {
        // Empty args
        let (cmd, args) = parse_slash_command("/help");
        assert_eq!(cmd, "help");
        assert_eq!(args, Vec::<String>::new());

        // Empty string
        let (cmd, args) = parse_slash_command("");
        assert!(cmd.is_empty());
        assert!(args.is_empty());

        // Just slash
        let (cmd, args) = parse_slash_command("/");
        assert!(cmd.is_empty());
        assert!(args.is_empty());

        // Unknown command still parses
        let (cmd, args) = parse_slash_command("/unknown arg1 arg2");
        assert_eq!(cmd, "unknown");
        assert_eq!(args, vec!["arg1", "arg2"]);
    }
}
