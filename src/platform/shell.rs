//! Native shell selection behind an explicit contract.
//!
//! Direct argument-vector execution is always preferred because it never
//! interprets metacharacters. Shell-string execution exists only for
//! explicitly authorized steps and must resolve the interpreter through
//! this module so higher layers never assume a fixed shell name.
//!
//! Linux/macOS/Unix: POSIX `sh -c`
//! Windows: `cmd.exe /D /S /C` (default) or `powershell.exe -NoProfile -NonInteractive -Command`

use std::path::Path;

/// How a step wishes to execute a child.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShellRequest {
    DirectArgv,
    ShellString(String),
}

/// Whether shell-string execution can be provided here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellAvailability {
    Available,
    Unsupported,
    Unknown,
}

/// Native interpreter name, or `None` when this backend intentionally
/// defers shell execution to a later native implementation.
pub fn native_shell_program() -> Option<&'static str> {
    #[cfg(any(target_os = "linux", target_os = "macos", all(unix, not(windows))))]
    {
        Some("sh")
    }
    #[cfg(windows)]
    {
        Some("cmd.exe")
    }
    #[cfg(all(not(unix), not(windows)))]
    {
        None
    }
}

/// Backend label used in diagnostics.
pub fn backend_name() -> &'static str {
    #[cfg(any(target_os = "linux", target_os = "macos", all(unix, not(windows))))]
    {
        "posix-shell"
    }
    #[cfg(windows)]
    {
        crate::platform::windows::shell_backend_name()
    }
    #[cfg(all(not(unix), not(windows)))]
    {
        "unavailable"
    }
}

/// Whether shell strings can run on the executing host.
pub fn shell_availability() -> ShellAvailability {
    match native_shell_program() {
        Some(_) => ShellAvailability::Available,
        None => {
            if cfg!(windows) {
                ShellAvailability::Unsupported
            } else {
                ShellAvailability::Unknown
            }
        }
    }
}

/// Build a shell-wrapped command. Fails closed when no native interpreter
/// is available instead of falling back to an unrelated shell.
///
/// Linux/macOS/Unix: `sh -c "shell_str"`
/// Windows: `cmd.exe /D /S /C "shell_str"` (uses platform::windows::shell)
pub fn build_shell_command(
    shell_str: &str,
    cwd: &Path,
) -> Result<tokio::process::Command, ShellError> {
    #[cfg(any(target_os = "linux", target_os = "macos", all(unix, not(windows))))]
    {
        let mut cmd = tokio::process::Command::new("sh");
        cmd.args(["-c", shell_str]);
        cmd.current_dir(cwd);
        Ok(cmd)
    }
    #[cfg(windows)]
    {
        Ok(crate::platform::windows::shell::build_cmd_command(
            shell_str, cwd,
        ))
    }
    #[cfg(all(not(unix), not(windows)))]
    {
        Err(ShellError::Unsupported(format!(
            "shell-string execution is unsupported on backend '{}'",
            backend_name()
        )))
    }
}

/// Try to build a PowerShell command on Windows for steps that explicitly
/// select PowerShell semantics.
pub fn try_build_powershell_command(
    shell_str: &str,
    cwd: &Path,
) -> Result<tokio::process::Command, ShellError> {
    #[cfg(windows)]
    {
        Ok(crate::platform::windows::shell::build_powershell_command(
            shell_str, cwd,
        ))
    }
    #[cfg(not(windows))]
    {
        let _ = (shell_str, cwd);
        Err(ShellError::Unsupported(
            "PowerShell only available on Windows".to_string(),
        ))
    }
}

/// Recognized interpreter file names for command-safety inspection.
pub fn is_shell_program(program: &str) -> bool {
    let name = Path::new(program)
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_else(|| program.to_string());
    #[cfg(not(windows))]
    {
        matches!(name.as_str(), "sh" | "bash" | "zsh" | "dash" | "ksh")
    }
    #[cfg(windows)]
    {
        crate::platform::windows::shell::is_windows_shell_program(&name)
            || matches!(name.as_str(), "sh" | "bash" | "zsh" | "dash" | "ksh")
    }
}

/// Typed shell backend failure.
#[derive(Debug, thiserror::Error)]
pub enum ShellError {
    #[error("shell backend unsupported: {0}")]
    Unsupported(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_shell_program_on_unix() {
        #[cfg(any(target_os = "linux", target_os = "macos", all(unix, not(windows))))]
        assert_eq!(native_shell_program(), Some("sh"));
    }

    #[test]
    fn native_shell_program_on_windows() {
        #[cfg(windows)]
        assert_eq!(native_shell_program(), Some("cmd.exe"));
    }

    #[test]
    fn build_shell_command_compiles() {
        use std::path::Path;
        let _ = build_shell_command("echo test", Path::new("."));
    }

    #[test]
    fn is_shell_program_recognizes_known() {
        assert!(is_shell_program("sh"));
        assert!(is_shell_program("bash"));
        assert!(!is_shell_program("python"));
    }
}
