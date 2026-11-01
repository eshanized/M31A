//! Structural syntax and delimiter balance validation.

use anyhow::{Result, anyhow};

pub struct SyntaxValidator;

impl SyntaxValidator {
    /// Validates basic structural delimiter parity ({}, [], ()) ignoring string literals and line comments.
    pub fn validate_structural_balance(content: &str, file_ext: Option<&str>) -> Result<()> {
        let is_c_like = matches!(
            file_ext,
            Some("rs" | "ts" | "js" | "c" | "cpp" | "go" | "java" | "cs" | "json")
        );

        if !is_c_like {
            return Ok(());
        }

        let mut stack = Vec::new();
        let mut in_string = false;
        let mut string_char = ' ';
        let mut in_line_comment = false;
        let mut escaped = false;

        let chars: Vec<char> = content.chars().collect();
        let mut idx = 0;

        while idx < chars.len() {
            let c = chars[idx];

            if in_line_comment {
                if c == '\n' {
                    in_line_comment = false;
                }
                idx += 1;
                continue;
            }

            if in_string {
                if escaped {
                    escaped = false;
                } else if c == '\\' {
                    escaped = true;
                } else if c == string_char {
                    in_string = false;
                }
                idx += 1;
                continue;
            }

            // Check for line comment start
            if c == '/' && idx + 1 < chars.len() && chars[idx + 1] == '/' {
                in_line_comment = true;
                idx += 2;
                continue;
            }

            if c == '"' || c == '\'' || c == '`' {
                in_string = true;
                string_char = c;
                idx += 1;
                continue;
            }

            match c {
                '{' => stack.push(('{', idx)),
                '(' => stack.push(('(', idx)),
                '[' => stack.push(('[', idx)),
                '}' => {
                    if let Some(('{', _)) = stack.pop() {
                    } else {
                        return Err(anyhow!(
                            "Unmatched closing brace '}}' at character position {}",
                            idx
                        ));
                    }
                }
                ')' => {
                    if let Some(('(', _)) = stack.pop() {
                    } else {
                        return Err(anyhow!(
                            "Unmatched closing parenthesis ')' at character position {}",
                            idx
                        ));
                    }
                }
                ']' => {
                    if let Some(('[', _)) = stack.pop() {
                    } else {
                        return Err(anyhow!(
                            "Unmatched closing bracket ']' at character position {}",
                            idx
                        ));
                    }
                }
                _ => {}
            }

            idx += 1;
        }

        if let Some((unclosed, pos)) = stack.pop() {
            return Err(anyhow!(
                "Unclosed delimiter '{}' starting near character position {}",
                unclosed,
                pos
            ));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_syntax() {
        let code = r#"
            fn main() {
                let x = (1 + 2);
                let arr = [1, 2, 3];
                // Comment with }
                println!("String with ( { [");
            }
        "#;
        assert!(SyntaxValidator::validate_structural_balance(code, Some("rs")).is_ok());
    }

    #[test]
    fn test_unmatched_brace() {
        let code = r#"
            fn main() {
                let x = 1;
            // Missing closing brace
        "#;
        assert!(SyntaxValidator::validate_structural_balance(code, Some("rs")).is_err());
    }
}
