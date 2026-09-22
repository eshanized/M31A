//! Checkpoint Manifest domain model (CHK-01, D-13).
//!
//! A CheckpointManifest represents a coherent durable boundary capturing all 9
//! specification-mandated state components bound by a content-addressed SHA-256 hash:
//! 1. Mission state & identity
//! 2. Task states (Map of TaskId -> TaskState)
//! 3. Agent metadata & context
//! 4. Background job states (Map of JobId -> state)
//! 5. Policy context hash
//! 6. Verification check IDs
//! 7. External artifact references (ArtifactId, SHA-256, byte size)
//! 8. Repository snapshot identity (RepositoryBaseline SHA-256)
//! 9. Scheduler watermark & cycle sequence

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

use crate::ids::{ArtifactId, CheckId, CheckpointId, JobId, MissionId, TaskId};
use crate::state_machine::TaskState;

/// Supported schema version for CheckpointManifest (CHK-04).
pub const CURRENT_CHECKPOINT_SCHEMA_VERSION: u32 = 1;

/// Durable checkpoint manifest recording all 9 state components bound by SHA-256 (CHK-01, D-13).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckpointManifest {
    pub checkpoint_id: CheckpointId,
    pub mission_id: MissionId,
    pub sequence: u64,
    pub stage: String,
    pub cycle: u64,
    pub schema_version: u32,
    pub snapshot_identity: String,
    pub task_states: BTreeMap<TaskId, TaskState>,
    pub job_states: BTreeMap<JobId, String>,
    pub policy_context_hash: String,
    pub verification_check_ids: Vec<CheckId>,
    pub artifact_references: Vec<(ArtifactId, String, u64)>,
    pub summary: String,
    pub created_at: DateTime<Utc>,
}

impl CheckpointManifest {
    /// Create a new checkpoint manifest with the current schema version.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        checkpoint_id: CheckpointId,
        mission_id: MissionId,
        sequence: u64,
        stage: impl Into<String>,
        cycle: u64,
        snapshot_identity: impl Into<String>,
        task_states: BTreeMap<TaskId, TaskState>,
        job_states: BTreeMap<JobId, String>,
        policy_context_hash: impl Into<String>,
        verification_check_ids: Vec<CheckId>,
        artifact_references: Vec<(ArtifactId, String, u64)>,
        summary: impl Into<String>,
    ) -> Self {
        Self {
            checkpoint_id,
            mission_id,
            sequence,
            stage: stage.into(),
            cycle,
            schema_version: CURRENT_CHECKPOINT_SCHEMA_VERSION,
            snapshot_identity: snapshot_identity.into(),
            task_states,
            job_states,
            policy_context_hash: policy_context_hash.into(),
            verification_check_ids,
            artifact_references,
            summary: summary.into(),
            created_at: Utc::now(),
        }
    }

    /// Compute the content-addressed SHA-256 hash of this manifest (D-13).
    ///
    /// Serializes the canonical JSON representation and hashes via SHA-256.
    pub fn compute_manifest_hash(&self) -> String {
        let serialized =
            serde_json::to_string(self).expect("CheckpointManifest serialization should not fail");
        let mut hasher = Sha256::new();
        hasher.update(serialized.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    /// Verify whether this manifest's schema version is supported.
    pub fn is_schema_supported(&self) -> bool {
        self.schema_version == CURRENT_CHECKPOINT_SCHEMA_VERSION
    }
}
