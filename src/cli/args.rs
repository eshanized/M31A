//! Clap 4 CLI Command & Argument Hierarchy (CLI-01, CLI-02).

use clap::{Args, Parser, Subcommand, ValueEnum};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Output formats supported by M31A CLI (CLI-02).
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum OutputFormat {
    #[default]
    Text,
    Json,
    StreamJson,
}

/// M31 Autonomous (M31A) - Command Line Interface.
#[derive(Parser, Debug, Clone)]
#[command(
    name = "m31a",
    version,
    about = "M31 Autonomous (M31A) - Rust-native autonomous software engineering runtime",
    long_about = "The model proposes. The runtime decides. M31A is an autonomous coding runtime providing non-bypassable policy gates, verifiable execution, and isolated git attribution."
)]
pub struct Cli {
    /// Path to custom configuration file.
    #[arg(global = true, short, long)]
    pub config: Option<PathBuf>,

    /// Named configuration profile to activate.
    #[arg(global = true, short, long)]
    pub profile: Option<String>,

    /// Override active model name.
    #[arg(global = true, long)]
    pub model: Option<String>,

    /// Override autonomy level/mode.
    #[arg(global = true, long)]
    pub autonomy: Option<String>,

    /// Output format: text, json, stream-json.
    #[arg(global = true, short, long, value_enum, default_value_t = OutputFormat::Text)]
    pub output: OutputFormat,

    /// Suppress informational console output.
    #[arg(global = true, short, long)]
    pub quiet: bool,

    /// Enable verbose diagnostic logs.
    #[arg(global = true, short, long)]
    pub verbose: bool,

    /// Path to target workspace directory.
    #[arg(global = true, short, long)]
    pub workspace: Option<PathBuf>,

    /// Subcommand to execute.
    #[command(subcommand)]
    pub command: Option<Commands>,
}

/// Primary CLI Subcommands (CLI-01).
#[derive(Subcommand, Debug, Clone, PartialEq)]
pub enum Commands {
    /// Interactive developer session operations: list, show, resume, new.
    Session(SessionArgs),

    /// Mission operations: run, list, show, pause, resume, cancel, fork.
    Mission(MissionArgs),

    /// Task management and inspection.
    Task(TaskArgs),

    /// Agent roles and pool inspection.
    Agent(AgentArgs),

    /// System capabilities and provider status.
    Capability(CapabilityArgs),

    /// Policy evaluation and security rule checks.
    Policy(PolicyArgs),

    /// State checkpoint management.
    Checkpoint(CheckpointArgs),

    /// Inspect generated execution artifacts.
    Artifact(ArtifactArgs),

    /// Run comprehensive environmental diagnostic probes.
    Doctor(DoctorArgs),

    /// Local telemetry and execution trace inspection.
    Telemetry(TelemetryArgs),

    /// Configuration validation and inspection.
    Config(ConfigArgs),

    /// Autonomous evaluation harness for acceptance scenarios.
    Eval(EvalArgs),

    /// Launch full Ratatui interactive TUI cockpit.
    Tui,

    /// Initialize workspace onboarding state (idempotent; use --force to re-enter setup).
    Init(InitArgs),

    /// Print version information.
    Version,
}

#[derive(Args, Debug, Clone, PartialEq)]
pub struct InitArgs {
    /// Force re-entry into first-run onboarding even if already initialized.
    #[arg(long)]
    pub force: bool,
}

#[derive(Args, Debug, Clone, PartialEq)]
pub struct SessionArgs {
    #[command(subcommand)]
    pub command: Option<SessionCommands>,
}

#[derive(Subcommand, Debug, Clone, PartialEq)]
pub enum SessionCommands {
    /// Start a new interactive developer session.
    New,

    /// List all known developer sessions.
    List,

    /// Show details and conversation history of a specific session.
    Show {
        /// Session UUID.
        id: String,
    },

    /// Resume an existing session.
    Resume {
        /// Session UUID.
        id: String,
    },
}

#[derive(Args, Debug, Clone, PartialEq)]
pub struct MissionArgs {
    #[command(subcommand)]
    pub command: MissionCommands,
}

#[derive(Subcommand, Debug, Clone, PartialEq)]
pub enum MissionCommands {
    /// Start a new autonomous mission.
    Run {
        /// Objective prompt describing the goal.
        prompt: String,

        /// Profile override for this mission.
        #[arg(long)]
        profile: Option<String>,

        /// Wait for operator approval when policy triggers Ask decision.
        #[arg(long)]
        wait_for_approval: bool,
    },
    /// List all known missions.
    List {
        /// Include finished/cancelled missions.
        #[arg(long)]
        all: bool,
    },
    /// Inspect details of a specific mission.
    Show {
        /// Mission UUID or prefix.
        id: String,
    },
    /// Pause a running mission.
    Pause {
        /// Mission UUID.
        id: String,
    },
    /// Resume a paused mission.
    Resume {
        /// Mission UUID.
        id: String,
    },
    /// Cancel a running mission.
    Cancel {
        /// Mission UUID.
        id: String,

        /// Reason for cancellation.
        #[arg(short, long)]
        reason: Option<String>,
    },
    /// Fork a mission into a fresh branch/mission.
    Fork {
        /// Source mission UUID to fork from.
        id: String,

        /// Updated goal prompt for the fork.
        #[arg(long)]
        prompt: Option<String>,
    },
}

