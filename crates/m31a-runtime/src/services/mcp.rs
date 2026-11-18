//! Model Context Protocol (MCP) server manager and process supervisor.

use m31a_core::config::McpConfig;
use m31a_mcp::McpManager;
use std::sync::Arc;

/// Initialize the MCP manager with the provided MCP configuration.
pub fn init_mcp_manager(config: McpConfig) -> Arc<McpManager> {
    Arc::new(McpManager::new(config))
}
