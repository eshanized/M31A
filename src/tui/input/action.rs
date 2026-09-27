//! Typed UiAction Normalization (COP-01, D-04).
//!
//! Normalizes raw crossterm key events into strongly typed UI actions,
//! decoupling terminal key sequences from business and rendering logic.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Strongly typed UI action normalized from keyboard and input events.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UiAction {
    // Navigation
    NavigateUp,
    NavigateDown,
    NavigateLeft,
    NavigateRight,
    PageUp,
    PageDown,
    Home,
    End,

    // Form & Modal Focus
    TabNext,
    TabPrev,

    // Dialog & Flow Control
    Select,
    Cancel,
    Submit,
    Quit,

    // Global Overlays & Toggles
    CommandPaletteToggle,
    HelpToggle,
    SearchToggle,
    ThemeCycle,

    // Editing Primitives
    TextInput(char),
    Paste(String),
    Backspace,
    Delete,
    DeleteWordBackward,
    MoveWordLeft,
    MoveWordRight,
}

/// Normalizes a crossterm `KeyEvent` into a typed `UiAction`.
pub fn normalize_key_event(event: KeyEvent) -> Option<UiAction> {
    // Handle Ctrl combinations
    if event.modifiers.contains(KeyModifiers::CONTROL) {
        return match event.code {
            KeyCode::Char('c') => Some(UiAction::Quit),
            KeyCode::Char('p') => Some(UiAction::CommandPaletteToggle),
            KeyCode::Char('h') | KeyCode::Char('?') => Some(UiAction::HelpToggle),
            KeyCode::Char('f') => Some(UiAction::SearchToggle),
            KeyCode::Char('t') => Some(UiAction::ThemeCycle),
            KeyCode::Char('w') => Some(UiAction::DeleteWordBackward),
            KeyCode::Left => Some(UiAction::MoveWordLeft),
            KeyCode::Right => Some(UiAction::MoveWordRight),
            _ => None,
        };
    }

    // Handle standard keys
    match event.code {
        KeyCode::Up => Some(UiAction::NavigateUp),
        KeyCode::Down => Some(UiAction::NavigateDown),
        KeyCode::Left => Some(UiAction::NavigateLeft),
        KeyCode::Right => Some(UiAction::NavigateRight),
        KeyCode::PageUp => Some(UiAction::PageUp),
        KeyCode::PageDown => Some(UiAction::PageDown),
        KeyCode::Home => Some(UiAction::Home),
        KeyCode::End => Some(UiAction::End),

        KeyCode::Tab => {
            if event.modifiers.contains(KeyModifiers::SHIFT) {
                Some(UiAction::TabPrev)
            } else {
                Some(UiAction::TabNext)
            }
        }
        KeyCode::BackTab => Some(UiAction::TabPrev),

        KeyCode::Enter => Some(UiAction::Submit),
        KeyCode::Esc => Some(UiAction::Cancel),

        KeyCode::Backspace => Some(UiAction::Backspace),
        KeyCode::Delete => Some(UiAction::Delete),

        KeyCode::Char(c) => Some(UiAction::TextInput(c)),
        _ => None,
    }
}
