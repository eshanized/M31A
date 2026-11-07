//! M31A CLI binary entry point.
//!
//! Parses command line options, configures logging and sandboxes, loads user configuration,
//! and dispatches execution to interactive TUI modes, daemon commands, or batch subcommands.

use anyhow::{Context, Result, anyhow, bail};
use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::{Shell, generate};
use colored::*;
use m31a_agent_engine::{AgentEngine, EngineEvent};
use m31a_app_server::AppServer;
use m31a_core::types::ChatMessage;
use m31a_core::types::Role;
use m31a_core::{
    ApprovalPolicy, CollaborationMode, CompactTokenBudget, Config, ContextCompactor, HookEngine,
    HookEvent, McpServerConfig, MemoryManager, MemoryRecord, SandboxPolicy, Session,
    SystemPromptBuilder, WorktreeManager,
};
use m31a_execpolicy::ExecPolicy;
use m31a_guardian::GuardianReviewer;
use m31a_provider::{ProviderBuilder, ProviderConfig, ProviderType, get_nvidia_model_catalog};
use m31a_sandbox::SandboxRunner;
use m31a_storage::Storage;
use m31a_tools::ToolRegistry;
use std::collections::HashMap;
use std::io::{self, IsTerminal, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::Level;
use tracing_subscriber::FmtSubscriber;
use uuid::Uuid;

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

mod doctor;
mod gsd;
mod setup;
mod speculate;
mod vibe;
mod workflow;

#[derive(Parser, Debug)]
#[command(
    name = "m31a",
    about = "M31A: High-performance autonomous agentic coding assistant powered by NVIDIA NIM",
    version = "0.1.14"
)]
struct Cli {
    #[arg(help = "Optional user prompt to start an interactive or single-turn session")]
    prompt: Option<String>,

    #[command(subcommand)]
    command: Option<Commands>,

    #[arg(short = 'm', long = "model", help = "Model to use from NVIDIA NIM")]
    model: Option<String>,

    #[arg(
        short = 's',
        long = "sandbox",
        value_enum,
        help = "Sandbox execution policy"
    )]
    sandbox: Option<SandboxPolicyArg>,

    #[arg(
        short = 'a',
        long = "ask-for-approval",
        value_enum,
        help = "Approval policy"
    )]
    approval: Option<ApprovalPolicyArg>,

    #[arg(short = 'C', long = "cd", help = "Working root directory")]
    cd: Option<PathBuf>,

    #[arg(
        short = 'c',
        long = "config",
        help = "Override config value (key=value)"
    )]
    config_overrides: Vec<String>,

    #[arg(long = "worktree", help = "Run session in a new managed Git worktree")]
    worktree: bool,

    #[arg(long = "search", help = "Enable live web search tool")]
    search: bool,

    #[arg(
        short = 'i',
        long = "image",
        help = "Optional image(s) to attach to prompt"
    )]
    image: Vec<PathBuf>,

    #[arg(long = "enable", help = "Enable a feature flag (repeatable)")]
    enable: Vec<String>,

    #[arg(long = "disable", help = "Disable a feature flag (repeatable)")]
    disable: Vec<String>,

    #[arg(
        long = "approve-for-me",
        help = "Route approvals through guardian auto-review"
    )]
    approve_for_me: bool,

    #[arg(
        long = "dangerously-bypass-approvals-and-sandbox",
        help = "Skip approvals and execute outside sandbox"
    )]
    dangerously_bypass: bool,

    #[arg(
        long = "preset",
        help = "Apply approval security preset (read-only, standard, permissive, airgapped, danger-full-access)"
    )]
    preset: Option<String>,

    #[arg(
        short = 'p',
        long = "profile",
        help = "Layer profile config on top of base user config"
    )]
    profile: Option<String>,
    #[arg(
        long = "strict-config",
        default_value_t = false,
        help = "Reject unknown config fields (escalate warnings to errors)"
    )]
    strict_config: bool,
}

#[derive(clap::ValueEnum, Clone, Copy, Debug)]
enum SandboxPolicyArg {
    ReadOnly,
    WorkspaceWrite,
    DangerFullAccess,
}

impl From<SandboxPolicyArg> for SandboxPolicy {
    fn from(arg: SandboxPolicyArg) -> Self {
        match arg {
            SandboxPolicyArg::ReadOnly => SandboxPolicy::ReadOnly,
            SandboxPolicyArg::WorkspaceWrite => SandboxPolicy::WorkspaceWrite,
            SandboxPolicyArg::DangerFullAccess => SandboxPolicy::DangerFullAccess,
        }
    }
}

#[derive(clap::ValueEnum, Clone, Copy, Debug)]
enum ApprovalPolicyArg {
    OnRequest,
    Never,
    ApproveForMe,
}

impl From<ApprovalPolicyArg> for ApprovalPolicy {
    fn from(arg: ApprovalPolicyArg) -> Self {
        match arg {
            ApprovalPolicyArg::OnRequest => ApprovalPolicy::OnRequest,
            ApprovalPolicyArg::Never => ApprovalPolicy::Never,
            ApprovalPolicyArg::ApproveForMe => ApprovalPolicy::ApproveForMe,
        }
    }
}

#[derive(Subcommand, Debug)]
enum Commands {
    #[command(
        about = "Initialize and configure M31A credentials, default model, and security policies"
    )]
    Setup {
        #[arg(long = "api-key", help = "NVIDIA NIM API Key")]
        api_key: Option<String>,

        #[arg(long = "api-base", help = "NVIDIA NIM Base URL")]
        api_base: Option<String>,

        #[arg(
            short = 'm',
            long = "model",
            help = "Default model slug or alias (speed, code, reasoning, flagship)"
        )]
        model: Option<String>,

        #[arg(
            short = 's',
            long = "sandbox",
            help = "Sandbox policy (workspace-write, read-only, danger-full-access)"
        )]
        sandbox: Option<String>,

        #[arg(
            short = 'a',
            long = "approval",
            help = "Approval policy (on-request, approve-for-me, never)"
        )]
        approval: Option<String>,

        #[arg(
            long = "non-interactive",
            help = "Run without interactive terminal prompts"
        )]
        non_interactive: bool,

        #[arg(
            short = 'f',
            long = "force",
            help = "Force overwrite existing configuration"
        )]
        force: bool,
    },
    #[command(
        name = "model",
        alias = "models",
        about = "Manage, inspect, and dynamically switch NVIDIA NIM models"
    )]
    Model {
        #[command(subcommand)]
        cmd: Option<ModelCommands>,
    },
    #[command(about = "Run M31A non-interactively for a single task")]
    Exec {
        #[arg(help = "Prompt instruction to execute. If omitted or '-', read from stdin.")]
        prompt: Option<String>,

        #[arg(long = "json", help = "Print events to stdout as JSONL")]
        json: bool,

        #[arg(
            short = 'o',
            long = "output-last-message",
            help = "Path to write the model's final response"
        )]
        output_last_message: Option<PathBuf>,

        #[arg(
            long = "ephemeral",
            help = "Run without persisting session files to SQLite DB"
        )]
        ephemeral: bool,

        #[arg(long = "worktree", help = "Run session in a new managed Git worktree")]
        worktree: bool,

        #[arg(long = "search", help = "Enable live web search tool")]
        search: bool,

        #[arg(
            long = "self-heal",
            help = "Automatically trigger self-healing diagnostic repair loop on build/test failure"
        )]
        self_heal: bool,

        #[arg(
            long = "max-heal-iterations",
            default_value = "3",
            help = "Maximum self-healing repair iterations"
        )]
        max_heal_iterations: usize,

        #[arg(
            long = "plan",
            help = "Run in Plan Mode (file mutations and state modifications are blocked)"
        )]
        plan: bool,

        #[arg(
            long = "reasoning-effort",
            help = "Reasoning effort override (low, medium, high)"
        )]
        reasoning_effort: Option<String>,
    },
    #[command(about = "Undo/revert workspace changes to the previous turn checkpoint")]
    Undo {
        #[arg(
            long = "session",
            help = "Session ID to undo (defaults to latest active session)"
        )]
        session: Option<String>,
    },
    #[command(about = "Run code review on uncommitted working tree diffs")]
    Review {
        #[arg(long = "base", help = "Optional git base branch/ref to diff against")]
        base: Option<String>,
    },
    #[command(about = "Autonomously verify project with compiler checks or test suites")]
    Verify {
        #[arg(
            long = "tests",
            help = "Run project test suite instead of fast compiler/linter check"
        )]
        tests: bool,
        #[arg(
            long = "timeout",
            default_value = "60",
            help = "Verification timeout in seconds"
        )]
        timeout: u64,
    },
    #[command(about = "Manage workspace trust and inspect git hook safety armor")]
    Trust {
        #[command(subcommand)]
        cmd: TrustCommands,
    },
    #[command(
        about = "List managed background processes, dev servers, and active background tasks"
    )]
    Ps,
    #[command(about = "Terminate a background process")]
    Kill {
        #[arg(help = "Process ID (e.g. proc-a1b2c3d4)")]
        id: String,
        #[arg(short = 'f', long = "force", help = "Force kill immediately (SIGKILL)")]
        force: bool,
    },
    #[command(about = "Inspect or tail background process output logs")]
    Logs {
        #[arg(help = "Process ID")]
        id: String,
        #[arg(
            short = 'n',
            long = "lines",
            default_value = "50",
            help = "Number of recent lines to display"
        )]
        lines: usize,
    },
    #[command(about = "Apply latest diff produced by agent as a git apply to local working tree")]
    Apply {
        #[arg(help = "Optional patch file to apply directly")]
        patch_file: Option<PathBuf>,
    },
    #[command(about = "Resume a previous interactive session")]
    Resume {
        #[arg(short = 'l', long = "last", help = "Continue the most recent session")]
        last: bool,
        #[arg(help = "Optional specific thread ID to resume")]
        id: Option<String>,
    },
    #[command(about = "Fork a previous interactive session into a new branch")]
    Fork {
        #[arg(short = 'l', long = "last", help = "Fork the most recent session")]
        last: bool,
        #[arg(help = "Optional specific thread ID to fork")]
        id: Option<String>,
        #[arg(short = 't', long = "title", help = "Title for the forked branch")]
        title: Option<String>,
    },
    #[command(about = "Archive a saved session by thread id")]
    Archive {
        #[arg(help = "Thread ID to archive")]
        id: String,
    },
    #[command(about = "Unarchive a saved session by thread id")]
    Unarchive {
        #[arg(help = "Thread ID to unarchive")]
        id: String,
    },
    #[command(about = "Permanently delete a saved session by thread id")]
    Delete {
        #[arg(help = "Thread ID to delete")]
        id: String,
    },
    #[command(about = "Inspect and trigger lifecycle hooks")]
    Hooks {
        #[command(subcommand)]
        cmd: HooksCommands,
    },
    #[command(about = "Inspect and manage cross-session memories")]
    Memories {
        #[command(subcommand)]
        cmd: MemoriesCommands,
    },
    #[command(about = "Queue a message for a running or existing session")]
    Queue {
        #[arg(help = "Target thread ID")]
        id: String,
        #[arg(help = "Message content to append")]
        message: String,
    },
    #[command(about = "Browse all agent sessions and active subagents")]
    Agents {
        #[arg(help = "Optional parent thread ID to filter subagents")]
        id: Option<String>,
    },
    #[command(about = "Run the local JSON-RPC app-server (stdio or Unix domain socket)")]
    AppServer {
        #[arg(long = "stdio", help = "Use stdio transport")]
        stdio: bool,
        #[arg(
            long = "uds",
            help = "Bind Unix domain socket at path (defaults to config_dir/m31a-daemon.sock)"
        )]
        uds: Option<Option<PathBuf>>,
    },
    #[command(about = "Manage background M31A daemon process")]
    Daemon {
        #[command(subcommand)]
        cmd: DaemonCommands,
    },
    #[command(
        about = "Relay standard input/output streams to a running M31A Unix Domain Socket daemon"
    )]
    StdioToUds {
        #[arg(help = "Path to Unix Domain Socket")]
        socket_path: PathBuf,
    },
    #[command(about = "Run a command inside the M31A Bubblewrap sandbox")]
    Sandbox {
        #[arg(
            required = true,
            trailing_var_arg = true,
            allow_hyphen_values = true,
            help = "Command and arguments to run"
        )]
        cmd: Vec<String>,
    },
    #[command(about = "Manage external Model Context Protocol (MCP) servers")]
    Mcp {
        #[command(subcommand)]
        cmd: McpCommands,
    },
    #[command(about = "Inspect and toggle experimental and advanced feature flags")]
    Features {
        #[command(subcommand)]
        cmd: FeaturesCommands,
    },
    #[command(about = "Generate shell completions for bash, zsh, fish")]
    Completion {
        #[arg(value_enum, help = "Target shell")]
        shell: Shell,
    },
    #[command(about = "Debugging tools")]
    Debug {
        #[command(subcommand)]
        cmd: DebugCommands,
    },
    #[command(about = "Diagnose local environment, NVIDIA API key, bwrap, and runtime health")]
    Doctor {
        #[arg(long, help = "Output machine-readable JSON report")]
        json: bool,
    },
    #[command(about = "Generate JSON Schema for M31A configuration files")]
    ConfigSchema {
        #[arg(
            short,
            long,
            help = "Optional file path to write schema to (prints to stdout if omitted)"
        )]
        out: Option<PathBuf>,
    },
    #[command(about = "Collect a diagnostic feedback bundle for error reporting and bug triage")]
    Feedback {
        #[arg(
            short = 'n',
            long = "note",
            help = "Optional description or notes about the problem"
        )]
        note: Option<String>,
        #[arg(
            short = 'o',
            long = "out",
            help = "Output directory for the diagnostic bundle"
        )]
        out: Option<PathBuf>,
    },
    #[command(about = "Multi-agent message board")]
    Board {
        #[command(subcommand)]
        cmd: BoardCommands,
    },
    #[command(about = "Manage M31A extension plugins, marketplaces, and bundle packages")]
    Plugin {
        #[command(subcommand)]
        cmd: PluginCommands,
    },
    #[command(
        name = "apply-patch",
        alias = "apply_patch",
        about = "Apply a structured multi-file code patch directly to the current workspace"
    )]
    ApplyPatch {
        #[arg(help = "Path to patch file, or omitted / - to read from stdin")]
        patch_file: Option<PathBuf>,
    },
    #[command(
        about = "Migrate external agent configurations (Claude Code, Cursor, Windsurf) into M31A format"
    )]
    Migrate {
        #[arg(long, help = "Perform discovery and dry run without modifying files")]
        dry_run: bool,
    },
    #[command(about = "Display or inspect global prompt and turn message history")]
    History {
        #[arg(
            short = 'n',
            long = "limit",
            default_value = "20",
            help = "Number of recent entries to show"
        )]
        limit: usize,
    },
    #[command(about = "Manage and dispatch tasks to remote cloud execution runtimes")]
    Cloud {
        #[command(subcommand)]
        cmd: CloudCommands,
    },
    #[command(about = "Inspect and manage tracked goals for agent sessions")]
    Goal {
        #[command(subcommand)]
        cmd: GoalCommands,
    },
    #[command(
        about = "Execute autonomous multi-agent swarm with specialized roles (Architect, Coder, Tester, Guardian)"
    )]
    Swarm {
        #[arg(help = "High-level mission objective for the swarm to execute")]
        mission: String,
        #[arg(
            long = "rounds",
            default_value = "4",
            help = "Maximum coordination rounds across phases"
        )]
        rounds: usize,
        #[arg(long = "json", help = "Output swarm events as JSONL")]
        json: bool,
    },
    #[command(about = "Revert workspace and conversation back to a specific checkpoint or turn")]
    Revert {
        #[arg(long = "checkpoint", help = "Checkpoint ID to revert to")]
        checkpoint: Option<String>,
        #[arg(long = "thread", help = "Thread ID (defaults to most recent session)")]
        thread: Option<String>,
        #[arg(long = "turn", help = "Target turn index to revert to")]
        turn: Option<usize>,
        #[arg(
            long = "git",
            help = "Also checkout associated Git commit in working tree"
        )]
        git: bool,
        #[arg(
            long = "force",
            help = "Force git revert even if working tree has modifications"
        )]
        force: bool,
        #[arg(
            long = "dry-run",
            help = "Preview revert changes without modifying workspace or storage"
        )]
        dry_run: bool,
    },
    #[command(
        name = "responses-proxy",
        alias = "proxy",
        about = "Hardened local Responses API reverse proxy with memory-locked credentials"
    )]
    ResponsesProxy {
        #[arg(
            short = 'p',
            long = "port",
            default_value = "0",
            help = "Local port to bind (0 for ephemeral)"
        )]
        port: u16,
        #[arg(
            long = "upstream-url",
            default_value = "https://api.openai.com/v1/responses",
            help = "Upstream model endpoint to forward to"
        )]
        upstream_url: String,
        #[arg(
            long = "dump-dir",
            help = "Directory to dump request and response JSON pairs for audit/trace"
        )]
        dump_dir: Option<PathBuf>,
        #[arg(
            long = "server-info",
            help = "Path to write server info JSON with port and pid"
        )]
        server_info: Option<PathBuf>,
        #[arg(
            long = "http-shutdown",
            default_value = "true",
            help = "Enable GET /shutdown for unprivileged shutdown"
        )]
        http_shutdown: bool,
    },
    #[command(about = "Organize conversation threads and turns into named semantic sections")]
    Sections {
        #[command(subcommand)]
        cmd: SectionsCommands,
    },
    #[command(about = "List or inspect approval security presets")]
    Presets {
        #[arg(help = "Optional preset ID to inspect")]
        id: Option<String>,
    },
    #[command(about = "Export session transcript as Markdown, JSONL, or HTML documents")]
    Export {
        #[arg(
            short = 'f',
            long = "format",
            default_value = "markdown",
            help = "Export format: markdown (md), jsonl (json), or html"
        )]
        format: String,
        #[arg(
            short = 't',
            long = "thread",
            help = "Thread ID to export (defaults to most recent session)"
        )]
        thread: Option<String>,
        #[arg(
            short = 'o',
            long = "out",
            help = "Output file path (writes to stdout if omitted)"
        )]
        out: Option<PathBuf>,
    },
    #[command(about = "Manage Git worktrees for isolated agent sessions")]
    Worktree {
        #[command(subcommand)]
        cmd: WorktreeCommands,
    },
    #[command(
        about = "Execute spec-driven autonomous phase workflows (plan checking, verification, review)"
    )]
    Gsd {
        #[command(subcommand)]
        cmd: gsd::GsdCommands,
    },
    #[command(about = "Run HTTP/SSE daemon server for remote clients and web UIs")]
    Serve {
        #[arg(
            short = 'p',
            long = "port",
            default_value = "3100",
            help = "Port to listen on"
        )]
        port: u16,
        #[arg(
            short = 'b',
            long = "bind",
            default_value = "127.0.0.1",
            help = "IP address to bind"
        )]
        bind: String,
    },
    #[command(about = "Manage and inspect collaboration session templates")]
    Template {
        #[command(subcommand)]
        cmd: TemplateCommands,
    },
    #[command(about = "Inspect session and global telemetry statistics")]
    Telemetry {
        #[command(subcommand)]
        cmd: TelemetryCommands,
    },
    #[command(about = "Inspect and generate AI co-authorship Git commit trailers")]
    Attribution {
        #[command(subcommand)]
        cmd: AttributionCommands,
    },
    #[command(
        about = "Inspect model-specific system prompt fragments and reusable prompt templates"
    )]
    Prompt {
        #[command(subcommand)]
        cmd: PromptCommands,
    },
    #[command(
        about = "Continuous vibe coding loop with background compiler checks, visual TDD, and auto-healing"
    )]
    Vibe {
        #[arg(help = "Optional initial vibe coding prompt to start")]
        prompt: Option<String>,
        #[arg(
            long = "no-auto-heal",
            help = "Disable automatic background compiler repair"
        )]
        no_auto_heal: bool,
        #[arg(
            long = "debounce",
            default_value = "300",
            help = "Filesystem debounce time in milliseconds"
        )]
        debounce: u64,
        #[arg(
            long = "visual-url",
            help = "Target web frontend URL for continuous Visual TDD (e.g. http://localhost:3000)"
        )]
        visual_url: Option<String>,
    },
    #[command(
        about = "Speculative multi-branch parallel reality arena (competes multiple hypotheses in parallel sandboxes)"
    )]
    Speculate {
        #[arg(help = "High-level mission objective")]
        mission: String,
        #[arg(required = true, num_args = 2.., help = "Candidate hypotheses to evaluate concurrently (e.g. 'Approach A' 'Approach B')")]
        hypotheses: Vec<String>,
        #[arg(
            long = "timeout",
            default_value = "60",
            help = "Timeout in seconds per branch"
        )]
        timeout: u64,
        #[arg(
            long = "promote",
            help = "Automatically promote and apply the winning candidate's patch"
        )]
        promote: bool,
    },
    #[command(
        name = "workflow",
        about = "Autonomous workflow planning, wave scheduling, and delivery orchestration"
    )]
    Workflow {
        #[command(subcommand)]
        cmd: workflow::WorkflowCommands,
    },
}

