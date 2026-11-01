//! Pre-commit AST and language syntax validation gate.
//!
//! Intercepts prospective file mutations (writes, surgical edits, patches) and validates
//! the code structure before saving to disk, preventing LLM syntax corruption.

use anyhow::{Result, anyhow};
use std::path::Path;

#[derive(Debug, Clone)]
pub struct SyntaxValidationError {
    pub file_path: String,
    pub language: String,
    pub line: Option<usize>,
    pub column: Option<usize>,
    pub message: String,
}

impl std::fmt::Display for SyntaxValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let (Some(l), Some(c)) = (self.line, self.column) {
            write!(
                f,
                "Syntax validation failed for {} ({}) at line {}, col {}: {}",
                self.file_path, self.language, l, c, self.message
            )
        } else if let Some(l) = self.line {
            write!(
                f,
                "Syntax validation failed for {} ({}) at line {}: {}",
                self.file_path, self.language, l, self.message
            )
        } else {
            write!(
                f,
                "Syntax validation failed for {} ({}): {}",
                self.file_path, self.language, self.message
            )
        }
    }
}

impl std::error::Error for SyntaxValidationError {}

pub struct SyntaxValidator;

impl SyntaxValidator {
    /// Validates the prospective file content based on file extension.
    /// Returns `Ok(())` if valid or skipped (unsupported format), or `Err` with details.
    pub fn validate_file(path: &Path, content: &str) -> Result<()> {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        let path_str = path.display().to_string();

        // 1. Universal check: detect unmerged git conflict markers in all code files
        if let Some((line, marker)) = find_conflict_marker(content) {
            return Err(anyhow!(SyntaxValidationError {
                file_path: path_str,
                language: ext,
                line: Some(line),
                column: Some(1),
                message: format!("Unresolved git merge conflict marker detected: '{marker}'"),
            }));
        }

        match ext.as_str() {
            "rs" => Self::validate_rust(&path_str, content),
            "json" => Self::validate_json(&path_str, content),
            "toml" => Self::validate_toml(&path_str, content),
            "ts" | "tsx" | "js" | "jsx" => Self::validate_javascript(&path_str, content),
            "py" => Self::validate_python(&path_str, content),
            "go" => Self::validate_delimiters(&path_str, "Go", content),
            "c" | "cpp" | "cc" | "h" | "hpp" => {
                Self::validate_delimiters(&path_str, "C/C++", content)
            }
            _ => Ok(()),
        }
    }

    /// Validates Rust code using `syn::parse_file`.
    pub fn validate_rust(path_str: &str, content: &str) -> Result<()> {
        match syn::parse_file(content) {
            Ok(_) => Ok(()),
            Err(syn_err) => {
                let span = syn_err.span();
                let line = span.start().line;
                let column = span.start().column;
                Err(anyhow!(SyntaxValidationError {
                    file_path: path_str.to_string(),
                    language: "Rust".to_string(),
                    line: Some(line),
                    column: Some(column),
                    message: syn_err.to_string(),
                }))
            }
        }
    }

    /// Validates JSON syntax.
    pub fn validate_json(path_str: &str, content: &str) -> Result<()> {
        if content.trim().is_empty() {
            return Ok(());
        }
        match serde_json::from_str::<serde_json::Value>(content) {
            Ok(_) => Ok(()),
            Err(err) => Err(anyhow!(SyntaxValidationError {
                file_path: path_str.to_string(),
                language: "JSON".to_string(),
                line: Some(err.line()),
                column: Some(err.column()),
                message: err.to_string(),
            })),
        }
    }

