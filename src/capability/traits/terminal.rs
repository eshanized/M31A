//! Terminal session service trait (CTL-01, CTL-02).

use crate::capability::error::CapabilityError;
use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Terminal session configuration for interactive process execution.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TerminalSessionConfig {
    pub command: String,
    pub args: Vec<String>,
    pub cwd: Option<PathBuf>,
    pub env: HashMap<String, String>,
    pub cols: u16,
    pub rows: u16,
}

impl Default for TerminalSessionConfig {
    fn default() -> Self {
        #[cfg(windows)]
        let command = "cmd.exe".to_string();
        #[cfg(not(windows))]
        let command = if std::path::Path::new("/bin/bash").exists() {
            "/bin/bash".to_string()
        } else {
            "/bin/sh".to_string()
        };

        Self {
            command,
            args: Vec::new(),
            cwd: None,
            env: HashMap::new(),
            cols: 80,
            rows: 24,
        }
    }
}

/// Status of an active or terminated terminal session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TerminalSessionStatus {
    pub session_id: String,
    pub is_alive: bool,
    pub exit_code: Option<i32>,
    pub cols: u16,
    pub rows: u16,
}

/// Asynchronous service seam for interactive terminal stream operations.
#[async_trait]
pub trait TerminalService: Send + Sync + 'static {
    /// Start a new interactive terminal command session.
    async fn start_session(
        &self,
        session_id: &str,
        config: TerminalSessionConfig,
    ) -> Result<(), CapabilityError>;

    /// Send input bytes to a terminal session stream.
    async fn write_input(&self, session_id: &str, input: &[u8]) -> Result<(), CapabilityError>;

    /// Read available output bytes from a terminal session stream up to timeout.
    async fn read_stream(
        &self,
        session_id: &str,
        timeout_ms: u64,
    ) -> Result<Vec<u8>, CapabilityError>;

    /// Resize terminal dimensions.
    async fn resize(&self, session_id: &str, cols: u16, rows: u16) -> Result<(), CapabilityError>;

    /// Terminate an active terminal session.
    async fn terminate_session(&self, session_id: &str) -> Result<(), CapabilityError>;

    /// Query the status of a terminal session.
    async fn session_status(
        &self,
        session_id: &str,
    ) -> Result<TerminalSessionStatus, CapabilityError>;
}
