//! Compact Layout (<100w, 80x24 minimum).
//!
//! Single column: quiet header, conversation, composer, minimal footer.
//! No sidebar, no telemetry, no dashboard.

use super::LayoutAreas;
use ratatui::layout::{Constraint, Direction, Layout, Rect};

/// Partition area into stacked single-column layout.
pub fn compute_compact_layout(area: Rect) -> LayoutAreas {
    if area.height < 3 {
        return LayoutAreas {
            header: Rect::default(),
            main: area,
            sidebar: None,
            telemetry: None,
            footer: Rect::default(),
        };
    }
    // Quiet header: 2 lines, Footer: 1 line, Main: remainder
    let header_h = if area.height >= 5 { 2 } else { 1 };
    let footer_h = if area.height >= 4 { 1 } else { 0 };
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(header_h),
            Constraint::Min(1),
            Constraint::Length(footer_h),
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
