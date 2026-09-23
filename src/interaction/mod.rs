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
pub mod events;
pub mod mentions;
pub mod parser;
pub mod runner;
pub mod session;
pub mod state;

pub use action::{ApplicationAction, InteractionIntent};
pub use approval::{InteractiveApprovalChannel, PendingApprovalPrompt};
pub use commands::{
    CommandContext, CommandHandler, CommandOutput, CommandSideEffect, SlashCommand,
    SlashCommandRegistry,
};
pub use events::InteractionEvent;
pub use mentions::{
    LineRange, MentionKind, MentionParser, MentionReference, MessageSegment, ParsedUserMessage,
    ResolutionStatus,
};
pub use parser::InteractionParser;
pub use runner::InteractiveSessionRunner;
pub use session::{ConversationTurn, Session, SessionState, SqliteSessionRepository};
pub use state::{SessionPromptState, StateTransitionError};
