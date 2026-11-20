//! Security and input validation boundaries for M31A Desktop.
//!
//! Enforces canonical filesystem path checks, directory constraints,
//! and workspace trust evaluation before any operation reaches runtime services.

use m31a_trust::{TrustDecision, TrustLevel, TrustStore};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SecurityError {
    #[error("Path is empty or invalid")]
    EmptyPath,

    #[error("Target path does not exist: '{0}'")]
    NotFound(String),

    #[error("Target path is not a directory: '{0}'")]
    NotADirectory(String),

    #[error("Failed to canonicalize path '{0}': {1}")]
    CanonicalizationFailed(String, std::io::Error),

    #[error("Workspace at '{0}' is untrusted and access was denied")]
    UntrustedWorkspace(String),

    #[error("Path '{0}' traverses outside workspace boundary '{1}'")]
    PathTraversal(String, String),

    #[error("Trust store lock is poisoned")]
    LockPoisoned,
}

/// Validates that a path string is non-empty, exists, resolves to a canonical directory,
/// and returns the canonicalized PathBuf.
pub fn validate_and_canonicalize_workspace_path(raw_path: &str) -> Result<PathBuf, SecurityError> {
    let trimmed = raw_path.trim();
    if trimmed.is_empty() {
        return Err(SecurityError::EmptyPath);
    }

    let path = Path::new(trimmed);
    let canonical = path.canonicalize().map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            SecurityError::NotFound(trimmed.to_string())
        } else {
            SecurityError::CanonicalizationFailed(trimmed.to_string(), error)
        }
    })?;

    if !canonical.is_dir() {
        return Err(SecurityError::NotADirectory(
            canonical.display().to_string(),
        ));
    }

    Ok(canonical)
}

/// Validates that a subpath resides strictly inside `workspace_root`.
pub fn validate_and_canonicalize_child_path(
    workspace_root: &Path,
    subpath: &str,
) -> Result<PathBuf, SecurityError> {
    let trimmed = subpath.trim();
    let target = if trimmed.is_empty() || trimmed == "." {
        workspace_root.to_path_buf()
    } else {
        workspace_root.join(trimmed)
    };

    let canonical = target.canonicalize().map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            SecurityError::NotFound(trimmed.to_string())
        } else {
            SecurityError::CanonicalizationFailed(trimmed.to_string(), error)
        }
    })?;

    if !canonical.starts_with(workspace_root) {
        return Err(SecurityError::PathTraversal(
            canonical.display().to_string(),
            workspace_root.display().to_string(),
        ));
    }

    Ok(canonical)
}

/// Checks whether a workspace directory is trusted by the trust store.
/// If `allow_untrusted` is true, authorizes the workspace for the current session.
pub fn verify_workspace_trust(
    trust_store: &Arc<Mutex<TrustStore>>,
    workspace_dir: &Path,
    allow_untrusted: bool,
) -> Result<bool, SecurityError> {
    let mut store = trust_store
        .lock()
        .map_err(|_| SecurityError::LockPoisoned)?;

    match store.evaluate(workspace_dir) {
        TrustDecision::Trusted | TrustDecision::SessionOnly => Ok(true),
        TrustDecision::Denied { .. } => Err(SecurityError::UntrustedWorkspace(
            workspace_dir.display().to_string(),
        )),
        TrustDecision::RequiresConfirmation { .. } if allow_untrusted => {
            store
                .trust(workspace_dir, TrustLevel::SessionOnly)
                .map_err(|_| {
                    SecurityError::UntrustedWorkspace(workspace_dir.display().to_string())
                })?;
            Ok(true)
        }
        TrustDecision::RequiresConfirmation { .. } => Ok(false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_validate_and_canonicalize_valid_dir() {
        let temp = tempdir().unwrap();
        let canonical =
            validate_and_canonicalize_workspace_path(temp.path().to_str().unwrap()).unwrap();
        assert!(canonical.is_dir());
    }

    #[test]
    fn test_validate_and_canonicalize_rejects_empty() {
        assert!(matches!(
            validate_and_canonicalize_workspace_path("   "),
            Err(SecurityError::EmptyPath)
        ));
    }

    #[test]
    fn test_validate_and_canonicalize_rejects_nonexistent() {
        assert!(matches!(
            validate_and_canonicalize_workspace_path("/path/that/does/not/exist/999111"),
            Err(SecurityError::NotFound(_))
        ));
    }

    #[test]
    fn test_session_grant_never_overrides_explicit_deny() {
        let temp = tempdir().unwrap();
        let trust_path = temp.path().join("trusted_directories.json");
        let store = Arc::new(Mutex::new(TrustStore::with_store_path(trust_path)));
        store
            .lock()
            .unwrap()
            .trust(temp.path(), TrustLevel::Denied)
            .unwrap();

        assert!(verify_workspace_trust(&store, temp.path(), true).is_err());
    }

    #[test]
    fn test_session_grant_only_applies_after_confirmation_required() {
        let temp = tempdir().unwrap();
        let trust_path = temp.path().join("trusted_directories.json");
        let store = Arc::new(Mutex::new(TrustStore::with_store_path(trust_path)));

        assert!(!verify_workspace_trust(&store, temp.path(), false).unwrap());
        assert!(verify_workspace_trust(&store, temp.path(), true).unwrap());
        assert_eq!(
            store.lock().unwrap().evaluate(temp.path()),
            TrustDecision::SessionOnly
        );
    }

    #[test]
    fn test_validate_and_canonicalize_rejects_file() {
        let temp = tempdir().unwrap();
        let file_path = temp.path().join("file.txt");
        std::fs::write(&file_path, "not a dir").unwrap();
        assert!(matches!(
            validate_and_canonicalize_workspace_path(file_path.to_str().unwrap()),
            Err(SecurityError::NotADirectory(_))
        ));
    }

    #[test]
    fn test_validate_and_canonicalize_child_path_accepts_valid_and_rejects_traversal() {
        let temp = tempdir().unwrap();
        let child_dir = temp.path().join("src");
        std::fs::create_dir(&child_dir).unwrap();
        let child_file = child_dir.join("main.rs");
        std::fs::write(&child_file, "fn main() {}").unwrap();

        let canonical = validate_and_canonicalize_child_path(temp.path(), "src/main.rs").unwrap();
        assert_eq!(canonical, child_file.canonicalize().unwrap());

        // Traversal attempt to escape root
        let outside = tempdir().unwrap();
        let escape_relative = format!(
            "../{}",
            outside.path().file_name().unwrap().to_str().unwrap()
        );
        let res = validate_and_canonicalize_child_path(temp.path(), &escape_relative);
        assert!(res.is_err());
    }
}
