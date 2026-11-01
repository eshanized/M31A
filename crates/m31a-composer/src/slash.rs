//! Canonical Slash Command Registry, Autocomplete & Quote-Aware Tokenizer (`m31a-composer::slash`).

use m31a_trust::OperationCapability;
use serde::{Deserialize, Serialize};

/// High-level execution category of a slash command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandExecutionClass {
    /// Pure client/UI view change (help, theme, modal viewing)
    ClientLocal,
    /// Asynchronous agent/tool execution
    AgentAction,
    /// Process/system execution (spawning, killing)
    ProcessSupervision,
    /// Storage, session, or git state manipulation
    StateModification,
}

/// When a slash command is available to execute.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextAvailability {
    /// Always available (idle, executing, modals closed)
    Always,
    /// Available only when idle (no generation running)
    IdleOnly,
    /// Available while generation turn is running
    RunningOnly,
}

/// Canonical Slash Command definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlashCommand {
    pub name: &'static str,
    pub aliases: &'static [&'static str],
    pub description: &'static str,
    pub usage: &'static str,
    pub category: &'static str,
    pub capability: OperationCapability,
    pub subcommands: &'static [&'static str],
    pub requires_confirmation: bool,
    pub execution_class: CommandExecutionClass,
    pub availability: ContextAvailability,
}

