//! Authenticate descriptor-backed mounts before sandboxed code can inherit them.

use std::collections::HashSet;
use std::fs;
use std::fs::File;
use std::io;
use std::os::fd::AsRawFd;
use std::os::fd::FromRawFd;
use std::os::unix::fs::MetadataExt;
use std::path::Path;

/// Verifies that all descriptor mounts are valid open file descriptors, not standard descriptors
/// (stdin/stdout/stderr), non-duplicate, have FD_CLOEXEC set, and match the destination device/inode.
pub fn verify_fd_mounts(mounts: &[String]) -> io::Result<()> {
    let mut claimed_descriptors = HashSet::with_capacity(mounts.len());

    for mount in mounts {
        let (descriptor, destination) = mount.split_once(':').ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("descriptor-backed mount must contain FD:DEST: {mount}"),
            )
        })?;
        let descriptor = descriptor.parse::<libc::c_int>().map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("descriptor-backed mount has an invalid descriptor: {mount}"),
            )
        })?;
        if descriptor <= libc::STDERR_FILENO {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("descriptor-backed mount cannot use a standard descriptor: {descriptor}"),
            ));
        }
        if !claimed_descriptors.insert(descriptor) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("descriptor-backed mount reuses descriptor {descriptor}"),
            ));
        }

        let flags = unsafe { libc::fcntl(descriptor, libc::F_GETFD) };
        if flags < 0 {
            return Err(io::Error::last_os_error());
        }
        // Take temporary reference to verify
        let file = unsafe { File::from_raw_fd(descriptor) };
        let result =
            unsafe { libc::fcntl(file.as_raw_fd(), libc::F_SETFD, flags | libc::FD_CLOEXEC) };
        if result < 0 {
            // Leak file to prevent close on drop if invalid fcntl
            std::mem::forget(file);
            return Err(io::Error::last_os_error());
        }

        let destination = Path::new(destination);
        if !destination.is_absolute() {
            std::mem::forget(file);
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "descriptor-backed mount destination must be absolute: {}",
                    destination.display()
                ),
            ));
        }

        let descriptor_metadata = file.metadata()?;
        let destination_metadata = fs::symlink_metadata(destination)?;
        if (descriptor_metadata.dev(), descriptor_metadata.ino())
            != (destination_metadata.dev(), destination_metadata.ino())
        {
            std::mem::forget(file);
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!(
                    "descriptor-backed mount does not match its destination: {}",
                    destination.display()
                ),
            ));
        }

        // Avoid dropping file descriptor so it remains available for mounting
        std::mem::forget(file);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verify_fd_mounts_invalid_format() {
        assert!(verify_fd_mounts(&["no_colon".to_string()]).is_err());
        assert!(verify_fd_mounts(&["abc:/dest".to_string()]).is_err());
        assert!(verify_fd_mounts(&["0:/dest".to_string()]).is_err()); // standard descriptor stdin
        assert!(verify_fd_mounts(&["1:/dest".to_string()]).is_err()); // standard descriptor stdout
        assert!(verify_fd_mounts(&["2:/dest".to_string()]).is_err()); // standard descriptor stderr
    }

    #[test]
    fn test_verify_fd_mounts_duplicate_descriptor() {
        assert!(verify_fd_mounts(&["5:/a".to_string(), "5:/b".to_string()]).is_err());
    }

    #[test]
    fn test_verify_fd_mounts_relative_dest() {
        // Descriptor 9999 is invalid on any typical machine, fails at fcntl F_GETFD
        assert!(verify_fd_mounts(&["9999:relative/path".to_string()]).is_err());
    }
}
