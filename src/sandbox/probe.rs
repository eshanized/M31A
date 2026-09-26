//! Platform introspection and sandbox capability detection probe (SND-02, SND-03).
//!
//! Detection facts come from the platform boundary. This probe keeps the
//! historical `SandboxCapabilities` shape so existing policy and provider
//! selection logic observes no behavior change on Linux.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::sandbox::capabilities::SandboxCapabilities;

/// System environment probe detecting available kernel and binary isolation mechanisms.
pub struct PlatformProbe;

impl PlatformProbe {
    /// Detect bubblewrap (`bwrap`) binary on host system.
    pub fn detect_bwrap_path() -> Option<PathBuf> {
        let candidates = [
            Path::new("/usr/bin/bwrap"),
            Path::new("/usr/sbin/bwrap"),
            Path::new("/bin/bwrap"),
        ];

        for candidate in candidates {
            if candidate.exists() && Self::verify_bwrap_executable(candidate) {
                return Some(candidate.to_path_buf());
            }
        }

        // Search PATH if not found in standard directories
        if let Ok(path_var) = std::env::var("PATH") {
            for dir in std::env::split_paths(&path_var) {
                let full = dir.join("bwrap");
                if full.exists() && Self::verify_bwrap_executable(&full) {
                    return Some(full);
                }
            }
        }

        None
    }

    /// Check that the bwrap candidate can execute and return its version.
    fn verify_bwrap_executable(path: &Path) -> bool {
        Command::new(path)
            .arg("--version")
            .output()
            .map(|out| out.status.success())
            .unwrap_or(false)
    }

    /// Probe unprivileged user namespace availability via the platform boundary.
    pub fn probe_user_namespaces() -> bool {
        crate::platform::filesystem::HostFilesystem::has_user_namespaces()
    }

    /// Probe unified control-group mount via the platform boundary.
    pub fn probe_cgroups_v2() -> bool {
        crate::platform::filesystem::HostFilesystem::has_cgroup_controllers()
    }

    /// Automatically probe and construct host platform capabilities matrix.
    pub fn probe_platform_capabilities() -> SandboxCapabilities {
        let bwrap_opt = Self::detect_bwrap_path();
        let userns = Self::probe_user_namespaces();
        let cgroups = Self::probe_cgroups_v2();

        if bwrap_opt.is_some() && userns {
            SandboxCapabilities {
                fs_read_only_root: true,
                fs_isolated_workspace_rw: true,
                fs_private_tempdir: true,
                fs_masked_credentials: true,
                net_deny_all: true,
                net_allowlist: false,
                proc_isolated_pid_ns: true,
                proc_group_isolation: true,
                proc_cgroup_v2: cgroups,
                proc_rlimit: true,
                user_namespaces: true,
            }
        } else {
            // Process isolation fallback
            SandboxCapabilities {
                fs_read_only_root: false,
                fs_isolated_workspace_rw: false,
                fs_private_tempdir: false,
                fs_masked_credentials: false,
                net_deny_all: false,
                net_allowlist: false,
                proc_isolated_pid_ns: false,
                proc_group_isolation: true,
                proc_cgroup_v2: false,
                proc_rlimit: true,
                user_namespaces: false,
            }
        }
    }
}
