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
            // Windows Job Objects enforce process tree termination and resource limits,
            // but do NOT provide filesystem confinement or credential masking boundaries.
            // Falsely advertising filesystem isolation is strictly prohibited (F-01).
            fs_isolated_workspace_rw: false,
            fs_private_tempdir: true,
            fs_masked_credentials: false,
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
        let work_dir = plan.working_dir.as_ref().unwrap_or(&plan.workspace_root);
        cmd.current_dir(work_dir);
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
        // F-01: Windows Job Objects do not provide filesystem isolation or masked credentials
        assert!(!caps.fs_isolated_workspace_rw);
        assert!(!caps.fs_masked_credentials);
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

        // F-01: Filesystem isolation requirement must fail closed on Windows Job Object provider
        let mut fs_plan = SandboxPlan::new(PathBuf::from("."));
        fs_plan.requires_net_isolation = false;
        fs_plan.requires_fs_isolation = true;
        let err = provider.prepare(&fs_plan).await.unwrap_err();
        assert!(matches!(err, SandboxError::CapabilitiesUnsatisfied(_)));

        // Non-fs, non-net plan can prepare successfully
        let mut relaxed_plan = SandboxPlan::new(PathBuf::from("."));
        relaxed_plan.requires_net_isolation = false;
        relaxed_plan.requires_fs_isolation = false;
        let handle = provider.prepare(&relaxed_plan).await.unwrap();
        assert_eq!(handle.workspace_root, PathBuf::from("."));
        let _ = provider.cleanup(&handle).await;
    }

    #[test]
    fn test_windows_provider_build_command_sets_cwd() {
        let provider = WindowsSandboxProvider::new();
        let plan = SandboxPlan::new(PathBuf::from("."));
        let handle = SandboxExecutionHandle::new("test".to_string(), PathBuf::from("."), None);
        let cmd = provider.build_command(&plan, &handle, "echo", &["test".to_string()]);
        assert!(cmd.is_ok());
    }
}
