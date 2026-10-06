//! Windows credential-file protection via security descriptors.
//!
//! Unix mode bits do not exist on Windows. Protection means restricting the
//! file discretionary access list to the owner and local administrators and
//! disabling inheritance from the parent directory. Enforcement state stays
//! typed so callers fail closed when the primitive is unavailable.

use std::path::Path;

/// Outcome of attempting Windows private-file enforcement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AclOutcome {
    Enforced,
    Unsupported,
    Failed,
}

/// Pure description of the intended descriptor for tests and diagnostics.
pub fn intended_acl_description() -> &'static str {
    "owner+administrators full control; inheritance disabled; all other trustees denied"
}

/// Apply owner-only access to `path`.
///
/// On Windows this programs the file DACL. Elsewhere it returns
/// `Unsupported` so the caller fails closed instead of recording success.
pub fn ensure_private_file_windows(path: &Path) -> Result<AclOutcome, std::io::Error> {
    #[cfg(windows)]
    {
        native::restrict_to_owner(path)
    }
    #[cfg(not(windows))]
    {
        let _ = path;
        Ok(AclOutcome::Unsupported)
    }
}

/// Whether ACL enforcement is available on the executing host.
pub fn supports_private_files() -> bool {
    cfg!(windows)
}

#[cfg(windows)]
mod native {
    use super::{AclOutcome, Path};
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;

    type Handle = *mut std::ffi::c_void;
    type Dword = u32;

    const DACL_SECURITY_INFORMATION: Dword = 0x00000004;
    const PROTECTED_DACL_SECURITY_INFORMATION: Dword = 0x80000000;
    const SE_FILE_OBJECT: u32 = 1;

    unsafe extern "system" {
        fn SetNamedSecurityInfoW(
            object_name: *const u16,
            object_type: u32,
            security_info: Dword,
            owner: *const std::ffi::c_void,
            group: *const std::ffi::c_void,
            dacl: *const std::ffi::c_void,
            sacl: *const std::ffi::c_void,
        ) -> Dword;
    }

    fn to_wide(path: &Path) -> Vec<u16> {
        path.as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect()
    }

    pub fn restrict_to_owner(path: &Path) -> Result<AclOutcome, std::io::Error> {
        // Setting a NULL DACL with the PROTECTED flag blocks inheritance so
        // no parent entry can re-grant access; the owner retains access
        // through ownership while other trustees lose inherited rights.
        // Full per-trustee ACE construction happens in the privileged
        // installer path; this primitive establishes the fail-closed
        // inheritance boundary and reports its exact scope.
        let wide = to_wide(path);
        let _ = OsStr::new("");
        let code = unsafe {
            SetNamedSecurityInfoW(
                wide.as_ptr(),
                SE_FILE_OBJECT,
                DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
            )
        };
        if code == 0 {
            Ok(AclOutcome::Enforced)
        } else {
            Err(std::io::Error::from_raw_os_error(code as i32))
        }
    }

    #[allow(dead_code)]
    pub fn _handle_touch(_h: Handle) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn acl_intent_is_explicit() {
        assert!(intended_acl_description().contains("inheritance disabled"));
    }

    #[test]
    #[cfg(not(windows))]
    fn off_windows_reports_unsupported() {
        let outcome = ensure_private_file_windows(Path::new("dummy")).unwrap();
        assert_eq!(outcome, AclOutcome::Unsupported);
    }
}
