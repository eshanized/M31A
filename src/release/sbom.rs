//! Software Bill of Materials generated from the exact resolved dependency
//! graph (`Cargo.lock`) of the release build.
//!
//! Honesty rules: every `[[package]]` in the lock file appears exactly once,
//! sorted deterministically; fields come only from the lock (name, version,
//! source, checksum). `Cargo.lock` carries no license texts, so no licenses
//! are claimed — the SBOM states this instead of inventing license data.

use serde::{Deserialize, Serialize};

/// One locked dependency.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SbomComponent {
    pub name: String,
    pub version: String,
    pub source: Option<String>,
    pub checksum: Option<String>,
}

/// Minimal CycloneDX-shaped SBOM document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sbom {
    #[serde(rename = "bomFormat")]
    pub bom_format: String,
    #[serde(rename = "specVersion")]
    pub spec_version: String,
    pub version: u32,
    pub metadata_tool: String,
    /// Explicitly recorded because `Cargo.lock` provides no license data.
    pub licenses_note: String,
    pub components: Vec<SbomComponent>,
}

/// Typed SBOM failures.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SbomError {
    #[error("SBOM error: {0}")]
    Parse(String),
    #[error("SBOM error: lock file contains zero packages")]
    Empty,
}

#[derive(Debug, Deserialize)]
struct CargoLock {
    #[serde(default)]
    package: Vec<CargoLockPackage>,
}

#[derive(Debug, Deserialize)]
struct CargoLockPackage {
    name: String,
    version: String,
    source: Option<String>,
    checksum: Option<String>,
}

/// Generate a deterministic SBOM from `Cargo.lock` document text.
/// Deterministic: components sorted by (name, version, source); repeated
/// generation over identical input yields identical bytes.
pub fn generate_sbom(lock_doc: &str) -> Result<(Sbom, String), SbomError> {
    let lock: CargoLock = toml::from_str(lock_doc)
        .map_err(|e| SbomError::Parse(format!("Cargo.lock invalid: {e}")))?;
    if lock.package.is_empty() {
        return Err(SbomError::Empty);
    }
    let mut components: Vec<SbomComponent> = lock
        .package
        .into_iter()
        .map(|p| SbomComponent {
            name: p.name,
            version: p.version,
            source: p.source,
            checksum: p.checksum,
        })
        .collect();
    components
        .sort_by(|a, b| (&a.name, &a.version, &a.source).cmp(&(&b.name, &b.version, &b.source)));
    let sbom = Sbom {
        bom_format: "CycloneDX".to_string(),
        spec_version: "1.6".to_string(),
        version: 1,
        metadata_tool: format!("m31a-release {}", crate::release::PKG_VERSION),
        licenses_note: "Cargo.lock provides no license data; licenses are not claimed here. See individual crate registries.".to_string(),
        components,
    };
    let json = serde_json::to_string_pretty(&sbom)
        .map_err(|e| SbomError::Parse(format!("SBOM serialization failed: {e}")))?;
    Ok((sbom, json))
}

/// Validate an SBOM document: parses, requires non-empty sorted components
/// with non-empty name/version.
pub fn validate_sbom(doc: &str) -> Result<Sbom, SbomError> {
    let sbom: Sbom =
        serde_json::from_str(doc).map_err(|e| SbomError::Parse(format!("SBOM invalid: {e}")))?;
    if sbom.components.is_empty() {
        return Err(SbomError::Empty);
    }
    let mut prev: Option<(&str, &str)> = None;
    for c in &sbom.components {
        if c.name.trim().is_empty() || c.version.trim().is_empty() {
            return Err(SbomError::Parse(
                "SBOM component missing name/version".to_string(),
            ));
        }
        let key = (c.name.as_str(), c.version.as_str());
        if let Some(p) = prev
            && p > key
        {
            return Err(SbomError::Parse("SBOM components not sorted".to_string()));
        }
        prev = Some(key);
    }
    Ok(sbom)
}

#[cfg(test)]
mod tests {
    use super::*;

    const LOCK: &str = r#"
version = 4

[[package]]
name = "zeta"
version = "2.0.0"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "abc123"

[[package]]
name = "alpha"
version = "1.0.0"
"#;

    #[test]
    fn generation_is_deterministic_and_sorted() {
        let (sbom, json_a) = generate_sbom(LOCK).unwrap();
        let (_, json_b) = generate_sbom(LOCK).unwrap();
        assert_eq!(json_a, json_b);
        assert_eq!(sbom.components.len(), 2);
        assert_eq!(sbom.components[0].name, "alpha");
        assert_eq!(sbom.components[1].name, "zeta");
        assert_eq!(sbom.components[1].checksum.as_deref(), Some("abc123"));
        assert!(validate_sbom(&json_a).is_ok());
    }

    #[test]
    fn empty_and_malformed_fail_closed() {
        assert!(matches!(
            generate_sbom("version = 4\n"),
            Err(SbomError::Empty)
        ));
        assert!(generate_sbom("[[[").is_err());
        assert!(validate_sbom("{}").is_err());
        assert!(validate_sbom("[]").is_err());
    }
}
