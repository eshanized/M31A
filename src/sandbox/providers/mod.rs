//! Concrete sandbox providers (Linux bubblewrap and process-level isolation).

pub mod bubblewrap;
pub mod process;

pub use bubblewrap::BubblewrapSandboxProvider;
pub use process::ProcessIsolationProvider;
