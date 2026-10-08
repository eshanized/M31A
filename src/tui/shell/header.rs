//! Quiet application shell header (conversation-first).
//!
//! Shows only high-value persistent information:
//! `M31A · model · branch · state`
//! Telemetry (tokens, cost, event counts, ids) lives in details/palette
//! surfaces — never in the default header.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::tui::model::TuiViewModel;
use crate::tui::navigation::ScreenId;
use crate::tui::replay::ReplayController;
use crate::tui::theme::ThemeTokens;

/// Render the quiet persistent application shell header.
pub fn render_header(
    f: &mut Frame,
    area: Rect,
    model: &TuiViewModel,
    screen: ScreenId,
    replay: &ReplayController,
    tokens: &ThemeTokens,
) {
    if area.height == 0 || area.width == 0 {
        return;
    }
    let is_mono = tokens.is_mono();

    if replay.is_active {
        let text = format!(
            " Replay · {}/{} · [ / ] step · Space play/pause · Esc exit",
            replay.cursor + 1,
            replay.history.len().max(1),
        );
        let p = Paragraph::new(Line::from(Span::styled(
            text,
            if is_mono {
                Style::default()
            } else {
                tokens.warning
            },
        )))
        .block(
            Block::default()
                .borders(Borders::BOTTOM)
                .border_style(tokens.separator),
        );
        f.render_widget(p, area);
        return;
    }

    // Semantic state — never color-only (symbol + word).
    let (marker, state_label, state_style) = header_state(model, tokens);

    let model_name = short_model_name(&model.active_model);
    let branch = short_branch(model);
    let screen_name = screen.title();

    // Row 0: M31A  model · branch · state  (+ screen when not on home)
    let mut spans: Vec<Span> = vec![
        Span::styled(" M31A ", tokens.text_primary),
        Span::styled("  ", tokens.text_muted),
    ];
    if !model_name.is_empty() && model_name != "default" {
        spans.push(Span::styled(model_name, tokens.text_secondary));
        spans.push(Span::styled(" · ", tokens.text_muted));
    }
    if !branch.is_empty() && branch != "N/A" {
        spans.push(Span::styled(branch, tokens.text_secondary));
        spans.push(Span::styled(" · ", tokens.text_muted));
    }
    spans.push(Span::styled(marker, state_style));
    spans.push(Span::styled(format!(" {state_label}"), state_style));
    if screen != ScreenId::Dashboard {
        spans.push(Span::styled(format!(" · {screen_name}"), tokens.text_muted));
    }
    // Governance gates get a quiet suffix (no telemetry).
    if model.lifecycle.stage.is_governance_gate() {
        spans.push(Span::styled(
            format!(" · {}", model.lifecycle.stage.label()),
            tokens.text_muted,
        ));
    }

    let lines = if area.height >= 3 {
        // Roomy terminals: header + hairline separator, no second data row.
        vec![
            Line::from(spans),
            Line::from(Span::styled(
                format!(
                    " {}",
                    "─".repeat((area.width as usize).saturating_sub(2).min(120))
                ),
                tokens.separator,
            )),
        ]
    } else {
        vec![Line::from(spans)]
    };

    let block = if area.height >= 3 {
        Block::default()
    } else {
        Block::default()
            .borders(Borders::BOTTOM)
            .border_style(tokens.separator)
    };
    f.render_widget(Paragraph::new(lines).block(block), area);
}

fn header_state<'a>(
    model: &'a TuiViewModel,
    tokens: &'a ThemeTokens,
) -> (&'static str, String, Style) {
    use crate::tui::model::{RuntimeStartupState as R, UiOperationState as S};
    // Startup state is authoritative in the header: while the runtime is
    // still initializing/hydrating (or failed), the header must say so
    // explicitly instead of a generic Ready/Working. Never suppress the
    // header itself — a missing bridge/runtime is a state, not a reason to
    // stop rendering.
    match &model.runtime_status {
        R::Booting
        | R::InitializingRuntime
        | R::HydratingSession
        | R::HydratingWorkspace
        | R::HydratingExecution => {
            return ("•", "Initializing".to_string(), tokens.text_secondary);
        }
        R::Failed(_) => {
            return ("×", "Startup failed".to_string(), tokens.error);
        }
        R::Ready => {}
    }
    match model.operation_state() {
        S::Failed => ("×", "Failed".to_string(), tokens.error),
        S::Cancelled => ("○", "Cancelled".to_string(), tokens.text_muted),
        S::AwaitingInput | S::AwaitingApproval | S::WaitingForUser | S::WaitingForApproval => {
            ("○", "Waiting".to_string(), tokens.warning)
        }
        S::Thinking
        | S::Planning
        | S::Executing
        | S::RunningTool
        | S::Verifying
        | S::Recovering
        | S::CommandRunning { .. } => {
            if let Some(summary) = model.execution_summary() {
                let short = truncate(&summary, 42);
                ("•", format!("Working · {short}"), tokens.text_secondary)
            } else {
                ("•", "Working".to_string(), tokens.text_secondary)
            }
        }
        S::Completed => ("✓", "Ready".to_string(), tokens.text_muted),
        S::Idle => ("○", "Ready".to_string(), tokens.text_muted),
    }
}

fn short_model_name(raw: &str) -> String {
    if raw.is_empty() || raw == "none" {
        return String::new();
    }
    // Keep provider-qualified ids readable: "meta/llama-3.1-70b-instruct" -> "llama-3.1-70b"
    let base = raw.rsplit('/').next().unwrap_or(raw);
    let short = base.strip_suffix("-instruct").unwrap_or(base);
    truncate(short, 28)
}

fn short_branch(model: &TuiViewModel) -> String {
    // Prefer the execution worktree branch when isolated execution is active,
    // with quiet workspace context so truth is never lost.
    if let Some(exec) = model.execution_worktree_branch.as_ref() {
        if !exec.is_empty() {
            let ws = model.git_branch.clone();
            if !ws.is_empty() && ws != "N/A" && ws != *exec {
                return truncate(exec, 24);
            }
            return truncate(exec, 24);
        }
    }
    let b = model.git_branch.clone();
    if b.is_empty() {
        return String::new();
    }
    truncate(&b, 24)
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else if max <= 1 {
        String::new()
    } else {
        format!("{}…", &s[..max.saturating_sub(1)])
    }
}
