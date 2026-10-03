//! Authoritative deployment context: "What am I?"
//!
//! `DeploymentContext` is immutable runtime metadata stamped at compile time
//! (channel + build facts) combined with the single canonical semantic
//! version (`CARGO_PKG_VERSION`). It is cheap to clone, `Send + Sync`, and
//! performs no filesystem or network I/O — channel resolution never blocks.

use serde::{Deserialize, Serialize};

use super::channel::DeploymentChannel;
use crate::release::version::{PKG_VERSION, RUNTIME_NAME};

/// Compile-time build facts injected by `build.rs`. Explicit `"unknown"`
/// sentinels when facts are unavailable (fail-obvious, never fake-clean).
const BUILD_COMMIT: &str = match option_env!("M31A_GIT_COMMIT") {
    Some(s) => s,
    None => "unknown",
};
const BUILD_BRANCH: &str = match option_env!("M31A_GIT_BRANCH") {
    Some(s) => s,
    None => "unknown",
};
const BUILD_TIMESTAMP: &str = match option_env!("M31A_BUILD_TIMESTAMP") {
    Some(s) => s,
    None => "unknown",
};
const BUILD_TARGET: &str = match option_env!("M31A_TARGET") {
    Some(s) => s,
    None => "unknown",
};
const BUILD_DIRTY_RAW: &str = match option_env!("M31A_GIT_DIRTY") {
    Some(s) => s,
    None => "false",
};
const BUILD_CHANNEL: &str = match option_env!("M31A_CHANNEL") {
    Some(s) => s,
    None => "production",
};

/// Authoritative deployment identity for the running artifact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeploymentContext {
    pub channel: DeploymentChannel,
    /// Canonical semantic package version (== `CARGO_PKG_VERSION`, no suffix).
    pub version: String,
    /// Deterministic build identity: sha256(version|channel|commit|target)[..16].
    /// Same immutable inputs → same ID. Runtime instance UUIDs are separate.
    pub build_id: String,
    pub commit: String,
    pub branch: String,
    pub build_timestamp: String,
    pub target: String,
    pub dirty: bool,
    /// Install-level artifact identity: `{binary}-{version}-{target}`.
    pub artifact_id: String,
}

impl DeploymentContext {
    /// The deployment context for this artifact. Pure + cheap (no I/O).
    pub fn current() -> Self {
        let channel = DeploymentChannel::current();
        let dirty = BUILD_DIRTY_RAW == "true";
        Self::from_parts(
            channel,
            PKG_VERSION,
            BUILD_COMMIT,
            BUILD_BRANCH,
            BUILD_TIMESTAMP,
            BUILD_TARGET,
            dirty,
        )
    }

    /// Construct from explicit parts (deterministic; used by tests and
    /// release tooling to describe a non-running artifact).
    pub fn from_parts(
        channel: DeploymentChannel,
        version: &str,
        commit: &str,
        branch: &str,
        build_timestamp: &str,
        target: &str,
        dirty: bool,
    ) -> Self {
        let build_id = derive_build_id(version, channel.as_str(), commit, target);
        let artifact_id = format!("{}-{version}-{target}", channel.binary_name());
        Self {
            channel,
            version: version.to_string(),
            build_id,
            commit: commit.to_string(),
            branch: branch.to_string(),
            build_timestamp: build_timestamp.to_string(),
            target: target.to_string(),
            dirty,
            artifact_id,
        }
    }

    pub fn is_development(&self) -> bool {
        self.channel.is_development()
    }

    pub fn is_production(&self) -> bool {
        self.channel.is_production()
    }

    /// Short `m31a --version` line. Production: `m31a 0.1.1`.
    /// Development: `m31a-dev 0.1.1-dev+<build12>` — channel is visible
    /// without creating a second semantic package version.
    pub fn cli_version_string(&self) -> String {
        match self.channel {
            DeploymentChannel::Production => format!("{RUNTIME_NAME} {}", self.version),
            DeploymentChannel::Development => {
                let short = self.build_id.chars().take(12).collect::<String>();
                format!(
                    "{} {}-dev+{}",
                    self.channel.binary_name(),
                    self.version,
                    short
                )
            }
        }
    }

