//! Operating-system abstraction boundary for the runtime.
//!
//! The runtime states requirements (terminate a worker tree, enforce a memory
//! budget, create a private file). Each operating system supplies the
//! mechanism underneath that requirement. Higher layers depend on the
//! contracts in this module rather than on raw system calls, fixed
//! filesystem paths, or shell names so native backends can vary without
//! rewriting runtime logic.
//!
//! Layout concentrates system conditionals here. Callers outside this
//! directory treat the public functions as the single source of truth for
//! host identity, capability reporting, and native behavior selection.

pub mod capabilities;
pub mod errors;
pub mod filesystem;
pub mod macos;
pub mod permissions;
pub mod process;
pub mod resources;
pub mod sandbox;
pub mod shell;
pub mod terminal;
pub mod windows;

pub use capabilities::{Capability, CapabilityState, PlatformCapabilities};
pub use errors::PlatformError;
pub use filesystem::HostFilesystem;
pub use permissions::{FileSecurityOutcome, ensure_private_file};
pub use process::{PlatformProcessError, terminate_process_tree_by_pid};
pub use resources::{LimitOutcome, ResourceBudget as PlatformResourceBudget};
pub use sandbox::{IsolationLevel, SandboxReadiness, describe_isolation};
pub use shell::{ShellAvailability, ShellRequest, native_shell_program};
pub use terminal::{TerminalBackend, TerminalCapabilities};

use serde::{Deserialize, Serialize};
use std::fmt;

/// Host operating-system family used for backend selection and diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PlatformFamily {
    Linux,
    Macos,
    Windows,
    OtherUnix,
    Unknown,
}

impl fmt::Display for PlatformFamily {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Linux => write!(f, "linux"),
            Self::Macos => write!(f, "macos"),
            Self::Windows => write!(f, "windows"),
            Self::OtherUnix => write!(f, "other-unix"),
            Self::Unknown => write!(f, "unknown"),
        }
    }
}

/// Canonical host description. There is exactly one authority for these
/// answers so diagnostics, release metadata, and backend selection agree.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlatformInfo {
    pub family: PlatformFamily,
    pub os_name: String,
    pub architecture: String,
    pub rust_target: String,
    pub process_backend: String,
    pub shell_backend: String,
    pub sandbox_backend: String,
}

impl PlatformInfo {
    /// Detect the host actually executing this binary.
    pub fn host() -> Self {
        let family = PlatformFamily::host();
        let (process_backend, shell_backend, sandbox_backend) = match family {
            PlatformFamily::Linux => (
                "linux-process-groups".to_string(),
                "posix-shell".to_string(),
                "bubblewrap-or-process-fallback".to_string(),
            ),
            PlatformFamily::Macos => (
                crate::platform::macos::process_backend_name().to_string(),
                "posix-shell".to_string(),
                "macos-seatbelt-or-foundation".to_string(),
            ),
            PlatformFamily::Windows => (
                crate::platform::windows::process_backend_name().to_string(),
                crate::platform::windows::shell_backend_name().to_string(),
                crate::platform::windows::sandbox_backend_name().to_string(),
            ),
            PlatformFamily::OtherUnix => (
                "generic-unix".to_string(),
                "posix-shell".to_string(),
                "process-fallback".to_string(),
            ),
            PlatformFamily::Unknown => (
                "unknown".to_string(),
                "unavailable".to_string(),
                "unavailable".to_string(),
            ),
        };
        Self {
            family,
            os_name: std::env::consts::OS.to_string(),
            architecture: std::env::consts::ARCH.to_string(),
            rust_target: current_rust_target().to_string(),
            process_backend,
            shell_backend,
            sandbox_backend,
        }
    }

    /// Human-readable one-line summary for diagnostics and release evidence.
    pub fn summary(&self) -> String {
        format!(
            "{} {} ({}, {})",
            self.os_name, self.architecture, self.family, self.rust_target
        )
    }
}

