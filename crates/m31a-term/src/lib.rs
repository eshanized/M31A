//! `m31a-term`: Terminal ergonomics, capability detection, PTY runner, and OSC 8 hyperlinks.

pub mod detection;
pub mod hyperlinks;
pub mod latex;
pub mod merman;
pub mod patchmark;
pub mod pty;
pub mod strip;

pub use detection::{ColorLevel, TerminalCapabilities, TerminalKind};
pub use hyperlinks::{
    format_clickable_path, format_file_hyperlink, format_git_commit_hyperlink, format_hyperlink,
    format_symbol_hyperlink, has_hyperlink, strip_hyperlinks,
};
pub use latex::{render_latex, render_latex_2d, to_subscript, to_superscript};
pub use merman::{is_mermaid_diagram, render_mermaid};
pub use patchmark::{TailHeal, compute_tail_heal, heal_streaming_markdown};
pub use pty::{PtyOutput, PtyRunner};
pub use strip::{expand_tabs, strip_ansi};
