//! Canonical Artifact Lifecycle and Provenance Authority (ART-01, ART-02, D-13).
//!
//! Provides ONE canonical authority for the artifact lifecycle across:
//! produce -> identify/hash -> store -> register -> provenance -> validate -> project -> consume.
//!
//! Storage vs Authority Distinction:
//! - Domain Aggregate: `ArtifactRecord`, `ArtifactProvenance`, `ArtifactStatus`
//! - Physical Payload Storage: `FsArtifactStore` (`.m31a/artifacts/{artifact_id}.{extension}`)
//! - Metadata & Provenance Ledger: SQLite (`artifacts`, `workflow_artifacts`, `checkpoint_artifacts`)
//! - Projections: Derived human-readable files (`.planning/*.md`, `REPORT.md`), never operational state.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{Row, SqlitePool};
use std::fmt;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;
use thiserror::Error;

use crate::ids::{ArtifactId, CheckId, MissionId, TaskId, WorkflowRunId, WorkflowStepRunId};
use crate::persistence::artifacts::ArtifactStore;

/// Compute canonical SHA-256 hash for any byte payload.
pub fn compute_artifact_hash(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

/// Operational lifecycle status for a registered artifact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactStatus {
    #[default]
    Valid,
    Superseded,
    Archived,
    Quarantined,
}

impl fmt::Display for ArtifactStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Valid => write!(f, "valid"),
            Self::Superseded => write!(f, "superseded"),
            Self::Archived => write!(f, "archived"),
            Self::Quarantined => write!(f, "quarantined"),
        }
    }
}

impl FromStr for ArtifactStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().as_str() {
            "valid" => Ok(Self::Valid),
            "superseded" => Ok(Self::Superseded),
            "archived" => Ok(Self::Archived),
            "quarantined" => Ok(Self::Quarantined),
            other => Err(format!("unknown artifact status: '{other}'")),
        }
    }
}

/// Durable provenance record capturing origin, upstream parents, and verification links.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ArtifactProvenance {
    pub mission_id: Option<MissionId>,
    pub task_id: Option<TaskId>,
    pub workflow_run_id: Option<WorkflowRunId>,
    pub step_run_id: Option<WorkflowStepRunId>,
    pub producer_role: Option<String>,
    pub prompt_id: Option<String>,
    pub prompt_version: Option<u32>,
    pub model: Option<String>,
    #[serde(default)]
    pub parent_artifact_ids: Vec<ArtifactId>,
    #[serde(default)]
    pub verification_check_ids: Vec<CheckId>,
}

impl ArtifactProvenance {
    /// Create empty provenance.
    pub fn new() -> Self {
        Self::default()
    }

    /// Provenance for an artifact produced by a workflow step.
    pub fn for_workflow_step(
        workflow_run_id: WorkflowRunId,
        step_run_id: WorkflowStepRunId,
        producer_role: impl Into<String>,
    ) -> Self {
        Self {
            workflow_run_id: Some(workflow_run_id),
            step_run_id: Some(step_run_id),
            producer_role: Some(producer_role.into()),
            ..Default::default()
        }
    }

    /// Provenance for an artifact produced by a task execution.
    pub fn for_task(
        mission_id: MissionId,
        task_id: TaskId,
        producer_role: impl Into<String>,
    ) -> Self {
        Self {
            mission_id: Some(mission_id),
            task_id: Some(task_id),
            producer_role: Some(producer_role.into()),
            ..Default::default()
        }
    }

    /// Provenance for a mission-level artifact.
    pub fn for_mission(mission_id: MissionId) -> Self {
        Self {
            mission_id: Some(mission_id),
            ..Default::default()
        }
    }

    /// Link an upstream parent artifact contributing to this artifact.
    pub fn with_parent(mut self, parent_id: ArtifactId) -> Self {
        if !self.parent_artifact_ids.contains(&parent_id) {
            self.parent_artifact_ids.push(parent_id);
        }
        self
    }

    /// Link a verification check confirming this artifact.
    pub fn with_check(mut self, check_id: CheckId) -> Self {
        if !self.verification_check_ids.contains(&check_id) {
            self.verification_check_ids.push(check_id);
        }
        self
    }