    /// Validates TOML syntax.
    pub fn validate_toml(path_str: &str, content: &str) -> Result<()> {
        if content.trim().is_empty() {
            return Ok(());
        }
        match toml::from_str::<toml::Value>(content) {
            Ok(_) => Ok(()),
            Err(err) => {
                let line_col = err.span().map(|span| {
                    let prefix = &content[..span.start.min(content.len())];
                    let line = prefix.lines().count();
                    let col = prefix.lines().last().map(|l| l.len() + 1).unwrap_or(1);
                    (line, col)
                });
                let (line, col) = line_col
                    .map(|(l, c)| (Some(l), Some(c)))
                    .unwrap_or((None, None));
                Err(anyhow!(SyntaxValidationError {
                    file_path: path_str.to_string(),
                    language: "TOML".to_string(),
                    line,
                    column: col,
                    message: err.message().to_string(),
                }))
            }
        }
    }

    /// Validates JavaScript/TypeScript code delimiters and string closures.
    pub fn validate_javascript(path_str: &str, content: &str) -> Result<()> {
        Self::validate_delimiters(path_str, "TypeScript/JavaScript", content)?;

        // Grammar sanity checks
        for (idx, line) in content.lines().enumerate() {
            let trimmed = line.trim();
            // Check for incomplete variable assignments: const x = ; or let y = ;
            if (trimmed.starts_with("const ")
                || trimmed.starts_with("let ")
                || trimmed.starts_with("var "))
                && (trimmed.ends_with("= ;") || trimmed.ends_with("=;"))
            {
                return Err(anyhow!(SyntaxValidationError {
                    file_path: path_str.to_string(),
                    language: "TypeScript/JavaScript".to_string(),
                    line: Some(idx + 1),
                    column: Some(trimmed.len()),
                    message: "Incomplete variable declaration: missing right-hand side expression before semicolon.".to_string(),
                }));
            }
        }
        Ok(())
    }

    /// Validates Python code indentation and delimiter balance.
    pub fn validate_python(path_str: &str, content: &str) -> Result<()> {
        // First check delimiter balance (parentheses, brackets, curly braces)
        Self::validate_delimiters(path_str, "Python", content)?;

        // Check for inconsistent mixed indentation (tabs vs spaces) and missing colons
        let mut has_tabs = false;
        let mut has_spaces = false;
        for (idx, line) in content.lines().enumerate() {
            let leading: String = line.chars().take_while(|c| c.is_whitespace()).collect();
            if leading.contains('\t') {
                has_tabs = true;
            }
            if leading.contains("  ") {
                has_spaces = true;
            }
            if has_tabs && has_spaces {
                return Err(anyhow!(SyntaxValidationError {
                    file_path: path_str.to_string(),
                    language: "Python".to_string(),
                    line: Some(idx + 1),
                    column: Some(1),
                    message: "Inconsistent indentation: mixed tabs and spaces detected."
                        .to_string(),
                }));
            }

            let trimmed = line.trim();
            if !trimmed.starts_with('#') && !trimmed.is_empty() {
                // Check missing colon on block declarations
                if (trimmed.starts_with("def ")
                    || trimmed.starts_with("class ")
                    || trimmed.starts_with("if ")
                    || trimmed.starts_with("elif ")
                    || trimmed == "else"
                    || trimmed.starts_with("while ")
                    || trimmed.starts_with("for "))
                    && !trimmed.ends_with(':')
                    && !trimmed.ends_with('\\')
                    && !trimmed.contains('#')
                {
                    // If line has balanced parentheses (not a multiline header)
                    let opens = trimmed.chars().filter(|&c| c == '(').count();
                    let closes = trimmed.chars().filter(|&c| c == ')').count();
                    if opens == closes {
                        return Err(anyhow!(SyntaxValidationError {
                            file_path: path_str.to_string(),
                            language: "Python".to_string(),
                            line: Some(idx + 1),
                            column: Some(trimmed.len()),
                            message: format!(
                                "Syntax error: missing trailing colon ':' on Python header line '{trimmed}'"
                            ),
                        }));
                    }
                }
            }
        }
        Ok(())
    }

