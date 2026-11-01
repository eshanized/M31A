//! Sovereign TypeScript ambient declaration (.d.ts) and execution contract generator.
//!
//! Synthesizes ambient TypeScript declaration files (.d.ts) exposing available tools to sandboxed scripts.
//! In Code Mode, the LLM composes multiple actions programmatically using JavaScript/TypeScript.
//! This module synthesizes complete ambient type definitions (`m31a_ambient.d.ts`) containing:
//! - Ambient global functions (`print`, `yieldToUser`).
//! - Ambient key-value state store (`store.get`, `store.set`).
//! - Typed namespaces declaring available agent tools (`declare namespace tools { ... }`)
//!   derived from active schemas.
//! - Strict system instruction prompt contracts guiding the LLM.

use crate::json_schema::render_json_schema_to_typescript_with_budget;
use m31a_tools::output_schema::ToolOutputSchema;
use serde::{Deserialize, Serialize};

/// Metadata description of a tool used to synthesize TypeScript ambient signatures.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AmbientToolSpec {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
    pub output_schema: Option<ToolOutputSchema>,
}

impl AmbientToolSpec {
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        input_schema: serde_json::Value,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            input_schema,
            output_schema: None,
        }
    }
}

/// Ambient declaration synthesizer for Code Mode.
pub struct AmbientDtsGenerator {
    max_type_definition_bytes: usize,
}

impl Default for AmbientDtsGenerator {
    fn default() -> Self {
        Self {
            max_type_definition_bytes: 32 * 1024, // 32 KiB type budget
        }
    }
}

impl AmbientDtsGenerator {
    pub fn new(max_type_definition_bytes: usize) -> Self {
        Self {
            max_type_definition_bytes,
        }
    }

    /// Synthesize complete `m31a_ambient.d.ts` declaration content.
    pub fn generate_ambient_dts(&self, tools: &[AmbientToolSpec]) -> String {
        let mut dts = String::new();

        dts.push_str("/**\n * M31A Autonomous Code Mode Ambient API Declarations\n */\n\n");

        // 1. Global utility functions
        dts.push_str("/** Print text output directly to the cell execution transcript. */\n");
        dts.push_str("declare function print(...args: any[]): void;\n\n");

        dts.push_str("/** Suspend execution and yield control to the user or supervisor for interactive input. */\n");
        dts.push_str("declare function yieldToUser(prompt: string): Promise<string>;\n\n");

        // 2. Global state store
        dts.push_str("/** Persistent cross-turn key-value storage for the current session. */\n");
        dts.push_str("declare const store: {\n");
        dts.push_str("    get<T = any>(key: string): T | undefined;\n");
        dts.push_str("    set(key: string, value: any): void;\n");
        dts.push_str("};\n\n");

        // 3. Namespace `tools` containing all active tool definitions
        dts.push_str("/** Available sovereign agent tools and MCP capabilities. */\n");
        dts.push_str("declare namespace tools {\n");

        for tool in tools {
            let identifier = sanitize_ts_identifier(&tool.name);
            let ts_type_name = format!("{}Args", to_pascal_case(&identifier));

            // Render typed interface for the tool's arguments
            let interface_body = render_json_schema_to_typescript_with_budget(
                &tool.input_schema,
                self.max_type_definition_bytes,
            );

            dts.push_str("    /**\n");
            for line in tool.description.lines() {
                dts.push_str(&format!("     * {}\n", line.trim()));
            }
            dts.push_str("     */\n");
            dts.push_str(&format!("    type {ts_type_name} = {interface_body};\n\n"));

            dts.push_str(&format!(
                "    function {identifier}(args: {ts_type_name}): Promise<any>;\n\n"
            ));
        }

        dts.push_str("}\n");
        dts
    }

    /// Generate Code Mode execution prompt contract for LLM system instructions.
    pub fn generate_execution_contract(&self, tools: &[AmbientToolSpec]) -> String {
        let dts = self.generate_ambient_dts(tools);

        format!(
            r#"## Code Mode Execution Protocol

You have access to **Code Mode**, allowing you to execute multi-step operations programmatically by generating JavaScript/TypeScript script cells.

### Execution Rules:
1. All available tools are exposed under the global `tools` namespace (e.g. `await tools.read_file({{ path: "..." }})`).
2. Tools are asynchronous and return Promises; you MUST use `await`.
3. Use `print(...args)` to emit diagnostic logs to the output transcript.
4. Use `store.set(key, val)` and `store.get(key)` to persist state across script executions.
5. You can yield to the user for interactive input with `await yieldToUser("question")`.
6. Return a final expression or value at the end of the cell to supply the cell's primary result.

### Ambient Type Declarations:
```typescript
{dts}
```
"#
        )
    }
}

fn sanitize_ts_identifier(name: &str) -> String {
    let mut out = String::new();
    for c in name.chars() {
        if c.is_alphanumeric() || c == '_' || c == '$' {
            out.push(c);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() || out.chars().next().unwrap().is_numeric() {
        format!("_{out}")
    } else {
        out
    }
}

fn to_pascal_case(ident: &str) -> String {
    let mut out = String::new();
    let mut capitalize = true;
    for c in ident.chars() {
        if c == '_' || c == '-' {
            capitalize = true;
        } else if capitalize {
            out.push(c.to_ascii_uppercase());
            capitalize = false;
        } else {
            out.push(c);
        }
    }
    if out.is_empty() {
        "Tool".to_string()
    } else {
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ambient_dts_globals_and_namespace() {
        let generator = AmbientDtsGenerator::default();
        let tools = vec![
            AmbientToolSpec {
                name: "read_file".to_string(),
                description: "Read file from disk".to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "path": { "type": "string" }
                    },
                    "required": ["path"]
                }),
                output_schema: None,
            },
            AmbientToolSpec {
                name: "mcp__git__status".to_string(),
                description: "Get repository status".to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {}
                }),
                output_schema: None,
            },
        ];

        let dts = generator.generate_ambient_dts(&tools);

        assert!(dts.contains("declare function print(...args: any[]): void;"));
        assert!(dts.contains("declare function yieldToUser(prompt: string): Promise<string>;"));
        assert!(dts.contains("declare const store:"));
        assert!(dts.contains("declare namespace tools {"));
        assert!(dts.contains("function read_file(args: ReadFileArgs): Promise<any>;"));
        assert!(dts.contains("function mcp__git__status(args: McpGitStatusArgs): Promise<any>;"));
    }

    #[test]
    fn test_execution_contract_prompt_format() {
        let generator = AmbientDtsGenerator::default();
        let tools = vec![AmbientToolSpec {
            name: "execute_command".to_string(),
            description: "Run shell command in sandbox".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "cmd": { "type": "string" }
                }
            }),
            output_schema: None,
        }];

        let contract = generator.generate_execution_contract(&tools);
        assert!(contract.contains("## Code Mode Execution Protocol"));
        assert!(contract.contains("declare namespace tools {"));
        assert!(contract.contains("function execute_command"));
    }
}