/// Single authoritative catalogue of all supported slash commands.
pub const CANONICAL_SLASH_COMMANDS: &[SlashCommand] = &[
    SlashCommand {
        name: "/help",
        aliases: &["/?"],
        description: "Show keybindings, commands, and M31A usage guide",
        usage: "/help",
        category: "Core",
        capability: OperationCapability::HarmlessUI,
        subcommands: &[],
        requires_confirmation: false,
        execution_class: CommandExecutionClass::ClientLocal,
        availability: ContextAvailability::Always,
    },
    SlashCommand {
        name: "/settings",
        aliases: &["/config"],
        description: "Open dedicated interactive settings and configuration manager",
        usage: "/settings",
        category: "Configuration",
        capability: OperationCapability::HarmlessUI,
        subcommands: &[],
        requires_confirmation: false,
        execution_class: CommandExecutionClass::ClientLocal,
        availability: ContextAvailability::Always,
    },
    SlashCommand {
        name: "/setup",
        aliases: &[],
        description: "Inspect M31A configuration and onboarding setup",
        usage: "/setup [settings|wizard|gui|edit]",
        category: "Configuration",
        capability: OperationCapability::HarmlessUI,
        subcommands: &["settings", "wizard", "gui", "edit"],
        requires_confirmation: false,
        execution_class: CommandExecutionClass::ClientLocal,
        availability: ContextAvailability::Always,
    },
    SlashCommand {
        name: "/palette",
        aliases: &["/cmd"],
        description: "Open universal fuzzy command palette",
        usage: "/palette",
        category: "Core",
        capability: OperationCapability::HarmlessUI,
        subcommands: &[],
        requires_confirmation: false,
        execution_class: CommandExecutionClass::ClientLocal,
        availability: ContextAvailability::Always,
    },
    SlashCommand {
        name: "/recap",
        aliases: &[],
        description: "Generate a bounded catch-up recap of this session",
        usage: "/recap",
        category: "Session",
        capability: OperationCapability::HarmlessUI,
        subcommands: &[],
        requires_confirmation: false,
        execution_class: CommandExecutionClass::ClientLocal,
        availability: ContextAvailability::Always,
    },
    SlashCommand {
        name: "/model",
        aliases: &["/models"],
        description: "Open model selection popup or switch active model",
        usage: "/model [slug|preset|route]",
        category: "Model",
        capability: OperationCapability::HarmlessUI,
        subcommands: &["route", "nim", "anthropic", "openai", "gemini", "ollama"],
        requires_confirmation: false,
        execution_class: CommandExecutionClass::ClientLocal,
        availability: ContextAvailability::Always,
    },
    SlashCommand {
        name: "/effort",
        aliases: &[],
        description: "Set reasoning effort override (low, medium, high, none)",
        usage: "/effort [low|medium|high|none]",
        category: "Model",
        capability: OperationCapability::HarmlessUI,
        subcommands: &["low", "medium", "high", "none"],
        requires_confirmation: false,
        execution_class: CommandExecutionClass::ClientLocal,
        availability: ContextAvailability::Always,
    },
    SlashCommand {
        name: "/context",
        aliases: &[],
        description: "Dynamic context monitor, file pinning, and cache refresh",
        usage: "/context [status|refresh|pin|unpin|clear|compact]",
        category: "Session",
        capability: OperationCapability::WorkspaceMetadataRead,
        subcommands: &["status", "refresh", "pin", "unpin", "clear", "compact"],
        requires_confirmation: false,
        execution_class: CommandExecutionClass::StateModification,
        availability: ContextAvailability::Always,
    },
    SlashCommand {
        name: "/plan",
        aliases: &[],
        description: "Toggle Plan mode vs Default execution mode",
        usage: "/plan",
        category: "Execution",
        capability: OperationCapability::HarmlessUI,
        subcommands: &[],
        requires_confirmation: false,
        execution_class: CommandExecutionClass::ClientLocal,
        availability: ContextAvailability::Always,
    },
    SlashCommand {
        name: "/goal",
        aliases: &["/objective"],
        description: "View or set long-running task objective and budget",
        usage: "/goal [objective|pause|resume|clear]",
        category: "Execution",
        capability: OperationCapability::WorkspaceMetadataRead,
        subcommands: &["pause", "resume", "clear"],
        requires_confirmation: false,
        execution_class: CommandExecutionClass::StateModification,
        availability: ContextAvailability::Always,
    },
    SlashCommand {
        name: "/vibe",
        aliases: &[],
        description: "Engage continuous background auto-healing vibe loop",
        usage: "/vibe",
        category: "Execution",
        capability: OperationCapability::WorkspaceMutation,
        subcommands: &[],
        requires_confirmation: true,
        execution_class: CommandExecutionClass::AgentAction,
        availability: ContextAvailability::IdleOnly,
    },
    SlashCommand {
        name: "/visual",
        aliases: &[],
        description: "Execute autonomous Visual TDD against web frontend",
        usage: "/visual [url]",
        category: "Execution",
        capability: OperationCapability::NetworkAccess,
        subcommands: &[],
        requires_confirmation: false,
        execution_class: CommandExecutionClass::AgentAction,
        availability: ContextAvailability::IdleOnly,
    },
    SlashCommand {
        name: "/diff",
        aliases: &[],
        description: "View uncommitted Git diff in working directory",
        usage: "/diff",
        category: "Git",
        capability: OperationCapability::WorkspaceMetadataRead,
        subcommands: &[],
        requires_confirmation: false,
        execution_class: CommandExecutionClass::ClientLocal,
        availability: ContextAvailability::Always,
    },
    SlashCommand {
        name: "/apply",
        aliases: &[],
        description: "View or apply uncommitted changes to git working tree",
        usage: "/apply [patch_file]",
        category: "Git",
        capability: OperationCapability::PatchApplication,
        subcommands: &[],
        requires_confirmation: true,
        execution_class: CommandExecutionClass::StateModification,
        availability: ContextAvailability::IdleOnly,
    },
    SlashCommand {
        name: "/review",
        aliases: &[],
        description: "Review current git diff for bugs and improvements",
        usage: "/review [ref]",
        category: "Review",
        capability: OperationCapability::WorkspaceMetadataRead,
        subcommands: &[],
        requires_confirmation: false,
        execution_class: CommandExecutionClass::AgentAction,
        availability: ContextAvailability::Always,
    },
    SlashCommand {
        name: "/verify",
        aliases: &["/check"],
        description: "Run autonomous compiler/linter check on workspace",
        usage: "/verify",
        category: "Verification",
        capability: OperationCapability::ShellExecution,
        subcommands: &[],
        requires_confirmation: false,
        execution_class: CommandExecutionClass::ProcessSupervision,
        availability: ContextAvailability::IdleOnly,
    },
    SlashCommand {
        name: "/test",
        aliases: &[],
        description: "Run workspace test suite with diagnostic reporting",
        usage: "/test",
        category: "Verification",
        capability: OperationCapability::ShellExecution,
        subcommands: &[],
        requires_confirmation: false,
        execution_class: CommandExecutionClass::ProcessSupervision,
        availability: ContextAvailability::IdleOnly,
    },
    SlashCommand {
        name: "/safety",
        aliases: &[],
        description: "Audit codebase blast-radius and data-flow taint invariants",
        usage: "/safety",
        category: "Security",
        capability: OperationCapability::WorkspaceMetadataRead,
        subcommands: &[],
        requires_confirmation: false,
        execution_class: CommandExecutionClass::ClientLocal,
        availability: ContextAvailability::Always,
    },
    SlashCommand {
        name: "/sandbox",
        aliases: &[],
        description: "Cycle or set sandbox policy: ReadOnly | WorkspaceWrite | DangerFullAccess",
        usage: "/sandbox [readonly|workspacewrite|dangerfullaccess]",
        category: "Security",
        capability: OperationCapability::SandboxSensitive,
        subcommands: &["readonly", "workspacewrite", "dangerfullaccess"],
        requires_confirmation: false,
        execution_class: CommandExecutionClass::ClientLocal,
        availability: ContextAvailability::Always,
    },
    SlashCommand {
        name: "/approval",
        aliases: &[],
        description: "Cycle or set approval policy: on-request | never | approve-for-me",
        usage: "/approval [on-request|never|approve-for-me]",
        category: "Security",
        capability: OperationCapability::HarmlessUI,
        subcommands: &["on-request", "never", "approve-for-me"],
        requires_confirmation: false,
        execution_class: CommandExecutionClass::ClientLocal,
        availability: ContextAvailability::Always,
    },
    SlashCommand {
        name: "/status",
        aliases: &["/info"],
        description: "Show session configuration, token usage, and environment",
        usage: "/status",
        category: "Session",
        capability: OperationCapability::WorkspaceMetadataRead,
        subcommands: &[],
        requires_confirmation: false,
        execution_class: CommandExecutionClass::ClientLocal,
        availability: ContextAvailability::Always,
    },
    SlashCommand {
        name: "/skills",
        aliases: &[],
        description: "List discovered skills in workspace and home directories",
        usage: "/skills",
        category: "Tools",
        capability: OperationCapability::WorkspaceMetadataRead,
        subcommands: &[],
        requires_confirmation: false,
        execution_class: CommandExecutionClass::ClientLocal,
        availability: ContextAvailability::Always,
    },
    SlashCommand {
        name: "/mcp",
        aliases: &[],
        description: "Browse and manage Model Context Protocol servers and tools",
        usage: "/mcp",
        category: "Tools",
        capability: OperationCapability::WorkspaceMetadataRead,
        subcommands: &[],
        requires_confirmation: false,
        execution_class: CommandExecutionClass::ClientLocal,
        availability: ContextAvailability::Always,
    },
    SlashCommand {
        name: "/templates",
        aliases: &[],
        description: "Browse and apply pre-configured collaboration workflows",
        usage: "/templates",
        category: "Tools",
        capability: OperationCapability::WorkspaceMetadataRead,
        subcommands: &[],
        requires_confirmation: false,
        execution_class: CommandExecutionClass::ClientLocal,
        availability: ContextAvailability::Always,
    },
    SlashCommand {
        name: "/persona",
        aliases: &[],
        description: "Activate specialized engineering persona directives",
        usage: "/persona [architect|developer|tester|reviewer|security]",
        category: "Configuration",
        capability: OperationCapability::HarmlessUI,
        subcommands: &["architect", "developer", "tester", "reviewer", "security"],
        requires_confirmation: false,
        execution_class: CommandExecutionClass::ClientLocal,
        availability: ContextAvailability::Always,
    },
    SlashCommand {
        name: "/bell",
        aliases: &[],
        description: "Toggle audio feedback and terminal bell chimes",
        usage: "/bell [on|off]",
        category: "Configuration",
        capability: OperationCapability::HarmlessUI,
        subcommands: &["on", "off"],
        requires_confirmation: false,
        execution_class: CommandExecutionClass::ClientLocal,
        availability: ContextAvailability::Always,
    },
    SlashCommand {
        name: "/agents",
        aliases: &["/subagents"],
        description: "List active subagents and multi-agent hierarchy",
        usage: "/agents",
        category: "Multi-Agent",
        capability: OperationCapability::WorkspaceMetadataRead,
        subcommands: &[],
        requires_confirmation: false,
        execution_class: CommandExecutionClass::ClientLocal,
        availability: ContextAvailability::Always,
    },
    SlashCommand {
        name: "/analytics",
        aliases: &["/stats"],
        description: "Open session token accounting & usage dashboard",
        usage: "/analytics",
        category: "Session",
        capability: OperationCapability::HarmlessUI,
        subcommands: &[],
        requires_confirmation: false,
        execution_class: CommandExecutionClass::ClientLocal,
        availability: ContextAvailability::Always,
    },
    SlashCommand {
        name: "/compact",
        aliases: &[],
        description: "Compact transcript history to reduce token context",
        usage: "/compact",
        category: "Session",
        capability: OperationCapability::HarmlessUI,
        subcommands: &[],
        requires_confirmation: false,
        execution_class: CommandExecutionClass::StateModification,
        availability: ContextAvailability::IdleOnly,
    },
    SlashCommand {
        name: "/clear",
        aliases: &["/cls"],
        description: "Clear the active conversation transcript view",
        usage: "/clear",
        category: "Session",
        capability: OperationCapability::HarmlessUI,
        subcommands: &[],
        requires_confirmation: false,
        execution_class: CommandExecutionClass::ClientLocal,
        availability: ContextAvailability::Always,
    },
    SlashCommand {
        name: "/undo",
        aliases: &[],
        description: "Revert session and workspace changes to the previous turn checkpoint",
        usage: "/undo",
        category: "Session",
        capability: OperationCapability::WorkspaceMutation,
        subcommands: &[],
        requires_confirmation: true,
        execution_class: CommandExecutionClass::StateModification,
        availability: ContextAvailability::IdleOnly,
    },
    SlashCommand {
        name: "/resume",
        aliases: &["/sessions"],
        description: "Open session picker to resume a previous conversation",
        usage: "/resume",
        category: "Session",
        capability: OperationCapability::HarmlessUI,
        subcommands: &[],
        requires_confirmation: false,
        execution_class: CommandExecutionClass::ClientLocal,
        availability: ContextAvailability::Always,
    },
    SlashCommand {
        name: "/fork",
        aliases: &[],
        description: "Fork current session into a new branch",
        usage: "/fork",
        category: "Session",
        capability: OperationCapability::WorkspaceMutation,
        subcommands: &[],
        requires_confirmation: false,
        execution_class: CommandExecutionClass::StateModification,
        availability: ContextAvailability::IdleOnly,
    },
    SlashCommand {
        name: "/stash",
        aliases: &[],
        description: "Save, pop, or list stashed prompt drafts",
        usage: "/stash [pop|list|clear]",
        category: "Composer",
        capability: OperationCapability::HarmlessUI,
        subcommands: &["pop", "list", "clear"],
        requires_confirmation: false,
        execution_class: CommandExecutionClass::ClientLocal,
        availability: ContextAvailability::Always,
    },
    SlashCommand {
        name: "/worktree",
        aliases: &[],
        description: "List or create isolated managed Git worktrees",
        usage: "/worktree [create|list|remove|prune]",
        category: "Git",
        capability: OperationCapability::GitMutation,
        subcommands: &["create", "list", "remove", "prune"],
        requires_confirmation: false,
        execution_class: CommandExecutionClass::StateModification,
        availability: ContextAvailability::Always,
    },
    SlashCommand {
        name: "/hooks",
        aliases: &[],
        description: "List configured lifecycle hooks",
        usage: "/hooks",
        category: "Tools",
        capability: OperationCapability::WorkspaceMetadataRead,
        subcommands: &[],
        requires_confirmation: false,
        execution_class: CommandExecutionClass::ClientLocal,
        availability: ContextAvailability::Always,
    },
    SlashCommand {
        name: "/memories",
        aliases: &[],
        description: "View consolidated cross-session memories and learnings",
        usage: "/memories",
        category: "Memory",
        capability: OperationCapability::WorkspaceMetadataRead,
        subcommands: &[],
        requires_confirmation: false,
        execution_class: CommandExecutionClass::ClientLocal,
        availability: ContextAvailability::Always,
    },
    SlashCommand {
        name: "/search",
        aliases: &[],
        description: "Perform a live web search for technical documentation",
        usage: "/search <query>",
        category: "Tools",
        capability: OperationCapability::NetworkAccess,
        subcommands: &[],
        requires_confirmation: false,
        execution_class: CommandExecutionClass::AgentAction,
        availability: ContextAvailability::Always,
    },
    SlashCommand {
        name: "/export",
        aliases: &[],
        description: "Export current conversation transcript as Markdown file",
        usage: "/export",
        category: "Session",
        capability: OperationCapability::WorkspaceMutation,
        subcommands: &[],
        requires_confirmation: false,
        execution_class: CommandExecutionClass::StateModification,
        availability: ContextAvailability::Always,
    },
    SlashCommand {
        name: "/lock",
        aliases: &[],
        description: "Lock a file path from agent modification",
        usage: "/lock <path>",
        category: "Concurrency",
        capability: OperationCapability::WorkspaceMutation,
        subcommands: &[],
        requires_confirmation: false,
        execution_class: CommandExecutionClass::StateModification,
        availability: ContextAvailability::Always,
    },
    SlashCommand {
        name: "/unlock",
        aliases: &[],
        description: "Unlock a file path to permit agent modifications",
        usage: "/unlock <path>",
        category: "Concurrency",
        capability: OperationCapability::WorkspaceMutation,
        subcommands: &[],
        requires_confirmation: false,
        execution_class: CommandExecutionClass::StateModification,
        availability: ContextAvailability::Always,
    },
    SlashCommand {
        name: "/file-locks",
        aliases: &[],
        description: "List all active file paths currently locked from editing",
        usage: "/file-locks",
        category: "Concurrency",
        capability: OperationCapability::WorkspaceMetadataRead,
        subcommands: &[],
        requires_confirmation: false,
        execution_class: CommandExecutionClass::ClientLocal,
        availability: ContextAvailability::Always,
    },
    SlashCommand {
        name: "/locks",
        aliases: &[],
        description: "Inspect active path reader-writer concurrency locks and leases",
        usage: "/locks",
        category: "Concurrency",
        capability: OperationCapability::WorkspaceMetadataRead,
        subcommands: &[],
        requires_confirmation: false,
        execution_class: CommandExecutionClass::ClientLocal,
        availability: ContextAvailability::Always,
    },
    SlashCommand {
        name: "/timing",
        aliases: &[],
        description: "Show causal critical path and direct tool timings modal",
        usage: "/timing",
        category: "Diagnostics",
        capability: OperationCapability::HarmlessUI,
        subcommands: &[],
        requires_confirmation: false,
        execution_class: CommandExecutionClass::ClientLocal,
        availability: ContextAvailability::Always,
    },
    SlashCommand {
        name: "/doctor",
        aliases: &[],
        description: "Run diagnostics on API key, bwrap, and SQLite",
        usage: "/doctor",
        category: "Diagnostics",
        capability: OperationCapability::WorkspaceMetadataRead,
        subcommands: &[],
        requires_confirmation: false,
        execution_class: CommandExecutionClass::ProcessSupervision,
        availability: ContextAvailability::Always,
    },
    SlashCommand {
        name: "/board",
        aliases: &[],
        description: "List recent multi-agent message board posts",
        usage: "/board",
        category: "Multi-Agent",
        capability: OperationCapability::WorkspaceMetadataRead,
        subcommands: &[],
        requires_confirmation: false,
        execution_class: CommandExecutionClass::ClientLocal,
        availability: ContextAvailability::Always,
    },
    SlashCommand {
        name: "/checkpoints",
        aliases: &[],
        description: "Browse saved session checkpoints and revert DAG",
        usage: "/checkpoints",
        category: "Session",
        capability: OperationCapability::WorkspaceMetadataRead,
        subcommands: &[],
        requires_confirmation: false,
        execution_class: CommandExecutionClass::ClientLocal,
        availability: ContextAvailability::Always,
    },
    SlashCommand {
        name: "/qr",
        aliases: &[],
        description: "Display mobile dev preview URL and connection guidance",
        usage: "/qr",
        category: "Tools",
        capability: OperationCapability::HarmlessUI,
        subcommands: &[],
        requires_confirmation: false,
        execution_class: CommandExecutionClass::ClientLocal,
        availability: ContextAvailability::Always,
    },
    SlashCommand {
        name: "/cd",
        aliases: &[],
        description: "Change current working directory",
        usage: "/cd <path>",
        category: "Navigation",
        capability: OperationCapability::WorkspaceMetadataRead,
        subcommands: &[],
        requires_confirmation: false,
        execution_class: CommandExecutionClass::StateModification,
        availability: ContextAvailability::Always,
    },
    SlashCommand {
        name: "/pwd",
        aliases: &[],
        description: "Print current working directory",
        usage: "/pwd",
        category: "Navigation",
        capability: OperationCapability::HarmlessUI,
        subcommands: &[],
        requires_confirmation: false,
        execution_class: CommandExecutionClass::ClientLocal,
        availability: ContextAvailability::Always,
    },
    SlashCommand {
        name: "/exit",
        aliases: &["/quit", "/q"],
        description: "Exit M31A interactive terminal",
        usage: "/exit",
        category: "Core",
        capability: OperationCapability::HarmlessUI,
        subcommands: &[],
        requires_confirmation: false,
        execution_class: CommandExecutionClass::ClientLocal,
        availability: ContextAvailability::Always,
    },
    SlashCommand {
        name: "/build",
        aliases: &[],
        description: "Switch to autonomous Build mode (full execution)",
        usage: "/build",
        category: "Execution",
        capability: OperationCapability::WorkspaceMutation,
        subcommands: &[],
        requires_confirmation: false,
        execution_class: CommandExecutionClass::ClientLocal,
        availability: ContextAvailability::Always,
    },
    SlashCommand {
        name: "/def",
        aliases: &["/definition"],
        description: "Find AST definition of code symbol across workspace",
        usage: "/def <symbol>",
        category: "Navigation",
        capability: OperationCapability::WorkspaceMetadataRead,
        subcommands: &[],
        requires_confirmation: false,
        execution_class: CommandExecutionClass::ClientLocal,
        availability: ContextAvailability::Always,
    },
    SlashCommand {
        name: "/outline",
        aliases: &["/symbols"],
        description: "Browse code symbols and structures in file",
        usage: "/outline [path]",
        category: "Navigation",
        capability: OperationCapability::WorkspaceMetadataRead,
        subcommands: &[],
        requires_confirmation: false,
        execution_class: CommandExecutionClass::ClientLocal,
        availability: ContextAvailability::Always,
    },
    SlashCommand {
        name: "/procs",
        aliases: &["/ps", "/processes"],
        description: "Inspect and manage background processes and servers",
        usage: "/procs",
        category: "Process",
        capability: OperationCapability::WorkspaceMetadataRead,
        subcommands: &[],
        requires_confirmation: false,
        execution_class: CommandExecutionClass::ClientLocal,
        availability: ContextAvailability::Always,
    },
    SlashCommand {
        name: "/kill",
        aliases: &[],
        description: "Terminate running background process by ID",
        usage: "/kill <process_id>",
        category: "Process",
        capability: OperationCapability::ShellExecution,
        subcommands: &[],
        requires_confirmation: true,
        execution_class: CommandExecutionClass::ProcessSupervision,
        availability: ContextAvailability::Always,
    },
    SlashCommand {
        name: "/logs",
        aliases: &[],
        description: "Inspect real-time tail output of background process",
        usage: "/logs <process_id>",
        category: "Process",
        capability: OperationCapability::WorkspaceMetadataRead,
        subcommands: &[],
        requires_confirmation: false,
        execution_class: CommandExecutionClass::ClientLocal,
        availability: ContextAvailability::Always,
    },
    SlashCommand {
        name: "/revert",
        aliases: &[],
        description: "Revert session and git worktree to a previous checkpoint",
        usage: "/revert [checkpoint_id]",
        category: "Session",
        capability: OperationCapability::WorkspaceMutation,
        subcommands: &[],
        requires_confirmation: true,
        execution_class: CommandExecutionClass::StateModification,
        availability: ContextAvailability::IdleOnly,
    },
    SlashCommand {
        name: "/rollout",
        aliases: &["/journal"],
        description: "Open rollout journal of deployed changes and evidence",
        usage: "/rollout",
        category: "Verification",
        capability: OperationCapability::WorkspaceMetadataRead,
        subcommands: &[],
        requires_confirmation: false,
        execution_class: CommandExecutionClass::ClientLocal,
        availability: ContextAvailability::Always,
    },
    SlashCommand {
        name: "/scratchpad",
        aliases: &["/codemode"],
        description: "Open interactive V8 Code Mode TypeScript/JavaScript scratchpad",
        usage: "/scratchpad",
        category: "Execution",
        capability: OperationCapability::WorkspaceMutation,
        subcommands: &[],
        requires_confirmation: false,
        execution_class: CommandExecutionClass::ClientLocal,
        availability: ContextAvailability::Always,
    },
    SlashCommand {
        name: "/theme",
        aliases: &[],
        description: "Switch active theme (everblush, dark, light, contrast)",
        usage: "/theme [everblush|dark|light|contrast]",
        category: "Configuration",
        capability: OperationCapability::HarmlessUI,
        subcommands: &["everblush", "dark", "light", "contrast"],
        requires_confirmation: false,
        execution_class: CommandExecutionClass::ClientLocal,
        availability: ContextAvailability::Always,
    },
    SlashCommand {
        name: "/trust",
        aliases: &[],
        description: "Manage workspace trust policy and run security audit",
        usage: "/trust [add|session|audit|revoke|status]",
        category: "Security",
        capability: OperationCapability::TrustManagement,
        subcommands: &["add", "session", "audit", "revoke", "status"],
        requires_confirmation: false,
        execution_class: CommandExecutionClass::StateModification,
        availability: ContextAvailability::Always,
    },
    SlashCommand {
        name: "/experimental",
        aliases: &[],
        description: "Open experimental features toggle menu",
        usage: "/experimental",
        category: "Configuration",
        capability: OperationCapability::HarmlessUI,
        subcommands: &[],
        requires_confirmation: false,
        execution_class: CommandExecutionClass::ClientLocal,
        availability: ContextAvailability::Always,
    },
];