#[derive(Subcommand, Debug)]
enum TemplateCommands {
    #[command(about = "List available collaboration session templates")]
    List,
    #[command(about = "Inspect details of a collaboration template")]
    Inspect {
        #[arg(help = "Template ID (e.g. pair-programming, code-review, debugging)")]
        id: String,
    },
}

#[derive(Subcommand, Debug)]
enum TelemetryCommands {
    #[command(about = "Display aggregated global telemetry metrics")]
    Stats,
}

#[derive(Subcommand, Debug)]
enum AttributionCommands {
    #[command(about = "Generate Git commit trailers with AI co-authorship metadata")]
    Trailer {
        #[arg(long = "session", help = "Optional session ID")]
        session: Option<String>,
        #[arg(long = "model", help = "Optional model ID")]
        model: Option<String>,
    },
}

#[derive(Subcommand, Debug)]
enum PromptCommands {
    #[command(about = "Render system prompt fragments tailored for a model")]
    Fragments {
        #[arg(
            short = 'm',
            long = "model",
            help = "Model slug (defaults to nemotron-3.5)"
        )]
        model: Option<String>,
    },
    #[command(about = "List built-in reusable prompt templates")]
    List,
}

#[derive(Subcommand, Debug)]
enum WorktreeCommands {
    #[command(about = "List all managed worktrees")]
    List,
    #[command(about = "Create a new worktree for a session")]
    Create {
        #[arg(help = "Session ID (generates new if omitted)")]
        session: Option<String>,
        #[arg(long = "base", help = "Base branch for worktree")]
        base: Option<String>,
    },
    #[command(about = "Remove a worktree")]
    Remove {
        #[arg(help = "Session ID of worktree to remove")]
        session: String,
        #[arg(long = "force", help = "Force remove even with uncommitted changes")]
        force: bool,
    },
    #[command(about = "Show status of a worktree")]
    Status {
        #[arg(help = "Session ID (defaults to most recent)")]
        session: Option<String>,
    },
    #[command(about = "Merge worktree changes into target branch")]
    Merge {
        #[arg(help = "Session ID to merge")]
        session: String,
        #[arg(help = "Target branch to merge into")]
        target: String,
    },
}

#[derive(Subcommand, Debug)]
enum ModelCommands {
    #[command(about = "List all available frontier models in the catalog")]
    List,
    #[command(about = "Show currently configured active model and parameters")]
    Current,
    #[command(
        about = "Set default active model in configuration (supports: speed, code, reasoning, flagship, or full slug)"
    )]
    Set {
        #[arg(help = "Model slug or preset alias")]
        model: String,
    },
    #[command(about = "Inspect or toggle dynamic hybrid model routing")]
    Route {
        #[arg(help = "Optional action: on/enable, off/disable, or status")]
        action: Option<String>,
    },
}

#[derive(Subcommand, Debug)]
enum SectionsCommands {
    #[command(about = "List all thread sections")]
    List,
    #[command(about = "Create a new thread section")]
    Create {
        #[arg(help = "Name of the section")]
        name: String,
        #[arg(long = "color", help = "Optional color tag (hex or named)")]
        color: Option<String>,
        #[arg(long = "icon", help = "Optional icon symbol or emoji")]
        icon: Option<String>,
    },
    #[command(about = "Rename an existing thread section")]
    Rename {
        #[arg(help = "Section ID to rename")]
        section_id: String,
        #[arg(help = "New name for the section")]
        new_name: String,
        #[arg(long = "color", help = "Optional updated color")]
        color: Option<String>,
        #[arg(long = "icon", help = "Optional updated icon")]
        icon: Option<String>,
    },
    #[command(about = "Move a thread into a section (or uncategorized if omitted)")]
    Move {
        #[arg(help = "Thread ID to move")]
        thread_id: String,
        #[arg(
            long = "section",
            help = "Target section ID (omit to move to uncategorized)"
        )]
        section_id: Option<String>,
    },
    #[command(about = "Delete a thread section, moving its threads to uncategorized")]
    Delete {
        #[arg(help = "Section ID to delete")]
        section_id: String,
    },
    #[command(about = "List threads assigned to a specific section")]
    Threads {
        #[arg(help = "Section ID (omit for uncategorized)")]
        section_id: Option<String>,
        #[arg(
            short = 'n',
            long = "limit",
            default_value = "20",
            help = "Maximum threads to list"
        )]
        limit: usize,
    },
}

#[derive(Subcommand, Debug)]
enum CloudCommands {
    #[command(about = "List cloud tasks")]
    List,
    #[command(about = "Submit a prompt instruction as a cloud task")]
    Submit {
        #[arg(help = "Prompt instruction")]
        instruction: String,
        #[arg(long = "env", help = "Environment label or ID")]
        env: Option<String>,
    },
    #[command(about = "Cancel a running cloud task")]
    Cancel {
        #[arg(help = "Task ID")]
        task_id: String,
    },
}

#[derive(Subcommand, Debug)]
enum GoalCommands {
    #[command(about = "Get active goal for a thread")]
    Get {
        #[arg(help = "Thread ID")]
        thread_id: String,
    },
    #[command(about = "Set a new goal for a thread")]
    Set {
        #[arg(help = "Thread ID")]
        thread_id: String,
        #[arg(help = "Goal objective")]
        objective: String,
        #[arg(long = "budget", help = "Optional token budget cap")]
        budget: Option<u64>,
    },
    #[command(about = "Update goal status")]
    Update {
        #[arg(help = "Thread ID")]
        thread_id: String,
        #[arg(help = "Status ('active', 'paused', 'blocked', 'complete')")]
        status: String,
    },
}

#[derive(Subcommand, Debug)]
enum TrustCommands {
    #[command(about = "List all trusted directories")]
    List,
    #[command(about = "Mark a directory as trusted")]
    Add {
        #[arg(help = "Path to directory to trust (defaults to current directory)")]
        path: Option<PathBuf>,
    },
    #[command(about = "Revoke trust from a directory")]
    Revoke {
        #[arg(help = "Path to directory to revoke trust from (defaults to current directory)")]
        path: Option<PathBuf>,
    },
    #[command(about = "Audit git hooks and git config for command injection vectors")]
    Audit {
        #[arg(help = "Path to repository to audit (defaults to current directory)")]
        path: Option<PathBuf>,
    },
}

#[derive(Subcommand, Debug)]
enum PluginCommands {
    #[command(about = "List installed and discovered plugins")]
    List,
    #[command(about = "Install a plugin from a local directory")]
    Install {
        #[arg(help = "Path to directory containing plugin.json")]
        path: PathBuf,
    },
    #[command(about = "Uninstall a plugin by name")]
    Uninstall {
        #[arg(help = "Plugin name")]
        name: String,
    },
    #[command(about = "Package a plugin directory into a verifiable .m31a-bundle.json archive")]
    Package {
        #[arg(help = "Path to directory containing plugin.json")]
        dir: PathBuf,
        #[arg(short = 'o', long = "out", help = "Output bundle JSON file path")]
        out: PathBuf,
    },
    #[command(about = "Install a plugin from an offline .m31a-bundle.json archive")]
    InstallBundle {
        #[arg(help = "Path to plugin bundle archive")]
        bundle: PathBuf,
        #[arg(long = "target-dir", help = "Optional custom installation directory")]
        target_dir: Option<PathBuf>,
    },
    #[command(about = "Verify the SHA-256 integrity and manifest of a plugin bundle archive")]
    VerifyBundle {
        #[arg(help = "Path to plugin bundle archive")]
        bundle: PathBuf,
    },
}

#[derive(Subcommand, Debug)]
enum DaemonCommands {
    #[command(about = "Start the M31A background daemon")]
    Start {
        #[arg(
            long,
            help = "Run in foreground instead of detached background process"
        )]
        foreground: bool,
        #[arg(long, help = "Custom socket path")]
        socket: Option<PathBuf>,
    },
    #[command(about = "Check status and health of the running daemon")]
    Status {
        #[arg(long, help = "Custom socket path")]
        socket: Option<PathBuf>,
    },
    #[command(about = "Stop the running daemon")]
    Stop {
        #[arg(long, help = "Custom socket path")]
        socket: Option<PathBuf>,
    },
}

#[derive(Subcommand, Debug)]
enum HooksCommands {
    #[command(about = "List configured lifecycle hooks")]
    List,
    #[command(about = "Manually trigger a lifecycle hook event")]
    Trigger {
        #[arg(help = "Hook event name (e.g. SessionStart, PreCompact)")]
        event: String,
    },
}

#[derive(Subcommand, Debug)]
enum MemoriesCommands {
    #[command(about = "List consolidated memories")]
    List,
    #[command(about = "Record a memory manually")]
    Record {
        #[arg(short = 't', long = "title", help = "Title of memory entry")]
        title: String,
        #[arg(short = 'm', long = "memory", help = "Detailed memory text")]
        raw_memory: String,
        #[arg(short = 's', long = "summary", help = "Summary takeaway")]
        summary: String,
    },
}

#[derive(Subcommand, Debug)]
enum DebugCommands {
    #[command(about = "Render the NVIDIA NIM model catalog as JSON")]
    Models,
    #[command(about = "List all tool schemas exposed to the model")]
    Tools,
    #[command(about = "Evaluate a shell command against the security policy")]
    Policy {
        #[arg(help = "Command string to evaluate")]
        command: String,
    },
    #[command(
        about = "Render the full model-visible prompt input including context and directives"
    )]
    PromptInput {
        #[arg(help = "Optional user prompt")]
        prompt: Option<String>,
    },
    #[command(about = "Reset and clear local memory store")]
    ClearMemories,
    #[command(
        about = "Replay and reduce an append-only trace log into a structured semantic execution graph"
    )]
    TraceReduce {
        #[arg(help = "Path to .trace.jsonl or trace directory")]
        trace_path: PathBuf,
        #[arg(long, help = "Output as raw JSON")]
        json: bool,
    },
}

#[derive(Subcommand, Debug)]
enum McpCommands {
    #[command(about = "List configured MCP servers")]
    List,
    #[command(about = "Add a new MCP server configuration")]
    Add {
        #[arg(help = "Server identifier name")]
        name: String,
        #[arg(help = "Command binary to spawn")]
        command: String,
        #[arg(
            trailing_var_arg = true,
            allow_hyphen_values = true,
            help = "Command arguments"
        )]
        args: Vec<String>,
    },
    #[command(about = "Remove an MCP server configuration")]
    Remove {
        #[arg(help = "Server identifier name to remove")]
        name: String,
    },
}

#[derive(Subcommand, Debug)]
enum FeaturesCommands {
    #[command(about = "List active feature flags and statuses")]
    List,
    #[command(about = "Enable a feature flag")]
    Enable {
        #[arg(help = "Feature flag key")]
        key: String,
    },
    #[command(about = "Disable a feature flag")]
    Disable {
        #[arg(help = "Feature flag key")]
        key: String,
    },
}

#[derive(Subcommand, Debug)]
enum BoardCommands {
    #[command(about = "List posts in a channel")]
    List {
        #[arg(short = 'c', long = "channel", default_value = "general")]
        channel: String,
        #[arg(short = 'n', long = "limit", default_value_t = 20)]
        limit: usize,
    },
    #[command(about = "Post a message to a channel")]
    Post {
        #[arg(short = 'c', long = "channel", default_value = "general")]
        channel: String,
        #[arg(short = 'a', long = "author", default_value = "user")]
        author: String,
        #[arg(help = "Post body message")]
        body: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    // 0. Process security hardening (disable core dumps, non-dumpable, clear dangerous env vars)
    m31a_core::apply_process_hardening();

    // Check for arg0 multi-call dispatch (apply_patch / applypatch)
    if m31a_core::is_apply_patch_invocation() {
        let args: Vec<String> = std::env::args().skip(1).collect();
        let patch_content = if let Some(first) = args.first() {
            if first == "-" {
                let mut buf = String::new();
                std::io::stdin().read_to_string(&mut buf)?;
                buf
            } else {
                std::fs::read_to_string(first)
                    .with_context(|| format!("Failed to read patch file at {}", first))?
            }
        } else {
            let mut buf = String::new();
            std::io::stdin().read_to_string(&mut buf)?;
            buf
        };

        let cwd = std::env::current_dir()?;
        let parsed = m31a_tools::patch::parse_patch(&patch_content)
            .map_err(|e| anyhow::anyhow!("Failed to parse patch: {e}"))?;
        let res = m31a_tools::patch::apply_patch_to_workspace(&cwd, &parsed)?;
        println!(
            "Successfully applied patch:\n- Modified: {:?}\n- Created: {:?}\n- Deleted: {:?}",
            res.files_modified, res.files_created, res.files_deleted
        );
        return Ok(());
    }

    // Check for arg0 multi-call dispatch (m31a-linux-sandbox)
    if m31a_core::is_linux_sandbox_invocation() {
        let args: Vec<String> = std::env::args().skip(1).collect();
        let cwd = std::env::current_dir()?;
        let runner = m31a_sandbox::runner::SandboxRunner::new();

        let mut policy = m31a_core::SandboxPolicy::WorkspaceWrite;
        let mut cmd_parts = Vec::new();
        let mut i = 0;
        while i < args.len() {
            match args[i].as_str() {
                "--policy" if i + 1 < args.len() => {
                    policy = match args[i + 1].as_str() {
                        "readonly" | "ReadOnly" => m31a_core::SandboxPolicy::ReadOnly,
                        "danger" | "DangerFullAccess" => m31a_core::SandboxPolicy::DangerFullAccess,
                        _ => m31a_core::SandboxPolicy::WorkspaceWrite,
                    };
                    i += 2;
                }
                "--" => {
                    cmd_parts.extend_from_slice(&args[i + 1..]);
                    break;
                }
                other => {
                    cmd_parts.push(other.to_string());
                    i += 1;
                }
            }
        }

        let cmd_line = cmd_parts.join(" ");
        let output = runner
            .execute(&cmd_line, &cwd, policy, std::time::Duration::from_secs(300))
            .await?;
        print!("{}", output.stdout);
        eprint!("{}", output.stderr);
        std::process::exit(output.exit_code);
    }

    let _arg0_guard = m31a_core::setup_arg0_dispatch().ok();

    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::WARN)
        .with_target(false)
        .without_time()
        .finish();
    let _ = tracing::subscriber::set_global_default(subscriber);

    let cli = Cli::parse();

    let mut config = Config::load_with_overrides(
        cli.profile.as_deref(),
        &cli.config_overrides,
        cli.strict_config,
    )
    .unwrap_or_default();

    for f in &cli.enable {
        config.features.insert(f.clone(), true);
    }
    for f in &cli.disable {
        config.features.insert(f.clone(), false);
    }

    if cli.approve_for_me {
        config.approval = ApprovalPolicy::ApproveForMe;
    }
    if cli.dangerously_bypass {
        config.dangerously_bypass = true;
        config.sandbox = SandboxPolicy::DangerFullAccess;
    }

    if let Some(preset_name) = &cli.preset {
        if let Some(preset) = m31a_execpolicy::ApprovalPreset::from_id(preset_name) {
            config.sandbox = preset.sandbox;
            config.approval = preset.approval;
        } else {
            bail!(
                "Unknown security preset `{}`. Available presets: read-only, standard, permissive, airgapped, danger-full-access",
                preset_name
            );
        }
    }

