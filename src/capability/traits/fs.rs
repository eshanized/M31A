//! File system capability service trait (CTL-01, CTL-02).

use crate::capability::error::CapabilityError;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Metadata description for a file or directory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileMetadata {
    pub size_bytes: u64,
    pub is_file: bool,
    pub is_dir: bool,
    pub is_readonly: bool,
    pub modified_ms: Option<u64>,
}

/// Asynchronous service seam for workspace file operations.
#[async_trait]
pub trait FileSystemService: Send + Sync + 'static {
    /// Read raw file bytes, optionally bounded by byte offset and byte limit.
    async fn read_file(
        &self,
        path: &Path,
        offset: Option<u64>,
        limit: Option<usize>,
    ) -> Result<Vec<u8>, CapabilityError>;

    /// Atomically write or overwrite content at path, returning bytes written.
    async fn write_file(&self, path: &Path, content: &[u8]) -> Result<usize, CapabilityError>;

    /// Replace exact old_content substring with new_content.
    async fn edit_file(
        &self,
        path: &Path,
        old_content: &str,
        new_content: &str,
    ) -> Result<(), CapabilityError>;

    /// List directory contents.
    async fn list_files(
        &self,
        path: &Path,
        recursive: bool,
    ) -> Result<Vec<PathBuf>, CapabilityError>;

    /// Query file or directory metadata.
    async fn file_metadata(&self, path: &Path) -> Result<FileMetadata, CapabilityError>;

    /// Delete a file within workspace boundary.
    async fn delete_file(&self, path: &Path) -> Result<(), CapabilityError>;

    /// Create a directory and all parent directories within workspace boundary.
    async fn create_directory(&self, path: &Path) -> Result<(), CapabilityError>;

    /// Move a file from src to dst within workspace boundary.
    async fn move_file(&self, src: &Path, dst: &Path) -> Result<(), CapabilityError>;

    /// Rename a file from src to dst within workspace boundary.
    async fn rename_file(&self, src: &Path, dst: &Path) -> Result<(), CapabilityError>;

    /// Copy a file from src to dst within workspace boundary, returning bytes copied.
    async fn copy_file(&self, src: &Path, dst: &Path) -> Result<u64, CapabilityError>;
}
