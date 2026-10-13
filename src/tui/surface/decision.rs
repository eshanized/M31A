//! Native interactive decision prompt surface.
//!
//! Renders pending decisions (planning choices, plan reviews, tasks reviews,
//! execution authorization, clarifications) as native TUI selectable option lists.

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::Modifier,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use crate::interaction::decision::InteractiveDecision;
use crate::tui::theme::ThemeTokens;

/// Render an interactive decision prompt in the assigned area.
pub fn render_decision_prompt(
    f: &mut Frame,
    area: Rect,
    decision: &InteractiveDecision,
    tokens: &ThemeTokens,
    is_focused: bool,
) {
    if area.width < 10 || area.height < 4 {
        return;
    }

    let border_style = if is_focused {
        tokens.border_focused.patch(tokens.accent)
    } else {
        tokens.border_default
    };

    let title = format!(" [?] {} ", decision.title);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style)
        .title(Span::styled(
            title,
            tokens.text_primary.add_modifier(Modifier::BOLD),
        ));

    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.width < 8 || inner.height < 3 {
        return;
    }

    // Split inner area into: prompt header (1 line), options list, and hint bar (1 line)
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Prompt line
            Constraint::Min(1),    // Options
            Constraint::Length(1), // Hint bar
        ])
        .split(inner);

    // 1. Prompt header
    let prompt_line = Line::from(vec![
        Span::styled("▸ ", tokens.accent.add_modifier(Modifier::BOLD)),
        Span::styled(&decision.prompt, tokens.text_primary),
    ]);
    f.render_widget(Paragraph::new(prompt_line), chunks[0]);

    // 2. Options list
    let available_rows = chunks[1].height as usize;
    let mut option_lines = Vec::new();

    for (i, opt) in decision.options.iter().enumerate() {
        if option_lines.len() >= available_rows {
            break;
        }

        let is_selected = i == decision.selected_index;
        let num_key = if i < 9 {
            format!("[{}] ", i + 1)
        } else {
            "[-] ".to_string()
        };

        if is_selected {
            let cursor = "  ▸ ";
            let row = Line::from(vec![
                Span::styled(cursor, tokens.accent.add_modifier(Modifier::BOLD)),
                Span::styled(num_key, tokens.accent.add_modifier(Modifier::BOLD)),
                Span::styled(&opt.label, tokens.text_primary.add_modifier(Modifier::BOLD)),
                Span::styled(format!(" — {}", opt.description), tokens.text_secondary),
            ]);
            option_lines.push(row);
        } else {
            let cursor = "    ";
            let row = Line::from(vec![
                Span::styled(cursor, tokens.text_muted),
                Span::styled(num_key, tokens.text_muted),
                Span::styled(&opt.label, tokens.text_primary),
                Span::styled(format!(" — {}", opt.description), tokens.text_muted),
            ]);
            option_lines.push(row);
        }
    }

    f.render_widget(Paragraph::new(option_lines), chunks[1]);

    // 3. Hint bar
    let num_hint = if decision.options.is_empty() {
        "".to_string()
    } else if decision.options.len() <= 9 {
        format!(" or 1-{}", decision.options.len())
    } else {
        " or 1-9".to_string()
    };
    let hint_line = Line::from(vec![
        Span::styled(format!("↑/↓{} to select", num_hint), tokens.text_muted),
        Span::styled(" · ", tokens.separator),
        Span::styled("Enter", tokens.accent.add_modifier(Modifier::BOLD)),
        Span::styled(" confirm", tokens.text_muted),
        Span::styled(" · ", tokens.separator),
        Span::styled("Esc", tokens.text_secondary),
        Span::styled(" cancel", tokens.text_muted),
    ]);
    f.render_widget(Paragraph::new(hint_line), chunks[2]);
}