    /// Validates paired delimiters: `()`, `[]`, `{}` ignoring comments and string literals.
    pub fn validate_delimiters(path_str: &str, language: &str, content: &str) -> Result<()> {
        let mut stack: Vec<(char, usize, usize)> = Vec::new(); // (char, line, col)
        let mut in_single_quote = false;
        let mut in_double_quote = false;
        let mut in_backtick = false;
        let mut in_line_comment = false;
        let mut in_block_comment = false;
        let mut escaped = false;

        let mut line = 1;
        let mut col = 0;

        let chars: Vec<char> = content.chars().collect();
        let mut i = 0;

        while i < chars.len() {
            let ch = chars[i];
            col += 1;

            if ch == '\n' {
                line += 1;
                col = 0;
                in_line_comment = false;
                i += 1;
                continue;
            }

            // Line comment start
            if !in_single_quote
                && !in_double_quote
                && !in_backtick
                && !in_block_comment
                && !in_line_comment
            {
                if ch == '/' && i + 1 < chars.len() && chars[i + 1] == '/' {
                    in_line_comment = true;
                    i += 2;
                    col += 1;
                    continue;
                }
                if ch == '#' && language == "Python" {
                    in_line_comment = true;
                    i += 1;
                    continue;
                }
                if ch == '/' && i + 1 < chars.len() && chars[i + 1] == '*' {
                    in_block_comment = true;
                    i += 2;
                    col += 1;
                    continue;
                }
            }

            // Block comment end
            if in_block_comment {
                if ch == '*' && i + 1 < chars.len() && chars[i + 1] == '/' {
                    in_block_comment = false;
                    i += 2;
                    col += 1;
                    continue;
                }
                i += 1;
                continue;
            }

            if in_line_comment {
                i += 1;
                continue;
            }

            // Handle escapes
            if escaped {
                escaped = false;
                i += 1;
                continue;
            }

            if ch == '\\' {
                escaped = true;
                i += 1;
                continue;
            }

            // Handle string literal toggles
            if ch == '\'' && !in_double_quote && !in_backtick {
                in_single_quote = !in_single_quote;
                i += 1;
                continue;
            }
            if ch == '"' && !in_single_quote && !in_backtick {
                in_double_quote = !in_double_quote;
                i += 1;
                continue;
            }
            if ch == '`' && !in_single_quote && !in_double_quote {
                in_backtick = !in_backtick;
                i += 1;
                continue;
            }

            if in_single_quote || in_double_quote || in_backtick {
                i += 1;
                continue;
            }

            // Track paired opening/closing delimiters
            match ch {
                '(' | '[' | '{' => {
                    stack.push((ch, line, col));
                }
                ')' => {
                    if let Some((top, o_line, o_col)) = stack.pop() {
                        if top != '(' {
                            return Err(anyhow!(SyntaxValidationError {
                                file_path: path_str.to_string(),
                                language: language.to_string(),
                                line: Some(line),
                                column: Some(col),
                                message: format!(
                                    "Mismatched closing delimiter: expected matching '{}' for opening at line {}, col {}",
                                    top, o_line, o_col
                                ),
                            }));
                        }
                    } else {
                        return Err(anyhow!(SyntaxValidationError {
                            file_path: path_str.to_string(),
                            language: language.to_string(),
                            line: Some(line),
                            column: Some(col),
                            message: "Extraneous closing parenthesis ')' with no matching opening."
                                .to_string(),
                        }));
                    }
                }
                ']' => {
                    if let Some((top, o_line, o_col)) = stack.pop() {
                        if top != '[' {
                            return Err(anyhow!(SyntaxValidationError {
                                file_path: path_str.to_string(),
                                language: language.to_string(),
                                line: Some(line),
                                column: Some(col),
                                message: format!(
                                    "Mismatched closing delimiter: expected matching '{}' for opening at line {}, col {}",
                                    top, o_line, o_col
                                ),
                            }));
                        }
                    } else {
                        return Err(anyhow!(SyntaxValidationError {
                            file_path: path_str.to_string(),
                            language: language.to_string(),
                            line: Some(line),
                            column: Some(col),
                            message: "Extraneous closing bracket ']' with no matching opening."
                                .to_string(),
                        }));
                    }
                }
                '}' => {
                    if let Some((top, o_line, o_col)) = stack.pop() {
                        if top != '{' {
                            return Err(anyhow!(SyntaxValidationError {
                                file_path: path_str.to_string(),
                                language: language.to_string(),
                                line: Some(line),
                                column: Some(col),
                                message: format!(
                                    "Mismatched closing delimiter: expected matching '{}' for opening at line {}, col {}",
                                    top, o_line, o_col
                                ),
                            }));
                        }
                    } else {
                        return Err(anyhow!(SyntaxValidationError {
                            file_path: path_str.to_string(),
                            language: language.to_string(),
                            line: Some(line),
                            column: Some(col),
                            message: "Extraneous closing brace '}' with no matching opening."
                                .to_string(),
                        }));
                    }
                }
                _ => {}
            }

            i += 1;
        }

        if in_single_quote || in_double_quote || in_backtick {
            return Err(anyhow!(SyntaxValidationError {
                file_path: path_str.to_string(),
                language: language.to_string(),
                line: Some(line),
                column: Some(col),
                message: "Unclosed string or template literal reaches end of file.".to_string(),
            }));
        }

        if let Some((top, o_line, o_col)) = stack.pop() {
            return Err(anyhow!(SyntaxValidationError {
                file_path: path_str.to_string(),
                language: language.to_string(),
                line: Some(o_line),
                column: Some(o_col),
                message: format!(
                    "Unclosed delimiter '{}' opened at line {}, col {}",
                    top, o_line, o_col
                ),
            }));
        }

        if in_block_comment {
            return Err(anyhow!(SyntaxValidationError {
                file_path: path_str.to_string(),
                language: language.to_string(),
                line: Some(line),
                column: Some(col),
                message: "Unclosed block comment '/*' reaches end of file.".to_string(),
            }));
        }

        Ok(())
    }
}

