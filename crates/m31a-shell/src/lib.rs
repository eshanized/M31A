//! `m31a-shell`: Interactive PTY and stateful persistent shell execution engine.
//!
//! Maintains long-lived shell sessions across agent turns, preserving directory context,
//! environment variables, and handling head-tail output buffering.

pub mod buffer;
pub mod pty;
pub mod session;

pub use buffer::HeadTailBuffer;
pub use pty::{PtyExecutionOutput, PtyRunner, PtySession, PtySize};
pub use session::{PersistentShellSession, ShellOutput};