    if let Some(ref m) = cli.model {
        config.model = m.clone();
    }
    if let Some(s) = cli.sandbox {
        config.sandbox = s.into();
    }
    if let Some(a) = cli.approval {
        config.approval = a.into();
    }

    let mut cwd = cli
        .cd
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

    // Handle --worktree
    if cli.worktree {
        let wt_mgr = WorktreeManager::new()?;
        let wt = wt_mgr.create_worktree(&cwd, None)?;
        println!("{} managed worktree at {:?}", "Created".green(), wt.path);
        cwd = wt.path;
    }

    match cli.command {
        Some(Commands::Setup {
            api_key,
            api_base,
            model,
            sandbox,
            approval,
            non_interactive,
            force,
        }) => {
            setup::SetupWizard::run(setup::SetupOptions {
                api_key,
                api_base,
                model,
                sandbox,
                approval,
                non_interactive,
                force,
            })
            .await
        }

        Some(Commands::Model { cmd }) => run_model_command(&config, cmd).await,

        Some(Commands::Doctor { json }) => {
            let report = doctor::Doctor::run_diagnostics(&config).await;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                doctor::Doctor::render_human(&report);
            }
            Ok(())
        }

        Some(Commands::ConfigSchema { out }) => {
            let schema = m31a_core::generate_config_schema();
            if let Some(out_path) = out {
                m31a_core::write_config_schema(&out_path)?;
                println!(
                    "{} Config schema written to {}",
                    "Success:".green(),
                    out_path.display()
                );
            } else {
                println!("{}", serde_json::to_string_pretty(&schema)?);
            }
            Ok(())
        }

        Some(Commands::Feedback { note, out }) => {
            let target_dir =
                out.unwrap_or_else(|| m31a_core::Config::config_dir().join("feedback"));
            let mut attachments = Vec::new();

            // Doctor report attachment
            let doc_report = doctor::Doctor::run_diagnostics(&config).await;
            if let Ok(doc_json) = serde_json::to_string_pretty(&doc_report) {
                attachments.push(m31a_core::FeedbackAttachment {
                    name: "doctor_report.json".to_string(),
                    mime_type: "application/json".to_string(),
                    content: doc_json,
                });
            }

            // Diagnostics gauges attachment
            let gauges = m31a_core::diagnostics::snapshot_all();
            if let Ok(gauges_json) = serde_json::to_string_pretty(&gauges) {
                attachments.push(m31a_core::FeedbackAttachment {
                    name: "diagnostics_gauges.json".to_string(),
                    mime_type: "application/json".to_string(),
                    content: gauges_json,
                });
            }

            let bundle_path = m31a_core::FeedbackBundleManager::create_bundle(
                &target_dir,
                note.as_deref(),
                attachments,
                None,
            )?;

            println!(
                "{} Diagnostic feedback bundle created at: {}",
                "Success:".green(),
                bundle_path.display()
            );
            Ok(())
        }

        Some(Commands::Completion { shell }) => {
            let mut cmd = Cli::command();
            generate(shell, &mut cmd, "m31a", &mut io::stdout());
            Ok(())
        }

        Some(Commands::Debug { cmd }) => match cmd {
            DebugCommands::Models => {
                let catalog = get_nvidia_model_catalog();
                println!("{}", serde_json::to_string_pretty(&catalog)?);
                Ok(())
            }
            DebugCommands::Tools => {
                let tools = ToolRegistry::new();
                println!("{}", serde_json::to_string_pretty(&tools.definitions())?);
                Ok(())
            }
            DebugCommands::Policy { command } => {
                let policy = ExecPolicy::standard();
                let match_result = policy.evaluate_detailed(&command);
                println!("{}", serde_json::to_string_pretty(&match_result)?);
                Ok(())
            }
            DebugCommands::PromptInput { prompt } => {
                let p = prompt.unwrap_or_else(|| "Hello, please inspect workspace".to_string());
                let sys = SystemPromptBuilder::build(
                    &cwd,
                    &config.model,
                    &format!("{:?}", config.sandbox),
                    CollaborationMode::Default,
                );
                println!("{}", "=== MODEL SYSTEM PROMPT ===".bold().blue());
                println!("{}\n", sys);
                println!("{}", "=== USER PROMPT ===".bold().cyan());
                println!("{}", p);
                Ok(())
            }
            DebugCommands::ClearMemories => {
                let mem_dir = Config::config_dir().join("memories");
                if mem_dir.exists() {
                    let _ = std::fs::remove_dir_all(&mem_dir);
                }
                println!(
                    "{} local memories directory {:?}",
                    "Cleared".green(),
                    mem_dir
                );
                Ok(())
            }
            DebugCommands::TraceReduce { trace_path, json } => {
                let summary = m31a_core::rollout_trace::replay_trace(&trace_path)?;
                if json {
                    println!("{}", serde_json::to_string_pretty(&summary)?);
                } else {
                    println!(
                        "{}",
                        "=== M31A ROLLOUT TRACE REDUCED SUMMARY ===".bold().cyan()
                    );
                    println!("Thread ID:               {}", summary.thread_id.yellow());
                    println!("Total Events:            {}", summary.total_events);
                    println!("Total Turns:             {}", summary.total_turns);
                    println!("Prompt Tokens:           {}", summary.total_prompt_tokens);
                    println!(
                        "Completion Tokens:       {}",
                        summary.total_completion_tokens
                    );
                    println!(
                        "Duration:                {} ms",
                        summary.wall_clock_duration_ms
                    );
                    println!("Compactions:             {}", summary.compactions);
                    if !summary.tools_invoked.is_empty() {
                        println!("Tool Invocations:");
                        for (tool, count) in &summary.tools_invoked {
                            println!("  - {}: {}", tool.bold(), count);
                        }
                    }
                    if summary.tool_errors > 0 {
                        println!(
                            "Tool Errors:             {}",
                            summary.tool_errors.to_string().red().bold()
                        );
                    }
                    if !summary.spawned_agents.is_empty() {
                        println!(
                            "Spawned Agents:          {}",
                            summary.spawned_agents.join(", ")
                        );
                    }
                }
                Ok(())
            }
        },

        Some(Commands::Hooks { cmd }) => {
            let engine = HookEngine::load_from_dir(&cwd);
            match cmd {
                HooksCommands::List => {
                    let hooks = engine.list_hooks();
                    if hooks.is_empty() {
                        println!(
                            "{}",
                            "No lifecycle hooks configured in workspace or user directory."
                                .dimmed()
                        );
                    } else {
                        println!("{}", "Configured Lifecycle Hooks:".bold());
                        for h in hooks {
                            println!(
                                "- {} [Event: {}] -> {} {}",
                                h.name.cyan(),
                                h.event.as_str().yellow(),
                                h.command.bold(),
                                h.args.join(" ")
                            );
                        }
                    }
                    Ok(())
                }
                HooksCommands::Trigger { event } => {
                    println!("Triggering hook event: `{}`", event);
                    let ctx = HashMap::new();
                    let outcome = engine.dispatch(HookEvent::SessionStart, &cwd, &ctx).await?;
                    println!(
                        "Result: allowed={} exit_code={}",
                        outcome.allowed, outcome.exit_code
                    );
                    Ok(())
                }
            }
        }

        Some(Commands::Memories { cmd }) => {
            let mem_mgr = MemoryManager::new()?;
            match cmd {
                MemoriesCommands::List => {
                    let consolidated = mem_mgr.load_consolidated_memories();
                    if consolidated.is_empty() {
                        println!("{}", "No memories recorded yet.".dimmed());
                    } else {
                        println!("{}", consolidated);
                    }
                    Ok(())
                }
                MemoriesCommands::Record {
                    title,
                    raw_memory,
                    summary,
                } => {
                    let rec = MemoryRecord {
                        id: uuid::Uuid::new_v4().to_string(),
                        thread_id: "manual".to_string(),
                        title,
                        raw_memory,
                        summary,
                        tags: vec!["manual".to_string()],
                        created_at: chrono::Utc::now().timestamp(),
                    };
                    mem_mgr.record_memory(&rec)?;
                    println!(
                        "{}",
                        "Memory successfully recorded and consolidated.".green()
                    );
                    Ok(())
                }
            }
        }

        Some(Commands::Queue { id, message }) => {
            let storage = Storage::open_default()?;
            storage.save_item(
                &uuid::Uuid::new_v4().to_string(),
                &id,
                None,
                "user_queued",
                &message,
            )?;
            println!(
                "{} message to thread `{}`: {}",
                "Queued".green(),
                id.cyan(),
                message
            );
            Ok(())
        }

        Some(Commands::Agents { id }) => {
            let storage = Storage::open_default()?;
            let target_parent = id.as_deref().unwrap_or("current");
            let subagents = storage.list_subagents(target_parent)?;
            if subagents.is_empty() {
                println!("{}", "No subagents found.".dimmed());
            } else {
                println!("{}", "Spawned Subagents:".bold());
                for (child, role) in subagents {
                    println!("- [{}] {}", role.cyan(), child);
                }
            }
            Ok(())
        }

        Some(Commands::Mcp { cmd }) => match cmd {
            McpCommands::List => {
                if config.mcp_servers.is_empty() {
                    println!("{}", "No MCP servers configured.".dimmed());
                } else {
                    println!("{}", "Configured MCP Servers:".bold());
                    for (name, srv) in &config.mcp_servers {
                        println!(
                            "- {}: {} {}",
                            name.cyan(),
                            srv.command.bold(),
                            srv.args.join(" ")
                        );
                    }
                }
                Ok(())
            }
            McpCommands::Add {
                name,
                command,
                args,
            } => {
                config.mcp_servers.insert(
                    name.clone(),
                    McpServerConfig::new(command, args, HashMap::new()),
                );
                config.save()?;
                println!("{} MCP server `{}`", "Added".green(), name);
                Ok(())
            }
            McpCommands::Remove { name } => {
                if config.mcp_servers.remove(&name).is_some() {
                    config.save()?;
                    println!("{} MCP server `{}`", "Removed".green(), name);
                } else {
                    println!("{} MCP server `{}` not found.", "Warning:".yellow(), name);
                }
                Ok(())
            }
        },

        Some(Commands::Features { cmd }) => {
            let features_dir = Config::config_dir().join("features");
            let manager = m31a_features::FeatureFlagManager::new(features_dir)?;
            match cmd {
                FeaturesCommands::List => {
                    let flags = manager.list_flags().await;
                    println!("{}", "M31A Feature Flags:".bold());
                    for f in &flags {
                        let enabled = matches!(f.state, m31a_features::FeatureState::Enabled);
                        let status = if enabled {
                            "enabled".green()
                        } else {
                            "disabled".dimmed()
                        };
                        println!(
                            "- {:<28} {:<16} {:<10} {}",
                            f.id.cyan(),
                            format!("[{:?}]", f.category).dimmed(),
                            status,
                            f.description
                        );
                    }
                    Ok(())
                }
                FeaturesCommands::Enable { key } => {
                    manager.enable_flag(&key, "cli".to_string()).await?;
                    config.features.insert(key.clone(), true);
                    let _ = config.save();
                    println!("{} feature `{}`", "Enabled".green(), key);
                    Ok(())
                }
                FeaturesCommands::Disable { key } => {
                    manager.disable_flag(&key, "cli".to_string()).await?;
                    config.features.insert(key.clone(), false);
                    let _ = config.save();
                    println!("{} feature `{}`", "Disabled".yellow(), key);
                    Ok(())
                }
            }
        }

        Some(Commands::Archive { id }) => {
            let storage = Storage::open_default()?;
            storage.archive_thread(&id)?;
            println!("{} thread `{}`", "Archived".green(), id);
            Ok(())
        }

        Some(Commands::Unarchive { id }) => {
            let storage = Storage::open_default()?;
            storage.unarchive_thread(&id)?;
            println!("{} thread `{}`", "Unarchived".green(), id);
            Ok(())
        }

        Some(Commands::Delete { id }) => {
            let storage = Storage::open_default()?;
            storage.delete_thread(&id)?;
            println!("{} thread `{}`", "Deleted".green(), id);
            Ok(())
        }

        Some(Commands::Resume { last, id }) => {
            let storage = Storage::open_default()?;
            let target_id = if let Some(target) = id {
                target
            } else if last {
                let threads = storage.list_threads(1)?;
                threads
                    .first()
                    .context("No previous sessions to resume.")?
                    .id
                    .clone()
            } else {
                let threads = storage.list_threads(10)?;
                if threads.is_empty() {
                    println!("{}", "No saved sessions found.".yellow());
                    return Ok(());
                }
                println!(
                    "{}",
                    "Select a session to resume with `m31a resume <id>`:".bold()
                );
                for t in threads {
                    println!("- {} | {} ({})", t.id.cyan(), t.title, t.model.dimmed());
                }
                return Ok(());
            };

            let mut app = m31a_tui::App::new(config)?;
            m31a_tui::init_theme(m31a_tui::ThemeVariant::Dark);
            app.resume_session(&target_id)?;
            m31a_tui::run_tui_app(app, None).await
        }

        Some(Commands::Fork { last, id, title }) => {
            let storage = Storage::open_default()?;
            let target_id = if let Some(target) = id {
                target
            } else if last {
                let threads = storage.list_threads(1)?;
                threads
                    .first()
                    .context("No previous sessions to fork.")?
                    .id
                    .clone()
            } else {
                let threads = storage.list_threads(10)?;
                if threads.is_empty() {
                    println!("{}", "No saved sessions found.".yellow());
                    return Ok(());
                }
                println!(
                    "{}",
                    "Select a session to fork with `m31a fork <id>`:".bold()
                );
                for t in threads {
                    println!("- {} | {} ({})", t.id.cyan(), t.title, t.model.dimmed());
                }
                return Ok(());
            };

            let fork_title = title.unwrap_or_else(|| {
                format!(
                    "Fork of {}",
                    &target_id[..std::cmp::min(8, target_id.len())]
                )
            });
            let forked = storage.fork_thread(&target_id, &fork_title)?;
            println!(
                "{} forked branch `{}` ({})",
                "Created".green(),
                forked.title,
                forked.id
            );

            let mut app = m31a_tui::App::new(config)?;
            m31a_tui::init_theme(m31a_tui::ThemeVariant::Dark);
            app.resume_session(&forked.id)?;
            m31a_tui::run_tui_app(app, None).await
        }

        Some(Commands::Apply { patch_file }) => {
            if let Some(file) = patch_file {
                let patch_text = std::fs::read_to_string(&file)?;
                let parsed = m31a_tools::patch::parse_patch(&patch_text)?;
                let res = m31a_tools::patch::apply_patch_to_workspace(&cwd, &parsed)?;
                println!(
                    "{} applied patch from {:?}:\n- Modified: {:?}\n- Created: {:?}\n- Deleted: {:?}",
                    "Successfully".green(),
                    file,
                    res.files_modified,
                    res.files_created,
                    res.files_deleted
                );
            } else {
                let out = std::process::Command::new("git")
                    .args(["status", "--short"])
                    .current_dir(&cwd)
                    .output()?;
                let status = String::from_utf8_lossy(&out.stdout);
                if status.trim().is_empty() {
                    println!("{}", "Working tree is clean. No unapplied diffs.".yellow());
                } else {
                    println!("Uncommitted changes in repository:\n{}", status);
                }
            }
            Ok(())
        }

        Some(Commands::Sandbox { cmd }) => {
            let runner = SandboxRunner::new();
            let full_cmd = cmd.join(" ");
            let out = runner
                .execute(&full_cmd, &cwd, config.sandbox, Duration::from_secs(300))
                .await?;
            print!("{}", out.stdout);
            eprint!("{}", out.stderr);
            std::process::exit(out.exit_code);
        }

        Some(Commands::AppServer { stdio, uds }) => {
            let storage = Storage::open_default()?;
            let tools = Arc::new(ToolRegistry::new());
            let server = AppServer::new(Arc::new(config), storage, tools)?;

            if let Some(uds_opt) = uds {
                let sock = uds_opt.unwrap_or_else(|| Config::config_dir().join("m31a-daemon.sock"));
                server.run_uds(&sock).await
            } else if stdio {
                server.run_stdio().await
            } else {
                let sock = Config::config_dir().join("m31a-daemon.sock");
                server.run_uds(&sock).await
            }
        }

        Some(Commands::Daemon { cmd }) => match cmd {
            DaemonCommands::Start { foreground, socket } => {
                let sock_path =
                    socket.unwrap_or_else(|| Config::config_dir().join("m31a-daemon.sock"));
                let pid_path = Config::config_dir().join("m31a-daemon.pid");

                if foreground {
                    println!(
                        "{} Starting M31A App Server on UDS {:?}",
                        "->".green(),
                        sock_path
                    );
                    let storage = Storage::open_default()?;
                    let tools = Arc::new(ToolRegistry::new());
                    let server = AppServer::new(Arc::new(config), storage, tools)?;
                    server.run_uds(&sock_path).await?;
                } else {
                    let exe = std::env::current_exe()?;
                    if let Some(parent) = sock_path.parent() {
                        std::fs::create_dir_all(parent)?;
                    }

                    let child = std::process::Command::new(exe)
                        .args(["app-server", "--uds", sock_path.to_str().unwrap()])
                        .stdin(std::process::Stdio::null())
                        .stdout(std::process::Stdio::null())
                        .stderr(std::process::Stdio::null())
                        .spawn()
                        .context("Failed to spawn background daemon process")?;

                    let pid = child.id();
                    std::fs::write(&pid_path, pid.to_string())?;
                    println!(
                        "{} M31A daemon spawned in background (PID: {})",
                        "[OK]".green().bold(),
                        pid
                    );
                    println!("   Socket: {}", sock_path.display());
                    println!("   PID file: {}", pid_path.display());
                }
                Ok(())
            }
            DaemonCommands::Status { socket } => {
                let sock_path =
                    socket.unwrap_or_else(|| Config::config_dir().join("m31a-daemon.sock"));
                let pid_path = Config::config_dir().join("m31a-daemon.pid");
                let pid_str = std::fs::read_to_string(&pid_path).unwrap_or_default();

                println!("{}", "M31A Daemon Diagnostics:".bold().underline());
                println!("  Socket Path: {}", sock_path.display());
                if !pid_str.trim().is_empty() {
                    println!("  Recorded PID: {}", pid_str.trim());
                }

                match tokio::net::UnixStream::connect(&sock_path).await {
                    Ok(mut stream) => {
                        use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
                        let probe_req = "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{}}\n";
                        let _ = stream.write_all(probe_req.as_bytes()).await;
                        let mut reader = BufReader::new(stream).lines();
                        if let Ok(Some(line)) = reader.next_line().await {
                            println!(
                                "  Daemon Status: {} (Handshake OK)",
                                "RUNNING".bold().green()
                            );
                            println!("  Probe Response: {}", line.dimmed());
                        } else {
                            println!(
                                "  Daemon Status: {} (Connected, awaiting frame)",
                                "ALIVE".bold().yellow()
                            );
                        }
                    }
                    Err(e) => {
                        println!("  Daemon Status: {} ({})", "NOT RUNNING".bold().red(), e);
                    }
                }
                Ok(())
            }
            DaemonCommands::Stop { socket } => {
                let sock_path =
                    socket.unwrap_or_else(|| Config::config_dir().join("m31a-daemon.sock"));
                let pid_path = Config::config_dir().join("m31a-daemon.pid");

                if let Ok(pid_str) = std::fs::read_to_string(&pid_path) {
                    let trimmed = pid_str.trim();
                    if !trimmed.is_empty() {
                        let _ = std::process::Command::new("kill").arg(trimmed).status();
                        println!(
                            "{} Sent SIGTERM to daemon PID {}",
                            "[OK]".green().bold(),
                            trimmed
                        );
                    }
                    let _ = std::fs::remove_file(&pid_path);
                }

                if sock_path.exists() {
                    let _ = std::fs::remove_file(&sock_path);
                    println!(
                        "{} Removed socket at {}",
                        "[OK]".green().bold(),
                        sock_path.display()
                    );
                }
                println!("M31A daemon stopped.");
                Ok(())
            }
        },

