//! IPC wire protocol and message types for Code Mode host-to-isolate communication.
//!
//! Defines execution requests, tool invocation RPCs, log streams, and script return values.

use crate::json_schema::{MCP_TYPESCRIPT_PREAMBLE, render_json_schema_to_typescript};
use m31a_core::types::ToolDefinition;
use serde::{Deserialize, Serialize};

pub const PUBLIC_TOOL_NAME: &str = "exec";
pub const WAIT_TOOL_NAME: &str = "wait";
pub const CODE_MODE_PRAGMA_PREFIX: &str = "// @exec:";
pub const DEFAULT_EXEC_YIELD_TIME_MS: u64 = 30000;
pub const DEFAULT_MAX_OUTPUT_TOKENS: usize = 10000;
const MAX_JS_SAFE_INTEGER: u64 = (1_u64 << 53) - 1;

pub const TOOL_SEARCH_GUIDANCE: &str = r#"Use `await tools.tool_search({query: "...", limit: 8})` for BM25-ranked discovery of deferred tools (up to `limit` matches; 8 by default). It returns `{tools: [{name, description}]}` with callable names and full TypeScript declarations. Print results with `text()` and inspect unfamiliar declarations before calling `tools[result.name](arguments)` in a later execution."#;

const EXEC_DESCRIPTION_TEMPLATE: &str = r#"Run JavaScript code to orchestrate/compose tool calls
- Evaluates the provided JavaScript code in an isolated execution sandbox as an async module.
- All nested tools are available on the global `tools` object, for example `await tools.exec_command(...)`. Tool names are exposed as normalized JavaScript identifiers, for example `await tools.mcp__ologs__get_profile(...)`.
- Nested tool methods take either a string or an object as their input argument.
- Nested tools return either an object or a string, based on the description.
- Runs raw JavaScript -- no Node, no file system, no network access, no console.
- Accepts raw JavaScript source text, not JSON, quoted strings, or markdown code fences.
- You may optionally start the tool input with a first-line pragma like `// @exec: {"yield_time_ms": 10000, "max_output_tokens": 1000}`.
- `yield_time_ms` asks `exec` to yield early if the script is still running. Defaults to {{ default_exec_yield_time_ms }} ms.
- `max_output_tokens` sets the token budget for direct `exec` results. Defaults to 10000 tokens.
- When the JS code is fully evaluated, unawaited promises are silently discarded.

- Global helpers:
- `exit()`: Immediately ends the current script successfully (like an early return from the top level).
- `text(value: string | number | boolean | undefined | null)`: Appends a text item. Non-string values are stringified with `JSON.stringify(...)` when possible.
- `image(imageUrlOrItem: string | { image_url: string } | ImageContent)`: Appends an image item. `image_url` should be a base64-encoded `data:` URL.
- `audio(audioUrlOrItem: string | { audio_url: string } | AudioContent)`: Appends an audio item. `audio_url` should be a base64-encoded `data:` URL.
- `store(key: string, value: any)`: stores a serializable value under a string key for later `exec` calls in the same session.
- `load(key: string)`: returns the stored value for a string key, or `undefined` if it is missing.
- `notify(value: string | number | boolean | undefined | null)`: immediately injects an extra `custom_tool_call_output` for the current `exec` call.
- `setTimeout(callback: () => void, delayMs?: number)`: schedules a callback to run later.
- `clearTimeout(timeoutId?: number)`: cancels a timeout created by `setTimeout`.
- `ALL_TOOLS`: metadata for the enabled nested tools as `{ name, description }` entries.
- `yield_control()`: yields the accumulated output to the model immediately while the script keeps running."#;

