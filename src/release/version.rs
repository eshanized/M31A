//! Canonical version authority.
//!
//! The ONLY version source is the `Cargo.toml` package version, captured at
//! compile time. The CLI `--version` output (clap `version`), runtime
//! banners, and release metadata all derive from this constant. Any second
//! version authority is a defect.

/// Crate name as released.
pub const RUNTIME_NAME: &str = "m31a";

/// Canonical package version from `Cargo.toml` (compile-time authority).
pub const PKG_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Runtime-reported version (`m31a` package version).
pub fn runtime_version() -> &'static str {
    PKG_VERSION
}

/// Exact `m31a --version` output format (`m31a X.Y.Z`), matching the clap
/// `version` attribute and `scripts/build-release.sh` expectations.
pub fn cli_version_string() -> String {
    format!("{RUNTIME_NAME} {PKG_VERSION}")
}

/// Parse the `version` field out of a `Cargo.toml` document (release tooling
/// cross-check; fails closed on missing/malformed values — never a default).
pub fn parse_cargo_toml_version(doc: &str) -> Result<String, String> {
    let parsed: toml::Value = doc
        .parse()
        .map_err(|e| format!("Cargo.toml is not valid TOML: {e}"))?;
    parsed
        .get("package")
        .and_then(|p| p.get("version"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| "Cargo.toml has no non-empty [package] version".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn package_and_runtime_versions_agree() {
        assert_eq!(PKG_VERSION, runtime_version());
        assert!(!PKG_VERSION.trim().is_empty());
    }

    #[test]
    fn cli_version_format_matches_release_contract() {
        assert_eq!(cli_version_string(), format!("m31a {PKG_VERSION}"));
    }

    #[test]
    fn cargo_toml_version_parse_rejects_defaults() {
        assert!(parse_cargo_toml_version("[package]\nversion = \"1.2.3\"\n").unwrap() == "1.2.3");
        assert!(parse_cargo_toml_version("[package]\n").is_err());
        assert!(parse_cargo_toml_version("[package]\nversion = \"\"\n").is_err());
        assert!(parse_cargo_toml_version("not toml [[[ ").is_err());
    }
}
