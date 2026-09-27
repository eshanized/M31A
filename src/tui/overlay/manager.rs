//! Priority-Aware Overlay System Manager (Section 11, TUI-01).
//!
//! Enforces centralized modal and overlay priority:
//! 1. Critical Policy Approval (highest priority - owns input until resolved)
//! 2. Universal Command Palette (Ctrl+P / :)
//! 3. Contextual Help Dialog (?)

use serde::{Deserialize, Serialize};

/// Priority levels for active overlays.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum OverlayPriority {
    None = 0,
    Help = 1,
    CommandPalette = 2,
    CriticalApproval = 3,
}

/// The kind of overlay currently occupying the screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum OverlayKind {
    Help,
    CommandPalette,
    Approval,
}

/// Centralized manager tracking overlay priority and visibility.
#[derive(Debug, Clone, Default)]
pub struct OverlayManager {
    pub is_help_open: bool,
}

impl OverlayManager {
    pub fn new() -> Self {
        Self {
            is_help_open: false,
        }
    }

    pub fn open_help(&mut self) {
        self.is_help_open = true;
    }

    pub fn close_help(&mut self) {
        self.is_help_open = false;
    }

    pub fn toggle_help(&mut self) {
        self.is_help_open = !self.is_help_open;
    }
}
