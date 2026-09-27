//! Reusable TUI Visual Component Primitives (TUI-01, TDS-01–04).

pub mod badge;
pub mod code;
pub mod diff;
pub mod key_hint;
pub mod progress;

pub use badge::{render_agent_tag, render_key_chip, render_risk_badge, render_status_badge};
pub use code::render_bounded_output;
pub use diff::{DiffParser, DiffStats, ParsedDiffLine, render_diff_lines};
pub use key_hint::{KeyHint, format_key_hints};
pub use progress::{format_progress_bar, render_progress_line};
