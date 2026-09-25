//! Machine-readable release manifest: schema, canonical serialization, and
//! fail-closed validation.
//!
//! A manifest is valid only when: all required fields are present and
//! well-formed; artifact names are unique, safe, and sorted; every entry
//! resolves inside the release directory; and no entry is ambiguous.

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::Path;

use crate::release::integrity::{sha256_file, validate_artifact_name};

/// One released artifact's identity + integrity metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactEntry {
    pub name: String,
    pub sha256: String,
    pub size_bytes: u64,
    pub kind: String,
}

/// Canonical release manifest schema (v1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseManifest {
    pub schema_version: u32,
    pub project: String,
    pub version: String,
    pub source_revision: String,
    pub build_target: String,
    pub toolchain_rustc: String,
    pub toolchain_cargo: String,
    pub build_timestamp: String,
    pub reproducible_build: bool,
    pub artifacts: Vec<ArtifactEntry>,
    pub sbom_file: Option<String>,
    pub sbom_sha256: Option<String>,
    pub provenance_file: Option<String>,
    pub migration_version: u32,
    pub release_status: String,
}

/// Typed manifest failures.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ManifestError {
    #[error("manifest error: {0}")]
    Schema(String),
    #[error("manifest error: duplicate artifact identity '{0}'")]
    Duplicate(String),
    #[error("manifest error: unsafe artifact name '{0}'")]
    UnsafeName(String),
    #[error("manifest error: {0}")]
    Integrity(String),
}

impl ReleaseManifest {
    /// Canonical JSON serialization: struct field order + sorted artifacts.
    /// Deterministic by construction (no maps, sorted vec).
    pub fn to_canonical_json(&self) -> Result<String, ManifestError> {
        let mut clone = self.clone();
        clone.artifacts.sort_by(|a, b| a.name.cmp(&b.name));
        serde_json::to_string_pretty(&clone)
            .map_err(|e| ManifestError::Schema(format!("serialization failed: {e}")))
    }

    /// Parse a manifest document. Malformed documents fail closed.
    pub fn parse_json(doc: &str) -> Result<Self, ManifestError> {
        serde_json::from_str(doc)
            .map_err(|e| ManifestError::Schema(format!("malformed manifest: {e}")))
    }

    /// Validate schema-level invariants (no filesystem access):
    /// schema version, non-empty identities, unique + safe + sorted artifact
    /// names, well-formed hashes/sizes, sbom/provenance references safe.
    pub fn validate_schema(&self) -> Result<(), ManifestError> {
        if self.schema_version != 1 {
            return Err(ManifestError::Schema(format!(
                "unsupported schema_version {}",
                self.schema_version
            )));
        }
        for field in [
            ("project", &self.project),
            ("version", &self.version),
            ("source_revision", &self.source_revision),
            ("build_target", &self.build_target),
            ("toolchain_rustc", &self.toolchain_rustc),
            ("toolchain_cargo", &self.toolchain_cargo),
            ("build_timestamp", &self.build_timestamp),
            ("release_status", &self.release_status),
        ] {
            if field.1.trim().is_empty() {
                return Err(ManifestError::Schema(format!(
                    "required field '{}' is empty",
                    field.0
                )));
            }
        }
        if self.artifacts.is_empty() {
            return Err(ManifestError::Schema(
                "manifest lists zero artifacts".to_string(),
            ));
        }
        let mut seen = BTreeSet::new();
        let mut prev: Option<&str> = None;
        for entry in &self.artifacts {
            validate_artifact_name(&entry.name)
                .map_err(|_| ManifestError::UnsafeName(entry.name.clone()))?;
            if !seen.insert(entry.name.clone()) {
                return Err(ManifestError::Duplicate(entry.name.clone()));
            }
            if entry.sha256.len() != 64 || !entry.sha256.chars().all(|c| c.is_ascii_hexdigit()) {
                return Err(ManifestError::Schema(format!(
                    "artifact '{}' has malformed sha256",
                    entry.name
                )));
            }
            if entry.kind.trim().is_empty() {
                return Err(ManifestError::Schema(format!(
                    "artifact '{}' has empty kind",
                    entry.name
                )));
            }
            if let Some(p) = prev
                && p >= entry.name.as_str()
            {
                return Err(ManifestError::Schema(
                    "artifacts are not in sorted order (non-canonical)".to_string(),
                ));
            }
            prev = Some(&entry.name);
        }
        for name in [&self.sbom_file, &self.provenance_file]
            .into_iter()
            .flatten()
        {
            validate_artifact_name(name).map_err(|_| ManifestError::UnsafeName(name.clone()))?;
        }
        if self.sbom_file.is_some() != self.sbom_sha256.is_some() {
            return Err(ManifestError::Schema(
                "sbom_file and sbom_sha256 must be jointly present or absent".to_string(),
            ));
        }
        Ok(())
    }

