//! Dual-buffer job output combining an in-memory ring buffer with append-only disk spooling (TL-02, D-16).
//!
//! Enforces:
//! - In-memory bounded ring buffer (e.g. 1000 lines / 64KB) for low-latency live tail queries.
//! - Append-only disk spool file written incrementally so output is preserved across runtime restarts.
//! - Terminal spool promotion: on completion, disk spool is hashed and promoted to an immutable artifact.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::VecDeque;
use std::fs::OpenOptions;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::ids::{ArtifactId, JobId, MissionId, TaskId};
use crate::persistence::artifacts::fs_store::ArtifactStore;
use crate::process::job::JobError;
use crate::process::types::JobOutputChunk;

pub const DEFAULT_MAX_RING_LINES: usize = 1000;
pub const DEFAULT_MAX_RING_BYTES: usize = 64 * 1024; // 64KB

/// Lifecycle state of a disk spool file (D-16).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SpoolLifecycleState {
    #[default]
    Open,
    Finalizing,
    Finalized,
    Corrupt,
}

/// Result of promoting disk spools to ArtifactStore (D-16).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpoolPromotionResult {
    pub artifact_id: ArtifactId,
    pub total_bytes: usize,
    pub stdout_bytes: usize,
    pub stderr_bytes: usize,
    pub sha256_hash: String,
}

/// Output stream selector.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamType {
    Stdout,
    Stderr,
}

/// In-memory ring buffer for streaming line output.
#[derive(Debug)]
struct RingBuffer {
    lines: VecDeque<String>,
    total_bytes: usize,
    max_lines: usize,
    max_bytes: usize,
}

impl RingBuffer {
    fn new(max_lines: usize, max_bytes: usize) -> Self {
        Self {
            lines: VecDeque::with_capacity(max_lines),
            total_bytes: 0,
            max_lines,
            max_bytes,
        }
    }

    fn push_line(&mut self, line: String) {
        let line_len = line.len();
        while (self.lines.len() >= self.max_lines || self.total_bytes + line_len > self.max_bytes)
            && !self.lines.is_empty()
        {
            if let Some(evicted) = self.lines.pop_front() {
                self.total_bytes = self.total_bytes.saturating_sub(evicted.len());
            }
        }
        self.total_bytes += line_len;
        self.lines.push_back(line);
    }

    fn get_tail(&self, limit: usize) -> Vec<String> {
        let count = self.lines.len().min(limit);
        self.lines
            .iter()
            .skip(self.lines.len() - count)
            .cloned()
            .collect()
    }
}

static ANSI_REGEX: std::sync::LazyLock<regex::Regex> =
    std::sync::LazyLock::new(|| regex::Regex::new(r"\x1B\[[0-9;]*[a-zA-Z]").unwrap());

/// Strip terminal ANSI escape sequences from a string (D-16).
pub fn strip_ansi(s: &str) -> String {
    ANSI_REGEX.replace_all(s, "").into_owned()
}

/// Dual-buffer output stream coordinator for an individual background job.
pub struct DualBufferOutput {
    job_id: JobId,
    stdout_spool: PathBuf,
    stderr_spool: PathBuf,
    stdout_ring: Mutex<RingBuffer>,
    stderr_ring: Mutex<RingBuffer>,
    is_finalized: Mutex<bool>,
    max_spool_bytes: Mutex<Option<u64>>,
    truncated: Mutex<bool>,
}

