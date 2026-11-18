//! Lifecycle hooks runtime discovery and MCP-backed dispatch.

use m31a_hooks::{HookConfig, HookRuntime};
use m31a_mcp::McpManager;
use std::path::Path;
use std::sync::Arc;

/// Discover and initialize the hook runtime for the given workspace directory.
///
/// MCP hook handlers are routed through the runtime-owned MCP manager instead
/// of spawning an unrelated server process from the hook layer.
pub fn init_hooks(cwd: &Path, mcp_manager: &Arc<McpManager>) -> HookRuntime {
    let config = HookConfig::discover(cwd);
    let manager = Arc::clone(mcp_manager);
    let invoker: m31a_hooks::McpHandlerFn = Arc::new(move |server, tool, payload| {
        let manager = Arc::clone(&manager);
        Box::pin(async move {
            let raw = manager
                .execute_tool_with_server(&server, &tool, payload)
                .await?;
            Ok(serde_json::from_str(&raw).unwrap_or(serde_json::Value::String(raw)))
        })
    });
    HookRuntime::new(config).with_mcp_invoker(invoker)
}
