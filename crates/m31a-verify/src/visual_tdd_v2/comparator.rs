//! Pixel-Level and Perceptual Screenshot Comparison Engine.
//!
//! Evaluates image differences using exact pixel comparison and weighted perceptual
//! color distance, supporting configurable thresholds and dynamic region masks.

use super::masking::MaskSet;
use anyhow::{Context, Result};
use image::{ImageBuffer, Rgba, RgbaImage};
use serde::{Deserialize, Serialize};

/// Bounding box enclosing a detected visual regression area.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiffBoundingBox {
    pub min_x: u32,
    pub min_y: u32,
    pub max_x: u32,
    pub max_y: u32,
}

impl DiffBoundingBox {
    pub fn new(x: u32, y: u32) -> Self {
        Self {
            min_x: x,
            min_y: y,
            max_x: x,
            max_y: y,
        }
    }

    pub fn expand_to_include(&mut self, x: u32, y: u32) {
        if x < self.min_x {
            self.min_x = x;
        }
        if x > self.max_x {
            self.max_x = x;
        }
        if y < self.min_y {
            self.min_y = y;
        }
        if y > self.max_y {
            self.max_y = y;
        }
    }

    pub fn width(&self) -> u32 {
        self.max_x.saturating_sub(self.min_x).saturating_add(1)
    }

    pub fn height(&self) -> u32 {
        self.max_y.saturating_sub(self.min_y).saturating_add(1)
    }

    pub fn area(&self) -> u64 {
        self.width() as u64 * self.height() as u64
    }
}

/// Comparison configuration and tolerance thresholds.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComparisonThreshold {
    /// Maximum allowable fraction of mismatched pixels before failing (0.001 = 0.1%)
    pub max_mismatch_ratio: f64,
    /// Color distance tolerance in [0.0, 1.0] below which a pixel delta is ignored (e.g. anti-aliasing)
    pub color_tolerance: f64,
    /// Whether alpha channel differences are evaluated
    pub check_alpha: bool,
}

impl Default for ComparisonThreshold {
    fn default() -> Self {
        Self {
            max_mismatch_ratio: 0.0005, // 0.05% threshold
            color_tolerance: 0.04,      // 4% perceptual delta
            check_alpha: true,
        }
    }
}

impl ComparisonThreshold {
    pub fn strict() -> Self {
        Self {
            max_mismatch_ratio: 0.0,
            color_tolerance: 0.0,
            check_alpha: true,
        }
    }

    pub fn relaxed() -> Self {
        Self {
            max_mismatch_ratio: 0.01, // 1% threshold
            color_tolerance: 0.08,    // 8% perceptual delta
            check_alpha: false,
        }
    }
}

/// Detailed outcome of a screenshot comparison.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComparisonOutcome {
    pub passed: bool,
    pub width: u32,
    pub height: u32,
    pub total_evaluated_pixels: u64,
    pub masked_pixels: u64,
    pub mismatched_pixels: u64,
    pub mismatch_ratio: f64,
    pub max_color_distance: f64,
    pub mean_color_distance: f64,
    pub bounding_boxes: Vec<DiffBoundingBox>,
    pub dimension_mismatch: bool,
    pub error_details: Option<String>,
}

pub struct VisualComparator;

impl VisualComparator {
    /// Computes weighted perceptual color distance in Rec. 709 luminance space:
    /// Returns value in range [0.0, 1.0].
    pub fn perceptual_distance(c1: &Rgba<u8>, c2: &Rgba<u8>, check_alpha: bool) -> f64 {
        let dr = (c1[0] as f64 - c2[0] as f64) / 255.0;
        let dg = (c1[1] as f64 - c2[1] as f64) / 255.0;
        let db = (c1[2] as f64 - c2[2] as f64) / 255.0;

        let rgb_dist_sq = 0.299 * (dr * dr) + 0.587 * (dg * dg) + 0.114 * (db * db);

        if check_alpha {
            let da = (c1[3] as f64 - c2[3] as f64) / 255.0;
            (rgb_dist_sq + 0.1 * (da * da)).sqrt().min(1.0)
        } else {
            rgb_dist_sq.sqrt().min(1.0)
        }
    }

