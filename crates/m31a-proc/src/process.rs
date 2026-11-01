//! Process handle representation and asynchronous I/O reader.
//!
//! Manages child process lifecycles, exit code propagation, and stdout/stderr line streaming.

use crate::buffer::{CircularOutputBuffer, ProcessLine};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "details")]
pub enum ProcessStatus {
    Running { pid: u32 },
    Exited { exit_code: i32 },
    Terminated { signal: i32 },
    Failed { error: String },
}

impl ProcessStatus {
    pub fn is_running(&self) -> bool {
        matches!(self, ProcessStatus::Running { .. })
    }

    pub fn display_badge(&self) -> (&'static str, &'static str) {
        match self {
            ProcessStatus::Running { .. } => ("RUNNING", "green"),
            ProcessStatus::Exited { exit_code: 0 } => ("EXITED(0)", "cyan"),
            ProcessStatus::Exited { .. } => ("EXITED(ERR)", "red"),
            ProcessStatus::Terminated { .. } => ("TERMINATED", "yellow"),
            ProcessStatus::Failed { .. } => ("FAILED", "red"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessInfo {
    pub id: String,
    pub label: String,
    pub command: String,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    pub pid: Option<u32>,
    pub status: ProcessStatus,
    pub started_at: DateTime<Utc>,
    pub uptime_seconds: u64,
    pub total_lines_logged: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessOutput {
    pub process_id: String,
    pub label: String,
    pub is_running: bool,
    pub offset: usize,
    pub total_lines: usize,
    pub lines: Vec<ProcessLine>,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessStopResult {
    pub id: String,
    pub label: String,
    pub stopped: bool,
    pub message: String,
}

pub struct ManagedProcess {
    pub id: String,
    pub label: String,
    pub command: String,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    pub pid: Option<u32>,
    pub started_at: DateTime<Utc>,
    pub status: Arc<RwLock<ProcessStatus>>,
    pub buffer: Arc<RwLock<CircularOutputBuffer>>,
    pub cancel_tx: Option<tokio::sync::oneshot::Sender<()>>,
}

impl ManagedProcess {
    pub async fn to_info(&self) -> ProcessInfo {
        let status = self.status.read().await.clone();
        let buf = self.buffer.read().await;
        let now = Utc::now();
        let uptime_seconds = now
            .signed_duration_since(self.started_at)
            .num_seconds()
            .max(0) as u64;

        ProcessInfo {
            id: self.id.clone(),
            label: self.label.clone(),
            command: self.command.clone(),
            args: self.args.clone(),
            cwd: self.cwd.clone(),
            pid: self.pid,
            status,
            started_at: self.started_at,
            uptime_seconds,
            total_lines_logged: buf.total_lines(),
        }
    }
}
