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

    let title_status = if decision.state.is_submitting() {
        " [Submitting...] "
    } else {
        " "
    };
    let title = format!(" [?] {}{}", decision.title, title_status);
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

    // Split inner area into: prompt header (1 line), body (options or custom input), and hint bar (1 line)
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Prompt line
            Constraint::Min(1),    // Body (options or custom input)
            Constraint::Length(1), // Hint bar
        ])
        .split(inner);

    // 1. Prompt header
    let prompt_line = Line::from(vec![
        Span::styled("▸ ", tokens.accent.add_modifier(Modifier::BOLD)),
        Span::styled(&decision.prompt, tokens.text_primary),
    ]);
    f.render_widget(Paragraph::new(prompt_line), chunks[0]);

    // 2. Body
    let available_rows = chunks[1].height as usize;
    let mut body_lines = Vec::new();

    if decision.is_custom_mode {
        body_lines.push(Line::from(vec![Span::styled(
            "  Custom Answer / Feedback:",
            tokens.accent.add_modifier(Modifier::BOLD),
        )]));
        body_lines.push(Line::from(vec![
            Span::styled("  ▸ ", tokens.accent.add_modifier(Modifier::BOLD)),
            Span::styled(
                &decision.custom_input,
                tokens.text_primary.add_modifier(Modifier::BOLD),
            ),
            Span::styled("█", tokens.accent),
        ]));
        if !decision.options.is_empty() {
            body_lines.push(Line::from(vec![Span::styled(
                "  (Press Esc to return to options)",
                tokens.text_muted,
            )]));
        }
    } else {
        for (i, opt) in decision.options.iter().enumerate() {
            if body_lines.len() >= available_rows {
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
                body_lines.push(row);
            } else {
                let cursor = "    ";
                let row = Line::from(vec![
                    Span::styled(cursor, tokens.text_muted),
                    Span::styled(num_key, tokens.text_muted),
                    Span::styled(&opt.label, tokens.text_primary),
                    Span::styled(format!(" — {}", opt.description), tokens.text_muted),
                ]);
                body_lines.push(row);
            }
        }
    }

    f.render_widget(Paragraph::new(body_lines), chunks[1]);

    // 3. Hint bar
    let hint_line = if decision.state.is_submitting() {
        Line::from(vec![Span::styled(
            "Submitting decision to runtime... please wait",
            tokens.accent,
        )])
    } else if decision.is_custom_mode {
        Line::from(vec![
            Span::styled("Type answer", tokens.text_muted),
            Span::styled(" · ", tokens.separator),
            Span::styled("Enter", tokens.accent.add_modifier(Modifier::BOLD)),
            Span::styled(" submit", tokens.text_muted),
            Span::styled(" · ", tokens.separator),
            Span::styled("Esc", tokens.text_secondary),
            Span::styled(" back to options", tokens.text_muted),
        ])
    } else {
        let num_hint = if decision.options.is_empty() {
            "".to_string()
        } else if decision.options.len() <= 9 {
            format!(" or 1-{}", decision.options.len())
        } else {
            " or 1-9".to_string()
        };
        let mut spans = vec![
            Span::styled(format!("↑/↓{} to select", num_hint), tokens.text_muted),
            Span::styled(" · ", tokens.separator),
            Span::styled("Enter", tokens.accent.add_modifier(Modifier::BOLD)),
            Span::styled(" confirm", tokens.text_muted),
        ];
        if decision.allow_custom_answer {
            spans.push(Span::styled(" · ", tokens.separator));
            spans.push(Span::styled(
                "Tab/c",
                tokens.accent.add_modifier(Modifier::BOLD),
            ));
            spans.push(Span::styled(" custom input", tokens.text_muted));
        }
        spans.push(Span::styled(" · ", tokens.separator));
        spans.push(Span::styled("Esc", tokens.text_secondary));
        spans.push(Span::styled(" cancel", tokens.text_muted));
        Line::from(spans)
    };
    f.render_widget(Paragraph::new(hint_line), chunks[2]);
}
