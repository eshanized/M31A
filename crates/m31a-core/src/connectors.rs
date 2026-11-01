//! Third-party service connector definitions, authentication schemes, and API bindings.
//!
//! Provides configuration models and integration interfaces for external service connectors.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

pub const CONNECTORS_CACHE_TTL: Duration = Duration::from_secs(3600);

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppBranding {
    pub category: Option<String>,
    pub developer: Option<String>,
    pub website: Option<String>,
    pub privacy_policy: Option<String>,
    pub terms_of_service: Option<String>,
    pub is_discoverable: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppReview {
    pub status: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppScreenshot {
    pub url: Option<String>,
    pub file_id: Option<String>,
    pub user_prompt: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppMetadata {
    pub review: Option<AppReview>,
    pub categories: Option<Vec<String>>,
    pub developer: Option<String>,
    pub version: Option<String>,
    pub screenshots: Option<Vec<AppScreenshot>>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ConnectorToolSummary {
    pub name: String,
    pub description: String,
    pub parameters_schema: serde_json::Value,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ConnectorMetadata {
    pub app_id: String,
    pub name: String,
    pub description: String,
    pub branding: Option<AppBranding>,
    pub metadata: Option<AppMetadata>,
    pub tools: Vec<ConnectorToolSummary>,
    pub updated_at_epoch_ms: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct AppToolPolicy {
    pub app_id: String,
    pub is_enabled: bool,
    pub allowed_tools: Option<Vec<String>>,
    pub blocked_tools: Option<Vec<String>>,
}

impl AppToolPolicy {
    pub fn is_tool_allowed(&self, tool_name: &str) -> bool {
        if !self.is_enabled {
            return false;
        }
        if let Some(ref blocked) = self.blocked_tools
            && blocked.iter().any(|b| b == tool_name)
        {
            return false;
        }
        if let Some(ref allowed) = self.allowed_tools {
            return allowed.iter().any(|a| a == tool_name);
        }
        true
    }
}

pub struct ConnectorDirectoryCache {
    cache_dir: PathBuf,
}

impl ConnectorDirectoryCache {
    pub fn new(cache_dir: PathBuf) -> Self {
        Self { cache_dir }
    }

    pub fn default_dir() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        PathBuf::from(home)
            .join(".cache")
            .join("m31a")
            .join("connectors")
    }

    pub fn store(&self, connector: &ConnectorMetadata) -> Result<()> {
        fs::create_dir_all(&self.cache_dir)?;
        let path = self.cache_dir.join(format!("{}.json", connector.app_id));
        let data = serde_json::to_string_pretty(connector)?;
        fs::write(path, data)?;
        Ok(())
    }

    pub fn load(&self, app_id: &str) -> Option<ConnectorMetadata> {
        let path = self.cache_dir.join(format!("{}.json", app_id));
        if !path.exists() {
            return None;
        }

        if let Ok(data) = fs::read_to_string(&path)
            && let Ok(metadata) = serde_json::from_str::<ConnectorMetadata>(&data)
        {
            let now = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64;

            let age_ms = now.saturating_sub(metadata.updated_at_epoch_ms);
            if Duration::from_millis(age_ms) < CONNECTORS_CACHE_TTL {
                return Some(metadata);
            }
        }
        None
    }

    pub fn list_cached_apps(&self) -> Vec<String> {
        let mut apps = Vec::new();
        if let Ok(entries) = fs::read_dir(&self.cache_dir) {
            for entry in entries.filter_map(Result::ok) {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("json")
                    && let Some(stem) = path.file_stem().and_then(|s| s.to_str())
                {
                    apps.push(stem.to_string());
                }
            }
        }
        apps.sort();
        apps
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_app_tool_policy() {
        let policy = AppToolPolicy {
            app_id: "app_github".to_string(),
            is_enabled: true,
            allowed_tools: Some(vec!["create_issue".to_string(), "list_repos".to_string()]),
            blocked_tools: Some(vec!["delete_repo".to_string()]),
        };

        assert!(policy.is_tool_allowed("create_issue"));
        assert!(!policy.is_tool_allowed("delete_repo"));
        assert!(!policy.is_tool_allowed("star_repo")); // Not in allowed list

        let disabled_policy = AppToolPolicy {
            app_id: "app_disabled".to_string(),
            is_enabled: false,
            allowed_tools: None,
            blocked_tools: None,
        };
        assert!(!disabled_policy.is_tool_allowed("any_tool"));
    }

    #[test]
    fn test_connector_directory_cache_roundtrip() {
        let dir = tempdir().unwrap();
        let cache = ConnectorDirectoryCache::new(dir.path().to_path_buf());

        let now_ms = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        let connector = ConnectorMetadata {
            app_id: "linear_app".to_string(),
            name: "Linear Integration".to_string(),
            description: "Manage Linear issues and projects".to_string(),
            branding: Some(AppBranding {
                category: Some("Productivity".to_string()),
                developer: Some("Linear".to_string()),
                website: Some("https://linear.app".to_string()),
                privacy_policy: None,
                terms_of_service: None,
                is_discoverable: true,
            }),
            metadata: None,
            tools: vec![ConnectorToolSummary {
                name: "create_ticket".to_string(),
                description: "Creates an issue in Linear".to_string(),
                parameters_schema: serde_json::json!({ "type": "object" }),
            }],
            updated_at_epoch_ms: now_ms,
        };

        cache.store(&connector).unwrap();

        let loaded = cache.load("linear_app").unwrap();
        assert_eq!(loaded.name, "Linear Integration");
        assert_eq!(loaded.tools.len(), 1);

        let cached_apps = cache.list_cached_apps();
        assert_eq!(cached_apps, vec!["linear_app".to_string()]);
    }
}
