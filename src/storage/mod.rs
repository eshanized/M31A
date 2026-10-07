//! Canonical M31A Storage Authority (L0-L1).
//!
//! Single authoritative platform-aware path system for every durable,
//! cached, and ephemeral store. All other path helpers
//! (`config::paths::PlatformPaths`, `persistence::paths`,
//! `deployment::DeploymentPaths`) delegate to — or are consumed through —
//! this layout so invalid placement is difficult.
//!
//! ```text
//! M31A_STORAGE
//! ├── User Configuration  (platform config dir / `config.toml`)
//! ├── User Data           (platform data dir / `m31a.db`, artifacts, telemetry)
//! ├── Cache               (platform cache dir / model catalog)
//! ├── Runtime State       (platform state dir / sockets, pid, logs)
//! ├── Credentials         (platform config dir / `credentials.json`, 0600)
//! └── Workspace           (project `.m31a/` — workspace-scoped metadata only)
//! ```
//!
//! Channel isolation: production uses the `m31a` app name, development uses
//! `m31a-dev`. Platform directories already embed the app name, so the same
//! filename in different channel directories is fully isolated.
//!
//! Test isolation: when the workspace lives under the OS temp dir and no
//! explicit `M31A_*_DIR` override is set, global paths are derived from a
//! per-workspace isolated root under the temp dir (outside the workspace).
//! This keeps `cargo test` hermetic without polluting the real user profile.
//! Setting `M31A_CONFIG_DIR` / `M31A_DATA_DIR` / `M31A_CACHE_DIR` /
//! `M31A_STATE_DIR` to a shared tempdir opts into shared global state across
//! workspaces (used by multi-workspace contract tests).

use std::path::{Path, PathBuf};

use crate::deployment::{DeploymentChannel, DeploymentPaths};

/// Storage class for every M31A store.
///
/// Makes invalid placement difficult: callers request a class, the layout
/// resolves the authoritative location. Anything not listed here must be
/// explicitly classified before gaining a path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StorageClass {
    /// User-wide configuration (`config.toml`). Never workspace-local.
    UserConfig,
    /// Durable application data (SQLite, indexes, profiles). Never workspace-local.
    UserData,
    /// Disposable generated state (model catalog). Never workspace-local.
    Cache,
    /// Temporary operational state (sockets, pid, journals). Never workspace-local.
    RuntimeState,
    /// Application logs (platform state/logs). Never workspace-local by default.
    Logs,
    /// User credentials (0600). Never workspace-local, never committed.
    Credentials,
    /// Application/global artifacts. Workspace builds stay workspace-local.
    GlobalArtifacts,
    /// Application-wide telemetry. Workspace mission records stay workspace-scoped.
    Telemetry,
    /// Workspace-scoped configuration (`<ws>/.m31a/config.toml`).
    WorkspaceConfig,
    /// Workspace identity and project metadata (init sentinel, guidance).
    WorkspaceMeta,
    /// Workspace-derived cache (worktrees, derived indexes). Disposable.
    WorkspaceDerived,
}

/// Configuration scope chosen by the user at onboarding / settings time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ConfigScope {
    /// Saved to the M31A user profile (all projects).
    #[default]
    Global,
    /// Saved only for this workspace (`<ws>/.m31a/config.toml`).
    Workspace,
}

impl ConfigScope {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Global => "global",
            Self::Workspace => "workspace",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "global" | "user" | "all" | "all-projects" | "m31a-wide" => Some(Self::Global),
            "workspace" | "project" | "this-workspace" | "local" => Some(Self::Workspace),
            _ => None,
        }
    }
}

impl std::fmt::Display for ConfigScope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Canonical storage layout for one workspace + channel.
///
/// Thin wrapper over [`DeploymentPaths`] with workspace awareness for
/// test isolation and legacy migration. Cheap to clone.
#[derive(Debug, Clone)]
pub struct StorageLayout {
    channel: DeploymentChannel,
    workspace_root: PathBuf,
    deployment: DeploymentPaths,
}

impl StorageLayout {
    pub fn new(workspace_root: impl Into<PathBuf>, channel: DeploymentChannel) -> Self {
        let workspace_root = workspace_root.into();
        let deployment = DeploymentPaths::new(channel);
        Self {
            channel,
            workspace_root,
            deployment,
        }
    }

    /// Layout for a workspace on the running artifact's channel.
    pub fn for_workspace(workspace_root: impl Into<PathBuf>) -> Self {
        let channel = DeploymentChannel::current();
        Self::new(workspace_root, channel)
    }

    pub fn channel(&self) -> DeploymentChannel {
        self.channel
    }

    pub fn workspace_root(&self) -> &Path {
        &self.workspace_root
    }