/// Backward compatibility alias.
pub const BUILTIN_SLASH_COMMANDS: &[SlashCommand] = CANONICAL_SLASH_COMMANDS;

/// Finds a command by name or alias.
pub fn find_command(name_or_alias: &str) -> Option<&'static SlashCommand> {
    let lower = name_or_alias.to_lowercase();
    CANONICAL_SLASH_COMMANDS.iter().find(|cmd| {
        cmd.name.eq_ignore_ascii_case(&lower)
            || cmd.aliases.iter().any(|a| a.eq_ignore_ascii_case(&lower))
    })
}

/// Returns all commands whose name or aliases match the given prefix.
pub fn matching_commands(prefix: &str) -> Vec<&'static SlashCommand> {
    let lower = prefix.trim().to_lowercase();
    if lower.is_empty() {
        return CANONICAL_SLASH_COMMANDS.iter().collect();
    }
    CANONICAL_SLASH_COMMANDS
        .iter()
        .filter(|cmd| {
            cmd.name.to_lowercase().starts_with(&lower)
                || cmd
                    .aliases
                    .iter()
                    .any(|a| a.to_lowercase().starts_with(&lower))
        })
        .collect()
}

/// Structured parse error for slash commands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SlashParseError {
    EmptyInput,
    MissingSlashPrefix,
    UnterminatedQuote { quote_char: char, position: usize },
    TrailingEscape { position: usize },
    UnknownCommand { name: String },
    UnknownSubcommand { command: String, subcommand: String },
    InvalidArgument { message: String },
}

