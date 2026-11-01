//! Interactive `@`-mentions autocomplete controller.

use m31a_fuzzy::{Candidate, WorkspaceScanner};
use std::path::Path;

#[derive(Debug, Clone)]
pub struct MentionsPopup {
    pub is_active: bool,
    pub query: String,
    pub candidates: Vec<Candidate>,
    pub selected_index: usize,
    pub at_char_col: usize,
}

impl Default for MentionsPopup {
    fn default() -> Self {
        Self::new()
    }
}

impl MentionsPopup {
    pub fn new() -> Self {
        Self {
            is_active: false,
            query: String::new(),
            candidates: Vec::new(),
            selected_index: 0,
            at_char_col: 0,
        }
    }

    /// Checks the current line and cursor to see if an `@` mention is active.
    pub fn update_from_line(&mut self, line: &str, cursor_col: usize, workspace_root: &Path) {
        let chars: Vec<char> = line.chars().collect();
        let prefix: String = chars[..cursor_col.min(chars.len())].iter().collect();

        if let Some(at_pos) = prefix.rfind('@') {
            // Check that '@' is either at the start of the line or preceded by whitespace
            let is_valid_start = at_pos == 0
                || prefix
                    .chars()
                    .nth(at_pos - 1)
                    .map(|c| c.is_whitespace())
                    .unwrap_or(false);

            if is_valid_start {
                let query = &prefix[at_pos + 1..];
                // If there's whitespace inside query, dismiss
                if !query.contains(char::is_whitespace) {
                    self.is_active = true;
                    self.query = query.to_string();
                    self.at_char_col = at_pos;

                    let all_files =
                        WorkspaceScanner::collect_workspace_files_cached(workspace_root, 300);
                    self.candidates = WorkspaceScanner::search(query, &all_files);
                    if self.selected_index >= self.candidates.len() {
                        self.selected_index = 0;
                    }
                    return;
                }
            }
        }

        self.dismiss();
    }

    pub fn dismiss(&mut self) {
        self.is_active = false;
        self.query.clear();
        self.candidates.clear();
        self.selected_index = 0;
    }

    pub fn move_up(&mut self) {
        if !self.candidates.is_empty() {
            self.selected_index = self.selected_index.saturating_sub(1);
        }
    }

    pub fn move_down(&mut self) {
        if !self.candidates.is_empty() && self.selected_index + 1 < self.candidates.len() {
            self.selected_index += 1;
        }
    }

    pub fn selected_candidate(&self) -> Option<&Candidate> {
        self.candidates.get(self.selected_index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mentions_activation() {
        let mut popup = MentionsPopup::new();
        let root = std::env::current_dir().unwrap();
        popup.update_from_line("Look at @main", 13, &root);
        assert!(popup.is_active);
        assert_eq!(popup.query, "main");
    }

    #[test]
    fn test_mentions_dismiss_on_space() {
        let mut popup = MentionsPopup::new();
        let root = std::env::current_dir().unwrap();
        popup.update_from_line("Look at @main and other", 23, &root);
        assert!(!popup.is_active);
    }
}
