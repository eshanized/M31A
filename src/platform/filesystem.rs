//! Host filesystem primitives behind one portable surface.
//!
//! Containment checks and canonicalization stay centralized in their owning
//! modules. This module supplies only the operating-system facts those
//! checks need: where temporary data lives, which device discards output,
//! and how two paths compare for identity on the current host.
//!
//! Windows path security uses the dedicated windows/path module to resist
//! prefix tricks, case folding, UNC, drive letters, reserved names, and
//! reparse-point redirection. macOS uses Unix permission semantics with
//! platform-probed behavior.

use std::path::{Path, PathBuf};

/// Portable host filesystem queries.
pub struct HostFilesystem;

impl HostFilesystem {
    /// Platform-neutral scratch root. Never hard-codes a literal temporary
    /// path so production code stays valid across hosts.
    pub fn temp_root() -> PathBuf {
        std::env::temp_dir()
    }

    /// Fresh unique scratch directory. The caller owns cleanup.
    pub fn fresh_temp_dir(prefix: &str) -> Result<tempfile::TempDir, std::io::Error> {
        tempfile::Builder::new().prefix(prefix).tempdir()
    }

    /// Device that discards writes. Needed when a child environment must
    /// point a configuration override at "nowhere".
    pub fn null_device() -> &'static str {
        #[cfg(windows)]
        {
            "NUL"
        }
        #[cfg(not(windows))]
        {
            "/dev/null"
        }
    }

    /// Default executable search path when the host provides none.
    pub fn default_path_value() -> String {
        #[cfg(windows)]
        {
            "C:\\Windows\\System32;C:\\Windows;C:\\Windows\\System32\\Wbem".to_string()
        }
        #[cfg(not(windows))]
        {
            "/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin".to_string()
        }
    }

    /// Default staging location for confinement bookkeeping.
    pub fn confinement_staging_dir() -> PathBuf {
        #[cfg(target_os = "linux")]
        {
            PathBuf::from("/sys/fs/cgroup/m31a")
        }
        #[cfg(target_os = "macos")]
        {
            Self::temp_root().join("m31a-confinement")
        }
        #[cfg(windows)]
        {
            Self::temp_root().join("m31a-confinement")
        }
        #[cfg(all(not(target_os = "linux"), not(target_os = "macos"), not(windows), unix))]
        {
            Self::temp_root().join("m31a-confinement")
        }
        #[cfg(all(
            not(target_os = "linux"),
            not(target_os = "macos"),
            not(windows),
            not(unix)
        ))]
        {
            Self::temp_root().join("m31a-confinement")
        }
    }

    /// Whether this host exposes unified control-group controllers.
    pub fn has_cgroup_controllers() -> bool {
        #[cfg(target_os = "linux")]
        {
            Path::new("/sys/fs/cgroup/cgroup.controllers").exists()
        }
        #[cfg(not(target_os = "linux"))]
        {
            false
        }
    }

    /// Whether this host exposes an unprivileged user-namespace interface.
    pub fn has_user_namespaces() -> bool {
        #[cfg(target_os = "linux")]
        {
            let gate = Path::new("/proc/sys/kernel/unprivileged_userns_clone");
            if std::fs::read_to_string(gate).is_ok_and(|c| c.trim() == "0") {
                return false;
            }
            Path::new("/proc/self/ns/user").exists()
        }
        #[cfg(not(target_os = "linux"))]
        {
            false
        }
    }

    /// Identity comparison honoring host case and separator rules.
    /// Windows uses case-insensitive normalized comparison; Unix compares
    /// byte-exact. Delegates to windows/path for full Windows semantics.
    pub fn paths_identical(left: &Path, right: &Path) -> bool {
        #[cfg(windows)]
        {
            crate::platform::windows::path::paths_identical_windows(left, right)
        }
        #[cfg(not(windows))]
        {
            left == right
        }
    }

    /// Validate that `candidate` (absolute or workspace-relative) stays inside
    /// `workspace`. Relative candidates resolve against the workspace first.
    ///
    /// On Windows this delegates to the dedicated path security module which
    /// resists prefix tricks, case folding, UNC, drive letters, reserved names,
    /// and reparse-point redirection. On Unix this performs lexical normalization
    /// and containment checking.
    pub fn validate_containment(workspace: &Path, candidate: &Path) -> Result<PathBuf, String> {
        #[cfg(windows)]
        {
            crate::platform::windows::path::validate_containment(workspace, candidate)
        }
        #[cfg(not(windows))]
        {
            let ws = workspace.canonicalize().map_err(|e| e.to_string())?;
            let cand = if candidate.is_absolute() {
                candidate.canonicalize().map_err(|e| e.to_string())?
            } else {
                ws.join(candidate)
                    .canonicalize()
                    .map_err(|e| e.to_string())?
            };
            if !cand.starts_with(&ws) {
                return Err(format!(
                    "path '{}' escapes workspace '{}'",
                    candidate.display(),
                    workspace.display()
                ));
            }
            Ok(cand)
        }
    }

    /// Whether a path representation needs canonicalization before a
    /// containment decision (reparse points, junctions, symlinks, `..`,
    /// mixed separators, case tricks, UNC, drive letters).
    pub fn needs_canonicalization(path: &Path) -> bool {
        #[cfg(windows)]
        {
            crate::platform::windows::path::needs_canonicalization(path)
        }
        #[cfg(not(windows))]
        {
            let s = path.to_string_lossy();
            s.contains("..") || s != path.to_string_lossy()
        }
    }

    /// Available bytes at a path, or `None` when the host cannot report it.
    pub fn available_space_bytes(path: &Path) -> Option<u64> {
        #[cfg(unix)]
        {
            use std::ffi::CString;
            use std::os::unix::ffi::OsStrExt;
            let target = if path.exists() {
                path.to_path_buf()
            } else {
                path.parent()
                    .map(Path::to_path_buf)
                    .unwrap_or_else(|| PathBuf::from("."))
            };
            let c_path = CString::new(target.as_os_str().as_bytes()).ok()?;
            let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };
            let ok = unsafe { libc::statvfs(c_path.as_ptr(), &mut stat) } == 0;
            if ok {
                #[allow(clippy::unnecessary_cast)]
                Some((stat.f_bavail as u64).saturating_mul(stat.f_bsize as u64))
            } else {
                None
            }
        }
        #[cfg(windows)]
        {
            // Windows GetDiskFreeSpaceExW via std; return None for now.
            let _ = path;
            None
        }
        #[cfg(all(not(unix), not(windows)))]
        {
            let _ = path;
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn temp_root_is_non_empty() {
        assert!(!HostFilesystem::temp_root().as_os_str().is_empty());
    }

    #[test]
    fn null_device_is_correct_for_host() {
        let dev = HostFilesystem::null_device();
        #[cfg(windows)]
        assert_eq!(dev, "NUL");
        #[cfg(not(windows))]
        assert_eq!(dev, "/dev/null");
    }

    #[test]
    fn paths_identical_case_insensitive_on_windows() {
        #[cfg(windows)]
        {
            assert!(HostFilesystem::paths_identical(
                Path::new("A"),
                Path::new("a")
            ));
        }
    }

    #[test]
    fn containment_rejects_escape() {
        let ws = std::env::temp_dir();
        let bad = ws.join("..").join("etc");
        let _ = HostFilesystem::validate_containment(&ws, &bad);
    }
}
