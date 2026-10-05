//! Local process, shell, and background job capability providers (CTL-01, CTL-02, TL-02).

use async_trait::async_trait;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use crate::capability::error::CapabilityError;
use crate::capability::providers::LocalSandboxProvider;
use crate::capability::traits::jobs::{JobDescriptor, JobOutputChunk, JobService, JobStatusInfo};
use crate::capability::traits::process::{ProcessOutput, ProcessService};
use crate::capability::traits::shell::{ShellOutput, ShellService};
use crate::ids::{AgentId, JobId, TaskId};
use crate::process::env::EnvironmentBuilder;
use crate::process::job::JobSupervisor;
use crate::process::supervisor::{ProcessError, ProcessSupervisor};
use crate::process::tree::ProcessTreeController;
use crate::sandbox::plan::SandboxPlan;
use crate::sandbox::provider::SandboxProvider;

/// Concrete local process provider implementing `ProcessService` and `ShellService`.
///
/// AUTHORITY CONTRACT: sandbox behavior derives from the resolved
/// [`SandboxEnforcement`](crate::sandbox::SandboxEnforcement) attached via
/// [`with_enforcement`](Self::with_enforcement) (production wires the
/// deployment `sandbox_mode`). Standalone instances without an attached
/// policy default to permissive-but-network-denied; the environment variable
/// can only strengthen, never weaken, the attached policy.
pub struct LocalProcessProvider {
    supervisor: Arc<ProcessSupervisor>,
    workspace_root: PathBuf,
    sandbox: Option<Arc<dyn SandboxProvider>>,
    sandbox_required: bool,
    enforcement: Option<crate::sandbox::SandboxEnforcement>,
    network_isolated: Option<bool>,
}

impl LocalProcessProvider {
    pub fn new(workspace_root: PathBuf) -> Self {
        let sandbox_prov = Arc::new(LocalSandboxProvider::new());
        Self {
            supervisor: Arc::new(ProcessSupervisor::default()),
            workspace_root,
            sandbox: Some(sandbox_prov.provider()),
            sandbox_required: false,
            enforcement: None,
            network_isolated: None,
        }
    }

    pub fn with_supervisor(workspace_root: PathBuf, supervisor: Arc<ProcessSupervisor>) -> Self {
        let sandbox_prov = Arc::new(LocalSandboxProvider::new());
        Self {
            supervisor,
            workspace_root,
            sandbox: Some(sandbox_prov.provider()),
            sandbox_required: false,
            enforcement: None,
            network_isolated: None,
        }
    }

    pub fn with_sandbox(
        workspace_root: PathBuf,
        supervisor: Arc<ProcessSupervisor>,
        sandbox: Option<Arc<dyn SandboxProvider>>,
        sandbox_required: bool,
    ) -> Self {
        Self {
            supervisor,
            workspace_root,
            sandbox,
            sandbox_required,
            enforcement: None,
            network_isolated: None,
        }
    }

    /// Attach the runtime-authoritative sandbox enforcement policy, derived
    /// from the deployment `sandbox_mode`. Production MUST call this; the
    /// resolved policy (not hardcoding, not the environment) controls
    /// isolation and network confinement.
    pub fn with_enforcement(mut self, enforcement: crate::sandbox::SandboxEnforcement) -> Self {
        self.sandbox_required = enforcement.isolation_required;
        self.network_isolated = Some(enforcement.network_isolated);
        self.enforcement = Some(enforcement);
        self
    }

    pub fn set_sandbox_required(&mut self, required: bool) {
        self.sandbox_required = required;
    }

    pub fn is_sandbox_required(&self) -> bool {
        // Deployment policy wins; the environment can only strengthen.
        // When an enforcement policy is attached, a `false` field with an
        // unset env can never produce `true` from elsewhere, and an env
        // `true` can never turn a required policy off.
        if let Some(ref enforcement) = self.enforcement {
            return enforcement.effective_required();
        }
        self.sandbox_required || crate::sandbox::SandboxEnforcement::env_requires_sandbox()
    }

    /// Effective network isolation: policy-derived when attached, deny by
    /// default otherwise. Network access is never enabled by hardcoding.
    fn is_network_isolated(&self) -> bool {
        if let Some(ref enforcement) = self.enforcement {
            return enforcement.network_isolated;
        }
        self.network_isolated.unwrap_or(true)
    }

    pub fn sandbox_provider(&self) -> Option<Arc<dyn SandboxProvider>> {
        self.sandbox.clone()
    }
}

