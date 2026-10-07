//! Centralized input resolution (Principles 9 + 15).
//!
//! Keyboard priority is explicit and tested here:
//!
//! ```text
//! dialog → overlay → composer → route → global
//! ```
//!
//! This module classifies keys; it never executes domain actions. Execution
//! stays in `TuiApp::handle_key` (existing M31A handlers, approval routing,
//! slash routing, navigation). Centralizing classification removes the
//! scattered `if palette.is_open … if approval.is_open …` ordering risk
//! without changing any binding.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// What the key means before any handler runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyAction {
    /// Let the active dialog consume the key.
    Dialog,
    /// Let the active overlay consume the key.
    Overlay,
    /// Replay scrub key (read-only).
    Replay,
    /// Open the command palette (Ctrl+P / `:` on route).
    OpenPalette,
    /// Cancel / exit chord (Ctrl+C, Ctrl+D, `q` on route).
    Exit,
    /// Regular composer editing key.
    Composer,
    /// Route / navigation key (digits, `?`, j/k, PgUp/PgDn, …).
    Route,
    /// Global fallback (help toggle, focus cycling, …).
    Global,
}

/// Context needed to classify a key without borrowing the whole app.
#[derive(Debug, Clone, Copy)]
pub struct KeyContext {
    pub replay_active: bool,
    pub dialog_open: bool,
    pub overlay_open: bool,
    pub composer_focused: bool,
}

impl KeyContext {
    pub fn new(
        replay_active: bool,
        dialog_open: bool,
        overlay_open: bool,
        composer_focused: bool,
    ) -> Self {
        Self {
            replay_active,
            dialog_open,
            overlay_open,
            composer_focused,
        }
    }
}

/// Classify `key` under the dialog → overlay → composer → route → global order.
pub fn resolve_key(key: KeyEvent, ctx: KeyContext) -> KeyAction {
    if ctx.replay_active {
        return KeyAction::Replay;
    }
    if ctx.dialog_open {
        return KeyAction::Dialog;
    }
    if ctx.overlay_open {
        // Overlays still allow Ctrl+P to re-focus the palette trivially;
        // everything else belongs to the overlay.
        if is_palette_chord(key) {
            return KeyAction::OpenPalette;
        }
        return KeyAction::Overlay;
    }
    if is_palette_chord(key) {
        return KeyAction::OpenPalette;
    }
    if is_exit_chord(key) {
        return KeyAction::Exit;
    }
    if ctx.composer_focused {
        return KeyAction::Composer;
    }
    // Route keys are handled by navigation; everything else is global.
    match key.code {
        KeyCode::Char('?') => KeyAction::Global,
        KeyCode::Tab | KeyCode::BackTab => KeyAction::Global,
        _ => KeyAction::Route,
    }
}

fn is_palette_chord(key: KeyEvent) -> bool {
    (matches!(key.code, KeyCode::Char('p') | KeyCode::Char('P'))
        && key.modifiers.contains(KeyModifiers::CONTROL))
        || (matches!(key.code, KeyCode::Char(':')) && key.modifiers.is_empty())
}

fn is_exit_chord(key: KeyEvent) -> bool {
    let ctrl_c =
        matches!(key.code, KeyCode::Char('c')) && key.modifiers.contains(KeyModifiers::CONTROL);
    let ctrl_d =
        matches!(key.code, KeyCode::Char('d')) && key.modifiers.contains(KeyModifiers::CONTROL);
    ctrl_c || ctrl_d
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn ctrl(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::CONTROL)
    }

    #[test]
    fn priority_dialog_over_everything() {
        let ctx = KeyContext::new(false, true, true, true);
        assert_eq!(resolve_key(key(KeyCode::Enter), ctx), KeyAction::Dialog);
    }

    #[test]
    fn ctrl_p_opens_palette_from_composer_and_route() {
        let composer = KeyContext::new(false, false, false, true);
        let route = KeyContext::new(false, false, false, false);
        assert_eq!(
            resolve_key(ctrl(KeyCode::Char('p')), composer),
            KeyAction::OpenPalette
        );
        assert_eq!(
            resolve_key(ctrl(KeyCode::Char('p')), route),
            KeyAction::OpenPalette
        );
    }

    #[test]
    fn ctrl_c_is_exit_chord_before_composer() {
        let ctx = KeyContext::new(false, false, false, true);
        assert_eq!(resolve_key(ctrl(KeyCode::Char('c')), ctx), KeyAction::Exit);
    }

    #[test]
    fn replay_locks_all_keys() {
        let ctx = KeyContext::new(true, false, false, true);
        assert_eq!(resolve_key(key(KeyCode::Enter), ctx), KeyAction::Replay);
    }
}