impl std::fmt::Display for SlashParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyInput => write!(f, "Slash command input is empty"),
            Self::MissingSlashPrefix => write!(f, "Slash command must begin with '/'"),
            Self::UnterminatedQuote {
                quote_char,
                position,
            } => {
                write!(
                    f,
                    "Unterminated quote '{}' at character position {}",
                    quote_char, position
                )
            }
            Self::TrailingEscape { position } => {
                write!(
                    f,
                    "Trailing escape character '\\' at character position {}",
                    position
                )
            }
            Self::UnknownCommand { name } => write!(f, "Unknown slash command '{}'", name),
            Self::UnknownSubcommand {
                command,
                subcommand,
            } => {
                write!(
                    f,
                    "Unknown subcommand '{}' for command '{}'",
                    subcommand, command
                )
            }
            Self::InvalidArgument { message } => write!(f, "Invalid argument: {}", message),
        }
    }
}

impl std::error::Error for SlashParseError {}

/// Fully parsed and resolved slash command invocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedSlashCommand {
    pub command: &'static SlashCommand,
    pub raw_command: String,
    pub args: Vec<String>,
    pub subcommand: Option<String>,
}

/// Tokenizes a slash command line, preserving empty quoted arguments `""`, escaped characters,
/// and returning structured parse errors for syntax errors.
pub fn parse_slash_tokens(input: &str) -> Result<Vec<String>, SlashParseError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(SlashParseError::EmptyInput);
    }
    if !trimmed.starts_with('/') {
        return Err(SlashParseError::MissingSlashPrefix);
    }

    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_single = false;
    let mut in_double = false;
    let mut quote_start = 0;
    let mut escaped = false;
    let mut token_active = false;

    for (pos, ch) in trimmed.char_indices() {
        if escaped {
            current.push(ch);
            token_active = true;
            escaped = false;
            continue;
        }

        match ch {
            '\\' => {
                escaped = true;
            }
            '\'' if !in_double => {
                if in_single {
                    in_single = false;
                    token_active = true;
                } else {
                    in_single = true;
                    quote_start = pos;
                    token_active = true;
                }
            }
            '"' if !in_single => {
                if in_double {
                    in_double = false;
                    token_active = true;
                } else {
                    in_double = true;
                    quote_start = pos;
                    token_active = true;
                }
            }
            ' ' | '\t' | '\r' | '\n' if !in_single && !in_double => {
                if token_active {
                    tokens.push(std::mem::take(&mut current));
                    token_active = false;
                }
            }
            _ => {
                current.push(ch);
                token_active = true;
            }
        }
    }

    if escaped {
        return Err(SlashParseError::TrailingEscape {
            position: trimmed.len() - 1,
        });
    }

    if in_single {
        return Err(SlashParseError::UnterminatedQuote {
            quote_char: '\'',
            position: quote_start,
        });
    }

    if in_double {
        return Err(SlashParseError::UnterminatedQuote {
            quote_char: '"',
            position: quote_start,
        });
    }

    if token_active {
        tokens.push(current);
    }

    Ok(tokens)
}

