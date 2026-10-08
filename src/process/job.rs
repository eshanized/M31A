//! Asynchronous background job supervisor, durable SQLite job manager, lifecycle management,
//! and crash-safe startup reconciliation (CTL-01, TL-02, JOB-01, JOB-02, per D-13, D-15, D-16, §235).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{Row, SqlitePool};
use thiserror::Error;
use tokio::io::AsyncReadExt;
use tokio::process::Command;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;

use crate::ids::{AgentId, ArtifactId, JobId, MissionId, TaskId};
use crate::persistence::artifacts::fs_store::ArtifactStore;
use crate::process::admission::JobAdmissionController;
use crate::process::env::EnvironmentBuilder;
use crate::process::spool::{
    DEFAULT_MAX_RING_BYTES, DEFAULT_MAX_RING_LINES, DualBufferOutput, StreamType,
};
use crate::process::tree::ProcessTreeController;
use crate::process::types::{JobDescriptor, JobOutputChunk, JobStatusInfo};
use crate::sandbox::ResourceLimits;

/// The 9 lifecycle states for background jobs (D-13).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobState {
    Submitted,
    Starting,
    Running,
    Completed,
    Failed,
    Cancelled,
    TimedOut,
    ResourceExceeded,
    Lost,
}

impl JobState {
    #[allow(non_upper_case_globals)]
    pub const Pending: JobState = JobState::Submitted;
    #[allow(non_upper_case_globals)]
    pub const Stopped: JobState = JobState::Cancelled;

    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            Self::Completed
                | Self::Failed
                | Self::Cancelled
                | Self::TimedOut
                | Self::ResourceExceeded
                | Self::Lost
        )
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Submitted => "submitted",
            Self::Starting => "starting",
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
            Self::TimedOut => "timed_out",
            Self::ResourceExceeded => "resource_exceeded",
            Self::Lost => "lost",
        }
    }
}

impl std::fmt::Display for JobState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for JobState {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "submitted" | "pending" => Ok(JobState::Submitted),
            "starting" => Ok(JobState::Starting),
            "running" => Ok(JobState::Running),
            "completed" => Ok(JobState::Completed),
            "failed" => Ok(JobState::Failed),
            "cancelled" | "stopped" => Ok(JobState::Cancelled),
            "timed_out" => Ok(JobState::TimedOut),
            "resource_exceeded" => Ok(JobState::ResourceExceeded),
            "lost" => Ok(JobState::Lost),
            other => Err(format!("Unknown job state: {}", other)),
        }
    }
}

/// Durable and in-memory background job descriptor (JOB-01, D-13).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BackgroundJobRecord {
    pub job_id: JobId,
    pub mission_id: MissionId,
    pub task_id: TaskId,
    pub agent_id: AgentId,
    pub tool_call_id: Option<String>,
    pub command: String,
    pub args: Vec<String>,
    pub working_dir: PathBuf,
    pub state: JobState,
    pub pid: Option<u32>,
    pub provider: String,
    pub resource_limits: ResourceLimits,
    pub stdout_spool_path: Option<PathBuf>,
    pub stderr_spool_path: Option<PathBuf>,
    pub artifact_id: Option<ArtifactId>,
    pub exit_code: Option<i32>,
    pub failure_reason: Option<String>,
    pub submitted_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
}

impl BackgroundJobRecord {
    /// Return the immutable ownership tuple (D-13).
    pub fn ownership(&self) -> (MissionId, TaskId, AgentId) {
        (self.mission_id, self.task_id, self.agent_id)
    }
}

/// Submission request for background job creation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubmitJobRequest {
    pub mission_id: MissionId,
    pub task_id: TaskId,
    pub agent_id: AgentId,
    pub tool_call_id: Option<String>,
    pub command: String,
    pub args: Vec<String>,
    pub working_dir: PathBuf,
    pub provider: String,
    pub resource_limits: ResourceLimits,
    pub timeout: Option<Duration>,
}

/// Typed background job errors.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum JobError {
    #[error("Job '{0}' not found")]
    NotFound(JobId),

    #[error("Failed to start job: {0}")]
    StartFailed(String),

    #[error("Job '{0}' is already terminated with state '{1}'")]
    AlreadyTerminal(JobId, &'static str),

    #[error("I/O error during job management: {0}")]
    Io(String),

    #[error("Access denied: task '{0}' does not own job '{1}'")]
    AccessDenied(TaskId, JobId),

    #[error("Capacity exceeded: {0}")]
    CapacityExceeded(String),

    #[error("Database error: {0}")]
    Database(String),

    #[error("Corrupt persisted job record: {0}")]
    Corrupt(String),
}

/// Handle tracking active background job runtime resources in memory.
struct ActiveJobHandle {
    cancel_token: CancellationToken,
    spool: Arc<DualBufferOutput>,
    pid: Arc<AtomicU32>,
    mission_id: MissionId,
    task_id: TaskId,
}

/// Read process `starttime` from the host kernel interface where available.
///
/// The Linux backend reads field 22 of the process status record. Hosts
/// without that interface return `None` so reconciliation treats the
/// identifier as unprovable rather than guessing.
pub fn read_linux_process_starttime(pid: u32) -> Option<u64> {
    crate::platform::process::read_process_starttime(pid)
}

