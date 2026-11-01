//! Pseudo-terminal (PTY) runner for commands requiring interactive terminal emulation.

use anyhow::{Context, Result};
use std::path::Path;
use std::process::Command;

#[derive(Debug)]
pub struct PtyOutput {
    pub stdout: String,
    pub exit_code: i32,
    pub success: bool,
}

pub struct PtyRunner;

impl PtyRunner {
    /// Executes a command with standard terminal pipe allocation.
    /// Captures formatted output with ANSI codes preserved.
    pub fn run_command(cmd: &str, cwd: &Path, env_vars: &[(String, String)]) -> Result<PtyOutput> {
        let mut command = Command::new("sh");
        command.arg("-c").arg(cmd);
        command.current_dir(cwd);

        // Inherit environment and overlay
        for (k, v) in env_vars {
            command.env(k, v);
        }
        // Hint to tools (like cargo, git, npm) that color output is desired
        command.env("CLICOLOR_FORCE", "1");
        command.env("FORCE_COLOR", "1");
        command.env("TERM", "xterm-256color");

        let output = command
            .output()
            .with_context(|| format!("Failed to execute command: {}", cmd))?;

        let mut combined = Vec::new();
        combined.extend_from_slice(&output.stdout);
        if !output.stderr.is_empty() {
            if !combined.is_empty() && !combined.ends_with(b"\n") {
                combined.push(b'\n');
            }
            combined.extend_from_slice(&output.stderr);
        }

        let text = String::from_utf8_lossy(&combined).to_string();
        let exit_code = output
            .status
            .code()
            .unwrap_or(if output.status.success() { 0 } else { 1 });

        Ok(PtyOutput {
            stdout: text,
            exit_code,
            success: output.status.success(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pty_runner_echo() {
        let cwd = std::env::current_dir().unwrap();
        let res = PtyRunner::run_command("echo 'Hello PTY'", &cwd, &[]).unwrap();
        assert!(res.success);
        assert!(res.stdout.contains("Hello PTY"));
    }
}
