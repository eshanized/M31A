//! Developer Interaction Layer & Agentic Interaction Protocol (PRD §01, CLI-01–CLI-04).
//!
//! "Input -> Interaction Parser -> Typed Ui/Application Action -> Application Command -> Runtime -> Events / State -> CLI/TUI projection."
//!
//! Exposes:
//! - `action`: Typed UI and Application Actions (`ApplicationAction`, `InteractionIntent`)
//! - `mentions`: @file, @directory, and @path:line parser and resolver
//! - `commands`: Centralized slash command registry and command metadata
//! - `session`: Durable session aggregate and conversation repository
//! - `state`: Session prompt state machine
//! - `events`: Decoupled user-facing interaction events
//! - `parser`: Unified input parser and deterministic intent classifier
//! - `approval`: Interactive CLI/TUI approval channel
//! - `runner`: Interactive REPL session loop

pub mod action;
pub mod approval;
pub mod commands;
pub mod continuation;
pub mod events;
pub mod mentions;
pub mod parser;
pub mod runner;
pub mod session;
pub mod state;
pub mod user_commands;

pub use action::{ApplicationAction, InteractionIntent};
pub use approval::{InteractiveApprovalChannel, PendingApprovalPrompt};
pub use commands::{
    CommandContext, CommandHandler, CommandOutput, CommandSideEffect, SlashCommand,
    SlashCommandRegistry,
};
pub use continuation::{ActiveExecution, handle_user_text_submitted};
pub use events::InteractionEvent;
pub use mentions::{
    LineRange, MentionKind, MentionParser, MentionReference, MessageSegment, ParsedUserMessage,
    ResolutionStatus,
};
pub use parser::InteractionParser;
pub use runner::InteractiveSessionRunner;
pub use session::{ConversationTurn, Session, SessionState, SqliteSessionRepository};
pub use state::{SessionPromptState, StateTransitionError};
pub use user_commands::{
    BoundArguments, PromptCommand, PromptCommandHandler, UserCommandError, UserCommandLoadReport,
    UserCommandRejection, global_user_commands_dir, global_user_commands_dir_for_channel,
    load_global_user_commands, load_global_user_commands_for_channel,
    load_global_user_commands_from_dir, parse_user_command_toml,
};
