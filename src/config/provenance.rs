//! 7-Layer Configuration Provenance & Immutable Constraint Protection (CFX-04, D-07).
//!
//! Enforces the 7-tier precedence hierarchy (CLI > Session > Mission > Workspace > User > System > BuiltIn)
//! and prevents lower-precedence configuration layers from weakening immutable safety constraints.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use thiserror::Error;

use crate::config::hierarchy::ConfigTier;

/// 8-Layer configuration precedence hierarchy (CFG-01).
/// Higher integer precedence wins during resolution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum ConfigLayer {
    Tier0SecurityInvariants = 0,
    Tier1System = 1,
    Tier2User = 2,
    Tier3Workspace = 3,
    Tier4Profile = 4,
    Tier5Environment = 5,
    Tier6Cli = 6,
    Tier7Session = 7,
}

#[allow(non_upper_case_globals)]
impl ConfigLayer {
    pub const BuiltIn: Self = Self::Tier0SecurityInvariants;
    pub const System: Self = Self::Tier1System;
    pub const User: Self = Self::Tier2User;
    pub const Workspace: Self = Self::Tier3Workspace;
    pub const Profile: Self = Self::Tier4Profile;
    pub const Mission: Self = Self::Tier4Profile;
    pub const Environment: Self = Self::Tier5Environment;
    pub const Session: Self = Self::Tier7Session;
    pub const Cli: Self = Self::Tier6Cli;

    /// Return the numeric precedence rank (0 = lowest, 7 = highest).
    pub fn precedence(&self) -> u8 {
        *self as u8
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Tier0SecurityInvariants => "Security Invariants",
            Self::Tier1System => "System Config",
            Self::Tier2User => "User Global",
            Self::Tier3Workspace => "Workspace Repo",
            Self::Tier4Profile => "Profile",
            Self::Tier5Environment => "Environment",
            Self::Tier6Cli => "CLI Override",
            Self::Tier7Session => "Session Override",
        }
    }

    pub fn badge(&self) -> &'static str {
        match self {
            Self::Tier0SecurityInvariants => "[INVARIANT]",
            Self::Tier1System => "[SYSTEM]",
            Self::Tier2User => "[USER]",
            Self::Tier3Workspace => "[WORKSPACE]",
            Self::Tier4Profile => "[PROFILE]",
            Self::Tier5Environment => "[ENV]",
            Self::Tier6Cli => "[CLI]",
            Self::Tier7Session => "[SESSION]",
        }
    }
}

impl From<ConfigLayer> for ConfigTier {
    fn from(layer: ConfigLayer) -> Self {
        match layer {
            ConfigLayer::Tier0SecurityInvariants => ConfigTier::Tier0SecurityInvariants,
            ConfigLayer::Tier1System => ConfigTier::Tier1System,
            ConfigLayer::Tier2User => ConfigTier::Tier2User,
            ConfigLayer::Tier3Workspace => ConfigTier::Tier3Workspace,
            ConfigLayer::Tier4Profile => ConfigTier::Tier4Profile,
            ConfigLayer::Tier5Environment => ConfigTier::Tier5Environment,
            ConfigLayer::Tier6Cli => ConfigTier::Tier6Cli,
            ConfigLayer::Tier7Session => ConfigTier::Tier7Session,
        }
    }
}

impl From<ConfigTier> for ConfigLayer {
    fn from(tier: ConfigTier) -> Self {
        match tier {
            ConfigTier::Tier0SecurityInvariants => ConfigLayer::Tier0SecurityInvariants,
            ConfigTier::Tier1System => ConfigLayer::Tier1System,
            ConfigTier::Tier2User => ConfigLayer::Tier2User,
            ConfigTier::Tier3Workspace => ConfigLayer::Tier3Workspace,
            ConfigTier::Tier4Profile => ConfigLayer::Tier4Profile,
            ConfigTier::Tier5Environment => ConfigLayer::Tier5Environment,
            ConfigTier::Tier6Cli => ConfigLayer::Tier6Cli,
            ConfigTier::Tier7Session => ConfigLayer::Tier7Session,
        }
    }
}

/// Errors originating from configuration provenance resolution.
#[derive(Debug, Error)]
pub enum ProvenanceError {
    #[error(
        "Cannot override immutable constraint '{key}' set by layer {existing_layer:?} from lower layer {attempted_layer:?}"
    )]
    ImmutableConstraintViolation {
        key: String,
        existing_layer: ConfigLayer,
        attempted_layer: ConfigLayer,
    },
}

/// A fully resolved configuration setting with complete origin tracking.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ResolvedValue {
    pub key: String,
    pub value: serde_json::Value,
    pub layer: ConfigLayer,
    pub source_file: Option<PathBuf>,
    pub is_immutable: bool,
}

/// Internal per-layer configuration entry.
#[derive(Debug, Clone, PartialEq)]
struct LayerEntry {
    value: serde_json::Value,
    source_file: Option<PathBuf>,
    is_immutable: bool,
}

