//! Credential-bearing file protection behind a portable contract.
//!
//! The security requirement is that private files are reachable only under
//! the intended owner policy. Unix enforces owner-only mode bits (0o600).
//! Windows restricts the file DACL to owner and administrators with
//! inheritance disabled. Hosts without the primitive must say so explicitly
//! instead of reporting success while leaving the file broadly readable.

use std::path::Path;

/// Outcome of attempting to restrict a private file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileSecurityOutcome {
    Enforced,
    Unsupported,
    Failed,
}

/// Restrict a file to owner-only access semantics.
///
/// Unix sets owner-read-write bits (0o600). Windows restricts the DACL to
/// owner+administrators with inheritance disabled. Other hosts return
/// `Unsupported` so the caller can fail closed instead of treating an
/// unenforced file as safe.
pub fn ensure_private_file(path: &Path) -> Result<FileSecurityOutcome, std::io::Error> {
    #[cfg(target_os = "linux")]
    {
        unix_enforce(path)
    }
    #[cfg(target_os = "macos")]
    {
        unix_enforce(path)
    }
    #[cfg(all(unix, not(target_os = "linux"), not(target_os = "macos")))]
    {
        unix_enforce(path)
    }
    #[cfg(windows)]
    {
        crate::platform::windows::acl::ensure_private_file_windows(path).map(|o| match o {
            crate::platform::windows::acl::AclOutcome::Enforced => FileSecurityOutcome::Enforced,
            crate::platform::windows::acl::AclOutcome::Unsupported => {
                FileSecurityOutcome::Unsupported
            }
            crate::platform::windows::acl::AclOutcome::Failed => FileSecurityOutcome::Failed,
        })
    }
    #[cfg(all(not(unix), not(windows)))]
    {
        let _ = path;
        Ok(FileSecurityOutcome::Unsupported)
    }
}

#[cfg(unix)]
fn unix_enforce(path: &Path) -> Result<FileSecurityOutcome, std::io::Error> {
    use std::os::unix::fs::PermissionsExt;
    let perms = std::fs::Permissions::from_mode(0o600);
    std::fs::set_permissions(path, perms)?;
    Ok(FileSecurityOutcome::Enforced)
}

/// Whether private-file enforcement is available on this host.
pub fn supports_private_files() -> bool {
    cfg!(target_os = "linux")
        || cfg!(target_os = "macos")
        || cfg!(all(
            unix,
            not(target_os = "linux"),
            not(target_os = "macos")
        ))
        || cfg!(windows)
}

/// Describe the enforcement mechanism for diagnostics.
pub fn enforcement_description() -> &'static str {
    #[cfg(target_os = "linux")]
    return "unix mode bits 0o600";
    #[cfg(target_os = "macos")]
    return "unix mode bits 0o600";
    #[cfg(all(unix, not(target_os = "linux"), not(target_os = "macos")))]
    return "unix mode bits 0o600";
    #[cfg(windows)]
    return "windows DACL (owner+administrators, no inheritance)";
    #[cfg(all(not(unix), not(windows)))]
    return "unsupported";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supports_files_on_targets() {
        #[cfg(any(target_os = "linux", target_os = "macos", windows, unix))]
        assert!(supports_private_files());
    }

    #[test]
    fn description_is_non_empty() {
        assert!(!enforcement_description().is_empty());
    }
}