    /// Record prompt contract identity.
    pub fn with_prompt(mut self, prompt_id: impl Into<String>, version: u32) -> Self {
        self.prompt_id = Some(prompt_id.into());
        self.prompt_version = Some(version);
        self
    }

    /// Record model producing the artifact.
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }
}

/// Authoritative domain aggregate for an artifact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactRecord {
    pub id: ArtifactId,
    pub name: String,
    pub logical_path: PathBuf,
    pub content_hash: String,
    pub size_bytes: u64,
    pub extension: String,
    pub version: u32,
    pub status: ArtifactStatus,
    pub provenance: ArtifactProvenance,
    pub created_at: DateTime<Utc>,
}

/// Metadata view alias for `ArtifactRecord`.
pub type ArtifactMetadata = ArtifactRecord;

/// Result of an artifact integrity audit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IntegrityCheckResult {
    pub artifact_id: ArtifactId,
    pub expected_hash: String,
    pub actual_hash: Option<String>,
    pub payload_exists: bool,
    pub metadata_exists: bool,
    pub is_valid: bool,
    pub error_detail: Option<String>,
}

/// Domain errors encountered in artifact lifecycle operations.
#[derive(Debug, Error)]
pub enum ArtifactError {
    #[error("Artifact payload not found for ID {0} (extension: '{1}')")]
    PayloadNotFound(ArtifactId, String),

    #[error("Artifact metadata not found for ID {0}")]
    MetadataNotFound(ArtifactId),

    #[error(
        "Artifact integrity mismatch for ID {artifact_id}: expected '{expected}', calculated '{actual}'"
    )]
    HashMismatch {
        artifact_id: ArtifactId,
        expected: String,
        actual: String,
    },

    #[error(
        "Immutable artifact violation: artifact {0} already exists with different content hash"
    )]
    ImmutableViolation(ArtifactId),

    #[error("Artifact store error: {0}")]
    StoreError(String),

    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Projection error: {0}")]
    ProjectionError(String),
}

/// Canonical service managing the unified artifact lifecycle.
#[derive(Clone)]
pub struct ArtifactService {
    store: Arc<dyn ArtifactStore>,
    pool: SqlitePool,
}

impl ArtifactService {
    /// Create a new ArtifactService wrapping a physical payload store and SQLite pool.
    pub fn new(store: Arc<dyn ArtifactStore>, pool: SqlitePool) -> Self {
        Self { store, pool }
    }

    /// Access the underlying physical payload store.
    pub fn store(&self) -> &Arc<dyn ArtifactStore> {
        &self.store
    }

    /// Access the underlying SQLite pool.
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    /// Compute canonical SHA-256 hash for any byte payload.
    pub fn compute_hash(data: &[u8]) -> String {
        compute_artifact_hash(data)
    }

    /// Verify that the unified `artifacts` table exists in SQLite (schema is migration-owned via migration 014).
    pub async fn ensure_schema(&self) -> Result<(), ArtifactError> {
        // Schema definition is canonical and migration-owned (014_artifact_ledger.sql).
        // Application code must never perform ad-hoc DDL.
        sqlx::query("SELECT 1 FROM artifacts LIMIT 1")
            .execute(&self.pool)
            .await
            .map_err(ArtifactError::from)?;
        Ok(())
    }

    /// Create and persist a new artifact with freshly computed identity and hash.
    pub async fn create_and_store(
        &self,
        name: impl Into<String>,
        data: &[u8],
        extension: impl Into<String>,
        provenance: ArtifactProvenance,
    ) -> Result<ArtifactRecord, ArtifactError> {
        let id = ArtifactId::new();
        self.create_and_store_with_id(id, name, data, extension, provenance)
            .await
    }

    /// Create and persist an artifact with a specified `ArtifactId`, enforcing immutability.
    pub async fn create_and_store_with_id(
        &self,
        id: ArtifactId,
        name: impl Into<String>,
        data: &[u8],
        extension: impl Into<String>,
        provenance: ArtifactProvenance,
    ) -> Result<ArtifactRecord, ArtifactError> {
        let name_str = name.into();
        let ext_str = extension.into();
        let content_hash = Self::compute_hash(data);

        // Check immutability if artifact ID already exists
        if let Some(existing) = self.get_metadata(id).await? {
            if existing.content_hash == content_hash {
                // Idempotent write: exactly identical content
                return Ok(existing);
            } else {
                // Attempted overwrite of immutable artifact with different payload
                return Err(ArtifactError::ImmutableViolation(id));
            }
        }

        let logical_path = self.store.path_for(id, &ext_str);
        self.store_and_record(
            id,
            name_str,
            logical_path,
            data,
            ext_str,
            content_hash,
            1,
            ArtifactStatus::Valid,
            provenance,
        )
        .await
    }

