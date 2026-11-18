//! Validated runtime configuration for M31A application runtime.

use crate::error::RuntimeError;
use m31a_core::Config;
use m31a_core::{ApprovalPolicy, SandboxPolicy};
use m31a_execpolicy::ExecPolicy;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Validated configuration used to initialize and parameterize an `M31aRuntime`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeConfig {
    /// Underlying core configuration (models, endpoints, credentials, feature flags).
    pub core: Config,
    /// Absolute or canonical workspace working directory root.
    pub cwd: PathBuf,
    /// Effective execution policy governing command executions and tool permissions.
    #[serde(skip, default = "ExecPolicy::standard")]
    pub exec_policy: ExecPolicy,
    /// Optional explicit session ID override.
    pub session_id: Option<String>,
    /// Whether file watching on the workspace root is enabled.
    pub enable_watcher: bool,
    /// Whether the live web search tool is enabled.
    pub enable_web_search: bool,
    /// Optional reasoning effort override ("low", "medium", "high").
    pub reasoning_effort: Option<String>,
}

impl RuntimeConfig {
    /// Create a new `RuntimeConfig` from a core `Config` and target workspace directory.
    pub fn new(core: Config, cwd: PathBuf) -> Self {
        let enable_web_search = core.features.get("web_search").copied().unwrap_or(true);
        let enable_watcher = core.features.get("watcher").copied().unwrap_or(false);
        let reasoning_effort = core.reasoning_effort.clone();

        Self {
            core,
            cwd,
            exec_policy: ExecPolicy::standard(),
            session_id: None,
            enable_watcher,
            enable_web_search,
            reasoning_effort,
        }
    }

    /// Load default configuration from current working directory and user config file.
    pub fn load_default() -> Result<Self, RuntimeError> {
        let cwd = std::env::current_dir().map_err(RuntimeError::Io)?;
        let core = Config::load().map_err(|e| RuntimeError::Config(e.to_string()))?;
        let config = Self::new(core, cwd);
        config.validate()?;
        Ok(config)
    }

    /// Create an isolated test configuration targeting a temporary workspace directory.
    pub fn for_test(mut core: Config, test_dir: &Path) -> Self {
        if core.api_key.is_none() {
            core.api_key = Some("test-credential".to_string());
        }
        if core.model.is_empty() || core.model == "mock-model" {
            core.model = "nvidia/nemotron-3.5-lightning-30b-a3b".to_string();
        }
        if core.model_provider.as_deref() == Some("mock") {
            core.model_provider = Some("nim".to_string());
        }
        Self {
            core,
            cwd: test_dir.to_path_buf(),
            exec_policy: ExecPolicy::standard(),
            session_id: Some("test-session".to_string()),
            enable_watcher: false,
            enable_web_search: false,
            reasoning_effort: None,
        }
    }

    /// Builder method to override sandbox policy.
    pub fn with_sandbox(mut self, sandbox: SandboxPolicy) -> Self {
        self.core.sandbox = sandbox;
        self
    }

    /// Builder method to override approval policy.
    pub fn with_approval(mut self, approval: ApprovalPolicy) -> Self {
        self.core.approval = approval;
        self
    }

    /// Builder method to override model.
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.core.model = model.into();
        self
    }

    /// Builder method to enable or disable web search tool.
    pub fn with_web_search(mut self, enabled: bool) -> Self {
        self.enable_web_search = enabled;
        self
    }

    /// Builder method to enable or disable workspace watcher.
    pub fn with_watcher(mut self, enabled: bool) -> Self {
        self.enable_watcher = enabled;
        self
    }

    /// Builder method to set reasoning effort.
    pub fn with_reasoning_effort(mut self, effort: Option<String>) -> Self {
        self.reasoning_effort = effort;
        self
    }

    /// Validate the runtime configuration.
    pub fn validate(&self) -> Result<(), RuntimeError> {
        if self.core.model.trim().is_empty() {
            return Err(RuntimeError::Config("Model cannot be empty".to_string()));
        }

        if !self.cwd.exists() {
            return Err(RuntimeError::Config(format!(
                "Workspace directory does not exist: {}",
                self.cwd.display()
            )));
        }

        Ok(())
    }
}
