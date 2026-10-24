//! Canonical document serialization and atomic file persistence.

use std::fs;
use std::path::Path;

use crate::document::envelope::{DocumentEnvelope, MarkdownDocument};
use crate::document::error::DocumentError;

/// Serializes a [`MarkdownDocument<T>`] into canonical Markdown format with YAML front matter.
pub fn serialize_markdown_document<T: serde::Serialize>(
    doc: &MarkdownDocument<T>,
) -> Result<String, DocumentError> {
    let envelope = DocumentEnvelope {
        kind: doc.kind,
        schema_version: doc.schema_version,
        data: &doc.data,
    };

    let yaml_str =
        serde_yaml::to_string(&envelope).map_err(|e| DocumentError::YamlError(e.to_string()))?;

    let trimmed_yaml = yaml_str.trim();

    let mut output = String::with_capacity(trimmed_yaml.len() + doc.body.len() + 16);
    output.push_str("---\n");
    output.push_str(trimmed_yaml);
    output.push_str("\n---\n");

    if !doc.body.is_empty() {
        output.push('\n');
        output.push_str(&doc.body);
    }

    Ok(output)
}

/// Atomically persists a document string to disk via temporary file, fsync, and atomic rename.
pub fn atomic_write_document(path: &Path, content: &str) -> Result<(), DocumentError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| DocumentError::IoError {
            path: parent.to_path_buf(),
            source: e,
        })?;
    }

    let tmp_path = path.with_extension(format!("tmp-{}", uuid::Uuid::now_v7()));

    // Write content to temporary file
    fs::write(&tmp_path, content).map_err(|e| DocumentError::IoError {
        path: tmp_path.clone(),
        source: e,
    })?;

    // Flush and fsync
    if let Ok(file) = fs::File::open(&tmp_path) {
        let _ = file.sync_all();
    }

    #[cfg(windows)]
    if path.exists() {
        let _ = fs::remove_file(path);
    }

    // Atomic rename
    fs::rename(&tmp_path, path).map_err(|e| {
        let _ = fs::remove_file(&tmp_path);
        DocumentError::IoError {
            path: path.to_path_buf(),
            source: e,
        }
    })?;

    Ok(())
}
