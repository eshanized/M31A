//! Capabilities and tool execution runtime subsystem.
//!
//! Owns the 15 core capability family definitions, capability instance descriptors,
//! operational health tracking, strongly typed permissions, and central registry (CTL-01, CTL-02, CTL-03).
//!
//! Core principle: The model proposes. The runtime decides.

pub mod error;
pub mod family;
pub mod health;
pub mod instance;
pub mod permissions;
pub mod providers;
pub mod registry;
pub mod traits;

pub use error::CapabilityError;
pub use family::CapabilityFamily;
pub use health::{
    CapabilityHealthState, CapabilityHealthTracker, HealthConfig, HealthTransitionRecord,
};
pub use instance::CapabilityInstance;
pub use permissions::{CapabilityOperation, CapabilityPermissions, RiskClass};
pub use registry::CapabilityRegistry;
pub use traits::*;
