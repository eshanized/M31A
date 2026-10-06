//! Native Windows Job Object sandbox provider (SND-02, SND-03).
//!
//! Enforces:
//! - Job Object containment boundary (tree termination + memory/CPU/process count limits).
//! - Workspace directory containment validation (via platform Windows path security).
//! - Deny-by-default environment sanitization.
//! - Private temporary directory per execution.
//! - Credential protection via Windows DACL enforcement.

use async_trait::async_trait;
use tokio::process::Command;

use crate::process::env::EnvironmentBuilder;
use crate::sandbox::capabilities::SandboxCapabilities;
use crate::sandbox::plan::{SandboxError, SandboxPlan};
use crate::sandbox::provider::{SandboxExecutionHandle, SandboxProvider};

/// Native Windows sandbox provider using Job Objects and path containment.
#[derive(Debug, Default, Clone)]
pub struct WindowsSandboxProvider;

impl WindowsSandboxProvider {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl SandboxProvider for WindowsSandboxProvider {
    fn id(&self) -> &str {
        "windows-job-object"
    }

    fn capabilities(&self) -> SandboxCapabilities {
        SandboxCapabilities {
            fs_read_only_root: false,
            fs_isolated_workspace_rw: true,
            fs_private_tempdir: true,
            fs_masked_credentials: true,
            net_deny_all: false,
            net_allowlist: false,
            proc_isolated_pid_ns: false,
            proc_group_isolation: true,
            proc_cgroup_v2: false,
            proc_rlimit: true,
            user_namespaces: false,
        }
    }

    async fn prepare(&self, plan: &SandboxPlan) -> Result<SandboxExecutionHandle, SandboxError> {
        plan.validate_against(&self.capabilities())?;

        if !plan.workspace_root.exists() {
            tokio::fs::create_dir_all(&plan.workspace_root)
                .await
                .map_err(|e| SandboxError::PreparationFailed(e.to_string()))?;
        }

        let td = crate::platform::filesystem::HostFilesystem::fresh_temp_dir("m31a-win-sb-")
            .map_err(|e| SandboxError::PreparationFailed(e.to_string()))?;
        let temp_dir = Some(td.path().to_path_buf());
        std::mem::forget(td);

        let id = format!("win-job-{}", uuid::Uuid::now_v7());
        Ok(SandboxExecutionHandle::new(
            id,
            plan.workspace_root.clone(),
            temp_dir,
        ))
    }

    fn build_command(
        &self,
        plan: &SandboxPlan,
        handle: &SandboxExecutionHandle,
        program: &str,
        args: &[String],
    ) -> Result<Command, SandboxError> {
        let mut env_builder = EnvironmentBuilder::new(&plan.workspace_root);
        for (k, v) in &plan.env_vars {
            let _ = env_builder.set_var(k, v);
        }

        if let Some(ref td) = handle.temp_dir {
            let _ = env_builder.set_var("TEMP", td.to_string_lossy().to_string());
            let _ = env_builder.set_var("TMP", td.to_string_lossy().to_string());
        }

        let mut cmd = Command::new(program);
        cmd.args(args);
        env_builder.apply(&mut cmd);

        crate::platform::process::configure_isolation(&mut cmd);

        Ok(cmd)
    }

    async fn cleanup(&self, handle: &SandboxExecutionHandle) -> Result<(), SandboxError> {
        if let Some(ref td) = handle.temp_dir
            && td.exists()
        {
            let _ = tokio::fs::remove_dir_all(td).await;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_windows_provider_capabilities() {
        let provider = WindowsSandboxProvider::new();
        let caps = provider.capabilities();
        assert!(caps.proc_group_isolation);
        assert!(caps.proc_rlimit);
        assert!(caps.fs_isolated_workspace_rw);
        assert!(!caps.user_namespaces);
        assert!(!caps.net_deny_all);
    }

    #[tokio::test]
    async fn test_windows_provider_prepare_validates_plan() {
        let provider = WindowsSandboxProvider::new();
        let mut plan = SandboxPlan::new(PathBuf::from("."));
        plan.requires_net_isolation = true;
        // net isolation must fail closed on Windows Job Object provider
        assert!(provider.prepare(&plan).await.is_err());
    }
}
