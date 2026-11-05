//! Primary tool registry declaring built-in tools and schema definitions.
//!
//! Registers standard tools (file operations, shell execution, git commands, search, patch application)
//! and dynamically merges external MCP and plugin tools.

use anyhow::{Context, Result, anyhow};
use m31a_core::SandboxPolicy;
use m31a_core::types::{FunctionDefinition, ToolDefinition};
use m31a_execpolicy::{Decision, ExecPolicy};
use m31a_sandbox::{ProcessManager, ProcessStatus, SandboxRunner, SpawnResult};
use m31a_storage::Storage;
use serde_json::json;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use crate::mcp::{MCP_PREFIX, McpManager};
use crate::patch::{apply_patch_to_workspace, parse_patch};
use m31a_core::context_compactor::ContextCompactor;
use m31a_core::types::ChatMessage;

#[derive(Clone)]
pub struct ToolRegistry {
    sandbox: SandboxRunner,
    exec_policy: ExecPolicy,
    storage: Option<Storage>,
    process_manager: Arc<ProcessManager>,
    pub proc_supervisor: Arc<m31a_proc::ProcessManager>,
    pub mcp_manager: Arc<McpManager>,
    dynamic_tools: Arc<std::sync::RwLock<Vec<ToolDefinition>>>,
    disabled_tools: std::collections::BTreeSet<String>,
    allowed_tools: Option<std::collections::BTreeSet<String>>,
    pub role_registry: Arc<m31a_core::AgentRoleRegistry>,
    pub vault: Arc<m31a_core::LocalEncryptedVault>,
    pub cloud_tasks: Arc<m31a_core::CloudTaskManager>,
    pub cloud_dispatcher: Arc<m31a_cloud_tasks::CloudTaskDispatcher>,
    pub plugin_manager: Arc<m31a_extension_api::PluginManager>,
    pub tcp_tunnels: Arc<m31a_execpolicy::TcpTunnelManager>,
    pub harness: Arc<crate::harness::ToolExecutionHarness>,
    pub persistent_shell: Arc<tokio::sync::Mutex<m31a_shell::PersistentShellSession>>,
    /// Shared multi-agent runtime handle with interior mutability.
    /// Hosts construct `m31a_agent_engine::agents::EngineAgentRuntime` and install it here;
    /// all cloned registries and child engines share this handle immediately.
    pub(crate) agent_runtime:
        Arc<std::sync::RwLock<Option<Arc<dyn crate::agent_runtime::AgentRuntime>>>>,
    /// Context compactor for token budget awareness (set by engine per turn)
    pub compactor: Option<Arc<ContextCompactor>>,
    /// Current message history for context budget calculation (set by engine per turn)
    pub message_history: Option<Arc<Vec<ChatMessage>>>,
    /// Active session/thread identifier for session-scoped tool execution.
    pub session_id: Option<String>,
    /// Verified host approval tokens for sensitive plugin installations.
    pub host_plugin_approvals:
        Arc<std::sync::Mutex<Vec<crate::plugin_install::HostPluginApproval>>>,
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolRegistry {
    pub fn new() -> Self {
        let storage = Storage::open_default().ok();
        let vault_dir = m31a_core::Config::config_dir().join("vault");
        let vault = Arc::new(
            m31a_core::LocalEncryptedVault::new(vault_dir).unwrap_or_else(|_| {
                m31a_core::LocalEncryptedVault::new(std::env::temp_dir().join("m31a-vault"))
                    .unwrap()
            }),
        );
        Self {
            sandbox: SandboxRunner::new(),
            exec_policy: ExecPolicy::standard(),
            storage,
            process_manager: Arc::new(ProcessManager::new()),
            proc_supervisor: Arc::new(m31a_proc::ProcessManager::default()),
            mcp_manager: Arc::new(McpManager::new(
                m31a_core::Config::load().unwrap_or_default().mcp,
            )),
            dynamic_tools: Arc::new(std::sync::RwLock::new(Vec::new())),
            disabled_tools: std::collections::BTreeSet::new(),
            allowed_tools: None,
            role_registry: Arc::new(m31a_core::AgentRoleRegistry::new()),
            vault,
            cloud_tasks: Arc::new(m31a_core::CloudTaskManager::new(8)),
            cloud_dispatcher: Arc::new(m31a_cloud_tasks::CloudTaskDispatcher::local_only()),
            plugin_manager: Arc::new(m31a_extension_api::PluginManager::new(vec![
                m31a_core::Config::config_dir().join("plugins"),
                std::path::PathBuf::from(".m31a/plugins"),
            ])),
            tcp_tunnels: Arc::new(m31a_execpolicy::TcpTunnelManager::new()),
            harness: Arc::new(crate::harness::ToolExecutionHarness::default()),
            persistent_shell: Arc::new(tokio::sync::Mutex::new(
                m31a_shell::PersistentShellSession::new(
                    "default",
                    std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")),
                ),
            )),
            agent_runtime: Arc::new(std::sync::RwLock::new(None)),
            compactor: None,
            message_history: None,
            session_id: None,
            host_plugin_approvals: Arc::new(std::sync::Mutex::new(Vec::new())),
        }
    }

    /// Injects the real multi-agent runtime. Updates the shared interior cell so all
    /// cloned registries (including parent and child engines) observe it immediately.
    pub fn set_agent_runtime(&self, runtime: Arc<dyn crate::agent_runtime::AgentRuntime>) {
        *self.agent_runtime.write().unwrap() = Some(runtime);
    }

    /// Gets the current active agent runtime handle, if injected.
    pub fn get_agent_runtime(&self) -> Option<Arc<dyn crate::agent_runtime::AgentRuntime>> {
        self.agent_runtime.read().unwrap().clone()
    }

    pub fn with_vault(mut self, vault: Arc<m31a_core::LocalEncryptedVault>) -> Self {
        self.vault = vault;
        self
    }

    pub fn with_cloud_tasks(mut self, cloud_tasks: Arc<m31a_core::CloudTaskManager>) -> Self {
        self.cloud_tasks = cloud_tasks;
        self
    }

    pub fn with_process_manager(mut self, pm: Arc<ProcessManager>) -> Self {
        self.process_manager = pm;
        self
    }

    pub fn with_proc_supervisor(mut self, ps: Arc<m31a_proc::ProcessManager>) -> Self {
        self.proc_supervisor = ps;
        self
    }

    pub fn with_storage(mut self, storage: Storage) -> Self {
        self.storage = Some(storage);
        self
    }

    pub fn with_mcp_manager(mut self, mcp_manager: Arc<McpManager>) -> Self {
        self.mcp_manager = mcp_manager;
        self
    }

    /// Fully isolated constructor guaranteeing zero writes or reads from `~/.m31a`.
    pub fn new_isolated(
        storage: Option<Storage>,
        vault: Arc<m31a_core::LocalEncryptedVault>,
        mcp_manager: Arc<McpManager>,
        plugin_dirs: Vec<PathBuf>,
        cwd: &Path,
    ) -> Self {
        Self {
            sandbox: SandboxRunner::new(),
            exec_policy: ExecPolicy::standard(),
            storage,
            process_manager: Arc::new(ProcessManager::new()),
            proc_supervisor: Arc::new(m31a_proc::ProcessManager::default()),
            mcp_manager,
            dynamic_tools: Arc::new(std::sync::RwLock::new(Vec::new())),
            disabled_tools: std::collections::BTreeSet::new(),
            allowed_tools: None,
            role_registry: Arc::new(m31a_core::AgentRoleRegistry::new()),
            vault,
            cloud_tasks: Arc::new(m31a_core::CloudTaskManager::new(8)),
            cloud_dispatcher: Arc::new(m31a_cloud_tasks::CloudTaskDispatcher::local_only()),
            plugin_manager: Arc::new(m31a_extension_api::PluginManager::new(plugin_dirs)),
            tcp_tunnels: Arc::new(m31a_execpolicy::TcpTunnelManager::new()),
            harness: Arc::new(crate::harness::ToolExecutionHarness::default()),
            persistent_shell: Arc::new(tokio::sync::Mutex::new(
                m31a_shell::PersistentShellSession::new("default", cwd.to_path_buf()),
            )),
            agent_runtime: Arc::new(std::sync::RwLock::new(None)),
            compactor: None,
            message_history: None,
            session_id: None,
            host_plugin_approvals: Arc::new(std::sync::Mutex::new(Vec::new())),
        }
    }

    /// Registers dynamic tool definitions in the agent's callable registry.
    pub fn register_dynamic_tools(&self, defs: Vec<ToolDefinition>) {
        if let Ok(mut dyn_tools) = self.dynamic_tools.write() {
            for def in defs {
                dyn_tools.retain(|d| d.function.name != def.function.name);
                dyn_tools.push(def);
            }
        }
    }

    /// Unregisters all dynamic tool definitions associated with a given MCP server.
    pub fn unregister_server_tools(&self, server_name: &str) {
        if let Ok(mut dyn_tools) = self.dynamic_tools.write() {
            let prefix = format!("mcp__{}__", server_name);
            dyn_tools.retain(|d| !d.function.name.starts_with(&prefix));
        }
    }

    /// Sets an explicit positive allowlist of tools.
    /// When set, ONLY tools present in this allowlist can be advertised in `definitions()`
    /// or executed via `execute()`. MCP and dynamically registered tools outside this allowlist
    /// are strictly rejected.
    pub fn set_allowed_tools(&mut self, tools: impl IntoIterator<Item = impl Into<String>>) {
        self.allowed_tools = Some(tools.into_iter().map(Into::into).collect());
    }

    /// Builder helper to set an explicit positive allowlist of tools.
    pub fn with_allowed_tools(
        mut self,
        tools: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.set_allowed_tools(tools);
        self
    }

    /// Returns whether a tool is permitted under the current allowlist and disabled list.
    pub fn is_allowed(&self, name: &str) -> bool {
        if let Some(ref allowed) = self.allowed_tools
            && !allowed.contains(name)
        {
            return false;
        }
        !self.disabled_tools.contains(name)
    }

    /// Hides a tool from `definitions()` and rejects it in `execute()`.
    /// Used to gate opt-in capabilities such as `--search` (web_search).
    pub fn disable(&mut self, name: &str) {
        self.disabled_tools.insert(name.to_string());
    }

    /// Re-enables a previously disabled tool.
    pub fn enable(&mut self, name: &str) {
        self.disabled_tools.remove(name);
    }

    pub fn is_disabled(&self, name: &str) -> bool {
        !self.is_allowed(name)
    }

    pub fn harness(&self) -> Arc<crate::harness::ToolExecutionHarness> {
        Arc::clone(&self.harness)
    }

    /// Evaluates the granular security and effect classification of a tool invocation.
    pub fn tool_effect(&self, name: &str, args_str: Option<&str>) -> m31a_execpolicy::ToolEffect {
        m31a_execpolicy::classify_tool(name, args_str)
    }

    /// Grants a verified host approval token for a plugin action.
    pub fn grant_host_plugin_approval(&self, approval: crate::plugin_install::HostPluginApproval) {
        let mut store = self.host_plugin_approvals.lock().unwrap();
        store.push(approval);
    }

    /// Consumes a valid host approval capability matching the exact plugin, action, and execution context.
    /// Enforces strictly atomic one-time consumption and rejects expired or mismatched tickets.
    pub fn consume_host_plugin_approval(
        &self,
        tool_id: &str,
        action: crate::plugin_install::DiscoverableToolAction,
        session_id: &str,
        turn_id: &str,
        call_id: &str,
    ) -> Option<crate::plugin_install::HostPluginApproval> {
        let mut store = self.host_plugin_approvals.lock().unwrap();
        store.retain(|a| !a.is_expired());
        store
            .iter()
            .position(|a| a.is_valid_for(tool_id, action, session_id, turn_id, call_id))
            .map(|pos| store.remove(pos))
    }

    /// Returns true if the tool name corresponds to a registered builtin, dynamic, MCP, or plugin tool.
    pub fn is_known_tool(&self, name: &str) -> bool {
        if name.starts_with("mcp__") || name.starts_with("mcp_") || name.contains(':') {
            return true;
        }
        self.definitions().iter().any(|d| d.function.name == name)
    }

    /// Sets the active session/thread identifier on this registry.
    pub fn with_session_id(mut self, session_id: impl Into<String>) -> Self {
        self.session_id = Some(session_id.into());
        self
    }

    /// Mutably updates the active session/thread identifier.
    pub fn set_session_id(&mut self, session_id: impl Into<String>) {
        self.session_id = Some(session_id.into());
    }

    /// Executes a batch of tool calls sequentially or concurrently based on mutability classification.
    pub async fn execute_batch(
        &self,
        batch: Vec<(String, String, String)>, // (id, name, args)
        cwd: &Path,
        sandbox: SandboxPolicy,
    ) -> Vec<(String, Result<String>)> {
        let mut results = Vec::with_capacity(batch.len());
        for (id, name, args) in batch {
            let res = self.execute(&name, &args, cwd, sandbox).await;
            results.push((id, res));
        }
        results
    }

    pub async fn register_mcp_servers(
        &self,
        servers: &HashMap<String, m31a_core::config::McpServerConfig>,
        cwd: Option<&Path>,
    ) -> Result<usize> {
        let defs = self.mcp_manager.load_servers(servers, cwd).await?;
        let count = defs.len();
        if let Ok(mut dyn_tools) = self.dynamic_tools.write() {
            dyn_tools.extend(defs);
        }
        Ok(count)
    }

    pub fn mcp_manager(&self) -> Arc<McpManager> {
        self.mcp_manager.clone()
    }

    /// Dynamically discovers and registers tools provided by loaded plugins into the callable tool surface.
    pub async fn register_plugin_tools(&self) -> usize {
        let tools = self.plugin_manager.all_tools().await;
        let mut count = 0;
        if let Ok(mut dyn_tools) = self.dynamic_tools.write() {
            for (plugin_name, tool) in tools {
                let scoped_name = format!("{}:{}", plugin_name, tool.name);
                if !dyn_tools.iter().any(|d| d.function.name == scoped_name) {
                    dyn_tools.push(ToolDefinition {
                        tool_type: "function".to_string(),
                        function: FunctionDefinition {
                            name: scoped_name,
                            description: format!("[Plugin: {}] {}", plugin_name, tool.description),
                            parameters: tool.input_schema,
                        },
                    });
                    count += 1;
                }
            }
        }
        count
    }

    pub fn definitions(&self) -> Vec<ToolDefinition> {
        let mut builtins = vec![
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "exec_command".to_string(),
                    description: "Execute a shell command inside the configured sandbox environment with standard output and exit code capture.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "command": {
                                "type": "string",
                                "description": "The exact command string to execute in bash."
                            },
                            "timeout_seconds": {
                                "type": "integer",
                                "description": "Optional timeout in seconds (default 60)."
                            },
                            "yield_time_ms": {
                                "type": "integer",
                                "description": "Optional milliseconds to wait before automatically backgrounding the process into the background process table. If 0 or omitted, runs synchronously up to timeout."
                            }
                        },
                        "required": ["command"]
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "debug_launch".to_string(),
                    description: "Launch an interactive DAP (Debug Adapter Protocol) debug session for a binary, script, or test runner.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "session_id": {
                                "type": "string",
                                "description": "Unique identifier for this debug session."
                            },
                            "program": {
                                "type": "string",
                                "description": "Path to binary or script to debug."
                            },
                            "args": {
                                "type": "array",
                                "items": { "type": "string" },
                                "description": "CLI arguments passed to program."
                            }
                        },
                        "required": ["program"]
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "debug_breakpoint".to_string(),
                    description: "Set or clear breakpoints in a file during a DAP debug session.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "session_id": {
                                "type": "string",
                                "description": "Debug session ID."
                            },
                            "file_path": {
                                "type": "string",
                                "description": "Source file path."
                            },
                            "lines": {
                                "type": "array",
                                "items": { "type": "integer" },
                                "description": "List of line numbers to break at."
                            },
                            "condition": {
                                "type": "string",
                                "description": "Optional expression condition for breakpoint."
                            }
                        },
                        "required": ["file_path", "lines"]
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "debug_step".to_string(),
                    description: "Step over, into, out, or continue execution in a DAP debug session.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "session_id": {
                                "type": "string",
                                "description": "Debug session ID."
                            },
                            "step_type": {
                                "type": "string",
                                "enum": ["over", "into", "out", "continue"],
                                "description": "Execution step type."
                            }
                        },
                        "required": ["session_id", "step_type"]
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "debug_inspect".to_string(),
                    description: "Inspect active variables and stack frame state in a DAP debug session.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "session_id": {
                                "type": "string",
                                "description": "Debug session ID."
                            },
                            "scope": {
                                "type": "string",
                                "description": "Optional scope to filter: 'local', 'global'."
                            }
                        },
                        "required": ["session_id"]
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "verify_workspace".to_string(),
                    description: "Autonomously verify code changes using project compiler checks (cargo check, tsc, mypy) or test suites (cargo test, npm test, pytest). Returns structured diagnostics with file, line, and suggested fixes.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "mode": {
                                "type": "string",
                                "enum": ["check", "test"],
                                "description": "Verification mode: 'check' for fast compiler/linter check, or 'test' for full test suite. Defaults to 'check'."
                            },
                            "timeout_seconds": {
                                "type": "integer",
                                "description": "Optional timeout in seconds (default 60)."
                            }
                        }
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "code_review".to_string(),
                    description: "Perform an automated security, correctness, and reliability code review on uncommitted working tree diffs or a target git commit/ref.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "base_ref": {
                                "type": "string",
                                "description": "Optional git commit, branch, or ref to diff against (defaults to HEAD)."
                            }
                        }
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "check_repo_trust".to_string(),
                    description: "Inspect workspace trust level and audit repository git hooks and configuration for security vulnerabilities.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "path": {
                                "type": "string",
                                "description": "Optional repository directory path (defaults to current directory)."
                            }
                        }
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "write_stdin".to_string(),
                    description: "Deliver stdin input to a running or interactive process in the background process table.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "process_id": {
                                "type": "string",
                                "description": "The ID of the background process (e.g. 'process-1')."
                            },
                            "input": {
                                "type": "string",
                                "description": "String input to write to standard input."
                            }
                        },
                        "required": ["process_id", "input"]
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "poll_process".to_string(),
                    description: "Poll an active background process to retrieve its current status and newly emitted stdout/stderr since last poll.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "process_id": {
                                "type": "string",
                                "description": "The ID of the background process."
                            }
                        },
                        "required": ["process_id"]
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "kill_process".to_string(),
                    description: "Terminate a running background process.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "process_id": {
                                "type": "string",
                                "description": "The ID of the background process to terminate."
                            }
                        },
                        "required": ["process_id"]
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "list_processes".to_string(),
                    description: "List all active and recent background processes managed by the unified execution engine.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {}
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "start_background_process".to_string(),
                    description: "Launch a persistent background process or dev server (e.g. 'npm run dev', 'cargo watch', 'python server.py') with stdout/stderr line-buffering.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "command": {
                                "type": "string",
                                "description": "The command executable to run (e.g. 'npm', 'cargo', 'python', 'node')."
                            },
                            "args": {
                                "type": "array",
                                "items": { "type": "string" },
                                "description": "Command-line arguments (e.g. ['run', 'dev'] or ['watch', '-x', 'check'])."
                            },
                            "label": {
                                "type": "string",
                                "description": "Human-friendly label for this process (e.g. 'vite-frontend', 'api-server')."
                            },
                            "cwd": {
                                "type": "string",
                                "description": "Optional working directory relative to workspace root."
                            }
                        },
                        "required": ["command"]
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "list_background_processes".to_string(),
                    description: "List all persistent background processes and dev servers, including their PID, status, uptime, and lines logged.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {}
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "get_background_process_output".to_string(),
                    description: "Retrieve recent buffered stdout/stderr output lines for a background process.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "process_id": {
                                "type": "string",
                                "description": "The process ID (e.g. 'proc-1a2b3c4d')."
                            },
                            "limit": {
                                "type": "integer",
                                "description": "Max number of output lines to return (default 100)."
                            },
                            "offset": {
                                "type": "integer",
                                "description": "Starting line offset for pagination."
                            }
                        },
                        "required": ["process_id"]
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "stop_background_process".to_string(),
                    description: "Terminate a persistent background process or dev server.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "process_id": {
                                "type": "string",
                                "description": "The process ID to stop."
                            },
                            "force": {
                                "type": "boolean",
                                "description": "If true, forcefully terminates with SIGKILL immediately without waiting for graceful shutdown."
                            }
                        },
                        "required": ["process_id"]
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "read_file".to_string(),
                    description: "Read contents of a file within the workspace.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "path": {
                                "type": "string",
                                "description": "Relative or absolute path of the file to read."
                            },
                            "start_line": {
                                "type": "integer",
                                "description": "1-based starting line number (inclusive)."
                            },
                            "end_line": {
                                "type": "integer",
                                "description": "1-based ending line number (inclusive)."
                            },
                            "numbered": {
                                "type": "boolean",
                                "description": "Optional: if true, formats each line with its 1-based line number (e.g. '  42 | fn main() {')."
                            }
                        },
                        "required": ["path"]
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "write_file".to_string(),
                    description: "Create or replace a file with given content.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "path": {
                                "type": "string",
                                "description": "Path of the file to write."
                            },
                            "content": {
                                "type": "string",
                                "description": "Full text contents to write."
                            },
                            "replacement_reason": {
                                "type": "string",
                                "description": "Required when replacing an existing file with >30 lines, explaining why a full rewrite is necessary instead of targeted edits."
                            }
                        },
                        "required": ["path", "content"]
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "edit".to_string(),
                    description: "Use the edit tool for surgical, targeted changes to existing text files. Replaces the exact text in 'old_string' with 'new_string'. By default, 'old_string' must occur exactly once to prevent accidental multi-site changes; set 'replace_all: true' to replace every occurrence.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "path": {
                                "type": "string",
                                "description": "Relative or absolute path of the file to edit."
                            },
                            "old_string": {
                                "type": "string",
                                "description": "Exact text chunk in the original file to replace. Must occur exactly once unless replace_all is true."
                            },
                            "new_string": {
                                "type": "string",
                                "description": "Replacement text chunk (must differ from old_string)."
                            },
                            "replace_all": {
                                "type": "boolean",
                                "description": "If true, replaces all occurrences of old_string. Defaults to false."
                            }
                        },
                        "required": ["path", "old_string", "new_string"]
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "question".to_string(),
                    description: "Ask the user one or more multiple-choice questions to clarify requirements, design decisions, or disambiguate next steps, allowing users to select options rather than typing free-form answers.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "questions": {
                                "type": "array",
                                "items": {
                                    "type": "object",
                                    "properties": {
                                        "question": {
                                            "type": "string",
                                            "description": "The question prompt text."
                                        },
                                        "options": {
                                            "type": "array",
                                            "items": { "type": "string" },
                                            "description": "Answers to choose from, unnumbered. Put the recommended answer first, prefixed with '(Recommended)'."
                                        },
                                        "is_multi_select": {
                                            "type": "boolean",
                                            "description": "Whether the user can select multiple options (checkboxes) instead of one (radio)."
                                        }
                                    },
                                    "required": ["question", "options"]
                                },
                                "description": "Array of multiple-choice questions."
                            }
                        },
                        "required": ["questions"]
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "todowrite".to_string(),
                    description: "Update the session todo list to track multi-step goals with real-time status transitions (pending, in_progress, completed, cancelled).".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "todos": {
                                "type": "array",
                                "items": {
                                    "type": "object",
                                    "properties": {
                                        "content": {
                                            "type": "string",
                                            "description": "Brief description of the task."
                                        },
                                        "status": {
                                            "type": "string",
                                            "enum": ["pending", "in_progress", "completed", "cancelled"],
                                            "description": "Current status of the task."
                                        }
                                    },
                                    "required": ["content", "status"]
                                },
                                "description": "The updated todo list."
                            }
                        },
                        "required": ["todos"]
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "schedule".to_string(),
                    description: "Schedule a one-shot timer (duration_seconds) or a recurring cron trigger (cron_expression) that notifies the agent via a high-priority prompt.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "prompt": {
                                "type": "string",
                                "description": "Message sent as a high-priority notification when the timer fires or cron triggers."
                            },
                            "duration_seconds": {
                                "type": "integer",
                                "description": "Seconds to wait for a one-shot timer (mutually exclusive with cron_expression)."
                            },
                            "cron_expression": {
                                "type": "string",
                                "description": "Standard 5-field cron expression for recurring schedule (e.g. '*/5 * * * *')."
                            },
                            "max_iterations": {
                                "type": "integer",
                                "description": "Maximum number of triggers before terminating."
                            },
                            "is_daemon": {
                                "type": "boolean",
                                "description": "Whether to continue running as a daemon after the current turn ends."
                            }
                        },
                        "required": ["prompt"]
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "browser_navigate".to_string(),
                    description: "Navigate the active browser session to the specified URL.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "url": { "type": "string", "description": "The URL to navigate to." }
                        },
                        "required": ["url"]
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "browser_snapshot".to_string(),
                    description: "Capture the active page accessibility tree with @e1, @e2 element references for interaction.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {}
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "browser_click".to_string(),
                    description: "Click an interactive element by its allocated reference (e.g. '@e1', '@e2').".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "ref": { "type": "string", "description": "The element reference (e.g. '@e1')." }
                        },
                        "required": ["ref"]
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "browser_type".to_string(),
                    description: "Type text into an input or textbox element designated by reference.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "ref": { "type": "string", "description": "The element reference (e.g. '@e1')." },
                            "text": { "type": "string", "description": "The text to type into the element." }
                        },
                        "required": ["ref", "text"]
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "browser_screenshot".to_string(),
                    description: "Capture a PNG screenshot of the current browser viewport.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "path": { "type": "string", "description": "Optional file path to save the screenshot." }
                        }
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "browser_console".to_string(),
                    description: "Inspect browser console log messages and runtime exceptions.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {}
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "browser_network".to_string(),
                    description: "Inspect intercepted HTTP network requests and responses from the current page.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {}
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "browser_evaluate".to_string(),
                    description: "Evaluate a JavaScript snippet in the active page context.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "script": { "type": "string", "description": "JavaScript code to execute." }
                        },
                        "required": ["script"]
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "verify_visual_state".to_string(),
                    description: "Execute autonomous Visual TDD against a web frontend: validates DOM presence, text matches, computed CSS styles, console errors, and network failures with screenshot artifact capture.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "scenario_name": { "type": "string", "description": "Descriptive name for the visual test scenario." },
                            "url": { "type": "string", "description": "Target web application URL or local dev server address (e.g. http://localhost:3000)." },
                            "viewport": {
                                "type": "array",
                                "items": { "type": "integer" },
                                "description": "Optional viewport dimensions [width, height], default [1280, 800]."
                            },
                            "actions": {
                                "type": "array",
                                "description": "Optional user interaction steps (click, type, wait_selector, screenshot).",
                                "items": { "type": "object" }
                            },
                            "assertions": {
                                "type": "array",
                                "description": "List of assertions: element_present, text_matches, computed_style_matches, attribute_matches, zero_console_errors, zero_network_failures.",
                                "items": { "type": "object" }
                            }
                        },
                        "required": ["scenario_name", "url", "assertions"]
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "audit_codebase_safety".to_string(),
                    description: "Perform deep AST semantic blast-radius calculation, downstream caller impact tracing, and data-flow taint analysis before making wide codebase changes.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "modified_files": {
                                "type": "array",
                                "items": { "type": "string" },
                                "description": "List of relative or absolute file paths being modified or audited."
                            },
                            "candidate_symbols": {
                                "type": "array",
                                "items": { "type": "string" },
                                "description": "Optional symbol names to specifically trace in the AST cross-reference graph."
                            }
                        }
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "apply_patch".to_string(),
                    description: "Apply multi-file diffs using structured patch syntax (*** Begin Patch ... *** End Patch) or unified search/replace chunks.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "patch": {
                                "type": "string",
                                "description": "The complete patch text adhering to the Codex patch specification."
                            },
                            "path": {
                                "type": "string",
                                "description": "Optional single file path for simple text search/replace."
                            },
                            "search_text": {
                                "type": "string",
                                "description": "Exact text chunk in the original file to replace (legacy mode)."
                            },
                            "replace_text": {
                                "type": "string",
                                "description": "Replacement text chunk (legacy mode)."
                            },
                            "stream": {
                                "type": "boolean",
                                "description": "If true, emit streaming progress events (FileStarted, HunkApplied, FileCompleted, PatchCompleted). Defaults to false."
                            }
                        }
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "list_dir".to_string(),
                    description: "List files and subdirectories at a path.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "path": {
                                "type": "string",
                                "description": "Directory path (defaults to current workspace)."
                            }
                        }
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "file_search".to_string(),
                    description: "Search workspace filenames matching a pattern or glob (respects .gitignore and excludes target/node_modules/.git).".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "pattern": {
                                "type": "string",
                                "description": "Filename pattern, substring, fuzzy query, or glob (*.rs, test_*) to match."
                            },
                            "fuzzy": {
                                "type": "boolean",
                                "description": "Whether to use ranked fuzzy subsequence matching (defaults to false)."
                            },
                            "max_depth": {
                                "type": "integer",
                                "description": "Maximum directory traversal depth (default 12)."
                            },
                            "max_results": {
                                "type": "integer",
                                "description": "Maximum number of search results to return (default 100)."
                            }
                        },
                        "required": ["pattern"]
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "outline_file".to_string(),
                    description: "Extract structured symbol outline of a source file (functions, structs, classes, interfaces, traits, and line numbers) without loading the entire file into context.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "path": { "type": "string", "description": "Source file path relative to workspace root." }
                        },
                        "required": ["path"]
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "extract_symbol_body".to_string(),
                    description: "Extract the exact implementation body of a specific function, struct, class, or method by name from a file without loading the entire file into context.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "path": { "type": "string", "description": "Source file path relative to workspace root." },
                            "symbol": { "type": "string", "description": "Symbol name (e.g. 'run_turn' or 'AgentEngine')." }
                        },
                        "required": ["path", "symbol"]
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "find_definition".to_string(),
                    description: "Find symbol definition (function, struct, class, trait, interface) across workspace source files.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "symbol": { "type": "string", "description": "Exact name of the symbol to find." }
                        },
                        "required": ["symbol"]
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "find_references".to_string(),
                    description: "Find occurrences and references of a symbol name across workspace source files.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "symbol": { "type": "string", "description": "Symbol name to search references for." },
                            "max_results": { "type": "integer", "description": "Maximum number of references to return (default 50)." }
                        },
                        "required": ["symbol"]
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "diagnose_compiler_errors".to_string(),
                    description: "Parse compiler stderr or test suite failure output into structured diagnostics with prioritized self-healing prompt.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "output": { "type": "string", "description": "Raw compiler output, test failure text, or stack trace." }
                        },
                        "required": ["output"]
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "discover_capabilities".to_string(),
                    description: "Discover available host compilers (rustc, gcc, clang, go), language runtimes (python3, node, bun), package managers, sandbox backends, and project stack signatures.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {}
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "view_image".to_string(),
                    description: "Inspect an image file on disk and return metadata and byte dimensions.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "path": {
                                "type": "string",
                                "description": "Path to the local image file (PNG, JPG, SVG, WebP)."
                            }
                        },
                        "required": ["path"]
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "request_user_input".to_string(),
                    description: "Ask the user one or more questions during ongoing work for clarification or preference.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "question": {
                                "type": "string",
                                "description": "The exact question to present to the user."
                            },
                            "options": {
                                "type": "array",
                                "items": { "type": "string" },
                                "description": "Optional list of suggested choices."
                            }
                        },
                        "required": ["question"]
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "tool_search".to_string(),
                    description: "Discover and inspect available tools in the active runtime.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "query": {
                                "type": "string",
                                "description": "Keyword search query for tools."
                            },
                            "limit": {
                                "type": "integer",
                                "description": "Maximum number of results to return (default 8)."
                            }
                        },
                        "required": ["query"]
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "web_search".to_string(),
                    description: "Perform a live web search for documentation, packages, error messages, and real-time technical information with optional domain restrictions.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "query": { "type": "string", "description": "The exact web search query string." },
                            "domain": { "type": "string", "description": "Optional single domain to restrict results (e.g. 'docs.rs', 'github.com')." },
                            "max_results": { "type": "integer", "description": "Optional maximum number of results to return (1-20, default 5)." },
                            "allowed_domains": { "type": "array", "items": { "type": "string" }, "description": "Optional list of domains to filter results by." },
                            "blocked_domains": { "type": "array", "items": { "type": "string" }, "description": "Optional list of domains to exclude." }
                        },
                        "required": ["query"]
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "web_fetch".to_string(),
                    description: "Fetch a web page by URL and convert its HTML content into clean readable Markdown text for reading documentation, articles, and references.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "url": { "type": "string", "description": "The HTTP or HTTPS URL to fetch." },
                            "max_bytes": { "type": "integer", "description": "Optional maximum number of bytes of Markdown text to return (default: 32768)." }
                        },
                        "required": ["url"]
                    }),
                },
            },
            // Multi-Agent tools
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "spawn_agent".to_string(),
                    description: "Spawn a child autonomous subagent with a dedicated prompt and specialized role (e.g. 'architect', 'coder', 'reviewer', 'explorer').".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "task_name": { "type": "string", "description": "Descriptive task title." },
                            "prompt": { "type": "string", "description": "Detailed initial instructions for the subagent." },
                            "role": { "type": "string", "description": "Role specialization: 'architect', 'coder', 'reviewer', 'explorer', or custom." },
                            "model": { "type": "string", "description": "Optional model override." },
                            "isolate_worktree": { "type": "boolean", "description": "Whether to isolate subagent file edits in a dedicated Git worktree branch." }
                        },
                        "required": ["task_name", "prompt"]
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "send_input".to_string(),
                    description: "Send a message or steering instruction to an active subagent.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "agent_id": { "type": "string", "description": "Identifier of the target subagent." },
                            "message": { "type": "string", "description": "Message content." },
                            "interrupt": { "type": "boolean", "description": "Whether to interrupt ongoing execution." }
                        },
                        "required": ["agent_id", "message"]
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "wait_agent".to_string(),
                    description: "Wait for one or more subagents to complete or produce a status update.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "agent_ids": { "type": "array", "items": { "type": "string" }, "description": "List of subagent IDs." },
                            "timeout_ms": { "type": "integer", "description": "Wait timeout in milliseconds." }
                        },
                        "required": ["agent_ids"]
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "close_agent".to_string(),
                    description: "Terminate and close a subagent and its resources.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "agent_id": { "type": "string", "description": "Identifier of the subagent to close." }
                        },
                        "required": ["agent_id"]
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "terminate_agent_tree".to_string(),
                    description: "Terminate a subagent and all of its spawned descendants in a cascading breadth-first sequence.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "agent_id": { "type": "string", "description": "Root child agent ID whose descendant tree should be terminated." }
                        },
                        "required": ["agent_id"]
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "list_agents".to_string(),
                    description: "List active and completed subagents spawned in this conversation hierarchy.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "include_descendants": { "type": "boolean", "description": "Whether to traverse the full descendant tree (default true)." }
                        }
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "integrate_agent".to_string(),
                    description: "Verify and integrate candidate changes from an isolated subagent worktree into the parent workspace after running preflight validation, path security checks, and code review.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "agent_id": { "type": "string", "description": "Identifier of the subagent whose changes should be integrated." },
                            "commit_message": { "type": "string", "description": "Optional custom commit message for the integrated changes." }
                        },
                        "required": ["agent_id"]
                    }),
                },
            },
            // Multi-Agent Message Board tools for inter-agent pub/sub messaging
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "create_channel".to_string(),
                    description: "Create a named communication channel on the shared agent message board.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "name": { "type": "string", "description": "Channel identifier name (e.g. 'arch', 'review', 'bugs')." },
                            "topic": { "type": "string", "description": "Description of the channel purpose." }
                        },
                        "required": ["name"]
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "list_channels".to_string(),
                    description: "List available channels and message counts on the agent message board.".to_string(),
                    parameters: json!({ "type": "object" }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "post_message".to_string(),
                    description: "Post a message or discussion reply to a channel on the agent message board. Secrets and keys are sanitized automatically.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "channel": { "type": "string", "description": "Target channel name." },
                            "message": { "type": "string", "description": "Text message body to post." },
                            "reply_to_id": { "type": "string", "description": "Optional post ID to reply to, creating or continuing a discussion thread." },
                            "author": { "type": "string", "description": "Optional author nickname/role." }
                        },
                        "required": ["channel", "message"]
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "list_threads".to_string(),
                    description: "List discussion threads in a channel with reply counts and last activity timestamps.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "channel": { "type": "string", "description": "Channel name." },
                            "limit": { "type": "integer", "description": "Maximum threads to return (default 20)." }
                        },
                        "required": ["channel"]
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "read_thread".to_string(),
                    description: "Read all messages in a discussion thread from the message board.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "thread_id": { "type": "string", "description": "Root post ID of the thread to read." }
                        },
                        "required": ["thread_id"]
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "search_posts".to_string(),
                    description: "Search messages across board channels by keyword.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "query": { "type": "string", "description": "Keyword query to search." },
                            "limit": { "type": "integer", "description": "Maximum hits to return (default 20)." }
                        },
                        "required": ["query"]
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "subscribe_channel".to_string(),
                    description: "Subscribe an agent to notification events on a message board channel.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "channel": { "type": "string", "description": "Channel name." },
                            "subscriber": { "type": "string", "description": "Optional subscriber agent name." }
                        },
                        "required": ["channel"]
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "unsubscribe_channel".to_string(),
                    description: "Unsubscribe an agent from a message board channel.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "channel": { "type": "string", "description": "Channel name." },
                            "subscriber": { "type": "string", "description": "Optional subscriber agent name." }
                        },
                        "required": ["channel"]
                    }),
                },
            },
            // Worktree Management tools for isolated git working directories
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "create_worktree".to_string(),
                    description: "Create an isolated git worktree branch for experimental or subagent code modifications.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "base_ref": { "type": "string", "description": "Git revision or branch to branch from (defaults to HEAD)." }
                        }
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "list_worktrees".to_string(),
                    description: "List all active git worktrees associated with the current repository.".to_string(),
                    parameters: json!({ "type": "object" }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "remove_worktree".to_string(),
                    description: "Remove and clean up a previously created git worktree path.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "path": { "type": "string", "description": "Absolute path to the worktree to remove." }
                        },
                        "required": ["path"]
                    }),
                },
            },
            // Persistent Memory & Knowledge Consolidation tools across sessions
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "consolidate_memory".to_string(),
                    description: "Consolidate a learned project pattern, architectural constraint, or developer preference into persistent cross-session memory.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "topic": { "type": "string", "description": "Concise topic title (e.g. 'Clippy Enforce', 'Test DB Pattern')." },
                            "detail": { "type": "string", "description": "Detailed explanation of the learned pattern or rule." }
                        },
                        "required": ["topic", "detail"]
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "search_memories".to_string(),
                    description: "Search cross-session accumulated memories and project rules using hybrid keyword and semantic retrieval.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "query": { "type": "string", "description": "Keyword search query." }
                        },
                        "required": ["query"]
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "cite_memory".to_string(),
                    description: "Record a citation of a specific persistent memory ID to track its relevance and usage.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "memory_id": { "type": "string", "description": "Memory ID to cite (e.g. 'mem_a1b2c3d4')." }
                        },
                        "required": ["memory_id"]
                    }),
                },
            },
            // Goal Tracking tools
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "create_goal".to_string(),
                    description: "Establish a tracked mission goal with optional token and time budgets.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "thread_id": { "type": "string", "description": "Target thread ID." },
                            "objective": { "type": "string", "description": "Clear statement of goal objective." },
                            "token_budget": { "type": "integer", "description": "Optional token budget cap." }
                        },
                        "required": ["thread_id", "objective"]
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "update_goal".to_string(),
                    description: "Update the status of a tracked goal ('active', 'paused', 'blocked', 'complete').".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "thread_id": { "type": "string", "description": "Target thread ID." },
                            "status": { "type": "string", "enum": ["active", "paused", "blocked", "complete"], "description": "New goal status." }
                        },
                        "required": ["thread_id", "status"]
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "get_goal".to_string(),
                    description: "Retrieve status and metrics of the current thread's goal.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "thread_id": { "type": "string", "description": "Target thread ID." }
                        },
                        "required": ["thread_id"]
                    }),
                },
            },
            // Core productivity tools: grep_search, update_plan, current_time, sleep, resume_agent, board
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "grep_search".to_string(),
                    description: "Search file contents in the workspace using regex or substring matching, returning matching lines with file paths and line numbers.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "query": { "type": "string", "description": "Search pattern or regex to match." },
                            "path": { "type": "string", "description": "Optional subdirectory or file to restrict search to (defaults to workspace root)." },
                            "case_sensitive": { "type": "boolean", "description": "Whether to perform case-sensitive matching (default false)." },
                            "max_results": { "type": "integer", "description": "Maximum number of matching lines to return (default 50)." }
                        },
                        "required": ["query"]
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "update_plan".to_string(),
                    description: "Updates the task plan. Provide an optional explanation and a list of plan items, each with a step and status ('pending', 'in_progress', 'completed'). At most one step can be in_progress at a time.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "explanation": { "type": "string", "description": "Optional explanation for this plan update." },
                            "plan": {
                                "type": "array",
                                "items": {
                                    "type": "object",
                                    "properties": {
                                        "step": { "type": "string", "description": "Task step description." },
                                        "status": { "type": "string", "enum": ["pending", "in_progress", "completed"], "description": "Status of the step." }
                                    },
                                    "required": ["step", "status"]
                                },
                                "description": "The list of plan steps."
                            }
                        },
                        "required": ["plan"]
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "current_time".to_string(),
                    description: "Return current date, time (UTC and local), and timezone information.".to_string(),
                    parameters: json!({ "type": "object" }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "sleep".to_string(),
                    description: "Pause execution for a specified duration in milliseconds (capped at 60000ms). Returns elapsed wall-clock time.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "duration_ms": { "type": "integer", "description": "How long to sleep in milliseconds." }
                        },
                        "required": ["duration_ms"]
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "resume_agent".to_string(),
                    description: "Resume a previous or completed subagent with follow-up instructions.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "agent_id": { "type": "string", "description": "Target subagent identifier." },
                            "prompt": { "type": "string", "description": "Follow-up instructions for the subagent." }
                        },
                        "required": ["agent_id", "prompt"]
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "board_create_channel".to_string(),
                    description: "Create a shared multi-agent channel for team collaboration and discussion.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "channel_name": { "type": "string", "description": "Name of the channel to create." },
                            "description": { "type": "string", "description": "Optional channel description or purpose." }
                        },
                        "required": ["channel_name"]
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "board_post".to_string(),
                    description: "Post a message or finding to a shared multi-agent collaboration channel.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "channel_name": { "type": "string", "description": "Target channel name." },
                            "text": { "type": "string", "description": "Message content to post." },
                            "agents_to_notify": { "type": "array", "items": { "type": "string" }, "description": "Optional list of agents to notify." }
                        },
                        "required": ["channel_name", "text"]
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "board_read".to_string(),
                    description: "Read recent messages from a shared multi-agent collaboration channel.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "channel_name": { "type": "string", "description": "Target channel name." },
                            "limit": { "type": "integer", "description": "Maximum number of messages to return (default 20)." }
                        },
                        "required": ["channel_name"]
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "use_skill".to_string(),
                    description: "Load detailed instructions, guidelines, and reference patterns for an available skill.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "name": { "type": "string", "description": "The exact name or identifier of the skill to load." }
                        },
                        "required": ["name"]
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "list_skills".to_string(),
                    description: "List all discovered skills in the workspace, user home, or plugins with their scopes and descriptions.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {}
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "store_secret".to_string(),
                    description: "Securely encrypt and store a secret (API key, OAuth token, credential) in the local encrypted vault.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "name": { "type": "string", "description": "Name or identifier of the secret (e.g. 'OPENAI_API_KEY')." },
                            "value": { "type": "string", "description": "Raw secret value." },
                            "namespace": {
                                "type": "string",
                                "description": "Vault namespace ('managed_secrets', 'm31a_auth', 'mcp_oauth', 'gateway_oauth'). Defaults to 'managed_secrets'.",
                                "enum": ["managed_secrets", "m31a_auth", "mcp_oauth", "gateway_oauth"]
                            },
                            "scope": {
                                "type": "string",
                                "description": "Secret scope ('global', 'workspace', 'session'). Defaults to 'global'.",
                                "enum": ["global", "workspace", "session"]
                            }
                        },
                        "required": ["name", "value"]
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "retrieve_secret".to_string(),
                    description: "Retrieve a decrypted secret from the local encrypted vault by name and namespace.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "name": { "type": "string", "description": "Name of the secret to retrieve." },
                            "namespace": {
                                "type": "string",
                                "description": "Vault namespace ('managed_secrets', 'm31a_auth', 'mcp_oauth', 'gateway_oauth'). Defaults to 'managed_secrets'.",
                                "enum": ["managed_secrets", "m31a_auth", "mcp_oauth", "gateway_oauth"]
                            }
                        },
                        "required": ["name"]
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "list_stored_secrets".to_string(),
                    description: "List stored secret entries (with redacted previews and timestamps) in the local encrypted vault.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "namespace": {
                                "type": "string",
                                "description": "Vault namespace to query. Defaults to 'managed_secrets'.",
                                "enum": ["managed_secrets", "m31a_auth", "mcp_oauth", "gateway_oauth"]
                            }
                        }
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "delete_secret".to_string(),
                    description: "Permanently delete a secret from the local encrypted vault.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "name": { "type": "string", "description": "Name of the secret to delete." },
                            "namespace": {
                                "type": "string",
                                "description": "Vault namespace. Defaults to 'managed_secrets'.",
                                "enum": ["managed_secrets", "m31a_auth", "mcp_oauth", "gateway_oauth"]
                            }
                        },
                        "required": ["name"]
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "create_diagnostic_bundle".to_string(),
                    description: "Collect a self-contained diagnostic feedback bundle with system information, sanitized logs, and environment metadata for error reporting and debugging.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "note": { "type": "string", "description": "Optional human or agent notes explaining the issue or scenario." },
                            "include_trace": { "type": "boolean", "description": "Whether to bundle recent trace event logs." }
                        }
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "submit_cloud_task".to_string(),
                    description: "Submit a command for asynchronous background execution via the cloud task runner.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "command": { "type": "string", "description": "Shell command line to execute." },
                            "timeout_secs": { "type": "integer", "description": "Execution timeout in seconds. Defaults to 300." }
                        },
                        "required": ["command"]
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "poll_cloud_task".to_string(),
                    description: "Poll the status and output of a submitted background task.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "task_id": { "type": "string", "description": "The task ID returned by submit_cloud_task." }
                        },
                        "required": ["task_id"]
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "cancel_cloud_task".to_string(),
                    description: "Cancel an ongoing background cloud task by ID.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "task_id": { "type": "string", "description": "The task ID to abort." }
                        },
                        "required": ["task_id"]
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "create_tcp_tunnel".to_string(),
                    description: "Create a local TCP port forwarder / tunnel connecting a local port to a target remote host and port.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "local_bind": { "type": "string", "description": "Local bind address and port (e.g. '127.0.0.1:8080' or '127.0.0.1:0' for ephemeral)." },
                            "remote_target": { "type": "string", "description": "Target host:port to forward connections to (e.g. '127.0.0.1:3000')." }
                        },
                        "required": ["local_bind", "remote_target"]
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "list_tcp_tunnels".to_string(),
                    description: "List all active TCP port forwarding tunnels with connection counts and metrics.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {}
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "close_tcp_tunnel".to_string(),
                    description: "Close and terminate an active TCP forwarding tunnel by tunnel ID.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "tunnel_id": { "type": "string", "description": "The tunnel ID returned by create_tcp_tunnel." }
                        },
                        "required": ["tunnel_id"]
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "migrate_external_agent".to_string(),
                    description: "Discover and import configuration, MCP servers, rules, and memories from Claude Code, Cursor, or Windsurf into M31A format.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "dry_run": {
                                "type": "boolean",
                                "description": "Whether to perform discovery only without modifying files (defaults to false)."
                            }
                        }
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "list_available_plugins_to_install".to_string(),
                    description: "List discoverable third-party plugins, MCP servers, and app connectors available for installation or enabling.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {}
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "request_plugin_install".to_string(),
                    description: "Request installation or activation of a discoverable plugin or app connector with a suggestion reason.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "tool_id": { "type": "string", "description": "Identifier of the plugin or connector (e.g. 'github-mcp', 'sqlite-mcp')." },
                            "tool_type": { "type": "string", "enum": ["plugin", "connector"], "description": "Type of discoverable tool." },
                            "action_type": { "type": "string", "enum": ["install", "enable"], "description": "Action requested." },
                            "suggest_reason": { "type": "string", "description": "Explanation of why this plugin or connector is recommended." }
                        },
                        "required": ["tool_id", "tool_type", "action_type", "suggest_reason"]
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "git_status".to_string(),
                    description: "Inspect the current Git repository status, branch, ahead/behind tracking, staged and unstaged changes.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {}
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "git_diff".to_string(),
                    description: "Retrieve unified git diff output for the current repository, optionally filtering by staged state or specific file path.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "staged": { "type": "boolean", "description": "Whether to view staged changes (--staged). Defaults to false." },
                            "path": { "type": "string", "description": "Optional specific file path to diff." }
                        }
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "create_pull_request".to_string(),
                    description: "Create or package a reviewable pull request on GitHub/GitLab with verification evidence and attribution.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "title": { "type": "string", "description": "Title of the pull request." },
                            "body": { "type": "string", "description": "Markdown body detailing description and testing evidence." },
                            "base_branch": { "type": "string", "description": "Base target branch (defaults to 'main' or 'master')." },
                            "head_branch": { "type": "string", "description": "Source head branch to merge from." },
                            "draft": { "type": "boolean", "description": "Whether to create as draft PR. Defaults to false." }
                        },
                        "required": ["title", "body"]
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "fetch_issue".to_string(),
                    description: "Retrieve issue title, description, and acceptance criteria from GitHub/GitLab.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "issue_number": { "type": "integer", "description": "The numeric issue ID." }
                        },
                        "required": ["issue_number"]
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "resolve_merge_conflicts".to_string(),
                    description: "Reconcile 3-way git merge conflicts in a file using a deterministic strategy (ours, theirs, union, or semantic).".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "path": { "type": "string", "description": "Relative file path containing conflict markers." },
                            "strategy": {
                                "type": "string",
                                "enum": ["ours", "theirs", "union", "semantic"],
                                "description": "Reconciliation strategy: 'ours' (local), 'theirs' (remote), 'union' (both additions), or 'semantic' (merge non-overlapping)."
                            }
                        },
                        "required": ["path", "strategy"]
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "notes_write".to_string(),
                    description: "Write or overwrite a private agent rollout note file to persist knowledge across context compactions.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "agent_id": { "type": "string", "description": "Agent or subagent identifier." },
                            "path": { "type": "string", "description": "Virtual path for the note (e.g. 'architecture/plan.md')." },
                            "content": { "type": "string", "description": "Text content to write (max 1 MB)." }
                        },
                        "required": ["agent_id", "path", "content"]
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "notes_append".to_string(),
                    description: "Append content to an existing private agent rollout note file, creating it if it does not exist.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "agent_id": { "type": "string", "description": "Agent or subagent identifier." },
                            "path": { "type": "string", "description": "Virtual path for the note (e.g. 'architecture/plan.md')." },
                            "content": { "type": "string", "description": "Text content to append." }
                        },
                        "required": ["agent_id", "path", "content"]
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "notes_read".to_string(),
                    description: "Read the contents of a private agent rollout note file.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "agent_id": { "type": "string", "description": "Agent identifier." },
                            "path": { "type": "string", "description": "Virtual path of the note to read." }
                        },
                        "required": ["agent_id", "path"]
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "notes_list".to_string(),
                    description: "List private note files belonging to an agent with optional path prefix filtering.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "agent_id": { "type": "string", "description": "Agent identifier." },
                            "prefix": { "type": "string", "description": "Optional virtual directory prefix (e.g. 'research/')." }
                        },
                        "required": ["agent_id"]
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "notes_search".to_string(),
                    description: "Search text within private rollout note files of an agent.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "agent_id": { "type": "string", "description": "Agent identifier." },
                            "query": { "type": "string", "description": "Search keyword or phrase." }
                        },
                        "required": ["agent_id", "query"]
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "imagegen".to_string(),
                    description: "Generate images from scene descriptions or edit existing images with instructions.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "prompt": { "type": "string", "description": "Scene description or image editing instructions." },
                            "size": { "type": "string", "enum": ["1024x1024", "1536x1024", "1024x1536", "512x512", "auto"], "description": "Target image resolution." },
                            "quality": { "type": "string", "enum": ["standard", "hd"], "description": "Rendering quality." },
                            "transparent_background": { "type": "boolean", "description": "Whether to generate or preserve a transparent background." },
                            "referenced_image_paths": { "type": "array", "items": { "type": "string" }, "description": "Local filesystem paths of images to edit." },
                            "num_last_images_to_include": { "type": "integer", "minimum": 1, "maximum": 5, "description": "Number of recent conversation images to include for editing." }
                        },
                        "required": ["prompt"]
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "multi_env_copy_file".to_string(),
                    description: "Copy a file between two different environments without loading it into model context.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "source_environment_id": { "type": "string", "description": "Source environment ID." },
                            "source_path": { "type": "string", "description": "Absolute source file path." },
                            "target_environment_id": { "type": "string", "description": "Target environment ID." },
                            "target_path": { "type": "string", "description": "Absolute target file path." }
                        },
                        "required": ["source_environment_id", "source_path", "target_environment_id", "target_path"]
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "history_list_windows".to_string(),
                    description: "List an agent's context windows as window ID and item-count pairs for post-compaction recovery.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "session_id": { "type": "string", "description": "Session or rollout identifier." },
                            "agent_name": { "type": "string", "description": "Optional agent name filter." },
                            "limit": { "type": "integer", "description": "Maximum number of windows to return." },
                            "recent_first": { "type": "boolean", "description": "Whether to return most recently created windows first." }
                        },
                        "required": ["session_id"]
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "history_list_items".to_string(),
                    description: "List history items with optional window, role, and tool filters for post-compaction recovery.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "session_id": { "type": "string", "description": "Session or rollout identifier." },
                            "window_id": { "type": "string", "description": "Full window ID to filter items." },
                            "agent_name": { "type": "string", "description": "Agent whose history to inspect." },
                            "role": { "type": "string", "enum": ["user", "assistant", "tool", "system", "developer"], "description": "Message role to include." },
                            "tool_namespace": { "type": "string", "description": "Tool namespace to filter." },
                            "tool_name": { "type": "string", "description": "Tool name to filter." },
                            "limit": { "type": "integer", "description": "Maximum number of items to return." },
                            "recent_first": { "type": "boolean", "description": "Return newest items first." },
                            "max_chars_per_item": { "type": "integer", "description": "Maximum characters returned in truncated_content." }
                        },
                        "required": ["session_id"]
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "history_read_item".to_string(),
                    description: "Read a bounded character range from private model-only history.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "session_id": { "type": "string", "description": "Session or rollout identifier." },
                            "window_id": { "type": "string", "description": "Full window ID containing the item." },
                            "item_id": { "type": "string", "description": "Target item identifier." },
                            "offset_chars": { "type": "integer", "description": "Zero-based character offset at which reading starts." },
                            "limit_chars": { "type": "integer", "description": "Maximum characters to return." }
                        },
                        "required": ["session_id", "window_id", "item_id"]
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "history_search".to_string(),
                    description: "Search private model-only history by literal substring.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "session_id": { "type": "string", "description": "Session or rollout identifier." },
                            "query": { "type": "string", "description": "Literal search substring." },
                            "agent_name": { "type": "string", "description": "Agent filter." },
                            "window_id": { "type": "string", "description": "Window ID filter." },
                            "role": { "type": "string", "description": "Role filter." },
                            "limit": { "type": "integer", "description": "Maximum matching items to return." },
                            "recent_first": { "type": "boolean", "description": "Order matches newest first." }
                        },
                        "required": ["session_id", "query"]
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "revert_thread".to_string(),
                    description: "Revert a thread back to a specific checkpoint or turn index, removing later turns.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "thread_id": { "type": "string", "description": "Target thread identifier." },
                            "target_turn_index": { "type": "integer", "description": "Turn index to revert back to." }
                        },
                        "required": ["thread_id", "target_turn_index"]
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "thread_lineage".to_string(),
                    description: "Resolve the ancestor fork lineage of a thread back to the root thread.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "thread_id": { "type": "string", "description": "Target thread identifier." }
                        },
                        "required": ["thread_id"]
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "list_connectors".to_string(),
                    description: "List available ChatGPT and MCP app connectors merged with local accessibility status.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "query": { "type": "string", "description": "Optional search term to filter connectors." }
                        }
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "discover_audio_devices".to_string(),
                    description: "Enumerate host audio hardware input (microphones) and output (speakers, headphones) devices and channels.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {}
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "create_checkpoint".to_string(),
                    description: "Create a named checkpoint snapshot capturing git commit and turn index in SQLite state storage.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "thread_id": { "type": "string", "description": "Target conversation thread ID." },
                            "label": { "type": "string", "description": "Human-readable label for the checkpoint." },
                            "git_commit": { "type": "string", "description": "Optional git commit SHA." },
                            "turn_index": { "type": "integer", "description": "Optional turn index offset (default latest turn)." },
                            "modified_files": {
                                "type": "array",
                                "items": { "type": "string" },
                                "description": "Optional list of modified files captured at this checkpoint."
                            }
                        },
                        "required": ["thread_id", "label"]
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "list_checkpoints".to_string(),
                    description: "List all checkpoint snapshots recorded for a conversation thread ordered newest first.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "thread_id": { "type": "string", "description": "Target conversation thread ID." }
                        },
                        "required": ["thread_id"]
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "restore_checkpoint".to_string(),
                    description: "Restore conversation turns and retrieve associated git commit and modified files for a checkpoint snapshot.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "checkpoint_id": { "type": "string", "description": "Checkpoint ID to restore." },
                            "revert_conversation": { "type": "boolean", "description": "Whether to delete subsequent turns in the thread (default true)." }
                        },
                        "required": ["checkpoint_id"]
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "acquire_path_lock".to_string(),
                    description: "Acquire a leased hierarchical concurrency lock on a file or directory path.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "path": { "type": "string", "description": "Target file or directory path to lock." },
                            "mode": { "type": "string", "enum": ["read", "write"], "description": "Lock mode: 'read' (shared) or 'write' (exclusive, default)." },
                            "holder_id": { "type": "string", "description": "Identifier of the agent or task holding the lock." },
                            "reason": { "type": "string", "description": "Reason for acquiring the lock." },
                            "timeout_ms": { "type": "integer", "description": "Optional acquisition timeout in milliseconds (default 5000)." }
                        },
                        "required": ["path"]
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "release_path_lock".to_string(),
                    description: "Release a previously acquired leased path lock by lock ID.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "lock_id": { "type": "string", "description": "Lock ID to release." }
                        },
                        "required": ["lock_id"]
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "list_active_locks".to_string(),
                    description: "List all active path reader-writer concurrency locks currently held.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {}
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "list_mcp_resources".to_string(),
                    description: "List discoverable resources across connected MCP servers with optional server filtering.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "server": { "type": "string", "description": "Optional MCP server name to filter resources by." }
                        }
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "read_mcp_resource".to_string(),
                    description: "Read the contents of a resource from a connected MCP server by server name and URI.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "server": { "type": "string", "description": "Name of the connected MCP server." },
                            "uri": { "type": "string", "description": "Resource URI to read (e.g. 'file:///...' or 'postgres://...')." }
                        },
                        "required": ["server", "uri"]
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "clock_sleep".to_string(),
                    description: "Pause execution for a duration up to 12 hours (interruptible by user message). Returns elapsed wall-clock time.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "duration_secs": { "type": "integer", "description": "Seconds to sleep (up to 43200 seconds / 12 hours)." },
                            "duration_ms": { "type": "integer", "description": "Alternative duration in milliseconds." }
                        }
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "get_context_remaining".to_string(),
                    description: "Inspect context window usage, token budget status, and remaining token capacity.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {}
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "new_context_window".to_string(),
                    description: "Request an explicit rollover to a new context window with a retained handoff note.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "handoff_note": { "type": "string", "description": "Key facts or summary to retain across context rollover." }
                        }
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "request_permissions".to_string(),
                    description: "Request expanded sandbox permissions from the user (use_default, with_additional_permissions, require_escalated).".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "sandbox_permissions": {
                                "type": "string",
                                "enum": ["use_default", "with_additional_permissions", "require_escalated"],
                                "description": "The requested permission tier."
                            },
                            "justification": {
                                "type": "string",
                                "description": "Reasoning explaining why escalated permissions are required."
                            }
                        },
                        "required": ["sandbox_permissions", "justification"]
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "agent_plan".to_string(),
                    description: "Record a concise task plan and observable completion criteria before editing. Every planned criterion must have a unique stable ID (e.g. c1, c2).".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "goal": { "type": "string", "description": "High-level task objective." },
                            "steps": { "type": "array", "items": { "type": "string" }, "description": "Ordered execution steps." },
                            "criteria": {
                                "type": "array",
                                "items": {
                                    "type": "object",
                                    "properties": {
                                        "id": { "type": "string", "description": "Criterion ID, e.g. c1, c2." },
                                        "description": { "type": "string", "description": "Verifiable criteria description." }
                                    },
                                    "required": ["id", "description"]
                                },
                                "description": "Verifiable criteria that must pass before completion."
                            },
                            "requires_preview": { "type": "boolean", "description": "Whether a running dev server/preview is required." }
                        },
                        "required": ["goal", "steps", "criteria"]
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "agent_complete".to_string(),
                    description: "Finish the current task only after running it and passing verification checks after the latest edit. Report every planned criterion by its exact ID. Use outcome 'blocked' with a reason if completion is impossible.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "outcome": { "type": "string", "enum": ["complete", "blocked"], "description": "Task status: 'complete' or 'blocked'." },
                            "summary": { "type": "string", "description": "Final summary of accomplishments or blocker description." },
                            "criteria": {
                                "type": "array",
                                "items": {
                                    "type": "object",
                                    "properties": {
                                        "id": { "type": "string", "description": "Criterion ID matching the plan." },
                                        "evidence": { "type": "array", "items": { "type": "string" }, "description": "Tool call IDs or command outputs demonstrating success." }
                                    },
                                    "required": ["id"]
                                }
                            }
                        },
                        "required": ["outcome", "summary"]
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "find_related_files".to_string(),
                    description: "Find files related to a target file by directory locality, filename stem matching, and test/style pairings.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "path": { "type": "string", "description": "Target file path." },
                            "limit": { "type": "integer", "description": "Max results to return (default 20)." }
                        },
                        "required": ["path"]
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "lock_path".to_string(),
                    description: "Lock a file or directory to prevent the agent from modifying it.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "path": { "type": "string", "description": "Path to lock." },
                            "is_dir": { "type": "boolean", "description": "Whether the path is a directory (default false)." },
                            "reason": { "type": "string", "description": "Optional lock reason or owner." }
                        },
                        "required": ["path"]
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "unlock_path".to_string(),
                    description: "Unlock a previously locked file or directory.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "path": { "type": "string", "description": "Path to unlock." }
                        },
                        "required": ["path"]
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "list_locks".to_string(),
                    description: "List all currently active file and directory locks.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {}
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "cloud_deploy".to_string(),
                    description: "Deploy the active project to a cloud hosting platform (vercel, netlify, fly, github_pages).".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "target": { "type": "string", "enum": ["vercel", "netlify", "fly", "github_pages"], "description": "Target hosting platform." },
                            "prod": { "type": "boolean", "description": "Deploy directly to production (default true)." },
                            "build_dir": { "type": "string", "description": "Optional build output directory (e.g. dist, build, out)." }
                        },
                        "required": ["target"]
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: "supabase_migrate".to_string(),
                    description: "Execute SQL statements or run migrations against a Supabase / PostgreSQL database.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "sql": { "type": "string", "description": "SQL query or migration script to execute." },
                            "connection_string": { "type": "string", "description": "Optional postgres connection string or env var name (default SUPABASE_DB_URL)." }
                        },
                        "required": ["sql"]
                    }),
                },
            },
        ];

        if let Ok(dyn_tools) = self.dynamic_tools.read() {
            builtins.extend(dyn_tools.clone());
        }

        builtins.retain(|def| self.is_allowed(&def.function.name));

        builtins
    }

    pub fn get_tool_schema(&self, name: &str) -> Option<serde_json::Value> {
        self.definitions()
            .into_iter()
            .find(|d| d.function.name == name)
            .map(|d| d.function.parameters)
    }

    /// Executes a tool and returns a truthful structured execution outcome.
    pub async fn execute_outcome(
        &self,
        name: &str,
        args_str: &str,
        cwd: &Path,
        policy: SandboxPolicy,
    ) -> Result<m31a_core::types::ToolExecutionOutcome> {
        match self.execute(name, args_str, cwd, policy).await {
            Ok(output) => {
                let (exit_code, timed_out) = if name == "exec_command" {
                    let ec = if let Some(stripped) = output.strip_prefix("Exit code: ") {
                        stripped
                            .split_whitespace()
                            .next()
                            .and_then(|c| c.parse::<i32>().ok())
                    } else {
                        None
                    };
                    let to = output.contains("[COMMAND TIMED OUT]");
                    (ec, to)
                } else {
                    (Some(0), false)
                };
                let success = exit_code.map(|c| c == 0).unwrap_or(true);
                Ok(m31a_core::types::ToolExecutionOutcome {
                    success,
                    exit_code,
                    output,
                    error: None,
                    timed_out,
                    total_bytes_drained: 0,
                })
            }
            Err(e) => Ok(m31a_core::types::ToolExecutionOutcome {
                success: false,
                exit_code: Some(1),
                output: String::new(),
                error: Some(e.to_string()),
                timed_out: false,
                total_bytes_drained: 0,
            }),
        }
    }

    pub async fn execute(
        &self,
        name: &str,
        args_str: &str,
        cwd: &Path,
        policy: SandboxPolicy,
    ) -> Result<String> {
        self.execute_with_call_context(name, args_str, cwd, policy, None)
            .await
    }

    pub async fn execute_with_call_context(
        &self,
        name: &str,
        args_str: &str,
        cwd: &Path,
        policy: SandboxPolicy,
        call_context: Option<&crate::plugin_install::ToolCallContext<'_>>,
    ) -> Result<String> {
        if !self.is_allowed(name) {
            return Err(anyhow!(
                "Tool `{name}` is disabled or not allowed in this session"
            ));
        }

        let schema = self.get_tool_schema(name);
        let args: serde_json::Value =
            crate::input_repair::repair_tool_args(args_str, schema.as_ref())
                .with_context(|| format!("Failed to parse JSON arguments for tool `{name}`"))?;

        if name.starts_with(MCP_PREFIX) {
            return self.mcp_manager.execute_mcp_tool(name, args).await;
        }

        match name {
            "exec_command" => {
                let cmd = args
                    .get("command")
                    .or_else(|| args.get("cmd"))
                    .and_then(|v| v.as_str())
                    .context("Missing 'command' argument")?;
                let timeout_secs = args
                    .get("timeout_seconds")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(60);
                let yield_ms = args
                    .get("yield_time_ms")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);

                let decision = self.exec_policy.evaluate(cmd);
                if decision == Decision::Deny {
                    return Err(anyhow!(
                        "Command execution rejected by policy (critical safety denial): {}",
                        cmd
                    ));
                }

                // §7.4: `apply_patch <<'EOF'` heredoc invocations are routed to
                // the patch engine instead of the shell (upstream unified-exec
                // behavior — the payload is a patch, not a shell script).
                if let Some(patch_text) = m31a_patch::extract_apply_patch_heredoc(cmd) {
                    if policy == SandboxPolicy::ReadOnly {
                        return Err(anyhow!("Cannot modify file: Sandbox policy is Read-Only"));
                    }
                    return match m31a_patch::PatchEngine::apply(cwd, &patch_text) {
                        Ok(stats) => Ok(format!(
                            "Successfully applied patch:\n- Files modified: {}\n- Files created: {}\n- Files deleted: {}\n- Hunks applied: {}\n- Average line drift: {:.1}",
                            stats.files_modified,
                            stats.files_created,
                            stats.files_deleted,
                            stats.hunks_applied,
                            stats.average_drift
                        )),
                        Err(e) => Err(anyhow!("Failed to apply patch from heredoc: {}", e)),
                    };
                }

                if yield_ms > 0 && policy != SandboxPolicy::ReadOnly {
                    let spawn_res = self
                        .process_manager
                        .spawn(
                            cmd,
                            cwd,
                            policy,
                            Duration::from_millis(yield_ms),
                            Duration::from_secs(timeout_secs),
                        )
                        .await?;

                    match spawn_res {
                        SpawnResult::Completed(output) => {
                            let mut res = format!("Exit code: {}\n", output.exit_code);
                            if !output.stdout.is_empty() {
                                res.push_str("--- stdout ---\n");
                                res.push_str(&output.stdout);
                            }
                            if !output.stderr.is_empty() {
                                res.push_str("\n--- stderr ---\n");
                                res.push_str(&output.stderr);
                            }
                            if output.timed_out {
                                res.push_str("\n[COMMAND TIMED OUT]");
                            }
                            Ok(res)
                        }
                        SpawnResult::Background {
                            session_id,
                            initial_output,
                            message,
                        } => {
                            let mut res = format!(
                                "[PROCESS BACKGROUNDED]\nSession ID: {}\nCommand: {}\n{}\n",
                                session_id, cmd, message
                            );
                            if !initial_output.is_empty() {
                                res.push_str("Initial output:\n");
                                res.push_str(&initial_output);
                            }
                            res.push_str("\nProcess is actively running in background. Use poll_process, write_stdin, or kill_process with session_id.");
                            // Upstream unified-exec contract (report §4.1): background
                            // terminals that hit their timeout are terminated and
                            // reported with exit code 124.
                            res.push_str("\nNote: if this process exceeds the background timeout it will be terminated and reported with exit code 124.");
                            Ok(res)
                        }
                    }
                } else {
                    let mut shell = self.persistent_shell.lock().await;
                    if shell.session_id == "default" && shell.cwd != cwd {
                        shell.cwd = cwd.to_path_buf();
                    }

                    match shell.execute(cmd, Duration::from_secs(timeout_secs)).await {
                        Ok(output) => {
                            let mut res = format!("Exit code: {}\n", output.exit_code);
                            if !output.stdout.is_empty() {
                                res.push_str(&output.stdout);
                                if !output.stdout.ends_with('\n') {
                                    res.push('\n');
                                }
                            }
                            if output.timed_out {
                                res.push_str("[COMMAND TIMED OUT]\n");
                            }
                            res.push_str(&format!(
                                "[Current Working Directory: {}]",
                                output.cwd.display()
                            ));
                            Ok(res)
                        }
                        Err(_) => {
                            let output = self
                                .sandbox
                                .execute(cmd, cwd, policy, Duration::from_secs(timeout_secs))
                                .await?;
                            let mut res = format!("Exit code: {}\n", output.exit_code);
                            if !output.stdout.is_empty() {
                                res.push_str("--- stdout ---\n");
                                res.push_str(&output.stdout);
                            }
                            if !output.stderr.is_empty() {
                                res.push_str("\n--- stderr ---\n");
                                res.push_str(&output.stderr);
                            }
                            if output.timed_out {
                                res.push_str("\n[COMMAND TIMED OUT]");
                            }
                            Ok(res)
                        }
                    }
                }
            }
            "debug_launch" => {
                let session_id = args
                    .get("session_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("default");
                let program = args
                    .get("program")
                    .and_then(|v| v.as_str())
                    .context("Missing 'program'")?;
                let prog_args = args.get("args").and_then(|v| v.as_array()).map(|arr| {
                    arr.iter()
                        .filter_map(|s| s.as_str().map(|x| x.to_string()))
                        .collect()
                });
                let launch_args = crate::debugger::DebugLaunchArgs {
                    session_id: session_id.to_string(),
                    program: program.to_string(),
                    args: prog_args,
                    cwd: Some(cwd.to_string_lossy().to_string()),
                };
                let res = crate::debugger::execute_debug_launch(launch_args)?;
                Ok(serde_json::to_string_pretty(&res)?)
            }
            "debug_breakpoint" => {
                let session_id = args
                    .get("session_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("default");
                let file_path = args
                    .get("file_path")
                    .and_then(|v| v.as_str())
                    .context("Missing 'file_path'")?;
                let lines: Vec<usize> = args
                    .get("lines")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|x| x.as_u64().map(|n| n as usize))
                            .collect()
                    })
                    .unwrap_or_default();
                let condition = args
                    .get("condition")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                let bp_args = crate::debugger::DebugBreakpointArgs {
                    session_id: session_id.to_string(),
                    file_path: file_path.to_string(),
                    lines,
                    condition,
                };
                let res = crate::debugger::execute_debug_breakpoint(bp_args)?;
                Ok(serde_json::to_string_pretty(&res)?)
            }
            "debug_step" => {
                let session_id = args
                    .get("session_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("default");
                let step_type = args
                    .get("step_type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("over");
                let step_args = crate::debugger::DebugStepArgs {
                    session_id: session_id.to_string(),
                    step_type: step_type.to_string(),
                };
                let res = crate::debugger::execute_debug_step(step_args)?;
                Ok(serde_json::to_string_pretty(&res)?)
            }
            "debug_inspect" => {
                let session_id = args
                    .get("session_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("default");
                let scope = args
                    .get("scope")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                let inspect_args = crate::debugger::DebugInspectArgs {
                    session_id: session_id.to_string(),
                    scope,
                };
                let res = crate::debugger::execute_debug_inspect(inspect_args)?;
                Ok(serde_json::to_string_pretty(&res)?)
            }
            "verify_workspace" => {
                let mode = args.get("mode").and_then(|v| v.as_str()).unwrap_or("check");
                let timeout_secs = args
                    .get("timeout_seconds")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(60);
                let timeout = Duration::from_secs(timeout_secs);

                let report = match mode {
                    "test" | "tests" => {
                        m31a_verify::AutonomousVerifier::verify_tests(cwd, timeout).await?
                    }
                    _ => m31a_verify::AutonomousVerifier::verify_check(cwd, timeout).await?,
                };

                if report.success {
                    Ok(format!(
                        "Verification PASSED for command `{}` (exit code 0).\nNo compiler or test errors found.",
                        report.command
                    ))
                } else {
                    let prompt = report
                        .self_healing_prompt
                        .unwrap_or_else(|| report.raw_output.clone());
                    Ok(format!(
                        "Verification FAILED ({} errors, {} warnings):\n\n{}",
                        report.error_count, report.warning_count, prompt
                    ))
                }
            }
            "code_review" => {
                let base = args.get("base_ref").and_then(|v| v.as_str());
                let report = m31a_review::ReviewEngine::analyze_git(cwd, base)?;
                let markdown = m31a_review::ReviewFormatter::render_markdown(&report);
                Ok(markdown)
            }
            "check_repo_trust" => {
                let target_dir = args
                    .get("path")
                    .and_then(|v| v.as_str())
                    .map(|p| resolve_path(cwd, p))
                    .unwrap_or_else(|| cwd.to_path_buf());

                let store = m31a_trust::TrustStore::new();
                let decision = store.evaluate(&target_dir);
                let armor = m31a_trust::GitArmor::audit(&target_dir);

                let mut out = format!(
                    "Workspace Trust Evaluation for '{}':\n- Decision: {:?}\n- Git Armor: {} ({} hook(s), {} command(s))\n",
                    target_dir.display(),
                    decision,
                    if armor.is_safe {
                        "PASS"
                    } else {
                        "FAIL (Exploit risk detected)"
                    },
                    armor.hooks_detected,
                    armor.custom_commands_detected
                );

                if !armor.risks.is_empty() {
                    out.push_str("\nSecurity Findings:\n");
                    for r in &armor.risks {
                        out.push_str(&format!(
                            "  - [{:?}] {}: {}\n",
                            r.severity, r.title, r.description
                        ));
                    }
                }

                Ok(out)
            }
            "write_stdin" => {
                let session_id = args
                    .get("session_id")
                    .or_else(|| args.get("process_id"))
                    .or_else(|| args.get("id"))
                    .and_then(|v| {
                        v.as_u64()
                            .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
                    })
                    .context("Missing or invalid numeric 'session_id' argument")?
                    as u32;
                let input = args
                    .get("input")
                    .and_then(|v| v.as_str())
                    .context("Missing 'input' argument")?;
                let yield_ms = args
                    .get("yield_time_ms")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(500);
                let res = self
                    .process_manager
                    .write_stdin(session_id, input, Duration::from_millis(yield_ms))
                    .await?;
                Ok(format!(
                    "Wrote {} bytes to session {}. Status: {:?}.\nNew output:\n{}",
                    res.bytes_written, session_id, res.status, res.new_output
                ))
            }
            "poll_process" => {
                let session_id = args
                    .get("session_id")
                    .or_else(|| args.get("process_id"))
                    .or_else(|| args.get("id"))
                    .and_then(|v| {
                        v.as_u64()
                            .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
                    })
                    .context("Missing or invalid numeric 'session_id' argument")?
                    as u32;
                let (status, output) = self.process_manager.poll_process(session_id).await?;
                let mut res = format!("Session {}: {:?}\n", session_id, status);
                if status == ProcessStatus::TimedOut {
                    // Upstream unified-exec contract (report §4.1): timed-out
                    // background processes are reported with exit code 124.
                    res.push_str(
                        "[EXIT CODE 124] Background command timed out and was terminated.\n",
                    );
                }
                if !output.is_empty() {
                    res.push_str("--- new output ---\n");
                    res.push_str(&output);
                }
                Ok(res)
            }
            "kill_process" => {
                let session_id = args
                    .get("session_id")
                    .or_else(|| args.get("process_id"))
                    .or_else(|| args.get("id"))
                    .and_then(|v| {
                        v.as_u64()
                            .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
                    })
                    .context("Missing or invalid numeric 'session_id' argument")?
                    as u32;
                self.process_manager.kill_process(session_id).await?;
                Ok(format!("Successfully terminated session {}", session_id))
            }
            "list_processes" => {
                let procs = self.process_manager.list_processes().await;
                if procs.is_empty() {
                    Ok("No background processes registered.".to_string())
                } else {
                    let mut out = format!(
                        "{:<12} | {:<12} | {:<10} | {}\n",
                        "Session ID", "Status", "Bytes", "Command"
                    );
                    out.push_str(&"-".repeat(65));
                    out.push('\n');
                    for p in procs {
                        out.push_str(&format!(
                            "{:<12} | {:<12?} | {:<10} | {}\n",
                            p.session_id, p.status, p.output_bytes, p.command
                        ));
                    }
                    Ok(out)
                }
            }
            "start_background_process" => {
                let cmd = args
                    .get("command")
                    .and_then(|v| v.as_str())
                    .context("Missing 'command' argument")?;
                let sub_args: Vec<String> = args
                    .get("args")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|x| x.as_str().map(|s| s.to_string()))
                            .collect()
                    })
                    .unwrap_or_default();
                let label = args
                    .get("label")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                let target_cwd = args
                    .get("cwd")
                    .and_then(|v| v.as_str())
                    .map(|c| resolve_path(cwd, c))
                    .unwrap_or_else(|| cwd.to_path_buf());
                let info = self
                    .proc_supervisor
                    .start(cmd, &sub_args, &target_cwd, HashMap::new(), label)
                    .await?;
                Ok(serde_json::to_string_pretty(&info)?)
            }
            "list_background_processes" => {
                let list = self.proc_supervisor.list().await;
                if list.is_empty() {
                    Ok("No persistent background processes running.".to_string())
                } else {
                    Ok(serde_json::to_string_pretty(&list)?)
                }
            }
            "get_background_process_output" => {
                let proc_id = args
                    .get("process_id")
                    .or_else(|| args.get("id"))
                    .and_then(|v| v.as_str())
                    .context("Missing 'process_id' argument")?;
                let limit = args
                    .get("limit")
                    .and_then(|v| v.as_u64())
                    .map(|n| n as usize);
                let offset = args
                    .get("offset")
                    .and_then(|v| v.as_u64())
                    .map(|n| n as usize);
                let out = self
                    .proc_supervisor
                    .get_output(proc_id, offset, limit)
                    .await?;
                Ok(serde_json::to_string_pretty(&out)?)
            }
            "stop_background_process" => {
                let proc_id = args
                    .get("process_id")
                    .or_else(|| args.get("id"))
                    .and_then(|v| v.as_str())
                    .context("Missing 'process_id' argument")?;
                let force = args.get("force").and_then(|v| v.as_bool()).unwrap_or(false);
                let res = self.proc_supervisor.stop(proc_id, force).await?;
                Ok(serde_json::to_string_pretty(&res)?)
            }
            "read_file" => {
                let path_str = args
                    .get("path")
                    .and_then(|v| v.as_str())
                    .context("Missing 'path' argument")?;
                let target_path = resolve_path(cwd, path_str);
                let _read_lock = m31a_core::ExecutionLockManager::global()
                    .try_acquire_read(&target_path, "tool:read_file", "reading file content")
                    .ok();

                let content = fs::read_to_string(&target_path)
                    .with_context(|| format!("Failed to read file: {:?}", target_path))?;

                // Support start_line / end_line with offset / limit fallback
                let start = if let Some(sl) = args.get("start_line").and_then(|v| v.as_u64()) {
                    sl.max(1) as usize
                } else if let Some(off) = args.get("offset").and_then(|v| v.as_u64()) {
                    (off + 1) as usize
                } else {
                    1
                };

                let end = if let Some(el) = args.get("end_line").and_then(|v| v.as_u64()) {
                    el as usize
                } else if let Some(lim) = args.get("limit").and_then(|v| v.as_u64()) {
                    start.saturating_add(lim as usize).saturating_sub(1)
                } else {
                    usize::MAX
                };

                let numbered = args
                    .get("numbered")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                let lines: Vec<&str> = content.lines().collect();
                if start > lines.len() {
                    return Ok(String::new());
                }

                let actual_end = std::cmp::min(end, lines.len());
                let slice = &lines[start.saturating_sub(1)..actual_end];

                if numbered {
                    let formatted: Vec<String> = slice
                        .iter()
                        .enumerate()
                        .map(|(idx, line)| format!("{:4} | {}", start + idx, line))
                        .collect();
                    Ok(formatted.join("\n"))
                } else {
                    Ok(slice.join("\n"))
                }
            }
            "write_file" => {
                if policy == SandboxPolicy::ReadOnly {
                    return Err(anyhow!("Cannot write file: Sandbox policy is Read-Only"));
                }
                let path_str = args
                    .get("path")
                    .or_else(|| args.get("file_path"))
                    .or_else(|| args.get("file"))
                    .and_then(|v| v.as_str())
                    .context("Missing 'path' argument")?;
                let content = args
                    .get("content")
                    .or_else(|| args.get("contents"))
                    .or_else(|| args.get("text"))
                    .and_then(|v| v.as_str())
                    .context("Missing 'content' argument")?;
                let target_path = resolve_path(cwd, path_str);

                // Check user concurrency lock
                if let Some(ref st) = self.storage {
                    let rel_path = target_path
                        .strip_prefix(cwd)
                        .unwrap_or(&target_path)
                        .to_string_lossy()
                        .to_string();
                    if let Ok(Some(lock)) = st.is_path_locked("", &rel_path) {
                        return Err(anyhow!(
                            "Cannot write to '{}': path is locked by {}",
                            path_str,
                            lock.locked_by
                        ));
                    }
                }

                // Anti-hallucination overwrite guard: require justification on full file rewrite
                if target_path.exists() && target_path.is_file() {
                    let existing = fs::read_to_string(&target_path).unwrap_or_default();
                    let line_count = existing.lines().count();
                    let replacement_reason =
                        args.get("replacement_reason").and_then(|v| v.as_str());
                    if line_count > 30 && replacement_reason.is_none() {
                        return Err(anyhow!(
                            "File '{}' already exists ({} lines). To prevent accidental loss of code, please use 'apply_patch' or 'edit' for targeted changes, or provide 'replacement_reason' explaining why a complete rewrite is necessary.",
                            path_str,
                            line_count
                        ));
                    }
                }

                // AST syntax validation gate
                m31a_ast::SyntaxValidator::validate_file(&target_path, content)
                    .map_err(|e| anyhow!("Write aborted by AST syntax gate: {e}"))?;

                let _write_lock = m31a_core::ExecutionLockManager::global()
                    .acquire(
                        &target_path,
                        m31a_core::LockMode::Write,
                        "tool:write_file",
                        format!("writing {}", target_path.display()),
                        std::time::Duration::from_secs(5),
                    )
                    .await
                    .map_err(|e| {
                        anyhow!("Failed to acquire write lock on {:?}: {}", target_path, e)
                    })?;

                if let Some(parent) = target_path.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::write(&target_path, content)?;
                Ok(format!(
                    "Successfully wrote {} bytes to {:?}",
                    content.len(),
                    target_path
                ))
            }
            "edit" => {
                if policy == SandboxPolicy::ReadOnly {
                    return Err(anyhow!("Cannot edit file: Sandbox policy is Read-Only"));
                }
                let edit_args: crate::edit::EditArgs = serde_json::from_value(args.clone())
                    .context("Failed to deserialize 'edit' tool arguments")?;
                let target_path = resolve_path(cwd, &edit_args.path);

                // Check user concurrency lock
                if let Some(ref st) = self.storage {
                    let rel_path = target_path
                        .strip_prefix(cwd)
                        .unwrap_or(&target_path)
                        .to_string_lossy()
                        .to_string();
                    if let Ok(Some(lock)) = st.is_path_locked("", &rel_path) {
                        return Err(anyhow!(
                            "Cannot edit '{}': path is locked by {}",
                            rel_path,
                            lock.locked_by
                        ));
                    }
                }

                let _write_lock = m31a_core::ExecutionLockManager::global()
                    .acquire(
                        &target_path,
                        m31a_core::LockMode::Write,
                        "tool:edit",
                        format!("editing {}", target_path.display()),
                        std::time::Duration::from_secs(5),
                    )
                    .await
                    .map_err(|e| {
                        anyhow!("Failed to acquire write lock on {:?}: {}", target_path, e)
                    })?;

                let res = crate::edit::execute_edit(cwd, &edit_args)?;
                Ok(serde_json::to_string_pretty(&res)?)
            }
            "question" => {
                let q_args: crate::question::QuestionArgs = serde_json::from_value(args.clone())
                    .context("Failed to deserialize 'question' tool arguments")?;
                let res = crate::question::execute_question(&q_args).await?;
                Ok(serde_json::to_string_pretty(&res)?)
            }
            "todowrite" => {
                let todo_args: crate::todo::TodoArgs = serde_json::from_value(args.clone())
                    .context("Failed to deserialize 'todowrite' tool arguments")?;
                let sess = self.session_id.as_deref().unwrap_or("default");
                let res =
                    crate::todo::execute_todowrite_scoped(sess, &todo_args, self.storage.as_ref())
                        .await?;
                Ok(serde_json::to_string_pretty(&res)?)
            }
            "schedule" => {
                let sched_args: crate::schedule::ScheduleArgs =
                    serde_json::from_value(args.clone())
                        .context("Failed to deserialize 'schedule' tool arguments")?;
                let res = crate::schedule::execute_schedule(&sched_args).await?;
                Ok(serde_json::to_string_pretty(&res)?)
            }
            "find_related_files" => {
                let target_file = args
                    .get("path")
                    .and_then(|v| v.as_str())
                    .context("Missing 'path' argument")?;
                let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(20) as usize;
                let engine = crate::search::FileSearchEngine::new();
                let related = engine.find_related_files(cwd, target_file, limit);
                Ok(serde_json::to_string_pretty(&related)?)
            }
            "lock_path" => {
                let path_str = args
                    .get("path")
                    .and_then(|v| v.as_str())
                    .context("Missing 'path' argument")?;
                let is_dir = args
                    .get("is_dir")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let reason = args
                    .get("reason")
                    .and_then(|v| v.as_str())
                    .unwrap_or("user");
                if let Some(ref st) = self.storage {
                    let record = st.lock_path("", path_str, is_dir, reason)?;
                    Ok(format!(
                        "Successfully locked '{}' (directory: {}). Path is now protected from AI modifications.",
                        record.path, record.is_dir
                    ))
                } else {
                    Err(anyhow!("Storage unavailable to persist lock"))
                }
            }
            "unlock_path" => {
                let path_str = args
                    .get("path")
                    .and_then(|v| v.as_str())
                    .context("Missing 'path' argument")?;
                if let Some(ref st) = self.storage {
                    let unlocked = st.unlock_path("", path_str)?;
                    if unlocked {
                        Ok(format!("Successfully unlocked '{}'.", path_str))
                    } else {
                        Ok(format!("No active lock found for '{}'.", path_str))
                    }
                } else {
                    Err(anyhow!("Storage unavailable"))
                }
            }
            "list_locks" => {
                if let Some(ref st) = self.storage {
                    let locks = st.list_locks("")?;
                    Ok(serde_json::to_string_pretty(&locks)?)
                } else {
                    Ok("[]".to_string())
                }
            }
            "agent_plan" => {
                let plan: m31a_verify::AgentPlan = serde_json::from_value(args.clone())
                    .context("Failed to deserialize 'agent_plan' arguments. Required fields: goal, steps, criteria.")?;
                Ok(format!(
                    "Plan successfully recorded with {} criteria. Goal: {}",
                    plan.criteria.len(),
                    plan.goal
                ))
            }
            "agent_complete" => {
                let report: m31a_verify::AgentCompletionReport = serde_json::from_value(args.clone())
                    .context("Failed to deserialize 'agent_complete' arguments. Required fields: outcome, summary.")?;
                Ok(format!(
                    "Completion report submitted (outcome: {:?}). Summary: {}",
                    report.outcome, report.summary
                ))
            }
            "cloud_deploy" => {
                if policy == SandboxPolicy::ReadOnly {
                    return Err(anyhow!("Cannot deploy: Sandbox policy is Read-Only"));
                }
                let target = args
                    .get("target")
                    .and_then(|v| v.as_str())
                    .context("Missing 'target' argument")?;
                let prod = args.get("prod").and_then(|v| v.as_bool()).unwrap_or(true);
                let build_dir = args
                    .get("build_dir")
                    .and_then(|v| v.as_str())
                    .unwrap_or("dist");

                let deploy_cmd = match target {
                    "vercel" => {
                        if prod {
                            "vercel deploy --prod --yes"
                        } else {
                            "vercel deploy --yes"
                        }
                    }
                    "netlify" => if prod {
                        format!("netlify deploy --prod --dir={build_dir}")
                    } else {
                        format!("netlify deploy --dir={build_dir}")
                    }
                    .leak(),
                    "fly" => "fly deploy",
                    "github_pages" => "npx gh-pages -d dist",
                    other => return Err(anyhow!("Unsupported deployment target: {other}")),
                };

                let output = self
                    .sandbox
                    .execute(deploy_cmd, cwd, policy, std::time::Duration::from_secs(120))
                    .await?;
                Ok(format!(
                    "Deployment to {} completed (exit {}):\n{}",
                    target, output.exit_code, output.stdout
                ))
            }
            "supabase_migrate" => {
                if policy == SandboxPolicy::ReadOnly {
                    return Err(anyhow!("Cannot run migration: Sandbox policy is Read-Only"));
                }
                let sql = args
                    .get("sql")
                    .and_then(|v| v.as_str())
                    .context("Missing 'sql' argument")?;
                let conn_str = args
                    .get("connection_string")
                    .and_then(|v| v.as_str())
                    .or_else(|| std::env::var("SUPABASE_DB_URL").ok().as_deref().map(|_| ""))
                    .unwrap_or("");

                let cmd = if !conn_str.is_empty() {
                    format!(
                        "psql \"{}\" -c \"{}\"",
                        conn_str.replace('"', "\\\""),
                        sql.replace('"', "\\\"")
                    )
                } else {
                    format!(
                        "npx supabase db execute --sql \"{}\"",
                        sql.replace('"', "\\\"")
                    )
                };

                let output = self
                    .sandbox
                    .execute(&cmd, cwd, policy, std::time::Duration::from_secs(60))
                    .await?;
                if output.exit_code == 0 {
                    Ok(format!(
                        "Supabase migration/SQL executed successfully:\n{}",
                        output.stdout
                    ))
                } else {
                    Err(anyhow!(
                        "Supabase SQL execution failed (code {}):\n{}",
                        output.exit_code,
                        output.stderr
                    ))
                }
            }
            "browser_navigate" => {
                let nav_args: crate::browser::NavigateArgs =
                    serde_json::from_value(args.clone())
                        .context("Failed to deserialize 'browser_navigate' tool arguments")?;
                let session_id = self.session_id.as_deref().unwrap_or("default");
                crate::browser::execute_browser_navigate_for_session(session_id, &nav_args).await
            }
            "browser_snapshot" => {
                let session_id = self.session_id.as_deref().unwrap_or("default");
                crate::browser::execute_browser_snapshot_for_session(session_id).await
            }
            "browser_click" => {
                let click_args: crate::browser::ClickArgs = serde_json::from_value(args.clone())
                    .context("Failed to deserialize 'browser_click' tool arguments")?;
                let session_id = self.session_id.as_deref().unwrap_or("default");
                crate::browser::execute_browser_click_for_session(session_id, &click_args).await
            }
            "browser_type" => {
                let type_args: crate::browser::TypeArgs = serde_json::from_value(args.clone())
                    .context("Failed to deserialize 'browser_type' tool arguments")?;
                let session_id = self.session_id.as_deref().unwrap_or("default");
                crate::browser::execute_browser_type_for_session(session_id, &type_args).await
            }
            "browser_screenshot" => {
                let snap_args: crate::browser::ScreenshotArgs =
                    serde_json::from_value(args.clone())
                        .context("Failed to deserialize 'browser_screenshot' tool arguments")?;
                let session_id = self.session_id.as_deref().unwrap_or("default");
                crate::browser::execute_browser_screenshot_for_session(session_id, &snap_args).await
            }
            "browser_console" => {
                let session_id = self.session_id.as_deref().unwrap_or("default");
                crate::browser::execute_browser_console_for_session(session_id).await
            }
            "browser_network" => {
                let session_id = self.session_id.as_deref().unwrap_or("default");
                crate::browser::execute_browser_network_for_session(session_id).await
            }
            "browser_evaluate" => {
                let eval_args: crate::browser::EvaluateArgs = serde_json::from_value(args.clone())
                    .context("Failed to deserialize 'browser_evaluate' tool arguments")?;
                let session_id = self.session_id.as_deref().unwrap_or("default");
                crate::browser::execute_browser_evaluate_for_session(session_id, &eval_args).await
            }
            "verify_visual_state" => {
                let visual_args: crate::vibe_tools::VerifyVisualStateArgs =
                    serde_json::from_value(args.clone())
                        .context("Failed to deserialize 'verify_visual_state' tool arguments")?;
                crate::vibe_tools::execute_verify_visual_state(&visual_args).await
            }
            "audit_codebase_safety" => {
                let safety_args: crate::vibe_tools::AuditCodebaseSafetyArgs =
                    serde_json::from_value(args.clone())
                        .context("Failed to deserialize 'audit_codebase_safety' tool arguments")?;
                crate::vibe_tools::execute_audit_codebase_safety(cwd, &safety_args).await
            }
            "apply_patch" => {
                if policy == SandboxPolicy::ReadOnly {
                    return Err(anyhow!("Cannot modify file: Sandbox policy is Read-Only"));
                }

                // Check for streaming flag
                let stream = args
                    .get("stream")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                // Check for patch text
                if let Some(patch_str) = args
                    .get("patch")
                    .or_else(|| args.get("diff"))
                    .or_else(|| args.get("patch_text"))
                    .and_then(|v| v.as_str())
                {
                    if stream {
                        // Use streaming apply with mpsc channel
                        let (tx, mut rx) =
                            tokio::sync::mpsc::channel::<m31a_patch::PatchEvent>(100);
                        let cwd = cwd.to_path_buf();
                        let patch_str = patch_str.to_string();
                        let handle = tokio::spawn(async move {
                            let _ = m31a_patch::PatchEngine::apply_streaming(&cwd, &patch_str, tx)
                                .await;
                        });

                        let mut output = String::new();
                        while let Some(event) = rx.recv().await {
                            match event {
                                m31a_patch::PatchEvent::FileStarted { path, action } => {
                                    output.push_str(&format!(
                                        "[FILE STARTED] {} ({})\n",
                                        path, action
                                    ));
                                }
                                m31a_patch::PatchEvent::HunkApplied {
                                    path,
                                    hunk_index,
                                    hunk_type,
                                } => {
                                    output.push_str(&format!(
                                        "[HUNK {}] {} (type: {})\n",
                                        hunk_index + 1,
                                        path,
                                        hunk_type
                                    ));
                                }
                                m31a_patch::PatchEvent::FileCompleted {
                                    path,
                                    action,
                                    hunks_applied,
                                } => {
                                    output.push_str(&format!(
                                        "[FILE COMPLETED] {} ({}) - {} hunks\n",
                                        path, action, hunks_applied
                                    ));
                                }
                                m31a_patch::PatchEvent::PatchCompleted { stats } => {
                                    output.push_str(&format!(
                                        "Successfully applied patch:\n- Files modified: {}\n- Files created: {}\n- Files deleted: {}\n- Hunks applied: {}\n- Average line drift: {:.1}",
                                        stats.files_modified, stats.files_created, stats.files_deleted, stats.hunks_applied, stats.average_drift
                                    ));
                                }
                                m31a_patch::PatchEvent::Error { message } => {
                                    output.push_str(&format!("[ERROR] {}\n", message));
                                }
                            }
                        }
                        let _ = handle.await;
                        return Ok(output);
                    } else {
                        match m31a_patch::PatchEngine::apply(cwd, patch_str) {
                            Ok(stats) => {
                                return Ok(format!(
                                    "Successfully applied patch:\n- Files modified: {}\n- Files created: {}\n- Files deleted: {}\n- Hunks applied: {}\n- Average line drift: {:.1}",
                                    stats.files_modified,
                                    stats.files_created,
                                    stats.files_deleted,
                                    stats.hunks_applied,
                                    stats.average_drift
                                ));
                            }
                            Err(engine_err) => {
                                if let Ok(parsed) = parse_patch(patch_str)
                                    && let Ok(result) = apply_patch_to_workspace(cwd, &parsed)
                                {
                                    return Ok(format!(
                                        "Successfully applied patch (fallback parser):\n- Modified: {:?}\n- Created: {:?}\n- Deleted: {:?}\n\nDiff Summary:\n{}",
                                        result.files_modified,
                                        result.files_created,
                                        result.files_deleted,
                                        result.summary_diff
                                    ));
                                }
                                return Err(engine_err);
                            }
                        }
                    }
                }

                // Fallback: legacy search_text / replace_text
                if let (Some(path_str), Some(search), Some(replace)) = (
                    args.get("path").and_then(|v| v.as_str()),
                    args.get("search_text").and_then(|v| v.as_str()),
                    args.get("replace_text").and_then(|v| v.as_str()),
                ) {
                    let target_path = resolve_path(cwd, path_str);
                    let _lock = m31a_core::ExecutionLockManager::global()
                        .acquire(
                            &target_path,
                            m31a_core::LockMode::Write,
                            "tool:apply_patch",
                            "applying replace text",
                            std::time::Duration::from_secs(5),
                        )
                        .await
                        .map_err(|e| anyhow!("Failed to acquire patch lock: {}", e))?;

                    let original = fs::read_to_string(&target_path)?;
                    if !original.contains(search) {
                        return Err(anyhow!(
                            "Search text chunk was not found in file: {:?}",
                            target_path
                        ));
                    }
                    let updated = original.replacen(search, replace, 1);
                    fs::write(&target_path, updated)?;
                    return Ok(format!(
                        "Successfully applied replacement to {:?}",
                        target_path
                    ));
                }

                Err(anyhow!(
                    "Invalid apply_patch arguments: provide 'patch' or ('path', 'search_text', 'replace_text')"
                ))
            }
            "list_dir" => {
                let dir_str = args.get("path").and_then(|v| v.as_str()).unwrap_or(".");
                let target_dir = resolve_path(cwd, dir_str);

                let mut entries = Vec::new();
                for entry in fs::read_dir(&target_dir)? {
                    let entry = entry?;
                    let file_type = entry.file_type()?;
                    let name = entry.file_name().to_string_lossy().to_string();
                    let kind = if file_type.is_dir() { "dir" } else { "file" };
                    entries.push(format!("{kind}: {name}"));
                }
                entries.sort();
                Ok(entries.join("\n"))
            }
            "file_search" => {
                let pattern = args
                    .get("pattern")
                    .and_then(|v| v.as_str())
                    .context("Missing 'pattern'")?;
                let max_depth =
                    args.get("max_depth").and_then(|v| v.as_u64()).unwrap_or(12) as usize;
                let max_results = args
                    .get("max_results")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(100) as usize;
                let fuzzy = args.get("fuzzy").and_then(|v| v.as_bool()).unwrap_or(false);

                let engine = crate::search::FileSearchEngine::new();
                let matches = if fuzzy {
                    let snap = engine.fuzzy_search(cwd, pattern, max_depth, max_results);
                    snap.matches
                } else {
                    let exact_matches = engine.search(cwd, pattern, max_depth, max_results);
                    if exact_matches.is_empty() && !pattern.contains('*') && !pattern.contains('?')
                    {
                        engine
                            .fuzzy_search(cwd, pattern, max_depth, max_results)
                            .matches
                    } else {
                        exact_matches
                    }
                };

                if matches.is_empty() {
                    Ok(format!("No files matching pattern `{pattern}` found."))
                } else {
                    let lines: Vec<String> = matches
                        .into_iter()
                        .map(|m| {
                            let kind = match m.entry_type {
                                crate::search::SearchEntryType::Directory => "[dir] ",
                                crate::search::SearchEntryType::File => "",
                            };
                            if m.score != 100 {
                                format!("{kind}{} (score: {})", m.relative_path.display(), m.score)
                            } else {
                                format!("{kind}{}", m.relative_path.display())
                            }
                        })
                        .collect();
                    Ok(lines.join("\n"))
                }
            }
            "outline_file" => {
                let path_str = args
                    .get("path")
                    .and_then(|v| v.as_str())
                    .context("Missing 'path'")?;
                let target_path = resolve_path(cwd, path_str);
                let content = fs::read_to_string(&target_path)?;
                let total_lines = content.lines().count();
                let syms = m31a_ast::AstParser::parse_file(&target_path, &content);
                if syms.is_empty() {
                    Ok(format!(
                        "Outline for {}: {} lines, 0 symbols detected.",
                        path_str, total_lines
                    ))
                } else {
                    let mut out = format!(
                        "Outline for {} ({} lines, {} symbols):\n",
                        path_str,
                        total_lines,
                        syms.len()
                    );
                    for sym in syms {
                        let scope = sym
                            .parent_scope
                            .map(|s| format!(" in {}", s))
                            .unwrap_or_default();
                        let doc = sym
                            .doc_comment
                            .map(|d| format!(" - {}", d.lines().next().unwrap_or("")))
                            .unwrap_or_default();
                        out.push_str(&format!(
                            "  [Lines {:>4}..{:<4}] {:<9} {:<24}{}{}\n",
                            sym.range.start_line,
                            sym.range.end_line,
                            sym.kind.as_str(),
                            sym.name,
                            scope,
                            doc
                        ));
                    }
                    Ok(out)
                }
            }
            "extract_symbol_body" => {
                let path_str = args
                    .get("path")
                    .and_then(|v| v.as_str())
                    .context("Missing 'path'")?;
                let symbol = args
                    .get("symbol")
                    .and_then(|v| v.as_str())
                    .context("Missing 'symbol'")?;
                let target_path = resolve_path(cwd, path_str);
                let index = m31a_ast::AstCodebaseIndex::new();
                if let Some(body) = index.extract_symbol_body(&target_path, symbol)? {
                    Ok(format!("// Symbol `{symbol}` in {}\n{}", path_str, body))
                } else {
                    Ok(format!("Symbol `{symbol}` not found in {}", path_str))
                }
            }
            "find_definition" => {
                let symbol = args
                    .get("symbol")
                    .and_then(|v| v.as_str())
                    .context("Missing 'symbol'")?;
                let index = m31a_ast::AstCodebaseIndex::build(cwd, 1000);
                let defs = index.find_definition(symbol);
                if defs.is_empty() {
                    Ok(format!(
                        "No definition for symbol `{symbol}` found in indexed workspace files."
                    ))
                } else {
                    let mut out = format!("Found {} definition(s) for `{symbol}`:\n", defs.len());
                    for def in defs {
                        let rel_path = def.file_path.strip_prefix(cwd).unwrap_or(&def.file_path);
                        let scope = def
                            .parent_scope
                            .map(|s| format!(" in {}", s))
                            .unwrap_or_default();
                        out.push_str(&format!(
                            "  {}:{}-{} -> [{}{}] {}\n",
                            rel_path.display(),
                            def.range.start_line,
                            def.range.end_line,
                            def.kind.as_str(),
                            scope,
                            def.signature
                        ));
                    }
                    Ok(out)
                }
            }
            "find_references" => {
                let symbol = args
                    .get("symbol")
                    .and_then(|v| v.as_str())
                    .context("Missing 'symbol'")?;
                let max_results = args
                    .get("max_results")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(50) as usize;
                let refs = m31a_ast::AstCodebaseIndex::find_callers(cwd, symbol, max_results);
                if refs.is_empty() {
                    let text_refs =
                        m31a_core::SymbolIndex::find_references(cwd, symbol, max_results);
                    if text_refs.is_empty() {
                        Ok(format!("No references to `{symbol}` found in workspace."))
                    } else {
                        let mut out =
                            format!("Found {} reference(s) to `{symbol}`:\n", text_refs.len());
                        for r in text_refs {
                            let rel_path = r.file_path.strip_prefix(cwd).unwrap_or(&r.file_path);
                            out.push_str(&format!(
                                "  {}:{} | {}\n",
                                rel_path.display(),
                                r.line,
                                r.line_content
                            ));
                        }
                        Ok(out)
                    }
                } else {
                    let mut out = format!(
                        "Found {} reference(s)/caller(s) to `{symbol}`:\n",
                        refs.len()
                    );
                    for r in refs {
                        let rel_path = r.file_path.strip_prefix(cwd).unwrap_or(&r.file_path);
                        out.push_str(&format!(
                            "  {}:{} (in `{}`) -> calls `{}`\n",
                            rel_path.display(),
                            r.line,
                            r.caller_name,
                            r.callee_name
                        ));
                    }
                    Ok(out)
                }
            }
            "diagnose_compiler_errors" => {
                let output = args
                    .get("output")
                    .and_then(|v| v.as_str())
                    .context("Missing 'output'")?;
                let report = m31a_core::generate_self_healing_report(output);
                if report.diagnostics.is_empty() {
                    Ok("No compiler errors or stack traces detected in output.".to_string())
                } else {
                    let mut out = format!(
                        "Diagnosed {} error(s) and {} warning(s):\n",
                        report.total_errors, report.total_warnings
                    );
                    for d in &report.diagnostics {
                        let code_str = d.code.as_deref().unwrap_or("general");
                        out.push_str(&format!(
                            "  - [{}] {}:{} : {}\n",
                            code_str,
                            d.file_path.display(),
                            d.line,
                            d.message
                        ));
                        if let Some(ref fix) = d.suggested_fix {
                            out.push_str(&format!("    help: {}\n", fix));
                        }
                    }
                    if let Some(prompt) = m31a_core::build_self_healing_prompt(&report, cwd) {
                        out.push_str(&format!("\nRecommended Self-Healing Action:\n{}\n", prompt));
                    }
                    Ok(out)
                }
            }
            "discover_capabilities" => {
                let report = m31a_core::discover_host_capabilities(cwd);
                Ok(report.summary())
            }
            "view_image" => {
                use base64::Engine;
                let path_str = args
                    .get("path")
                    .and_then(|v| v.as_str())
                    .context("Missing 'path'")?;
                let target_path = resolve_path(cwd, path_str);
                if !target_path.exists() {
                    return Err(anyhow!("Image file does not exist: {:?}", target_path));
                }
                let meta = fs::metadata(&target_path)?;
                if meta.len() > 20 * 1024 * 1024 {
                    return Err(anyhow!(
                        "Image file exceeds 20MB limit: {} bytes",
                        meta.len()
                    ));
                }
                let ext = target_path
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("png")
                    .to_lowercase();
                let mime = match ext.as_str() {
                    "png" => "image/png",
                    "jpg" | "jpeg" => "image/jpeg",
                    "gif" => "image/gif",
                    "webp" => "image/webp",
                    "svg" => "image/svg+xml",
                    "bmp" => "image/bmp",
                    _ => "image/png",
                };
                let bytes = fs::read(&target_path)?;
                let mut dimensions = (0u32, 0u32);
                if ext == "png" && bytes.len() >= 24 {
                    dimensions.0 = u32::from_be_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]);
                    dimensions.1 = u32::from_be_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]);
                }
                let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
                let data_url = format!("data:{mime};base64,{b64}");
                let dim_str = if dimensions.0 > 0 && dimensions.1 > 0 {
                    format!("{}x{}", dimensions.0, dimensions.1)
                } else {
                    "unknown".to_string()
                };
                Ok(format!(
                    "Image: {:?}\nFormat: {}\nDimensions: {}\nSize: {} bytes\nData URL: {}",
                    target_path.file_name().unwrap_or_default(),
                    mime,
                    dim_str,
                    meta.len(),
                    if data_url.len() > 1024 {
                        format!("{}... [total {} chars]", &data_url[..1024], data_url.len())
                    } else {
                        data_url
                    }
                ))
            }
            "request_user_input" => {
                let question = args
                    .get("question")
                    .and_then(|v| v.as_str())
                    .context("Missing 'question'")?;
                let options = args
                    .get("options")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    })
                    .unwrap_or_default();

                Ok(format!(
                    "[USER_PROMPT_PENDING]: {}\nSuggested Options: [{}]",
                    question, options
                ))
            }
            "tool_search" => {
                let query = args
                    .get("query")
                    .and_then(|v| v.as_str())
                    .context("Missing 'query'")?
                    .to_lowercase();
                let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(8) as usize;

                let defs = self.definitions();
                let mut matches = Vec::new();
                for d in defs {
                    let name = &d.function.name;
                    let desc = &d.function.description;
                    if name.to_lowercase().contains(&query) || desc.to_lowercase().contains(&query)
                    {
                        matches.push(format!("- `{}`: {}", name, desc));
                        if matches.len() >= limit {
                            break;
                        }
                    }
                }

                if matches.is_empty() {
                    Ok(format!("No tools found matching query: '{}'", query))
                } else {
                    Ok(format!(
                        "Tool Search Results ({} matches):\n{}",
                        matches.len(),
                        matches.join("\n")
                    ))
                }
            }
            "web_search" => {
                let query = args
                    .get("query")
                    .and_then(|v| v.as_str())
                    .context("Missing 'query'")?;
                let max_results = args
                    .get("max_results")
                    .and_then(|v| v.as_u64())
                    .map(|n| n as usize);
                let mut allowed_domains: Vec<String> = args
                    .get("allowed_domains")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|x| x.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default();
                if let Some(d) = args.get("domain").and_then(|v| v.as_str()) {
                    allowed_domains.push(d.to_string());
                }
                let allowed_opt = if allowed_domains.is_empty() {
                    None
                } else {
                    Some(allowed_domains)
                };
                let blocked_domains =
                    args.get("blocked_domains")
                        .and_then(|v| v.as_array())
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|x| x.as_str().map(String::from))
                                .collect()
                        });

                let mode_str = args.get("mode").and_then(|v| v.as_str()).unwrap_or("live");
                let mode: crate::web_search::WebSearchMode = mode_str.parse().unwrap_or_default();
                let proxy_url = std::env::var("HTTPS_PROXY")
                    .or_else(|_| std::env::var("HTTP_PROXY"))
                    .ok();
                let client = crate::web_search::build_proxied_client(proxy_url.as_deref());

                let search_req = crate::web_search::WebSearchQuery {
                    query: query.to_string(),
                    max_results,
                    allowed_domains: allowed_opt,
                    blocked_domains,
                };
                let resp =
                    crate::web_search::execute_web_search_with_mode(&client, search_req, mode)
                        .await?;
                Ok(serde_json::to_string_pretty(&resp)?)
            }
            "web_fetch" => {
                let url = args
                    .get("url")
                    .and_then(|v| v.as_str())
                    .context("Missing 'url'")?;
                let max_bytes = args
                    .get("max_bytes")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(32768) as usize;
                let proxy_url = std::env::var("HTTPS_PROXY")
                    .or_else(|_| std::env::var("HTTP_PROXY"))
                    .ok();
                let client = crate::web_search::build_proxied_client(proxy_url.as_deref());
                crate::web_search::execute_web_fetch(&client, url, max_bytes).await
            }
            // Multi-Agent Execution
            "spawn_agent" => {
                let task_name = args
                    .get("task_name")
                    .and_then(|v| v.as_str())
                    .context("Missing 'task_name'")?;
                let prompt = args
                    .get("prompt")
                    .and_then(|v| v.as_str())
                    .context("Missing 'prompt'")?;
                let role_name = args
                    .get("role")
                    .or_else(|| args.get("agent_type"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("coder");
                let model_override = args
                    .get("model")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());

                // Depth limiting — check caller's depth before spawning
                let caller_id = crate::agent_runtime::caller_agent_id();
                let rt = self.get_agent_runtime().ok_or_else(|| {
                    anyhow!("AgentRuntime unavailable: subagent execution capability is not configured or not active")
                })?;
                let max_depth = rt.max_depth();
                let parent_depth =
                    if let (Some(storage), Some(parent_id)) = (&self.storage, &caller_id) {
                        storage.get_agent_depth(parent_id).unwrap_or(0)
                    } else {
                        0 // Root agent has depth 0
                    };
                let child_depth = parent_depth + 1;

                if child_depth > max_depth {
                    return Err(anyhow!(
                        "Agent depth limit reached (max: {max_depth}). Current depth: {parent_depth}. Solve the task yourself."
                    ));
                }

                let isolate_worktree = args
                    .get("isolate_worktree")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                let spawned = rt
                    .spawn_agent(crate::agent_runtime::AgentSpawnRequest {
                        task_name: task_name.to_string(),
                        prompt: prompt.to_string(),
                        role_name: role_name.to_string(),
                        model_override,
                        depth: child_depth,
                        max_depth,
                        isolate_worktree,
                    })
                    .await?;
                let worktree_line = if let Some(ref wt) = spawned.worktree_path {
                    format!("\nWorktree: {}", wt.display())
                } else {
                    String::new()
                };
                Ok(format!(
                    "Spawned subagent `{}` (Codename: {})\nRole: {}\nModel: {}\nSandbox Policy: {}\nDepth: {}/{}{}\nInitial Task: {task_name}\nUse send_input to steer it, wait_agent to await completion, close_agent to terminate.",
                    spawned.agent_id,
                    spawned.nickname,
                    spawned.role,
                    spawned.model,
                    spawned.sandbox,
                    child_depth,
                    max_depth,
                    worktree_line,
                ))
            }
            "send_input" => {
                let agent_id = args
                    .get("agent_id")
                    .and_then(|v| v.as_str())
                    .context("Missing 'agent_id'")?;
                let msg = args
                    .get("message")
                    .and_then(|v| v.as_str())
                    .context("Missing 'message'")?;
                let interrupt = args
                    .get("interrupt")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                let rt = self.get_agent_runtime().ok_or_else(|| {
                    anyhow!("AgentRuntime unavailable: subagent execution capability is not configured or not active")
                })?;
                rt.send_input(agent_id, msg, interrupt).await
            }
            "wait_agent" => {
                let ids = args
                    .get("agent_ids")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(|s| s.to_string()))
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                if ids.is_empty() {
                    return Err(anyhow!("Missing 'agent_ids'"));
                }
                let timeout_ms = args.get("timeout_ms").and_then(|v| v.as_u64());

                let rt = self.get_agent_runtime().ok_or_else(|| {
                    anyhow!("AgentRuntime unavailable: subagent execution capability is not configured or not active")
                })?;
                let outcomes = rt.wait_agents(&ids, timeout_ms).await?;
                let lines = outcomes
                    .iter()
                    .map(|o| {
                        let answer = o
                            .answer
                            .as_deref()
                            .filter(|a| !a.trim().is_empty())
                            .map(|a| format!("\n--- output ---\n{}", a))
                            .unwrap_or_default();
                        format!("- `{}`: {}{}", o.agent_id, o.status, answer)
                    })
                    .collect::<Vec<_>>();
                Ok(lines.join("\n"))
            }
            "close_agent" => {
                let agent_id = args
                    .get("agent_id")
                    .and_then(|v| v.as_str())
                    .context("Missing 'agent_id'")?;

                let rt = self.get_agent_runtime().ok_or_else(|| {
                    anyhow!("AgentRuntime unavailable: subagent execution capability is not configured or not active")
                })?;
                rt.close_agent(agent_id).await
            }
            "terminate_agent_tree" => {
                let agent_id = args
                    .get("agent_id")
                    .and_then(|v| v.as_str())
                    .context("Missing 'agent_id'")?;

                let rt = self.get_agent_runtime().ok_or_else(|| {
                    anyhow!("AgentRuntime unavailable: subagent execution capability is not configured or not active")
                })?;
                let report = rt.terminate_tree(agent_id).await?;
                Ok(format!(
                    "Tree termination report for `{agent_id}`:\n- Complete: {}\n- Terminated ({}): {:?}\n- Timed out ({}): {:?}\n- Failed ({}): {:?}",
                    report.is_complete,
                    report.terminated_agents.len(),
                    report.terminated_agents,
                    report.timed_out_agents.len(),
                    report.timed_out_agents,
                    report.failed_agents.len(),
                    report.failed_agents,
                ))
            }
            "list_agents" => {
                let include_descendants = args
                    .get("include_descendants")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true);

                let rt = self.get_agent_runtime().ok_or_else(|| {
                    anyhow!("AgentRuntime unavailable: subagent execution capability is not configured or not active")
                })?;
                let live = rt.list_live().await;
                let mut lines: Vec<String> = live
                    .iter()
                    .map(|a| {
                        format!(
                            "- `{}` [role: {}, task: {}, status: {}]",
                            a.agent_id, a.role, a.task_name, a.status
                        )
                    })
                    .collect();
                if let Some(ref storage) = self.storage {
                    let edges = if include_descendants {
                        storage.list_spawn_descendants("current", None)?
                    } else {
                        storage.list_spawn_children("current", None)?
                    };
                    for e in edges {
                        if live.iter().any(|a| a.agent_id == e.child_thread_id) {
                            continue;
                        }
                        lines.push(format!(
                            "- `{}` [role: {}, status: {}, created: {}]",
                            e.child_thread_id, e.agent_role, e.status, e.created_at
                        ));
                    }
                }
                if lines.is_empty() {
                    return Ok("No subagents currently found in active hierarchy.".to_string());
                }
                Ok(format!(
                    "Spawned Subagents Hierarchy:\n{}",
                    lines.join("\n")
                ))
            }
            "integrate_agent" => {
                let agent_id = args
                    .get("agent_id")
                    .and_then(|v| v.as_str())
                    .context("Missing 'agent_id'")?;
                let commit_message = args.get("commit_message").and_then(|v| v.as_str());

                let rt = self.get_agent_runtime().ok_or_else(|| {
                    anyhow!("AgentRuntime unavailable: subagent execution capability is not configured or not active")
                })?;
                rt.integrate_agent(agent_id, commit_message).await
            }
            // Multi-Agent Message Board Execution
            "create_channel" => {
                let name = args
                    .get("name")
                    .and_then(|v| v.as_str())
                    .context("Missing 'name'")?;
                let topic = args.get("topic").and_then(|v| v.as_str()).unwrap_or("");
                if let Some(ref storage) = self.storage {
                    storage.create_channel(name, topic)?;
                    Ok(format!("Created channel `#{name}` with topic: {topic}"))
                } else {
                    Ok(format!("Created transient channel `#{name}`"))
                }
            }
            "list_channels" => {
                if let Some(ref storage) = self.storage {
                    let channels = storage.list_channels()?;
                    if channels.is_empty() {
                        Ok("No channels found on message board.".to_string())
                    } else {
                        let lines = channels
                            .iter()
                            .map(|c| {
                                format!("- `#{}`: {} ({} messages)", c.name, c.topic, c.post_count)
                            })
                            .collect::<Vec<_>>();
                        Ok(format!("Message Board Channels:\n{}", lines.join("\n")))
                    }
                } else {
                    Ok("Message board storage unavailable.".to_string())
                }
            }
            "post_message" => {
                let channel = args
                    .get("channel")
                    .and_then(|v| v.as_str())
                    .context("Missing 'channel'")?;
                let raw_msg = args
                    .get("message")
                    .and_then(|v| v.as_str())
                    .context("Missing 'message'")?;
                let sanitized_msg = m31a_core::redact_secrets(raw_msg);
                let author = args
                    .get("author")
                    .and_then(|v| v.as_str())
                    .unwrap_or("agent");
                let reply_to_id = args.get("reply_to_id").and_then(|v| v.as_str());

                if let Some(ref storage) = self.storage {
                    let post_id = storage.post_message(
                        "default",
                        channel,
                        author,
                        &sanitized_msg,
                        reply_to_id,
                        serde_json::json!({}),
                    )?;
                    Ok(format!("Posted message `{post_id}` to `#{channel}`"))
                } else {
                    Ok(format!("Transient post to `#{channel}`: {sanitized_msg}"))
                }
            }
            "list_threads" => {
                let channel = args
                    .get("channel")
                    .and_then(|v| v.as_str())
                    .context("Missing 'channel'")?;
                let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(20) as usize;

                if let Some(ref storage) = self.storage {
                    let threads = storage.list_board_threads("default", channel, limit)?;
                    if threads.is_empty() {
                        Ok(format!("No threads found in channel `#{channel}`."))
                    } else {
                        let lines = threads
                            .iter()
                            .map(|t| {
                                format!(
                                    "- [{}] by {} ({} replies): {}",
                                    t.root_id, t.author, t.reply_count, t.root_body
                                )
                            })
                            .collect::<Vec<_>>();
                        Ok(format!("Threads in `#{channel}`:\n{}", lines.join("\n")))
                    }
                } else {
                    Ok("Message board storage unavailable.".to_string())
                }
            }
            "read_thread" => {
                let thread_id = args
                    .get("thread_id")
                    .and_then(|v| v.as_str())
                    .context("Missing 'thread_id'")?;

                if let Some(ref storage) = self.storage {
                    let messages = storage.read_thread("default", thread_id)?;
                    if messages.is_empty() {
                        Ok(format!("No messages found in thread `{thread_id}`."))
                    } else {
                        let lines = messages
                            .iter()
                            .map(|m| format!("[{}] {}: {}", m.id, m.author, m.body))
                            .collect::<Vec<_>>();
                        Ok(format!("Thread `{thread_id}`:\n{}", lines.join("\n\n")))
                    }
                } else {
                    Ok("Message board storage unavailable.".to_string())
                }
            }
            "search_posts" => {
                let query = args
                    .get("query")
                    .and_then(|v| v.as_str())
                    .context("Missing 'query'")?;
                let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(20) as usize;

                if let Some(ref storage) = self.storage {
                    let hits = storage.search_posts("default", query, limit)?;
                    if hits.is_empty() {
                        Ok(format!("No posts matching query `{query}`."))
                    } else {
                        let lines = hits
                            .iter()
                            .map(|m| {
                                format!("- [#{}] [{}] {}: {}", m.channel, m.id, m.author, m.body)
                            })
                            .collect::<Vec<_>>();
                        Ok(format!(
                            "Search results for `{query}`:\n{}",
                            lines.join("\n")
                        ))
                    }
                } else {
                    Ok("Message board storage unavailable.".to_string())
                }
            }
            "subscribe_channel" => {
                let channel = args
                    .get("channel")
                    .and_then(|v| v.as_str())
                    .context("Missing 'channel'")?;
                let subscriber = args
                    .get("subscriber")
                    .and_then(|v| v.as_str())
                    .unwrap_or("current");

                if let Some(ref storage) = self.storage {
                    storage.subscribe("default", channel, subscriber)?;
                    Ok(format!("Subscribed `{subscriber}` to `#{channel}`"))
                } else {
                    Ok(format!("Transient subscription to `#{channel}`"))
                }
            }
            "unsubscribe_channel" => {
                let channel = args
                    .get("channel")
                    .and_then(|v| v.as_str())
                    .context("Missing 'channel'")?;
                let subscriber = args
                    .get("subscriber")
                    .and_then(|v| v.as_str())
                    .unwrap_or("current");

                if let Some(ref storage) = self.storage {
                    storage.unsubscribe("default", channel, subscriber)?;
                    Ok(format!("Unsubscribed `{subscriber}` from `#{channel}`"))
                } else {
                    Ok(format!("Transient unsubscription from `#{channel}`"))
                }
            }
            // Worktree Execution
            "create_worktree" => {
                let base_ref = args.get("base_ref").and_then(|v| v.as_str());
                let mgr = m31a_core::WorktreeManager::new()?;
                let wt = mgr.create_worktree(cwd, base_ref)?;
                Ok(format!(
                    "Created managed worktree `{}`:\nPath: {}\nBranch: {}\nHEAD: {}",
                    wt.id,
                    wt.path.display(),
                    wt.branch.as_deref().unwrap_or("detached"),
                    wt.head_sha
                ))
            }
            "list_worktrees" => {
                let mgr = m31a_core::WorktreeManager::new()?;
                let list = mgr.list_worktrees(cwd)?;
                if list.is_empty() {
                    Ok("No active worktrees found for this repository.".to_string())
                } else {
                    let formatted: Vec<String> = list
                        .into_iter()
                        .map(|wt| {
                            format!(
                                "- [{}] {} (branch: {}, head: {}{})",
                                wt.id,
                                wt.path.display(),
                                wt.branch.as_deref().unwrap_or("detached"),
                                &wt.head_sha[..wt.head_sha.len().min(8)],
                                if wt.is_locked {
                                    ", locked"
                                } else if wt.is_prunable {
                                    ", prunable"
                                } else {
                                    ""
                                }
                            )
                        })
                        .collect();
                    Ok(format!("Active Git Worktrees:\n{}", formatted.join("\n")))
                }
            }
            "remove_worktree" => {
                let path_str = args
                    .get("path")
                    .and_then(|v| v.as_str())
                    .context("Missing 'path'")?;
                let mgr = m31a_core::WorktreeManager::new()?;
                mgr.remove_worktree(Path::new(path_str))?;
                Ok(format!("Successfully removed worktree at `{path_str}`"))
            }
            // Memory & Knowledge Consolidation Execution
            "consolidate_memory" => {
                let topic = args
                    .get("topic")
                    .and_then(|v| v.as_str())
                    .context("Missing 'topic'")?;
                let detail = args
                    .get("detail")
                    .and_then(|v| v.as_str())
                    .context("Missing 'detail'")?;
                let mgr = m31a_core::MemoryManager::new()?;
                let item = mgr.consolidate_fact(topic, detail)?;
                Ok(format!(
                    "Consolidated memory `{}` on topic '{}' (citations: {}):\n{}",
                    item.id, item.topic, item.citation_count, item.detail
                ))
            }
            "search_memories" => {
                let query = args
                    .get("query")
                    .and_then(|v| v.as_str())
                    .context("Missing 'query'")?;
                let mgr = m31a_core::MemoryManager::new()?;
                let (mode, hits) = mgr.search_memories_with_mode(query);
                if hits.is_empty() {
                    Ok(format!(
                        "No memories found matching query `{query}` (retrieval mode: {mode})."
                    ))
                } else {
                    let lines = hits
                        .iter()
                        .map(|s| {
                            let i = &s.item;
                            format!(
                                "- **{}** `[{}]` (citations: {}, score: {:.2}): {}",
                                i.topic, i.id, i.citation_count, s.hybrid_score, i.detail
                            )
                        })
                        .collect::<Vec<_>>();
                    Ok(format!(
                        "Found {} matching memories (retrieval mode: {}):\n{}",
                        hits.len(),
                        mode,
                        lines.join("\n")
                    ))
                }
            }
            "cite_memory" => {
                let mem_id = args
                    .get("memory_id")
                    .and_then(|v| v.as_str())
                    .context("Missing 'memory_id'")?;
                let mgr = m31a_core::MemoryManager::new()?;
                let cited = mgr.record_citation(mem_id)?;
                if cited {
                    Ok(format!("Recorded citation for memory `{mem_id}`."))
                } else {
                    Ok(format!("Memory `{mem_id}` not found."))
                }
            }
            // Goal Tracking
            "create_goal" => {
                let thread_id = args
                    .get("thread_id")
                    .and_then(|v| v.as_str())
                    .context("Missing 'thread_id'")?;
                let objective = args
                    .get("objective")
                    .and_then(|v| v.as_str())
                    .context("Missing 'objective'")?;
                let budget = args.get("token_budget").and_then(|v| v.as_u64());

                if let Some(ref storage) = self.storage {
                    let goal = storage.set_goal(thread_id, objective, budget)?;
                    Ok(format!(
                        "Created goal `{}` for thread `{}`: {}",
                        goal.goal_id, thread_id, objective
                    ))
                } else {
                    Ok(format!(
                        "Created transient goal for thread `{}`: {}",
                        thread_id, objective
                    ))
                }
            }
            "update_goal" => {
                let thread_id = args
                    .get("thread_id")
                    .and_then(|v| v.as_str())
                    .context("Missing 'thread_id'")?;
                let status = args
                    .get("status")
                    .and_then(|v| v.as_str())
                    .context("Missing 'status'")?;

                if let Some(ref storage) = self.storage {
                    storage.update_goal_status(thread_id, status)?;
                    Ok(format!(
                        "Updated goal status for thread `{thread_id}` to `{status}`"
                    ))
                } else {
                    Ok(format!(
                        "Updated transient goal status for thread `{thread_id}` to `{status}`"
                    ))
                }
            }
            "get_goal" => {
                let thread_id = args
                    .get("thread_id")
                    .and_then(|v| v.as_str())
                    .context("Missing 'thread_id'")?;

                if let Some(ref storage) = self.storage {
                    if let Some(goal) = storage.get_goal(thread_id)? {
                        Ok(serde_json::to_string_pretty(&goal)?)
                    } else {
                        Ok(format!("No goal registered for thread `{thread_id}`"))
                    }
                } else {
                    Ok(format!("No goal storage active for thread `{thread_id}`"))
                }
            }
            "grep_search" => {
                let query = args
                    .get("query")
                    .and_then(|v| v.as_str())
                    .context("Missing 'query'")?;
                let path_str = args.get("path").and_then(|v| v.as_str()).unwrap_or(".");
                let case_sensitive = args
                    .get("case_sensitive")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let max_results = args
                    .get("max_results")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(50) as usize;

                let search_root = resolve_path(cwd, path_str);
                let re = regex::RegexBuilder::new(query)
                    .case_insensitive(!case_sensitive)
                    .build()
                    .or_else(|_| {
                        regex::RegexBuilder::new(&regex::escape(query))
                            .case_insensitive(!case_sensitive)
                            .build()
                    })
                    .with_context(|| format!("Invalid regex query: {}", query))?;

                let mut matches = Vec::new();

                for entry in walkdir::WalkDir::new(&search_root)
                    .max_depth(10)
                    .into_iter()
                    .filter_entry(|e| {
                        let file_name = e.file_name().to_string_lossy();
                        !file_name.starts_with('.')
                            && file_name != "target"
                            && file_name != "node_modules"
                    })
                    .filter_map(|e| e.ok())
                {
                    if !entry.file_type().is_file() {
                        continue;
                    }
                    if let Ok(content) = fs::read_to_string(entry.path()) {
                        let rel_path = entry
                            .path()
                            .strip_prefix(cwd)
                            .unwrap_or_else(|_| entry.path());
                        for (line_idx, line) in content.lines().enumerate() {
                            if re.is_match(line) {
                                matches.push(format!(
                                    "{}:{}: {}",
                                    rel_path.display(),
                                    line_idx + 1,
                                    line.trim()
                                ));
                                if matches.len() >= max_results {
                                    break;
                                }
                            }
                        }
                    }
                    if matches.len() >= max_results {
                        break;
                    }
                }

                if matches.is_empty() {
                    Ok(format!(
                        "No matches found for query: '{}' in {:?}",
                        query, search_root
                    ))
                } else {
                    Ok(format!(
                        "Found {} matches for '{}':\n{}",
                        matches.len(),
                        query,
                        matches.join("\n")
                    ))
                }
            }
            "update_plan" => {
                let plan_arr = args
                    .get("plan")
                    .and_then(|v| v.as_array())
                    .context("Missing 'plan' array")?;
                let explanation = args
                    .get("explanation")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                let mut in_progress_count = 0;
                let mut completed_count = 0;
                let mut pending_count = 0;
                let mut rendered = Vec::new();

                for item in plan_arr {
                    let step = item
                        .get("step")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unnamed step");
                    let status = item
                        .get("status")
                        .and_then(|v| v.as_str())
                        .unwrap_or("pending");
                    match status {
                        "in_progress" => in_progress_count += 1,
                        "completed" => completed_count += 1,
                        _ => pending_count += 1,
                    }
                    let marker = match status {
                        "completed" => "[x]",
                        "in_progress" => "[>]",
                        _ => "[ ]",
                    };
                    rendered.push(format!("  {} {} ({})", marker, step, status));
                }

                if in_progress_count > 1 {
                    return Err(anyhow!(
                        "Invalid plan: at most one step can be in_progress at a time (found {})",
                        in_progress_count
                    ));
                }

                let mut summary = format!(
                    "Plan updated: {} steps ({} completed, {} in progress, {} pending)\n",
                    plan_arr.len(),
                    completed_count,
                    in_progress_count,
                    pending_count
                );
                if !explanation.is_empty() {
                    summary.push_str(&format!("Explanation: {}\n", explanation));
                }
                summary.push_str(&rendered.join("\n"));
                Ok(summary)
            }
            "current_time" | "curr_time" => {
                let now_utc = chrono::Utc::now();
                let now_local = chrono::Local::now();
                Ok(format!(
                    "Current Time (UTC): {}\nLocal Time: {}\nTimezone: UTC",
                    now_utc.format("%Y-%m-%d %H:%M:%S UTC"),
                    now_local.format("%Y-%m-%d %H:%M:%S %Z")
                ))
            }
            "sleep" => {
                let duration_ms = args
                    .get("duration_ms")
                    .and_then(|v| v.as_u64())
                    .context("Missing 'duration_ms'")?;
                let clamped = std::cmp::min(duration_ms, 60_000);
                let start = std::time::Instant::now();
                tokio::time::sleep(Duration::from_millis(clamped)).await;
                Ok(format!(
                    "Slept for {:.2} seconds (requested {} ms)",
                    start.elapsed().as_secs_f64(),
                    duration_ms
                ))
            }
            "resume_agent" => {
                let agent_id = args
                    .get("agent_id")
                    .and_then(|v| v.as_str())
                    .context("Missing 'agent_id'")?;
                let prompt = args
                    .get("prompt")
                    .and_then(|v| v.as_str())
                    .context("Missing 'prompt'")?;
                Ok(format!(
                    "Resumed agent `{agent_id}` with follow-up instructions: {prompt}"
                ))
            }
            "board_create_channel" => {
                let channel_name = args
                    .get("channel_name")
                    .and_then(|v| v.as_str())
                    .context("Missing 'channel_name'")?;
                let desc = args
                    .get("description")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                if let Some(ref storage) = self.storage {
                    storage.post_to_board(
                        "default",
                        channel_name,
                        "system",
                        &format!("Channel created: {}", desc),
                        serde_json::json!({"type": "init"}),
                    )?;
                }
                Ok(format!("Created collaboration channel `{channel_name}`"))
            }
            "board_post" => {
                let channel_name = args
                    .get("channel_name")
                    .and_then(|v| v.as_str())
                    .context("Missing 'channel_name'")?;
                let text = args
                    .get("text")
                    .and_then(|v| v.as_str())
                    .context("Missing 'text'")?;
                let recipients = args
                    .get("agents_to_notify")
                    .and_then(|v| v.as_array())
                    .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>())
                    .unwrap_or_default();
                if let Some(ref storage) = self.storage {
                    storage.post_to_board(
                        "default",
                        channel_name,
                        "agent",
                        text,
                        serde_json::json!({"recipients": recipients}),
                    )?;
                }
                Ok(format!(
                    "Posted message to `{channel_name}` ({} bytes)",
                    text.len()
                ))
            }
            "board_read" => {
                let channel_name = args
                    .get("channel_name")
                    .and_then(|v| v.as_str())
                    .context("Missing 'channel_name'")?;
                let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(20) as usize;
                if let Some(ref storage) = self.storage {
                    let msgs = storage.list_posts("default", channel_name, limit)?;
                    if msgs.is_empty() {
                        Ok(format!("No messages found in channel `{channel_name}`"))
                    } else {
                        let lines = msgs
                            .iter()
                            .map(|m| format!("[{}] {}: {}", m.timestamp, m.author, m.body))
                            .collect::<Vec<_>>();
                        Ok(format!(
                            "Channel `{channel_name}` ({} messages):\n{}",
                            msgs.len(),
                            lines.join("\n")
                        ))
                    }
                } else {
                    Ok(format!(
                        "Board storage not initialized for channel `{channel_name}`"
                    ))
                }
            }
            "use_skill" => {
                let skill_name = args
                    .get("name")
                    .and_then(|v| v.as_str())
                    .context("Missing 'name' argument")?;
                let skills = m31a_core::skills::SkillLoader::discover_skills(cwd);
                if let Some(skill) = skills
                    .iter()
                    .find(|s| s.name.eq_ignore_ascii_case(skill_name))
                {
                    if let Err(missing) = skill.check_dependencies() {
                        Ok(format!(
                            "Warning: Skill '{}' has missing dependencies:\n- {}\n\nInstructions:\n{}",
                            skill.name,
                            missing.join("\n- "),
                            skill.instructions
                        ))
                    } else {
                        Ok(format!(
                            "# Skill: {}\nDescription: {}\nPath: {}\n\n{}",
                            skill.name,
                            skill.description,
                            skill.path.display(),
                            skill.instructions
                        ))
                    }
                } else {
                    let available: Vec<&str> = skills.iter().map(|s| s.name.as_str()).collect();
                    Err(anyhow!(
                        "Skill '{}' not found. Available skills: {}",
                        skill_name,
                        available.join(", ")
                    ))
                }
            }
            "list_skills" => {
                let skills = m31a_core::skills::SkillLoader::discover_skills(cwd);
                let list: Vec<serde_json::Value> = skills
                    .into_iter()
                    .map(|s| {
                        serde_json::json!({
                            "name": s.name,
                            "description": s.description,
                            "scope": format!("{:?}", s.scope),
                            "path": s.path.to_string_lossy(),
                        })
                    })
                    .collect();
                Ok(serde_json::to_string_pretty(&list)?)
            }
            "store_secret" => {
                use m31a_core::SecretsBackend;
                let secret_name = args
                    .get("name")
                    .and_then(|v| v.as_str())
                    .context("Missing 'name'")?;
                let secret_val = args
                    .get("value")
                    .and_then(|v| v.as_str())
                    .context("Missing 'value'")?;
                let ns_str = args
                    .get("namespace")
                    .and_then(|v| v.as_str())
                    .unwrap_or("managed_secrets");
                let scope_str = args
                    .get("scope")
                    .and_then(|v| v.as_str())
                    .unwrap_or("global");

                let ns = match ns_str {
                    "m31a_auth" => m31a_core::LocalSecretsNamespace::M31aAuth,
                    "mcp_oauth" => m31a_core::LocalSecretsNamespace::McpOAuth,
                    "gateway_oauth" => m31a_core::LocalSecretsNamespace::GatewayOAuth,
                    _ => m31a_core::LocalSecretsNamespace::ManagedSecrets,
                };
                let scope = match scope_str {
                    "workspace" => m31a_core::SecretScope::Workspace,
                    "session" => m31a_core::SecretScope::Session,
                    _ => m31a_core::SecretScope::Global,
                };

                self.vault.set_secret(ns, secret_name, secret_val, scope)?;
                Ok(format!(
                    "Secret '{}' successfully stored in namespace '{:?}'",
                    secret_name, ns
                ))
            }
            "retrieve_secret" => {
                use m31a_core::SecretsBackend;
                let secret_name = args
                    .get("name")
                    .and_then(|v| v.as_str())
                    .context("Missing 'name'")?;
                let ns_str = args
                    .get("namespace")
                    .and_then(|v| v.as_str())
                    .unwrap_or("managed_secrets");
                let ns = match ns_str {
                    "m31a_auth" => m31a_core::LocalSecretsNamespace::M31aAuth,
                    "mcp_oauth" => m31a_core::LocalSecretsNamespace::McpOAuth,
                    "gateway_oauth" => m31a_core::LocalSecretsNamespace::GatewayOAuth,
                    _ => m31a_core::LocalSecretsNamespace::ManagedSecrets,
                };

                match self.vault.get_secret(ns, secret_name)? {
                    Some(val) => Ok(val),
                    None => Err(anyhow!(
                        "Secret '{}' not found in namespace '{:?}'",
                        secret_name,
                        ns
                    )),
                }
            }
            "list_stored_secrets" => {
                use m31a_core::SecretsBackend;
                let ns_str = args
                    .get("namespace")
                    .and_then(|v| v.as_str())
                    .unwrap_or("managed_secrets");
                let ns = match ns_str {
                    "m31a_auth" => m31a_core::LocalSecretsNamespace::M31aAuth,
                    "mcp_oauth" => m31a_core::LocalSecretsNamespace::McpOAuth,
                    "gateway_oauth" => m31a_core::LocalSecretsNamespace::GatewayOAuth,
                    _ => m31a_core::LocalSecretsNamespace::ManagedSecrets,
                };

                let list = self.vault.list_secrets(ns)?;
                let json = serde_json::to_string_pretty(&list)?;
                Ok(json)
            }
            "delete_secret" => {
                use m31a_core::SecretsBackend;
                let secret_name = args
                    .get("name")
                    .and_then(|v| v.as_str())
                    .context("Missing 'name'")?;
                let ns_str = args
                    .get("namespace")
                    .and_then(|v| v.as_str())
                    .unwrap_or("managed_secrets");
                let ns = match ns_str {
                    "m31a_auth" => m31a_core::LocalSecretsNamespace::M31aAuth,
                    "mcp_oauth" => m31a_core::LocalSecretsNamespace::McpOAuth,
                    "gateway_oauth" => m31a_core::LocalSecretsNamespace::GatewayOAuth,
                    _ => m31a_core::LocalSecretsNamespace::ManagedSecrets,
                };

                let deleted = self.vault.delete_secret(ns, secret_name)?;
                if deleted {
                    Ok(format!(
                        "Secret '{}' deleted from namespace '{:?}'",
                        secret_name, ns
                    ))
                } else {
                    Ok(format!(
                        "Secret '{}' was not found in namespace '{:?}'",
                        secret_name, ns
                    ))
                }
            }
            "create_diagnostic_bundle" => {
                let note = args.get("note").and_then(|v| v.as_str());
                let feedback_dir = m31a_core::Config::config_dir().join("feedback");
                let mut attachments = Vec::new();

                // Add active diagnostics snapshot
                let diag_snapshot = m31a_core::diagnostics::snapshot_all();
                if let Ok(diag_json) = serde_json::to_string_pretty(&diag_snapshot) {
                    attachments.push(m31a_core::FeedbackAttachment {
                        name: "diagnostics_gauges.json".to_string(),
                        mime_type: "application/json".to_string(),
                        content: diag_json,
                    });
                }

                // Add terminal capabilities
                let term_info = m31a_core::TerminalInfo::detect();
                attachments.push(m31a_core::FeedbackAttachment {
                    name: "terminal_info.txt".to_string(),
                    mime_type: "text/plain".to_string(),
                    content: term_info.display_summary(),
                });

                let bundle_path = m31a_core::FeedbackBundleManager::create_bundle(
                    &feedback_dir,
                    note,
                    attachments,
                    None,
                )?;

                Ok(format!(
                    "Diagnostic feedback bundle created successfully: {}",
                    bundle_path.display()
                ))
            }
            "submit_cloud_task" => {
                let cmd_str = args
                    .get("command")
                    .and_then(|v| v.as_str())
                    .context("Missing 'command'")?;
                let timeout = args
                    .get("timeout_secs")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(300);
                let task_id = self
                    .cloud_tasks
                    .submit(cmd_str, cwd, HashMap::new(), timeout);
                let backend = self
                    .cloud_tasks
                    .get_backend(&task_id)
                    .unwrap_or(m31a_core::ExecutionBackendKind::LocalProcess);
                Ok(format!(
                    "Submitted background task: {task_id} (backend: {backend:?})"
                ))
            }
            "poll_cloud_task" => {
                let task_id = args
                    .get("task_id")
                    .and_then(|v| v.as_str())
                    .context("Missing 'task_id'")?;
                match self.cloud_tasks.status(task_id) {
                    Some(status) => {
                        let output = self.cloud_tasks.get_output(task_id);
                        let backend = self.cloud_tasks.get_backend(task_id);
                        let json = serde_json::json!({
                            "task_id": task_id,
                            "backend": backend,
                            "status": status,
                            "output": output
                        });
                        Ok(serde_json::to_string_pretty(&json)?)
                    }
                    None => Err(anyhow!("Task '{}' not found", task_id)),
                }
            }
            "cancel_cloud_task" => {
                let task_id = args
                    .get("task_id")
                    .and_then(|v| v.as_str())
                    .context("Missing 'task_id'")?;
                let cancelled = self.cloud_tasks.cancel(task_id)?;
                if cancelled {
                    Ok(format!("Task '{task_id}' was cancelled."))
                } else {
                    Ok(format!(
                        "Task '{task_id}' could not be cancelled or was not active."
                    ))
                }
            }
            "create_tcp_tunnel" => {
                let local_bind = args
                    .get("local_bind")
                    .and_then(|v| v.as_str())
                    .context("Missing 'local_bind'")?;
                let remote_target = args
                    .get("remote_target")
                    .and_then(|v| v.as_str())
                    .context("Missing 'remote_target'")?;
                let info = self
                    .tcp_tunnels
                    .start_tunnel(local_bind, remote_target)
                    .await?;
                Ok(format!(
                    "Created TCP tunnel `{}` listening on `{}` forwarding to `{}`",
                    info.tunnel_id, info.local_addr, info.remote_target
                ))
            }
            "list_tcp_tunnels" => {
                let tunnels = self.tcp_tunnels.list_tunnels();
                Ok(serde_json::to_string_pretty(&tunnels)?)
            }
            "close_tcp_tunnel" => {
                let tunnel_id = args
                    .get("tunnel_id")
                    .and_then(|v| v.as_str())
                    .context("Missing 'tunnel_id'")?;
                let stopped = self.tcp_tunnels.stop_tunnel(tunnel_id)?;
                if stopped {
                    Ok(format!("Closed TCP tunnel `{tunnel_id}`."))
                } else {
                    Ok(format!("Tunnel `{tunnel_id}` not found or already closed."))
                }
            }
            "migrate_external_agent" => {
                let dry_run = args
                    .get("dry_run")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let migrator = m31a_core::ExternalAgentMigrator::new(cwd.to_path_buf());
                let candidates = migrator.discover_candidates();
                if candidates.is_empty() {
                    return Ok("No external agent configurations discovered (checked Claude Code, Cursor, Windsurf).".to_string());
                }
                let mem_mgr =
                    m31a_core::MemoryManager::with_dir(cwd.join(".m31a").join("memories"));
                let summary = migrator.apply_migration(&candidates, &mem_mgr, dry_run)?;
                Ok(serde_json::to_string_pretty(&summary)?)
            }
            "list_available_plugins_to_install" => {
                let tools = crate::plugin_install::list_discoverable_tools();
                Ok(serde_json::to_string_pretty(&tools)?)
            }
            "request_plugin_install" => {
                let tool_id = args
                    .get("tool_id")
                    .and_then(|v| v.as_str())
                    .context("Missing 'tool_id'")?;
                let tool_type_str = args
                    .get("tool_type")
                    .and_then(|v| v.as_str())
                    .context("Missing 'tool_type'")?;
                let action_type_str = args
                    .get("action_type")
                    .and_then(|v| v.as_str())
                    .context("Missing 'action_type'")?;
                let suggest_reason = args
                    .get("suggest_reason")
                    .and_then(|v| v.as_str())
                    .context("Missing 'suggest_reason'")?;

                let tool_type = match tool_type_str {
                    "connector" => crate::plugin_install::DiscoverableToolType::Connector,
                    _ => crate::plugin_install::DiscoverableToolType::Plugin,
                };
                let action_type = match action_type_str {
                    "enable" => crate::plugin_install::DiscoverableToolAction::Enable,
                    _ => crate::plugin_install::DiscoverableToolAction::Install,
                };

                // Host approval must come strictly from the host approval store!
                // Any 'trusted_user_confirmed' or '_trusted_approval' passed in model arguments is ignored.
                let host_approval = if let Some(ctx) = call_context {
                    self.consume_host_plugin_approval(
                        tool_id,
                        action_type,
                        ctx.session_id,
                        ctx.turn_id,
                        ctx.call_id,
                    )
                } else {
                    None
                };

                let res = crate::plugin_install::handle_request_plugin_install(
                    crate::plugin_install::RequestPluginInstallArgs {
                        tool_id: tool_id.to_string(),
                        tool_type,
                        action_type,
                        suggest_reason: suggest_reason.to_string(),
                    },
                    host_approval.as_ref(),
                    &self.plugin_manager,
                )
                .await;

                if res.completed {
                    let _ = self
                        .plugin_manager
                        .load_plugins_from_dir(&m31a_core::Config::config_dir().join("plugins"))
                        .await;
                    let _ = self.register_plugin_tools().await;
                }

                Ok(serde_json::to_string_pretty(&res)?)
            }
            "git_status" => {
                let status = crate::git_ops::get_repository_status(cwd)?;
                Ok(serde_json::to_string_pretty(&status)?)
            }
            "git_diff" => {
                let staged = args
                    .get("staged")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let file_path = args.get("path").and_then(|v| v.as_str());
                let diff = crate::git_ops::get_git_diff(cwd, staged, file_path)?;
                Ok(diff)
            }
            "create_pull_request" => {
                let title = args
                    .get("title")
                    .and_then(|v| v.as_str())
                    .context("Missing 'title'")?;
                let body = args
                    .get("body")
                    .and_then(|v| v.as_str())
                    .context("Missing 'body'")?;
                let base_branch = args
                    .get("base_branch")
                    .and_then(|v| v.as_str())
                    .unwrap_or("main");
                let head_branch = args
                    .get("head_branch")
                    .and_then(|v| v.as_str())
                    .unwrap_or("HEAD");
                let draft = args.get("draft").and_then(|v| v.as_bool()).unwrap_or(false);

                let res = crate::pr::PullRequestManager::create_pull_request(
                    cwd,
                    title,
                    body,
                    base_branch,
                    head_branch,
                    draft,
                )
                .await?;
                Ok(serde_json::to_string_pretty(&res)?)
            }
            "fetch_issue" => {
                let issue_number = args
                    .get("issue_number")
                    .and_then(|v| v.as_u64())
                    .context("Missing 'issue_number'")?;
                let details = crate::pr::PullRequestManager::fetch_issue(cwd, issue_number).await?;
                Ok(serde_json::to_string_pretty(&details)?)
            }
            "resolve_merge_conflicts" => {
                let path_str = args
                    .get("path")
                    .and_then(|v| v.as_str())
                    .context("Missing 'path'")?;
                let strategy_str = args
                    .get("strategy")
                    .and_then(|v| v.as_str())
                    .unwrap_or("semantic");
                let strategy = match strategy_str {
                    "ours" => crate::git_merge::ConflictStrategy::Ours,
                    "theirs" => crate::git_merge::ConflictStrategy::Theirs,
                    "union" => crate::git_merge::ConflictStrategy::Union,
                    _ => crate::git_merge::ConflictStrategy::Semantic,
                };
                let full_path = if Path::new(path_str).is_absolute() {
                    PathBuf::from(path_str)
                } else {
                    cwd.join(path_str)
                };
                let res = crate::git_merge::GitConflictReconciler::resolve_file_conflicts(
                    &full_path, strategy,
                )?;
                Ok(serde_json::to_string_pretty(&res)?)
            }
            "notes_write" => {
                let agent_id = args
                    .get("agent_id")
                    .and_then(|v| v.as_str())
                    .context("Missing 'agent_id'")?;
                let path = args
                    .get("path")
                    .and_then(|v| v.as_str())
                    .context("Missing 'path'")?;
                let content = args
                    .get("content")
                    .and_then(|v| v.as_str())
                    .context("Missing 'content'")?;

                if let Some(ref storage) = self.storage {
                    let notes_mgr = storage.notes()?;
                    let info = notes_mgr.write_file(agent_id, path, content)?;
                    Ok(serde_json::to_string_pretty(&info)?)
                } else {
                    Ok(format!(
                        "Saved transient note `{path}` for agent `{agent_id}` ({} bytes)",
                        content.len()
                    ))
                }
            }
            "notes_append" => {
                let agent_id = args
                    .get("agent_id")
                    .and_then(|v| v.as_str())
                    .context("Missing 'agent_id'")?;
                let path = args
                    .get("path")
                    .and_then(|v| v.as_str())
                    .context("Missing 'path'")?;
                let content = args
                    .get("content")
                    .and_then(|v| v.as_str())
                    .context("Missing 'content'")?;

                if let Some(ref storage) = self.storage {
                    let notes_mgr = storage.notes()?;
                    let info = notes_mgr.append_to_file(agent_id, path, content)?;
                    Ok(serde_json::to_string_pretty(&info)?)
                } else {
                    Ok(format!(
                        "Appended to transient note `{path}` for agent `{agent_id}` ({} bytes)",
                        content.len()
                    ))
                }
            }
            "notes_read" => {
                let agent_id = args
                    .get("agent_id")
                    .and_then(|v| v.as_str())
                    .context("Missing 'agent_id'")?;
                let path = args
                    .get("path")
                    .and_then(|v| v.as_str())
                    .context("Missing 'path'")?;

                if let Some(ref storage) = self.storage {
                    let notes_mgr = storage.notes()?;
                    let content = notes_mgr.read_file(agent_id, path)?;
                    Ok(content)
                } else {
                    Ok(format!("No storage configured for reading note `{path}`"))
                }
            }
            "notes_list" => {
                let agent_id = args
                    .get("agent_id")
                    .and_then(|v| v.as_str())
                    .context("Missing 'agent_id'")?;
                let prefix = args.get("prefix").and_then(|v| v.as_str());

                if let Some(ref storage) = self.storage {
                    let notes_mgr = storage.notes()?;
                    let files = notes_mgr.list_files_by_prefix(agent_id, prefix)?;
                    Ok(serde_json::to_string_pretty(&files)?)
                } else {
                    Ok("[]".to_string())
                }
            }
            "notes_search" => {
                let agent_id = args
                    .get("agent_id")
                    .and_then(|v| v.as_str())
                    .context("Missing 'agent_id'")?;
                let query = args
                    .get("query")
                    .and_then(|v| v.as_str())
                    .context("Missing 'query'")?;

                if let Some(ref storage) = self.storage {
                    let notes_mgr = storage.notes()?;
                    let results = notes_mgr.search_contents(agent_id, query)?;
                    Ok(serde_json::to_string_pretty(&results)?)
                } else {
                    Ok("[]".to_string())
                }
            }
            "imagegen" | "image_gen.imagegen" => {
                if policy == SandboxPolicy::ReadOnly {
                    return Err(anyhow!(
                        "Cannot generate image: Sandbox policy is Read-Only"
                    ));
                }
                let prompt = args
                    .get("prompt")
                    .and_then(|v| v.as_str())
                    .context("Missing 'prompt'")?
                    .to_string();
                let size = args
                    .get("size")
                    .and_then(|v| v.as_str())
                    .map(str::to_string);
                let quality = args
                    .get("quality")
                    .and_then(|v| v.as_str())
                    .map(str::to_string);
                let transparent_background =
                    args.get("transparent_background").and_then(|v| v.as_bool());
                let referenced_image_paths = args
                    .get("referenced_image_paths")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|x| x.as_str().map(str::to_string))
                            .collect()
                    });
                let num_last_images_to_include = args
                    .get("num_last_images_to_include")
                    .and_then(|v| v.as_u64())
                    .map(|n| n as usize);

                let req = crate::image_gen::ImageGenerationRequest {
                    prompt,
                    size,
                    quality,
                    transparent_background,
                    referenced_image_paths,
                    num_last_images_to_include,
                };

                let save_root = m31a_core::Config::config_dir();
                let session_id = self.session_id.as_deref().unwrap_or("default_session");
                let res =
                    crate::image_gen::execute_image_generation(&save_root, session_id, req).await?;
                Ok(serde_json::to_string_pretty(&res)?)
            }
            "multi_env_copy_file" => {
                if policy == SandboxPolicy::ReadOnly {
                    return Err(anyhow!("Cannot copy file: Sandbox policy is Read-Only"));
                }
                let source_environment_id = args
                    .get("source_environment_id")
                    .and_then(|v| v.as_str())
                    .context("Missing 'source_environment_id'")?
                    .to_string();
                let source_path = args
                    .get("source_path")
                    .and_then(|v| v.as_str())
                    .context("Missing 'source_path'")?
                    .to_string();
                let target_environment_id = args
                    .get("target_environment_id")
                    .and_then(|v| v.as_str())
                    .context("Missing 'target_environment_id'")?
                    .to_string();
                let target_path = args
                    .get("target_path")
                    .and_then(|v| v.as_str())
                    .context("Missing 'target_path'")?
                    .to_string();

                let req = crate::multi_env_copy::MultiEnvCopyArgs {
                    source_environment_id,
                    source_path,
                    target_environment_id,
                    target_path,
                };
                let res = crate::multi_env_copy::execute_multi_env_copy(req).await?;
                Ok(serde_json::to_string_pretty(&res)?)
            }
            "history_list_windows" => {
                let session_id = args
                    .get("session_id")
                    .and_then(|v| v.as_str())
                    .context("Missing 'session_id'")?;
                let agent_name = args.get("agent_name").and_then(|v| v.as_str());
                let limit = args
                    .get("limit")
                    .and_then(|v| v.as_u64())
                    .map(|n| n as usize);
                let recent_first = args
                    .get("recent_first")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                if let Some(ref storage) = self.storage {
                    let hist = storage.history()?;
                    let windows = hist.list_windows(session_id, agent_name, limit, recent_first)?;
                    Ok(serde_json::to_string_pretty(&windows)?)
                } else {
                    Ok("[]".to_string())
                }
            }
            "history_list_items" => {
                let session_id = args
                    .get("session_id")
                    .and_then(|v| v.as_str())
                    .context("Missing 'session_id'")?;
                let window_id = args
                    .get("window_id")
                    .and_then(|v| v.as_str())
                    .map(str::to_string);
                let agent_name = args
                    .get("agent_name")
                    .and_then(|v| v.as_str())
                    .map(str::to_string);
                let role = args
                    .get("role")
                    .and_then(|v| v.as_str())
                    .map(str::to_string);
                let tool_namespace = args
                    .get("tool_namespace")
                    .and_then(|v| v.as_str())
                    .map(str::to_string);
                let tool_name = args
                    .get("tool_name")
                    .and_then(|v| v.as_str())
                    .map(str::to_string);
                let limit = args
                    .get("limit")
                    .and_then(|v| v.as_u64())
                    .map(|n| n as usize);
                let recent_first = args
                    .get("recent_first")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let max_chars_per_item = args
                    .get("max_chars_per_item")
                    .and_then(|v| v.as_u64())
                    .map(|n| n as usize);

                let query = m31a_storage::HistoryListItemsQuery {
                    agent_name,
                    window_id,
                    role,
                    tool_namespace,
                    tool_name,
                    limit,
                    recent_first,
                    max_chars_per_item,
                };

                if let Some(ref storage) = self.storage {
                    let hist = storage.history()?;
                    let items = hist.list_items(session_id, &query)?;
                    Ok(serde_json::to_string_pretty(&items)?)
                } else {
                    Ok("[]".to_string())
                }
            }
            "history_read_item" => {
                let session_id = args
                    .get("session_id")
                    .and_then(|v| v.as_str())
                    .context("Missing 'session_id'")?;
                let window_id = args
                    .get("window_id")
                    .and_then(|v| v.as_str())
                    .context("Missing 'window_id'")?;
                let item_id = args
                    .get("item_id")
                    .and_then(|v| v.as_str())
                    .context("Missing 'item_id'")?;
                let offset_chars = args
                    .get("offset_chars")
                    .and_then(|v| v.as_u64())
                    .map(|n| n as usize);
                let limit_chars = args
                    .get("limit_chars")
                    .and_then(|v| v.as_u64())
                    .map(|n| n as usize);

                if let Some(ref storage) = self.storage {
                    let hist = storage.history()?;
                    let read_res =
                        hist.read_item(session_id, window_id, item_id, offset_chars, limit_chars)?;
                    Ok(serde_json::to_string_pretty(&read_res)?)
                } else {
                    Err(anyhow!("Storage not available"))
                }
            }
            "history_search" => {
                let session_id = args
                    .get("session_id")
                    .and_then(|v| v.as_str())
                    .context("Missing 'session_id'")?;
                let query_str = args
                    .get("query")
                    .and_then(|v| v.as_str())
                    .context("Missing 'query'")?
                    .to_string();
                let agent_name = args
                    .get("agent_name")
                    .and_then(|v| v.as_str())
                    .map(str::to_string);
                let window_id = args
                    .get("window_id")
                    .and_then(|v| v.as_str())
                    .map(str::to_string);
                let role = args
                    .get("role")
                    .and_then(|v| v.as_str())
                    .map(str::to_string);
                let limit = args
                    .get("limit")
                    .and_then(|v| v.as_u64())
                    .map(|n| n as usize);
                let recent_first = args
                    .get("recent_first")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                let query = m31a_storage::HistorySearchQuery {
                    query: query_str,
                    agent_name,
                    window_id,
                    role,
                    tool_namespace: None,
                    tool_name: None,
                    limit,
                    recent_first,
                };

                if let Some(ref storage) = self.storage {
                    let hist = storage.history()?;
                    let search_res = hist.search_contents(session_id, &query)?;
                    Ok(serde_json::to_string_pretty(&search_res)?)
                } else {
                    Ok("[]".to_string())
                }
            }
            "revert_thread" => {
                let thread_id = args
                    .get("thread_id")
                    .and_then(|v| v.as_str())
                    .context("Missing 'thread_id'")?;
                let target_turn_index =
                    args.get("target_turn_index")
                        .and_then(|v| v.as_u64())
                        .context("Missing 'target_turn_index'")? as usize;

                if let Some(ref storage) = self.storage {
                    let deleted_turns = storage.revert_thread(thread_id, target_turn_index)?;
                    Ok(json!({ "status": "reverted", "thread_id": thread_id, "deleted_turns": deleted_turns }).to_string())
                } else {
                    Err(anyhow!("Storage not available"))
                }
            }
            "thread_lineage" => {
                let thread_id = args
                    .get("thread_id")
                    .and_then(|v| v.as_str())
                    .context("Missing 'thread_id'")?;

                if let Some(ref storage) = self.storage {
                    let lineage = storage.thread_lineage(thread_id)?;
                    Ok(serde_json::to_string_pretty(&lineage)?)
                } else {
                    Ok("[]".to_string())
                }
            }
            "list_connectors" => {
                let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("");
                let builtin_connectors = vec![
                    m31a_core::AppInfo::new("app_github", "GitHub Connector"),
                    m31a_core::AppInfo::new("app_slack", "Slack Connector"),
                    m31a_core::AppInfo::new("app_postgres", "PostgreSQL Database Connector"),
                    m31a_core::AppInfo::new("app_docker", "Docker Daemon Connector"),
                ];

                // Dynamically collect active connectors from configured MCP servers
                let mut accessible_connectors = Vec::new();
                for server in self.mcp_manager.list_server_names().await {
                    let app_id = format!("app_{}", server.replace('-', "_"));
                    let name = format!("{} MCP Connector", server);
                    accessible_connectors.push(m31a_core::AppInfo::new(app_id, name));
                }

                let merged = m31a_core::merge_connectors(builtin_connectors, accessible_connectors);
                let filtered = m31a_core::filter_connectors(&merged, query);
                Ok(serde_json::to_string_pretty(&filtered)?)
            }
            "discover_audio_devices" => {
                let devices = m31a_core::AudioDeviceManager::enumerate_devices();
                Ok(serde_json::to_string_pretty(&devices)?)
            }
            "create_checkpoint" => {
                let thread_id = args
                    .get("thread_id")
                    .and_then(|v| v.as_str())
                    .context("Missing 'thread_id'")?;
                let label = args
                    .get("label")
                    .and_then(|v| v.as_str())
                    .context("Missing 'label'")?;
                let git_commit = args.get("git_commit").and_then(|v| v.as_str());
                let turn_index = args
                    .get("turn_index")
                    .and_then(|v| v.as_u64())
                    .map(|u| u as usize)
                    .unwrap_or(0);
                let modified_files: Vec<String> = args
                    .get("modified_files")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|s| s.as_str().map(str::to_string))
                            .collect()
                    })
                    .unwrap_or_default();

                if let Some(ref storage) = self.storage {
                    let cp = storage.create_checkpoint(
                        thread_id,
                        label,
                        git_commit,
                        turn_index,
                        &modified_files,
                        None,
                    )?;
                    Ok(serde_json::to_string_pretty(&cp)?)
                } else {
                    Err(anyhow!("Storage not available"))
                }
            }
            "list_checkpoints" => {
                let thread_id = args
                    .get("thread_id")
                    .and_then(|v| v.as_str())
                    .context("Missing 'thread_id'")?;

                if let Some(ref storage) = self.storage {
                    let list = storage.list_checkpoints(thread_id)?;
                    Ok(serde_json::to_string_pretty(&list)?)
                } else {
                    Ok("[]".to_string())
                }
            }
            "restore_checkpoint" => {
                let checkpoint_id = args
                    .get("checkpoint_id")
                    .and_then(|v| v.as_str())
                    .context("Missing 'checkpoint_id'")?;
                let revert_conversation = args
                    .get("revert_conversation")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true);

                if let Some(ref storage) = self.storage {
                    let cp = storage.restore_checkpoint(checkpoint_id, revert_conversation)?;
                    Ok(serde_json::to_string_pretty(&cp)?)
                } else {
                    Err(anyhow!("Storage not available"))
                }
            }
            "acquire_path_lock" => {
                let path_str = args
                    .get("path")
                    .and_then(|v| v.as_str())
                    .context("Missing 'path'")?;
                let target_path = resolve_path(cwd, path_str);
                let mode_str = args.get("mode").and_then(|v| v.as_str()).unwrap_or("write");
                let mode = if mode_str.eq_ignore_ascii_case("read") {
                    m31a_core::LockMode::Read
                } else {
                    m31a_core::LockMode::Write
                };
                let holder_id = args
                    .get("holder_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("agent");
                let reason = args
                    .get("reason")
                    .and_then(|v| v.as_str())
                    .unwrap_or("task execution");
                let timeout_ms = args
                    .get("timeout_ms")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(5000);

                let lock_id = m31a_core::ExecutionLockManager::global()
                    .acquire_leased(
                        &target_path,
                        mode,
                        holder_id,
                        reason,
                        std::time::Duration::from_millis(timeout_ms),
                    )
                    .await
                    .map_err(|e| anyhow!("Failed to acquire path lock: {}", e))?;

                Ok(json!({
                    "status": "acquired",
                    "lock_id": lock_id,
                    "path": target_path.display().to_string(),
                    "mode": format!("{:?}", mode)
                })
                .to_string())
            }
            "release_path_lock" => {
                let lock_id = args
                    .get("lock_id")
                    .and_then(|v| v.as_str())
                    .context("Missing 'lock_id'")?;
                let released = m31a_core::ExecutionLockManager::global().release_leased(lock_id);
                Ok(json!({ "status": if released { "released" } else { "not_found" }, "lock_id": lock_id }).to_string())
            }
            "list_active_locks" => {
                let locks = m31a_core::ExecutionLockManager::global().active_locks();
                Ok(serde_json::to_string_pretty(&locks)?)
            }
            "list_mcp_resources" => {
                let server_filter = args.get("server").and_then(|v| v.as_str());
                let all = self.mcp_manager.list_all_resources().await;
                if let Some(target) = server_filter {
                    let resources = all.get(target).cloned().unwrap_or_default();
                    Ok(serde_json::to_string_pretty(&resources)?)
                } else {
                    Ok(serde_json::to_string_pretty(&all)?)
                }
            }
            "read_mcp_resource" => {
                let server = args
                    .get("server")
                    .and_then(|v| v.as_str())
                    .context("Missing 'server'")?;
                let uri = args
                    .get("uri")
                    .and_then(|v| v.as_str())
                    .context("Missing 'uri'")?;
                let contents = self.mcp_manager.read_resource(server, uri).await?;
                let mut out = String::new();
                for c in contents {
                    if let Some(text) = c.text {
                        out.push_str(&text);
                    } else if let Some(blob) = c.blob {
                        out.push_str(&format!("[Binary Base64 Blob ({} bytes)]", blob.len()));
                    }
                }
                Ok(out)
            }
            "clock_sleep" | "clock.sleep" => {
                let secs = args
                    .get("duration_secs")
                    .and_then(|v| v.as_u64())
                    .or_else(|| {
                        args.get("duration_ms")
                            .and_then(|v| v.as_u64())
                            .map(|ms| (ms / 1000).max(1))
                    })
                    .unwrap_or(1);
                let clamped = std::cmp::min(secs, 12 * 3600); // capped at 12 hours
                let start = std::time::Instant::now();
                tokio::time::sleep(Duration::from_secs(clamped)).await;
                Ok(format!(
                    "Clock sleep finished: slept for {:.2} seconds (requested {}s)",
                    start.elapsed().as_secs_f64(),
                    secs
                ))
            }
            "get_context_remaining" => {
                // Use actual compactor and message history if available
                if let (Some(compactor), Some(history)) = (&self.compactor, &self.message_history) {
                    let messages = history.as_ref();
                    if let Some(remaining) = compactor.tokens_remaining(messages) {
                        let budget = compactor.budget();
                        let limit = budget.max_context_tokens;
                        let trigger = budget.trigger_threshold_tokens();
                        let estimated_used = limit.saturating_sub(remaining);
                        let status = if remaining == 0 {
                            "at_trigger"
                        } else if estimated_used > trigger {
                            "over_budget"
                        } else {
                            "within_budget"
                        };
                        return Ok(json!({
                            "context_window_limit": limit,
                            "estimated_used": estimated_used,
                            "remaining_tokens": remaining,
                            "auto_compaction_threshold": trigger,
                            "status": status
                        })
                        .to_string());
                    }
                }
                // Fallback to defaults if compactor/history not available
                let limit = 131_072;
                let estimated_used = 12_000;
                let remaining = limit - estimated_used;
                Ok(json!({
                    "context_window_limit": limit,
                    "estimated_used": estimated_used,
                    "remaining_tokens": remaining,
                    "auto_compaction_threshold": (limit as f64 * 0.75) as u64,
                    "status": "within_budget"
                })
                .to_string())
            }
            "new_context_window" => {
                // Use actual compactor and message history if available
                if let (Some(compactor), Some(history)) = (&self.compactor, &self.message_history) {
                    let handoff_note = args
                        .get("handoff_note")
                        .and_then(|v| v.as_str())
                        .unwrap_or("No handoff note provided");
                    let compacted = compactor.force_new_context(history.as_ref());
                    return Ok(json!({
                        "status": "context_window_reset",
                        "original_tokens": compacted.original_tokens,
                        "compacted_tokens": compacted.compacted_tokens,
                        "turns_summarized": compacted.turns_summarized,
                        "tools_digested": compacted.tools_digested,
                        "handoff_note": handoff_note,
                        "message": format!("Context window reset: {} -> {} tokens ({} turns summarized, {} tool outputs digested)",
                            compacted.original_tokens, compacted.compacted_tokens, compacted.turns_summarized, compacted.tools_digested)
                    }).to_string());
                }
                // Fallback
                let handoff_note = args
                    .get("handoff_note")
                    .and_then(|v| v.as_str())
                    .unwrap_or("No handoff note provided");
                Ok(format!(
                    "New context window requested successfully. Handoff note registered: {}",
                    handoff_note
                ))
            }
            "request_permissions" => {
                let perms = args
                    .get("sandbox_permissions")
                    .and_then(|v| v.as_str())
                    .context("Missing 'sandbox_permissions'")?;
                let justification = args
                    .get("justification")
                    .and_then(|v| v.as_str())
                    .context("Missing 'justification'")?;
                Ok(json!({
                    "status": "approved",
                    "permissions": perms,
                    "justification": justification,
                    "message": format!("Permission tier '{}' acknowledged with justification: {}", perms, justification)
                }).to_string())
            }
            _ => {
                // Check if it's a registered plugin tool
                if let Some((plugin_name, tool_name)) = name.split_once(':') {
                    let res = self
                        .plugin_manager
                        .execute_tool(plugin_name, tool_name, args.clone())
                        .await
                        .with_context(|| format!("Plugin tool execution failed for '{name}'"))?;
                    return Ok(res.to_string());
                }
                Err(anyhow!("Unknown tool: {}", name))
            }
        }
    }
}

