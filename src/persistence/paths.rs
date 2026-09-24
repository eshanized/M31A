//! Platform-aware path resolution

use directories::ProjectDirs;
use std::path::PathBuf;

/// Get the global data directory for M31A.
/// Linux: ~/.local/share/m31a/
/// macOS: ~/Library/Application Support/com.m31a.M31A/
/// Windows: %LOCALAPPDATA%\m31a\data\
pub fn global_data_dir() -> Option<PathBuf> {
    ProjectDirs::from("com", "m31a", "m31a").map(|dirs| dirs.data_dir().to_path_buf())
}

/// Get the global config directory for M31A.
/// Linux: ~/.config/m31a/
/// macOS: ~/Library/Application Support/com.m31a.m31a/
/// Windows: %APPDATA%\m31a\m31a\config\
pub fn global_config_dir() -> Option<PathBuf> {
    ProjectDirs::from("com", "m31a", "m31a").map(|dirs| dirs.config_dir().to_path_buf())
}

/// Get the project-local directory for M31A.
/// Always <workspace_root>/.m31a/
pub fn project_local_dir(workspace_root: &std::path::Path) -> PathBuf {
    workspace_root.join(".m31a")
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
