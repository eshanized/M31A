//! Declarative policy file loader and syntax validator (POL-01, D-01).

use std::path::{Path, PathBuf};
use thiserror::Error;

use crate::policy::rule::{CURRENT_POLICY_SCHEMA_VERSION, PolicyDocument};

/// Strongly typed errors encountered during policy file loading and parsing.
#[derive(Debug, Error)]
pub enum PolicyFileError {
    #[error("policy file not found: {0}")]
    NotFound(PathBuf),

    #[error("policy file IO error: {0}")]
    Io(String),

    #[error("policy file parse failed: {0}")]
    ParseFailed(String),

    #[error("unsupported policy schema version '{0}', expected '{CURRENT_POLICY_SCHEMA_VERSION}'")]
    UnsupportedVersion(String),
}

impl PolicyDocument {
    /// Parse a policy document from a TOML string, validating version and unknown fields.
    pub fn from_toml_str(content: &str) -> Result<Self, PolicyFileError> {
        let doc: PolicyDocument =
            toml::from_str(content).map_err(|e| PolicyFileError::ParseFailed(e.to_string()))?;

        if doc.version != CURRENT_POLICY_SCHEMA_VERSION {
            return Err(PolicyFileError::UnsupportedVersion(doc.version));
        }

        Ok(doc)
    }

    /// Load and validate a policy document synchronously from a filesystem path.
    pub fn load_from_file(path: &Path) -> Result<Self, PolicyFileError> {
        if !path.exists() {
            return Err(PolicyFileError::NotFound(path.to_path_buf()));
        }

        let content = std::fs::read_to_string(path)
            .map_err(|e| PolicyFileError::Io(format!("failed to read {}: {e}", path.display())))?;

        Self::from_toml_str(&content)
    }

    /// Load and validate a policy document asynchronously from a filesystem path.
    pub async fn load_from_file_async(path: &Path) -> Result<Self, PolicyFileError> {
        if !tokio::fs::try_exists(path).await.unwrap_or(false) {
            return Err(PolicyFileError::NotFound(path.to_path_buf()));
        }

        let content = tokio::fs::read_to_string(path)
            .await
            .map_err(|e| PolicyFileError::Io(format!("failed to read {}: {e}", path.display())))?;

        Self::from_toml_str(&content)
    }
}
