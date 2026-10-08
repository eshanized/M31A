//! Contextual Right Rail for Active Session Workspace (MODE B).
//!
//! Compact, secondary status information displayed alongside the primary conversation:
//! - SESSION: active/working/waiting/ready state & duration
//! - ACTION: pending approval prompt if any
//! - CURRENT: live working activity
//! - CONTEXT: token count & cost
//! - TODO: task progress checklist
//! - AGENTS: active agents & roles
//! - TOOLS: current or recent tool execution
//! - GIT: branch & working copy status
//! - VERIFICATION: test/check summary

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::tui::model::{TuiViewModel, UiOperationState};
use crate::tui::theme::ThemeTokens;

/// Render the contextual right rail.
pub fn render_context_rail(
    f: &mut Frame,
    area: Rect,
    model: &TuiViewModel,
    tokens: &ThemeTokens,
    _is_focused: bool,
    scroll_offset: usize,
) {
    if area.width < 15 || area.height < 4 {
        return;
    }

    let mut lines: Vec<Line> = Vec::new();
    let max_w = (area.width as usize).saturating_sub(2);

    // Section 1: SESSION
    lines.push(section_header("SESSION", tokens));
    let op_state = model.operation_state();
    let (dot, label, st) = match op_state {
        UiOperationState::Failed => ("×", "Failed".to_string(), tokens.error),
        UiOperationState::Cancelled => ("○", "Cancelled".to_string(), tokens.text_muted),
        UiOperationState::WaitingForUser | UiOperationState::AwaitingInput => {
            ("○", "Waiting for input".to_string(), tokens.warning)
        }
        UiOperationState::WaitingForApproval | UiOperationState::AwaitingApproval => {
            ("!", "Waiting for approval".to_string(), tokens.warning)
        }
        UiOperationState::Thinking => (
            "•",
            model
                .thought_duration_label()
                .unwrap_or_else(|| "Thinking".to_string()),
            tokens.text_secondary,
        ),
        UiOperationState::Planning => ("•", "Planning".to_string(), tokens.text_secondary),
        UiOperationState::Executing => ("•", "Executing".to_string(), tokens.text_secondary),
        UiOperationState::RunningTool => ("•", "Running tool".to_string(), tokens.text_secondary),
        UiOperationState::Verifying => ("•", "Verifying".to_string(), tokens.text_secondary),
        UiOperationState::Recovering => ("•", "Recovering".to_string(), tokens.warning),
        UiOperationState::CommandRunning { .. } => {
            ("•", "Running command".to_string(), tokens.text_secondary)
        }
        UiOperationState::Completed => ("✓", "Ready".to_string(), tokens.text_muted),
        UiOperationState::Idle => ("○", "Ready".to_string(), tokens.text_muted),
    };
    lines.push(Line::from(vec![
        Span::styled(format!("  {dot} "), st),
        Span::styled(truncate_str(&label, max_w.saturating_sub(4)), st),
    ]));
    lines.push(Line::raw(""));

    // Section 2: ACTION (only if pending approval)
    if !model.approvals.is_empty() {
        lines.push(section_header("ACTION REQUIRED", tokens));
        if let Some(req) = model.approvals.first() {
            lines.push(Line::from(vec![
                Span::styled("  ! ", tokens.warning),
                Span::styled(
                    truncate_str(
                        &format!("Approve: {}", req.tool_name),
                        max_w.saturating_sub(4),
                    ),
                    tokens.warning,
                ),
            ]));
            if !req.risk_tier.is_empty() {
                lines.push(Line::from(vec![
                    Span::styled("    Risk: ", tokens.text_muted),
                    Span::styled(
                        truncate_str(&req.risk_tier, max_w.saturating_sub(10)),
                        tokens.error,
                    ),
                ]));
            }
        }
        lines.push(Line::raw(""));
    }

    // Section 3: CURRENT ACTIVITY (if in progress)
    if let Some(ref activity) = model.live_activity {
        lines.push(section_header("CURRENT", tokens));
        lines.push(Line::from(vec![
            Span::styled("  • ", tokens.text_secondary),
            Span::styled(
                truncate_str(activity, max_w.saturating_sub(4)),
                tokens.text_secondary,
            ),
        ]));
        lines.push(Line::raw(""));
    }

    // Section 4: CONTEXT & TOKENS (only when usage exists)
    let total_tokens = model.model_usage.effective_total_tokens();
    if total_tokens > 0 {
        lines.push(section_header("CONTEXT", tokens));
        let token_label = if total_tokens >= 1_000_000 {
            format!("{:.1}M", total_tokens as f64 / 1_000_000.0)
        } else if total_tokens >= 1_000 {
            format!("{:.1}k", total_tokens as f64 / 1_000.0)
        } else {
            format!("{total_tokens}")
        };
        let cost_label = match model.model_usage.total_cost_cents {
            Some(c) if c > 0 => format!(" (${:.2})", c as f64 / 100.0),
            Some(_) if total_tokens == 0 => " ($0.00)".to_string(),
            Some(_) => " (unpriced)".to_string(),
            None if total_tokens > 0 => " (unpriced)".to_string(),
            None => String::new(),
        };
        lines.push(Line::from(vec![
            Span::styled("  Tokens: ", tokens.text_muted),
            Span::styled(format!("{token_label}{cost_label}"), tokens.text_secondary),
        ]));
        lines.push(Line::raw(""));
    }

    // Section 5: TODO / TASKS (if any)
    if !model.tasks.is_empty() {
        let total_tasks = model.tasks.len();
        let completed_tasks = model
            .tasks
            .iter()
            .filter(|t| t.status == "succeeded" || t.status == "completed" || t.progress_pct == 100)
            .count();
        lines.push(section_header(
            format!("TODO ({completed_tasks}/{total_tasks})"),
            tokens,
        ));
        for task in model.tasks.iter().take(6) {
            let (marker, marker_st) = match task.status.to_lowercase().as_str() {
                "succeeded" | "completed" => ("✓", tokens.success),
                "running" | "in_progress" => ("•", tokens.accent),
                "failed" => ("×", tokens.error),
                _ => ("○", tokens.text_muted),
            };
            lines.push(Line::from(vec![
                Span::styled(format!("  {marker} "), marker_st),
                Span::styled(
                    truncate_str(&task.title, max_w.saturating_sub(4)),
                    tokens.text_secondary,
                ),
            ]));
        }
        if total_tasks > 6 {
            lines.push(Line::from(Span::styled(
                format!("  … +{} more", total_tasks - 6),
                tokens.text_muted,
            )));
        }
        lines.push(Line::raw(""));
    }

    // Section 6: AGENTS (if any)
    if !model.agents.is_empty() {
        lines.push(section_header(
            format!("AGENTS ({})", model.agents.len()),
            tokens,
        ));
        for agent in model.agents.iter().take(4) {
            let st = match agent.state.to_lowercase().as_str() {
                "running" | "active" => tokens.accent,
                "failed" => tokens.error,
                _ => tokens.text_muted,
            };
            lines.push(Line::from(vec![
                Span::styled("  • ", st),
                Span::styled(
                    truncate_str(&agent.role, max_w.saturating_sub(12)),
                    tokens.text_secondary,
                ),
                Span::styled(format!(" · {}", agent.state), tokens.text_muted),
            ]));
        }
        lines.push(Line::raw(""));
    }

    // Section 7: LIVE TOOLS (if any)
    if !model.live_tools.is_empty() {
        lines.push(section_header(
            format!("TOOLS ({})", model.live_tools.len()),
            tokens,
        ));
        for tool in model.live_tools.iter().take(3) {
            lines.push(Line::from(vec![
                Span::styled("  • ", tokens.accent),
                Span::styled(
                    truncate_str(&tool.tool_name, max_w.saturating_sub(4)),
                    tokens.text_secondary,
                ),
            ]));
        }
        lines.push(Line::raw(""));
    }

    // Section 8: GIT
    let branch = if model.git_branch.is_empty() || model.git_branch == "N/A" {
        "master"
    } else {
        &model.git_branch
    };
    lines.push(section_header("GIT", tokens));
    lines.push(Line::from(vec![
        Span::styled("  Branch: ", tokens.text_muted),
        Span::styled(
            truncate_str(branch, max_w.saturating_sub(10)),
            tokens.text_secondary,
        ),
    ]));
    lines.push(Line::raw(""));

    // Section 9: VERIFICATION (if summary exists)
    if model.verification_summary.total_checks > 0 {
        lines.push(section_header("VERIFICATION", tokens));
        let pass = model.verification_summary.passed_count;
        let tot = model.verification_summary.total_checks;
        let v_st = if model.verification_summary.failed_count > 0 {
            tokens.error
        } else if pass == tot {
            tokens.success
        } else {
            tokens.text_secondary
        };
        lines.push(Line::from(vec![
            Span::styled("  Passed: ", tokens.text_muted),
            Span::styled(format!("{pass}/{tot}"), v_st),
        ]));
    }

    // Apply scroll offset if content exceeds area height
    let visible_lines: Vec<Line> = lines
        .into_iter()
        .skip(scroll_offset)
        .take(area.height as usize)
        .collect();

    let block = Block::default()
        .borders(Borders::LEFT)
        .border_style(tokens.separator);

    f.render_widget(Paragraph::new(visible_lines).block(block), area);
}

fn section_header(title: impl Into<String>, tokens: &ThemeTokens) -> Line<'static> {
    Line::from(vec![
        Span::styled(" ", tokens.text_muted),
        Span::styled(title.into(), tokens.text_muted),
    ])
}

fn truncate_str(s: &str, max: usize) -> String {
    if max == 0 {
        return String::new();
    }
    match s.char_indices().nth(max) {
        None => s.to_string(),
        Some((idx, _)) => s[..idx].to_string(),
    }
}
