//! `m31a-code-mode`: Programmatic multi-tool execution runtime for LLMs.
//!
//! Allows language models to orchestrate multiple tools within sandboxed JavaScript/TypeScript
//! execution environments with full type safety, ambient declarations, and IPC streaming.

pub mod ambient_dts;
pub mod cell_actor;
pub mod ipc_channel;
pub mod json_schema;
pub mod orchestrator;
pub mod protocol;
pub mod runtime;

pub use ambient_dts::{AmbientDtsGenerator, AmbientToolSpec};
pub use cell_actor::{
    CellActor, CellActorError, CellCheckpoint, CellExecutionBudget, CellLifecycleState,
    CellYieldReason,
};
pub use ipc_channel::{IpcFramingCodec, IpcMessage, IpcPeer};
pub use json_schema::{
    DEFAULT_INPUT_SCHEMA_MAX_BYTES, MCP_TYPESCRIPT_PREAMBLE, render_json_schema_to_typescript,
    render_json_schema_to_typescript_with_budget,
};
pub use orchestrator::{
    CODE_MODE_PRAGMA_PREFIX, CellState, CodeModeOrchestrator, DEFAULT_EXEC_YIELD_TIME_MS,
    DEFAULT_MAX_OUTPUT_TOKENS, EXEC_TOOL_NAME, ExecPragma, WAIT_TOOL_NAME,
};
pub use protocol::{
    CodeModeToolKind, EnabledToolMetadata, PUBLIC_TOOL_NAME, ParsedExecSource,
    build_exec_tool_description, build_wait_tool_description, normalize_code_mode_identifier,
    parse_exec_source,
};
pub use runtime::{CodeModeError, CodeModeRuntime, ProcessLifecycleState, RuntimeExecutionOutcome};

#[async_trait::async_trait]
pub trait CodeModeToolDispatcher: Send + Sync {
    async fn execute(
        &self,
        name: &str,
        args: &str,
        cwd: &std::path::Path,
        policy: m31a_core::SandboxPolicy,
    ) -> anyhow::Result<String>;
}

#[async_trait::async_trait]
impl CodeModeToolDispatcher for m31a_tools::ToolRegistry {
    async fn execute(
        &self,
        name: &str,
        args: &str,
        cwd: &std::path::Path,
        policy: m31a_core::SandboxPolicy,
    ) -> anyhow::Result<String> {
        self.execute(name, args, cwd, policy).await
    }
}
