//! Quiet contextual help overlay.

use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use crate::tui::theme::ThemeTokens;

/// Render the floating contextual help overlay (quiet, readable).
pub fn render_help_overlay(f: &mut Frame, area: Rect, tokens: &ThemeTokens) {
    let width = 68u16.min(area.width.saturating_sub(4));
    let height = 22u16.min(area.height.saturating_sub(2));

    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    let modal_area = Rect::new(x, y, width, height);

    f.render_widget(Clear, modal_area);

    let key_style = tokens.text_primary;
    let title_style = tokens.text_muted;

    let row = |key: &'static str, desc: &'static str| {
        Line::from(vec![
            Span::styled(format!("  {key:<14}"), key_style),
            Span::styled(desc, tokens.text_secondary),
        ])
    };

    let lines = vec![
        Line::raw(""),
        Line::styled("  M31A help", tokens.text_primary),
        Line::raw(""),
        Line::styled("  Composer", title_style),
        row("Enter", "send"),
        row("Shift+Enter", "newline"),
        row("/", "commands"),
        row("@", "files"),
        row("Up / Down", "history"),
        Line::raw(""),
        Line::styled("  Navigation", title_style),
        row("Ctrl+P", "command palette"),
        row("1-0, L, M, A, R", "screens (when not typing)"),
        row("PgUp / PgDn", "scroll conversation"),
        row("?", "this help"),
        row("q", "quit"),
        Line::raw(""),
        Line::styled("  Esc closes · ? reopens", tokens.text_muted),
    ];

    let block = Block::default()
        .title(" Help ")
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_style(tokens.separator)
        .style(tokens.surface);

    let p = Paragraph::new(lines).block(block);
    f.render_widget(p, modal_area);
    let _ = Style::default();
}
