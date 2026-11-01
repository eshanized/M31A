//! macOS Seatbelt Sandbox Backend
//!
//! Implements sandboxing using macOS's built-in Seatbelt (sandbox-exec) framework.
//! Provides robust SBPL profile generation matching OpenAI Codex security guarantees:
//! - Closed-by-default execution policy
//! - System TLS (`com.apple.TrustEvaluationAgent`) and user info resolution
//! - Developer toolchain access (`/opt/homebrew`, `/usr/local`, `/Library/Developer`)
//! - Protected metadata write denials (`.git`, `.m31a`, `.ssh`, `.env`, `.aws`)
//! - POSIX IPC and sysctl compatibility for multithreading/multiprocessing runtimes

use anyhow::{Context, Result};
use m31a_core::config::SandboxPolicy;
use std::path::Path;
use std::process::Command;

/// Standard absolute path to the macOS Seatbelt sandbox binary.
pub const MACOS_PATH_TO_SEATBELT_EXECUTABLE: &str = "/usr/bin/sandbox-exec";

/// Protected workspace metadata subpaths that must remain protected from sandboxed modification.
pub const SEATBELT_PROTECTED_METADATA_NAMES: &[&str] =
    &[".git", ".m31a", ".ssh", ".env", ".aws", ".agents"];

/// Seatbelt sandbox implementation for macOS
pub struct SeatbeltSandbox {
    profile: String,
}

impl SeatbeltSandbox {
    /// Create a new Seatbelt sandbox
    pub fn new(policy: SandboxPolicy, workspace_root: &Path) -> Result<Self> {
        let profile = Self::generate_profile(policy, workspace_root)?;
        Ok(Self { profile })
    }

    /// Access the generated SBPL profile string
    pub fn profile(&self) -> &str {
        &self.profile
    }

    /// Generate Seatbelt SBPL profile based on policy and workspace root
    pub fn generate_profile(policy: SandboxPolicy, workspace_root: &Path) -> Result<String> {
        let workspace_str = workspace_root.to_string_lossy();
        let workspace_clean = workspace_str.trim_end_matches('/');

        let profile = match policy {
            SandboxPolicy::ReadOnly => {
                format!(
                    r#"(version 1)
(deny default)

; process lifecycle & signals
(allow process-exec)
(allow process-fork)
(allow signal (target same-sandbox))
(allow process-info* (target same-sandbox))

; sysctl reads
(allow sysctl-read)

; system TLS certificate evaluation & directory services
(allow mach-lookup
  (global-name "com.apple.TrustEvaluationAgent")
  (global-name "com.apple.system.opendirectoryd.libinfo"))

; POSIX IPC for multiprocessing (Python, Node, OpenMP)
(allow ipc-posix-sem)
(allow ipc-posix-shm*)

; standard character devices & pseudo terminals
(allow file-read* file-test-existence file-write-data
  (literal "/dev/null")
  (literal "/dev/zero"))
(allow file-read* file-test-existence
  (literal "/dev/random")
  (literal "/dev/urandom"))
(allow file-read* file-test-existence file-write-data
  (subpath "/dev/fd"))

; platform frameworks, libraries, and dev tools
(allow file-read*
  (subpath "/usr")
  (subpath "/System")
  (subpath "/Library")
  (subpath "/opt/homebrew")
  (subpath "/usr/local")
  (subpath "/Library/Developer/CommandLineTools")
  (subpath "/private/var/db/timezone")
  (subpath "/private/etc")
  (literal "/etc")
  (literal "/tmp")
  (literal "/var")
  (literal "/private/tmp"))

; developer & system executable locations
(allow process-exec
  (subpath "/bin")
  (subpath "/sbin")
  (subpath "/usr/bin")
  (subpath "/usr/sbin")
  (subpath "/usr/local/bin")
  (subpath "/opt/homebrew/bin")
  (subpath "/Library/Developer/CommandLineTools/usr/bin"))

; workspace read access
(allow file-read* (subpath "{workspace}"))
(deny file-write*)
(allow network*)
"#,
                    workspace = workspace_clean
                )
            }
            SandboxPolicy::WorkspaceWrite => {
                let mut protected_denies = String::new();
                for meta in SEATBELT_PROTECTED_METADATA_NAMES {
                    protected_denies.push_str(&format!(
                        "  (literal \"{workspace}/{meta}\")\n  (subpath \"{workspace}/{meta}\")\n",
                        workspace = workspace_clean,
                        meta = meta
                    ));
                }

                format!(
                    r#"(version 1)
(deny default)

; process lifecycle & signals
(allow process-exec)
(allow process-fork)
(allow signal (target same-sandbox))
(allow process-info* (target same-sandbox))

; sysctl reads
(allow sysctl-read)

; system TLS certificate evaluation & directory services
(allow mach-lookup
  (global-name "com.apple.TrustEvaluationAgent")
  (global-name "com.apple.system.opendirectoryd.libinfo"))

; POSIX IPC for multiprocessing (Python, Node, OpenMP)
(allow ipc-posix-sem)
(allow ipc-posix-shm*)

; standard character devices & pseudo terminals
(allow file-read* file-test-existence file-write-data
  (literal "/dev/null")
  (literal "/dev/zero"))
(allow file-read* file-test-existence
  (literal "/dev/random")
  (literal "/dev/urandom"))
(allow file-read* file-test-existence file-write-data
  (subpath "/dev/fd"))

; platform frameworks, libraries, and dev tools
(allow file-read*
  (subpath "/usr")
  (subpath "/System")
  (subpath "/Library")
  (subpath "/opt/homebrew")
  (subpath "/usr/local")
  (subpath "/Library/Developer/CommandLineTools")
  (subpath "/private/var/db/timezone")
  (subpath "/private/etc")
  (literal "/etc")
  (literal "/tmp")
  (literal "/var")
  (literal "/private/tmp"))

; developer & system executable locations
(allow process-exec
  (subpath "/bin")
  (subpath "/sbin")
  (subpath "/usr/bin")
  (subpath "/usr/sbin")
  (subpath "/usr/local/bin")
  (subpath "/opt/homebrew/bin")
  (subpath "/Library/Developer/CommandLineTools/usr/bin")
  (subpath "{workspace}"))

; workspace read and write permissions
(allow file-read* (subpath "{workspace}"))
(allow file-write* (subpath "{workspace}"))

; authority boundary root unlink defense
(deny file-write-unlink (require-all (literal "{workspace}") (vnode-type DIRECTORY)))

; deny modifying or recreating protected workspace metadata
(deny file-write*
{protected_denies})

; network access
(allow network*)
"#,
                    workspace = workspace_clean,
                    protected_denies = protected_denies.trim_end()
                )
            }
            SandboxPolicy::DangerFullAccess => r#"(version 1)
(deny default)

(allow process-exec)
(allow process-fork)
(allow signal (target same-sandbox))
(allow process-info* (target same-sandbox))

(allow file-read* (subpath "/"))
(allow file-write* (subpath "/"))
(allow process-exec (subpath "/"))

(allow network*)
(allow sysctl-read)
(allow sysctl-write)
(allow mach-lookup)
(allow ipc-posix-sem)
(allow ipc-posix-shm*)
"#
            .to_string(),
        };

        Ok(profile)
    }

