//! Multi-Column Wide Layouts (Standard, Large, Ultra-wide).
//!
//! Conversation-first: the conversation always dominates. Wide viewports
//! add a modest contextual inspector only when explicitly requested via
//! navigation detail — never a permanent telemetry sidebar by default.
//! `sidebar`/`telemetry` rects are therefore None unless the caller opens
//! a detail; the workspace renders conversation full-width.

use super::LayoutAreas;
use ratatui::layout::{Constraint, Direction, Layout, Rect};

/// Partition area into Standard layout (100–159w): full-width cockpit.
pub fn compute_standard_layout(area: Rect) -> LayoutAreas {
    let vert = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Min(12),
            Constraint::Length(1),
        ])
        .split(area);

    LayoutAreas {
        header: vert[0],
        sidebar: None,
        main: vert[1],
        telemetry: None,
        footer: vert[2],
    }
}

/// Partition area into Large layout (160–219w): full-width cockpit.
pub fn compute_large_layout(area: Rect) -> LayoutAreas {
    let vert = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Min(12),
            Constraint::Length(1),
        ])
        .split(area);

    LayoutAreas {
        header: vert[0],
        sidebar: None,
        main: vert[1],
        telemetry: None,
        footer: vert[2],
    }
}

/// Partition area into UltraWide layout (>=220w): conversation width is
/// constrained for readability; surplus is left to the workspace to use
/// for contextual inspectors when open.
pub fn compute_ultrawide_layout(area: Rect) -> LayoutAreas {
    let vert = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Min(12),
            Constraint::Length(1),
        ])
        .split(area);

    LayoutAreas {
        header: vert[0],
        sidebar: None,
        main: vert[1],
        telemetry: None,
        footer: vert[2],
    }
}
