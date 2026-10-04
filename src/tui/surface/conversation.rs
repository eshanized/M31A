//! Primary conversational execution surface (conversation-first).
//!
//! Quiet, open layout: no boxes around the stream, no telemetry dashboard.
//! Hierarchy comes from spacing, role labels, and restrained emphasis.

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph, Wrap};

use crate::tui::model::TuiViewModel;
use crate::tui::theme::ThemeTokens;

/// Render the primary conversation & execution stream surface.
pub fn render_conversation_surface(
    f: &mut Frame,
    area: Rect,
    model: &TuiViewModel,
    tokens: &ThemeTokens,
    _is_focused: bool,
) {
    if area.width < 10 || area.height < 4 {
        return;
    }

    // Reserve one line at top for quiet execution state when active.
    let execution_line = inline_execution_line(model, tokens);
    let (status_area, history_area) = if let Some(ref line) = execution_line {
        let _ = line;
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Min(3)])
            .split(area);
        (Some(chunks[0]), chunks[1])
    } else {
        (None, area)
    };

    if let (Some(sa), Some(line)) = (status_area, execution_line) {
        f.render_widget(Paragraph::new(line), sa);
    }

    let mut lines: Vec<Line<'_>> = Vec::new();

    // Quiet governance hint (single muted line, not a warning banner).
    // Terminal failure/completion also gets one quiet line so empty
    // histories still communicate outcome without a dashboard.
    if model.lifecycle.stage.is_governance_gate() {
        let mut hint = format!("  {} — respond below", model.lifecycle.stage.label());
        if let Some(rev) = model.lifecycle.plan_revision {
            hint.push_str(&format!(" · plan {rev}"));
        }
        if let Some(hash) = model.lifecycle.plan_hash.as_ref() {
            hint.push_str(&format!(" ({})", hash.chars().take(8).collect::<String>()));
        }
        if let Some(rev) = model.lifecycle.task_revision {
            hint.push_str(&format!(" · tasks {rev}"));
        }
        lines.push(Line::from(Span::styled(hint, tokens.text_muted)));
        lines.push(Line::raw(""));
    } else if matches!(
        model.lifecycle.stage,
        crate::tui::lifecycle::TuiLifecycleStage::Failed
            | crate::tui::lifecycle::TuiLifecycleStage::Rejected
            | crate::tui::lifecycle::TuiLifecycleStage::Cancelled
    ) {
        let mut hint = format!("  {}", model.lifecycle.stage.label());
        if let Some(reason) = model.lifecycle.failure_reason.as_ref() {
            let short = if reason.len() > 80 {
                format!("{}…", &reason[..80])
            } else {
                reason.clone()
            };
            hint.push_str(&format!(" — {short}"));
        }
        lines.push(Line::from(Span::styled(hint, tokens.text_secondary)));
        lines.push(Line::raw(""));
    }

    if model.conversation.is_empty() {
        render_welcome_empty_state(&mut lines, tokens);
    } else {
        let max_content_width = history_area.width.saturating_sub(4);
        for item in &model.conversation {
            let rendered = item.render_lines_with_icons(max_content_width, tokens, &model.icons);
            lines.extend(rendered);
            lines.push(Line::raw(""));
        }
    }

    // Viewport scrolling & auto-follow logic
    let total_lines = lines.len();
    let viewport_height = history_area.height as usize;

    let scroll_y = if total_lines > viewport_height {
        let max_scroll = total_lines.saturating_sub(viewport_height);
        max_scroll.saturating_sub(model.scroll_offset)
    } else {
        0
    };

    // No bordered block, no title noise. Open stream; scroll position is
    // discoverable via the quiet footer, not a panel title.
    let block = Block::default();
    let p = Paragraph::new(lines)
        .block(block)
        .scroll((scroll_y as u16, 0))
        .wrap(Wrap { trim: false });

    f.render_widget(p, history_area);
}

/// Quiet inline execution state: "• Working · <what>" or approval notice.
fn inline_execution_line<'a>(model: &'a TuiViewModel, tokens: &'a ThemeTokens) -> Option<Line<'a>> {
    if !model.approvals.is_empty() {
        return Some(Line::from(vec![
            Span::styled(" ○ ", tokens.warning),
            Span::styled("Waiting for your approval", tokens.text_secondary),
        ]));
    }
    let summary = model.execution_summary()?;
    let spin = model.spinner.current().to_string();
    Some(Line::from(vec![
        Span::styled(format!(" {spin} "), tokens.text_muted),
        Span::styled(summary, tokens.text_secondary),
    ]))
}

fn render_welcome_empty_state(lines: &mut Vec<Line<'_>>, tokens: &ThemeTokens) {
    lines.push(Line::raw(""));
    lines.push(Line::from(Span::styled("  M31A", tokens.text_primary)));
    lines.push(Line::raw(""));
    lines.push(Line::from(Span::styled(
        "  Your autonomous software engineering workspace.",
        tokens.text_secondary,
    )));
    lines.push(Line::raw(""));
    lines.push(Line::from(Span::styled(
        "  Describe what you want to build, fix, inspect, or understand.",
        tokens.text_secondary,
    )));
    lines.push(Line::raw(""));
    lines.push(Line::from(Span::styled("  > ", tokens.text_muted)));
    lines.push(Line::raw(""));
    lines.push(Line::from(Span::styled(
        "  / for commands   @ for files   ? for help",
        tokens.text_muted,
    )));
}