    /// Execute a command within the Seatbelt sandbox
    pub fn execute(&self, command: &[String], working_dir: &Path) -> Result<CommandOutput> {
        if command.is_empty() {
            return Err(anyhow::anyhow!("Empty command"));
        }

        // Write profile to temporary file
        let profile_path =
            std::env::temp_dir().join(format!("m31a-seatbelt-{}.sb", uuid::Uuid::new_v4()));
        std::fs::write(&profile_path, &self.profile)?;

        let sandbox_bin = if Path::new(MACOS_PATH_TO_SEATBELT_EXECUTABLE).exists() {
            MACOS_PATH_TO_SEATBELT_EXECUTABLE
        } else {
            "sandbox-exec"
        };

        // Build sandbox-exec command
        let mut cmd = Command::new(sandbox_bin);
        cmd.arg("-f").arg(&profile_path);
        cmd.arg("--").arg(&command[0]);
        for arg in &command[1..] {
            cmd.arg(arg);
        }
        cmd.current_dir(working_dir);

        let output = cmd.output().context("Failed to execute sandbox-exec")?;

        // Clean up profile file
        let _ = std::fs::remove_file(&profile_path);

        Ok(CommandOutput {
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            exit_code: output.status.code().unwrap_or(-1),
        })
    }

    /// Check if Seatbelt is available
    pub fn is_available() -> bool {
        if Path::new(MACOS_PATH_TO_SEATBELT_EXECUTABLE).exists() {
            return true;
        }
        Command::new("sandbox-exec")
            .arg("-h")
            .output()
            .map(|o| o.status.success() || o.status.code() == Some(1))
            .unwrap_or(false)
    }
}

/// Command output
#[derive(Debug, Clone)]
pub struct CommandOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_profile_generation_workspace_write() {
        let policy = SandboxPolicy::WorkspaceWrite;
        let workspace = Path::new("/tmp/test_workspace");
        let profile = SeatbeltSandbox::generate_profile(policy, workspace).unwrap();

        assert!(profile.contains("(version 1)"));
        assert!(profile.contains("allow file-read* (subpath \"/tmp/test_workspace\")"));
        assert!(profile.contains("allow file-write* (subpath \"/tmp/test_workspace\")"));
        assert!(profile.contains("com.apple.TrustEvaluationAgent"));
        assert!(profile.contains("/opt/homebrew"));
        assert!(profile.contains("deny file-write*"));
        assert!(profile.contains("/tmp/test_workspace/.git"));
        assert!(profile.contains("/tmp/test_workspace/.m31a"));
        assert!(profile.contains("/tmp/test_workspace/.ssh"));
        assert!(profile.contains("/tmp/test_workspace/.env"));
    }

    #[test]
    fn test_profile_generation_read_only() {
        let policy = SandboxPolicy::ReadOnly;
        let workspace = Path::new("/tmp/test_ro");
        let profile = SeatbeltSandbox::generate_profile(policy, workspace).unwrap();

        assert!(profile.contains("(deny file-write*)"));
        assert!(profile.contains("allow file-read* (subpath \"/tmp/test_ro\")"));
        assert!(profile.contains("com.apple.TrustEvaluationAgent"));
        assert!(!profile.contains("allow file-write* (subpath \"/tmp/test_ro\")"));
    }

    #[test]
    fn test_profile_danger_full_access() {
        let policy = SandboxPolicy::DangerFullAccess;
        let workspace = Path::new("/tmp/test_danger");
        let profile = SeatbeltSandbox::generate_profile(policy, workspace).unwrap();

        assert!(profile.contains("allow file-write* (subpath \"/\")"));
        assert!(profile.contains("allow sysctl-write"));
    }
}
