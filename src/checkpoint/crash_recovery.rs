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
    #[error("Crash recovery scan cancelled: {0}")]
    Cancelled(String),
}

/// Scan-scoped shared validation state.
///
/// Database-wide `PRAGMA integrity_check` and the current workspace
/// baseline are properties of the whole scan, not of any single mission.
/// Capturing them once per `scan_all_in_flight` (rather than once per
/// mission) removes O(M) redundant full-DB scans and O(M) full-workspace
/// filesystem walks while preserving per-mission drift semantics: each
/// mission still loads its own saved baseline and checkpoint manifest and
/// compares against the shared current file-hash map.
struct ScanSharedContext {
    /// `None` when the database-wide integrity check passed; `Some(detail)`
    /// with the failure detail when it did not (fail-closed for every mission).
    integrity_failure: Option<String>,
    /// Current workspace file hashes captured once per scan (`None` when the
    /// workspace root is missing or capture failed; the error is recorded in
    /// `baseline_error` and each safe mission degrades to `NeedsRepair`).
    baseline_hashes: Option<std::collections::BTreeMap<String, String>>,
    /// Baseline capture failure detail (when `baseline_hashes` is `None`
    /// because capture failed, as opposed to workspace-missing which is
    /// decided per mission from disk state).
    baseline_error: Option<String>,
    /// Whether the workspace root existed at capture time.
    workspace_existed: bool,
}

/// Outcome of background jobs reconciliation.
#[derive(Debug, Clone, Default)]
pub struct BackgroundJobsReconciliation {
    pub reconciled_count: usize,
    pub killed_count: usize,
    pub sealed_count: usize,
    pub failed_kills: Vec<(u32, String)>,
    pub spool_failures: Vec<(String, String)>,
}