const WAIT_DESCRIPTION_TEMPLATE: &str = r#"- Use `wait` only after `exec` returns `Script running with cell ID ...`.
- `cell_id` identifies the running `exec` cell to resume.
- `yield_time_ms` controls how long to wait for more output before yielding again. Defaults to 10000 ms.
- `max_tokens` limits how much new output this wait call returns. Defaults to 10000 tokens.
- `terminate: true` stops the running cell; false or omitted waits for output.
- `wait` returns only the new output since the last yield, or the final completion or termination result for that cell.
- If the cell is still running, `wait` may yield again with the same `cell_id`.
- If the cell has already finished, `wait` returns the completed result and closes the cell."#;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CodeModeToolKind {
    Function,
    Freeform,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct EnabledToolMetadata {
    pub tool_name: String,
    pub global_name: String,
    pub description: String,
    pub kind: CodeModeToolKind,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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

impl Default for ExecPragma {
    fn default() -> Self {
        Self {
            yield_time_ms: DEFAULT_EXEC_YIELD_TIME_MS,
            max_output_tokens: DEFAULT_MAX_OUTPUT_TOKENS,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedExecSource {
    pub code: String,
    pub yield_time_ms: u64,
    pub max_output_tokens: usize,
}

pub fn normalize_code_mode_identifier(tool_key: &str) -> String {
    let mut identifier = String::new();

    for (index, ch) in tool_key.chars().enumerate() {
        let is_valid = if index == 0 {
            ch == '_' || ch == '$' || ch.is_ascii_alphabetic()
        } else {
            ch == '_' || ch == '$' || ch.is_ascii_alphanumeric()
        };

        if is_valid {
            identifier.push(ch);
        } else {
            identifier.push('_');
        }
    }

    if identifier.is_empty() {
        "_".to_string()
    } else {
        identifier
    }
}

pub fn parse_exec_source(input: &str) -> Result<ParsedExecSource, String> {
    if input.trim().is_empty() {
        return Err("exec expects non-empty JavaScript source text".to_string());
    }

    let mut yield_time_ms = DEFAULT_EXEC_YIELD_TIME_MS;
    let mut max_output_tokens = DEFAULT_MAX_OUTPUT_TOKENS;

    let mut lines = input.splitn(2, '\n');
    let first_line = lines.next().unwrap_or_default();
    let rest = lines.next().unwrap_or_default();
    let trimmed = first_line.trim_start();

    if let Some(pragma_str) = trimmed.strip_prefix(CODE_MODE_PRAGMA_PREFIX) {
        if rest.trim().is_empty() {
            return Err(
                "exec pragma must be followed by JavaScript source on subsequent lines".to_string(),
            );
        }

        let directive = pragma_str.trim();
        if !directive.is_empty()
            && let Ok(pragma) = serde_json::from_str::<ExecPragma>(directive)
        {
            if pragma.yield_time_ms > MAX_JS_SAFE_INTEGER {
                return Err("exec pragma yield_time_ms exceeds safe integer limit".to_string());
            }
            yield_time_ms = pragma.yield_time_ms;
            max_output_tokens = pragma.max_output_tokens;
        }
        return Ok(ParsedExecSource {
            code: rest.to_string(),
            yield_time_ms,
            max_output_tokens,
        });
    }

    Ok(ParsedExecSource {
        code: input.to_string(),
        yield_time_ms,
        max_output_tokens,
    })
}

pub fn build_exec_tool_description(
    tools: &[ToolDefinition],
    default_exec_yield_time_ms: u64,
) -> String {
    let mut output = EXEC_DESCRIPTION_TEMPLATE.replace(
        "{{ default_exec_yield_time_ms }}",
        &default_exec_yield_time_ms.to_string(),
    );

    output.push_str("\n\n---\n### Available Nested Tools on `tools` Object:\n\n```ts\n");
    output.push_str(MCP_TYPESCRIPT_PREAMBLE);
    output.push_str("\n\ndeclare const tools: {\n");

    for tool in tools {
        if tool.function.name == PUBLIC_TOOL_NAME || tool.function.name == WAIT_TOOL_NAME {
            continue;
        }
        let ident = normalize_code_mode_identifier(&tool.function.name);
        let param_type = render_json_schema_to_typescript(&tool.function.parameters);
        output.push_str(&format!(
            "  /**\n   * {}\n   */\n",
            tool.function.description.replace('\n', "\n   * ")
        ));
        output.push_str(&format!(
            "  {ident}(args: {param_type}): Promise<string>;\n\n"
        ));
    }

    output.push_str("};\n```\n");
    output
}

pub fn build_wait_tool_description() -> String {
    WAIT_DESCRIPTION_TEMPLATE.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use m31a_core::types::FunctionDefinition;
    use serde_json::json;

    #[test]
    fn test_normalize_identifier() {
        assert_eq!(normalize_code_mode_identifier("read_file"), "read_file");
        assert_eq!(
            normalize_code_mode_identifier("mcp::github::create_issue"),
            "mcp__github__create_issue"
        );
        assert_eq!(normalize_code_mode_identifier("123invalid"), "_23invalid");
    }

    #[test]
    fn test_parse_exec_source_with_pragma() {
        let input =
            "// @exec: {\"yield_time_ms\": 5000, \"max_output_tokens\": 4000}\nconsole.log(123);";
        let parsed = parse_exec_source(input).unwrap();
        assert_eq!(parsed.yield_time_ms, 5000);
        assert_eq!(parsed.max_output_tokens, 4000);
        assert_eq!(parsed.code, "console.log(123);");
    }

    #[test]
    fn test_build_exec_tool_description() {
        let tools = vec![ToolDefinition {
            tool_type: "function".to_string(),
            function: FunctionDefinition {
                name: "exec_command".to_string(),
                description: "Runs command in sandbox".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": { "command": { "type": "string" } },
                    "required": ["command"]
                }),
            },
        }];

        let desc = build_exec_tool_description(&tools, 30000);
        assert!(desc.contains("exec_command(args:"));
        assert!(desc.contains("command: string;"));
    }
}
