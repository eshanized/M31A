//! Persistent Application Shell (Section 5, TUI-01).

pub mod context_rail;
pub mod footer;
pub mod header;
pub mod workspace;

pub use context_rail::render_context_rail;
pub use footer::{render_footer, render_footer_full};
pub use header::render_header;
pub use workspace::render_workspace;
