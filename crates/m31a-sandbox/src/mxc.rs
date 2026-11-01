//! Windows MXC (Microsoft Container) Sandbox Backend
//!
//! Implements sandboxing using Windows containers with process isolation.

use anyhow::{Context, Result};
use m31a_core::config::SandboxPolicy;
use std::path::Path;
use std::process::Command;

/// MXC sandbox implementation for Windows
pub struct MxcSandbox {
    policy: SandboxPolicy,
    container_name: String,
}

impl MxcSandbox {
    /// Create a new MXC sandbox
    pub fn new(policy: SandboxPolicy, _workspace_root: &Path) -> Result<Self> {
        let container_name = format!("m31a-sandbox-{}", uuid::Uuid::new_v4());
        Ok(Self {
            policy,
            container_name,
        })
    }

    /// Execute a command within the MXC sandbox
    pub fn execute(&self, command: &[String], working_dir: &Path) -> Result<CommandOutput> {
        if command.is_empty() {
            return Err(anyhow::anyhow!("Empty command"));
        }

        // Use docker with process isolation (MXC-compatible)
        let mut cmd = Command::new("docker");
        cmd.arg("run")
            .arg("--rm")
            .arg("--isolation=process")
            .arg("--name")
            .arg(&self.container_name)
            .arg("-v")
            .arg(format!("{}:/workspace", working_dir.display()))
            .arg("-w")
            .arg("/workspace");

        // Apply policy-specific restrictions
        match self.policy {
            SandboxPolicy::ReadOnly => {
                // Read-only filesystem
                cmd.arg("--read-only");
            }
            SandboxPolicy::WorkspaceWrite => {
                // Allow writes to workspace
            }
            SandboxPolicy::DangerFullAccess => {
                // Full access
                cmd.arg("--privileged");
            }
        }

        // Network restrictions based on policy
        match self.policy {
            SandboxPolicy::ReadOnly | SandboxPolicy::WorkspaceWrite => {
                cmd.arg("--network=none");
            }
            SandboxPolicy::DangerFullAccess => {
                // Allow network
            }
        }

        // Add command
        cmd.arg("mcr.microsoft.com/windows/servercore:ltsc2022");
        cmd.arg("cmd").arg("/c");
        for arg in command {
            cmd.arg(arg);
        }

        let output = cmd.output().context("Failed to execute docker container")?;

        Ok(CommandOutput {
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            exit_code: output.status.code().unwrap_or(-1),
        })
    }

    /// Check if MXC/Windows Containers are genuinely available and configured for Windows containers
    pub fn is_available() -> bool {
        let output = match Command::new("docker")
            .args(["info", "--format", "{{.OSType}}"])
            .output()
        {
            Ok(o) if o.status.success() => o,
            _ => {
                return Command::new("docker")
                    .arg("version")
                    .output()
                    .map(|o| {
                        if o.status.success() {
                            let text = String::from_utf8_lossy(&o.stdout);
                            text.contains("windows/amd64") || text.contains("OSType: windows")
                        } else {
                            false
                        }
                    })
                    .unwrap_or(false);
            }
        };

        let ostype = String::from_utf8_lossy(&output.stdout)
            .trim()
            .to_lowercase();
        ostype == "windows"
    }
}

/// Command output
#[derive(Debug, Clone)]
pub struct CommandOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mxc_sandbox_creation() {
        let workspace = std::path::Path::new("C:\\temp\\test");
        let sandbox = MxcSandbox::new(SandboxPolicy::WorkspaceWrite, workspace).unwrap();
        assert!(!sandbox.container_name.is_empty());
    }
}
