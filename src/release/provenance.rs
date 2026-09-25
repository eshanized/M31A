//! Release provenance: factual, verifiable build metadata.
//!
//! The repository defines NO signing infrastructure. Provenance is therefore
//! an explicit factual record — source revision, version, target, toolchain,
//! timestamp policy, artifact digests — never a fake signature, placeholder
//! signature, or manufactured "verified" state. `SIGNING_MECHANISM` states
//! this; validation rejects any manifest claiming otherwise.

use serde::{Deserialize, Serialize};
use std::path::Path;

/// Declared signing mechanism for M31A releases. The repository has none;
/// provenance verification is consistency of facts, not signature checks.
pub const SIGNING_MECHANISM: &str = "none";

/// Factual provenance record for one release build.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Provenance {
    pub project: String,
    pub version: String,
    pub source_revision: String,
    pub source_dirty: bool,
    pub build_target: String,
    pub toolchain_rustc: String,
    pub toolchain_cargo: String,
    /// RFC 3339 UTC. When `SOURCE_DATE_EPOCH` is set and valid it is honored
    /// (reproducible timestamp); otherwise wall-clock time is recorded and
    /// `reproducible_timestamp` is false.
    pub build_timestamp: String,
    pub reproducible_timestamp: bool,
    pub signing_mechanism: String,
    pub artifact_digests: Vec<ArtifactDigest>,
}

/// Artifact digest entry bound into provenance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactDigest {
    pub name: String,
    pub sha256: String,
    pub size_bytes: u64,
}

/// Typed provenance failures. Collection fails closed — no partial records,
// no defaulted revisions, no invented timestamps.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ProvenanceError {
    #[error("provenance error: {0}")]
    Collection(String),
    #[error("provenance error: refusing to sign — repository defines no signing mechanism")]
    SigningRefused,
    #[error("provenance error: manifest claims signing mechanism '{0}'")]
    UnexpectedSigning(String),
}

