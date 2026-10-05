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

use crate::sandbox::plan::NetworkConfinement;

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
        match mode.trim().to_lowercase().as_str() {
            "strict" | "standard" => Self {
                isolation_required: true,
                network_isolated: true,
                network_confinement: NetworkConfinement::Isolated,
            },
            "permissive" => Self {
                isolation_required: false,
                network_isolated: true,
                network_confinement: NetworkConfinement::Isolated,
            },
            "disabled" => Self {
                isolation_required: false,
                network_isolated: false,
                network_confinement: NetworkConfinement::LoopbackOnly,
            },
            _ => Self {
                isolation_required: true,
                network_isolated: true,
                network_confinement: NetworkConfinement::Isolated,
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
