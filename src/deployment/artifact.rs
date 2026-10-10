//! Canonical release artifact model.
//!
//! An artifact's identity lives in structured metadata, never in filenames
//! alone. Every artifact records version, channel, target, format, sha256,
//! size, build ID, and source commit so production provenance is traceable
//! end-to-end (commit → CI run → build → checksum → manifest → release).

use serde::{Deserialize, Serialize};

use super::channel::DeploymentChannel;
use crate::release::integrity::validate_artifact_name;

/// Canonical release artifact identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseArtifact {
    pub artifact_id: String,
    pub version: String,
    pub channel: DeploymentChannel,
    pub target: String,
    pub format: String,
    pub filename: String,
    pub sha256: String,
    pub size: u64,
    pub build_id: String,
    pub git_commit: String,
}

/// Canonical artifact packaging format distinguishing archives from raw executables.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactFormat {
    TarGz,
    Zip,
    RawExecutable,
}

impl ArtifactFormat {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_lowercase().as_str() {
            "tar.gz" | "targz" => Some(Self::TarGz),
            "zip" => Some(Self::Zip),
            "raw" | "raw_executable" | "binary" | "executable" => Some(Self::RawExecutable),
            _ => None,
        }
    }

    pub fn is_archive(&self) -> bool {
        matches!(self, Self::TarGz | Self::Zip)
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::TarGz => "tar.gz",
            Self::Zip => "zip",
            Self::RawExecutable => "raw",
        }
    }
}

impl std::fmt::Display for ArtifactFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Typed artifact validation failures.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ArtifactError {
    #[error("artifact error: {0}")]
    Schema(String),
}

impl ReleaseArtifact {
    /// Parsed format of this artifact.
    pub fn parsed_format(&self) -> Result<ArtifactFormat, ArtifactError> {
        ArtifactFormat::parse(&self.format)
            .ok_or_else(|| ArtifactError::Schema(format!("unsupported artifact format '{}'", self.format)))
    }

    /// Expected binary executable name for this artifact's target and channel.
    pub fn expected_executable_name(&self) -> String {
        let base = self.channel.binary_name();
        if self.target.contains("windows") {
            format!("{base}.exe")
        } else {
            base.to_string()
        }
    }

    /// Fail-closed validation: non-empty identities, valid semver, known
    /// channel, supported target, safe filename, well-formed sha256/size.
    pub fn validate(&self) -> Result<(), ArtifactError> {
        let err = |m: &str| ArtifactError::Schema(m.to_string());
        if self.artifact_id.trim().is_empty() {
            return Err(err("artifact_id is empty"));
        }
        if self.version.trim().is_empty() {
            return Err(err("version is empty"));
        }
        if semver::Version::parse(self.version.trim()).is_err() {
            return Err(err("version is not valid semver"));
        }
        if self.target.trim().is_empty() {
            return Err(err("target is empty"));
        }
        if !is_supported_target(&self.target) {
            return Err(err("unsupported target triple"));
        }
        if self.format.trim().is_empty() {
            return Err(err("format is empty"));
        }
        let parsed_fmt = self.parsed_format()?;
        validate_artifact_name(&self.filename)
            .map_err(|e| err(&format!("unsafe filename: {e}")))?;
        match parsed_fmt {
            ArtifactFormat::TarGz => {
                if !self.filename.ends_with(".tar.gz") {
                    return Err(err("tar.gz artifact filename must end with .tar.gz"));
                }
            }
            ArtifactFormat::Zip => {
                if !self.filename.ends_with(".zip") {
                    return Err(err("zip artifact filename must end with .zip"));
                }
            }
            ArtifactFormat::RawExecutable => {}
        }
        if self.sha256.len() != 64 || !self.sha256.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(err("sha256 must be 64 hex chars"));
        }
        if self.build_id.trim().is_empty() {
            return Err(err("build_id is empty"));
        }
        if self.git_commit.trim().is_empty() {
            return Err(err("git_commit is empty"));
        }
        Ok(())
    }
}

/// Supported release target matrix (repo-canonical; never silently reduced).
pub fn is_supported_target(target: &str) -> bool {
    matches!(
        target,
        "x86_64-unknown-linux-gnu"
            | "aarch64-unknown-linux-gnu"
            | "x86_64-apple-darwin"
            | "aarch64-apple-darwin"
            | "x86_64-pc-windows-msvc"
            | "aarch64-pc-windows-msvc"
    )
}

/// All six supported targets in canonical order.
pub const SUPPORTED_TARGETS: [&str; 6] = [
    "x86_64-unknown-linux-gnu",
    "aarch64-unknown-linux-gnu",
    "x86_64-apple-darwin",
    "aarch64-apple-darwin",
    "x86_64-pc-windows-msvc",
    "aarch64-pc-windows-msvc",
];

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> ReleaseArtifact {
        ReleaseArtifact {
            artifact_id: "m31a-0.1.1-x86_64-unknown-linux-gnu".to_string(),
            version: "0.1.1".to_string(),
            channel: DeploymentChannel::Production,
            target: "x86_64-unknown-linux-gnu".to_string(),
            format: "tar.gz".to_string(),
            filename: "m31a-linux-x64.tar.gz".to_string(),
            sha256: "a".repeat(64),
            size: 1024,
            build_id: "0123456789abcdef".to_string(),
            git_commit: "abc123".to_string(),
        }
    }

    #[test]
    fn valid_artifact_passes() {
        assert!(sample().validate().is_ok());
    }

    #[test]
    fn invalid_artifacts_rejected() {
        let mut a = sample();
        a.version = "not-semver".to_string();
        assert!(a.validate().is_err());
        let mut a = sample();
        a.target = "mips-unknown".to_string();
        assert!(a.validate().is_err());
        let mut a = sample();
        a.sha256 = "zz".to_string();
        assert!(a.validate().is_err());
        let mut a = sample();
        a.filename = "../evil".to_string();
        assert!(a.validate().is_err());
        let mut a = sample();
        a.git_commit = String::new();
        assert!(a.validate().is_err());
    }

    #[test]
    fn target_matrix_complete() {
        assert_eq!(SUPPORTED_TARGETS.len(), 6);
        for t in SUPPORTED_TARGETS {
            assert!(is_supported_target(t));
        }
    }
}