/// Reconcile starting/running jobs on daemon startup after a crash (JOB-02, D-13, §235).
///
/// Verifies recorded identifiers through the platform start-time Backend
/// where the host provides one. Unprovable PIDs or recycling mismatches
/// are transitioned to `JobState::Lost` in SQLite.
///
/// CRITICAL SAFETY LAW (D-13, §235): Never issue `kill` signals during startup reconciliation
/// to avoid killing recycled unrelated host processes.
pub async fn reconcile_jobs_on_startup(pool: &SqlitePool) -> Result<usize, sqlx::Error> {
    let rows = sqlx::query(
        r#"
        SELECT id, pid, recovery_metadata_json, state
        FROM jobs
        WHERE state IN ('submitted', 'starting', 'running')
        "#,
    )
    .fetch_all(pool)
    .await?;

    let mut reconciled_count = 0;
    let now = Utc::now().to_rfc3339();

    for row in rows {
        let job_id_raw: Vec<u8> = row.get("id");
        let pid_opt: Option<i64> = row.get("pid");
        let recovery_json_opt: Option<String> = row.get("recovery_metadata_json");

        let (mark_lost, reason) = if let Some(pid_i64) = pid_opt {
            let pid = pid_i64 as u32;
            let current_ident = crate::process::identity::ProcessIdentity::capture(pid);

            if let Some(ref r_json) = recovery_json_opt
                && let Some(recorded_ident) =
                    crate::process::identity::ProcessIdentity::parse_metadata(r_json)
            {
                if current_ident.is_live() && recorded_ident.matches(&current_ident) {
                    // Start time matched, but pipes/supervision are severed across daemon crash.
                    // Per D-13 & §235, unprovable/severed jobs must be marked Lost without kill signal.
                    (
                        true,
                        "Process disconnected after daemon restart".to_string(),
                    )
                } else {
                    // PID was recycled by another process or does not exist!
                    (
                        true,
                        format!(
                            "Process unprovable after daemon restart: PID {} was recycled by an unrelated process or exited",
                            pid
                        ),
                    )
                }
            } else if current_ident.is_live() {
                (
                    true,
                    "Process disconnected after daemon restart".to_string(),
                )
            } else {
                // Process does not exist
                (
                    true,
                    format!(
                        "Process unprovable after daemon restart: PID {} does not exist",
                        pid
                    ),
                )
            }
        } else {
            // Never received a PID
            (
                true,
                "Job was not started prior to daemon restart".to_string(),
            )
        };

        if mark_lost {
            // CRITICAL SAFETY INVARIANT (D-13, §235):
            // Never issue kill signals to PIDs during crash reconciliation to prevent killing recycled processes.
            sqlx::query(
                r#"
                UPDATE jobs
                SET state = 'lost',
                    failure_reason = ?,
                    completed_at = ?
                WHERE id = ?
                "#,
            )
            .bind(&reason)
            .bind(&now)
            .bind(&job_id_raw)
            .execute(pool)
            .await?;

            reconciled_count += 1;
        }
    }

    Ok(reconciled_count)
}

/// Durable SQLite-backed background job manager (JOB-01, JOB-02, D-13, D-15, D-16).
#[derive(Clone)]
pub struct JobManager {
    pool: SqlitePool,
    admission: Arc<JobAdmissionController>,
    artifact_store: Arc<dyn ArtifactStore>,
    spool_dir: PathBuf,
    active_jobs: Arc<RwLock<HashMap<JobId, Arc<ActiveJobHandle>>>>,
    workspace_root: Option<PathBuf>,
}

impl JobManager {
    /// Create a new SQLite-backed JobManager.
    pub fn new(
        pool: SqlitePool,
        admission: Arc<JobAdmissionController>,
        artifact_store: Arc<dyn ArtifactStore>,
        spool_dir: PathBuf,
    ) -> Self {
        Self {
            pool,
            admission,
            artifact_store,
            spool_dir,
            active_jobs: Arc::new(RwLock::new(HashMap::new())),
            workspace_root: None,
        }
    }

