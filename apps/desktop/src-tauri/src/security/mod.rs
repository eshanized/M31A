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
    if !path.exists() {
        return Err(SecurityError::NotFound(trimmed.to_string()));
    }

    let canonical = path
        .canonicalize()
        .map_err(|e| SecurityError::CanonicalizationFailed(trimmed.to_string(), e))?;

    if !canonical.is_dir() {
        return Err(SecurityError::NotADirectory(
            canonical.display().to_string(),
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

    let decision = store.evaluate(workspace_dir);
    if matches!(
        decision,
        TrustDecision::Trusted | TrustDecision::SessionOnly
    ) {
        return Ok(true);
    }

    if allow_untrusted {
        store
            .trust(workspace_dir, TrustLevel::SessionOnly)
            .map_err(|_| SecurityError::UntrustedWorkspace(workspace_dir.display().to_string()))?;
        return Ok(true);
    }

    Err(SecurityError::UntrustedWorkspace(
        workspace_dir.display().to_string(),
    ))
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
    fn test_validate_and_canonicalize_rejects_file() {
        let temp = tempdir().unwrap();
        let file_path = temp.path().join("file.txt");
        std::fs::write(&file_path, "not a dir").unwrap();
        assert!(matches!(
            validate_and_canonicalize_workspace_path(file_path.to_str().unwrap()),
            Err(SecurityError::NotADirectory(_))
        ));
    }
}
