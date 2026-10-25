//! Platform-Aware Configuration & Data Directories (CFG-03, D-14).
//!
//! Compatibility facade over the canonical storage authority ([`crate::storage::StorageLayout`]).
//! All canonical paths, candidates, and channel-conditional policies are delegated to
//! `StorageLayout` and `DeploymentPaths` so that storage placement has a single authoritative policy.

use std::path::{Path, PathBuf};

/// Platform path resolver supporting Linux (XDG), macOS (Library), and Windows (AppData/ProgramData)
/// with environment variable overrides (M31A_CONFIG_DIR, M31A_DATA_DIR, M31A_CACHE_DIR, M31A_STATE_DIR).
///
/// This is a compatibility layer delegating to [`crate::storage::StorageLayout`].
#[derive(Debug, Clone)]
pub struct PlatformPaths {
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

    /// Channel-aware constructor.
    pub fn for_channel(channel: crate::deployment::DeploymentChannel) -> Self {
        Self { channel }
    }

    /// Deployment channel these paths resolve for.
    pub fn channel(&self) -> crate::deployment::DeploymentChannel {
        self.channel
    }

    fn storage_layout(&self) -> crate::storage::StorageLayout {
        let ws = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        crate::storage::StorageLayout::new(ws, self.channel)
    }

    /// User configuration directory (delegates to canonical `StorageLayout`).
    pub fn config_dir(&self) -> PathBuf {
        self.storage_layout().user_config_dir()
    }

    /// User data directory (delegates to canonical `StorageLayout`).
    pub fn data_dir(&self) -> PathBuf {
        self.storage_layout().user_data_dir()
    }

    /// Cache directory (delegates to canonical `StorageLayout`).
    pub fn cache_dir(&self) -> PathBuf {
        self.storage_layout().user_cache_dir()
    }

    /// State directory (delegates to canonical `StorageLayout`).
    pub fn state_dir(&self) -> PathBuf {
        self.storage_layout().user_state_dir()
    }

    /// System configuration directory (/etc/m31a on Unix, ProgramData\m31a on Windows).
    pub fn system_config_dir(&self) -> PathBuf {
        self.storage_layout().system_config_dir()
    }

    /// Canonical user global config file (`config.md`).
    pub fn canonical_user_config_file(&self) -> PathBuf {
        self.storage_layout().canonical_user_config_file()
    }

    /// Candidate paths for user global config file in priority order (canonical, then legacy fallbacks).
    pub fn user_config_candidates(&self) -> Vec<PathBuf> {
        self.storage_layout().user_config_candidates()
    }

    /// Default path for user global config file (canonical Markdown authority).
    pub fn user_config_file(&self) -> PathBuf {
        self.storage_layout().user_config_file()
    }

    /// Canonical system config file (`config.md`).
    pub fn canonical_system_config_file(&self) -> PathBuf {
        self.storage_layout().canonical_system_config_file()
    }

    /// Candidate paths for system config file in priority order (canonical, then legacy fallbacks).
    pub fn system_config_candidates(&self) -> Vec<PathBuf> {
        self.storage_layout().system_config_candidates()
    }

    /// Default path for system config file (canonical Markdown authority).
    pub fn system_config_file(&self) -> PathBuf {
        self.storage_layout().system_config_file()
    }

    /// Canonical workspace config file (.m31a/config.md).
    pub fn canonical_workspace_config_file(workspace_root: &Path) -> PathBuf {
        crate::storage::StorageLayout::for_workspace(workspace_root)
            .canonical_workspace_config_file()
    }

    /// Candidate paths for workspace config in priority order (canonical, then legacy fallbacks).
    pub fn workspace_config_candidates(workspace_root: &Path) -> Vec<PathBuf> {
        crate::storage::StorageLayout::for_workspace(workspace_root).workspace_config_candidates()
    }

    /// Workspace config file inside a workspace repository (canonical Markdown authority).
    pub fn workspace_config_file(workspace_root: &Path) -> PathBuf {
        crate::storage::StorageLayout::for_workspace(workspace_root).workspace_config_file()
    }

    /// Candidate paths for mission configuration in priority order.
    pub fn mission_config_candidates(worktree_root: &Path) -> Vec<PathBuf> {
        crate::storage::StorageLayout::for_workspace(worktree_root).mission_config_candidates()
    }

    /// Mission config file inside a mission worktree (canonical Markdown authority).
    pub fn mission_config_file(worktree_root: &Path) -> PathBuf {
        crate::storage::StorageLayout::for_workspace(worktree_root).mission_config_file()
    }

    /// Candidate paths for system admin policy in priority order (canonical, then legacy).
    pub fn system_policy_candidates(&self) -> Vec<PathBuf> {
        self.storage_layout().system_policy_candidates()
    }

    /// Resolved system admin policy file (canonical Markdown authority).
    pub fn system_policy_file(&self) -> PathBuf {
        self.storage_layout().system_policy_file()
    }

    /// Candidate paths for organization policy in priority order (canonical, then legacy).
    pub fn organization_policy_candidates(&self) -> Vec<PathBuf> {
        self.storage_layout().organization_policy_candidates()
    }

    /// Resolved organization policy file (canonical Markdown authority).
    pub fn organization_policy_file(&self) -> PathBuf {
        self.storage_layout().organization_policy_file()
    }

    /// Candidate paths for workspace policy in priority order (canonical, then legacy).
    pub fn workspace_policy_candidates(workspace_root: &Path) -> Vec<PathBuf> {
        crate::storage::StorageLayout::for_workspace(workspace_root).workspace_policy_candidates()
    }

    /// Resolved workspace policy file (canonical Markdown authority).
    pub fn workspace_policy_file(workspace_root: &Path) -> PathBuf {
        crate::storage::StorageLayout::for_workspace(workspace_root).workspace_policy_file()
    }

    /// Candidate paths for user policy in priority order (canonical, then legacy).
    pub fn user_policy_candidates(&self) -> Vec<PathBuf> {
        self.storage_layout().user_policy_candidates()
    }

    /// Resolved user policy file (canonical Markdown authority).
    pub fn user_policy_file(&self) -> PathBuf {
        self.storage_layout().user_policy_file()
    }
}
