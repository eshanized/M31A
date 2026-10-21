//! Sandbox provider subsystem and confinement runtime (SND-01, SND-02, SND-03).
//!
//! Non-negotiable invariants:
//! - "The model proposes. The runtime decides." (Model output is untrusted).
//! - Pluggable `SandboxProvider` abstraction with explicit `SandboxCapabilities` matrix.
//! - Unsupported sandbox guarantees are NEVER represented as implemented guarantees.
//! - Execution boundaries fail closed if requested capabilities are unsatisfied.
//! - Network confinement is deny-by-default (--unshare-net).

pub mod capabilities;
pub mod enforcement;
pub mod limits;
pub mod mode;
pub mod plan;
pub mod probe;
pub mod provider;
pub mod providers;

pub use capabilities::SandboxCapabilities;
pub use enforcement::{EnforcementDimensionStatus, ProcessEnforcementRecord, SandboxEnforcement};
pub use limits::{ResourceLimits, apply_pre_exec_limits};
pub use mode::SandboxMode;
pub use plan::{NetworkConfinement, SandboxError, SandboxPlan, SandboxViolation};
pub use probe::PlatformProbe;
pub use provider::{SandboxExecutionHandle, SandboxProvider};
pub use providers::{BubblewrapSandboxProvider, ProcessIsolationProvider};
