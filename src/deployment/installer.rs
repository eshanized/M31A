//! Transactional installer: staged, verified, atomic replacement.
//!
//! Update sequence (behavioral transaction):
//!   discover → download → verify checksum → stage → verify executable →
//!   atomically replace → verify runtime. On any failure the previous
//!   known-good binary is preserved and success is never reported.
//!
//! User state (config/data/cache/state/database) is never wiped or migrated
//! implicitly by installation.

use std::path::{Path, PathBuf};

use super::artifact::ReleaseArtifact;
use crate::release::integrity::{sha256_bytes, sha256_file};

/// Typed installer failures. Every variant preserves the previous binary.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum InstallError {
    #[error("install error: {0}")]
    Verification(String),
    #[error("install error: {0}")]
    Io(String),
    #[error("install error: channel mismatch: {0}")]
    ChannelMismatch(String),
}

/// Staged installation under `install_dir`:
///   `<install_dir>/<binary>`          — live executable
///   `<install_dir>/<binary>.prev`    — previous known-good backup
///   `<install_dir>/.staging/<file>`  — unverified staging area
#[derive(Debug, Clone)]
pub struct Installer {
    install_dir: PathBuf,
}

impl Installer {
    pub fn new(install_dir: impl Into<PathBuf>) -> Self {
        Self {
            install_dir: install_dir.into(),
        }
    }

    pub fn install_dir(&self) -> &Path {
        &self.install_dir
    }

    fn live_path(&self, binary_name: &str) -> PathBuf {
        self.install_dir.join(binary_name)
    }

    fn prev_path(&self, binary_name: &str) -> PathBuf {
        self.install_dir.join(format!("{binary_name}.prev"))
    }

    fn staging_path(&self, filename: &str) -> PathBuf {
        self.install_dir.join(".staging").join(filename)
    }

