//! Self-healing tool input repair engine automatically correcting malformed JSON, unescaped characters, and schema mismatches.
//!
//! Frontier models (DeepSeek R1, Llama 3.3, Nemotron, GPT, etc.) frequently emit malformed
//! tool arguments:
//! - Markdown-fenced JSON blocks (e.g. ````json { ... } ````)
//! - Stringified root or nested JSON (e.g. `args = "{\"command\": \"ls\"}"`)
//! - String numbers or booleans where schema expects primitives (e.g. `{"timeout_seconds": "30", "recursive": "true"}`)
//! - Stringified arrays (e.g. `{"paths": "[\"src/main.rs\"]"}`)
//! - Null placeholders for optional fields (e.g. `{"offset": null}`)
//! - Unwanted wrapper envelopes (e.g. `{"arguments": { ... }}` or `{"parameters": { ... }}`)
//!
//! This module transparently inspects, normalizes, and repairs tool inputs before deserialization
//! and execution, preventing avoidable agent turn failures.

use anyhow::{Context, Result};
use serde_json::{Map, Value, json};

const MAX_REPAIR_DEPTH: usize = 6;

/// Sanitizes and repairs a raw tool argument string into a canonical `serde_json::Value`.
pub fn repair_tool_args(raw_args: &str, schema: Option<&Value>) -> Result<Value> {
    let cleaned = clean_raw_string(raw_args);

    // 1. Initial JSON parsing attempt
    let mut parsed: Value = match serde_json::from_str(&cleaned) {
        Ok(v) => v,
        Err(_) => {
            // Attempt trailing-comma cleanup or extracting outermost bracketed object/array
            let fallback_cleaned = clean_trailing_commas_and_syntax(&cleaned);
            match serde_json::from_str(&fallback_cleaned) {
                Ok(v) => v,
                Err(_) => {
                    // Try extracting innermost { ... }
                    if let Some(extracted) = extract_outermost_json(&cleaned) {
                        serde_json::from_str(&extracted).with_context(|| {
                            format!("Failed to parse tool arguments JSON: `{raw_args}`")
                        })?
                    } else {
                        // If it's an unquoted plain string and the tool takes a single string parameter
                        if let Some(s) = schema {
                            if let Some(repaired) = attempt_single_string_fallback(&cleaned, s) {
                                repaired
                            } else {
                                anyhow::bail!("Failed to parse tool arguments JSON: `{raw_args}`");
                            }
                        } else {
                            anyhow::bail!("Failed to parse tool arguments JSON: `{raw_args}`");
                        }
                    }
                }
            }
        }
    };

    // 2. If parsed value is a string that itself contains JSON, unwrap it
    if let Value::String(inner_str) = &parsed {
        let inner_trimmed = inner_str.trim();
        if ((inner_trimmed.starts_with('{') && inner_trimmed.ends_with('}'))
            || (inner_trimmed.starts_with('[') && inner_trimmed.ends_with(']')))
            && let Ok(inner_val) = serde_json::from_str::<Value>(inner_trimmed)
        {
            parsed = inner_val;
        }
    }

    // 3. Unwrap known outer envelopes like {"arguments": {...}}, {"parameters": {...}}, {"input": {...}}
    unwrap_outer_envelopes(&mut parsed, schema);

    // 4. Parameter alias and property name normalization
    normalize_property_aliases(&mut parsed, schema);

    // 5. Recursive schema-directed and heuristic type repair
    repair_json_value(&mut parsed, schema, 0);

    Ok(parsed)
}

/// Cleans markdown code fences, leading/trailing backticks, and whitespace.
pub fn clean_raw_string(raw: &str) -> String {
    let mut s = raw.trim();

    // Strip markdown code fences ```json ... ``` or ``` ... ```
    if s.starts_with("```") {
        if let Some(first_newline) = s.find('\n') {
            let after_fence = &s[first_newline + 1..];
            if let Some(end_fence) = after_fence.rfind("```") {
                s = after_fence[..end_fence].trim();
            } else {
                s = after_fence.trim();
            }
        } else if s.ends_with("```") && s.len() >= 6 {
            s = s[3..s.len() - 3].trim();
        }
    }

    // Strip single backticks `...`
    if s.starts_with('`') && s.ends_with('`') && s.len() >= 2 {
        s = s[1..s.len() - 1].trim();
    }

    // Unescape if the entire payload is a double-quoted JSON string literal
    if s.starts_with('"')
        && s.ends_with('"')
        && s.len() >= 2
        && let Ok(unescaped) = serde_json::from_str::<String>(s)
    {
        let trimmed = unescaped.trim();
        if trimmed.starts_with('{') || trimmed.starts_with('[') {
            return trimmed.to_string();
        }
    }

    s.to_string()
}

