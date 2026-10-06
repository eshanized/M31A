//! Composable Configuration Profiles & Single Inheritance (CFG-04).

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

use crate::config::merge::{ConfigError, MonotonicSecurityMerger, deep_merge_toml};
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

/// Resolver loading and composing profiles with single inheritance ('extends = "base"').
#[derive(Clone)]
pub struct ProfileResolver {
    profiles: HashMap<String, toml::Value>,
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

    /// Instantiate resolver pre-loaded with the 7 canonical profiles (SKL-02, D-11).
    pub fn with_canonical_profiles() -> Self {
        let mut resolver = Self::new();
        resolver.register_canonical_profiles();
        resolver
    }

    pub fn register_profile(&mut self, name: &str, profile_toml: toml::Value) {
        self.profiles.insert(name.to_string(), profile_toml);
    }

    fn register_canonical_profiles(&mut self) {
        // Canonical profiles are declarative packaged configuration in
        // `assets/profiles/*.toml`, loaded through the same parser/resolver
        // as external profiles (no Rust-embedded TOML blobs).
        const CANONICAL_ASSETS: &[(&str, &str)] = &[
            ("safe", include_str!("../../assets/profiles/safe.toml")),
            (
                "conservative",
                include_str!("../../assets/profiles/conservative.toml"),
            ),
            ("coding", include_str!("../../assets/profiles/coding.toml")),
            (
                "balanced",
                include_str!("../../assets/profiles/balanced.toml"),
            ),
            (
                "research",
                include_str!("../../assets/profiles/research.toml"),
            ),
            (
                "code_reviewer",
                include_str!("../../assets/profiles/code_reviewer.toml"),
            ),
            (
                "code-reviewer",
                include_str!("../../assets/profiles/code-reviewer.toml"),
            ),
            (
                "autonomous",
                include_str!("../../assets/profiles/autonomous.toml"),
            ),
            ("ci", include_str!("../../assets/profiles/ci.toml")),
            (
                "security_review",
                include_str!("../../assets/profiles/security_review.toml"),
            ),
            (
                "security-review",
                include_str!("../../assets/profiles/security-review.toml"),
            ),
            (
                "release",
                include_str!("../../assets/profiles/release.toml"),
            ),
        ];

        for (name, toml_str) in CANONICAL_ASSETS {
            if let Ok(val) = toml::from_str::<toml::Value>(toml_str) {
                self.register_profile(name, val);
            }
        }
    }

    /// Resolve and merge a named profile, recursively resolving its inheritance hierarchy.
    ///
    /// Single inheritance: child extends parent.
    /// Invariant: Merged profile cannot relax safety policies of parent (CFG-04).
    pub fn resolve_profile(&self, profile_name: &str) -> Result<toml::Value, ConfigError> {
        let mut visited = HashSet::new();
        self.resolve_profile_recursive(profile_name, &mut visited)
    }

    fn resolve_profile_recursive(
        &self,
        profile_name: &str,
        visited: &mut HashSet<String>,
    ) -> Result<toml::Value, ConfigError> {
        if !visited.insert(profile_name.to_string()) {
            return Err(ConfigError::CyclicProfileInheritance(
                profile_name.to_string(),
            ));
        }

        let raw = self
            .profiles
            .get(profile_name)
            .cloned()
            .ok_or_else(|| ConfigError::ProfileNotFound(profile_name.to_string()))?;

        // Check if extends parent
        let extends_opt = raw
            .get("extends")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let final_toml = if let Some(parent_name) = extends_opt {
            let mut parent_merged = self.resolve_profile_recursive(&parent_name, visited)?;

            // Monotonic security check: child cannot downgrade parent security
            MonotonicSecurityMerger::check(&parent_merged, &raw)?;

            // Deep merge child onto parent
            deep_merge_toml(&mut parent_merged, raw);
            parent_merged
        } else {
            raw
        };

        Ok(final_toml)
    }

    /// Compose a resolved profile onto a base AppConfig TOML.
    pub fn apply_to_app_config(
        &self,
        profile_name: &str,
        mut base_config_toml: toml::Value,
    ) -> Result<AppConfig, ConfigError> {
        let profile_toml = self.resolve_profile(profile_name)?;
        MonotonicSecurityMerger::check(&base_config_toml, &profile_toml)?;
        deep_merge_toml(&mut base_config_toml, profile_toml);

        base_config_toml
            .try_into()
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
        let normalized = profile_name.trim().to_lowercase().replace('-', "_");
        let resolver = Self::with_canonical_profiles();
        let resolved = resolver.resolve_profile(&normalized).or_else(|_| {
            // try raw id (covers already-canonical with hyphen kept)
            resolver.resolve_profile(profile_name)
        })?;
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
    /// profile TOML (inheritance applied) for the given id.
    pub fn canonical_metadata_for_profile(profile_name: &str) -> Result<toml::Value, ConfigError> {
        let normalized = profile_name.trim().to_lowercase().replace('-', "_");
        let resolver = Self::with_canonical_profiles();
        resolver
            .resolve_profile(&normalized)
            .or_else(|_| resolver.resolve_profile(profile_name))
    }

    /// Whether an id is a known canonical profile or alias.
    pub fn is_canonical_profile(profile_name: &str) -> bool {
        let normalized = profile_name.trim().to_lowercase().replace('-', "_");
        Self::canonical_profile_names().contains(&normalized.as_str())
            || normalized == "code_reviewer"
            || normalized == "security_review"
    }
}
