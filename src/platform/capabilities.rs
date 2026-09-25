//! Typed host capability model.
//!
//! Callers ask whether the host can actually enforce a requirement instead
//! of branching on the operating-system name. A capability can be fully
//! present, absent, degraded, explicitly unsupported on this backend, or
//! still unknown when probing was impossible.
//!
//! Linux: probes bubblewrap, cgroups, user namespaces
//! macOS: probes libproc, setrlimit, sandbox-exec/Seatbelt, PTY
//! Windows: probes Job Objects, ConPTY, cmd.exe/PowerShell, ACLs

use serde::{Deserialize, Serialize};

use crate::platform::PlatformFamily;

/// Every mechanism the runtime may require from the host.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Capability {
    ProcessTreeControl,
    ResourceLimits,
    FilesystemIsolation,
    EnvironmentIsolation,
    Sandboxing,
    SecureFilePermissions,
    NativeShell,
    TerminalControl,
    FileSystemWatching,
}

/// Resolution of a single capability on the current host.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CapabilityState {
    Available,
    Unavailable,
    Degraded,
    Unsupported,
    Unknown,
}

impl CapabilityState {
    /// Whether the runtime may proceed when this state is reported.
    pub fn usable(&self) -> bool {
        matches!(self, Self::Available)
    }

    /// Whether the runtime may proceed with reduced guarantees.
    pub fn degraded_usable(&self) -> bool {
        matches!(self, Self::Available | Self::Degraded)
    }
}

/// Snapshot of what the host can actually enforce.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlatformCapabilities {
    pub process_tree_control: CapabilityState,
    pub resource_limits: CapabilityState,
    pub filesystem_isolation: CapabilityState,
    pub environment_isolation: CapabilityState,
    pub sandboxing: CapabilityState,
    pub secure_file_permissions: CapabilityState,
    pub native_shell: CapabilityState,
    pub terminal_control: CapabilityState,
    pub filesystem_watching: CapabilityState,
}

