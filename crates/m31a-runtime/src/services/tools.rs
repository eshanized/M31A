//! Tool registry initialization, MCP wiring, and child agent runtime registration.

use crate::error::RuntimeError;
use m31a_core::LocalEncryptedVault;
use m31a_mcp::McpManager;
use m31a_storage::Storage;
use m31a_tools::ToolRegistry;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Initialize and configure the central tool registry.
pub fn init_tool_registry(
    storage: Option<Storage>,
    vault: Arc<LocalEncryptedVault>,
    mcp_manager: Arc<McpManager>,
    extra_dirs: Vec<PathBuf>,
    cwd: &Path,
    enable_web_search: bool,
) -> Result<ToolRegistry, RuntimeError> {
    let mut tools = ToolRegistry::new_isolated(storage, vault, mcp_manager, extra_dirs, cwd);

    if !enable_web_search {
        tools.disable("web_search");
    }

    Ok(tools)
}
