//! Identify WSLg's duplicate root without relying on filtered environment variables
//! or kernel branding. An unrelated directory at the same path is left untouched.
//! Explicit alias paths are rejected before building the restricted filesystem.

use std::fs;
use std::io;
use std::os::unix::fs::MetadataExt;
use std::path::Path;

pub const WSL_INTEROP_DIR: &str = "/run/WSL";
pub const WSLG_DISTRO_ROOT: &str = "/mnt/wslg/distro";

/// Checks if the given path is a duplicate of the filesystem root (e.g. `/mnt/wslg/distro`).
pub fn is_duplicate_root(path: &Path) -> io::Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            let root = fs::metadata("/")?;
            Ok(metadata.is_dir() && (metadata.dev(), metadata.ino()) == (root.dev(), root.ino()))
        }
        Err(err)
            if matches!(
                err.kind(),
                io::ErrorKind::NotFound | io::ErrorKind::NotADirectory
            ) =>
        {
            Ok(false)
        }
        Err(_) => {
            let mountinfo = fs::read_to_string("/proc/self/mountinfo")?;
            let root = mount_identity(&mountinfo, "/")?
                .ok_or_else(|| io::Error::other("root mount identity is unavailable"))?;
            Ok(Some(root) == mount_identity(&mountinfo, &path.to_string_lossy())?)
        }
    }
}

/// Ensures the given path does not reside under WSLg's duplicate distro root.
pub fn ensure_supported_path(path: &Path) -> anyhow::Result<()> {
    if path.starts_with(WSLG_DISTRO_ROOT)
        || fs::canonicalize(path).is_ok_and(|resolved| resolved.starts_with(WSLG_DISTRO_ROOT))
    {
        anyhow::bail!(
            "restricted sandboxes do not support paths under {WSLG_DISTRO_ROOT}: {}; use the corresponding path under the primary filesystem root instead",
            path.display()
        );
    }
    Ok(())
}

fn mount_identity<'a>(mountinfo: &'a str, path: &str) -> io::Result<Option<(&'a str, &'a str)>> {
    let mut identities = mountinfo.lines().filter_map(|line| {
        let mut fields = line.split_ascii_whitespace();
        let device = fields.nth(2)?;
        let root = fields.next()?;
        let mountpoint = fields.next()?;
        (mountpoint == path).then_some((device, root))
    });
    let identity = identities.next();
    if identities.any(|other| Some(other) != identity) {
        return Err(io::Error::other("mount identity is ambiguous"));
    }
    Ok(identity)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mount_identity_parsing() {
        let mountinfo = "1 0 8:1 / / rw - ext4 /dev/sda1 rw\n\
                         2 1 8:1 / /mnt/wslg/distro rw - ext4 /dev/sda1 rw\n";
        let root_id = mount_identity(mountinfo, "/").unwrap();
        let wslg_id = mount_identity(mountinfo, "/mnt/wslg/distro").unwrap();
        assert_eq!(root_id, Some(("8:1", "/")));
        assert_eq!(wslg_id, Some(("8:1", "/")));
    }

    #[test]
    fn test_ensure_supported_path_blocks_wslg_distro() {
        assert!(ensure_supported_path(Path::new("/mnt/wslg/distro/etc/passwd")).is_err());
        assert!(ensure_supported_path(Path::new("/etc/passwd")).is_ok());
    }
}
