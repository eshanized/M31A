//! Scrollable, wrapped diff and log viewer for terminal review.
//!
//! Scrollable diff and log viewer with viewport clamping and keyboard navigation:
//! - Caches wrapped lines for arbitrary terminal widths.
//! - Viewport clamping to prevent out-of-bounds scrolling.
//! - Keyboard navigation: scroll up/down, page up/down, jump to top/bottom.

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ScrollViewState {
    pub scroll: u16,
    pub viewport_h: u16,
    pub content_h: u16,
}

impl ScrollViewState {
    pub fn clamp(&mut self) {
        let max_scroll = self.content_h.saturating_sub(self.viewport_h);
        if self.scroll > max_scroll {
            self.scroll = max_scroll;
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct ScrollableDiff {
    raw: Vec<String>,
    wrapped: Vec<String>,
    wrap_cols: Option<u16>,
    pub state: ScrollViewState,
}

impl ScrollableDiff {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_content(&mut self, lines: Vec<String>) {
        self.raw = lines;
        self.wrapped.clear();
        self.state.content_h = 0;
        self.wrap_cols = None;
    }

    pub fn set_width(&mut self, width: u16) {
        if self.wrap_cols == Some(width) {
            return;
        }
        self.wrap_cols = Some(width);
        self.rewrap(width);
        self.state.clamp();
    }

    pub fn set_viewport(&mut self, height: u16) {
        self.state.viewport_h = height;
        self.state.clamp();
    }

    pub fn scroll_up(&mut self, delta: u16) {
        self.state.scroll = self.state.scroll.saturating_sub(delta);
    }

    pub fn scroll_down(&mut self, delta: u16) {
        self.state.scroll = self.state.scroll.saturating_add(delta);
        self.state.clamp();
    }

    pub fn scroll_top(&mut self) {
        self.state.scroll = 0;
    }

    pub fn scroll_bottom(&mut self) {
        self.state.scroll = self.state.content_h.saturating_sub(self.state.viewport_h);
    }

    pub fn visible_lines(&self) -> &[String] {
        if self.wrapped.is_empty() {
            return &[];
        }
        let start = self.state.scroll as usize;
        let count = self.state.viewport_h as usize;
        let end = (start + count).min(self.wrapped.len());
        if start >= self.wrapped.len() {
            &[]
        } else {
            &self.wrapped[start..end]
        }
    }

    pub fn total_lines(&self) -> usize {
        self.wrapped.len()
    }

    fn rewrap(&mut self, width: u16) {
        self.wrapped.clear();
        if width == 0 {
            self.wrapped = self.raw.clone();
            self.state.content_h = self.wrapped.len() as u16;
            return;
        }

        let width_usize = width as usize;
        for line in &self.raw {
            if line.len() <= width_usize {
                self.wrapped.push(line.clone());
            } else {
                let mut current = line.as_str();
                while !current.is_empty() {
                    if current.len() <= width_usize {
                        self.wrapped.push(current.to_string());
                        break;
                    } else {
                        let split_idx = floor_char_boundary(current, width_usize);
                        self.wrapped.push(current[..split_idx].to_string());
                        current = &current[split_idx..];
                    }
                }
            }
        }
        self.state.content_h = self.wrapped.len() as u16;
    }
}

fn floor_char_boundary(s: &str, mut index: usize) -> usize {
    if index >= s.len() {
        return s.len();
    }
    while !s.is_char_boundary(index) {
        index = index.saturating_sub(1);
    }
    if index == 0 { 1 } else { index }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scrollable_diff_wrapping_and_clamping() {
        let mut diff = ScrollableDiff::new();
        diff.set_content(vec![
            "line 1: short".to_string(),
            "line 2: a very long unified diff line that exceeds twenty columns easily".to_string(),
            "line 3: end".to_string(),
        ]);

        diff.set_viewport(4);
        diff.set_width(20);

        assert!(diff.total_lines() > 3);
        assert_eq!(diff.visible_lines().len(), 4);

        diff.scroll_down(10);
        let max_scroll = diff.state.content_h.saturating_sub(diff.state.viewport_h);
        assert_eq!(diff.state.scroll, max_scroll);

        diff.scroll_top();
        assert_eq!(diff.state.scroll, 0);
    }
}