/// Tokenizes a slash command line into a vector of arguments (lossy fallback for callers).
pub fn tokenize_slash_command(input: &str) -> Vec<String> {
    parse_slash_tokens(input)
        .unwrap_or_else(|_| input.split_whitespace().map(|s| s.to_string()).collect())
}

/// Parses and resolves a slash command line against the canonical registry.
pub fn parse_and_resolve(input: &str) -> Result<ParsedSlashCommand, SlashParseError> {
    let tokens = parse_slash_tokens(input)?;
    if tokens.is_empty() {
        return Err(SlashParseError::EmptyInput);
    }

    let cmd_name = &tokens[0];
    let cmd = find_command(cmd_name).ok_or_else(|| SlashParseError::UnknownCommand {
        name: cmd_name.clone(),
    })?;

    let args = tokens[1..].to_vec();
    let subcommand = if !cmd.subcommands.is_empty() && !args.is_empty() {
        let sub_candidate = args[0].to_lowercase();
        if cmd
            .subcommands
            .iter()
            .any(|s| s.eq_ignore_ascii_case(&sub_candidate))
        {
            Some(sub_candidate)
        } else {
            None
        }
    } else {
        None
    };

    Ok(ParsedSlashCommand {
        command: cmd,
        raw_command: cmd_name.clone(),
        args,
        subcommand,
    })
}

