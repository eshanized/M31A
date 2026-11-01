//! Autonomous self-healing diagnostic engine for compiler errors and test failures.
//!
//! Parses raw compiler output and runtime stack traces across Rust, Python, TypeScript, and Go.
//! Extracts structured diagnostics (file, line, column, error code, message, compiler suggestion)
//! to enable targeted self-correction loops without manual user intervention.

use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum DiagnosticSeverity {
    Error,
    Warning,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Diagnostic {
    pub file_path: PathBuf,
    pub line: usize,
    pub column: Option<usize>,
    pub severity: DiagnosticSeverity,
    pub code: Option<String>,
    pub message: String,
    pub snippet: Option<String>,
    pub suggested_fix: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SelfHealingReport {
    pub total_errors: usize,
    pub total_warnings: usize,
    pub diagnostics: Vec<Diagnostic>,
    pub prioritized_fix_target: Option<Diagnostic>,
}

/// Parses raw stderr/stdout text from compiler or test runner runs.
pub fn parse_compiler_diagnostics(output: &str) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    // 1. Check for Rust JSON diagnostics if available
    for line in output.lines() {
        if line.starts_with('{')
            && line.contains("\"$message_type\":\"diagnostic\"")
            && let Ok(val) = serde_json::from_str::<serde_json::Value>(line)
            && let Some(diag) = parse_rust_json_diagnostic(&val)
        {
            diagnostics.push(diag);
        }
    }

    if !diagnostics.is_empty() {
        return diagnostics;
    }

    // 2. Parse human-readable Rust, Python, TypeScript, and Go error formats
    let lines: Vec<&str> = output.lines().collect();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i].trim();

        // Rust format: "error[E0308]: mismatched types" followed by " --> crates/path/file.rs:12:5"
        if line.starts_with("error[")
            || (line.starts_with("error:") && !line.starts_with("error: could not compile"))
        {
            let code = if line.starts_with("error[") {
                line.split(']')
                    .next()
                    .and_then(|s| s.strip_prefix("error["))
                    .map(|s| s.to_string())
            } else {
                None
            };
            let message = line
                .split_once(':')
                .map(|(_, m)| m.trim().to_string())
                .unwrap_or_else(|| line.to_string());

            // Scan next lines for `--> path:line:col`
            let mut location = None;
            let mut snippet = None;
            let mut suggested_fix = None;

            for next_line in &lines[(i + 1)..(i + 15).min(lines.len())] {
                let next_trim = next_line.trim();
                if next_trim.starts_with("--> ") {
                    let loc_str = next_trim.strip_prefix("--> ").unwrap().trim();
                    location = parse_file_line_col(loc_str);
                } else if next_trim.starts_with("help: ") {
                    suggested_fix =
                        Some(next_trim.strip_prefix("help: ").unwrap().trim().to_string());
                } else if next_trim.contains('|') && !next_trim.starts_with("-->") {
                    if snippet.is_none() {
                        snippet = Some(next_line.to_string());
                    }
                } else if next_trim.starts_with("error[") || next_trim.starts_with("error:") {
                    break;
                }
            }

            if let Some((path, line_num, col)) = location {
                diagnostics.push(Diagnostic {
                    file_path: path,
                    line: line_num,
                    column: col,
                    severity: DiagnosticSeverity::Error,
                    code,
                    message,
                    snippet,
                    suggested_fix,
                });
            }
        }

        // TypeScript format: "src/index.ts:14:5 - error TS2304: Cannot find name 'foo'."
        if line.contains(" - error TS")
            && let Some((loc_part, err_part)) = line.split_once(" - error ")
        {
            let code = err_part.split(':').next().map(|s| s.trim().to_string());
            let message = err_part
                .split_once(':')
                .map(|(_, m)| m.trim().to_string())
                .unwrap_or_else(|| err_part.to_string());
            if let Some((path, line_num, col)) = parse_file_line_col(loc_part.trim()) {
                diagnostics.push(Diagnostic {
                    file_path: path,
                    line: line_num,
                    column: col,
                    severity: DiagnosticSeverity::Error,
                    code,
                    message,
                    snippet: None,
                    suggested_fix: None,
                });
            }
        }

        // Python format: "  File "test.py", line 42, in my_func" followed by error message
        if line.starts_with("File \"")
            && line.contains("\", line ")
            && let Some((file_part, rest)) = line
                .strip_prefix("File \"")
                .and_then(|s| s.split_once("\", line "))
        {
            let path = PathBuf::from(file_part);
            let line_num = rest
                .split(&[',', ' '][..])
                .next()
                .and_then(|n| n.parse::<usize>().ok())
                .unwrap_or(1);

            // Look ahead for the error line
            let mut err_msg = String::new();
            for next_line in &lines[(i + 1)..(i + 6).min(lines.len())] {
                let next = next_line.trim();
                if next.ends_with("Error")
                    || next.contains("Error: ")
                    || next.contains("Exception: ")
                {
                    err_msg = next.to_string();
                    break;
                }
            }

            if !err_msg.is_empty() {
                diagnostics.push(Diagnostic {
                    file_path: path,
                    line: line_num,
                    column: None,
                    severity: DiagnosticSeverity::Error,
                    code: err_msg.split(':').next().map(|s| s.to_string()),
                    message: err_msg,
                    snippet: None,
                    suggested_fix: None,
                });
            }
        }

        i += 1;
    }

    diagnostics
}

