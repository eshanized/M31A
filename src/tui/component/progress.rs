//! Progress Bar and Visual Progress Primitives (TUI-01, TDS-02).
//!
//! Provides dense, high-clarity progress bars and metrics for task DAG,
//! verification pipelines, and job execution.

use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

use crate::tui::theme::{ThemeMode, ThemeTokens};

/// Render an ASCII/Unicode progress bar string.
pub fn format_progress_bar(percentage: u8, width: usize) -> String {
    let pct = percentage.min(100) as usize;
    if width < 6 {
        return format!("{pct}%");
    }

    // Allocate 5 chars for " 100%"
    let bar_width = width.saturating_sub(5).max(3);
    let filled = (pct * bar_width) / 100;
    let unfilled = bar_width.saturating_sub(filled);

    let filled_str = "█".repeat(filled);
    let unfilled_str = "░".repeat(unfilled);

    format!("{filled_str}{unfilled_str} {pct:>3}%")
}

/// Render a styled progress line.
pub fn render_progress_line<'a>(
    label: &str,
    percentage: u8,
    width: u16,
    tokens: &ThemeTokens,
) -> Line<'a> {
    let is_mono = tokens.mode == ThemeMode::MonochromeANSI || ThemeTokens::is_no_color_active();
    let label_width = 16.min(width as usize / 3);
    let truncated_label = if label.len() > label_width {
        format!(
            "{:.width$}...",
            label,
            width = label_width.saturating_sub(3)
        )
    } else {
        format!("{label:<label_width$}")
    };

    let remaining_width = (width as usize).saturating_sub(label_width + 2);
    let bar_str = format_progress_bar(percentage, remaining_width);

    let bar_style = if is_mono {
        Style::default()
    } else if percentage >= 100 {
        tokens.status_ok
    } else if percentage > 0 {
        tokens.status_running
    } else {
        tokens.text_muted
    };

    Line::from(vec![
        Span::styled(
            truncated_label,
            if is_mono {
                Style::default().add_modifier(Modifier::BOLD)
            } else {
                tokens.text_primary.add_modifier(Modifier::BOLD)
            },
        ),
        Span::raw(" "),
        Span::styled(bar_str, bar_style),
    ])
}
