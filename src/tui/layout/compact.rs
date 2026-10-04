//! Compact Layout (<100w, 80x24 minimum).
//!
//! Single column: quiet header, conversation, composer, minimal footer.
//! No sidebar, no telemetry, no dashboard.

use super::LayoutAreas;
use ratatui::layout::{Constraint, Direction, Layout, Rect};

/// Partition area into stacked single-column layout.
pub fn compute_compact_layout(area: Rect) -> LayoutAreas {
    // Quiet header: 2 lines, Footer: 1 line, Main: remainder
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Min(10),
            Constraint::Length(1),
        ])
        .split(area);

    LayoutAreas {
        header: chunks[0],
        main: chunks[1],
        sidebar: None,
        telemetry: None,
        footer: chunks[2],
    }
}
