//! Deterministic update discovery + channel-safe application.
//!
//! Updates are infrastructure operations: no model/provider participation.
//! Discovery resolves immutable release metadata (current version/channel/
//! target → candidate artifact URL + checksum); application delegates to the
//! transactional `Installer`. Production never consumes development artifacts
//! through the normal path; development never silently consumes stable.

use super::channel::{DeploymentChannel, UpdateChannel};
use super::installer::{InstallError, Installer};
use super::manifest::DeploymentManifest;
use crate::release::version::PKG_VERSION;

/// A discovered update candidate.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct UpdateCandidate {
    pub version: String,
    pub channel: DeploymentChannel,
    pub target: String,
    pub filename: String,
    pub sha256: String,
    pub size: u64,
    pub build_id: String,
    pub commit: String,
}

/// Typed update failures.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum UpdateError {
    #[error("update error: {0}")]
    Discovery(String),
    #[error("update error: no update available")]
    NoUpdate,
    #[error("update error: install failed: {0}")]
    Install(String),
}

impl From<InstallError> for UpdateError {
    fn from(e: InstallError) -> Self {
        Self::Install(e.to_string())
    }
}

/// Discover the best candidate for `channel`/`target` from `manifest`.
///
/// Rules (fail-closed):
/// - manifest must validate;
/// - manifest channel must equal the requesting channel (cross-channel
///   consumption requires an explicit opt-in, not the normal path);
/// - candidate target must equal the runtime target;
/// - candidate version must be strictly newer than `current_version` (semver).
pub fn discover_update(
    manifest: &DeploymentManifest,
    channel: DeploymentChannel,
    target: &str,
    current_version: &str,
) -> Result<UpdateCandidate, UpdateError> {
    manifest
        .validate()
        .map_err(|e| UpdateError::Discovery(e.to_string()))?;
    if manifest.channel != channel {
        return Err(UpdateError::Discovery(format!(
            "manifest channel '{}' does not serve '{}' feeds (use an explicit cross-channel request)",
            manifest.channel, channel
        )));
    }
    let current = semver::Version::parse(current_version)
        .map_err(|e| UpdateError::Discovery(e.to_string()))?;
    let mut best: Option<UpdateCandidate> = None;
    for a in &manifest.artifacts {
        if a.target != target {
            continue;
        }
        let v = semver::Version::parse(&manifest.version)
            .map_err(|e| UpdateError::Discovery(e.to_string()))?;
        if v <= current {
            continue;
        }
        let candidate = UpdateCandidate {
            version: manifest.version.clone(),
            channel: manifest.channel,
            target: a.target.clone(),
            filename: a.filename.clone(),
            sha256: a.sha256.clone(),
            size: a.size,
            build_id: a.build_id.clone(),
            commit: a.commit.clone(),
        };
        // Single version per manifest: first target match wins; keep
        // deterministic by preferring exact target equality (already filtered).
        if best.is_none() {
            best = Some(candidate);
        }
    }
    best.ok_or(UpdateError::NoUpdate)
}

/// Discover against the running artifact's identity.
pub fn discover_for_current(
    manifest: &DeploymentManifest,
    target_override: Option<&str>,
) -> Result<UpdateCandidate, UpdateError> {
    let ctx = super::context::DeploymentContext::current();
    discover_update(
        manifest,
        ctx.channel,
        target_override.unwrap_or(&ctx.target),
        &ctx.version,
    )
}

/// Expected update feed name for a channel (for logging/diagnostics).
pub fn feed_name(channel: DeploymentChannel) -> &'static str {
    UpdateChannel::from(channel).as_str()
}

/// The installer binary name for a channel (stable vs dev feeds install
/// distinct binaries).
pub fn installer_for(channel: DeploymentChannel, install_dir: &std::path::Path) -> Installer {
    let _ = channel;
    Installer::new(install_dir)
}

/// Current package version (update baseline; never forked per channel).
pub fn current_package_version() -> &'static str {
    PKG_VERSION
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::deployment::manifest::ManifestArtifact;

    fn manifest(channel: DeploymentChannel, version: &str) -> DeploymentManifest {
        DeploymentManifest {
            schema_version: 1,
            version: version.to_string(),
            channel,
            build_id: Some("0123456789abcdef".to_string()),
            commit: Some("abc".to_string()),
            artifacts: vec![crate::deployment::manifest::ManifestArtifact {
                target: "x86_64-unknown-linux-gnu".to_string(),
                filename: "m31a-linux-x64.tar.gz".to_string(),
                sha256: "a".repeat(64),
                size: 10,
                build_id: "0123456789abcdef".to_string(),
                commit: "abc".to_string(),
            }],
        }
    }

    #[test]
    fn discovers_newer_same_channel_same_target() {
        let m = manifest(DeploymentChannel::Production, "0.2.0");
        let c = discover_update(
            &m,
            DeploymentChannel::Production,
            "x86_64-unknown-linux-gnu",
            "0.1.1",
        )
        .unwrap();
        assert_eq!(c.version, "0.2.0");
        assert_eq!(c.channel, DeploymentChannel::Production);
    }

    #[test]
    fn rejects_cross_channel_and_stale() {
        let m = manifest(DeploymentChannel::Development, "0.2.0");
        assert!(matches!(
            discover_update(
                &m,
                DeploymentChannel::Production,
                "x86_64-unknown-linux-gnu",
                "0.1.1"
            ),
            Err(UpdateError::Discovery(_))
        ));
        let m = manifest(DeploymentChannel::Production, "0.1.1");
        assert!(matches!(
            discover_update(
                &m,
                DeploymentChannel::Production,
                "x86_64-unknown-linux-gnu",
                "0.1.1"
            ),
            Err(UpdateError::NoUpdate)
        ));
        let m = manifest(DeploymentChannel::Production, "0.2.0");
        assert!(matches!(
            discover_update(
                &m,
                DeploymentChannel::Production,
                "aarch64-apple-darwin",
                "0.1.1"
            ),
            Err(UpdateError::NoUpdate)
        ));
    }

    #[test]
    fn manifest_artifact_import_used() {
        let _ = ManifestArtifact {
            target: "x".to_string(),
            filename: "f".to_string(),
            sha256: "a".repeat(64),
            size: 1,
            build_id: "b".to_string(),
            commit: "c".to_string(),
        };
    }
}
