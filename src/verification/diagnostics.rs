//! Structured compiler and linter diagnostics subsystem (Issue 11).
//!
//! Parses raw diagnostic streams from Rust, TypeScript/JS, Python, and Go into
//! canonical, strongly-typed diagnostics with source spans, severity, error codes,
//! and suggested replacements.

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;

/// Severity level of a diagnostic item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    Note,
    Help,
}

impl std::fmt::Display for DiagnosticSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Error => write!(f, "error"),
            Self::Warning => write!(f, "warning"),
            Self::Note => write!(f, "note"),
            Self::Help => write!(f, "help"),
        }
    }
}

/// Source span pinpointing a diagnostic in the codebase.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticSpan {
    pub file_path: String,
    pub line_start: usize,
    pub line_end: usize,
    pub col_start: usize,
    pub col_end: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suggested_replacement: Option<String>,
}

/// Canonical structured diagnostic item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticItem {
    pub file_path: String,
    pub line: usize,
    pub column: usize,
    pub severity: DiagnosticSeverity,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suggested_fix: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub snippet_lines: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub spans: Vec<DiagnosticSpan>,
    pub source: String,
}

impl DiagnosticItem {
    /// Render this diagnostic item as a clean line for inclusion in model context.
    pub fn render_for_prompt(&self) -> String {
        let code_str = self
            .code
            .as_ref()
            .map(|c| format!(" [{c}]"))
            .unwrap_or_default();
        let mut out = format!(
            "[{}] {}:{}:{}{}: {}",
            self.severity.to_string().to_uppercase(),
            self.file_path,
            self.line,
            self.column,
            code_str,
            self.message
        );
        if let Some(ref fix) = self.suggested_fix {
            out.push_str(&format!("\n  Suggested fix: {fix}"));
        }
        if !self.snippet_lines.is_empty() {
            out.push_str(&format!(
                "\n  Context:\n    {}",
                self.snippet_lines.join("\n    ")
            ));
        }
        out
    }
}

// ---------------------------------------------------------------------------
// 1. Rust Parser
// ---------------------------------------------------------------------------

static RUST_SPAN_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*-->\s+([^:]+):(\d+):(\d+)").expect("valid regex"));
static RUST_CODE_MSG_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(error|warning|help|note)(?:\[([A-Za-z0-9]+)\])?:\s*(.*)").expect("valid regex")
});
static RUST_HELP_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*help:\s*(.*)").expect("valid regex"));

/// Parse Rust compiler diagnostics from human-readable stderr or JSON message format.
pub fn parse_rust_diagnostics(output: &str) -> Vec<DiagnosticItem> {
    let mut items = Vec::new();

    // Check if output is json lines from cargo --message-format=json
    for line in output.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('{') && trimmed.ends_with('}') {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(trimmed) {
                if v.get("reason").and_then(|r| r.as_str()) == Some("compiler-message") {
                    if let Some(msg) = v.get("message") {
                        if let Some(item) = parse_cargo_json_message(msg) {
                            items.push(item);
                            continue;
                        }
                    }
                }
            }
        }
    }

    if !items.is_empty() {
        return items;
    }

    // Parse human-readable rustc output
    let lines: Vec<&str> = output.lines().collect();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        if let Some(caps) = RUST_CODE_MSG_RE.captures(line) {
            let sev_str = &caps[1];
            let severity = match sev_str {
                "warning" => DiagnosticSeverity::Warning,
                "help" => DiagnosticSeverity::Help,
                "note" => DiagnosticSeverity::Note,
                _ => DiagnosticSeverity::Error,
            };
            let code = caps.get(2).map(|m| m.as_str().to_string());
            let message = caps
                .get(3)
                .map(|m| m.as_str().trim().to_string())
                .unwrap_or_default();

            let mut file_path = "unknown".to_string();
            let mut line_num = 1;
            let mut col_num = 1;
            let mut suggested_fix = None;
            let mut snippet_lines = Vec::new();

            // Look ahead for `--> file:line:col` and `help:`
            let mut j = i + 1;
            while j < lines.len() && j <= i + 15 {
                let next_line = lines[j];
                if RUST_CODE_MSG_RE.is_match(next_line) {
                    break;
                }
                if let Some(span_caps) = RUST_SPAN_RE.captures(next_line) {
                    file_path = span_caps[1].to_string();
                    line_num = span_caps[2].parse().unwrap_or(1);
                    col_num = span_caps[3].parse().unwrap_or(1);
                } else if let Some(help_caps) = RUST_HELP_RE.captures(next_line) {
                    suggested_fix = Some(help_caps[1].trim().to_string());
                } else if next_line.contains('|') && !next_line.trim().is_empty() {
                    snippet_lines.push(next_line.to_string());
                }
                j += 1;
            }

            items.push(DiagnosticItem {
                file_path,
                line: line_num,
                column: col_num,
                severity,
                code,
                message,
                suggested_fix,
                snippet_lines,
                spans: Vec::new(),
                source: "rustc".to_string(),
            });
        }
        i += 1;
    }

    items
}

