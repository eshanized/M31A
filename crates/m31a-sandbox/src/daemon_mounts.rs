//! Find host mount aliases that need privileged socket directory masks.
//!
//! Mount roots describe filesystem identity; canonical paths alone miss bind mounts.
//! Btrfs subvolume device numbers can differ from the containing mount's device.

use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SocketFilesystem {
    Btrfs,
    Other,
}

/// Identifies all host paths and bind-mount aliases for `directory` that must be masked
/// to prevent sandboxed child processes from accessing sensitive daemon sockets.
pub fn daemon_socket_mask_paths(
    directory: &Path,
    masked_root: Option<&Path>,
) -> io::Result<BTreeSet<PathBuf>> {
    let directory_file = fs::File::open(directory)?;
    let metadata = directory_file.metadata()?;
    let device = metadata.dev();

    // Check filesystem type using statvfs or fallback
    let filesystem = SocketFilesystem::Other;

    // Read mount_id from /proc/self/fdinfo/<fd> if available on Linux
    let mount_id = {
        use std::os::fd::AsRawFd;
        let fd = directory_file.as_raw_fd();
        fs::read_to_string(format!("/proc/self/fdinfo/{fd}"))
            .ok()
            .and_then(|fdinfo| {
                fdinfo
                    .lines()
                    .find_map(|line| line.strip_prefix("mnt_id:"))
                    .and_then(|id| id.trim().parse::<u64>().ok())
            })
            .map(|id| id.to_string())
    };

    let mountinfo = fs::read("/proc/self/mountinfo")?;
    let major = ((device >> 8) & 0xfff) as u32;
    let minor = ((device & 0xff) | ((device >> 12) & 0xfff00)) as u32;
    let dev_str = format!("{}:{}", major, minor);

    check_mounts(
        directory,
        &dev_str,
        filesystem,
        mount_id.as_deref(),
        &mountinfo,
        masked_root,
    )
}