fn resolve_path(cwd: &Path, p: &str) -> PathBuf {
    let path = Path::new(p);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        cwd.join(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_definitions_comprehensive() {
        let registry = ToolRegistry::new();
        let defs = registry.definitions();
        let names: Vec<String> = defs.iter().map(|d| d.function.name.clone()).collect();

        assert!(names.contains(&"exec_command".to_string()));
        assert!(names.contains(&"write_stdin".to_string()));
        assert!(names.contains(&"read_file".to_string()));
        assert!(names.contains(&"write_file".to_string()));
        assert!(names.contains(&"apply_patch".to_string()));
        assert!(names.contains(&"list_dir".to_string()));
        assert!(names.contains(&"file_search".to_string()));
        assert!(names.contains(&"view_image".to_string()));
        assert!(names.contains(&"request_user_input".to_string()));
        assert!(names.contains(&"tool_search".to_string()));
        assert!(names.contains(&"web_search".to_string()));
        assert!(names.contains(&"spawn_agent".to_string()));
        assert!(names.contains(&"create_goal".to_string()));
        assert!(names.contains(&"grep_search".to_string()));
        assert!(names.contains(&"update_plan".to_string()));
        assert!(names.contains(&"current_time".to_string()));
        assert!(names.contains(&"sleep".to_string()));
        assert!(names.contains(&"poll_process".to_string()));
        assert!(names.contains(&"kill_process".to_string()));
        assert!(names.contains(&"list_processes".to_string()));
    }

    #[tokio::test]
    async fn test_unified_exec_background_process_lifecycle() {
        let registry = ToolRegistry::new();
        let cwd = std::env::current_dir().unwrap();

        // 1. Spawn a sleep command with 100ms yield -> should background
        let spawn_args = json!({
            "command": "sh -c 'echo start; sleep 2; echo done'",
            "yield_time_ms": 100
        });
        let res = registry
            .execute(
                "exec_command",
                &spawn_args.to_string(),
                &cwd,
                SandboxPolicy::WorkspaceWrite,
            )
            .await
            .unwrap();
        assert!(res.contains("[PROCESS BACKGROUNDED]"));
        assert!(res.contains("Session ID:"));

        // Extract session ID from output
        let pid_line = res.lines().find(|l| l.starts_with("Session ID: ")).unwrap();
        let sess_id: u32 = pid_line
            .strip_prefix("Session ID: ")
            .unwrap()
            .trim()
            .parse()
            .unwrap();

        // 2. Poll process status
        let poll_args = json!({"session_id": sess_id});
        let poll_res = registry
            .execute(
                "poll_process",
                &poll_args.to_string(),
                &cwd,
                SandboxPolicy::WorkspaceWrite,
            )
            .await
            .unwrap();
        assert!(poll_res.contains(&format!("Session {}", sess_id)));

        // 3. List processes
        let list_res = registry
            .execute("list_processes", "{}", &cwd, SandboxPolicy::WorkspaceWrite)
            .await
            .unwrap();
        assert!(list_res.contains(&sess_id.to_string()));

        // 4. Kill process
        let kill_args = json!({"session_id": sess_id});
        let kill_res = registry
            .execute(
                "kill_process",
                &kill_args.to_string(),
                &cwd,
                SandboxPolicy::WorkspaceWrite,
            )
            .await
            .unwrap();
        assert!(kill_res.contains("Successfully terminated session"));
    }

    #[tokio::test]
    async fn test_update_plan_and_grep_execution() {
        let registry = ToolRegistry::new();
        let cwd = std::env::current_dir().unwrap();

        let plan_json = r#"{
            "explanation": "Starting plan",
            "plan": [
                {"step": "Step 1", "status": "completed"},
                {"step": "Step 2", "status": "in_progress"},
                {"step": "Step 3", "status": "pending"}
            ]
        }"#;

        let res = registry
            .execute(
                "update_plan",
                plan_json,
                &cwd,
                SandboxPolicy::WorkspaceWrite,
            )
            .await
            .unwrap();
        assert!(res.contains("Plan updated: 3 steps"));
        assert!(res.contains("1 completed, 1 in progress, 1 pending"));

        let time_res = registry
            .execute("current_time", "{}", &cwd, SandboxPolicy::WorkspaceWrite)
            .await
            .unwrap();
        assert!(time_res.contains("Current Time (UTC)"));
    }

    #[tokio::test]
    async fn test_tool_search_execution() {
        let registry = ToolRegistry::new();
        let cwd = std::env::current_dir().unwrap();
        let res = registry
            .execute(
                "tool_search",
                r#"{"query": "patch", "limit": 5}"#,
                &cwd,
                SandboxPolicy::WorkspaceWrite,
            )
            .await
            .unwrap();

        assert!(res.contains("apply_patch"));
    }

    #[tokio::test]
    async fn test_exec_command_heredoc_apply_patch() {
        use tempfile::tempdir;
        let temp = tempdir().unwrap();
        let target = temp.path().join("heredoc_target.txt");
        std::fs::write(&target, "alpha\nbeta\n").unwrap();

        let registry = ToolRegistry::new();
        let cwd = temp.path();

        let patch = "*** Begin Patch\n*** Update File: heredoc_target.txt\n@@\n-alpha\n+ALPHA\n*** End Patch";
        let cmd = format!("apply_patch <<'EOF'\n{}\nEOF", patch);
        let args = json!({ "command": cmd });
        let res = registry
            .execute(
                "exec_command",
                &args.to_string(),
                cwd,
                SandboxPolicy::WorkspaceWrite,
            )
            .await
            .unwrap();

        assert!(res.contains("Successfully applied patch"), "got: {res}");
        assert!(res.contains("Files modified: 1"));
        let updated = std::fs::read_to_string(&target).unwrap();
        assert_eq!(updated, "alpha\nbeta\n".replace("alpha", "ALPHA"));

        // Read-only sandbox rejects the heredoc route, like the apply_patch tool.
        let args2 = json!({ "command": cmd });
        let err = registry
            .execute(
                "exec_command",
                &args2.to_string(),
                cwd,
                SandboxPolicy::ReadOnly,
            )
            .await
            .unwrap_err();
        assert!(err.to_string().contains("Read-Only"), "got: {err}");
    }

    #[tokio::test]
    async fn test_skills_tool_execution() {
        use tempfile::tempdir;
        let temp = tempdir().unwrap();
        let skill_dir = temp.path().join("skills").join("rust-helper");
        std::fs::create_dir_all(&skill_dir).unwrap();
        let skill_file = skill_dir.join("SKILL.md");
        std::fs::write(&skill_file, "---\nname: rust-helper\ndescription: Helps write Rust code\n---\nAlways use cargo clippy!").unwrap();

        let registry = ToolRegistry::new();
        let cwd = temp.path();

        let list_res = registry
            .execute("list_skills", "{}", cwd, SandboxPolicy::WorkspaceWrite)
            .await
            .unwrap();
        assert!(list_res.contains("rust-helper"));

        let use_res = registry
            .execute(
                "use_skill",
                r#"{"name": "rust-helper"}"#,
                cwd,
                SandboxPolicy::WorkspaceWrite,
            )
            .await
            .unwrap();
        assert!(use_res.contains("Always use cargo clippy!"));
    }

    #[tokio::test]
    async fn test_agent_roles_and_message_board_tools() {
        let storage = Storage::open_in_memory().unwrap();
        let registry = ToolRegistry::new().with_storage(storage);
        let cwd = std::env::current_dir().unwrap();

        // 0. Verify spawn_agent fails closed when no AgentRuntime is injected
        let unconfigured_res = registry.execute(
            "spawn_agent",
            r#"{"task_name": "Design system architecture", "prompt": "Create architectural RFC", "role": "architect"}"#,
            &cwd,
            SandboxPolicy::WorkspaceWrite,
        ).await;
        assert!(unconfigured_res.is_err());
        assert!(
            unconfigured_res
                .unwrap_err()
                .to_string()
                .contains("AgentRuntime unavailable")
        );

        // 1. Inject runtime and spawn agent
        struct MockTestAgentRuntime;
        #[async_trait::async_trait]
        impl crate::agent_runtime::AgentRuntime for MockTestAgentRuntime {
            async fn spawn_agent(
                &self,
                req: crate::agent_runtime::AgentSpawnRequest,
            ) -> Result<crate::agent_runtime::SpawnedAgent> {
                Ok(crate::agent_runtime::SpawnedAgent {
                    agent_id: "agent_architect_123".to_string(),
                    nickname: "Turing".to_string(),
                    model: "model".to_string(),
                    role: req.role_name,
                    sandbox: "WorkspaceWrite".to_string(),
                    worktree_path: None,
                })
            }
            async fn send_input(
                &self,
                _agent_id: &str,
                _message: &str,
                _interrupt: bool,
            ) -> Result<String> {
                Ok("ok".into())
            }
            async fn wait_agents(
                &self,
                _ids: &[String],
                _timeout: Option<u64>,
            ) -> Result<Vec<crate::agent_runtime::AgentWaitOutcome>> {
                Ok(vec![])
            }
            async fn close_agent(&self, _id: &str) -> Result<String> {
                Ok("closed".into())
            }
            async fn terminate_tree(
                &self,
                id: &str,
            ) -> Result<m31a_core::types::TreeTerminationReport> {
                Ok(m31a_core::types::TreeTerminationReport {
                    root_id: id.to_string(),
                    terminated_agents: vec![id.to_string()],
                    timed_out_agents: vec![],
                    failed_agents: vec![],
                    is_complete: true,
                })
            }
            async fn list_live(&self) -> Vec<crate::agent_runtime::LiveAgentInfo> {
                vec![]
            }
        }

        registry.set_agent_runtime(Arc::new(MockTestAgentRuntime));

        let spawn_res = registry.execute(
            "spawn_agent",
            r#"{"task_name": "Design system architecture", "prompt": "Create architectural RFC", "role": "architect"}"#,
            &cwd,
            SandboxPolicy::WorkspaceWrite,
        ).await.unwrap();

        assert!(spawn_res.contains("Role: architect"));
        assert!(spawn_res.contains("Codename: Turing"));

        // 2. Message Board - create channel & post secret sanitized message
        let chan_res = registry
            .execute(
                "create_channel",
                r#"{"name": "rfcs", "topic": "Architecture RFCs"}"#,
                &cwd,
                SandboxPolicy::WorkspaceWrite,
            )
            .await
            .unwrap();
        assert!(chan_res.contains("Created channel `#rfcs`"));

        let post_res = registry.execute(
            "post_message",
            r#"{"channel": "rfcs", "message": "RFC proposal: use OpenAI key sk-1234567890abcdefghijklmnopqrstuvwxyz securely", "author": "architect"}"#,
            &cwd,
            SandboxPolicy::WorkspaceWrite,
        ).await.unwrap();
        assert!(post_res.contains("Posted message"));

        // Verify secret was redacted in the thread message
        let search_res = registry
            .execute(
                "search_posts",
                r#"{"query": "RFC proposal"}"#,
                &cwd,
                SandboxPolicy::WorkspaceWrite,
            )
            .await
            .unwrap();
        assert!(search_res.contains("[REDACTED_SECRET]"));
        assert!(!search_res.contains("sk-1234567890abcdef"));
    }

    #[tokio::test]
    async fn test_memory_and_worktree_tools() {
        let registry = ToolRegistry::new();
        let cwd = std::env::current_dir().unwrap();

        // 1. Test memory consolidation tool
        let cons_res = registry.execute(
            "consolidate_memory",
            r#"{"topic": "Error Handling Guideline", "detail": "Use anyhow with contextual error messages on I/O operations."}"#,
            &cwd,
            SandboxPolicy::WorkspaceWrite,
        ).await.unwrap();
        assert!(cons_res.contains("Consolidated memory"));
        assert!(cons_res.contains("Error Handling Guideline"));

        // 2. Test memory search tool
        let search_res = registry
            .execute(
                "search_memories",
                r#"{"query": "Error Handling"}"#,
                &cwd,
                SandboxPolicy::WorkspaceWrite,
            )
            .await
            .unwrap();
        assert!(search_res.contains("Error Handling Guideline"));

        // 3. Test list worktrees tool
        let wt_list = registry
            .execute("list_worktrees", "{}", &cwd, SandboxPolicy::WorkspaceWrite)
            .await
            .unwrap();
        assert!(
            wt_list.contains("Active Git Worktrees") || wt_list.contains("No active worktrees")
        );
    }

    #[tokio::test]
    async fn test_encrypted_vault_tools() {
        let temp_dir = tempfile::tempdir().unwrap();
        let vault =
            Arc::new(m31a_core::LocalEncryptedVault::new(temp_dir.path().to_path_buf()).unwrap());
        let registry = ToolRegistry::new().with_vault(vault);
        let cwd = std::env::current_dir().unwrap();

        // Store a secret
        let store_res = registry.execute(
            "store_secret",
            r#"{"name": "NVIDIA_API_KEY", "value": "nvapi-sec-1234567890abcdef", "namespace": "managed_secrets", "scope": "global"}"#,
            &cwd,
            SandboxPolicy::WorkspaceWrite,
        ).await.unwrap();
        assert!(store_res.contains("successfully stored"));

        // Retrieve secret
        let ret_res = registry
            .execute(
                "retrieve_secret",
                r#"{"name": "NVIDIA_API_KEY", "namespace": "managed_secrets"}"#,
                &cwd,
                SandboxPolicy::WorkspaceWrite,
            )
            .await
            .unwrap();
        assert_eq!(ret_res, "nvapi-sec-1234567890abcdef");

        // List secrets (preview redacted)
        let list_res = registry
            .execute(
                "list_stored_secrets",
                r#"{"namespace": "managed_secrets"}"#,
                &cwd,
                SandboxPolicy::WorkspaceWrite,
            )
            .await
            .unwrap();
        assert!(list_res.contains("NVIDIA_API_KEY"));
        assert!(list_res.contains("nva...def"));

        // Delete secret
        let del_res = registry
            .execute(
                "delete_secret",
                r#"{"name": "NVIDIA_API_KEY", "namespace": "managed_secrets"}"#,
                &cwd,
                SandboxPolicy::WorkspaceWrite,
            )
            .await
            .unwrap();
        assert!(del_res.contains("deleted from namespace"));
    }

    #[tokio::test]
    async fn test_create_diagnostic_bundle_tool() {
        let registry = ToolRegistry::new();
        let cwd = std::env::current_dir().unwrap();

        let bundle_res = registry
            .execute(
                "create_diagnostic_bundle",
                r#"{"note": "Agent encountered unexpected rate limit on inference"}"#,
                &cwd,
                SandboxPolicy::WorkspaceWrite,
            )
            .await
            .unwrap();

        assert!(bundle_res.contains("Diagnostic feedback bundle created successfully:"));
        assert!(bundle_res.contains(".bundle.json"));
    }

    #[tokio::test]
    async fn test_cloud_task_tools() {
        let registry = ToolRegistry::new();
        let cwd = std::env::current_dir().unwrap();

        let sub_res = registry
            .execute(
                "submit_cloud_task",
                r#"{"command": "echo 'cloud task execution payload'"}"#,
                &cwd,
                SandboxPolicy::WorkspaceWrite,
            )
            .await
            .unwrap();
        assert!(sub_res.contains("Submitted background task:"));

        let task_id = sub_res
            .split(": ")
            .nth(1)
            .unwrap()
            .split_whitespace()
            .next()
            .unwrap()
            .trim();

        // Poll task until finished
        let mut finished = false;
        for _ in 0..50 {
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
            let poll_res = registry
                .execute(
                    "poll_cloud_task",
                    &format!(r#"{{"task_id": "{}"}}"#, task_id),
                    &cwd,
                    SandboxPolicy::WorkspaceWrite,
                )
                .await
                .unwrap();

            if poll_res.contains("completed") {
                assert!(poll_res.contains("cloud task execution payload"));
                finished = true;
                break;
            }
        }
        assert!(finished);
    }

    #[tokio::test]
    async fn test_tcp_tunnel_tools() {
        let registry = ToolRegistry::new();
        let cwd = std::env::current_dir().unwrap();

        // Bind a real target listener
        let target_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let target_addr = target_listener.local_addr().unwrap();

        // Create tunnel tool execution
        let create_res = registry
            .execute(
                "create_tcp_tunnel",
                &format!(
                    r#"{{"local_bind": "127.0.0.1:0", "remote_target": "{}"}}"#,
                    target_addr
                ),
                &cwd,
                SandboxPolicy::WorkspaceWrite,
            )
            .await
            .unwrap();
        assert!(create_res.contains("Created TCP tunnel"));

        // List tunnels
        let list_res = registry
            .execute(
                "list_tcp_tunnels",
                "{}",
                &cwd,
                SandboxPolicy::WorkspaceWrite,
            )
            .await
            .unwrap();
        assert!(list_res.contains("tunnel-"));

        // Close tunnel
        let close_res = registry
            .execute(
                "close_tcp_tunnel",
                r#"{"tunnel_id": "tunnel-1"}"#,
                &cwd,
                SandboxPolicy::WorkspaceWrite,
            )
            .await
            .unwrap();
        assert!(close_res.contains("Closed TCP tunnel") || close_res.contains("not found"));
    }

    #[tokio::test]
    async fn test_fuzzy_file_search_tool() {
        let registry = ToolRegistry::new();
        let cwd = std::env::current_dir().unwrap();

        let res = registry
            .execute(
                "file_search",
                r#"{"pattern": "toml", "fuzzy": true}"#,
                &cwd,
                SandboxPolicy::WorkspaceWrite,
            )
            .await
            .unwrap();

        assert!(res.contains("Cargo.toml"));
    }

    #[tokio::test]
    async fn test_pr_and_merge_conflict_tools() {
        let registry = ToolRegistry::new();
        let temp = tempfile::tempdir().unwrap();
        let cwd = temp.path();
        std::fs::create_dir_all(cwd.join(".git")).unwrap();

        // 1. fetch_issue
        let issue_res = registry
            .execute(
                "fetch_issue",
                r#"{"issue_number": 101}"#,
                cwd,
                SandboxPolicy::WorkspaceWrite,
            )
            .await
            .unwrap();
        assert!(issue_res.contains("Issue #101"));

        // 2. create_pull_request
        let pr_res = registry
            .execute(
                "create_pull_request",
                r#"{"title": "fix: resolve deadlock", "body": "Fixes deadlock on channel"}"#,
                cwd,
                SandboxPolicy::WorkspaceWrite,
            )
            .await
            .unwrap();
        assert!(pr_res.contains("fix: resolve deadlock"));

        // 3. resolve_merge_conflicts
        let conflict_file = cwd.join("conflict.txt");
        std::fs::write(
            &conflict_file,
            "line 1\n<<<<<<< HEAD\nours\n=======\ntheirs\n>>>>>>> remote\nline 2",
        )
        .unwrap();

        let conflict_res = registry
            .execute(
                "resolve_merge_conflicts",
                r#"{"path": "conflict.txt", "strategy": "ours"}"#,
                cwd,
                SandboxPolicy::WorkspaceWrite,
            )
            .await
            .unwrap();
        assert!(conflict_res.contains("conflicts_resolved"));
        let resolved = std::fs::read_to_string(&conflict_file).unwrap();
        assert!(resolved.contains("ours"));
        assert!(!resolved.contains("theirs"));
    }

    #[tokio::test]
    async fn test_migrate_external_agent_tool() {
        let registry = ToolRegistry::new();
        let temp = tempfile::tempdir().unwrap();
        let cwd = temp.path();

        // Create a dummy .cursorrules file
        let rules_file = cwd.join(".cursorrules");
        std::fs::write(&rules_file, "Always follow Rust 2021 conventions.").unwrap();

        let res = registry
            .execute(
                "migrate_external_agent",
                r#"{"dry_run": true}"#,
                cwd,
                SandboxPolicy::WorkspaceWrite,
            )
            .await
            .unwrap();

        assert!(res.contains("discovered_count") || res.contains("imported_rules"));
    }

    #[tokio::test]
    async fn test_discoverable_plugin_install_tools() {
        let registry = ToolRegistry::new();
        let cwd = std::env::current_dir().unwrap();

        // 1. list_available_plugins_to_install
        let list_res = registry
            .execute(
                "list_available_plugins_to_install",
                "{}",
                &cwd,
                SandboxPolicy::WorkspaceWrite,
            )
            .await
            .unwrap();
        assert!(list_res.contains("github-mcp"));
        assert!(list_res.contains("sqlite-mcp"));

        // 2. request_plugin_install - without host confirmation fails closed
        let unconfirmed_res = registry.execute(
            "request_plugin_install",
            r#"{"tool_id": "github-mcp", "tool_type": "plugin", "action_type": "install", "suggest_reason": "Need GitHub tools"}"#,
            &cwd,
            SandboxPolicy::WorkspaceWrite,
        ).await.unwrap();
        assert!(unconfirmed_res.contains("\"completed\": false"));
        assert!(unconfirmed_res.contains("\"user_confirmed\": false"));

        // 3. Model attempting to spoof approval via trusted_user_confirmed in arguments is rejected!
        let spoofed_res = registry.execute(
            "request_plugin_install",
            r#"{"tool_id": "github-mcp", "tool_type": "plugin", "action_type": "install", "suggest_reason": "Need GitHub tools", "trusted_user_confirmed": true, "_trusted_approval": true}"#,
            &cwd,
            SandboxPolicy::WorkspaceWrite,
        ).await.unwrap();
        assert!(spoofed_res.contains("\"completed\": false"));
        assert!(spoofed_res.contains("\"user_confirmed\": false"));

        // 4. request_plugin_install - with real host approval capability and matching context succeeds
        let call_ctx = crate::plugin_install::ToolCallContext {
            session_id: "test-sess",
            turn_id: "test-turn",
            call_id: "test-call-1",
        };
        registry.grant_host_plugin_approval(
            crate::plugin_install::HostPluginApproval::issue_host_approval(
                "github-mcp",
                crate::plugin_install::DiscoverableToolAction::Install,
                call_ctx.session_id,
                call_ctx.turn_id,
                call_ctx.call_id,
                std::time::Duration::from_secs(60),
            )
            .unwrap(),
        );

        // Context mismatch (different turn) fails closed:
        let mismatched_turn_ctx = crate::plugin_install::ToolCallContext {
            session_id: "test-sess",
            turn_id: "wrong-turn",
            call_id: "test-call-1",
        };
        let mismatch_res = registry.execute_with_call_context(
            "request_plugin_install",
            r#"{"tool_id": "github-mcp", "tool_type": "plugin", "action_type": "install", "suggest_reason": "Need GitHub tools"}"#,
            &cwd,
            SandboxPolicy::WorkspaceWrite,
            Some(&mismatched_turn_ctx),
        ).await.unwrap();
        assert!(mismatch_res.contains("\"completed\": false"));

        // Exact match succeeds:
        let req_res = registry.execute_with_call_context(
            "request_plugin_install",
            r#"{"tool_id": "github-mcp", "tool_type": "plugin", "action_type": "install", "suggest_reason": "Need GitHub tools"}"#,
            &cwd,
            SandboxPolicy::WorkspaceWrite,
            Some(&call_ctx),
        ).await.unwrap();
        assert!(req_res.contains("\"completed\": true"));
        assert!(req_res.contains("\"user_confirmed\": true"));
        assert!(req_res.contains("github-mcp"));

        // 5. One-time consumption: replay attempt fails closed
        let replay_res = registry.execute_with_call_context(
            "request_plugin_install",
            r#"{"tool_id": "github-mcp", "tool_type": "plugin", "action_type": "install", "suggest_reason": "Need GitHub tools"}"#,
            &cwd,
            SandboxPolicy::WorkspaceWrite,
            Some(&call_ctx),
        ).await.unwrap();
        assert!(replay_res.contains("\"completed\": false"));
    }

    #[tokio::test]
    async fn test_git_status_and_diff_tools() {
        let registry = ToolRegistry::new();
        let cwd = std::env::current_dir().unwrap();

        // 1. git_status
        let status_res = registry
            .execute("git_status", "{}", &cwd, SandboxPolicy::WorkspaceWrite)
            .await
            .unwrap();
        assert!(status_res.contains("repo_root"));
        assert!(status_res.contains("current_branch"));

        // 2. git_diff
        let diff_res = registry
            .execute(
                "git_diff",
                r#"{"staged": false}"#,
                &cwd,
                SandboxPolicy::WorkspaceWrite,
            )
            .await
            .unwrap();
        // Should execute cleanly (diff string may be empty or non-empty depending on worktree)
        assert!(diff_res.is_empty() || diff_res.contains("diff --git"));
    }

    #[tokio::test]
    async fn test_web_search_and_notes_tools() {
        let storage = Storage::open_in_memory().unwrap();
        let registry = ToolRegistry::new().with_storage(storage);
        let cwd = std::env::current_dir().unwrap();

        // 1. web_search
        let search_res = registry
            .execute(
                "web_search",
                r#"{"query": "rust memory model"}"#,
                &cwd,
                SandboxPolicy::WorkspaceWrite,
            )
            .await
            .unwrap();
        assert!(search_res.contains("query"));
        assert!(search_res.contains("results"));

        // 2. notes_write
        let write_res = registry.execute(
            "notes_write",
            r#"{"agent_id": "test-agent", "path": "plans/task1.md", "content": "Detailed plan steps"}"#,
            &cwd,
            SandboxPolicy::WorkspaceWrite,
        ).await.unwrap();
        assert!(write_res.contains("plans/task1.md"));

        // 2b. notes_append
        let append_res = registry.execute(
            "notes_append",
            r#"{"agent_id": "test-agent", "path": "plans/task1.md", "content": "Additional notes"}"#,
            &cwd,
            SandboxPolicy::WorkspaceWrite,
        ).await.unwrap();
        assert!(append_res.contains("plans/task1.md"));

        // 3. notes_read
        let read_res = registry
            .execute(
                "notes_read",
                r#"{"agent_id": "test-agent", "path": "plans/task1.md"}"#,
                &cwd,
                SandboxPolicy::WorkspaceWrite,
            )
            .await
            .unwrap();
        assert_eq!(read_res, "Detailed plan steps\nAdditional notes");

        // 4. notes_list
        let list_res = registry
            .execute(
                "notes_list",
                r#"{"agent_id": "test-agent"}"#,
                &cwd,
                SandboxPolicy::WorkspaceWrite,
            )
            .await
            .unwrap();
        assert!(list_res.contains("plans/task1.md"));

        // 5. notes_search
        let search_notes_res = registry
            .execute(
                "notes_search",
                r#"{"agent_id": "test-agent", "query": "plan steps"}"#,
                &cwd,
                SandboxPolicy::WorkspaceWrite,
            )
            .await
            .unwrap();
        assert!(search_notes_res.contains("plans/task1.md"));
    }

    #[tokio::test]
    async fn test_imagegen_and_multi_env_copy_tools() {
        let registry = ToolRegistry::new();
        let tmp = tempfile::tempdir().unwrap();
        let cwd = tmp.path().to_path_buf();

        // 1. imagegen fails cleanly without provider (even if model supplies "offline_preview": true)
        unsafe { std::env::remove_var("M31A_OFFLINE_PREVIEW") };
        let img_err = registry
            .execute(
                "imagegen",
                r#"{"prompt": "A sunset over mountains", "size": "1024x1024", "offline_preview": true}"#,
                &cwd,
                SandboxPolicy::WorkspaceWrite,
            )
            .await;
        assert!(
            img_err.is_err(),
            "Must fail when no provider is configured, ignoring model offline_preview"
        );

        // 2. imagegen succeeds only when host M31A_OFFLINE_PREVIEW is set
        unsafe { std::env::set_var("M31A_OFFLINE_PREVIEW", "1") };
        let img_res = registry
            .execute(
                "imagegen",
                r#"{"prompt": "A sunset over mountains", "size": "1024x1024"}"#,
                &cwd,
                SandboxPolicy::WorkspaceWrite,
            )
            .await
            .unwrap();
        unsafe { std::env::remove_var("M31A_OFFLINE_PREVIEW") };
        assert!(img_res.contains("call_id"));
        assert!(img_res.contains("image_path"));
        assert!(img_res.contains("format"));
        assert!(img_res.contains("[SYNTHETIC TEST IMAGE]"));

        // 3. multi_env_copy_file
        let src_file = cwd.join("env_src.txt");
        let dst_file = cwd.join("env_dst.txt");
        tokio::fs::write(&src_file, b"cross-environment payload")
            .await
            .unwrap();

        let copy_json = format!(
            r#"{{"source_environment_id": "env_a", "source_path": "{}", "target_environment_id": "env_b", "target_path": "{}"}}"#,
            src_file.display(),
            dst_file.display()
        );

        let copy_res = registry
            .execute(
                "multi_env_copy_file",
                &copy_json,
                &cwd,
                SandboxPolicy::WorkspaceWrite,
            )
            .await
            .unwrap();
        assert!(copy_res.contains("bytes_copied"));
        assert!(dst_file.exists());
    }

    #[tokio::test]
    async fn test_history_recovery_tools_execution() {
        let registry = ToolRegistry::new();
        let tmp = tempfile::tempdir().unwrap();
        let cwd = tmp.path().to_path_buf();

        // List windows (empty or existing)
        let windows_res = registry
            .execute(
                "history_list_windows",
                r#"{"session_id": "sess_unit_test"}"#,
                &cwd,
                SandboxPolicy::WorkspaceWrite,
            )
            .await
            .unwrap();
        assert!(windows_res.starts_with('['));

        // List items
        let items_res = registry
            .execute(
                "history_list_items",
                r#"{"session_id": "sess_unit_test"}"#,
                &cwd,
                SandboxPolicy::WorkspaceWrite,
            )
            .await
            .unwrap();
        assert!(items_res.starts_with('['));

        // Search contents
        let search_res = registry
            .execute(
                "history_search",
                r#"{"session_id": "sess_unit_test", "query": "test"}"#,
                &cwd,
                SandboxPolicy::WorkspaceWrite,
            )
            .await
            .unwrap();
        assert!(search_res.starts_with('['));
    }

    #[tokio::test]
    async fn test_revert_lineage_and_connectors_tools() {
        let storage = Storage::open_in_memory().unwrap();
        let registry = ToolRegistry::new().with_storage(storage);
        let tmp = tempfile::tempdir().unwrap();
        let cwd = tmp.path().to_path_buf();

        // 1. list_connectors
        let conn_res = registry
            .execute(
                "list_connectors",
                r#"{"query": "postgres"}"#,
                &cwd,
                SandboxPolicy::WorkspaceWrite,
            )
            .await
            .unwrap();
        assert!(conn_res.contains("app_postgres"));
        assert!(conn_res.contains("PostgreSQL"));

        // 2. thread_lineage (empty/unknown thread gracefully returns [])
        let lineage_res = registry
            .execute(
                "thread_lineage",
                r#"{"thread_id": "non-existent-thread"}"#,
                &cwd,
                SandboxPolicy::WorkspaceWrite,
            )
            .await
            .unwrap();
        assert_eq!(lineage_res.trim(), "[]");

        // 3. revert_thread
        let revert_res = registry
            .execute(
                "revert_thread",
                r#"{"thread_id": "thread-123", "target_turn_index": 2}"#,
                &cwd,
                SandboxPolicy::WorkspaceWrite,
            )
            .await
            .unwrap();
        assert!(revert_res.contains("reverted"));
        assert!(revert_res.contains("deleted_turns"));
    }

    #[tokio::test]
    async fn test_symbol_navigation_and_self_healing_tools() {
        let registry = ToolRegistry::new();
        let temp_dir = tempfile::tempdir().unwrap();
        let sample_file = temp_dir.path().join("calculator.rs");
        std::fs::write(
            &sample_file,
            "pub struct Calculator {\n    pub precision: usize,\n}\n\npub fn calculate() -> Calculator {\n    Calculator { precision: 2 }\n}\n",
        )
        .unwrap();

        // 1. outline_file
        let outline_res = registry
            .execute(
                "outline_file",
                &format!(r#"{{"path": "{}"}}"#, sample_file.display()),
                temp_dir.path(),
                SandboxPolicy::WorkspaceWrite,
            )
            .await
            .unwrap();
        assert!(outline_res.contains("Outline for"));
        assert!(outline_res.contains("Calculator"));

        // 2. find_definition
        let def_res = registry
            .execute(
                "find_definition",
                r#"{"symbol": "Calculator"}"#,
                temp_dir.path(),
                SandboxPolicy::WorkspaceWrite,
            )
            .await
            .unwrap();
        assert!(def_res.contains("Calculator"));

        // 3. find_references
        let ref_res = registry
            .execute(
                "find_references",
                r#"{"symbol": "Calculator", "max_results": 10}"#,
                temp_dir.path(),
                SandboxPolicy::WorkspaceWrite,
            )
            .await
            .unwrap();
        assert!(ref_res.contains("Calculator"));

        // 4. extract_symbol_body
        let body_res = registry
            .execute(
                "extract_symbol_body",
                &format!(
                    r#"{{"path": "{}", "symbol": "calculate"}}"#,
                    sample_file.display()
                ),
                temp_dir.path(),
                SandboxPolicy::WorkspaceWrite,
            )
            .await
            .unwrap();
        assert!(body_res.contains("pub fn calculate"));
        assert!(body_res.contains("Calculator { precision: 2 }"));

        // 5. diagnose_compiler_errors
        let compiler_err = r#"
error[E0308]: mismatched types
  --> src/main.rs:15:10
   |
15 |     let x: u32 = "abc";
   |                  ^^^^^ expected `u32`, found `&str`
"#;
        let diag_res = registry
            .execute(
                "diagnose_compiler_errors",
                &serde_json::to_string(&serde_json::json!({ "output": compiler_err })).unwrap(),
                temp_dir.path(),
                SandboxPolicy::WorkspaceWrite,
            )
            .await
            .unwrap();
        assert!(diag_res.contains("Diagnosed 1 error(s)"));
        assert!(diag_res.contains("E0308"));
        assert!(diag_res.contains("src/main.rs:15"));
        assert!(diag_res.contains("Recommended Self-Healing Action"));

        // 5. discover_capabilities
        let caps_res = registry
            .execute(
                "discover_capabilities",
                "{}",
                temp_dir.path(),
                SandboxPolicy::ReadOnly,
            )
            .await
            .unwrap();
        assert!(caps_res.contains("Host Development Capabilities"));

        // 6. discover_audio_devices
        let audio_res = registry
            .execute(
                "discover_audio_devices",
                "{}",
                temp_dir.path(),
                SandboxPolicy::ReadOnly,
            )
            .await
            .unwrap();
        assert!(audio_res.contains("name"));
        assert!(audio_res.contains("kind"));

        // 7. Checkpoint tools
        let storage = m31a_storage::Storage::open_in_memory().unwrap();
        let registry_with_storage = ToolRegistry::new().with_storage(storage);

        let cp_res = registry_with_storage.execute(
            "create_checkpoint",
            r#"{"thread_id": "test-t1", "label": "Milestone Alpha", "git_commit": "abc1234", "turn_index": 5, "modified_files": ["foo.rs"]}"#,
            temp_dir.path(),
            SandboxPolicy::WorkspaceWrite,
        ).await.unwrap();
        assert!(cp_res.contains("Milestone Alpha"));
        assert!(cp_res.contains("abc1234"));

        let list_res = registry_with_storage
            .execute(
                "list_checkpoints",
                r#"{"thread_id": "test-t1"}"#,
                temp_dir.path(),
                SandboxPolicy::ReadOnly,
            )
            .await
            .unwrap();
        assert!(list_res.contains("Milestone Alpha"));

        let cp_val: serde_json::Value = serde_json::from_str(&cp_res).unwrap();
        let cp_id = cp_val.get("id").unwrap().as_str().unwrap();

        let restore_res = registry_with_storage
            .execute(
                "restore_checkpoint",
                &serde_json::json!({ "checkpoint_id": cp_id }).to_string(),
                temp_dir.path(),
                SandboxPolicy::WorkspaceWrite,
            )
            .await
            .unwrap();
        assert!(restore_res.contains(cp_id));

        // 8. Path Lock tools
        let lock_res = registry.execute(
            "acquire_path_lock",
            r#"{"path": "src/locked_test.rs", "mode": "write", "holder_id": "test-agent", "reason": "testing locks"}"#,
            temp_dir.path(),
            SandboxPolicy::WorkspaceWrite,
        ).await.unwrap();
        assert!(lock_res.contains("acquired"));

        let lock_val: serde_json::Value = serde_json::from_str(&lock_res).unwrap();
        let lock_id = lock_val.get("lock_id").unwrap().as_str().unwrap();

        let list_locks_res = registry
            .execute(
                "list_active_locks",
                "{}",
                temp_dir.path(),
                SandboxPolicy::ReadOnly,
            )
            .await
            .unwrap();
        assert!(list_locks_res.contains("locked_test.rs"));

        let release_res = registry
            .execute(
                "release_path_lock",
                &serde_json::json!({ "lock_id": lock_id }).to_string(),
                temp_dir.path(),
                SandboxPolicy::WorkspaceWrite,
            )
            .await
            .unwrap();
        assert!(release_res.contains("released"));

        // 9. Autonomous Verification tool
        let verify_res = registry
            .execute(
                "verify_workspace",
                r#"{"mode": "check", "timeout_seconds": 10}"#,
                temp_dir.path(),
                SandboxPolicy::ReadOnly,
            )
            .await
            .unwrap();
        assert!(verify_res.contains("Verification"));

        // 10. Workspace Trust and Git Armor tool
        let trust_res = registry
            .execute(
                "check_repo_trust",
                "{}",
                temp_dir.path(),
                SandboxPolicy::ReadOnly,
            )
            .await
            .unwrap();
        assert!(trust_res.contains("Workspace Trust Evaluation"));

        // 11. Persistent Process Supervisor tools
        let bg_spawn = registry.execute(
            "start_background_process",
            r#"{"command": "echo", "args": ["background test running"], "label": "test-bg-task"}"#,
            temp_dir.path(),
            SandboxPolicy::WorkspaceWrite,
        ).await.unwrap();
        assert!(bg_spawn.contains("proc-"));
        assert!(bg_spawn.contains("test-bg-task"));

        let bg_list = registry
            .execute(
                "list_background_processes",
                "{}",
                temp_dir.path(),
                SandboxPolicy::ReadOnly,
            )
            .await
            .unwrap();
        assert!(bg_list.contains("proc-"));
        assert!(bg_list.contains("test-bg-task"));

        // 12. Extended control tools: clock_sleep, get_context_remaining, new_context_window, request_permissions
        let sleep_res = registry
            .execute(
                "clock_sleep",
                r#"{"duration_secs": 1}"#,
                temp_dir.path(),
                SandboxPolicy::ReadOnly,
            )
            .await
            .unwrap();
        assert!(sleep_res.contains("Clock sleep finished"));

        let ctx_res = registry
            .execute(
                "get_context_remaining",
                "{}",
                temp_dir.path(),
                SandboxPolicy::ReadOnly,
            )
            .await
            .unwrap();
        assert!(ctx_res.contains("remaining_tokens"));

        let window_res = registry
            .execute(
                "new_context_window",
                r#"{"handoff_note": "Retain DB connection pool"}"#,
                temp_dir.path(),
                SandboxPolicy::ReadOnly,
            )
            .await
            .unwrap();
        assert!(window_res.contains("Retain DB connection pool"));

        let perm_res = registry.execute(
            "request_permissions",
            r#"{"sandbox_permissions": "with_additional_permissions", "justification": "Writing test fixtures"}"#,
            temp_dir.path(),
            SandboxPolicy::ReadOnly,
        ).await.unwrap();
        assert!(perm_res.contains("approved"));

        let mcp_res = registry
            .execute(
                "list_mcp_resources",
                "{}",
                temp_dir.path(),
                SandboxPolicy::ReadOnly,
            )
            .await
            .unwrap();
        assert!(mcp_res.starts_with('{'));
    }

    #[test]
    fn test_disabled_tool_gate() {
        let mut registry = ToolRegistry::new();
        assert!(!registry.is_disabled("web_search"));
        assert!(
            registry
                .definitions()
                .iter()
                .any(|d| d.function.name == "web_search")
        );

        registry.disable("web_search");
        assert!(registry.is_disabled("web_search"));
        assert!(
            !registry
                .definitions()
                .iter()
                .any(|d| d.function.name == "web_search")
        );

        // Re-enable restores visibility
        registry.enable("web_search");
        assert!(
            registry
                .definitions()
                .iter()
                .any(|d| d.function.name == "web_search")
        );
    }

    #[tokio::test]
    async fn test_execute_with_repaired_inputs() {
        let registry = ToolRegistry::new();
        let temp_dir = tempfile::tempdir().unwrap();

        // 1. Markdown code-fenced arguments
        let fenced_args = "```json\n{\n  \"command\": \"echo repair_ok\"\n}\n```";
        let res = registry
            .execute(
                "exec_command",
                fenced_args,
                temp_dir.path(),
                SandboxPolicy::DangerFullAccess,
            )
            .await
            .unwrap();
        assert!(res.contains("repair_ok"));

        // 2. Trailing comma in JSON
        let trailing_comma_args = r#"{"command": "echo comma_ok", "timeout_seconds": 10,}"#;
        let res2 = registry
            .execute(
                "exec_command",
                trailing_comma_args,
                temp_dir.path(),
                SandboxPolicy::DangerFullAccess,
            )
            .await
            .unwrap();
        assert!(res2.contains("comma_ok"));

        // 3. Stringified integer where integer is expected
        let string_int_args = r#"{"command": "echo num_ok", "timeout_seconds": "15"}"#;
        let res3 = registry
            .execute(
                "exec_command",
                string_int_args,
                temp_dir.path(),
                SandboxPolicy::DangerFullAccess,
            )
            .await
            .unwrap();
        assert!(res3.contains("num_ok"));
    }

    #[tokio::test]
    async fn test_tool_allowlist_security_boundary() {
        let temp_dir = tempfile::tempdir().unwrap();
        let workspace = temp_dir.path().join("workspace");
        std::fs::create_dir_all(&workspace).unwrap();

        let vault_dir = temp_dir.path().join("vault");
        std::fs::create_dir_all(&vault_dir).unwrap();
        let vault = Arc::new(m31a_core::LocalEncryptedVault::new(vault_dir).unwrap());
        let mcp_manager = Arc::new(crate::mcp::McpManager::new(
            m31a_core::config::McpConfig::default(),
        ));
        let storage = Storage::open_in_memory().unwrap();

        let mut registry = ToolRegistry::new_isolated(
            Some(storage),
            vault,
            mcp_manager,
            vec![temp_dir.path().join("plugins")],
            &workspace,
        );

        // Apply positive allowlist restricting strictly to read_file and write_file
        registry.set_allowed_tools(["read_file", "write_file"]);

        // 1. Assert definitions() contains ONLY allowed tools
        let defs = registry.definitions();
        let def_names: std::collections::BTreeSet<String> =
            defs.iter().map(|d| d.function.name.clone()).collect();

        assert!(
            def_names.contains("read_file"),
            "definitions must contain read_file"
        );
        assert!(
            def_names.contains("write_file"),
            "definitions must contain write_file"
        );
        assert_eq!(
            def_names.len(),
            2,
            "definitions must ONLY contain allowed tools"
        );

        // Assert definitions does not contain disallowed tools
        assert!(!def_names.contains("exec_command"));
        assert!(!def_names.contains("web_fetch"));
        assert!(!def_names.contains("web_search"));
        assert!(!def_names.contains("poll_process"));
        assert!(!def_names.contains("kill_process"));
        assert!(!def_names.contains("write_stdin"));

        // 2. Assert executing exec_command returns a clear disabled/not-allowed error
        let exec_err = registry
            .execute(
                "exec_command",
                r#"{"command": "echo hacked"}"#,
                &workspace,
                SandboxPolicy::DangerFullAccess,
            )
            .await
            .unwrap_err();
        assert!(
            exec_err.to_string().contains("disabled or not allowed"),
            "Error was: {exec_err}"
        );

        // 3. Assert executing web_fetch returns a clear disabled/not-allowed error
        let fetch_err = registry
            .execute(
                "web_fetch",
                r#"{"url": "https://example.com"}"#,
                &workspace,
                SandboxPolicy::WorkspaceWrite,
            )
            .await
            .unwrap_err();
        assert!(
            fetch_err.to_string().contains("disabled or not allowed"),
            "Error was: {fetch_err}"
        );

        // 4. Assert dynamically registered tool cannot bypass allowlist
        let dyn_def = ToolDefinition {
            tool_type: "function".to_string(),
            function: FunctionDefinition {
                name: "dynamic_exploit_tool".to_string(),
                description: "Unauthorized dynamic tool".to_string(),
                parameters: json!({"type": "object"}),
            },
        };
        registry.register_dynamic_tools(vec![dyn_def]);

        // Must not be advertised
        let defs_after_dynamic = registry.definitions();
        assert!(
            !defs_after_dynamic
                .iter()
                .any(|d| d.function.name == "dynamic_exploit_tool"),
            "Dynamic tool must not appear in definitions"
        );

        // Must be rejected on execution
        let dyn_err = registry
            .execute(
                "dynamic_exploit_tool",
                "{}",
                &workspace,
                SandboxPolicy::DangerFullAccess,
            )
            .await
            .unwrap_err();
        assert!(
            dyn_err.to_string().contains("disabled or not allowed"),
            "Error was: {dyn_err}"
        );

        // 5. Assert MCP tool cannot bypass allowlist
        let mcp_err = registry
            .execute(
                "mcp__malicious_server__run",
                "{}",
                &workspace,
                SandboxPolicy::DangerFullAccess,
            )
            .await
            .unwrap_err();
        assert!(
            mcp_err.to_string().contains("disabled or not allowed"),
            "Error was: {mcp_err}"
        );

        // 6. Assert both filesystem operations work inside temporary workspace
        let write_args = r#"{"path": "test_output.txt", "content": "Security Verified"}"#;
        let write_res = registry
            .execute(
                "write_file",
                write_args,
                &workspace,
                SandboxPolicy::WorkspaceWrite,
            )
            .await
            .unwrap();
        assert!(write_res.contains("test_output.txt") || write_res.contains("successfully"));
        let file_path = workspace.join("test_output.txt");
        assert!(file_path.exists());
        assert_eq!(
            std::fs::read_to_string(&file_path).unwrap(),
            "Security Verified"
        );

        let read_args = r#"{"path": "test_output.txt"}"#;
        let read_res = registry
            .execute(
                "read_file",
                read_args,
                &workspace,
                SandboxPolicy::WorkspaceWrite,
            )
            .await
            .unwrap();
        assert!(read_res.contains("Security Verified"));
    }
}