    /// Pin job working-directory containment to an authorized workspace root.
    /// When set, every submitted job's working directory is validated against
    /// this root before spawn; without it, jobs still get command-safety
    /// validation, environment scrubbing, and output bounds, but not
    /// workspace containment.
    pub fn with_workspace_root(mut self, root: PathBuf) -> Self {
        self.workspace_root = Some(root);
        self
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    pub fn admission(&self) -> &Arc<JobAdmissionController> {
        &self.admission
    }

    pub fn artifact_store(&self) -> &Arc<dyn ArtifactStore> {
        &self.artifact_store
    }

    pub fn spool_dir(&self) -> &PathBuf {
        &self.spool_dir
    }

    /// Submit an asynchronous background job with durable SQLite registration (JOB-01, D-13).
    pub async fn submit_job(&self, req: SubmitJobRequest) -> Result<JobId, JobError> {
        let job_id = JobId::new();
        let now = Utc::now().to_rfc3339();

        let args_json = serde_json::to_string(&req.args)
            .map_err(|e| JobError::StartFailed(format!("Failed to serialize args: {}", e)))?;
        let limits_json = serde_json::to_string(&req.resource_limits).map_err(|e| {
            JobError::StartFailed(format!("Failed to serialize resource limits: {}", e))
        })?;
        let working_dir_str = req.working_dir.to_string_lossy().to_string();

        let spool = Arc::new(
            DualBufferOutput::new(
                job_id,
                self.spool_dir.clone(),
                DEFAULT_MAX_RING_LINES,
                DEFAULT_MAX_RING_BYTES,
            )
            .map_err(|e| JobError::Io(format!("Failed to initialize dual-buffer output: {}", e)))?
            // Bound durable per-stream output by the job's resource limits
            // instead of growing spool files without bound.
            .with_max_spool_bytes(req.resource_limits.max_output_bytes as u64),
        );

        let stdout_spool = spool.stdout_spool_path().to_string_lossy().to_string();
        let stderr_spool = spool.stderr_spool_path().to_string_lossy().to_string();

        // 1. Durably record job in SQLite with state 'submitted'
        sqlx::query(
            r#"
            INSERT INTO jobs (
                id, mission_id, task_id, agent_id, tool_call_id,
                command, args_json, working_dir, state,
                pid, provider, resource_limits_json,
                stdout_spool_path, stderr_spool_path, artifact_id,
                exit_code, failure_reason, heartbeat_at, recovery_metadata_json,
                submitted_at, started_at, completed_at
            ) VALUES (
                ?, ?, ?, ?, ?,
                ?, ?, ?, 'submitted',
                NULL, ?, ?,
                ?, ?, NULL,
                NULL, NULL, ?, NULL,
                ?, NULL, NULL
            )
            "#,
        )
        .bind(job_id.as_bytes().as_slice())
        .bind(req.mission_id.as_bytes().as_slice())
        .bind(req.task_id.as_bytes().as_slice())
        .bind(req.agent_id.as_bytes().as_slice())
        .bind(&req.tool_call_id)
        .bind(&req.command)
        .bind(&args_json)
        .bind(&working_dir_str)
        .bind(&req.provider)
        .bind(&limits_json)
        .bind(&stdout_spool)
        .bind(&stderr_spool)
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await
        .map_err(|e| JobError::Database(e.to_string()))?;

        // 2. Register runtime handle in active_jobs
        let cancel_token = CancellationToken::new();
        let pid_atomic = Arc::new(AtomicU32::new(0));

        let handle = Arc::new(ActiveJobHandle {
            cancel_token: cancel_token.clone(),
            spool: Arc::clone(&spool),
            pid: Arc::clone(&pid_atomic),
            mission_id: req.mission_id,
            task_id: req.task_id,
        });

        {
            let mut active = self.active_jobs.write().await;
            active.insert(job_id, Arc::clone(&handle));
        }

        // 3. Spawn background execution task
        let runner_pool = self.pool.clone();
        let runner_admission = Arc::clone(&self.admission);
        let runner_store = Arc::clone(&self.artifact_store);
        let runner_spool = Arc::clone(&spool);
        let runner_active = Arc::clone(&self.active_jobs);
        let runner_workspace = self.workspace_root.clone();
        let req_clone = req.clone();

        tokio::spawn(async move {
            // Step 3a: Wait for admission permit
            let acquire_timeout = req_clone.timeout.unwrap_or(Duration::from_secs(
                crate::config::canonical::DEFAULT_WORKFLOW_STEP_TIMEOUT_SECS,
            ));
            let permit_res = runner_admission
                .acquire_permit(req_clone.mission_id, acquire_timeout)
                .await;

            let permit = match permit_res {
                Ok(p) => p,
                Err(e) => {
                    let reason = format!("Admission failed: {}", e);
                    let now_ts = Utc::now().to_rfc3339();
                    let _ = sqlx::query(
                        "UPDATE jobs SET state = 'resource_exceeded', failure_reason = ?, completed_at = ? WHERE id = ?"
                    )
                    .bind(&reason)
                    .bind(&now_ts)
                    .bind(job_id.as_bytes().as_slice())
                    .execute(&runner_pool)
                    .await;

                    let mut active = runner_active.write().await;
                    active.remove(&job_id);
                    return;
                }
            };

            // Step 3b: Transition Submitted -> Starting
            let _ = sqlx::query("UPDATE jobs SET state = 'starting' WHERE id = ?")
                .bind(job_id.as_bytes().as_slice())
                .execute(&runner_pool)
                .await;

            if cancel_token.is_cancelled() {
                let now_ts = Utc::now().to_rfc3339();
                let _ = sqlx::query(
                    "UPDATE jobs SET state = 'cancelled', completed_at = ? WHERE id = ?",
                )
                .bind(&now_ts)
                .bind(job_id.as_bytes().as_slice())
                .execute(&runner_pool)
                .await;

                let mut active = runner_active.write().await;
                active.remove(&job_id);
                drop(permit);
                return;
            }

            // Step 3c: Build the child through the canonical hardened spawn
            // helper: working-directory containment, command safety, and
            // deny-by-default environment scrubbing. A violation fails the
            // job closed before any process exists.
            let spawn_root = runner_workspace
                .clone()
                .unwrap_or_else(|| req_clone.working_dir.clone());
            let hardened = crate::process::hardened::HardenedSpawn::new(spawn_root);
            let mut cmd = match hardened.build_command(
                &req_clone.command,
                &req_clone.args,
                Some(&req_clone.working_dir),
            ) {
                Ok(cmd) => cmd,
                Err(e) => {
                    let reason = format!("Job rejected by process security boundary: {}", e);
                    let now_ts = Utc::now().to_rfc3339();
                    let _ = sqlx::query(
                        "UPDATE jobs SET state = 'failed', failure_reason = ?, completed_at = ? WHERE id = ?"
                    )
                    .bind(&reason)
                    .bind(&now_ts)
                    .bind(job_id.as_bytes().as_slice())
                    .execute(&runner_pool)
                    .await;

                    let mut active = runner_active.write().await;
                    active.remove(&job_id);
                    drop(permit);
                    return;
                }
            };
            let budget = crate::platform::resources::ResourceBudget {
                max_cpu_seconds: req_clone.resource_limits.cpu_time_secs,
                max_memory_bytes: req_clone.resource_limits.memory_bytes,
                max_processes: req_clone.resource_limits.max_processes,
                max_open_files: req_clone.resource_limits.max_open_files,
                max_output_bytes: Some(req_clone.resource_limits.max_output_bytes),
            };
            cmd.stdout(Stdio::piped());
            cmd.stderr(Stdio::piped());

            let (mut child, tree) = match ProcessTreeController::spawn_isolated_with_budget(
                cmd,
                Some(&budget),
            ) {
                Ok((c, t)) => (c, t),
                Err(e) => {
                    let reason = format!("Failed to spawn process: {}", e);
                    let now_ts = Utc::now().to_rfc3339();
                    let _ = sqlx::query(
                            "UPDATE jobs SET state = 'failed', failure_reason = ?, completed_at = ? WHERE id = ?"
                        )
                        .bind(&reason)
                        .bind(&now_ts)
                        .bind(job_id.as_bytes().as_slice())
                        .execute(&runner_pool)
                        .await;

                    let mut active = runner_active.write().await;
                    active.remove(&job_id);
                    drop(permit);
                    return;
                }
            };

            let pid = tree.pid();
            pid_atomic.store(pid, Ordering::SeqCst);

            let ident = crate::process::identity::ProcessIdentity::capture(pid);
            let recovery_json = serde_json::to_string(&ident).unwrap_or_default();

            let started_at = Utc::now().to_rfc3339();

            // Step 3d: Transition Starting -> Running
            let _ = sqlx::query(
                r#"
                UPDATE jobs SET
                    state = 'running',
                    pid = ?,
                    recovery_metadata_json = ?,
                    started_at = ?
                WHERE id = ?
                "#,
            )
            .bind(pid as i64)
            .bind(&recovery_json)
            .bind(&started_at)
            .bind(job_id.as_bytes().as_slice())
            .execute(&runner_pool)
            .await;

            // Step 3e: Stream stdout and stderr concurrently without pipe deadlock
            let mut stdout = child.stdout.take();
            let mut stderr = child.stderr.take();

            let spool_out = Arc::clone(&runner_spool);
            let stdout_task = tokio::spawn(async move {
                if let Some(mut pipe) = stdout.take() {
                    let mut buf = [0u8; 4096];
                    while let Ok(n) = pipe.read(&mut buf).await {
                        if n == 0 {
                            break;
                        }
                        let _ = spool_out.append(StreamType::Stdout, &buf[..n]);
                    }
                }
            });

            let spool_err = Arc::clone(&runner_spool);
            let stderr_task = tokio::spawn(async move {
                if let Some(mut pipe) = stderr.take() {
                    let mut buf = [0u8; 4096];
                    while let Ok(n) = pipe.read(&mut buf).await {
                        if n == 0 {
                            break;
                        }
                        let _ = spool_err.append(StreamType::Stderr, &buf[..n]);
                    }
                }
            });

            // Step 3f: Await completion, timeout, or cancellation
            let timeout_dur = req_clone.timeout;

            let final_state: JobState;
            let mut exit_code: Option<i32> = None;
            let mut failure_reason: Option<String> = None;

            tokio::select! {
                wait_res = child.wait() => {
                    let _ = stdout_task.await;
                    let _ = stderr_task.await;
                    match wait_res {
                        Ok(status) => {
                            exit_code = status.code();
                            if status.success() {
                                final_state = JobState::Completed;
                            } else {
                                final_state = JobState::Failed;
                                failure_reason = Some(format!("Process exited with status {:?}", status.code()));
                            }
                        }
                        Err(e) => {
                            final_state = JobState::Failed;
                            failure_reason = Some(e.to_string());
                        }
                    }
                }
                _ = async {
                    if let Some(to) = timeout_dur {
                        tokio::time::sleep(to).await;
                    } else {
                        std::future::pending::<()>().await;
                    }
                } => {
                    let _ = tree.terminate_supervised(&mut child, Duration::from_millis(1000)).await;
                    let _ = stdout_task.await;
                    let _ = stderr_task.await;
                    final_state = JobState::TimedOut;
                    failure_reason = Some("Job execution timed out".into());
                }
                _ = cancel_token.cancelled() => {
                    let _ = tree.terminate_supervised(&mut child, Duration::from_millis(1000)).await;
                    let _ = stdout_task.await;
                    let _ = stderr_task.await;
                    final_state = JobState::Cancelled;
                    failure_reason = Some("Job was cancelled".into());
                }
            }

            // Step 3g: Atomic promotion of output spools to ArtifactStore (D-16)
            let promo_result = runner_spool
                .promote_to_artifact_store(
                    runner_store.as_ref(),
                    req_clone.mission_id,
                    req_clone.task_id,
                    job_id,
                )
                .await;

            let artifact_id = promo_result.ok().map(|p| p.artifact_id);
            let artifact_bytes = artifact_id.as_ref().map(|id| id.as_bytes().to_vec());

            // Step 3h: Update SQLite with terminal state
            let completed_at = Utc::now().to_rfc3339();
            let state_str = final_state.as_str();
            // Never claim complete output when the spool cap dropped bytes;
            // record truncation honestly in the terminal row.
            if runner_spool.truncated() {
                let note = format!(
                    "output truncated at {} bytes per stream",
                    req_clone.resource_limits.max_output_bytes
                );
                failure_reason = Some(match failure_reason.take() {
                    Some(prev) => format!("{prev}; {note}"),
                    None => note,
                });
            }

            let _ = sqlx::query(
                r#"
                UPDATE jobs SET
                    state = ?,
                    artifact_id = ?,
                    exit_code = ?,
                    failure_reason = ?,
                    completed_at = ?
                WHERE id = ?
                "#,
            )
            .bind(state_str)
            .bind(artifact_bytes)
            .bind(exit_code.map(|c| c as i64))
            .bind(failure_reason)
            .bind(&completed_at)
            .bind(job_id.as_bytes().as_slice())
            .execute(&runner_pool)
            .await;

            // Step 3i: Release runtime handle and permit
            {
                let mut active = runner_active.write().await;
                active.remove(&job_id);
            }
            drop(permit);
        });

        Ok(job_id)
    }

    /// Retrieve durable status record for a job from SQLite.
    pub async fn job_status(&self, job_id: &JobId) -> Result<BackgroundJobRecord, JobError> {
        let row_opt = sqlx::query(
            r#"
            SELECT
                id, mission_id, task_id, agent_id, tool_call_id,
                command, args_json, working_dir, state,
                pid, provider, resource_limits_json,
                stdout_spool_path, stderr_spool_path, artifact_id,
                exit_code, failure_reason, heartbeat_at, recovery_metadata_json,
                submitted_at, started_at, completed_at
            FROM jobs
            WHERE id = ?
            "#,
        )
        .bind(job_id.as_bytes().as_slice())
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| JobError::Database(e.to_string()))?;

        let row = row_opt.ok_or(JobError::NotFound(*job_id))?;
        decode_job_row(&row)
    }

