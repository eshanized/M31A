//! Contextual Help & Keybindings Overlay (Section 30, TUI-01).
//!
//! Provides clean, categorised cheat-sheet triggered by `?` or `/help`.

use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use crate::tui::theme::{ThemeMode, ThemeTokens};

/// Render the floating contextual help overlay.
pub fn render_help_overlay(f: &mut Frame, area: Rect, tokens: &ThemeTokens) {
    let width = 76u16.min(area.width.saturating_sub(4));
    let height = 24u16.min(area.height.saturating_sub(2));

    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    let modal_area = Rect::new(x, y, width, height);

    f.render_widget(Clear, modal_area);

    let is_mono = tokens.mode == ThemeMode::MonochromeANSI || ThemeTokens::is_no_color_active();
    let border_style = if is_mono {
        Style::default().add_modifier(Modifier::BOLD)
    } else {
        tokens.accent_primary.add_modifier(Modifier::BOLD)
    };

    let key_style = if is_mono {
        Style::default().add_modifier(Modifier::BOLD)
    } else {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    };

    let title_style = if is_mono {
        Style::default().add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
    } else {
        tokens.accent_primary.add_modifier(Modifier::BOLD)
    };

    let lines = vec![
        Line::raw(""),
        Line::styled("Navigation & Workspace Focus:", title_style),
        Line::from(vec![
            Span::styled("  [Tab]       ", key_style),
            Span::styled(
                "Cycle focus between Composer, Timeline, and Panels",
                tokens.text_secondary,
            ),
        ]),
        Line::from(vec![
            Span::styled("  [Shift+Tab] ", key_style),
            Span::styled("Cycle focus backwards", tokens.text_secondary),
        ]),
        Line::from(vec![
            Span::styled("  [Enter / i] ", key_style),
            Span::styled(
                "Focus interactive composer to begin typing",
                tokens.text_secondary,
            ),
        ]),
        Line::from(vec![
            Span::styled("  [Esc]       ", key_style),
            Span::styled(
                "Unfocus composer, dismiss autocomplete, or close overlay",
                tokens.text_secondary,
            ),
        ]),
        Line::raw(""),
        Line::styled("Command Palette & Search:", title_style),
        Line::from(vec![
            Span::styled("  [Ctrl+P]    ", key_style),
            Span::styled(
                "Universal fuzzy command palette (views, commands, actions)",
                tokens.text_secondary,
            ),
        ]),
        Line::from(vec![
            Span::styled("  [/]         ", key_style),
            Span::styled(
                "Focus composer and prefix slash command (/help, /status, /diff)",
                tokens.text_secondary,
            ),
        ]),
        Line::from(vec![
            Span::styled("  [@]         ", key_style),
            Span::styled(
                "Mention workspace files or directories with auto-complete",
                tokens.text_secondary,
            ),
        ]),
        Line::raw(""),
        Line::styled("Surfaces & Secondary Inspection:", title_style),
        Line::from(vec![Span::styled(
            "  [1] Dashboard  [2] Mission    [3] Tasks DAG   [4] Agents Swarm",
            tokens.text_primary,
        )]),
        Line::from(vec![Span::styled(
            "  [5] Tools      [6] Jobs       [7] Verify      [8] Git Diff",
            tokens.text_primary,
        )]),
        Line::from(vec![Span::styled(
            "  [9] Approvals  [0] Doctor     [L] Logs        [M] Telemetry",
            tokens.text_primary,
        )]),
        Line::from(vec![Span::styled(
            "  [A] Artifacts  [R] Replay     [?] Help        [q] Quit",
            tokens.text_primary,
        )]),
        Line::raw(""),
        Line::styled("Scrolling & Follow Mode:", title_style),
        Line::from(vec![
            Span::styled("  [PgUp/PgDn] ", key_style),
            Span::styled(
                "Scroll conversation viewport (pauses auto-follow)",
                tokens.text_secondary,
            ),
        ]),
        Line::from(vec![
            Span::styled("  [End]       ", key_style),
            Span::styled(
                "Jump to bottom and re-engage live auto-follow",
                tokens.text_secondary,
            ),
        ]),
        Line::raw(""),
        Line::styled("Press [Esc] or [?] to close this guide", tokens.text_muted),
    ];

    let block = Block::default()
        .title(" M31A Cockpit Keyboard & Interaction Guide [?] ")
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_style(border_style)
        .style(tokens.bg_overlay);

    let p = Paragraph::new(lines).block(block);
    f.render_widget(p, modal_area);
}
