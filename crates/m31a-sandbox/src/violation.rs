//! Normalized sandbox violation detection and keyword analysis.
//! Normalizes sandbox violation signals and analyzes error patterns to guide recovery.

use serde::{Deserialize, Serialize};

/// Maximum characters retained in a sandbox violation diagnostic snippet.
pub const OUTPUT_SNIPPET_MAX_CHARS: usize = 512;

/// Known sandbox enforcement backends.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SandboxViolationBackend {
    LinuxSandbox,
    ManagedNetworkProxy,
    Seatbelt,
    WindowsSandbox,
}

impl SandboxViolationBackend {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::LinuxSandbox => "linux_sandbox",
            Self::ManagedNetworkProxy => "managed_network_proxy",
            Self::Seatbelt => "seatbelt",
            Self::WindowsSandbox => "windows_sandbox",
        }
    }
}

/// Reason for a filesystem sandbox violation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileSystemSandboxViolationReason {
    OperationNotPermitted,
    PermissionDenied,
    ReadOnlyFileSystem,
    PolicyDenied,
    FailedToWriteFile,
}

/// Normalized filesystem sandbox violation event.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileSystemSandboxViolation {
    pub backend: SandboxViolationBackend,
    pub reason: FileSystemSandboxViolationReason,
    pub command: Option<String>,
    pub output_snippet: String,
    pub exit_code: Option<i32>,
}

/// Normalized network sandbox violation event.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetworkSandboxViolation {
    pub backend: SandboxViolationBackend,
    pub host: String,
    pub port: u16,
    pub reason: String,
}

/// Enum representing observed sandbox violations.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SandboxViolationEvent {
    FileSystem(FileSystemSandboxViolation),
    Network(NetworkSandboxViolation),
}

const SANDBOX_DENIED_KEYWORDS: [(FileSystemSandboxViolationReason, &str); 7] = [
    (
        FileSystemSandboxViolationReason::OperationNotPermitted,
        "operation not permitted",
    ),
    (
        FileSystemSandboxViolationReason::PermissionDenied,
        "permission denied",
    ),
    (
        FileSystemSandboxViolationReason::ReadOnlyFileSystem,
        "read-only file system",
    ),
    (FileSystemSandboxViolationReason::PolicyDenied, "seccomp"),
    (FileSystemSandboxViolationReason::PolicyDenied, "sandbox"),
    (FileSystemSandboxViolationReason::PolicyDenied, "landlock"),
    (
        FileSystemSandboxViolationReason::FailedToWriteFile,
        "failed to write file",
    ),
];

/// Analyzes process execution output to identify filesystem sandbox violations.
pub fn detect_filesystem_violation(
    backend: SandboxViolationBackend,
    command: Option<&str>,
    stdout: &str,
    stderr: &str,
    exit_code: Option<i32>,
) -> Option<FileSystemSandboxViolation> {
    // A clean successful exit code (0) does not indicate a sandbox violation
    if exit_code == Some(0) {
        return None;
    }

    let combined = if stderr.is_empty() { stdout } else { stderr };

    let lower = combined.to_lowercase();

    for (reason, keyword) in SANDBOX_DENIED_KEYWORDS {
        if let Some(pos) = lower.find(keyword) {
            let start = pos.saturating_sub(64);
            let end = (pos + keyword.len() + 128).min(combined.len());
            let raw_snippet = &combined[start..end];

            let snippet: String = raw_snippet.chars().take(OUTPUT_SNIPPET_MAX_CHARS).collect();

            return Some(FileSystemSandboxViolation {
                backend,
                reason,
                command: command.map(str::to_string),
                output_snippet: snippet.trim().to_string(),
                exit_code,
            });
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_read_only_filesystem_violation() {
        let stderr = "touch: cannot touch '/etc/config.json': Read-only file system\n";
        let violation = detect_filesystem_violation(
            SandboxViolationBackend::LinuxSandbox,
            Some("touch /etc/config.json"),
            "",
            stderr,
            Some(1),
        )
        .expect("should detect violation");

        assert_eq!(violation.backend, SandboxViolationBackend::LinuxSandbox);
        assert_eq!(
            violation.reason,
            FileSystemSandboxViolationReason::ReadOnlyFileSystem
        );
        assert!(violation.output_snippet.contains("Read-only file system"));
        assert_eq!(violation.exit_code, Some(1));
    }

    #[test]
    fn test_detect_seccomp_policy_denied() {
        let stderr = "fatal: socket call failed (seccomp policy denied)\n";
        let violation = detect_filesystem_violation(
            SandboxViolationBackend::LinuxSandbox,
            Some("nc -l 8080"),
            "",
            stderr,
            Some(142),
        )
        .expect("should detect seccomp violation");

        assert_eq!(
            violation.reason,
            FileSystemSandboxViolationReason::PolicyDenied
        );
        assert!(violation.output_snippet.contains("seccomp"));
    }

    #[test]
    fn test_ignore_successful_commands() {
        let stdout = "permission denied handled gracefully\n";
        let violation = detect_filesystem_violation(
            SandboxViolationBackend::LinuxSandbox,
            Some("test_cmd"),
            stdout,
            "",
            Some(0),
        );
        assert!(violation.is_none());
    }
}