/// Run a command and return trimmed stdout, or a typed collection error.
fn run_capture(program: &str, args: &[&str], dir: &Path) -> Result<String, ProvenanceError> {
    let out = std::process::Command::new(program)
        .args(args)
        .current_dir(dir)
        .output()
        .map_err(|e| ProvenanceError::Collection(format!("cannot run {program}: {e}")))?;
    if !out.status.success() {
        return Err(ProvenanceError::Collection(format!(
            "{program} {} failed with {}",
            args.join(" "),
            out.status
        )));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// Collect factual provenance for `repo_dir`.
///
/// Fails closed when git/toolchain facts are unavailable. Records whether the
/// source tree is dirty. Never invents a revision, timestamp, or signature.
pub fn collect_provenance(
    repo_dir: &Path,
    version: &str,
    build_target: &str,
    artifacts: Vec<ArtifactDigest>,
) -> Result<Provenance, ProvenanceError> {
    if version.trim().is_empty() {
        return Err(ProvenanceError::Collection(
            "refusing provenance with empty version".to_string(),
        ));
    }
    if build_target.trim().is_empty() {
        return Err(ProvenanceError::Collection(
            "refusing provenance with empty build target".to_string(),
        ));
    }
    let source_revision = run_capture("git", &["rev-parse", "HEAD"], repo_dir)?;
    if source_revision.is_empty() {
        return Err(ProvenanceError::Collection(
            "empty source revision".to_string(),
        ));
    }
    let status = run_capture("git", &["status", "--porcelain"], repo_dir)?;
    let toolchain_rustc = run_capture("rustc", &["--version"], repo_dir)?;
    let toolchain_cargo = run_capture("cargo", &["--version"], repo_dir)?;

    let (build_timestamp, reproducible_timestamp) = match std::env::var("SOURCE_DATE_EPOCH") {
        Ok(epoch) => match epoch.trim().parse::<i64>() {
            Ok(secs) if secs >= 0 => (
                chrono::DateTime::from_timestamp(secs, 0)
                    .map(|dt| dt.to_rfc3339_opts(chrono::SecondsFormat::Secs, true))
                    .unwrap_or_default(),
                true,
            ),
            _ => {
                return Err(ProvenanceError::Collection(
                    "SOURCE_DATE_EPOCH is set but not a valid non-negative integer".to_string(),
                ));
            }
        },
        Err(_) => (
            chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
            false,
        ),
    };
    if build_timestamp.is_empty() {
        return Err(ProvenanceError::Collection(
            "refusing provenance with empty timestamp".to_string(),
        ));
    }

    Ok(Provenance {
        project: "m31a".to_string(),
        version: version.to_string(),
        source_revision,
        source_dirty: !status.is_empty(),
        build_target: build_target.to_string(),
        toolchain_rustc,
        toolchain_cargo,
        build_timestamp,
        reproducible_timestamp,
        signing_mechanism: SIGNING_MECHANISM.to_string(),
        artifact_digests: artifacts,
    })
}

/// Verify provenance consistency: rejects empty facts, unexpected signing
/// claims, and digest mismatches against `dir`. Existence alone never verifies.
pub fn verify_provenance(provenance: &Provenance, dir: &Path) -> Result<(), ProvenanceError> {
    if provenance.signing_mechanism != SIGNING_MECHANISM {
        return Err(ProvenanceError::UnexpectedSigning(
            provenance.signing_mechanism.clone(),
        ));
    }
    for field in [
        ("project", &provenance.project),
        ("version", &provenance.version),
        ("source_revision", &provenance.source_revision),
        ("build_target", &provenance.build_target),
        ("toolchain_rustc", &provenance.toolchain_rustc),
        ("toolchain_cargo", &provenance.toolchain_cargo),
        ("build_timestamp", &provenance.build_timestamp),
    ] {
        if field.1.trim().is_empty() {
            return Err(ProvenanceError::Collection(format!(
                "provenance field '{}' is empty",
                field.0
            )));
        }
    }
    for digest in &provenance.artifact_digests {
        crate::release::integrity::validate_artifact_name(&digest.name)
            .map_err(|e| ProvenanceError::Collection(format!("unsafe artifact name: {e}")))?;
        let path = dir.join(&digest.name);
        let meta = std::fs::metadata(&path).map_err(|_| {
            ProvenanceError::Collection(format!("artifact missing: '{}'", digest.name))
        })?;
        if meta.len() != digest.size_bytes {
            return Err(ProvenanceError::Collection(format!(
                "artifact '{}' size mismatch",
                digest.name
            )));
        }
        let computed = crate::release::integrity::sha256_file(&path)
            .map_err(|e| ProvenanceError::Collection(format!("artifact '{}': {e}", digest.name)))?;
        if computed != digest.sha256 {
            return Err(ProvenanceError::Collection(format!(
                "artifact '{}' digest mismatch",
                digest.name
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signing_is_explicitly_absent() {
        assert_eq!(SIGNING_MECHANISM, "none");
    }

    #[test]
    fn empty_identity_fails_closed() {
        let dir = tempfile::tempdir().unwrap();
        assert!(collect_provenance(dir.path(), "", "t", vec![]).is_err());
        assert!(collect_provenance(dir.path(), "0.1.0", "", vec![]).is_err());
    }

    #[test]
    fn unexpected_signing_claim_rejected() {
        let mut p = Provenance {
            project: "m31a".to_string(),
            version: "0.1.0".to_string(),
            source_revision: "abc".to_string(),
            source_dirty: false,
            build_target: "t".to_string(),
            toolchain_rustc: "r".to_string(),
            toolchain_cargo: "c".to_string(),
            build_timestamp: "2026-01-01T00:00:00Z".to_string(),
            reproducible_timestamp: false,
            signing_mechanism: "sigstore".to_string(),
            artifact_digests: vec![],
        };
        let dir = tempfile::tempdir().unwrap();
        assert!(matches!(
            verify_provenance(&p, dir.path()),
            Err(ProvenanceError::UnexpectedSigning(_))
        ));
        p.signing_mechanism = SIGNING_MECHANISM.to_string();
        assert!(verify_provenance(&p, dir.path()).is_ok());
    }
}
