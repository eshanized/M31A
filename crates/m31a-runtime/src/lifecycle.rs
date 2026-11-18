//! Runtime lifecycle states, transition validation, and shutdown reports.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// High-level lifecycle state of the M31A application runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeState {
    #[default]
    Initializing,
    Ready,
    ShuttingDown,
    Stopped,
    Failed,
}

impl RuntimeState {
    pub fn is_operational(&self) -> bool {
        matches!(self, RuntimeState::Ready)
    }

    pub fn is_terminal(&self) -> bool {
        matches!(self, RuntimeState::Stopped | RuntimeState::Failed)
    }
}

/// Structured summary of actions performed during runtime shutdown.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ShutdownReport {
    pub cleanly_stopped: bool,
    pub background_processes_stopped: usize,
    pub sandbox_processes_stopped: usize,
    pub active_turns_cancelled: usize,
    pub watcher_stopped: bool,
    pub mcp_servers_stopped: usize,
    pub persistent_shell_stopped: bool,
    pub timestamp: DateTime<Utc>,
}

impl Default for ShutdownReport {
    fn default() -> Self {
        Self {
            cleanly_stopped: true,
            background_processes_stopped: 0,
            sandbox_processes_stopped: 0,
            active_turns_cancelled: 0,
            watcher_stopped: false,
            mcp_servers_stopped: 0,
            persistent_shell_stopped: false,
            timestamp: Utc::now(),
        }
    }
}