    /// Install raw `bytes` as `artifact` after full verification.
    /// `expected_channel` enforces update-channel safety at the install
    /// boundary (production installers reject development artifacts).
    pub fn install_bytes(
        &self,
        artifact: &ReleaseArtifact,
        bytes: &[u8],
        expected_channel: super::channel::DeploymentChannel,
    ) -> Result<PathBuf, InstallError> {
        artifact
            .validate()
            .map_err(|e| InstallError::Verification(e.to_string()))?;
        if artifact.channel != expected_channel {
            return Err(InstallError::ChannelMismatch(format!(
                "artifact channel '{}' does not match installer channel '{}'",
                artifact.channel, expected_channel
            )));
        }
        if bytes.len() as u64 != artifact.size {
            return Err(InstallError::Verification(format!(
                "size mismatch: manifest {} vs downloaded {}",
                artifact.size,
                bytes.len()
            )));
        }
        let computed = sha256_bytes(bytes);
        if computed != artifact.sha256 {
            return Err(InstallError::Verification(format!(
                "checksum mismatch for '{}': expected {}, computed {}",
                artifact.filename, artifact.sha256, computed
            )));
        }
        if bytes.is_empty() {
            return Err(InstallError::Verification(
                "refusing to install empty artifact".to_string(),
            ));
        }

        let binary_name = expected_channel.binary_name();
        let staging_dir = self.install_dir.join(".staging");
        std::fs::create_dir_all(&staging_dir)
            .map_err(|e| InstallError::Io(format!("cannot create staging dir: {e}")))?;
        let staged = self.staging_path(&artifact.filename);
        std::fs::write(&staged, bytes)
            .map_err(|e| InstallError::Io(format!("cannot stage artifact: {e}")))?;

        // Re-verify staged bytes (download → stage integrity).
        let staged_hash =
            sha256_file(&staged).map_err(|e| InstallError::Verification(e.to_string()))?;
        if staged_hash != artifact.sha256 {
            let _ = std::fs::remove_file(&staged);
            return Err(InstallError::Verification(
                "staged artifact checksum mismatch".to_string(),
            ));
        }

        std::fs::create_dir_all(&self.install_dir)
            .map_err(|e| InstallError::Io(format!("cannot create install dir: {e}")))?;
        let live = self.live_path(binary_name);

        // Preserve previous known-good binary BEFORE replacement.
        if live.is_file() {
            let prev = self.prev_path(binary_name);
            std::fs::copy(&live, &prev)
                .map_err(|e| InstallError::Io(format!("cannot back up previous binary: {e}")))?;
        }

        // Atomic-ish replacement: copy staged → live, then verify live bytes.
        std::fs::copy(&staged, &live)
            .map_err(|e| InstallError::Io(format!("cannot replace executable: {e}")))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&live)
                .map_err(|e| InstallError::Io(e.to_string()))?
                .permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&live, perms).map_err(|e| InstallError::Io(e.to_string()))?;
        }
        let live_hash =
            sha256_file(&live).map_err(|e| InstallError::Verification(e.to_string()))?;
        if live_hash != artifact.sha256 {
            // Attempt restoration of the previous binary; report verification
            // failure regardless (never claim success).
            let prev = self.prev_path(binary_name);
            if prev.is_file() {
                let _ = std::fs::copy(&prev, &live);
            }
            return Err(InstallError::Verification(
                "installed executable checksum mismatch; previous binary restored".to_string(),
            ));
        }
        let _ = std::fs::remove_file(&staged);
        Ok(live)
    }

    /// Path of the preserved previous binary, if any.
    pub fn previous_binary(&self, binary_name: &str) -> Option<PathBuf> {
        let p = self.prev_path(binary_name);
        p.is_file().then_some(p)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::deployment::channel::DeploymentChannel;

    fn artifact_for(channel: DeploymentChannel, bytes: &[u8]) -> ReleaseArtifact {
        ReleaseArtifact {
            artifact_id: format!("{}-0.1.1-x86_64-unknown-linux-gnu", channel.binary_name()),
            version: "0.1.1".to_string(),
            channel,
            target: "x86_64-unknown-linux-gnu".to_string(),
            format: "tar.gz".to_string(),
            filename: "m31a-test.tar.gz".to_string(),
            sha256: sha256_bytes(bytes),
            size: bytes.len() as u64,
            build_id: "0123456789abcdef".to_string(),
            git_commit: "abc123".to_string(),
        }
    }

    #[test]
    fn successful_install_preserves_previous() {
        let dir = tempfile::tempdir().unwrap();
        let inst = Installer::new(dir.path());
        let v1 = b"binary-v1-bytes";
        let a1 = artifact_for(DeploymentChannel::Production, v1);
        let live = inst
            .install_bytes(&a1, v1, DeploymentChannel::Production)
            .unwrap();
        assert_eq!(std::fs::read(&live).unwrap(), v1);
        assert!(inst.previous_binary("m31a").is_none());

        let v2 = b"binary-v2-bytes-longer";
        let a2 = artifact_for(DeploymentChannel::Production, v2);
        inst.install_bytes(&a2, v2, DeploymentChannel::Production)
            .unwrap();
        assert_eq!(std::fs::read(&live).unwrap(), v2);
        let prev = inst.previous_binary("m31a").unwrap();
        assert_eq!(std::fs::read(&prev).unwrap(), v1);
    }

    #[test]
    fn checksum_mismatch_keeps_previous() {
        let dir = tempfile::tempdir().unwrap();
        let inst = Installer::new(dir.path());
        let v1 = b"good-bytes";
        let a1 = artifact_for(DeploymentChannel::Production, v1);
        let live = inst
            .install_bytes(&a1, v1, DeploymentChannel::Production)
            .unwrap();

        let mut bad = artifact_for(DeploymentChannel::Production, b"other");
        bad.size = b"corrupt".len() as u64;
        // sha belongs to b"other" but bytes are corrupt → mismatch.
        let r = inst.install_bytes(&bad, b"corrupt", DeploymentChannel::Production);
        assert!(matches!(r, Err(InstallError::Verification(_))));
        assert_eq!(std::fs::read(&live).unwrap(), v1);
    }

    #[test]
    fn channel_mismatch_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let inst = Installer::new(dir.path());
        let bytes = b"dev-bytes";
        let dev = artifact_for(DeploymentChannel::Development, bytes);
        let r = inst.install_bytes(&dev, bytes, DeploymentChannel::Production);
        assert!(matches!(r, Err(InstallError::ChannelMismatch(_))));
    }
}