/// Configuration service tracking layered entries and resolving precedence.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct ConfigurationService {
    // Layer -> (Key -> LayerEntry)
    layers: BTreeMap<ConfigLayer, HashMap<String, LayerEntry>>,
}

impl ConfigurationService {
    pub fn new() -> Self {
        Self {
            layers: BTreeMap::new(),
        }
    }

    /// Set a configuration value at a designated layer.
    ///
    /// If an immutable constraint has been defined at any higher or equal layer,
    /// an attempt to override it with a different value from a lower layer is strictly rejected (T-13-09).
    pub fn set_value(
        &mut self,
        layer: ConfigLayer,
        key: &str,
        value: serde_json::Value,
        source_file: Option<PathBuf>,
        is_immutable: bool,
    ) -> Result<(), ProvenanceError> {
        // Verify immutable constraints across all registered layers
        for (&existing_layer, entries) in &self.layers {
            if let Some(existing_entry) = entries.get(key) {
                if existing_entry.is_immutable && existing_layer <= layer && layer < existing_layer
                {
                    return Err(ProvenanceError::ImmutableConstraintViolation {
                        key: key.to_string(),
                        existing_layer,
                        attempted_layer: layer,
                    });
                }
                // System or BuiltIn immutable constraints cannot be altered by other layers
                if existing_entry.is_immutable
                    && (existing_layer == ConfigLayer::Tier0SecurityInvariants
                        || existing_layer == ConfigLayer::Tier1System)
                    && layer != existing_layer
                {
                    return Err(ProvenanceError::ImmutableConstraintViolation {
                        key: key.to_string(),
                        existing_layer,
                        attempted_layer: layer,
                    });
                }
            }
        }

        let layer_map = self.layers.entry(layer).or_default();
        layer_map.insert(
            key.to_string(),
            LayerEntry {
                value,
                source_file,
                is_immutable,
            },
        );

        Ok(())
    }

    /// Resolve a specific key following the 8-layer precedence hierarchy.
    /// Evaluates from Session (7) down to SecurityInvariants (0).
    pub fn resolve(&self, key: &str) -> Option<ResolvedValue> {
        let layers_descending = [
            ConfigLayer::Tier7Session,
            ConfigLayer::Tier6Cli,
            ConfigLayer::Tier5Environment,
            ConfigLayer::Tier4Profile,
            ConfigLayer::Tier3Workspace,
            ConfigLayer::Tier2User,
            ConfigLayer::Tier1System,
            ConfigLayer::Tier0SecurityInvariants,
        ];

        for layer in layers_descending {
            if let Some(entries) = self.layers.get(&layer)
                && let Some(entry) = entries.get(key)
            {
                return Some(ResolvedValue {
                    key: key.to_string(),
                    value: entry.value.clone(),
                    layer,
                    source_file: entry.source_file.clone(),
                    is_immutable: entry.is_immutable,
                });
            }
        }

        None
    }

    /// Resolve all distinct keys registered across all layers.
    pub fn all_resolved(&self) -> Vec<ResolvedValue> {
        let mut keys = std::collections::BTreeSet::new();
        for entries in self.layers.values() {
            for key in entries.keys() {
                keys.insert(key.clone());
            }
        }

        let mut resolved = Vec::new();
        for key in keys {
            if let Some(val) = self.resolve(&key) {
                resolved.push(val);
            }
        }

        resolved
    }

    /// Group all resolved settings by domain (Providers, Models, Autonomy, Security, UI).
    pub fn grouped_by_domain(&self) -> BTreeMap<String, Vec<ResolvedValue>> {
        let all = self.all_resolved();
        let mut grouped: BTreeMap<String, Vec<ResolvedValue>> = BTreeMap::new();

        for item in all {
            let domain = if item.key.starts_with("provider.") {
                "Providers".to_string()
            } else if item.key.starts_with("model.") {
                "Models".to_string()
            } else if item.key.starts_with("autonomy.") {
                "Autonomy".to_string()
            } else if item.key.starts_with("security.") {
                "Security".to_string()
            } else if item.key.starts_with("ui.") {
                "UI".to_string()
            } else {
                "General".to_string()
            };

            grouped.entry(domain).or_default().push(item);
        }

        grouped
    }

    /// Return all registered layer entries for a given key in ascending layer order.
    pub fn get_all_entries_for_key(
        &self,
        key: &str,
    ) -> Vec<(ConfigLayer, serde_json::Value, Option<PathBuf>, bool)> {
        let mut entries = Vec::new();
        for (&layer, map) in &self.layers {
            if let Some(entry) = map.get(key) {
                entries.push((
                    layer,
                    entry.value.clone(),
                    entry.source_file.clone(),
                    entry.is_immutable,
                ));
            }
        }
        entries
    }
}
