//! Cross-Platform Sandbox Execution Abstraction.
//!
//! Provides a unified, fail-closed platform abstraction for discovering
//! sandbox capabilities, selecting backends, and enforcing isolation guarantees.

pub mod capabilities;
pub mod error;

pub use capabilities::{PlatformCapabilityReport, SandboxBackendKind};
pub use error::SandboxError;

use crate::appcontainer::AppContainerSandbox;
use crate::seatbelt::SeatbeltSandbox;
use m31a_core::config::SandboxPolicy;

pub struct SandboxPlatform;

impl SandboxPlatform {
    /// Discovers available sandbox backends on the current host.
    pub fn probe() -> PlatformCapabilityReport {
        let mut diagnostics = Vec::new();

        let bwrap_available = if cfg!(target_os = "linux") {
            let available = which::which("bwrap").ok().filter(|path| {
                std::process::Command::new(path)
                    .args(["--ro-bind", "/", "/", "/bin/true"])
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .status()
                    .map(|s| s.success())
                    .unwrap_or(false)
            }).is_some();
            if available {
                diagnostics.push("Functional Bubblewrap container engine detected.".to_string());
            } else {
                diagnostics.push("Bubblewrap unavailable or unprivileged user namespaces restricted.".to_string());
            }
            available
        } else {
            false
        };

        let seatbelt_available = if cfg!(target_os = "macos") {
            let available = SeatbeltSandbox::is_available();
            if available {
                diagnostics.push("Apple Seatbelt (sandbox-exec) framework detected.".to_string());
            } else {
                diagnostics.push("macOS Seatbelt sandbox-exec binary not available.".to_string());
            }
            available
        } else {
            false
        };

        let appcontainer_available = if cfg!(target_os = "windows") {
            let available = AppContainerSandbox::is_available();
            if available {
                diagnostics.push("Windows Win32 AppContainer isolation available.".to_string());
            } else {
                diagnostics.push("Windows AppContainer API not supported on this Windows release.".to_string());
            }
            available
        } else {
            false
        };

        let landlock_available = if cfg!(target_os = "linux") {
            crate::landlock::landlock_abi() > 0
        } else {
            false
        };

        let seccomp_available = cfg!(target_os = "linux");

        let primary_backend = if bwrap_available {
            SandboxBackendKind::Bubblewrap
        } else if seatbelt_available {
            SandboxBackendKind::Seatbelt
        } else if appcontainer_available {
            SandboxBackendKind::AppContainer
        } else {
            SandboxBackendKind::Unsandboxed
        };

        PlatformCapabilityReport {
            os: std::env::consts::OS,
            primary_backend,
            bwrap_available,
            seatbelt_available,
            appcontainer_available,
            landlock_available,
            seccomp_available,
            fail_closed_enforced: true,
            diagnostics,
        }
    }

    /// Validates that the requested policy can be strictly enforced.
    ///
    /// # Security Invariant: Fail-Closed
    /// If the policy requires isolation (`ReadOnly` or `WorkspaceWrite`) but
    /// no functional kernel sandbox boundary exists on the host, this returns
    /// `Err(SandboxError::IsolationUnavailable)`. Execution must NEVER silently
    /// fall back to unsandboxed host execution.
    pub fn assert_policy_enforceable(policy: SandboxPolicy) -> Result<SandboxBackendKind, SandboxError> {
        let caps = Self::probe();

        if policy == SandboxPolicy::DangerFullAccess {
            return Ok(caps.primary_backend);
        }

        if caps.has_kernel_sandbox() {
            Ok(caps.primary_backend)
        } else {
            Err(SandboxError::IsolationUnavailable {
                policy,
                reason: format!(
                    "No functional sandbox backend available on host OS '{}' (bwrap={}, seatbelt={}, appcontainer={}). Unsandboxed fallback rejected by fail-closed security policy.",
                    caps.os, caps.bwrap_available, caps.seatbelt_available, caps.appcontainer_available
                ),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_platform_probe_and_fail_closed_validation() {
        let caps = SandboxPlatform::probe();
        assert!(!caps.os.is_empty());

        // DangerFullAccess should always be allowed
        assert!(SandboxPlatform::assert_policy_enforceable(SandboxPolicy::DangerFullAccess).is_ok());

        // If host has sandbox, ReadOnly is allowed; if not, must fail closed
        let ro_res = SandboxPlatform::assert_policy_enforceable(SandboxPolicy::ReadOnly);
        if caps.has_kernel_sandbox() {
            assert!(ro_res.is_ok());
        } else {
            assert!(ro_res.is_err());
            match ro_res.unwrap_err() {
                SandboxError::IsolationUnavailable { policy, .. } => {
                    assert_eq!(policy, SandboxPolicy::ReadOnly);
                }
                other => panic!("Expected IsolationUnavailable error, got {:?}", other),
            }
        }
    }
}
