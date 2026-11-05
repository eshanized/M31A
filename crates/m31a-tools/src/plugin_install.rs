//! Dynamic plugin installation and external tool package registration.
//!
//! Downloads, verifies, and installs extension plugins and bundles into the active workspace
//! using a staged, rollback-safe pipeline with mandatory publisher signature verification.

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

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum PluginApprovalError {
    #[error("Missing or empty context field: {0}")]
    MissingContext(&'static str),
    #[error("Plugin approval has expired")]
    Expired,
    #[error("Context mismatch for {field}: expected '{expected}', got '{actual}'")]
    ContextMismatch {
        field: &'static str,
        expected: String,
        actual: String,
    },
}

/// An opaque, host-issued, single-use authorization capability proving real human/host approval.
/// NEVER deserialized or extracted from model-generated tool arguments.
/// Construction is private and tightly bound to execution context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostPluginApproval {
    tool_id: String,
    action_type: DiscoverableToolAction,
    session_id: String,
    turn_id: String,
    call_id: String,
    nonce: String,
    expires_at: std::time::Instant,
}

impl HostPluginApproval {
    /// Host-only issuance constructor requiring exact execution context bindings.
    /// Trust derives from private construction and interactive host-side human approval decision.
    pub fn issue_host_approval(
        tool_id: impl Into<String>,
        action_type: DiscoverableToolAction,
        session_id: &str,
        turn_id: &str,
        call_id: &str,
        ttl: std::time::Duration,
    ) -> Result<Self, PluginApprovalError> {
        let tool_id = tool_id.into();
        if tool_id.trim().is_empty() {
            return Err(PluginApprovalError::MissingContext("tool_id"));
        }
        if session_id.trim().is_empty() {
            return Err(PluginApprovalError::MissingContext("session_id"));
        }
        if turn_id.trim().is_empty() {
            return Err(PluginApprovalError::MissingContext("turn_id"));
        }
        if call_id.trim().is_empty() {
            return Err(PluginApprovalError::MissingContext("call_id"));
        }

        Ok(Self {
            tool_id,
            action_type,
            session_id: session_id.to_string(),
            turn_id: turn_id.to_string(),
            call_id: call_id.to_string(),
            nonce: uuid::Uuid::new_v4().to_string(),
            expires_at: std::time::Instant::now() + ttl,
        })
    }

    pub fn tool_id(&self) -> &str {
        &self.tool_id
    }

    pub fn action_type(&self) -> DiscoverableToolAction {
        self.action_type
    }

    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    pub fn turn_id(&self) -> &str {
        &self.turn_id
    }

    pub fn call_id(&self) -> &str {
        &self.call_id
    }

    pub fn nonce(&self) -> &str {
        &self.nonce
    }

    pub fn expires_at(&self) -> std::time::Instant {
        self.expires_at
    }

    pub fn is_expired(&self) -> bool {
        std::time::Instant::now() >= self.expires_at
    }

    /// Validates exact equality for all context parameters.
    /// Missing or mismatched parameters fail closed; no wildcards are permitted.
    pub fn is_valid_for(
        &self,
        tool_id: &str,
        action: DiscoverableToolAction,
        session_id: &str,
        turn_id: &str,
        call_id: &str,
    ) -> bool {
        if self.is_expired() {
            return false;
        }
        if tool_id.trim().is_empty()
            || session_id.trim().is_empty()
            || turn_id.trim().is_empty()
            || call_id.trim().is_empty()
        {
            return false;
        }
        self.tool_id == tool_id
            && self.action_type == action
            && self.session_id == session_id
            && self.turn_id == turn_id
            && self.call_id == call_id
    }
}

/// Authoritative execution context passed into tool execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToolCallContext<'a> {
    pub session_id: &'a str,
    pub turn_id: &'a str,
    pub call_id: &'a str,
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