#[async_trait]
impl ProcessService for LocalProcessProvider {
    async fn spawn_command(
        &self,
        program: &str,
        args: &[String],
        working_dir: Option<&Path>,
        timeout_secs: u64,
    ) -> Result<ProcessOutput, CapabilityError> {
        let valid_cwd =
            crate::process::env::validate_working_directory(working_dir, &self.workspace_root)
                .map_err(|e| CapabilityError::PermissionDenied(e.to_string()))?;

        crate::process::env::check_command_safety(program, args)
            .map_err(|e| CapabilityError::PermissionDenied(e.to_string()))?;

        let timeout_dur = Duration::from_secs(timeout_secs);
        let sandbox_required = self.is_sandbox_required();

        // Capability-based provider selection: any backend reporting
        // filesystem isolation satisfies the request. The concrete
        // mechanism underneath stays an implementation detail.
        let isolated_provider = self
            .sandbox
            .as_ref()
            .filter(|p| p.capabilities().fs_isolated_workspace_rw);

        if let Some(provider) = isolated_provider {
            // Sandboxed path via Bubblewrap. Filesystem AND network isolation
            // derive from the resolved runtime enforcement policy (deny by
            // default) — never hardcoded off.
            let net_isolated = self.is_network_isolated();
            let confinement = self
                .enforcement
                .as_ref()
                .map(|e| e.network_confinement.clone())
                .unwrap_or(crate::sandbox::plan::NetworkConfinement::Isolated);
            let mut plan = SandboxPlan::new(self.workspace_root.clone())
                .with_fs_isolation(true)
                .with_net_isolation(net_isolated)
                .with_network_confinement(confinement);

            plan.working_dir = Some(valid_cwd.clone());

            let handle =
                provider
                    .prepare(&plan)
                    .await
                    .map_err(|e| CapabilityError::ExecutionFailed {
                        exit_code: None,
                        message: format!("Sandbox preparation failed: {}", e),
                    })?;

            let build_res = provider.build_command(&plan, &handle, program, args);
            let cmd_res = match build_res {
                Ok(cmd) => self.supervisor.run_supervised(cmd, timeout_dur, None).await,
                Err(e) => {
                    let _ = provider.cleanup(&handle).await;
                    return Err(CapabilityError::ExecutionFailed {
                        exit_code: None,
                        message: format!("Sandbox command construction failed: {}", e),
                    });
                }
            };

            let _ = provider.cleanup(&handle).await;

            cmd_res.map_err(|e| match e {
                ProcessError::TimedOut(dur) => {
                    CapabilityError::Timeout(format!("Command timed out after {:?}", dur))
                }
                ProcessError::Cancelled => {
                    CapabilityError::Other("Command execution cancelled".to_string())
                }
                other => CapabilityError::Io(other.to_string()),
            })
        } else if sandbox_required {
            // Fail-closed invariant: Sandboxing was required, but filesystem
            // isolation is unavailable on this host.
            Err(CapabilityError::PermissionDenied(
                "Sandbox execution is strictly required, but filesystem isolation is unavailable on this host"
                    .to_string(),
            ))
        } else {
            // Direct host execution fallback when sandboxing is NOT required and no isolation backend is available
            let env_builder = EnvironmentBuilder::new(&valid_cwd);

            self.supervisor
                .run_command(
                    program,
                    args,
                    &valid_cwd,
                    Some(&env_builder),
                    Some(timeout_dur),
                    None,
                )
                .await
                .map_err(|e| match e {
                    ProcessError::TimedOut(dur) => {
                        CapabilityError::Timeout(format!("Command timed out after {:?}", dur))
                    }
                    ProcessError::Cancelled => {
                        CapabilityError::Other("Command execution cancelled".to_string())
                    }
                    other => CapabilityError::Io(other.to_string()),
                })
        }
    }

    async fn terminate_process(&self, pid: u32) -> Result<(), CapabilityError> {
        ProcessTreeController::terminate_by_pid(pid, Duration::from_millis(1000))
            .await
            .map_err(|e| CapabilityError::Io(e.to_string()))
    }
}

#[async_trait]
impl ShellService for LocalProcessProvider {
    async fn execute_bounded_shell(
        &self,
        command: &str,
        working_dir: Option<&Path>,
        timeout_secs: u64,
    ) -> Result<ShellOutput, CapabilityError> {
        let valid_cwd =
            crate::process::env::validate_working_directory(working_dir, &self.workspace_root)
                .map_err(|e| CapabilityError::PermissionDenied(e.to_string()))?;

        // Shell interpreter resolves through the platform contract. Hosts
        // without a native interpreter fail closed here.
        let shell_program = crate::platform::shell::native_shell_program().ok_or_else(|| {
            CapabilityError::PermissionDenied(
                "shell-string execution is unsupported on this backend".to_string(),
            )
        })?;
        let args = vec!["-c".to_string(), command.to_string()];
        crate::process::env::check_command_safety(shell_program, &args)
            .map_err(|e| CapabilityError::PermissionDenied(e.to_string()))?;

        let proc_output = self
            .spawn_command(shell_program, &args, Some(&valid_cwd), timeout_secs)
            .await?;

        Ok(ShellOutput {
            exit_code: proc_output.exit_code,
            stdout: proc_output.stdout,
            stderr: proc_output.stderr,
        })
    }
}