#[derive(Args, Debug, Clone, PartialEq)]
pub struct TaskArgs {
    #[command(subcommand)]
    pub command: TaskCommands,
}

#[derive(Subcommand, Debug, Clone, PartialEq)]
pub enum TaskCommands {
    /// List tasks within a mission.
    List {
        /// Mission UUID to filter by.
        #[arg(short, long)]
        mission_id: Option<String>,
    },
    /// Inspect a specific task.
    Show {
        /// Task UUID.
        id: String,
    },
}

#[derive(Args, Debug, Clone, PartialEq)]
pub struct AgentArgs {
    #[command(subcommand)]
    pub command: AgentCommands,
}

#[derive(Subcommand, Debug, Clone, PartialEq)]
pub enum AgentCommands {
    /// List active or registered agent roles.
    List,
}

#[derive(Args, Debug, Clone, PartialEq)]
pub struct CapabilityArgs {
    #[command(subcommand)]
    pub command: CapabilityCommands,
}

#[derive(Subcommand, Debug, Clone, PartialEq)]
pub enum CapabilityCommands {
    /// List available capabilities and their provider health status.
    List,
}

#[derive(Args, Debug, Clone, PartialEq)]
pub struct PolicyArgs {
    #[command(subcommand)]
    pub command: PolicyCommands,
}

#[derive(Subcommand, Debug, Clone, PartialEq)]
pub enum PolicyCommands {
    /// Check whether a tool or action is allowed under current policy.
    Check {
        /// Tool or action identifier (e.g. "fs:write", "cli:bash").
        tool: String,

        /// Optional mission ID context.
        #[arg(short, long)]
        mission_id: Option<String>,
    },
}

#[derive(Args, Debug, Clone, PartialEq)]
pub struct CheckpointArgs {
    #[command(subcommand)]
    pub command: CheckpointCommands,
}

#[derive(Subcommand, Debug, Clone, PartialEq)]
pub enum CheckpointCommands {
    /// List checkpoints for a mission.
    List {
        #[arg(short, long)]
        mission_id: Option<String>,
    },
    /// Restore execution state from a checkpoint.
    Restore {
        /// Checkpoint UUID.
        id: String,
    },
}

#[derive(Args, Debug, Clone, PartialEq)]
pub struct ArtifactArgs {
    #[command(subcommand)]
    pub command: ArtifactCommands,
}

#[derive(Subcommand, Debug, Clone, PartialEq)]
pub enum ArtifactCommands {
    /// List artifacts, optionally filtered by mission ID.
    List {
        /// Optional mission UUID filter.
        mission_id: Option<String>,
    },

    /// Show or extract an artifact by ID.
    Show {
        /// Artifact identifier.
        id: String,
    },
}

#[derive(Args, Debug, Clone, PartialEq)]
pub struct DoctorArgs {
    /// Emit machine-readable JSON health report.
    #[arg(long)]
    pub json: bool,

    /// Filter probes by category (environment, git, models, sandbox, storage, network).
    #[arg(long)]
    pub category: Option<String>,
}

#[derive(Args, Debug, Clone, PartialEq)]
pub struct ConfigArgs {
    #[command(subcommand)]
    pub command: ConfigCommands,
}

#[derive(Subcommand, Debug, Clone, PartialEq)]
pub enum ConfigCommands {
    /// Validate configuration schema and report errors.
    Validate {
        /// Path to configuration file to validate.
        #[arg(long)]
        path: Option<PathBuf>,
    },
    /// Get a configuration value by key.
    Get { key: String },
    /// Set a configuration value by key.
    Set { key: String, value: String },
    /// List all loaded configuration sources in precedence order.
    Sources,
    /// Explain the resolution and provenance of a configuration key.
    Explain { key: String },
}

#[derive(Args, Debug, Clone, PartialEq)]
pub struct TelemetryArgs {
    #[command(subcommand)]
    pub command: TelemetryCommands,
}

#[derive(Subcommand, Debug, Clone, PartialEq)]
pub enum TelemetryCommands {
    /// Inspect recorded telemetry spans and metrics for a mission.
    Inspect {
        /// Mission ID to inspect.
        mission_id: String,

        /// Show high-level summary.
        #[arg(long)]
        summary: bool,

        /// Show recorded execution spans.
        #[arg(long)]
        spans: bool,

        /// Show metric samples.
        #[arg(long)]
        metrics: bool,

        /// Export machine-readable format (json or ndjson).
        #[arg(long)]
        export: Option<String>,
    },
}

#[derive(Args, Debug, Clone, PartialEq)]
pub struct EvalArgs {
    #[command(subcommand)]
    pub command: EvalCommands,
}

#[derive(Subcommand, Debug, Clone, PartialEq)]
pub enum EvalCommands {
    /// Execute one or all autonomous acceptance evaluation scenarios.
    Run {
        /// Target specific scenario by ID (e.g. "a", "b", "c").
        #[arg(long, short)]
        scenario: Option<String>,

        /// Run all acceptance scenarios.
        #[arg(long)]
        all: bool,

        /// Number of benchmark iterations (default 1).
        #[arg(long, short)]
        iterations: Option<usize>,

        /// Output format (markdown or json).
        #[arg(long, short)]
        output: Option<String>,
    },
}