    /// Retrieve live or spooled output chunk for a job (D-16).
    pub async fn job_output(
        &self,
        job_id: &JobId,
        offset: u64,
        limit: usize,
    ) -> Result<JobOutputChunk, JobError> {
        // First check in-memory active handles
        let handle_opt = {
            let active = self.active_jobs.read().await;
            active.get(job_id).cloned()
        };

        if let Some(handle) = handle_opt {
            return handle
                .spool
                .read_chunk(offset, limit)
                .map_err(|e| JobError::Io(e.to_string()));
        }

        // Otherwise read directly from durable spool paths recorded in SQLite
        let record = self.job_status(job_id).await?;
        let stdout_path = record
            .stdout_spool_path
            .unwrap_or_else(|| self.spool_dir.join(format!("job_{}_stdout.spool", job_id)));
        let stderr_path = record
            .stderr_spool_path
            .unwrap_or_else(|| self.spool_dir.join(format!("job_{}_stderr.spool", job_id)));

        // If stdout_path exists, read slice
        let (stdout_text, next_off, is_eof) = if stdout_path.exists() {
            let mut file =
                std::fs::File::open(&stdout_path).map_err(|e| JobError::Io(e.to_string()))?;
            let len = file
                .metadata()
                .map_err(|e| JobError::Io(e.to_string()))?
                .len();
            if offset >= len {
                (String::new(), len, true)
            } else {
                use std::io::{Read, Seek, SeekFrom};
                file.seek(SeekFrom::Start(offset))
                    .map_err(|e| JobError::Io(e.to_string()))?;
                let to_read = (limit as u64).min(len - offset) as usize;
                let mut buf = vec![0u8; to_read];
                file.read_exact(&mut buf)
                    .map_err(|e| JobError::Io(e.to_string()))?;
                let text = String::from_utf8_lossy(&buf).to_string();
                let n_off = offset + to_read as u64;
                (text, n_off, n_off >= len)
            }
        } else {
            (String::new(), offset, true)
        };

        let stderr_text = if stderr_path.exists() {
            std::fs::read_to_string(&stderr_path).unwrap_or_default()
        } else {
            String::new()
        };

        Ok(JobOutputChunk {
            job_id: job_id.to_string(),
            stdout: crate::process::spool::strip_ansi(&stdout_text),
            stderr: crate::process::spool::strip_ansi(&stderr_text),
            next_offset: next_off,
            is_eof: is_eof && record.state.is_terminal(),
        })
    }

