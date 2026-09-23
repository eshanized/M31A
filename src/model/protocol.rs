//! Layered tool-call protocol, argument extraction, truncation detection,
//! and constrained safe recovery (P3-B, MDL-01, MDL-03).

use serde_json::Value;

/// Classification of tool-call parameter status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolCallValidationOutcome {
    /// Valid JSON object matching expected parameters.
    Valid(Value),
    /// Truncated JSON arguments that were safely and deterministically recovered.
    Recovered {
        value: Value,
        repair_applied: String,
    },
    /// Irrecoverable syntax failure (unparseable JSON).
    SyntaxError {
        message: String,
        raw_snippet: String,
    },
    /// Truncated beyond safe recovery (e.g. cut off inside key or empty token).
    UnrecoverableTruncation {
        message: String,
        raw_snippet: String,
    },
}

/// Core protocol engine for robust model-to-tool argument transport.
pub struct ToolCallProtocol;

impl ToolCallProtocol {
    /// Process raw tool arguments through the layered protocol:
    /// Extraction -> Truncation Detection -> Safe Recovery -> Structural Validation.
    pub fn process_arguments(
        raw_args: &str,
        finish_reason: Option<&str>,
    ) -> ToolCallValidationOutcome {
        let trimmed = raw_args.trim();

        // 1. Check oversized arguments (> 5MB protection against memory exhaustion)
        if trimmed.len() > 5 * 1024 * 1024 {
            return ToolCallValidationOutcome::SyntaxError {
                message: format!(
                    "Tool arguments exceed 5MB safety limit ({} bytes)",
                    trimmed.len()
                ),
                raw_snippet: trimmed[..200].to_string(),
            };
        }

        // 2. Empty arguments handle as empty object
        if trimmed.is_empty() || trimmed == "{}" || trimmed == "null" {
            return ToolCallValidationOutcome::Valid(serde_json::json!({}));
        }

        // 3. Direct JSON parsing attempt
        if let Ok(parsed) = serde_json::from_str::<Value>(trimmed) {
            if parsed.is_object() {
                return ToolCallValidationOutcome::Valid(parsed);
            } else {
                return ToolCallValidationOutcome::SyntaxError {
                    message: format!(
                        "Expected JSON object for tool parameters, received: {}",
                        parsed
                    ),
                    raw_snippet: trimmed[..trimmed.len().min(120)].to_string(),
                };
            }
        }

        // 4. Truncation and structural defect detection
        let is_length_finish = finish_reason == Some("length");
        let has_unclosed_delimiter = Self::detect_unclosed_delimiters(trimmed);

        // 5. Constrained safe recovery attempt
        if (is_length_finish
            || has_unclosed_delimiter
            || trimmed.ends_with(',')
            || trimmed.ends_with(':'))
            && let Some((recovered_val, repair_desc)) = Self::attempt_safe_recovery(trimmed)
        {
            return ToolCallValidationOutcome::Recovered {
                value: recovered_val,
                repair_applied: repair_desc,
            };
        }

        // 6. Extraction from markdown code blocks or wrapping quotes
        if let Some(extracted) = Self::extract_from_markdown_or_quotes(trimmed)
            && let Ok(parsed) = serde_json::from_str::<Value>(&extracted)
            && parsed.is_object()
        {
            return ToolCallValidationOutcome::Recovered {
                value: parsed,
                repair_applied: "Extracted JSON from markdown code block or wrapper".to_string(),
            };
        }

        // 7. Irrecoverable syntax failure
        let preview = if trimmed.len() > 150 {
            format!("{}...", &trimmed[..150])
        } else {
            trimmed.to_string()
        };

        if is_length_finish {
            ToolCallValidationOutcome::UnrecoverableTruncation {
                message: "Tool arguments were truncated by provider token limit and could not be safely repaired.".to_string(),
                raw_snippet: preview,
            }
        } else {
            ToolCallValidationOutcome::SyntaxError {
                message: "Malformed JSON arguments syntax.".to_string(),
                raw_snippet: preview,
            }
        }
    }

