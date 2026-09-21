//! Artifact store capability provider wrapping FsArtifactStore (CTL-02, D-01, D-03).

use crate::capability::error::CapabilityError;
use crate::capability::traits::artifacts::ArtifactStoreService;
use crate::ids::ArtifactId;
use crate::persistence::artifacts::{
    ArtifactError, ArtifactProvenance, ArtifactService, ArtifactStore, FsArtifactStore,
};
use async_trait::async_trait;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;

/// Native provider for artifact storage wrapping FsArtifactStore and optional ArtifactService.
pub struct FsArtifactStoreProvider {
    store: Arc<dyn ArtifactStore>,
    service: Option<Arc<ArtifactService>>,
}

impl FsArtifactStoreProvider {
    /// Create a new provider with the given base directory.
    pub fn new(base_dir: impl AsRef<Path>) -> Self {
        Self {
            store: Arc::new(FsArtifactStore::new(base_dir)),
            service: None,
        }
    }

    /// Create from an existing Arc<dyn ArtifactStore>.
    pub fn from_arc(store: Arc<dyn ArtifactStore>) -> Self {
        Self {
            store,
            service: None,
        }
    }

    /// Create from a canonical ArtifactService.
    pub fn from_service(service: Arc<ArtifactService>) -> Self {
        Self {
            store: service.store().clone(),
            service: Some(service),
        }
    }

    /// Attach a canonical ArtifactService for lifecycle and provenance tracking.
    pub fn with_service(mut self, service: Arc<ArtifactService>) -> Self {
        self.service = Some(service);
        self
    }
}

#[async_trait]
impl ArtifactStoreService for FsArtifactStoreProvider {
    async fn store_artifact(
        &self,
        artifact_id: &str,
        data: &[u8],
        extension: &str,
    ) -> Result<PathBuf, CapabilityError> {
        let id = ArtifactId::from_str(artifact_id).map_err(|e| {
            CapabilityError::InvalidArgument(format!("invalid artifact ID UUID {artifact_id}: {e}"))
        })?;

        if let Some(ref service) = self.service {
            let provenance = ArtifactProvenance {
                producer_role: Some("capability_provider".to_string()),
                ..Default::default()
            };
            service
                .create_and_store_with_id(id, format!("artifact_{id}"), data, extension, provenance)
                .await
                .map_err(|e| CapabilityError::Io(e.to_string()))?;
            Ok(self.store.path_for(id, extension))
        } else {
            self.store
                .store(id, data, extension)
                .await
                .map_err(|e| CapabilityError::Io(e.to_string()))
        }
    }

    async fn load_artifact(
        &self,
        artifact_id: &str,
        extension: &str,
    ) -> Result<Vec<u8>, CapabilityError> {
        let id = ArtifactId::from_str(artifact_id).map_err(|e| {
            CapabilityError::InvalidArgument(format!("invalid artifact ID UUID {artifact_id}: {e}"))
        })?;

        if let Some(ref service) = self.service {
            service
                .retrieve_payload(id, extension)
                .await
                .map_err(|e| match e {
                    ArtifactError::PayloadNotFound(..) | ArtifactError::MetadataNotFound(..) => {
                        CapabilityError::NotFound(e.to_string())
                    }
                    other => CapabilityError::Io(other.to_string()),
                })
        } else {
            self.store
                .retrieve(id, extension)
                .await
                .map_err(|e| CapabilityError::NotFound(e.to_string()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_artifact_provider() {
        let dir = tempdir().unwrap();
        let provider = FsArtifactStoreProvider::new(dir.path());

        let id = ArtifactId::new().to_string();
        let path = provider
            .store_artifact(&id, b"artifact payload", "txt")
            .await
            .unwrap();
        assert!(path.exists());

        let retrieved = provider.load_artifact(&id, "txt").await.unwrap();
        assert_eq!(retrieved, b"artifact payload");
    }
}
