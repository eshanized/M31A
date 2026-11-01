//! Theme-aware diff renderer with line numbers and gutter styling.

use crate::engine::{DiffLine, DiffLineKind};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

pub struct DiffRenderer;

impl DiffRenderer {
    /// Renders a slice of DiffLines to styled Ratatui Lines for terminal display.
    pub fn render_lines(lines: &[DiffLine], max_lines: usize) -> Vec<Line<'static>> {
        let mut out = Vec::new();

        for dl in lines.iter().take(max_lines) {
            let (gutter_symbol, gutter_style, line_style) = match dl.kind {
                DiffLineKind::Add => (
                    "+",
                    Style::default()
                        .fg(Color::Rgb(46, 160, 67))
                        .add_modifier(Modifier::BOLD),
                    Style::default()
                        .fg(Color::Rgb(126, 231, 135))
                        .bg(Color::Rgb(15, 40, 20)),
                ),
                DiffLineKind::Delete => (
                    "-",
                    Style::default()
                        .fg(Color::Rgb(248, 81, 73))
                        .add_modifier(Modifier::BOLD),
                    Style::default()
                        .fg(Color::Rgb(255, 160, 150))
                        .bg(Color::Rgb(45, 15, 15)),
                ),
                DiffLineKind::Context => (
                    " ",
                    Style::default().fg(Color::DarkGray),
                    Style::default().fg(Color::Gray),
                ),
                DiffLineKind::HunkHeader => (
                    "@",
                    Style::default().fg(Color::Cyan),
                    Style::default().fg(Color::Cyan).bg(Color::Rgb(10, 25, 40)),
                ),
            };

            let old_str = dl
                .old_line_num
                .map(|n| format!("{:3}", n))
                .unwrap_or_else(|| "   ".to_string());
            let new_str = dl
                .new_line_num
                .map(|n| format!("{:3}", n))
                .unwrap_or_else(|| "   ".to_string());

            let mut spans = vec![Span::styled(
                format!("{} {} {} ", old_str, new_str, gutter_symbol),
                gutter_style,
            )];

            for ws in &dl.word_spans {
                if ws.is_highlighted && matches!(dl.kind, DiffLineKind::Add | DiffLineKind::Delete)
                {
                    spans.push(Span::styled(
                        ws.text.clone(),
                        line_style
                            .add_modifier(Modifier::BOLD)
                            .add_modifier(Modifier::UNDERLINED),
                    ));
                } else {
                    spans.push(Span::styled(ws.text.clone(), line_style));
                }
            }

            out.push(Line::from(spans));
        }

        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::DiffEngine;

    #[test]
    fn test_render_lines() {
        let (lines, _) = DiffEngine::compute_diff("a\n", "b\n");
        let rendered = DiffRenderer::render_lines(&lines, 10);
        assert_eq!(rendered.len(), 2);
    }
}
