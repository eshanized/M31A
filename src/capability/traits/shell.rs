//! Bounded shell execution service trait (CTL-01, CTL-02).

use crate::capability::error::CapabilityError;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Result of executing a shell command.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShellOutput {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

/// Asynchronous service seam for executing bounded shell commands.
#[async_trait]
pub trait ShellService: Send + Sync + 'static {
    /// Execute a shell script or pipeline under a strict timeout.
    async fn execute_bounded_shell(
        &self,
        command: &str,
        working_dir: Option<&Path>,
        timeout_secs: u64,
    ) -> Result<ShellOutput, CapabilityError>;
}
