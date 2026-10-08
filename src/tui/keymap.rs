//! The single input-priority authority.
//!
//! Keyboard ownership is explicit and tested here:
//!
//! ```text
//! replay → dialog → palette → help → wizard → palette-chord
//!     → interrupt → composer → route → global
//! ```
//!
//! This module classifies keys; it never executes domain actions. Execution
//! lives in `TuiApplication::handle_*_key`, which consumes every
//! classification — no result is discarded and no second matcher exists.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Which owner must handle the key. Every variant is consumed by
/// `TuiApplication::handle_key`; adding a variant requires adding its owner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyAction {
    /// Read-only replay scrub owns the key; mutations are blocked.
    Replay,
    /// Active approval dialog owns the key.
    Dialog,
    /// Open command palette owns the key.
    Palette,
    /// Open help overlay owns the key (closes on Esc/?/q, swallows the rest).
    Help,
    /// Open setup wizard dialog owns the key.
    Wizard,
    /// Palette chord outside the palette: open it.
    OpenPalette,
    /// Interrupt chord (Ctrl+C): cancel, degraded dispatch, or composer clear.
    Exit,
    /// Focused composer owns the key (editing + submission).
    Composer,
    /// Route / navigation key (digits, j/k, PgUp/PgDn, …).
    Route,
    /// Global fallback (help toggle, focus cycling, …).
    Global,
}

/// Context needed to classify a key without borrowing the whole app.
#[derive(Debug, Clone, Copy)]
pub struct KeyContext {
    pub replay_active: bool,
    pub dialog_open: bool,
    pub palette_open: bool,
    pub help_open: bool,
    pub wizard_open: bool,
    pub composer_focused: bool,
}

impl KeyContext {
    pub fn new(
        replay_active: bool,
        dialog_open: bool,
        palette_open: bool,
        help_open: bool,
        wizard_open: bool,
        composer_focused: bool,
    ) -> Self {
        Self {
            replay_active,
            dialog_open,
            palette_open,
            help_open,
            wizard_open,
            composer_focused,
        }
    }
}

/// Classify `key` under the single ownership order above.
pub fn resolve_key(key: KeyEvent, ctx: KeyContext) -> KeyAction {
    if ctx.replay_active {
        return KeyAction::Replay;
    }
    if ctx.dialog_open {
        return KeyAction::Dialog;
    }
    if ctx.palette_open {
        return KeyAction::Palette;
    }
    if ctx.help_open {
        return KeyAction::Help;
    }
    if ctx.wizard_open {
        return KeyAction::Wizard;
    }
    if is_palette_chord(key, ctx.composer_focused) {
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

fn is_palette_chord(key: KeyEvent, composer_focused: bool) -> bool {
    // Ctrl+P opens the palette from anywhere. Bare `:` only opens it in
    // route mode: inside the composer it is typed as text (matching the
    // historical dispatch, which never intercepted `:` while focused).
    (matches!(key.code, KeyCode::Char('p') | KeyCode::Char('P'))
        && key.modifiers.contains(KeyModifiers::CONTROL))
        || (matches!(key.code, KeyCode::Char(':')) && key.modifiers.is_empty() && !composer_focused)
}

fn is_exit_chord(key: KeyEvent) -> bool {
    // Ctrl+C only: the interrupt owner handles cancel/clear/fall-through.
    // Ctrl+D and `q` keep their historical meaning downstream (composer text
    // or route key); the event loop's `should_exit` owns loop exit.
    matches!(key.code, KeyCode::Char('c')) && key.modifiers.contains(KeyModifiers::CONTROL)
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
        let ctx = KeyContext::new(false, true, true, true, true, true);
        assert_eq!(resolve_key(key(KeyCode::Enter), ctx), KeyAction::Dialog);
    }

    #[test]
    fn transient_owners_beat_composer_and_route() {
        let key_enter = key(KeyCode::Enter);
        assert_eq!(
            resolve_key(
                key_enter,
                KeyContext::new(false, false, true, false, false, true)
            ),
            KeyAction::Palette
        );
        assert_eq!(
            resolve_key(
                key_enter,
                KeyContext::new(false, false, false, true, false, true)
            ),
            KeyAction::Help
        );
        assert_eq!(
            resolve_key(
                key_enter,
                KeyContext::new(false, false, false, false, true, true)
            ),
            KeyAction::Wizard
        );
        assert_eq!(
            resolve_key(
                key_enter,
                KeyContext::new(false, false, false, false, false, true)
            ),
            KeyAction::Composer
        );
        assert_eq!(
            resolve_key(
                key_enter,
                KeyContext::new(false, false, false, false, false, false)
            ),
            KeyAction::Route
        );
    }

    #[test]
    fn ctrl_p_opens_palette_from_composer_and_route() {
        let composer = KeyContext::new(false, false, false, false, false, true);
        let route = KeyContext::new(false, false, false, false, false, false);
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
    fn colon_types_in_composer_but_opens_palette_on_route() {
        let composer = KeyContext::new(false, false, false, false, false, true);
        let route = KeyContext::new(false, false, false, false, false, false);
        let colon = key(KeyCode::Char(':'));
        assert_eq!(resolve_key(colon, composer), KeyAction::Composer);
        assert_eq!(resolve_key(colon, route), KeyAction::OpenPalette);
    }

    #[test]
    fn ctrl_c_is_exit_chord_before_composer() {
        let ctx = KeyContext::new(false, false, false, false, false, true);
        assert_eq!(resolve_key(ctrl(KeyCode::Char('c')), ctx), KeyAction::Exit);
    }

    #[test]
    fn replay_locks_all_keys() {
        let ctx = KeyContext::new(true, false, false, false, false, true);
        assert_eq!(resolve_key(key(KeyCode::Enter), ctx), KeyAction::Replay);
    }
}