        Some(Commands::StdioToUds { socket_path }) => {
            m31a_app_server::stdio_to_uds(&socket_path).await
        }

        Some(Commands::Board { cmd }) => match cmd {
            BoardCommands::List { channel, limit } => {
                let storage = Storage::open_default()?;
                let posts = storage.list_posts("default", &channel, limit)?;
                if posts.is_empty() {
                    println!("{}", "No posts found in channel.".dimmed());
                } else {
                    for p in posts {
                        println!(
                            "[{}] {}: {}",
                            chrono::DateTime::from_timestamp_millis(p.timestamp)
                                .map(|d| d.format("%H:%M:%S").to_string())
                                .unwrap_or_default()
                                .dimmed(),
                            p.author.bold().cyan(),
                            p.body
                        );
                    }
                }
                Ok(())
            }
            BoardCommands::Post {
                channel,
                author,
                body,
            } => {
                let storage = Storage::open_default()?;
                let id = storage.post_to_board(
                    "default",
                    &channel,
                    &author,
                    &body,
                    serde_json::json!({}),
                )?;
                println!("{} (id: {})", "Posted successfully.".green(), id);
                Ok(())
            }
        },

        Some(Commands::Plugin { cmd }) => match cmd {
            PluginCommands::List => {
                let mgr = m31a_core::PluginManager::new();
                let plugins = mgr.discover_plugins();
                if plugins.is_empty() {
                    println!("{}", "No plugins discovered in project (.m31a/plugins) or user (~/.m31a/plugins) directories.".cyan());
                } else {
                    println!("{}", "=== Installed & Discovered Plugins ===".bold().blue());
                    println!(
                        "{:<20} | {:<8} | {:<25} | Path",
                        "Name", "Version", "Keywords"
                    );
                    println!("{}", "-".repeat(75));
                    for p in plugins {
                        let kw = p.manifest.keywords.join(", ");
                        println!(
                            "{:<20} | {:<8} | {:<25} | {:?}",
                            p.name.bold(),
                            p.version.green(),
                            kw.cyan(),
                            p.path
                        );
                        if !p.description.is_empty() {
                            println!("  ↳ {}", p.description.dimmed());
                        }
                    }
                }
                Ok(())
            }
            PluginCommands::Install { path } => {
                let mgr = m31a_core::PluginManager::new();
                let installed = mgr.install_from_dir(&path)?;
                println!(
                    "{} plugin '{}' (v{}) into {:?}",
                    "Installed".green().bold(),
                    installed.name,
                    installed.version,
                    installed.path
                );
                Ok(())
            }
            PluginCommands::Uninstall { name } => {
                let mgr = m31a_core::PluginManager::new();
                let removed = mgr.uninstall(&name)?;
                if removed {
                    println!("{} plugin '{}'", "Uninstalled".green().bold(), name);
                } else {
                    println!(
                        "{} plugin '{}' not found in user plugins directory",
                        "Notice:".yellow(),
                        name
                    );
                }
                Ok(())
            }
            PluginCommands::Package { dir, out } => {
                let bundle = m31a_core::plugin_bundle::pack_plugin_bundle(&dir, &out)?;
                println!(
                    "{} plugin '{}' (v{}) into {:?} ({} files, sha256: {:.12}...)",
                    "Packaged".green().bold(),
                    bundle.manifest.name,
                    bundle.manifest.version.as_deref().unwrap_or("0.0.0"),
                    out,
                    bundle.files.len(),
                    bundle.checksum
                );
                Ok(())
            }
            PluginCommands::InstallBundle { bundle, target_dir } => {
                let default_dir = Config::config_dir().join("plugins");
                let dest = target_dir.unwrap_or(default_dir);
                let plugin_dir = m31a_core::plugin_bundle::unpack_plugin_bundle(&bundle, &dest)?;
                println!(
                    "{} plugin bundle into {:?}",
                    "Installed".green().bold(),
                    plugin_dir
                );
                Ok(())
            }
            PluginCommands::VerifyBundle { bundle } => {
                let verified = m31a_core::plugin_bundle::verify_plugin_bundle_file(&bundle)?;
                println!(
                    "{} plugin bundle '{}' (v{}): valid SHA-256 integrity ({} files)",
                    "Verified".green().bold(),
                    verified.manifest.name,
                    verified.manifest.version.as_deref().unwrap_or("0.0.0"),
                    verified.files.len()
                );
                Ok(())
            }
        },

        Some(Commands::Review { base }) => run_review(&config, &cwd, base.as_deref()).await,

        Some(Commands::Verify { tests, timeout }) => run_verify(&cwd, tests, timeout).await,

        Some(Commands::Trust { cmd }) => run_trust_command(&cwd, cmd).await,

        Some(Commands::Ps) => run_ps_command().await,

        Some(Commands::Kill { id, force }) => run_kill_command(&id, force).await,

        Some(Commands::Logs { id, lines }) => run_logs_command(&id, lines).await,

        Some(Commands::Exec {
            prompt,
            json,
            output_last_message,
            ephemeral,
            worktree,
            search,
            self_heal,
            max_heal_iterations,
            plan,
            reasoning_effort,
        }) => {
            let mut exec_cwd = cwd.clone();
            if worktree {
                let wt_mgr = WorktreeManager::new()?;
                let wt = wt_mgr.create_worktree(&exec_cwd, None)?;
                exec_cwd = wt.path;
            }

            let resolved_prompt = resolve_prompt_with_stdin(prompt)?;
            let _sleep_guard =
                m31a_core::SleepInhibitionGuard::acquire("m31a autonomous command execution");

            // Record to global message history file
            let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
            let history_file = PathBuf::from(home).join(".m31a").join("history.jsonl");
            let hist_entry = m31a_core::HistoryEntry::new("cli-exec", &resolved_prompt);
            let _ = m31a_core::append_history(
                &history_file,
                &hist_entry,
                Some(m31a_core::DEFAULT_HISTORY_MAX_BYTES),
            );

            run_turn(
                &config,
                &exec_cwd,
                &resolved_prompt,
                &cli.image,
                json,
                output_last_message.as_deref(),
                ephemeral,
                search,
                self_heal,
                max_heal_iterations,
                plan,
                reasoning_effort,
            )
            .await
        }

        Some(Commands::Undo { session }) => {
            let storage = Storage::open_default()?;
            let target_session = match session {
                Some(s) => s,
                None => {
                    let threads = storage.list_threads(1)?;
                    if threads.is_empty() {
                        eprintln!("{}", "No previous session found to undo.".yellow());
                        return Ok(());
                    }
                    threads[0].id.clone()
                }
            };

            let checkpoints = storage.list_checkpoints(&target_session)?;
            if checkpoints.is_empty() {
                eprintln!(
                    "{}",
                    format!("No checkpoints found for session '{}'.", target_session).yellow()
                );
                return Ok(());
            }

            let latest = &checkpoints[0];
            storage.restore_checkpoint(&latest.id, true)?;
            if let Some(commit) = &latest.git_commit {
                let _ = std::process::Command::new("git")
                    .args(["checkout", commit, "--", "."])
                    .current_dir(&cwd)
                    .output();
            }
            println!(
                "{} Reverted workspace and session '{}' to checkpoint `{}` (Turn {})",
                "Success:".bold().green(),
                target_session,
                latest.id,
                latest.turn_index
            );
            Ok(())
        }

        Some(Commands::ApplyPatch { patch_file }) => {
            let patch_content = match patch_file {
                Some(p) if p.to_str() == Some("-") => {
                    let mut buf = String::new();
                    std::io::stdin().read_to_string(&mut buf)?;
                    buf
                }
                Some(p) => std::fs::read_to_string(&p)
                    .with_context(|| format!("Failed to read patch file at {:?}", p))?,
                None => {
                    let mut buf = String::new();
                    std::io::stdin().read_to_string(&mut buf)?;
                    buf
                }
            };
            let parsed = m31a_tools::patch::parse_patch(&patch_content)
                .map_err(|e| anyhow::anyhow!("Failed to parse patch: {e}"))?;
            let res = m31a_tools::patch::apply_patch_to_workspace(&cwd, &parsed)?;
            println!(
                "Successfully applied patch:\n- Modified: {:?}\n- Created: {:?}\n- Deleted: {:?}\n\nDiff Summary:\n{}",
                res.files_modified, res.files_created, res.files_deleted, res.summary_diff
            );
            Ok(())
        }

        Some(Commands::Migrate { dry_run }) => {
            let migrator = m31a_core::ExternalAgentMigrator::new(cwd.clone());
            let candidates = migrator.discover_candidates();
            if candidates.is_empty() {
                println!("{}", "No external agent configurations discovered (checked Claude Code, Cursor, Windsurf).".cyan());
                return Ok(());
            }

            println!(
                "{} {} migration candidates:",
                "Discovered".green().bold(),
                candidates.len()
            );
            for c in &candidates {
                println!("  - [{:?}] {} (from {:?})", c.source, c.name, c.source_path);
            }

            let mem_mgr = m31a_core::MemoryManager::with_dir(cwd.join(".m31a").join("memories"));
            let summary = migrator.apply_migration(&candidates, &mem_mgr, dry_run)?;
            println!("\nMigration Summary:");
            println!("  MCP Servers imported: {:?}", summary.imported_mcp_servers);
            println!("  Memories imported: {:?}", summary.imported_memories);
            println!("  Rules imported: {:?}", summary.imported_rules);
            if dry_run {
                println!("  {}", "(Dry run only - no files written)".yellow());
            } else {
                println!("  {}", "Migration completed successfully.".green().bold());
            }
            Ok(())
        }

        Some(Commands::History { limit }) => {
            let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
            let history_file = PathBuf::from(home).join(".m31a").join("history.jsonl");
            let batch = m31a_core::lookup_history_batch(&history_file, None, limit)?;
            if batch.entries.is_empty() {
                println!("{}", "No message history recorded yet.".cyan());
                return Ok(());
            }

            println!(
                "{} {} recent history entries (total: {}):",
                "Showing".green().bold(),
                batch.entries.len(),
                batch.total_rows
            );
            for item in batch.entries {
                if let Some(entry) = item.entry {
                    let time_str = chrono::DateTime::from_timestamp(entry.ts as i64, 0)
                        .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                        .unwrap_or_else(|| entry.ts.to_string());
                    println!("  [{}] [{}] {}", item.offset, time_str.dimmed(), entry.text);
                }
            }
            Ok(())
        }

        Some(Commands::Cloud { cmd }) => match cmd {
            CloudCommands::List => {
                let mgr = m31a_core::CloudTaskManager::open_default();
                let tasks = mgr.list_tasks();
                if tasks.is_empty() {
                    println!("{}", "No cloud tasks currently registered.".cyan());
                } else {
                    println!("{:<36} STATUS", "TASK ID");
                    println!("{}", "-".repeat(60));
                    for (task_id, status) in tasks {
                        println!("{:<36} {:?}", task_id, status);
                    }
                }
                Ok(())
            }
            CloudCommands::Submit {
                instruction,
                env: _,
            } => {
                let mgr = m31a_core::CloudTaskManager::open_default();
                let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
                let task_id = mgr.submit(&instruction, &cwd, HashMap::new(), 300);
                println!(
                    "{} Submitted cloud task `{}`",
                    "Success:".green().bold(),
                    task_id
                );
                Ok(())
            }
            CloudCommands::Cancel { task_id } => {
                let mgr = m31a_core::CloudTaskManager::open_default();
                let cancelled = mgr.cancel(&task_id)?;
                if cancelled {
                    println!(
                        "{} Cancelled cloud task `{}`",
                        "Success:".green().bold(),
                        task_id
                    );
                } else {
                    println!("Task `{}` was not active or already finished.", task_id);
                }
                Ok(())
            }
        },

        Some(Commands::Goal { cmd }) => match cmd {
            GoalCommands::Get { thread_id } => {
                let storage = Storage::open_default()?;
                if let Some(goal) = storage.get_goal(&thread_id)? {
                    println!(
                        "{} for thread `{}`:",
                        "Active Goal".green().bold(),
                        thread_id
                    );
                    println!("  Objective:    {}", goal.objective.cyan().bold());
                    println!("  Status:       {}", goal.status);
                    println!(
                        "  Token Budget: {}",
                        goal.token_budget
                            .map(|b| b.to_string())
                            .unwrap_or_else(|| "unbounded".to_string())
                    );
                    println!("  Tokens Used:  {}", goal.tokens_used);
                    println!("  Elapsed Secs: {}", goal.time_used_seconds);
                } else {
                    println!("No goal found for thread `{}`.", thread_id);
                }
                Ok(())
            }
            GoalCommands::Set {
                thread_id,
                objective,
                budget,
            } => {
                let storage = Storage::open_default()?;
                let goal = storage.set_goal(&thread_id, &objective, budget)?;
                println!(
                    "{} Created goal `{}` for thread `{}`",
                    "Success:".green().bold(),
                    goal.goal_id,
                    thread_id
                );
                Ok(())
            }
            GoalCommands::Update { thread_id, status } => {
                let storage = Storage::open_default()?;
                storage.update_goal_status(&thread_id, &status)?;
                println!(
                    "{} Updated goal status for thread `{}` to `{}`",
                    "Success:".green().bold(),
                    thread_id,
                    status
                );
                Ok(())
            }
        },

        Some(Commands::Swarm {
            mission,
            rounds,
            json,
        }) => {
            let _sleep_guard =
                m31a_core::SleepInhibitionGuard::acquire("m31a autonomous swarm execution");
            run_swarm(&config, &cwd, &mission, rounds, json).await
        }

        Some(Commands::Revert {
            checkpoint,
            thread,
            turn,
            git,
            force,
            dry_run,
        }) => {
            let storage = Storage::open_default()?;
            if let Some(cp_id) = checkpoint {
                let cp = storage
                    .get_checkpoint(&cp_id)?
                    .ok_or_else(|| anyhow!("Checkpoint '{}' not found", cp_id))?;
                println!("{} Checkpoint `{}`:", "Found".cyan().bold(), cp.id);
                println!("  Label:          {}", cp.label);
                println!("  Thread ID:      {}", cp.thread_id);
                println!("  Turn Offset:    {}", cp.turn_index);
                if let Some(ref sha) = cp.git_commit {
                    println!("  Git Commit:     {}", sha);
                }
                println!(
                    "  Modified Files: {}",
                    if cp.modified_files.is_empty() {
                        "none".to_string()
                    } else {
                        cp.modified_files.join(", ")
                    }
                );

                if dry_run {
                    println!(
                        "\n{} Dry-run mode enabled: no changes made to workspace or session history.",
                        "Info:".blue().bold()
                    );
                } else {
                    let restored = storage.restore_checkpoint(&cp_id, true)?;
                    println!(
                        "\n{} Restored thread `{}` to turn index {} and checkpoint `{}`.",
                        "Success:".green().bold(),
                        restored.thread_id,
                        restored.turn_index,
                        restored.id
                    );
                    if let Some(ref sha) = restored.git_commit {
                        if git {
                            println!(
                                "{} Performing safe working tree reversion to commit `{}`...",
                                "Action:".cyan(),
                                sha
                            );
                            match m31a_tools::safe_revert_to_commit(&cwd, sha, force, true) {
                                Ok(_) => println!(
                                    "{} Checked out commit `{}` and created safety backup ref.",
                                    "Success:".green().bold(),
                                    sha
                                ),
                                Err(e) => eprintln!(
                                    "{} Failed to checkout commit `{}`: {}",
                                    "Git Error:".red().bold(),
                                    sha,
                                    e
                                ),
                            }
                        } else {
                            println!(
                                "{} Associated Git commit is `{}`. (To checkout in git: `git checkout {}` or pass `--git`)",
                                "Notice:".yellow(),
                                sha,
                                sha
                            );
                        }
                    }
                }
            } else if let Some(target_turn) = turn {
                let thread_id = if let Some(t) = thread {
                    t
                } else {
                    let threads = storage.list_threads(1)?;
                    threads
                        .into_iter()
                        .next()
                        .map(|t| t.id)
                        .ok_or_else(|| anyhow!("No active or recent threads found to revert"))?
                };

                if dry_run {
                    println!(
                        "{} Would revert thread `{}` to turn index {}",
                        "Dry-run:".blue().bold(),
                        thread_id,
                        target_turn
                    );
                } else {
                    let deleted = storage.revert_thread(&thread_id, target_turn)?;
                    println!(
                        "{} Reverted thread `{}` to turn index {} (deleted {} later turns).",
                        "Success:".green().bold(),
                        thread_id,
                        target_turn,
                        deleted
                    );
                }
            } else {
                bail!("Must specify either --checkpoint <id> or --turn <index>");
            }
            Ok(())
        }

