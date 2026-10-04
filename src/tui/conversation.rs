//! TUI Conversation Timeline Presentation Models (PRD §01, CLI-02, CLI-04).
//!
//! Provides strongly typed, human-readable presentation models for the
//! live interactive conversation stream:
//! - User prompts with @mention badges
//! - Assistant reasoning and model responses
//! - Real-time tool invocations and structured results
//! - Verification gate proofs and summaries
//! - Policy approval escalation prompts and resolutions
//! - System lifecycle events and error reporting
//!
//! Enforces:
//! - Semantic badges (`[USER]`, `[ASST]`, `[TOOL]`, `[VERIFY]`, `[ASK]`, `[ERR]`, `[SYS]`)
//! - Secret scrubbing and terminal escape sanitization
//! - Theme-aware rendering honoring `NO_COLOR`
//! - Non-color semantic status encoding

use chrono::{DateTime, Utc};
use ratatui::text::{Line, Span};
use serde::{Deserialize, Serialize};

use crate::interaction::session::ConversationTurn;
use crate::tui::icons::{IconKey, IconRegistry};
use crate::tui::sanitizer::sanitize_terminal_text;
use crate::tui::theme::ThemeTokens;

/// Individual presentation turn or runtime event in the conversation timeline.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TuiConversationItem {
    /// Natural language prompt submitted by the operator.
    User {
        id: String,
        sequence: u64,
        text: String,
        mentions: Vec<String>,
        timestamp: DateTime<Utc>,
    },

    /// Synthesized response or reasoning from the assistant model.
    Assistant {
        id: String,
        sequence: u64,
        text: String,
        #[serde(default)]
        streaming: bool,
        timestamp: DateTime<Utc>,
    },

    /// Autonomous tool execution started.
    ToolActivity {
        call_id: String,
        tool_name: String,
        parameters: String,
        timestamp: DateTime<Utc>,
    },

    /// Autonomous tool execution completed.
    ToolResult {
        call_id: String,
        tool_name: String,
        success: bool,
        output_preview: String,
        timestamp: DateTime<Utc>,
    },

    /// Independent verification gate evaluation result.
    Verification {
        sequence: u64,
        passed: bool,
        summary: String,
        timestamp: DateTime<Utc>,
    },

    /// Policy approval escalation prompt.
    Approval {
        request_id: String,
        tool_name: String,
        details: String,
        decision: Option<String>,
        timestamp: DateTime<Utc>,
    },

    /// Error encountered during execution.
    Error {
        message: String,
        timestamp: DateTime<Utc>,
    },

    /// System lifecycle milestone (session created, resumed, profile switched).
    System {
        text: String,
        timestamp: DateTime<Utc>,
    },

    /// Recovery, remediation, or worktree rollback action.
    Recovery {
        action: String,
        details: String,
        timestamp: DateTime<Utc>,
    },

    /// Governed discovery: runtime needs operator answers before planning.
    Discovery {
        questions: Vec<String>,
        timestamp: DateTime<Utc>,
    },

    /// Candidate plan revision awaiting explicit operator acceptance.
    PlanReview {
        revision: u32,
        plan_id: String,
        content_hash: Option<String>,
        objective: String,
        task_count: usize,
        timestamp: DateTime<Utc>,
    },

    /// Candidate task set awaiting explicit operator acceptance.
    TaskReview {
        plan_revision: u32,
        task_revision: u32,
        content_hash: Option<String>,
        task_count: usize,
        timestamp: DateTime<Utc>,
    },

    /// Explicit workspace-modification authorization requested.
    AuthRequired {
        plan_revision: u32,
        task_revision: u32,
        message: String,
        timestamp: DateTime<Utc>,
    },

    /// Execution authorized for exact revisions — not yet executing.
    AuthGranted {
        authorization_id: String,
        plan_revision: u32,
        task_revision: u32,
        timestamp: DateTime<Utc>,
    },

    /// Explicit failure with classified context (runtime/tool/verification/
    /// policy/authorization/persistence/lifecycle).
    Failure {
        context: String,
        reason: String,
        timestamp: DateTime<Utc>,
    },
}