fn parse_cargo_json_message(msg: &serde_json::Value) -> Option<DiagnosticItem> {
    let level_str = msg.get("level")?.as_str()?;
    let severity = match level_str {
        "warning" => DiagnosticSeverity::Warning,
        "note" => DiagnosticSeverity::Note,
        "help" => DiagnosticSeverity::Help,
        _ => DiagnosticSeverity::Error,
    };
    let message = msg.get("message")?.as_str()?.to_string();
    let code = msg
        .get("code")
        .and_then(|c| c.get("code"))
        .and_then(|c| c.as_str())
        .map(ToString::to_string);

    let mut file_path = "unknown".to_string();
    let mut line = 1;
    let mut column = 1;
    let mut spans = Vec::new();
    let mut suggested_fix = None;

    if let Some(span_array) = msg.get("spans").and_then(|s| s.as_array()) {
        for s in span_array {
            if let Some(file_name) = s.get("file_name").and_then(|f| f.as_str()) {
                if file_path == "unknown"
                    && s.get("is_primary")
                        .and_then(|p| p.as_bool())
                        .unwrap_or(false)
                {
                    file_path = file_name.to_string();
                    line = s.get("line_start").and_then(|l| l.as_u64()).unwrap_or(1) as usize;
                    column = s.get("column_start").and_then(|c| c.as_u64()).unwrap_or(1) as usize;
                }
                if let Some(replacement) = s.get("suggested_replacement").and_then(|r| r.as_str()) {
                    suggested_fix = Some(replacement.to_string());
                }
                spans.push(DiagnosticSpan {
                    file_path: file_name.to_string(),
                    line_start: s.get("line_start").and_then(|l| l.as_u64()).unwrap_or(1) as usize,
                    line_end: s.get("line_end").and_then(|l| l.as_u64()).unwrap_or(1) as usize,
                    col_start: s.get("column_start").and_then(|c| c.as_u64()).unwrap_or(1) as usize,
                    col_end: s.get("column_end").and_then(|c| c.as_u64()).unwrap_or(1) as usize,
                    text: s.get("text").and_then(|t| t.as_array()).map(|arr| {
                        arr.iter()
                            .filter_map(|t| t.get("text").and_then(|s| s.as_str()))
                            .collect::<Vec<_>>()
                            .join("\n")
                    }),
                    suggested_replacement: s
                        .get("suggested_replacement")
                        .and_then(|r| r.as_str())
                        .map(ToString::to_string),
                });
            }
        }
    }

    Some(DiagnosticItem {
        file_path,
        line,
        column,
        severity,
        code,
        message,
        suggested_fix,
        snippet_lines: Vec::new(),
        spans,
        source: "rustc".to_string(),
    })
}

// ---------------------------------------------------------------------------
// 2. TypeScript / JavaScript Parser
// ---------------------------------------------------------------------------

