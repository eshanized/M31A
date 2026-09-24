//! Filesystem artifact store implementation
//!
//! Per PST-03, filesystem storage for large blobs (logs, reports, patches)
//! outside relational rows. Artifact IDs are UUIDs, not user-provided strings.

use crate::error::M31AError;
use crate::ids::ArtifactId;
use std::path::{Path, PathBuf};
use tokio::fs::{File, OpenOptions};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// Trait for artifact storage operations.
#[async_trait::async_trait]
pub trait ArtifactStore: Send + Sync {
    /// Store artifact data and return the path where it was stored.
    async fn store(
        &self,
        artifact_id: ArtifactId,
        data: &[u8],
        extension: &str,
    ) -> Result<PathBuf, M31AError>;

    /// Retrieve artifact data by ID and extension.
    async fn retrieve(
        &self,
        artifact_id: ArtifactId,
        extension: &str,
    ) -> Result<Vec<u8>, M31AError>;

    /// Get the expected path for an artifact without reading it.
    fn path_for(&self, artifact_id: ArtifactId, extension: &str) -> PathBuf;

    /// Check if an artifact payload file exists without reading it.
    async fn exists(&self, artifact_id: ArtifactId, extension: &str) -> bool {
        self.path_for(artifact_id, extension).exists()
    }

    /// Base directory for file-backed stores, if any (used for orphan payload reclamation).
    fn store_base_dir(&self) -> Option<PathBuf> {
        None
    }
}

/// Filesystem-based artifact store.
///
/// Artifacts are stored as files named `{artifact_id}.{extension}` in the base directory.
/// ArtifactId is a UUID, preventing path traversal attacks.
pub struct FsArtifactStore {
    base_dir: PathBuf,
}

impl FsArtifactStore {
    /// Create a new FsArtifactStore with the given base directory.
    pub fn new(base_dir: impl AsRef<Path>) -> Self {
        Self {
            base_dir: base_dir.as_ref().to_path_buf(),
        }
    }

    /// Return the base directory path for stored artifacts.
    pub fn base_dir(&self) -> &Path {
        &self.base_dir
    }

    /// Get the file path for an artifact.
    fn artifact_path(&self, artifact_id: ArtifactId, extension: &str) -> PathBuf {
        // ArtifactId is a UUID, so it's safe to use directly in filename
        // Extension is validated to be alphanumeric only
        let safe_ext = extension
            .chars()
            .filter(|c| c.is_alphanumeric())
            .collect::<String>();
        self.base_dir.join(format!("{}.{}", artifact_id, safe_ext))
    }

    /// Ensure the base directory exists.
    async fn ensure_dir(&self) -> Result<(), M31AError> {
        tokio::fs::create_dir_all(&self.base_dir)
            .await
            .map_err(|e| {
                M31AError::persistence(format!("failed to create artifact directory: {}", e))
            })
    }
}

#[async_trait::async_trait]
impl ArtifactStore for FsArtifactStore {
    /// Store artifact data to the filesystem atomically using a staging file.
    ///
    /// Returns the path where the artifact was stored.
    async fn store(
        &self,
        artifact_id: ArtifactId,
        data: &[u8],
        extension: &str,
    ) -> Result<PathBuf, M31AError> {
        self.ensure_dir().await?;

        let safe_ext = extension
            .chars()
            .filter(|c| c.is_alphanumeric())
            .collect::<String>();
        let final_path = self.artifact_path(artifact_id, extension);
        let tmp_path = self.base_dir.join(format!(
            ".tmp_{}_{}.{}",
            artifact_id,
            uuid::Uuid::now_v7(),
            safe_ext
        ));

        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&tmp_path)
            .await
            .map_err(|e| {
                M31AError::persistence(format!("failed to create temporary artifact file: {}", e))
            })?;

        file.write_all(data)
            .await
            .map_err(|e| M31AError::persistence(format!("failed to write artifact data: {}", e)))?;
        file.flush()
            .await
            .map_err(|e| M31AError::persistence(format!("failed to flush artifact: {}", e)))?;
        file.sync_all()
            .await
            .map_err(|e| M31AError::persistence(format!("failed to sync artifact file: {}", e)))?;
        drop(file);

        tokio::fs::rename(&tmp_path, &final_path)
            .await
            .map_err(|e| {
                M31AError::persistence(format!("failed to commit artifact file: {}", e))
            })?;

