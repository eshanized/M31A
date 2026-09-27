//! Explicit Focus Management Model for TUI 2.0 (TUI-01, TDS-01).
//!
//! Provides single-source-of-truth focus ownership across:
//! - Composer: Input editor for natural language and slash commands
//! - Conversation: Main execution stream (scrollable, interactive items)
//! - ContextPanel: Master/detail side surface (tasks, agents, tools, diff, etc.)
//! - Overlay: Modals, Command Palette, Help, Approval dialogs

use serde::{Deserialize, Serialize};

/// The explicit focus target in the TUI shell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum FocusTarget {
    #[default]
    Composer,
    Conversation,
    ContextPanel,
    Overlay,
}

impl FocusTarget {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Composer => "COMPOSER",
            Self::Conversation => "STREAM",
            Self::ContextPanel => "DETAILS",
            Self::Overlay => "MODAL",
        }
    }

    pub fn is_composer(&self) -> bool {
        matches!(self, Self::Composer)
    }

    pub fn is_conversation(&self) -> bool {
        matches!(self, Self::Conversation)
    }

    pub fn is_context_panel(&self) -> bool {
        matches!(self, Self::ContextPanel)
    }

    pub fn is_overlay(&self) -> bool {
        matches!(self, Self::Overlay)
    }
}

/// Controller managing focus transitions and cycle ordering.
#[derive(Debug, Clone, Default)]
pub struct FocusManager {
    current: FocusTarget,
    previous: Option<FocusTarget>,
}

impl FocusManager {
    pub fn new(initial: FocusTarget) -> Self {
        Self {
            current: initial,
            previous: None,
        }
    }

    pub fn current(&self) -> FocusTarget {
        self.current
    }

    pub fn set_focus(&mut self, target: FocusTarget) {
        if self.current != target {
            self.previous = Some(self.current);
            self.current = target;
        }
    }

    /// Focus composer directly.
    pub fn focus_composer(&mut self) {
        self.set_focus(FocusTarget::Composer);
    }

    /// Unfocus composer and return to conversation or previous focus.
    pub fn unfocus_composer(&mut self) {
        if self.current == FocusTarget::Composer {
            let next = match self.previous {
                Some(FocusTarget::Composer) | None => FocusTarget::Conversation,
                Some(p) => p,
            };
            self.set_focus(next);
        }
    }

    /// Focus modal overlay, remembering where focus was.
    pub fn enter_overlay(&mut self) {
        self.set_focus(FocusTarget::Overlay);
    }

    /// Exit modal overlay, restoring previous focus target.
    pub fn exit_overlay(&mut self) {
        if self.current == FocusTarget::Overlay {
            let restored = self.previous.unwrap_or(FocusTarget::Composer);
            self.current = restored;
            self.previous = None;
        }
    }

    /// Cycle to next workspace focus area (Tab key).
    /// Does not cycle into Overlay.
    pub fn cycle_next(&mut self, has_context_panel: bool) {
        let next = match self.current {
            FocusTarget::Composer => FocusTarget::Conversation,
            FocusTarget::Conversation => {
                if has_context_panel {
                    FocusTarget::ContextPanel
                } else {
                    FocusTarget::Composer
                }
            }
            FocusTarget::ContextPanel => FocusTarget::Composer,
            FocusTarget::Overlay => FocusTarget::Overlay,
        };
        self.set_focus(next);
    }

    /// Cycle to previous workspace focus area (Shift+Tab).
    pub fn cycle_prev(&mut self, has_context_panel: bool) {
        let prev = match self.current {
            FocusTarget::Composer => {
                if has_context_panel {
                    FocusTarget::ContextPanel
                } else {
                    FocusTarget::Conversation
                }
            }
            FocusTarget::Conversation => FocusTarget::Composer,
            FocusTarget::ContextPanel => FocusTarget::Conversation,
            FocusTarget::Overlay => FocusTarget::Overlay,
        };
        self.set_focus(prev);
    }
}
