//! 7-Tier Hierarchical Configuration Engine (CFG-01, D-12).

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::config::merge::{ConfigError, MonotonicSecurityMerger, deep_merge_toml};

/// 8-Tier authoritative configuration precedence ordering (CFG-01).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum ConfigTier {
    /// Tier 0: Immutable Security Invariants hardcoded in the kernel binary.
    Tier0SecurityInvariants = 0,
    /// Tier 1: System-wide machine configuration (/etc/m31a).
    Tier1System = 1,
    /// Tier 2: User global configuration (~/.config/m31a).
    Tier2User = 2,
    /// Tier 3: Workspace repository configuration (<repo>/.m31a/config.toml).
    Tier3Workspace = 3,
    /// Tier 4: Composable execution profile (safe, coding, autonomous, etc.).
    Tier4Profile = 4,
    /// Tier 5: Environment variable overrides (M31A_*, std::env).
    Tier5Environment = 5,
    /// Tier 6: CLI command-line argument overrides (--config, --model, etc.).
    Tier6Cli = 6,
    /// Tier 7: Interactive session dynamic overrides (/model, /profile).
    Tier7Session = 7,
}

#[allow(non_upper_case_globals)]
impl ConfigTier {
    pub const BuiltinDefaults: Self = Self::Tier0SecurityInvariants;
    pub const System: Self = Self::Tier1System;
    pub const User: Self = Self::Tier2User;
    pub const Workspace: Self = Self::Tier3Workspace;
    pub const Profile: Self = Self::Tier4Profile;
    pub const Mission: Self = Self::Tier4Profile;
    pub const Environment: Self = Self::Tier5Environment;
    pub const SessionOverrides: Self = Self::Tier7Session;
    pub const CliOverrides: Self = Self::Tier6Cli;

    pub fn name(&self) -> &'static str {
        match self {
            Self::Tier0SecurityInvariants => "Tier0SecurityInvariants",
            Self::Tier1System => "Tier1System",
            Self::Tier2User => "Tier2User",
            Self::Tier3Workspace => "Tier3Workspace",
            Self::Tier4Profile => "Tier4Profile",
            Self::Tier5Environment => "Tier5Environment",
            Self::Tier6Cli => "Tier6Cli",
            Self::Tier7Session => "Tier7Session",
        }
    }
}

/// Precedence engine maintaining tiered configuration layers and resolving final config.
#[derive(Default, Clone)]
pub struct ConfigPrecedenceEngine {
    layers: BTreeMap<ConfigTier, toml::Value>,
}

impl ConfigPrecedenceEngine {
    pub fn new() -> Self {
        Self {
            layers: BTreeMap::new(),
        }
    }

    /// Set or replace the configuration TOML value for a specific tier.
    pub fn set_layer(&mut self, tier: ConfigTier, value: toml::Value) {
        self.layers.insert(tier, value);
    }

    /// Set layer from a TOML string.
    pub fn set_layer_from_str(
        &mut self,
        tier: ConfigTier,
        toml_str: &str,
    ) -> Result<(), ConfigError> {
        let val: toml::Value = toml_str
            .parse()
            .map_err(|e: toml::de::Error| ConfigError::TomlParseError(e.to_string()))?;
        self.set_layer(tier, val);
        Ok(())
    }

    /// Remove a layer.
    pub fn remove_layer(&mut self, tier: ConfigTier) -> Option<toml::Value> {
        self.layers.remove(&tier)
    }

    /// Check if a tier layer is present.
    pub fn has_layer(&self, tier: ConfigTier) -> bool {
        self.layers.contains_key(&tier)
    }

    /// Get a reference to a tier layer if present.
    pub fn get_layer(&self, tier: ConfigTier) -> Option<&toml::Value> {
        self.layers.get(&tier)
    }

    /// Resolve the complete configuration as a raw merged toml::Value.
    ///
    /// Iterates tiers in order BuiltinDefaults -> System -> User -> Workspace -> Mission -> SessionOverrides -> CliOverrides.
    /// At each step, MonotonicSecurityMerger validates that incoming lower tiers do not downgrade security.
    pub fn resolve_raw(&self) -> Result<toml::Value, ConfigError> {
        let mut base = toml::Value::Table(toml::map::Map::new());

        for layer in self.layers.values() {
            // Check security monotonic accumulation
            MonotonicSecurityMerger::check(&base, layer)?;

            // Apply deep merge
            deep_merge_toml(&mut base, layer.clone());
        }

        Ok(base)
    }

    /// Resolve the merged configuration into a strongly typed struct.
    pub fn resolve<T: serde::de::DeserializeOwned>(&self) -> Result<T, ConfigError> {
        let raw = self.resolve_raw()?;
        raw.try_into()
            .map_err(|e| ConfigError::ValidationError(e.to_string()))
    }
}