    /// Full validation against a release directory: schema first, then every
    /// artifact must exist with matching bytes and size. No manifest entry
    /// may reference a missing artifact; existence alone never verifies.
    pub fn validate_against_dir(&self, dir: &Path) -> Result<(), ManifestError> {
        self.validate_schema()?;
        for entry in &self.artifacts {
            let path = dir.join(&entry.name);
            let meta = std::fs::metadata(&path).map_err(|_| {
                ManifestError::Integrity(format!("artifact missing: '{}'", entry.name))
            })?;
            if meta.len() != entry.size_bytes {
                return Err(ManifestError::Integrity(format!(
                    "artifact '{}' size mismatch: manifest {} vs actual {}",
                    entry.name,
                    entry.size_bytes,
                    meta.len()
                )));
            }
            let computed =
                sha256_file(&path).map_err(|e| ManifestError::Integrity(e.to_string()))?;
            if computed != entry.sha256 {
                return Err(ManifestError::Integrity(format!(
                    "artifact '{}' hash mismatch",
                    entry.name
                )));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> ReleaseManifest {
        ReleaseManifest {
            schema_version: 1,
            project: "m31a".to_string(),
            version: "0.1.0".to_string(),
            source_revision: "abc123".to_string(),
            build_target: "x86_64-unknown-linux-gnu".to_string(),
            toolchain_rustc: "rustc 1.85.0".to_string(),
            toolchain_cargo: "cargo 1.85.0".to_string(),
            build_timestamp: "2026-01-01T00:00:00Z".to_string(),
            reproducible_build: false,
            artifacts: vec![ArtifactEntry {
                name: "m31a.tar.gz".to_string(),
                sha256: "a".repeat(64),
                size_bytes: 10,
                kind: "archive".to_string(),
            }],
            sbom_file: None,
            sbom_sha256: None,
            provenance_file: None,
            migration_version: 24,
            release_status: "candidate".to_string(),
        }
    }

    #[test]
    fn canonical_json_is_deterministic() {
        let a = sample().to_canonical_json().unwrap();
        let b = sample().to_canonical_json().unwrap();
        assert_eq!(a, b);
        let parsed = ReleaseManifest::parse_json(&a).unwrap();
        assert_eq!(parsed, sample());
    }

    #[test]
    fn schema_rejects_duplicates_unsorted_unsafe() {
        let mut m = sample();
        m.artifacts.push(m.artifacts[0].clone());
        assert!(matches!(
            m.validate_schema(),
            Err(ManifestError::Duplicate(_))
        ));
        let mut m = sample();
        m.artifacts[0].name = "../evil".to_string();
        assert!(matches!(
            m.validate_schema(),
            Err(ManifestError::UnsafeName(_))
        ));
        let mut m = sample();
        m.artifacts.push(ArtifactEntry {
            name: "a.bin".to_string(),
            sha256: "b".repeat(64),
            size_bytes: 1,
            kind: "binary".to_string(),
        });
        assert!(matches!(m.validate_schema(), Err(ManifestError::Schema(_))));
        let mut m = sample();
        m.schema_version = 99;
        assert!(matches!(m.validate_schema(), Err(ManifestError::Schema(_))));
        let mut m = sample();
        m.artifacts.clear();
        assert!(matches!(m.validate_schema(), Err(ManifestError::Schema(_))));
    }

    #[test]
    fn malformed_documents_fail_closed() {
        assert!(ReleaseManifest::parse_json("").is_err());
        assert!(ReleaseManifest::parse_json("{}").is_err());
        assert!(ReleaseManifest::parse_json("{\"schema_version\": 1}").is_err());
    }
}
