//! Secure environment configuration status (CFG-01, CFG-03).
//!
//! Production credential rule: production code under `src/` MUST NOT discover
//! LLM API keys through the process environment and MUST NOT load `.env`
//! files. The single production credential authority is
//! [`crate::runtime_authorities::resolve_runtime_credentials`] (canonical
//! channel-isolated credential store → legacy workspace migration file, and
//! nothing else).
//!
//! `.env` loading and LLM API-key environment reads for explicitly opted-in
//! live-model testing live exclusively under `tests/support/` (see
//! `tests/support/credentials.rs`). They are never part of the production
//! library.
//!
//! The only environment values consulted here are NON-SECRET, explicitly
//! supported Tier-5 configuration overrides (model selection display only).
//! Secrets (API keys, tokens) are NEVER logged or printed.

use std::path::{Path, PathBuf};

/// Safe diagnostic representation of configured model status without exposing credentials.
#[derive(Debug, Clone)]
pub struct SafeEnvironmentStatus {
    pub provider_configured: bool,
    pub model_configured: String,
    pub api_key_configured: bool,
}

impl SafeEnvironmentStatus {
    /// Detect currently configured production provider and model status safely.
    /// Reflects credentials the production runtime can actually use (stored secure credentials).
    /// Never reads LLM API-key environment variables and never loads `.env`.
    pub fn probe() -> Self {
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        Self::probe_workspace(&cwd)
    }

    /// Detect configured provider and model status for a specific workspace.
    ///
    /// Credential presence comes solely from the canonical credential
    /// resolver. The displayed model name prefers the explicit non-secret
    /// Tier-5 overrides (`M31A_MODEL` / `NVIDIA_MODEL`) and otherwise echoes
    /// the resolved configuration's active model. No credential environment
    /// variable is consulted on any path.
    pub fn probe_workspace(workspace_root: &Path) -> Self {
        let channel = crate::deployment::DeploymentChannel::current();
        let creds =
            crate::runtime_authorities::resolve_runtime_credentials(workspace_root, channel);
        let has_creds = creds.api_key.is_some();

        let model = std::env::var("M31A_MODEL")
            .or_else(|_| std::env::var("NVIDIA_MODEL"))
            .ok()
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| {
                crate::config::ResolvedConfiguration::builder(workspace_root)
                    .build_fallback()
                    .active_model
            });

        Self {
            provider_configured: has_creds,
            model_configured: model,
            api_key_configured: has_creds,
        }
    }

    /// Log safe diagnostic status without printing any credentials.
    pub fn log_status(&self) {
        tracing::debug!(
            provider_configured = self.provider_configured,
            model_configured = %self.model_configured,
            api_key_configured = self.api_key_configured,
            "environment status"
        );
    }
}
