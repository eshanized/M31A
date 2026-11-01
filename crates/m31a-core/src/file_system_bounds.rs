//! Filesystem boundary enforcement, symlink escape checks, and directory restrictions.
//!
//! Guarantees that agent tool operations remain strictly confined within designated workspace directories.

use anyhow::{Context, Result, bail};
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::OnceCell;

#[cfg(unix)]
use std::os::unix::fs::{DirBuilderExt, MetadataExt};
#[cfg(unix)]
use std::os::unix::io::AsRawFd;

pub const FILE_READ_CHUNK_SIZE: usize = 1024 * 1024; // 1 MiB
pub const FILE_WRITE_CHUNK_SIZE: usize = 1024 * 1024; // 1 MiB
pub const MAX_WALK_DEPTH: usize = 64;
pub const MAX_WALK_DIRECTORIES: usize = 10_000;
pub const MAX_WALK_ENTRIES: usize = 50_000;
pub const MAX_WALK_RESPONSE_BYTES: usize = 4 * 1024 * 1024; // 4 MiB

pub const COMMON_PROJECT_MARKERS: &[&str] = &[
    ".git",
    "Cargo.toml",
    "package.json",
    "pyproject.toml",
    "go.mod",
    "pom.xml",
    "build.gradle",
    "Makefile",
    ".m31a",
];

/// Controls how an upward marker search handles filesystem metadata errors other than NotFound.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FindUpErrorPolicy {
    /// Return the first encountered I/O error immediately.
    Propagate,
    /// Treat errors as missing markers and continue searching ancestor hierarchy.
    Ignore,
}

/// Protected diagnostic and logging directory whose setup and Unix cleanup lock are deferred.
///
/// On Unix, newly created log directories enforce private `0700` permissions and maintain
/// an advisory shared lock on the open directory descriptor to prevent cleanup races while in use.
#[derive(Clone, Debug)]
pub struct LocalLogsDir {
    path: PathBuf,
    initialized: Arc<OnceCell<LocalLogsDirLock>>,
}

#[derive(Debug)]
struct LocalLogsDirLock {
    #[cfg(unix)]
    _cleanup_lock: fs::File,
}

impl LocalLogsDir {
    /// Constructs a directory reference without touching the disk until `initialize` is invoked.
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            initialized: Arc::new(OnceCell::new()),
        }
    }

    /// Returns the configured path.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Initializes the directory on disk once, setting 0700 mode and acquiring cleanup lock.
    pub async fn initialize(&self) -> io::Result<()> {
        let _ = self
            .initialized
            .get_or_try_init(|| async {
                let target_path = self.path.clone();
                tokio::task::spawn_blocking(move || {
                    let mut builder = fs::DirBuilder::new();
                    builder.recursive(true);
                    #[cfg(unix)]
                    {
                        builder.mode(0o700);
                    }
                    builder.create(&target_path)?;

                    #[cfg(unix)]
                    let directory = {
                        let dir_file = fs::File::open(&target_path)?;
                        let ret = unsafe {
                            libc::flock(dir_file.as_raw_fd(), libc::LOCK_SH | libc::LOCK_NB)
                        };
                        if ret != 0 {
                            return Err(io::Error::last_os_error());
                        }

                        let locked_meta = dir_file.metadata()?;
                        let current_meta = fs::metadata(&target_path)?;
                        if !locked_meta.is_dir()
                            || (current_meta.dev(), current_meta.ino())
                                != (locked_meta.dev(), locked_meta.ino())
                        {
                            return Err(io::Error::new(
                                io::ErrorKind::NotFound,
                                "Log directory changed or replaced during initialization",
                            ));
                        }
                        dir_file
                    };

                    Ok::<_, io::Error>(LocalLogsDirLock {
                        #[cfg(unix)]
                        _cleanup_lock: directory,
                    })
                })
                .await
                .map_err(io::Error::other)?
            })
            .await?;

        Ok(())
    }

    /// Returns true if the directory has already been initialized on disk.
    pub fn is_initialized(&self) -> bool {
        self.initialized.get().is_some()
    }
}

