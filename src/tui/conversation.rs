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
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use serde::{Deserialize, Serialize};

use crate::interaction::session::ConversationTurn;
use crate::tui::sanitizer::sanitize_terminal_text;
use crate::tui::theme::{ThemeMode, ThemeTokens};

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
    /// Bracketed semantic badge for the item type.
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
                    timestamp: *created_at,
                }
            }
        }
    }

    /// Render item into formatted terminal lines honoring theme and NO_COLOR.
    pub fn render_lines(&self, width: u16, tokens: &ThemeTokens) -> Vec<Line<'static>> {
        let is_mono = tokens.mode == ThemeMode::MonochromeANSI || std::env::var("NO_COLOR").is_ok();
        let time_str = self.timestamp().format("%H:%M:%S").to_string();
        let time_span = Span::styled(
            format!("[{time_str}] "),
            if is_mono {
                Style::default()
            } else {
                Style::default().fg(Color::DarkGray)
            },
        );

        match self {
            Self::User {
                sequence,
                text,
                mentions,
                ..
            } => {
                let badge_style = if is_mono {
                    Style::default().add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD)
                };

                let mut header_spans = vec![
                    time_span,
                    Span::styled(format!("[USER #{sequence}] "), badge_style),
                ];

                for m in mentions {
                    let m_style = if is_mono {
                        Style::default().add_modifier(Modifier::UNDERLINED)
                    } else {
                        Style::default().fg(Color::Yellow)
                    };
                    header_spans.push(Span::styled(format!("@{m} "), m_style));
                }

                let mut lines = vec![Line::from(header_spans)];
                let sanitized = sanitize_terminal_text(text);
                for l in sanitized.lines() {
                    lines.push(Line::from(vec![
                        Span::raw("  "),
                        Span::styled(
                            l.to_string(),
                            if is_mono {
                                Style::default()
                            } else {
                                Style::default().fg(Color::White)
                            },
                        ),
                    ]));
                }
                lines
            }

            Self::Assistant { sequence, text, .. } => {
                let badge_style = if is_mono {
                    Style::default().add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD)
                };

                let mut lines = vec![Line::from(vec![
                    time_span,
                    Span::styled(format!("[ASST #{sequence}]"), badge_style),
                ])];

                let sanitized = sanitize_terminal_text(text);
                for l in sanitized.lines() {
                    lines.push(Line::from(vec![
                        Span::raw("  "),
                        Span::styled(
                            l.to_string(),
                            if is_mono {
                                Style::default()
                            } else {
                                Style::default().fg(Color::Gray)
                            },
                        ),
                    ]));
                }
                lines
            }

            Self::ToolActivity {
                tool_name,
                parameters,
                ..
            } => {
                let badge_style = if is_mono {
                    Style::default().add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::Yellow)
                };

                let param_preview = if parameters.len() > 80 {
                    format!("{}...", &parameters[..80])
                } else {
                    parameters.clone()
                };

                vec![Line::from(vec![
                    time_span,
                    Span::styled("[TOOL:START] ", badge_style),
                    Span::styled(
                        format!("{tool_name} "),
                        Style::default().add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        param_preview,
                        if is_mono {
                            Style::default()
                        } else {
                            Style::default().fg(Color::DarkGray)
                        },
                    ),
                ])]
            }

            Self::ToolResult {
                tool_name,
                success,
                output_preview,
                ..
            } => {
                let (badge, badge_style) = if *success {
                    (
                        "[TOOL:OK] ",
                        if is_mono {
                            Style::default()
                        } else {
                            Style::default().fg(Color::Green)
                        },
                    )
                } else {
                    (
                        "[TOOL:FAIL] ",
                        if is_mono {
                            Style::default().add_modifier(Modifier::BOLD)
                        } else {
                            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
                        },
                    )
                };

                let preview = sanitize_terminal_text(output_preview);
                let first_line = preview.lines().next().unwrap_or("");
                let bounded_preview = if first_line.len() > (width as usize).saturating_sub(25) {
                    let end = (width as usize).saturating_sub(28).max(10);
                    if end < first_line.len() {
                        format!("{}...", &first_line[..end])
                    } else {
                        first_line.to_string()
                    }
                } else {
                    first_line.to_string()
                };

                vec![Line::from(vec![
                    time_span,
                    Span::styled(badge, badge_style),
                    Span::raw(format!("{tool_name}: ")),
                    Span::styled(
                        bounded_preview,
                        if is_mono {
                            Style::default()
                        } else {
                            Style::default().fg(Color::DarkGray)
                        },
                    ),
                ])]
            }

            Self::Verification {
                passed, summary, ..
            } => {
                let (badge, style) = if *passed {
                    (
                        "[VERIFY:PASSED] ",
                        if is_mono {
                            Style::default().add_modifier(Modifier::BOLD)
                        } else {
                            Style::default()
                                .fg(Color::Green)
                                .add_modifier(Modifier::BOLD)
                        },
                    )
                } else {
                    (
                        "[VERIFY:FAILED] ",
                        if is_mono {
                            Style::default().add_modifier(Modifier::BOLD)
                        } else {
                            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
                        },
                    )
                };

                vec![Line::from(vec![
                    time_span,
                    Span::styled(badge, style),
                    Span::styled(
                        sanitize_terminal_text(summary),
                        Style::default().add_modifier(Modifier::BOLD),
                    ),
                ])]
            }

            Self::Approval {
                tool_name,
                details,
                decision,
                ..
            } => {
                let badge = if let Some(d) = decision {
                    format!("[APPROVAL:{d}] ")
                } else {
                    "[ASK:APPROVAL REQUIRED] ".to_string()
                };

                let style = if is_mono {
                    Style::default().add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                        .fg(Color::Magenta)
                        .add_modifier(Modifier::BOLD)
                };

                vec![Line::from(vec![
                    time_span,
                    Span::styled(badge, style),
                    Span::styled(
                        format!("{tool_name}: "),
                        Style::default().add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(sanitize_terminal_text(details)),
                ])]
            }

            Self::Discovery { questions, .. } => {
                let badge_style = if is_mono {
                    Style::default().add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD)
                };
                let mut lines = vec![Line::from(vec![
                    time_span,
                    Span::styled("[DISCOVERY REQUIRED] ", badge_style),
                    Span::raw(format!("{} question(s) need your input", questions.len())),
                ])];
                for (i, q) in questions.iter().enumerate() {
                    lines.push(Line::from(vec![
                        Span::raw(format!("  {}. ", i + 1)),
                        Span::raw(sanitize_terminal_text(q)),
                    ]));
                }
                lines.push(Line::from(vec![Span::styled(
                    "  [enter] answer   [tab] next   [esc] close",
                    Style::default().add_modifier(Modifier::DIM),
                )]));
                lines
            }

            Self::PlanReview {
                revision,
                plan_id,
                content_hash,
                objective,
                task_count,
                ..
            } => {
                let badge_style = if is_mono {
                    Style::default().add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD)
                };
                let mut lines = vec![Line::from(vec![
                    time_span,
                    Span::styled(
                        format!("[PLAN R{revision} — READY FOR REVIEW] "),
                        badge_style,
                    ),
                ])];
                lines.push(Line::raw(format!("  plan: {plan_id}")));
                lines.push(Line::raw(format!(
                    "  objective: {}",
                    sanitize_terminal_text(objective)
                )));
                lines.push(Line::raw(format!("  tasks: {task_count}")));
                if let Some(h) = content_hash {
                    let short: String = h.chars().take(8).collect();
                    lines.push(Line::raw(format!("  revision: {revision}  hash: {short}")));
                } else {
                    lines.push(Line::raw(format!("  revision: {revision}")));
                }
                lines.push(Line::from(vec![Span::styled(
                    "  [e] edit   [r] revise   [a] accept   [x] reject   [i] inspect",
                    Style::default().add_modifier(Modifier::DIM),
                )]));
                lines
            }

            Self::TaskReview {
                plan_revision,
                task_revision,
                content_hash,
                task_count,
                ..
            } => {
                let badge_style = if is_mono {
                    Style::default().add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD)
                };
                let mut lines = vec![Line::from(vec![
                    time_span,
                    Span::styled(
                        format!("[TASKS R{task_revision} — READY FOR REVIEW] "),
                        badge_style,
                    ),
                ])];
                lines.push(Line::raw(format!(
                    "  plan rev: {plan_revision}  tasks: {task_count}"
                )));
                if let Some(h) = content_hash {
                    let short: String = h.chars().take(8).collect();
                    lines.push(Line::raw(format!(
                        "  revision: {task_revision}  hash: {short}"
                    )));
                } else {
                    lines.push(Line::raw(format!("  revision: {task_revision}")));
                }
                lines.push(Line::from(vec![Span::styled(
                    "  [e] edit   [r] revise   [a] accept   [x] reject   [i] inspect graph",
                    Style::default().add_modifier(Modifier::DIM),
                )]));
                lines
            }

            Self::AuthRequired {
                plan_revision,
                task_revision,
                message,
                ..
            } => {
                let badge_style = if is_mono {
                    Style::default().add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
                };
                vec![
                    Line::from(vec![
                        time_span,
                        Span::styled("[EXECUTION AUTHORIZATION] ", badge_style),
                    ]),
                    Line::raw("  You are authorizing workspace modification."),
                    Line::raw(format!(
                        "  plan revision: {plan_revision}  task revision: {task_revision}"
                    )),
                    Line::raw(format!("  {}", sanitize_terminal_text(message))),
                    Line::raw("  This approval is bound to these exact artifacts."),
                    Line::from(vec![Span::styled(
                        "  [authorize]   [cancel]   [inspect approved snapshot]",
                        Style::default().add_modifier(Modifier::DIM),
                    )]),
                ]
            }

            Self::AuthGranted {
                authorization_id,
                plan_revision,
                task_revision,
                ..
            } => {
                let badge_style = if is_mono {
                    Style::default().add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD)
                };
                vec![
                    Line::from(vec![
                        time_span,
                        Span::styled("[AUTHORIZED — NOT YET EXECUTING] ", badge_style),
                    ]),
                    Line::raw(format!("  authorization: {authorization_id}")),
                    Line::raw(format!(
                        "  plan revision: {plan_revision}  task revision: {task_revision}"
                    )),
                    Line::raw("  Authorization does not mean execution has started."),
                ]
            }

            Self::Failure {
                context, reason, ..
            } => {
                let style = if is_mono {
                    Style::default().add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
                };
                vec![
                    Line::from(vec![
                        time_span,
                        Span::styled("[FAILED] ", style),
                        Span::styled(
                            sanitize_terminal_text(context),
                            Style::default().add_modifier(Modifier::BOLD),
                        ),
                    ]),
                    Line::raw(format!("  {}", sanitize_terminal_text(reason))),
                    Line::from(vec![Span::styled(
                        "  [inspect] [retry] [revise] — only where the runtime supports it",
                        Style::default().add_modifier(Modifier::DIM),
                    )]),
                ]
            }

            Self::Error { message, .. } => {
                let style = if is_mono {
                    Style::default().add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
                };

                vec![Line::from(vec![
                    time_span,
                    Span::styled("[ERR] ", style),
                    Span::styled(sanitize_terminal_text(message), style),
                ])]
            }

            Self::System { text, .. } => {
                let style = if is_mono {
                    Style::default()
                } else {
                    Style::default().fg(Color::Blue)
                };

                vec![Line::from(vec![
                    time_span,
                    Span::styled("[SYS] ", style),
                    Span::raw(sanitize_terminal_text(text)),
                ])]
            }

            Self::Recovery {
                action, details, ..
            } => {
                let badge_style = if is_mono {
                    Style::default().add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD)
                };

                let mut lines = vec![Line::from(vec![
                    time_span,
                    Span::styled("[RECOVERY] ", badge_style),
                    Span::styled(
                        format!("{action} "),
                        Style::default().add_modifier(Modifier::BOLD),
                    ),
                ])];
                let sanitized = sanitize_terminal_text(details);
                for l in sanitized.lines() {
                    lines.push(Line::from(vec![
                        Span::raw("    ↳ "),
                        Span::styled(
                            l.to_string(),
                            if is_mono {
                                Style::default()
                            } else {
                                Style::default().fg(Color::Yellow)
                            },
                        ),
                    ]));
                }
                lines
            }
        }
    }
}