    /// Cancel a running job, escalating through process-tree signals (JOB-02, PRC-01).
    pub async fn cancel_job(&self, job_id: &JobId, grace: Duration) -> Result<(), JobError> {
        let handle_opt = {
            let active = self.active_jobs.read().await;
            active.get(job_id).cloned()
        };

        if let Some(handle) = handle_opt {
            handle.cancel_token.cancel();
            let pid = handle.pid.load(Ordering::SeqCst);
            if pid > 0 {
                let _ = ProcessTreeController::terminate_by_pid(pid, grace).await;
            }
            return Ok(());
        }

        // If not in active_jobs, verify SQLite record
        let record = self.job_status(job_id).await?;
        if record.state.is_terminal() {
            return Ok(());
        }

        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "UPDATE jobs SET state = 'cancelled', failure_reason = 'Cancelled by request', completed_at = ? WHERE id = ?"
        )
        .bind(&now)
        .bind(job_id.as_bytes().as_slice())
        .execute(&self.pool)
        .await
        .map_err(|e| JobError::Database(e.to_string()))?;

        if let Some(pid) = record.pid
            && pid > 0
        {
            let _ = ProcessTreeController::terminate_by_pid(pid, grace).await;
        }

        Ok(())
    }

    /// Hierarchically cancel all active jobs belonging to a task (PRC-01, JOB-02).
    pub async fn cancel_task_jobs(&self, task_id: TaskId) -> Result<usize, JobError> {
        let matching_jobs: Vec<JobId> = {
            let active = self.active_jobs.read().await;
            active
                .iter()
                .filter(|(_, h)| h.task_id == task_id)
                .map(|(id, _)| *id)
                .collect()
        };

        let count = matching_jobs.len();
        for id in matching_jobs {
            let _ = self.cancel_job(&id, Duration::from_millis(500)).await;
        }

        Ok(count)
    }

    /// Hierarchically cancel all active jobs belonging to a mission (PRC-01, JOB-02).
    pub async fn cancel_mission_jobs(&self, mission_id: MissionId) -> Result<usize, JobError> {
        let matching_jobs: Vec<JobId> = {
            let active = self.active_jobs.read().await;
            active
                .iter()
                .filter(|(_, h)| h.mission_id == mission_id)
                .map(|(id, _)| *id)
                .collect()
        };

        let count = matching_jobs.len();
        for id in matching_jobs {
            let _ = self.cancel_job(&id, Duration::from_millis(500)).await;
        }

        Ok(count)
    }

    /// Cancel all active background jobs across the entire runtime on shutdown or reset.
    pub async fn cancel_all_jobs(&self) -> Result<usize, JobError> {
        let all_jobs: Vec<JobId> = {
            let active = self.active_jobs.read().await;
            active.keys().copied().collect()
        };

        let count = all_jobs.len();
        for id in all_jobs {
            let _ = self.cancel_job(&id, Duration::from_millis(500)).await;
        }

        Ok(count)
    }

    /// Check whether a background job is still actively running on the host OS.
    pub async fn check_liveness(&self, job_id: &JobId) -> Result<bool, JobError> {
        let record = self.job_status(job_id).await?;
        if record.state.is_terminal() {
            return Ok(false);
        }
        if let Some(pid) = record.pid {
            if pid > 0 {
                return Ok(crate::platform::process::is_process_alive(pid));
            }
        }
        Ok(false)
    }

    /// List jobs matching optional mission and task filters.
    pub async fn list_jobs(
        &self,
        filter_mission: Option<MissionId>,
        filter_task: Option<TaskId>,
    ) -> Result<Vec<BackgroundJobRecord>, JobError> {
        let rows = if let (Some(m), Some(t)) = (filter_mission, filter_task) {
            sqlx::query(
                r#"
                SELECT
                    id, mission_id, task_id, agent_id, tool_call_id,
                    command, args_json, working_dir, state,
                    pid, provider, resource_limits_json,
                    stdout_spool_path, stderr_spool_path, artifact_id,
                    exit_code, failure_reason, heartbeat_at, recovery_metadata_json,
                    submitted_at, started_at, completed_at
                FROM jobs
                WHERE mission_id = ? AND task_id = ?
                ORDER BY submitted_at DESC
                "#,
            )
            .bind(m.as_bytes().as_slice())
            .bind(t.as_bytes().as_slice())
            .fetch_all(&self.pool)
            .await
        } else if let Some(m) = filter_mission {
            sqlx::query(
                r#"
                SELECT
                    id, mission_id, task_id, agent_id, tool_call_id,
                    command, args_json, working_dir, state,
                    pid, provider, resource_limits_json,
                    stdout_spool_path, stderr_spool_path, artifact_id,
                    exit_code, failure_reason, heartbeat_at, recovery_metadata_json,
                    submitted_at, started_at, completed_at
                FROM jobs
                WHERE mission_id = ?
                ORDER BY submitted_at DESC
                "#,
            )
            .bind(m.as_bytes().as_slice())
            .fetch_all(&self.pool)
            .await
        } else if let Some(t) = filter_task {
            sqlx::query(
                r#"
                SELECT
                    id, mission_id, task_id, agent_id, tool_call_id,
                    command, args_json, working_dir, state,
                    pid, provider, resource_limits_json,
                    stdout_spool_path, stderr_spool_path, artifact_id,
                    exit_code, failure_reason, heartbeat_at, recovery_metadata_json,
                    submitted_at, started_at, completed_at
                FROM jobs
                WHERE task_id = ?
                ORDER BY submitted_at DESC
                "#,
            )
            .bind(t.as_bytes().as_slice())
            .fetch_all(&self.pool)
            .await
        } else {
            sqlx::query(
                r#"
                SELECT
                    id, mission_id, task_id, agent_id, tool_call_id,
                    command, args_json, working_dir, state,
                    pid, provider, resource_limits_json,
                    stdout_spool_path, stderr_spool_path, artifact_id,
                    exit_code, failure_reason, heartbeat_at, recovery_metadata_json,
                    submitted_at, started_at, completed_at
                FROM jobs
                ORDER BY submitted_at DESC
                "#,
            )
            .fetch_all(&self.pool)
            .await
        }
        .map_err(|e| JobError::Database(e.to_string()))?;

        let mut records = Vec::with_capacity(rows.len());
        for row in rows {
            records.push(decode_job_row(&row)?);
        }

        Ok(records)
    }

    /// Number of active in-memory job runtime handles.
    pub async fn active_job_count(&self) -> usize {
        self.active_jobs.read().await.len()
    }
}