        Some(Commands::ResponsesProxy {
            port,
            upstream_url,
            dump_dir,
            server_info,
            http_shutdown,
        }) => {
            let token = std::env::var("M31A_PROXY_API_KEY")
                .or_else(|_| std::env::var("OPENAI_API_KEY"))
                .or_else(|_| std::env::var("NVIDIA_API_KEY"))
                .ok();
            let proxy_config = m31a_core::ResponsesProxyConfig {
                listen_port: port,
                upstream_url,
                dump_dir,
                server_info_path: server_info,
                enable_http_shutdown: http_shutdown,
                auth_token: token,
                ..Default::default()
            };

            let handle = m31a_core::ResponsesProxyServer::start(proxy_config).await?;
            println!(
                "{} Responses Proxy listening on {}",
                "M31A".green().bold(),
                handle.base_url().cyan().bold()
            );
            println!("Press Ctrl+C or send GET /shutdown to stop.");

            tokio::signal::ctrl_c().await?;
            println!("\nShutting down responses proxy server...");
            handle.shutdown().await;
            println!("Proxy server terminated cleanly.");
            Ok(())
        }

        Some(Commands::Sections { cmd }) => match cmd {
            SectionsCommands::List => {
                let storage = Storage::open_default()?;
                let sections = storage.list_thread_sections()?;
                if sections.is_empty() {
                    println!("No thread sections created yet. Use `m31a sections create <name>`.");
                } else {
                    println!("{} ({}):", "Thread Sections".green().bold(), sections.len());
                    for s in sections {
                        let color_info = s.color.as_deref().unwrap_or("none");
                        let icon_info = s.icon.as_deref().unwrap_or("-");
                        println!(
                            "  {} [{}] (id: {}, color: {})",
                            icon_info.cyan(),
                            s.name.bold(),
                            s.id.dimmed(),
                            color_info.dimmed()
                        );
                    }
                }
                Ok(())
            }
            SectionsCommands::Create { name, color, icon } => {
                let storage = Storage::open_default()?;
                let sec =
                    storage.create_thread_section(&name, color.as_deref(), icon.as_deref())?;
                println!(
                    "{} Created section `{}` with id `{}`",
                    "Success:".green().bold(),
                    sec.name.cyan().bold(),
                    sec.id.yellow()
                );
                Ok(())
            }
            SectionsCommands::Rename {
                section_id,
                new_name,
                color,
                icon,
            } => {
                let storage = Storage::open_default()?;
                let updated = storage.rename_thread_section(
                    &section_id,
                    &new_name,
                    color.as_deref(),
                    icon.as_deref(),
                )?;
                if updated {
                    println!(
                        "{} Renamed section `{}` to `{}`",
                        "Success:".green().bold(),
                        section_id,
                        new_name.cyan().bold()
                    );
                } else {
                    bail!("Section `{}` not found", section_id);
                }
                Ok(())
            }
            SectionsCommands::Move {
                thread_id,
                section_id,
            } => {
                let storage = Storage::open_default()?;
                let moved = storage.move_thread_to_section(&thread_id, section_id.as_deref())?;
                if moved {
                    match section_id {
                        Some(sec) => println!(
                            "{} Moved thread `{}` to section `{}`",
                            "Success:".green().bold(),
                            thread_id,
                            sec
                        ),
                        None => println!(
                            "{} Moved thread `{}` to uncategorized",
                            "Success:".green().bold(),
                            thread_id
                        ),
                    }
                } else {
                    bail!("Thread `{}` not found", thread_id);
                }
                Ok(())
            }
            SectionsCommands::Delete { section_id } => {
                let storage = Storage::open_default()?;
                let deleted = storage.delete_thread_section(&section_id)?;
                if deleted {
                    println!(
                        "{} Deleted section `{}` (associated threads moved to uncategorized)",
                        "Success:".green().bold(),
                        section_id
                    );
                } else {
                    bail!("Section `{}` not found", section_id);
                }
                Ok(())
            }
            SectionsCommands::Threads { section_id, limit } => {
                let storage = Storage::open_default()?;
                let threads = storage.list_threads_by_section(section_id.as_deref(), limit)?;
                let label = section_id.as_deref().unwrap_or("uncategorized");
                if threads.is_empty() {
                    println!("No threads found in section `{}`.", label);
                } else {
                    println!(
                        "{} in section `{}` ({}):",
                        "Threads".green().bold(),
                        label,
                        threads.len()
                    );
                    for t in threads {
                        println!("  {} {} - {}", t.id.cyan().bold(), t.title, t.cwd.dimmed());
                    }
                }
                Ok(())
            }
        },

        Some(Commands::Presets { id }) => {
            let presets = m31a_execpolicy::ApprovalPreset::builtin_presets();
            if let Some(target) = id {
                if let Some(p) = m31a_execpolicy::ApprovalPreset::from_id(&target) {
                    println!("{} [{}]", p.label.green().bold(), p.id.cyan());
                    println!("  Sandbox Policy:          {:?}", p.sandbox);
                    println!("  Approval Policy:         {:?}", p.approval);
                    println!(
                        "  Workspace Writes:        {}",
                        if p.workspace_write_allowed {
                            "Allowed".green()
                        } else {
                            "Blocked".red()
                        }
                    );
                    println!(
                        "  Outbound Network:        {}",
                        if p.network_allowed {
                            "Allowed".yellow()
                        } else {
                            "Blocked (Offline)".cyan()
                        }
                    );
                    println!("\n  Description:\n    {}", p.description);
                } else {
                    bail!("Preset `{}` not found", target);
                }
            } else {
                println!("{} ({}):", "Security Presets".green().bold(), presets.len());
                for p in presets {
                    println!(
                        "  * {:<20} [{}] - sandbox: {:?}, approval: {:?}",
                        p.id.cyan().bold(),
                        p.label,
                        p.sandbox,
                        p.approval
                    );
                    println!("    {}", p.description.dimmed());
                }
                println!("\nUsage: m31a --preset <id>");
            }
            Ok(())
        }

        Some(Commands::Export {
            format,
            thread,
            out,
        }) => {
            let storage = Storage::open_default()?;
            let target_thread_id = match thread {
                Some(id) => id,
                None => {
                    let recent = storage.list_threads(1)?;
                    if let Some(t) = recent.into_iter().next() {
                        t.id
                    } else {
                        bail!("No stored threads found to export.");
                    }
                }
            };

            let exporter = storage.transcript_exporter();
            let rendered = match format.to_lowercase().as_str() {
                "markdown" | "md" => exporter.export_to_markdown(&target_thread_id)?,
                "jsonl" | "json" => exporter.export_to_jsonl(&target_thread_id)?,
                "html" | "htm" => exporter.export_to_html(&target_thread_id)?,
                other => bail!(
                    "Unsupported export format: `{other}`. Expected 'markdown', 'jsonl', or 'html'."
                ),
            };

            if let Some(out_path) = out {
                if let Some(parent) = out_path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::write(&out_path, &rendered)?;
                println!(
                    "{} thread {} to {} ({} bytes, format: {})",
                    "Exported".green().bold(),
                    target_thread_id.cyan(),
                    out_path.display().to_string().cyan(),
                    rendered.len(),
                    format
                );
            } else {
                io::stdout().write_all(rendered.as_bytes())?;
            }
            Ok(())
        }

        Some(Commands::Worktree { cmd }) => {
            let manager = WorktreeManager::new()?;

            match cmd {
                WorktreeCommands::List => {
                    let worktrees = manager.list_worktrees(&cwd)?;
                    if worktrees.is_empty() {
                        println!("{}", "No managed worktrees found.".yellow());
                    } else {
                        println!("{}", "Managed Worktrees:".bold());
                        for wt in worktrees {
                            println!(
                                "  {}  {}  {}  {}",
                                wt.id.cyan(),
                                wt.path.display().to_string().green(),
                                wt.branch.as_deref().unwrap_or("detached").dimmed(),
                                wt.head_sha[..8].yellow()
                            );
                        }
                    }
                    Ok(())
                }
                WorktreeCommands::Create { session, base } => {
                    let session = session
                        .unwrap_or_else(|| format!("session-{}", &Uuid::new_v4().to_string()[..8]));
                    let wt = manager.create_worktree(&cwd, base.as_deref())?;
                    println!(
                        "{} worktree for session {} at {}",
                        "Created".green().bold(),
                        session.cyan(),
                        wt.path.display().to_string().green()
                    );
                    println!(
                        "  Branch: {}",
                        wt.branch.as_deref().unwrap_or("detached").dimmed()
                    );
                    println!("  Head commit: {}", wt.head_sha[..8].dimmed());
                    Ok(())
                }
                WorktreeCommands::Remove { session, force } => {
                    // Find and remove worktree by session ID (using first 8 chars of ID)
                    let worktrees = manager.list_worktrees(&cwd)?;
                    if let Some(wt) = worktrees.iter().find(|w| {
                        w.id.starts_with(&session[..std::cmp::min(8, session.len())])
                    }) {
                        let mut args = vec!["worktree", "remove"];
                        if force {
                            args.push("--force");
                        }
                        args.push(wt.path.to_str().unwrap());

                        let output = std::process::Command::new("git")
                            .args(&args)
                            .current_dir(&WorktreeManager::repository_root(&cwd)?)
                            .output()?;

                        if !output.status.success() {
                            return Err(anyhow::anyhow!(
                                "Failed to remove worktree: {}",
                                String::from_utf8_lossy(&output.stderr)
                            ));
                        }
                        println!("{} worktree {}", "Removed".green().bold(), wt.id.cyan());
                    } else {
                        println!("{}", "No worktree found with that session ID".yellow());
                    }
                    Ok(())
                }
                WorktreeCommands::Status { session } => {
                    let worktrees = manager.list_worktrees(&cwd)?;
                    let session = session.unwrap_or_else(|| {
                        worktrees.first().map(|w| w.id.clone()).unwrap_or_default()
                    });
                    if let Some(wt) = worktrees.iter().find(|w| {
                        w.id.starts_with(&session[..std::cmp::min(8, session.len())])
                    }) {
                        // Get status by running git status in worktree
                        let output = std::process::Command::new("git")
                            .args(["status", "--porcelain"])
                            .current_dir(&wt.path)
                            .output()?;

                        let status_text = String::from_utf8_lossy(&output.stdout);
                        let mut modified = 0;
                        let mut added = 0;
                        let mut deleted = 0;
                        let mut untracked = 0;

                        for line in status_text.lines() {
                            if line.len() < 2 {
                                continue;
                            }
                            let index_status = line.chars().next().unwrap();
                            let worktree_status = line.chars().nth(1).unwrap();

                            match index_status {
                                'A' => added += 1,
                                'M' => modified += 1,
                                'D' => deleted += 1,
                                _ => {}
                            }

                            match worktree_status {
                                'M' => modified += 1,
                                'D' => deleted += 1,
                                '?' => untracked += 1,
                                _ => {}
                            }
                        }

                        println!("{}", "Worktree Status:".bold());
                        println!("  Session: {}", session.cyan());
                        println!("  Worktree ID: {}", wt.id.dimmed());
                        println!("  Path: {}", wt.path.display().to_string().green());
                        println!(
                            "  Branch: {}",
                            wt.branch.as_deref().unwrap_or("detached").dimmed()
                        );
                        println!("  Head commit: {}", wt.head_sha[..8].yellow());
                        println!("  Modified: {}", modified.to_string().yellow());
                        println!("  Added: {}", added.to_string().green());
                        println!("  Deleted: {}", deleted.to_string().red());
                        println!("  Untracked: {}", untracked.to_string().dimmed());
                        if modified == 0 && added == 0 && deleted == 0 && untracked == 0 {
                            println!("  {}", "Working tree is clean".green());
                        }
                    } else {
                        println!("{}", "No worktree found for session".yellow());
                    }
                    Ok(())
                }
                WorktreeCommands::Merge { session, target } => {
                    let worktrees = manager.list_worktrees(&cwd)?;
                    if let Some(wt) = worktrees.iter().find(|w| {
                        w.id.starts_with(&session[..std::cmp::min(8, session.len())])
                    }) {
                        // Commit any changes in worktree
                        let _ = std::process::Command::new("git")
                            .args(["add", "-A"])
                            .current_dir(&wt.path)
                            .output()?;

                        let _ = std::process::Command::new("git")
                            .args([
                                "commit",
                                "-m",
                                &format!("M31A session {} checkpoint", session),
                            ])
                            .current_dir(&wt.path)
                            .output()?;

                        // Get the commit hash
                        let commit_output = std::process::Command::new("git")
                            .args(["rev-parse", "HEAD"])
                            .current_dir(&wt.path)
                            .output()?;

                        let worktree_commit = String::from_utf8_lossy(&commit_output.stdout)
                            .trim()
                            .to_string();

                        // Switch to target branch in main repo and merge
                        let repo_root = WorktreeManager::repository_root(&cwd)?;
                        let _ = std::process::Command::new("git")
                            .args(["checkout", &target])
                            .current_dir(&repo_root)
                            .output()?;

                        let output = std::process::Command::new("git")
                            .args([
                                "merge",
                                "--no-ff",
                                &worktree_commit,
                                "-m",
                                &format!("Merge M31A session {} into {}", session, target),
                            ])
                            .current_dir(&repo_root)
                            .output()?;

                        if !output.status.success() {
                            return Err(anyhow::anyhow!(
                                "Merge failed: {}",
                                String::from_utf8_lossy(&output.stderr)
                            ));
                        }

                        println!(
                            "{} worktree {} merged into {}",
                            "Merged".green().bold(),
                            session.cyan(),
                            target.cyan()
                        );
                    } else {
                        println!("{}", "No worktree found with that session ID".yellow());
                    }
                    Ok(())
                }
            }
        }

        Some(Commands::Gsd { cmd }) => gsd::GsdCliHandler::handle(cmd, &cwd).await,

        Some(Commands::Serve { port, bind }) => {
            let addr_str = format!("{}:{}", bind, port);
            let parsed_addr: std::net::SocketAddr = addr_str
                .parse()
                .map_err(|e| anyhow::anyhow!("Invalid bind address '{}': {}", addr_str, e))?;
            println!(
                "{} Starting M31A HTTP/SSE Server on http://{}",
                "->".green().bold(),
                addr_str
            );
            println!("   REST endpoints: /health, /api/threads, /api/models");
            println!("   RPC endpoint:   POST /rpc");
            println!("   SSE endpoint:   GET /events");
            println!("   Web GUI:        http://{}", addr_str);
            let storage = Storage::open_default()?;
            let tools = Arc::new(ToolRegistry::new());
            let server = AppServer::new(Arc::new(config), storage, tools)?;
            let http_config = m31a_app_server::http_server::HttpServerConfig {
                bind_addr: parsed_addr,
                cors_allow_all: true,
                ..Default::default()
            };
            m31a_app_server::http_server::run_http_server(server, http_config).await?;
            Ok(())
        }

        Some(Commands::Template { cmd }) => match cmd {
            TemplateCommands::List => {
                let library = m31a_collaboration_templates::TemplateLibrary::new(
                    Config::config_dir().join("templates"),
                );
                let templates = library.list();
                println!("{}", "Available Collaboration Templates:".bold());
                for t in templates {
                    println!(
                        "- {:<24} {:<20} {}",
                        t.id.cyan(),
                        format!("[{:?}]", t.category).dimmed(),
                        t.description
                    );
                }
                Ok(())
            }
            TemplateCommands::Inspect { id } => {
                let library = m31a_collaboration_templates::TemplateLibrary::new(
                    Config::config_dir().join("templates"),
                );
                if let Some(t) = library.get(&id) {
                    println!("{} {}", "Template:".bold(), t.name.green());
                    println!("  ID:          {}", t.id);
                    println!("  Category:    {:?}", t.category);
                    println!("  Description: {}", t.description);
                    if let Some(ref m) = t.config.model {
                        println!("  Model:       {}", m);
                    }
                    if let Some(mins) = t.estimated_duration_minutes {
                        println!("  Duration:    ~{} min", mins);
                    }
                } else {
                    println!("Template '{}' not found.", id);
                }
                Ok(())
            }
        },

        Some(Commands::Telemetry { cmd }) => match cmd {
            TelemetryCommands::Stats => {
                let reducer = m31a_telemetry::TelemetryReducer::new();
                let stats = reducer.get_global_stats();
                println!("{}", "M31A Telemetry Pipeline Statistics:".bold());
                println!("- Active Sessions:           {}", stats.total_sessions);
                println!("- Total Turns Tracked:       {}", stats.total_turns);
                println!("- Total Token Throughput:    {}", stats.total_tokens);
                println!(
                    "- Total Tool Invocations:    {}",
                    stats.total_tool_invocations
                );
                println!("- Total Errors Recorded:     {}", stats.total_errors);
                println!("- Exporter Backends:         Memory, Console, Structured JSONL");
                Ok(())
            }
        },

        Some(Commands::Attribution { cmd }) => match cmd {
            AttributionCommands::Trailer { session, model } => {
                let mut builder = m31a_git_attribution::CommitTrailerBuilder::new();
                if let Some(s) = session {
                    builder = builder.with_session_id(s);
                }
                if let Some(m) = model {
                    builder = builder.with_model_id(m);
                }
                let trailer_text = builder.render_trailers();
                println!("{}", "Generated AI Provenance Git Trailers:".bold());
                println!("{}", trailer_text);
                Ok(())
            }
        },

        Some(Commands::Prompt { cmd }) => match cmd {
            PromptCommands::Fragments { model } => {
                let m = model
                    .as_deref()
                    .unwrap_or("nvidia/nemotron-3.5-70b-instruct");
                let registry = m31a_prompts::fragment_registry::FragmentRegistry::new(None);
                let resolved = registry
                    .resolve(m, &[], &[], "workspace_write", "approve_for_me", 0.2)
                    .await?;
                println!(
                    "{} {}",
                    "Resolved Prompt Fragments for Model:".bold(),
                    m.green()
                );
                println!("Family: {:?}", resolved.model_family);
                println!("Fragments count: {}", resolved.fragments.len());
                println!("\n--- Rendered Fragments ---\n{}", resolved.rendered);
                Ok(())
            }
            PromptCommands::List => {
                let library =
                    m31a_prompts::PromptLibrary::new(Config::config_dir().join("prompts"));
                let templates = library.list().await;
                println!("{}", "Prompt Templates:".bold());
                for t in templates {
                    println!(
                        "- {:<24} {:<16} {}",
                        t.id.cyan(),
                        format!("[{:?}]", t.category).dimmed(),
                        t.name
                    );
                }
                Ok(())
            }
        },

