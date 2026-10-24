//! Platform-Aware Configuration & Data Directories (CFG-03, D-14).
//!
//! Compatibility layer over the canonical storage authority
//! ([`crate::storage::StorageLayout`] / [`crate::deployment::DeploymentPaths`]).
//! Global directory resolution delegates to `DeploymentPaths`; only legacy
//! fallback file locations (`.m31`, `/etc/m31`) remain here as explicit
//! migration boundaries.

use std::path::{Path, PathBuf};

/// Platform path resolver supporting Linux (XDG), macOS (Library), and Windows (AppData/ProgramData)
/// with environment variable overrides (M31A_CONFIG_DIR, M31A_DATA_DIR, M31A_CACHE_DIR, M31A_STATE_DIR).
#[derive(Debug, Clone)]
pub struct PlatformPaths {
    deployment: crate::deployment::DeploymentPaths,
    channel: crate::deployment::DeploymentChannel,
}

impl Default for PlatformPaths {
    fn default() -> Self {
        Self::new()
    }
}

impl PlatformPaths {
    pub fn new() -> Self {
        Self::for_channel(crate::deployment::DeploymentChannel::current())
    }

    /// Channel-aware constructor. Delegates global directory policy to the
    /// canonical [`crate::deployment::DeploymentPaths`].
    pub fn for_channel(channel: crate::deployment::DeploymentChannel) -> Self {
        Self {
            deployment: crate::deployment::DeploymentPaths::new(channel),
            channel,
        }
    }

    /// Deployment channel these paths resolve for.
    pub fn channel(&self) -> crate::deployment::DeploymentChannel {
        self.channel
    }

    fn deployment_paths(&self) -> &crate::deployment::DeploymentPaths {
        &self.deployment
    }

    /// User configuration directory (delegates to canonical `DeploymentPaths`).
    pub fn config_dir(&self) -> PathBuf {
        self.deployment_paths().config_dir()
    }

    /// User data directory (delegates to canonical `DeploymentPaths`).
    pub fn data_dir(&self) -> PathBuf {
        self.deployment_paths().data_dir()
    }

    /// Cache directory (delegates to canonical `DeploymentPaths`).
    pub fn cache_dir(&self) -> PathBuf {
        self.deployment_paths().cache_dir()
    }

    /// State directory (delegates to canonical `DeploymentPaths`).
    pub fn state_dir(&self) -> PathBuf {
        self.deployment_paths().state_dir()
    }

    /// System configuration directory (/etc/m31a on Unix, ProgramData\m31a on Windows).
    pub fn system_config_dir(&self) -> PathBuf {
        if let Ok(dir) = std::env::var("M31A_SYSTEM_CONFIG_DIR") {
            return PathBuf::from(dir);
        }
        #[cfg(windows)]
        {
            if let Ok(prog_data) = std::env::var("ProgramData") {
                PathBuf::from(prog_data).join("m31a")
            } else {
                PathBuf::from(r"C:\ProgramData\m31a")
            }
        }
        #[cfg(not(windows))]
        {
            PathBuf::from("/etc/m31a")
        }
    }

    /// Canonical user global config file (`config.md`).
    pub fn canonical_user_config_file(&self) -> PathBuf {
        self.config_dir().join("config.md")
    }

    /// Candidate paths for user global config file in priority order (canonical, then legacy fallbacks).
    pub fn user_config_candidates(&self) -> Vec<PathBuf> {
        let mut candidates = vec![
            self.config_dir().join("config.md"),
            self.config_dir().join("settings.md"),
            self.config_dir().join("config.toml"),
        ];
        if let Some(home) = std::env::var_os("HOME") {
            let home_path = PathBuf::from(home);
            let m31a_path = home_path.join(".config/m31a/config.toml");
            let m31_path = home_path.join(".config/m31/config.toml");
            if !candidates.contains(&m31a_path) {
                candidates.push(m31a_path);
            }
            if !candidates.contains(&m31_path) {
                candidates.push(m31_path);
            }
        }
        candidates
    }

