//! Unicode 13 subcell octant glyph micro-rendering and smootherstep animations.
//!
//! Provides a 2×4 intra-cell subpixel canvas for high-density audio waveforms,
//! progress graphs, and 20-FPS smootherstep pulse shaders.

/// Maps an 8-bit mask to a Unicode 13 Symbols for Legacy Computing octant glyph
/// or standard terminal block element fallbacks.
///
/// Bit layout for 2×4 intra-cell subpixel matrix:
/// ```text
/// ┌───┬───┐
/// │ 0 │ 4 │
/// ├───┼───┤
/// │ 1 │ 5 │
/// ├───┼───┤
/// │ 2 │ 6 │
/// ├───┼───┤
/// │ 3 │ 7 │
/// └───┴───┘
/// ```
pub fn octant_glyph(mask: u8) -> char {
    match mask {
        0 => ' ',
        0b0000_1111 => '▌',
        0b1111_0000 => '▐',
        0b0011_0011 => '▄',
        0b1100_1100 => '▀',
        0b1111_1111 => '█',
        _ => char::from_u32(0x1CD00 + mask as u32).unwrap_or('·'),
    }
}

/// A 2×4 subpixel canvas rendered into terminal character cells.
#[derive(Debug, Clone, PartialEq)]
pub struct SubcellCanvas {
    width_cells: usize,
    height_cells: usize,
    /// 2D grid of 8-bit octant masks indexed by `[row_cell][col_cell]`
    cells: Vec<Vec<u8>>,
}

impl SubcellCanvas {
    pub fn new(width_cells: usize, height_cells: usize) -> Self {
        Self {
            width_cells,
            height_cells,
            cells: vec![vec![0; width_cells]; height_cells],
        }
    }

    pub fn pixel_width(&self) -> usize {
        self.width_cells * 2
    }

    pub fn pixel_height(&self) -> usize {
        self.height_cells * 4
    }

    pub fn set_pixel(&mut self, x: usize, y: usize, on: bool) {
        if x >= self.pixel_width() || y >= self.pixel_height() {
            return;
        }

        let cell_x = x / 2;
        let cell_y = y / 4;
        let sub_x = x % 2;
        let sub_y = y % 4;

        let bit_index = sub_x * 4 + sub_y;
        if on {
            self.cells[cell_y][cell_x] |= 1 << bit_index;
        } else {
            self.cells[cell_y][cell_x] &= !(1 << bit_index);
        }
    }

    /// Clears the canvas.
    pub fn clear(&mut self) {
        for row in &mut self.cells {
            row.fill(0);
        }
    }

    /// Plots a normalized waveform (values 0.0 to 1.0) into the subcell canvas.
    pub fn plot_waveform(&mut self, samples: &[f32]) {
        self.clear();
        let max_x = self.pixel_width().min(samples.len());
        let h = self.pixel_height();

        for (x, &sample) in samples.iter().take(max_x).enumerate() {
            let val = sample.clamp(0.0, 1.0);
            let bar_height = (val * (h as f32)).round() as usize;
            for y_from_bottom in 0..bar_height {
                let y = h.saturating_sub(1).saturating_sub(y_from_bottom);
                self.set_pixel(x, y, true);
            }
        }
    }

    /// Renders the subcell canvas into a vector of character lines.
    pub fn render_to_lines(&self) -> Vec<String> {
        let mut lines = Vec::with_capacity(self.height_cells);
        for row in &self.cells {
            let line: String = row.iter().map(|&mask| octant_glyph(mask)).collect();
            lines.push(line);
        }
        lines
    }
}

/// Ken Perlin's smootherstep polynomial `6x^5 - 15x^4 + 10x^3` clamped between 0.0 and 1.0.
pub fn smootherstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

/// Calculates a 20-FPS clamped pulse factor [0.0..1.0] for status icons and active tabs.
pub fn tab_pulse_factor(frame_tick: u64) -> f32 {
    // 20 ticks per second cycle = 1.0 Hz pulse
    let phase = (frame_tick % 20) as f32 / 20.0;
    if phase < 0.5 {
        smootherstep(0.0, 0.5, phase)
    } else {
        smootherstep(1.0, 0.5, phase)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_octant_glyph_known_masks() {
        assert_eq!(octant_glyph(0), ' ');
        assert_eq!(octant_glyph(0b0000_1111), '▌');
        assert_eq!(octant_glyph(0b1111_0000), '▐');
        assert_eq!(octant_glyph(0b0011_0011), '▄');
        assert_eq!(octant_glyph(0b1100_1100), '▀');
        assert_eq!(octant_glyph(0b1111_1111), '█');
    }

    #[test]
    fn test_subcell_canvas_rendering() {
        let mut canvas = SubcellCanvas::new(2, 1);
        assert_eq!(canvas.pixel_width(), 4);
        assert_eq!(canvas.pixel_height(), 4);

        // Turn on all pixels on left cell
        for y in 0..4 {
            canvas.set_pixel(0, y, true);
            canvas.set_pixel(1, y, true);
        }
        let lines = canvas.render_to_lines();
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].chars().next(), Some('█'));
        assert_eq!(lines[0].chars().nth(1), Some(' '));
    }

    #[test]
    fn test_smootherstep_clamping() {
        assert_eq!(smootherstep(0.0, 1.0, -0.5), 0.0);
        assert_eq!(smootherstep(0.0, 1.0, 0.0), 0.0);
        assert_eq!(smootherstep(0.0, 1.0, 0.5), 0.5);
        assert_eq!(smootherstep(0.0, 1.0, 1.0), 1.0);
        assert_eq!(smootherstep(0.0, 1.0, 1.5), 1.0);
    }
}