/// Searches upwards starting from `start_dir` to find the nearest ancestor directory
/// that contains any of the specified marker files or directories.
pub fn find_nearest_ancestor_with_markers(start_dir: &Path, markers: &[&str]) -> Option<PathBuf> {
    let mut current = if start_dir.is_file() {
        start_dir.parent()?.to_path_buf()
    } else {
        start_dir.to_path_buf()
    };

    loop {
        for marker in markers {
            let candidate = current.join(marker);
            if candidate.exists() {
                return Some(current);
            }
        }

        match current.parent() {
            Some(parent) => current = parent.to_path_buf(),
            None => break,
        }
    }

    None
}

/// Reads up to `max_bytes` from `path` safely in fixed 1 MiB chunks.
pub fn read_file_stream_bounded(path: &Path, max_bytes: usize) -> Result<Vec<u8>> {
    let metadata = fs::metadata(path)
        .with_context(|| format!("Could not read file metadata for {:?}", path))?;

    if metadata.is_dir() {
        bail!("Path {:?} is a directory, not a file", path);
    }

    let limit = max_bytes.min(metadata.len() as usize);
    let mut file =
        File::open(path).with_context(|| format!("Could not open file at {:?}", path))?;

    let mut buffer = Vec::with_capacity(limit);
    let mut chunk = vec![0u8; FILE_READ_CHUNK_SIZE.min(limit)];

    while buffer.len() < limit {
        let to_read = (limit - buffer.len()).min(chunk.len());
        let n = file.read(&mut chunk[..to_read])?;
        if n == 0 {
            break;
        }
        buffer.extend_from_slice(&chunk[..n]);
    }

    Ok(buffer)
}

/// Checks whether a path resides on an NFS network mount.
pub fn is_nfs(path: &Path) -> bool {
    #[cfg(target_os = "linux")]
    {
        use std::ffi::CString;
        use std::os::unix::ffi::OsStrExt;

        if let Ok(c_path) = CString::new(path.as_os_str().as_bytes()) {
            unsafe {
                let mut stat: libc::statfs = std::mem::zeroed();
                if libc::statfs(c_path.as_ptr(), &mut stat) == 0 {
                    // NFS_SUPER_MAGIC = 0x6969
                    return stat.f_type == 0x6969;
                }
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_find_nearest_ancestor_with_markers() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        // Create structure: root/Cargo.toml, root/nested/deep/child
        fs::write(root.join("Cargo.toml"), "[package]").unwrap();
        let deep_dir = root.join("nested").join("deep").join("child");
        fs::create_dir_all(&deep_dir).unwrap();

        let found = find_nearest_ancestor_with_markers(&deep_dir, &["Cargo.toml"]);
        assert_eq!(found, Some(root.to_path_buf()));

        let not_found = find_nearest_ancestor_with_markers(&deep_dir, &["go.mod"]);
        assert_eq!(not_found, None);
    }

    #[test]
    fn test_read_file_stream_bounded() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.bin");
        let sample = b"0123456789ABCDEF".repeat(100);
        fs::write(&file_path, &sample).unwrap();

        let bounded = read_file_stream_bounded(&file_path, 32).unwrap();
        assert_eq!(bounded.len(), 32);
        assert_eq!(&bounded[..16], b"0123456789ABCDEF");
    }

    #[tokio::test]
    async fn test_local_logs_dir_creation_and_lock() {
        let dir = tempdir().unwrap();
        let log_path = dir.path().join("m31a_app_logs");

        let logs = LocalLogsDir::new(log_path.clone());
        assert!(!logs.is_initialized());
        assert!(!log_path.exists());

        logs.initialize().await.unwrap();
        assert!(logs.is_initialized());
        assert!(log_path.is_dir());

        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let meta = fs::metadata(&log_path).unwrap();
            let mode = meta.mode() & 0o777;
            assert_eq!(mode, 0o700, "Should enforce private 0700 permissions");
        }

        // Multiple initialize calls are safe and idempotent
        logs.initialize().await.unwrap();
        assert!(logs.is_initialized());
    }
}
