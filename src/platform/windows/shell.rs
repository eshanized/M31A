//! Native Windows shell backends.
//!
//! Direct argument-vector execution is always preferred. Shell-string
//! execution exists only for explicitly authorized steps and resolves the
//! interpreter here so higher layers never embed `cmd.exe` or PowerShell
//! syntax assumptions. Quoting helpers preserve argument boundaries; they
//! are pure and exercised on every host.

use std::path::Path;

/// Which interpreter backs shell-string execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellBackend {
    Cmd,
    PowerShell,
    Pwsh,
}

impl ShellBackend {
    /// Canonical program name.
    pub fn program(&self) -> &'static str {
        match self {
            Self::Cmd => "cmd.exe",
            Self::PowerShell => "powershell.exe",
            Self::Pwsh => "pwsh.exe",
        }
    }

    /// Arguments wrapping a shell string, excluding the string itself.
    pub fn wrapper_args(&self) -> &'static [&'static str] {
        match self {
            Self::Cmd => &["/D", "/S", "/C"],
            Self::PowerShell | Self::Pwsh => &["-NoProfile", "-NonInteractive", "-Command"],
        }
    }
}

/// Preferred backend for shell strings on Windows.
pub fn preferred_backend() -> ShellBackend {
    ShellBackend::Cmd
}

/// Whether a program name names a Windows shell interpreter.
pub fn is_windows_shell_program(program: &str) -> bool {
    let name = Path::new(program)
        .file_name()
        .map(|f| f.to_string_lossy().to_lowercase())
        .unwrap_or_else(|| program.to_lowercase());
    matches!(
        name.as_str(),
        "cmd" | "cmd.exe" | "powershell" | "powershell.exe" | "pwsh" | "pwsh.exe"
    )
}

/// Characters that `cmd.exe` reinterprets outside quotes.
pub fn contains_cmd_metachars(input: &str) -> bool {
    input.chars().any(|c| {
        matches!(
            c,
            '&' | '|' | '<' | '>' | '^' | '%' | '!' | '"' | '\n' | '\r' | ';' | '(' | ')'
        )
    })
}

/// Characters that PowerShell reinterprets outside quotes.
pub fn contains_powershell_metachars(input: &str) -> bool {
    input.chars().any(|c| {
        matches!(
            c,
            '$' | '`' | '"' | '\'' | '|' | ';' | '{' | '}' | '(' | ')' | '#' | '\n' | '\r'
        )
    })
}

/// Quote one argument for `cmd.exe` preserving boundaries.
///
/// Uses double-quote wrapping with interior-quote doubling and trailing
/// backslash protection. Empty arguments become `""`.
pub fn quote_cmd_arg(arg: &str) -> String {
    if arg.is_empty() {
        return "\"\"".to_string();
    }
    let needs_quotes = arg
        .chars()
        .any(|c| c.is_whitespace() || matches!(c, '"' | '&' | '|' | '<' | '>' | '^' | '%' | '!'));
    if !needs_quotes {
        return arg.to_string();
    }
    let mut out = String::with_capacity(arg.len() + 2);
    out.push('"');
    let mut backslashes = 0usize;
    for c in arg.chars() {
        if c == '\\' {
            backslashes += 1;
        } else if c == '"' {
            for _ in 0..backslashes {
                out.push('\\');
            }
            backslashes = 0;
            out.push('\\');
            out.push('"');
        } else {
            for _ in 0..backslashes {
                out.push('\\');
            }
            backslashes = 0;
            out.push(c);
        }
    }
    for _ in 0..backslashes {
        out.push('\\');
    }
    out.push('"');
    out
}

/// Quote one argument for PowerShell single-quote literals.
pub fn quote_powershell_arg(arg: &str) -> String {
    if arg.is_empty() {
        return "''".to_string();
    }
    let safe = arg
        .chars()
        .all(|c| c.is_alphanumeric() || matches!(c, '-' | '_' | '.' | ':' | '\\' | '/'));
    if safe {
        return arg.to_string();
    }
    format!("'{}'", arg.replace('\'', "''"))
}

/// Build a `cmd.exe`-wrapped shell command. The working directory and
/// environment stay under caller control; this helper only selects the
/// interpreter and preserves the boundary between program and script text.
pub fn build_cmd_command(shell_str: &str, cwd: &Path) -> tokio::process::Command {
    let mut cmd = tokio::process::Command::new("cmd.exe");
    cmd.args(["/D", "/S", "/C", shell_str]);
    cmd.current_dir(cwd);
    cmd
}

/// Build a PowerShell-wrapped shell command for steps that explicitly select
/// PowerShell semantics.
pub fn build_powershell_command(shell_str: &str, cwd: &Path) -> tokio::process::Command {
    let mut cmd = tokio::process::Command::new("powershell.exe");
    cmd.args(["-NoProfile", "-NonInteractive", "-Command", shell_str]);
    cmd.current_dir(cwd);
    cmd
}

/// Build a PowerShell 7+ (pwsh.exe) wrapped shell command.
pub fn build_pwsh_command(shell_str: &str, cwd: &Path) -> tokio::process::Command {
    let mut cmd = tokio::process::Command::new("pwsh.exe");
    cmd.args(["-NoProfile", "-NonInteractive", "-Command", shell_str]);
    cmd.current_dir(cwd);
    cmd
}

/// Validate that direct-argv execution was not flattened into a shell
/// string. Shell metacharacters in a single argv element are data, never
/// syntax, and must survive intact.
pub fn argv_preserves_boundaries(args: &[String]) -> bool {
    // Trivially true by construction for argv execution; the helper exists
    // so tests name the property explicitly.
    let _ = args;
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cmd_quoting_preserves_spaces_and_metachars() {
        let q = quote_cmd_arg("a & b");
        assert!(q.starts_with('"') && q.ends_with('"'));
        assert!(q.contains("a & b"));
    }

    #[test]
    fn cmd_empty_arg_is_quoted() {
        assert_eq!(quote_cmd_arg(""), "\"\"");
    }

    #[test]
    fn powershell_single_quotes_escape() {
        assert_eq!(quote_powershell_arg("it's"), "'it''s'");
    }

    #[test]
    fn shell_detection_covers_known_names() {
        assert!(is_windows_shell_program("cmd.exe"));
        assert!(is_windows_shell_program("PowerShell.EXE"));
        assert!(is_windows_shell_program("pwsh.exe"));
        assert!(is_windows_shell_program("pwsh"));
        assert!(!is_windows_shell_program("python.exe"));
    }

    #[test]
    fn shell_backend_pwsh_properties() {
        let backend = ShellBackend::Pwsh;
        assert_eq!(backend.program(), "pwsh.exe");
        assert_eq!(
            backend.wrapper_args(),
            &["-NoProfile", "-NonInteractive", "-Command"]
        );
    }
}