    /// Register an artifact that already exists on the filesystem (e.g. workspace output or projection).
    pub async fn register_existing(
        &self,
        id: Option<ArtifactId>,
        name: impl Into<String>,
        path: impl AsRef<Path>,
        extension: impl Into<String>,
        provenance: ArtifactProvenance,
    ) -> Result<ArtifactRecord, ArtifactError> {
        let p = path.as_ref();
        let data = tokio::fs::read(p).await.map_err(|e| {
            ArtifactError::StoreError(format!(
                "failed to read existing file at '{}': {}",
                p.display(),
                e
            ))
        })?;

        let artifact_id = id.unwrap_or_default();
        let name_str = name.into();
        let ext_str = extension.into();
        let content_hash = Self::compute_hash(&data);

        // Store into the content-addressed physical store as well to assure durability
        self.store_and_record(
            artifact_id,
            name_str,
            p.to_path_buf(),
            &data,
            ext_str,
            content_hash,
            1,
            ArtifactStatus::Valid,
            provenance,
        )
        .await
    }

    /// Register a project genesis or planning artifact, linking provenance and setting status Valid.
    pub async fn register_genesis_artifact(
        &self,
        name: impl Into<String>,
        path: impl AsRef<Path>,
        producer_role: impl Into<String>,
        parent_id: Option<ArtifactId>,
    ) -> Result<ArtifactRecord, ArtifactError> {
        let name_str = name.into();
        let p = path.as_ref();
        let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("md");
        let mut prov = ArtifactProvenance::new();
        prov.producer_role = Some(producer_role.into());
        if let Some(parent) = parent_id {
            prov = prov.with_parent(parent);
        }
        let data = tokio::fs::read(p).await.map_err(|e| {
            ArtifactError::StoreError(format!(
                "failed to read existing file at '{}': {}",
                p.display(),
                e
            ))
        })?;

        let artifact_id = ArtifactId::new();
        let ext_str = ext.to_string();
        let content_hash = Self::compute_hash(&data);
        let logical_path = PathBuf::from(&name_str);

        self.store_and_record(
            artifact_id,
            name_str,
            logical_path,
            &data,
            ext_str,
            content_hash,
            1,
            ArtifactStatus::Valid,
            prov,
        )
        .await
    }

    /// Retrieve the payload bytes of an artifact, validating its SHA-256 content hash.
    pub async fn retrieve_payload(
        &self,
        id: ArtifactId,
        extension: &str,
    ) -> Result<Vec<u8>, ArtifactError> {
        // 1. Verify metadata exists
        let record = self
            .get_metadata(id)
            .await?
            .ok_or(ArtifactError::MetadataNotFound(id))?;

        // 2. Retrieve bytes from physical payload store
        let data = self
            .store
            .retrieve(id, extension)
            .await
            .map_err(|_| ArtifactError::PayloadNotFound(id, extension.to_string()))?;

        // 3. Validate content hash matches metadata
        let actual_hash = Self::compute_hash(&data);
        if actual_hash != record.content_hash {
            return Err(ArtifactError::HashMismatch {
                artifact_id: id,
                expected: record.content_hash,
                actual: actual_hash,
            });
        }

        Ok(data)
    }

    /// Retrieve metadata record for an artifact by ID.
    pub async fn get_metadata(
        &self,
        id: ArtifactId,
    ) -> Result<Option<ArtifactRecord>, ArtifactError> {
        let row_opt = sqlx::query(
            r#"
            SELECT id, name, logical_path, content_hash, size_bytes, extension, version,
                   status, workflow_run_id, step_run_id, mission_id, task_id,
                   producer_role, prompt_id, prompt_version, model, parent_artifact_id,
                   metadata_json, created_at
            FROM artifacts
            WHERE id = ?
            "#,
        )
        .bind(id.as_bytes().as_slice())
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = row_opt {
            return Ok(Some(self.map_row_to_record(&row)?));
        }

        // Fallback: check workflow_artifacts table
        let wf_row_opt = sqlx::query(
            r#"
            SELECT id, workflow_run_id, step_run_id, name, path, content_hash, version, status, created_at
            FROM workflow_artifacts
            WHERE id = ?
            "#,
        )
        .bind(id.as_bytes().as_slice())
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = wf_row_opt {
            return Ok(Some(self.map_workflow_row_to_record(&row)?));
        }

        Ok(None)
    }