static TSC_FORMAT_1: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^([^(\s]+)\((\d+),(\d+)\):\s*(error|warning)\s+(TS\d+):\s*(.*)")
        .expect("valid regex")
});
static TSC_FORMAT_2: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^([^:\s]+):(\d+):(\d+)\s*-\s*(error|warning)\s+(TS\d+):\s*(.*)")
        .expect("valid regex")
});
static ESLINT_COMPACT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^([^:\s]+):\s*line\s*(\d+),\s*col\s*(\d+),\s*(Error|Warning)\s*-\s*(.*)")
        .expect("valid regex")
});

/// Parse TypeScript (tsc) and ESLint diagnostics.
pub fn parse_typescript_diagnostics(output: &str) -> Vec<DiagnosticItem> {
    let mut items = Vec::new();

    for line in output.lines() {
        let trimmed = line.trim();
        if let Some(caps) = TSC_FORMAT_1.captures(trimmed) {
            let file_path = caps[1].to_string();
            let line: usize = caps[2].parse().unwrap_or(1);
            let column: usize = caps[3].parse().unwrap_or(1);
            let severity = if &caps[4] == "warning" {
                DiagnosticSeverity::Warning
            } else {
                DiagnosticSeverity::Error
            };
            let code = Some(caps[5].to_string());
            let message = caps[6].trim().to_string();

            items.push(DiagnosticItem {
                file_path,
                line,
                column,
                severity,
                code,
                message,
                suggested_fix: None,
                snippet_lines: Vec::new(),
                spans: Vec::new(),
                source: "tsc".to_string(),
            });
        } else if let Some(caps) = TSC_FORMAT_2.captures(trimmed) {
            let file_path = caps[1].to_string();
            let line: usize = caps[2].parse().unwrap_or(1);
            let column: usize = caps[3].parse().unwrap_or(1);
            let severity = if &caps[4] == "warning" {
                DiagnosticSeverity::Warning
            } else {
                DiagnosticSeverity::Error
            };
            let code = Some(caps[5].to_string());
            let message = caps[6].trim().to_string();

            items.push(DiagnosticItem {
                file_path,
                line,
                column,
                severity,
                code,
                message,
                suggested_fix: None,
                snippet_lines: Vec::new(),
                spans: Vec::new(),
                source: "tsc".to_string(),
            });
        } else if let Some(caps) = ESLINT_COMPACT.captures(trimmed) {
            let file_path = caps[1].to_string();
            let line: usize = caps[2].parse().unwrap_or(1);
            let column: usize = caps[3].parse().unwrap_or(1);
            let severity = if caps[4].eq_ignore_ascii_case("warning") {
                DiagnosticSeverity::Warning
            } else {
                DiagnosticSeverity::Error
            };
            let raw_msg = caps[5].trim().to_string();
            let code = if let Some(rule_start) = raw_msg.rfind('(') {
                if raw_msg.ends_with(')') {
                    Some(raw_msg[rule_start + 1..raw_msg.len() - 1].to_string())
                } else {
                    None
                }
            } else {
                None
            };

            items.push(DiagnosticItem {
                file_path,
                line,
                column,
                severity,
                code,
                message: raw_msg,
                suggested_fix: None,
                snippet_lines: Vec::new(),
                spans: Vec::new(),
                source: "eslint".to_string(),
            });
        }
    }

    items
}

// ---------------------------------------------------------------------------
// 3. Python Parser
// ---------------------------------------------------------------------------

static MYPY_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^([^:\s]+):(\d+):(?:\d+:)?\s*(error|warning|note):\s*(.*?)(?:\s*\[(.*?)\])?$")
        .expect("valid regex")
});
static RUFF_FLAKE8_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^([^:\s]+):(\d+):(\d+):\s*([A-Z]\d+)\s+(.*)").expect("valid regex")
});
static PYTEST_FAIL_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^FAILED\s+([^:]+)::([^\s]+)\s*-\s*(.*)").expect("valid regex"));

