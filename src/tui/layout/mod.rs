//! Responsive Layout Classification & Breakdown (TUI-02).
//!
//! Classifies terminal dimensions into 4 tiers:
//! - Compact (<100w): Single-column stacked layout with tabbed pane switching; optimized for 80x24.
//! - Standard (100–159w): Dual-column layout (sidebar + main cockpit).
//! - Large (160–219w): Three-column layout (nav sidebar + cockpit + telemetry).
//! - UltraWide (>=220w): Full wide-screen layout with expanded inspector panes.

pub mod compact;
pub mod wide;

use crate::tui::view_tier::ViewTier;
use ratatui::layout::{Constraint, Direction, Layout, Rect};

pub use crate::tui::view_tier::ViewTier as LayoutViewTier;

/// Responsive layout tier based on terminal width.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LayoutTier {
    Compact,
    Standard,
    Large,
    UltraWide,
}

impl From<ViewTier> for LayoutTier {
    fn from(tier: ViewTier) -> Self {
        match tier {
            ViewTier::Compact => Self::Compact,
            ViewTier::Standard => Self::Standard,
            ViewTier::Large => Self::Large,
            ViewTier::UltraWide => Self::UltraWide,
        }
    }
}

impl From<LayoutTier> for ViewTier {
    fn from(tier: LayoutTier) -> Self {
        match tier {
            LayoutTier::Compact => Self::Compact,
            LayoutTier::Standard => Self::Standard,
            LayoutTier::Large => Self::Large,
            LayoutTier::UltraWide => Self::UltraWide,
        }
    }
}

impl LayoutTier {
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
    pub fn partition(area: Rect) -> (ViewTier, LayoutAreas) {
        let tier = ViewTier::from_dimensions(area.width, area.height);
        let layout_tier: LayoutTier = tier.into();

        let areas = match layout_tier {
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
    let (tier, areas) = ResponsiveLayout::partition(area);
    (tier.into(), areas)
}