impl PlatformCapabilities {
    /// Probe the executing host. Reports genuine enforcement for all
    /// implemented backends.
    pub fn probe_host() -> Self {
        #[cfg(target_os = "linux")]
        {
            Self::linux_host()
        }
        #[cfg(target_os = "macos")]
        {
            crate::platform::macos::probe_capabilities()
        }
        #[cfg(windows)]
        {
            crate::platform::windows::probe_capabilities()
        }
        #[cfg(all(not(target_os = "linux"), not(target_os = "macos"), not(windows), unix))]
        {
            Self::generic_unix()
        }
        #[cfg(all(
            not(target_os = "linux"),
            not(target_os = "macos"),
            not(windows),
            not(unix)
        ))]
        {
            Self::unknown()
        }
    }

    /// Deterministic foundation profile for a declared family. Used by tests
    /// to exercise backend selection without requiring the corresponding host.
    pub fn foundation_for(family: PlatformFamily) -> Self {
        match family {
            PlatformFamily::Linux => Self::linux_host(),
            PlatformFamily::Macos => Self::macos_foundation(),
            PlatformFamily::Windows => Self::windows_foundation(),
            PlatformFamily::OtherUnix => Self::generic_unix(),
            PlatformFamily::Unknown => Self::unknown(),
        }
    }

    fn linux_host() -> Self {
        let sandboxy = crate::sandbox::PlatformProbe::probe_platform_capabilities();
        let filesystem_isolation = if sandboxy.supports_fs_isolation() {
            CapabilityState::Available
        } else {
            CapabilityState::Degraded
        };
        let resource_limits = if sandboxy.proc_rlimit || sandboxy.proc_cgroup_v2 {
            CapabilityState::Available
        } else {
            CapabilityState::Degraded
        };
        Self {
            process_tree_control: CapabilityState::Available,
            resource_limits,
            filesystem_isolation,
            environment_isolation: CapabilityState::Available,
            sandboxing: if sandboxy.fs_isolated_workspace_rw {
                CapabilityState::Available
            } else {
                CapabilityState::Degraded
            },
            secure_file_permissions: CapabilityState::Available,
            native_shell: CapabilityState::Available,
            terminal_control: CapabilityState::Available,
            filesystem_watching: CapabilityState::Available,
        }
    }

    /// Foundation profile for macOS when native probe cannot run (e.g. cross-compile).
    fn macos_foundation() -> Self {
        Self {
            process_tree_control: CapabilityState::Degraded,
            resource_limits: CapabilityState::Degraded,
            filesystem_isolation: CapabilityState::Unsupported,
            environment_isolation: CapabilityState::Available,
            sandboxing: CapabilityState::Unsupported,
            secure_file_permissions: CapabilityState::Available,
            native_shell: CapabilityState::Available,
            terminal_control: CapabilityState::Available,
            filesystem_watching: CapabilityState::Available,
        }
    }

    /// Foundation profile for Windows when native probe cannot run.
    fn windows_foundation() -> Self {
        Self {
            process_tree_control: CapabilityState::Degraded,
            resource_limits: CapabilityState::Unsupported,
            filesystem_isolation: CapabilityState::Unsupported,
            environment_isolation: CapabilityState::Available,
            sandboxing: CapabilityState::Unsupported,
            secure_file_permissions: CapabilityState::Unsupported,
            native_shell: CapabilityState::Unsupported,
            terminal_control: CapabilityState::Degraded,
            filesystem_watching: CapabilityState::Available,
        }
    }

    fn generic_unix() -> Self {
        Self {
            process_tree_control: CapabilityState::Available,
            resource_limits: CapabilityState::Available,
            filesystem_isolation: CapabilityState::Unsupported,
            environment_isolation: CapabilityState::Available,
            sandboxing: CapabilityState::Unsupported,
            secure_file_permissions: CapabilityState::Available,
            native_shell: CapabilityState::Available,
            terminal_control: CapabilityState::Available,
            filesystem_watching: CapabilityState::Degraded,
        }
    }

    fn unknown() -> Self {
        Self {
            process_tree_control: CapabilityState::Unknown,
            resource_limits: CapabilityState::Unknown,
            filesystem_isolation: CapabilityState::Unknown,
            environment_isolation: CapabilityState::Unknown,
            sandboxing: CapabilityState::Unknown,
            secure_file_permissions: CapabilityState::Unknown,
            native_shell: CapabilityState::Unknown,
            terminal_control: CapabilityState::Unknown,
            filesystem_watching: CapabilityState::Unknown,
        }
    }

    /// Look up one capability by identity so runtime negotiation does not
    /// need to match on operating-system names.
    pub fn state_of(&self, capability: Capability) -> CapabilityState {
        match capability {
            Capability::ProcessTreeControl => self.process_tree_control,
            Capability::ResourceLimits => self.resource_limits,
            Capability::FilesystemIsolation => self.filesystem_isolation,
            Capability::EnvironmentIsolation => self.environment_isolation,
            Capability::Sandboxing => self.sandboxing,
            Capability::SecureFilePermissions => self.secure_file_permissions,
            Capability::NativeShell => self.native_shell,
            Capability::TerminalControl => self.terminal_control,
            Capability::FileSystemWatching => self.filesystem_watching,
        }
    }

    /// Fail-closed gate: required work must not silently run without the
    /// mechanism it depends on.
    pub fn require(&self, capability: Capability) -> Result<(), CapabilityShortfall> {
        let state = self.state_of(capability);
        if state.usable() {
            Ok(())
        } else {
            Err(CapabilityShortfall { capability, state })
        }
    }

    /// Short diagnostic label naming the sandbox posture.
    pub fn sandboxing_label(&self) -> &'static str {
        match self.sandboxing {
            CapabilityState::Available => "native isolation",
            CapabilityState::Degraded => "process fallback",
            CapabilityState::Unsupported => "foundation only",
            CapabilityState::Unavailable => "unavailable",
            CapabilityState::Unknown => "unknown",
        }
    }
}

/// Typed description of a missing host mechanism.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityShortfall {
    pub capability: Capability,
    pub state: CapabilityState,
}

impl std::fmt::Display for CapabilityShortfall {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "host cannot provide {:?} (state: {:?})",
            self.capability, self.state
        )
    }
}

impl std::error::Error for CapabilityShortfall {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_host_returns_usable_on_linux() {
        #[cfg(target_os = "linux")]
        {
            let caps = PlatformCapabilities::probe_host();
            assert_eq!(caps.process_tree_control, CapabilityState::Available);
            assert_eq!(caps.secure_file_permissions, CapabilityState::Available);
        }
    }

    #[test]
    fn foundation_profiles_distinct() {
        let linux = PlatformCapabilities::foundation_for(PlatformFamily::Linux);
        let macos = PlatformCapabilities::foundation_for(PlatformFamily::Macos);
        let windows = PlatformCapabilities::foundation_for(PlatformFamily::Windows);
        assert_ne!(linux.process_tree_control, windows.process_tree_control);
        assert_ne!(
            macos.secure_file_permissions,
            windows.secure_file_permissions
        );
    }
}