/// Candidate completion item for slash autocomplete (derives from canonical registry).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlashCompletionItem {
    pub name: String,
    pub display: String,
    pub insert_text: String,
    pub description: String,
    pub is_subcommand: bool,
}

/// Computes autocomplete suggestions based on the canonical registry and schema.
pub fn get_slash_completions(input: &str) -> Vec<SlashCompletionItem> {
    let trimmed = input.trim_start();
    if !trimmed.starts_with('/') {
        return Vec::new();
    }

    let has_trailing_space = input.ends_with(' ') || input.ends_with('\t');
    let parts: Vec<&str> = trimmed.split_whitespace().collect();

    // Case 1: Typing top-level command: e.g. "/con" or "/"
    if parts.len() <= 1 && !has_trailing_space {
        let query = parts.first().copied().unwrap_or("/");
        return matching_commands(query)
            .into_iter()
            .map(|cmd| SlashCompletionItem {
                name: cmd.name.to_string(),
                display: cmd.name.to_string(),
                insert_text: format!("{} ", cmd.name),
                description: cmd.description.to_string(),
                is_subcommand: false,
            })
            .collect();
    }

    // Case 2: Top-level command complete, suggest subcommands from canonical schema
    let cmd_str = parts[0];
    if let Some(cmd) = find_command(cmd_str)
        && !cmd.subcommands.is_empty()
    {
        let sub_query = if parts.len() > 1 && !has_trailing_space {
            parts[1].to_lowercase()
        } else {
            String::new()
        };

        return cmd
            .subcommands
            .iter()
            .filter(|sub| sub.to_lowercase().starts_with(&sub_query))
            .map(|sub| SlashCompletionItem {
                name: format!("{} {}", cmd.name, sub),
                display: format!("{} {}", cmd.name, sub),
                insert_text: format!("{} {}", cmd.name, sub),
                description: format!("Subcommand for {}: {}", cmd.name, sub),
                is_subcommand: true,
            })
            .collect();
    }

    Vec::new()
}