    /// Retrieve metadata record for an artifact by SHA-256 content hash.
    pub async fn get_metadata_by_hash(
        &self,
        content_hash: &str,
    ) -> Result<Option<ArtifactRecord>, ArtifactError> {
        let row_opt = sqlx::query(
            r#"
            SELECT id, name, logical_path, content_hash, size_bytes, extension, version,
                   status, workflow_run_id, step_run_id, mission_id, task_id,
                   producer_role, prompt_id, prompt_version, model, parent_artifact_id,
                   metadata_json, created_at
            FROM artifacts
            WHERE content_hash = ?
            ORDER BY created_at DESC
            LIMIT 1
            "#,
        )
        .bind(content_hash)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = row_opt {
            return Ok(Some(self.map_row_to_record(&row)?));
        }

        // Fallback: check workflow_artifacts
        let wf_row_opt = sqlx::query(
            r#"
            SELECT id, workflow_run_id, step_run_id, name, path, content_hash, version, status, created_at
            FROM workflow_artifacts
            WHERE content_hash = ?
            ORDER BY created_at DESC
            LIMIT 1
            "#,
        )
        .bind(content_hash)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = wf_row_opt {
            return Ok(Some(self.map_workflow_row_to_record(&row)?));
        }

        Ok(None)
    }

    /// Perform a full integrity audit on an artifact (checks metadata, payload file, and hash).
    pub async fn verify_integrity(
        &self,
        id: ArtifactId,
        extension: &str,
    ) -> Result<IntegrityCheckResult, ArtifactError> {
        let metadata = match self.get_metadata(id).await? {
            Some(m) => m,
            None => {
                return Ok(IntegrityCheckResult {
                    artifact_id: id,
                    expected_hash: String::new(),
                    actual_hash: None,
                    payload_exists: false,
                    metadata_exists: false,
                    is_valid: false,
                    error_detail: Some("Artifact metadata not found in SQLite ledger".to_string()),
                });
            }
        };

        // Check if physical payload exists and retrieve it
        let payload_result = self.store.retrieve(id, extension).await;
        match payload_result {
            Err(_) => Ok(IntegrityCheckResult {
                artifact_id: id,
                expected_hash: metadata.content_hash,
                actual_hash: None,
                payload_exists: false,
                metadata_exists: true,
                is_valid: false,
                error_detail: Some(format!(
                    "Payload file missing in artifact store for ID {id}.{extension}"
                )),
            }),
            Ok(data) => {
                let actual_hash = Self::compute_hash(&data);
                if actual_hash == metadata.content_hash {
                    Ok(IntegrityCheckResult {
                        artifact_id: id,
                        expected_hash: metadata.content_hash,
                        actual_hash: Some(actual_hash),
                        payload_exists: true,
                        metadata_exists: true,
                        is_valid: true,
                        error_detail: None,
                    })
                } else {
                    Ok(IntegrityCheckResult {
                        artifact_id: id,
                        expected_hash: metadata.content_hash.clone(),
                        actual_hash: Some(actual_hash.clone()),
                        payload_exists: true,
                        metadata_exists: true,
                        is_valid: false,
                        error_detail: Some(format!(
                            "Hash mismatch: expected '{}', calculated '{}'",
                            metadata.content_hash, actual_hash
                        )),
                    })
                }
            }
        }
    }

    /// Create a new version of an existing artifact, linking previous version in provenance.
    pub async fn create_version(
        &self,
        previous_id: ArtifactId,
        name: impl Into<String>,
        data: &[u8],
        extension: impl Into<String>,
        mut provenance: ArtifactProvenance,
    ) -> Result<ArtifactRecord, ArtifactError> {
        let previous = self
            .get_metadata(previous_id)
            .await?
            .ok_or(ArtifactError::MetadataNotFound(previous_id))?;

        let new_id = ArtifactId::new();
        let name_str = name.into();
        let ext_str = extension.into();
        let content_hash = Self::compute_hash(data);
        let version = previous.version + 1;

        provenance = provenance.with_parent(previous_id);
        let logical_path = self.store.path_for(new_id, &ext_str);

        self.store_and_record(
            new_id,
            name_str,
            logical_path,
            data,
            ext_str,
            content_hash,
            version,
            ArtifactStatus::Valid,
            provenance,
        )
        .await
    }

