//! `m31a-runtime`: Headless application runtime and service coordinator for M31A.
//!
//! Decouples the M31A runtime from presentation layers (CLI, TUI, AppServer, and future Tauri desktop).
//! Exposes a strongly-typed, independently testable application engine with lifecycle management,
//! authoritative service ownership, and presentation-independent event streaming.

pub mod builder;
pub mod config;
pub mod context;
pub mod error;
pub mod events;
pub mod lifecycle;
pub mod runtime;
pub mod services;

pub use builder::RuntimeBuilder;
pub use config::RuntimeConfig;
pub use context::{ActiveTurnHandle, PendingApprovalEntry, PendingQuestionEntry, TurnRequest};
pub use error::RuntimeError;
pub use events::RuntimeEvent;
pub use lifecycle::{RuntimeState, ShutdownReport};
pub use runtime::M31aRuntime;
pub use services::RuntimeServices;
