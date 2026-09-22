//! Checkpoint Manager implementing the durable two-phase protocol (CHK-01, D-13).
//!
//! Durability order (prepare → promote → verify → commit):
//! - Step 1: External Artifact Staging
//!   - Write all external verification/evidence artifacts to `staging/{checkpoint_id}/`.
//!   - Flush and sync files to disk via `tokio::fs::File::sync_all`.
//!   - Compute content-addressed SHA-256 hashes and sizes; validate strictly against manifest.
//! - Step 2: Payload Promotion & Verification
//!   - Promote payloads to the authoritative `FsArtifactStore` FIRST, read each payload
//!     back, and verify its hash. The checkpoint row is only committed after every
//!     payload is provably durable — a checkpoint row therefore never claims payload
//!     durability for an absent payload.
//! - Step 3: Atomic SQLite Commit
//!   - Begin single SQLite transaction.
//!   - Insert checkpoint metadata, manifest JSON, and manifest hash into `checkpoints`.
//!   - Insert artifact linkages into `checkpoint_artifacts`.
//!   - Insert/update the unified `artifacts` ledger with hash verification
//!     (immutability: conflicting content under an existing identity fails).
//!   - Commit SQLite transaction.
//!   - If a crash occurs between promotion and commit, payloads exist without
//!     metadata rows: detectable orphans reclaimed by `gc_orphan_payloads`;
//!     restore refuses to run without both halves.

use chrono::Utc;
use sha2::{Digest, Sha256};
use sqlx::{Row, SqlitePool};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::io::AsyncWriteExt;

use crate::checkpoint::integrity::CheckpointIntegrityValidator;
use crate::checkpoint::manifest::CheckpointManifest;
use crate::checkpoint::resume::SafeResumeEngine;
use crate::ids::{ArtifactId, CheckpointId};
use crate::persistence::artifacts::fs_store::ArtifactStore;

/// Staged artifact input to be committed with a checkpoint.
#[derive(Debug, Clone)]
pub struct StagedArtifactInput {
    pub artifact_id: ArtifactId,
    pub data: Vec<u8>,
    pub extension: String,
    pub artifact_type: String,
}

impl StagedArtifactInput {
    pub fn new(
        artifact_id: ArtifactId,
        data: Vec<u8>,
        extension: impl Into<String>,
        artifact_type: impl Into<String>,
    ) -> Self {
        Self {
            artifact_id,
            data,
            extension: extension.into(),
            artifact_type: artifact_type.into(),
        }
    }
}

