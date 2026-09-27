//! Multi-Column Wide Layouts (Standard, Large, Ultra-wide) (TUI-02).

use super::LayoutAreas;
use ratatui::layout::{Constraint, Direction, Layout, Rect};

/// Partition area into Standard layout (100–159w): Sidebar + Main Cockpit.
pub fn compute_standard_layout(area: Rect) -> LayoutAreas {
    let vert = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(12),
            Constraint::Length(2),
        ])
        .split(area);

    let middle = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(26), Constraint::Min(60)])
        .split(vert[1]);

    LayoutAreas {
        header: vert[0],
        sidebar: Some(middle[0]),
        main: middle[1],
        telemetry: None,
        footer: vert[2],
    }
}

/// Partition area into Large layout (160–219w): Sidebar + Main Cockpit + Telemetry.
pub fn compute_large_layout(area: Rect) -> LayoutAreas {
    let vert = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(12),
            Constraint::Length(2),
        ])
        .split(area);

    let middle = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(28),
            Constraint::Min(80),
            Constraint::Length(36),
        ])
        .split(vert[1]);

    LayoutAreas {
        header: vert[0],
        sidebar: Some(middle[0]),
        main: middle[1],
        telemetry: Some(middle[2]),
        footer: vert[2],
    }
}

/// Partition area into UltraWide layout (>=220w): Expanded Sidebar + Ultra Cockpit + Telemetry.
pub fn compute_ultrawide_layout(area: Rect) -> LayoutAreas {
    let vert = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(12),
            Constraint::Length(2),
        ])
        .split(area);

    let middle = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(34),
            Constraint::Min(120),
            Constraint::Length(45),
        ])
        .split(vert[1]);

    LayoutAreas {
        header: vert[0],
        sidebar: Some(middle[0]),
        main: middle[1],
        telemetry: Some(middle[2]),
        footer: vert[2],
    }
}