    /// Default path for user global config file (resolves canonical Markdown first).
    pub fn user_config_file(&self) -> PathBuf {
        let candidates = self.user_config_candidates();
        let mut found_md: Option<PathBuf> = None;
        let mut second_md: Option<PathBuf> = None;
        let mut found_legacy: Option<PathBuf> = None;

        for candidate in candidates {
            if candidate.exists() {
                if candidate.extension().is_some_and(|e| e == "md") {
                    if found_md.is_none() {
                        found_md = Some(candidate);
                    } else if second_md.is_none() {
                        second_md = Some(candidate);
                    }
                } else if found_legacy.is_none() {
                    found_legacy = Some(candidate);
                }
            }
        }

        if let Some(md) = found_md {
            if let Some(other_md) = second_md {
                tracing::warn!(
                    "Both '{}' and '{}' user configurations exist. '{}' took precedence.",
                    md.display(),
                    other_md.display(),
                    md.display()
                );
            }
            if let Some(legacy) = found_legacy {
                tracing::warn!(
                    "Both Markdown ('{}') and legacy TOML ('{}') user configurations exist. Markdown took precedence.",
                    md.display(),
                    legacy.display()
                );
            }
            return md;
        }

        if let Some(legacy) = found_legacy {
            return legacy;
        }

        self.config_dir().join("config.md")
    }

    /// Canonical system config file (`config.md`).
    pub fn canonical_system_config_file(&self) -> PathBuf {
        self.system_config_dir().join("config.md")
    }

    /// Candidate paths for system config file in priority order (canonical, then legacy fallbacks).
    pub fn system_config_candidates(&self) -> Vec<PathBuf> {
        vec![
            self.system_config_dir().join("config.md"),
            self.system_config_dir().join("config.toml"),
            PathBuf::from("/etc/m31/config.toml"),
        ]
    }

    /// Default path for system config file (resolves canonical Markdown first).
    pub fn system_config_file(&self) -> PathBuf {
        let candidates = self.system_config_candidates();
        let mut found_md: Option<PathBuf> = None;
        let mut found_legacy: Option<PathBuf> = None;

        for candidate in candidates {
            if candidate.exists() {
                if candidate.extension().is_some_and(|e| e == "md") {
                    if found_md.is_none() {
                        found_md = Some(candidate);
                    }
                } else if found_legacy.is_none() {
                    found_legacy = Some(candidate);
                }
            }
        }

        if let Some(md) = found_md {
            if let Some(legacy) = found_legacy {
                tracing::warn!(
                    "Both Markdown ('{}') and legacy TOML ('{}') system configurations exist. Markdown took precedence.",
                    md.display(),
                    legacy.display()
                );
            }
            return md;
        }

        if let Some(legacy) = found_legacy {
            return legacy;
        }

        self.system_config_dir().join("config.md")
    }

    /// Canonical workspace config file (.m31a/config.md).
    pub fn canonical_workspace_config_file(workspace_root: &Path) -> PathBuf {
        workspace_root.join(".m31a").join("config.md")
    }

    /// Candidate paths for workspace config in priority order (canonical, then legacy fallbacks).
    pub fn workspace_config_candidates(workspace_root: &Path) -> Vec<PathBuf> {
        vec![
            workspace_root.join(".m31a").join("config.md"),
            workspace_root.join(".m31a").join("settings.md"),
            workspace_root.join(".m31a").join("M31A.toml"),
            workspace_root.join(".m31a").join("config.toml"),
            workspace_root.join(".m31").join("config.toml"),
        ]
    }

    /// Workspace config file inside a workspace repository.
    /// Resolves canonical `.m31a/config.md` first, falling back to legacy `.m31a/M31A.toml`, `.m31a/config.toml` or `.m31/config.toml`.
    pub fn workspace_config_file(workspace_root: &Path) -> PathBuf {
        let candidates = Self::workspace_config_candidates(workspace_root);
        let mut found_md: Option<PathBuf> = None;
        let mut second_md: Option<PathBuf> = None;
        let mut found_legacy: Option<PathBuf> = None;

        for candidate in candidates {
            if candidate.exists() {
                if candidate.extension().is_some_and(|e| e == "md") {
                    if found_md.is_none() {
                        found_md = Some(candidate);
                    } else if second_md.is_none() {
                        second_md = Some(candidate);
                    }
                } else if found_legacy.is_none() {
                    found_legacy = Some(candidate);
                }
            }
        }

        if let Some(md) = found_md {
            if let Some(other_md) = second_md {
                tracing::warn!(
                    "Both '{}' and '{}' workspace configurations exist. '{}' took precedence.",
                    md.display(),
                    other_md.display(),
                    md.display()
                );
            }
            if let Some(legacy) = found_legacy {
                tracing::warn!(
                    "Both Markdown ('{}') and legacy TOML ('{}') workspace configurations exist. Markdown took precedence.",
                    md.display(),
                    legacy.display()
                );
            }
            return md;
        }

        if let Some(legacy) = found_legacy {
            return legacy;
        }

        Self::canonical_workspace_config_file(workspace_root)
    }

