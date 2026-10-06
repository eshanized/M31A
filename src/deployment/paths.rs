//! Centralized deployment-aware path/profile abstraction.
//!
//! All channel-conditional path logic lives here. The rest of the runtime
//! consumes canonical paths from `DeploymentPaths` — no scattered
//! `if development { ... } else { ... }` at callsites.
//!
//! Principles:
//! - Global (user) state is fully isolated: `m31a` vs `m31a-dev` app names,
//!   so config/data/cache/state/runtime files never collide.
//! - Project-local `.m31a/` is a shared workspace directory by default for
//!   source metadata (init, repo knowledge), with deployment-scoped
//!   runtime/session state isolated underneath it. Production keeps legacy
//!   filenames (backward compatible); development uses `-dev` suffixed
//!   siblings so it can never corrupt production state.

use std::path::{Path, PathBuf};

use directories::{BaseDirs, ProjectDirs};

use super::channel::DeploymentChannel;

/// Channel-aware platform paths. Immutable; cheap to clone.
#[derive(Debug, Clone)]
pub struct DeploymentPaths {
    channel: DeploymentChannel,
    project_dirs: Option<ProjectDirs>,
    base_dirs: Option<BaseDirs>,
}

impl DeploymentPaths {
    pub fn new(channel: DeploymentChannel) -> Self {
        let qualifier: &str = "com";
        let (org, app) = match channel {
            DeploymentChannel::Production => ("m31a", "m31a"),
            DeploymentChannel::Development => ("m31a", "m31a-dev"),
        };
        Self {
            channel,
            project_dirs: ProjectDirs::from(qualifier, org, app),
            base_dirs: BaseDirs::new(),
        }
    }

    /// Paths for the running artifact's channel.
    pub fn current() -> Self {
        Self::new(DeploymentChannel::current())
    }

    pub fn channel(&self) -> DeploymentChannel {
        self.channel
    }

