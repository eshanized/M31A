//! Platform-Aware Configuration & Data Directories (CFG-03, D-14).

use directories::{BaseDirs, ProjectDirs};
use std::path::{Path, PathBuf};

/// Platform path resolver supporting Linux (XDG), macOS (Library), and Windows (AppData/ProgramData)
/// with environment variable overrides (M31A_CONFIG_DIR, M31A_DATA_DIR, M31A_CACHE_DIR, M31A_STATE_DIR).
#[derive(Debug, Clone)]
pub struct PlatformPaths {
    project_dirs: Option<ProjectDirs>,
    base_dirs: Option<BaseDirs>,
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

    /// Channel-aware constructor. Production uses the legacy `m31a` app name
    /// (backward compatible); development uses `m31a-dev` for full global
    /// state isolation. All resolution below honors the selected channel.
    pub fn for_channel(channel: crate::deployment::DeploymentChannel) -> Self {
        let app: &str = match channel {
            crate::deployment::DeploymentChannel::Production => "m31a",
            crate::deployment::DeploymentChannel::Development => "m31a-dev",
        };
        Self {
            project_dirs: ProjectDirs::from("com", "m31a", app),
            base_dirs: BaseDirs::new(),
            channel,
        }
    }

    /// Deployment channel these paths resolve for.
    pub fn channel(&self) -> crate::deployment::DeploymentChannel {
        self.channel
    }

    fn channel_env_override(&self, dev_name: &str, shared_name: &str) -> Option<PathBuf> {
        // Channel-scoped override wins for development; shared legacy names
        // keep working as explicit operator overrides for both channels.
        if self.channel.is_development()
            && let Ok(v) = std::env::var(dev_name)
            && !v.trim().is_empty()
        {
            return Some(PathBuf::from(v));
        }
        std::env::var(shared_name)
            .ok()
            .filter(|v| !v.trim().is_empty())
            .map(PathBuf::from)
    }

    /// User configuration directory (e.g. ~/.config/m31a on Linux, ~/Library/Application Support/com.m31a.m31a on macOS, %APPDATA%\m31a\m31a\config on Windows).
    /// Development resolves to the isolated `m31a-dev` app name on all platforms.
    pub fn config_dir(&self) -> PathBuf {
        if let Some(p) = self.channel_env_override("M31A_DEV_CONFIG_DIR", "M31A_CONFIG_DIR") {
            return p;
        }
        if let Some(ref proj) = self.project_dirs {
            proj.config_dir().to_path_buf()
        } else if let Some(ref base) = self.base_dirs {
            base.config_dir().join(self.channel.app_dir_name())
        } else {
            PathBuf::from(".m31a/config")
        }
    }

    /// User data directory.
    pub fn data_dir(&self) -> PathBuf {
        if let Some(p) = self.channel_env_override("M31A_DEV_DATA_DIR", "M31A_DATA_DIR") {
            return p;
        }
        if let Some(ref proj) = self.project_dirs {
            proj.data_dir().to_path_buf()
        } else if let Some(ref base) = self.base_dirs {
            base.data_dir().join(self.channel.app_dir_name())
        } else {
            PathBuf::from(".m31a/data")
        }
    }

    /// Cache directory.
    pub fn cache_dir(&self) -> PathBuf {
        if let Some(p) = self.channel_env_override("M31A_DEV_CACHE_DIR", "M31A_CACHE_DIR") {
            return p;
        }
        if let Some(ref proj) = self.project_dirs {
            proj.cache_dir().to_path_buf()
        } else if let Some(ref base) = self.base_dirs {
            base.cache_dir().join(self.channel.app_dir_name())
        } else {
            PathBuf::from(".m31a/cache")
        }
    }

    /// State directory (runtime state, sockets, pid files).
    pub fn state_dir(&self) -> PathBuf {
        if let Some(p) = self.channel_env_override("M31A_DEV_STATE_DIR", "M31A_STATE_DIR") {
            return p;
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
