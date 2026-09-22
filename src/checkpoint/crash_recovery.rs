//! Staged startup crash recovery scanner, process reconciliation & spool sealing (CHK-02, CHK-03, D-14, D-15).
//!
//! Enforces:
//! - Deterministic classification into 4 recovery states: `SafeToResume`, `NeedsRepair`, `Ambiguous`, `Corrupt`.
//! - Fail-closed: `Ambiguous` and `Corrupt` states are NEVER auto-promoted to `SafeToResume` or success.
//! - PID recycling protection: process starttimes from `/proc/<pid>/stat` are verified before sending signals.
//! - Surviving abandoned process groups are terminated via `ProcessTreeController` signal escalation.
//! - Open output spools are sealed with `[INTERRUPTED]` and promoted to `FsArtifactStore`.
//! - Complete scan results are persisted in SQLite prior to resumption.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{Row, SqlitePool};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;

use crate::checkpoint::integrity::{CheckpointIntegrityError, CheckpointIntegrityValidator};
use crate::checkpoint::manifest::CheckpointManifest;
use crate::ids::{CheckpointId, MissionId};
use crate::persistence::artifacts::fs_store::ArtifactStore;
use crate::process::job::read_linux_process_starttime;
use crate::process::spool::seal_and_promote_spool_paths;
use crate::process::tree::ProcessTreeController;
use crate::repo::drift::{RepositoryBaseline, detect_drift};

/// Deterministic crash recovery classification for unfinished missions (CHK-02, D-14).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CrashRecoveryClassification {
    /// Valid checkpoint, all artifacts valid, repo workspace compatible, jobs reconciled.
    SafeToResume,
    /// Valid checkpoint, but state requires deterministic repair (e.g. restoring workspace files).
    NeedsRepair,
    /// Unexplained repo drift, uncertain job outcomes, or conflicting evidence -> HALT / ESCALATE.
    Ambiguous,
    /// Missing required artifacts, corrupt SQLite, or invalid schema -> ROLLBACK or HALT.
    Corrupt,
}

impl CrashRecoveryClassification {
    /// Fail-closed rule (CHK-03): Ambiguous and Corrupt states are never safe to resume.
    pub fn is_safe(&self) -> bool {
        matches!(self, Self::SafeToResume)
    }

    /// String representation for logging and persistence.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::SafeToResume => "safe_to_resume",
            Self::NeedsRepair => "needs_repair",
            Self::Ambiguous => "ambiguous",
            Self::Corrupt => "corrupt",
        }
    }
}

/// Structured result of a startup crash recovery scan (D-14).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrashRecoveryResult {
    pub scan_id: Uuid,
    pub mission_id: MissionId,
    pub classification: CrashRecoveryClassification,
    pub checkpoint_id: Option<CheckpointId>,
    pub reconciled_jobs_count: usize,
    pub killed_process_groups_count: usize,
    pub sealed_spools_count: usize,
    pub explanation: String,
    pub scanned_at: DateTime<Utc>,
}

