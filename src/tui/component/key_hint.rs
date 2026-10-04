//! Dynamic Keyboard Hint Presentation Primitives (TUI-01, TDS-01).
//!
//! Provides responsive footer keybindings and contextual command hints.

use ratatui::text::{Line, Span};

use crate::tui::theme::ThemeTokens;

/// Individual keyboard shortcut descriptor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyHint {
    pub key: &'static str,
    pub description: &'static str,
    pub priority: u8, // Higher priority survives width truncation
}

impl KeyHint {
    pub const fn new(key: &'static str, description: &'static str, priority: u8) -> Self {
        Self {
            key,
            description,
            priority,
        }
    }
}

/// Formatter for a sequence of key hints fitting inside a target width.
pub fn format_key_hints<'a>(hints: &[KeyHint], max_width: u16, tokens: &ThemeTokens) -> Line<'a> {
    let key_style = tokens.text_secondary;
    let desc_style = tokens.text_muted;
    let sep_style = tokens.text_muted;

    let mut spans: Vec<Span<'a>> = Vec::new();
    let mut current_len = 0usize;
    let limit = max_width as usize;

    // Filter hints by priority if width is constrained
    let mut active_hints: Vec<&KeyHint> = hints.iter().collect();
    if limit < 80 {
        active_hints.retain(|h| h.priority >= 2);
    }
    if limit < 60 {
        active_hints.retain(|h| h.priority >= 3);
    }

    for (i, hint) in active_hints.iter().enumerate() {
        let chunk_len = hint.key.len() + hint.description.len() + 4; // "[key] desc "
        if current_len + chunk_len > limit && !spans.is_empty() {
            break;
        }

        if i > 0 {
            spans.push(Span::styled(" · ", sep_style));
            current_len += 3;
        }

        spans.push(Span::styled(format!("{} ", hint.key), key_style));
        spans.push(Span::styled(hint.description, desc_style));
        current_len += chunk_len;
    }

    Line::from(spans)
}
