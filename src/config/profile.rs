//! Composable Configuration Profiles & Single Inheritance (CFG-04).

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

use crate::config::merge::{
    ConfigError, MonotonicSecurityMerger, deep_merge_json, toml_to_json_value,
};
use crate::config::schema::{AgentsConfig, AppConfig, PolicyConfig, RuntimeConfig};

/// Profile configuration declaring role defaults, capabilities, policies, and resource limits.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(deny_unknown_fields)]
pub struct ProfileConfig {
    pub name: String,
    #[serde(default)]
    pub extends: Option<String>,
    #[serde(default)]
    pub agent_defaults: Option<AgentsConfig>,
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(default)]
    pub policy_overrides: Option<PolicyConfig>,
    #[serde(default)]
    pub runtime_overrides: Option<RuntimeConfig>,
}

/// Conversion trait to accept either `serde_json::Value` or legacy `toml::Value` when registering profiles.
pub trait IntoProfileValue {
    fn into_json_value(self) -> serde_json::Value;
}

impl IntoProfileValue for serde_json::Value {
    fn into_json_value(self) -> serde_json::Value {
        self
    }
}

impl IntoProfileValue for toml::Value {
    fn into_json_value(self) -> serde_json::Value {
        toml_to_json_value(self)
    }
}

/// Resolver loading and composing profiles with single inheritance ('extends = "base"').
#[derive(Clone)]
pub struct ProfileResolver {
    profiles: HashMap<String, serde_json::Value>,
}

impl Default for ProfileResolver {
    fn default() -> Self {
        Self::with_canonical_profiles()
    }
}

impl ProfileResolver {
    pub fn new() -> Self {
        Self {
            profiles: HashMap::new(),
        }
    }

    /// Instantiate resolver pre-loaded with the canonical profiles (SKL-02, D-11).
    pub fn with_canonical_profiles() -> Self {
        let mut resolver = Self::new();
        resolver.register_canonical_profiles();
        resolver
    }

    pub fn register_profile(&mut self, name: &str, profile_val: impl IntoProfileValue) {
        self.profiles
            .insert(normalize_profile_name(name), profile_val.into_json_value());
    }

    /// Parse a profile document string (Markdown with front matter or legacy TOML) into a format-neutral JSON value.
    pub fn parse_profile_document(doc_str: &str) -> Result<serde_json::Value, ConfigError> {
        if doc_str.trim_start().starts_with("---") {
            let val = crate::document::parse_markdown_value(
                doc_str,
                crate::document::DocumentKind::Profile,
            )
            .map_err(|e| ConfigError::ValidationError(e.to_string()))?;
            serde_json::to_value(&val.data).map_err(|e| ConfigError::ValidationError(e.to_string()))
        } else {
            let toml_val: toml::Value =
                toml::from_str(doc_str).map_err(|e| ConfigError::TomlParseError(e.to_string()))?;
            Ok(toml_to_json_value(toml_val))
        }
    }

    /// Register a profile from a document string (Markdown or TOML).
    pub fn register_profile_str(&mut self, name: &str, doc_str: &str) -> Result<(), ConfigError> {
        let val = Self::parse_profile_document(doc_str)?;
        self.register_profile(name, val);
        Ok(())
    }

    fn register_canonical_profiles(&mut self) {
        // Canonical profiles are declarative packaged configuration in
        // `assets/profiles/*.md`, loaded through the same parser/resolver
        // as external profiles (no Rust-embedded TOML blobs).
        const CANONICAL_ASSETS: &[(&str, &str)] = &[
            ("safe", include_str!("../../assets/profiles/safe.md")),
            (
                "conservative",
                include_str!("../../assets/profiles/conservative.md"),
            ),
            ("coding", include_str!("../../assets/profiles/coding.md")),
            (
                "balanced",
                include_str!("../../assets/profiles/balanced.md"),
            ),
            (
                "research",
                include_str!("../../assets/profiles/research.md"),
            ),
            (
                "code_reviewer",
                include_str!("../../assets/profiles/code_reviewer.md"),
            ),
            (
                "autonomous",
                include_str!("../../assets/profiles/autonomous.md"),
            ),
            ("ci", include_str!("../../assets/profiles/ci.md")),
            (
                "security_review",
                include_str!("../../assets/profiles/security_review.md"),
            ),
            ("release", include_str!("../../assets/profiles/release.md")),
        ];

        for (name, doc_str) in CANONICAL_ASSETS {
            let val = Self::parse_profile_document(doc_str).unwrap_or_else(|e| {
                panic!("bundled canonical profile '{name}' must be valid: {e}")
            });
            self.register_profile(name, val);
        }
    }

    /// Resolve and merge a named profile, recursively resolving its inheritance hierarchy.
    ///
    /// Single inheritance: child extends parent.
    /// Invariant: Merged profile cannot relax safety policies of parent (CFG-04).
    pub fn resolve_profile(&self, profile_name: &str) -> Result<serde_json::Value, ConfigError> {
        let mut visited = HashSet::new();
        self.resolve_profile_recursive(&normalize_profile_name(profile_name), &mut visited)
    }

    /// Resolve profile as legacy `toml::Value` for backward-compatible test assertions.
    pub fn resolve_profile_toml(&self, profile_name: &str) -> Result<toml::Value, ConfigError> {
        let json_val = self.resolve_profile(profile_name)?;
        Ok(crate::config::resolved::json_to_toml(&json_val))
    }