/// Concrete local job provider implementing `JobService` backed by `JobSupervisor`.
pub struct LocalJobProvider {
    supervisor: Arc<JobSupervisor>,
    workspace_root: PathBuf,
}

impl LocalJobProvider {
    pub fn new(workspace_root: PathBuf, supervisor: Arc<JobSupervisor>) -> Self {
        Self {
            supervisor,
            workspace_root,
        }
    }

    pub fn supervisor(&self) -> &Arc<JobSupervisor> {
        &self.supervisor
    }
}

#[async_trait]
impl JobService for LocalJobProvider {
    async fn start_job(
        &self,
        command: &str,
        args: &[String],
    ) -> Result<JobDescriptor, CapabilityError> {
        // Compatibility path: no execution identity is available, so the
        // job is unattributed by construction. Production callers MUST use
        // `start_job_scoped` with the real context instead.
        crate::process::env::check_command_safety(command, args)
            .map_err(|e| CapabilityError::PermissionDenied(e.to_string()))?;

        // The supervisor runs in the provider workspace; validate containment
        // explicitly because JobSupervisor executes without an implicit directory check.
        crate::process::env::validate_working_directory(
            Some(&self.workspace_root),
            &self.workspace_root,
        )
        .map_err(|e| CapabilityError::PermissionDenied(e.to_string()))?;

        let env_builder = EnvironmentBuilder::new(&self.workspace_root);

        self.supervisor
            .start_job(
                crate::ids::MissionId::new(),
                TaskId::new(),
                AgentId::new(),
                crate::sandbox::ResourceLimits::default(),
                command,
                args,
                &self.workspace_root,
                Some(&env_builder),
                None,
            )
            .await
            .map_err(|e| CapabilityError::Io(e.to_string()))
    }

    /// Canonical scoped submission: real identity propagates and the
    /// effective resource limits persist with the durable job record.
    async fn start_job_scoped(
        &self,
        mission_id: crate::ids::MissionId,
        task_id: TaskId,
        agent_id: crate::ids::AgentId,
        resource_limits: crate::sandbox::ResourceLimits,
        command: &str,
        args: &[String],
    ) -> Result<JobDescriptor, CapabilityError> {
        crate::process::env::check_command_safety(command, args)
            .map_err(|e| CapabilityError::PermissionDenied(e.to_string()))?;

        crate::process::env::validate_working_directory(
            Some(&self.workspace_root),
            &self.workspace_root,
        )
        .map_err(|e| CapabilityError::PermissionDenied(e.to_string()))?;

        let env_builder = EnvironmentBuilder::new(&self.workspace_root);

        self.supervisor
            .start_job(
                mission_id,
                task_id,
                agent_id,
                resource_limits,
                command,
                args,
                &self.workspace_root,
                Some(&env_builder),
                None,
            )
            .await
            .map_err(|e| CapabilityError::Io(e.to_string()))
    }

    async fn get_status(&self, job_id: &str) -> Result<JobStatusInfo, CapabilityError> {
        let jid = JobId::parse(job_id)
            .map_err(|e| CapabilityError::InvalidArgument(format!("Invalid job ID: {}", e)))?;

        self.supervisor
            .job_status(&jid)
            .await
            .map_err(|e| CapabilityError::NotFound(e.to_string()))
    }

    async fn get_output(
        &self,
        job_id: &str,
        offset: u64,
        limit: usize,
    ) -> Result<JobOutputChunk, CapabilityError> {
        let jid = JobId::parse(job_id)
            .map_err(|e| CapabilityError::InvalidArgument(format!("Invalid job ID: {}", e)))?;

        self.supervisor
            .job_output(&jid, offset, limit)
            .await
            .map_err(|e| CapabilityError::Io(e.to_string()))
    }

    async fn stop_job(&self, job_id: &str) -> Result<(), CapabilityError> {
        let jid = JobId::parse(job_id)
            .map_err(|e| CapabilityError::InvalidArgument(format!("Invalid job ID: {}", e)))?;

        self.supervisor
            .job_stop(&jid, 1000)
            .await
            .map_err(|e| CapabilityError::Io(e.to_string()))
    }
}
