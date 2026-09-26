//! Process-level fallback isolation provider (SND-02, SND-03).
//!
//! Non-negotiable invariant (SND-03, D-05):
//! Explicitly declares the absence of filesystem and network isolation.
//! If a plan requires filesystem or network confinement, it strictly
//! fails closed with `CapabilitiesUnsatisfied` and never performs silent fallback.

use async_trait::async_trait;
use tokio::process::Command;

use crate::process::env::EnvironmentBuilder;
use crate::sandbox::capabilities::SandboxCapabilities;
use crate::sandbox::plan::{SandboxError, SandboxPlan};
use crate::sandbox::provider::{SandboxExecutionHandle, SandboxProvider};

/// Process-level fallback isolation provider.
#[derive(Debug, Default, Clone)]
pub struct ProcessIsolationProvider;

impl ProcessIsolationProvider {
    pub fn new() -> Self {
        Self
    }

    /// Build a configured Tokio Command ready for execution with resource limits and process group.
    pub fn build_command(
        &self,
        plan: &SandboxPlan,
        program: &str,
        args: &[String],
    ) -> Result<Command, SandboxError> {
        let mut env_builder = EnvironmentBuilder::new(&plan.workspace_root);
        for (k, v) in &plan.env_vars {
            let _ = env_builder.set_var(k, v);
        }

        let mut cmd = Command::new(program);
        cmd.args(args);
        env_builder.apply(&mut cmd);

        crate::platform::process::configure_isolation(&mut cmd);
        // Bounds enforcement comes from the platform contract.
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
}

#[async_trait]
impl SandboxProvider for ProcessIsolationProvider {
    fn id(&self) -> &str {
        "process"
    }

    fn capabilities(&self) -> SandboxCapabilities {
        SandboxCapabilities {
            fs_read_only_root: false,
            fs_isolated_workspace_rw: false,
            fs_private_tempdir: false,
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
        // Invariant (SND-03, D-05): Strict fail-closed validation against provider capabilities
        plan.validate_against(&self.capabilities())?;

        // Ensure workspace exists
        if !plan.workspace_root.exists() {
            tokio::fs::create_dir_all(&plan.workspace_root)
                .await
                .map_err(|e| SandboxError::PreparationFailed(e.to_string()))?;
        }

        let id = format!("proc-{}", uuid::Uuid::now_v7());
        Ok(SandboxExecutionHandle::new(
            id,
            plan.workspace_root.clone(),
            None,
        ))
    }

    fn build_command(
        &self,
        plan: &SandboxPlan,
        _handle: &SandboxExecutionHandle,
        program: &str,
        args: &[String],
    ) -> Result<Command, SandboxError> {
        self.build_command(plan, program, args)
    }

    async fn cleanup(&self, _handle: &SandboxExecutionHandle) -> Result<(), SandboxError> {
        Ok(())
    }
}