/// Cleans trailing commas (e.g. `{"foo": "bar",}`) before closing brackets.
fn clean_trailing_commas_and_syntax(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let chars: Vec<char> = s.chars().collect();
    let len = chars.len();
    let mut i = 0;
    let mut in_str = false;
    let mut escape = false;

    while i < len {
        let c = chars[i];
        if in_str {
            if escape {
                escape = false;
            } else if c == '\\' {
                escape = true;
            } else if c == '"' {
                in_str = false;
            }
            out.push(c);
            i += 1;
            continue;
        }

        if c == '"' {
            in_str = true;
            out.push(c);
            i += 1;
            continue;
        }

        if c == ',' {
            // Look ahead for closing brace or bracket
            let mut j = i + 1;
            while j < len && chars[j].is_whitespace() {
                j += 1;
            }
            if j < len && (chars[j] == '}' || chars[j] == ']') {
                // Skip the trailing comma
                i += 1;
                continue;
            }
        }

        out.push(c);
        i += 1;
    }

    out
}

/// Attempts to find the first `{` and matching last `}`.
fn extract_outermost_json(s: &str) -> Option<String> {
    let start_brace = s.find('{');
    let start_bracket = s.find('[');

    let (start_idx, open_char, close_char) = match (start_brace, start_bracket) {
        (Some(b), Some(k)) => {
            if b < k {
                (b, '{', '}')
            } else {
                (k, '[', ']')
            }
        }
        (Some(b), None) => (b, '{', '}'),
        (None, Some(k)) => (k, '[', ']'),
        (None, None) => return None,
    };

    let mut depth = 0;
    let mut in_str = false;
    let mut escape = false;

    let chars = s[start_idx..].char_indices();
    for (idx, c) in chars {
        if in_str {
            if escape {
                escape = false;
            } else if c == '\\' {
                escape = true;
            } else if c == '"' {
                in_str = false;
            }
            continue;
        }

        if c == '"' {
            in_str = true;
            continue;
        }

        if c == open_char {
            depth += 1;
        } else if c == close_char {
            depth -= 1;
            if depth == 0 {
                return Some(s[start_idx..=start_idx + idx].to_string());
            }
        }
    }

    None
}

/// Fallback if the model provided a raw single string for a tool that requires only one string parameter.
fn attempt_single_string_fallback(raw: &str, schema: &Value) -> Option<Value> {
    let props = schema.get("properties")?.as_object()?;
    let required = schema.get("required").and_then(|r| r.as_array());

    // If there is exactly one required property of type string
    if let Some(req_arr) = required
        && req_arr.len() == 1
    {
        let key = req_arr[0].as_str()?;
        let prop_type = props.get(key)?.get("type")?.as_str()?;
        if prop_type == "string" {
            return Some(json!({ key: raw }));
        }
    }

    None
}

/// Unwraps common wrappers like `{"arguments": { ... }}` if the tool schema does not expect that wrapper.
fn unwrap_outer_envelopes(val: &mut Value, schema: Option<&Value>) {
    if let Value::Object(map) = val {
        let envelope_keys = ["arguments", "parameters", "args", "input"];
        for key in &envelope_keys {
            if map.len() == 1 && map.contains_key(*key) {
                // If schema does not define this key as a property, unwrap
                let schema_expects_key = schema
                    .and_then(|s| s.get("properties"))
                    .and_then(|p| p.get(*key))
                    .is_some();

                if !schema_expects_key && let Some(inner) = map.remove(*key) {
                    if let Value::String(s) = inner {
                        if let Ok(decoded) = serde_json::from_str::<Value>(&s) {
                            *val = decoded;
                            return;
                        }
                    } else if inner.is_object() || inner.is_array() {
                        *val = inner;
                        return;
                    }
                }
            }
        }
    }
}

