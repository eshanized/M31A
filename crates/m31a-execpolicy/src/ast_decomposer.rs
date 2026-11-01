//! Shell AST Decomposer and Command Sequence Validator.
//!
//! Decomposes complex command chains joined by safe operators (`&&`, `||`, `;`, `|`)
//! into discrete command argument vectors (`Vec<Vec<String>>`).
//!
//! Strict fail-closed policy: Rejects scripts containing dynamic shell constructs
//! such as subshells, command substitution (`$()`), process substitution (`<()`),
//! backticks (`` `...` ``), arithmetic evaluation (`$((...))`), redirection operators
//! (`>`, `<`, `>>`, `2>`), control flow (`if`, `for`, `while`), or backgrounding (`&`).

/// Result of analyzing and decomposing a shell command sequence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellDecomposition {
    pub commands: Vec<Vec<String>>,
}

pub struct ShellAstDecomposer;

impl ShellAstDecomposer {
    /// Attempts to parse and decompose a shell script into a sequence of discrete
    /// command argv vectors.
    ///
    /// Returns `Some(commands)` if every command consists only of plain words and
    /// quoted strings joined strictly by safe operators (`&&`, `||`, `;`, `|`).
    ///
    /// Returns `None` if any disallowed syntax (subshells, command substitution,
    /// redirection, control flow, unclosed quotes, etc.) is detected.
    pub fn try_decompose_sequence(script: &str) -> Option<Vec<Vec<String>>> {
        let trimmed = script.trim();
        if trimmed.is_empty() {
            return Some(Vec::new());
        }

        // 1. Scan for forbidden shell tokens before detailed parsing
        if Self::contains_forbidden_tokens(trimmed) {
            return None;
        }

        // 2. Lex into tokens and operator delimiters
        let segments = Self::split_safe_operators(trimmed)?;
        if segments.is_empty() {
            return None;
        }

        let mut commands = Vec::with_capacity(segments.len());
        for segment in segments {
            let seg_trimmed = segment.trim();
            if seg_trimmed.is_empty() {
                // Empty command between operators e.g. "cmd1 && ; cmd2" is invalid
                return None;
            }

            let mut argv = Self::parse_command_words(seg_trimmed)?;
            if argv.is_empty() {
                return None;
            }

            // Strip leading environment variable assignments (e.g. RUST_LOG=info cargo test)
            while !argv.is_empty() && Self::is_env_assignment(&argv[0]) {
                argv.remove(0);
            }

            if argv.is_empty() {
                return None;
            }

            // Check if this is a shell wrapper e.g. `bash -lc "cmd1 && cmd2"` or `sh -c "cmd"`
            if let Some(inner_script) = Self::extract_shell_wrapper_script(&argv) {
                // Recursively decompose the inner script. If the inner script contains forbidden
                // constructs or fails to parse, fail closed by returning None.
                let inner_cmds = Self::try_decompose_sequence(&inner_script)?;
                if inner_cmds.is_empty() {
                    return None;
                }
                commands.extend(inner_cmds);
                continue;
            }

            // Check if command word itself is a shell keyword
            let prog = argv[0].to_ascii_lowercase();
            if matches!(
                prog.as_str(),
                "if" | "then"
                    | "else"
                    | "elif"
                    | "fi"
                    | "for"
                    | "while"
                    | "until"
                    | "do"
                    | "done"
                    | "case"
                    | "esac"
                    | "select"
                    | "function"
            ) {
                return None;
            }

            commands.push(argv);
        }

        Some(commands)
    }

    /// Checks if a token matches the POSIX environment variable assignment syntax `NAME=VALUE`.
    fn is_env_assignment(token: &str) -> bool {
        if let Some((name, _)) = token.split_once('=') {
            if name.is_empty() {
                return false;
            }
            let mut chars = name.chars();
            let first = chars.next().unwrap();
            if !first.is_ascii_alphabetic() && first != '_' {
                return false;
            }
            chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
        } else {
            false
        }
    }

    /// Extracts inner script if argv represents a shell invocation with a `-c`, `-lc`, or `-Command` flag.
    fn extract_shell_wrapper_script(argv: &[String]) -> Option<String> {
        if argv.is_empty() {
            return None;
        }

        let prog_path = std::path::Path::new(&argv[0]);
        let prog_name = prog_path.file_name()?.to_str()?.to_ascii_lowercase();

        let is_unix_shell = matches!(prog_name.as_str(), "bash" | "sh" | "zsh" | "dash" | "ash");
        let is_powershell = matches!(
            prog_name.as_str(),
            "pwsh" | "powershell" | "powershell.exe" | "pwsh.exe"
        );

        if is_unix_shell {
            // Check [shell, "-c" | "-lc" | "-cl", script]
            if argv.len() >= 3 && matches!(argv[1].as_str(), "-c" | "-lc" | "-cl") {
                return Some(argv[2].clone());
            }
        } else if is_powershell {
            // Check [pwsh, "-Command" | "-c", script]
            if argv.len() >= 3
                && matches!(
                    argv[1].to_ascii_lowercase().as_str(),
                    "-c" | "-command" | "--command"
                )
            {
                return Some(argv[2].clone());
            }
        }

        None
    }

