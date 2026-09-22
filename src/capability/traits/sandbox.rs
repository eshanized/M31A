//! Sandbox boundary and environment service trait (CTL-01, CTL-02).

use crate::capability::error::CapabilityError;
use crate::sandbox::capabilities::SandboxCapabilities;
use crate::sandbox::plan::SandboxPlan;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Configuration for creating an isolated sandbox environment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SandboxConfig {
    pub workspace_root: PathBuf,
    pub env_vars: HashMap<String, String>,
    pub read_only: bool,
}

impl SandboxConfig {
    /// Convert configuration to a canonical SandboxPlan.
    pub fn to_sandbox_plan(&self) -> SandboxPlan {
        let mut plan = SandboxPlan::new(self.workspace_root.clone())
            .with_fs_isolation(true)
            .with_net_isolation(true);
        for (k, v) in &self.env_vars {
            plan = plan.with_env_var(k, v);
        }
        plan
    }
}

/// Handle to an active sandbox environment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SandboxHandle {
    pub id: String,
    pub workspace_root: PathBuf,
}

/// Asynchronous service seam for sandbox preparation and cleanup.
#[async_trait]
pub trait SandboxService: Send + Sync + 'static {
    /// Report explicit sandbox capabilities supported by this service.
    fn capabilities(&self) -> Option<SandboxCapabilities> {
        None
    }

    /// Prepare an isolated environment according to configuration.
    async fn prepare_environment(
        &self,
        config: &SandboxConfig,
    ) -> Result<SandboxHandle, CapabilityError>;

    /// Tear down and release sandbox resources.
    async fn cleanup_sandbox(&self, handle: &SandboxHandle) -> Result<(), CapabilityError>;
}
