//! Git Timeline & Unstaged Diff Surface (Section 24, TUI-01).
//!
//! Provides a dedicated, high-grade terminal diff inspection surface:
//! - Branch status and worktree attribution
//! - Diff metrics: files changed, +additions (green), -deletions (red)
//! - Syntax-highlighted hunks with old/new line numbers
//! - Scrollable viewport with non-blocking key navigation

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

use crate::tui::component::diff::{DiffParser, render_diff_lines};
use crate::tui::model::TuiViewModel;
use crate::tui::theme::{ThemeMode, ThemeTokens};

/// Render the Git & Diff surface.
pub fn render_git_surface(
    f: &mut Frame,
    area: Rect,
    model: &TuiViewModel,
    tokens: &ThemeTokens,
    is_focused: bool,
    scroll_offset: usize,
) {
    if area.width < 10 || area.height < 4 {
        return;
    }

    let is_mono = tokens.mode == ThemeMode::MonochromeANSI || ThemeTokens::is_no_color_active();
    let border_style = if is_focused {
        tokens.border_focused
    } else if is_mono {
        Style::default()
    } else {
        tokens.border_default
    };

    if model.git_branch == "disabled" || model.git_branch == "DISABLED" {
        let p = Paragraph::new(vec![
            Line::raw(""),
            Line::styled(
                "  Git integration is disabled by user preference.",
                tokens.text_muted.add_modifier(Modifier::BOLD),
            ),
            Line::styled(
                "  M31A operates directly on workspace filesystem files without version control.",
                tokens.text_secondary,
            ),
        ])
        .block(
            Block::default()
                .title(" Git [Disabled] ")
                .borders(Borders::NONE)
                .border_style(border_style),
        );
        f.render_widget(p, area);
        return;
    }

    let (meta_area, diff_area) = if area.height >= 12 {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(4), Constraint::Min(6)])
            .split(area);
        (Some(chunks[0]), chunks[1])
    } else {
        (None, area)
    };

    let diff_text = model.git_diff_text.as_deref().unwrap_or("");
    let (parsed_lines, stats) = DiffParser::parse(diff_text);

    // 1. Render Meta Header if space permits
    if let Some(m_area) = meta_area {
        let mut meta_lines = Vec::new();
        meta_lines.push(Line::from(vec![
            Span::styled("Branch : ", tokens.text_muted),
            Span::styled(
                &model.git_branch,
                tokens.accent_primary.add_modifier(Modifier::BOLD),
            ),
            Span::raw(" | "),
            Span::styled(
                format!(
                    "{} files changed",
                    stats
                        .files_changed
                        .max(if diff_text.is_empty() { 0 } else { 1 })
                ),
                tokens.text_secondary,
            ),
            Span::raw(" | "),
            Span::styled(
                format!("+{} additions", stats.additions),
                tokens.diff_addition.add_modifier(Modifier::BOLD),
            ),
            Span::raw(" | "),
            Span::styled(
                format!("-{} deletions", stats.deletions),
                tokens.diff_deletion.add_modifier(Modifier::BOLD),
            ),
        ]));

        let p_meta = Paragraph::new(meta_lines).block(
            Block::default()
                .title(" Git ")
                .borders(Borders::NONE)
                .border_style(border_style),
        );
        f.render_widget(p_meta, m_area);
    }

    // 2. Render Diff Content
    if diff_text.is_empty() {
        let p = Paragraph::new(vec![
            Line::raw(""),
            Line::styled(
                "  Working tree is clean. No unstaged changes detected.",
                tokens.text_muted,
            ),
            Line::styled(
                "  File modifications will appear here with syntax coloring and line numbers.",
                tokens.text_secondary,
            ),
        ])
        .block(
            Block::default()
                .title(" Diff ")
                .borders(Borders::NONE)
                .border_style(border_style),
        );
        f.render_widget(p, diff_area);
        return;
    }

    let rendered_lines =
        render_diff_lines(&parsed_lines, diff_area.width.saturating_sub(4), tokens);
    let total_lines = rendered_lines.len();
    let viewport_height = diff_area.height.saturating_sub(2) as usize;

    let max_scroll = total_lines.saturating_sub(viewport_height);
    let effective_scroll = scroll_offset.min(max_scroll);

    let title = format!(
        " Diff ({} lines) [Scroll: {}/{}] ",
        total_lines, effective_scroll, max_scroll
    );

    let p = Paragraph::new(rendered_lines)
        .block(
            Block::default()
                .title(title)
                .borders(Borders::NONE)
                .border_style(border_style),
        )
        .scroll((effective_scroll as u16, 0))
        .wrap(Wrap { trim: false });

    f.render_widget(p, diff_area);
}
