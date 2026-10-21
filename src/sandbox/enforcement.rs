//! Runtime-authoritative sandbox enforcement policy (SND-04).
//!
//! The deployment configuration's `sandbox_mode` (`strict` | `standard` |
//! `permissive` | `disabled`) is the SOLE authority for whether execution is
//! isolated. Execution backends MUST derive their behavior from the resolved
//! [`SandboxEnforcement`], never from hardcoded `sandbox_required = false`.
//!
//! ```text
//! strict     → isolation required; network denied (fail closed if unavailable)
//! standard   → isolation required; network denied (fail closed if unavailable)
//! permissive → isolation best-effort; network denied whenever sandboxed
//! disabled   → no sandboxing (explicit operator opt-out; autonomous
//!              execution requiring isolation still fails closed upstream)
//! ```
//!
//! The `M31A_REQUIRE_SANDBOX` environment variable can only STRENGTHEN this
//! policy (request isolation when the deployment allows fallback); it can
//! never weaken a compiled `required` policy into unisolated execution.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::sandbox::plan::NetworkConfinement;

/// Status of an individual security control dimension (AUD-04).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", content = "details")]
pub enum EnforcementDimensionStatus {
    /// Actively enforced on the running process with concrete evidence.
    Active(String),
    /// Configured and supported by host, but idle (process not spawned yet or completed).
    Ready(String),
    /// Platform or provider lacks capability for this control mechanism.
    Unsupported(String),
    /// Enforcement was attempted but failed at runtime / process boundary.
    Failed(String),
    /// Protection cannot be verified (e.g. process exited, unverified startup state, PID recycled).
    Unverified(String),
    /// Disabled by explicit operator policy.
    Disabled,
}

impl Default for EnforcementDimensionStatus {
    fn default() -> Self {
        Self::Ready("unspecified".to_string())
    }
}

impl EnforcementDimensionStatus {
    pub fn is_active(&self) -> bool {
        matches!(self, Self::Active(_))
    }

    pub fn display_label(&self) -> &'static str {
        match self {
            Self::Active(_) => "enforced (active)",
            Self::Ready(_) => "capable (idle - no running jobs)",
            Self::Unsupported(_) => "unsupported on host",
            Self::Failed(_) => "failed",
            Self::Unverified(_) => "unverified",
            Self::Disabled => "disabled",
        }
    }
}

/// Authoritative per-process or per-job enforcement record (AUD-04).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessEnforcementRecord {
    pub job_id: String,
    pub pid: Option<u32>,
    pub provider: String,
    pub isolation: EnforcementDimensionStatus,
    pub network: EnforcementDimensionStatus,
    pub resource_limits: EnforcementDimensionStatus,
    pub watchdog: EnforcementDimensionStatus,
    pub recorded_at: DateTime<Utc>,
    pub failure_reason: Option<String>,
}

impl Default for ProcessEnforcementRecord {
    fn default() -> Self {
        Self {
            job_id: String::new(),
            pid: None,
            provider: "local_process".to_string(),
            isolation: EnforcementDimensionStatus::default(),
            network: EnforcementDimensionStatus::default(),
            resource_limits: EnforcementDimensionStatus::default(),
            watchdog: EnforcementDimensionStatus::default(),
            recorded_at: Utc::now(),
            failure_reason: None,
        }
    }
}

/// Resolved sandbox enforcement for one runtime scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SandboxEnforcement {
    /// Whether isolated execution is required (fail closed if unavailable).
    pub isolation_required: bool,
    /// Whether sandboxed execution denies network access.
    pub network_isolated: bool,
    /// Network confinement applied whenever sandboxed.
    pub network_confinement: NetworkConfinement,
}

impl SandboxEnforcement {
    /// Resolve from the authoritative deployment sandbox mode.
    /// Unknown modes fail closed to `strict`.
    pub fn from_sandbox_mode(mode: &str) -> Self {
        let parsed = mode
            .parse::<super::SandboxMode>()
            .unwrap_or(super::SandboxMode::Strict);
        Self::from_mode(parsed)
    }

    /// Resolve from typed SandboxMode.
    pub fn from_mode(mode: super::SandboxMode) -> Self {
        match mode {
            super::SandboxMode::Strict | super::SandboxMode::Standard => Self {
                isolation_required: true,
                network_isolated: true,
                network_confinement: NetworkConfinement::Isolated,
            },
            super::SandboxMode::Permissive => Self {
                isolation_required: false,
                network_isolated: true,
                network_confinement: NetworkConfinement::Isolated,
            },
            super::SandboxMode::Disabled => Self {
                isolation_required: false,
                network_isolated: false,
                network_confinement: NetworkConfinement::LoopbackOnly,
            },
        }
    }

    /// Whether `M31A_REQUIRE_SANDBOX=1/true` is set (strengthen-only signal).
    pub fn env_requires_sandbox() -> bool {
        std::env::var("M31A_REQUIRE_SANDBOX")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false)
    }

    /// Effective requirement: deployment policy OR strengthen-only env.
    /// The environment can never weaken a required policy.
    pub fn effective_required(&self) -> bool {
        self.isolation_required || Self::env_requires_sandbox()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strict_and_standard_require_isolation_with_denied_network() {
        for mode in ["strict", "standard"] {
            let e = SandboxEnforcement::from_sandbox_mode(mode);
            assert!(e.isolation_required);
            assert!(e.network_isolated);
            assert_eq!(e.network_confinement, NetworkConfinement::Isolated);
        }
    }

    #[test]
    fn unknown_mode_fails_closed_to_strict() {
        let e = SandboxEnforcement::from_sandbox_mode("bogus");
        assert!(e.isolation_required);
        assert!(e.network_isolated);
    }

    #[test]
    fn disabled_never_requires() {
        let e = SandboxEnforcement::from_sandbox_mode("disabled");
        assert!(!e.isolation_required);
    }

    #[test]
    fn permissive_sandboxes_isolated_when_available() {
        let e = SandboxEnforcement::from_sandbox_mode("permissive");
        assert!(!e.isolation_required);
        assert!(e.network_isolated);
        assert_eq!(e.network_confinement, NetworkConfinement::Isolated);
    }
}