/// Core mount analysis routine operating on raw `mountinfo` bytes.
pub fn check_mounts(
    directory: &Path,
    device: &str,
    filesystem: SocketFilesystem,
    mount_id: Option<&str>,
    mountinfo: &[u8],
    masked_root: Option<&Path>,
) -> io::Result<BTreeSet<PathBuf>> {
    let invalid = || io::Error::other("cannot establish app-server socket mount isolation");
    let mut mounts = Vec::new();

    for line in mountinfo
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
    {
        let mut fields = line.split(|byte| *byte == b' ');
        let prefix: Vec<_> = fields.by_ref().take(5).collect();
        let [id, parent, mount_device, root, destination] = prefix.as_slice() else {
            return Err(invalid());
        };
        let destination = mount_path(destination)?;
        let mount_filesystem = fields
            .skip_while(|field| *field != b"-")
            .nth(1)
            .ok_or_else(invalid)?;
        mounts.push((
            *id,
            *parent,
            *mount_device,
            *root,
            destination,
            mount_filesystem,
        ));
    }

    let selected_index = mount_id
        .map(|mount_id| {
            let mut matching = mounts
                .iter()
                .enumerate()
                .filter(|(_, (id, ..))| *id == mount_id.as_bytes());
            let (index, (_, _, mount_device, _, _, mount_filesystem)) =
                matching.next().ok_or_else(invalid)?;
            if matching.next().is_some()
                || (*mount_device != device.as_bytes()
                    && (filesystem != SocketFilesystem::Btrfs || *mount_filesystem != b"btrfs"))
            {
                return Err(invalid());
            }
            Ok(index)
        })
        .transpose()?;

    // Btrfs reports a per-subvolume st_dev, while mountinfo uses the superblock device.
    let mount_device = selected_index
        .map(|index| mounts[index].2)
        .unwrap_or(device.as_bytes());

    let mounts = mounts
        .into_iter()
        .map(|(id, parent, candidate_device, root, destination, _)| {
            let root = (candidate_device == device.as_bytes() || candidate_device == mount_device)
                .then(|| mount_path(root))
                .transpose()?;
            Ok((id, parent, candidate_device, root, destination))
        })
        .collect::<io::Result<Vec<_>>>()?;

    let (location, containing_mount) = if let Some((mount_id, index)) = mount_id.zip(selected_index)
    {
        let selected = &mounts[index];
        let (_, _, _, root, destination) = selected;
        let root = root.as_ref().ok_or_else(invalid)?;
        let relative = directory.strip_prefix(destination).map_err(|_| invalid())?;
        let mut current = Some(selected);
        let mut visible_child: Option<&Path> = None;
        let mut visited = BTreeSet::new();

        while let Some((id, parent, _, _, destination)) = current {
            if !visited.insert(id)
                || mounts.iter().any(|(child_id, child_parent, _, _, child)| {
                    child_id != id
                        && child_parent == id
                        && directory.starts_with(child)
                        && !visible_child.is_some_and(|visible| child.starts_with(visible))
                })
            {
                return Err(invalid());
            }
            if id == parent {
                break;
            }
            visible_child = Some(destination);
            current = mounts.iter().find(|(id, ..)| id == parent);
        }
        (root.join(relative), Some((mount_id, destination)))
    } else {
        let locations: BTreeSet<_> = mounts
            .iter()
            .filter_map(|(_, _, _, root, destination)| {
                let root = root.as_ref()?;
                directory
                    .strip_prefix(destination)
                    .ok()
                    .map(|relative| root.join(relative))
            })
            .collect();
        if locations.len() != 1 {
            return Err(invalid());
        }
        (locations.into_iter().next().ok_or_else(invalid)?, None)
    };

    let mut mask_paths = BTreeSet::from([directory.to_path_buf()]);

    for (id, _, _, root, destination) in &mounts {
        let nested = destination != directory && destination.starts_with(directory);
        let alias = if let Some(root) = root {
            if let Ok(relative) = location.strip_prefix(root) {
                Some((destination.join(relative), !relative.as_os_str().is_empty()))
            } else if root.starts_with(&location) {
                Some((destination.clone(), false))
            } else {
                None
            }
        } else {
            None
        };

        let exposed_alias = alias.filter(|(path, _)| {
            let hidden = containing_mount.is_some_and(|(mount_id, containing_mount)| {
                *id != mount_id.as_bytes()
                    && containing_mount.starts_with(destination)
                    && path.starts_with(containing_mount)
            });
            !path.starts_with(directory)
                && !masked_root.is_some_and(|root| path.starts_with(root))
                && !hidden
        });

        if nested
            || exposed_alias
                .as_ref()
                .is_some_and(|(_, can_mask)| !can_mask)
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!(
                    "app-server socket directory has an unsupported host mount at {}; remove the bind-mount alias or nested mount before starting the sandbox",
                    destination.display()
                ),
            ));
        }

        if let Some((path, _)) = exposed_alias {
            mask_paths.insert(path);
        }
    }

    Ok(mask_paths)
}

