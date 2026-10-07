//! Responsive Layout Classification & Breakdown (TUI-02).
//!
//! Classifies terminal dimensions into 4 tiers:
//! - Compact (<100w): Single-column stacked layout with tabbed pane switching; optimized for 80x24.
//! - Standard (100–159w): Dual-column layout (sidebar + main cockpit).
//! - Large (160–219w): Three-column layout (nav sidebar + cockpit + telemetry).
//! - UltraWide (>=220w): Full wide-screen layout with expanded inspector panes.

pub mod compact;
pub mod wide;

use ratatui::layout::{Constraint, Direction, Layout, Rect};

/// Responsive layout tier based on terminal dimensions.
///
/// This is the ONE terminal-classification type in the TUI. Width and height
/// both participate: narrow or short viewports collapse to `Compact`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LayoutTier {
    /// Compact tier: < 100 columns or < 30 rows. Single-column stacked layout.
    Compact,
    /// Standard tier: 100–159 columns. Dual-column (sidebar + cockpit).
    Standard,
    /// Large tier: 160–219 columns. Three-column cockpit.
    Large,
    /// UltraWide tier: >= 220 columns. Full wide cockpit with inspectors.
    UltraWide,
}

impl LayoutTier {
    /// Minimum supported terminal width in columns.
    pub const MIN_WIDTH: u16 = 80;
    /// Minimum supported terminal height in rows.
    pub const MIN_HEIGHT: u16 = 24;

    /// Classify terminal dimensions into a `LayoutTier`.
    pub fn from_dimensions(width: u16, height: u16) -> Self {
        if width < 100 || height < Self::MIN_HEIGHT {
            Self::Compact
        } else if width < 160 {
            Self::Standard
        } else if width < 220 {
            Self::Large
        } else {
            Self::UltraWide
        }
    }

    /// Whether the dimensions fall below the absolute 80x24 minimum.
    pub fn is_below_minimum(width: u16, height: u16) -> bool {
        width < Self::MIN_WIDTH || height < Self::MIN_HEIGHT
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Compact => "compact",
            Self::Standard => "standard",
            Self::Large => "large",
            Self::UltraWide => "ultrawide",
        }
    }
}

/// Classify terminal dimensions into responsive tier.
pub fn classify_terminal_size(width: u16, _height: u16) -> LayoutTier {
    if width < 100 {
        LayoutTier::Compact
    } else if width < 160 {
        LayoutTier::Standard
    } else if width < 220 {
        LayoutTier::Large
    } else {
        LayoutTier::UltraWide
    }
}

/// Rectangles allocated for the primary screen components in a frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutAreas {
    pub header: Rect,
    pub main: Rect,
    pub sidebar: Option<Rect>,
    pub telemetry: Option<Rect>,
    pub footer: Rect,
}

/// Helper struct for computing responsive layouts and verifying viewport bounds.
#[derive(Debug, Clone)]
pub struct ResponsiveLayout;

impl ResponsiveLayout {
    /// Computes responsive layout areas for a given area, honoring header, body, footer,
    /// and multi-column partitioning.
    pub fn partition(area: Rect) -> (LayoutTier, LayoutAreas) {
        let tier = LayoutTier::from_dimensions(area.width, area.height);

        let areas = match tier {
            LayoutTier::Compact => compact::compute_compact_layout(area),
            LayoutTier::Standard => wide::compute_standard_layout(area),
            LayoutTier::Large => wide::compute_large_layout(area),
            LayoutTier::UltraWide => wide::compute_ultrawide_layout(area),
        };

        (tier, areas)
    }

    /// Splits an area vertically into Header (3 lines), Content (remaining), and Footer (1 line).
    pub fn split_vertical_chrome(area: Rect) -> (Rect, Rect, Rect) {
        if area.height < 5 {
            return (area, Rect::default(), Rect::default());
        }

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(1),
                Constraint::Length(1),
            ])
            .split(area);

        (chunks[0], chunks[1], chunks[2])
    }
}

/// Compute layout partition for the current terminal area.
pub fn compute_layout(area: Rect) -> (LayoutTier, LayoutAreas) {
    ResponsiveLayout::partition(area)
}
