//! Linux bubblewrap unprivileged namespace sandbox provider (SND-01, SND-02, D-06, D-07).

use async_trait::async_trait;
use std::path::{Path, PathBuf};
use tempfile::tempdir;
use tokio::process::Command;

use crate::sandbox::capabilities::SandboxCapabilities;
use crate::sandbox::plan::{NetworkConfinement, SandboxError, SandboxPlan};
use crate::sandbox::provider::{SandboxExecutionHandle, SandboxProvider};

/// Linux container isolation provider utilizing bubblewrap (`bwrap`).
#[derive(Debug, Clone)]
pub struct BubblewrapSandboxProvider {
    bwrap_path: PathBuf,
}

impl BubblewrapSandboxProvider {
    pub fn new(bwrap_path: PathBuf) -> Self {
        Self { bwrap_path }
    }

    /// Build bubblewrap command-line argument vector for execution.
    pub fn build_command_args(
        &self,
        plan: &SandboxPlan,
        private_tmp: &Path,
        program: &str,
        args: &[String],
    ) -> Vec<String> {
        let mut bwrap_args = Vec::new();

        // 1. Mount system toolchains and core runtime read-only (D-06)
        bwrap_args.extend([
            "--ro-bind".to_string(),
            "/usr".to_string(),
            "/usr".to_string(),
        ]);
        bwrap_args.extend([
            "--symlink".to_string(),
            "usr/bin".to_string(),
            "/bin".to_string(),
        ]);
        bwrap_args.extend([
            "--symlink".to_string(),
            "usr/sbin".to_string(),
            "/sbin".to_string(),
        ]);
        bwrap_args.extend([
            "--symlink".to_string(),
            "usr/lib".to_string(),
            "/lib".to_string(),
        ]);

        if Path::new("/lib64").exists() {
            bwrap_args.extend([
                "--symlink".to_string(),
                "usr/lib64".to_string(),
                "/lib64".to_string(),
            ]);
        }

        // Essential read-only configuration mounts (if present on host)
        if Path::new("/etc/resolv.conf").exists() {
            bwrap_args.extend([
                "--ro-bind".to_string(),
                "/etc/resolv.conf".to_string(),
                "/etc/resolv.conf".to_string(),
            ]);
        }
        if Path::new("/etc/ssl").exists() {
            bwrap_args.extend([
                "--ro-bind".to_string(),
                "/etc/ssl".to_string(),
                "/etc/ssl".to_string(),
            ]);
        }
        if Path::new("/etc/pki").exists() {
            bwrap_args.extend([
                "--ro-bind".to_string(),
                "/etc/pki".to_string(),
                "/etc/pki".to_string(),
            ]);
        }
        if Path::new("/etc/alternatives").exists() {
            bwrap_args.extend([
                "--ro-bind".to_string(),
                "/etc/alternatives".to_string(),
                "/etc/alternatives".to_string(),
            ]);
        }

        // 2. Kernel procfs and devfs mounts
        bwrap_args.extend(["--proc".to_string(), "/proc".to_string()]);
        bwrap_args.extend(["--dev".to_string(), "/dev".to_string()]);

        // 3. Private tmpfs mounted at /tmp
        bwrap_args.extend(["--tmpfs".to_string(), "/tmp".to_string()]);
        bwrap_args.extend([
            "--bind".to_string(),
            private_tmp.to_string_lossy().to_string(),
            "/tmp".to_string(),
        ]);

        // 4. Authorized workspace mount (read-write at /workspace)
        let workspace_canon = plan
            .workspace_root
            .canonicalize()
            .unwrap_or_else(|_| plan.workspace_root.clone());
        bwrap_args.extend([
            "--bind".to_string(),
            workspace_canon.to_string_lossy().to_string(),
            "/workspace".to_string(),
        ]);

        let target_chdir = if let Some(ref cwd) = plan.working_dir {
            let cwd_canon = cwd.canonicalize().unwrap_or_else(|_| cwd.clone());
            if let Ok(rel) = cwd_canon.strip_prefix(&workspace_canon) {
                if rel.as_os_str().is_empty() {
                    "/workspace".to_string()
                } else {
                    format!("/workspace/{}", rel.display())
                }
            } else {
                "/workspace".to_string()
            }
        } else {
            "/workspace".to_string()
        };

        bwrap_args.extend(["--chdir".to_string(), target_chdir]);

        // 4b. Protected runtime and repository paths reinforcement (P0 Process Boundary)
        // Ensure .git is mounted read-only so ordinary sandboxed execution cannot mutate host repository
        let host_git = workspace_canon.join(".git");
        if host_git.exists() {
            bwrap_args.extend([
                "--ro-bind".to_string(),
                host_git.to_string_lossy().to_string(),
                "/workspace/.git".to_string(),
            ]);
        }

        // Mask internal runtime state .m31a with an empty read-only directory
        let host_m31a = workspace_canon.join(".m31a");
        if host_m31a.exists() {
            let empty_m31a = private_tmp.join("empty_m31a");
            let _ = std::fs::create_dir_all(&empty_m31a);
            bwrap_args.extend([
                "--ro-bind".to_string(),
                empty_m31a.to_string_lossy().to_string(),
                "/workspace/.m31a".to_string(),
            ]);
        }

        // 5. Additional read-only mounts declared in plan (authorized only).
        // Denied targets are never mounted (validate_against already fails
        // closed at prepare time; this is defense in depth at exec time).
        for extra_ro in &plan.extra_ro_mounts {
            if crate::sandbox::plan::SandboxPlan::is_denied_extra_ro_mount(extra_ro) {
                continue;
            }
            if extra_ro.exists()
                && let Ok(canon) = extra_ro.canonicalize()
            {
                bwrap_args.extend([
                    "--ro-bind".to_string(),
                    canon.to_string_lossy().to_string(),
                    canon.to_string_lossy().to_string(),
                ]);
            }
        }

        // 6. Deny-by-default clean environment (D-15)
        bwrap_args.push("--clearenv".to_string());
        bwrap_args.extend([
            "--setenv".to_string(),
            "HOME".to_string(),
            "/workspace".to_string(),
        ]);
        bwrap_args.extend([
            "--setenv".to_string(),
            "PWD".to_string(),
            "/workspace".to_string(),
        ]);
        bwrap_args.extend([
            "--setenv".to_string(),
            "TMPDIR".to_string(),
            "/tmp".to_string(),
        ]);

        let has_path = plan.env_vars.contains_key("PATH");
        for (k, v) in &plan.env_vars {
            let upper = k.to_ascii_uppercase();
            if crate::process::env::FORBIDDEN_GIT_REDIRECT_VARS.contains(&upper.as_str()) {
                continue;
            }
            bwrap_args.extend(["--setenv".to_string(), k.clone(), v.clone()]);
        }
        if !has_path {
            bwrap_args.extend([
                "--setenv".to_string(),
                "PATH".to_string(),
                "/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin".to_string(),
            ]);
        }

        // 7. Security and namespace confinement (D-07)
        bwrap_args.push("--die-with-parent".to_string());
        bwrap_args.push("--unshare-pid".to_string());
        bwrap_args.push("--unshare-ipc".to_string());

        if plan.network_confinement == NetworkConfinement::Isolated {
            bwrap_args.push("--unshare-net".to_string());
        }

        // 8. Command execution boundary
        bwrap_args.push("--".to_string());
        bwrap_args.push(program.to_string());
        bwrap_args.extend(args.iter().cloned());

        bwrap_args
    }

