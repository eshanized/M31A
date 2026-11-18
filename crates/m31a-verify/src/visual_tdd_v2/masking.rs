//! Region-Specific Comparison and Dynamic Masking.
//!
//! Provides coordinate-based and CSS-selector-based masking to ignore
//! volatile regions (e.g. timestamps, randomized avatars, carousels, cursor blinks).

use serde::{Deserialize, Serialize};

/// A rectangular boundary region to mask during image comparison.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RectRegion {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl RectRegion {
    pub fn new(x: u32, y: u32, width: u32, height: u32) -> Self {
        Self { x, y, width, height }
    }

    /// Returns whether this rectangle contains point (px, py).
    #[inline]
    pub fn contains(&self, px: u32, py: u32) -> bool {
        px >= self.x
            && px < self.x.saturating_add(self.width)
            && py >= self.y
            && py < self.y.saturating_add(self.height)
    }
}

/// Dynamic or static mask specification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MaskRule {
    /// Explicit pixel coordinate bounding box
    Rect(RectRegion),
    /// CSS Selector to resolve in DOM into one or more bounding rects
    Selector { selector: String },
}

/// Set of resolved rectangular regions to exclude from visual regression comparison.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MaskSet {
    pub regions: Vec<RectRegion>,
}

impl MaskSet {
    pub fn new() -> Self {
        Self {
            regions: Vec::new(),
        }
    }

    pub fn with_rect(mut self, x: u32, y: u32, width: u32, height: u32) -> Self {
        self.regions.push(RectRegion::new(x, y, width, height));
        self
    }

    pub fn add_region(&mut self, region: RectRegion) {
        self.regions.push(region);
    }

    /// Checks if pixel (px, py) is contained in any active mask region.
    #[inline]
    pub fn is_masked(&self, px: u32, py: u32) -> bool {
        for region in &self.regions {
            if region.contains(px, py) {
                return true;
            }
        }
        false
    }

    pub fn is_empty(&self) -> bool {
        self.regions.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rect_region_containment() {
        let rect = RectRegion::new(10, 20, 100, 50);
        assert!(rect.contains(10, 20));
        assert!(rect.contains(50, 45));
        assert!(rect.contains(109, 69));
        assert!(!rect.contains(110, 70));
        assert!(!rect.contains(9, 20));
        assert!(!rect.contains(50, 19));
    }

    #[test]
    fn test_mask_set_multiple() {
        let masks = MaskSet::new()
            .with_rect(0, 0, 10, 10)
            .with_rect(100, 100, 50, 50);

        assert!(masks.is_masked(5, 5));
        assert!(masks.is_masked(120, 120));
        assert!(!masks.is_masked(50, 50));
    }
}
