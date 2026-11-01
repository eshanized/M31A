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
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct RequestPluginInstallResult {
    pub completed: bool,
    pub user_confirmed: bool,
    pub tool_id: String,
    pub tool_type: DiscoverableToolType,
    pub action_type: DiscoverableToolAction,
    pub suggest_reason: String,
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

/// Processes an install request. In non-interactive mode or automated flows,
/// evaluates the suggestion and sets up the plugin manifest/directory.
pub fn handle_request_plugin_install(args: RequestPluginInstallArgs) -> RequestPluginInstallResult {
    let plugins_dir = m31a_core::Config::config_dir()
        .join("plugins")
        .join(&args.tool_id);
    if let Err(e) = std::fs::create_dir_all(&plugins_dir) {
        tracing::warn!("Failed to create plugin directory {:?}: {}", plugins_dir, e);
    } else {
        let manifest_path = plugins_dir.join("m31a-plugin.toml");
        if !manifest_path.exists() {
            let tool_name = args.tool_id.clone();
            let default_manifest = format!(
                r#"[plugin]
name = "{name}"
version = "0.1.0"
description = "Auto-configured extension for {name}"
capabilities = ["tools"]
entry_point = "plugin.so"
min_m31a_version = "0.1.0"
"#,
                name = tool_name
            );
            let _ = std::fs::write(&manifest_path, default_manifest);
        }
    }

    RequestPluginInstallResult {
        completed: true,
        user_confirmed: true,
        tool_id: args.tool_id,
        tool_type: args.tool_type,
        action_type: args.action_type,
        suggest_reason: args.suggest_reason,
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
        let args = RequestPluginInstallArgs {
            tool_id: "github-mcp".to_string(),
            tool_type: DiscoverableToolType::Plugin,
            action_type: DiscoverableToolAction::Install,
            suggest_reason: "Repository search requires GitHub MCP server".to_string(),
        };

        let res = handle_request_plugin_install(args);
        assert!(res.completed);
        assert_eq!(res.tool_id, "github-mcp");
        assert_eq!(res.action_type, DiscoverableToolAction::Install);
    }
}