/// Errors occurring during two-phase checkpoint management.
#[derive(Debug, thiserror::Error)]
pub enum CheckpointError {
    #[error("I/O error during artifact staging: {0}")]
    Io(#[from] std::io::Error),
    #[error("Persistence error: {0}")]
    Persistence(#[from] sqlx::Error),
    #[error("Artifact store error: {0}")]
    ArtifactStore(String),
    #[error("Artifact hash mismatch for {artifact_id}: expected {expected}, computed {computed}")]
    ArtifactHashMismatch {
        artifact_id: ArtifactId,
        expected: String,
        computed: String,
    },
    #[error("Artifact size mismatch for {artifact_id}: expected {expected}, computed {computed}")]
    ArtifactSizeMismatch {
        artifact_id: ArtifactId,
        expected: u64,
        computed: u64,
    },
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("Missing artifact reference in manifest: {0}")]
    MissingArtifactReference(ArtifactId),
    #[error("Checkpoint '{0}' not found")]
    NotFound(CheckpointId),
    #[error("Artifact identity conflict: '{0}'")]
    ArtifactConflict(String),
    #[error("Checkpoint validation error: {0}")]
    Validation(#[from] crate::checkpoint::integrity::CheckpointIntegrityError),
    #[error("Safe resume error: {0}")]
    SafeResume(#[from] crate::checkpoint::resume::SafeResumeError),
    #[error("Resume authorization revalidation failed: {0}")]
    ResumeAuthorization(String),
    #[error("Mission status update failed: {0}")]
    MissionStatus(String),
}

/// Result of a completed checkpoint restoration pass (D-16).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CheckpointRestoreResult {
    pub checkpoint_id: CheckpointId,
    pub mission_id: crate::ids::MissionId,
    pub sequence: u64,
    pub stage: String,
    pub cycle: u64,
    pub preserved_tasks: usize,
    pub invalidated_tasks: usize,
    pub rescheduled_tasks: usize,
    pub summary: String,
}

/// Two-phase checkpoint manager.
pub struct CheckpointManager {
    pool: SqlitePool,
    artifact_store: Arc<dyn ArtifactStore>,
    staging_dir: PathBuf,
}

impl CheckpointManager {
    /// Create a new CheckpointManager.
    pub fn new(
        pool: SqlitePool,
        artifact_store: Arc<dyn ArtifactStore>,
        staging_dir: impl AsRef<Path>,
    ) -> Self {
        Self {
            pool,
            artifact_store,
            staging_dir: staging_dir.as_ref().to_path_buf(),
        }
    }

    /// Access the staging directory path.
    pub fn staging_dir(&self) -> &Path {
        &self.staging_dir
    }

    /// Execute the strict two-phase checkpoint creation protocol (D-13).
    pub async fn create_checkpoint(
        &self,
        manifest: &CheckpointManifest,
        artifacts: Vec<StagedArtifactInput>,
    ) -> Result<CheckpointId, CheckpointError> {
        let checkpoint_id = manifest.checkpoint_id;
        let staging_cp_dir = self.staging_dir.join(checkpoint_id.to_string());

        // Every artifact the manifest references must be supplied as a payload.
        // A manifest claiming an absent payload can never become a checkpoint row.
        for (ref_id, _, _) in &manifest.artifact_references {
            if !artifacts.iter().any(|a| &a.artifact_id == ref_id) {
                return Err(CheckpointError::MissingArtifactReference(*ref_id));
            }
        }

        // ---------------------------------------------------------------------
        // Step 1: External Artifact Staging & Durability Flush (D-13)
        // ---------------------------------------------------------------------
        tokio::fs::create_dir_all(&staging_cp_dir).await?;

        for artifact in &artifacts {
            let staged_file_path = staging_cp_dir.join(format!(
                "{}.{}",
                artifact.artifact_id,
                artifact.extension.trim_start_matches('.')
            ));

            let mut file = tokio::fs::OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(&staged_file_path)
                .await?;

            file.write_all(&artifact.data).await?;
            file.sync_all().await?;

            // Compute and validate hash and size against manifest
            let mut hasher = Sha256::new();
            hasher.update(&artifact.data);
            let computed_hash = format!("{:x}", hasher.finalize());
            let computed_size = artifact.data.len() as u64;

            // Confirm artifact is registered in manifest
            if let Some((_, expected_hash, expected_size)) = manifest
                .artifact_references
                .iter()
                .find(|(id, _, _)| *id == artifact.artifact_id)
            {
                if *expected_hash != computed_hash {
                    return Err(CheckpointError::ArtifactHashMismatch {
                        artifact_id: artifact.artifact_id,
                        expected: expected_hash.clone(),
                        computed: computed_hash,
                    });
                }
                if *expected_size != computed_size {
                    return Err(CheckpointError::ArtifactSizeMismatch {
                        artifact_id: artifact.artifact_id,
                        expected: *expected_size,
                        computed: computed_size,
                    });
                }
            } else {
                return Err(CheckpointError::MissingArtifactReference(
                    artifact.artifact_id,
                ));
            }
        }

        // ---------------------------------------------------------------------
        // Step 2: Atomic SQLite Persistence (D-13)
        // ---------------------------------------------------------------------
        // Promote payloads BEFORE the metadata commit and verify each payload
        // by reading it back. Only provably durable payloads are ever
        // referenced by a committed checkpoint row.
        for artifact in &artifacts {
            self.promote_payload(artifact).await?;
        }

        let mut tx = self.pool.begin().await?;

        let manifest_json = serde_json::to_string(manifest)?;
        let manifest_hash = manifest.compute_manifest_hash();

        sqlx::query(
            r#"
            INSERT INTO checkpoints (id, mission_id, sequence, stage, cycle, state_summary, manifest_json, manifest_hash, created_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(checkpoint_id.as_bytes().as_slice())
        .bind(manifest.mission_id.as_bytes().as_slice())
        .bind(manifest.sequence as i64)
        .bind(&manifest.stage)
        .bind(manifest.cycle as i64)
        .bind(&manifest.summary)
        .bind(&manifest_json)
        .bind(&manifest_hash)
        .bind(manifest.created_at.to_rfc3339())
        .execute(&mut *tx)
        .await?;

        for artifact in &artifacts {
            let mut hasher = Sha256::new();
            hasher.update(&artifact.data);
            let hash = format!("{:x}", hasher.finalize());
            let record_id = uuid::Uuid::now_v7();
            let now_str = Utc::now().to_rfc3339();

            sqlx::query(
                r#"
                INSERT INTO checkpoint_artifacts (id, checkpoint_id, artifact_id, sha256_hash, size_bytes, artifact_type, created_at)
                VALUES (?, ?, ?, ?, ?, ?, ?)
                "#,
            )
            .bind(record_id.as_bytes().as_slice())
            .bind(checkpoint_id.as_bytes().as_slice())
            .bind(artifact.artifact_id.as_bytes().as_slice())
            .bind(&hash)
            .bind(artifact.data.len() as i64)
            .bind(&artifact.artifact_type)
            .bind(&now_str)
            .execute(&mut *tx)
            .await?;
        }

        // Atomic commit guarantees metadata and linkages are durable together.
        // Payloads were already promoted and hash-verified in Step 2, so a
        // committed row always references present payloads.
        tx.commit().await?;

        // Clean up staged artifacts for this checkpoint. Leftovers are
        // detectable orphans reclaimed by `clean_orphaned_staging`; a cleanup
        // failure is surfaced loudly, never silently ignored.
        if let Err(e) = tokio::fs::remove_dir_all(&staging_cp_dir).await {
            tracing::warn!(
                checkpoint_id = %checkpoint_id,
                "checkpoint committed but staging cleanup failed (reclaim via clean_orphaned_staging): {e}"
            );
        }

        Ok(checkpoint_id)
    }

    /// Promote one staged payload into the authoritative artifact store,
    /// verify it by reading it back, and record it in the unified `artifacts`
    /// ledger with immutability enforcement.
    ///
    /// An existing identity carrying different content fails closed with
    /// `ArtifactConflict` — a restored checkpoint can never silently mutate
    /// unrelated evidence. Identical content is idempotent.
    async fn promote_payload(&self, artifact: &StagedArtifactInput) -> Result<(), CheckpointError> {
        let mut hasher = Sha256::new();
        hasher.update(&artifact.data);
        let content_hash = format!("{:x}", hasher.finalize());

        let extensions = if artifact.extension == "bin" {
            vec!["bin".to_string()]
        } else {
            vec![artifact.extension.clone(), "bin".to_string()]
        };
        for ext in &extensions {
            self.artifact_store
                .store(artifact.artifact_id, &artifact.data, ext)
                .await
                .map_err(|e| CheckpointError::ArtifactStore(e.to_string()))?;
            // Read-back verification: the payload must be present and exact.
            let back = self
                .artifact_store
                .retrieve(artifact.artifact_id, ext)
                .await
                .map_err(|e| CheckpointError::ArtifactStore(e.to_string()))?;
            let mut verify = Sha256::new();
            verify.update(&back);
            let back_hash = format!("{:x}", verify.finalize());
            if back_hash != content_hash {
                return Err(CheckpointError::ArtifactHashMismatch {
                    artifact_id: artifact.artifact_id,
                    expected: content_hash,
                    computed: back_hash,
                });
            }
        }

        // Unified ledger with check-on-conflict immutability.
        let now = Utc::now().to_rfc3339();
        let existing: Option<String> =
            sqlx::query_scalar("SELECT content_hash FROM artifacts WHERE id = ?")
                .bind(artifact.artifact_id.as_bytes().as_slice())
                .fetch_optional(&self.pool)
                .await?;
        if let Some(existing_hash) = existing {
            if existing_hash != content_hash {
                return Err(CheckpointError::ArtifactConflict(format!(
                    "artifact {} already recorded with different content",
                    artifact.artifact_id
                )));
            }
            return Ok(());
        }
        sqlx::query(
            r#"
            INSERT INTO artifacts (
                id, name, logical_path, content_hash, size_bytes, extension, version, status,
                workflow_run_id, step_run_id, mission_id, task_id, producer_role, prompt_id,
                prompt_version, model, parent_artifact_id, metadata_json, created_at
            ) VALUES (?, ?, ?, ?, ?, ?, 1, 'valid', NULL, NULL, NULL, NULL, 'checkpoint', NULL, NULL, NULL, NULL, NULL, ?)
            "#,
        )
        .bind(artifact.artifact_id.as_bytes().as_slice())
        .bind(format!("checkpoint-{}", artifact.artifact_id))
        .bind(format!("checkpoints/{}", artifact.artifact_id))
        .bind(&content_hash)
        .bind(artifact.data.len() as i64)
        .bind(&artifact.extension)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Garbage-collect promoted payloads that reference no durable metadata
    /// (crash between promotion and commit leaves detectable orphans).
    /// A payload is live when its identity appears in `artifacts` or `checkpoint_artifacts`;
    /// otherwise its files are removed.
    /// Returns the number of reclaimed payload files.
    pub async fn gc_orphan_payloads(&self) -> Result<usize, CheckpointError> {
        let Some(base_dir) = self.artifact_store.store_base_dir() else {
            return Ok(0);
        };
        let mut read_dir = tokio::fs::read_dir(&base_dir)
            .await
            .map_err(CheckpointError::Io)?;
        let mut reclaimed = 0;
        while let Some(entry) = read_dir.next_entry().await.map_err(CheckpointError::Io)? {
            let name = entry.file_name().to_string_lossy().to_string();
            let Some((stem, _)) = name.rsplit_once('.') else {
                continue;
            };
            let Ok(uuid) = uuid::Uuid::parse_str(stem) else {
                continue;
            };
            let id_bytes = uuid.as_bytes().to_vec();
            let in_artifacts: i64 =
                sqlx::query_scalar("SELECT COUNT(*) FROM artifacts WHERE id = ?")
                    .bind(id_bytes.as_slice())
                    .fetch_one(&self.pool)
                    .await?;
            if in_artifacts > 0 {
                continue;
            }
            let in_links: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM checkpoint_artifacts WHERE artifact_id = ?",
            )
            .bind(id_bytes.as_slice())
            .fetch_one(&self.pool)
            .await?;
            if in_links > 0 {
                continue;
            }
            tokio::fs::remove_file(entry.path())
                .await
                .map_err(CheckpointError::Io)?;
            reclaimed += 1;
        }
        Ok(reclaimed)
    }

    /// Garbage-collect uncommitted staged directories left behind by simulated crashes.
    pub async fn clean_orphaned_staging(&self) -> Result<usize, CheckpointError> {
        let mut cleaned = 0;
        if !self.staging_dir.exists() {
            return Ok(0);
        }

        let mut entries = tokio::fs::read_dir(&self.staging_dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.is_dir() {
                let dir_name = entry.file_name().to_string_lossy().to_string();
                if let Ok(cp_id) = dir_name.parse::<uuid::Uuid>() {
                    let exists_in_db = sqlx::query_scalar::<_, i64>(
                        "SELECT COUNT(*) FROM checkpoints WHERE id = ?",
                    )
                    .bind(cp_id.as_bytes().as_slice())
                    .fetch_one(&self.pool)
                    .await?
                        > 0;

                    if !exists_in_db {
                        let _ = tokio::fs::remove_dir_all(&path).await;
                        cleaned += 1;
                    }
                }
            }
        }

        Ok(cleaned)
    }

    /// Restore a checkpoint into the target workspace, verifying integrity and reconciling tasks (CHK-04, CHK-05, D-16).
    pub async fn restore_checkpoint(
        &self,
        checkpoint_id: CheckpointId,
        target_workspace: &Path,
    ) -> Result<CheckpointRestoreResult, CheckpointError> {
        let row = sqlx::query(
            "SELECT mission_id, sequence, stage, cycle, manifest_json, manifest_hash FROM checkpoints WHERE id = ?",
        )
        .bind(checkpoint_id.as_bytes().as_slice())
        .fetch_optional(&self.pool)
        .await?;

        let row = row.ok_or(CheckpointError::NotFound(checkpoint_id))?;

        let manifest_json: String = row.get("manifest_json");
        let manifest_hash: Option<String> = row.get("manifest_hash");
        let manifest: CheckpointManifest = serde_json::from_str(&manifest_json)?;

        // 1. 5-point integrity validation (CHK-04)
        let validator = CheckpointIntegrityValidator::new(self.artifact_store.clone());
        validator
            .validate_with_recorded_hash(&manifest, manifest_hash.as_deref())
            .await?;

        // 1b. Revalidate the execution authorization before replaying execution.
        // Stale/corrupt/unauthorized bindings halt here; pre-execution or
        // session-less lanes proceed (nothing to rebind).
        let lifecycle_repo =
            crate::persistence::sqlite::repositories::SqliteLifecycleRepository::new(
                self.pool.clone(),
            );
        lifecycle_repo
            .revalidate_authorization_for_resume(manifest.mission_id)
            .await
            .map_err(|e| CheckpointError::ResumeAuthorization(e.to_string()))?;

        // 2. Safe resume reconciliation (CHK-05, D-16)
        let resume_engine = SafeResumeEngine::new(
            self.pool.clone(),
            self.artifact_store.clone(),
            target_workspace,
        );
        let resume_report = resume_engine
            .resume_mission(manifest.mission_id, &manifest)
            .await?;

        // 3. Update mission status in database to Executing via canonical mission repository.
        // The status write is runtime truth — propagate failures instead of swallowing them.
        let mission_repo = crate::persistence::sqlite::repositories::SqliteMissionRepository::new(
            self.pool.clone(),
        );
        mission_repo
            .update_status(
                manifest.mission_id,
                crate::state_machine::MissionState::Executing,
            )
            .await
            .map_err(|e| CheckpointError::MissionStatus(e.to_string()))?;

        let summary = format!(
            "Restored checkpoint {} (cycle {}, stage {}): {} preserved, {} invalidated, {} rescheduled",
            checkpoint_id,
            manifest.cycle,
            manifest.stage,
            resume_report.preserved_tasks.len(),
            resume_report.invalidated_tasks.len(),
            resume_report.rescheduled_tasks.len()
        );

        Ok(CheckpointRestoreResult {
            checkpoint_id,
            mission_id: manifest.mission_id,
            sequence: manifest.sequence,
            stage: manifest.stage,
            cycle: manifest.cycle,
            preserved_tasks: resume_report.preserved_tasks.len(),
            invalidated_tasks: resume_report.invalidated_tasks.len(),
            rescheduled_tasks: resume_report.rescheduled_tasks.len(),
            summary,
        })
    }

    /// Retrieve the latest checkpoint info (sequence, stage, cycle) for a mission.
    pub async fn get_latest_checkpoint_info(
        pool: &SqlitePool,
        mission_id: crate::ids::MissionId,
    ) -> Result<Option<(i64, String, i64)>, sqlx::Error> {
        sqlx::query_as(
            r#"
            SELECT sequence, stage, cycle FROM checkpoints
            WHERE mission_id = ?
            ORDER BY cycle DESC, sequence DESC
            LIMIT 1
            "#,
        )
        .bind(mission_id.as_bytes().as_slice())
        .fetch_optional(pool)
        .await
    }

    /// Retrieve the checkpoint manifest by checkpoint ID.
    pub async fn get_checkpoint_manifest(
        pool: &SqlitePool,
        checkpoint_id: CheckpointId,
    ) -> Result<Option<(i64, CheckpointManifest)>, sqlx::Error> {
        let row = sqlx::query("SELECT cycle, manifest_json FROM checkpoints WHERE id = ?")
            .bind(checkpoint_id.as_bytes().as_slice())
            .fetch_optional(pool)
            .await?;

        if let Some(row) = row {
            let cycle: i64 = row.get("cycle");
            let manifest_json: Option<String> = row.get("manifest_json");
            if let Some(json) = manifest_json
                && let Ok(manifest) = serde_json::from_str::<CheckpointManifest>(&json)
            {
                return Ok(Some((cycle, manifest)));
            }
        }
        Ok(None)
    }
}
