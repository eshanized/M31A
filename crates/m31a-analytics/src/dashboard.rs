//! Full-screen analytics dashboard modal view.

use crate::charts::{render_bar, render_sparkline};
use crate::metrics::SessionTokenMetrics;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

pub struct AnalyticsDashboard;

impl AnalyticsDashboard {
    pub fn build_lines(metrics: &SessionTokenMetrics, width: usize) -> Vec<Line<'static>> {
        let mut lines = Vec::new();

        // Header
        lines.push(Line::from(vec![Span::styled(
            " M31A FRONTIER INTELLIGENCE - SESSION ANALYTICS ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )]));
        lines.push(Line::from(""));

        // Overview Cards
        lines.push(Line::from(vec![
            Span::styled(
                "Tokens Total: ",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("{}  ", metrics.total_tokens),
                Style::default().fg(Color::White),
            ),
            Span::styled(
                "Est. Cost: ",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("${:.4}  ", metrics.estimated_cost_usd),
                Style::default().fg(Color::White),
            ),
            Span::styled(
                "Turns: ",
                Style::default()
                    .fg(Color::Magenta)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("{}  ", metrics.turn_count),
                Style::default().fg(Color::White),
            ),
            Span::styled(
                "Last TTFT: ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("{}ms", metrics.ttft_ms),
                Style::default().fg(Color::White),
            ),
        ]));
        lines.push(Line::from(""));

        // Bar Chart Breakdown
        let max_tok = metrics.total_tokens.max(1);
        lines.push(Line::from(vec![Span::styled(
            " Token Distribution:",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )]));

        lines.push(Line::from(render_bar(
            "Prompt",
            metrics.prompt_tokens,
            max_tok,
            width,
        )));
        lines.push(Line::from(render_bar(
            "Completion",
            metrics.completion_tokens,
            max_tok,
            width,
        )));
        lines.push(Line::from(render_bar(
            "Reasoning (R1)",
            metrics.reasoning_tokens,
            max_tok,
            width,
        )));
        lines.push(Line::from(""));

        // Model Breakdown
        if !metrics.model_breakdown.is_empty() {
            lines.push(Line::from(vec![Span::styled(
                " Model Consumption:",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )]));
            for (model, count) in &metrics.model_breakdown {
                lines.push(Line::from(render_bar(model, *count, max_tok, width)));
            }
            lines.push(Line::from(""));
        }

        // Sparkline trend
        let default_series = [metrics.total_tokens];
        let trend = if !metrics.turn_token_history.is_empty() {
            metrics.turn_token_history.as_slice()
        } else {
            &default_series
        };
        lines.push(Line::from(vec![
            Span::styled(
                " Activity Sparkline: [",
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled(
                render_sparkline(trend),
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("] (recent turns)", Style::default().fg(Color::DarkGray)),
        ]));

        lines.push(Line::from(""));
        lines.push(Line::from(vec![Span::styled(
            " Press [Esc] or [q] to close dashboard",
            Style::default().fg(Color::DarkGray),
        )]));

        lines
    }
}