fn parse_file_line_col(loc: &str) -> Option<(PathBuf, usize, Option<usize>)> {
    let parts: Vec<&str> = loc.split(':').collect();
    if parts.len() >= 2 {
        let path = PathBuf::from(parts[0]);
        let line = parts[1].parse::<usize>().ok()?;
        let col = if parts.len() >= 3 {
            parts[2].parse::<usize>().ok()
        } else {
            None
        };
        Some((path, line, col))
    } else {
        None
    }
}

fn parse_rust_json_diagnostic(val: &serde_json::Value) -> Option<Diagnostic> {
    let message = val.get("message")?.as_str()?.to_string();
    let level = val.get("level")?.as_str()?;
    let severity = if level == "error" {
        DiagnosticSeverity::Error
    } else {
        DiagnosticSeverity::Warning
    };

    let code = val
        .get("code")
        .and_then(|c| c.get("code"))
        .and_then(|s| s.as_str())
        .map(|s| s.to_string());

    let spans = val.get("spans")?.as_array()?;
    let primary_span = spans.iter().find(|s| {
        s.get("is_primary")
            .and_then(|b| b.as_bool())
            .unwrap_or(false)
    })?;

    let file_name = primary_span.get("file_name")?.as_str()?;
    let line_start = primary_span.get("line_start")?.as_u64()? as usize;
    let col_start = primary_span
        .get("column_start")
        .and_then(|c| c.as_u64())
        .map(|c| c as usize);
    let snippet = primary_span
        .get("text")
        .and_then(|t| t.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|line| line.get("text").and_then(|s| s.as_str()))
                .collect::<Vec<_>>()
                .join("\n")
        });

    let suggested_fix = primary_span
        .get("suggested_replacement")
        .and_then(|s| s.as_str())
        .map(|s| s.to_string());

    Some(Diagnostic {
        file_path: PathBuf::from(file_name),
        line: line_start,
        column: col_start,
        severity,
        code,
        message,
        snippet,
        suggested_fix,
    })
}

/// Evaluates compiler diagnostics and constructs a prioritized self-healing report.
pub fn generate_self_healing_report(output: &str) -> SelfHealingReport {
    let diagnostics = parse_compiler_diagnostics(output);
    let total_errors = diagnostics
        .iter()
        .filter(|d| d.severity == DiagnosticSeverity::Error)
        .count();
    let total_warnings = diagnostics
        .iter()
        .filter(|d| d.severity == DiagnosticSeverity::Warning)
        .count();
    let prioritized_fix_target = diagnostics
        .iter()
        .find(|d| d.severity == DiagnosticSeverity::Error)
        .cloned();

    SelfHealingReport {
        total_errors,
        total_warnings,
        diagnostics,
        prioritized_fix_target,
    }
}

