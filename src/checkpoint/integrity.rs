//! Checkpoint Integrity Validator implementing 5-point verification (CHK-04).
//!
//! 5-Point Validation Pass:
//! 1. Schema compatibility: verifies manifest schema_version is supported.
//! 2. State integrity: verifies manifest hash matches recomputed canonical hash, and all state components are coherent.
//! 3. Artifact availability: verifies every artifact in artifact_references exists in FsArtifactStore and matches recorded SHA-256 and size.
//! 4. Repository compatibility: verifies snapshot_identity matches expected workspace baseline.
//! 5. Policy compatibility: verifies policy_context_hash matches active policy rules.

use sha2::{Digest, Sha256};
use std::sync::Arc;

use crate::checkpoint::manifest::{CURRENT_CHECKPOINT_SCHEMA_VERSION, CheckpointManifest};
use crate::ids::ArtifactId;
use crate::persistence::artifacts::fs_store::ArtifactStore;

/// Errors emitted when checkpoint integrity validation fails.
#[derive(Debug, thiserror::Error)]
pub enum CheckpointIntegrityError {
    #[error("Unsupported schema version: {0} (supported: {CURRENT_CHECKPOINT_SCHEMA_VERSION})")]
    UnsupportedSchemaVersion(u32),

    #[error("Manifest hash mismatch: recorded {recorded}, computed {computed}")]
    ManifestHashMismatch { recorded: String, computed: String },

    #[error("State integrity violation: {0}")]
    StateIntegrity(String),

    #[error("Referenced artifact missing from store: {0}")]
    ArtifactMissing(ArtifactId),

    #[error("Artifact hash mismatch for {artifact_id}: expected {expected}, found {found}")]
    ArtifactHashMismatch {
        artifact_id: ArtifactId,
        expected: String,
        found: String,
    },

    #[error("Artifact size mismatch for {artifact_id}: expected {expected}, found {found}")]
    ArtifactSizeMismatch {
        artifact_id: ArtifactId,
        expected: u64,
        found: u64,
    },

    #[error("Repository baseline mismatch: expected {expected}, found {found}")]
    RepositoryMismatch { expected: String, found: String },

    #[error("Policy context mismatch: expected {expected}, found {found}")]
    PolicyMismatch { expected: String, found: String },

    #[error("Empty required field in checkpoint manifest: {0}")]
    EmptyField(String),
}

/// 5-point Checkpoint Integrity Validator (CHK-04).
pub struct CheckpointIntegrityValidator {
    artifact_store: Arc<dyn ArtifactStore>,
    active_policy_hash: Option<String>,
    expected_repo_hash: Option<String>,
}

impl CheckpointIntegrityValidator {
    /// Create a new validator with the given artifact store.
    pub fn new(artifact_store: Arc<dyn ArtifactStore>) -> Self {
        Self {
            artifact_store,
            active_policy_hash: None,
            expected_repo_hash: None,
        }
    }

    /// Attach expected policy context hash for validation.
    pub fn with_policy_hash(mut self, hash: impl Into<String>) -> Self {
        self.active_policy_hash = Some(hash.into());
        self
    }

    /// Attach expected repository baseline hash for validation.
    pub fn with_repo_hash(mut self, hash: impl Into<String>) -> Self {
        self.expected_repo_hash = Some(hash.into());
        self
    }

    /// Perform the 5-point integrity validation pass on a checkpoint manifest.
    pub async fn validate(
        &self,
        manifest: &CheckpointManifest,
    ) -> Result<(), CheckpointIntegrityError> {
        let computed = manifest.compute_manifest_hash();
        self.validate_with_recorded_hash(manifest, Some(&computed))
            .await
    }

