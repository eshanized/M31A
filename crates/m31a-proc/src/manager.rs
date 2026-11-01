//! Background process manager supervising long-running tasks.
//!
//! Tracks active child processes, handles termination signals, and monitors resource utilization.

use crate::buffer::{CircularOutputBuffer, OutputStream, ProcessLine};
use crate::error::ProcessError;
use crate::process::{
    ManagedProcess, ProcessInfo, ProcessOutput, ProcessStatus, ProcessStopResult,
};
use chrono::Utc;
use m31a_core::SandboxPolicy;
use m31a_execpolicy::{Decision, ExecPolicy};
use m31a_sandbox::runner::SandboxRunner;
use std::collections::HashMap;
use std::path::Path;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::RwLock;
use tracing::{debug, error, info};

#[derive(Clone)]
pub struct ProcessManagerConfig {
    pub default_buffer_capacity: usize,
    pub termination_grace_period_ms: u64,
    pub exec_policy: Option<ExecPolicy>,
    pub sandbox_policy: Option<SandboxPolicy>,
}

impl Default for ProcessManagerConfig {
    fn default() -> Self {
        Self {
            default_buffer_capacity: 5000,
            termination_grace_period_ms: 2000,
            exec_policy: None,
            sandbox_policy: None,
        }
    }
}

#[derive(Clone)]
pub struct ProcessManager {
    inner: Arc<ProcessManagerInner>,
}

struct ProcessManagerInner {
    processes: RwLock<HashMap<String, Arc<ManagedProcess>>>,
    config: ProcessManagerConfig,
}

impl Default for ProcessManager {
    fn default() -> Self {
        Self::new(ProcessManagerConfig::default())
    }
}

impl ProcessManager {
    pub fn new(config: ProcessManagerConfig) -> Self {
        Self {
            inner: Arc::new(ProcessManagerInner {
                processes: RwLock::new(HashMap::new()),
                config,
            }),
        }
    }

    /// Spawns a new background process with streaming stdout/stderr buffers,
    /// enforcing security policies and sandbox isolation.
    pub async fn start(
        &self,
        command: &str,
        args: &[String],
        cwd: &Path,
        env: HashMap<String, String>,
        label: Option<String>,
    ) -> Result<ProcessInfo, ProcessError> {
        let short_id = format!("proc-{}", &uuid::Uuid::new_v4().to_string()[..8]);
        let proc_label = label.unwrap_or_else(|| {
            let cmd_base = Path::new(command)
                .file_name()
                .map(|f| f.to_string_lossy().to_string())
                .unwrap_or_else(|| command.to_string());
            if args.is_empty() {
                cmd_base
            } else {
                format!("{} {}", cmd_base, args.join(" "))
            }
        });

        // 1. Enforce execution policy if configured
        let full_cmd = if args.is_empty() {
            command.to_string()
        } else {
            format!("{} {}", command, args.join(" "))
        };

        if let Some(ref policy) = self.inner.config.exec_policy {
            let decision = policy.evaluate(&full_cmd);
            if decision == Decision::Deny {
                return Err(ProcessError::PolicyViolation(
                    full_cmd,
                    "Command rejected by active execution security policy".to_string(),
                ));
            }
        }

        // 2. Build sandboxed command if sandbox policy is configured
        let mut cmd = match self.inner.config.sandbox_policy {
            Some(policy) if policy != SandboxPolicy::DangerFullAccess => {
                let runner = SandboxRunner::new();
                if runner.has_bwrap() {
                    let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
                    runner
                        .build_bwrap_process(command, &arg_refs, cwd, policy)
                        .map_err(|e| {
                            ProcessError::SpawnFailed(command.to_string(), e.to_string())
                        })?
                } else {
                    let mut c = tokio::process::Command::new(command);
                    c.args(args);
                    c.current_dir(cwd);
                    c
                }
            }
            _ => {
                let mut c = tokio::process::Command::new(command);
                c.args(args);
                c.current_dir(cwd);
                c
            }
        };

        cmd.stdin(Stdio::null());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());
        for (k, v) in env {
            cmd.env(k, v);
        }

