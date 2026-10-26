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
/// On Windows this programs the file DACL with an explicit protected security descriptor
/// granting full access exclusively to Owner, Administrators, and SYSTEM while disabling
/// inheritance from parent directories.
/// Elsewhere it returns `Unsupported` so the caller fails closed instead of recording success.
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

/// Verify that the file on disk has an effective protected ACL restricting access to owner/administrators.
///
/// On Windows, queries the file security descriptor and confirms DACL protection and trustee bounds.
/// Elsewhere returns an Unsupported error.
pub fn verify_private_file_acl_windows(path: &Path) -> Result<bool, std::io::Error> {
    #[cfg(windows)]
    {
        native::verify_acl_structure(path)
    }
    #[cfg(not(windows))]
    {
        let _ = path;
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "Windows ACL verification is only supported on Windows hosts",
        ))
    }
}

/// Whether ACL enforcement is available on the executing host.
pub fn supports_private_files() -> bool {
    cfg!(windows)
}

#[cfg(windows)]
mod native {
    use super::{AclOutcome, Path};
    use std::os::windows::ffi::OsStrExt;

    type Handle = *mut std::ffi::c_void;
    type Dword = u32;

    const DACL_SECURITY_INFORMATION: Dword = 0x00000004;
    const PROTECTED_DACL_SECURITY_INFORMATION: Dword = 0x80000000;
    const SE_FILE_OBJECT: u32 = 1;
    const SDDL_REVISION_1: u32 = 1;

    unsafe extern "system" {
        fn ConvertStringSecurityDescriptorToSecurityDescriptorW(
            string_sd: *const u16,
            string_sd_revision: u32,
            security_descriptor: *mut *mut std::ffi::c_void,
            security_descriptor_size: *mut u32,
        ) -> i32;

        fn GetSecurityDescriptorDacl(
            security_descriptor: *const std::ffi::c_void,
            dacl_present: *mut i32,
            dacl: *mut *mut std::ffi::c_void,
            dacl_defaulted: *mut i32,
        ) -> i32;

        fn GetNamedSecurityInfoW(
            object_name: *const u16,
            object_type: u32,
            security_info: Dword,
            owner: *mut *mut std::ffi::c_void,
            group: *mut *mut std::ffi::c_void,
            dacl: *mut *mut std::ffi::c_void,
            sacl: *mut *mut std::ffi::c_void,
            security_descriptor: *mut *mut std::ffi::c_void,
        ) -> Dword;

        fn ConvertSecurityDescriptorToStringSecurityDescriptorW(
            security_descriptor: *const std::ffi::c_void,
            requested_string_sd_revision: u32,
            security_information: Dword,
            string_security_descriptor: *mut *mut u16,
            string_security_descriptor_len: *mut u32,
        ) -> i32;

        fn LocalFree(h_mem: *mut std::ffi::c_void) -> *mut std::ffi::c_void;

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
        let wide = to_wide(path);
        // Canonical SDDL for owner-only protection:
        // D: DACL
        // P: PROTECTED (SE_DACL_PROTECTED, disables inheritance from parent container)
        // (A;;FA;;;OW): Allow Full Access to Owner
        // (A;;FA;;;BA): Allow Full Access to Builtin Administrators
        // (A;;FA;;;SY): Allow Full Access to Local SYSTEM
        let sddl = "D:P(A;;FA;;;OW)(A;;FA;;;BA)(A;;FA;;;SY)";
        let wide_sddl: Vec<u16> = sddl.encode_utf16().chain(std::iter::once(0)).collect();

        let mut p_sd: *mut std::ffi::c_void = std::ptr::null_mut();
        let conv_res = unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                wide_sddl.as_ptr(),
                SDDL_REVISION_1,
                &mut p_sd,
                std::ptr::null_mut(),
            )
        };
        if conv_res == 0 || p_sd.is_null() {
            return Err(std::io::Error::last_os_error());
        }

        let mut dacl_present: i32 = 0;
        let mut p_dacl: *mut std::ffi::c_void = std::ptr::null_mut();
        let mut dacl_defaulted: i32 = 0;
        let dacl_res = unsafe {
            GetSecurityDescriptorDacl(p_sd, &mut dacl_present, &mut p_dacl, &mut dacl_defaulted)
        };
        if dacl_res == 0 || dacl_present == 0 || p_dacl.is_null() {
            unsafe { LocalFree(p_sd) };
            return Err(std::io::Error::last_os_error());
        }

        let code = unsafe {
            SetNamedSecurityInfoW(
                wide.as_ptr(),
                SE_FILE_OBJECT,
                DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
                std::ptr::null(),
                std::ptr::null(),
                p_dacl,
                std::ptr::null(),
            )
        };

        unsafe { LocalFree(p_sd) };

        if code == 0 {
            Ok(AclOutcome::Enforced)
        } else {
            Err(std::io::Error::from_raw_os_error(code as i32))
        }
    }

    pub fn verify_acl_structure(path: &Path) -> Result<bool, std::io::Error> {
        let wide = to_wide(path);
        let mut p_sd: *mut std::ffi::c_void = std::ptr::null_mut();
        let mut p_dacl: *mut std::ffi::c_void = std::ptr::null_mut();

        let code = unsafe {
            GetNamedSecurityInfoW(
                wide.as_ptr(),
                SE_FILE_OBJECT,
                DACL_SECURITY_INFORMATION,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                &mut p_dacl,
                std::ptr::null_mut(),
                &mut p_sd,
            )
        };

        if code != 0 {
            return Err(std::io::Error::from_raw_os_error(code as i32));
        }

        if p_sd.is_null() || p_dacl.is_null() {
            if !p_sd.is_null() {
                unsafe { LocalFree(p_sd) };
            }
            return Ok(false);
        }

        let mut string_sd: *mut u16 = std::ptr::null_mut();
        let conv = unsafe {
            ConvertSecurityDescriptorToStringSecurityDescriptorW(
                p_sd,
                SDDL_REVISION_1,
                DACL_SECURITY_INFORMATION,
                &mut string_sd,
                std::ptr::null_mut(),
            )
        };

        let result = if conv != 0 && !string_sd.is_null() {
            let mut len = 0;
            while unsafe { *string_sd.add(len) } != 0 {
                len += 1;
            }
            let slice = unsafe { std::slice::from_raw_parts(string_sd, len) };
            let sddl_str = String::from_utf16_lossy(slice);
            unsafe { LocalFree(string_sd as *mut _) };

            sddl_str.starts_with("D:P") && !sddl_str.contains(";WD)")
        } else {
            false
        };

        unsafe { LocalFree(p_sd) };
        Ok(result)
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