    /// Perform the 5-point integrity validation pass, also validating against a recorded SQLite manifest hash.
    pub async fn validate_with_recorded_hash(
        &self,
        manifest: &CheckpointManifest,
        recorded_manifest_hash: Option<&str>,
    ) -> Result<(), CheckpointIntegrityError> {
        // ---------------------------------------------------------------------
        // 1. Schema Compatibility (CHK-04 Point 1)
        // ---------------------------------------------------------------------
        if !manifest.is_schema_supported() {
            return Err(CheckpointIntegrityError::UnsupportedSchemaVersion(
                manifest.schema_version,
            ));
        }

        // ---------------------------------------------------------------------
        // 2. State Integrity & Canonical Hash (CHK-04 Point 2)
        // ---------------------------------------------------------------------
        let computed_manifest_hash = manifest.compute_manifest_hash();
        match recorded_manifest_hash {
            Some(rec_hash) if !rec_hash.trim().is_empty() => {
                if rec_hash != computed_manifest_hash {
                    return Err(CheckpointIntegrityError::ManifestHashMismatch {
                        recorded: rec_hash.to_string(),
                        computed: computed_manifest_hash,
                    });
                }
            }
            _ => {
                return Err(CheckpointIntegrityError::ManifestHashMismatch {
                    recorded: "<missing>".to_string(),
                    computed: computed_manifest_hash,
                });
            }
        }

        if manifest.snapshot_identity.is_empty() {
            return Err(CheckpointIntegrityError::EmptyField(
                "snapshot_identity".into(),
            ));
        }
        if manifest.policy_context_hash.is_empty() {
            return Err(CheckpointIntegrityError::EmptyField(
                "policy_context_hash".into(),
            ));
        }

        // ---------------------------------------------------------------------
        // 3. Artifact Availability & Cryptographic Hashes (CHK-04 Point 3)
        // ---------------------------------------------------------------------
        for (artifact_id, expected_hash, expected_size) in &manifest.artifact_references {
            // Attempt retrieving via known extensions
            let mut retrieved_bytes = None;
            for ext in ["bin", "txt", "json", "log"] {
                if let Ok(bytes) = self.artifact_store.retrieve(*artifact_id, ext).await {
                    retrieved_bytes = Some(bytes);
                    break;
                }
            }

            let bytes = match retrieved_bytes {
                Some(b) => b,
                None => return Err(CheckpointIntegrityError::ArtifactMissing(*artifact_id)),
            };

            // Verify size
            let actual_size = bytes.len() as u64;
            if actual_size != *expected_size {
                return Err(CheckpointIntegrityError::ArtifactSizeMismatch {
                    artifact_id: *artifact_id,
                    expected: *expected_size,
                    found: actual_size,
                });
            }

            // Verify cryptographic SHA-256 hash
            let mut hasher = Sha256::new();
            hasher.update(&bytes);
            let actual_hash = format!("{:x}", hasher.finalize());

            if &actual_hash != expected_hash {
                return Err(CheckpointIntegrityError::ArtifactHashMismatch {
                    artifact_id: *artifact_id,
                    expected: expected_hash.clone(),
                    found: actual_hash,
                });
            }
        }

        // ---------------------------------------------------------------------
        // 4. Repository Compatibility (CHK-04 Point 4)
        // ---------------------------------------------------------------------
        if let Some(ref expected_repo) = self.expected_repo_hash
            && &manifest.snapshot_identity != expected_repo
        {
            return Err(CheckpointIntegrityError::RepositoryMismatch {
                expected: expected_repo.clone(),
                found: manifest.snapshot_identity.clone(),
            });
        }

        // ---------------------------------------------------------------------
        // 5. Policy Compatibility (CHK-04 Point 5)
        // ---------------------------------------------------------------------
        if let Some(ref expected_policy) = self.active_policy_hash
            && &manifest.policy_context_hash != expected_policy
        {
            return Err(CheckpointIntegrityError::PolicyMismatch {
                expected: expected_policy.clone(),
                found: manifest.policy_context_hash.clone(),
            });
        }

        Ok(())
    }
}
