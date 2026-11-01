//! Execution orchestrator for Code Mode scripts.
//!
//! Coordinates script compilation, type generation, sandbox process spawning, and tool dispatch.

use anyhow::{Context, Result};
use m31a_core::SandboxPolicy;
use m31a_core::types::{FunctionDefinition, ToolDefinition};
use m31a_tools::ToolRegistry;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

pub use crate::protocol::{
    CODE_MODE_PRAGMA_PREFIX, DEFAULT_EXEC_YIELD_TIME_MS, DEFAULT_MAX_OUTPUT_TOKENS,
    PUBLIC_TOOL_NAME as EXEC_TOOL_NAME, WAIT_TOOL_NAME, build_exec_tool_description,
    build_wait_tool_description, parse_exec_source,
};
use crate::runtime::CodeModeRuntime;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecPragma {
    #[serde(default = "default_yield_ms")]
    pub yield_time_ms: u64,
    #[serde(default = "default_max_tokens")]
    pub max_output_tokens: usize,
}

fn default_yield_ms() -> u64 {
    DEFAULT_EXEC_YIELD_TIME_MS
}

fn default_max_tokens() -> usize {
    DEFAULT_MAX_OUTPUT_TOKENS
}

#[derive(Debug, Clone)]
pub struct CellState {
    pub cell_id: String,
    pub output_accumulator: String,
    pub is_finished: bool,
    pub unconsumed_offset: usize,
}

#[derive(Clone)]
pub struct CodeModeOrchestrator {
    cells: Arc<Mutex<HashMap<String, CellState>>>,
    runtime: CodeModeRuntime,
}

impl Default for CodeModeOrchestrator {
    fn default() -> Self {
        Self::new()
    }
}

impl CodeModeOrchestrator {
    pub fn new() -> Self {
        Self {
            cells: Arc::new(Mutex::new(HashMap::new())),
            runtime: CodeModeRuntime::new(),
        }
    }

    pub fn with_runtime(runtime: CodeModeRuntime) -> Self {
        Self {
            cells: Arc::new(Mutex::new(HashMap::new())),
            runtime,
        }
    }

    pub fn runtime(&self) -> &CodeModeRuntime {
        &self.runtime
    }

    pub fn tool_definitions(&self, all_tools: &[ToolDefinition]) -> Vec<ToolDefinition> {
        let exec_desc = build_exec_tool_description(all_tools, DEFAULT_EXEC_YIELD_TIME_MS);
        let wait_desc = build_wait_tool_description();

        vec![
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: EXEC_TOOL_NAME.to_string(),
                    description: exec_desc,
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "code": {
                                "type": "string",
                                "description": "The JavaScript or TypeScript source code to evaluate in the isolated Code Mode environment."
                            }
                        },
                        "required": ["code"]
                    }),
                },
            },
            ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: WAIT_TOOL_NAME.to_string(),
                    description: wait_desc,
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "cell_id": {
                                "type": "string",
                                "description": "Identifier of the cell returned by exec."
                            },
                            "yield_time_ms": {
                                "type": "integer",
                                "description": "Timeout in ms to wait for more output before yielding again (default 10000)."
                            },
                            "max_tokens": {
                                "type": "integer",
                                "description": "Maximum token budget for returned output."
                            },
                            "terminate": {
                                "type": "boolean",
                                "description": "Whether to terminate the cell immediately."
                            }
                        },
                        "required": ["cell_id"]
                    }),
                },
            },
        ]
    }

    pub fn store(&self, key: &str, value: serde_json::Value) {
        self.runtime.store(key, value);
    }

    pub fn load(&self, key: &str) -> Option<serde_json::Value> {
        self.runtime.load(key)
    }

    pub async fn execute_batch(
        &self,
        source: &str,
        registry: &ToolRegistry,
        cwd: &Path,
        policy: SandboxPolicy,
    ) -> Result<String> {
        let parsed = parse_exec_source(source).map_err(|e| anyhow::anyhow!(e))?;

        let cell_id = format!("cell_{}", Uuid::new_v4().simple());
        let outcome = self
            .runtime
            .run(&parsed.code, registry, cwd, policy, parsed.yield_time_ms)
            .await?;

        let mut cell = CellState {
            cell_id: cell_id.clone(),
            output_accumulator: outcome.output.clone(),
            is_finished: !outcome.yielded,
            unconsumed_offset: 0,
        };

        let result = if outcome.yielded {
            let out = self.truncate_to_budget(&cell.output_accumulator, parsed.max_output_tokens);
            cell.unconsumed_offset = cell.output_accumulator.len();
            self.cells.lock().unwrap().insert(cell_id.clone(), cell);
            format!("Script running with cell ID {cell_id}\n\n[Intermediate Observation]:\n{out}")
        } else {
            let final_output =
                self.truncate_to_budget(&cell.output_accumulator, parsed.max_output_tokens);
            self.cells.lock().unwrap().insert(cell_id, cell);
            final_output
        };

        Ok(result)
    }

    pub fn wait_cell(
        &self,
        cell_id: &str,
        _timeout_ms: u64,
        max_tokens: Option<usize>,
        terminate: bool,
    ) -> Result<String> {
        let mut cells = self.cells.lock().unwrap();
        let cell = cells
            .get_mut(cell_id)
            .context("Cell not found or already closed")?;

        if terminate {
            cell.is_finished = true;
            return Ok(format!("Cell {} terminated by user request.", cell_id));
        }

        let slice = &cell.output_accumulator[cell.unconsumed_offset..];
        cell.unconsumed_offset = cell.output_accumulator.len();

        let budget = max_tokens.unwrap_or(DEFAULT_MAX_OUTPUT_TOKENS);
        let out = self.truncate_to_budget(slice, budget);

        if cell.is_finished {
            Ok(format!("Cell {} finished.\n\n{}", cell_id, out))
        } else {
            Ok(format!(
                "Script running with cell ID {}\n\n{}",
                cell_id, out
            ))
        }
    }

    fn truncate_to_budget(&self, text: &str, max_tokens: usize) -> String {
        let estimated_chars = max_tokens.saturating_mul(4);
        if text.len() <= estimated_chars {
            text.to_string()
        } else {
            format!(
                "{}\n\n[OUTPUT TRUNCATED: Exceeded {} tokens limit]",
                &text[..estimated_chars],
                max_tokens
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_orchestrator_nested_tools_and_store() {
        let orch = CodeModeOrchestrator::new();
        let registry = ToolRegistry::new();
        let cwd = std::env::current_dir().unwrap();
        orch.runtime()
            .trust_store()
            .lock()
            .unwrap()
            .trust(&cwd, m31a_trust::TrustLevel::SessionOnly)
            .unwrap();

        let script = r#"
            store("counter", 42)
            text("Starting orchestration...")
            await tools.read_file({"path": "Cargo.toml", "start_line": 1, "end_line": 3})
        "#;

        let policy = if m31a_sandbox::SandboxRunner::new().has_bwrap() {
            SandboxPolicy::WorkspaceWrite
        } else {
            SandboxPolicy::DangerFullAccess
        };

        let res = orch
            .execute_batch(script, &registry, &cwd, policy)
            .await
            .unwrap();
        assert!(
            res.contains("Starting orchestration..."),
            "res did not contain expected text. Got: {:?}",
            res
        );
        assert!(res.contains(">>> Tool: read_file"));
        assert_eq!(orch.load("counter"), Some(serde_json::json!(42)));
    }
}