    /// Fast-path rejection of dynamic shell constructs.
    fn contains_forbidden_tokens(s: &str) -> bool {
        let bytes = s.as_bytes();
        let len = bytes.len();
        let mut in_single_quote = false;
        let mut in_double_quote = false;
        let mut i = 0;

        while i < len {
            let b = bytes[i];

            if b == b'\\' && i + 1 < len {
                i += 2;
                continue;
            }

            if b == b'\'' && !in_double_quote {
                in_single_quote = !in_single_quote;
                i += 1;
                continue;
            }

            if b == b'"' && !in_single_quote {
                in_double_quote = !in_double_quote;
                i += 1;
                continue;
            }

            // Inside single quotes, all characters are strictly literal in POSIX shell
            if in_single_quote {
                i += 1;
                continue;
            }

            // Characters checked outside single quotes (in double quotes, backticks & $() still expand!)
            if b == b'`' {
                return true; // Backtick command substitution
            }

            if b == b'$' && i + 1 < len {
                let next = bytes[i + 1];
                if next == b'(' || next == b'{' || next == b'[' {
                    return true; // $(...) or $((...)) or ${...} expansion
                }
            }

            // Characters checked strictly OUTSIDE both single and double quotes:
            if !in_double_quote {
                match b {
                    b'(' | b')' => return true,   // Subshell or function definition
                    b'<' | b'>' => return true,   // Redirection or process substitution
                    b'{' | b'}' => return true,   // Brace block or group
                    b'\n' | b'\r' => return true, // Multiline script
                    b'&' => {
                        // Allow '&&' but forbid backgrounding '&' or redirection '&>'
                        if i + 1 < len && bytes[i + 1] == b'&' {
                            i += 2;
                            continue;
                        }
                        return true;
                    }
                    _ => {}
                }
            }

            i += 1;
        }

        // If quotes remain unclosed at end of string
        in_single_quote || in_double_quote
    }

    /// Splits the script along safe chain/pipeline operators: `&&`, `||`, `;`, `|`.
    fn split_safe_operators(s: &str) -> Option<Vec<String>> {
        let mut segments = Vec::new();
        let mut current = String::new();
        let chars: Vec<char> = s.chars().collect();
        let len = chars.len();
        let mut i = 0;
        let mut in_single_quote = false;
        let mut in_double_quote = false;

        while i < len {
            let c = chars[i];

            if c == '\\' && i + 1 < len {
                current.push(c);
                current.push(chars[i + 1]);
                i += 2;
                continue;
            }

            if c == '\'' && !in_double_quote {
                in_single_quote = !in_single_quote;
                current.push(c);
                i += 1;
                continue;
            }

            if c == '"' && !in_single_quote {
                in_double_quote = !in_double_quote;
                current.push(c);
                i += 1;
                continue;
            }

            if !in_single_quote && !in_double_quote {
                // Check for '&&'
                if c == '&' && i + 1 < len && chars[i + 1] == '&' {
                    segments.push(current.trim().to_string());
                    current.clear();
                    i += 2;
                    continue;
                }

                // Check for '||'
                if c == '|' && i + 1 < len && chars[i + 1] == '|' {
                    segments.push(current.trim().to_string());
                    current.clear();
                    i += 2;
                    continue;
                }

                // Check for ';' or '|'
                if c == ';' || c == '|' {
                    segments.push(current.trim().to_string());
                    current.clear();
                    i += 1;
                    continue;
                }
            }

            current.push(c);
            i += 1;
        }

        if in_single_quote || in_double_quote {
            return None;
        }

        if !current.trim().is_empty() {
            segments.push(current.trim().to_string());
        }

        Some(segments)
    }