fn camel_to_snake(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 4);
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() {
            if i > 0 {
                out.push('_');
            }
            for lc in c.to_lowercase() {
                out.push(lc);
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn resolve_synonym<'a>(
    alias: &str,
    expected: &std::collections::HashSet<&'a str>,
) -> Option<&'a str> {
    let lower = alias.to_ascii_lowercase();
    let lower_str = lower.as_str();

    let candidates: &[(&str, &[&str])] = &[
        (
            "command",
            &[
                "cmd",
                "shell_command",
                "exec",
                "script",
                "bash_command",
                "run",
                "c",
            ],
        ),
        (
            "path",
            &[
                "file",
                "filepath",
                "file_path",
                "filename",
                "file_name",
                "target_path",
                "target_file",
                "path_to_file",
                "dir",
                "directory",
                "location",
                "p",
            ],
        ),
        (
            "content",
            &[
                "contents",
                "code",
                "text",
                "body",
                "file_content",
                "data",
                "payload",
            ],
        ),
        (
            "contents",
            &[
                "content",
                "code",
                "text",
                "body",
                "file_content",
                "data",
                "payload",
            ],
        ),
        (
            "query",
            &[
                "pattern",
                "search_term",
                "regex",
                "search",
                "term",
                "q",
                "keyword",
                "filter",
            ],
        ),
        (
            "patch",
            &["diff", "patch_text", "hunk", "patch_content", "code_patch"],
        ),
        (
            "old_string",
            &[
                "old_text",
                "search_string",
                "target",
                "find",
                "find_text",
                "before",
                "search",
                "original",
            ],
        ),
        (
            "new_string",
            &[
                "new_text",
                "replace_string",
                "replacement",
                "replace",
                "after",
                "updated",
            ],
        ),
        (
            "todos",
            &["tasks", "items", "task_list", "todo_list", "todo"],
        ),
        ("process_id", &["proc_id", "pid", "session_id", "id"]),
        ("agent_id", &["id", "target", "target_agent", "agent"]),
        (
            "message",
            &["prompt", "text", "content", "input", "instruction", "msg"],
        ),
        (
            "timeout_seconds",
            &["timeout", "timeout_sec", "timeout_s", "timeout_secs"],
        ),
        (
            "yield_time_ms",
            &["yield_ms", "yield_time", "yield_timeout", "yield"],
        ),
        (
            "start_line",
            &[
                "start",
                "from_line",
                "line_start",
                "line_start_number",
                "offset",
            ],
        ),
        (
            "end_line",
            &["end", "to_line", "line_end", "line_end_number", "limit"],
        ),
        ("url", &["uri", "link", "address", "endpoint"]),
        ("target_ref", &["element_ref", "ref", "target", "selector"]),
        ("prompt", &["instruction", "query", "message", "text"]),
    ];

    for &(canon, aliases) in candidates {
        if expected.contains(canon) && (lower_str == canon || aliases.contains(&lower_str)) {
            return expected.get(canon).copied();
        }
    }

    None
}

/// Normalizes parameter name aliases (e.g. `cmd` -> `command`, `file` -> `path`, `pattern` -> `query`)
/// according to the properties expected by the tool's JSON schema.
pub fn normalize_property_aliases(val: &mut Value, schema: Option<&Value>) {
    let Some(schema) = schema else { return };
    let Some(props) = schema.get("properties").and_then(|p| p.as_object()) else {
        return;
    };
    let expected_keys: std::collections::HashSet<&str> = props.keys().map(String::as_str).collect();

    if let Value::Object(map) = val {
        let current_keys: Vec<String> = map.keys().cloned().collect();

        // If schema has exactly one required key and map has exactly one key that is unexpected, remap it
        if let Some(req_arr) = schema.get("required").and_then(|r| r.as_array())
            && req_arr.len() == 1
            && current_keys.len() == 1
            && let Some(req_key) = req_arr[0].as_str()
            && !expected_keys.contains(current_keys[0].as_str())
            && !map.contains_key(req_key)
            && let Some(v) = map.remove(&current_keys[0])
        {
            map.insert(req_key.to_string(), v);
            return;
        }

        // Iterate keys and apply camelCase-to-snake_case and synonym aliases
        for k in current_keys {
            if expected_keys.contains(k.as_str()) {
                continue;
            }

            // Check camelCase to snake_case
            let snake = camel_to_snake(&k);
            if expected_keys.contains(snake.as_str()) && !map.contains_key(&snake) {
                if let Some(v) = map.remove(&k) {
                    map.insert(snake, v);
                }
                continue;
            }

            // Synonym lookup
            if let Some(target_key) = resolve_synonym(&k, &expected_keys)
                && !map.contains_key(target_key)
                && let Some(v) = map.remove(&k)
            {
                map.insert(target_key.to_string(), v);
            }
        }
    }
}

