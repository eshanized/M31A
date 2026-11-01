//! Process manager overseeing process spawning, timeouts, signal delivery, and clean termination.
//!
//! Manages active sandbox processes, child tracking, and resource cleanup.

use anyhow::{Context, Result, anyhow};
use chrono::{DateTime, Utc};
use m31a_core::SandboxPolicy;
use std::collections::HashMap;
use std::path::Path;
use std::process::Stdio;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::Mutex;
use tracing::{debug, warn};

use crate::runner::{CommandOutput, SandboxRunner};

/// Maximum number of background processes allowed to run concurrently (64).
const MAX_PROCESSES: usize = 64;

pub const UNIFIED_EXEC_ENV: [(&str, &str); 10] = [
    ("NO_COLOR", "1"),
    ("TERM", "dumb"),
    ("LANG", "C.UTF-8"),
    ("LC_CTYPE", "C.UTF-8"),
    ("LC_ALL", "C.UTF-8"),
    ("COLORTERM", ""),
    ("PAGER", "cat"),
    ("GIT_PAGER", "cat"),
    ("GH_PAGER", "cat"),
    ("M31A_CI", "1"),
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProcessStatus {
    Running,
    Exited(i32),
    TimedOut,
    Terminated,
}

#[derive(Debug, Clone)]
pub enum SpawnResult {
    Completed(CommandOutput),
    Background {
        session_id: u32,
        initial_output: String,
        message: String,
    },
}

#[derive(Debug, Clone)]
pub struct StdinResult {
    pub session_id: u32,
    pub bytes_written: usize,
    pub new_output: String,
    pub status: ProcessStatus,
}

#[derive(Debug, Clone)]
pub struct ProcessInfo {
    pub session_id: u32,
    pub command: String,
    pub started_at: DateTime<Utc>,
    pub status: ProcessStatus,
    pub output_bytes: usize,
}

#[allow(dead_code)]
struct ActiveProcess {
    pub session_id: u32,
    pub command: String,
    pub started_at: DateTime<Utc>,
    pub last_used: Instant,
    pub stdin: Option<ChildStdin>,
    pub output_buffer: Arc<Mutex<Vec<u8>>>,
    pub status: Arc<Mutex<ProcessStatus>>,
    pub child: Arc<Mutex<Option<Child>>>,
}

#[derive(Clone)]
pub struct ProcessManager {
    sandbox: SandboxRunner,
    processes: Arc<Mutex<HashMap<u32, ActiveProcess>>>,
    next_id: Arc<AtomicU32>,
}

impl Default for ProcessManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ProcessManager {
    pub fn new() -> Self {
        Self {
            sandbox: SandboxRunner::new(),
            processes: Arc::new(Mutex::new(HashMap::new())),
            next_id: Arc::new(AtomicU32::new(1)),
        }
    }

    pub async fn spawn(
        &self,
        command_line: &str,
        cwd: &Path,
        policy: SandboxPolicy,
        yield_time: Duration,
        total_timeout: Duration,
    ) -> Result<SpawnResult> {
        let session_id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let use_bwrap = self.sandbox.has_bwrap() && policy != SandboxPolicy::DangerFullAccess;

        let mut cmd = if use_bwrap {
            self.sandbox
                .build_bwrap_command(command_line, cwd, policy)?
        } else {
            let mut c = Command::new("bash");
            c.arg("-c").arg(command_line);
            c.current_dir(cwd);
            c

            // §3.3: Landlock filesystem confinement fallback (below, so the
            // `let` binding above keeps its existing shape).
        };

        // §3.3: Landlock fallback — applied to the non-bwrap command only.
        if !use_bwrap {
            if let Some(ruleset) = crate::landlock::build_landlock_rules(&policy, cwd) {
                unsafe {
                    cmd.pre_exec(crate::landlock::pre_exec_hook(ruleset));
                }
            }

            // §3.2: seccomp filtering fallback (Landlock first, then seccomp).
            if let Some(mode) = crate::seccomp_filter::seccomp_mode_for_policy(policy) {
                let plan = crate::seccomp_filter::SeccompFilterPlan::build(mode, true);
                let hook = crate::seccomp_filter::pre_exec_seccomp_hook(&plan)?;
                unsafe {
                    cmd.pre_exec(hook);
                }
            }
        }

        for (k, v) in UNIFIED_EXEC_ENV {
            cmd.env(k, v);
        }

        cmd.stdin(Stdio::piped());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        let mut child = cmd
            .spawn()
            .with_context(|| format!("Failed to spawn process: {}", command_line))?;
        let stdin = child.stdin.take();
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();

        let output_buffer = Arc::new(Mutex::new(Vec::new()));
        let status = Arc::new(Mutex::new(ProcessStatus::Running));

        // Start background stdout reader
        if let Some(mut out) = stdout {
            let buf_clone = output_buffer.clone();
            tokio::spawn(async move {
                let mut chunk = [0u8; 4096];
                while let Ok(n) = out.read(&mut chunk).await {
                    if n == 0 {
                        break;
                    }
                    let mut b = buf_clone.lock().await;
                    b.extend_from_slice(&chunk[..n]);
                }
            });
        }

        // Start background stderr reader
        if let Some(mut err) = stderr {
            let buf_clone = output_buffer.clone();
            tokio::spawn(async move {
                let mut chunk = [0u8; 4096];
                while let Ok(n) = err.read(&mut chunk).await {
                    if n == 0 {
                        break;
                    }
                    let mut b = buf_clone.lock().await;
                    b.extend_from_slice(&chunk[..n]);
                }
            });
        }

        // Wait up to yield_time
        let yield_sleep = tokio::time::sleep(yield_time);
        tokio::pin!(yield_sleep);

        let mut completed_early = false;
        let mut exit_code = -1;

        tokio::select! {
            res = child.wait() => {
                completed_early = true;
                match res {
                    Ok(exit_status) => {
                        exit_code = exit_status.code().unwrap_or(-1);
                        *status.lock().await = ProcessStatus::Exited(exit_code);
                    }
                    Err(e) => {
                        warn!("Process wait error: {}", e);
                    }
                }
            }
            _ = &mut yield_sleep => {
                debug!("Process {} did not complete within yield window ({:?}), moving to background", session_id, yield_time);
            }
        }

        if completed_early {
            // Give readers brief time to flush
            tokio::time::sleep(Duration::from_millis(50)).await;
            let output_bytes = output_buffer.lock().await.clone();
            let output_str = String::from_utf8_lossy(&output_bytes).to_string();

            Ok(SpawnResult::Completed(CommandOutput {
                exit_code,
                stdout: output_str,
                stderr: String::new(),
                timed_out: false,
            }))
        } else {
            // Background process management
            let initial_bytes = output_buffer.lock().await.clone();
            let initial_output = String::from_utf8_lossy(&initial_bytes).to_string();

            let child_cell = Arc::new(Mutex::new(Some(child)));
            let child_cell_clone = child_cell.clone();
            let status_clone = status.clone();

            // Spawn background exit watcher
            tokio::spawn(async move {
                let timeout_sleep = tokio::time::sleep(total_timeout);
                tokio::pin!(timeout_sleep);

                let wait_fut = async {
                    let mut guard = child_cell_clone.lock().await;
                    if let Some(ref mut c) = *guard {
                        c.wait().await
                    } else {
                        Err(std::io::Error::other("no child"))
                    }
                };

                tokio::select! {
                    res = wait_fut => {
                        if let Ok(st) = res {
                            let code = st.code().unwrap_or(-1);
                            *status_clone.lock().await = ProcessStatus::Exited(code);
                        }
                    }
                    _ = &mut timeout_sleep => {
                        let mut guard = child_cell_clone.lock().await;
                        if let Some(ref mut c) = *guard {
                            let _ = c.kill().await;
                        }
                        *status_clone.lock().await = ProcessStatus::TimedOut;
                    }
                }
            });

            // Prune old processes before inserting new one (LRU, skip stdin-locked)
            self.prune_if_needed().await;

            let active = ActiveProcess {
                session_id,
                command: command_line.to_string(),
                started_at: Utc::now(),
                last_used: Instant::now(),
                stdin,
                output_buffer,
                status,
                child: child_cell,
            };

            self.processes.lock().await.insert(session_id, active);

            Ok(SpawnResult::Background {
                session_id,
                initial_output,
                message: format!(
                    "Command is still running after {:.1}s. Moved to background with session_id: {session_id}.",
                    yield_time.as_secs_f64()
                ),
            })
        }
    }

    pub async fn write_stdin(
        &self,
        session_id: u32,
        input: &str,
        yield_time: Duration,
    ) -> Result<StdinResult> {
        let mut processes = self.processes.lock().await;
        let proc = processes.get_mut(&session_id).ok_or_else(|| {
            anyhow!(
                "No active background process with session_id: {}",
                session_id
            )
        })?;

        // Update last_used on interaction
        proc.last_used = Instant::now();

        let stdin_opt = proc.stdin.as_mut().ok_or_else(|| {
            anyhow!(
                "Standard input pipe is closed for session_id: {}",
                session_id
            )
        })?;

        let before_len = proc.output_buffer.lock().await.len();
        stdin_opt
            .write_all(input.as_bytes())
            .await
            .with_context(|| format!("Failed to write to stdin of session_id: {}", session_id))?;
        stdin_opt
            .flush()
            .await
            .with_context(|| format!("Failed to flush stdin for session_id: {}", session_id))?;

        tokio::time::sleep(yield_time).await;

        let cur_bytes = proc.output_buffer.lock().await.clone();
        let new_bytes = if cur_bytes.len() > before_len {
            &cur_bytes[before_len..]
        } else {
            &[]
        };

        let new_output = String::from_utf8_lossy(new_bytes).to_string();
        let current_status = proc.status.lock().await.clone();

        Ok(StdinResult {
            session_id,
            bytes_written: input.len(),
            new_output,
            status: current_status,
        })
    }

    pub async fn poll_process(&self, session_id: u32) -> Result<(ProcessStatus, String)> {
        let processes = self.processes.lock().await;
        let proc = processes
            .get(&session_id)
            .ok_or_else(|| anyhow!("No process with session_id: {}", session_id))?;

        let status = proc.status.lock().await.clone();
        let bytes = proc.output_buffer.lock().await.clone();
        let text = String::from_utf8_lossy(&bytes).to_string();

        Ok((status, text))
    }

    pub async fn kill_process(&self, session_id: u32) -> Result<()> {
        let mut processes = self.processes.lock().await;
        let proc = processes
            .get_mut(&session_id)
            .ok_or_else(|| anyhow!("No process with session_id: {}", session_id))?;

        let mut child_guard = proc.child.lock().await;
        if let Some(ref mut c) = *child_guard {
            let _ = c.kill().await;
        }
        *proc.status.lock().await = ProcessStatus::Terminated;
        Ok(())
    }

    pub async fn list_processes(&self) -> Vec<ProcessInfo> {
        let processes = self.processes.lock().await;
        let mut list = Vec::new();

        for (&session_id, proc) in processes.iter() {
            let status = proc.status.lock().await.clone();
            let output_bytes = proc.output_buffer.lock().await.len();
            list.push(ProcessInfo {
                session_id,
                command: proc.command.clone(),
                started_at: proc.started_at,
                status,
                output_bytes,
            });
        }

        list.sort_by_key(|p| p.session_id);
        list
    }

    /// Prune background processes if we exceed MAX_PROCESSES.
    /// Uses LRU eviction, skipping processes with active stdin (interaction).
    async fn prune_if_needed(&self) {
        let mut processes = self.processes.lock().await;
        if processes.len() < MAX_PROCESSES {
            return;
        }

        // Find LRU candidate that doesn't have active stdin
        let mut lru_session_id = None;
        let mut oldest = Instant::now();

        for (&session_id, proc) in processes.iter() {
            // Skip if process has stdin (active interaction)
            if proc.stdin.is_some() {
                continue;
            }
            // Skip if process is still running and recently used
            let status = proc.status.lock().await;
            if *status == ProcessStatus::Running
                && proc.last_used.elapsed() < Duration::from_secs(60)
            {
                continue;
            }

            if proc.last_used < oldest {
                oldest = proc.last_used;
                lru_session_id = Some(session_id);
            }
        }

        if let Some(to_remove) = lru_session_id
            && let Some(proc) = processes.remove(&to_remove)
        {
            // Kill the process if still running
            let status = proc.status.lock().await.clone();
            if status == ProcessStatus::Running {
                let mut child_guard = proc.child.lock().await;
                if let Some(ref mut c) = *child_guard {
                    let _ = c.kill().await;
                }
            }
            warn!("Pruned background process {} (LRU eviction)", to_remove);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_oneshot_execution() {
        let pm = ProcessManager::new();
        let cwd = std::env::current_dir().unwrap();
        let res = pm
            .spawn(
                "echo 'hello unified exec'",
                &cwd,
                SandboxPolicy::WorkspaceWrite,
                Duration::from_secs(5),
                Duration::from_secs(10),
            )
            .await
            .unwrap();

        match res {
            SpawnResult::Completed(out) => {
                assert_eq!(out.exit_code, 0);
                assert!(out.stdout.contains("hello unified exec"));
            }
            SpawnResult::Background { .. } => panic!("Expected completed execution"),
        }
    }

    #[tokio::test]
    async fn test_background_and_stdin_interaction() {
        let pm = ProcessManager::new();
        let cwd = std::env::current_dir().unwrap();

        // Spawn a command that reads a line of stdin
        let res = pm
            .spawn(
                "read line; echo \"Received: $line\"",
                &cwd,
                SandboxPolicy::WorkspaceWrite,
                Duration::from_millis(100),
                Duration::from_secs(10),
            )
            .await
            .unwrap();

        match res {
            SpawnResult::Background { session_id, .. } => {
                let stdin_res = pm
                    .write_stdin(session_id, "test_input\n", Duration::from_millis(200))
                    .await
                    .unwrap();
                assert_eq!(stdin_res.session_id, session_id);
                assert!(stdin_res.new_output.contains("Received: test_input"));
            }
            SpawnResult::Completed(_) => panic!("Expected background execution"),
        }
    }
}
