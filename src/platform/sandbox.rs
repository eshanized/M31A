//! Capability-based sandbox vocabulary.
//!
//! Higher layers request an isolation level. This module maps the request
//! onto what the host probe actually found. The bubblewrap binary remains a
//! Linux implementation detail; macOS uses Seatbelt/sandbox-exec; Windows
//! uses Job Object process containment. The runtime reasons about
//! filesystem or network isolation, never about a binary name.

use crate::platform::capabilities::{CapabilityState, PlatformCapabilities};
use serde::{Deserialize, Serialize};

/// Ordered isolation requirements understood across all backends.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum IsolationLevel {
    NoIsolation,
    WorkspaceContainment,
    ProcessIsolation,
    FilesystemIsolation,
    NetworkIsolation,
    FullSandbox,
}

impl IsolationLevel {
    /// Whether this level needs filesystem confinement.
    pub fn needs_filesystem(&self) -> bool {
        matches!(
            self,
            Self::FilesystemIsolation | Self::NetworkIsolation | Self::FullSandbox
        )
    }

    /// Whether this level needs network denial.
    pub fn needs_network(&self) -> bool {
        matches!(self, Self::NetworkIsolation | Self::FullSandbox)
    }
}

/// Whether a requested level can run on the probed host.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SandboxReadiness {
    Ready,
    Degraded,
    Unsupported,
}

/// Backend label for diagnostics. Never used for policy branching.
pub fn backend_name(capabilities: &PlatformCapabilities) -> &'static str {
    #[cfg(target_os = "linux")]
    {
        if capabilities.filesystem_isolation == CapabilityState::Available {
            "bubblewrap"
        } else if capabilities.sandboxing == CapabilityState::Degraded {
            "process-fallback"
        } else {
            "foundation"
        }
    }
    #[cfg(target_os = "macos")]
    {
        if capabilities.filesystem_isolation == CapabilityState::Degraded {
            "seatbelt-degraded"
        } else {
            "macos-foundation"
        }
    }
    #[cfg(windows)]
    {
        if capabilities.process_tree_control == CapabilityState::Available {
            "job-object-containment"
        } else {
            "windows-foundation"
        }
    }
    #[cfg(all(not(target_os = "linux"), not(target_os = "macos"), not(windows)))]
    {
        if capabilities.filesystem_isolation == CapabilityState::Available {
            "bubblewrap-or-native"
        } else if capabilities.sandboxing == CapabilityState::Degraded {
            "process-fallback"
        } else {
            "foundation"
        }
    }
}

/// Describe which isolation vocabulary the host supports.
pub fn describe_isolation(capabilities: &PlatformCapabilities) -> Vec<IsolationLevel> {
    let mut out = vec![
        IsolationLevel::NoIsolation,
        IsolationLevel::WorkspaceContainment,
    ];
    if capabilities.process_tree_control == CapabilityState::Available
        || capabilities.process_tree_control == CapabilityState::Degraded
    {
        out.push(IsolationLevel::ProcessIsolation);
    }
    if capabilities.filesystem_isolation == CapabilityState::Available
        || capabilities.filesystem_isolation == CapabilityState::Degraded
    {
        out.push(IsolationLevel::FilesystemIsolation);
    }
    if capabilities.sandboxing == CapabilityState::Available {
        out.push(IsolationLevel::NetworkIsolation);
        out.push(IsolationLevel::FullSandbox);
    }
    out
}

/// Fail-closed negotiation: required isolation must be present or the
/// operation is refused with a typed shortfall.
pub fn check_isolation_available(
    capabilities: &PlatformCapabilities,
    required: IsolationLevel,
) -> Result<SandboxReadiness, SandboxShortfall> {
    let supported = describe_isolation(capabilities);
    if supported.contains(&required) {
        // A degraded filesystem backend is usable only with explicit caller
        // acceptance: report Degraded, never silently Ready.
        if required.needs_filesystem()
            && capabilities.filesystem_isolation == CapabilityState::Degraded
        {
            return Ok(SandboxReadiness::Degraded);
        }
        return Ok(SandboxReadiness::Ready);
    }
    if required.needs_filesystem() && capabilities.filesystem_isolation == CapabilityState::Degraded
    {
        return Ok(SandboxReadiness::Degraded);
    }
    Err(SandboxShortfall { required })
}

/// Typed refusal naming the isolation that could not be provided.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SandboxShortfall {
    pub required: IsolationLevel,
}

impl std::fmt::Display for SandboxShortfall {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "required isolation {:?} is unavailable on this host",
            self.required
        )
    }
}

impl std::error::Error for SandboxShortfall {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::capabilities::{CapabilityState, PlatformCapabilities};

    #[test]
    fn describe_isolation_includes_process_isolation_when_tree_control_available() {
        let caps = PlatformCapabilities {
            process_tree_control: CapabilityState::Available,
            resource_limits: CapabilityState::Unsupported,
            filesystem_isolation: CapabilityState::Unsupported,
            environment_isolation: CapabilityState::Available,
            sandboxing: CapabilityState::Unsupported,
            secure_file_permissions: CapabilityState::Available,
            native_shell: CapabilityState::Available,
            terminal_control: CapabilityState::Available,
            filesystem_watching: CapabilityState::Available,
        };
        let levels = describe_isolation(&caps);
        assert!(levels.contains(&IsolationLevel::ProcessIsolation));
    }

    #[test]
    fn filesystem_isolation_degraded_allows_degraded_readiness() {
        let caps = PlatformCapabilities {
            process_tree_control: CapabilityState::Available,
            resource_limits: CapabilityState::Available,
            filesystem_isolation: CapabilityState::Degraded,
            environment_isolation: CapabilityState::Available,
            sandboxing: CapabilityState::Unsupported,
            secure_file_permissions: CapabilityState::Available,
            native_shell: CapabilityState::Available,
            terminal_control: CapabilityState::Available,
            filesystem_watching: CapabilityState::Available,
        };
        let result = check_isolation_available(&caps, IsolationLevel::FilesystemIsolation);
        assert_eq!(result, Ok(SandboxReadiness::Degraded));
    }
}