        Ok(final_path)
    }

    /// Retrieve artifact data from the filesystem.
    async fn retrieve(
        &self,
        artifact_id: ArtifactId,
        extension: &str,
    ) -> Result<Vec<u8>, M31AError> {
        let path = self.artifact_path(artifact_id, extension);

        let mut file = File::open(&path)
            .await
            .map_err(|e| M31AError::persistence(format!("failed to open artifact file: {}", e)))?;

        let mut data = Vec::new();
        file.read_to_end(&mut data)
            .await
            .map_err(|e| M31AError::persistence(format!("failed to read artifact data: {}", e)))?;

        Ok(data)
    }

    /// Get the expected path for an artifact.
    fn path_for(&self, artifact_id: ArtifactId, extension: &str) -> PathBuf {
        self.artifact_path(artifact_id, extension)
    }

    fn store_base_dir(&self) -> Option<PathBuf> {
        Some(self.base_dir.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::ArtifactId;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_fs_artifact_store_store_and_retrieve() {
        let _dir = tempdir().unwrap(); // Keep TempDir alive
        let store = FsArtifactStore::new(_dir.path());

        let artifact_id = ArtifactId::new();
        let data = b"test artifact data";
        let path = store.store(artifact_id, data, "txt").await.unwrap();

        // Verify path structure
        assert!(path.to_string_lossy().contains(&artifact_id.to_string()));
        assert!(path.to_string_lossy().ends_with(".txt"));

        // Retrieve and verify
        let retrieved = store.retrieve(artifact_id, "txt").await.unwrap();
        assert_eq!(retrieved, data);
    }

    #[tokio::test]
    async fn test_fs_artifact_store_path_for() {
        let _dir = tempdir().unwrap(); // Keep TempDir alive
        let store = FsArtifactStore::new(_dir.path());

        let artifact_id = ArtifactId::new();
        let path = store.path_for(artifact_id, "log");

        assert!(path.to_string_lossy().contains(&artifact_id.to_string()));
        assert!(path.to_string_lossy().ends_with(".log"));
    }

    #[tokio::test]
    async fn test_fs_artifact_store_binary_data() {
        let _dir = tempdir().unwrap(); // Keep TempDir alive
        let store = FsArtifactStore::new(_dir.path());

        let artifact_id = ArtifactId::new();
        // Binary data with null bytes and various values
        let data: Vec<u8> = (0..=255).collect();
        store.store(artifact_id, &data, "bin").await.unwrap();

        let retrieved = store.retrieve(artifact_id, "bin").await.unwrap();
        assert_eq!(retrieved, data);
    }

    #[tokio::test]
    async fn test_fs_artifact_store_multiple_artifacts() {
        let _dir = tempdir().unwrap(); // Keep TempDir alive
        let store = FsArtifactStore::new(_dir.path());

        let id1 = ArtifactId::new();
        let id2 = ArtifactId::new();

        store.store(id1, b"artifact 1", "txt").await.unwrap();
        store.store(id2, b"artifact 2", "json").await.unwrap();

        let data1 = store.retrieve(id1, "txt").await.unwrap();
        let data2 = store.retrieve(id2, "json").await.unwrap();

        assert_eq!(data1, b"artifact 1");
        assert_eq!(data2, b"artifact 2");
    }

    #[tokio::test]
    async fn test_fs_artifact_store_extension_sanitization() {
        let _dir = tempdir().unwrap(); // Keep TempDir alive
        let store = FsArtifactStore::new(_dir.path());

        let artifact_id = ArtifactId::new();
        // Extension with special characters should be sanitized
        store
            .store(artifact_id, b"data", "txt;rm -rf /")
            .await
            .unwrap();

        let path = store.path_for(artifact_id, "txt;rm -rf /");
        // Only alphanumeric chars should remain
        assert!(path.to_string_lossy().ends_with(".txtrmrf"));
    }

    #[tokio::test]
    async fn test_fs_artifact_store_retrieve_nonexistent() {
        let _dir = tempdir().unwrap(); // Keep TempDir alive
        let store = FsArtifactStore::new(_dir.path());

        let artifact_id = ArtifactId::new();
        let result = store.retrieve(artifact_id, "txt").await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, M31AError::PersistenceError(_)));
    }
}