fn decode_job_row(row: &sqlx::sqlite::SqliteRow) -> Result<BackgroundJobRecord, JobError> {
    fn decode_id(raw: &[u8], field: &str) -> Result<[u8; 16], JobError> {
        if raw.len() != 16 {
            return Err(JobError::Corrupt(format!(
                "corrupt {field} identity: expected 16 bytes, got {}",
                raw.len()
            )));
        }
        let mut arr = [0u8; 16];
        arr.copy_from_slice(raw);
        Ok(arr)
    }
    let id_raw: Vec<u8> = row.get("id");
    let id_b = decode_id(&id_raw, "job id")?;

    let mission_raw: Vec<u8> = row.get("mission_id");
    let mission_b = decode_id(&mission_raw, "mission id")?;

    let task_raw: Vec<u8> = row.get("task_id");
    let task_b = decode_id(&task_raw, "task id")?;

    let agent_raw: Vec<u8> = row.get("agent_id");
    let agent_b = decode_id(&agent_raw, "agent id")?;

    let state_str: String = row.get("state");
    let state = state_str
        .parse()
        .map_err(|_| JobError::Corrupt(format!("corrupt job state value: '{state_str}'")))?;

    let args_json: String = row.get("args_json");
    let args = serde_json::from_str(&args_json)
        .map_err(|e| JobError::Corrupt(format!("corrupt job args_json: {e}")))?;

    let limits_json: String = row.get("resource_limits_json");
    let resource_limits = serde_json::from_str(&limits_json)
        .map_err(|e| JobError::Corrupt(format!("corrupt job resource_limits_json: {e}")))?;

    let working_dir: String = row.get("working_dir");
    let stdout_spool_path: Option<String> = row.get("stdout_spool_path");
    let stderr_spool_path: Option<String> = row.get("stderr_spool_path");

    let artifact_id = row
        .get::<Option<Vec<u8>>, _>("artifact_id")
        .map(|raw| decode_id(&raw, "artifact id").map(ArtifactId::from_bytes))
        .transpose()?;

    let submitted_at_str: String = row.get("submitted_at");
    let submitted_at = DateTime::parse_from_rfc3339(&submitted_at_str)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|e| JobError::Corrupt(format!("corrupt job submitted_at: {e}")))?;

    let started_at = row.get::<Option<String>, _>("started_at").and_then(|s| {
        DateTime::parse_from_rfc3339(&s)
            .map(|dt| dt.with_timezone(&Utc))
            .ok()
    });

    let completed_at = row.get::<Option<String>, _>("completed_at").and_then(|s| {
        DateTime::parse_from_rfc3339(&s)
            .map(|dt| dt.with_timezone(&Utc))
            .ok()
    });

    let pid = row.get::<Option<i64>, _>("pid").map(|p| p as u32);
    let exit_code = row.get::<Option<i64>, _>("exit_code").map(|c| c as i32);

    Ok(BackgroundJobRecord {
        job_id: JobId::from_bytes(id_b),
        mission_id: MissionId::from_bytes(mission_b),
        task_id: TaskId::from_bytes(task_b),
        agent_id: AgentId::from_bytes(agent_b),
        tool_call_id: row.get("tool_call_id"),
        command: row.get("command"),
        args,
        working_dir: PathBuf::from(working_dir),
        state,
        pid,
        provider: row.get("provider"),
        resource_limits,
        stdout_spool_path: stdout_spool_path.map(PathBuf::from),
        stderr_spool_path: stderr_spool_path.map(PathBuf::from),
        artifact_id,
        exit_code,
        failure_reason: row.get("failure_reason"),
        submitted_at,
        started_at,
        completed_at,
    })
}

/// In-memory background job supervisor preserved for provider compatibility (D-13, D-16).
///
/// CANONICAL PRODUCTION JOB AUTHORITY (with [`with_pool`](Self::with_pool)):
/// every submission carries the real mission/task/agent identity (never fresh
/// identifiers) and the effective resource limits, and every lifecycle
/// transition (`submitted` → `running` → terminal) is written to the durable
/// `jobs` table in the SAME row shape the startup reconciler reads. Production
/// execution therefore creates exactly the records startup recovery
/// reconciles — one lifecycle, not two competing implementations.
pub struct JobSupervisor {
    jobs: Arc<RwLock<HashMap<JobId, BackgroundJobRecord>>>,
    spools: Arc<RwLock<HashMap<JobId, Arc<DualBufferOutput>>>>,
    spool_dir: PathBuf,
    artifact_store: Option<Arc<dyn ArtifactStore>>,
    default_timeout: Duration,
    pool: Option<SqlitePool>,
}

impl JobSupervisor {
    /// Create a new JobSupervisor.
    pub fn new(spool_dir: PathBuf) -> Self {
        Self {
            jobs: Arc::new(RwLock::new(HashMap::new())),
            spools: Arc::new(RwLock::new(HashMap::new())),
            spool_dir,
            artifact_store: None,
            default_timeout: Duration::from_secs(
                crate::config::canonical::DEFAULT_WORKFLOW_STEP_TIMEOUT_SECS,
            ),
            pool: None,
        }
    }

    /// Attach the durable jobs ledger: submissions and lifecycle transitions
    /// are written to the `jobs` table in the reconciler's row shape.
    pub fn with_pool(mut self, pool: SqlitePool) -> Self {
        self.pool = Some(pool);
        self
    }

    /// Attach an artifact store for promoting completed job spools.
    pub fn with_artifact_store(mut self, store: Arc<dyn ArtifactStore>) -> Self {
        self.artifact_store = Some(store);
        self
    }

    /// Set a custom default timeout ceiling for background jobs.
    pub fn with_default_timeout(mut self, timeout: Duration) -> Self {
        self.default_timeout = timeout;
        self
    }

