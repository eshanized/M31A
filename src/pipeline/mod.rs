//! Non-bypassable 11-stage tool execution pipeline, output budgeting, and typed error taxonomy (TL-03, TL-04).
//!
//! Enforces compile-time and runtime guarantees:
//! - 11 linear state-machine stages (`stages`)
//! - Physical side effects occur strictly in Stage 9 (`execution`)
//! - Budget-aware output capture and artifact externalization (`capture`)
//! - Strongly typed model-facing error taxonomy with bounded hints (`error`)
//! - Production dispatcher connecting `WorkerRunner` to the pipeline (`dispatcher`)

pub mod capture;
pub mod dedup;
pub mod dispatcher;
pub mod error;
pub mod runner;
pub mod stages;

pub use capture::OutputCaptureManager;
pub use dedup::{FenceVerdict, MutationDedupFence};
pub use dispatcher::ProductionActionDispatcher;
pub use error::{ToolError, ToolErrorCategory};
pub use runner::ToolPipelineRunner;