/// Popup state for slash command autocompletion in text input views.
#[derive(Debug, Clone)]
pub struct SlashPopup {
    pub is_active: bool,
    pub matches: Vec<SlashCompletionItem>,
    pub selected_index: usize,
}

impl Default for SlashPopup {
    fn default() -> Self {
        Self::new()
    }
}

impl SlashPopup {
    pub fn new() -> Self {
        Self {
            is_active: false,
            matches: Vec::new(),
            selected_index: 0,
        }
    }

    pub fn update(&mut self, text: &str) {
        if text.starts_with('/') && !text.contains('\n') {
            self.matches = get_slash_completions(text);
            self.is_active = !self.matches.is_empty();
            if self.selected_index >= self.matches.len() {
                self.selected_index = 0;
            }
        } else {
            self.is_active = false;
            self.matches.clear();
            self.selected_index = 0;
        }
    }

    pub fn move_up(&mut self) {
        if !self.matches.is_empty() {
            self.selected_index = self.selected_index.saturating_sub(1);
        }
    }

    pub fn move_down(&mut self) {
        if !self.matches.is_empty() && self.selected_index + 1 < self.matches.len() {
            self.selected_index += 1;
        }
    }

    pub fn selected_item(&self) -> Option<&SlashCompletionItem> {
        self.matches.get(self.selected_index)
    }