fn find_conflict_marker(content: &str) -> Option<(usize, String)> {
    for (idx, line) in content.lines().enumerate() {
        if line.starts_with("<<<<<<< ") || line == "=======" || line.starts_with(">>>>>>> ") {
            return Some((idx + 1, line.to_string()));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_valid_rust_syntax() {
        let code = "fn main() { println!(\"Hello\"); }";
        assert!(SyntaxValidator::validate_file(&PathBuf::from("main.rs"), code).is_ok());
    }

    #[test]
    fn test_invalid_rust_syntax() {
        let code = "fn main( { println!(\"Broken\"); ";
        let res = SyntaxValidator::validate_file(&PathBuf::from("main.rs"), code);
        assert!(res.is_err());
        let err_str = res.unwrap_err().to_string();
        assert!(err_str.contains("Syntax validation failed"));
    }

    #[test]
    fn test_valid_json_syntax() {
        let json = r#"{"name": "m31a", "version": "0.1.0"}"#;
        assert!(SyntaxValidator::validate_file(&PathBuf::from("package.json"), json).is_ok());
    }

    #[test]
    fn test_invalid_json_syntax() {
        let json = r#"{"name": "m31a", "version": }"#;
        assert!(SyntaxValidator::validate_file(&PathBuf::from("package.json"), json).is_err());
    }

    #[test]
    fn test_unmatched_brackets_ts() {
        let code = "function compute() { if (true) { return [1, 2; } }";
        assert!(SyntaxValidator::validate_file(&PathBuf::from("app.ts"), code).is_err());
    }

    #[test]
    fn test_conflict_marker_detection() {
        let code = "fn hello() {\n<<<<<<< HEAD\nlet x = 1;\n=======\nlet x = 2;\n>>>>>>> branch\n}";
        assert!(SyntaxValidator::validate_file(&PathBuf::from("lib.rs"), code).is_err());
    }
}
