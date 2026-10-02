//! Rollback seam: restore the previous known-good binary.
//!
//! The installer preserves `<binary>.prev` on every replacement. Rollback is
//! an explicit operation (command or automatic post-update recovery): it
//! verifies the backup exists, restores it atomically over the live binary,
//! and reports the restored identity. Rollback never invents a version and
//! never deletes the only working executable.

use std::path::{Path, PathBuf};

/// Typed rollback failures.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RollbackError {
    #[error("rollback error: {0}")]
    Io(String),
    #[error("rollback error: no previous binary to restore")]
    NoBackup,
}

/// Restore `<install_dir>/<binary>.prev` over `<install_dir>/<binary>`.
pub fn rollback(install_dir: &Path, binary_name: &str) -> Result<PathBuf, RollbackError> {
    let live = install_dir.join(binary_name);
    let prev = install_dir.join(format!("{binary_name}.prev"));
    if !prev.is_file() {
        return Err(RollbackError::NoBackup);
    }
    // Keep a copy of the (broken) live binary for forensics before restore.
    if live.is_file() {
        let broken = install_dir.join(format!("{binary_name}.broken"));
        let _ = std::fs::copy(&live, &broken);
    }
    std::fs::copy(&prev, &live).map_err(|e| RollbackError::Io(e.to_string()))?;
    Ok(live)
}

/// Whether a rollback backup exists.
pub fn rollback_available(install_dir: &Path, binary_name: &str) -> bool {
    install_dir.join(format!("{binary_name}.prev")).is_file()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rollback_restores_previous() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("m31a"), b"new-broken").unwrap();
        std::fs::write(dir.path().join("m31a.prev"), b"old-good").unwrap();
        assert!(rollback_available(dir.path(), "m31a"));
        let live = rollback(dir.path(), "m31a").unwrap();
        assert_eq!(std::fs::read(&live).unwrap(), b"old-good");
    }

    #[test]
    fn rollback_without_backup_fails_closed() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!rollback_available(dir.path(), "m31a"));
        assert_eq!(rollback(dir.path(), "m31a"), Err(RollbackError::NoBackup));
    }
}
