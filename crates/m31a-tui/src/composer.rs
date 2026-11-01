//! Canonical authoritative ChatComposer managing text buffers, grapheme-aware cursor movement,
//! multiline editing, viewport scrolling, undo/redo history, and autocomplete popups.

pub use m31a_composer::slash::{
    BUILTIN_SLASH_COMMANDS, CANONICAL_SLASH_COMMANDS, SlashCommand, find_command,
    matching_commands, tokenize_slash_command,
};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

pub type SlashCommandInfo = SlashCommand;
pub const SLASH_COMMANDS: &[SlashCommand] = CANONICAL_SLASH_COMMANDS;

/// Canonical authoritative text composer for terminal user interactions.
#[derive(Debug, Clone)]
pub struct ChatComposer {
    /// Authoritative text content.
    pub buffer: String,
    /// Byte offset in `buffer`, guaranteed to align with a grapheme cluster boundary.
    pub cursor: usize,
    /// Selection byte range [start, end) if active.
    pub selection: Option<(usize, usize)>,
    /// History of previously submitted prompts.
    pub history: Vec<String>,
    /// Current navigation index into history (None = current draft).
    pub history_index: Option<usize>,
    /// Temporary storage for draft text when navigating history.
    pub temp_buffer: String,
    /// Undo history recording (buffer_snapshot, cursor_offset).
    pub undo_stack: Vec<(String, usize)>,
    /// Redo history recording (buffer_snapshot, cursor_offset).
    pub redo_stack: Vec<(String, usize)>,
    /// Top visible line offset for multiline scroll viewport.
    pub viewport_offset: usize,
    /// Active candidate popup selection index.
    pub popup_index: usize,
}

impl Default for ChatComposer {
    fn default() -> Self {
        Self::new()
    }
}

