//! Structured Execution Audit Event Models.
//!
//! Captures kernel-level execution provenance, process lifecycle events,
//! filesystem accesses, network connections, and buffer overflow tracking.

use serde::{Deserialize, Serialize};

/// Categorized type of an observed kernel audit event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AuditEventKind {
    /// Process spawned via `sys_enter_execve`
    ProcessExec {
        pid: u32,
        ppid: u32,
        uid: u32,
        binary_path: String,
        args: Vec<String>,
    },
    /// Process completed via `sched_process_exit`
    ProcessExit {
        pid: u32,
        ppid: u32,
        exit_code: i32,
        duration_ms: u64,
    },
    /// File opened via `sys_enter_openat`
    FileOpen {
        pid: u32,
        path: String,
        flags: u32,
        read_only: bool,
    },
    /// Outbound network connection attempted via `sys_enter_connect`
    NetworkConnect {
        pid: u32,
        destination_ip: String,
        port: u16,
        protocol: String,
    },
    /// Ring buffer overflow occurred; audit events were lost due to backpressure
    RingBufferOverflow {
        lost_events_count: u64,
        buffer_capacity: usize,
    },
}

/// A timestamped, correlated audit record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditRecord {
    pub event_id: String,
    pub timestamp_ns: u64,
    pub turn_id: Option<String>,
    pub sandbox_id: Option<String>,
    pub kind: AuditEventKind,
}

impl AuditRecord {
    pub fn new(turn_id: Option<String>, sandbox_id: Option<String>, kind: AuditEventKind) -> Self {
        let now_ns = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;

        Self {
            event_id: uuid::Uuid::new_v4().to_string(),
            timestamp_ns: now_ns,
            turn_id,
            sandbox_id,
            kind,
        }
    }

    pub fn pid(&self) -> Option<u32> {
        match &self.kind {
            AuditEventKind::ProcessExec { pid, .. } => Some(*pid),
            AuditEventKind::ProcessExit { pid, .. } => Some(*pid),
            AuditEventKind::FileOpen { pid, .. } => Some(*pid),
            AuditEventKind::NetworkConnect { pid, .. } => Some(*pid),
            AuditEventKind::RingBufferOverflow { .. } => None,
        }
    }
}
