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
        #[serde(default)]
        expanded: bool,
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
        #[serde(default)]
        structured_questions: Vec<crate::workflow::genesis::DynamicQuestion>,
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
        let width = (max_width as usize).clamp(20, 220);
        let role = |name: &str| Line::from(Span::styled(name.to_string(), tokens.text_muted));
        // Width-aware prose: every content line is wrapped to `width` so no
        // unbounded string can determine widget geometry or escape its region.
        let body = |t: &str| -> Vec<Line<'_>> {
            wrap_conversation_text(&sanitize_terminal_text(t), width, tokens.text_primary)
        };
        let secondary = |t: &str| -> Vec<Line<'_>> {
            wrap_conversation_text(&sanitize_terminal_text(t), width, tokens.text_secondary)
        };
        let meta = |t: String| -> Vec<Line<'_>> {
            wrap_conversation_text(&sanitize_terminal_text(&t), width, tokens.text_muted)
        };
        let meta_line = |t: String| -> Line<'_> {
            Line::from(Span::styled(
                truncate_to_width(&sanitize_terminal_text(&t), width),
                tokens.text_muted,
            ))
        };

        match self {
            TuiConversationItem::User { text, mentions, .. } => {
                lines.push(role("You"));
                lines.push(Line::raw(""));
                lines.extend(body(text));
                if !mentions.is_empty() {
                    lines.extend(meta(format!(" @{}", mentions.join(" @"))));
                }
            }
            TuiConversationItem::Assistant {
                text, streaming, ..
            } => {
                lines.push(role("M31A"));
                lines.push(Line::raw(""));
                // Command output that lands in an Assistant/System bubble is
                // rendered as structured wrapped prose, not one long line.
                lines.extend(render_command_output_text(text, width, tokens));
                if *streaming {
                    if let Some(last) = lines.last_mut() {
                        last.spans.push(Span::styled(" ▋", tokens.text_muted));
                    } else {
                        lines.push(meta_line("▋".to_string()));
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
                    let short = truncate_to_width(&sanitize_terminal_text(parameters), 120);
                    lines.extend(meta(format!("  {short}")));
                }
            }
            TuiConversationItem::ToolResult {
                tool_name,
                success,
                output_preview,
                expanded,
                ..
            } => {
                lines.push(role(&format!("Tool · {tool_name}")));
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
                let total_preview_lines = output_preview.lines().count();
                let limit = if *expanded { 30 } else { 4 };
                let bounded = crate::tui::component::code::render_bounded_output(
                    output_preview,
                    limit,
                    max_width.saturating_sub(4),
                    None,
                    tokens,
                );
                for l in bounded {
                    lines.push(l);
                }
                if !*expanded && (total_preview_lines > 4 || output_preview.len() > 200) {
                    lines.push(Line::from(Span::styled(
                        "  ... (Enter to expand)",
                        tokens.text_muted,
                    )));
                } else if *expanded && total_preview_lines > 4 {
                    lines.push(Line::from(Span::styled(
                        "  ... (Enter to collapse)",
                        tokens.text_muted,
                    )));
                }
            }
            TuiConversationItem::Verification {
                passed, summary, ..
            } => {
                lines.push(role("Verification"));
                if summary.is_empty() {
                    lines.push(meta_line(if *passed {
                        "  ✓ passed".to_string()
                    } else {
                        "  × failed".to_string()
                    }));
                } else if *passed {
                    lines.push(Line::from(vec![
                        Span::styled("  ✓ ", tokens.success),
                        Span::styled(
                            truncate_to_width(
                                &sanitize_terminal_text(summary),
                                width.saturating_sub(4),
                            ),
                            tokens.text_secondary,
                        ),
                    ]));
                    // Long summaries wrap on continuation lines.
                    for extra in wrap_conversation_text(
                        &sanitize_terminal_text(summary),
                        width.saturating_sub(4),
                        tokens.text_secondary,
                    )
                    .into_iter()
                    .skip(1)
                    {
                        lines.push(extra);
                    }
                } else {
                    lines.extend(wrap_prefixed(
                        "  × ",
                        &sanitize_terminal_text(summary),
                        width,
                        tokens.text_primary,
                    ));
                    lines.push(meta_line("  Press Enter to inspect".to_string()));
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
                        lines.extend(meta(format!("  ✓ {tool_name} approved")));
                    }
                    Some(_) => {
                        lines.extend(meta(format!("  × {tool_name} denied")));
                    }
                    None => {
                        lines.extend(meta(format!("  ○ {tool_name} needs your approval")));
                        if !details.is_empty() {
                            lines.extend(secondary(details));
                        }
                    }
                }
            }
            TuiConversationItem::Error { message, .. } => {
                lines.push(role("Failed"));
                lines.extend(wrap_prefixed(
                    "  ",
                    &sanitize_terminal_text(message),
                    width,
                    tokens.text_primary,
                ));
                lines.push(meta_line("  Press Enter to inspect".to_string()));
            }
            TuiConversationItem::System { text, .. } => {
                // Structured command output: paragraphs, fields, lists.
                // Never one giant merged line.
                lines.push(role("M31A"));
                lines.push(Line::raw(""));
                lines.extend(render_command_output_text(text, width, tokens));
            }
            TuiConversationItem::Recovery {
                action, details, ..
            } => {
                lines.push(role("Recovery"));
                lines.extend(meta(format!("  {action}")));
                if !details.is_empty() {
                    lines.extend(meta(format!("  {}", sanitize_terminal_text(details))));
                }
            }
            TuiConversationItem::Discovery {
                questions,
                structured_questions,
                ..
            } => {
                lines.push(role("Input needed"));
                if !structured_questions.is_empty() {
                    for q in structured_questions {
                        lines.extend(wrap_prefixed(
                            &format!("  ? [{}] ", q.question_id),
                            &sanitize_terminal_text(&q.text),
                            width,
                            tokens.text_primary,
                        ));
                        for (idx, opt) in q.options.iter().enumerate() {
                            lines.extend(meta(format!("      {}) {}", idx + 1, opt)));
                        }
                        if q.allow_freeform {
                            lines.extend(meta("      (free-form answer allowed)".to_string()));
                        } else {
                            lines.extend(meta(
                                "      (select one of the above options)".to_string(),
                            ));
                        }
                    }
                } else {
                    for q in questions {
                        lines.extend(wrap_prefixed(
                            "  ? ",
                            &sanitize_terminal_text(q),
                            width,
                            tokens.text_primary,
                        ));
                    }
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
                lines.extend(meta(format!(
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
                lines.extend(meta(format!(
                    "  {task_count} tasks · /tasks accept · /tasks regen"
                )));
            }
            TuiConversationItem::AuthRequired { message, .. } => {
                lines.push(role("Authorization needed"));
                lines.extend(body(message));
                lines.extend(meta("  /authorize yes · /authorize no".to_string()));
            }
            TuiConversationItem::AuthGranted {
                authorization_id, ..
            } => {
                lines.push(role("Authorized"));
                lines.extend(meta(format!("  {authorization_id} · not yet executing")));
            }
            TuiConversationItem::Failure {
                context, reason, ..
            } => {
                lines.push(role("Failed"));
                lines.extend(wrap_prefixed(
                    "  ",
                    &sanitize_terminal_text(context),
                    width,
                    tokens.text_primary,
                ));
                lines.extend(wrap_prefixed(
                    "  ",
                    &sanitize_terminal_text(reason),
                    width,
                    tokens.text_secondary,
                ));
                lines.push(meta_line("  Press Enter to inspect".to_string()));
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
                expanded: false,
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

    /// Width available for a content line inside the conversation surface.
    pub fn content_width(area_width: u16) -> usize {
        (area_width.saturating_sub(4) as usize).clamp(20, 220)
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

/// Unicode-safe truncation to `width` chars.
fn truncate_to_width(s: &str, width: usize) -> String {
    let width = width.max(1);
    if s.chars().count() <= width {
        return s.to_string();
    }
    if width == 1 {
        return "…".to_string();
    }
    let kept: String = s.chars().take(width.saturating_sub(1)).collect();
    format!("{kept}…")
}

/// Wrap `text` to `width`, preserving blank-line paragraph breaks.
fn wrap_conversation_text(
    text: &str,
    width: usize,
    style: ratatui::style::Style,
) -> Vec<Line<'static>> {
    let width = width.clamp(20, 220);
    let mut out = Vec::new();
    for paragraph in text.split('\n') {
        if paragraph.trim().is_empty() {
            out.push(Line::raw(""));
            continue;
        }
        let mut current = String::new();
        let mut current_len = 0usize;
        for word in paragraph.split_whitespace() {
            let wlen = word.chars().count();
            if wlen >= width {
                if !current.is_empty() {
                    let taken = std::mem::take(&mut current);
                    out.push(Line::from(Span::styled(taken, style)));
                    current_len = 0;
                }
                let chars: Vec<char> = word.chars().collect();
                for chunk in chars.chunks(width.max(1)) {
                    let s: String = chunk.iter().collect();
                    out.push(Line::from(Span::styled(s, style)));
                }
                continue;
            }
            if current.is_empty() {
                current.push_str(word);
                current_len = wlen;
            } else if current_len + 1 + wlen <= width {
                current.push(' ');
                current.push_str(word);
                current_len += 1 + wlen;
            } else {
                let taken = std::mem::take(&mut current);
                out.push(Line::from(Span::styled(taken, style)));
                current.push_str(word);
                current_len = wlen;
            }
        }
        if !current.is_empty() {
            out.push(Line::from(Span::styled(current, style)));
        }
    }
    if out.is_empty() {
        out.push(Line::raw(""));
    }
    out
}

/// Wrap `text` with a fixed `prefix` on the first line and aligned
/// continuation indent on wrapped lines.
fn wrap_prefixed(
    prefix: &str,
    text: &str,
    width: usize,
    style: ratatui::style::Style,
) -> Vec<Line<'static>> {
    let width = width.clamp(20, 220);
    let indent: String = " ".repeat(prefix.chars().count());
    let inner = width.saturating_sub(prefix.chars().count()).max(10);
    let mut out = Vec::new();
    let mut first = true;
    for paragraph in text.split('\n') {
        if paragraph.trim().is_empty() {
            out.push(Line::raw(""));
            first = true;
            continue;
        }
        let mut current = String::new();
        let mut current_len = 0usize;
        for word in paragraph.split_whitespace() {
            let wlen = word.chars().count();
            if wlen >= inner {
                if !current.is_empty() {
                    let pre: &str = if first { prefix } else { &indent };
                    let mut line = String::with_capacity(pre.len() + current.len());
                    line.push_str(pre);
                    line.push_str(&current);
                    out.push(Line::from(Span::styled(line, style)));
                    current.clear();
                    current_len = 0;
                    first = false;
                }
                let chars: Vec<char> = word.chars().collect();
                for chunk in chars.chunks(inner.max(1)) {
                    let s: String = chunk.iter().collect();
                    let pre: &str = if first { prefix } else { &indent };
                    let mut line = String::with_capacity(pre.len() + s.len());
                    line.push_str(pre);
                    line.push_str(&s);
                    out.push(Line::from(Span::styled(line, style)));
                    first = false;
                }
                continue;
            }
            if current.is_empty() {
                current.push_str(word);
                current_len = wlen;
            } else if current_len + 1 + wlen <= inner {
                current.push(' ');
                current.push_str(word);
                current_len += 1 + wlen;
            } else {
                let pre: &str = if first { prefix } else { &indent };
                let mut line = String::with_capacity(pre.len() + current.len());
                line.push_str(pre);
                line.push_str(&current);
                out.push(Line::from(Span::styled(line, style)));
                current.clear();
                first = false;
                current.push_str(word);
                current_len = wlen;
            }
        }
        if !current.is_empty() {
            let pre: &str = if first { prefix } else { &indent };
            let mut line = String::with_capacity(pre.len() + current.len());
            line.push_str(pre);
            line.push_str(&current);
            out.push(Line::from(Span::styled(line, style)));
        }
    }
    if out.is_empty() {
        out.push(Line::from(Span::styled(prefix.to_string(), style)));
    }
    out
}

/// Render raw command-output text as structured terminal UI.
///
/// The command layer already emits stacked `Label\\n  value` blocks; this
/// renderer preserves those paragraph breaks and wraps every physical line
/// to `width` so long values stay readable and never escape their region.
fn render_command_output_text(
    text: &str,
    width: usize,
    tokens: &ThemeTokens,
) -> Vec<Line<'static>> {
    let width = width.clamp(20, 220);
    let sanitized = sanitize_terminal_text(text);
    let mut out = Vec::new();
    for paragraph in sanitized.split('\n') {
        if paragraph.trim().is_empty() {
            out.push(Line::raw(""));
            continue;
        }
        // Preserve intentional indentation (field values are `  …`).
        let indent_len = paragraph.chars().take_while(|c| *c == ' ').count().min(6);
        let (indent, content): (String, &str) = {
            let idx = paragraph
                .char_indices()
                .nth(indent_len)
                .map(|(i, _)| i)
                .unwrap_or(paragraph.len());
            (paragraph[..idx].to_string(), &paragraph[idx..])
        };
        if content.trim().is_empty() {
            out.push(Line::raw(""));
            continue;
        }
        let inner = width.saturating_sub(indent.chars().count()).max(10);
        let mut current = String::new();
        let mut current_len = 0usize;
        for word in content.split_whitespace() {
            let wlen = word.chars().count();
            // Headings (no indent) get primary emphasis; values stay
            // secondary so the stream keeps its quiet hierarchy.
            let style = if indent.is_empty() {
                tokens.text_primary
            } else {
                tokens.text_secondary
            };
            if wlen >= inner {
                if !current.is_empty() {
                    let mut line = String::with_capacity(indent.len() + current.len());
                    line.push_str(&indent);
                    line.push_str(&current);
                    out.push(Line::from(Span::styled(line, style)));
                    current.clear();
                    current_len = 0;
                }
                let chars: Vec<char> = word.chars().collect();
                for chunk in chars.chunks(inner.max(1)) {
                    let s: String = chunk.iter().collect();
                    let mut line = String::with_capacity(indent.len() + s.len());
                    line.push_str(&indent);
                    line.push_str(&s);
                    out.push(Line::from(Span::styled(line, style)));
                }
                continue;
            }
            if current.is_empty() {
                current.push_str(word);
                current_len = wlen;
            } else if current_len + 1 + wlen <= inner {
                current.push(' ');
                current.push_str(word);
                current_len += 1 + wlen;
            } else {
                let mut line = String::with_capacity(indent.len() + current.len());
                line.push_str(&indent);
                line.push_str(&current);
                out.push(Line::from(Span::styled(line, style)));
                current.clear();
                current.push_str(word);
                current_len = wlen;
            }
        }
        if !current.is_empty() {
            let style = if indent.is_empty() {
                tokens.text_primary
            } else {
                tokens.text_secondary
            };
            let mut line = String::with_capacity(indent.len() + current.len());
            line.push_str(&indent);
            line.push_str(&current);
            out.push(Line::from(Span::styled(line, style)));
        }
    }
    if out.is_empty() {
        out.push(Line::raw("(empty output)"));
    }
    out
}