    /// Detect whether a JSON string has unclosed quotes, braces, or brackets.
    pub fn detect_unclosed_delimiters(text: &str) -> bool {
        let mut in_string = false;
        let mut escaped = false;
        let mut stack = Vec::new();

        for ch in text.chars() {
            if escaped {
                escaped = false;
                continue;
            }
            if ch == '\\' {
                escaped = true;
                continue;
            }
            if ch == '"' {
                in_string = !in_string;
                continue;
            }
            if in_string {
                continue;
            }
            match ch {
                '{' => stack.push('}'),
                '[' => stack.push(']'),
                '}' | ']' => {
                    if let Some(expected) = stack.pop() {
                        if expected != ch {
                            return true;
                        }
                    } else {
                        return true;
                    }
                }
                _ => {}
            }
        }

        in_string || !stack.is_empty()
    }

    /// Deterministic safe recovery of truncated JSON structures.
    /// Closes open strings and balancing delimiters without inventing missing semantic keys or values.
    pub fn attempt_safe_recovery(raw: &str) -> Option<(Value, String)> {
        let mut s = raw.trim().to_string();
        if s.is_empty() {
            return None;
        }

        // Strip trailing incomplete escape sequence
        if s.ends_with('\\') {
            s.pop();
        }

        let mut in_string = false;
        let mut escaped = false;
        let mut delimiter_stack = Vec::new();

        for ch in s.chars() {
            if escaped {
                escaped = false;
                continue;
            }
            if ch == '\\' {
                escaped = true;
                continue;
            }
            if ch == '"' {
                in_string = !in_string;
                continue;
            }
            if in_string {
                continue;
            }
            match ch {
                '{' => delimiter_stack.push('}'),
                '[' => delimiter_stack.push(']'),
                '}' | ']' => {
                    delimiter_stack.pop();
                }
                _ => {}
            }
        }

        let mut repairs = Vec::new();

        // 1. Close unclosed string literal
        if in_string {
            s.push('"');
            repairs.push("closed unclosed string quote");
        }

        // 2. Clean trailing trailing commas or incomplete key-values (e.g. `,` or `,"foo":` without value)
        let trimmed_end = s.trim_end();
        if let Some(stripped) = trimmed_end.strip_suffix(',') {
            s = stripped.to_string();
            repairs.push("removed trailing comma");
        } else if trimmed_end.ends_with(':') {
            // Cut off at property name: remove `,"key":` or `{"key":`
            if let Some(quote_idx) = s.rfind('"')
                && let Some(prev_quote) = s[..quote_idx].rfind('"')
                && let Some(comma_or_brace) = s[..prev_quote].rfind([',', '{'])
            {
                let is_brace = s.as_bytes()[comma_or_brace] == b'{';
                s.truncate(if is_brace {
                    comma_or_brace + 1
                } else {
                    comma_or_brace
                });
                repairs.push("trimmed incomplete trailing property");
            }
        }

        // 3. Close open delimiters in reverse stack order
        while let Some(closing) = delimiter_stack.pop() {
            s.push(closing);
            repairs.push("closed balancing delimiter");
        }

        if let Ok(parsed) = serde_json::from_str::<Value>(&s)
            && parsed.is_object()
        {
            return Some((parsed, repairs.join(", ")));
        }

        None
    }