/// Parse Python diagnostics from mypy, ruff, flake8, or pytest failures.
pub fn parse_python_diagnostics(output: &str) -> Vec<DiagnosticItem> {
    let mut items = Vec::new();

    for line in output.lines() {
        let trimmed = line.trim();
        if let Some(caps) = MYPY_RE.captures(trimmed) {
            let file_path = caps[1].to_string();
            let line: usize = caps[2].parse().unwrap_or(1);
            let severity = match &caps[3] {
                "warning" => DiagnosticSeverity::Warning,
                "note" => DiagnosticSeverity::Note,
                _ => DiagnosticSeverity::Error,
            };
            let message = caps[4].trim().to_string();
            let code = caps.get(5).map(|m| m.as_str().to_string());

            items.push(DiagnosticItem {
                file_path,
                line,
                column: 1,
                severity,
                code,
                message,
                suggested_fix: None,
                snippet_lines: Vec::new(),
                spans: Vec::new(),
                source: "mypy".to_string(),
            });
        } else if let Some(caps) = RUFF_FLAKE8_RE.captures(trimmed) {
            let file_path = caps[1].to_string();
            let line: usize = caps[2].parse().unwrap_or(1);
            let column: usize = caps[3].parse().unwrap_or(1);
            let code = Some(caps[4].to_string());
            let message = caps[5].trim().to_string();

            items.push(DiagnosticItem {
                file_path,
                line,
                column,
                severity: DiagnosticSeverity::Error,
                code,
                message,
                suggested_fix: None,
                snippet_lines: Vec::new(),
                spans: Vec::new(),
                source: "ruff".to_string(),
            });
        } else if let Some(caps) = PYTEST_FAIL_RE.captures(trimmed) {
            let file_path = caps[1].to_string();
            let test_fn = caps[2].to_string();
            let message = caps[3].trim().to_string();

            items.push(DiagnosticItem {
                file_path,
                line: 1,
                column: 1,
                severity: DiagnosticSeverity::Error,
                code: Some(format!("test:{test_fn}")),
                message,
                suggested_fix: None,
                snippet_lines: Vec::new(),
                spans: Vec::new(),
                source: "pytest".to_string(),
            });
        }
    }

    items
}

// ---------------------------------------------------------------------------
// 4. Go Parser
// ---------------------------------------------------------------------------

static GO_BUILD_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^([^:\s]+):(\d+):(\d+):\s*(.*)").expect("valid regex"));

/// Parse Go build and go vet diagnostics.
pub fn parse_go_diagnostics(output: &str) -> Vec<DiagnosticItem> {
    let mut items = Vec::new();

    for line in output.lines() {
        let trimmed = line.trim();
        if let Some(caps) = GO_BUILD_RE.captures(trimmed) {
            let file_path = caps[1].to_string();
            let line: usize = caps[2].parse().unwrap_or(1);
            let column: usize = caps[3].parse().unwrap_or(1);
            let message = caps[4].trim().to_string();

            items.push(DiagnosticItem {
                file_path,
                line,
                column,
                severity: DiagnosticSeverity::Error,
                code: None,
                message,
                suggested_fix: None,
                snippet_lines: Vec::new(),
                spans: Vec::new(),
                source: "go".to_string(),
            });
        }
    }

    items
}

// ---------------------------------------------------------------------------
// 5. Universal Parser
// ---------------------------------------------------------------------------

/// Universal diagnostic parser dispatching by hint or autodetection.
pub fn parse_diagnostics(output: &str, language_hint: Option<&str>) -> Vec<DiagnosticItem> {
    match language_hint.map(str::to_lowercase).as_deref() {
        Some("rust") | Some("rs") => parse_rust_diagnostics(output),
        Some("typescript") | Some("ts") | Some("javascript") | Some("js") => {
            parse_typescript_diagnostics(output)
        }
        Some("python") | Some("py") => parse_python_diagnostics(output),
        Some("go") | Some("golang") => parse_go_diagnostics(output),
        _ => {
            // Autodetect
            let rust = parse_rust_diagnostics(output);
            if !rust.is_empty() {
                return rust;
            }
            let ts = parse_typescript_diagnostics(output);
            if !ts.is_empty() {
                return ts;
            }
            let py = parse_python_diagnostics(output);
            if !py.is_empty() {
                return py;
            }
            parse_go_diagnostics(output)
        }
    }
}