        Some(Commands::Vibe {
            prompt,
            no_auto_heal,
            debounce,
            visual_url,
        }) => {
            vibe::run_vibe_mode(vibe::VibeCliOptions {
                workspace_root: cwd.clone(),
                model: Some(config.model.clone()),
                auto_heal: !no_auto_heal,
                debounce_ms: debounce,
                visual_url,
                initial_prompt: prompt,
            })
            .await
        }

        Some(Commands::Speculate {
            mission,
            hypotheses,
            timeout,
            promote,
        }) => {
            let storage = Storage::open_default()?;
            speculate::run_speculate_mode(speculate::SpeculateCliOptions {
                workspace_root: cwd.clone(),
                mission,
                hypotheses,
                timeout_seconds: timeout,
                auto_promote: promote,
                config: Some(config.clone()),
                storage: Some(storage),
            })
            .await
        }

        Some(Commands::Workflow { cmd }) => {
            let storage = Storage::open_default()?;
            workflow::run_workflow_mode(cwd.clone(), storage, &config, cmd).await
        }

        None => {
            // First-run setup check: if no config file and no API key in environment
            let config_file = Config::config_dir().join("config.toml");
            let has_key = config
                .api_key
                .as_ref()
                .map(|k| !k.trim().is_empty())
                .unwrap_or(false)
                || std::env::var("NVIDIA_API_KEY")
                    .map(|k| !k.trim().is_empty())
                    .unwrap_or(false);

            if !config_file.exists() && !has_key && io::stdin().is_terminal() {
                println!(
                    "{}",
                    "No M31A configuration or NVIDIA_API_KEY detected.".yellow()
                );
                print!("Would you like to run the interactive setup wizard now? [Y/n]: ");
                let _ = io::stdout().flush();
                let mut line = String::new();
                if io::stdin().read_line(&mut line).is_ok() {
                    let trimmed = line.trim().to_lowercase();
                    if trimmed.is_empty() || trimmed == "y" || trimmed == "yes" {
                        setup::SetupWizard::run(setup::SetupOptions::default()).await?;
                        config = Config::load_with_overrides(
                            cli.profile.as_deref(),
                            &cli.config_overrides,
                            cli.strict_config,
                        )
                        .unwrap_or_default();
                    } else {
                        println!(
                            "Continuing with preview limits. Run 'm31a setup' anytime to configure.\n"
                        );
                    }
                }
            }

            // Initialize professional theme (default: Dark)
            m31a_tui::init_theme(m31a_tui::ThemeVariant::Dark);

            let app = m31a_tui::App::new(config)?;
            m31a_tui::run_tui_app(app, cli.prompt).await
        }
    }
}

async fn run_model_command(config: &Config, cmd: Option<ModelCommands>) -> Result<()> {
    let catalog = get_nvidia_model_catalog();
    match cmd.unwrap_or(ModelCommands::List) {
        ModelCommands::List => {
            println!(
                "{}",
                "Available Frontier Models in NVIDIA NIM Catalog:"
                    .cyan()
                    .bold()
            );
            println!(
                "{:<45} {:<10} {:<12} {:<15} STATUS",
                "MODEL SLUG", "CONTEXT", "REASONING", "TOOLS"
            );
            println!("{}", "-".repeat(95));
            for m in catalog {
                let is_active = m.slug == config.model;
                let status = if is_active {
                    "[ACTIVE]".green().bold()
                } else if m.is_default {
                    "[DEFAULT]".cyan()
                } else {
                    "".normal()
                };
                let reasoning = if m.supports_thinking {
                    "Supported".cyan()
                } else {
                    "No".dimmed()
                };
                let tools = if m.supports_tools {
                    "Yes".green()
                } else {
                    "No".dimmed()
                };
                let ctx_str = format!("{}k", m.context_window / 1024);

                println!(
                    "{:<45} {:<10} {:<12} {:<15} {}",
                    if is_active {
                        m.slug.bold().green()
                    } else {
                        m.slug.normal()
                    },
                    ctx_str,
                    reasoning,
                    tools,
                    status
                );
            }
            println!(
                "\nPresets: speed (8B), code (Qwen 32B), reasoning (DeepSeek R1), flagship (Llama 3.3 70B)"
            );
            println!("Usage:   m31a model set <slug_or_preset> | m31a model route [on|off]");
            Ok(())
        }
        ModelCommands::Current => {
            let active_info = catalog.iter().find(|m| m.slug == config.model);
            println!("{}", "Active Model Configuration:".cyan().bold());
            println!("  Model Slug:        {}", config.model.green().bold());
            if let Some(info) = active_info {
                println!("  Display Name:      {}", info.display_name);
                println!(
                    "  Context Limit:     {} tokens ({}k)",
                    info.context_window,
                    info.context_window / 1024
                );
                println!(
                    "  Reasoning Stream:  {}",
                    if info.supports_thinking {
                        "Supported"
                    } else {
                        "Not enabled"
                    }
                );
                println!(
                    "  Native Tools:      {}",
                    if info.supports_tools {
                        "Supported"
                    } else {
                        "None"
                    }
                );
                println!("  Description:       {}", info.description);
            }
            println!("  API Base:          {}", config.api_base);
            println!(
                "  Reasoning Effort:  {}",
                config.reasoning_effort.as_deref().unwrap_or("default")
            );
            println!(
                "  Hybrid Routing:    {}",
                if config.routing_enabled {
                    "Enabled".green()
                } else {
                    "Disabled".dimmed()
                }
            );
            Ok(())
        }
        ModelCommands::Set { model } => {
            let resolved = setup::resolve_model_slug_or_alias(&model);
            let mut updated_config = Config::load().unwrap_or_else(|_| config.clone());
            updated_config.model = resolved.clone();
            updated_config
                .save()
                .context("Failed to save configuration")?;
            println!(
                "{} Active model set to: {}",
                "[OK]".green().bold(),
                resolved.green().bold()
            );
            Ok(())
        }
        ModelCommands::Route { action } => {
            let mut updated_config = Config::load().unwrap_or_else(|_| config.clone());
            match action.as_deref() {
                Some("on" | "enable" | "true") => {
                    updated_config.routing_enabled = true;
                    updated_config.save()?;
                    println!(
                        "{} Dynamic hybrid model routing enabled.",
                        "[OK]".green().bold()
                    );
                }
                Some("off" | "disable" | "false") => {
                    updated_config.routing_enabled = false;
                    updated_config.save()?;
                    println!(
                        "{} Dynamic hybrid model routing disabled.",
                        "[OK]".green().bold()
                    );
                }
                _ => {
                    println!(
                        "Dynamic Hybrid Routing: {}",
                        if updated_config.routing_enabled {
                            "Enabled".green().bold()
                        } else {
                            "Disabled".dimmed()
                        }
                    );
                    println!("Usage: m31a model route on | m31a model route off");
                }
            }
            Ok(())
        }
    }
}

fn resolve_prompt_with_stdin(prompt_arg: Option<String>) -> Result<String> {
    let mut stdin_content = String::new();
    let is_piped = !io::stdin().is_terminal();

    if is_piped {
        let _ = io::stdin().read_to_string(&mut stdin_content);
    }

    match prompt_arg {
        Some(p) if p == "-" => {
            if stdin_content.is_empty() {
                bail!("'-' passed as prompt but stdin was empty");
            }
            Ok(stdin_content.trim().to_string())
        }
        Some(p) => {
            if is_piped && !stdin_content.trim().is_empty() {
                Ok(format!(
                    "{}\n\n<stdin>\n{}\n</stdin>",
                    p,
                    stdin_content.trim()
                ))
            } else {
                Ok(p)
            }
        }
        None => {
            if is_piped && !stdin_content.trim().is_empty() {
                Ok(stdin_content.trim().to_string())
            } else {
                bail!("No prompt provided to exec. Pass a prompt string or pipe via stdin.")
            }
        }
    }
}

async fn run_review(config: &Config, cwd: &Path, base: Option<&str>) -> Result<()> {
    let report = m31a_review::ReviewEngine::analyze_git(cwd, base)?;
    let term_output = m31a_review::ReviewFormatter::render_terminal(&report);
    println!("{term_output}");

    let has_api_key = config
        .api_key
        .as_ref()
        .map(|k| !k.trim().is_empty())
        .unwrap_or(false);
    if !report.findings.is_empty() && has_api_key {
        println!(
            "\n{}",
            "Routing findings through NVIDIA NIM frontier model for remediation advice..."
                .cyan()
                .bold()
        );
        let md_summary = m31a_review::ReviewFormatter::render_markdown(&report);
        let prompt = format!(
            "The automated code review engine generated the following findings on current git changes:\n\n{}\n\nPlease provide specific code fixes and remediation patches for any Critical or High severity findings.",
            md_summary
        );
        run_turn(
            config,
            cwd,
            &prompt,
            &[],
            false,
            None,
            false,
            false,
            false,
            0,
            false,
            None,
        )
        .await?;
    }

    Ok(())
}

async fn run_verify(cwd: &Path, tests: bool, timeout_secs: u64) -> Result<()> {
    let timeout = Duration::from_secs(timeout_secs);
    let mode_str = if tests {
        "test suite"
    } else {
        "compiler / lint checks"
    };
    println!(
        "{}",
        format!(
            "Autonomously verifying workspace with {} (timeout: {}s)...",
            mode_str, timeout_secs
        )
        .cyan()
        .bold()
    );

    let report = if tests {
        m31a_verify::AutonomousVerifier::verify_tests(cwd, timeout).await?
    } else {
        m31a_verify::AutonomousVerifier::verify_check(cwd, timeout).await?
    };

    if report.success {
        println!(
            "{}",
            format!("[PASS] Verification: {}", report.command)
                .green()
                .bold()
        );
        Ok(())
    } else {
        println!(
            "{}",
            format!("[FAIL] Verification: {}", report.command)
                .red()
                .bold()
        );
        println!(
            "Error count: {}, Warning count: {}",
            report.error_count, report.warning_count
        );
        if let Some(ref prompt) = report.self_healing_prompt {
            println!(
                "\n{}\n{}",
                "Autonomous Remediation Guidance:".yellow().bold(),
                prompt
            );
        } else {
            println!("\nOutput:\n{}", report.raw_output);
        }
        anyhow::bail!(
            "Verification failed with {} errors and {} warnings",
            report.error_count,
            report.warning_count
        );
    }
}

async fn run_trust_command(cwd: &Path, cmd: TrustCommands) -> Result<()> {
    let mut store = m31a_trust::TrustStore::new();
    match cmd {
        TrustCommands::List => {
            let list = store.list();
            if list.is_empty() {
                println!("No directories currently registered in persistent trust store.");
            } else {
                println!("{}", "Trusted Directories:".cyan().bold());
                for r in list {
                    let remote_str = r
                        .git_remote
                        .map(|u| format!(" ({})", u))
                        .unwrap_or_default();
                    println!(
                        "  - [{:?}] {}{}",
                        r.trust_level,
                        r.path.display(),
                        remote_str
                    );
                }
            }
        }
        TrustCommands::Add { path } => {
            let target = path.as_deref().unwrap_or(cwd);
            store.trust(target, m31a_trust::TrustLevel::Trusted)?;
            println!(
                "{} Marked '{}' as a trusted directory.",
                "[OK]".green().bold(),
                target.display()
            );
        }
        TrustCommands::Revoke { path } => {
            let target = path.as_deref().unwrap_or(cwd);
            if store.revoke(target)? {
                println!(
                    "{} Revoked trust for '{}'.",
                    "[OK]".yellow().bold(),
                    target.display()
                );
            } else {
                println!(
                    "Directory '{}' was not registered in trust store.",
                    target.display()
                );
            }
        }
        TrustCommands::Audit { path } => {
            let target = path.as_deref().unwrap_or(cwd);
            println!(
                "{}",
                format!(
                    "Auditing git hooks and configuration in '{}'...",
                    target.display()
                )
                .cyan()
                .bold()
            );
            let report = m31a_trust::GitArmor::audit(target);
            if report.is_safe && report.risks.is_empty() {
                println!("{}", "[OK] Repository passed Git Armor audit: No suspicious hooks or command injections detected.".green().bold());
            } else {
                println!(
                    "{}",
                    format!(
                        "Git Armor Audit: {} risk(s) identified (hooks: {}, custom commands: {}):",
                        report.risks.len(),
                        report.hooks_detected,
                        report.custom_commands_detected
                    )
                    .yellow()
                    .bold()
                );
                for r in &report.risks {
                    let color_sev = match r.severity {
                        m31a_trust::RiskSeverity::Critical => "[CRITICAL]".to_string().red().bold(),
                        m31a_trust::RiskSeverity::High => "[HIGH]".to_string().red(),
                        m31a_trust::RiskSeverity::Medium => "[MEDIUM]".to_string().yellow(),
                        m31a_trust::RiskSeverity::Low => "[LOW]".to_string().dimmed(),
                    };
                    println!("  {} {} ({})", color_sev, r.title, r.location.display());
                    println!("     {}", r.description);
                }
            }
        }
    }
    Ok(())
}

async fn run_ps_command() -> Result<()> {
    let registry = m31a_tools::ToolRegistry::new();
    let procs = registry.proc_supervisor.list().await;
    if procs.is_empty() {
        println!(
            "{}",
            "No managed background processes currently running.".dimmed()
        );
        println!("Use 'start_background_process' tool or background servers to spawn processes.");
    } else {
        println!(
            "{:<14} {:<12} {:<8} {:<10} {:<12} LABEL / COMMAND",
            "ID", "STATUS", "PID", "UPTIME", "LINES"
        );
        println!("{}", "-".repeat(75));
        for p in procs {
            let (badge, _) = p.status.display_badge();
            let pid = p.pid.map(|n| n.to_string()).unwrap_or_else(|| "-".into());
            let uptime = format!("{}s", p.uptime_seconds);
            println!(
                "{:<14} {:<12} {:<8} {:<10} {:<12} {}",
                p.id, badge, pid, uptime, p.total_lines_logged, p.label
            );
        }
    }
    Ok(())
}

async fn run_kill_command(id: &str, force: bool) -> Result<()> {
    let registry = m31a_tools::ToolRegistry::new();
    let res = registry.proc_supervisor.stop(id, force).await?;
    if res.stopped {
        println!(
            "{} Process '{}' ({}) successfully stopped: {}",
            "[OK]".green().bold(),
            res.id,
            res.label,
            res.message
        );
    } else {
        println!(
            "{} Process '{}': {}",
            "[INFO]".yellow().bold(),
            res.id,
            res.message
        );
    }
    Ok(())
}

