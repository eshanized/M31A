//! Terminal and console boundary.
//!
//! Runtime layers need dimensions, raw-mode control, and PTY availability.
//! Unix (Linux/macOS) supplies PTY-backed terminals; Windows uses ConPTY.

use crate::platform::capabilities::CapabilityState;
use serde::{Deserialize, Serialize};

/// Native terminal mechanism backing the semantic operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TerminalBackend {
    UnixPty,
    WindowsConPty,
    Unavailable,
}

/// Semantic terminal profile of the host.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalCapabilities {
    pub backend: TerminalBackend,
    pub raw_mode: CapabilityState,
    pub dimensions: CapabilityState,
    pub pty: CapabilityState,
}

impl TerminalCapabilities {
    /// Probe the executing host.
    pub fn probe_host() -> Self {
        #[cfg(any(target_os = "linux", target_os = "macos", all(unix, not(windows))))]
        {
            Self::unix_foundation()
        }
        #[cfg(windows)]
        {
            crate::platform::windows::conpty::terminal_capabilities()
        }
        #[cfg(all(
            not(any(target_os = "linux", target_os = "macos")),
            not(unix),
            not(windows)
        ))]
        {
            Self::unavailable()
        }
    }

    /// Unix terminal profile backed by PTY-capable consoles.
    pub fn unix_foundation() -> Self {
        Self {
            backend: TerminalBackend::UnixPty,
            raw_mode: CapabilityState::Available,
            dimensions: CapabilityState::Available,
            pty: CapabilityState::Available,
        }
    }

    /// Hosts with no usable console.
    pub fn unavailable() -> Self {
        Self {
            backend: TerminalBackend::Unavailable,
            raw_mode: CapabilityState::Unavailable,
            dimensions: CapabilityState::Unavailable,
            pty: CapabilityState::Unsupported,
        }
    }

    /// Windows foundation profile (used when native probe cannot run).
    pub fn windows_foundation() -> Self {
        Self {
            backend: TerminalBackend::WindowsConPty,
            raw_mode: CapabilityState::Degraded,
            dimensions: CapabilityState::Available,
            pty: CapabilityState::Unsupported,
        }
    }

    /// Whether interactive PTY features may be used here.
    pub fn supports_pty(&self) -> bool {
        self.pty == CapabilityState::Available
    }
}

/// Backend label for diagnostics.
pub fn backend_name() -> &'static str {
    #[cfg(any(target_os = "linux", target_os = "macos", all(unix, not(windows))))]
    {
        "unix-pty"
    }
    #[cfg(windows)]
    {
        "windows-conpty"
    }
    #[cfg(all(
        not(any(target_os = "linux", target_os = "macos")),
        not(unix),
        not(windows)
    ))]
    {
        "unavailable"
    }
}

/// Whether PTY-backed terminals are available on the executing host.
pub fn pty_available() -> bool {
    #[cfg(any(target_os = "linux", target_os = "macos", all(unix, not(windows))))]
    {
        std::path::Path::new("/dev/ptmx").exists() || std::path::Path::new("/dev/ptc").exists()
    }
    #[cfg(windows)]
    {
        crate::platform::windows::conpty::conpty_available()
    }
    #[cfg(all(
        not(any(target_os = "linux", target_os = "macos")),
        not(unix),
        not(windows)
    ))]
    {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_host_returns_foundation_on_unix() {
        #[cfg(any(target_os = "linux", target_os = "macos", all(unix, not(windows))))]
        {
            let caps = TerminalCapabilities::probe_host();
            assert_eq!(caps.backend, TerminalBackend::UnixPty);
            assert_eq!(caps.pty, CapabilityState::Available);
        }
    }

    #[test]
    fn backend_name_is_non_empty() {
        assert!(!backend_name().is_empty());
    }
}