/// Errors emitted during crash recovery scanning.
#[derive(Debug, thiserror::Error)]
pub enum CrashRecoveryError {
    #[error("Database error during crash scan: {0}")]
    Database(#[from] sqlx::Error),
    #[error("I/O error during crash scan: {0}")]
    Io(#[from] std::io::Error),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("Integrity error: {0}")]
    Integrity(#[from] CheckpointIntegrityError),
}

/// Staged Startup Crash Recovery Scanner (CHK-02, CHK-03, D-14, D-15).
pub struct StartupCrashRecoveryScanner {
    pool: SqlitePool,
    artifact_store: Arc<dyn ArtifactStore>,
    workspace_root: PathBuf,
}

impl StartupCrashRecoveryScanner {
    /// Create a new scanner instance.
    pub fn new(
        pool: SqlitePool,
        artifact_store: Arc<dyn ArtifactStore>,
        workspace_root: impl AsRef<Path>,
    ) -> Self {
        Self {
            pool,
            artifact_store,
            workspace_root: workspace_root.as_ref().to_path_buf(),
        }
    }

    /// Scan and reconcile a specific mission after daemon startup (D-14).
    pub async fn scan_and_reconcile(
        &self,
        mission_id: MissionId,
    ) -> Result<CrashRecoveryResult, CrashRecoveryError> {
        // ---------------------------------------------------------------------
        // Step 1: SQLite WAL Recovery & Database Integrity Check (D-14 Step 1)
        // ---------------------------------------------------------------------
        // PRAGMA integrity_check returns one row per message; a corrupt
        // database can return Ok(rows) whose text is not "ok". Both
        // transport errors and non-ok payloads fail closed.
        let pragma_row = sqlx::query("PRAGMA integrity_check;")
            .fetch_one(&self.pool)
            .await;
        let integrity_ok = match &pragma_row {
            Err(_) => false,
            Ok(row) => {
                use sqlx::Row as _;
                let text: Option<String> = row.try_get::<Option<String>, _>(0).unwrap_or(None);
                matches!(
                    text.as_deref().map(str::to_lowercase).as_deref(),
                    Some("ok")
                )
            }
        };

        if !integrity_ok {
            let detail = match &pragma_row {
                Err(e) => format!("SQLite integrity check failed: {e}"),
                Ok(_) => "SQLite integrity check returned non-ok payload".to_string(),
            };
            let result = CrashRecoveryResult {
                scan_id: Uuid::now_v7(),
                mission_id,
                classification: CrashRecoveryClassification::Corrupt,
                checkpoint_id: None,
                reconciled_jobs_count: 0,
                killed_process_groups_count: 0,
                sealed_spools_count: 0,
                explanation: detail,
                scanned_at: Utc::now(),
            };
            self.persist_scan_result(&result).await?;
            return Ok(result);
        }

        // ---------------------------------------------------------------------
        // Step 2: Checkpoint Manifest Validation (D-14 Step 2, CHK-04)
        // ---------------------------------------------------------------------
        let cp_row = sqlx::query(
            r#"
            SELECT id, sequence, stage, cycle, manifest_json, manifest_hash
            FROM checkpoints
            WHERE mission_id = ?
            ORDER BY cycle DESC, sequence DESC
            LIMIT 1
            "#,
        )
        .bind(mission_id.as_bytes().as_slice())
        .fetch_optional(&self.pool)
        .await?;

        let mut manifest_opt: Option<CheckpointManifest> = None;
        let mut checkpoint_id_opt: Option<CheckpointId> = None;
        let mut classification = CrashRecoveryClassification::SafeToResume;
        let mut explanation = "Clean recovery state; safe to resume.".to_string();

        if let Some(r) = cp_row {
            let cp_id_bytes: Vec<u8> = r.get("id");
            let mut cp_bytes = [0u8; 16];
            if cp_id_bytes.len() == 16 {
                cp_bytes.copy_from_slice(&cp_id_bytes);
                checkpoint_id_opt = Some(CheckpointId::from_bytes(cp_bytes));
            }

            let manifest_json_opt: Option<String> = r.get("manifest_json");
            let manifest_hash_opt: Option<String> = r.get("manifest_hash");

            if let Some(manifest_json) = manifest_json_opt {
                match serde_json::from_str::<CheckpointManifest>(&manifest_json) {
                    Ok(manifest) => {
                        let validator =
                            CheckpointIntegrityValidator::new(self.artifact_store.clone());
                        if let Err(e) = validator
                            .validate_with_recorded_hash(&manifest, manifest_hash_opt.as_deref())
                            .await
                        {
                            classification = CrashRecoveryClassification::Corrupt;
                            explanation = format!("Checkpoint integrity validation failed: {e}");
                        } else {
                            manifest_opt = Some(manifest);
                        }
                    }
                    Err(e) => {
                        classification = CrashRecoveryClassification::Corrupt;
                        explanation = format!("Checkpoint manifest JSON is corrupted: {e}");
                    }
                }
            } else {
                classification = CrashRecoveryClassification::Corrupt;
                explanation = "Checkpoint manifest is missing from SQLite".to_string();
            }
        }

        // ---------------------------------------------------------------------
        // Step 3: Repository Workspace Reconciliation (D-14 Step 3)
        // ---------------------------------------------------------------------
        if classification.is_safe()
            && let Some(ref manifest) = manifest_opt
        {
            if self.workspace_root.exists() {
                // Capture current workspace baseline
                match RepositoryBaseline::capture(&self.workspace_root, mission_id, None, None) {
                    Ok(current_baseline) => {
                        // Load newest recorded baseline for mission
                        if let Ok(Some(saved_baseline)) =
                            RepositoryBaseline::load_latest_for_mission(&self.pool, mission_id)
                                .await
                        {
                            let authorized = BTreeSet::new();
                            let drift = detect_drift(
                                &saved_baseline,
                                &current_baseline.file_hashes,
                                &authorized,
                            );
                            if drift.has_drift {
                                classification = CrashRecoveryClassification::Ambiguous;
                                explanation = format!(
                                    "Unexplained repository drift detected (additions: {}, modifications: {}, deletions: {})",
                                    drift.unexpected_additions.len(),
                                    drift.unexpected_modifications.len(),
                                    drift.unexpected_deletions.len()
                                );
                            }
                        } else if !manifest.snapshot_identity.is_empty() {
                            // Compare baseline hash if saved baseline row wasn't present
                            let mut hasher = Sha256::new();
                            let fp_json =
                                serde_json::to_string(&current_baseline.file_hashes).unwrap();
                            hasher.update(fp_json.as_bytes());
                            let curr_hash = format!("{:x}", hasher.finalize());
                            if curr_hash != manifest.snapshot_identity {
                                classification = CrashRecoveryClassification::Ambiguous;
                                explanation =
                                    "Current workspace does not match checkpoint snapshot identity"
                                        .to_string();
                            }
                        }
                    }
                    Err(e) => {
                        classification = CrashRecoveryClassification::NeedsRepair;
                        explanation = format!("Failed to capture current workspace baseline: {e}");
                    }
                }
            } else {
                classification = CrashRecoveryClassification::NeedsRepair;
                explanation = "Workspace root directory does not exist on disk".to_string();
            }
        }

        // ---------------------------------------------------------------------
        // Step 4: Background Job & Task Reconciliation (D-14 Step 4, D-15, D-16)
        // ---------------------------------------------------------------------
        let (reconciled_jobs, killed_pgs, sealed_spools) =
            self.reconcile_background_jobs(mission_id).await?;
        let reconciled_tasks = self.reconcile_in_flight_tasks(mission_id).await?;
        if reconciled_tasks > 0 {
            explanation =
                format!("{explanation} ({reconciled_tasks} in-flight tasks reconciled to pending)");
        }

        // ---------------------------------------------------------------------
        // Step 5, 6 & 7: Finalize Classification & Persist Scan Result (D-14)
        // ---------------------------------------------------------------------
        let result = CrashRecoveryResult {
            scan_id: Uuid::now_v7(),
            mission_id,
            classification,
            checkpoint_id: checkpoint_id_opt,
            reconciled_jobs_count: reconciled_jobs,
            killed_process_groups_count: killed_pgs,
            sealed_spools_count: sealed_spools,
            explanation,
            scanned_at: Utc::now(),
        };

        self.persist_scan_result(&result).await?;

        Ok(result)
    }

    /// Reconcile in-flight tasks from crashed runs: transition running tasks back to pending (D-14, D-16).
    pub async fn reconcile_in_flight_tasks(
        &self,
        mission_id: MissionId,
    ) -> Result<usize, sqlx::Error> {
        let task_repo =
            crate::persistence::sqlite::repositories::SqliteTaskRepository::new(self.pool.clone());
        task_repo
            .reset_running_tasks_to_pending(mission_id)
            .await
            .map_err(|e| sqlx::Error::Protocol(e.to_string()))
    }

    /// Reconcile background jobs: terminate abandoned process groups and seal spools (D-15).
    async fn reconcile_background_jobs(
        &self,
        mission_id: MissionId,
    ) -> Result<(usize, usize, usize), sqlx::Error> {
        let rows = sqlx::query(
            r#"
            SELECT id, pid, recovery_metadata_json, stdout_spool_path, stderr_spool_path
            FROM jobs
            WHERE mission_id = ? AND state IN ('submitted', 'starting', 'running')
            "#,
        )
        .bind(mission_id.as_bytes().as_slice())
        .fetch_all(&self.pool)
        .await?;

        let mut reconciled_count = 0;
        let mut killed_count = 0;
        let mut sealed_count = 0;
        let now_str = Utc::now().to_rfc3339();

        for row in rows {
            let job_id_raw: Vec<u8> = row.get("id");
            let pid_opt: Option<i64> = row.get("pid");
            let recovery_json_opt: Option<String> = row.get("recovery_metadata_json");
            let stdout_spool_path_opt: Option<String> = row.get("stdout_spool_path");
            let stderr_spool_path_opt: Option<String> = row.get("stderr_spool_path");

            // 1. Process termination check via Linux starttime (D-15)
            if let Some(pid_i64) = pid_opt {
                let pid = pid_i64 as u32;
                let current_starttime = read_linux_process_starttime(pid);

                if let Some(curr_st) = current_starttime {
                    let mut matches_original = false;
                    if let Some(ref r_json) = recovery_json_opt
                        && let Ok(v) = serde_json::from_str::<serde_json::Value>(r_json)
                        && let Some(recorded_st) = v.get("linux_starttime").and_then(|s| s.as_u64())
                    {
                        matches_original = recorded_st == curr_st;
                    }

                    if matches_original {
                        // Abandoned process group survived! Terminate cleanly via two-phase signal escalation (D-15)
                        let _ = ProcessTreeController::kill_process_group(
                            pid,
                            Duration::from_millis(1000),
                        )
                        .await;
                        killed_count += 1;
                    }
                    // Else: PID was recycled by another process. DO NOT SEND SIGNALS (D-15).
                }
            }

            // 2. Seal spools with [INTERRUPTED] and promote to FsArtifactStore (D-15)
            let stdout_path = stdout_spool_path_opt.as_deref().map(Path::new);
            let stderr_path = stderr_spool_path_opt.as_deref().map(Path::new);

            let artifact_id = if stdout_path.is_some() || stderr_path.is_some() {
                if let Ok(aid) = seal_and_promote_spool_paths(
                    stdout_path,
                    stderr_path,
                    self.artifact_store.as_ref(),
                )
                .await
                {
                    sealed_count += 1;
                    Some(aid)
                } else {
                    None
                }
            } else {
                None
            };

            // 3. Mark job as Lost in SQLite
            sqlx::query(
                r#"
                UPDATE jobs
                SET state = 'Lost', artifact_id = ?, completed_at = ?, failure_reason = 'Terminated by startup crash recovery scanner'
                WHERE id = ?
                "#,
            )
            .bind(artifact_id.as_ref().map(|id| id.as_bytes().as_slice()))
            .bind(&now_str)
            .bind(&job_id_raw)
            .execute(&self.pool)
            .await?;

            reconciled_count += 1;
        }

        Ok((reconciled_count, killed_count, sealed_count))
    }

    /// Persist the scan result to SQLite before returning (Step 7).
    async fn persist_scan_result(&self, result: &CrashRecoveryResult) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            INSERT INTO crash_recovery_scans (id, mission_id, classification, checkpoint_id, reconciled_jobs_count, killed_process_groups_count, sealed_spools_count, explanation, scanned_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(result.scan_id.as_bytes().as_slice())
        .bind(result.mission_id.as_bytes().as_slice())
        .bind(result.classification.as_str())
        .bind(result.checkpoint_id.as_ref().map(|cp| cp.as_bytes().as_slice()))
        .bind(result.reconciled_jobs_count as i64)
        .bind(result.killed_process_groups_count as i64)
        .bind(result.sealed_spools_count as i64)
        .bind(&result.explanation)
        .bind(result.scanned_at.to_rfc3339())
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Scan and reconcile all in-flight or interrupted missions across the database (D-14, F-14).
    pub async fn scan_all_in_flight(&self) -> Result<Vec<CrashRecoveryResult>, CrashRecoveryError> {
        let rows = sqlx::query(
            "SELECT id FROM missions WHERE LOWER(status) NOT IN ('completed', 'succeeded', 'failed', 'cancelled')"
        )
        .fetch_all(&self.pool)
        .await?;

        let mut results = Vec::new();
        for row in rows {
            let id_raw: Vec<u8> = row.get("id");
            if id_raw.len() == 16 {
                let mut bytes = [0u8; 16];
                bytes.copy_from_slice(&id_raw);
                let mid = MissionId::from_bytes(bytes);
                let res = self.scan_and_reconcile(mid).await?;
                results.push(res);
            }
        }
        Ok(results)
    }
}
