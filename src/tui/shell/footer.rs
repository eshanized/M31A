//! Context-Sensitive Application Shell Footer (Section 5, 19, 30, TUI-01).
//!
//! Provides dynamic keyboard shortcuts and focus indicator that adapt
//! to the active surface and operator focus target.

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::Span;
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::tui::component::key_hint::{KeyHint, format_key_hints};
use crate::tui::focus::FocusTarget;
use crate::tui::lifecycle::TuiLifecycleStage;
use crate::tui::navigation::ScreenId;
use crate::tui::theme::{ThemeMode, ThemeTokens};

/// Render the context-sensitive shell footer.
pub fn render_footer(
    f: &mut Frame,
    area: Rect,
    screen: ScreenId,
    focus: FocusTarget,
    is_composer_focused: bool,
    lifecycle: &crate::tui::lifecycle::TuiLifecycleProjection,
    tokens: &ThemeTokens,
) {
    let is_mono = tokens.mode == ThemeMode::MonochromeANSI || ThemeTokens::is_no_color_active();

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(14), Constraint::Min(20)])
        .split(area);

    // 1. Focus Indicator Badge
    let (focus_label, focus_style) = match focus {
        FocusTarget::Composer => (
            " [COMPOSER] ",
            if is_mono {
                Style::default().add_modifier(Modifier::BOLD)
            } else {
                tokens.status_running.add_modifier(Modifier::BOLD)
            },
        ),
        FocusTarget::Conversation => (
            " [STREAM]   ",
            if is_mono {
                Style::default()
            } else {
                tokens.accent_primary
            },
        ),
        FocusTarget::ContextPanel => (
            " [DETAILS]  ",
            if is_mono {
                Style::default()
            } else {
                tokens.status_ok
            },
        ),
        FocusTarget::Overlay => (
            " [MODAL]    ",
            if is_mono {
                Style::default().add_modifier(Modifier::REVERSED)
            } else {
                tokens.status_warning.add_modifier(Modifier::BOLD)
            },
        ),
    };

    let p_focus = Paragraph::new(Span::styled(focus_label, focus_style)).block(
        Block::default()
            .borders(Borders::TOP)
            .border_style(tokens.border_default),
    );
    f.render_widget(p_focus, chunks[0]);

    // 2. Dynamic Hints Line
    let hints =
        if is_composer_focused {
            // Lifecycle-aware composer hints: the composer must never imply that
            // free text advances governance.
            match lifecycle.stage {
                TuiLifecycleStage::DiscoveryRequired => vec![
                    KeyHint::new("Enter", "Answer question", 3),
                    KeyHint::new("Tab", "Autocomplete", 2),
                    KeyHint::new("Ctrl+P", "Palette", 3),
                    KeyHint::new("Esc", "Unfocus", 3),
                    KeyHint::new("?", "Help", 1),
                ],
                TuiLifecycleStage::PlanReviewRequired
                | TuiLifecycleStage::PlanRevisionAvailable => vec![
                    KeyHint::new("/plan accept", "Accept", 3),
                    KeyHint::new("/plan revise", "Revise", 2),
                    KeyHint::new("Ctrl+P", "Palette", 3),
                    KeyHint::new("Esc", "Unfocus", 3),
                    KeyHint::new("?", "Help", 1),
                ],
                TuiLifecycleStage::TasksReviewRequired
                | TuiLifecycleStage::TaskRevisionAvailable => vec![
                    KeyHint::new("/tasks accept", "Accept", 3),
                    KeyHint::new("/tasks regen", "Regen", 2),
                    KeyHint::new("Ctrl+P", "Palette", 3),
                    KeyHint::new("Esc", "Unfocus", 3),
                    KeyHint::new("?", "Help", 1),
                ],
                TuiLifecycleStage::ExecutionAuthorizationRequired => vec![
                    KeyHint::new("/authorize yes", "Authorize", 3),
                    KeyHint::new("/authorize no", "Reject", 2),
                    KeyHint::new("Ctrl+P", "Palette", 3),
                    KeyHint::new("Esc", "Unfocus", 3),
                    KeyHint::new("?", "Help", 1),
                ],
                _ => vec![
                    KeyHint::new("Enter", "Submit", 3),
                    KeyHint::new("Shift+Enter", "Multiline", 2),
                    KeyHint::new("Tab", "Autocomplete", 2),
                    KeyHint::new("Ctrl+P", "Palette", 3),
                    KeyHint::new("Esc", "Unfocus", 3),
                    KeyHint::new("?", "Help", 1),
                ],
            }
        } else {
            match screen {
                ScreenId::Dashboard => vec![
                    KeyHint::new("Enter/i", "Focus Composer", 3),
                    KeyHint::new("Tab", "Switch Pane", 3),
                    KeyHint::new("PgUp/PgDn", "Scroll", 2),
                    KeyHint::new("1-0", "Screens", 2),
                    KeyHint::new("Ctrl+P", "Palette", 3),
                    KeyHint::new("Space", "Pause/Resume", 1),
                    KeyHint::new("?", "Help", 2),
                    KeyHint::new("q", "Quit", 3),
                ],
                ScreenId::Git => vec![
                    KeyHint::new("PgUp/PgDn", "Scroll Diff", 3),
                    KeyHint::new("Esc/1", "Dashboard", 3),
                    KeyHint::new("Ctrl+P", "Palette", 2),
                    KeyHint::new("?", "Help", 2),
                    KeyHint::new("q", "Quit", 3),
                ],
                _ => vec![
                    KeyHint::new("Esc/1", "Dashboard", 3),
                    KeyHint::new("j/k", "Navigate", 3),
                    KeyHint::new("1-0", "Screens", 2),
                    KeyHint::new("Ctrl+P", "Palette", 3),
                    KeyHint::new("?", "Help", 2),
                    KeyHint::new("q", "Quit", 3),
                ],
            }
        };

    let hint_line = format_key_hints(&hints, chunks[1].width.saturating_sub(2), tokens);
    let p_hints = Paragraph::new(hint_line).block(
        Block::default()
            .borders(Borders::TOP)
            .border_style(tokens.border_default),
    );
    f.render_widget(p_hints, chunks[1]);
}