impl DualBufferOutput {
    /// Initialize dual buffers for a job inside `spool_dir`.
    pub fn new(
        job_id: JobId,
        spool_dir: PathBuf,
        max_ring_lines: usize,
        max_ring_bytes: usize,
    ) -> Result<Self, std::io::Error> {
        std::fs::create_dir_all(&spool_dir)?;
        let stdout_spool = spool_dir.join(format!("job_{}_stdout.spool", job_id));
        let stderr_spool = spool_dir.join(format!("job_{}_stderr.spool", job_id));

        // Touch the spool files
        let _ = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&stdout_spool)?;
        let _ = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&stderr_spool)?;

        Ok(Self {
            job_id,
            stdout_spool,
            stderr_spool,
            stdout_ring: Mutex::new(RingBuffer::new(max_ring_lines, max_ring_bytes)),
            stderr_ring: Mutex::new(RingBuffer::new(max_ring_lines, max_ring_bytes)),
            is_finalized: Mutex::new(false),
            max_spool_bytes: Mutex::new(None),
            truncated: Mutex::new(false),
        })
    }

    /// Bound per-stream durable spool size.
    /// Chunks beyond the cap are dropped and the truncation flag is set;
    /// use [`Self::truncated`] to report it honestly.
    pub fn with_max_spool_bytes(self, cap: u64) -> Self {
        *self.max_spool_bytes.lock().unwrap() = Some(cap);
        self
    }

    /// Whether any output chunk was dropped by the spool cap.
    pub fn truncated(&self) -> bool {
        *self.truncated.lock().unwrap()
    }

    /// Append a chunk of raw output bytes to the spool and ring buffer.
    pub fn append(&self, stream: StreamType, data: &[u8]) -> Result<(), std::io::Error> {
        if *self.is_finalized.lock().unwrap() {
            return Ok(());
        }

        let spool_path = match stream {
            StreamType::Stdout => &self.stdout_spool,
            StreamType::Stderr => &self.stderr_spool,
        };

        // Enforce the per-stream durable cap. Excess bytes are dropped (never
        // buffered unboundedly) and the truncation flag is set so callers
        // report it instead of claiming complete output.
        let mut chunk = data;
        if let Some(cap) = *self.max_spool_bytes.lock().unwrap() {
            let cur = std::fs::metadata(spool_path).map(|m| m.len()).unwrap_or(0);
            if cur >= cap {
                *self.truncated.lock().unwrap() = true;
                return Ok(());
            }
            let allowed = (cap - cur) as usize;
            if chunk.len() > allowed {
                chunk = &chunk[..allowed];
                *self.truncated.lock().unwrap() = true;
            }
        }

        // Write to append-only disk spool
        let mut file = OpenOptions::new().append(true).open(spool_path)?;
        file.write_all(chunk)?;
        file.flush()?;

        // Push text lines to in-memory ring buffer (sanitizing terminal escape codes)
        let text = String::from_utf8_lossy(chunk);
        let sanitized = strip_ansi(&text);
        let mut ring = match stream {
            StreamType::Stdout => self.stdout_ring.lock().unwrap(),
            StreamType::Stderr => self.stderr_ring.lock().unwrap(),
        };

        for line in sanitized.lines() {
            ring.push_line(line.to_string());
        }

        Ok(())
    }

    /// Read an output chunk from the spool file starting at byte `offset` up to `limit` bytes.
    pub fn read_chunk(&self, offset: u64, limit: usize) -> Result<JobOutputChunk, std::io::Error> {
        let (stdout_text, next_stdout_off, stdout_eof) =
            self.read_stream_slice(&self.stdout_spool, offset, limit)?;
        let (stderr_text, _, _) = self.read_stream_slice(&self.stderr_spool, 0, limit)?;

        Ok(JobOutputChunk {
            job_id: self.job_id.to_string(),
            stdout: strip_ansi(&stdout_text),
            stderr: strip_ansi(&stderr_text),
            next_offset: next_stdout_off,
            is_eof: stdout_eof && *self.is_finalized.lock().unwrap(),
        })
    }

    fn read_stream_slice(
        &self,
        path: &PathBuf,
        offset: u64,
        limit: usize,
    ) -> Result<(String, u64, bool), std::io::Error> {
        if !path.exists() {
            return Ok((String::new(), offset, true));
        }

        let mut file = OpenOptions::new().read(true).open(path)?;
        let file_len = file.metadata()?.len();

        if offset >= file_len {
            return Ok((String::new(), file_len, true));
        }

        file.seek(SeekFrom::Start(offset))?;
        let to_read = (limit as u64).min(file_len - offset) as usize;
        let mut buf = vec![0u8; to_read];
        file.read_exact(&mut buf)?;

        let text = String::from_utf8_lossy(&buf).to_string();
        let next_offset = offset + to_read as u64;
        let is_eof = next_offset >= file_len;

        Ok((text, next_offset, is_eof))
    }

    /// Read live tail lines from the in-memory ring buffer without hitting disk.
    pub fn read_tail(&self, stream: StreamType, limit: usize) -> Vec<String> {
        match stream {
            StreamType::Stdout => self.stdout_ring.lock().unwrap().get_tail(limit),
            StreamType::Stderr => self.stderr_ring.lock().unwrap().get_tail(limit),
        }
    }

    /// Path to the append-only stdout disk spool file.
    pub fn stdout_spool_path(&self) -> &PathBuf {
        &self.stdout_spool
    }

    /// Path to the append-only stderr disk spool file.
    pub fn stderr_spool_path(&self) -> &PathBuf {
        &self.stderr_spool
    }

    /// Promote stdout and stderr disk spools atomically to `ArtifactStore` (D-16).
    pub async fn promote_to_artifact_store(
        &self,
        artifact_store: &dyn ArtifactStore,
        _mission_id: MissionId,
        _task_id: TaskId,
        _job_id: JobId,
    ) -> Result<SpoolPromotionResult, JobError> {
        {
            let mut finalized = self.is_finalized.lock().unwrap();
            *finalized = true;
        }

        let mut stdout_buf = Vec::new();
        if self.stdout_spool.exists()
            && let Ok(mut f) = OpenOptions::new().read(true).open(&self.stdout_spool)
        {
            let _ = f.read_to_end(&mut stdout_buf);
        }

        let mut stderr_buf = Vec::new();
        if self.stderr_spool.exists()
            && let Ok(mut f) = OpenOptions::new().read(true).open(&self.stderr_spool)
        {
            let _ = f.read_to_end(&mut stderr_buf);
        }

        let stdout_bytes = stdout_buf.len();
        let stderr_bytes = stderr_buf.len();

        let mut combined = stdout_buf;
        if !stderr_buf.is_empty() {
            if !combined.is_empty() {
                combined.extend_from_slice(b"\n--- STDERR ---\n");
            }
            combined.extend_from_slice(&stderr_buf);
        }

        let total_bytes = combined.len();

        let mut hasher = Sha256::new();
        hasher.update(&combined);
        let sha256_hash = format!("{:x}", hasher.finalize());

        let artifact_id = ArtifactId::new();
        artifact_store
            .store(artifact_id, &combined, "txt")
            .await
            .map_err(|e| {
                JobError::Io(format!("Failed to promote spool to artifact store: {}", e))
            })?;

        Ok(SpoolPromotionResult {
            artifact_id,
            total_bytes,
            stdout_bytes,
            stderr_bytes,
            sha256_hash,
        })
    }

    /// Finalize output on terminal job completion and promote spool file to `ArtifactStore`.
    pub async fn finalize(
        &self,
        artifact_store: Option<&Arc<dyn ArtifactStore>>,
    ) -> Result<Option<ArtifactId>, String> {
        let Some(store) = artifact_store else {
            let mut finalized = self.is_finalized.lock().unwrap();
            *finalized = true;
            return Ok(None);
        };

        let result = self
            .promote_to_artifact_store(store.as_ref(), MissionId::new(), TaskId::new(), self.job_id)
            .await
            .map_err(|e| e.to_string())?;

        Ok(Some(result.artifact_id))
    }

    /// Finalize with the REAL job ownership. Production paths MUST use this:
    /// [`finalize`](Self::finalize) fabricates identifiers and exists only
    /// for standalone/test callers without execution context.
    pub async fn finalize_for(
        &self,
        artifact_store: &dyn ArtifactStore,
        mission_id: MissionId,
        task_id: TaskId,
    ) -> Result<ArtifactId, String> {
        let result = self
            .promote_to_artifact_store(artifact_store, mission_id, task_id, self.job_id)
            .await
            .map_err(|e| e.to_string())?;
        Ok(result.artifact_id)
    }

    /// Seal spool with [INTERRUPTED] marker and promote to ArtifactStore on crash recovery (D-15).
    pub async fn seal_interrupted_and_promote(
        &self,
        artifact_store: &dyn ArtifactStore,
        mission_id: MissionId,
        task_id: TaskId,
    ) -> Result<SpoolPromotionResult, JobError> {
        let _ = self.append(StreamType::Stdout, b"\n[INTERRUPTED]\n");
        self.promote_to_artifact_store(artifact_store, mission_id, task_id, self.job_id)
            .await
    }
}