    /// One-line cockpit label (unobtrusive): version + channel.
    pub fn cockpit_label(&self) -> String {
        match self.channel {
            DeploymentChannel::Production => format!("v{} PRODUCTION", self.version),
            DeploymentChannel::Development => format!("v{} DEVELOPMENT", self.version),
        }
    }

    /// Multi-line diagnostic report for `m31a doctor` / `version --verbose`.
    /// Never includes secrets.
    pub fn verbose_report(&self) -> String {
        format!(
            "Deployment:\n  Channel: {}\n  Version: {}\n  Target: {}\n  Build: {}\n  Commit: {}\n  Branch: {}\n  Built: {}\n  Dirty: {}\n  Artifact: {}\n  Update channel: {}",
            self.channel,
            self.version,
            self.target,
            self.build_id,
            self.commit,
            self.branch,
            self.build_timestamp,
            self.dirty,
            self.artifact_id,
            self.channel.update_channel(),
        )
    }

    /// JSON-safe diagnostic map (no secrets).
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "channel": self.channel.as_str(),
            "version": self.version,
            "build_id": self.build_id,
            "commit": self.commit,
            "branch": self.branch,
            "build_timestamp": self.build_timestamp,
            "target": self.target,
            "dirty": self.dirty,
            "artifact_id": self.artifact_id,
            "update_channel": self.channel.update_channel().as_str(),
            "binary": self.channel.binary_name(),
        })
    }

    /// Build-time channel marker (for audit: must agree with
    /// `DeploymentChannel::current()`).
    pub fn build_channel_marker() -> &'static str {
        BUILD_CHANNEL
    }
}

/// Deterministic build identity from immutable build metadata.
/// Different commit/target/channel → distinguishable IDs.
pub fn derive_build_id(version: &str, channel: &str, commit: &str, target: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(version.as_bytes());
    h.update(b"|");
    h.update(channel.as_bytes());
    h.update(b"|");
    h.update(commit.as_bytes());
    h.update(b"|");
    h.update(target.as_bytes());
    format!("{:x}", h.finalize())[..16].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_authority_preserved() {
        let ctx = DeploymentContext::current();
        assert_eq!(ctx.version, PKG_VERSION);
        assert_eq!(ctx.version, crate::release::version::runtime_version());
    }

    #[test]
    fn build_channel_marker_agrees_with_current() {
        let marker = DeploymentContext::build_channel_marker();
        assert_eq!(marker, DeploymentContext::current().channel.as_str());
    }

    #[test]
    fn build_id_deterministic_and_discriminating() {
        let a = derive_build_id("0.1.1", "production", "abc", "x86_64-unknown-linux-gnu");
        let b = derive_build_id("0.1.1", "production", "abc", "x86_64-unknown-linux-gnu");
        assert_eq!(a, b);
        assert_eq!(a.len(), 16);
        let c = derive_build_id("0.1.1", "production", "abd", "x86_64-unknown-linux-gnu");
        assert_ne!(a, c);
        let d = derive_build_id("0.1.1", "development", "abc", "x86_64-unknown-linux-gnu");
        assert_ne!(a, d);
        let e = derive_build_id("0.1.1", "production", "abc", "aarch64-apple-darwin");
        assert_ne!(a, e);
    }

    #[test]
    fn cli_version_distinguishes_channel_without_forking_semver() {
        let prod = DeploymentContext::from_parts(
            DeploymentChannel::Production,
            "0.1.1",
            "abc",
            "master",
            "2026-01-01T00:00:00Z",
            "x86_64-unknown-linux-gnu",
            false,
        );
        assert_eq!(prod.cli_version_string(), "m31a 0.1.1");
        assert_eq!(prod.version, "0.1.1");
        let dev = DeploymentContext::from_parts(
            DeploymentChannel::Development,
            "0.1.1",
            "abc",
            "dev",
            "2026-01-01T00:00:00Z",
            "x86_64-unknown-linux-gnu",
            true,
        );
        assert!(dev.cli_version_string().starts_with("m31a-dev 0.1.1-dev+"));
        // Canonical semver untouched in both channels.
        assert_eq!(dev.version, "0.1.1");
    }

    #[test]
    fn verbose_report_has_no_secrets_shape() {
        let ctx = DeploymentContext::current();
        let r = ctx.verbose_report();
        assert!(r.contains("Channel:"));
        assert!(r.contains("Version:"));
        assert!(!r.contains("API_KEY"));
    }
}
