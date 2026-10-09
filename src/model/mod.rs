//! Model subsystem.
//!
//! Owns model provider abstractions, wire protocol serialization, SSE streaming,
//! model routing, health management, and token telemetry (MDL-01 to MDL-05).
//!
//! Core principle: The model proposes. The runtime decides.

pub mod catalog;
pub mod persistence;
pub mod pricing;
pub mod protocol;
pub mod provider;
pub mod router;
pub mod types;

pub use catalog::*;
pub use persistence::*;
pub use pricing::*;
pub use protocol::*;
pub use provider::*;
pub use router::*;
pub use types::*;
