//! Zero-Dependency Reusable Text Input Component (COP-01, CFX-03, D-04, D-08).
//!
//! Provides Unicode-safe cursor navigation, single-line and multiline editing,
//! history navigation, word boundary traversal, and secret masking.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::{Block, Borders, Paragraph, Widget};
use unicode_segmentation::UnicodeSegmentation;

use crate::tui::theme::ThemeTokens;

/// Mode of text entry: single-line field or multiline composition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InputMode {
    #[default]
    SingleLine,
    MultiLine,
}

/// Masking policy for sensitive fields (API keys, tokens, passwords).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InputMasking {
    #[default]
    Plain,
    Masked(char),
}

/// A zero-dependency, Unicode-aware text input component.
#[derive(Debug, Clone)]
pub struct TextInput {
    content: String,
    cursor_char_idx: usize,
    mode: InputMode,
    masking: InputMasking,
    placeholder: String,
    history: Vec<String>,
    history_idx: Option<usize>,
    max_length: Option<usize>,
}

impl Default for TextInput {
    fn default() -> Self {
        Self::new(InputMode::SingleLine)
    }
}

impl TextInput {
    pub fn new(mode: InputMode) -> Self {
        Self {
            content: String::new(),
            cursor_char_idx: 0,
            mode,
            masking: InputMasking::Plain,
            placeholder: String::new(),
            history: Vec::new(),
            history_idx: None,
            max_length: None,
        }
    }

    pub fn single_line() -> Self {
        Self::new(InputMode::SingleLine)
    }

    pub fn multi_line() -> Self {
        Self::new(InputMode::MultiLine)
    }

    pub fn set_mode(&mut self, mode: InputMode) {
        self.mode = mode;
    }

    pub fn with_placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    pub fn with_masking(mut self, masking: InputMasking) -> Self {
        self.masking = masking;
        self
    }

    pub fn with_max_length(mut self, max: usize) -> Self {
        self.max_length = Some(max);
        self
    }

    pub fn value(&self) -> &str {
        &self.content
    }

    pub fn text(&self) -> &str {
        &self.content
    }

    pub fn set_value(&mut self, val: &str) {
        self.content = val.to_string();
        self.cursor_char_idx = self.content.graphemes(true).count();
    }

    pub fn set_text(&mut self, val: &str) {
        self.set_value(val);
    }