    /// Build a configured Tokio Command ready for execution with resource limits.
    pub fn build_command(
        &self,
        plan: &SandboxPlan,
        handle: &SandboxExecutionHandle,
        program: &str,
        args: &[String],
    ) -> Result<Command, SandboxError> {
        let private_tmp = handle.temp_dir.as_ref().ok_or_else(|| {
            SandboxError::PreparationFailed("Handle lacks private tempdir".into())
        })?;

        let bwrap_args = self.build_command_args(plan, private_tmp, program, args);
        let mut cmd = Command::new(&self.bwrap_path);
        cmd.args(&bwrap_args);

        // Bounds enforcement comes from the platform contract; this backend
        // only wires the hook into the sandboxed command.
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
impl SandboxProvider for BubblewrapSandboxProvider {
    fn id(&self) -> &str {
        "bubblewrap"
    }

    fn capabilities(&self) -> SandboxCapabilities {
        SandboxCapabilities {
            fs_read_only_root: true,
            fs_isolated_workspace_rw: true,
            fs_private_tempdir: true,
            fs_masked_credentials: true,
            net_deny_all: true,
            net_allowlist: false,
            proc_isolated_pid_ns: true,
            proc_group_isolation: true,
            proc_cgroup_v2: false,
            proc_rlimit: true,
            user_namespaces: true,
        }
    }

    async fn prepare(&self, plan: &SandboxPlan) -> Result<SandboxExecutionHandle, SandboxError> {
        // Invariant (SND-03, D-05): Validate plan against capabilities
        plan.validate_against(&self.capabilities())?;

        // Ensure workspace directory exists
        if !plan.workspace_root.exists() {
            tokio::fs::create_dir_all(&plan.workspace_root)
                .await
                .map_err(|e| SandboxError::PreparationFailed(e.to_string()))?;
        }

        // Allocate isolated private temporary directory
        let temp_dir = tempdir().map_err(|e| SandboxError::PreparationFailed(e.to_string()))?;
        let temp_path = temp_dir.path().to_path_buf();
        std::mem::forget(temp_dir);

        let id = format!("bwrap-{}", uuid::Uuid::now_v7());
        Ok(SandboxExecutionHandle::new(
            id,
            plan.workspace_root.clone(),
            Some(temp_path),
        ))
    }

    fn build_command(
        &self,
        plan: &SandboxPlan,
        handle: &SandboxExecutionHandle,
        program: &str,
        args: &[String],
    ) -> Result<Command, SandboxError> {
        self.build_command(plan, handle, program, args)
    }

    async fn cleanup(&self, handle: &SandboxExecutionHandle) -> Result<(), SandboxError> {
        if let Some(tmp) = handle.temp_dir.as_ref().filter(|t| t.exists()) {
            let _ = tokio::fs::remove_dir_all(tmp).await;
        }
        Ok(())
    }
}
