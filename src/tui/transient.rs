//! Transient UI as a separate layer (Principle 9).
//!
//! ```text
//! base route + dialog + overlay + notification
//! ```
//!
//! Dialogs and overlays never destroy underlying route state. This module
//! only classifies *what* is on top; the actual widgets remain M31A's
//! existing approval modal, command palette, help overlay, and wizard.

use crossterm::event::KeyEvent;

/// Which transient layer owns the next keypress.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransientLayer {
    /// No transient UI: the route/composer owns input.
    None,
    /// Modal dialog (approval, wizard): highest priority.
    Dialog,
    /// Floating overlay (palette, help, registry overlay).
    Overlay,
    /// Replay banner owns input while active (read-only scrub).
    Replay,
}

impl TransientLayer {
    pub fn is_active(&self) -> bool {
        !matches!(self, Self::None)
    }
}

/// Snapshot of transient UI: dialog + overlay + notification line.
#[derive(Debug, Clone, Default)]
pub struct TransientUi {
    pub dialog_open: bool,
    pub overlay_open: bool,
    pub replay_active: bool,
    pub notification: Option<String>,
}

impl TransientUi {
    pub fn new() -> Self {
        Self::default()
    }

    /// Classify the topmost transient layer for input priority.
    pub fn top_layer(&self) -> TransientLayer {
        if self.replay_active {
            TransientLayer::Replay
        } else if self.dialog_open {
            TransientLayer::Dialog
        } else if self.overlay_open {
            TransientLayer::Overlay
        } else {
            TransientLayer::None
        }
    }

    pub fn notify(&mut self, line: impl Into<String>) {
        self.notification = Some(line.into());
    }

    pub fn clear_notification(&mut self) {
        self.notification = None;
    }
}

/// Read transient state from the live application without mutating it.
///
/// Kept as a free function so both `TuiApp` and `TuiApplication` share one
/// classification (no duplicated priority logic).
pub fn classify_transient(
    replay_active: bool,
    approval_open: bool,
    palette_open: bool,
    help_open: bool,
    overlay_open: bool,
) -> TransientUi {
    TransientUi {
        dialog_open: approval_open,
        overlay_open: palette_open || help_open || overlay_open,
        replay_active,
        notification: None,
    }
}

/// Keyboard priority (Principle 9): dialog → overlay → composer → route → global.
///
/// Returns the layer that must handle `key` first. Callers still route the
/// key to M31A's existing handlers; this only makes the order explicit and
/// testable.
pub fn input_priority(transient: &TransientUi, composer_focused: bool) -> InputPriority {
    match transient.top_layer() {
        TransientLayer::Replay => InputPriority::Replay,
        TransientLayer::Dialog => InputPriority::Dialog,
        TransientLayer::Overlay => InputPriority::Overlay,
        TransientLayer::None => {
            if composer_focused {
                InputPriority::Composer
            } else {
                InputPriority::Route
            }
        }
    }
}

/// Ordered input owner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputPriority {
    Dialog,
    Overlay,
    Composer,
    Route,
    Global,
    Replay,
}

#[allow(unused)]
pub fn describe_key(_key: &KeyEvent) -> &'static str {
    "key"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dialog_beats_overlay_beats_composer() {
        let t = TransientUi {
            dialog_open: true,
            overlay_open: true,
            replay_active: false,
            notification: None,
        };
        assert_eq!(
            input_priority(&t, true),
            InputPriority::Dialog,
            "dialog has top priority"
        );
        let t = TransientUi {
            dialog_open: false,
            overlay_open: true,
            replay_active: false,
            notification: None,
        };
        assert_eq!(input_priority(&t, true), InputPriority::Overlay);
        let t = TransientUi::new();
        assert_eq!(input_priority(&t, true), InputPriority::Composer);
        assert_eq!(input_priority(&t, false), InputPriority::Route);
    }

    #[test]
    fn replay_locks_input_to_read_only_scrub() {
        let t = TransientUi {
            replay_active: true,
            ..TransientUi::new()
        };
        assert_eq!(input_priority(&t, true), InputPriority::Replay);
    }
}