impl PlatformFamily {
    /// Detect the host family from compile-time target constants.
    pub fn host() -> Self {
        if cfg!(target_os = "linux") {
            Self::Linux
        } else if cfg!(target_os = "macos") {
            Self::Macos
        } else if cfg!(target_os = "windows") {
            Self::Windows
        } else if cfg!(unix) {
            Self::OtherUnix
        } else {
            Self::Unknown
        }
    }

    /// Backend chosen for a declared family. Used by tests to exercise
    /// backend selection without requiring the corresponding host.
    pub fn process_backend_name(&self) -> &'static str {
        match self {
            Self::Linux => "linux-process-groups",
            Self::Macos => "macos-process-groups-libproc",
            Self::Windows => "windows-job-objects",
            Self::OtherUnix => "generic-unix",
            Self::Unknown => "unknown",
        }
    }
}

/// Best-effort compile-time target triple for release metadata.
fn current_rust_target() -> &'static str {
    if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        "x86_64-unknown-linux-gnu"
    } else if cfg!(all(target_os = "linux", target_arch = "aarch64")) {
        "aarch64-unknown-linux-gnu"
    } else if cfg!(all(target_os = "macos", target_arch = "x86_64")) {
        "x86_64-apple-darwin"
    } else if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        "aarch64-apple-darwin"
    } else if cfg!(all(target_os = "windows", target_arch = "x86_64")) {
        "x86_64-pc-windows-msvc"
    } else if cfg!(all(target_os = "windows", target_arch = "aarch64")) {
        "aarch64-pc-windows-msvc"
    } else {
        "unknown-target"
    }
}

/// Bundled native implementations handed to runtime construction.
///
/// The object carries mechanisms only. Authorization, budgeting, and
/// completion decisions stay in the layers above it.
#[derive(Debug, Clone)]
pub struct PlatformServices {
    pub info: PlatformInfo,
    pub capabilities: PlatformCapabilities,
    pub terminal: TerminalCapabilities,
}

impl PlatformServices {
    /// Probe the executing host once and bundle the result.
    pub fn host() -> Self {
        Self {
            info: PlatformInfo::host(),
            capabilities: PlatformCapabilities::probe_host(),
            terminal: TerminalCapabilities::probe_host(),
        }
    }

    /// Deterministic constructor for a declared family. Reports the
    /// foundation-level capability profile for families whose native
    /// enforcement is intentionally deferred.
    pub fn for_family(family: PlatformFamily) -> Self {
        let info = PlatformInfo {
            family,
            os_name: family.to_string(),
            architecture: std::env::consts::ARCH.to_string(),
            rust_target: current_rust_target().to_string(),
            process_backend: family.process_backend_name().to_string(),
            shell_backend: match family {
                PlatformFamily::Linux | PlatformFamily::Macos | PlatformFamily::OtherUnix => {
                    "posix-shell".to_string()
                }
                _ => "windows-cmd-powershell".to_string(),
            },
            sandbox_backend: match family {
                PlatformFamily::Linux => "bubblewrap-or-process-fallback".to_string(),
                PlatformFamily::Macos => "seatbelt-or-foundation".to_string(),
                PlatformFamily::Windows => "job-object-containment".to_string(),
                _ => "foundation".to_string(),
            },
        };
        let capabilities = match family {
            PlatformFamily::Linux => PlatformCapabilities::probe_host(),
            _ => PlatformCapabilities::foundation_for(family),
        };
        let terminal = match family {
            PlatformFamily::Linux | PlatformFamily::Macos | PlatformFamily::OtherUnix => {
                TerminalCapabilities::unix_foundation()
            }
            PlatformFamily::Windows => TerminalCapabilities::windows_foundation(),
            PlatformFamily::Unknown => TerminalCapabilities::unavailable(),
        };
        Self {
            info,
            capabilities,
            terminal,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn platform_info_host_is_consistent() {
        let info = PlatformInfo::host();
        assert!(!info.process_backend.is_empty());
        assert!(!info.shell_backend.is_empty());
        assert!(!info.sandbox_backend.is_empty());
    }

    #[test]
    fn platform_services_host_probes_capabilities() {
        let services = PlatformServices::host();
        assert!(services.capabilities.environment_isolation == CapabilityState::Available);
    }
}
