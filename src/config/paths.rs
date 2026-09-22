//! Platform-Aware Configuration & Data Directories (CFG-03, D-14).

use directories::{BaseDirs, ProjectDirs};
use std::path::{Path, PathBuf};

/// Platform path resolver supporting Linux (XDG), macOS (Library), and Windows (AppData/ProgramData)
/// with environment variable overrides (M31A_CONFIG_DIR, M31A_DATA_DIR, M31A_CACHE_DIR, M31A_STATE_DIR).
#[derive(Debug, Clone)]
pub struct PlatformPaths {
    project_dirs: Option<ProjectDirs>,
    base_dirs: Option<BaseDirs>,
}

impl Default for PlatformPaths {
    fn default() -> Self {
        Self::new()
    }
}

impl PlatformPaths {
    pub fn new() -> Self {
        Self {
            project_dirs: ProjectDirs::from("com", "m31a", "m31a"),
            base_dirs: BaseDirs::new(),
        }
    }

    /// User configuration directory (e.g. ~/.config/m31a on Linux, ~/Library/Application Support/com.m31a.m31a on macOS, %APPDATA%\m31a\m31a\config on Windows).
    pub fn config_dir(&self) -> PathBuf {
        if let Ok(dir) = std::env::var("M31A_CONFIG_DIR") {
            return PathBuf::from(dir);
        }
        if let Some(ref proj) = self.project_dirs {
            proj.config_dir().to_path_buf()
        } else if let Some(ref base) = self.base_dirs {
            base.config_dir().join("m31a")
        } else {
            PathBuf::from(".m31a/config")
        }
    }

    /// User data directory.
    pub fn data_dir(&self) -> PathBuf {
        if let Ok(dir) = std::env::var("M31A_DATA_DIR") {
            return PathBuf::from(dir);
        }
        if let Some(ref proj) = self.project_dirs {
            proj.data_dir().to_path_buf()
        } else if let Some(ref base) = self.base_dirs {
            base.data_dir().join("m31a")
        } else {
            PathBuf::from(".m31a/data")
        }
    }

    /// Cache directory.
    pub fn cache_dir(&self) -> PathBuf {
        if let Ok(dir) = std::env::var("M31A_CACHE_DIR") {
            return PathBuf::from(dir);
        }
        if let Some(ref proj) = self.project_dirs {
            proj.cache_dir().to_path_buf()
        } else if let Some(ref base) = self.base_dirs {
            base.cache_dir().join("m31a")
        } else {
            PathBuf::from(".m31a/cache")
        }
    }

    /// State directory (runtime state, sockets, pid files).
    pub fn state_dir(&self) -> PathBuf {
        if let Ok(dir) = std::env::var("M31A_STATE_DIR") {
            return PathBuf::from(dir);
        }
        if let Some(ref proj) = self.project_dirs {
            if let Some(state) = proj.state_dir() {
                return state.to_path_buf();
            }
            proj.data_local_dir().join("state")
        } else {
            PathBuf::from(".m31a/state")
        }
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

    /// Default path for user global config file.
    pub fn user_config_file(&self) -> PathBuf {
        let p = self.config_dir().join("config.toml");
        if !p.exists()
            && let Some(home) = std::env::var_os("HOME")
        {
            let legacy = std::path::PathBuf::from(home).join(".config/m31/config.toml");
            if legacy.exists() {
                return legacy;
            }
        }
        p
    }

    /// Default path for system config file.
    pub fn system_config_file(&self) -> PathBuf {
        let p = self.system_config_dir().join("config.toml");
        if !p.exists() {
            let legacy = PathBuf::from("/etc/m31/config.toml");
            if legacy.exists() {
                return legacy;
            }
        }
        p
    }

    /// Workspace config file inside a workspace repository.
    pub fn workspace_config_file(workspace_root: &Path) -> PathBuf {
        let primary = workspace_root.join(".m31a").join("config.toml");
        if !primary.exists() {
            let legacy = workspace_root.join(".m31").join("config.toml");
            if legacy.exists() {
                return legacy;
            }
        }
        primary
    }

    /// Mission config file inside a mission worktree.
    pub fn mission_config_file(worktree_root: &Path) -> PathBuf {
        let primary = worktree_root.join(".m31a").join("mission.toml");
        if !primary.exists() {
            let legacy = worktree_root.join(".m31").join("mission.toml");
            if legacy.exists() {
                return legacy;
            }
        }
        primary
    }
}
