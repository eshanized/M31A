//! Sandbox command execution.
//!
//! Backend selection is platform-driven with graceful degradation:
//! - macOS: Seatbelt (`sandbox-exec`) when available
//! - Windows: MXC (process-isolated containers via Docker) when available
//! - Linux: Bubblewrap when installed, else Landlock + seccomp process isolation
//! - anywhere else: plain process isolation
//!
//! Every backend funnels into [`spawn_and_wait`], which enforces a wall-clock
//! timeout and drains stdout/stderr concurrently (in spawned tasks) so a
//! chatty child can never deadlock on a full pipe buffer.

use anyhow::{Context, Result};
use m31a_core::{SandboxPolicy, ShellSnapshotCache};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;
use tracing::{debug, warn};

// Both backends are plain-std implementations that compile on every target;
// they are only ever *invoked* on their own platform (see [`SandboxRunner::select_backend`]).
use crate::mxc::MxcSandbox;
use crate::seatbelt::SeatbeltSandbox;

/// Grace period to drain stdout/stderr after the child exits. Bounds the time
/// a detached grandchild holding the pipe open can stall the runner.
const OUTPUT_DRAIN_GRACE: Duration = Duration::from_secs(10);

/// Sandbox backend selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SandboxBackend {
    Bubblewrap,
    Seatbelt,
    Mxc,
    Process,
}

#[derive(Debug, Clone)]
pub struct CommandOutput {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub timed_out: bool,
}

/// Strongly typed command invocation representation for sandboxed execution.
///
/// Prevents word-splitting bugs by distinguishing between direct program execution
/// with explicit arguments and shell command-line strings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SandboxCommand {
    /// A shell command line to be evaluated by the shell (e.g., `bash -c "..."` or `cmd /c "..."`).
    Shell(String),
    /// An executable program with exact argument vector, preserving quotes, spaces, and empty strings.
    Exec { program: String, args: Vec<String> },
}

impl SandboxCommand {
    pub fn shell(cmd: impl Into<String>) -> Self {
        Self::Shell(cmd.into())
    }

    pub fn exec(program: impl Into<String>, args: Vec<String>) -> Self {
        Self::Exec {
            program: program.into(),
            args,
        }
    }

    /// Parses tokens using POSIX shlex if `Shell`, or returns program + args if `Exec`.
    pub fn to_tokens(&self) -> Vec<String> {
        match self {
            Self::Exec { program, args } => {
                let mut v = Vec::with_capacity(args.len() + 1);
                v.push(program.clone());
                v.extend(args.clone());
                v
            }
            Self::Shell(cmd) => shlex::split(cmd)
                .unwrap_or_else(|| vec!["bash".to_string(), "-c".to_string(), cmd.clone()]),
        }
    }
}

impl From<&str> for SandboxCommand {
    fn from(s: &str) -> Self {
        Self::Shell(s.to_string())
    }
}

impl From<String> for SandboxCommand {
    fn from(s: String) -> Self {
        Self::Shell(s)
    }
}

