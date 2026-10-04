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
    model: &mut TuiViewModel,
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

    // Viewport scrolling & auto-follow logic (§12–§15).
    //
    // The scroll state mutated by keyboard/mouse input is exactly the state
    // consumed here: one authoritative conversation viewport owned by the
    // view model. `note_rendered_frame` preserves the viewport when new
    // lines arrive while the operator reads older content.
    //
    // Lines are owned before the mutable viewport update so the immutable
    // conversation borrow never overlaps the viewport write.
    let owned: Vec<Line<'static>> = lines
        .into_iter()
        .map(|l| {
            Line::from(
                l.spans
                    .into_iter()
                    .map(|s| Span::styled(s.content.into_owned(), s.style))
                    .collect::<Vec<_>>(),
            )
        })
        .collect();
    let total_lines = owned.len();
    let viewport_height = history_area.height as usize;
    model.note_rendered_frame(total_lines, viewport_height);

    let total = model.content_height;
    let viewport = model.viewport_height.max(1);
    let max_scroll = model.max_scroll();
    let scroll_y = max_scroll.saturating_sub(model.scroll_offset);

    // Reserve a thin scrollbar column only when content overflows. The
    // scrollbar is derived from actual content height — never faked.
    let overflow = total > viewport_height && history_area.width > 12;
    let (text_area, bar_area) = if overflow {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(8), Constraint::Length(1)])
            .split(history_area);
        (chunks[0], Some(chunks[1]))
    } else {
        (history_area, None)
    };

    // No bordered block, no title noise. Open stream; scroll position is
    // discoverable via the scrollbar and the quiet footer, not a panel title.
    let block = Block::default();
    let p = Paragraph::new(owned)
        .block(block)
        .scroll((scroll_y as u16, 0))
        .wrap(Wrap { trim: false });

    f.render_widget(p, text_area);

    if let Some(bar) = bar_area {
        render_scrollbar(f, bar, total, viewport, scroll_y, tokens);
    }

    // Subtle "new activity below" indicator: only when the operator scrolled
    // up and new items arrived since. Never yanks the viewport.
    if !model.follow && model.unseen_count > 0 && history_area.height >= 3 {
        let label = format!(" ↓ {} new · End to follow ", model.unseen_count);
        let w = label.chars().count().min(text_area.width as usize).max(8) as u16;
        let popup = Rect::new(
            text_area.x + text_area.width.saturating_sub(w + 2),
            text_area.y + text_area.height.saturating_sub(1),
            w + 2,
            1,
        );
        let pill = Paragraph::new(Line::from(Span::styled(label, tokens.text_secondary)));
        f.render_widget(pill, popup);
    }
}

/// Thin scrollbar on the far edge of the conversation region.
///
/// `total` / `viewport` / `scroll_top` are real rendered-line counts from
/// the current frame. Track is `│`, thumb is `█`.
fn render_scrollbar(
    f: &mut Frame,
    area: Rect,
    total: usize,
    viewport: usize,
    scroll_top: usize,
    tokens: &ThemeTokens,
) {
    use ratatui::text::Text;
    if area.height == 0 || total <= viewport {
        return;
    }
    let h = area.height as usize;
    let thumb_len = ((viewport * h) / total).max(1).min(h);
    let max_top = total.saturating_sub(viewport).max(1);
    let thumb_start = (scroll_top * (h.saturating_sub(thumb_len))) / max_top;
    let mut buf: Vec<Line<'_>> = Vec::with_capacity(h);
    for row in 0..h {
        let in_thumb = row >= thumb_start && row < thumb_start + thumb_len;
        buf.push(Line::from(Span::styled(
            if in_thumb { "█" } else { "│" },
            if in_thumb {
                tokens.text_secondary
            } else {
                tokens.separator
            },
        )));
    }
    f.render_widget(Paragraph::new(Text::from(buf)), area);
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
