//! Model routing, circuit breaker health tracking, and recovery subsystem (MDL-02, MDL-04).
//!
//! Enforces deterministic two-stage model resolution and runtime-local circuit health.

pub mod health;
pub mod recovery;
pub mod resolver;

pub use health::*;
pub use recovery::*;
pub use resolver::*;