impl ChatComposer {
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
            cursor: 0,
            selection: None,
            history: Vec::new(),
            history_index: None,
            temp_buffer: String::new(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            viewport_offset: 0,
            popup_index: 0,
        }
    }

    /// Records current state for undo before a mutating operation.
    fn push_undo(&mut self) {
        if self.undo_stack.len() >= 100 {
            self.undo_stack.remove(0);
        }
        self.undo_stack.push((self.buffer.clone(), self.cursor));
        self.redo_stack.clear();
    }

    /// Undoes the last editing action. Returns true if undo succeeded.
    pub fn undo(&mut self) -> bool {
        if let Some((prev_buf, prev_cursor)) = self.undo_stack.pop() {
            self.redo_stack.push((self.buffer.clone(), self.cursor));
            self.buffer = prev_buf;
            self.cursor = prev_cursor.min(self.buffer.len());
            self.ensure_cursor_boundary();
            self.popup_index = 0;
            true
        } else {
            false
        }
    }

    /// Redoes the previously undone editing action. Returns true if redo succeeded.
    pub fn redo(&mut self) -> bool {
        if let Some((next_buf, next_cursor)) = self.redo_stack.pop() {
            self.undo_stack.push((self.buffer.clone(), self.cursor));
            self.buffer = next_buf;
            self.cursor = next_cursor.min(self.buffer.len());
            self.ensure_cursor_boundary();
            self.popup_index = 0;
            true
        } else {
            false
        }
    }

    /// Ensures that the cursor byte offset is always on a valid UTF-8 and grapheme boundary.
    fn ensure_cursor_boundary(&mut self) {
        if self.cursor >= self.buffer.len() {
            self.cursor = self.buffer.len();
            return;
        }
        if !self.buffer.is_char_boundary(self.cursor) {
            // Find closest char boundary <= cursor
            let mut valid = self.cursor;
            while valid > 0 && !self.buffer.is_char_boundary(valid) {
                valid -= 1;
            }
            self.cursor = valid;
        }
        // Snap to nearest preceding grapheme cluster boundary
        let mut best_boundary = 0;
        for (byte_idx, _) in self.buffer.grapheme_indices(true) {
            if byte_idx > self.cursor {
                break;
            }
            best_boundary = byte_idx;
        }
        self.cursor = best_boundary;
    }

    /// Inserts a single unicode character at the current cursor position.
    pub fn insert_char(&mut self, c: char) {
        self.push_undo();
        self.ensure_cursor_boundary();
        if self.cursor >= self.buffer.len() {
            self.buffer.push(c);
            self.cursor = self.buffer.len();
        } else {
            self.buffer.insert(self.cursor, c);
            self.cursor += c.len_utf8();
        }
        self.popup_index = 0;
    }

    /// Inserts a string slice at the current cursor position.
    pub fn insert_str(&mut self, s: &str) {
        if s.is_empty() {
            return;
        }
        self.push_undo();
        self.ensure_cursor_boundary();
        if self.cursor >= self.buffer.len() {
            self.buffer.push_str(s);
            self.cursor = self.buffer.len();
        } else {
            self.buffer.insert_str(self.cursor, s);
            self.cursor += s.len();
        }
        self.popup_index = 0;
    }

    /// Inserts a newline at the cursor position.
    pub fn on_newline(&mut self) {
        self.insert_char('\n');
    }

    /// Deletes the preceding grapheme cluster (backspace).
    pub fn delete_backspace(&mut self) {
        self.ensure_cursor_boundary();
        if self.cursor == 0 {
            return;
        }
        self.push_undo();
        // Find preceding grapheme cluster boundary
        let mut prev_boundary = 0;
        for (idx, _) in self.buffer.grapheme_indices(true) {
            if idx >= self.cursor {
                break;
            }
            prev_boundary = idx;
        }
        self.buffer.drain(prev_boundary..self.cursor);
        self.cursor = prev_boundary;
        self.popup_index = 0;
    }

    /// Deletes the succeeding grapheme cluster (delete forward).
    pub fn delete_forward(&mut self) {
        self.ensure_cursor_boundary();
        if self.cursor >= self.buffer.len() {
            return;
        }
        self.push_undo();
        let next_boundary = self.buffer[self.cursor..]
            .grapheme_indices(true)
            .nth(1)
            .map(|(idx, _)| self.cursor + idx)
            .unwrap_or(self.buffer.len());

        self.buffer.drain(self.cursor..next_boundary);
        self.popup_index = 0;
    }

    /// Moves cursor left by one grapheme cluster.
    pub fn move_cursor_left(&mut self) {
        self.ensure_cursor_boundary();
        if self.cursor == 0 {
            return;
        }
        let mut prev_boundary = 0;
        for (idx, _) in self.buffer.grapheme_indices(true) {
            if idx >= self.cursor {
                break;
            }
            prev_boundary = idx;
        }
        self.cursor = prev_boundary;
    }

    /// Moves cursor right by one grapheme cluster.
    pub fn move_cursor_right(&mut self) {
        self.ensure_cursor_boundary();
        if self.cursor >= self.buffer.len() {
            return;
        }
        let next_boundary = self.buffer[self.cursor..]
            .grapheme_indices(true)
            .nth(1)
            .map(|(idx, _)| self.cursor + idx)
            .unwrap_or(self.buffer.len());

        self.cursor = next_boundary;
    }

    /// Moves cursor to the beginning of the current line (or buffer if single line).
    pub fn move_cursor_home(&mut self) {
        self.ensure_cursor_boundary();
        let before = &self.buffer[..self.cursor];
        if let Some(pos) = before.rfind('\n') {
            self.cursor = pos + 1;
        } else {
            self.cursor = 0;
        }
    }

    /// Moves cursor to the end of the current line (or buffer if single line).
    pub fn move_cursor_end(&mut self) {
        self.ensure_cursor_boundary();
        let after = &self.buffer[self.cursor..];
        if let Some(pos) = after.find('\n') {
            self.cursor += pos;
        } else {
            self.cursor = self.buffer.len();
        }
    }

    /// Checks if the composer text buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    /// Returns byte length of the text buffer.
    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    /// Clears the composer text and resets cursor and history index.
    pub fn clear(&mut self) {
        self.buffer.clear();
        self.cursor = 0;
        self.selection = None;
        self.history_index = None;
        self.viewport_offset = 0;
        self.popup_index = 0;
        self.undo_stack.clear();
        self.redo_stack.clear();
    }

    /// Replaces the current composer text and moves cursor to the end.
    pub fn set_text(&mut self, text: &str) {
        self.buffer = text.to_string();
        self.cursor = self.buffer.len();
        self.history_index = None;
        self.viewport_offset = 0;
        self.popup_index = 0;
    }

    /// Submits the current prompt, saving non-empty trimmed text to history, and clears the composer.
    pub fn submit(&mut self) -> String {
        let text = self.buffer.trim().to_string();
        if !text.is_empty() && self.history.last().map(|s| s.as_str()) != Some(&text) {
            self.history.push(text.clone());
        }
        self.clear();
        text
    }

    /// Navigates to previous prompt in history.
    pub fn history_prev(&mut self) {
        if self.history.is_empty() {
            return;
        }
        match self.history_index {
            None => {
                self.temp_buffer = self.buffer.clone();
                let new_idx = self.history.len() - 1;
                self.history_index = Some(new_idx);
                self.buffer = self.history[new_idx].clone();
                self.cursor = self.buffer.len();
            }
            Some(idx) => {
                if idx > 0 {
                    let new_idx = idx - 1;
                    self.history_index = Some(new_idx);
                    self.buffer = self.history[new_idx].clone();
                    self.cursor = self.buffer.len();
                }
            }
        }
        self.viewport_offset = 0;
    }

    /// Navigates to next prompt in history, restoring the draft at the end.
    pub fn history_next(&mut self) {
        if let Some(idx) = self.history_index {
            if idx + 1 < self.history.len() {
                let new_idx = idx + 1;
                self.history_index = Some(new_idx);
                self.buffer = self.history[new_idx].clone();
                self.cursor = self.buffer.len();
            } else {
                self.history_index = None;
                self.buffer = self.temp_buffer.clone();
                self.cursor = self.buffer.len();
            }
        }
        self.viewport_offset = 0;
    }

    /// Returns (line_index, byte_column_in_line) for current cursor position.
    pub fn cursor_line_and_col(&self) -> (usize, usize) {
        let before_cursor = &self.buffer[..self.cursor.min(self.buffer.len())];
        let line_idx = before_cursor.matches('\n').count();
        let last_line = before_cursor.split('\n').next_back().unwrap_or("");
        (line_idx, last_line.len())
    }

    /// Returns display cell width of the cursor in its current line using `unicode_width`.
    pub fn cursor_display_col(&self) -> (usize, usize) {
        let before_cursor = &self.buffer[..self.cursor.min(self.buffer.len())];
        let line_idx = before_cursor.matches('\n').count();
        let last_line = before_cursor.split('\n').next_back().unwrap_or("");
        let display_width = UnicodeWidthStr::width(last_line);
        (line_idx, display_width)
    }

    /// Adjusts `viewport_offset` so that the line containing the cursor is always visible.
    pub fn update_viewport(&mut self, visible_lines: usize) {
        let (cursor_line, _) = self.cursor_line_and_col();
        if visible_lines == 0 {
            return;
        }
        if cursor_line < self.viewport_offset {
            self.viewport_offset = cursor_line;
        } else if cursor_line >= self.viewport_offset + visible_lines {
            self.viewport_offset = cursor_line + 1 - visible_lines;
        }
    }

    /// Moves cursor up one line, preserving terminal display cell column where possible.
    /// If at the first line, returns true to signal history navigation.
    pub fn move_up_or_history(&mut self) -> bool {
        let (line_idx, _) = self.cursor_line_and_col();
        if line_idx == 0 {
            self.history_prev();
            true
        } else {
            let (_, target_col_width) = self.cursor_display_col();
            let lines: Vec<&str> = self.buffer.split('\n').collect();
            let target_line_idx = line_idx - 1;
            let target_line = lines[target_line_idx];

            // Find grapheme boundary in target_line closest to target_col_width
            let mut best_byte_offset = 0;
            let mut current_width = 0;
            for (byte_offset, grapheme) in target_line.grapheme_indices(true) {
                let g_width = UnicodeWidthStr::width(grapheme);
                if current_width + g_width > target_col_width {
                    break;
                }
                current_width += g_width;
                best_byte_offset = byte_offset + grapheme.len();
            }

            // Compute absolute byte position
            let mut new_cursor = 0;
            for (idx, line) in lines.iter().enumerate() {
                if idx == target_line_idx {
                    new_cursor += best_byte_offset;
                    break;
                }
                new_cursor += line.len() + 1; // line + '\n'
            }
            self.cursor = new_cursor;
            false
        }
    }

    /// Moves cursor down one line, preserving terminal display cell column where possible.
    /// If at the last line, returns true to signal history navigation.
    pub fn move_down_or_history(&mut self) -> bool {
        let (line_idx, _) = self.cursor_line_and_col();
        let lines: Vec<&str> = self.buffer.split('\n').collect();
        if line_idx + 1 >= lines.len() {
            self.history_next();
            true
        } else {
            let (_, target_col_width) = self.cursor_display_col();
            let target_line_idx = line_idx + 1;
            let target_line = lines[target_line_idx];

            let mut best_byte_offset = 0;
            let mut current_width = 0;
            for (byte_offset, grapheme) in target_line.grapheme_indices(true) {
                let g_width = UnicodeWidthStr::width(grapheme);
                if current_width + g_width > target_col_width {
                    break;
                }
                current_width += g_width;
                best_byte_offset = byte_offset + grapheme.len();
            }

            let mut new_cursor = 0;
            for (idx, line) in lines.iter().enumerate() {
                if idx == target_line_idx {
                    new_cursor += best_byte_offset;
                    break;
                }
                new_cursor += line.len() + 1;
            }
            self.cursor = new_cursor;
            false
        }
    }

    /// Matches slash command candidates for autocomplete.
    pub fn matching_slash_commands(&self) -> Vec<&'static SlashCommandInfo> {
        let trimmed = self.buffer.trim();
        if trimmed.starts_with('/') {
            let filter = trimmed.split_whitespace().next().unwrap_or("/");
            matching_commands(filter)
        } else {
            Vec::new()
        }
    }

    /// Completes the currently selected slash command candidate.
    pub fn complete_slash_command(&mut self) -> bool {
        let matches = self.matching_slash_commands();
        if !matches.is_empty() {
            let selected = matches[self.popup_index % matches.len()];
            self.buffer = format!("{} ", selected.name);
            self.cursor = self.buffer.len();
            return true;
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_composer_typing_and_submitting() {
        let mut composer = ChatComposer::new();
        composer.insert_str("Hello World");
        assert_eq!(composer.buffer, "Hello World");
        assert_eq!(composer.cursor, 11);

        composer.delete_backspace();
        assert_eq!(composer.buffer, "Hello Worl");
        assert_eq!(composer.cursor, 10);

        let submitted = composer.submit();
        assert_eq!(submitted, "Hello Worl");
        assert_eq!(composer.buffer, "");
        assert_eq!(composer.history.len(), 1);
    }

    #[test]
    fn test_slash_command_matching_and_completion() {
        let mut composer = ChatComposer::new();
        composer.insert_str("/mode");
        let matches = composer.matching_slash_commands();
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].name, "/model");

        let completed = composer.complete_slash_command();
        assert!(completed);
        assert_eq!(composer.buffer, "/model ");
    }

    #[test]
    fn test_history_navigation() {
        let mut composer = ChatComposer::new();
        composer.insert_str("first prompt");
        composer.submit();
        composer.insert_str("second prompt");
        composer.submit();

        composer.history_prev();
        assert_eq!(composer.buffer, "second prompt");

        composer.history_prev();
        assert_eq!(composer.buffer, "first prompt");

        composer.history_next();
        assert_eq!(composer.buffer, "second prompt");

        composer.history_next();
        assert_eq!(composer.buffer, "");
    }

    #[test]
    fn test_multibyte_unicode_and_combining_characters() {
        let mut composer = ChatComposer::new();

        // 1. Multibyte Cyrillic
        composer.insert_str("Привет");
        assert_eq!(composer.buffer, "Привет");
        assert_eq!(composer.cursor, "Привет".len()); // 12 bytes
        composer.move_cursor_left();
        // Moved left by 1 Cyrillic grapheme 'т' (2 bytes)
        assert_eq!(composer.cursor, "Привет".len() - 2);

        // 2. Combining accent: 'e' + '\u{0301}' (acute accent) = 'é'
        composer.clear();
        composer.insert_str("e\u{0301}");
        assert_eq!(composer.buffer, "e\u{0301}");
        // Deleting backspace must delete the whole combining grapheme, not leave orphan accent
        composer.delete_backspace();
        assert_eq!(composer.buffer, "");
        assert_eq!(composer.cursor, 0);
    }

    #[test]
    fn test_wide_cjk_characters_and_display_width() {
        let mut composer = ChatComposer::new();
        composer.insert_str("世界");
        assert_eq!(composer.buffer, "世界");
        // Each CJK character is 3 bytes in UTF-8
        assert_eq!(composer.cursor, 6);

        let (line, display_col) = composer.cursor_display_col();
        assert_eq!(line, 0);
        // Each CJK character has terminal display cell width of 2, so width is 4
        assert_eq!(display_col, 4);

        composer.move_cursor_left();
        assert_eq!(composer.cursor, 3);
        let (_, display_col) = composer.cursor_display_col();
        assert_eq!(display_col, 2);

        composer.delete_backspace();
        assert_eq!(composer.buffer, "界");
        assert_eq!(composer.cursor, 0);
    }

    #[test]
    fn test_emoji_and_zwj_sequences() {
        let mut composer = ChatComposer::new();
        // Crab emoji
        composer.insert_str("🦀 Rust");
        assert_eq!(composer.buffer, "🦀 Rust");

        // Move left across " Rust"
        for _ in 0..5 {
            composer.move_cursor_left();
        }
        assert_eq!(composer.cursor, "🦀".len());

        // Backspace should delete the entire crab emoji cleanly
        composer.delete_backspace();
        assert_eq!(composer.buffer, " Rust");
        assert_eq!(composer.cursor, 0);

        // ZWJ sequence: family 👨‍👩‍👧‍👦
        composer.clear();
        let family = "👨‍👩‍👧‍👦";
        composer.insert_str(family);
        assert_eq!(composer.buffer, family);
        composer.delete_backspace();
        assert_eq!(composer.buffer, "");
        assert_eq!(composer.cursor, 0);
    }

    #[test]
    fn test_undo_and_redo() {
        let mut composer = ChatComposer::new();
        composer.insert_str("Hello");
        composer.insert_str(" World");
        assert_eq!(composer.buffer, "Hello World");

        assert!(composer.undo());
        assert_eq!(composer.buffer, "Hello");

        assert!(composer.redo());
        assert_eq!(composer.buffer, "Hello World");
    }

    #[test]
    fn test_multiline_editing_and_viewport_scroll() {
        let mut composer = ChatComposer::new();
        composer.insert_str("Line 1\nLine 2\nLine 3\nLine 4\nLine 5");
        let (line_idx, _) = composer.cursor_line_and_col();
        assert_eq!(line_idx, 4);

        // Visible box of 3 lines: viewport should scroll down to include Line 5
        composer.update_viewport(3);
        assert_eq!(composer.viewport_offset, 2); // lines 2, 3, 4 visible

        // Move up
        composer.move_up_or_history();
        assert_eq!(composer.cursor_line_and_col().0, 3);
        composer.move_up_or_history();
        assert_eq!(composer.cursor_line_and_col().0, 2);
        composer.move_up_or_history();
        assert_eq!(composer.cursor_line_and_col().0, 1);

        composer.update_viewport(3);
        assert_eq!(composer.viewport_offset, 1);
    }
}
