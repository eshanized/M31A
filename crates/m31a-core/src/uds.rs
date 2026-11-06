//! Unix Domain Socket helpers with strict owner permissions and stale socket reclamation.
//!
//! Unix Domain Socket client and server IPC abstractions with secure permissions.

use std::io::{self, ErrorKind};
use std::path::{Path, PathBuf};
#[cfg(unix)]
use tokio::net::{UnixListener, UnixStream};

/// Owner-only permission mode (0700) keeping control socket directories private.
pub const SOCKET_DIR_MODE: u32 = 0o700;
const SOCKET_DIR_PERMISSION_BITS: u32 = 0o777;

/// Ensures `socket_dir` exists and is strictly restricted to the current user (0700 on Unix).
pub async fn prepare_private_socket_directory(socket_dir: impl AsRef<Path>) -> io::Result<()> {
    let dir = socket_dir.as_ref();

    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        use std::os::unix::fs::PermissionsExt;

        let mut builder = std::fs::DirBuilder::new();
        builder.recursive(true);
        builder.mode(SOCKET_DIR_MODE);

        match builder.create(dir) {
            Ok(()) => return Ok(()),
            Err(e) if e.kind() == ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e),
        }

        let meta = tokio::fs::symlink_metadata(dir).await?;
        if !meta.is_dir() {
            return Err(io::Error::new(
                ErrorKind::AlreadyExists,
                format!(
                    "socket directory path exists and is not a directory: {}",
                    dir.display()
                ),
            ));
        }

        let perms = meta.permissions();
        if perms.mode() & SOCKET_DIR_PERMISSION_BITS != SOCKET_DIR_MODE {
            tokio::fs::set_permissions(dir, std::fs::Permissions::from_mode(SOCKET_DIR_MODE))
                .await?;
        }
    }

    #[cfg(not(unix))]
    {
        tokio::fs::create_dir_all(dir).await?;
    }

    Ok(())
}

/// Discovers the standard runtime directory for daemon sockets.
/// Checks `XDG_RUNTIME_DIR` (`/run/user/<uid>/m31a`) with fallback to `~/.config/m31a`.
pub fn shared_daemon_socket_directory() -> PathBuf {
    if let Ok(runtime_dir) = std::env::var("XDG_RUNTIME_DIR") {
        let p = PathBuf::from(runtime_dir).join("m31a");
        return p;
    }

    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".config").join("m31a")
}

/// Returns the canonical path for the default daemon socket file.
pub fn default_daemon_socket_path() -> PathBuf {
    shared_daemon_socket_directory().join("m31a-daemon.sock")
}

/// Checks whether a socket file is stale by attempting an asynchronous connection.
/// Returns `Ok(true)` if the socket file exists but refuses connections (dead daemon).
/// Returns `Ok(false)` if a daemon actively answers or path doesn't exist.
pub async fn is_stale_socket_path(socket_path: impl AsRef<Path>) -> io::Result<bool> {
    let path = socket_path.as_ref();
    if !path.exists() {
        return Ok(false);
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::FileTypeExt;
        let meta = tokio::fs::symlink_metadata(path).await?;
        if !meta.file_type().is_socket() {
            return Ok(false);
        }

        // Try connecting to see if another live process is listening
        match UnixStream::connect(path).await {
            Ok(_) => {
                // Another live process is answering! Not stale.
                Ok(false)
            }
            Err(e) if e.kind() == ErrorKind::ConnectionRefused => {
                // Socket file exists but no process is listening -> stale!
                Ok(true)
            }
            Err(_) => Ok(true),
        }
    }

    #[cfg(not(unix))]
    {
        Ok(false)
    }
}

/// Safely removes a stale socket file if no process is currently listening on it.
pub async fn unlink_stale_socket_if_needed(socket_path: impl AsRef<Path>) -> io::Result<bool> {
    let path = socket_path.as_ref();
    if is_stale_socket_path(path).await? {
        tokio::fs::remove_file(path).await?;
        Ok(true)
    } else {
        Ok(false)
    }
}