    /// Durable `submitted` row in the reconciler's row shape. Called before
    /// the child spawns; failure fails the start.
    #[allow(clippy::too_many_arguments)]
    async fn durable_insert_submitted(
        &self,
        job_id: &JobId,
        mission_id: &MissionId,
        task_id: &TaskId,
        agent_id: &AgentId,
        command: &str,
        args: &[String],
        cwd: &Path,
        resource_limits: &ResourceLimits,
        stdout_spool: &Path,
        stderr_spool: &Path,
    ) -> Result<(), JobError> {
        let pool = self
            .pool
            .clone()
            .ok_or_else(|| JobError::StartFailed("job ledger pool not attached".to_string()))?;
        let args_json = serde_json::to_string(args)
            .map_err(|e| JobError::StartFailed(format!("args serialization failed: {e}")))?;
        let limits_json = serde_json::to_string(resource_limits)
            .map_err(|e| JobError::StartFailed(format!("limits serialization failed: {e}")))?;
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            r#"
            INSERT INTO jobs (
                id, mission_id, task_id, agent_id, tool_call_id,
                command, args_json, working_dir, state,
                pid, provider, resource_limits_json,
                stdout_spool_path, stderr_spool_path, artifact_id,
                exit_code, failure_reason, heartbeat_at, recovery_metadata_json,
                submitted_at, started_at, completed_at
            ) VALUES (
                ?, ?, ?, ?, NULL,
                ?, ?, ?, 'submitted',
                NULL, 'local_process', ?,
                ?, ?, NULL,
                NULL, NULL, ?, NULL,
                ?, NULL, NULL
            )
            "#,
        )
        .bind(job_id.as_bytes().as_slice())
        .bind(mission_id.as_bytes().as_slice())
        .bind(task_id.as_bytes().as_slice())
        .bind(agent_id.as_bytes().as_slice())
        .bind(command)
        .bind(&args_json)
        .bind(cwd.to_string_lossy().to_string())
        .bind(&limits_json)
        .bind(stdout_spool.to_string_lossy().to_string())
        .bind(stderr_spool.to_string_lossy().to_string())
        .bind(&now)
        .bind(&now)
        .execute(&pool)
        .await
        .map_err(|e| JobError::Database(format!("durable job submission failed: {e}")))?;
        Ok(())
    }

    /// Durable `running` transition with verifiable process identity.
    async fn durable_update_running(
        &self,
        job_id: &JobId,
        pid: u32,
        recovery_metadata_json: &str,
    ) -> Result<(), JobError> {
        let pool = self
            .pool
            .clone()
            .ok_or_else(|| JobError::StartFailed("job ledger pool not attached".to_string()))?;
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "UPDATE jobs SET state = 'running', pid = ?, recovery_metadata_json = ?, started_at = ? WHERE id = ?",
        )
        .bind(pid as i64)
        .bind(recovery_metadata_json)
        .bind(&now)
        .bind(job_id.as_bytes().as_slice())
        .execute(&pool)
        .await
        .map_err(|e| JobError::Database(format!("durable job running update failed: {e}")))?;
        Ok(())
    }

    /// Launch a supervised asynchronous background job (D-13).
    ///
    /// Identity and limits are CALLER-PROVIDED and authoritative: the mission,
    /// task, and agent must be the real execution context, and
    /// `resource_limits` the effective limits derived from runtime
    /// policy/budget. Nothing is defaulted or freshly minted here.
    #[allow(clippy::too_many_arguments)]
    pub async fn start_job(
        &self,
        mission_id: MissionId,
        task_id: TaskId,
        agent_id: AgentId,
        resource_limits: ResourceLimits,
        command: &str,
        args: &[String],
        cwd: &Path,
        env_builder: Option<&EnvironmentBuilder>,
        timeout_override: Option<Duration>,
    ) -> Result<JobDescriptor, JobError> {
        let job_id = JobId::new();
        // Effective timeout: explicit override wins; otherwise the
        // authoritative per-task limit applies, capped by the supervisor
        // ceiling. The task's wall-clock budget is never silently discarded.
        let limit_timeout =
            Duration::from_millis(resource_limits.timeout_ms).max(Duration::from_secs(1));
        let timeout_dur =
            timeout_override.unwrap_or_else(|| limit_timeout.min(self.default_timeout));

        // Initialize dual buffer
        let spool = Arc::new(
            DualBufferOutput::new(
                job_id,
                self.spool_dir.clone(),
                DEFAULT_MAX_RING_LINES,
                DEFAULT_MAX_RING_BYTES,
            )
            .map_err(|e| JobError::Io(format!("Failed to initialize dual-buffer output: {}", e)))?,
        );

        // Build command with isolation
        let mut cmd = Command::new(command);
        cmd.args(args);
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        if let Some(builder) = env_builder {
            builder.apply(&mut cmd);
        } else {
            let default_builder = EnvironmentBuilder::new(cwd);
            default_builder.apply(&mut cmd);
        }

        // Durable submission FIRST (when a ledger pool is attached): the
        // `jobs` row exists before the child spawns, so a crash between spawn
        // and bookkeeping can never orphan an unrecorded process. A ledger
        // write failure fails the start — running an unreconcileable job
        // would violate the single-lifecycle invariant.
        if self.pool.is_some() {
            self.durable_insert_submitted(
                &job_id,
                &mission_id,
                &task_id,
                &agent_id,
                command,
                args,
                cwd,
                &resource_limits,
                spool.stdout_spool_path(),
                spool.stderr_spool_path(),
            )
            .await?;
        }

        let (mut child, tree) = ProcessTreeController::spawn_isolated(cmd)
            .map_err(|e| JobError::StartFailed(e.to_string()))?;

        let pid = tree.pid();
        let started_at = Utc::now();

        // Durable running state with verifiable process identity (pid +
        // linux starttime) so startup reconciliation can prove attachment and
        // never kill a recycled unrelated PID.
        if self.pool.is_some() {
            let recovery_meta = serde_json::json!({
                "linux_starttime": read_linux_process_starttime(pid),
            })
            .to_string();
            self.durable_update_running(&job_id, pid, &recovery_meta)
                .await?;
        }

        let record = BackgroundJobRecord {
            job_id,
            mission_id,
            task_id,
            agent_id,
            tool_call_id: None,
            command: command.to_string(),
            args: args.to_vec(),
            working_dir: cwd.to_path_buf(),
            state: JobState::Running,
            pid: Some(pid),
            provider: "local_process".to_string(),
            resource_limits: resource_limits.clone(),
            stdout_spool_path: Some(spool.stdout_spool_path().clone()),
            stderr_spool_path: Some(spool.stderr_spool_path().clone()),
            artifact_id: None,
            exit_code: None,
            failure_reason: None,
            submitted_at: started_at,
            started_at: Some(started_at),
            completed_at: None,
        };

        {
            let mut jobs_lock = self.jobs.write().await;
            jobs_lock.insert(job_id, record);
            let mut spools_lock = self.spools.write().await;
            spools_lock.insert(job_id, Arc::clone(&spool));
        }

        // Spawn background supervision task to stream output and track completion
        let jobs_ref = Arc::clone(&self.jobs);
        let spool_ref = Arc::clone(&spool);
        let artifact_store = self.artifact_store.clone();
        let durable_pool = self.pool.clone();

        tokio::spawn(async move {
            let mut stdout = child.stdout.take();
            let mut stderr = child.stderr.take();

            let spool_out = Arc::clone(&spool_ref);
            let stdout_task = tokio::spawn(async move {
                if let Some(mut pipe) = stdout.take() {
                    let mut buf = [0u8; 4096];
                    while let Ok(n) = pipe.read(&mut buf).await {
                        if n == 0 {
                            break;
                        }
                        let _ = spool_out.append(StreamType::Stdout, &buf[..n]);
                    }
                }
            });

            let spool_err = Arc::clone(&spool_ref);
            let stderr_task = tokio::spawn(async move {
                if let Some(mut pipe) = stderr.take() {
                    let mut buf = [0u8; 4096];
                    while let Ok(n) = pipe.read(&mut buf).await {
                        if n == 0 {
                            break;
                        }
                        let _ = spool_err.append(StreamType::Stderr, &buf[..n]);
                    }
                }
            });

            // Wait for exit or timeout
            let final_state: JobState;
            let final_code: Option<i32>;

            match tokio::time::timeout(timeout_dur, child.wait()).await {
                Ok(Ok(status)) => {
                    let _ = stdout_task.await;
                    let _ = stderr_task.await;
                    final_code = status.code();
                    final_state = if status.success() {
                        JobState::Completed
                    } else {
                        JobState::Failed
                    };
                }
                Ok(Err(_)) => {
                    let _ = stdout_task.await;
                    let _ = stderr_task.await;
                    final_code = None;
                    final_state = JobState::Failed;
                }
                Err(_) => {
                    // Timed out: kill process group
                    let _ = tree
                        .terminate_supervised(&mut child, Duration::from_millis(1000))
                        .await;
                    let _ = stdout_task.await;
                    let _ = stderr_task.await;
                    final_code = None;
                    final_state = JobState::TimedOut;
                }
            };

            // Finalize spool and promote to artifact store with the REAL
            // job ownership (never fabricated identifiers).
            let artifact_id = match &artifact_store {
                Some(store) => spool_ref
                    .finalize_for(store.as_ref(), mission_id, task_id)
                    .await
                    .ok(),
                None => {
                    let _ = spool_ref.finalize(None).await;
                    None
                }
            };

            // Update record
            let mut jobs_lock = jobs_ref.write().await;
            if let Some(rec) = jobs_lock.get_mut(&job_id) {
                if rec.state != JobState::Cancelled {
                    rec.state = final_state;
                }
                rec.completed_at = Some(Utc::now());
                rec.exit_code = final_code;
                rec.artifact_id = artifact_id;
            }
            let terminal_state = jobs_lock
                .get(&job_id)
                .map(|r| r.state)
                .unwrap_or(final_state);
            drop(jobs_lock);

            // Durable terminal state (best-effort after execution: the
            // process already ran, so a ledger failure is recorded as a
            // warning with the in-memory record as truth, never a fake
            // success row).
            if let Some(pool) = durable_pool {
                let now_str = Utc::now().to_rfc3339();
                let state_str = terminal_state.as_str();
                if let Err(e) = sqlx::query(
                    "UPDATE jobs SET state = ?, exit_code = ?, completed_at = ? WHERE id = ?",
                )
                .bind(state_str)
                .bind(final_code)
                .bind(&now_str)
                .bind(job_id.as_bytes().as_slice())
                .execute(&pool)
                .await
                {
                    tracing::warn!("durable job terminal update failed for {job_id}: {e}");
                }
            }
        });

        Ok(JobDescriptor {
            job_id: job_id.to_string(),
            command: command.to_string(),
            pid: Some(pid),
            started_at_ms: started_at.timestamp_millis().max(0) as u64,
        })
    }

    /// Retrieve status snapshot of a background job.
    pub async fn job_status(&self, job_id: &JobId) -> Result<JobStatusInfo, JobError> {
        let jobs_lock = self.jobs.read().await;
        let rec = jobs_lock.get(job_id).ok_or(JobError::NotFound(*job_id))?;

        let started_at = rec.started_at.unwrap_or(rec.submitted_at);
        let duration_ms = match rec.completed_at {
            Some(completed) => (completed - started_at).num_milliseconds().max(0) as u64,
            None => (Utc::now() - started_at).num_milliseconds().max(0) as u64,
        };

        let state_str = if rec.state == JobState::Cancelled {
            "stopped".to_string()
        } else {
            rec.state.as_str().to_string()
        };

        Ok(JobStatusInfo {
            job_id: rec.job_id.to_string(),
            state: state_str,
            exit_code: rec.exit_code,
            running_ms: duration_ms,
        })
    }

    /// Retrieve buffered output chunk starting at byte `offset`.
    pub async fn job_output(
        &self,
        job_id: &JobId,
        offset: u64,
        limit: usize,
    ) -> Result<JobOutputChunk, JobError> {
        let spools_lock = self.spools.read().await;
        let spool = spools_lock.get(job_id).ok_or(JobError::NotFound(*job_id))?;

        spool
            .read_chunk(offset, limit)
            .map_err(|e| JobError::Io(e.to_string()))
    }

    /// Stop and terminate a running job process tree (D-14).
    pub async fn job_stop(&self, job_id: &JobId, grace_ms: u64) -> Result<(), JobError> {
        let pid_opt: Option<u32>;
        {
            let mut jobs_lock = self.jobs.write().await;
            let rec = jobs_lock
                .get_mut(job_id)
                .ok_or(JobError::NotFound(*job_id))?;

            if rec.state.is_terminal() {
                return Ok(());
            }

            rec.state = JobState::Cancelled;
            pid_opt = rec.pid;
        }

        if let Some(pid) = pid_opt {
            let _ =
                ProcessTreeController::terminate_by_pid(pid, Duration::from_millis(grace_ms)).await;
        }

        Ok(())
    }

    /// Reaps all background jobs belonging to a task upon task completion, preventing orphan leaks (D-13).
    pub async fn reap_task_jobs(&self, task_id: &TaskId) -> usize {
        let mut to_stop = Vec::new();
        {
            let jobs_lock = self.jobs.read().await;
            for rec in jobs_lock.values() {
                if &rec.task_id == task_id && !rec.state.is_terminal() {
                    to_stop.push(rec.job_id);
                }
            }
        }

        let count = to_stop.len();
        for job_id in to_stop {
            let _ = self.job_stop(&job_id, 500).await;
        }
        count
    }

    /// List all background job records.
    pub async fn list_jobs(&self) -> Vec<BackgroundJobRecord> {
        self.jobs.read().await.values().cloned().collect()
    }
}