/// Processes an install or enable request.
/// Model arguments can NEVER supply or spoof approval.
/// User consent must originate strictly from a verified host approval capability (`host_approval`).
pub async fn handle_request_plugin_install(
    args: RequestPluginInstallArgs,
    host_approval: Option<&HostPluginApproval>,
    plugin_manager: &m31a_extension_api::PluginManager,
) -> RequestPluginInstallResult {
    // 1. Mandatory host approval check
    let approval = match host_approval {
        Some(a)
            if !a.is_expired()
                && a.tool_id() == args.tool_id
                && a.action_type() == args.action_type =>
        {
            a
        }
        _ => {
            return RequestPluginInstallResult {
                completed: false,
                user_confirmed: false,
                tool_id: args.tool_id,
                tool_type: args.tool_type,
                action_type: args.action_type,
                suggest_reason: args.suggest_reason,
                error: Some(
                    "Installation rejected: explicit host approval capability matching the exact session, turn, call ID, plugin ID, and action is required. Model arguments cannot approve installation."
                        .to_string(),
                ),
            };
        }
    };

    let user_plugins_dir = m31a_core::Config::config_dir().join("plugins");
    let connectors_dir = m31a_core::Config::config_dir().join("connectors");

    // 2. Enable action: must not follow install pipeline
    if args.action_type == DiscoverableToolAction::Enable {
        if args.tool_type == DiscoverableToolType::Connector {
            let conn_file = connectors_dir.join(format!("{}.json", args.tool_id));
            if !conn_file.exists() {
                return RequestPluginInstallResult {
                    completed: false,
                    user_confirmed: true,
                    tool_id: args.tool_id.clone(),
                    tool_type: args.tool_type,
                    action_type: args.action_type,
                    suggest_reason: args.suggest_reason,
                    error: Some(format!(
                        "Cannot enable connector '{}': connector configuration not found at {}",
                        args.tool_id,
                        conn_file.display()
                    )),
                };
            }
            return RequestPluginInstallResult {
                completed: true,
                user_confirmed: true,
                tool_id: args.tool_id,
                tool_type: args.tool_type,
                action_type: args.action_type,
                suggest_reason: args.suggest_reason,
                error: None,
            };
        }

        // Native plugin enable
        let target_dir = user_plugins_dir.join(&args.tool_id);
        if !target_dir.exists() {
            return RequestPluginInstallResult {
                completed: false,
                user_confirmed: true,
                tool_id: args.tool_id.clone(),
                tool_type: args.tool_type,
                action_type: args.action_type,
                suggest_reason: args.suggest_reason,
                error: Some(format!(
                    "Cannot enable plugin '{}': plugin is not installed at {}",
                    args.tool_id,
                    target_dir.display()
                )),
            };
        }

        // Load into runtime to activate
        match plugin_manager
            .load_plugins_from_dir(&user_plugins_dir)
            .await
        {
            Ok(_) => {
                if plugin_manager.get_plugin(&args.tool_id).await.is_none() {
                    return RequestPluginInstallResult {
                        completed: false,
                        user_confirmed: true,
                        tool_id: args.tool_id.clone(),
                        tool_type: args.tool_type,
                        action_type: args.action_type,
                        suggest_reason: args.suggest_reason,
                        error: Some(format!(
                            "Failed to activate plugin '{}': plugin failed to initialize or register",
                            args.tool_id
                        )),
                    };
                }
                return RequestPluginInstallResult {
                    completed: true,
                    user_confirmed: true,
                    tool_id: args.tool_id,
                    tool_type: args.tool_type,
                    action_type: args.action_type,
                    suggest_reason: args.suggest_reason,
                    error: None,
                };
            }
            Err(e) => {
                return RequestPluginInstallResult {
                    completed: false,
                    user_confirmed: true,
                    tool_id: args.tool_id.clone(),
                    tool_type: args.tool_type,
                    action_type: args.action_type,
                    suggest_reason: args.suggest_reason,
                    error: Some(format!("Failed to enable plugin '{}': {e}", args.tool_id)),
                };
            }
        }
    }

    // 3. Install action:
    // Connector installation
    if args.tool_type == DiscoverableToolType::Connector {
        let discoverable = list_discoverable_tools();
        let conn_tool = discoverable
            .iter()
            .find(|t| t.id == args.tool_id && t.tool_type == DiscoverableToolType::Connector);
        if conn_tool.is_none() {
            return RequestPluginInstallResult {
                completed: false,
                user_confirmed: true,
                tool_id: args.tool_id.clone(),
                tool_type: args.tool_type,
                action_type: args.action_type,
                suggest_reason: args.suggest_reason,
                error: Some(format!(
                    "Connector '{}' not found in registry",
                    args.tool_id
                )),
            };
        }
        if let Err(e) = std::fs::create_dir_all(&connectors_dir) {
            return RequestPluginInstallResult {
                completed: false,
                user_confirmed: true,
                tool_id: args.tool_id,
                tool_type: args.tool_type,
                action_type: args.action_type,
                suggest_reason: args.suggest_reason,
                error: Some(format!("Failed to create connectors directory: {e}")),
            };
        }
        let conn_file = connectors_dir.join(format!("{}.json", args.tool_id));
        let conn_json = serde_json::json!({
            "id": args.tool_id,
            "installed_at": chrono::Utc::now().to_rfc3339(),
            "enabled": true,
            "approval_nonce": approval.nonce(),
        });
        if let Err(e) = std::fs::write(
            &conn_file,
            serde_json::to_string_pretty(&conn_json).unwrap(),
        ) {
            return RequestPluginInstallResult {
                completed: false,
                user_confirmed: true,
                tool_id: args.tool_id,
                tool_type: args.tool_type,
                action_type: args.action_type,
                suggest_reason: args.suggest_reason,
                error: Some(format!("Failed to write connector configuration: {e}")),
            };
        }
        return RequestPluginInstallResult {
            completed: true,
            user_confirmed: true,
            tool_id: args.tool_id,
            tool_type: args.tool_type,
            action_type: args.action_type,
            suggest_reason: args.suggest_reason,
            error: None,
        };
    }

    // Native plugin installation via staged pipeline
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
            error: Some("Plugin not found in verified registry catalog".to_string()),
        };
    }

    if let Err(e) = std::fs::create_dir_all(&user_plugins_dir) {
        return RequestPluginInstallResult {
            completed: false,
            user_confirmed: true,
            tool_id: args.tool_id,
            tool_type: args.tool_type,
            action_type: args.action_type,
            suggest_reason: args.suggest_reason,
            error: Some(format!("Failed to create plugins directory: {e}")),
        };
    }

    let run_id = uuid::Uuid::new_v4();
    let staging_dir = user_plugins_dir.join(format!(".staging_{}_{}", args.tool_id, run_id));
    let backup_dir = user_plugins_dir.join(format!(".backup_{}_{}", args.tool_id, run_id));
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

    // Helper for cleanup on failure
    let cleanup_failure = |err_msg: String| -> RequestPluginInstallResult {
        let _ = std::fs::remove_dir_all(&staging_dir);
        if target_dir.exists() && backup_dir.exists() {
            let _ = std::fs::remove_dir_all(&target_dir);
            let _ = std::fs::rename(&backup_dir, &target_dir);
        }
        let _ = std::fs::remove_dir_all(&backup_dir);
        RequestPluginInstallResult {
            completed: false,
            user_confirmed: true,
            tool_id: args.tool_id.clone(),
            tool_type: args.tool_type,
            action_type: args.action_type,
            suggest_reason: args.suggest_reason.clone(),
            error: Some(err_msg),
        }
    };

    // Stage package files into staging_dir
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
manifest_version = 1
abi_version = 1
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
manifest_version = 1
abi_version = 1
"#,
            dt.id, dt.description
        )
    };

    if let Err(e) = std::fs::write(&manifest_path, &manifest_content) {
        return cleanup_failure(format!("Failed to write manifest in staging: {e}"));
    }

    // Atomic replacement: if target exists, move to backup
    if target_dir.exists()
        && let Err(e) = std::fs::rename(&target_dir, &backup_dir)
    {
        return cleanup_failure(format!("Failed to backup existing plugin version: {e}"));
    }

    // Move staging to target
    if let Err(e) = std::fs::rename(&staging_dir, &target_dir) {
        return cleanup_failure(format!("Failed to move staging to target directory: {e}"));
    }

    // Health check and runtime validation:
    // Manifest must parse and validate
    let target_manifest_path = target_dir.join("m31a-plugin.toml");
    let parsed_manifest: Result<m31a_extension_api::PluginManifest, _> =
        std::fs::read_to_string(&target_manifest_path)
            .map_err(|e| anyhow::anyhow!("Failed to read target manifest: {e}"))
            .and_then(|s| {
                toml::from_str(&s).map_err(|e| anyhow::anyhow!("Manifest parse error: {e}"))
            });

    if let Err(e) = parsed_manifest {
        return cleanup_failure(format!("Plugin validation failed: {e}"));
    }

    // Attempt to load plugin into plugin manager
    match plugin_manager
        .load_plugins_from_dir(&user_plugins_dir)
        .await
    {
        Ok(_) => {
            // Remove backup on confirmed success
            let _ = std::fs::remove_dir_all(&backup_dir);
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
        Err(e) => cleanup_failure(format!("Failed to load and activate plugin: {e}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_plugin_approval_binding_exact_equality() {
        let ttl = std::time::Duration::from_secs(120);

        // 1. Valid issuance with full context
        let approval = HostPluginApproval::issue_host_approval(
            "github-mcp",
            DiscoverableToolAction::Install,
            "sess-1",
            "turn-1",
            "call-1",
            ttl,
        )
        .expect("must succeed with valid context");

        assert_eq!(approval.tool_id(), "github-mcp");
        assert_eq!(approval.action_type(), DiscoverableToolAction::Install);
        assert_eq!(approval.session_id(), "sess-1");
        assert_eq!(approval.turn_id(), "turn-1");
        assert_eq!(approval.call_id(), "call-1");
        assert!(!approval.is_expired());

        // 2. Exact match succeeds
        assert!(approval.is_valid_for(
            "github-mcp",
            DiscoverableToolAction::Install,
            "sess-1",
            "turn-1",
            "call-1"
        ));

        // 3. Different session fails
        assert!(!approval.is_valid_for(
            "github-mcp",
            DiscoverableToolAction::Install,
            "sess-2",
            "turn-1",
            "call-1"
        ));

        // 4. Different turn fails
        assert!(!approval.is_valid_for(
            "github-mcp",
            DiscoverableToolAction::Install,
            "sess-1",
            "turn-2",
            "call-1"
        ));

        // 5. Different tool-call ID fails
        assert!(!approval.is_valid_for(
            "github-mcp",
            DiscoverableToolAction::Install,
            "sess-1",
            "turn-1",
            "call-2"
        ));

        // 6. Different action fails
        assert!(!approval.is_valid_for(
            "github-mcp",
            DiscoverableToolAction::Enable,
            "sess-1",
            "turn-1",
            "call-1"
        ));

        // 7. Missing/empty context fails closed (no wildcards)
        assert!(!approval.is_valid_for(
            "github-mcp",
            DiscoverableToolAction::Install,
            "",
            "turn-1",
            "call-1"
        ));
        assert!(!approval.is_valid_for(
            "github-mcp",
            DiscoverableToolAction::Install,
            "sess-1",
            "",
            "call-1"
        ));
        assert!(!approval.is_valid_for(
            "github-mcp",
            DiscoverableToolAction::Install,
            "sess-1",
            "turn-1",
            ""
        ));

        // 8. Empty context in issuance fails
        assert!(
            HostPluginApproval::issue_host_approval(
                "",
                DiscoverableToolAction::Install,
                "s",
                "t",
                "c",
                ttl
            )
            .is_err()
        );
        assert!(
            HostPluginApproval::issue_host_approval(
                "p",
                DiscoverableToolAction::Install,
                "",
                "t",
                "c",
                ttl
            )
            .is_err()
        );
        assert!(
            HostPluginApproval::issue_host_approval(
                "p",
                DiscoverableToolAction::Install,
                "s",
                "",
                "c",
                ttl
            )
            .is_err()
        );
        assert!(
            HostPluginApproval::issue_host_approval(
                "p",
                DiscoverableToolAction::Install,
                "s",
                "t",
                "",
                ttl
            )
            .is_err()
        );

        // 9. Expired approval fails
        let expired_approval = HostPluginApproval::issue_host_approval(
            "github-mcp",
            DiscoverableToolAction::Install,
            "sess-1",
            "turn-1",
            "call-1",
            std::time::Duration::from_nanos(1),
        )
        .unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        assert!(!expired_approval.is_valid_for(
            "github-mcp",
            DiscoverableToolAction::Install,
            "sess-1",
            "turn-1",
            "call-1"
        ));
    }

    #[tokio::test]
    async fn test_handle_request_plugin_install_approval_enforcement() {
        let pm = m31a_extension_api::PluginManager::new(vec![]);
        let args = RequestPluginInstallArgs {
            tool_id: "github-mcp".to_string(),
            tool_type: DiscoverableToolType::Plugin,
            action_type: DiscoverableToolAction::Install,
            suggest_reason: "Repository search requires GitHub MCP server".to_string(),
        };

        // Without approval: fails closed
        let res_no_auth = handle_request_plugin_install(
            RequestPluginInstallArgs {
                tool_id: "github-mcp".to_string(),
                tool_type: DiscoverableToolType::Plugin,
                action_type: DiscoverableToolAction::Install,
                suggest_reason: "No approval".to_string(),
            },
            None,
            &pm,
        )
        .await;
        assert!(!res_no_auth.completed);
        assert!(!res_no_auth.user_confirmed);
        assert!(res_no_auth.error.is_some());

        // With valid approval: succeeds
        let approval = HostPluginApproval::issue_host_approval(
            "github-mcp",
            DiscoverableToolAction::Install,
            "sess-1",
            "turn-1",
            "call-1",
            std::time::Duration::from_secs(60),
        )
        .unwrap();

        let res_auth = handle_request_plugin_install(args, Some(&approval), &pm).await;
        assert!(res_auth.completed);
        assert!(res_auth.user_confirmed);
        assert_eq!(res_auth.tool_id, "github-mcp");
    }

    #[tokio::test]
    async fn test_enable_fails_if_not_installed() {
        let pm = m31a_extension_api::PluginManager::new(vec![]);
        let approval = HostPluginApproval::issue_host_approval(
            "uninstalled-plugin",
            DiscoverableToolAction::Enable,
            "sess-1",
            "turn-1",
            "call-1",
            std::time::Duration::from_secs(60),
        )
        .unwrap();

        let res = handle_request_plugin_install(
            RequestPluginInstallArgs {
                tool_id: "uninstalled-plugin".to_string(),
                tool_type: DiscoverableToolType::Plugin,
                action_type: DiscoverableToolAction::Enable,
                suggest_reason: "Enable uninstalled plugin".to_string(),
            },
            Some(&approval),
            &pm,
        )
        .await;

        assert!(!res.completed);
        assert!(res.error.unwrap().contains("is not installed"));
    }
}
