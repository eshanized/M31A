//! View Tier & Geometry Breakpoints (TDS-05, D-05).
//!
//! Classifies terminal viewports into 4 responsive tiers and guards the
//! absolute minimum 80x24 operating viewport.

use serde::{Deserialize, Serialize};

/// The 4 responsive viewport tiers for M31A TUI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum ViewTier {
    /// Compact tier: < 100 columns or < 30 rows. Single-column or stacked layout.
    Compact,
    /// Standard tier: 100–159 columns, 30–49 rows. Dual-column (sidebar + cockpit).
    Standard,
    /// Large tier: 160–219 columns, 50–79 rows. Three-column cockpit.
    Large,
    /// UltraWide tier: >= 220 columns, >= 80 rows. Full wide cockpit with expanded inspectors.
    UltraWide,
}

impl ViewTier {
    /// Minimum supported terminal width in columns.
    pub const MIN_WIDTH: u16 = 80;
    /// Minimum supported terminal height in rows.
    pub const MIN_HEIGHT: u16 = 24;

    /// Classifies terminal dimensions into a `ViewTier`.
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

    /// Determines if the current terminal dimensions fall below the absolute 80x24 minimum.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_view_tier_classification() {
        assert_eq!(ViewTier::from_dimensions(80, 24), ViewTier::Compact);
        assert_eq!(ViewTier::from_dimensions(99, 29), ViewTier::Compact);
        assert_eq!(ViewTier::from_dimensions(100, 30), ViewTier::Standard);
        assert_eq!(ViewTier::from_dimensions(159, 49), ViewTier::Standard);
        assert_eq!(ViewTier::from_dimensions(160, 50), ViewTier::Large);
        assert_eq!(ViewTier::from_dimensions(219, 79), ViewTier::Large);
        assert_eq!(ViewTier::from_dimensions(220, 80), ViewTier::UltraWide);
    }

    #[test]
    fn test_minimum_boundary_check() {
        assert!(ViewTier::is_below_minimum(79, 24));
        assert!(ViewTier::is_below_minimum(80, 23));
        assert!(!ViewTier::is_below_minimum(80, 24));
        assert!(!ViewTier::is_below_minimum(120, 40));
    }
}