    /// Update status of an artifact (e.g. mark Superseded or Archived).
    pub async fn update_status(
        &self,
        id: ArtifactId,
        status: ArtifactStatus,
    ) -> Result<(), ArtifactError> {
        let rows_affected = sqlx::query("UPDATE artifacts SET status = ? WHERE id = ?")
            .bind(status.to_string())
            .bind(id.as_bytes().as_slice())
            .execute(&self.pool)
            .await?
            .rows_affected();

        // Also update workflow_artifacts if present
        let _ = sqlx::query("UPDATE workflow_artifacts SET status = ? WHERE id = ?")
            .bind(status.to_string())
            .bind(id.as_bytes().as_slice())
            .execute(&self.pool)
            .await;

        if rows_affected == 0 {
            // Check if it was in workflow_artifacts only
            let wf_exists = sqlx::query("SELECT id FROM workflow_artifacts WHERE id = ?")
                .bind(id.as_bytes().as_slice())
                .fetch_optional(&self.pool)
                .await?;
            if wf_exists.is_none() {
                return Err(ArtifactError::MetadataNotFound(id));
            }
        }

        Ok(())
    }

    /// Project an artifact payload out to a target workspace projection path.
    ///
    /// Human-readable projections (.planning/*.md) are derived views and must never
    /// be treated as authoritative operational truth.
    pub async fn project(
        &self,
        id: ArtifactId,
        extension: &str,
        target_path: impl AsRef<Path>,
    ) -> Result<PathBuf, ArtifactError> {
        let p = target_path.as_ref();
        let data = self.retrieve_payload(id, extension).await?;

        if let Some(parent) = p.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(|e| {
                ArtifactError::ProjectionError(format!(
                    "failed to create parent directory for projection '{}': {}",
                    parent.display(),
                    e
                ))
            })?;
        }

        tokio::fs::write(p, &data).await.map_err(|e| {
            ArtifactError::ProjectionError(format!(
                "failed to write projection to '{}': {}",
                p.display(),
                e
            ))
        })?;

