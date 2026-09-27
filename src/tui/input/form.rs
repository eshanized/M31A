//! Form Field Validation Engine & Multi-Field Management (COP-01, COP-02, D-04).
//!
//! Provides declarative validation rules, Tab/Shift-Tab focus traversal,
//! error messaging, and form-level submission gates.

use std::path::Path;
use std::sync::Arc;

use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Rect};

use super::text_input::TextInput;
use crate::tui::theme::ThemeTokens;

/// Type alias for custom validation closures.
pub type CustomValidationFn = Arc<dyn Fn(&str) -> Result<(), String> + Send + Sync>;

/// Declarative validation rules for form fields.
#[derive(Clone)]
pub enum ValidationRule {
    NonEmpty,
    MinLength(usize),
    MaxLength(usize),
    NumericRange(f64, f64),
    DirectoryExists,
    Custom(CustomValidationFn),
}

impl std::fmt::Debug for ValidationRule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NonEmpty => write!(f, "NonEmpty"),
            Self::MinLength(len) => write!(f, "MinLength({len})"),
            Self::MaxLength(len) => write!(f, "MaxLength({len})"),
            Self::NumericRange(min, max) => write!(f, "NumericRange({min}, {max})"),
            Self::DirectoryExists => write!(f, "DirectoryExists"),
            Self::Custom(_) => write!(f, "CustomClosure"),
        }
    }
}

/// An individual field within a Form.
#[derive(Debug, Clone)]
pub struct FormField {
    pub id: String,
    pub label: String,
    pub input: TextInput,
    pub rules: Vec<ValidationRule>,
    pub error: Option<String>,
    pub optional: bool,
}

impl FormField {
    pub fn new(id: impl Into<String>, label: impl Into<String>, input: TextInput) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            input,
            rules: Vec::new(),
            error: None,
            optional: false,
        }
    }

    pub fn with_rule(mut self, rule: ValidationRule) -> Self {
        self.rules.push(rule);
        self
    }

    pub fn optional(mut self) -> Self {
        self.optional = true;
        self
    }

    pub fn value(&self) -> &str {
        self.input.value()
    }

    pub fn validate(&mut self) -> bool {
        let val = self.input.value().trim();

        if val.is_empty() {
            if self.optional {
                self.error = None;
                return true;
            } else if self
                .rules
                .iter()
                .any(|r| matches!(r, ValidationRule::NonEmpty))
            {
                self.error = Some(format!("{} cannot be empty", self.label));
                return false;
            }
        }

        for rule in &self.rules {
            match rule {
                ValidationRule::NonEmpty => {
                    if val.is_empty() {
                        self.error = Some(format!("{} is required", self.label));
                        return false;
                    }
                }
                ValidationRule::MinLength(min) => {
                    if val.chars().count() < *min {
                        self.error = Some(format!(
                            "{} must be at least {} characters",
                            self.label, min
                        ));
                        return false;
                    }
                }
                ValidationRule::MaxLength(max) => {
                    if val.chars().count() > *max {
                        self.error =
                            Some(format!("{} cannot exceed {} characters", self.label, max));
                        return false;
                    }
                }
                ValidationRule::NumericRange(min, max) => match val.parse::<f64>() {
                    Ok(num) => {
                        if num < *min || num > *max {
                            self.error = Some(format!(
                                "{} must be between {} and {}",
                                self.label, min, max
                            ));
                            return false;
                        }
                    }
                    Err(_) => {
                        self.error = Some(format!("{} must be a valid number", self.label));
                        return false;
                    }
                },
                ValidationRule::DirectoryExists => {
                    let path = Path::new(val);
                    if !path.is_dir() {
                        self.error = Some(format!("Directory does not exist: {}", val));
                        return false;
                    }
                }
                ValidationRule::Custom(func) => {
                    if let Err(msg) = func(val) {
                        self.error = Some(msg);
                        return false;
                    }
                }
            }
        }

        self.error = None;
        true
    }
}

/// A multi-field interactive Form with focus management.
#[derive(Debug, Clone)]
pub struct Form {
    pub fields: Vec<FormField>,
    pub focused_idx: usize,
}

impl Form {
    pub fn new(fields: Vec<FormField>) -> Self {
        Self {
            fields,
            focused_idx: 0,
        }
    }

    pub fn focus_next(&mut self) {
        if !self.fields.is_empty() {
            self.focused_idx = (self.focused_idx + 1) % self.fields.len();
        }
    }

    pub fn focus_prev(&mut self) {
        if !self.fields.is_empty() {
            if self.focused_idx == 0 {
                self.focused_idx = self.fields.len() - 1;
            } else {
                self.focused_idx -= 1;
            }
        }
    }

    pub fn focus_index(&self) -> usize {
        self.focused_idx
    }

    pub fn focus_field(&mut self, idx: usize) {
        if idx < self.fields.len() {
            self.focused_idx = idx;
        }
    }

    pub fn active_field(&self) -> Option<&FormField> {
        self.fields.get(self.focused_idx)
    }

    pub fn active_field_mut(&mut self) -> Option<&mut FormField> {
        self.fields.get_mut(self.focused_idx)
    }

    pub fn get_value(&self, id: &str) -> Option<&str> {
        self.fields.iter().find(|f| f.id == id).map(|f| f.value())
    }

    pub fn validate_all(&mut self) -> bool {
        let mut is_valid = true;
        for field in &mut self.fields {
            if !field.validate() {
                is_valid = false;
            }
        }
        is_valid
    }

    pub fn render(&self, area: Rect, buf: &mut Buffer, theme: &ThemeTokens) {
        if self.fields.is_empty() || area.height < 3 {
            return;
        }

        let field_height = 3u16;
        let total_field_height = (self.fields.len() as u16) * field_height;
        let constraints: Vec<Constraint> = self
            .fields
            .iter()
            .map(|_| Constraint::Length(field_height))
            .collect();

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(constraints)
            .split(Rect {
                height: total_field_height.min(area.height),
                ..area
            });

        for (idx, field) in self.fields.iter().enumerate() {
            if idx >= chunks.len() {
                break;
            }

            let is_focused = idx == self.focused_idx;
            let field_area = chunks[idx];

            field
                .input
                .render_widget(field_area, buf, theme, is_focused, Some(&field.label));

            // If error is present, display error tag inside bottom line
            if let Some(err_msg) = &field.error {
                let err_style = theme.status_failed;
                let err_text = format!(" ⚠ {} ", err_msg);
                let err_x =
                    field_area.x + field_area.width.saturating_sub(err_text.len() as u16 + 2);
                let err_y = field_area.y + field_area.height.saturating_sub(1);
                buf.set_string(err_x, err_y, &err_text, err_style);
            }
        }
    }
}
