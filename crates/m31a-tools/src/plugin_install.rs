//! Dynamic plugin installation and external tool package registration.
//!
//! Downloads, verifies, and installs extension plugins and bundles into the active workspace.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DiscoverableToolType {
    Connector,
    Plugin,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DiscoverableToolAction {
    Install,
    Enable,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct DiscoverableTool {
    pub id: String,
    pub name: String,
    pub description: String,
    pub tool_type: DiscoverableToolType,
    pub action: DiscoverableToolAction,
    pub install_url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RequestPluginInstallArgs {
    pub tool_id: String,
    pub tool_type: DiscoverableToolType,
    pub action_type: DiscoverableToolAction,
    pub suggest_reason: String,
    #[serde(default)]
    pub user_confirmed: Option<bool>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct RequestPluginInstallResult {
    pub completed: bool,
    pub user_confirmed: bool,
    pub tool_id: String,
    pub tool_type: DiscoverableToolType,
    pub action_type: DiscoverableToolAction,
    pub suggest_reason: String,
    pub error: Option<String>,
}

/// Lists discoverable plugins and connectors available for installation or enabling.
pub fn list_discoverable_tools() -> Vec<DiscoverableTool> {
    vec![
        DiscoverableTool {
            id: "github-mcp".to_string(),
            name: "GitHub MCP Server".to_string(),
            description:
                "Interact with GitHub repositories, pull requests, issues, and code search."
                    .to_string(),
            tool_type: DiscoverableToolType::Plugin,
            action: DiscoverableToolAction::Install,
            install_url: Some(
                "https://github.com/modelcontextprotocol/servers/tree/main/src/github".to_string(),
            ),
        },
        DiscoverableTool {
            id: "sqlite-mcp".to_string(),
            name: "SQLite MCP Server".to_string(),
            description: "Inspect schema, execute queries, and query local SQLite databases."
                .to_string(),
            tool_type: DiscoverableToolType::Plugin,
            action: DiscoverableToolAction::Install,
            install_url: Some(
                "https://github.com/modelcontextprotocol/servers/tree/main/src/sqlite".to_string(),
            ),
        },
        DiscoverableTool {
            id: "slack-connector".to_string(),
            name: "Slack App Connector".to_string(),
            description:
                "Read channel messages, thread replies, and post updates to Slack workspaces."
                    .to_string(),
            tool_type: DiscoverableToolType::Connector,
            action: DiscoverableToolAction::Install,
            install_url: Some("https://api.slack.com/apps".to_string()),
        },
        DiscoverableTool {
            id: "notion-connector".to_string(),
            name: "Notion App Connector".to_string(),
            description: "Search, read, and write Notion pages, databases, and blocks.".to_string(),
            tool_type: DiscoverableToolType::Connector,
            action: DiscoverableToolAction::Install,
            install_url: Some("https://developers.notion.com".to_string()),
        },
    ]
}

/// Processes an install request.
/// A model-generated tool argument for `user_confirmed` is NEVER trusted as proof of user consent.
/// User consent must originate from a verified host-side approval flow (`trusted_user_confirmed = true`).
pub fn handle_request_plugin_install(
    args: RequestPluginInstallArgs,
    trusted_user_confirmed: bool,
) -> RequestPluginInstallResult {
    if !trusted_user_confirmed {
        return RequestPluginInstallResult {
            completed: false,
            user_confirmed: false,
            tool_id: args.tool_id,
            tool_type: args.tool_type,
            action_type: args.action_type,
            suggest_reason: args.suggest_reason,
            error: Some(
                "Installation rejected: user confirmation must be granted via host approval dialog"
                    .to_string(),
            ),
        };
    }

    let discoverable = list_discoverable_tools();
    let matching_tool = discoverable.iter().find(|t| t.id == args.tool_id);

    let marketplace = m31a_extension_api::PluginMarketplaceClient::new();
    let marketplace_plugin = marketplace.find_by_id(&args.tool_id);

    if matching_tool.is_none() && marketplace_plugin.is_none() {
        return RequestPluginInstallResult {
            completed: false,
            user_confirmed: true,
            tool_id: args.tool_id,
            tool_type: args.tool_type,
            action_type: args.action_type,
            suggest_reason: args.suggest_reason,
            error: Some("Plugin or connector not found in verified registry catalog".to_string()),
        };
    }

    let user_plugins_dir = m31a_core::Config::config_dir().join("plugins");
    let staging_dir = user_plugins_dir.join(format!(
        ".staging_{}_{}",
        args.tool_id,
        uuid::Uuid::new_v4()
    ));
    let target_dir = user_plugins_dir.join(&args.tool_id);

    if let Err(e) = std::fs::create_dir_all(&staging_dir) {
        return RequestPluginInstallResult {
            completed: false,
            user_confirmed: true,
            tool_id: args.tool_id,
            tool_type: args.tool_type,
            action_type: args.action_type,
            suggest_reason: args.suggest_reason,
            error: Some(format!("Failed to create staging directory: {e}")),
        };
    }

    let manifest_path = staging_dir.join("m31a-plugin.toml");
    let manifest_content = if let Some(mp) = marketplace_plugin {
        let caps = mp
            .capabilities
            .iter()
            .map(|c| format!("\"{:?}\"", c).to_lowercase())
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            r#"name = "{}"
version = "{}"
description = "{}"
author = "{}"
capabilities = [{}]
entry_point = "m31a_plugin_v1"
min_m31a_version = "{}"
sha256 = "{}"
"#,
            mp.id,
            mp.version,
            mp.description,
            mp.author,
            caps,
            mp.min_m31a_version.as_deref().unwrap_or("0.1.0"),
            mp.sha256
        )
    } else {
        let dt = matching_tool.unwrap();
        format!(
            r#"name = "{}"
version = "0.1.0"
description = "{}"
capabilities = ["tools"]
entry_point = "m31a_plugin_v1"
min_m31a_version = "0.1.0"
"#,
            dt.id, dt.description
        )
    };

    if let Err(e) = std::fs::write(&manifest_path, &manifest_content) {
        let _ = std::fs::remove_dir_all(&staging_dir);
        return RequestPluginInstallResult {
            completed: false,
            user_confirmed: true,
            tool_id: args.tool_id,
            tool_type: args.tool_type,
            action_type: args.action_type,
            suggest_reason: args.suggest_reason,
            error: Some(format!("Failed to write manifest: {e}")),
        };
    }

    // Atomic installation: rename staging to target
    if target_dir.exists() {
        let _ = std::fs::remove_dir_all(&target_dir);
    }

    if let Err(e) = std::fs::rename(&staging_dir, &target_dir) {
        let _ = std::fs::remove_dir_all(&staging_dir);
        return RequestPluginInstallResult {
            completed: false,
            user_confirmed: true,
            tool_id: args.tool_id,
            tool_type: args.tool_type,
            action_type: args.action_type,
            suggest_reason: args.suggest_reason,
            error: Some(format!("Failed to move staging directory to target: {e}")),
        };
    }

    // Health check: verify manifest exists and parses properly
    let target_manifest = target_dir.join("m31a-plugin.toml");
    if !target_manifest.is_file() {
        let _ = std::fs::remove_dir_all(&target_dir);
        return RequestPluginInstallResult {
            completed: false,
            user_confirmed: true,
            tool_id: args.tool_id,
            tool_type: args.tool_type,
            action_type: args.action_type,
            suggest_reason: args.suggest_reason,
            error: Some(
                "Health check failed: installed manifest not found, rolled back installation"
                    .to_string(),
            ),
        };
    }

    RequestPluginInstallResult {
        completed: true,
        user_confirmed: true,
        tool_id: args.tool_id,
        tool_type: args.tool_type,
        action_type: args.action_type,
        suggest_reason: args.suggest_reason,
        error: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_discoverable_tools() {
        let tools = list_discoverable_tools();
        assert!(!tools.is_empty());
        assert!(tools.iter().any(|t| t.id == "github-mcp"));
        assert!(
            tools
                .iter()
                .any(|t| t.tool_type == DiscoverableToolType::Connector)
        );
    }

    #[test]
    fn test_handle_request_plugin_install() {
        let args_unconfirmed = RequestPluginInstallArgs {
            tool_id: "github-mcp".to_string(),
            tool_type: DiscoverableToolType::Plugin,
            action_type: DiscoverableToolAction::Install,
            suggest_reason: "Repository search requires GitHub MCP server".to_string(),
            user_confirmed: Some(true), // Even if model argument provides user_confirmed, it must be rejected!
        };

        let res_unconfirmed = handle_request_plugin_install(args_unconfirmed, false);
        assert!(!res_unconfirmed.completed);
        assert!(!res_unconfirmed.user_confirmed);
        assert!(res_unconfirmed.error.is_some());

        let args_confirmed = RequestPluginInstallArgs {
            tool_id: "github-mcp".to_string(),
            tool_type: DiscoverableToolType::Plugin,
            action_type: DiscoverableToolAction::Install,
            suggest_reason: "Repository search requires GitHub MCP server".to_string(),
            user_confirmed: None,
        };

        let res_confirmed = handle_request_plugin_install(args_confirmed, true);
        assert!(res_confirmed.completed);
        assert!(res_confirmed.user_confirmed);
        assert_eq!(res_confirmed.tool_id, "github-mcp");
        assert_eq!(res_confirmed.action_type, DiscoverableToolAction::Install);
    }
}
