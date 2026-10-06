//! Native macOS Seatbelt sandbox provider (SND-02, SND-03).
//!
//! Enforces:
//! - Workspace directory write isolation via `sandbox-exec` Seatbelt profiles.
//! - Network denial egress boundary when configured.
//! - Process group isolation and POSIX resource limits via platform contract.
//! - Deny-by-default environment sanitization.
//! - Private temporary directory per execution.

use async_trait::async_trait;
use tokio::process::Command;

use crate::platform::macos::{find_sandbox_exec, seatbelt_profile_for_workspace};
use crate::process::env::EnvironmentBuilder;
use crate::sandbox::capabilities::SandboxCapabilities;
use crate::sandbox::plan::{SandboxError, SandboxPlan};
use crate::sandbox::provider::{SandboxExecutionHandle, SandboxProvider};

/// Native macOS Seatbelt sandbox provider.
#[derive(Debug, Default, Clone)]
pub struct SeatbeltSandboxProvider;

impl SeatbeltSandboxProvider {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl SandboxProvider for SeatbeltSandboxProvider {
    fn id(&self) -> &str {
        "macos-seatbelt"
    }

    fn capabilities(&self) -> SandboxCapabilities {
        SandboxCapabilities {
            fs_read_only_root: false,
            fs_isolated_workspace_rw: true,
            fs_private_tempdir: true,
            fs_masked_credentials: false,
            net_deny_all: true,
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

        let td = crate::platform::filesystem::HostFilesystem::fresh_temp_dir("m31a-mac-sb-")
            .map_err(|e| SandboxError::PreparationFailed(e.to_string()))?;
        let temp_dir = Some(td.path().to_path_buf());
        std::mem::forget(td);

        let id = format!("mac-seatbelt-{}", uuid::Uuid::now_v7());
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
        let sandbox_bin = find_sandbox_exec().ok_or_else(|| {
            SandboxError::PreparationFailed("sandbox-exec binary not found on host".to_string())
        })?;

        let temp_root = handle
            .temp_dir
            .as_deref()
            .unwrap_or_else(|| std::path::Path::new("/tmp"));
        let profile = seatbelt_profile_for_workspace(&plan.workspace_root, temp_root);

        let mut cmd = Command::new(sandbox_bin);
        cmd.arg("-p");
        cmd.arg(profile);
        cmd.arg(program);
        cmd.args(args);
        cmd.current_dir(&plan.workspace_root);

        let mut env_builder = EnvironmentBuilder::new(&plan.workspace_root);
        for (k, v) in &plan.env_vars {
            let _ = env_builder.set_var(k, v);
        }

        if let Some(ref td) = handle.temp_dir {
            let _ = env_builder.set_var("TMPDIR", td.to_string_lossy().to_string());
        }

        env_builder.apply(&mut cmd);
        crate::platform::process::configure_isolation(&mut cmd);

        let limits = plan.resource_limits.clone();
        let budget = crate::platform::resources::ResourceBudget {
            max_cpu_seconds: limits.cpu_time_secs,
            max_memory_bytes: limits.memory_bytes,
            max_processes: limits.max_processes,
            max_open_files: limits.max_open_files,
            max_output_bytes: Some(limits.max_output_bytes),
        };
        crate::platform::resources::install_pre_exec_hook(&mut cmd, budget);

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
    fn test_seatbelt_provider_capabilities() {
        let provider = SeatbeltSandboxProvider::new();
        let caps = provider.capabilities();
        assert!(caps.fs_isolated_workspace_rw);
        assert!(caps.net_deny_all);
        assert!(caps.proc_group_isolation);
        assert!(!caps.user_namespaces);
    }

    #[tokio::test]
    async fn test_seatbelt_provider_prepare_validates_plan() {
        let provider = SeatbeltSandboxProvider::new();
        let mut plan = SandboxPlan::new(PathBuf::from("."));
        plan.network_confinement = crate::sandbox::plan::NetworkConfinement::Allowlist {
            domains: vec!["example.com".into()],
        };
        // network allowlist must fail closed on Seatbelt provider
        assert!(provider.prepare(&plan).await.is_err());
    }
}
