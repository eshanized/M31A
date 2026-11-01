//! Connector directory merging, catalog normalization, and accessibility sorting.
//! Merges connector catalogs, normalizes tool schemas, and filters accessible third-party services.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// High-level application/connector descriptor.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct AppInfo {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub is_accessible: bool,
    pub install_url: Option<String>,
    pub logo_url: Option<String>,
    pub logo_url_dark: Option<String>,
    pub distribution_channel: Option<String>,
    pub plugin_display_names: Vec<String>,
    pub tags: Vec<String>,
}

impl AppInfo {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            description: None,
            is_accessible: false,
            install_url: None,
            logo_url: None,
            logo_url_dark: None,
            distribution_channel: None,
            plugin_display_names: Vec::new(),
            tags: Vec::new(),
        }
    }
}

/// Computes the deep install URL for a given connector app.
pub fn connector_install_url(_name: &str, id: &str) -> String {
    let mut encoded = String::new();
    for b in id.bytes() {
        if b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'.' || b == b'~' {
            encoded.push(b as char);
        } else {
            encoded.push_str(&format!("%{:02X}", b));
        }
    }
    format!("https://chatgpt.com/apps/{}", encoded)
}

/// Sorts connectors so accessible connectors appear first, followed by alphabetical order.
pub fn sort_connectors_by_accessibility_and_name(connectors: &mut [AppInfo]) {
    connectors.sort_by(|a, b| {
        b.is_accessible
            .cmp(&a.is_accessible)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
}

/// Merges directory catalog connectors with locally accessible connectors from active MCP tools.
pub fn merge_connectors(
    connectors: Vec<AppInfo>,
    accessible_connectors: Vec<AppInfo>,
) -> Vec<AppInfo> {
    let mut merged: HashMap<String, AppInfo> = connectors
        .into_iter()
        .map(|mut c| {
            c.is_accessible = false;
            (c.id.clone(), c)
        })
        .collect();

    for mut c in accessible_connectors {
        c.is_accessible = true;
        let id = c.id.clone();
        if let Some(existing) = merged.get_mut(&id) {
            existing.is_accessible = true;
            if existing.name == existing.id && c.name != c.id {
                existing.name = c.name;
            }
            if existing.description.is_none() && c.description.is_some() {
                existing.description = c.description;
            }
            if existing.logo_url.is_none() && c.logo_url.is_some() {
                existing.logo_url = c.logo_url;
            }
            if existing.logo_url_dark.is_none() && c.logo_url_dark.is_some() {
                existing.logo_url_dark = c.logo_url_dark;
            }
            if existing.distribution_channel.is_none() && c.distribution_channel.is_some() {
                existing.distribution_channel = c.distribution_channel;
            }
            existing.plugin_display_names.extend(c.plugin_display_names);
        } else {
            merged.insert(id, c);
        }
    }

    let mut list = merged.into_values().collect::<Vec<_>>();
    for c in &mut list {
        if c.install_url.is_none() {
            c.install_url = Some(connector_install_url(&c.name, &c.id));
        }
        c.plugin_display_names.sort_unstable();
        c.plugin_display_names.dedup();
    }

    sort_connectors_by_accessibility_and_name(&mut list);
    list
}

/// Marks connectors matching plugin IDs as accessible.
pub fn merge_plugin_connectors<I>(connectors: Vec<AppInfo>, plugin_app_ids: I) -> Vec<AppInfo>
where
    I: IntoIterator<Item = String>,
{
    let id_set: std::collections::HashSet<String> = plugin_app_ids.into_iter().collect();
    let mut list = connectors;
    for c in &mut list {
        if id_set.contains(&c.id) {
            c.is_accessible = true;
        }
    }
    sort_connectors_by_accessibility_and_name(&mut list);
    list
}

/// Filters connectors by query keyword matching name, description, or tags.
pub fn filter_connectors(connectors: &[AppInfo], query: &str) -> Vec<AppInfo> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return connectors.to_vec();
    }

    connectors
        .iter()
        .filter(|c| {
            c.name.to_lowercase().contains(&q)
                || c.id.to_lowercase().contains(&q)
                || c.description
                    .as_deref()
                    .unwrap_or("")
                    .to_lowercase()
                    .contains(&q)
                || c.tags.iter().any(|t| t.to_lowercase().contains(&q))
        })
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_merge_and_sort_connectors() {
        let cat1 = AppInfo::new("app_github", "GitHub Connector");
        let cat2 = AppInfo::new("app_slack", "Slack Connector");

        let mut acc = AppInfo::new("app_slack", "Slack Integration");
        acc.description = Some("Slack notifications and channels".to_string());
        acc.plugin_display_names = vec!["Slack Plugin".to_string()];

        let merged = merge_connectors(vec![cat1, cat2], vec![acc]);

        assert_eq!(merged.len(), 2);
        // Slack is accessible, should be sorted first!
        assert_eq!(merged[0].id, "app_slack");
        assert!(merged[0].is_accessible);
        assert_eq!(
            merged[0].description,
            Some("Slack notifications and channels".to_string())
        );
        assert_eq!(merged[0].plugin_display_names, vec!["Slack Plugin"]);

        // GitHub is not accessible, should be second
        assert_eq!(merged[1].id, "app_github");
        assert!(!merged[1].is_accessible);
        assert!(merged[1].install_url.is_some());
    }

    #[test]
    fn test_filter_connectors() {
        let mut app1 = AppInfo::new("github", "GitHub");
        app1.tags = vec!["git".to_string(), "vcs".to_string()];

        let mut app2 = AppInfo::new("jira", "Atlassian Jira");
        app2.tags = vec!["tracking".to_string(), "issues".to_string()];

        let apps = vec![app1, app2];

        let res = filter_connectors(&apps, "git");
        assert_eq!(res.len(), 1);
        assert_eq!(res[0].id, "github");

        let res_empty = filter_connectors(&apps, "");
        assert_eq!(res_empty.len(), 2);
    }
}
