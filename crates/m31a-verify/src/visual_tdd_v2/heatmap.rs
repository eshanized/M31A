//! Visual Difference Heatmap Generation.
//!
//! Synthesizes high-contrast visual difference heatmaps overlaid on baseline imagery,
//! outlining regression bounding boxes for immediate developer comprehension.

use super::comparator::{ComparisonOutcome, DiffBoundingBox};
use anyhow::{Context, Result};
use image::{ImageFormat, Rgba, RgbaImage};
use std::io::Cursor;
use std::path::Path;

/// Options for styling the generated visual difference heatmap.
#[derive(Debug, Clone)]
pub struct HeatmapOptions {
    /// Bounding box outline color
    pub box_border_color: Rgba<u8>,
    /// Thickness in pixels for bounding box outline
    pub border_thickness: u32,
}

impl Default for HeatmapOptions {
    fn default() -> Self {
        Self {
            box_border_color: Rgba([255, 140, 0, 255]), // Vivid amber/orange
            border_thickness: 2,
        }
    }
}

pub struct VisualHeatmap;

impl VisualHeatmap {
    /// Overlays annotated bounding boxes onto the diff image and returns the final heatmap.
    pub fn annotate_heatmap(
        mut diff_image: RgbaImage,
        outcome: &ComparisonOutcome,
        options: &HeatmapOptions,
    ) -> RgbaImage {
        for bbox in &outcome.bounding_boxes {
            Self::draw_bounding_box(&mut diff_image, bbox, options);
        }
        diff_image
    }

    /// Draws a rectangular border around the given bounding box.
    fn draw_bounding_box(image: &mut RgbaImage, bbox: &DiffBoundingBox, options: &HeatmapOptions) {
        let (w, h) = image.dimensions();
        let border = options.border_thickness;
        let color = options.box_border_color;

        let min_x = bbox.min_x.saturating_sub(border);
        let max_x = (bbox.max_x + border).min(w.saturating_sub(1));
        let min_y = bbox.min_y.saturating_sub(border);
        let max_y = (bbox.max_y + border).min(h.saturating_sub(1));

        // Horizontal borders (top and bottom)
        for t in 0..border {
            let top_y = min_y + t;
            let bottom_y = max_y.saturating_sub(t);

            for x in min_x..=max_x {
                if top_y < h {
                    image.put_pixel(x, top_y, color);
                }
                if bottom_y < h {
                    image.put_pixel(x, bottom_y, color);
                }
            }
        }

        // Vertical borders (left and right)
        for t in 0..border {
            let left_x = min_x + t;
            let right_x = max_x.saturating_sub(t);

            for y in min_y..=max_y {
                if left_x < w {
                    image.put_pixel(left_x, y, color);
                }
                if right_x < w {
                    image.put_pixel(right_x, y, color);
                }
            }
        }
    }

    /// Saves heatmap image to a PNG file on disk.
    pub fn save_heatmap(heatmap: &RgbaImage, destination: &Path) -> Result<()> {
        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent).with_context(|| {
                format!("Failed to create directory for heatmap at {:?}", parent)
            })?;
        }
        heatmap
            .save_with_format(destination, ImageFormat::Png)
            .with_context(|| format!("Failed to write heatmap PNG to {:?}", destination))?;
        Ok(())
    }

    /// Encodes heatmap image to in-memory PNG bytes.
    pub fn to_png_bytes(heatmap: &RgbaImage) -> Result<Vec<u8>> {
        let mut bytes = Vec::new();
        let mut cursor = Cursor::new(&mut bytes);
        heatmap
            .write_to(&mut cursor, ImageFormat::Png)
            .context("Failed to encode heatmap to PNG buffer")?;
        Ok(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_heatmap_annotation_and_save() {
        let base = RgbaImage::from_pixel(100, 100, Rgba([30, 30, 30, 255]));
        let outcome = ComparisonOutcome {
            passed: false,
            width: 100,
            height: 100,
            total_evaluated_pixels: 10000,
            masked_pixels: 0,
            mismatched_pixels: 100,
            mismatch_ratio: 0.01,
            max_color_distance: 0.8,
            mean_color_distance: 0.008,
            bounding_boxes: vec![DiffBoundingBox {
                min_x: 20,
                min_y: 20,
                max_x: 40,
                max_y: 40,
            }],
            dimension_mismatch: false,
            error_details: None,
        };

        let annotated = VisualHeatmap::annotate_heatmap(base, &outcome, &HeatmapOptions::default());
        // Verify border pixel on top-left outer border has border color
        let border_pixel = annotated.get_pixel(18, 18);
        assert_eq!(*border_pixel, Rgba([255, 140, 0, 255]));

        let bytes = VisualHeatmap::to_png_bytes(&annotated).unwrap();
        assert!(!bytes.is_empty());
        assert_eq!(&bytes[1..4], b"PNG");
    }
}