    pub fn selected_command(&self) -> Option<&'static SlashCommand> {
        self.matches
            .get(self.selected_index)
            .and_then(|item| find_command(&item.name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_canonical_catalog_contains_expected_commands() {
        assert!(CANONICAL_SLASH_COMMANDS.len() >= 50);
        assert!(find_command("/help").is_some());
        assert!(find_command("/settings").is_some());
        assert!(find_command("/config").is_some());
        assert!(find_command("/exit").is_some());
        assert!(find_command("/quit").is_some());
        assert!(find_command("/q").is_some());
        assert!(find_command("/models").is_some());
    }

    #[test]
    fn test_table_driven_slash_parser() {
        struct Case {
            input: &'static str,
            expected: Result<Vec<&'static str>, ()>,
        }

        let cases = vec![
            Case {
                input: "/search \"rust tokio\" --limit 5",
                expected: Ok(vec!["/search", "rust tokio", "--limit", "5"]),
            },
            Case {
                input: "/lock 'src/my file.rs'",
                expected: Ok(vec!["/lock", "src/my file.rs"]),
            },
            Case {
                input: r#"/cd src/my\ special\ dir"#,
                expected: Ok(vec!["/cd", "src/my special dir"]),
            },
            Case {
                input: "/model meta/llama-3.3-70b-instruct",
                expected: Ok(vec!["/model", "meta/llama-3.3-70b-instruct"]),
            },
            Case {
                input: "/effort \"\"",
                expected: Ok(vec!["/effort", ""]),
            },
            Case {
                input: "/trust 'session'",
                expected: Ok(vec!["/trust", "session"]),
            },
            Case {
                input: "/search \"unterminated quote",
                expected: Err(()),
            },
            Case {
                input: "/search 'unterminated single",
                expected: Err(()),
            },
            Case {
                input: "/cd trailing_escape\\",
                expected: Err(()),
            },
            Case {
                input: "not_a_slash_cmd",
                expected: Err(()),
            },
            Case {
                input: "",
                expected: Err(()),
            },
        ];

        for case in cases {
            let res = parse_slash_tokens(case.input);
            match case.expected {
                Ok(expected_tokens) => {
                    assert!(
                        res.is_ok(),
                        "Expected ok for '{}', got error: {:?}",
                        case.input,
                        res.err()
                    );
                    let tokens = res.unwrap();
                    let as_strs: Vec<&str> = tokens.iter().map(|s| s.as_str()).collect();
                    assert_eq!(
                        as_strs, expected_tokens,
                        "Failed for input '{}'",
                        case.input
                    );
                }
                Err(()) => {
                    assert!(
                        res.is_err(),
                        "Expected error for '{}', but got: {:?}",
                        case.input,
                        res.unwrap()
                    );
                }
            }
        }
    }

    #[test]
    fn test_subcommand_autocomplete_derives_from_schema() {
        let completions = get_slash_completions("/context ");
        assert!(
            completions.len() >= 5,
            "Expected at least 5 subcommands for /context, got: {:?}",
            completions
        );
        assert!(completions.iter().any(|c| c.display == "/context status"));
        assert!(completions.iter().any(|c| c.display == "/context refresh"));
        assert!(completions.iter().any(|c| c.display == "/context compact"));

        // Partial subcommand query
        let partial = get_slash_completions("/context ref");
        assert_eq!(partial.len(), 1);
        assert_eq!(partial[0].display, "/context refresh");

        // Trust subcommands
        let trust_subs = get_slash_completions("/trust ");
        assert!(trust_subs.iter().any(|c| c.display == "/trust session"));
        assert!(trust_subs.iter().any(|c| c.display == "/trust add"));
        assert!(trust_subs.iter().any(|c| c.display == "/trust audit"));
    }

    #[test]
    fn test_alias_resolution_and_metadata() {
        let parsed = parse_and_resolve("/quit").unwrap();
        assert_eq!(parsed.command.name, "/exit");
        assert_eq!(
            parsed.command.execution_class,
            CommandExecutionClass::ClientLocal
        );

        let parsed_check = parse_and_resolve("/check").unwrap();
        assert_eq!(parsed_check.command.name, "/verify");
        assert_eq!(
            parsed_check.command.capability,
            OperationCapability::ShellExecution
        );

        let unknown = parse_and_resolve("/nonexistent_cmd_xyz");
        assert!(matches!(
            unknown,
            Err(SlashParseError::UnknownCommand { .. })
        ));
    }
}