    pub fn deployment(&self) -> &DeploymentPaths {
        &self.deployment
    }

    // ------------------------------------------------------------------
    // Test-isolation helper
    // ------------------------------------------------------------------

    /// True when an explicit operator override is set for the given pair.
    fn has_override(dev_name: &str, shared_name: &str) -> bool {
        std::env::var(dev_name)
            .map(|v| !v.trim().is_empty())
            .unwrap_or(false)
            || std::env::var(shared_name)
                .map(|v| !v.trim().is_empty())
                .unwrap_or(false)
    }

    /// Whether this workspace should use an isolated global root (test mode).
    ///
    /// Active only when the workspace lives under the OS temp dir and no
    /// explicit `M31A_*_DIR` override is set. Production workspaces (outside
    /// temp) always resolve to the real platform directories.
    fn uses_isolated_global(&self) -> bool {
        if Self::has_override("M31A_DEV_CONFIG_DIR", "M31A_CONFIG_DIR")
            || Self::has_override("M31A_DEV_DATA_DIR", "M31A_DATA_DIR")
            || Self::has_override("M31A_DEV_CACHE_DIR", "M31A_CACHE_DIR")
            || Self::has_override("M31A_DEV_STATE_DIR", "M31A_STATE_DIR")
        {
            return false;
        }
        let tmp = std::env::temp_dir();
        // Workspace under temp dir (covers `tempfile::tempdir()` in tests).
        if self.workspace_root.starts_with(&tmp) {
            return true;
        }
        if let Ok(canon_tmp) = tmp.canonicalize() {
            if self.workspace_root.starts_with(&canon_tmp) {
                return true;
            }
            if let Ok(canon_ws) = self.workspace_root.canonicalize() {
                if canon_ws.starts_with(&canon_tmp) || canon_ws.starts_with(&tmp) {
                    return true;
                }
            }
        }
        // Also recognize workspace roots inside target/tmp or CARGO_TARGET_TMPDIR (common test fixtures)
        if self.workspace_root.to_string_lossy().contains("target/tmp") {
            return true;
        }
        if let Some(cargo_tmp) = std::env::var_os("CARGO_TARGET_TMPDIR") {
            if self.workspace_root.starts_with(cargo_tmp) {
                return true;
            }
        }
        false
    }

    /// Per-workspace isolated global root (outside the workspace, under temp).
    ///
    /// Deterministic per workspace + channel so parallel tests never share
    /// mutable state, while still satisfying "not workspace-rooted".
    fn isolated_global_root(&self) -> PathBuf {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(self.workspace_root.as_os_str().as_encoded_bytes());
        hasher.update(self.channel.app_dir_name().as_bytes());
        let digest = hasher.finalize();
        let hex: String = digest.iter().take(8).map(|b| format!("{b:02x}")).collect();
        std::env::temp_dir().join(format!("m31a-test-global-{hex}"))
    }

    /// Resolve a platform dir, substituting the isolated test root when
    /// [`Self::uses_isolated_global`] is active.
    fn resolve_dir(&self, kind: &str, platform: PathBuf) -> PathBuf {
        if !self.uses_isolated_global() {
            return platform;
        }
        let root = self.isolated_global_root();
        match kind {
            "config" => root.join("config"),
            "data" => root.join("data"),
            "cache" => root.join("cache"),
            "state" => root.join("state"),
            _ => root.join(kind),
        }
    }

    // ------------------------------------------------------------------
    // User / application authorities (canonical)
    // ------------------------------------------------------------------

    pub fn user_config_dir(&self) -> PathBuf {
        let p = self.deployment.config_dir();
        self.resolve_dir("config", p)
    }

    pub fn user_config_file(&self) -> PathBuf {
        self.user_config_dir().join("config.toml")
    }

    pub fn user_data_dir(&self) -> PathBuf {
        let p = self.deployment.data_dir();
        self.resolve_dir("data", p)
    }

    pub fn user_cache_dir(&self) -> PathBuf {
        let p = self.deployment.cache_dir();
        self.resolve_dir("cache", p)
    }

    pub fn user_state_dir(&self) -> PathBuf {
        let p = self.deployment.state_dir();
        self.resolve_dir("state", p)
    }

    /// Canonical production SQLite path (platform user data).
    pub fn global_db_path(&self) -> PathBuf {
        if self.uses_isolated_global() {
            return self.user_data_dir().join("m31a.db");
        }
        self.deployment.global_db_path()
    }

    /// Canonical credential store (platform user config, 0600).
    ///
    /// Channel isolation comes from the channel-specific config dir
    /// (`m31a` vs `m31a-dev`); the filename is stable.
    pub fn global_credentials_file(&self) -> PathBuf {
        self.user_config_dir().join("credentials.json")
    }