    /// Process a standard crossterm KeyEvent. Returns true if state changed.
    pub fn handle_key(&mut self, key: crossterm::event::KeyEvent) -> bool {
        use crossterm::event::{KeyCode, KeyModifiers};
        match key.code {
            KeyCode::Char('w') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.delete_word_backward();
                true
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                let byte_pos = self.byte_offset_for_char(self.cursor_char_idx);
                self.content.drain(..byte_pos);
                self.cursor_char_idx = 0;
                true
            }
            KeyCode::Char('k') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                let byte_pos = self.byte_offset_for_char(self.cursor_char_idx);
                self.content.truncate(byte_pos);
                true
            }
            KeyCode::Char('a') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.move_cursor_home();
                true
            }
            KeyCode::Char('e') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.move_cursor_end();
                true
            }
            KeyCode::Char(c) => {
                self.insert_char(c);
                true
            }
            KeyCode::Backspace => {
                if key.modifiers.contains(KeyModifiers::CONTROL) {
                    self.delete_word_backward();
                } else {
                    self.delete_backward();
                }
                true
            }
            KeyCode::Delete => {
                self.delete_forward();
                true
            }
            KeyCode::Left => {
                if key.modifiers.contains(KeyModifiers::CONTROL) {
                    self.move_cursor_word_left();
                } else {
                    self.move_cursor_left();
                }
                true
            }
            KeyCode::Right => {
                if key.modifiers.contains(KeyModifiers::CONTROL) {
                    self.move_cursor_word_right();
                } else {
                    self.move_cursor_right();
                }
                true
            }
            KeyCode::Home => {
                self.move_cursor_home();
                true
            }
            KeyCode::End => {
                self.move_cursor_end();
                true
            }
            KeyCode::Up => {
                if self.mode == InputMode::MultiLine {
                    self.move_cursor_up();
                } else {
                    self.history_prev();
                }
                true
            }
            KeyCode::Down => {
                if self.mode == InputMode::MultiLine {
                    self.move_cursor_down();
                } else {
                    self.history_next();
                }
                true
            }
            _ => false,
        }
    }

    pub fn clear(&mut self) {
        self.content.clear();
        self.cursor_char_idx = 0;
        self.history_idx = None;
    }

    pub fn char_count(&self) -> usize {
        self.content.graphemes(true).count()
    }

    pub fn cursor_char_index(&self) -> usize {
        self.cursor_char_idx
    }

    pub fn mode(&self) -> InputMode {
        self.mode
    }

    pub fn masking(&self) -> InputMasking {
        self.masking
    }

    /// Inserts a single character at the current cursor position.
    pub fn insert_char(&mut self, c: char) {
        if self.mode == InputMode::SingleLine && (c == '\n' || c == '\r') {
            return;
        }

        if let Some(max) = self.max_length
            && self.char_count() >= max
        {
            return;
        }

        let byte_pos = self.byte_offset_for_char(self.cursor_char_idx);
        self.content.insert(byte_pos, c);
        // Inserting a scalar can merge with an adjacent grapheme (e.g. a
        // combining mark), so scalar count is not a valid cursor increment.
        let cursor_byte = byte_pos + c.len_utf8();
        self.cursor_char_idx = self.content[..cursor_byte].graphemes(true).count();
    }

    /// Inserts a string at the current cursor position, sanitizing newlines in single-line mode.
    pub fn insert_str(&mut self, text: &str) {
        for g in text.graphemes(true) {
            if self.mode == InputMode::SingleLine && (g.contains('\n') || g.contains('\r')) {
                continue;
            }
            if let Some(max) = self.max_length
                && self.char_count() >= max
            {
                break;
            }
            let byte_pos = self.byte_offset_for_char(self.cursor_char_idx);
            self.content.insert_str(byte_pos, g);
            let cursor_byte = byte_pos + g.len();
            self.cursor_char_idx = self.content[..cursor_byte].graphemes(true).count();
        }
    }

    /// Deletes the character immediately preceding the cursor (Backspace).
    pub fn delete_backward(&mut self) {
        if self.cursor_char_idx == 0 {
            return;
        }

        let target_idx = self.cursor_char_idx - 1;
        let start_byte = self.byte_offset_for_char(target_idx);
        let end_byte = self.byte_offset_for_char(self.cursor_char_idx);

        self.content.replace_range(start_byte..end_byte, "");
        self.cursor_char_idx = target_idx;
    }

    /// Deletes the character at the current cursor position (Delete).
    pub fn delete_forward(&mut self) {
        if self.cursor_char_idx >= self.char_count() {
            return;
        }

        let start_byte = self.byte_offset_for_char(self.cursor_char_idx);
        let end_byte = self.byte_offset_for_char(self.cursor_char_idx + 1);

        self.content.replace_range(start_byte..end_byte, "");
    }

    /// Deletes the word immediately preceding the cursor (Ctrl-W / Ctrl-Backspace).
    pub fn delete_word_backward(&mut self) {
        if self.cursor_char_idx == 0 {
            return;
        }

        let target_char_idx = self.find_word_boundary_backward();
        let start_byte = self.byte_offset_for_char(target_char_idx);
        let end_byte = self.byte_offset_for_char(self.cursor_char_idx);

        self.content.replace_range(start_byte..end_byte, "");
        self.cursor_char_idx = target_char_idx;
    }

    pub fn move_cursor_left(&mut self) {
        if self.cursor_char_idx > 0 {
            self.cursor_char_idx -= 1;
        }
    }

    pub fn move_cursor_right(&mut self) {
        if self.cursor_char_idx < self.char_count() {
            self.cursor_char_idx += 1;
        }
    }

    pub fn move_cursor_home(&mut self) {
        if self.mode == InputMode::SingleLine {
            self.cursor_char_idx = 0;
        } else {
            // In multiline, jump to start of current line
            let mut line_start = 0;
            for (idx, g) in self.content.graphemes(true).enumerate() {
                if idx >= self.cursor_char_idx {
                    break;
                }
                if g == "\n" {
                    line_start = idx + 1;
                }
            }
            self.cursor_char_idx = line_start;
        }
    }

    pub fn move_cursor_end(&mut self) {
        if self.mode == InputMode::SingleLine {
            self.cursor_char_idx = self.char_count();
        } else {
            // In multiline, jump to end of current line
            let graphemes: Vec<&str> = self.content.graphemes(true).collect();
            let mut target = self.cursor_char_idx;
            while target < graphemes.len() && graphemes[target] != "\n" {
                target += 1;
            }
            self.cursor_char_idx = target;
        }
    }

    pub fn move_cursor_word_left(&mut self) {
        self.cursor_char_idx = self.find_word_boundary_backward();
    }

    pub fn move_cursor_word_right(&mut self) {
        self.cursor_char_idx = self.find_word_boundary_forward();
    }

    pub fn move_cursor_up(&mut self) {
        if self.mode != InputMode::MultiLine || self.cursor_char_idx == 0 {
            return;
        }

        let lines: Vec<&str> = self.content.split('\n').collect();
        let (line_idx, col_idx) = self.current_line_and_col();
        if line_idx > 0 {
            let prev_line_len = lines[line_idx - 1].graphemes(true).count();
            let target_col = col_idx.min(prev_line_len);
            let mut target_char = 0;
            for line in lines.iter().take(line_idx - 1) {
                target_char += line.graphemes(true).count() + 1;
            }
            target_char += target_col;
            self.cursor_char_idx = target_char;
        }
    }

    pub fn move_cursor_down(&mut self) {
        if self.mode != InputMode::MultiLine {
            return;
        }

        let lines: Vec<&str> = self.content.split('\n').collect();
        let (line_idx, col_idx) = self.current_line_and_col();
        if line_idx + 1 < lines.len() {
            let next_line_len = lines[line_idx + 1].graphemes(true).count();
            let target_col = col_idx.min(next_line_len);
            let mut target_char = 0;
            for line in lines.iter().take(line_idx + 1) {
                target_char += line.graphemes(true).count() + 1;
            }
            target_char += target_col;
            self.cursor_char_idx = target_char.min(self.char_count());
        }
    }

    /// History management: save submitted value to history.
    pub fn push_history(&mut self, entry: &str) {
        let trimmed = entry.trim();
        if !trimmed.is_empty() {
            self.history.push(trimmed.to_string());
        }
        self.history_idx = None;
    }

    pub fn history_prev(&mut self) {
        if self.history.is_empty() {
            return;
        }

        let next_idx = match self.history_idx {
            None => self.history.len().saturating_sub(1),
            Some(idx) => idx.saturating_sub(1),
        };

        self.history_idx = Some(next_idx);
        if let Some(entry) = self.history.get(next_idx).cloned() {
            self.set_value(&entry);
        }
    }

    pub fn history_next(&mut self) {
        if let Some(idx) = self.history_idx {
            if idx + 1 < self.history.len() {
                let next_idx = idx + 1;
                self.history_idx = Some(next_idx);
                if let Some(entry) = self.history.get(next_idx).cloned() {
                    self.set_value(&entry);
                }
            } else {
                self.history_idx = None;
                self.clear();
            }
        }
    }

    /// Formats the string to be rendered according to `InputMasking`.
    pub fn display_text(&self) -> String {
        match self.masking {
            InputMasking::Plain => self.content.clone(),
            InputMasking::Masked(mask_char) => {
                let mut out = String::with_capacity(self.content.len());
                for c in self.content.chars() {
                    if c == '\n' {
                        out.push('\n');
                    } else {
                        out.push(mask_char);
                    }
                }
                out
            }
        }
    }

    /// Computes byte offset corresponding to a grapheme cluster index.
    fn byte_offset_for_char(&self, target_char_idx: usize) -> usize {
        for (idx, (offset, _)) in self.content.grapheme_indices(true).enumerate() {
            if idx == target_char_idx {
                return offset;
            }
        }
        self.content.len()
    }

    fn find_word_boundary_backward(&self) -> usize {
        if self.cursor_char_idx == 0 {
            return 0;
        }

        let graphemes: Vec<&str> = self.content.graphemes(true).collect();
        let mut i = self.cursor_char_idx - 1;

        // Skip spaces immediately behind cursor
        while i > 0 && graphemes[i].chars().all(|c| c.is_whitespace()) {
            i -= 1;
        }

        // Skip non-whitespace characters to start of word
        while i > 0 && !graphemes[i - 1].chars().all(|c| c.is_whitespace()) {
            i -= 1;
        }

        i
    }

    fn find_word_boundary_forward(&self) -> usize {
        let graphemes: Vec<&str> = self.content.graphemes(true).collect();
        let len = graphemes.len();
        if self.cursor_char_idx >= len {
            return len;
        }

        let mut i = self.cursor_char_idx;

        // Skip non-whitespace characters
        while i < len && !graphemes[i].chars().all(|c| c.is_whitespace()) {
            i += 1;
        }

        // Skip trailing spaces
        while i < len && graphemes[i].chars().all(|c| c.is_whitespace()) {
            i += 1;
        }

        i
    }

    pub fn current_line_and_col(&self) -> (usize, usize) {
        let mut line = 0;
        let mut col = 0;
        for (idx, g) in self.content.graphemes(true).enumerate() {
            if idx >= self.cursor_char_idx {
                break;
            }
            if g == "\n" {
                line += 1;
                col = 0;
            } else {
                col += 1;
            }
        }
        (line, col)
    }

    /// Renders the input widget into a Ratatui buffer.
    pub fn render_widget(
        &self,
        area: Rect,
        buf: &mut Buffer,
        theme: &ThemeTokens,
        focused: bool,
        label: Option<&str>,
    ) {
        let border_style = if focused {
            theme.border_focused
        } else {
            theme.border_default
        };

        let title = label.unwrap_or(if self.mode == InputMode::MultiLine {
            "Editor"
        } else {
            "Input"
        });

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title(format!(" {} ", title));

        let inner_area = block.inner(area);
        block.render(area, buf);

        if self.content.is_empty() && !self.placeholder.is_empty() {
            let p = Paragraph::new(self.placeholder.as_str()).style(theme.text_muted);
            p.render(inner_area, buf);
        } else {
            let rendered_text = self.display_text();
            let p = Paragraph::new(rendered_text.as_str()).style(theme.text_primary);
            p.render(inner_area, buf);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_text_input_unicode_and_editing() {
        let mut input = TextInput::new(InputMode::SingleLine);
        input.insert_str("Hello, 🚀 world!");
        assert_eq!(input.value(), "Hello, 🚀 world!");
        assert_eq!(input.char_count(), 15);

        input.move_cursor_home();
        assert_eq!(input.cursor_char_index(), 0);

        input.move_cursor_word_right();
        assert_eq!(input.cursor_char_index(), 7); // After "Hello, "

        input.delete_backward();
        assert_eq!(input.value(), "Hello,🚀 world!");
    }

    #[test]
    fn test_combining_marks_keep_cursor_on_grapheme_boundary() {
        let mut input = TextInput::new(InputMode::SingleLine);
        input.insert_str("a");
        input.insert_char('\u{0301}');
        assert_eq!(input.value(), "a\u{0301}");
        assert_eq!(input.cursor_char_index(), 1);
        assert_eq!(input.char_count(), 1);

        input.clear();
        input.insert_str("a");
        input.insert_str("\u{0301}");
        assert_eq!(input.value(), "a\u{0301}");
        assert_eq!(input.cursor_char_index(), 1);
        assert_eq!(input.char_count(), 1);
        input.insert_char('b');
        assert_eq!(input.value(), "a\u{0301}b");
        assert_eq!(input.cursor_char_index(), 2);
    }

    #[test]
    fn test_text_input_secret_masking() {
        let mut input =
            TextInput::new(InputMode::SingleLine).with_masking(InputMasking::Masked('*'));
        input.insert_str("secret-token-123");
        assert_eq!(input.value(), "secret-token-123");
        assert_eq!(input.display_text(), "****************");
    }
}
