//! Centralized Modal & Overlay System (Section 11, TUI-01).

pub mod help;
pub mod manager;

pub use help::render_help_overlay;
pub use manager::{OverlayKind, OverlayManager, OverlayPriority};
