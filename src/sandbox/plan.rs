//! Sandbox execution plan, network confinement modes, and fail-closed validation (SND-02, SND-03, D-05).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use thiserror::Error;

use crate::sandbox::capabilities::SandboxCapabilities;
use crate::sandbox::limits::ResourceLimits;

/// Network isolation policy for a sandboxed execution (D-07).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum NetworkConfinement {
    /// Deny all outbound and inbound networking (--unshare-net).
    #[default]
    Isolated,
    /// Loopback interface only (for local IPC/daemons).
    LoopbackOnly,
    /// Explicit domain name destination allowlist.
    Allowlist { domains: Vec<String> },
}

/// Concrete security or resource violation detected during sandbox execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Error)]
pub enum SandboxViolation {
    #[error("Resource limit '{limit}' exceeded: {detail}")]
    ResourceLimitViolation { limit: String, detail: String },
    #[error("Execution timed out after {duration_ms}ms")]
    Timeout { duration_ms: u64 },
    #[error("Network violation: unauthorized attempt to access {target}")]
    NetworkViolation { target: String },
    #[error("Filesystem violation: unauthorized access to {path}")]
    FilesystemViolation { path: String },
}

/// Typed error returned during sandbox planning, preparation, or execution.
#[derive(Debug, Error)]
pub enum SandboxError {
    #[error("Sandbox capabilities unsatisfied: {0}")]
    CapabilitiesUnsatisfied(String),

    #[error("Sandbox preparation failed: {0}")]
    PreparationFailed(String),

    #[error("Sandbox cleanup failed: {0}")]
    CleanupFailed(String),

    #[error("Sandbox execution failed: {0}")]
    ExecutionFailed(String),

    #[error("Resource exhausted: {0}")]
    ResourceExhausted(String),

    #[error("Sandbox violation: {0}")]
    Violation(#[from] SandboxViolation),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Complete execution specification for an isolated sandboxed task (SND-02).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SandboxPlan {
    /// Authorized task workspace directory.
    pub workspace_root: PathBuf,
    /// Whether filesystem containment is strictly required.
    pub requires_fs_isolation: bool,
    /// Whether network isolation is strictly required.
    pub requires_net_isolation: bool,
    /// Mode of network confinement.
    pub network_confinement: NetworkConfinement,
    /// Resource execution limits.
    pub resource_limits: ResourceLimits,
    /// Filtered environment variables.
    pub env_vars: HashMap<String, String>,
    /// Additional host paths mounted read-only (e.g. toolchains, cache).
    pub extra_ro_mounts: Vec<PathBuf>,
    /// Working directory for command execution (defaults to workspace_root if None).
    #[serde(default)]
    pub working_dir: Option<PathBuf>,
}

impl SandboxPlan {
    /// Create a new default isolated plan for a workspace.
    pub fn new(workspace_root: PathBuf) -> Self {
        Self {
            workspace_root,
            requires_fs_isolation: true,
            requires_net_isolation: true,
            network_confinement: NetworkConfinement::Isolated,
            resource_limits: ResourceLimits::default(),
            env_vars: HashMap::new(),
            extra_ro_mounts: Vec::new(),
            working_dir: None,
        }
    }

    /// Builder: specify target working directory inside workspace.
    pub fn with_working_dir(mut self, working_dir: PathBuf) -> Self {
        self.working_dir = Some(working_dir);
        self
    }

    /// Builder: specify filesystem isolation requirement.
    pub fn with_fs_isolation(mut self, required: bool) -> Self {
        self.requires_fs_isolation = required;
        self
    }

    /// Builder: specify network isolation requirement.
    pub fn with_net_isolation(mut self, required: bool) -> Self {
        self.requires_net_isolation = required;
        self
    }

    /// Builder: set network confinement mode.
    pub fn with_network_confinement(mut self, mode: NetworkConfinement) -> Self {
        self.network_confinement = mode;
        self
    }

    /// Builder: set resource limits.
    pub fn with_resource_limits(mut self, limits: ResourceLimits) -> Self {
        self.resource_limits = limits;
        self
    }

    /// Builder: add environment variable.
    pub fn with_env_var(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env_vars.insert(key.into(), value.into());
        self
    }

    /// Builder: add extra read-only mount.
    pub fn with_extra_ro_mount(mut self, path: PathBuf) -> Self {
        self.extra_ro_mounts.push(path);
        self
    }

    /// Whether an extra read-only mount target is denied by sandbox policy.
    /// Read-only does not imply authorized: credential stores, secret
    /// material, protected runtime state, and repository internals are never
    /// mountable as extra mounts.
    pub fn is_denied_extra_ro_mount(path: &std::path::Path) -> bool {
        let canon = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        let lower = canon.to_string_lossy().to_lowercase();
        // secret / credential stores
        for marker in [
            "credentials.json",
            "credentials-dev.json",
            ".ssh",
            ".aws",
            ".gnupg",
            "id_rsa",
            "id_ed25519",
            ".pki",
            "/etc/shadow",
            "/etc/gshadow",
        ] {
            if lower.contains(marker) {
                return true;
            }
        }
        // protected runtime state: any `.m31a` dir or global m31a state
        if canon
            .components()
            .any(|c| c.as_os_str() == ".m31a" || c.as_os_str() == ".m31")
        {
            return true;
        }
        // repository internals must not be re-mounted via extra mounts
        // (bubblewrap already governs `.git` explicitly as read-only).
        if canon.components().any(|c| c.as_os_str() == ".git") {
            return true;
        }
        false
    }

    /// Validate extra read-only mounts against sandbox authorization rules.
    pub fn validate_extra_ro_mounts(&self) -> Result<(), SandboxError> {
        for mount in &self.extra_ro_mounts {
            if !mount.exists() {
                return Err(SandboxError::PreparationFailed(format!(
                    "extra read-only mount does not exist: {}",
                    mount.display()
                )));
            }
            if Self::is_denied_extra_ro_mount(mount) {
                return Err(SandboxError::PreparationFailed(format!(
                    "extra read-only mount denied by sandbox policy: {}",
                    mount.display()
                )));
            }
            // mount must canonicalize inside host fs (no dangling symlink escape)
            if mount.canonicalize().is_err() {
                return Err(SandboxError::PreparationFailed(format!(
                    "extra read-only mount failed canonicalization: {}",
                    mount.display()
                )));
            }
        }
        Ok(())
    }

    /// Validate plan against provider capabilities.
    ///
    /// Non-negotiable invariant (SND-03, D-05):
    /// Fails closed immediately if any required capability is missing.
    /// Silent downgrades to uncontained execution are strictly prohibited.
    pub fn validate_against(&self, caps: &SandboxCapabilities) -> Result<(), SandboxError> {
        if self.requires_fs_isolation && !caps.fs_isolated_workspace_rw {
            return Err(SandboxError::CapabilitiesUnsatisfied(
                "Filesystem isolation required but unsupported by selected provider".into(),
            ));
        }

        if self.requires_net_isolation && !caps.net_deny_all {
            return Err(SandboxError::CapabilitiesUnsatisfied(
                "Network isolation required but unsupported by selected provider".into(),
            ));
        }

        if matches!(
            self.network_confinement,
            NetworkConfinement::Allowlist { .. }
        ) && !caps.net_allowlist
        {
            return Err(SandboxError::CapabilitiesUnsatisfied(
                "Network allowlist required but unsupported by selected provider".into(),
            ));
        }

        // extra mounts are authorized here (fail-closed before execution).
        self.validate_extra_ro_mounts()?;

        Ok(())
    }
}