    fn resolve_profile_recursive(
        &self,
        profile_name: &str,
        visited: &mut HashSet<String>,
    ) -> Result<serde_json::Value, ConfigError> {
        let normalized_name = normalize_profile_name(profile_name);
        if !visited.insert(normalized_name.clone()) {
            return Err(ConfigError::CyclicProfileInheritance(normalized_name));
        }

        let raw = self
            .profiles
            .get(&normalized_name)
            .cloned()
            .ok_or_else(|| ConfigError::ProfileNotFound(normalized_name.clone()))?;

        // Check if extends parent
        let extends_opt = raw
            .get("extends")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let final_val = if let Some(parent_name) = extends_opt {
            let mut parent_merged = self.resolve_profile_recursive(&parent_name, visited)?;

            // Monotonic security check: child cannot downgrade parent security
            MonotonicSecurityMerger::check_json(&parent_merged, &raw)?;

            // Deep merge child onto parent
            deep_merge_json(&mut parent_merged, raw);
            parent_merged
        } else {
            raw
        };

        Ok(final_val)
    }

    /// Compose a resolved profile onto a base AppConfig.
    pub fn apply_to_app_config(
        &self,
        profile_name: &str,
        mut base_config_json: serde_json::Value,
    ) -> Result<AppConfig, ConfigError> {
        let profile_json = self.resolve_profile(profile_name)?;
        MonotonicSecurityMerger::check_json(&base_config_json, &profile_json)?;
        deep_merge_json(&mut base_config_json, profile_json);

        serde_json::from_value(base_config_json)
            .map_err(|e| ConfigError::ValidationError(e.to_string()))
    }

    /// List all canonical profile names available in the system.
    pub fn canonical_profile_names() -> &'static [&'static str] {
        &[
            "balanced",
            "autonomous",
            "conservative",
            "code_reviewer",
            "safe",
            "coding",
            "research",
            "ci",
            "security_review",
            "release",
        ]
    }

    /// Canonical autonomy mode for a profile id (single definition site).
    /// Resolves inheritance so `balanced` inherits `coding`, `conservative`
    /// inherits `safe`, etc. Unknown ids fail closed (never silently map to
    /// a default); hyphen aliases (`code-reviewer`, `security-review`) fold
    /// to their canonical underscore form.
    pub fn canonical_autonomy_for_profile(profile_name: &str) -> Result<String, ConfigError> {
        let normalized = normalize_profile_name(profile_name);
        let resolver = Self::with_canonical_profiles();
        let resolved = resolver.resolve_profile(&normalized)?;
        resolved
            .get("autonomy_mode")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| {
                ConfigError::ValidationError(format!(
                    "profile '{profile_name}' carries no canonical autonomy_mode"
                ))
            })
    }

    /// Full canonical metadata for wizard/runtime projections: resolved
    /// profile JSON (inheritance applied) for the given id.
    pub fn canonical_metadata_for_profile(
        profile_name: &str,
    ) -> Result<serde_json::Value, ConfigError> {
        let normalized = normalize_profile_name(profile_name);
        let resolver = Self::with_canonical_profiles();
        resolver.resolve_profile(&normalized)
    }

    /// Full canonical metadata for wizard/runtime projections: resolved
    /// profile TOML (inheritance applied) for backward compatibility.
    pub fn canonical_metadata_for_profile_toml(
        profile_name: &str,
    ) -> Result<toml::Value, ConfigError> {
        let json_val = Self::canonical_metadata_for_profile(profile_name)?;
        Ok(crate::config::resolved::json_to_toml(&json_val))
    }

    /// Whether an id is a known canonical profile or alias.
    pub fn is_canonical_profile(profile_name: &str) -> bool {
        let normalized = normalize_profile_name(profile_name);
        Self::canonical_profile_names().contains(&normalized.as_str())
    }
}

fn normalize_profile_name(name: &str) -> String {
    name.trim().to_lowercase().replace('-', "_")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hyphenated_profile_aliases_resolve_to_canonical_profiles() {
        let resolver = ProfileResolver::with_canonical_profiles();

        for (alias, canonical) in [
            ("code-reviewer", "code_reviewer"),
            ("security-review", "security_review"),
        ] {
            assert_eq!(
                resolver.resolve_profile(alias).unwrap(),
                resolver.resolve_profile(canonical).unwrap(),
                "{alias} must resolve to the canonical {canonical} profile"
            );
        }
    }

    #[test]
    fn profile_names_are_normalized_when_registered_and_resolved() {
        let profile: toml::Value = toml::from_str(
            r#"
                name = "custom-profile"
                autonomy_mode = "safe"
            "#,
        )
        .unwrap();

        let mut resolver = ProfileResolver::new();
        resolver.register_profile("  Custom-Profile  ", profile.clone());

        let expected_json = serde_json::json!({
            "name": "custom-profile",
            "autonomy_mode": "safe"
        });
        assert_eq!(
            resolver.resolve_profile("custom-profile").unwrap(),
            expected_json
        );
        assert_eq!(
            resolver.resolve_profile("CUSTOM_PROFILE").unwrap(),
            expected_json
        );
        assert_eq!(
            resolver.resolve_profile_toml("custom-profile").unwrap(),
            profile
        );
        assert_eq!(
            resolver.resolve_profile_toml("CUSTOM_PROFILE").unwrap(),
            profile
        );
    }

    #[test]
    fn canonical_profile_inventory_excludes_aliases() {
        let names = ProfileResolver::canonical_profile_names();
        assert_eq!(names.len(), 10);
        assert!(!names.contains(&"code-reviewer"));
        assert!(!names.contains(&"security-review"));
        assert!(ProfileResolver::is_canonical_profile("code-reviewer"));
        assert!(ProfileResolver::is_canonical_profile("security-review"));
    }
}