    /// Extract JSON string from markdown code blocks or surrounding text.
    fn extract_from_markdown_or_quotes(text: &str) -> Option<String> {
        let trimmed = text.trim();
        // Check ```json ... ```
        if let Some(start) = trimmed.find("```json") {
            let after = &trimmed[start + 7..];
            if let Some(end) = after.find("```") {
                return Some(after[..end].trim().to_string());
            }
        }
        // Check ``` ... ```
        if let Some(start) = trimmed.find("```") {
            let after = &trimmed[start + 3..];
            if let Some(end) = after.find("```") {
                return Some(after[..end].trim().to_string());
            }
        }
        // Check outer braces
        if let (Some(first_brace), Some(last_brace)) = (trimmed.find('{'), trimmed.rfind('}'))
            && first_brace < last_brace
        {
            return Some(trimmed[first_brace..=last_brace].to_string());
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_json() {
        let raw = r#"{"path": "src/main.rs", "old_content": "foo", "new_content": "bar"}"#;
        let outcome = ToolCallProtocol::process_arguments(raw, None);
        match outcome {
            ToolCallValidationOutcome::Valid(val) => {
                assert_eq!(val["path"], "src/main.rs");
                assert_eq!(val["new_content"], "bar");
            }
            other => panic!("Expected Valid, got {:?}", other),
        }
    }

    #[test]
    fn test_empty_or_null_json() {
        assert!(matches!(
            ToolCallProtocol::process_arguments("", None),
            ToolCallValidationOutcome::Valid(_)
        ));
        assert!(matches!(
            ToolCallProtocol::process_arguments("{}", None),
            ToolCallValidationOutcome::Valid(_)
        ));
        assert!(matches!(
            ToolCallProtocol::process_arguments("null", None),
            ToolCallValidationOutcome::Valid(_)
        ));
    }

    #[test]
    fn test_truncated_unclosed_string_recovery() {
        let raw = r#"{"path": "src/main.rs", "new_content": "hello world"#;
        let outcome = ToolCallProtocol::process_arguments(raw, None);
        match outcome {
            ToolCallValidationOutcome::Recovered {
                value,
                repair_applied,
            } => {
                assert_eq!(value["path"], "src/main.rs");
                assert_eq!(value["new_content"], "hello world");
                assert!(repair_applied.contains("closed unclosed string quote"));
            }
            other => panic!("Expected Recovered, got {:?}", other),
        }
    }

    #[test]
    fn test_truncated_trailing_comma_recovery() {
        let raw = r#"{"path": "src/main.rs", "old_content": "foo","#;
        let outcome = ToolCallProtocol::process_arguments(raw, None);
        match outcome {
            ToolCallValidationOutcome::Recovered {
                value,
                repair_applied,
            } => {
                assert_eq!(value["path"], "src/main.rs");
                assert_eq!(value["old_content"], "foo");
                assert!(repair_applied.contains("removed trailing comma"));
            }
            other => panic!("Expected Recovered, got {:?}", other),
        }
    }

    #[test]
    fn test_markdown_codeblock_extraction() {
        let raw = "```json\n{\"path\": \"test.rs\", \"content\": \"123\"}\n```";
        let outcome = ToolCallProtocol::process_arguments(raw, None);
        match outcome {
            ToolCallValidationOutcome::Recovered { value, .. } => {
                assert_eq!(value["path"], "test.rs");
                assert_eq!(value["content"], "123");
            }
            other => panic!("Expected Recovered, got {:?}", other),
        }
    }

    #[test]
    fn test_syntax_error_unrecoverable() {
        let raw = r#"{"path": not_valid_json_at_all"#;
        let outcome = ToolCallProtocol::process_arguments(raw, None);
        assert!(matches!(
            outcome,
            ToolCallValidationOutcome::SyntaxError { .. }
        ));
    }

    #[test]
    fn test_token_limit_length_truncation() {
        // Recoverable: incomplete trailing property dropped safely
        let recoverable = r#"{"path": "src/lib.rs", "content": "#;
        let outcome = ToolCallProtocol::process_arguments(recoverable, Some("length"));
        assert!(matches!(
            outcome,
            ToolCallValidationOutcome::Recovered { .. }
        ));

        // Unrecoverable: cut off mid-key with no parseable property
        let unrecoverable = r#"{"incomplete_ke"#;
        let outcome = ToolCallProtocol::process_arguments(unrecoverable, Some("length"));
        assert!(matches!(
            outcome,
            ToolCallValidationOutcome::UnrecoverableTruncation { .. }
        ));
    }
}