/// Decodes octal escape sequences used by the Linux kernel in `/proc/self/mountinfo`.
pub fn mount_path(encoded: &[u8]) -> io::Result<PathBuf> {
    use std::os::unix::ffi::OsStringExt;
    let mut decoded = Vec::new();
    let mut bytes = encoded.iter().copied();
    while let Some(byte) = bytes.next() {
        decoded.push(if byte == b'\\' {
            let digits: Vec<_> = bytes.by_ref().take(3).collect();
            match digits.as_slice() {
                b"040" => b' ',
                b"011" => b'\t',
                b"012" => b'\n',
                b"134" => b'\\',
                _ => return Err(io::Error::other("invalid mountinfo path escape")),
            }
        } else {
            byte
        });
    }
    let path = PathBuf::from(std::ffi::OsString::from_vec(decoded));
    if !path.is_absolute() {
        return Err(io::Error::other("mountinfo path is not absolute"));
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check_mounts_simple(
        directory: &Path,
        device: &str,
        mount_id: Option<&str>,
        mountinfo: &[u8],
    ) -> io::Result<BTreeSet<PathBuf>> {
        check_mounts(
            directory,
            device,
            SocketFilesystem::Other,
            mount_id,
            mountinfo,
            None,
        )
    }

    fn check_btrfs_mounts(mount_id: &str, mountinfo: &[u8]) -> io::Result<BTreeSet<PathBuf>> {
        check_mounts(
            Path::new("/tmp/codex-daemon-1000"),
            "0:2",
            SocketFilesystem::Btrfs,
            Some(mount_id),
            mountinfo,
            None,
        )
    }

    #[test]
    fn test_mount_path_octal_decoding() {
        assert_eq!(
            mount_path(b"/home/user/my\\040folder").unwrap(),
            PathBuf::from("/home/user/my folder")
        );
        assert_eq!(
            mount_path(b"/var/tab\\011dir").unwrap(),
            PathBuf::from("/var/tab\tdir")
        );
        assert_eq!(
            mount_path(b"/slash\\134path").unwrap(),
            PathBuf::from("/slash\\path")
        );
    }

    #[test]
    fn test_btrfs_bind_mount_masks_aliases_for_both_devices() {
        let mounts = b"1 0 0:1 / / rw - btrfs disk rw\n\
                       2 1 0:1 /var/lib/system-tmp /tmp rw shared:1 - btrfs disk rw\n\
                       3 1 0:1 /var/lib/system-tmp /mount-device-alias rw - btrfs disk rw\n\
                       4 1 0:2 /var/lib/system-tmp /stat-device-alias rw - btrfs disk rw\n\
                       5 1 0:3 mnt:[1234] /run/example.mnt rw - nsfs nsfs rw\n";
        let result = check_btrfs_mounts("2", mounts).unwrap();
        assert_eq!(
            result,
            BTreeSet::from([
                PathBuf::from("/tmp/codex-daemon-1000"),
                PathBuf::from("/var/lib/system-tmp/codex-daemon-1000"),
                PathBuf::from("/mount-device-alias/codex-daemon-1000"),
                PathBuf::from("/stat-device-alias/codex-daemon-1000"),
            ])
        );
    }

    #[test]
    fn test_device_mismatch_requires_verified_btrfs() {
        let mounts = b"1 0 0:1 / / rw - ext4 disk rw\n";
        let res = check_mounts(
            Path::new("/tmp/codex-daemon-1000"),
            "0:2",
            SocketFilesystem::Btrfs,
            Some("1"),
            mounts,
            None,
        );
        assert!(res.is_err());
    }

    #[test]
    fn test_ancestor_alias_identified() {
        let mounts = b"1 0 8:1 / / rw - ext4 /dev/sda1 rw\n\
                       2 1 8:1 /tmp /host-tmp rw - ext4 /dev/sda1 rw\n";
        let res = check_mounts_simple(
            Path::new("/tmp/codex-daemon-1000"),
            "8:1",
            Some("1"),
            mounts,
        )
        .unwrap();
        assert!(res.contains(&PathBuf::from("/tmp/codex-daemon-1000")));
        assert!(res.contains(&PathBuf::from("/host-tmp/codex-daemon-1000")));
    }

    #[test]
    fn test_nested_mount_rejected() {
        let mounts = b"1 0 8:1 / / rw - ext4 /dev/sda1 rw\n\
                       2 1 8:2 /nested /tmp/codex-daemon-1000/nested rw - tmpfs tmpfs rw\n";
        let res = check_mounts_simple(
            Path::new("/tmp/codex-daemon-1000"),
            "8:1",
            Some("1"),
            mounts,
        );
        assert!(res.is_err());
        assert_eq!(res.unwrap_err().kind(), io::ErrorKind::PermissionDenied);
    }
}