    /// Parses a single command segment into argv words, properly handling quotes and escapes.
    fn parse_command_words(s: &str) -> Option<Vec<String>> {
        let mut words = Vec::new();
        let mut current_word = String::new();
        let chars: Vec<char> = s.chars().collect();
        let len = chars.len();
        let mut i = 0;
        let mut in_single_quote = false;
        let mut in_double_quote = false;
        let mut has_token = false;

        while i < len {
            let c = chars[i];

            if c == '\\' && i + 1 < len {
                let next = chars[i + 1];
                if in_single_quote {
                    current_word.push(c);
                    current_word.push(next);
                } else if in_double_quote {
                    // In double quotes, backslash only escapes ", \, $, `, and \n
                    if matches!(next, '"' | '\\' | '$' | '`' | '\n') {
                        current_word.push(next);
                    } else {
                        current_word.push(c);
                        current_word.push(next);
                    }
                } else {
                    current_word.push(next);
                }
                has_token = true;
                i += 2;
                continue;
            }

            if c == '\'' && !in_double_quote {
                in_single_quote = !in_single_quote;
                has_token = true;
                i += 1;
                continue;
            }

            if c == '"' && !in_single_quote {
                in_double_quote = !in_double_quote;
                has_token = true;
                i += 1;
                continue;
            }

            if !in_single_quote && !in_double_quote && c.is_whitespace() {
                if has_token {
                    words.push(current_word.clone());
                    current_word.clear();
                    has_token = false;
                }
                i += 1;
                continue;
            }

            current_word.push(c);
            has_token = true;
            i += 1;
        }

        if in_single_quote || in_double_quote {
            return None;
        }

        if has_token {
            words.push(current_word);
        }

        Some(words)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_command_decomposition() {
        let script = "cargo check --workspace";
        let res = ShellAstDecomposer::try_decompose_sequence(script).expect("parse simple command");
        assert_eq!(res, vec![vec!["cargo", "check", "--workspace"]]);
    }

    #[test]
    fn test_pipeline_and_logical_operators() {
        let script = "git status --porcelain && cargo test -p m31a-core || echo failed";
        let res = ShellAstDecomposer::try_decompose_sequence(script).expect("parse operators");
        assert_eq!(res.len(), 3);
        assert_eq!(res[0], vec!["git", "status", "--porcelain"]);
        assert_eq!(res[1], vec!["cargo", "test", "-p", "m31a-core"]);
        assert_eq!(res[2], vec!["echo", "failed"]);
    }

    #[test]
    fn test_quoted_arguments_with_spaces() {
        let script = "echo 'hello world' \"nested space\" arg3";
        let res = ShellAstDecomposer::try_decompose_sequence(script).expect("parse quotes");
        assert_eq!(
            res,
            vec![vec!["echo", "hello world", "nested space", "arg3"]]
        );
    }

    #[test]
    fn test_subshell_rejection() {
        let script = "(cd /tmp && rm -rf *)";
        assert!(ShellAstDecomposer::try_decompose_sequence(script).is_none());
    }

    #[test]
    fn test_command_substitution_rejection() {
        let script1 = "echo $(whoami)";
        let script2 = "echo `id`";
        let script3 = "cat <(echo secret)";
        assert!(ShellAstDecomposer::try_decompose_sequence(script1).is_none());
        assert!(ShellAstDecomposer::try_decompose_sequence(script2).is_none());
        assert!(ShellAstDecomposer::try_decompose_sequence(script3).is_none());
    }

    #[test]
    fn test_redirection_rejection() {
        let script1 = "echo bad > /etc/shadow";
        let script2 = "cat < /etc/passwd";
        let script3 = "ls >> log.txt";
        assert!(ShellAstDecomposer::try_decompose_sequence(script1).is_none());
        assert!(ShellAstDecomposer::try_decompose_sequence(script2).is_none());
        assert!(ShellAstDecomposer::try_decompose_sequence(script3).is_none());
    }

    #[test]
    fn test_control_flow_rejection() {
        let script1 = "if true; then echo ok; fi";
        let script2 = "for f in *; do rm $f; done";
        assert!(ShellAstDecomposer::try_decompose_sequence(script1).is_none());
        assert!(ShellAstDecomposer::try_decompose_sequence(script2).is_none());
    }

    #[test]
    fn test_backgrounding_rejection() {
        let script = "sleep 10 &";
        assert!(ShellAstDecomposer::try_decompose_sequence(script).is_none());
    }

    #[test]
    fn test_shell_wrapper_unwrapping_bash_lc() {
        let script = "bash -lc \"git status --porcelain && cargo check\"";
        let res = ShellAstDecomposer::try_decompose_sequence(script).expect("unwrap bash -lc");
        assert_eq!(
            res,
            vec![vec!["git", "status", "--porcelain"], vec!["cargo", "check"],]
        );
    }

    #[test]
    fn test_env_assignment_stripping() {
        let script = "RUST_LOG=info CARGO_TERM_COLOR=always cargo test";
        let res = ShellAstDecomposer::try_decompose_sequence(script).expect("strip env vars");
        assert_eq!(res, vec![vec!["cargo", "test"]]);
    }

    #[test]
    fn test_shell_wrapper_with_forbidden_dynamic_constructs_fails_closed() {
        let script = "bash -c \"echo $(cat /etc/passwd)\"";
        assert!(ShellAstDecomposer::try_decompose_sequence(script).is_none());

        let script_redirection = "sh -c \"echo secret > /tmp/evil\"";
        assert!(ShellAstDecomposer::try_decompose_sequence(script_redirection).is_none());
    }
}