/// Standalone function to seal interrupted spool files on disk and promote to `ArtifactStore` (D-15).
pub async fn seal_and_promote_spool_paths(
    stdout_path: Option<&std::path::Path>,
    stderr_path: Option<&std::path::Path>,
    artifact_store: &dyn ArtifactStore,
) -> Result<ArtifactId, std::io::Error> {
    let mut combined = Vec::new();

    if let Some(p) = stdout_path
        && p.exists()
    {
        if let Ok(mut f) = OpenOptions::new().append(true).open(p) {
            let _ = f.write_all(b"\n[INTERRUPTED]\n");
        }
        if let Ok(mut f) = OpenOptions::new().read(true).open(p) {
            let _ = f.read_to_end(&mut combined);
        }
    }

    let mut stderr_buf = Vec::new();
    if let Some(p) = stderr_path
        && p.exists()
    {
        if let Ok(mut f) = OpenOptions::new().append(true).open(p) {
            let _ = f.write_all(b"\n[INTERRUPTED]\n");
        }
        if let Ok(mut f) = OpenOptions::new().read(true).open(p) {
            let _ = f.read_to_end(&mut stderr_buf);
        }
    }

    if !stderr_buf.is_empty() {
        if !combined.is_empty() {
            combined.extend_from_slice(b"\n--- STDERR ---\n");
        }
        combined.extend_from_slice(&stderr_buf);
    }

    if combined.is_empty() {
        combined.extend_from_slice(b"[INTERRUPTED]\n");
    }

    let artifact_id = ArtifactId::new();
    let _ = artifact_store
        .store(artifact_id, &combined, "txt")
        .await
        .map_err(|e| std::io::Error::other(e.to_string()))?;

    Ok(artifact_id)
}
