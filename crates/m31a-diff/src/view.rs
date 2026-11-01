//! Interactive scrollable diff viewer state and navigation.

use crate::engine::{DiffEngine, DiffLine, DiffStats};

#[derive(Debug, Clone)]
pub struct DiffViewerState {
    pub file_path: String,
    pub lines: Vec<DiffLine>,
    pub stats: DiffStats,
    pub scroll_offset: usize,
    pub max_scroll: usize,
}

impl DiffViewerState {
    pub fn new(file_path: impl Into<String>, old_text: &str, new_text: &str) -> Self {
        let (lines, stats) = DiffEngine::compute_diff(old_text, new_text);
        let max_scroll = lines.len().saturating_sub(1);
        Self {
            file_path: file_path.into(),
            lines,
            stats,
            scroll_offset: 0,
            max_scroll,
        }
    }

    pub fn from_unified_patch(file_path: impl Into<String>, patch: &str) -> Self {
        let (lines, stats) = DiffEngine::parse_unified_diff(patch);
        let max_scroll = lines.len().saturating_sub(1);
        Self {
            file_path: file_path.into(),
            lines,
            stats,
            scroll_offset: 0,
            max_scroll,
        }
    }

    pub fn scroll_up(&mut self, amount: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(amount);
    }

    pub fn scroll_down(&mut self, amount: usize) {
        self.scroll_offset = (self.scroll_offset + amount).min(self.max_scroll);
    }

    pub fn scroll_page_up(&mut self) {
        self.scroll_up(15);
    }

    pub fn scroll_page_down(&mut self) {
        self.scroll_down(15);
    }
}
