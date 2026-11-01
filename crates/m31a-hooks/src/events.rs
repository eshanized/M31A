//! Event taxonomy for engine lifecycle hook interception.
//!
//! 14 lifecycle events; each dispatch carries a JSON payload (tool name,
//! tool input, result, prompt, …) and may produce a disposition that the
//! engine applies at the corresponding seam.

use serde::{Deserialize, Serialize};

/// All hookable engine lifecycle events emitted during session execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HookEvent {
    /// Before a model request is sent.
    PreModelRequest,
    /// After a model response is received.
    PostModelRequest,
    /// Before a tool call is dispatched — may rewrite input or block.
    PreToolUse,
    /// When an approval is requested — may auto-approve/deny.
    PermissionRequest,
    /// After a tool result is produced — gates result acceptance.
    PostToolUse,
    /// Before context compaction begins.
    PreCompact,
    /// After context compaction completes.
    PostCompact,
    /// Engine/session started. Sources: Startup/Resume/Clear/Compact/Fork.
    SessionStart,
    /// Engine/session ending.
    SessionEnd,
    /// A user prompt was submitted — may block submission.
    UserPromptSubmit,
    /// A child agent was spawned.
    SubagentStart,
    /// A child agent finished.
    SubagentStop,
    /// The turn is about to end — may block continuation.
    Stop,
    /// Turn interrupted (cancel/steer).
    Interrupt,
}

impl HookEvent {
    /// Config key used in `hooks.json` (snake_case names).
    pub fn config_key(&self) -> &'static str {
        match self {
            HookEvent::PreModelRequest => "pre_model_request",
            HookEvent::PostModelRequest => "post_model_request",
            HookEvent::PreToolUse => "pre_tool_use",
            HookEvent::PermissionRequest => "permission_request",
            HookEvent::PostToolUse => "post_tool_use",
            HookEvent::PreCompact => "pre_compact",
            HookEvent::PostCompact => "post_compact",
            HookEvent::SessionStart => "session_start",
            HookEvent::SessionEnd => "session_end",
            HookEvent::UserPromptSubmit => "user_prompt_submit",
            HookEvent::SubagentStart => "subagent_start",
            HookEvent::SubagentStop => "subagent_stop",
            HookEvent::Stop => "stop",
            HookEvent::Interrupt => "interrupt",
        }
    }

    /// The payload field this event's subject is matched against (tool name
    /// for tool events). Matchers apply to this field.
    pub fn matcher_subject_field(&self) -> &'static str {
        match self {
            HookEvent::PreToolUse | HookEvent::PostToolUse | HookEvent::PermissionRequest => "tool",
            _ => "event",
        }
    }
}

/// Where a SessionStart hook fired from (upstream semantics).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionStartSource {
    Startup,
    Resume,
    Clear,
    Compact,
    Fork,
}

/// The engine's decision after applying hook outcomes at a seam.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HookDisposition {
    /// Proceed unchanged.
    Allow,
    /// Block the operation with a reason surfaced to the model/user.
    Block(String),
    /// Rewrite the tool input (PreToolUse only) with the new argument JSON.
    Rewrite { input: String },
}