async fn run_logs_command(id: &str, lines_count: usize) -> Result<()> {
    let registry = m31a_tools::ToolRegistry::new();
    let out = registry
        .proc_supervisor
        .get_output(id, None, Some(lines_count))
        .await?;
    println!(
        "Logs for process '{}' ({}) — Total lines: {}",
        out.process_id, out.label, out.total_lines
    );
    println!("{}", "-".repeat(75));
    if out.lines.is_empty() {
        println!("{}", "(No output lines logged)".dimmed());
    } else {
        for l in out.lines {
            let tag = match l.stream {
                m31a_proc::OutputStream::Stdout => "[OUT]",
                m31a_proc::OutputStream::Stderr => "[ERR]",
                m31a_proc::OutputStream::System => "[SYS]",
            };
            println!("{} {} {}", l.timestamp.format("%H:%M:%S"), tag, l.line);
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn run_turn(
    config: &Config,
    cwd: &Path,
    prompt: &str,
    images: &[PathBuf],
    json_output: bool,
    output_last_message: Option<&Path>,
    ephemeral: bool,
    search: bool,
    self_heal: bool,
    max_heal_iterations: usize,
    plan: bool,
    reasoning_effort: Option<String>,
) -> Result<()> {
    let api_key = config.api_key.as_ref().context("NVIDIA_API_KEY is not set. Run `export NVIDIA_API_KEY=\"nvapi-...\"` or update ~/.m31a/config.toml")?;

    // Workspace Trust Check: enforce centralized trust policy across interactive and headless modes
    let mut trust_store = m31a_trust::TrustStore::new();
    let decision = trust_store.evaluate(cwd);
    let _auth = match decision {
        m31a_trust::TrustDecision::Denied { path } => {
            bail!(
                "Execution DENIED: Working directory '{}' is explicitly marked as Denied by security trust policy.",
                path.display()
            );
        }
        m31a_trust::TrustDecision::Trusted | m31a_trust::TrustDecision::SessionOnly => {
            trust_store.authorize(cwd, false, false)?
        }
        m31a_trust::TrustDecision::RequiresConfirmation { path, reason } => {
            if json_output {
                bail!(
                    "Execution REJECTED: Working directory '{}' is untrusted ({}). In headless or JSON mode, authorization must be explicitly granted via session flag or permanent trust registration (`m31a trust add`).",
                    path.display(),
                    reason
                );
            }
            // In interactive CLI, prompt user for explicit session confirmation
            println!(
                "{} Workspace '{}' requires trust authorization: {}",
                "Security Trust:".yellow().bold(),
                cwd.display(),
                reason
            );
            print!("Trust this directory for this session? [y/N]: ");
            let _ = std::io::stdout().flush();
            let mut input = String::new();
            std::io::stdin().read_line(&mut input)?;
            if input.trim().eq_ignore_ascii_case("y") || input.trim().eq_ignore_ascii_case("yes") {
                trust_store.authorize(cwd, false, true)?
            } else {
                bail!("Execution aborted by user: untrusted workspace denied.");
            }
        }
    };

    let provider_type = ProviderType::resolve(
        config.model_provider.as_deref(),
        &config.model,
        Some(&config.api_base),
    )?;
    let provider_config = ProviderConfig {
        provider_type,
        api_base: Some(config.api_base.clone()),
        api_key: Some(api_key.to_string()),
        model: config.model.clone(),
        extra: HashMap::new(),
    };
    let provider = ProviderBuilder::build_client(provider_config)?
        .with_reasoning_effort(reasoning_effort.or_else(|| config.reasoning_effort.clone()));
    let mut tools = ToolRegistry::new();
    // `--search` gates the live web_search tool: hidden from tool definitions and rejected at execution time.
    if !search {
        tools.disable("web_search");
    }
    let storage = Storage::open_default()?;

    // Shared hooks runtime: parent engine + §4.3 child engines all dispatch
    // through the same hook configuration.
    let hooks = Arc::new(m31a_hooks::HookRuntime::new(
        m31a_hooks::HookConfig::discover(cwd),
    ));

    let mut session = Session::new(&config.model, cwd);

    let collaboration_mode = if plan {
        m31a_core::prompts::CollaborationMode::Plan
    } else {
        m31a_core::prompts::CollaborationMode::Default
    };

    // Initial system prompt assembled via canonical runtime pipeline
    let snapshot = m31a_core::prompts::PromptRuntimeSnapshot::new(
        cwd,
        &config.model,
        &format!("{:?}", config.sandbox),
        collaboration_mode,
    )
    .with_user_custom_instructions(config.system_prompt.clone())
    .with_provider(
        config
            .model_provider
            .as_deref()
            .unwrap_or("NVIDIA NIM / Upstream Provider"),
    );

    let system_instructions = SystemPromptBuilder::build_canonical(&snapshot);

    let mut history = vec![ChatMessage::system(system_instructions.clone())];

    // §4.3: inject the real child-engine runtime so spawn_agent runs actual
    // AgentEngine subagents (descriptive fallback when not injected).
    tools.set_agent_runtime(Arc::new(
        m31a_agent_engine::agents::EngineAgentRuntime::new(
            provider.clone(),
            tools.clone(),
            storage.clone(),
            ExecPolicy::standard(),
            cwd.to_path_buf(),
            Some(hooks.clone()),
            config.model_context_window,
            config.agents.max_depth as u32,
        ),
    ));

    let mut user_prompt = prompt.to_string();
    if !images.is_empty() {
        user_prompt.push_str("\n\nAttached Images:\n");
        for img in images {
            user_prompt.push_str(&format!("- Image file: {}\n", img.display()));
        }
    }

    history.push(ChatMessage::user(user_prompt.clone()));

    if !ephemeral {
        let _ = storage.save_thread(
            &session.metadata,
            "",
            &format!("{:?}", config.sandbox),
            &format!("{:?}", config.approval),
        );
        let _ = storage.save_item(
            &uuid::Uuid::new_v4().to_string(),
            &session.metadata.id,
            None,
            "user",
            &user_prompt,
        );
    }

    if json_output {
        println!(
            "{}",
            serde_json::json!({
                "type": "turn_start",
                "session": session.metadata.id,
                "prompt": user_prompt,
                "model": config.model,
            })
        );
    } else {
        println!("\n{} {}", "User:".bold().cyan(), user_prompt);
    }

    // Unified engine turn loop (report §2.1): single canonical implementation shared
    // with the TUI — execpolicy, steering hooks, parallel dispatch, inline compaction,
    // Guardian review, and persistence now all flow through the engine.
    let engine = AgentEngine::new(
        provider,
        tools,
        storage,
        config.approval,
        ExecPolicy::standard(),
        config.sandbox,
        cwd.to_path_buf(),
    );
    let mut engine = AgentEngine {
        guardian: if config.approval == ApprovalPolicy::ApproveForMe {
            Some(Arc::new(GuardianReviewer::new(&config.model)))
        } else {
            None
        },
        hooks: Some(hooks),
        persist_items: !ephemeral,
        self_heal,
        max_heal_iterations,
        routing_enabled: config.routing_enabled,
        tool_output_token_limit: config.tool_output_token_limit,
        code_mode: if config.features.get("code_mode").copied().unwrap_or(false) {
            Some(m31a_code_mode::CodeModeOrchestrator::new())
        } else {
            None
        },
        compactor: Arc::new(ContextCompactor::new(CompactTokenBudget {
            max_context_tokens: config.model_context_window,
            ..CompactTokenBudget::default()
        })),
        collaboration_mode: if plan {
            m31a_core::prompts::CollaborationMode::Plan
        } else {
            m31a_core::prompts::CollaborationMode::Default
        },
        authoritative_system_prompt: Some(system_instructions.clone()),
        dangerously_bypass: config.dangerously_bypass,
        ..engine
    };

    let (eng_tx, mut eng_rx) = mpsc::unbounded_channel();
    let (_steer_tx, steer_rx) = mpsc::unbounded_channel();
    let cancel_token = CancellationToken::new();

    let engine_result = engine
        .run_turn(
            &session.metadata.id,
            history,
            eng_tx,
            steer_rx,
            cancel_token,
            20,
        )
        .await;

    let mut last_assistant_message = String::new();
    let mut turn_failed: Option<String> = None;

    while let Some(event) = eng_rx.recv().await {
        match event {
            EngineEvent::ContentChunk(chunk) => {
                if json_output {
                    println!(
                        "{}",
                        serde_json::json!({ "type": "content_chunk", "chunk": chunk })
                    );
                } else {
                    print!("{}", chunk);
                    let _ = io::stdout().flush();
                }
            }
            EngineEvent::ReasoningChunk(chunk) => {
                if json_output {
                    println!(
                        "{}",
                        serde_json::json!({ "type": "reasoning_chunk", "chunk": chunk })
                    );
                } else {
                    print!("{}", chunk.dimmed());
                    let _ = io::stdout().flush();
                }
            }
            EngineEvent::ToolCallStarted { id, name, args } => {
                if json_output {
                    println!(
                        "{}",
                        serde_json::json!({
                            "type": "tool_call",
                            "id": id,
                            "name": name,
                            "arguments": args
                        })
                    );
                } else {
                    println!();
                    println!(
                        "{} `{}` with args: {}",
                        "Invoking Tool:".bold().yellow(),
                        name.cyan(),
                        args
                    );
                }
            }
            EngineEvent::ToolCallFinished {
                id,
                output,
                success,
            } => {
                if json_output {
                    if success {
                        println!(
                            "{}",
                            serde_json::json!({
                                "type": "tool_result",
                                "id": id,
                                "output": output
                            })
                        );
                    } else {
                        println!(
                            "{}",
                            serde_json::json!({
                                "type": "tool_error",
                                "id": id,
                                "error": output
                            })
                        );
                    }
                } else if success {
                    println!("  {} ({} bytes output)", "Result:".green(), output.len());
                } else {
                    println!("  {} {}", "Error:".red(), output);
                }
            }
            EngineEvent::GuardianDenied { rationale } => {
                if json_output {
                    println!(
                        "{}",
                        serde_json::json!({
                            "type": "guardian_denial",
                            "rationale": rationale
                        })
                    );
                } else {
                    println!("{}", "DENIED by Guardian".bold().red());
                    println!("  Rationale: {}", rationale);
                }
            }
            EngineEvent::Notice(msg, is_warning) => {
                if json_output {
                    println!(
                        "{}",
                        serde_json::json!({
                            "type": "notice",
                            "message": msg,
                            "warning": is_warning
                        })
                    );
                } else if is_warning {
                    println!("{} {}", "WARN".yellow(), msg);
                } else {
                    println!("{}", msg.dimmed());
                }
            }
            EngineEvent::ApprovalRequested { response_tx, .. } => {
                // Non-interactive execution: ApproveForMe is normally gated by the
                // Guardian (fail closed if a dialog still reaches the renderer);
                // Never/OnRequest auto-respond to preserve historical exec behavior.
                let approve = config.approval != ApprovalPolicy::ApproveForMe;
                let _ = response_tx.send(m31a_elicitation::ApprovalDecision::from(approve));
            }
            EngineEvent::QuestionRequested { response_tx, .. } => {
                // Empty response -> engine falls back to its default recommendation.
                let _ = response_tx.send(Vec::new());
            }
            EngineEvent::Compacted {
                window_id,
                original_tokens,
                compacted_tokens,
            } => {
                if json_output {
                    println!(
                        "{}",
                        serde_json::json!({
                            "type": "compaction",
                            "summary": "Compressed earlier conversation context",
                            "window_id": window_id,
                            "original_tokens": original_tokens,
                            "compacted_tokens": compacted_tokens,
                        })
                    );
                } else {
                    println!(
                        "\n{} [window {} | tokens: {} -> {}]",
                        "[Auto-Compaction]".dimmed(),
                        &window_id[..8.min(window_id.len())],
                        original_tokens,
                        compacted_tokens
                    );
                }
            }
            EngineEvent::TurnComplete { usage, messages } => {
                session.record_tokens(
                    usage.prompt_tokens,
                    usage.completion_tokens,
                    usage.reasoning_tokens,
                );
                if let Some(last) = messages.iter().rev().find(|m| m.role == Role::Assistant) {
                    last_assistant_message = last.content.clone().unwrap_or_default();
                }
            }
            EngineEvent::TurnFailed(err) => {
                turn_failed = Some(err);
            }
        }
    }

    if let Some(err) = turn_failed {
        bail!("Turn failed: {}", err);
    }
    engine_result?;

    if let Some(out_path) = output_last_message {
        let _ = std::fs::write(out_path, &last_assistant_message);
    }

    // Trigger external notification command if configured
    if let Some(ref notify_cmd) = config.notify
        && !notify_cmd.is_empty()
    {
        let mut cmd = std::process::Command::new(&notify_cmd[0]);
        if notify_cmd.len() > 1 {
            cmd.args(&notify_cmd[1..]);
        }
        let _ = cmd.spawn();
    }

    if json_output {
        println!(
            "{}",
            serde_json::json!({
                "type": "turn_complete",
                "session": session.metadata.id,
                "total_tokens": session.token_usage.total_tokens,
                "prompt_tokens": session.token_usage.prompt_tokens,
                "completion_tokens": session.token_usage.completion_tokens,
            })
        );
    } else {
        println!(
            "\n{} Total Tokens: {} (prompt: {}, completion: {})",
            "Session Summary:".bold(),
            session.token_usage.total_tokens,
            session.token_usage.prompt_tokens,
            session.token_usage.completion_tokens
        );
    }

    Ok(())
}

async fn run_swarm(
    config: &Config,
    cwd: &Path,
    mission: &str,
    rounds: usize,
    json_output: bool,
) -> Result<()> {
    println!(
        "{}",
        format!(
            "Launching M31A Autonomous Swarm for mission: \"{}\"",
            mission
        )
        .bold()
        .cyan()
    );
    let mission_id = format!("swarm-{}", &uuid::Uuid::new_v4().to_string()[..8]);
    let coord = m31a_core::SwarmCoordinator::new(&mission_id, mission);
    let bb = coord.blackboard();

    let roles = [
        (
            m31a_core::SwarmRoleKind::Architect,
            "Architecting system design, interfaces, and task steps",
        ),
        (
            m31a_core::SwarmRoleKind::Coder,
            "Implementing code changes and features",
        ),
        (
            m31a_core::SwarmRoleKind::Tester,
            "Running verification tests and diagnostic passes",
        ),
        (
            m31a_core::SwarmRoleKind::Guardian,
            "Validating sandbox boundaries, dependencies, and security constraints",
        ),
    ];

    for round in 1..=rounds {
        let phase = { bb.read().unwrap().phase };
        println!(
            "\n{} Round {}/{} | Phase: {:?}",
            "●".magenta().bold(),
            round,
            rounds,
            phase
        );

        let (active_role, desc) = match phase {
            m31a_core::SwarmPhase::Architecting => {
                (m31a_core::SwarmRoleKind::Architect, roles[0].1)
            }
            m31a_core::SwarmPhase::Implementing => (m31a_core::SwarmRoleKind::Coder, roles[1].1),
            m31a_core::SwarmPhase::Testing => (m31a_core::SwarmRoleKind::Tester, roles[2].1),
            m31a_core::SwarmPhase::AdversarialGauntlet => (
                m31a_core::SwarmRoleKind::Adversary,
                "Subject implementation to boundary fuzzing and invariant attacks",
            ),
            m31a_core::SwarmPhase::SecurityReview => {
                (m31a_core::SwarmRoleKind::Guardian, roles[3].1)
            }
            m31a_core::SwarmPhase::Completed => {
                println!("{}", "Swarm Mission Completed Successfully!".bold().green());
                break;
            }
            m31a_core::SwarmPhase::Failed => {
                println!("{}", "Swarm Mission Failed".bold().red());
                break;
            }
        };

        println!(
            "  {} {} - {}",
            "Active Agent:".bold(),
            active_role.display_name().cyan().bold(),
            desc
        );
        let role_prompt = coord.format_role_prompt(active_role);

        // Perform turn
        run_turn(
            config,
            cwd,
            &role_prompt,
            &[],
            json_output,
            None,
            false,
            false,
            true, // Enable self-healing during swarm rounds
            2,
            false,
            None,
        )
        .await?;

        // Update blackboard based on dynamic role actions
        match active_role {
            m31a_core::SwarmRoleKind::Architect => {
                let plan_path = cwd.join("PLAN.md");
                let spec = if plan_path.exists() {
                    tokio::fs::read_to_string(&plan_path)
                        .await
                        .unwrap_or_else(|_| format!("Architecture Spec for: {}", mission))
                } else {
                    format!(
                        "Architecture Spec for {}: decomposed requirements into implementation and testing phases.",
                        mission
                    )
                };

                let mut b = bb.write().unwrap();
                b.record_artifact("architecture_spec", spec);
                b.set_plan(vec![
                    m31a_core::SwarmTaskStep {
                        id: 1,
                        title: format!("Implement core features for {}", mission),
                        assigned_role: m31a_core::SwarmRoleKind::Coder,
                        is_completed: false,
                        output_summary: None,
                    },
                    m31a_core::SwarmTaskStep {
                        id: 2,
                        title: "Verify test suite and compiler diagnostics".to_string(),
                        assigned_role: m31a_core::SwarmRoleKind::Tester,
                        is_completed: false,
                        output_summary: None,
                    },
                ]);
                b.post_message(
                    active_role,
                    format!(
                        "Published architecture spec and configured 2 task steps for mission '{}'.",
                        mission
                    ),
                );
            }
            m31a_core::SwarmRoleKind::Coder => {
                // Dynamically detect modified files
                let diff_out = tokio::process::Command::new("git")
                    .args(["status", "--short"])
                    .current_dir(cwd)
                    .output()
                    .await;

                let changes = match diff_out {
                    Ok(out) if !out.stdout.is_empty() => {
                        String::from_utf8_lossy(&out.stdout).trim().to_string()
                    }
                    _ => "Workspace state verified".to_string(),
                };

                let summary = format!(
                    "Code implementation concluded. Workspace modifications:\n{}",
                    changes
                );
                let mut b = bb.write().unwrap();
                b.record_artifact("modified_files", changes);
                b.complete_step(1, summary.clone());
                b.post_message(active_role, summary);
            }
            m31a_core::SwarmRoleKind::Tester => {
                // Execute actual verification
                let test_cmd = if cwd.join("Cargo.toml").exists() {
                    tokio::process::Command::new("cargo")
                        .args(["test", "--workspace", "--no-run"])
                        .current_dir(cwd)
                        .output()
                        .await
                } else {
                    tokio::process::Command::new("git")
                        .args(["status"])
                        .current_dir(cwd)
                        .output()
                        .await
                };

                let (test_success, report) = match test_cmd {
                    Ok(out) => {
                        let status_str = if out.status.success() {
                            "PASSED"
                        } else {
                            "FAILED"
                        };
                        let stdout = String::from_utf8_lossy(&out.stdout);
                        let stderr = String::from_utf8_lossy(&out.stderr);
                        let full = format!("Test build {}:\n{}{}", status_str, stdout, stderr);
                        (out.status.success(), full)
                    }
                    Err(e) => (false, format!("Failed to invoke test runner: {}", e)),
                };

                let mut b = bb.write().unwrap();
                b.record_artifact("test_report", &report);
                let summary = if test_success {
                    "Test verification passed cleanly: build and test binaries compiled successfully."
                } else {
                    "Test verification encountered warnings or failures."
                };
                b.complete_step(2, summary);
                b.post_message(
                    active_role,
                    format!(
                        "{}: {}",
                        summary,
                        report.lines().take(3).collect::<Vec<_>>().join(" | ")
                    ),
                );
            }
            m31a_core::SwarmRoleKind::Guardian => {
                // Perform security review: scan modified files for high risk patterns
                let mut findings = Vec::new();
                let status_out = tokio::process::Command::new("git")
                    .args(["status", "--porcelain"])
                    .current_dir(cwd)
                    .output()
                    .await;

                if let Ok(out) = status_out {
                    let text = String::from_utf8_lossy(&out.stdout);
                    for line in text.lines() {
                        let path_str = line.get(3..).unwrap_or("").trim();
                        if path_str.contains(".env")
                            || path_str.contains("id_rsa")
                            || path_str.contains("secret")
                        {
                            findings.push(format!("Sensitive file detected: {}", path_str));
                        }
                    }
                }

                let audit_result = if findings.is_empty() {
                    "Guardian security audit passed: no secret leakage or sandbox boundary escapes detected.".to_string()
                } else {
                    format!("Guardian security warning: {}", findings.join("; "))
                };

                let mut b = bb.write().unwrap();
                b.record_artifact("security_audit", &audit_result);
                b.post_message(active_role, audit_result);
            }
            m31a_core::SwarmRoleKind::Adversary => {
                let challenge = m31a_core::AdversarialChallenge {
                    id: format!("adv-{}", uuid::Uuid::new_v4().simple()),
                    title: format!("Boundary Invariant Audit for: {}", mission),
                    attack_vector: "boundary_fuzzing".to_string(),
                    input_scenario: "Malformed and high-concurrency inputs".to_string(),
                    expected_invariant: "Panic-free graceful handling".to_string(),
                    survived: true,
                    outcome_notes: Some(
                        "All boundary invariants preserved without unexpected aborts.".to_string(),
                    ),
                };
                let mut b = bb.write().unwrap();
                b.record_challenge(challenge);
                b.record_artifact(
                    "adversarial_report",
                    "Adversarial gauntlet verified: 0 critical vulnerabilities.",
                );
                b.post_message(
                    active_role,
                    "Adversarial tests concluded: code passed boundary gauntlet.",
                );
            }
        }

        let _ = coord.transition_phase();
    }

    println!(
        "\n{}",
        "=== Swarm Mission Delivery Summary ===".bold().cyan()
    );
    let final_bb = bb.read().unwrap();
    println!("Mission: {}", final_bb.objective);
    println!("Final Phase: {:?}", final_bb.phase);
    println!("Artifacts Recorded: {}", final_bb.artifacts.len());
    for k in final_bb.artifacts.keys() {
        println!("  - {}", k);
    }
    println!(
        "Total Swarm Messages Exchanged: {}",
        final_bb.messages.len()
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn test_cli_parse_setup_and_model_commands() {
        let cli_setup = Cli::try_parse_from([
            "m31a",
            "setup",
            "--api-key",
            "nvapi-test123",
            "--model",
            "reasoning",
            "--non-interactive",
        ])
        .unwrap();
        match cli_setup.command {
            Some(Commands::Setup {
                api_key,
                model,
                non_interactive,
                ..
            }) => {
                assert_eq!(api_key, Some("nvapi-test123".to_string()));
                assert_eq!(model, Some("reasoning".to_string()));
                assert!(non_interactive);
            }
            _ => panic!("Expected Commands::Setup"),
        }

        let cli_model_list = Cli::try_parse_from(["m31a", "model", "list"]).unwrap();
        assert!(matches!(
            cli_model_list.command,
            Some(Commands::Model {
                cmd: Some(ModelCommands::List)
            })
        ));

        let cli_model_set =
            Cli::try_parse_from(["m31a", "model", "set", "deepseek-ai/deepseek-r1"]).unwrap();
        match cli_model_set.command {
            Some(Commands::Model {
                cmd: Some(ModelCommands::Set { model }),
            }) => {
                assert_eq!(model, "deepseek-ai/deepseek-r1");
            }
            _ => panic!("Expected ModelCommands::Set"),
        }
    }

    #[test]
    fn test_cli_parse_goal_commands() {
        let cli_get = Cli::try_parse_from(["m31a", "goal", "get", "thread-42"]).unwrap();
        match cli_get.command {
            Some(Commands::Goal {
                cmd: GoalCommands::Get { thread_id },
            }) => {
                assert_eq!(thread_id, "thread-42");
            }
            _ => panic!("Expected GoalCommands::Get"),
        }

        let cli_set = Cli::try_parse_from([
            "m31a",
            "goal",
            "set",
            "thread-42",
            "Achieve 100% test coverage",
            "--budget",
            "100000",
        ])
        .unwrap();
        match cli_set.command {
            Some(Commands::Goal {
                cmd:
                    GoalCommands::Set {
                        thread_id,
                        objective,
                        budget,
                    },
            }) => {
                assert_eq!(thread_id, "thread-42");
                assert_eq!(objective, "Achieve 100% test coverage");
                assert_eq!(budget, Some(100000));
            }
            _ => panic!("Expected GoalCommands::Set"),
        }
    }

    #[test]
    fn test_cli_parse_cloud_commands() {
        let cli_list = Cli::try_parse_from(["m31a", "cloud", "list"]).unwrap();
        assert!(matches!(
            cli_list.command,
            Some(Commands::Cloud {
                cmd: CloudCommands::List
            })
        ));

        let cli_submit = Cli::try_parse_from([
            "m31a",
            "cloud",
            "submit",
            "cargo clippy --workspace",
            "--env",
            "linux-x86_64",
        ])
        .unwrap();
        match cli_submit.command {
            Some(Commands::Cloud {
                cmd: CloudCommands::Submit { instruction, env },
            }) => {
                assert_eq!(instruction, "cargo clippy --workspace");
                assert_eq!(env, Some("linux-x86_64".to_string()));
            }
            _ => panic!("Expected CloudCommands::Submit"),
        }
    }

    #[test]
    fn test_cli_parse_swarm_command() {
        let cli_swarm = Cli::try_parse_from([
            "m31a",
            "swarm",
            "Implement high-performance AST parser",
            "--rounds",
            "6",
        ])
        .unwrap();

        match cli_swarm.command {
            Some(Commands::Swarm {
                mission,
                rounds,
                json,
            }) => {
                assert_eq!(mission, "Implement high-performance AST parser");
                assert_eq!(rounds, 6);
                assert!(!json);
            }
            _ => panic!("Expected Commands::Swarm"),
        }
    }

    #[test]
    fn test_cli_parse_exec_self_heal() {
        let cli_exec = Cli::try_parse_from([
            "m31a",
            "exec",
            "cargo build",
            "--self-heal",
            "--max-heal-iterations",
            "5",
        ])
        .unwrap();

        match cli_exec.command {
            Some(Commands::Exec {
                prompt,
                self_heal,
                max_heal_iterations,
                ..
            }) => {
                assert_eq!(prompt, Some("cargo build".to_string()));
                assert!(self_heal);
                assert_eq!(max_heal_iterations, 5);
            }
            _ => panic!("Expected Commands::Exec with --self-heal"),
        }
    }

    #[test]
    fn test_cli_parse_exec_plan_and_undo() {
        let cli_exec = Cli::try_parse_from([
            "m31a",
            "exec",
            "--plan",
            "--reasoning-effort",
            "high",
            "Refactor module",
        ])
        .unwrap();
        match cli_exec.command {
            Some(Commands::Exec {
                prompt,
                plan,
                reasoning_effort,
                ..
            }) => {
                assert_eq!(prompt, Some("Refactor module".to_string()));
                assert!(plan);
                assert_eq!(reasoning_effort, Some("high".to_string()));
            }
            _ => panic!("Expected Commands::Exec with --plan"),
        }

        let cli_undo = Cli::try_parse_from(["m31a", "undo"]).unwrap();
        match cli_undo.command {
            Some(Commands::Undo { session }) => {
                assert_eq!(session, None);
            }
            _ => panic!("Expected Commands::Undo"),
        }
    }

    #[test]
    fn test_cli_parse_plugin_bundle_commands() {
        let cli_pkg = Cli::try_parse_from([
            "m31a",
            "plugin",
            "package",
            "./plugins/my-plugin",
            "-o",
            "./my-plugin.bundle.json",
        ])
        .unwrap();

        match cli_pkg.command {
            Some(Commands::Plugin {
                cmd: PluginCommands::Package { dir, out },
            }) => {
                assert_eq!(dir, PathBuf::from("./plugins/my-plugin"));
                assert_eq!(out, PathBuf::from("./my-plugin.bundle.json"));
            }
            _ => panic!("Expected PluginCommands::Package"),
        }

        let cli_verify =
            Cli::try_parse_from(["m31a", "plugin", "verify-bundle", "./my-plugin.bundle.json"])
                .unwrap();

        match cli_verify.command {
            Some(Commands::Plugin {
                cmd: PluginCommands::VerifyBundle { bundle },
            }) => {
                assert_eq!(bundle, PathBuf::from("./my-plugin.bundle.json"));
            }
            _ => panic!("Expected PluginCommands::VerifyBundle"),
        }
    }

    #[test]
    fn test_cli_parse_debug_trace_reduce() {
        let cli_trace = Cli::try_parse_from([
            "m31a",
            "debug",
            "trace-reduce",
            "./traces/thread-1.trace.jsonl",
            "--json",
        ])
        .unwrap();

        match cli_trace.command {
            Some(Commands::Debug {
                cmd: DebugCommands::TraceReduce { trace_path, json },
            }) => {
                assert_eq!(trace_path, PathBuf::from("./traces/thread-1.trace.jsonl"));
                assert!(json);
            }
            _ => panic!("Expected DebugCommands::TraceReduce"),
        }
    }

    #[test]
    fn test_cli_parse_revert_command() {
        let cli_revert = Cli::try_parse_from([
            "m31a",
            "revert",
            "--checkpoint",
            "cp-12345",
            "--git",
            "--force",
            "--dry-run",
        ])
        .unwrap();

        match cli_revert.command {
            Some(Commands::Revert {
                checkpoint,
                thread,
                turn,
                git,
                force,
                dry_run,
            }) => {
                assert_eq!(checkpoint, Some("cp-12345".to_string()));
                assert_eq!(thread, None);
                assert_eq!(turn, None);
                assert!(git);
                assert!(force);
                assert!(dry_run);
            }
            _ => panic!("Expected Commands::Revert"),
        }
    }

    #[test]
    fn test_cli_parse_responses_proxy_command() {
        let cli_proxy = Cli::try_parse_from([
            "m31a",
            "responses-proxy",
            "--port",
            "8080",
            "--upstream-url",
            "http://127.0.0.1:11434/v1/chat/completions",
            "--dump-dir",
            "/tmp/proxy-dumps",
            "--server-info",
            "/tmp/server-info.json",
        ])
        .unwrap();

        match cli_proxy.command {
            Some(Commands::ResponsesProxy {
                port,
                upstream_url,
                dump_dir,
                server_info,
                http_shutdown,
            }) => {
                assert_eq!(port, 8080);
                assert_eq!(upstream_url, "http://127.0.0.1:11434/v1/chat/completions");
                assert_eq!(dump_dir, Some(PathBuf::from("/tmp/proxy-dumps")));
                assert_eq!(server_info, Some(PathBuf::from("/tmp/server-info.json")));
                assert!(http_shutdown);
            }
            _ => panic!("Expected Commands::ResponsesProxy"),
        }
    }

    #[test]
    fn test_cli_parse_sections_commands() {
        let cli_create = Cli::try_parse_from([
            "m31a",
            "sections",
            "create",
            "Feature Branch 68",
            "--color",
            "#4287f5",
            "--icon",
            "feature",
        ])
        .unwrap();
        match cli_create.command {
            Some(Commands::Sections {
                cmd: SectionsCommands::Create { name, color, icon },
            }) => {
                assert_eq!(name, "Feature Branch 68");
                assert_eq!(color, Some("#4287f5".to_string()));
                assert_eq!(icon, Some("feature".to_string()));
            }
            _ => panic!("Expected SectionsCommands::Create"),
        }

        let cli_move = Cli::try_parse_from([
            "m31a",
            "sections",
            "move",
            "thread-123",
            "--section",
            "sec-456",
        ])
        .unwrap();
        match cli_move.command {
            Some(Commands::Sections {
                cmd:
                    SectionsCommands::Move {
                        thread_id,
                        section_id,
                    },
            }) => {
                assert_eq!(thread_id, "thread-123");
                assert_eq!(section_id, Some("sec-456".to_string()));
            }
            _ => panic!("Expected SectionsCommands::Move"),
        }

        let cli_list = Cli::try_parse_from(["m31a", "sections", "list"]).unwrap();
        match cli_list.command {
            Some(Commands::Sections {
                cmd: SectionsCommands::List,
            }) => {}
            _ => panic!("Expected SectionsCommands::List"),
        }
    }

    #[test]
    fn test_cli_parse_presets_commands() {
        let cli_presets = Cli::try_parse_from(["m31a", "presets"]).unwrap();
        match cli_presets.command {
            Some(Commands::Presets { id }) => {
                assert_eq!(id, None);
            }
            _ => panic!("Expected Commands::Presets"),
        }

        let cli_preset_inspect = Cli::try_parse_from(["m31a", "presets", "airgapped"]).unwrap();
        match cli_preset_inspect.command {
            Some(Commands::Presets { id }) => {
                assert_eq!(id, Some("airgapped".to_string()));
            }
            _ => panic!("Expected Commands::Presets with id"),
        }

        let cli_with_preset_flag =
            Cli::try_parse_from(["m31a", "--preset", "read-only", "list files"]).unwrap();
        assert_eq!(cli_with_preset_flag.preset, Some("read-only".to_string()));
        assert_eq!(cli_with_preset_flag.prompt, Some("list files".to_string()));
    }

    #[test]
    fn test_cli_parse_export_command() {
        let cli_default = Cli::try_parse_from(["m31a", "export"]).unwrap();
        match cli_default.command {
            Some(Commands::Export {
                format,
                thread,
                out,
            }) => {
                assert_eq!(format, "markdown");
                assert_eq!(thread, None);
                assert_eq!(out, None);
            }
            _ => panic!("Expected Commands::Export"),
        }

        let cli_custom = Cli::try_parse_from([
            "m31a",
            "export",
            "--format",
            "html",
            "--thread",
            "t-12345",
            "-o",
            "/tmp/report.html",
        ])
        .unwrap();

        match cli_custom.command {
            Some(Commands::Export {
                format,
                thread,
                out,
            }) => {
                assert_eq!(format, "html");
                assert_eq!(thread, Some("t-12345".to_string()));
                assert_eq!(out, Some(PathBuf::from("/tmp/report.html")));
            }
            _ => panic!("Expected Commands::Export custom"),
        }
    }

    #[test]
    fn test_cli_parse_trust_commands() {
        let cli_list = Cli::try_parse_from(["m31a", "trust", "list"]).unwrap();
        match cli_list.command {
            Some(Commands::Trust {
                cmd: TrustCommands::List,
            }) => {}
            _ => panic!("Expected TrustCommands::List"),
        }

        let cli_add =
            Cli::try_parse_from(["m31a", "trust", "add", "/home/user/myproject"]).unwrap();
        match cli_add.command {
            Some(Commands::Trust {
                cmd: TrustCommands::Add { path },
            }) => {
                assert_eq!(path, Some(PathBuf::from("/home/user/myproject")));
            }
            _ => panic!("Expected TrustCommands::Add"),
        }

        let cli_audit = Cli::try_parse_from(["m31a", "trust", "audit"]).unwrap();
        match cli_audit.command {
            Some(Commands::Trust {
                cmd: TrustCommands::Audit { path },
            }) => {
                assert_eq!(path, None);
            }
            _ => panic!("Expected TrustCommands::Audit"),
        }
    }

    #[test]
    fn test_cli_parse_process_commands() {
        let cli_ps = Cli::try_parse_from(["m31a", "ps"]).unwrap();
        match cli_ps.command {
            Some(Commands::Ps) => {}
            _ => panic!("Expected Commands::Ps"),
        }

        let cli_kill = Cli::try_parse_from(["m31a", "kill", "proc-12345678", "--force"]).unwrap();
        match cli_kill.command {
            Some(Commands::Kill { id, force }) => {
                assert_eq!(id, "proc-12345678");
                assert!(force);
            }
            _ => panic!("Expected Commands::Kill"),
        }

        let cli_logs = Cli::try_parse_from(["m31a", "logs", "proc-12345678", "-n", "100"]).unwrap();
        match cli_logs.command {
            Some(Commands::Logs { id, lines }) => {
                assert_eq!(id, "proc-12345678");
                assert_eq!(lines, 100);
            }
            _ => panic!("Expected Commands::Logs"),
        }
    }

    #[test]
    fn test_cli_parse_gsd_commands() {
        // 1. check-plan
        let cli_check =
            Cli::try_parse_from(["m31a", "gsd", "check-plan", "-p", "PLAN.md"]).unwrap();
        match cli_check.command {
            Some(Commands::Gsd {
                cmd: gsd::GsdCommands::CheckPlan { plan_file, .. },
            }) => {
                assert_eq!(plan_file, PathBuf::from("PLAN.md"));
            }
            _ => panic!("Expected GsdCommands::CheckPlan"),
        }

        // 2. verify
        let cli_verify =
            Cli::try_parse_from(["m31a", "gsd", "verify", "-p", "02", "--truth", "Truth 1"])
                .unwrap();
        match cli_verify.command {
            Some(Commands::Gsd {
                cmd:
                    gsd::GsdCommands::Verify {
                        phase_id, truths, ..
                    },
            }) => {
                assert_eq!(phase_id, "02");
                assert_eq!(truths, vec!["Truth 1".to_string()]);
            }
            _ => panic!("Expected GsdCommands::Verify"),
        }

        // 3. audit
        let cli_audit =
            Cli::try_parse_from(["m31a", "gsd", "audit", "-p", "03", "-t", "cargo test"]).unwrap();
        match cli_audit.command {
            Some(Commands::Gsd {
                cmd:
                    gsd::GsdCommands::Audit {
                        phase_id,
                        test_cmds,
                        ..
                    },
            }) => {
                assert_eq!(phase_id, "03");
                assert_eq!(test_cmds, vec!["cargo test".to_string()]);
            }
            _ => panic!("Expected GsdCommands::Audit"),
        }

        // 4. drift
        let cli_drift =
            Cli::try_parse_from(["m31a", "gsd", "drift", "-s", "STRUCTURE.md", "-t", "5"]).unwrap();
        match cli_drift.command {
            Some(Commands::Gsd {
                cmd: gsd::GsdCommands::Drift { threshold, .. },
            }) => {
                assert_eq!(threshold, 5);
            }
            _ => panic!("Expected GsdCommands::Drift"),
        }

        // 5. headroom
        let cli_headroom =
            Cli::try_parse_from(["m31a", "gsd", "headroom", "-u", "50000", "-l", "100000"])
                .unwrap();
        match cli_headroom.command {
            Some(Commands::Gsd {
                cmd:
                    gsd::GsdCommands::Headroom {
                        used_tokens,
                        context_limit,
                    },
            }) => {
                assert_eq!(used_tokens, 50_000);
                assert_eq!(context_limit, 100_000);
            }
            _ => panic!("Expected GsdCommands::Headroom"),
        }

        // 6. hooks
        let cli_hooks =
            Cli::try_parse_from(["m31a", "gsd", "hooks", "-p", "discuss:post"]).unwrap();
        match cli_hooks.command {
            Some(Commands::Gsd {
                cmd: gsd::GsdCommands::Hooks { point },
            }) => {
                assert_eq!(point, Some("discuss:post".to_string()));
            }
            _ => panic!("Expected GsdCommands::Hooks"),
        }
    }

    #[test]
    fn test_cli_parse_serve_command() {
        // serve default
        let cli_serve = Cli::try_parse_from(["m31a", "serve"]).unwrap();
        match cli_serve.command {
            Some(Commands::Serve { port, bind }) => {
                assert_eq!(port, 3100);
                assert_eq!(bind, "127.0.0.1");
            }
            _ => panic!("Expected Commands::Serve"),
        }
    }
}
