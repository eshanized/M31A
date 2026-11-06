//! Host Sandbox Engine Discovery, Capability Probing, and Engine Launcher.
//!
//! Inspects the host environment to detect available isolation mechanisms (Bubblewrap,
//! unprivileged user namespaces, Seccomp BPF filters, WSLg environments), determines
//! capabilities (such as `--ro-bind-fd` and `--as-pid-1` support), and selects the
//! optimal containment strategy.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Containment isolation mode selected for the host environment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContainmentStrategy {
    /// Full containerized isolation using Bubblewrap user namespaces.
    BubblewrapContainer,
    /// Hardened execution using process isolation, POSIX limits, and seccomp filters.
    ProcessHardeningDirect,
}

/// Detailed capabilities of the host system's container runtime.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SandboxEngineCapabilities {
    pub strategy: ContainmentStrategy,
    pub bwrap_path: Option<PathBuf>,
    pub bwrap_version: Option<String>,
    pub supports_ro_bind_fd: bool,
    pub supports_as_pid_1: bool,
    pub unprivileged_userns_allowed: bool,
    pub seccomp_available: bool,
    pub is_wsl: bool,
    pub is_wslg: bool,
}

impl SandboxEngineCapabilities {
    /// Formats a user-friendly diagnostic report.
    pub fn diagnostic_summary(&self) -> String {
        let mut lines = Vec::new();
        lines.push(format!("Sandbox Containment Strategy: {:?}", self.strategy));
        lines.push(format!(
            "Bubblewrap Path: {}",
            self.bwrap_path
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "not found".to_string())
        ));
        lines.push(format!(
            "Bubblewrap Version: {}",
            self.bwrap_version.as_deref().unwrap_or("unknown")
        ));
        lines.push(format!(
            "Supports --ro-bind-fd: {}",
            self.supports_ro_bind_fd
        ));
        lines.push(format!("Supports --as-pid-1: {}", self.supports_as_pid_1));
        lines.push(format!(
            "Unprivileged User Namespaces: {}",
            if self.unprivileged_userns_allowed {
                "enabled"
            } else {
                "disabled/restricted"
            }
        ));
        lines.push(format!(
            "Seccomp BPF: {}",
            if self.seccomp_available {
                "available"
            } else {
                "unavailable"
            }
        ));
        lines.push(format!("WSL Environment: {}", self.is_wsl));
        lines.push(format!("WSLg GUI: {}", self.is_wslg));
        lines.join("\n")
    }
}

/// Prober for inspecting host container and sandbox capabilities.
pub struct SandboxLauncherProbe;

impl SandboxLauncherProbe {
    /// Probe the current host system and determine the optimal sandbox containment strategy.
    pub fn probe() -> SandboxEngineCapabilities {
        let bwrap_path = which::which("bwrap").ok();
        let bwrap_version = bwrap_path
            .as_ref()
            .and_then(|p| Self::probe_bwrap_version(p));

        let unprivileged_userns_allowed = Self::check_unprivileged_userns();
        let seccomp_available = Self::check_seccomp_available();
        let is_wsl = Self::check_wsl();
        let is_wslg = Self::check_wslg();

        let (supports_ro_bind_fd, supports_as_pid_1) = if let Some(path) = &bwrap_path {
            Self::probe_bwrap_features(path)
        } else {
            (false, false)
        };

        // Decide containment strategy
        let strategy = if bwrap_path.is_some() && unprivileged_userns_allowed {
            ContainmentStrategy::BubblewrapContainer
        } else {
            ContainmentStrategy::ProcessHardeningDirect
        };

        SandboxEngineCapabilities {
            strategy,
            bwrap_path,
            bwrap_version,
            supports_ro_bind_fd,
            supports_as_pid_1,
            unprivileged_userns_allowed,
            seccomp_available,
            is_wsl,
            is_wslg,
        }
    }

    fn probe_bwrap_version(path: &Path) -> Option<String> {
        let output = Command::new(path).arg("--version").output().ok()?;
        if output.status.success() {
            let ver_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
            Some(ver_str)
        } else {
            None
        }
    }

    fn probe_bwrap_features(path: &Path) -> (bool, bool) {
        let output = Command::new(path).arg("--help").output().ok();
        if let Some(out) = output {
            let help = String::from_utf8_lossy(&out.stdout);
            let has_ro_bind_fd = help.contains("--ro-bind-fd");
            let has_as_pid_1 = help.contains("--as-pid-1");
            (has_ro_bind_fd, has_as_pid_1)
        } else {
            (false, false)
        }
    }

    fn check_unprivileged_userns() -> bool {
        // Check /proc/sys/kernel/unprivileged_userns_clone if present (Debian/Ubuntu older)
        let userns_sysctl = Path::new("/proc/sys/kernel/unprivileged_userns_clone");
        if userns_sysctl.exists()
            && let Ok(content) = fs::read_to_string(userns_sysctl)
            && content.trim() == "0"
        {
            return false;
        }

        // Check /proc/sys/kernel/apparmor_restrict_unprivileged_userns if present (Ubuntu 24.04+)
        let apparmor_sysctl = Path::new("/proc/sys/kernel/apparmor_restrict_unprivileged_userns");
        if apparmor_sysctl.exists()
            && let Ok(content) = fs::read_to_string(apparmor_sysctl)
            && content.trim() == "1"
        {
            return false;
        }

        // Active probe: check if creating user namespace actually succeeds
        #[cfg(target_os = "linux")]
        {
            if let Ok(bwrap) = which::which("bwrap") {
                let success = std::process::Command::new(bwrap)
                    .args(["--ro-bind", "/", "/", "/bin/true"])
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .status()
                    .map(|s| s.success())
                    .unwrap_or(false);
                if !success {
                    return false;
                }
            }
        }

        true
    }

    fn check_seccomp_available() -> bool {
        // PR_SET_SECCOMP is standard on Linux 3.5+
        unsafe {
            let res = libc::prctl(libc::PR_GET_SECCOMP, 0, 0, 0, 0);
            res >= 0
        }
    }

    fn check_wsl() -> bool {
        let osrelease = Path::new("/proc/sys/kernel/osrelease");
        if let Ok(content) = fs::read_to_string(osrelease) {
            let lower = content.to_ascii_lowercase();
            lower.contains("microsoft") || lower.contains("wsl")
        } else {
            false
        }
    }

    fn check_wslg() -> bool {
        Path::new("/mnt/wslg").exists()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sandbox_probe_execution() {
        let caps = SandboxLauncherProbe::probe();
        let summary = caps.diagnostic_summary();
        assert!(summary.contains("Sandbox Containment Strategy:"));
        assert!(summary.contains("Seccomp BPF:"));
    }

    #[test]
    fn test_diagnostic_summary_formatting() {
        let caps = SandboxEngineCapabilities {
            strategy: ContainmentStrategy::BubblewrapContainer,
            bwrap_path: Some(PathBuf::from("/usr/bin/bwrap")),
            bwrap_version: Some("bubblewrap 0.8.0".to_string()),
            supports_ro_bind_fd: true,
            supports_as_pid_1: true,
            unprivileged_userns_allowed: true,
            seccomp_available: true,
            is_wsl: false,
            is_wslg: false,
        };

        let summary = caps.diagnostic_summary();
        assert!(summary.contains("/usr/bin/bwrap"));
        assert!(summary.contains("bubblewrap 0.8.0"));
        assert!(summary.contains("Supports --ro-bind-fd: true"));
        assert!(summary.contains("Unprivileged User Namespaces: enabled"));
    }
}
