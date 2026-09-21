//! Immutable artifact storage service trait (CTL-01, CTL-02).

use crate::capability::error::CapabilityError;
use async_trait::async_trait;
use std::path::PathBuf;

/// Asynchronous service seam for storing and loading oversized or durable artifacts.
#[async_trait]
pub trait ArtifactStoreService: Send + Sync + 'static {
    /// Store artifact bytes under the specified ID and extension, returning physical path.
    async fn store_artifact(
        &self,
        artifact_id: &str,
        data: &[u8],
        extension: &str,
    ) -> Result<PathBuf, CapabilityError>;

    /// Retrieve artifact bytes by ID and extension.
    async fn load_artifact(
        &self,
        artifact_id: &str,
        extension: &str,
    ) -> Result<Vec<u8>, CapabilityError>;
}