/// Staged Startup Crash Recovery Scanner (CHK-02, CHK-03, D-14, D-15).
pub struct StartupCrashRecoveryScanner {
    pool: SqlitePool,
    artifact_store: Arc<dyn ArtifactStore>,
    workspace_root: PathBuf,
    /// Number of full database-wide `PRAGMA integrity_check` executions
    /// performed by this scanner instance. Exposed for regression tests
    /// proving the scan-scope (once-per-scan) invariant.
    integrity_checks: Arc<std::sync::atomic::AtomicUsize>,
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
            integrity_checks: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        }
    }

    /// Number of full database-wide integrity checks executed so far.
    ///
    /// Regression-test hook proving the scan-scope invariant: a scan over
    /// N in-flight missions must perform exactly one integrity check, not N.
    pub fn integrity_check_count(&self) -> usize {
        self.integrity_checks
            .load(std::sync::atomic::Ordering::SeqCst)
    }

    /// Execute the database-wide `PRAGMA integrity_check` exactly once.
    ///
    /// `PRAGMA integrity_check` scans the entire database file; it is a
    /// property of the database, never of a single mission. Callers must
    /// invoke this once per scan and reuse the outcome for every mission.
    /// Both transport errors and non-`ok` payloads fail closed.
    async fn check_database_integrity_once(&self) -> Option<String> {
        self.integrity_checks
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let started = std::time::Instant::now();
        let pragma_row = sqlx::query("PRAGMA integrity_check;")
            .fetch_one(&self.pool)
            .await;
        let elapsed = started.elapsed();
        let integrity_ok = match &pragma_row {
            Err(_) => false,
            Ok(row) => {
                let text: Option<String> = row.try_get::<Option<String>, _>(0).unwrap_or(None);
                matches!(
                    text.as_deref().map(str::to_lowercase).as_deref(),
                    Some("ok")
                )
            }
        };
        tracing::info!(
            stage = crate::startup_progress::stage::CRASH_RECOVERY_INTEGRITY,
            status = if integrity_ok { "success" } else { "failure" },
            elapsed_ms = elapsed.as_millis() as u64,
            "database integrity check completed"
        );
        if integrity_ok {
            None
        } else {
            Some(match &pragma_row {
                Err(e) => format!("SQLite integrity check failed: {e}"),
                Ok(_) => "SQLite integrity check returned non-ok payload".to_string(),
            })
        }
    }

    /// Capture the current workspace file-hash map once per scan.
    ///
    /// `RepositoryBaseline::capture` walks the entire workspace and hashes
    /// every file (synchronous blocking I/O). It is offloaded with
    /// `spawn_blocking` so the Tokio worker is never stalled, and captured
    /// once per scan: the resulting hashes are identical for every mission
    /// in the scan (the `mission_id` parameter only tags ownership).
    async fn capture_shared_baseline_once(&self) -> ScanSharedContext {
        let workspace_existed = self.workspace_root.exists();
        if !workspace_existed {
            return ScanSharedContext {
                integrity_failure: None,
                baseline_hashes: None,
                baseline_error: None,
                workspace_existed: false,
            };
        }
        let root = self.workspace_root.clone();
        let started = std::time::Instant::now();
        // `capture` needs a mission tag; the file hashes do not depend on it.
        // Use a fresh random id for the throwaway tag — only `file_hashes` is reused.
        let blocking = tokio::task::spawn_blocking(move || {
            RepositoryBaseline::capture(&root, MissionId::new(), None, None)
        })
        .await;
        let elapsed = started.elapsed();
        match blocking {
            Err(e) => {
                tracing::warn!(
                    stage = crate::startup_progress::stage::CRASH_RECOVERY_WORKSPACE,
                    status = "warning",
                    elapsed_ms = elapsed.as_millis() as u64,
                    "workspace baseline capture task failed: {e}"
                );
                ScanSharedContext {
                    integrity_failure: None,
                    baseline_hashes: None,
                    baseline_error: Some(format!(
                        "Failed to capture current workspace baseline: {e}"
                    )),
                    workspace_existed: true,
                }
            }
            Ok(Err(e)) => {
                tracing::warn!(
                    stage = crate::startup_progress::stage::CRASH_RECOVERY_WORKSPACE,
                    status = "warning",
                    elapsed_ms = elapsed.as_millis() as u64,
                    "workspace baseline capture failed: {e}"
                );
                ScanSharedContext {
                    integrity_failure: None,
                    baseline_hashes: None,
                    baseline_error: Some(format!(
                        "Failed to capture current workspace baseline: {e}"
                    )),
                    workspace_existed: true,
                }
            }
            Ok(Ok(baseline)) => {
                tracing::info!(
                    stage = crate::startup_progress::stage::CRASH_RECOVERY_WORKSPACE,
                    status = "success",
                    elapsed_ms = elapsed.as_millis() as u64,
                    file_count = baseline.file_hashes.len(),
                    "shared current-baseline snapshot captured once for scan"
                );
                ScanSharedContext {
                    integrity_failure: None,
                    baseline_hashes: Some(baseline.file_hashes),
                    baseline_error: None,
                    workspace_existed: true,
                }
            }
        }
    }

    /// Prepare scan-scoped shared state: one integrity check + one baseline capture.
    async fn prepare_scan_context(&self) -> ScanSharedContext {
        let integrity_failure = self.check_database_integrity_once().await;
        if integrity_failure.is_some() {
            // Fail closed without touching the filesystem: no baseline is
            // needed when every mission will be classified Corrupt.
            return ScanSharedContext {
                integrity_failure,
                baseline_hashes: None,
                baseline_error: None,
                workspace_existed: self.workspace_root.exists(),
            };
        }
        let mut ctx = self.capture_shared_baseline_once().await;
        ctx.integrity_failure = None;
        ctx
    }

    /// Scan and reconcile a specific mission after daemon startup (D-14).
    ///
    /// Single-mission entry point: performs the scan-scoped validation
    /// (one integrity check + one baseline capture) for exactly this
    /// mission, then reconciles it. When called in a loop over many
    /// missions prefer [`Self::scan_all_in_flight`], which shares those
    /// database-wide/filesystem-wide steps across the whole scan.
    pub async fn scan_and_reconcile(
        &self,
        mission_id: MissionId,
    ) -> Result<CrashRecoveryResult, CrashRecoveryError> {
        self.scan_and_reconcile_cancelled(mission_id, None).await
    }

    /// Single-mission reconcile with cooperative cancellation.
    pub async fn scan_and_reconcile_cancelled(
        &self,
        mission_id: MissionId,
        cancel: Option<&tokio_util::sync::CancellationToken>,
    ) -> Result<CrashRecoveryResult, CrashRecoveryError> {
        if let Some(tok) = cancel
            && tok.is_cancelled()
        {
            return Err(CrashRecoveryError::Cancelled(
                "crash recovery cancelled before single-mission scan".to_string(),
            ));
        }
        let ctx = self.prepare_scan_context().await;
        if let Some(tok) = cancel
            && tok.is_cancelled()
        {
            return Err(CrashRecoveryError::Cancelled(
                "crash recovery cancelled during single-mission scan".to_string(),
            ));
        }
        self.reconcile_one_with_context(mission_id, &ctx).await
    }

    /// Reconcile one mission against pre-validated scan-shared state.
    ///
    /// `ctx.integrity_failure == Some` fails closed to `Corrupt` without
    /// examining checkpoints, workspace, jobs, or tasks. Otherwise runs
    /// steps 2 (checkpoint), 3 (workspace drift against the shared
    /// baseline), 4 (jobs/tasks), and 5-7 (persist). Never promotes an
    /// unexamined mission to `SafeToResume`.
    async fn reconcile_one_with_context(
        &self,
        mission_id: MissionId,
        ctx: &ScanSharedContext,
    ) -> Result<CrashRecoveryResult, CrashRecoveryError> {
        // ---------------------------------------------------------------------
        // Step 1 (shared): Database-wide integrity outcome reused for this mission.
        // ---------------------------------------------------------------------
        if let Some(detail) = ctx.integrity_failure.as_ref() {
            let result = CrashRecoveryResult {
                scan_id: Uuid::now_v7(),
                mission_id,
                classification: CrashRecoveryClassification::Corrupt,
                checkpoint_id: None,
                reconciled_jobs_count: 0,
                killed_process_groups_count: 0,
                sealed_spools_count: 0,
                explanation: detail.clone(),
                scanned_at: Utc::now(),
            };
            // Persistence failures are NOT swallowed: they propagate so the
            // caller (runtime assembly) records an actionable recovery error
            // instead of reporting fake success.
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
        } else {
            // No checkpoint exists for this mission.
            // Check whether the mission performed work requiring recovery.
            let completed_or_failed_tasks: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM tasks WHERE mission_id = ? AND LOWER(status) IN ('succeeded', 'completed', 'failed')"
            )
            .bind(mission_id.as_bytes().as_slice())
            .fetch_one(&self.pool)
            .await?;

            let job_count: i64 =
                sqlx::query_scalar("SELECT COUNT(*) FROM jobs WHERE mission_id = ?")
                    .bind(mission_id.as_bytes().as_slice())
                    .fetch_one(&self.pool)
                    .await?;

            let mutation_count: i64 =
                sqlx::query_scalar("SELECT COUNT(*) FROM change_proposals WHERE mission_id = ?")
                    .bind(mission_id.as_bytes().as_slice())
                    .fetch_one(&self.pool)
                    .await?;

            if completed_or_failed_tasks == 0 && job_count == 0 && mutation_count == 0 {
                classification = CrashRecoveryClassification::SafeToResume;
                explanation = "Pristine mission without checkpoints; safe to resume.".to_string();
            } else {
                classification = CrashRecoveryClassification::NeedsRepair;
                explanation = format!(
                    "Mission has uncheckpointed work ({completed_or_failed_tasks} completed/failed tasks, {job_count} jobs, {mutation_count} mutations) but no checkpoints exist; cannot safely resume"
                );
            }
        }

        // ---------------------------------------------------------------------
        // Step 3: Repository Workspace Reconciliation (D-14 Step 3)
        //
        // The current file-hash map was captured ONCE per scan (shared
        // `ctx`); per-mission semantics are preserved because each mission
        // still loads its OWN saved baseline / checkpoint snapshot identity
        // and compares it against the shared current hashes. No mission is
        // skipped and drift is still evaluated per mission.
        // ---------------------------------------------------------------------
        if classification.is_safe()
            && let Some(ref manifest) = manifest_opt
        {
            if !ctx.workspace_existed || !self.workspace_root.exists() {
                classification = CrashRecoveryClassification::NeedsRepair;
                explanation = "Workspace root directory does not exist on disk".to_string();
            } else if let Some(ref baseline_err) = ctx.baseline_error {
                classification = CrashRecoveryClassification::NeedsRepair;
                explanation = baseline_err.clone();
            } else if let Some(ref current_hashes) = ctx.baseline_hashes {
                // Load newest recorded baseline for mission
                match RepositoryBaseline::load_latest_for_mission(&self.pool, mission_id).await {
                    Ok(Some(saved_baseline)) => {
                        let authorized = BTreeSet::new();
                        let drift = detect_drift(&saved_baseline, current_hashes, &authorized);
                        if drift.has_drift {
                            classification = CrashRecoveryClassification::Ambiguous;
                            explanation = format!(
                                "Unexplained repository drift detected (additions: {}, modifications: {}, deletions: {})",
                                drift.unexpected_additions.len(),
                                drift.unexpected_modifications.len(),
                                drift.unexpected_deletions.len()
                            );
                        }
                    }
                    Ok(None) => {
                        if !manifest.snapshot_identity.is_empty() {
                            // Compare baseline hash if saved baseline row wasn't present
                            let mut hasher = Sha256::new();
                            let fp_json = serde_json::to_string(current_hashes).unwrap();
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
                        classification = CrashRecoveryClassification::Corrupt;
                        explanation =
                            format!("Database error reading repository baseline for mission: {e}");
                    }
                }
            } else {
                classification = CrashRecoveryClassification::NeedsRepair;
                explanation =
                    "Failed to capture current workspace baseline: no shared baseline available"
                        .to_string();
            }
        }

        // ---------------------------------------------------------------------
        // Step 4: Background Job & Task Reconciliation (D-14 Step 4, D-15, D-16)
        // ---------------------------------------------------------------------
        let jobs_reconcile = self.reconcile_background_jobs(mission_id).await?;
        let reconciled_tasks = self.reconcile_in_flight_tasks(mission_id).await?;
        if reconciled_tasks > 0 {
            explanation =
                format!("{explanation} ({reconciled_tasks} in-flight tasks reconciled to pending)");
        }
        if !jobs_reconcile.failed_kills.is_empty() {
            classification = CrashRecoveryClassification::NeedsRepair;
            explanation = format!(
                "{explanation} (failed to terminate abandoned processes: {:?})",
                jobs_reconcile.failed_kills
            );
        }
        if !jobs_reconcile.spool_failures.is_empty() {
            explanation = format!(
                "{explanation} (spool promotion failures: {:?})",
                jobs_reconcile.spool_failures
            );
        }

        // ---------------------------------------------------------------------
        // Step 5, 6 & 7: Finalize Classification & Persist Scan Result (D-14)
        // ---------------------------------------------------------------------
        let result = CrashRecoveryResult {
            scan_id: Uuid::now_v7(),
            mission_id,
            classification,
            checkpoint_id: checkpoint_id_opt,
            reconciled_jobs_count: jobs_reconcile.reconciled_count,
            killed_process_groups_count: jobs_reconcile.killed_count,
            sealed_spools_count: jobs_reconcile.sealed_count,
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
    ) -> Result<BackgroundJobsReconciliation, sqlx::Error> {
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
        let mut failed_kills = Vec::new();
        let mut spool_failures = Vec::new();
        let now_str = Utc::now().to_rfc3339();

        for row in rows {
            let job_id_raw: Vec<u8> = row.get("id");
            let pid_opt: Option<i64> = row.get("pid");
            let recovery_json_opt: Option<String> = row.get("recovery_metadata_json");
            let stdout_spool_path_opt: Option<String> = row.get("stdout_spool_path");
            let stderr_spool_path_opt: Option<String> = row.get("stderr_spool_path");

            let mut process_termination_failed = None;

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
                        match ProcessTreeController::kill_process_group(
                            pid,
                            Duration::from_millis(1000),
                        )
                        .await
                        {
                            Ok(()) => {
                                killed_count += 1;
                            }
                            Err(e) => {
                                let err_msg = e.to_string();
                                failed_kills.push((pid, err_msg.clone()));
                                process_termination_failed = Some(err_msg);
                            }
                        }
                    }
                    // Else: PID was recycled by another process. DO NOT SEND SIGNALS (D-15).
                }
            }

            // 2. Seal spools with [INTERRUPTED] and promote to FsArtifactStore (D-15)
            let stdout_path = stdout_spool_path_opt.as_deref().map(Path::new);
            let stderr_path = stderr_spool_path_opt.as_deref().map(Path::new);

            let mut spool_promotion_failed = None;
            let artifact_id = if stdout_path.is_some() || stderr_path.is_some() {
                match seal_and_promote_spool_paths(
                    stdout_path,
                    stderr_path,
                    self.artifact_store.as_ref(),
                )
                .await
                {
                    Ok(aid) => {
                        sealed_count += 1;
                        Some(aid)
                    }
                    Err(e) => {
                        let err_msg = e.to_string();
                        let job_repr = Uuid::from_slice(&job_id_raw)
                            .map(|u| u.to_string())
                            .unwrap_or_else(|_| format!("{:?}", job_id_raw));
                        spool_failures.push((job_repr, err_msg.clone()));
                        spool_promotion_failed = Some(err_msg);
                        None
                    }
                }
            } else {
                None
            };

            // 3. Mark job as Lost in SQLite with truthful failure reason
            let failure_reason = match (process_termination_failed, spool_promotion_failed) {
                (Some(k_err), Some(s_err)) => {
                    format!(
                        "Terminated by startup crash recovery scanner (kill failed: {k_err}; spool promotion failed: {s_err})"
                    )
                }
                (Some(k_err), None) => {
                    format!("Terminated by startup crash recovery scanner (kill failed: {k_err})")
                }
                (None, Some(s_err)) => {
                    format!(
                        "Terminated by startup crash recovery scanner (spool promotion failed: {s_err})"
                    )
                }
                (None, None) => "Terminated by startup crash recovery scanner".to_string(),
            };

            sqlx::query(
                r#"
                UPDATE jobs
                SET state = 'Lost', artifact_id = ?, completed_at = ?, failure_reason = ?
                WHERE id = ?
                "#,
            )
            .bind(artifact_id.as_ref().map(|id| id.as_bytes().as_slice()))
            .bind(&now_str)
            .bind(&failure_reason)
            .bind(&job_id_raw)
            .execute(&self.pool)
            .await?;

            reconciled_count += 1;
        }

        Ok(BackgroundJobsReconciliation {
            reconciled_count,
            killed_count,
            sealed_count,
            failed_kills,
            spool_failures,
        })
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
    ///
    /// Performance invariant: the database-wide `PRAGMA integrity_check`
    /// and the workspace baseline capture each execute exactly ONCE per
    /// scan, regardless of mission count. Per-mission work (checkpoint
    /// validation, saved-baseline load, drift comparison, job/task
    /// reconciliation, result persistence) still executes per mission in
    /// deterministic mission-id order. Database writes, process kills,
    /// spool sealing, and task mutations are never parallelized.
    pub async fn scan_all_in_flight(&self) -> Result<Vec<CrashRecoveryResult>, CrashRecoveryError> {
        self.scan_all_in_flight_with_progress(None, None).await
    }

    /// Scan all in-flight missions with live progress and cooperative cancellation.
    ///
    /// - `progress`: when `Some`, receives `Recovering unfinished missions (i/n)…`
    ///   updates derived from real counters.
    /// - `cancel`: when `Some` and cancelled, the scan stops between missions
    ///   and returns `Err(CrashRecoveryError::Cancelled)` so partial recovery
    ///   is never misreported as complete.
    pub async fn scan_all_in_flight_with_progress(
        &self,
        mut progress: Option<&mut crate::startup_progress::StartupProgressReporter>,
        cancel: Option<&tokio_util::sync::CancellationToken>,
    ) -> Result<Vec<CrashRecoveryResult>, CrashRecoveryError> {
        if let Some(tok) = cancel
            && tok.is_cancelled()
        {
            return Err(CrashRecoveryError::Cancelled(
                "crash recovery cancelled before enumeration".to_string(),
            ));
        }
        let enum_started = std::time::Instant::now();
        let rows = sqlx::query(
            "SELECT id FROM missions WHERE LOWER(status) NOT IN ('completed', 'succeeded', 'failed', 'cancelled')"
        )
        .fetch_all(&self.pool)
        .await?;
        tracing::info!(
            stage = crate::startup_progress::stage::CRASH_RECOVERY_ENUMERATE,
            status = "success",
            elapsed_ms = enum_started.elapsed().as_millis() as u64,
            in_flight_missions = rows.len(),
            "enumerated in-flight missions"
        );

        let mut mission_ids = Vec::new();
        for row in rows {
            let id_raw: Vec<u8> = row.get("id");
            if id_raw.len() == 16 {
                let mut bytes = [0u8; 16];
                bytes.copy_from_slice(&id_raw);
                mission_ids.push(MissionId::from_bytes(bytes));
            }
        }
        // Deterministic ordering: the existing safety model reconciles jobs,
        // kills processes, seals spools, and mutates tasks sequentially.
        mission_ids.sort_by_key(|a| a.to_string());

        if mission_ids.is_empty() {
            // Database-wide integrity check still executes once to ensure DB health,
            // but expensive workspace hash scanning is skipped when there are no in-flight missions.
            let _ = self.check_database_integrity_once().await;
            return Ok(Vec::new());
        }

        // Scan-scoped shared validation: exactly one integrity check and one
        // baseline capture for the whole scan.
        let ctx = self.prepare_scan_context().await;

        let mut results = Vec::new();
        let total = mission_ids.len();
        for (idx, mid) in mission_ids.into_iter().enumerate() {
            if let Some(tok) = cancel
                && tok.is_cancelled()
            {
                return Err(CrashRecoveryError::Cancelled(format!(
                    "crash recovery cancelled after {idx}/{total} missions"
                )));
            }
            if let Some(rep) = progress.as_mut() {
                rep.report_stage_with_counts(
                    crate::startup_progress::stage::CRASH_RECOVERY_CHECKPOINTS,
                    format!("Recovering unfinished missions ({}/{})…", idx + 1, total),
                    idx,
                    total,
                );
            }
            // Persistence errors propagate (never swallowed into fake success).
            let res = self.reconcile_one_with_context(mid, &ctx).await?;
            results.push(res);
        }
        tracing::info!(
            stage = crate::startup_progress::stage::CRASH_RECOVERY_TASKS,
            status = "success",
            reconciled_missions = results.len(),
            "crash recovery scan completed"
        );
        Ok(results)
    }
}
