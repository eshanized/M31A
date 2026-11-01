//! `m31a-watcher`: Asynchronous workspace filesystem watcher.
//!
//! Monitors workspace file modifications, debouncing rapid file changes and filtering noise
//! from compiler outputs, temporary artifacts, and version control directories.

pub mod events;
pub mod filter;
pub mod watcher;

pub use events::{FileChangeEvent, FileChangeKind, WorkspaceChangeBatch};
pub use filter::WatcherFilter;
pub use watcher::{WatcherConfig, WorkspaceWatcher};
