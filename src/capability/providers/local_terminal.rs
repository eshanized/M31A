//! Local terminal session capability provider (CTL-02, D-01, D-03).

use crate::capability::error::CapabilityError;
use crate::capability::traits::terminal::{
    TerminalService, TerminalSessionConfig, TerminalSessionStatus,
};
use crate::process::env::EnvironmentBuilder;
use crate::process::tree::ProcessTreeController;
use crate::telemetry::redactor::SecretRedactor;
use async_trait::async_trait;
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::Command;
use tokio::sync::{Mutex, RwLock, mpsc};

const MAX_BUFFER_BYTES: usize = 1024 * 1024; // 1 MB ring buffer per session

struct TerminalSessionHandle {
    pid: u32,
    stdin_tx: mpsc::Sender<Vec<u8>>,
    output_buffer: Arc<Mutex<VecDeque<u8>>>,
    exit_status: Arc<Mutex<Option<i32>>>,
    cols: u16,
    rows: u16,
}

/// Native local terminal provider managing real isolated child process sessions.
pub struct LocalTerminalProvider {
    workspace_root: Option<PathBuf>,
    sessions: Arc<RwLock<HashMap<String, TerminalSessionHandle>>>,
}

impl Default for LocalTerminalProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl LocalTerminalProvider {
    pub fn new() -> Self {
        Self {
            workspace_root: None,
            sessions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn with_workspace(workspace_root: impl Into<PathBuf>) -> Self {
        Self {
            workspace_root: Some(workspace_root.into()),
            sessions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    async fn spawn_session_internal(
        &self,
        session_id: &str,
        config: TerminalSessionConfig,
    ) -> Result<TerminalSessionHandle, CapabilityError> {
        let mut cmd = Command::new(&config.command);
        cmd.args(&config.args);

        // Workspace / working directory confinement
        let effective_cwd = if let Some(ref cwd) = config.cwd {
            if let Some(ref root) = self.workspace_root {
                let canonical_root = root.canonicalize().unwrap_or_else(|_| root.clone());
                let canonical_cwd = cwd.canonicalize().unwrap_or_else(|_| cwd.clone());
                if !canonical_cwd.starts_with(&canonical_root) {
                    return Err(CapabilityError::PathOutOfBounds {
                        path: cwd.display().to_string(),
                        workspace: root.display().to_string(),
                    });
                }
            }
            cwd.clone()
        } else if let Some(ref root) = self.workspace_root {
            root.clone()
        } else {
            std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
        };
        // Environment isolation: build safe base env and apply working directory
        let env_builder = EnvironmentBuilder::new(&effective_cwd);
        env_builder.apply(&mut cmd);
        for (k, v) in &config.env {
            cmd.env(k, v);
        }
        cmd.env("TERM", "xterm-256color");
        cmd.env("COLUMNS", config.cols.to_string());
        cmd.env("LINES", config.rows.to_string());

        cmd.stdin(Stdio::piped());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        let (mut child, _tree) = ProcessTreeController::spawn_isolated(cmd)
            .map_err(|e| CapabilityError::Io(e.to_string()))?;

        let pid = child.id().unwrap_or(0);
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| CapabilityError::ExecutionFailed {
                exit_code: None,
                message: "Failed to capture stdin for terminal child".to_string(),
            })?;
        let mut stdout = child
            .stdout
            .take()
            .ok_or_else(|| CapabilityError::ExecutionFailed {
                exit_code: None,
                message: "Failed to capture stdout for terminal child".to_string(),
            })?;
        let mut stderr = child
            .stderr
            .take()
            .ok_or_else(|| CapabilityError::ExecutionFailed {
                exit_code: None,
                message: "Failed to capture stderr for terminal child".to_string(),
            })?;

        let output_buffer = Arc::new(Mutex::new(VecDeque::new()));
        let exit_status = Arc::new(Mutex::new(None));
        let (stdin_tx, mut stdin_rx) = mpsc::channel::<Vec<u8>>(64);

        // Stdin forwarder task
        tokio::spawn(async move {
            while let Some(bytes) = stdin_rx.recv().await {
                if stdin.write_all(&bytes).await.is_err() || stdin.flush().await.is_err() {
                    break;
                }
            }
        });

        // Stdout reader task
        let out_buf_clone = Arc::clone(&output_buffer);
        tokio::spawn(async move {
            let mut buf = [0u8; 4096];
            let redactor = SecretRedactor::new();
            while let Ok(n) = stdout.read(&mut buf).await {
                if n == 0 {
                    break;
                }
                let raw_chunk = &buf[..n];
                let scrubbed = if let Ok(s) = std::str::from_utf8(raw_chunk) {
                    redactor.redact_text(s).into_bytes()
                } else {
                    raw_chunk.to_vec()
                };

                let mut guard = out_buf_clone.lock().await;
                guard.extend(scrubbed);
                while guard.len() > MAX_BUFFER_BYTES {
                    guard.pop_front();
                }
            }
        });

        // Stderr reader task
        let err_buf_clone = Arc::clone(&output_buffer);
        tokio::spawn(async move {
            let mut buf = [0u8; 4096];
            let redactor = SecretRedactor::new();
            while let Ok(n) = stderr.read(&mut buf).await {
                if n == 0 {
                    break;
                }
                let raw_chunk = &buf[..n];
                let scrubbed = if let Ok(s) = std::str::from_utf8(raw_chunk) {
                    redactor.redact_text(s).into_bytes()
                } else {
                    raw_chunk.to_vec()
                };

                let mut guard = err_buf_clone.lock().await;
                guard.extend(scrubbed);
                while guard.len() > MAX_BUFFER_BYTES {
                    guard.pop_front();
                }
            }
        });

        // Child process lifetime supervisor
        let exit_clone = Arc::clone(&exit_status);
        let sid_string = session_id.to_string();
        tokio::spawn(async move {
            match child.wait().await {
                Ok(status) => {
                    let code = status.code().unwrap_or(-1);
                    *exit_clone.lock().await = Some(code);
                }
                Err(e) => {
                    tracing::warn!(session_id = %sid_string, error = %e, "terminal session wait failed");
                    *exit_clone.lock().await = Some(-1);
                }
            }
        });

        Ok(TerminalSessionHandle {
            pid,
            stdin_tx,
            output_buffer,
            exit_status,
            cols: config.cols,
            rows: config.rows,
        })
    }
}

#[async_trait]
impl TerminalService for LocalTerminalProvider {
    async fn start_session(
        &self,
        session_id: &str,
        config: TerminalSessionConfig,
    ) -> Result<(), CapabilityError> {
        let handle = self.spawn_session_internal(session_id, config).await?;
        let mut sessions = self.sessions.write().await;
        sessions.insert(session_id.to_string(), handle);
        Ok(())
    }

    async fn write_input(&self, session_id: &str, input: &[u8]) -> Result<(), CapabilityError> {
        let sender = {
            let sessions = self.sessions.read().await;
            sessions.get(session_id).map(|s| s.stdin_tx.clone())
        };

        let tx = match sender {
            Some(tx) => tx,
            None => {
                return Err(CapabilityError::NotFound(format!(
                    "Terminal session '{session_id}' not found; session must be started explicitly before writing input"
                )));
            }
        };

        tx.send(input.to_vec())
            .await
            .map_err(|e| CapabilityError::Io(format!("failed to send input to terminal: {e}")))?;
        Ok(())
    }

    async fn read_stream(
        &self,
        session_id: &str,
        timeout_ms: u64,
    ) -> Result<Vec<u8>, CapabilityError> {
        let buffer_arc = {
            let sessions = self.sessions.read().await;
            sessions
                .get(session_id)
                .map(|s| Arc::clone(&s.output_buffer))
        };

        let Some(buf) = buffer_arc else {
            return Err(CapabilityError::NotFound(format!(
                "Terminal session '{session_id}' not found"
            )));
        };

        let start = tokio::time::Instant::now();
        let timeout = Duration::from_millis(timeout_ms);

        loop {
            {
                let mut guard = buf.lock().await;
                if !guard.is_empty() {
                    return Ok(guard.drain(..).collect());
                }
            }

            if start.elapsed() >= timeout {
                return Ok(Vec::new());
            }

            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    async fn resize(&self, session_id: &str, cols: u16, rows: u16) -> Result<(), CapabilityError> {
        let mut sessions = self.sessions.write().await;
        let session = sessions
            .get_mut(session_id)
            .ok_or_else(|| CapabilityError::NotFound(format!("session {session_id} not found")))?;
        session.cols = cols;
        session.rows = rows;
        Ok(())
    }

    async fn terminate_session(&self, session_id: &str) -> Result<(), CapabilityError> {
        let handle = {
            let mut sessions = self.sessions.write().await;
            sessions.remove(session_id)
        };

        if let Some(h) = handle {
            let _ =
                ProcessTreeController::terminate_by_pid(h.pid, Duration::from_millis(500)).await;
        }
        Ok(())
    }

    async fn session_status(
        &self,
        session_id: &str,
    ) -> Result<TerminalSessionStatus, CapabilityError> {
        let sessions = self.sessions.read().await;
        let session = sessions
            .get(session_id)
            .ok_or_else(|| CapabilityError::NotFound(format!("session {session_id} not found")))?;

        let exit_code = *session.exit_status.lock().await;
        let is_alive = exit_code.is_none();

        Ok(TerminalSessionStatus {
            session_id: session_id.to_string(),
            is_alive,
            exit_code,
            cols: session.cols,
            rows: session.rows,
        })
    }
}