        // On Unix, run in own process group so child processes can be reaped
        #[cfg(unix)]
        cmd.process_group(0);

        let mut child = cmd
            .spawn()
            .map_err(|e| ProcessError::SpawnFailed(command.to_string(), e.to_string()))?;

        let pid = child.id();
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();

        let started_at = Utc::now();
        let buffer = Arc::new(RwLock::new(CircularOutputBuffer::new(
            self.inner.config.default_buffer_capacity,
        )));

        let status = Arc::new(RwLock::new(match pid {
            Some(p) => ProcessStatus::Running { pid: p },
            None => ProcessStatus::Running { pid: 0 },
        }));

        let (cancel_tx, cancel_rx) = tokio::sync::oneshot::channel::<()>();

        // Background reader for stdout
        if let Some(out) = stdout {
            let buf_clone = buffer.clone();
            tokio::spawn(async move {
                let reader = BufReader::new(out);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    let mut b = buf_clone.write().await;
                    b.push(OutputStream::Stdout, line);
                }
            });
        }

        // Background reader for stderr
        if let Some(err) = stderr {
            let buf_clone = buffer.clone();
            tokio::spawn(async move {
                let reader = BufReader::new(err);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    let mut b = buf_clone.write().await;
                    b.push(OutputStream::Stderr, line);
                }
            });
        }

        // Background supervisor task to monitor process exit
        let status_clone = status.clone();
        let buf_clone = buffer.clone();
        let id_for_task = short_id.clone();
        tokio::spawn(async move {
            tokio::select! {
                exit_res = child.wait() => {
                    let mut st = status_clone.write().await;
                    let mut b = buf_clone.write().await;
                    match exit_res {
                        Ok(exit_status) => {
                            if let Some(code) = exit_status.code() {
                                *st = ProcessStatus::Exited { exit_code: code };
                                b.push(OutputStream::System, format!("[Process exited with code {}]", code));
                                debug!(id = %id_for_task, code, "Managed process exited");
                            } else {
                                #[cfg(unix)]
                                {
                                    use std::os::unix::process::ExitStatusExt;
                                    let sig = exit_status.signal().unwrap_or(0);
                                    *st = ProcessStatus::Terminated { signal: sig };
                                    b.push(OutputStream::System, format!("[Process terminated by signal {}]", sig));
                                    debug!(id = %id_for_task, sig, "Managed process terminated by signal");
                                }
                                #[cfg(not(unix))]
                                {
                                    *st = ProcessStatus::Terminated { signal: 0 };
                                    b.push(OutputStream::System, "[Process terminated]".to_string());
                                }
                            }
                        }
                        Err(e) => {
                            *st = ProcessStatus::Failed { error: e.to_string() };
                            b.push(OutputStream::System, format!("[Wait error: {}]", e));
                            error!(id = %id_for_task, error = %e, "Managed process wait failed");
                        }
                    }
                }
                _ = cancel_rx => {
                    // Canceled via sender
                    let _ = child.kill().await;
                    let mut st = status_clone.write().await;
                    *st = ProcessStatus::Terminated { signal: 9 };
                    let mut b = buf_clone.write().await;
                    b.push(OutputStream::System, "[Process forcefully killed by supervisor]".to_string());
                }
            }
        });

        let managed = Arc::new(ManagedProcess {
            id: short_id.clone(),
            label: proc_label,
            command: command.to_string(),
            args: args.to_vec(),
            cwd: cwd.to_path_buf(),
            pid,
            started_at,
            status,
            buffer,
            cancel_tx: Some(cancel_tx),
        });

        let info = managed.to_info().await;
        let mut map = self.inner.processes.write().await;
        map.insert(short_id, managed);

        info!(id = %info.id, cmd = %command, pid = ?pid, "Started background process");
        Ok(info)
    }

    /// Lists all managed background processes.
    pub async fn list(&self) -> Vec<ProcessInfo> {
        let map = self.inner.processes.read().await;
        let mut result = Vec::with_capacity(map.len());
        for p in map.values() {
            result.push(p.to_info().await);
        }
        result.sort_by_key(|a| std::cmp::Reverse(a.started_at));
        result
    }

    /// Returns the number of managed background processes synchronously without blocking.
    pub fn process_count(&self) -> usize {
        self.inner
            .processes
            .try_read()
            .map(|m| m.len())
            .unwrap_or(0)
    }

    /// Gets metadata for a specific process.
    pub async fn get(&self, id: &str) -> Option<ProcessInfo> {
        let map = self.inner.processes.read().await;
        if let Some(p) = map.get(id) {
            Some(p.to_info().await)
        } else {
            None
        }
    }

    /// Retrieves streaming process output.
    pub async fn get_output(
        &self,
        id: &str,
        offset: Option<usize>,
        limit: Option<usize>,
    ) -> Result<ProcessOutput, ProcessError> {
        let map = self.inner.processes.read().await;
        let p = map
            .get(id)
            .ok_or_else(|| ProcessError::NotFound(id.to_string()))?;

        let status = p.status.read().await;
        let is_running = status.is_running();
        let buf = p.buffer.read().await;

        let limit = limit.unwrap_or(200).min(2000);
        let offset = offset.unwrap_or(0);

        let (lines, total_lines) = buf.slice(offset, limit);
        let text = buf.as_text(limit);

        Ok(ProcessOutput {
            process_id: p.id.clone(),
            label: p.label.clone(),
            is_running,
            offset,
            total_lines,
            lines,
            text,
        })
    }

    /// Tails the most recent lines of output.
    pub async fn tail_output(
        &self,
        id: &str,
        limit: usize,
    ) -> Result<Vec<ProcessLine>, ProcessError> {
        let map = self.inner.processes.read().await;
        let p = map
            .get(id)
            .ok_or_else(|| ProcessError::NotFound(id.to_string()))?;
        let buf = p.buffer.read().await;
        Ok(buf.tail(limit))
    }

    /// Stops a background process cleanly (graceful SIGTERM then SIGKILL, or immediate force).
    pub async fn stop(&self, id: &str, force: bool) -> Result<ProcessStopResult, ProcessError> {
        let map = self.inner.processes.read().await;
        let p = map
            .get(id)
            .ok_or_else(|| ProcessError::NotFound(id.to_string()))?;

        let is_running = {
            let st = p.status.read().await;
            st.is_running()
        };

        if !is_running {
            return Ok(ProcessStopResult {
                id: p.id.clone(),
                label: p.label.clone(),
                stopped: false,
                message: "Process is not running".to_string(),
            });
        }

        let pid = match p.pid {
            Some(pid) => pid,
            None => {
                return Err(ProcessError::TerminationFailed(
                    id.to_string(),
                    "Process has no valid PID".to_string(),
                ));
            }
        };

        if force {
            self.kill_pid_force(pid);
            let mut st = p.status.write().await;
            *st = ProcessStatus::Terminated { signal: 9 };
            let mut b = p.buffer.write().await;
            b.push(
                OutputStream::System,
                "[Process forcefully terminated (SIGKILL)]".to_string(),
            );
            return Ok(ProcessStopResult {
                id: p.id.clone(),
                label: p.label.clone(),
                stopped: true,
                message: "Forcefully killed (SIGKILL)".to_string(),
            });
        }

        // Graceful termination
        #[cfg(unix)]
        {
            // Send SIGTERM to process group or pid
            unsafe {
                libc::kill(pid as i32, libc::SIGTERM);
            }

            // Wait for grace period
            let grace = Duration::from_millis(self.inner.config.termination_grace_period_ms);
            let check_interval = Duration::from_millis(100);
            let mut elapsed = Duration::from_millis(0);
            let mut exited = false;

            while elapsed < grace {
                tokio::time::sleep(check_interval).await;
                elapsed += check_interval;
                let st = p.status.read().await;
                if !st.is_running() {
                    exited = true;
                    break;
                }
            }

            if !exited {
                // Force kill with SIGKILL
                unsafe {
                    libc::kill(pid as i32, libc::SIGKILL);
                }
                let mut st = p.status.write().await;
                *st = ProcessStatus::Terminated { signal: 9 };
                let mut b = p.buffer.write().await;
                b.push(
                    OutputStream::System,
                    "[Grace period expired; killed with SIGKILL]".to_string(),
                );
                Ok(ProcessStopResult {
                    id: p.id.clone(),
                    label: p.label.clone(),
                    stopped: true,
                    message: "Terminated with SIGKILL after timeout".to_string(),
                })
            } else {
                Ok(ProcessStopResult {
                    id: p.id.clone(),
                    label: p.label.clone(),
                    stopped: true,
                    message: "Gracefully terminated (SIGTERM)".to_string(),
                })
            }
        }

        #[cfg(not(unix))]
        {
            self.kill_pid_force(pid);
            let mut st = p.status.write().await;
            *st = ProcessStatus::Terminated { signal: 9 };
            Ok(ProcessStopResult {
                id: p.id.clone(),
                label: p.label.clone(),
                stopped: true,
                message: "Terminated".to_string(),
            })
        }
    }

    /// Stops all running background processes.
    pub async fn stop_all(&self) -> Vec<ProcessStopResult> {
        let procs = self.list().await;
        let mut results = Vec::new();
        for p in procs {
            if p.status.is_running()
                && let Ok(res) = self.stop(&p.id, false).await
            {
                results.push(res);
            }
        }
        results
    }

    /// Purges finished/terminated processes older than max_age_seconds from memory.
    pub async fn cleanup_finished(&self, max_age_seconds: u64) {
        let mut map = self.inner.processes.write().await;
        let now = Utc::now();
        map.retain(|_, p| {
            let st = match p.status.try_read() {
                Ok(guard) => guard.clone(),
                Err(_) => return true,
            };
            if st.is_running() {
                true
            } else {
                let age = now.signed_duration_since(p.started_at).num_seconds().max(0) as u64;
                age < max_age_seconds
            }
        });
    }

    #[cfg(unix)]
    fn kill_pid_force(&self, pid: u32) {
        unsafe {
            libc::kill(pid as i32, libc::SIGKILL);
        }
    }

    #[cfg(not(unix))]
    fn kill_pid_force(&self, _pid: u32) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_process_lifecycle() {
        let mgr = ProcessManager::default();
        let temp = tempdir().unwrap();

        // Start a short-lived echo process
        let info = mgr
            .start(
                "echo",
                &["hello from m31a".to_string()],
                temp.path(),
                HashMap::new(),
                Some("test-echo".into()),
            )
            .await
            .expect("failed to spawn echo");

        assert_eq!(info.label, "test-echo");
        assert!(info.pid.is_some());

        // Wait a brief moment for execution to complete
        tokio::time::sleep(Duration::from_millis(200)).await;

        let output = mgr.get_output(&info.id, None, None).await.unwrap();
        assert!(output.text.contains("hello from m31a"));

        let list = mgr.list().await;
        assert_eq!(list.len(), 1);
        assert!(!list[0].status.is_running());
    }

    #[tokio::test]
    async fn test_process_stop() {
        let mgr = ProcessManager::default();
        let temp = tempdir().unwrap();

        // Start a sleep process
        let info = mgr
            .start(
                "sleep",
                &["10".to_string()],
                temp.path(),
                HashMap::new(),
                Some("test-sleep".into()),
            )
            .await
            .expect("failed to spawn sleep");

        assert!(info.status.is_running());

        // Terminate it
        let stop_res = mgr.stop(&info.id, false).await.expect("failed to stop");
        assert!(stop_res.stopped);

        // Verify it is no longer running
        let updated = mgr.get(&info.id).await.unwrap();
        assert!(!updated.status.is_running());
    }
}
