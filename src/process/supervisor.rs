//! Supervised foreground command runner with timeouts and cancellation (TL-02, Law 8, per D-13, D-14).

use std::path::Path;
use std::process::Stdio;
use std::time::Duration;
use thiserror::Error;
use tokio::io::AsyncReadExt;
use tokio::process::Command;
use tokio_util::sync::CancellationToken;

use crate::process::env::EnvironmentBuilder;
use crate::process::tree::ProcessTreeController;
use crate::process::types::ProcessOutput;

pub const DEFAULT_FOREGROUND_TIMEOUT: Duration = Duration::from_secs(30);
pub const DEFAULT_GRACE_PERIOD: Duration = Duration::from_millis(1000);

/// Typed process execution errors.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ProcessError {
    #[error("Failed to spawn process: {0}")]
    SpawnFailed(String),

    #[error("Process execution was cancelled")]
    Cancelled,

    #[error("Process execution timed out after {0:?}")]
    TimedOut(Duration),

    #[error("I/O error during process execution: {0}")]
    Io(String),

    #[error("Process execution failed: {0}")]
    ExecutionFailed(String),
}

/// Supervised foreground command runner enforcing bounds, cancellation, and isolation.
#[derive(Debug, Clone)]
pub struct ProcessSupervisor {
    default_timeout: Duration,
    grace_period: Duration,
}

impl Default for ProcessSupervisor {
    fn default() -> Self {
        Self::new(DEFAULT_FOREGROUND_TIMEOUT, DEFAULT_GRACE_PERIOD)
    }
}

impl ProcessSupervisor {
    /// Create a new supervisor with custom timeout and termination grace period.
    pub fn new(default_timeout: Duration, grace_period: Duration) -> Self {
        Self {
            default_timeout,
            grace_period,
        }
    }

    /// Run a bounded foreground command with process group isolation and cancellation.
    pub async fn run_command(
        &self,
        program: &str,
        args: &[String],
        cwd: &Path,
        env_builder: Option<&EnvironmentBuilder>,
        timeout_override: Option<Duration>,
        token: Option<&CancellationToken>,
    ) -> Result<ProcessOutput, ProcessError> {
        let timeout_dur = timeout_override.unwrap_or(self.default_timeout);

        let mut cmd = Command::new(program);
        cmd.args(args);

        if let Some(builder) = env_builder {
            builder.apply(&mut cmd);
        } else {
            let default_builder = EnvironmentBuilder::new(cwd);
            default_builder.apply(&mut cmd);
        }

        self.run_supervised(cmd, timeout_dur, token).await
    }

    /// Run an already prepared Tokio `Command` under supervision with process-group isolation,
    /// timeout enforcement, and cancellation handling.
    pub async fn run_supervised(
        &self,
        mut cmd: Command,
        timeout_dur: Duration,
        token: Option<&CancellationToken>,
    ) -> Result<ProcessOutput, ProcessError> {
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        // Spawn isolated in its own process group
        let (mut child, tree) = ProcessTreeController::spawn_isolated(cmd)
            .map_err(|e| ProcessError::SpawnFailed(e.to_string()))?;

        // Take output pipes
        let mut stdout_pipe = child.stdout.take();
        let mut stderr_pipe = child.stderr.take();

        // Tasks to concurrently buffer stdout and stderr
        let stdout_handle = tokio::spawn(async move {
            let mut buf = Vec::new();
            if let Some(mut pipe) = stdout_pipe.take() {
                let _ = pipe.read_to_end(&mut buf).await;
            }
            buf
        });

        let stderr_handle = tokio::spawn(async move {
            let mut buf = Vec::new();
            if let Some(mut pipe) = stderr_pipe.take() {
                let _ = pipe.read_to_end(&mut buf).await;
            }
            buf
        });

        // Multiplex child wait, cancellation, and timeout
        let sleep_fut = tokio::time::sleep(timeout_dur);
        tokio::pin!(sleep_fut);

        tokio::select! {
            // Path 1: Normal child exit
            wait_res = child.wait() => {
                let status = wait_res.map_err(|e| ProcessError::Io(e.to_string()))?;
                let stdout_bytes = stdout_handle.await.unwrap_or_default();
                let stderr_bytes = stderr_handle.await.unwrap_or_default();

                Ok(ProcessOutput {
                    exit_code: status.code().unwrap_or(-1),
                    stdout: String::from_utf8_lossy(&stdout_bytes).to_string(),
                    stderr: String::from_utf8_lossy(&stderr_bytes).to_string(),
                })
            }

            // Path 2: Cooperative cancellation
            _ = async {
                if let Some(tok) = token {
                    tok.cancelled().await;
                } else {
                    std::future::pending::<()>().await;
                }
            } => {
                let _ = tree.terminate_supervised(&mut child, self.grace_period).await;
                let _ = stdout_handle.await;
                let _ = stderr_handle.await;
                Err(ProcessError::Cancelled)
            }

            // Path 3: Wall-clock timeout
            _ = &mut sleep_fut => {
                let _ = tree.terminate_supervised(&mut child, self.grace_period).await;
                let _ = stdout_handle.await;
                let _ = stderr_handle.await;
                Err(ProcessError::TimedOut(timeout_dur))
            }
        }
    }
}

/// Create a direct argv Tokio Command without invoking any shell (PRC-02, D-14).
///
/// Arguments with spaces, quotes, or control characters are passed directly
/// to `execve` without string concatenation or shell escaping vulnerabilities.
pub fn execute_direct_argv(program: &str, args: &[String], cwd: &Path) -> Command {
    let mut cmd = Command::new(program);
    cmd.args(args);
    cmd.current_dir(cwd);
    let env_builder = crate::process::env::EnvironmentBuilder::new(cwd);
    env_builder.apply(&mut cmd);
    cmd
}

/// Create a shell-wrapped Tokio Command for explicit, policy-authorized shell execution (PRC-02, D-14).
///
/// NOTE: Shell execution MUST pass through explicit Stage 7 policy clearance.
/// The interpreter resolves through the platform shell contract so higher
/// layers never assume a fixed shell name.
pub fn execute_shell_string(shell_str: &str, cwd: &Path) -> Command {
    match crate::platform::shell::build_shell_command(shell_str, cwd) {
        Ok(mut cmd) => {
            let env_builder = crate::process::env::EnvironmentBuilder::new(cwd);
            env_builder.apply(&mut cmd);
            cmd
        }
        Err(_) => {
            // Fail-closed fallback for hosts without a native interpreter:
            // use a program name that cannot resolve so spawning fails
            // explicitly instead of running under an unrelated shell.
            let mut cmd = Command::new("__m31a_unsupported_shell__");
            cmd.current_dir(cwd);
            let _ = shell_str;
            cmd
        }
    }
}

/// Fallible shell-string constructor reporting unsupported backends explicitly.
pub fn try_execute_shell_string(
    shell_str: &str,
    cwd: &Path,
) -> Result<Command, crate::platform::shell::ShellError> {
    let mut cmd = crate::platform::shell::build_shell_command(shell_str, cwd)?;
    let env_builder = crate::process::env::EnvironmentBuilder::new(cwd);
    env_builder.apply(&mut cmd);
    Ok(cmd)
}