    /// Candidate paths for mission configuration in priority order.
    pub fn mission_config_candidates(worktree_root: &Path) -> Vec<PathBuf> {
        vec![
            worktree_root.join(".m31a").join("mission.md"),
            worktree_root.join(".m31a").join("mission.toml"),
            worktree_root.join(".m31").join("mission.toml"),
        ]
    }

    /// Mission config file inside a mission worktree.
    pub fn mission_config_file(worktree_root: &Path) -> PathBuf {
        for candidate in Self::mission_config_candidates(worktree_root) {
            if candidate.exists() {
                return candidate;
            }
        }
        worktree_root.join(".m31a").join("mission.md")
    }

    /// Candidate paths for system admin policy in priority order (canonical, then legacy).
    pub fn system_policy_candidates(&self) -> Vec<PathBuf> {
        vec![
            self.system_config_dir().join("policy.md"),
            self.system_config_dir().join("policy.toml"),
            PathBuf::from("/etc/m31/policy.toml"),
        ]
    }

    /// Resolved system admin policy file (first existing candidate, or canonical primary).
    pub fn system_policy_file(&self) -> PathBuf {
        for candidate in self.system_policy_candidates() {
            if candidate.exists() {
                return candidate;
            }
        }
        self.system_config_dir().join("policy.md")
    }

    /// Candidate paths for organization policy in priority order (canonical, then legacy).
    pub fn organization_policy_candidates(&self) -> Vec<PathBuf> {
        vec![
            self.system_config_dir().join("organization.md"),
            self.system_config_dir().join("organization.toml"),
            PathBuf::from("/etc/m31/organization.toml"),
        ]
    }

    /// Resolved organization policy file (first existing candidate, or canonical primary).
    pub fn organization_policy_file(&self) -> PathBuf {
        for candidate in self.organization_policy_candidates() {
            if candidate.exists() {
                return candidate;
            }
        }
        self.system_config_dir().join("organization.md")
    }

    /// Candidate paths for workspace policy in priority order (canonical, then legacy).
    pub fn workspace_policy_candidates(workspace_root: &Path) -> Vec<PathBuf> {
        vec![
            workspace_root.join(".m31a").join("policy.md"),
            workspace_root.join(".m31a").join("policy.toml"),
            workspace_root.join(".m31").join("policy.toml"),
        ]
    }

    /// Resolved workspace policy file (first existing candidate, or canonical primary).
    pub fn workspace_policy_file(workspace_root: &Path) -> PathBuf {
        for candidate in Self::workspace_policy_candidates(workspace_root) {
            if candidate.exists() {
                return candidate;
            }
        }
        workspace_root.join(".m31a").join("policy.md")
    }

    /// Candidate paths for user policy in priority order (canonical, then legacy).
    pub fn user_policy_candidates(&self) -> Vec<PathBuf> {
        let mut candidates = vec![
            self.config_dir().join("policy.md"),
            self.config_dir().join("policy.toml"),
        ];
        if let Some(home) = std::env::var_os("HOME") {
            let home_path = PathBuf::from(home);
            let m31a_path = home_path.join(".config/m31a/policy.toml");
            let m31_path = home_path.join(".config/m31/policy.toml");
            if !candidates.contains(&m31a_path) {
                candidates.push(m31a_path);
            }
            if !candidates.contains(&m31_path) {
                candidates.push(m31_path);
            }
        }
        candidates
    }

    /// Resolved user policy file (first existing candidate, or canonical primary).
    pub fn user_policy_file(&self) -> PathBuf {
        for candidate in self.user_policy_candidates() {
            if candidate.exists() {
                return candidate;
            }
        }
        self.config_dir().join("policy.md")
    }
}
