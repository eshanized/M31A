//! Supervised process execution service trait (CTL-01, CTL-02).

use crate::capability::error::CapabilityError;
use async_trait::async_trait;
use std::path::Path;

pub use crate::process::ProcessOutput;

/// Asynchronous service seam for supervised process spawning and termination.
#[async_trait]
pub trait ProcessService: Send + Sync + 'static {
    /// Spawn an isolated executable with argument array, bounds, and timeout.
    async fn spawn_command(
        &self,
        program: &str,
        args: &[String],
        working_dir: Option<&Path>,
        timeout_secs: u64,
    ) -> Result<ProcessOutput, CapabilityError>;

    /// Terminate a running process by PID.
    async fn terminate_process(&self, pid: u32) -> Result<(), CapabilityError>;
}
