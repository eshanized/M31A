//! Platform-aware path resolution.
//!
//! Canonical authority: [`crate::storage::StorageLayout`]. This module
//! retains its public functions for compatibility and delegates to the
//! single canonical authority (`deployment::DeploymentPaths` / `storage`).

use directories::ProjectDirs;
use std::path::PathBuf;

/// Get the global data directory for M31A.
/// Linux: ~/.local/share/m31a/
/// macOS: ~/Library/Application Support/com.m31a.M31A/
/// Windows: %LOCALAPPDATA%\m31a\data\
/// Production keeps these legacy paths (backward compatible).
pub fn global_data_dir() -> Option<PathBuf> {
    global_data_dir_for_channel(crate::deployment::DeploymentChannel::current())
}

/// Get the global config directory for M31A.
/// Linux: ~/.config/m31a/
/// macOS: ~/Library/Application Support/com.m31a.m31a/
/// Windows: %APPDATA%\m31a\m31a\config\
pub fn global_config_dir() -> Option<PathBuf> {
    global_config_dir_for_channel(crate::deployment::DeploymentChannel::current())
}

/// Channel-aware global data directory. Development resolves to the isolated
/// `m31a-dev` app name so side-by-side installations never share mutable state.
pub fn global_data_dir_for_channel(
    channel: crate::deployment::DeploymentChannel,
) -> Option<PathBuf> {
    ProjectDirs::from("com", "m31a", channel.app_dir_name())
        .map(|dirs| dirs.data_dir().to_path_buf())
}

/// Channel-aware global config directory.
pub fn global_config_dir_for_channel(
    channel: crate::deployment::DeploymentChannel,
) -> Option<PathBuf> {
    ProjectDirs::from("com", "m31a", channel.app_dir_name())
        .map(|dirs| dirs.config_dir().to_path_buf())
}

/// Get the project-local directory for M31A.
/// Always <workspace_root>/.m31a/ — one shared workspace directory for both
/// channels; deployment-scoped state is isolated inside it (see
/// `crate::deployment::DeploymentPaths`).
pub fn project_local_dir(workspace_root: &std::path::Path) -> PathBuf {
    workspace_root.join(".m31a")
}

/// Channel-aware project-local database path (LEGACY migration source).
///
/// Production keeps the legacy `.m31a/m31a.db`; development uses the
/// isolated `.m31a/m31a-dev.db`. NEW code MUST use the canonical global
/// authority (`crate::storage::StorageLayout::global_db_path`), never this.
pub fn project_db_path(
    workspace_root: &std::path::Path,
    channel: crate::deployment::DeploymentChannel,
) -> PathBuf {
    crate::deployment::DeploymentPaths::project_db_path(workspace_root, channel)
}

/// Canonical global database path for a workspace (platform user data).
///
/// Delegates to [`crate::storage::StorageLayout`] so test isolation and
/// channel rules live in exactly one place.
pub fn canonical_db_path_for_workspace(workspace_root: &std::path::Path) -> PathBuf {
    crate::storage::StorageLayout::for_workspace(workspace_root).global_db_path()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_project_local_dir() {
        let root = Path::new("/home/user/project");
        let local = project_local_dir(root);
        assert_eq!(local, Path::new("/home/user/project/.m31a"));
    }
}
