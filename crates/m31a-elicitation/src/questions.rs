//! Interactive question elicitation widget for user inputs and option selection.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuestionItem {
    pub id: String,
    pub question: String,
    pub options: Vec<String>,
    pub is_multi_select: bool,
    pub selected_indices: HashSet<usize>,
    pub cursor_idx: usize,
    pub custom_input: String,
    pub is_custom_focused: bool,
}

impl QuestionItem {
    pub fn new(question: impl Into<String>, options: Vec<String>, is_multi: bool) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            question: question.into(),
            options,
            is_multi_select: is_multi,
            selected_indices: HashSet::new(),
            cursor_idx: 0,
            custom_input: String::new(),
            is_custom_focused: false,
        }
    }

    pub fn move_up(&mut self) {
        if self.is_custom_focused {
            self.is_custom_focused = false;
            self.cursor_idx = self.options.len().saturating_sub(1);
        } else if self.cursor_idx > 0 {
            self.cursor_idx -= 1;
        }
    }

    pub fn move_down(&mut self) {
        if !self.is_custom_focused {
            if self.cursor_idx + 1 < self.options.len() {
                self.cursor_idx += 1;
            } else {
                self.is_custom_focused = true;
            }
        }
    }

    pub fn toggle_selection(&mut self) {
        if self.is_custom_focused {
            return;
        }

        if self.is_multi_select {
            if self.selected_indices.contains(&self.cursor_idx) {
                self.selected_indices.remove(&self.cursor_idx);
            } else {
                self.selected_indices.insert(self.cursor_idx);
            }
        } else {
            self.selected_indices.clear();
            self.selected_indices.insert(self.cursor_idx);
        }
    }

    pub fn submit(&self) -> Vec<String> {
        let mut answers = Vec::new();

        if !self.custom_input.trim().is_empty() {
            answers.push(self.custom_input.trim().to_string());
        }

        let mut sorted_indices: Vec<usize> = self.selected_indices.iter().copied().collect();
        sorted_indices.sort_unstable();

        for idx in sorted_indices {
            if let Some(opt) = self.options.get(idx) {
                answers.push(opt.clone());
            }
        }

        if answers.is_empty() && !self.options.is_empty() && !self.is_custom_focused {
            answers.push(self.options[self.cursor_idx].clone());
        }

        answers
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_question_toggle_and_submit() {
        let mut q = QuestionItem::new("Pick DB", vec!["PostgreSQL".into(), "SQLite".into()], false);
        q.cursor_idx = 1;
        q.toggle_selection();
        let res = q.submit();
        assert_eq!(res, vec!["SQLite".to_string()]);
    }
}
