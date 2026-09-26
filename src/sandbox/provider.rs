//! Pluggable sandbox provider contract and execution lifecycle handle (SND-02, D-05).

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::sandbox::capabilities::SandboxCapabilities;
use crate::sandbox::plan::{SandboxError, SandboxPlan};

/// Active execution environment handle holding allocated resources and temp dirs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SandboxExecutionHandle {
    /// Unique execution identifier.
    pub id: String,
    /// Authorized workspace path.
    pub workspace_root: PathBuf,
    /// Private temporary directory allocated for this execution (cleaned up on drop/cleanup).
    pub temp_dir: Option<PathBuf>,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
}

impl SandboxExecutionHandle {
    pub fn new(id: impl Into<String>, workspace_root: PathBuf, temp_dir: Option<PathBuf>) -> Self {
        Self {
            id: id.into(),
            workspace_root,
            temp_dir,
            created_at: Utc::now(),
        }
    }
}

use tokio::process::Command;

/// Pluggable sandbox provider abstraction (SND-02).
#[async_trait]
pub trait SandboxProvider: Send + Sync + 'static {
    /// Unique provider identifier (e.g. "bubblewrap", "process").
    fn id(&self) -> &str;

    /// Explicit capabilities supported by this provider on the current platform.
    fn capabilities(&self) -> SandboxCapabilities;

    /// Prepare isolated execution environment according to plan.
    ///
    /// Must validate plan against capabilities and fail closed if unsatisfied.
    async fn prepare(&self, plan: &SandboxPlan) -> Result<SandboxExecutionHandle, SandboxError>;

    /// Build a configured Tokio Command ready for execution within the sandbox environment.
    fn build_command(
        &self,
        plan: &SandboxPlan,
        handle: &SandboxExecutionHandle,
        program: &str,
        args: &[String],
    ) -> Result<Command, SandboxError>;

    /// Release allocated sandbox resources, temporary directories, and mounts.
    async fn cleanup(&self, handle: &SandboxExecutionHandle) -> Result<(), SandboxError>;
}
