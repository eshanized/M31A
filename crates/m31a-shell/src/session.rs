//! Stateful persistent shell session maintaining working directory and environment across turns.

use anyhow::{Context, Result};
use m31a_core::SandboxPolicy;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use crate::buffer::HeadTailBuffer;
use crate::pty::PersistentPtyProcess;

#[derive(Debug, Clone)]
pub struct ShellOutput {
    pub stdout: String,
    pub exit_code: i32,
    pub success: bool,
    pub timed_out: bool,
    pub cwd: PathBuf,
}

pub struct PersistentShellSession {
    pub session_id: String,
    pub cwd: PathBuf,
    pub env: HashMap<String, String>,
    pub buffer: HeadTailBuffer,
    pub sandbox_policy: Option<SandboxPolicy>,
    process: Option<PersistentPtyProcess>,
}

impl PersistentShellSession {
    pub fn new(session_id: impl Into<String>, initial_cwd: impl Into<PathBuf>) -> Self {
        let mut env = HashMap::new();
        // Seed from current process environment
        for (k, v) in std::env::vars() {
            env.insert(k, v);
        }

        Self {
            session_id: session_id.into(),
            cwd: initial_cwd.into(),
            env,
            buffer: HeadTailBuffer::default(),
            sandbox_policy: None,
            process: None,
        }
    }

    pub fn with_sandbox(mut self, policy: SandboxPolicy) -> Self {
        self.sandbox_policy = Some(policy);
        self
    }

    /// Sets an environment variable for subsequent shell commands.
    pub fn set_env(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.env.insert(key.into(), value.into());
    }

    /// Executes a command within this persistent shell session.
    /// Captures output, updates working directory if `cd` was used,
    /// and keeps the underlying PTY child shell alive across executions.
    pub async fn execute(&mut self, cmd: &str, timeout: Duration) -> Result<ShellOutput> {
        // Ensure child process is active
        let needs_spawn = match &mut self.process {
            Some(proc) => !proc.is_alive(),
            None => true,
        };

        if needs_spawn {
            let env_pairs: Vec<(String, String)> = self
                .env
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            let proc = PersistentPtyProcess::spawn(&self.cwd, &env_pairs, self.sandbox_policy)
                .context("Failed to spawn persistent PTY shell process")?;
            self.process = Some(proc);
        }

        let pty_res = self
            .process
            .as_mut()
            .unwrap()
            .execute(cmd, timeout)
            .await
            .context("PTY execution failed")?;

        if let Some(new_cwd) = pty_res.cwd
            && new_cwd.exists()
        {
            self.cwd = new_cwd;
        }

        let processed_output = self.buffer.process(&pty_res.stdout);
        let success = pty_res.exit_code == 0 && !pty_res.timed_out;

        Ok(ShellOutput {
            stdout: processed_output,
            exit_code: pty_res.exit_code,
            success,
            timed_out: pty_res.timed_out,
            cwd: self.cwd.clone(),
        })
    }
}