    /// Compares two images loaded from raw bytes with masking and thresholds.
    pub fn compare_bytes(
        baseline_bytes: &[u8],
        actual_bytes: &[u8],
        masks: &MaskSet,
        threshold: &ComparisonThreshold,
    ) -> Result<(ComparisonOutcome, RgbaImage)> {
        let baseline_img = image::load_from_memory(baseline_bytes)
            .context("Failed to decode baseline image bytes")?
            .to_rgba8();
        let actual_img = image::load_from_memory(actual_bytes)
            .context("Failed to decode actual image bytes")?
            .to_rgba8();

        Self::compare_images(&baseline_img, &actual_img, masks, threshold)
    }

    /// Compares two RGBA images and generates an amplified diff mask image.
    pub fn compare_images(
        baseline: &RgbaImage,
        actual: &RgbaImage,
        masks: &MaskSet,
        threshold: &ComparisonThreshold,
    ) -> Result<(ComparisonOutcome, RgbaImage)> {
        let (b_w, b_h) = baseline.dimensions();
        let (a_w, a_h) = actual.dimensions();

        let max_w = b_w.max(a_w);
        let max_h = b_h.max(a_h);

        let dimension_mismatch = (b_w != a_w) || (b_h != a_h);

        let mut diff_img: RgbaImage = ImageBuffer::new(max_w, max_h);

        let mut total_pixels = 0u64;
        let mut masked_pixels = 0u64;
        let mut mismatched_pixels = 0u64;
        let mut sum_color_dist = 0.0f64;
        let mut max_color_dist = 0.0f64;

        let mut bounding_box: Option<DiffBoundingBox> = None;

        for y in 0..max_h {
            for x in 0..max_w {
                if masks.is_masked(x, y) {
                    masked_pixels += 1;
                    // Draw dim blue/gray for masked regions in diff view
                    diff_img.put_pixel(x, y, Rgba([40, 60, 90, 180]));
                    continue;
                }

                total_pixels += 1;

                let b_pixel = if x < b_w && y < b_h {
                    Some(baseline.get_pixel(x, y))
                } else {
                    None
                };

                let a_pixel = if x < a_w && y < a_h {
                    Some(actual.get_pixel(x, y))
                } else {
                    None
                };

                match (b_pixel, a_pixel) {
                    (Some(b), Some(a)) => {
                        let dist = Self::perceptual_distance(b, a, threshold.check_alpha);
                        sum_color_dist += dist;
                        if dist > max_color_dist {
                            max_color_dist = dist;
                        }

                        if dist > threshold.color_tolerance {
                            mismatched_pixels += 1;
                            // Update bounding box
                            match bounding_box {
                                Some(ref mut b) => b.expand_to_include(x, y),
                                None => bounding_box = Some(DiffBoundingBox::new(x, y)),
                            }

                            // Vivid magenta/red for regression
                            let intensity = (dist * 255.0).clamp(140.0, 255.0) as u8;
                            diff_img.put_pixel(x, y, Rgba([intensity, 0, intensity / 2, 255]));
                        } else {
                            // Dimmed baseline for background
                            let gray = (0.299 * b[0] as f64
                                + 0.587 * b[1] as f64
                                + 0.114 * b[2] as f64) as u8;
                            let dim_gray = gray / 3;
                            diff_img.put_pixel(x, y, Rgba([dim_gray, dim_gray, dim_gray, 255]));
                        }
                    }
                    _ => {
                        // Dimension overflow area: mark as bright yellow diff
                        mismatched_pixels += 1;
                        sum_color_dist += 1.0;
                        max_color_dist = 1.0;
                        match bounding_box {
                            Some(ref mut b) => b.expand_to_include(x, y),
                            None => bounding_box = Some(DiffBoundingBox::new(x, y)),
                        }
                        diff_img.put_pixel(x, y, Rgba([255, 200, 0, 255]));
                    }
                }
            }
        }

        let mismatch_ratio = if total_pixels > 0 {
            mismatched_pixels as f64 / total_pixels as f64
        } else {
            0.0
        };

        let mean_color_distance = if total_pixels > 0 {
            sum_color_dist / total_pixels as f64
        } else {
            0.0
        };

        let passed = !dimension_mismatch && (mismatch_ratio <= threshold.max_mismatch_ratio);

        let mut bounding_boxes = Vec::new();
        if let Some(b) = bounding_box {
            bounding_boxes.push(b);
        }

        let error_details = if !passed {
            if dimension_mismatch {
                Some(format!(
                    "Image dimension mismatch: baseline is {}x{}, but actual screenshot is {}x{}",
                    b_w, b_h, a_w, a_h
                ))
            } else {
                Some(format!(
                    "Visual mismatch ratio {:.4}% exceeded threshold {:.4}% ({} differing pixels, max ΔC={:.3})",
                    mismatch_ratio * 100.0,
                    threshold.max_mismatch_ratio * 100.0,
                    mismatched_pixels,
                    max_color_dist
                ))
            }
        } else {
            None
        };

        Ok((
            ComparisonOutcome {
                passed,
                width: max_w,
                height: max_h,
                total_evaluated_pixels: total_pixels,
                masked_pixels,
                mismatched_pixels,
                mismatch_ratio,
                max_color_distance: max_color_dist,
                mean_color_distance,
                bounding_boxes,
                dimension_mismatch,
                error_details,
            },
            diff_img,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identical_images_pass_strictly() {
        let img1 = RgbaImage::from_pixel(100, 100, Rgba([255, 255, 255, 255]));
        let img2 = RgbaImage::from_pixel(100, 100, Rgba([255, 255, 255, 255]));

        let (outcome, _) = VisualComparator::compare_images(
            &img1,
            &img2,
            &MaskSet::new(),
            &ComparisonThreshold::strict(),
        )
        .unwrap();

        assert!(outcome.passed);
        assert_eq!(outcome.mismatched_pixels, 0);
        assert_eq!(outcome.mismatch_ratio, 0.0);
    }

    #[test]
    fn test_differing_images_detected() {
        let img1 = RgbaImage::from_pixel(100, 100, Rgba([255, 255, 255, 255]));
        let mut img2 = RgbaImage::from_pixel(100, 100, Rgba([255, 255, 255, 255]));

        // Paint 10x10 black square in center (100 pixels)
        for y in 45..55 {
            for x in 45..55 {
                img2.put_pixel(x, y, Rgba([0, 0, 0, 255]));
            }
        }

        let (outcome, _) = VisualComparator::compare_images(
            &img1,
            &img2,
            &MaskSet::new(),
            &ComparisonThreshold::strict(),
        )
        .unwrap();

        assert!(!outcome.passed);
        assert_eq!(outcome.mismatched_pixels, 100);
        assert_eq!(outcome.total_evaluated_pixels, 10000);
        assert!((outcome.mismatch_ratio - 0.01).abs() < 1e-6);
        assert_eq!(outcome.bounding_boxes.len(), 1);
        assert_eq!(outcome.bounding_boxes[0].min_x, 45);
        assert_eq!(outcome.bounding_boxes[0].max_x, 54);
    }

    #[test]
    fn test_masked_region_ignored() {
        let img1 = RgbaImage::from_pixel(100, 100, Rgba([255, 255, 255, 255]));
        let mut img2 = RgbaImage::from_pixel(100, 100, Rgba([255, 255, 255, 255]));

        // Paint 10x10 black square in center
        for y in 45..55 {
            for x in 45..55 {
                img2.put_pixel(x, y, Rgba([0, 0, 0, 255]));
            }
        }

        // Mask the 10x10 square
        let masks = MaskSet::new().with_rect(45, 45, 10, 10);

        let (outcome, _) = VisualComparator::compare_images(
            &img1,
            &img2,
            &masks,
            &ComparisonThreshold::strict(),
        )
        .unwrap();

        assert!(outcome.passed);
        assert_eq!(outcome.mismatched_pixels, 0);
        assert_eq!(outcome.masked_pixels, 100);
    }
}