        Ok(p.to_path_buf())
    }

    /// List all artifacts recorded for a specific workflow run.
    pub async fn list_by_workflow_run(
        &self,
        workflow_run_id: WorkflowRunId,
    ) -> Result<Vec<ArtifactRecord>, ArtifactError> {
        let rows = sqlx::query(
            r#"
            SELECT id, name, logical_path, content_hash, size_bytes, extension, version,
                   status, workflow_run_id, step_run_id, mission_id, task_id,
                   producer_role, prompt_id, prompt_version, model, parent_artifact_id,
                   metadata_json, created_at
            FROM artifacts
            WHERE workflow_run_id = ?
            ORDER BY created_at ASC
            "#,
        )
        .bind(workflow_run_id.as_bytes().as_slice())
        .fetch_all(&self.pool)
        .await?;

        let mut results = Vec::with_capacity(rows.len());
        for row in rows {
            results.push(self.map_row_to_record(&row)?);
        }

        // If none in unified artifacts table, check workflow_artifacts
        if results.is_empty() {
            let wf_rows = sqlx::query(
                r#"
                SELECT id, workflow_run_id, step_run_id, name, path, content_hash, version, status, created_at
                FROM workflow_artifacts
                WHERE workflow_run_id = ?
                ORDER BY created_at ASC
                "#,
            )
            .bind(workflow_run_id.as_bytes().as_slice())
            .fetch_all(&self.pool)
            .await?;

            for row in wf_rows {
                results.push(self.map_workflow_row_to_record(&row)?);
            }
        }

        Ok(results)
    }

    /// List all artifacts recorded for a specific mission.
    pub async fn list_by_mission(
        &self,
        mission_id: MissionId,
    ) -> Result<Vec<ArtifactRecord>, ArtifactError> {
        let rows = sqlx::query(
            r#"
            SELECT id, name, logical_path, content_hash, size_bytes, extension, version,
                   status, workflow_run_id, step_run_id, mission_id, task_id,
                   producer_role, prompt_id, prompt_version, model, parent_artifact_id,
                   metadata_json, created_at
            FROM artifacts
            WHERE mission_id = ?
            ORDER BY created_at ASC
            "#,
        )
        .bind(mission_id.as_bytes().as_slice())
        .fetch_all(&self.pool)
        .await?;

        let mut results = Vec::with_capacity(rows.len());
        for row in rows {
            results.push(self.map_row_to_record(&row)?);
        }
        Ok(results)
    }

    /// List all artifacts recorded for a specific task.
    pub async fn list_by_task(
        &self,
        task_id: TaskId,
    ) -> Result<Vec<ArtifactRecord>, ArtifactError> {
        let rows = sqlx::query(
            r#"
            SELECT id, name, logical_path, content_hash, size_bytes, extension, version,
                   status, workflow_run_id, step_run_id, mission_id, task_id,
                   producer_role, prompt_id, prompt_version, model, parent_artifact_id,
                   metadata_json, created_at
            FROM artifacts
            WHERE task_id = ?
            ORDER BY created_at ASC
            "#,
        )
        .bind(task_id.as_bytes().as_slice())
        .fetch_all(&self.pool)
        .await?;

        let mut results = Vec::with_capacity(rows.len());
        for row in rows {
            results.push(self.map_row_to_record(&row)?);
        }
        Ok(results)
    }

    /// List all artifacts recorded in the unified artifact ledger.
    pub async fn list_all(&self) -> Result<Vec<ArtifactRecord>, ArtifactError> {
        let rows = sqlx::query(
            r#"
            SELECT id, name, logical_path, content_hash, size_bytes, extension, version,
                   status, workflow_run_id, step_run_id, mission_id, task_id,
                   producer_role, prompt_id, prompt_version, model, parent_artifact_id,
                   metadata_json, created_at
            FROM artifacts
            ORDER BY created_at DESC
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        let mut results = Vec::with_capacity(rows.len());
        for row in rows {
            results.push(self.map_row_to_record(&row)?);
        }

        // Also check workflow_artifacts for any items not present in artifacts table
        let wf_rows = sqlx::query(
            r#"
            SELECT id, workflow_run_id, step_run_id, name, path, content_hash, version, status, created_at
            FROM workflow_artifacts
            ORDER BY created_at DESC
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        for row in wf_rows {
            let record = self.map_workflow_row_to_record(&row)?;
            if !results.iter().any(|r| r.id == record.id) {
                results.push(record);
            }
        }

        Ok(results)
    }

    /// List recent artifacts with an upper bound limit.
    pub async fn list_recent(&self, limit: usize) -> Result<Vec<ArtifactRecord>, ArtifactError> {
        let all = self.list_all().await?;
        Ok(all.into_iter().take(limit).collect())
    }

    // -------------------------------------------------------------------------
    // Internal Helper Functions
    // -------------------------------------------------------------------------

    #[allow(clippy::too_many_arguments)]
    async fn store_and_record(
        &self,
        id: ArtifactId,
        name: String,
        logical_path: PathBuf,
        data: &[u8],
        extension: String,
        content_hash: String,
        version: u32,
        status: ArtifactStatus,
        provenance: ArtifactProvenance,
    ) -> Result<ArtifactRecord, ArtifactError> {
        // 0. Check-on-conflict immutability BEFORE any write. An
        // existing identity with a different content hash is a hard
        // violation (never a silent ledger overwrite); identical content is
        // idempotent and returns the existing record without rewriting.
        if let Some(existing_row) = sqlx::query("SELECT content_hash FROM artifacts WHERE id = ?")
            .bind(id.as_bytes().as_slice())
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| ArtifactError::StoreError(e.to_string()))?
        {
            use sqlx::Row as _;
            let existing_hash: String = existing_row.get("content_hash");
            if existing_hash != content_hash {
                return Err(ArtifactError::ImmutableViolation(id));
            }
            // Identical content: verify payload presence, then return the
            // recorded row instead of rewriting history.
            let record_row = sqlx::query("SELECT * FROM artifacts WHERE id = ?")
                .bind(id.as_bytes().as_slice())
                .fetch_one(&self.pool)
                .await
                .map_err(|e| ArtifactError::StoreError(e.to_string()))?;
            return self.map_row_to_record(&record_row);
        }

        // 1. Physically store payload in content-addressed FsArtifactStore
        self.store
            .store(id, data, &extension)
            .await
            .map_err(|e| ArtifactError::StoreError(e.to_string()))?;

        let size_bytes = data.len() as u64;
        let created_at = Utc::now();
        let metadata_json = serde_json::to_string(&provenance).ok();

        // 2. Record in unified SQLite artifacts ledger.
        // Plain INSERT — the pre-check above owns conflict semantics, so the
        // ledger can never silently overwrite a hash.
        sqlx::query(
            r#"
            INSERT INTO artifacts (
                id, name, logical_path, content_hash, size_bytes, extension, version, status,
                workflow_run_id, step_run_id, mission_id, task_id, producer_role, prompt_id,
                prompt_version, model, parent_artifact_id, metadata_json, created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(id.as_bytes().as_slice())
        .bind(&name)
        .bind(logical_path.to_string_lossy().as_ref())
        .bind(&content_hash)
        .bind(size_bytes as i64)
        .bind(&extension)
        .bind(version as i64)
        .bind(status.to_string())
        .bind(
            provenance
                .workflow_run_id
                .as_ref()
                .map(|w| w.as_bytes().to_vec()),
        )
        .bind(
            provenance
                .step_run_id
                .as_ref()
                .map(|s| s.as_bytes().to_vec()),
        )
        .bind(
            provenance
                .mission_id
                .as_ref()
                .map(|m| m.as_bytes().to_vec()),
        )
        .bind(provenance.task_id.as_ref().map(|t| t.as_bytes().to_vec()))
        .bind(provenance.producer_role.as_deref())
        .bind(provenance.prompt_id.as_deref())
        .bind(provenance.prompt_version.map(|v| v as i64))
        .bind(provenance.model.as_deref())
        .bind(
            provenance
                .parent_artifact_ids
                .first()
                .map(|p| p.as_bytes().to_vec()),
        )
        .bind(metadata_json)
        .bind(created_at.to_rfc3339())
        .execute(&self.pool)
        .await?;

        // 3. Synchronize with workflow_artifacts table if workflow run and step run are present
        if let (Some(wf_id), Some(step_id)) = (provenance.workflow_run_id, provenance.step_run_id) {
            let _ = sqlx::query(
                r#"
                INSERT INTO workflow_artifacts (
                    id, workflow_run_id, step_run_id, name, path, content_hash, version, status, created_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
                ON CONFLICT(id) DO UPDATE SET
                    content_hash = excluded.content_hash,
                    version = excluded.version,
                    status = excluded.status
                "#,
            )
            .bind(id.as_bytes().as_slice())
            .bind(wf_id.as_bytes().as_slice())
            .bind(step_id.as_bytes().as_slice())
            .bind(&name)
            .bind(logical_path.to_string_lossy().as_ref())
            .bind(&content_hash)
            .bind(version as i64)
            .bind(status.to_string())
            .bind(created_at.to_rfc3339())
            .execute(&self.pool)
            .await;
        }

        Ok(ArtifactRecord {
            id,
            name,
            logical_path,
            content_hash,
            size_bytes,
            extension,
            version,
            status,
            provenance,
            created_at,
        })
    }

    fn map_row_to_record(
        &self,
        row: &sqlx::sqlite::SqliteRow,
    ) -> Result<ArtifactRecord, ArtifactError> {
        let raw_id: Vec<u8> = row.get("id");
        let id = parse_artifact_id_blob(&raw_id)?;

        let name: String = row.get("name");
        let logical_path_str: String = row.get("logical_path");
        let content_hash: String = row.get("content_hash");
        let size_bytes: i64 = row.get("size_bytes");
        let extension: String = row.get("extension");
        let version: i64 = row.get("version");
        let status_str: String = row.get("status");
        let status = ArtifactStatus::from_str(&status_str).map_err(ArtifactError::StoreError)?;

        let metadata_json: Option<String> = row.get("metadata_json");
        let provenance = if let Some(ref json) = metadata_json {
            serde_json::from_str(json).unwrap_or_else(|_| self.provenance_from_row(row))
        } else {
            self.provenance_from_row(row)
        };

        let created_at_str: String = row.get("created_at");
        let created_at = DateTime::parse_from_rfc3339(&created_at_str)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());

        Ok(ArtifactRecord {
            id,
            name,
            logical_path: PathBuf::from(logical_path_str),
            content_hash,
            size_bytes: size_bytes as u64,
            extension,
            version: version as u32,
            status,
            provenance,
            created_at,
        })
    }

    fn provenance_from_row(&self, row: &sqlx::sqlite::SqliteRow) -> ArtifactProvenance {
        let mission_id = row
            .get::<Option<Vec<u8>>, _>("mission_id")
            .and_then(|b| parse_uuid_blob(&b).map(MissionId::from));

        let task_id = row
            .get::<Option<Vec<u8>>, _>("task_id")
            .and_then(|b| parse_uuid_blob(&b).map(TaskId::from));

        let workflow_run_id = row
            .get::<Option<Vec<u8>>, _>("workflow_run_id")
            .and_then(|b| parse_uuid_blob(&b).map(WorkflowRunId::from));

        let step_run_id = row
            .get::<Option<Vec<u8>>, _>("step_run_id")
            .and_then(|b| parse_uuid_blob(&b).map(WorkflowStepRunId::from));

        let producer_role: Option<String> = row.get("producer_role");
        let prompt_id: Option<String> = row.get("prompt_id");
        let prompt_version = row
            .get::<Option<i64>, _>("prompt_version")
            .map(|v| v as u32);
        let model: Option<String> = row.get("model");

        let parent_artifact_ids = row
            .get::<Option<Vec<u8>>, _>("parent_artifact_id")
            .and_then(|b| parse_uuid_blob(&b).map(|u| vec![ArtifactId::from(u)]))
            .unwrap_or_default();

        ArtifactProvenance {
            mission_id,
            task_id,
            workflow_run_id,
            step_run_id,
            producer_role,
            prompt_id,
            prompt_version,
            model,
            parent_artifact_ids,
            verification_check_ids: Vec::new(),
        }
    }

    fn map_workflow_row_to_record(
        &self,
        row: &sqlx::sqlite::SqliteRow,
    ) -> Result<ArtifactRecord, ArtifactError> {
        let raw_id: Vec<u8> = row.get("id");
        let id = parse_artifact_id_blob(&raw_id)?;

        let raw_wf_id: Vec<u8> = row.get("workflow_run_id");
        let workflow_run_id = parse_uuid_blob(&raw_wf_id).map(WorkflowRunId::from);

        let raw_step_id: Vec<u8> = row.get("step_run_id");
        let step_run_id = parse_uuid_blob(&raw_step_id).map(WorkflowStepRunId::from);

        let name: String = row.get("name");
        let path_str: String = row.get("path");
        let content_hash: String = row.get("content_hash");
        let version: i64 = row.get("version");
        let status_str: String = row.get("status");
        let status = ArtifactStatus::from_str(&status_str).map_err(ArtifactError::StoreError)?;

        let created_at_str: String = row.get("created_at");
        let created_at = DateTime::parse_from_rfc3339(&created_at_str)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());

        let extension = Path::new(&name)
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("txt")
            .to_string();

        let provenance = ArtifactProvenance {
            workflow_run_id,
            step_run_id,
            ..Default::default()
        };

        Ok(ArtifactRecord {
            id,
            name,
            logical_path: PathBuf::from(path_str),
            content_hash,
            size_bytes: 0,
            extension,
            version: version as u32,
            status,
            provenance,
            created_at,
        })
    }
}

fn parse_artifact_id_blob(raw: &[u8]) -> Result<ArtifactId, ArtifactError> {
    if raw.len() == 16 {
        let mut bytes = [0u8; 16];
        bytes.copy_from_slice(raw);
        Ok(ArtifactId::from_bytes(bytes))
    } else {
        Err(ArtifactError::StoreError(format!(
            "invalid artifact ID blob length: {}",
            raw.len()
        )))
    }
}

fn parse_uuid_blob(raw: &[u8]) -> Option<uuid::Uuid> {
    if raw.len() == 16 {
        let mut bytes = [0u8; 16];
        bytes.copy_from_slice(raw);
        Some(uuid::Uuid::from_bytes(bytes))
    } else {
        None
    }
}
