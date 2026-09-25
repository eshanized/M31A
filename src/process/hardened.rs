//! Canonical hardened process-spawn helper.
//!
//! Single choke point for production process execution outside the
//! capability-provider path:
//!
//! ```text
//! input
//!   → working-directory validation (containment)
//!   → command safety validation (protected paths, git redirection)
//!   → environment scrubbing (deny-by-default, secrets never inherited)
//!   → execution (caller owns timeout / cancellation / sandbox)
//! ```
//!
//! There is no second process-sandbox implementation: sandbox confinement and
//! resource limits reuse [`crate::sandbox`] and [`EnvironmentBuilder`].

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::process::env::{EnvironmentBuilder, ProcessSecurityViolation};

/// Failure building a hardened child command. Never a silent fallback: every
/// variant names the rejected input.
#[derive(Debug, thiserror::Error)]
pub enum HardenedSpawnError {
    #[error("process security violation: {0}")]
    Security(#[from] ProcessSecurityViolation),
    #[error("forbidden environment variable '{0}': {1}")]
    ForbiddenEnv(String, String),
}

/// Canonical hardened spawn configuration rooted in an authorized workspace.
#[derive(Debug, Clone)]
pub struct HardenedSpawn {
    workspace_root: PathBuf,
    extra_env: HashMap<String, String>,
}

impl HardenedSpawn {
    /// Root hardened spawning in the authorized task workspace.
    pub fn new(workspace_root: impl Into<PathBuf>) -> Self {
        Self {
            workspace_root: workspace_root.into(),
            extra_env: HashMap::new(),
        }
    }

    /// Add one explicit authorized environment variable. Fails closed when
    /// the key matches forbidden credential/loader/git-redirection patterns.
    pub fn with_env_var(
        mut self,
        key: impl Into<String>,
        value: impl Into<String>,
    ) -> Result<Self, HardenedSpawnError> {
        let key_str = key.into();
        let mut probe = EnvironmentBuilder::new(&self.workspace_root);
        probe
            .set_var(&key_str, value.into())
            .map_err(|e| HardenedSpawnError::ForbiddenEnv(key_str.clone(), e))?;
        let map = probe.build_map();
        if let Some(v) = map.get(&key_str) {
            self.extra_env.insert(key_str, v.clone());
        }
        Ok(self)
    }

    /// Build a scrubbed child command: validates the working directory
    /// against the workspace root, validates command safety, strips the
    /// entire inherited host environment, then applies only the trusted
    /// baseline plus explicit extras.
    pub fn build_command(
        &self,
        program: &str,
        args: &[String],
        working_dir: Option<&Path>,
    ) -> Result<tokio::process::Command, HardenedSpawnError> {
        if program.trim().is_empty() {
            return Err(HardenedSpawnError::Security(
                ProcessSecurityViolation::ProtectedPathAccess("empty program name".to_string()),
            ));
        }
        let valid_cwd =
            crate::process::env::validate_working_directory(working_dir, &self.workspace_root)?;
        crate::process::env::check_command_safety(program, args)?;

        let mut builder = EnvironmentBuilder::new(&self.workspace_root);
        for (k, v) in &self.extra_env {
            builder
                .set_var(k, v)
                .map_err(|e| HardenedSpawnError::ForbiddenEnv(k.clone(), e))?;
        }
        let mut cmd = tokio::process::Command::new(program);
        cmd.env_clear();
        cmd.envs(builder.build_map());
        cmd.current_dir(valid_cwd);
        cmd.args(args);
        cmd.kill_on_drop(true);
        Ok(cmd)
    }

    /// Access the authorized workspace root.
    pub fn workspace_root(&self) -> &Path {
        &self.workspace_root
    }
}