/// Formulates a prompt instruction specifically steering the model to heal the diagnosed error.
pub fn build_self_healing_prompt(
    report: &SelfHealingReport,
    workspace_root: &Path,
) -> Option<String> {
    let target = report.prioritized_fix_target.as_ref()?;
    let rel_path = target
        .file_path
        .strip_prefix(workspace_root)
        .unwrap_or(&target.file_path);

    let mut prompt = format!(
        "Autonomous Self-Healing Repair Target:\n\
         File: {}\n\
         Line: {}\n\
         Error: {}\n",
        rel_path.display(),
        target.line,
        target.message
    );

    if let Some(ref code) = target.code {
        prompt.push_str(&format!("Error Code: {}\n", code));
    }
    if let Some(ref snippet) = target.snippet {
        prompt.push_str(&format!("Offending Code:\n```\n{}\n```\n", snippet));
    }
    if let Some(ref fix) = target.suggested_fix {
        prompt.push_str(&format!("Compiler Suggestion: {}\n", fix));
    }

    prompt.push_str(
        "\nAction Required: Inspect the file around this line and apply a surgical fix using apply_patch to resolve the compiler error cleanly."
    );

    Some(prompt)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_rust_human_compiler_error() {
        let err = r#"
error[E0308]: mismatched types
  --> crates/m31a-core/src/runner.rs:42:15
   |
42 |         let id: u64 = "invalid_id";
   |                       ^^^^^^^^^^^^ expected `u64`, found `&str`
   |
help: consider parsing the string
"#;
        let diags = parse_compiler_diagnostics(err);
        assert_eq!(diags.len(), 1);
        let d = &diags[0];
        assert_eq!(d.file_path, PathBuf::from("crates/m31a-core/src/runner.rs"));
        assert_eq!(d.line, 42);
        assert_eq!(d.column, Some(15));
        assert_eq!(d.code.as_deref(), Some("E0308"));
        assert!(d.message.contains("mismatched types"));
    }

    #[test]
    fn test_parse_typescript_error() {
        let err = "src/client/api.ts:25:10 - error TS2304: Cannot find name 'ConfigOption'.";
        let diags = parse_compiler_diagnostics(err);
        assert_eq!(diags.len(), 1);
        let d = &diags[0];
        assert_eq!(d.file_path, PathBuf::from("src/client/api.ts"));
        assert_eq!(d.line, 25);
        assert_eq!(d.column, Some(10));
        assert_eq!(d.code.as_deref(), Some("TS2304"));
        assert!(d.message.contains("Cannot find name 'ConfigOption'"));
    }

    #[test]
    fn test_parse_python_traceback() {
        let err = r#"
Traceback (most recent call last):
  File "server/app.py", line 88, in handle_request
    data = json.loads(payload)
TypeError: the JSON object must be str, bytes or bytearray, not NoneType
"#;
        let diags = parse_compiler_diagnostics(err);
        assert_eq!(diags.len(), 1);
        let d = &diags[0];
        assert_eq!(d.file_path, PathBuf::from("server/app.py"));
        assert_eq!(d.line, 88);
        assert!(d.message.contains("TypeError"));
    }

    #[test]
    fn test_generate_self_healing_report_and_prompt() {
        let err = r#"
error[E0433]: failed to resolve: use of undeclared crate or module `foo`
  --> src/main.rs:10:5
"#;
        let report = generate_self_healing_report(err);
        assert_eq!(report.total_errors, 1);
        let prompt = build_self_healing_prompt(&report, Path::new("."));
        assert!(prompt.is_some());
        let p = prompt.unwrap();
        assert!(p.contains("src/main.rs"));
        assert!(p.contains("Line: 10"));
        assert!(p.contains("failed to resolve"));
    }
}