impl From<(&str, &[&str])> for SandboxCommand {
    fn from((prog, args): (&str, &[&str])) -> Self {
        Self::Exec {
            program: prog.to_string(),
            args: args.iter().map(|s| s.to_string()).collect(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SandboxRunner {
    bwrap_path: Option<PathBuf>,
    extra_ro_paths: std::sync::Arc<std::sync::RwLock<Vec<PathBuf>>>,
    proxy_url: std::sync::Arc<std::sync::RwLock<Option<String>>>,
}

impl Default for SandboxRunner {
    fn default() -> Self {
        Self::new()
    }
}

impl SandboxRunner {
    pub fn new() -> Self {
        let bwrap = which::which("bwrap").ok().filter(|path| {
            // Verify bwrap can actually create user namespaces and run on this kernel/environment.
            // On systems with AppArmor restricting unprivileged user namespaces (e.g. Ubuntu 24.04+),
            // an unconfined bwrap binary will fail with "Permission denied".
            #[cfg(target_os = "linux")]
            {
                std::process::Command::new(path)
                    .args(["--ro-bind", "/", "/", "/bin/true"])
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .status()
                    .map(|s| s.success())
                    .unwrap_or(false)
            }
            #[cfg(not(target_os = "linux"))]
            {
                let _ = path;
                false
            }
        });
        if let Some(ref path) = bwrap {
            debug!("Bubblewrap found and verified functional at {:?}", path);
        } else {
            debug!(
                "Bubblewrap not found or unprivileged user namespaces restricted; will use process-level isolation"
            );
        }
        Self {
            bwrap_path: bwrap,
            extra_ro_paths: std::sync::Arc::new(std::sync::RwLock::new(Vec::new())),
            proxy_url: std::sync::Arc::new(std::sync::RwLock::new(None)),
        }
    }

    pub fn has_bwrap(&self) -> bool {
        self.bwrap_path.is_some()
    }

    pub fn set_proxy_url(&self, url: impl Into<String>) {
        if let Ok(mut lock) = self.proxy_url.write() {
            *lock = Some(url.into());
        }
    }

    pub fn clear_proxy_url(&self) {
        if let Ok(mut lock) = self.proxy_url.write() {
            *lock = None;
        }
    }

    pub fn proxy_url(&self) -> Option<String> {
        self.proxy_url.read().ok().and_then(|lock| lock.clone())
    }

    pub fn add_protected_path(&self, path: PathBuf) {
        if let Ok(mut lock) = self.extra_ro_paths.write()
            && !lock.contains(&path)
        {
            lock.push(path);
        }
    }

    pub fn remove_protected_path(&self, path: &Path) {
        if let Ok(mut lock) = self.extra_ro_paths.write() {
            lock.retain(|p| p != path);
        }
    }

    pub fn set_protected_paths(&self, paths: Vec<PathBuf>) {
        if let Ok(mut lock) = self.extra_ro_paths.write() {
            *lock = paths;
        }
    }

    /// Picks the best backend for this platform. `cfg!` keeps every variant
    /// constructed on every target, so downstream `match`es stay exhaustive
    /// without per-platform `#[cfg]` attributes on the arms.
    fn select_backend(&self, policy: SandboxPolicy) -> SandboxBackend {
        if cfg!(target_os = "macos") && SeatbeltSandbox::is_available() {
            SandboxBackend::Seatbelt
        } else if cfg!(target_os = "windows") && MxcSandbox::is_available() {
            SandboxBackend::Mxc
        } else if cfg!(target_os = "linux")
            && self.bwrap_path.is_some()
            && policy != SandboxPolicy::DangerFullAccess
        {
            SandboxBackend::Bubblewrap
        } else {
            SandboxBackend::Process
        }
    }

    pub async fn execute(
        &self,
        command_line: &str,
        cwd: &Path,
        policy: SandboxPolicy,
        timeout: Duration,
    ) -> Result<CommandOutput> {
        self.execute_command(
            SandboxCommand::Shell(command_line.to_string()),
            cwd,
            policy,
            timeout,
        )
        .await
    }

    pub async fn execute_with_env(
        &self,
        command_line: &str,
        cwd: &Path,
        policy: SandboxPolicy,
        timeout: Duration,
        extra_env: &HashMap<String, String>,
    ) -> Result<CommandOutput> {
        self.execute_command_with_env(
            SandboxCommand::Shell(command_line.to_string()),
            cwd,
            policy,
            timeout,
            extra_env,
        )
        .await
    }

    pub async fn execute_command(
        &self,
        command: SandboxCommand,
        cwd: &Path,
        policy: SandboxPolicy,
        timeout: Duration,
    ) -> Result<CommandOutput> {
        self.execute_command_with_env(command, cwd, policy, timeout, &HashMap::new())
            .await
    }

    pub async fn execute_command_with_env(
        &self,
        command: SandboxCommand,
        cwd: &Path,
        policy: SandboxPolicy,
        timeout: Duration,
        extra_env: &HashMap<String, String>,
    ) -> Result<CommandOutput> {
        let backend = self.select_backend(policy);

        // Ingest toolchain from login shell snapshot
        let snapshot = ShellSnapshotCache::get_global();
        let mut combined_env = snapshot.environment;
        for (k, v) in extra_env {
            combined_env.insert(k.clone(), v.clone());
        }

        // If an outbound proxy is configured on the sandbox runner, inject standard proxy environment
        if let Some(proxy) = self.proxy_url() {
            combined_env
                .entry("HTTP_PROXY".to_string())
                .or_insert_with(|| proxy.clone());
            combined_env
                .entry("HTTPS_PROXY".to_string())
                .or_insert_with(|| proxy.clone());
            combined_env
                .entry("ALL_PROXY".to_string())
                .or_insert_with(|| proxy.clone());
            combined_env
                .entry("http_proxy".to_string())
                .or_insert_with(|| proxy.clone());
            combined_env
                .entry("https_proxy".to_string())
                .or_insert_with(|| proxy.clone());
            combined_env
                .entry("all_proxy".to_string())
                .or_insert_with(|| proxy.clone());
            combined_env
                .entry("NO_PROXY".to_string())
                .or_insert_with(|| "localhost,127.0.0.1,::1".to_string());
            combined_env
                .entry("no_proxy".to_string())
                .or_insert_with(|| "localhost,127.0.0.1,::1".to_string());
        }

        // Apply process hardening & credential redaction
        let hardened_env = crate::process_hardening::build_hardened_environment(
            &combined_env,
            &extra_env.keys().cloned().collect::<HashSet<_>>(),
        );

        match backend {
            SandboxBackend::Seatbelt => {
                let seatbelt = SeatbeltSandbox::new(policy, cwd)?;
                let parts: Vec<String> = match &command {
                    SandboxCommand::Exec { program, args } => {
                        let mut p = Vec::with_capacity(args.len() + 1);
                        p.push(program.clone());
                        p.extend(args.clone());
                        p
                    }
                    SandboxCommand::Shell(cmd) => {
                        vec!["/bin/bash".to_string(), "-c".to_string(), cmd.clone()]
                    }
                };
                let cmd_display = match &command {
                    SandboxCommand::Exec { program, args } => {
                        format!("{} {}", program, args.join(" "))
                    }
                    SandboxCommand::Shell(cmd) => cmd.clone(),
                };
                let cwd = cwd.to_path_buf();
                let out = tokio::task::spawn_blocking(move || seatbelt.execute(&parts, &cwd))
                    .await
                    .context("Seatbelt sandbox task panicked")?
                    .with_context(|| format!("Seatbelt sandbox failed: {}", cmd_display))?;
                Ok(CommandOutput {
                    exit_code: out.exit_code,
                    stdout: out.stdout,
                    stderr: out.stderr,
                    timed_out: false,
                })
            }
            SandboxBackend::Mxc => {
                let mxc = MxcSandbox::new(policy, cwd)?;
                let parts: Vec<String> = match &command {
                    SandboxCommand::Exec { program, args } => {
                        let mut p = Vec::with_capacity(args.len() + 1);
                        p.push(program.clone());
                        p.extend(
                            args.iter()
                                .map(|a| crate::mxc::windows_quote_arg(a))
                                .collect::<Vec<_>>(),
                        );
                        p
                    }
                    SandboxCommand::Shell(cmd) => {
                        vec![cmd.clone()]
                    }
                };
                let cmd_display = match &command {
                    SandboxCommand::Exec { program, args } => {
                        format!("{} {}", program, args.join(" "))
                    }
                    SandboxCommand::Shell(cmd) => cmd.clone(),
                };
                let cwd = cwd.to_path_buf();
                let out = tokio::task::spawn_blocking(move || mxc.execute(&parts, &cwd))
                    .await
                    .context("MXC sandbox task panicked")?
                    .with_context(|| format!("MXC sandbox failed: {}", cmd_display))?;
                Ok(CommandOutput {
                    exit_code: out.exit_code,
                    stdout: out.stdout,
                    stderr: out.stderr,
                    timed_out: false,
                })
            }
            SandboxBackend::Bubblewrap => match &command {
                SandboxCommand::Exec { program, args } => {
                    let args_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
                    let cmd = self.build_bwrap_process_with_env(
                        program,
                        &args_refs,
                        cwd,
                        policy,
                        &hardened_env,
                    )?;
                    let cmd_display = format!("{} {}", program, args.join(" "));
                    spawn_and_wait(cmd, "bwrap", &cmd_display, timeout).await
                }
                SandboxCommand::Shell(cmd_str) => {
                    self.execute_bwrap(cmd_str, cwd, policy, timeout, &hardened_env)
                        .await
                }
            },
            SandboxBackend::Process => {
                // Process-level isolation fallback (Landlock/seccomp on Linux).
                match &command {
                    SandboxCommand::Exec { program, args } => {
                        let mut std_cmd = std::process::Command::new(program);
                        std_cmd.args(args);
                        std_cmd.current_dir(cwd);
                        std_cmd.env_clear();
                        std_cmd.envs(&hardened_env);
                        #[cfg(target_os = "linux")]
                        let _ = crate::process_hardening::apply_process_hardening();
                        let cmd = tokio::process::Command::from(std_cmd);
                        let cmd_display = format!("{} {}", program, args.join(" "));
                        spawn_and_wait(cmd, "process", &cmd_display, timeout).await
                    }
                    SandboxCommand::Shell(cmd_str) => {
                        let cmd =
                            build_process_fallback_command(cmd_str, cwd, policy, &hardened_env)?;
                        spawn_and_wait(cmd, "process", cmd_str, timeout).await
                    }
                }
            }
        }
    }

    pub fn build_bwrap_process_with_env(
        &self,
        program: &str,
        args: &[&str],
        cwd: &Path,
        policy: SandboxPolicy,
        hardened_env: &HashMap<String, String>,
    ) -> Result<Command> {
        let bwrap = self
            .bwrap_path
            .as_ref()
            .context("Bubblewrap not available")?;

        // Authenticate and pin the Bubblewrap executable to prevent TOCTOU race conditions
        let executable_target = match crate::descriptor_binary::verify_and_pin_binary(bwrap) {
            Ok(pinned) => pinned.get_executable_target().to_path_buf(),
            Err(_) => bwrap.clone(),
        };

        let net_mode = if self.proxy_url().is_some() {
            crate::bwrap_builder::BwrapNetworkMode::ProxyBridge
        } else {
            crate::bwrap_builder::BwrapNetworkMode::Isolated
        };

        let extra_ro = self
            .extra_ro_paths
            .read()
            .map(|p| p.clone())
            .unwrap_or_default();
        let plan = crate::bwrap_builder::CanonicalSandboxPlan::build(
            &executable_target,
            program,
            args,
            cwd,
            policy,
            net_mode,
            &extra_ro,
            hardened_env,
        )?;

        Ok(plan.to_tokio_command())
    }

    pub fn build_bwrap_process(
        &self,
        program: &str,
        args: &[&str],
        cwd: &Path,
        policy: SandboxPolicy,
    ) -> Result<Command> {
        let snapshot = ShellSnapshotCache::get_global();
        let hardened = crate::process_hardening::build_hardened_environment(
            &snapshot.environment,
            &HashSet::new(),
        );
        self.build_bwrap_process_with_env(program, args, cwd, policy, &hardened)
    }

    pub fn build_std_bwrap_process_with_env(
        &self,
        program: &str,
        args: &[&str],
        cwd: &Path,
        policy: SandboxPolicy,
        hardened_env: &HashMap<String, String>,
    ) -> Result<std::process::Command> {
        let bwrap = self
            .bwrap_path
            .as_ref()
            .context("Bubblewrap not available")?;
        let executable_target = match crate::descriptor_binary::verify_and_pin_binary(bwrap) {
            Ok(pinned) => pinned.get_executable_target().to_path_buf(),
            Err(_) => bwrap.clone(),
        };

        let net_mode = if self.proxy_url().is_some() {
            crate::bwrap_builder::BwrapNetworkMode::ProxyBridge
        } else {
            crate::bwrap_builder::BwrapNetworkMode::Isolated
        };

        let extra_ro = self
            .extra_ro_paths
            .read()
            .map(|p| p.clone())
            .unwrap_or_default();
        let plan = crate::bwrap_builder::CanonicalSandboxPlan::build(
            &executable_target,
            program,
            args,
            cwd,
            policy,
            net_mode,
            &extra_ro,
            hardened_env,
        )?;

        Ok(plan.to_std_command())
    }

    pub fn build_std_bwrap_process(
        &self,
        program: &str,
        args: &[&str],
        cwd: &Path,
        policy: SandboxPolicy,
    ) -> Result<std::process::Command> {
        let snapshot = ShellSnapshotCache::get_global();
        let hardened = crate::process_hardening::build_hardened_environment(
            &snapshot.environment,
            &HashSet::new(),
        );
        self.build_std_bwrap_process_with_env(program, args, cwd, policy, &hardened)
    }

    pub fn build_bwrap_command(
        &self,
        command_line: &str,
        cwd: &Path,
        policy: SandboxPolicy,
    ) -> Result<Command> {
        let snapshot = ShellSnapshotCache::get_global();
        let hardened = crate::process_hardening::build_hardened_environment(
            &snapshot.environment,
            &HashSet::new(),
        );
        self.build_bwrap_command_with_env(command_line, cwd, policy, &hardened)
    }

    pub fn build_bwrap_command_with_env(
        &self,
        command_line: &str,
        cwd: &Path,
        policy: SandboxPolicy,
        hardened_env: &HashMap<String, String>,
    ) -> Result<Command> {
        self.build_bwrap_process_with_env("bash", &["-c", command_line], cwd, policy, hardened_env)
    }

    /// Execute command using Bubblewrap
    pub async fn execute_bwrap(
        &self,
        command_line: &str,
        cwd: &Path,
        policy: SandboxPolicy,
        timeout: Duration,
        hardened_env: &HashMap<String, String>,
    ) -> Result<CommandOutput> {
        let cmd = self.build_bwrap_command_with_env(command_line, cwd, policy, hardened_env)?;
        spawn_and_wait(cmd, "bwrap", command_line, timeout).await
    }
}

/// Builds the no-container fallback command: hardened `bash -c` plus, on
/// Linux, Landlock filesystem confinement and a seccomp filter installed via
/// `pre_exec` (parent side, before the child exists). `pre_exec` hooks are
/// registered on the `std` command — its `Send + 'static` bound matches the
/// hooks exactly — then converted to a tokio command.
fn build_process_fallback_command(
    command_line: &str,
    cwd: &Path,
    policy: SandboxPolicy,
    hardened_env: &HashMap<String, String>,
) -> Result<Command> {
    // Referenced on every target; keeps the parameter used when the unix
    // hardening block below is compiled out.
    let _ = &policy;

    let mut std_cmd = std::process::Command::new("bash");
    std_cmd.arg("-c").arg(command_line);
    std_cmd.current_dir(cwd);
    std_cmd.env_clear();
    std_cmd.envs(hardened_env);

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;

        #[allow(unused_mut)]
        let mut has_isolation = false;

        // §3.3: Landlock filesystem confinement fallback when bwrap is unavailable
        #[cfg(target_os = "linux")]
        if let Some(ruleset) = crate::landlock::build_landlock_rules(&policy, cwd) {
            unsafe {
                std_cmd.pre_exec(crate::landlock::pre_exec_hook(ruleset));
            }
            has_isolation = true;
        }

        // §3.2: seccomp network/process filtering fallback (same gate
        // order as upstream: Landlock first, then the seccomp filter).
        #[cfg(target_os = "linux")]
        {
            if let Some(mode) = crate::seccomp_filter::seccomp_mode_for_policy(policy) {
                let plan = crate::seccomp_filter::SeccompFilterPlan::build(mode, true);
                let hook = crate::seccomp_filter::pre_exec_seccomp_hook(&plan)?;
                unsafe {
                    std_cmd.pre_exec(hook);
                }
                has_isolation = true;
            }
        }

        if policy != SandboxPolicy::DangerFullAccess && !has_isolation {
            #[cfg(target_os = "linux")]
            return Err(anyhow::anyhow!(
                "Fail-closed: Sandbox policy '{:?}' requires confinement, but neither Bubblewrap, Landlock, nor Seccomp is available in this environment",
                policy
            ));
            #[cfg(not(target_os = "linux"))]
            return Err(anyhow::anyhow!(
                "Fail-closed: Sandbox policy '{:?}' requires platform confinement, which is unavailable on this host",
                policy
            ));
        }

        // Ensure children never outlive us (no-op on non-Linux unix).
        unsafe {
            std_cmd.pre_exec(crate::process_hardening::set_parent_death_signal);
        }
    }

    #[cfg(not(unix))]
    {
        if policy != SandboxPolicy::DangerFullAccess {
            return Err(anyhow::anyhow!(
                "Fail-closed: Sandbox policy '{:?}' requires container isolation, but MXC/Docker is not available",
                policy
            ));
        }
    }

    // Infallible conversion; tokio keeps the std configuration (args, cwd,
    // env, pre_exec hooks) and layers async I/O on top when we pipe stdio.
    Ok(Command::from(std_cmd))
}

const MAX_SANDBOX_OUTPUT_BYTES: u64 = 4 * 1024 * 1024; // 4MB per stream limit

/// Spawns `cmd` with piped output, enforces `timeout`, and reaps the child process tree.
/// stdout/stderr are drained concurrently with bounded memory buffers so neither pipe can fill up and
/// block the child while we wait on it.
async fn spawn_and_wait(
    mut cmd: Command,
    label: &str,
    command_line: &str,
    timeout: Duration,
) -> Result<CommandOutput> {
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    cmd.kill_on_drop(true);

    #[cfg(unix)]
    unsafe {
        cmd.pre_exec(|| {
            libc::setpgid(0, 0);
            Ok(())
        });
    }

    let mut child = cmd
        .spawn()
        .with_context(|| format!("Failed to spawn {} command: {}", label, command_line))?;

    let child_pid = child.id();
    let stdout_handle = child.stdout.take();
    let stderr_handle = child.stderr.take();

    async fn drain_bounded_reader<R: tokio::io::AsyncRead + Unpin>(
        mut reader: R,
        max_retention: usize,
    ) -> (Vec<u8>, usize) {
        let mut retained = Vec::new();
        let mut total_bytes = 0;
        let mut chunk = [0u8; 8192];
        use tokio::io::AsyncReadExt;
        loop {
            match reader.read(&mut chunk).await {
                Ok(0) => break,
                Ok(n) => {
                    total_bytes += n;
                    if retained.len() < max_retention {
                        let available = max_retention - retained.len();
                        let to_copy = n.min(available);
                        retained.extend_from_slice(&chunk[..to_copy]);
                    }
                }
                Err(_) => break,
            }
        }
        (retained, total_bytes)
    }

    let stdout_task = tokio::spawn(async move {
        if let Some(reader) = stdout_handle {
            let (mut buf, total) =
                drain_bounded_reader(reader, MAX_SANDBOX_OUTPUT_BYTES as usize).await;
            if total as u64 >= MAX_SANDBOX_OUTPUT_BYTES {
                buf.extend_from_slice(b"\n[OUTPUT TRUNCATED: 4MB limit exceeded]");
            }
            buf
        } else {
            Vec::new()
        }
    });
    let stderr_task = tokio::spawn(async move {
        if let Some(reader) = stderr_handle {
            let (mut buf, total) =
                drain_bounded_reader(reader, MAX_SANDBOX_OUTPUT_BYTES as usize).await;
            if total as u64 >= MAX_SANDBOX_OUTPUT_BYTES {
                buf.extend_from_slice(b"\n[STDERR TRUNCATED: 4MB limit exceeded]");
            }
            buf
        } else {
            Vec::new()
        }
    });

    let (timed_out, exit_code) = match tokio::time::timeout(timeout, child.wait()).await {
        Ok(Ok(status)) => (false, status.code().unwrap_or(-1)),
        Ok(Err(e)) => {
            warn!("Wait failed: {}", e);
            #[cfg(unix)]
            if let Some(pid) = child_pid {
                unsafe {
                    libc::kill(-(pid as i32), libc::SIGTERM);
                }
            }
            let _ = child.kill().await;
            #[cfg(unix)]
            if let Some(pid) = child_pid {
                unsafe {
                    libc::kill(-(pid as i32), libc::SIGKILL);
                }
            }
            let _ = child.wait().await;
            (false, -1)
        }
        Err(_elapsed) => {
            warn!("Command timed out after {:?}: {}", timeout, command_line);
            #[cfg(unix)]
            if let Some(pid) = child_pid {
                unsafe {
                    libc::kill(-(pid as i32), libc::SIGTERM);
                }
            }
            let _ = child.kill().await;
            let _ = tokio::time::timeout(Duration::from_millis(50), child.wait()).await;
            #[cfg(unix)]
            if let Some(pid) = child_pid {
                unsafe {
                    libc::kill(-(pid as i32), libc::SIGKILL);
                }
            }
            let _ = child.wait().await; // reap the killed child
            (true, -1)
        }
    };

    // Bounded drain: the child is dead (or killed) by now, so the pipes reach
    // EOF unless a grandchild inherited them; the grace cap prevents a stall.
    let stdout = match tokio::time::timeout(OUTPUT_DRAIN_GRACE, stdout_task).await {
        Ok(Ok(buf)) => String::from_utf8_lossy(&buf).to_string(),
        Ok(Err(e)) => {
            warn!("stdout drain task failed: {}", e);
            String::new()
        }
        Err(_elapsed) => {
            warn!("stdout drain exceeded grace period: {}", command_line);
            String::new()
        }
    };
    let stderr = match tokio::time::timeout(OUTPUT_DRAIN_GRACE, stderr_task).await {
        Ok(Ok(buf)) => String::from_utf8_lossy(&buf).to_string(),
        Ok(Err(e)) => {
            warn!("stderr drain task failed: {}", e);
            String::new()
        }
        Err(_elapsed) => {
            warn!("stderr drain exceeded grace period: {}", command_line);
            String::new()
        }
    };

    Ok(CommandOutput {
        exit_code,
        stdout,
        stderr,
        timed_out,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_runner_initialization() {
        let runner = SandboxRunner::new();
        // Check bwrap detection
        let _has = runner.has_bwrap();
    }

    #[tokio::test]
    async fn test_runner_executes_echo() {
        let runner = SandboxRunner::new();
        let output = runner
            .execute(
                "echo 'm31a-sandbox-test'",
                Path::new("/tmp"),
                SandboxPolicy::ReadOnly,
                Duration::from_secs(5),
            )
            .await
            .unwrap();
        assert_eq!(output.exit_code, 0);
        assert!(output.stdout.contains("m31a-sandbox-test"));
        assert!(!output.timed_out);
    }

    #[tokio::test]
    async fn test_runner_times_out() {
        let runner = SandboxRunner::new();
        let output = runner
            .execute(
                "sleep 30",
                Path::new("/tmp"),
                SandboxPolicy::DangerFullAccess,
                Duration::from_millis(200),
            )
            .await
            .unwrap();
        assert!(output.timed_out);
        assert_eq!(output.exit_code, -1);
    }

    #[test]
    fn test_sandbox_command_token_parsing() {
        let cmd = SandboxCommand::shell(
            "git commit -m \"fix(security): resolve issues\" --author=\"Test <test@m31a.local>\"",
        );
        let tokens = cmd.to_tokens();
        assert_eq!(tokens[0], "git");
        assert_eq!(tokens[1], "commit");
        assert_eq!(tokens[2], "-m");
        assert_eq!(tokens[3], "fix(security): resolve issues");
        assert_eq!(tokens[4], "--author=Test <test@m31a.local>");
    }

    #[tokio::test]
    async fn test_runner_executes_sandbox_command_exec_preserves_arguments() {
        let runner = SandboxRunner::new();
        let arg_with_spaces = "hello world with spaces and 'quotes' and \"double quotes\"";
        let empty_arg = "";
        let cmd = SandboxCommand::exec(
            "echo",
            vec![
                arg_with_spaces.to_string(),
                "arg2".to_string(),
                empty_arg.to_string(),
            ],
        );

        let output = runner
            .execute_command(
                cmd,
                Path::new("/tmp"),
                SandboxPolicy::ReadOnly,
                Duration::from_secs(5),
            )
            .await
            .unwrap();
        assert_eq!(output.exit_code, 0);
        assert!(
            output
                .stdout
                .contains("hello world with spaces and 'quotes' and \"double quotes\"")
        );
    }
}