    /// Canonical model-catalog cache (platform cache, channel-isolated).
    pub fn global_model_catalog_file(&self) -> PathBuf {
        self.user_cache_dir().join("model_catalog.json")
    }

    /// Canonical global artifacts dir (platform user data).
    pub fn global_artifacts_dir(&self) -> PathBuf {
        self.user_data_dir().join("artifacts")
    }

    /// Canonical global telemetry dir (platform user data).
    pub fn global_telemetry_dir(&self) -> PathBuf {
        self.user_data_dir().join("telemetry")
    }

    /// Canonical global staging dir (platform runtime state, ephemeral).
    pub fn global_staging_dir(&self) -> PathBuf {
        self.user_state_dir().join("staging")
    }

    pub fn log_dir(&self) -> PathBuf {
        if self.uses_isolated_global() {
            return self.user_state_dir().join("logs");
        }
        self.deployment.log_dir()
    }

    pub fn socket_path(&self) -> PathBuf {
        if self.uses_isolated_global() {
            return self
                .user_state_dir()
                .join(format!("{}.sock", self.channel.binary_name()));
        }
        self.deployment.socket_path()
    }

    pub fn pid_file(&self) -> PathBuf {
        if self.uses_isolated_global() {
            return self
                .user_state_dir()
                .join(format!("{}.pid", self.channel.binary_name()));
        }
        self.deployment.pid_file()
    }

    /// Canonical spool dir for background jobs (platform state, pooled).
    pub fn job_spool_dir(&self) -> PathBuf {
        self.user_state_dir().join("spools")
    }

    // ------------------------------------------------------------------
    // Workspace authorities (minimal, genuinely project-scoped)
    // ------------------------------------------------------------------

    /// Workspace root dir (`<ws>/.m31a`). Contains ONLY workspace-scoped
    /// state: config, identity, prompts, worktrees, planning metadata.
    pub fn workspace_dir(&self) -> PathBuf {
        DeploymentPaths::project_root(&self.workspace_root)
    }

    pub fn workspace_config_file(&self) -> PathBuf {
        crate::config::paths::PlatformPaths::workspace_config_file(&self.workspace_root)
    }

    /// Workspace identity sentinel (`<ws>/.m31a/init.json`). Stays local.
    pub fn workspace_init_sentinel(&self) -> PathBuf {
        self.workspace_dir().join("init.json")
    }

    /// Workspace prompt overrides (`<ws>/.m31a/prompts`). Stays local.
    pub fn workspace_prompts_dir(&self) -> PathBuf {
        self.workspace_dir().join("prompts")
    }

    /// Workspace worktrees root. Stays local (project execution isolation).
    pub fn workspace_worktrees_dir(&self) -> PathBuf {
        self.workspace_dir().join("worktrees")
    }

    /// Workspace-specific artifacts (project build/research outputs).
    pub fn workspace_artifacts_dir(&self) -> PathBuf {
        DeploymentPaths::project_artifacts_dir(&self.workspace_root, self.channel)
    }

    /// Workspace-specific staging (project execution staging).
    pub fn workspace_staging_dir(&self) -> PathBuf {
        DeploymentPaths::project_staging_dir(&self.workspace_root, self.channel)
    }

    /// Classify a path relative to the workspace dir as allowed workspace state.
    ///
    /// Returns false for legacy global stores that must not live in the
    /// workspace by default (`m31a.db`, `credentials*.json`, `logs/`,
    /// `cache/`, sockets, pid files, global telemetry).
    pub fn is_workspace_scoped_file(file_name: &str) -> bool {
        match file_name {
            "config.toml" | "mission.toml" | "policy.toml" | "init.json" => true,
            "m31a.db" | "m31a-dev.db" => false,
            "credentials.json" | "credentials-dev.json" => false,
            _ => {
                if file_name.ends_with(".sock") || file_name.ends_with(".pid") {
                    return false;
                }
                true
            }
        }
    }

    /// Ensure all canonical global directories exist.
    pub fn ensure_global_dirs(&self) -> std::io::Result<()> {
        for dir in [
            self.user_config_dir(),
            self.user_data_dir(),
            self.user_cache_dir(),
            self.user_state_dir(),
            self.global_artifacts_dir(),
            self.global_telemetry_dir(),
            self.global_staging_dir(),
            self.log_dir(),
            self.job_spool_dir(),
        ] {
            std::fs::create_dir_all(&dir)?;
        }
        Ok(())
    }

    /// Ensure the minimal workspace dir exists (identity + config only).
    pub fn ensure_workspace_dir(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(self.workspace_dir())?;
        Ok(())
    }
}

pub mod migration;
pub use migration::{
    MigrationReport, detect_legacy_state, is_legacy_present, migrate_legacy_workspace_state,
};