/// Recursively traverses and repairs JSON values according to the schema rules and heuristics.
pub fn repair_json_value(val: &mut Value, schema: Option<&Value>, depth: usize) {
    if depth > MAX_REPAIR_DEPTH {
        return;
    }

    match val {
        Value::Object(map) => {
            let props = schema
                .and_then(|s| s.get("properties"))
                .and_then(|p| p.as_object());
            let required_keys: Vec<String> = schema
                .and_then(|s| s.get("required"))
                .and_then(|r| r.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default();

            // 1. Remove optional null fields that cause deserialization errors
            let mut keys_to_remove = Vec::new();
            for (k, v) in map.iter() {
                if v.is_null() && !required_keys.contains(k) {
                    keys_to_remove.push(k.clone());
                }
            }
            for k in keys_to_remove {
                map.remove(&k);
            }

            // 2. Repair each field according to its property schema (or heuristics)
            let mut repaired_entries = Map::new();
            for (k, mut v) in map.clone() {
                let prop_schema = props.and_then(|p| p.get(&k));
                repair_single_field(&mut v, prop_schema);
                repair_json_value(&mut v, prop_schema, depth + 1);
                repaired_entries.insert(k, v);
            }
            *map = repaired_entries;
        }
        Value::Array(arr) => {
            let item_schema = schema.and_then(|s| s.get("items"));
            for item in arr.iter_mut() {
                repair_single_field(item, item_schema);
                repair_json_value(item, item_schema, depth + 1);
            }
        }
        _ => {}
    }
}

/// Repairs a scalar or compound field based on its expected schema.
fn repair_single_field(val: &mut Value, prop_schema: Option<&Value>) {
    // If schema specifies an expected type
    if let Some(ps) = prop_schema
        && let Some(type_val) = ps.get("type")
        && let Some(expected_type) = type_val.as_str()
    {
        match expected_type {
            "boolean" => {
                if let Value::String(s) = val {
                    let trimmed = s.trim().to_lowercase();
                    if trimmed == "true" || trimmed == "1" || trimmed == "yes" {
                        *val = Value::Bool(true);
                    } else if trimmed == "false" || trimmed == "0" || trimmed == "no" {
                        *val = Value::Bool(false);
                    }
                } else if let Value::Number(n) = val
                    && let Some(i) = n.as_i64()
                {
                    *val = Value::Bool(i != 0);
                }
            }
            "integer" => {
                if let Value::String(s) = val {
                    let trimmed = s.trim();
                    if let Ok(i) = trimmed.parse::<i64>() {
                        *val = json!(i);
                    } else if let Ok(f) = trimmed.parse::<f64>() {
                        *val = json!(f as i64);
                    }
                }
            }
            "number" => {
                if let Value::String(s) = val {
                    let trimmed = s.trim();
                    if let Ok(f) = trimmed.parse::<f64>() {
                        *val = json!(f);
                    }
                }
            }
            "array" => {
                if let Value::String(s) = val {
                    let trimmed = s.trim();
                    if trimmed.starts_with('[')
                        && trimmed.ends_with(']')
                        && let Ok(arr) = serde_json::from_str::<Value>(trimmed)
                        && arr.is_array()
                    {
                        *val = arr;
                        return;
                    }
                    // Single scalar -> [scalar]
                    *val = json!([trimmed]);
                }
            }
            "object" => {
                if let Value::String(s) = val {
                    let trimmed = s.trim();
                    if trimmed.starts_with('{')
                        && trimmed.ends_with('}')
                        && let Ok(obj) = serde_json::from_str::<Value>(trimmed)
                        && obj.is_object()
                    {
                        *val = obj;
                    }
                }
            }
            _ => {}
        }
    }

    // Heuristic repair for stringified objects/arrays even without schema
    if let Value::String(s) = val {
        let trimmed = s.trim();
        if ((trimmed.starts_with('{') && trimmed.ends_with('}'))
            || (trimmed.starts_with('[') && trimmed.ends_with(']')))
            && let Ok(parsed) = serde_json::from_str::<Value>(trimmed)
            && (parsed.is_object() || parsed.is_array())
        {
            *val = parsed;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clean_raw_string_markdown_fences() {
        let input = "```json\n{\n  \"command\": \"cargo check\"\n}\n```";
        let cleaned = clean_raw_string(input);
        assert_eq!(cleaned, "{\n  \"command\": \"cargo check\"\n}");
    }

    #[test]
    fn test_clean_raw_string_escaped_json() {
        let input = r#""{\"command\": \"ls -la\"}""#;
        let cleaned = clean_raw_string(input);
        assert_eq!(cleaned, r#"{"command": "ls -la"}"#);
    }

    #[test]
    fn test_repair_trailing_commas() {
        let input = r#"{"command": "cargo test", "timeout_seconds": 30,}"#;
        let repaired = repair_tool_args(input, None).unwrap();
        assert_eq!(repaired["command"], "cargo test");
        assert_eq!(repaired["timeout_seconds"], 30);
    }

    #[test]
    fn test_repair_string_numbers_and_booleans() {
        let schema = json!({
            "type": "object",
            "properties": {
                "command": { "type": "string" },
                "timeout_seconds": { "type": "integer" },
                "recursive": { "type": "boolean" },
                "ratio": { "type": "number" }
            },
            "required": ["command"]
        });

        let input = r#"{
            "command": "find .",
            "timeout_seconds": "45",
            "recursive": "true",
            "ratio": "0.75"
        }"#;

        let repaired = repair_tool_args(input, Some(&schema)).unwrap();
        assert_eq!(repaired["command"], "find .");
        assert_eq!(repaired["timeout_seconds"], 45);
        assert_eq!(repaired["recursive"], true);
        assert_eq!(repaired["ratio"], 0.75);
    }

    #[test]
    fn test_repair_unwanted_envelope() {
        let input = r#"{
            "arguments": {
                "command": "git status"
            }
        }"#;

        let repaired = repair_tool_args(input, None).unwrap();
        assert_eq!(repaired["command"], "git status");
    }

    #[test]
    fn test_repair_stringified_nested_json() {
        let input = r#"{
            "meta": "{\"author\": \"antigravity\", \"version\": 1}"
        }"#;

        let repaired = repair_tool_args(input, None).unwrap();
        assert_eq!(repaired["meta"]["author"], "antigravity");
        assert_eq!(repaired["meta"]["version"], 1);
    }

    #[test]
    fn test_repair_removes_optional_nulls() {
        let schema = json!({
            "type": "object",
            "properties": {
                "path": { "type": "string" },
                "offset": { "type": "integer" }
            },
            "required": ["path"]
        });

        let input = r#"{
            "path": "src/main.rs",
            "offset": null
        }"#;

        let repaired = repair_tool_args(input, Some(&schema)).unwrap();
        assert_eq!(repaired["path"], "src/main.rs");
        assert!(repaired.get("offset").is_none());
    }

    #[test]
    fn test_repair_stringified_array() {
        let schema = json!({
            "type": "object",
            "properties": {
                "files": { "type": "array", "items": { "type": "string" } }
            }
        });

        let input = r#"{ "files": "[\"a.rs\", \"b.rs\"]" }"#;
        let repaired = repair_tool_args(input, Some(&schema)).unwrap();
        assert_eq!(repaired["files"], json!(["a.rs", "b.rs"]));
    }

    #[test]
    fn test_repair_parameter_aliases_and_camel_case() {
        let schema = json!({
            "type": "object",
            "properties": {
                "command": { "type": "string" },
                "timeout_seconds": { "type": "integer" }
            },
            "required": ["command"]
        });

        // Test cmd -> command and timeoutSeconds -> timeout_seconds
        let input = r#"{ "cmd": "cargo build", "timeoutSeconds": "30" }"#;
        let repaired = repair_tool_args(input, Some(&schema)).unwrap();
        assert_eq!(repaired["command"], "cargo build");
        assert_eq!(repaired["timeout_seconds"], 30);
        assert!(repaired.get("cmd").is_none());
        assert!(repaired.get("timeoutSeconds").is_none());
    }

    #[test]
    fn test_repair_file_path_alias() {
        let schema = json!({
            "type": "object",
            "properties": {
                "path": { "type": "string" },
                "content": { "type": "string" }
            },
            "required": ["path", "content"]
        });

        let input = r##"{ "file_path": "README.md", "contents": "# M31A" }"##;
        let repaired = repair_tool_args(input, Some(&schema)).unwrap();
        assert_eq!(repaired["path"], "README.md");
        assert_eq!(repaired["content"], "# M31A");
    }
}
