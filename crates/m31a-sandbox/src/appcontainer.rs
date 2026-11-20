//! Windows AppContainer Sandbox Configuration and Planning Backend.
//!
//! Models security plans and explicit file system ACL grants for Windows AppContainer profiles:
//! - Dedicated AppContainer SID with low integrity level calculation
//! - Explicit DACL file system access grant formatting restricted to workspace trees
//! - Capability-gated network access planning (e.g. `internetClient` granted only when policy allows)
//! - Sensitive Windows system folders and user profiles modeled for denial
//!
//! Note: Process execution under AppContainer is disabled in this userspace build pending
//! restricted process token creation and fails-closed.

use anyhow::{Result, bail};
use m31a_core::config::SandboxPolicy;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

/// Well-known Windows AppContainer Capability SIDs
pub const WIN_CAPABILITY_INTERNET_CLIENT: &str = "S-1-15-3-1";
pub const WIN_CAPABILITY_PRIVATE_NETWORK_CLIENT_SERVER: &str = "S-1-15-3-3";

/// Explicit security configuration plan for a Windows AppContainer instance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppContainerSecurityPlan {
    pub profile_name: String,
    pub display_name: String,
    pub workspace_root: PathBuf,
    pub policy: SandboxPolicy,
    pub capability_sids: Vec<String>,
    pub read_only_workspace: bool,
    pub network_enabled: bool,
    pub protected_metadata_masks: Vec<String>,
}

/// AppContainer sandbox implementation
pub struct AppContainerSandbox {
    plan: AppContainerSecurityPlan,
}

impl AppContainerSandbox {
    pub const PROTECTED_METADATA_NAMES: &'static [&'static str] =
        &[".git", ".m31a", ".ssh", ".env", ".aws", ".agents"];

    /// Creates a new AppContainer configuration for the specified workspace and policy.
    pub fn new(policy: SandboxPolicy, workspace_root: &Path) -> Result<Self> {
        let profile_id = uuid::Uuid::new_v4();
        let profile_name = format!("M31A.Sandbox.{}", profile_id);
        let display_name = format!("M31A Autonomous Agent Sandbox ({})", profile_id);

        let mut capability_sids = Vec::new();
        let (read_only_workspace, network_enabled) = match policy {
            SandboxPolicy::ReadOnly => (true, false),
            SandboxPolicy::WorkspaceWrite => {
                capability_sids.push(WIN_CAPABILITY_INTERNET_CLIENT.to_string());
                (false, true)
            }
            SandboxPolicy::DangerFullAccess => {
                capability_sids.push(WIN_CAPABILITY_INTERNET_CLIENT.to_string());
                capability_sids.push(WIN_CAPABILITY_PRIVATE_NETWORK_CLIENT_SERVER.to_string());
                (false, true)
            }
        };

        let protected_metadata_masks = Self::PROTECTED_METADATA_NAMES
            .iter()
            .map(|s| s.to_string())
            .collect();

        let plan = AppContainerSecurityPlan {
            profile_name,
            display_name,
            workspace_root: workspace_root.to_path_buf(),
            policy,
            capability_sids,
            read_only_workspace,
            network_enabled,
            protected_metadata_masks,
        };

        Ok(Self { plan })
    }

    pub fn plan(&self) -> &AppContainerSecurityPlan {
        &self.plan
    }

    /// Verifies whether the current Windows platform supports native AppContainer creation.
    /// AppContainer is deliberately unavailable until the child is created with
    /// CreateProcessW and PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES.
    ///
    /// The former implementation launched cmd.exe with the caller's token and did
    /// not establish an AppContainer boundary.
    pub fn is_available() -> bool {
        false
    }

    /// Grants AppContainer SID filesystem ACLs to the target workspace path.
    pub fn build_icacls_grant_command(
        appcontainer_sid: &str,
        workspace: &Path,
        read_only: bool,
    ) -> Command {
        let mut cmd = Command::new("icacls");
        let perm = if read_only {
            "(OI)(CI)(RX)"
        } else {
            "(OI)(CI)(M)"
        };
        let grant_arg = format!("*\\{}:{}", appcontainer_sid, perm);
        cmd.arg(workspace.to_string_lossy().to_string())
            .arg("/grant")
            .arg(grant_arg);
        cmd
    }

    /// Executes a command within the AppContainer boundary.
    pub async fn execute(
        &self,
        command: &[String],
        working_dir: &Path,
        _timeout: Duration,
        extra_env: &HashMap<String, String>,
    ) -> Result<crate::runner::CommandOutput> {
        if command.is_empty() {
            bail!("Cannot execute empty command vector in AppContainer");
        }

        #[cfg(not(target_os = "windows"))]
        {
            let _ = (working_dir, extra_env);
            bail!(
                "Cannot execute Windows AppContainer sandbox on non-Windows host ({})",
                std::env::consts::OS
            );
        }

        #[cfg(target_os = "windows")]
        {
            self.execute_windows_appcontainer(command, working_dir, _timeout, extra_env)
                .await
        }
    }

    #[cfg(target_os = "windows")]
    async fn execute_windows_appcontainer(
        &self,
        command: &[String],
        working_dir: &Path,
        timeout: Duration,
        extra_env: &HashMap<String, String>,
    ) -> Result<crate::runner::CommandOutput> {
        let _ = (command, working_dir, timeout, extra_env);
        bail!(
            "Windows AppContainer execution is disabled: this build does not create a restricted AppContainer process token. Use a verified MXC backend or choose an explicitly authorized execution mode."
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_appcontainer_plan_read_only() {
        let ws = Path::new("C:\\projects\\app");
        let sandbox = AppContainerSandbox::new(SandboxPolicy::ReadOnly, ws).unwrap();
        let plan = sandbox.plan();

        assert!(plan.read_only_workspace);
        assert!(!plan.network_enabled);
        assert!(plan.capability_sids.is_empty());
        assert!(plan.protected_metadata_masks.contains(&".git".to_string()));
        assert!(plan.protected_metadata_masks.contains(&".env".to_string()));
    }

    #[test]
    fn test_appcontainer_plan_workspace_write() {
        let ws = Path::new("C:\\projects\\app");
        let sandbox = AppContainerSandbox::new(SandboxPolicy::WorkspaceWrite, ws).unwrap();
        let plan = sandbox.plan();

        assert!(!plan.read_only_workspace);
        assert!(plan.network_enabled);
        assert_eq!(plan.capability_sids, vec![WIN_CAPABILITY_INTERNET_CLIENT]);
    }

    #[test]
    fn test_icacls_command_formatting() {
        let ws = Path::new("C:\\workspace");
        let sid = "S-1-15-2-12345-67890";
        let cmd = AppContainerSandbox::build_icacls_grant_command(sid, ws, false);
        let args: Vec<_> = cmd
            .get_args()
            .map(|a| a.to_string_lossy().to_string())
            .collect();
        assert!(args.contains(&format!("*\\{}:(OI)(CI)(M)", sid)));
    }
}