    /// Env override names are channel-scoped first (`M31A_DEV_CONFIG_DIR`),
    /// falling back to the legacy shared names (`M31A_CONFIG_DIR`) so
    /// explicit operator overrides keep working.
    fn env_override(
        dev_name: &str,
        shared_name: &str,
        channel: DeploymentChannel,
    ) -> Option<PathBuf> {
        if channel.is_development()
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

    pub fn config_dir(&self) -> PathBuf {
        if let Some(p) = Self::env_override("M31A_DEV_CONFIG_DIR", "M31A_CONFIG_DIR", self.channel)
        {
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

    pub fn data_dir(&self) -> PathBuf {
        if let Some(p) = Self::env_override("M31A_DEV_DATA_DIR", "M31A_DATA_DIR", self.channel) {
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

    pub fn cache_dir(&self) -> PathBuf {
        if let Some(p) = Self::env_override("M31A_DEV_CACHE_DIR", "M31A_CACHE_DIR", self.channel) {
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

    pub fn state_dir(&self) -> PathBuf {
        if let Some(p) = Self::env_override("M31A_DEV_STATE_DIR", "M31A_STATE_DIR", self.channel) {
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

    pub fn user_config_file(&self) -> PathBuf {
        self.config_dir().join("config.toml")
    }

    /// SQLite database path (global). Per-channel file isolation.
    ///
    /// Canonical application database authority. Channel isolation comes
    /// from the channel-specific data dir (`m31a` vs `m31a-dev`); the
    /// filename is stable. This is the ONLY production database location;
    /// `project_db_path` below is legacy (migration source only).
    pub fn global_db_path(&self) -> PathBuf {
        self.data_dir().join("m31a.db")
    }

    /// Global credential store (platform user config, 0600).
    ///
    /// Canonical credential authority. Channel isolation comes from the
    /// channel-specific config dir; the filename is stable. This replaces
    /// `project_credentials_file` as the primary store.
    pub fn global_credentials_file(&self) -> PathBuf {
        self.config_dir().join("credentials.json")
    }

    /// Global model-catalog cache (platform cache, channel-isolated via dir).
    pub fn global_model_catalog_file(&self) -> PathBuf {
        self.cache_dir().join("model_catalog.json")
    }

    /// Global artifacts dir (platform user data).
    ///
    /// Application/global artifacts belong here. Workspace-specific
    /// build/research outputs may remain under `project_artifacts_dir`.
    pub fn global_artifacts_dir(&self) -> PathBuf {
        self.data_dir().join("artifacts")
    }

    /// Global telemetry dir (platform user data).
    ///
    /// Application-wide telemetry belongs here. Workspace mission records
    /// may remain under `project_telemetry_dir` where genuinely
    /// project-scoped.
    pub fn global_telemetry_dir(&self) -> PathBuf {
        self.data_dir().join("telemetry")
    }

    /// Global staging dir (platform runtime state, ephemeral).
    pub fn global_staging_dir(&self) -> PathBuf {
        self.state_dir().join("staging")
    }

    /// Global job spool dir (platform runtime state, pooled).
    pub fn global_spool_dir(&self) -> PathBuf {
        self.state_dir().join("spools")
    }

    /// Runtime socket path — per-channel filename prevents PID/socket
    /// collision for side-by-side installations.
    pub fn socket_path(&self) -> PathBuf {
        self.state_dir()
            .join(format!("{}.sock", self.channel.binary_name()))
    }

    /// PID file — per-channel filename prevents lock collision.
    pub fn pid_file(&self) -> PathBuf {
        self.state_dir()
            .join(format!("{}.pid", self.channel.binary_name()))
    }

    pub fn log_dir(&self) -> PathBuf {
        self.state_dir().join("logs")
    }

    // ------------------------------------------------------------------
    // Project-local `.m31a/` semantics (LEGACY + workspace-scoped only)
    // ------------------------------------------------------------------
    //
    // These remain for (a) legacy migration sources (db, credentials,
    // catalog) and (b) genuinely workspace-scoped state (worktrees, prompts,
    // workspace artifacts/staging where project-semantics require it).
    // NEW code MUST use the global authorities above for user/application
    // state (db, credentials, cache, logs, runtime sockets/pid). The
    // canonical resolver is `crate::storage::StorageLayout`.
    //
    // `project_db_path` / `project_credentials_file` are LEGACY migration
    // sources — never the primary production authority.

    /// Project-local root. ALWAYS `<workspace>/.m31a` for both channels —
    /// the workspace owns one shared directory; isolation happens inside it.
    pub fn project_root(workspace_root: &Path) -> PathBuf {
        workspace_root.join(".m31a")
    }

    /// Project-local SQLite path. Production keeps the legacy
    /// `.m31a/m31a.db` (backward compatible); development uses the isolated
    /// sibling `.m31a/m31a-dev.db`.
    pub fn project_db_path(workspace_root: &Path, channel: DeploymentChannel) -> PathBuf {
        match channel {
            DeploymentChannel::Production => workspace_root.join(".m31a").join("m31a.db"),
            DeploymentChannel::Development => workspace_root.join(".m31a").join("m31a-dev.db"),
        }
    }

    /// Project-local credentials file. NEVER auto-copied between channels.
    pub fn project_credentials_file(workspace_root: &Path, channel: DeploymentChannel) -> PathBuf {
        match channel {
            DeploymentChannel::Production => workspace_root.join(".m31a").join("credentials.json"),
            DeploymentChannel::Development => {
                workspace_root.join(".m31a").join("credentials-dev.json")
            }
        }
    }

    /// Project-local artifact directory. Production keeps the legacy
    /// `.m31a/artifacts` (backward compatible); development uses the isolated
    /// sibling `.m31a/artifacts-dev` so runs can never corrupt production artifacts.
    pub fn project_artifacts_dir(workspace_root: &Path, channel: DeploymentChannel) -> PathBuf {
        match channel {
            DeploymentChannel::Production => workspace_root.join(".m31a").join("artifacts"),
            DeploymentChannel::Development => workspace_root.join(".m31a").join("artifacts-dev"),
        }
    }

    /// Project-local telemetry directory. Production keeps the legacy
    /// `.m31a/telemetry`; development uses `.m31a/telemetry-dev`.
    pub fn project_telemetry_dir(workspace_root: &Path, channel: DeploymentChannel) -> PathBuf {
        match channel {
            DeploymentChannel::Production => workspace_root.join(".m31a").join("telemetry"),
            DeploymentChannel::Development => workspace_root.join(".m31a").join("telemetry-dev"),
        }
    }

    /// Project-local staging directory. Production keeps the legacy
    /// `.m31a/staging`; development uses `.m31a/staging-dev`.
    pub fn project_staging_dir(workspace_root: &Path, channel: DeploymentChannel) -> PathBuf {
        match channel {
            DeploymentChannel::Production => workspace_root.join(".m31a").join("staging"),
            DeploymentChannel::Development => workspace_root.join(".m31a").join("staging-dev"),
        }
    }

    /// Deployment-scoped runtime/session state inside the shared project
    /// dir: `.m31a/state/<channel>/`. Shared source metadata (init.json,
    /// repo knowledge) stays at the top level.
    pub fn project_state_dir(workspace_root: &Path, channel: DeploymentChannel) -> PathBuf {
        workspace_root
            .join(".m31a")
            .join("state")
            .join(channel.as_str())
    }

    /// Per-channel socket inside the project dir (side-by-side safety).
    pub fn project_socket_path(workspace_root: &Path, channel: DeploymentChannel) -> PathBuf {
        Self::project_state_dir(workspace_root, channel)
            .join(format!("{}.sock", channel.binary_name()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn global_dirs_differ_by_channel() {
        // Force deterministic comparison via app-dir names even when
        // ProjectDirs is unavailable on the test host.
        let prod = DeploymentPaths::new(DeploymentChannel::Production);
        let dev = DeploymentPaths::new(DeploymentChannel::Development);
        assert_ne!(prod.channel.app_dir_name(), dev.channel.app_dir_name());
        // ProjectDirs-based paths embed the app name when available.
        if prod.project_dirs.is_some() {
            assert_ne!(prod.config_dir(), dev.config_dir());
            assert_ne!(prod.data_dir(), dev.data_dir());
            assert_ne!(prod.cache_dir(), dev.cache_dir());
            assert_ne!(prod.state_dir(), dev.state_dir());
        }
        assert_ne!(prod.socket_path(), dev.socket_path());
        assert_ne!(prod.pid_file(), dev.pid_file());
    }

    #[test]
    fn project_root_shared_but_state_isolated() {
        let ws = Path::new("/tmp/ws");
        assert_eq!(DeploymentPaths::project_root(ws), ws.join(".m31a"));
        // Production keeps legacy DB name (compat); dev is isolated.
        assert_eq!(
            DeploymentPaths::project_db_path(ws, DeploymentChannel::Production),
            ws.join(".m31a").join("m31a.db")
        );
        assert_eq!(
            DeploymentPaths::project_db_path(ws, DeploymentChannel::Development),
            ws.join(".m31a").join("m31a-dev.db")
        );
        assert_ne!(
            DeploymentPaths::project_state_dir(ws, DeploymentChannel::Production),
            DeploymentPaths::project_state_dir(ws, DeploymentChannel::Development)
        );
        assert_ne!(
            DeploymentPaths::project_credentials_file(ws, DeploymentChannel::Production),
            DeploymentPaths::project_credentials_file(ws, DeploymentChannel::Development)
        );
    }
}
