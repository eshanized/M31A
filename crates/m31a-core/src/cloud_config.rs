//! Enterprise-managed cloud configuration bundle loader and layering service.
//!
//! Enterprise cloud configuration profiles, policy bundles, and remote settings synchronization:
//! - Groups enterprise-managed configuration and requirements fragments.
//! - Merges cloud policies (highest priority first for config, reverse order for requirements).
//! - Enforces security and feature limits (e.g. rbac fast-mode restrictions).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, Instant};

pub const CLOUD_CONFIG_BUNDLE_TIMEOUT: Duration = Duration::from_secs(10);
pub const DEFAULT_CACHE_TTL: Duration = Duration::from_secs(300);

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CloudConfigFragment {
    pub id: String,
    pub name: String,
    pub contents: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CloudRequirementsFragment {
    pub id: String,
    pub name: String,
    pub contents: String,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct CloudConfigTomlBundle {
    pub enterprise_managed: Vec<CloudConfigFragment>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct CloudRequirementsTomlBundle {
    pub enterprise_managed: Vec<CloudRequirementsFragment>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct CloudConfigBundle {
    pub config_toml: CloudConfigTomlBundle,
    pub requirements_toml: CloudRequirementsTomlBundle,
}

impl CloudConfigBundle {
    pub fn is_empty(&self) -> bool {
        self.config_toml.enterprise_managed.is_empty()
            && self.requirements_toml.enterprise_managed.is_empty()
    }

    /// Merges enterprise-managed requirements fragments into an aggregate TOML string.
    pub fn merge_requirements_toml(&self) -> String {
        let mut merged = String::new();
        // Requirements fragments merge in reverse order (lowest to highest priority)
        for fragment in self.requirements_toml.enterprise_managed.iter().rev() {
            merged.push_str(&format!(
                "# Fragment: {} ({})\n",
                fragment.name, fragment.id
            ));
            merged.push_str(&fragment.contents);
            merged.push('\n');
        }
        merged
    }

    /// Merges enterprise-managed config fragments into an aggregate TOML string.
    pub fn merge_config_toml(&self) -> String {
        let mut merged = String::new();
        for fragment in &self.config_toml.enterprise_managed {
            merged.push_str(&format!("# Config: {} ({})\n", fragment.name, fragment.id));
            merged.push_str(&fragment.contents);
            merged.push('\n');
        }
        merged
    }
}

#[derive(Debug)]
pub struct CloudConfigCacheEntry {
    pub bundle: CloudConfigBundle,
    pub loaded_at: Instant,
    pub ttl: Duration,
}

impl CloudConfigCacheEntry {
    pub fn is_expired(&self) -> bool {
        self.loaded_at.elapsed() > self.ttl
    }
}

#[derive(Debug, Default)]
pub struct CloudConfigService {
    cache: HashMap<String, CloudConfigCacheEntry>,
}

impl CloudConfigService {
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
        }
    }

    pub fn insert_bundle(&mut self, organization_id: &str, bundle: CloudConfigBundle) {
        self.cache.insert(
            organization_id.to_string(),
            CloudConfigCacheEntry {
                bundle,
                loaded_at: Instant::now(),
                ttl: DEFAULT_CACHE_TTL,
            },
        );
    }

    pub fn get_bundle(&self, organization_id: &str) -> Option<&CloudConfigBundle> {
        self.cache
            .get(organization_id)
            .filter(|entry| !entry.is_expired())
            .map(|entry| &entry.bundle)
    }

    pub fn invalidate(&mut self, organization_id: &str) {
        self.cache.remove(organization_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cloud_config_bundle_empty() {
        let bundle = CloudConfigBundle::default();
        assert!(bundle.is_empty());
    }

    #[test]
    fn test_cloud_config_bundle_merge() {
        let bundle = CloudConfigBundle {
            config_toml: CloudConfigTomlBundle {
                enterprise_managed: vec![CloudConfigFragment {
                    id: "sec-1".to_string(),
                    name: "Network Sandbox Rule".to_string(),
                    contents: "network = \"restricted\"\n".to_string(),
                }],
            },
            requirements_toml: CloudRequirementsTomlBundle {
                enterprise_managed: vec![CloudRequirementsFragment {
                    id: "rbac-fast-mode".to_string(),
                    name: "Fast Mode Disablement".to_string(),
                    contents: "[features]\nfast_mode = false\n".to_string(),
                }],
            },
        };

        assert!(!bundle.is_empty());
        let config_str = bundle.merge_config_toml();
        assert!(config_str.contains("network = \"restricted\""));

        let req_str = bundle.merge_requirements_toml();
        assert!(req_str.contains("fast_mode = false"));
    }

    #[test]
    fn test_cloud_config_service_caching() {
        let mut svc = CloudConfigService::new();
        let bundle = CloudConfigBundle {
            config_toml: CloudConfigTomlBundle {
                enterprise_managed: vec![CloudConfigFragment {
                    id: "org-pol".to_string(),
                    name: "Org Policy".to_string(),
                    contents: "model = \"m31a-custom\"\n".to_string(),
                }],
            },
            requirements_toml: CloudRequirementsTomlBundle::default(),
        };

        svc.insert_bundle("org-123", bundle);
        let retrieved = svc.get_bundle("org-123");
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().config_toml.enterprise_managed.len(), 1);

        svc.invalidate("org-123");
        assert!(svc.get_bundle("org-123").is_none());
    }
}
