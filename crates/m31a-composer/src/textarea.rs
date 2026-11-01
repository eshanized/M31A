//! Multi-line editable textarea with soft-wrapping, undo/redo, and kill buffer.

#[derive(Debug, Clone)]
pub struct TextArea {
    pub lines: Vec<String>,
    pub cursor_row: usize,
    pub cursor_col: usize,
    kill_buffer: Option<String>,
    undo_stack: Vec<(Vec<String>, usize, usize)>,
    redo_stack: Vec<(Vec<String>, usize, usize)>,
}

impl Default for TextArea {
    fn default() -> Self {
        Self::new()
    }
}

impl TextArea {
    pub fn new() -> Self {
        Self {
            lines: vec![String::new()],
            cursor_row: 0,
            cursor_col: 0,
            kill_buffer: None,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }

    pub fn text(&self) -> String {
        self.lines.join("\n")
    }

    pub fn set_text(&mut self, text: &str) {
        self.snapshot_undo();
        self.lines = text.lines().map(|s| s.to_string()).collect();
        if self.lines.is_empty() {
            self.lines.push(String::new());
        }
        self.cursor_row = self.lines.len() - 1;
        self.cursor_col = self.lines[self.cursor_row].chars().count();
    }

    pub fn clear(&mut self) {
        self.snapshot_undo();
        self.lines = vec![String::new()];
        self.cursor_row = 0;
        self.cursor_col = 0;
    }

    pub fn insert_char(&mut self, ch: char) {
        self.snapshot_undo();
        if ch == '\n' {
            let curr_line = &self.lines[self.cursor_row];
            let chars: Vec<char> = curr_line.chars().collect();
            let left: String = chars[..self.cursor_col].iter().collect();
            let right: String = chars[self.cursor_col..].iter().collect();

            self.lines[self.cursor_row] = left;
            self.lines.insert(self.cursor_row + 1, right);
            self.cursor_row += 1;
            self.cursor_col = 0;
        } else {
            let line = &mut self.lines[self.cursor_row];
            let mut chars: Vec<char> = line.chars().collect();
            chars.insert(self.cursor_col, ch);
            *line = chars.into_iter().collect();
            self.cursor_col += 1;
        }
    }

    pub fn insert_str(&mut self, s: &str) {
        for ch in s.chars() {
            self.insert_char(ch);
        }
    }

    pub fn backspace(&mut self) {
        if self.cursor_col > 0 {
            self.snapshot_undo();
            let line = &mut self.lines[self.cursor_row];
            let mut chars: Vec<char> = line.chars().collect();
            chars.remove(self.cursor_col - 1);
            *line = chars.into_iter().collect();
            self.cursor_col -= 1;
        } else if self.cursor_row > 0 {
            self.snapshot_undo();
            let removed = self.lines.remove(self.cursor_row);
            self.cursor_row -= 1;
            self.cursor_col = self.lines[self.cursor_row].chars().count();
            self.lines[self.cursor_row].push_str(&removed);
        }
    }

    pub fn delete_forward(&mut self) {
        let line_len = self.lines[self.cursor_row].chars().count();
        if self.cursor_col < line_len {
            self.snapshot_undo();
            let line = &mut self.lines[self.cursor_row];
            let mut chars: Vec<char> = line.chars().collect();
            chars.remove(self.cursor_col);
            *line = chars.into_iter().collect();
        } else if self.cursor_row + 1 < self.lines.len() {
            self.snapshot_undo();
            let next_line = self.lines.remove(self.cursor_row + 1);
            self.lines[self.cursor_row].push_str(&next_line);
        }
    }

    pub fn kill_line(&mut self) {
        let line = &self.lines[self.cursor_row];
        let chars: Vec<char> = line.chars().collect();
        if self.cursor_col < chars.len() {
            self.snapshot_undo();
            let killed: String = chars[self.cursor_col..].iter().collect();
            self.lines[self.cursor_row] = chars[..self.cursor_col].iter().collect();
            self.kill_buffer = Some(killed);
        } else if self.cursor_row + 1 < self.lines.len() {
            self.snapshot_undo();
            let next = self.lines.remove(self.cursor_row + 1);
            self.lines[self.cursor_row].push_str(&next);
            self.kill_buffer = Some("\n".to_string());
        }
    }

    pub fn yank(&mut self) {
        if let Some(ref text) = self.kill_buffer.clone() {
            self.snapshot_undo();
            self.insert_str(text);
        }
    }

    pub fn move_cursor_left(&mut self) {
        if self.cursor_col > 0 {
            self.cursor_col -= 1;
        } else if self.cursor_row > 0 {
            self.cursor_row -= 1;
            self.cursor_col = self.lines[self.cursor_row].chars().count();
        }
    }

    pub fn move_cursor_right(&mut self) {
        let line_len = self.lines[self.cursor_row].chars().count();
        if self.cursor_col < line_len {
            self.cursor_col += 1;
        } else if self.cursor_row + 1 < self.lines.len() {
            self.cursor_row += 1;
            self.cursor_col = 0;
        }
    }

    pub fn move_cursor_up(&mut self) {
        if self.cursor_row > 0 {
            self.cursor_row -= 1;
            self.cursor_col = self
                .cursor_col
                .min(self.lines[self.cursor_row].chars().count());
        }
    }

    pub fn move_cursor_down(&mut self) {
        if self.cursor_row + 1 < self.lines.len() {
            self.cursor_row += 1;
            self.cursor_col = self
                .cursor_col
                .min(self.lines[self.cursor_row].chars().count());
        }
    }

    pub fn move_cursor_home(&mut self) {
        self.cursor_col = 0;
    }

    pub fn move_cursor_end(&mut self) {
        self.cursor_col = self.lines[self.cursor_row].chars().count();
    }

    pub fn undo(&mut self) {
        if let Some((lines, row, col)) = self.undo_stack.pop() {
            self.redo_stack
                .push((self.lines.clone(), self.cursor_row, self.cursor_col));
            self.lines = lines;
            self.cursor_row = row;
            self.cursor_col = col;
        }
    }

    pub fn redo(&mut self) {
        if let Some((lines, row, col)) = self.redo_stack.pop() {
            self.undo_stack
                .push((self.lines.clone(), self.cursor_row, self.cursor_col));
            self.lines = lines;
            self.cursor_row = row;
            self.cursor_col = col;
        }
    }

    fn snapshot_undo(&mut self) {
        self.undo_stack
            .push((self.lines.clone(), self.cursor_row, self.cursor_col));
        if self.undo_stack.len() > 100 {
            self.undo_stack.remove(0);
        }
        self.redo_stack.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_textarea_typing_and_newlines() {
        let mut area = TextArea::new();
        area.insert_str("Hello");
        area.insert_char('\n');
        area.insert_str("World");
        assert_eq!(area.text(), "Hello\nWorld");
        assert_eq!(area.cursor_row, 1);
        assert_eq!(area.cursor_col, 5);
    }

    #[test]
    fn test_textarea_undo_redo() {
        let mut area = TextArea::new();
        area.insert_str("A");
        area.insert_str("B");
        assert_eq!(area.text(), "AB");
        area.undo();
        assert_eq!(area.text(), "A");
        area.redo();
        assert_eq!(area.text(), "AB");
    }

    #[test]
    fn test_textarea_kill_yank() {
        let mut area = TextArea::new();
        area.insert_str("Hello World");
        area.cursor_col = 5;
        area.kill_line();
        assert_eq!(area.text(), "Hello");
        area.yank();
        assert_eq!(area.text(), "Hello World");
    }
}
