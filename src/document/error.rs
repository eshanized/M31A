//! Error types for the canonical Markdown document architecture.

use std::path::PathBuf;
use thiserror::Error;

/// Strongly typed errors encountered during document parsing, validation, and serialization.
#[derive(Debug, Error)]
pub enum DocumentError {
    #[error("Document exceeds maximum size limit ({actual} bytes > {max} bytes)")]
    DocumentSizeExceeded { actual: usize, max: usize },

    #[error("Front-matter block exceeds maximum size limit ({actual} bytes > {max} bytes)")]
    FrontMatterSizeExceeded { actual: usize, max: usize },

    #[error("Document missing opening front-matter delimiter ('---') on first line")]
    MissingOpeningDelimiter,

    #[error("Document missing closing front-matter delimiter ('---')")]
    MissingClosingDelimiter,

    #[error("Front-matter block is empty")]
    EmptyFrontMatter,

    #[error("Document kind mismatch: expected '{expected}', found '{found}'")]
    KindMismatch { expected: String, found: String },

    #[error("Unknown or invalid document kind: '{0}'")]
    UnknownKind(String),

    #[error("Unsupported schema version: {found} (supported: 1..={max_supported})")]
    UnsupportedSchemaVersion { found: u32, max_supported: u32 },

    #[error("Front-matter YAML parse error: {0}")]
    YamlError(String),

    #[error("Duplicate YAML key '{key}' detected in front matter")]
    DuplicateKey { key: String },

    #[error("YAML security violation: {reason}")]
    SecurityViolation { reason: String },

    #[error("Domain validation error in document: {0}")]
    ValidationError(String),

    #[error("IO error for '{path}': {source}")]
    IoError {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("Document file not found: {0}")]
    NotFound(PathBuf),
}