/// Binds a Unix domain socket listener after preparing the directory, preventing symlink attacks,
/// cleaning any stale socket file, rejecting non-socket paths or active sockets, and enforcing
/// restrictive 0600 file permissions on Unix.
#[cfg(unix)]
pub async fn bind_unix_listener(socket_path: impl AsRef<Path>) -> io::Result<UnixListener> {
    let path = socket_path.as_ref();
    if let Some(parent) = path.parent() {
        prepare_private_socket_directory(parent).await?;
    }

    // Validate existing path using symlink_metadata
    match tokio::fs::symlink_metadata(path).await {
        Ok(meta) => {
            #[cfg(unix)]
            {
                use std::os::unix::fs::FileTypeExt;
                if meta.file_type().is_symlink() {
                    return Err(io::Error::new(
                        ErrorKind::AlreadyExists,
                        format!(
                            "Socket path '{}' is a symlink; rejecting symlink",
                            path.display()
                        ),
                    ));
                }
                if !meta.file_type().is_socket() {
                    return Err(io::Error::new(
                        ErrorKind::AlreadyExists,
                        format!(
                            "Socket path '{}' exists and is not a socket file",
                            path.display()
                        ),
                    ));
                }
            }

            // Path is a socket file. Check if an active server is listening.
            if is_stale_socket_path(path).await? {
                tokio::fs::remove_file(path).await?;
            } else {
                return Err(io::Error::new(
                    ErrorKind::AddrInUse,
                    format!(
                        "Socket path '{}' is already in use by an active server",
                        path.display()
                    ),
                ));
            }
        }
        Err(e) if e.kind() == ErrorKind::NotFound => {
            // Path does not exist, safe to proceed
        }
        Err(e) => return Err(e),
    }

    let listener = match UnixListener::bind(path) {
        Ok(l) => l,
        Err(err) if err.kind() == ErrorKind::InvalidInput => {
            // If the advertised path exceeds sockaddr_un sun_path (~108 bytes),
            // canonicalize parent and bind relative/canonical path.
            if let Some(parent) = path.parent() {
                let canonical_parent = tokio::fs::canonicalize(parent).await?;
                let file_name = path.file_name().ok_or_else(|| {
                    io::Error::new(ErrorKind::InvalidInput, "missing socket filename")
                })?;
                let short_path = canonical_parent.join(file_name);
                UnixListener::bind(&short_path)?
            } else {
                return Err(err);
            }
        }
        Err(err) => return Err(err),
    };

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        tokio::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).await?;
    }

    Ok(listener)
}

#[cfg(not(unix))]
pub async fn bind_unix_listener(socket_path: impl AsRef<Path>) -> io::Result<()> {
    let _ = socket_path;
    Err(io::Error::new(
        ErrorKind::Unsupported,
        "Unix domain sockets are not supported on this platform",
    ))
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_prepare_private_socket_directory() {
        let tmp = tempdir().unwrap();
        let sock_dir = tmp.path().join("secure-sockets");

        prepare_private_socket_directory(&sock_dir)
            .await
            .expect("prepare dir");
        assert!(sock_dir.is_dir());

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let meta = std::fs::metadata(&sock_dir).unwrap();
            assert_eq!(meta.permissions().mode() & 0o777, 0o700);
        }
    }

    #[tokio::test]
    async fn test_bind_unix_listener_modes_and_symlink_rejection() {
        let tmp = tempdir().unwrap();
        let sock_path = tmp.path().join("test_secure.sock");

        // 1. Bind listener and assert permissions
        let listener = bind_unix_listener(&sock_path).await.expect("bind listener");
        assert!(sock_path.exists());

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let meta = std::fs::symlink_metadata(&sock_path).unwrap();
            assert_eq!(meta.permissions().mode() & 0o777, 0o600);
        }

        // 2. Active socket preservation: trying to bind again on active socket must fail with AddrInUse
        let err = bind_unix_listener(&sock_path).await.unwrap_err();
        assert_eq!(err.kind(), ErrorKind::AddrInUse);

        drop(listener);
        tokio::fs::remove_file(&sock_path).await.unwrap();

        // 3. Symlink rejection
        let target_file = tmp.path().join("real_target.txt");
        tokio::fs::write(&target_file, b"content").await.unwrap();

        #[cfg(unix)]
        {
            let symlink_path = tmp.path().join("symlink.sock");
            std::os::unix::fs::symlink(&target_file, &symlink_path).unwrap();
            let err = bind_unix_listener(&symlink_path).await.unwrap_err();
            assert_eq!(err.kind(), ErrorKind::AlreadyExists);
            assert!(err.to_string().contains("symlink"));
        }

        // 4. Regular file rejection
        let regular_path = tmp.path().join("regular.sock");
        tokio::fs::write(&regular_path, b"not a socket")
            .await
            .unwrap();
        let err = bind_unix_listener(&regular_path).await.unwrap_err();
        assert_eq!(err.kind(), ErrorKind::AlreadyExists);
        assert!(err.to_string().contains("not a socket file"));
    }

    #[tokio::test]
    async fn test_stale_socket_detection_and_rebinding() {
        let tmp = tempdir().unwrap();
        let sock_path = tmp.path().join("test.sock");

        // 1. Bind first listener
        let listener = bind_unix_listener(&sock_path)
            .await
            .expect("bind listener 1");
        assert!(sock_path.exists());

        // While listener is alive, is_stale should be false
        let is_stale = is_stale_socket_path(&sock_path).await.unwrap();
        assert!(!is_stale);

        // 2. Drop listener (simulates crash without unlinking socket file)
        drop(listener);
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;

        // Socket file still exists on disk, but nothing is listening -> is_stale should be true!
        let is_stale = is_stale_socket_path(&sock_path).await.unwrap();
        assert!(is_stale);

        // 3. Rebinding should automatically reclaim stale socket!
        let listener2 = bind_unix_listener(&sock_path)
            .await
            .expect("rebind listener 2");
        assert!(sock_path.exists());
        drop(listener2);
    }
}
