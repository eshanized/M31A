//! Structured diagnostic models and compiler error parser.

use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    Help,
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

pub struct DiagnosticParser;

impl DiagnosticParser {
    /// Parses raw compiler / test runner stdout and stderr into structured diagnostics.
    pub fn parse(output: &str) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        // 1. Rust JSON messages
        for line in output.lines() {
            if line.starts_with('{')
                && (line.contains("\"$message_type\":\"diagnostic\"")
                    || line.contains("\"reason\":\"compiler-message\""))
                && let Ok(v) = serde_json::from_str::<serde_json::Value>(line)
                && let Some(d) = Self::parse_rust_json(&v)
            {
                diagnostics.push(d);
            }
        }

        if !diagnostics.is_empty() {
            return diagnostics;
        }

        // 2. Human-readable lines
        let lines: Vec<&str> = output.lines().collect();
        let mut i = 0;

        while i < lines.len() {
            let line = lines[i].trim();

            // Rust human readable: "error[E0308]: mismatched types"
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
                let mut loc = None;
                let mut snippet = None;
                let mut suggested_fix = None;

                for next_line in &lines[(i + 1)..(i + 15).min(lines.len())] {
                    let next = next_line.trim();
                    if next.starts_with("--> ") {
                        let loc_str = next.strip_prefix("--> ").unwrap().trim();
                        loc = Self::parse_loc(loc_str);
                    } else if next.starts_with("help: ") {
                        suggested_fix =
                            Some(next.strip_prefix("help: ").unwrap().trim().to_string());
                    } else if next.contains('|') && !next.starts_with("-->") {
                        if snippet.is_none() {
                            snippet = Some(next_line.to_string());
                        }
                    } else if next.starts_with("error[") || next.starts_with("error:") {
                        break;
                    }
                }

                if let Some((path, l, c)) = loc {
                    diagnostics.push(Diagnostic {
                        file_path: path,
                        line: l,
                        column: c,
                        severity: DiagnosticSeverity::Error,
                        code,
                        message,
                        snippet,
                        suggested_fix,
                    });
                }
            }
            // TypeScript: "src/app.ts:42:15 - error TS2322: Type 'string' is not assignable to type 'number'."
            else if line.contains("error TS") {
                if let Some((loc_str, msg_part)) = line.split_once(" - error ") {
                    let loc = Self::parse_loc(loc_str.trim());
                    let (code, msg) = msg_part.split_once(':').unwrap_or(("", msg_part));
                    if let Some((path, l, c)) = loc {
                        diagnostics.push(Diagnostic {
                            file_path: path,
                            line: l,
                            column: c,
                            severity: DiagnosticSeverity::Error,
                            code: Some(code.trim().to_string()),
                            message: msg.trim().to_string(),
                            snippet: None,
                            suggested_fix: None,
                        });
                    }
                }
            }
            // Python pytest failure: "FAILED tests/test_calc.py::test_division - ZeroDivisionError: division by zero"
            else if line.starts_with("FAILED ") && line.contains(" - ") {
                let rest = line.strip_prefix("FAILED ").unwrap().trim();
                if let Some((target, msg)) = rest.split_once(" - ") {
                    let (path_str, code) = if let Some((p, f)) = target.split_once("::") {
                        (p.trim(), Some(f.trim().to_string()))
                    } else {
                        (target.trim(), None)
                    };
                    diagnostics.push(Diagnostic {
                        file_path: PathBuf::from(path_str),
                        line: 1,
                        column: None,
                        severity: DiagnosticSeverity::Error,
                        code,
                        message: msg.trim().to_string(),
                        snippet: None,
                        suggested_fix: None,
                    });
                }
            }
            // Python traceback: File "path/to/file.py", line 42, in <module>
            else if line.starts_with("File \"")
                && line.contains(", line ")
                && let Some((path_part, rest)) = line
                    .strip_prefix("File \"")
                    .and_then(|s| s.split_once("\", line "))
            {
                let path = PathBuf::from(path_part);
                let line_num = rest
                    .split(',')
                    .next()
                    .and_then(|n| n.trim().parse().ok())
                    .unwrap_or(1);
                // Next line usually contains the failed statement
                let snippet = if i + 1 < lines.len() {
                    Some(lines[i + 1].trim().to_string())
                } else {
                    None
                };
                // Scan forward for error message
                let mut error_msg = "Python runtime error".to_string();
                for next_line in &lines[(i + 1)..(i + 10).min(lines.len())] {
                    let l = next_line.trim();
                    if l.contains("Error:") || l.contains("Exception:") {
                        error_msg = l.to_string();
                        break;
                    }
                }

                diagnostics.push(Diagnostic {
                    file_path: path,
                    line: line_num,
                    column: None,
                    severity: DiagnosticSeverity::Error,
                    code: None,
                    message: error_msg,
                    snippet,
                    suggested_fix: None,
                });
            }

            i += 1;
        }

        diagnostics
    }

    fn parse_rust_json(v: &serde_json::Value) -> Option<Diagnostic> {
        let target = if v.get("message").is_some_and(|m| m.is_object()) {
            v.get("message")?
        } else {
            v
        };
        let msg = target.get("message")?.as_str()?;
        let code = target
            .get("code")
            .and_then(|c| c.get("code"))
            .and_then(|s| s.as_str())
            .map(|s| s.to_string());
        let spans = target.get("spans")?.as_array()?;
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
            .and_then(|arr| arr.first())
            .and_then(|obj| obj.get("text"))
            .and_then(|s| s.as_str())
            .map(|s| s.to_string());

        let suggested_fix = primary_span
            .get("suggested_replacement")
            .and_then(|s| s.as_str())
            .map(|s| s.to_string());

        let level_str = target
            .get("level")
            .and_then(|s| s.as_str())
            .unwrap_or("error");
        let severity = match level_str {
            "warning" => DiagnosticSeverity::Warning,
            "help" | "note" => DiagnosticSeverity::Help,
            _ => DiagnosticSeverity::Error,
        };

        Some(Diagnostic {
            file_path: PathBuf::from(file_name),
            line: line_start,
            column: col_start,
            severity,
            code,
            message: msg.to_string(),
            snippet,
            suggested_fix,
        })
    }

    fn parse_loc(s: &str) -> Option<(PathBuf, usize, Option<usize>)> {
        let parts: Vec<&str> = s.split(':').collect();
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_rust_human_readable_diagnostic() {
        let err = r#"error[E0308]: mismatched types
 --> src/main.rs:42:15
  |
42 |     let x: u32 = "hello";
  |                  ^^^^^^^ expected `u32`, found `&str`
"#;
        let diags = DiagnosticParser::parse(err);
        assert_eq!(diags.len(), 1);
        let d = &diags[0];
        assert_eq!(d.file_path, PathBuf::from("src/main.rs"));
        assert_eq!(d.line, 42);
        assert_eq!(d.column, Some(15));
        assert_eq!(d.code, Some("E0308".to_string()));
        assert!(d.message.contains("mismatched types"));
    }
}