impl TuiConversationItem {
    /// Bracketed semantic badge for the item type (legacy ASCII).
    pub fn badge(&self) -> &'static str {
        match self {
            Self::User { .. } => "[USER]",
            Self::Assistant { .. } => "[ASST]",
            Self::ToolActivity { .. } => "[TOOL:START]",
            Self::ToolResult { success, .. } => {
                if *success {
                    "[TOOL:OK]"
                } else {
                    "[TOOL:FAIL]"
                }
            }
            Self::Verification { passed, .. } => {
                if *passed {
                    "[VERIFY:PASSED]"
                } else {
                    "[VERIFY:FAILED]"
                }
            }
            Self::Approval { decision, .. } => {
                if decision.is_some() {
                    "[APPROVAL]"
                } else {
                    "[ASK]"
                }
            }
            Self::Error { .. } => "[ERR]",
            Self::System { .. } => "[SYS]",
            Self::Recovery { .. } => "[RECOVERY]",
            Self::Discovery { .. } => "[DISCOVERY]",
            Self::PlanReview { .. } => "[PLAN REVIEW]",
            Self::TaskReview { .. } => "[TASK REVIEW]",
            Self::AuthRequired { .. } => "[AUTH REQUIRED]",
            Self::AuthGranted { .. } => "[AUTHORIZED — NOT EXECUTING]",
            Self::Failure { .. } => "[FAILED]",
        }
    }

    /// Get the semantic icon key for this item type.
    pub fn icon_key(&self) -> IconKey {
        match self {
            Self::User { .. } => IconKey::User,
            Self::Assistant { .. } => IconKey::Assistant,
            Self::ToolActivity { .. } => IconKey::RunningTool,
            Self::ToolResult { success, .. } => {
                if *success {
                    IconKey::Success
                } else {
                    IconKey::Error
                }
            }
            Self::Verification { passed, .. } => {
                if *passed {
                    IconKey::Verification
                } else {
                    IconKey::Failed
                }
            }
            Self::Approval { decision, .. } => {
                if decision.is_some() {
                    IconKey::Approval
                } else {
                    IconKey::WaitingForApproval
                }
            }
            Self::Error { .. } => IconKey::Error,
            Self::System { .. } => IconKey::System,
            Self::Recovery { .. } => IconKey::Recovering,
            Self::Discovery { .. } => IconKey::Discovering,
            Self::PlanReview { .. } => IconKey::PlanReview,
            Self::TaskReview { .. } => IconKey::TaskReview,
            Self::AuthRequired { .. } => IconKey::AuthRequired,
            Self::AuthGranted { .. } => IconKey::AuthGranted,
            Self::Failure { .. } => IconKey::Failed,
        }
    }

    /// Render this conversation item as quiet styled lines.
    ///
    /// Conversation-first: role labels are subtle (`You`, `M31A`, `Tool`,
    /// `Verification`, …) with spacing and typography carrying hierarchy.
    /// Semantic badges (`badge()`) remain available for tests and details;
    /// the default stream does not shout them.
    pub fn render_lines_with_icons<'a>(
        &'a self,
        max_width: u16,
        tokens: &ThemeTokens,
        _icons: &IconRegistry,
    ) -> Vec<ratatui::text::Line<'a>> {
        let mut lines = Vec::new();
        let role = |name: &str| Line::from(Span::styled(name.to_string(), tokens.text_muted));
        let body = |t: &str| {
            sanitize_terminal_text(t)
                .lines()
                .map(|l| Line::from(Span::styled(l.to_string(), tokens.text_primary)))
                .collect::<Vec<_>>()
        };
        let secondary = |t: &str| {
            sanitize_terminal_text(t)
                .lines()
                .map(|l| Line::from(Span::styled(l.to_string(), tokens.text_secondary)))
                .collect::<Vec<_>>()
        };
        let meta = |t: String| Line::from(Span::styled(t, tokens.text_muted));

        match self {
            TuiConversationItem::User { text, mentions, .. } => {
                lines.push(role("You"));
                lines.extend(body(text));
                if !mentions.is_empty() {
                    lines.push(meta(format!(" @{}", mentions.join(" @"))));
                }
            }
            TuiConversationItem::Assistant {
                text, streaming, ..
            } => {
                lines.push(role("M31A"));
                lines.extend(body(text));
                if *streaming {
                    if let Some(last) = lines.last_mut() {
                        last.spans.push(Span::styled(" ▋", tokens.text_muted));
                    } else {
                        lines.push(meta("▋".to_string()));
                    }
                }
            }
            TuiConversationItem::ToolActivity {
                tool_name,
                parameters,
                ..
            } => {
                lines.push(role(&format!("Tool · {tool_name}")));
                if !parameters.is_empty() && parameters != "{}" {
                    let short = if parameters.len() > 120 {
                        format!("{}…", &parameters[..120])
                    } else {
                        parameters.clone()
                    };
                    lines.push(meta(format!("  {}", sanitize_terminal_text(&short))));
                }
            }
            TuiConversationItem::ToolResult {
                tool_name,
                success,
                output_preview,
                ..
            } => {
                lines.push(role(&format!("Tool · {tool_name}")));
                let bounded = crate::tui::component::code::render_bounded_output(
                    output_preview,
                    6,
                    max_width.saturating_sub(4),
                    None,
                    tokens,
                );
                if *success {
                    lines.push(Line::from(vec![
                        Span::styled("  ✓ ", tokens.success),
                        Span::styled("finished", tokens.text_secondary),
                    ]));
                } else {
                    lines.push(Line::from(vec![
                        Span::styled("  × ", tokens.error),
                        Span::styled("failed", tokens.error),
                    ]));
                }
                for l in bounded.into_iter().take(6) {
                    lines.push(l);
                }
            }
            TuiConversationItem::Verification {
                passed, summary, ..
            } => {
                lines.push(role("Verification"));
                if *passed {
                    lines.push(Line::from(vec![
                        Span::styled("  ✓ ", tokens.success),
                        Span::styled(sanitize_terminal_text(summary), tokens.text_secondary),
                    ]));
                } else {
                    lines.push(Line::from(vec![
                        Span::styled("  × ", tokens.error),
                        Span::styled(sanitize_terminal_text(summary), tokens.text_primary),
                    ]));
                    lines.push(meta("  Press Enter to inspect".to_string()));
                }
                if summary.is_empty() {
                    lines.push(meta(if *passed {
                        "  ✓ passed".to_string()
                    } else {
                        "  × failed".to_string()
                    }));
                }
            }
            TuiConversationItem::Approval {
                tool_name,
                details,
                decision,
                ..
            } => {
                lines.push(role("Approval"));
                match decision.as_deref() {
                    Some(d) if d.contains("Approve") => {
                        lines.push(Line::from(Span::styled(
                            format!("  ✓ {tool_name} approved"),
                            tokens.text_secondary,
                        )));
                    }
                    Some(_) => {
                        lines.push(Line::from(Span::styled(
                            format!("  × {tool_name} denied"),
                            tokens.text_primary,
                        )));
                    }
                    None => {
                        lines.push(Line::from(Span::styled(
                            format!("  ○ {tool_name} needs your approval"),
                            tokens.text_primary,
                        )));
                        if !details.is_empty() {
                            lines.extend(secondary(details));
                        }
                    }
                }
            }
            TuiConversationItem::Error { message, .. } => {
                lines.push(role("Failed"));
                lines.push(Line::from(Span::styled(
                    format!("  {}", sanitize_terminal_text(message)),
                    tokens.text_primary,
                )));
                lines.push(meta("  Press Enter to inspect".to_string()));
            }
            TuiConversationItem::System { text, .. } => {
                lines.push(meta(format!("  {}", sanitize_terminal_text(text))));
            }
            TuiConversationItem::Recovery {
                action, details, ..
            } => {
                lines.push(role("Recovery"));
                lines.push(Line::from(Span::styled(
                    format!("  {action}"),
                    tokens.text_secondary,
                )));
                if !details.is_empty() {
                    lines.push(meta(format!("  {}", sanitize_terminal_text(details))));
                }
            }
            TuiConversationItem::Discovery { questions, .. } => {
                lines.push(role("Input needed"));
                for q in questions {
                    lines.push(Line::from(Span::styled(
                        format!("  ? {}", sanitize_terminal_text(q)),
                        tokens.text_primary,
                    )));
                }
            }
            TuiConversationItem::PlanReview {
                revision,
                objective,
                task_count,
                content_hash,
                ..
            } => {
                let hash_suffix = content_hash
                    .as_ref()
                    .map(|h| format!(" ({})", h.chars().take(8).collect::<String>()))
                    .unwrap_or_default();
                lines.push(role(&format!(
                    "Plan · revision {revision}{hash_suffix} ready for review"
                )));
                if !objective.is_empty() {
                    lines.extend(secondary(objective));
                }
                lines.push(meta(format!(
                    "  {task_count} tasks · /plan accept · /plan revise"
                )));
            }
            TuiConversationItem::TaskReview {
                task_revision,
                task_count,
                content_hash,
                ..
            } => {
                let hash_suffix = content_hash
                    .as_ref()
                    .map(|h| format!(" ({})", h.chars().take(8).collect::<String>()))
                    .unwrap_or_default();
                lines.push(role(&format!(
                    "Tasks · revision {task_revision}{hash_suffix} ready for review"
                )));
                lines.push(meta(format!(
                    "  {task_count} tasks · /tasks accept · /tasks regen"
                )));
            }
            TuiConversationItem::AuthRequired { message, .. } => {
                lines.push(role("Authorization needed"));
                lines.push(Line::from(Span::styled(
                    sanitize_terminal_text(message),
                    tokens.text_primary,
                )));
                lines.push(meta("  /authorize yes · /authorize no".to_string()));
            }
            TuiConversationItem::AuthGranted {
                authorization_id, ..
            } => {
                lines.push(role("Authorized"));
                lines.push(meta(format!("  {authorization_id} · not yet executing")));
            }
            TuiConversationItem::Failure {
                context, reason, ..
            } => {
                lines.push(role("Failed"));
                lines.push(Line::from(Span::styled(
                    format!("  {context}"),
                    tokens.text_primary,
                )));
                lines.push(Line::from(Span::styled(
                    format!("  {}", sanitize_terminal_text(reason)),
                    tokens.text_secondary,
                )));
                lines.push(meta("  Press Enter to inspect".to_string()));
            }
        }
        lines
    }

    /// Timestamp of this conversation item.
    pub fn timestamp(&self) -> DateTime<Utc> {
        match self {
            Self::User { timestamp, .. } => *timestamp,
            Self::Assistant { timestamp, .. } => *timestamp,
            Self::ToolActivity { timestamp, .. } => *timestamp,
            Self::ToolResult { timestamp, .. } => *timestamp,
            Self::Verification { timestamp, .. } => *timestamp,
            Self::Approval { timestamp, .. } => *timestamp,
            Self::Error { timestamp, .. } => *timestamp,
            Self::System { timestamp, .. } => *timestamp,
            Self::Recovery { timestamp, .. } => *timestamp,
            Self::Discovery { timestamp, .. } => *timestamp,
            Self::PlanReview { timestamp, .. } => *timestamp,
            Self::TaskReview { timestamp, .. } => *timestamp,
            Self::AuthRequired { timestamp, .. } => *timestamp,
            Self::AuthGranted { timestamp, .. } => *timestamp,
            Self::Failure { timestamp, .. } => *timestamp,
        }
    }

    /// Convert a persisted `ConversationTurn` into a presentation item.
    pub fn from_turn(turn: &ConversationTurn) -> Self {
        match turn {
            ConversationTurn::UserMessage {
                id,
                sequence,
                content,
                mentions,
                created_at,
                ..
            } => {
                let mention_strs = mentions.iter().map(|m| m.raw_path.clone()).collect();
                Self::User {
                    id: id.to_string(),
                    sequence: *sequence,
                    text: content.clone(),
                    mentions: mention_strs,
                    timestamp: *created_at,
                }
            }
            ConversationTurn::AssistantMessage {
                id,
                sequence,
                content,
                created_at,
            } => Self::Assistant {
                id: id.to_string(),
                sequence: *sequence,
                text: content.clone(),
                streaming: false,
                timestamp: *created_at,
            },
            ConversationTurn::VerificationMessage {
                sequence,
                passed,
                summary,
                created_at,
                ..
            } => Self::Verification {
                sequence: *sequence,
                passed: *passed,
                summary: summary.clone(),
                timestamp: *created_at,
            },
            ConversationTurn::ToolCallMessage {
                call_id,
                tool_name,
                arguments,
                created_at,
                ..
            } => Self::ToolActivity {
                call_id: call_id.clone(),
                tool_name: tool_name.clone(),
                parameters: arguments.to_string(),
                timestamp: *created_at,
            },
            ConversationTurn::ToolResultMessage {
                call_id,
                tool_name,
                output,
                success,
                created_at,
                ..
            } => Self::ToolResult {
                call_id: call_id.clone(),
                tool_name: tool_name.clone(),
                success: *success,
                output_preview: output.clone(),
                timestamp: *created_at,
            },
            ConversationTurn::SystemMessage {
                content,
                created_at,
                ..
            } => Self::System {
                text: content.clone(),
                timestamp: *created_at,
            },
            ConversationTurn::ApprovalMessage {
                request_id,
                prompt,
                decision,
                created_at,
                ..
            } => Self::Approval {
                request_id: request_id.clone(),
                tool_name: "".to_string(),
                details: prompt.clone(),
                decision: decision.clone(),
                timestamp: *created_at,
            },
            ConversationTurn::AskUserMessage {
                id,
                sequence,
                question,
                options,
                answer,
                created_at,
            } => {
                let opts_text = options
                    .iter()
                    .map(|o| format!("  [{}] {}", o.id, o.label))
                    .collect::<Vec<_>>()
                    .join("\n");
                let text = match answer {
                    Some(ans) => format!("Q: {}\n{}\nAnswer: {}", question, opts_text, ans),
                    None => format!("Q: {}\n{}", question, opts_text),
                };
                Self::Assistant {
                    id: id.to_string(),
                    sequence: *sequence,
                    text,
                    streaming: false,
                    timestamp: *created_at,
                }
            }
        }
    }

    /// Render item into quiet terminal lines honoring theme and NO_COLOR.
    /// Delegates to the conversation-first renderer; width only bounds
    /// tool previews.
    pub fn render_lines(&self, width: u16, tokens: &ThemeTokens) -> Vec<Line<'static>> {
        let icons = IconRegistry::new(crate::tui::icons::IconMode::Ascii);
        let owned: Vec<Line<'static>> = self
            .render_lines_with_icons(width, tokens, &icons)
            .into_iter()
            .map(|l| {
                let spans = l
                    .spans
                    .into_iter()
                    .map(|s| Span::styled(s.content.to_string(), s.style))
                    .collect::<Vec<_>>();
                Line::from(spans)
            })
            .collect();
        owned
    }
}
