//! Governed verification executor (P0-02).
//!
//! Verification is execution. Verification commands (compiler / test /
//! linter strings, project-adapter overrides, `AutomatedTest{command}`)
//! must therefore pass through the SAME authoritative execution boundary as
//! every other child process — never a parallel direct-spawn path.
//!
//! Conceptual flow enforced here:
//!
//! ```text
//! verification configuration
//!     → argv parsing + shell-metachar rejection (no shell is ever invoked)
//!     → check_command_safety (protected paths, git redirection)
//!     → validate_working_directory (workspace containment, symlink-safe)
//!     → sanitized EnvironmentBuilder (env_clear; no secrets, no loaders)
//!     → ProcessSupervisor::run_supervised (process-group isolation,
//!        timeout, cancellation)
//!     → verification result (parsed by the calling runner)
//! ```
//!
//! There is exactly ONE authoritative execution boundary
//! (`ProcessSupervisor` + `EnvironmentBuilder` + `ProcessTreeController`).
//! This module contains no second copy of that logic; it only validates
//! verification-specific configuration and delegates execution.

use std::path::Path;
use std::time::Duration;

use tokio_util::sync::CancellationToken;

use crate::process::env::{EnvironmentBuilder, check_command_safety, validate_working_directory};
use crate::process::supervisor::ProcessSupervisor;

/// Default wall-clock bound for a governed verification command.
pub const DEFAULT_VERIFICATION_TIMEOUT: Duration = Duration::from_secs(60);

/// Shell metacharacters that are never valid in a directly-executed
/// verification command. Verification runners use direct `execve` argv
/// semantics (no shell); these characters indicate either a confused
/// configuration or an injection attempt, so they fail closed here rather
/// than being passed literally to the child.
const SHELL_METACHARACTERS: &[char] = &[';', '|', '&', '$', '`', '>', '<', '\n', '\r', '\u{0}'];

/// An approved verification command: validated program + argv, a contained
/// working directory, and a sanitized environment. Only values of this type
/// may be handed to the supervisor from verification code.
#[derive(Debug, Clone)]
pub struct ApprovedVerificationCommand {
    program: String,
    args: Vec<String>,
    cwd: std::path::PathBuf,
    extra_env: Vec<(String, String)>,
}

impl ApprovedVerificationCommand {
    pub fn program(&self) -> &str {
        &self.program
    }

    pub fn args(&self) -> &[String] {
        &self.args
    }
}

/// Parse and approve a verification command string for governed execution.
///
/// Rejects: empty commands (caller maps to not-applicable), shell
/// metacharacters, protected-path access, git redirection, workspace escapes.
/// On success returns the approved representation; execution still requires
/// [`execute_approved`] (parse ≠ permission to run).
pub fn approve_verification_command(
    command: &str,
    workspace_root: &Path,
    extra_env: Vec<(String, String)>,
) -> Result<ApprovedVerificationCommand, String> {
    if command.trim().is_empty() {
        return Err("verification command is empty".to_string());
    }
    for m in SHELL_METACHARACTERS {
        if command.contains(*m) {
            return Err(format!(
                "verification command rejected: shell metacharacter '{m}' is not permitted in directly-executed verification commands"
            ));
        }
    }
    // Parenthesized/brace subshell fragments without the above chars.
    if command.contains("$(") || command.contains("${") {
        return Err(
            "verification command rejected: command substitution is not permitted".to_string(),
        );
    }

    let parts: Vec<&str> = command.split_whitespace().collect();
    if parts.is_empty() {
        return Err("verification command is empty".to_string());
    }
    let program = parts[0].to_string();
    let args: Vec<String> = parts[1..].iter().map(|s| s.to_string()).collect();

    // Authority checks from the single process boundary.
    check_command_safety(&program, &args)
        .map_err(|e| format!("verification command rejected by process safety policy: {e}"))?;
    // Containment: the workspace root is pinned as the child cwd and must
    // canonicalize to itself (symlink escapes fail closed). NOTE: roots under
    // `.m31a/worktrees/*` are legitimate runtime execution roots (the agent
    // worktree model) and MUST approve — the protected-path invariant guards
    // agent tool file access, not the runtime's own contained cwd. A bare
    // `.git` root is still refused: no compilation unit can live there.
    if workspace_root
        .file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.eq_ignore_ascii_case(".git"))
    {
        return Err(format!(
            "verification workspace rejected: '{}' is a repository metadata directory, not an execution root",
            workspace_root.display()
        ));
    }
    // The root itself must not be a symlink escape: if the workspace root
    // is a symlink, the child would execute outside the authorized
    // directory. Ancestor symlinks are the operator's filesystem reality
    // and remain governed by validate_working_directory below.
    if let Ok(meta) = std::fs::symlink_metadata(workspace_root) {
        if meta.file_type().is_symlink() {
            return Err(format!(
                "verification workspace rejected: '{}' is a symlink, not an execution root",
                workspace_root.display()
            ));
        }
    }
    let cwd = validate_working_directory(Some(workspace_root), workspace_root)
        .map_err(|e| format!("verification workspace rejected: {e}"))?;

    // Extra env allowlist entries are validated by EnvironmentBuilder itself
    // (forbidden credential/loader keys fail closed here, before spawn).
    let probe = EnvironmentBuilder::new(&cwd);
    for (k, _) in &extra_env {
        if probe.is_forbidden_key(k) {
            return Err(format!(
                "verification environment rejected: '{k}' matches forbidden credential/loader pattern"
            ));
        }
    }

    Ok(ApprovedVerificationCommand {
        program,
        args,
        cwd,
        extra_env,
    })
}

/// Execute an approved verification command under the authoritative
/// supervisor: sanitized environment (no inherited secrets/loaders),
/// contained cwd, process-group isolation, timeout, and cancellation.
pub async fn execute_approved(
    approved: &ApprovedVerificationCommand,
    timeout: Duration,
    cancellation: Option<&CancellationToken>,
) -> Result<crate::process::types::ProcessOutput, String> {
    let mut env_builder = EnvironmentBuilder::new(&approved.cwd);
    for (k, v) in &approved.extra_env {
        env_builder
            .set_var(k.clone(), v.clone())
            .map_err(|e| format!("verification environment rejected: {e}"))?;
    }

    let supervisor = ProcessSupervisor::default();
    supervisor
        .run_command(
            &approved.program,
            &approved.args,
            &approved.cwd,
            Some(&env_builder),
            Some(timeout),
            cancellation,
        )
        .await
        .map_err(|e| format!("governed verification execution failed: {e}"))
}

/// Convenience: approve + execute a verification command string in one step.
///
/// Returns the child output for result parsing by the calling runner.
/// Empty commands are an approval error (callers map emptiness to an
/// explicit not-applicable verdict BEFORE calling here).
pub async fn execute_governed_verification(
    command: &str,
    workspace_root: &Path,
    timeout: Duration,
    extra_env: Vec<(String, String)>,
    cancellation: Option<&CancellationToken>,
) -> Result<crate::process::types::ProcessOutput, String> {
    let approved = approve_verification_command(command, workspace_root, extra_env)?;
    execute_approved(&approved, timeout, cancellation).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ws() -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    }

    #[test]
    fn shell_injection_rejected() {
        for cmd in [
            "cargo check; rm -rf /",
            "cargo test | tee out",
            "cargo check && evil",
            "cargo check $(evil)",
            "cargo `evil`",
            "cargo check > /tmp/x",
            "cargo $HOME check",
        ] {
            assert!(
                approve_verification_command(cmd, &ws(), vec![]).is_err(),
                "{cmd} must be rejected"
            );
        }
    }

    #[test]
    fn workspace_escape_rejected() {
        // A bare `.git` directory is not an execution root.
        assert!(approve_verification_command("cargo check", &ws().join(".git"), vec![]).is_err());
        // Symlink escapes fail closed: the root must canonicalize to itself.
        let tmp_dir = ws().join("tmp");
        let _ = std::fs::create_dir_all(&tmp_dir);
        let link = tmp_dir.join("executor-symlink-probe");
        let _ = std::fs::remove_file(&link);
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink("/tmp", &link).expect("symlink probe");
            assert!(approve_verification_command("cargo check", &link, vec![]).is_err());
            let _ = std::fs::remove_file(&link);
        }
    }

    #[test]
    fn worktree_roots_under_m31a_approve() {
        // The runtime agent-worktree model executes under
        // `.m31a/worktrees/*`: these roots must approve (P0-02 must not
        // break the worktree execution model).
        let root = ws().join("tmp/executor-worktree-probe/.m31a/worktrees/task-01");
        std::fs::create_dir_all(&root).expect("worktree probe dir");
        assert!(approve_verification_command("cargo check", &root, vec![]).is_ok());
        let _ = std::fs::remove_dir_all(ws().join("tmp/executor-worktree-probe"));
    }

    #[test]
    fn credential_env_rejected() {
        let err = approve_verification_command(
            "cargo test",
            &ws(),
            vec![("NVIDIA_API_KEY".to_string(), "x".to_string())],
        )
        .unwrap_err();
        assert!(err.contains("forbidden"), "got: {err}");
        let err = approve_verification_command(
            "cargo test",
            &ws(),
            vec![("LD_PRELOAD".to_string(), "x".to_string())],
        )
        .unwrap_err();
        assert!(err.contains("forbidden"), "got: {err}");
    }

    #[test]
    fn benign_extra_env_approved() {
        let approved = approve_verification_command(
            "cargo test --lib",
            &ws(),
            vec![
                ("RUST_TEST_THREADS".to_string(), "2".to_string()),
                ("CARGO_BUILD_JOBS".to_string(), "2".to_string()),
            ],
        )
        .expect("benign verification command must approve");
        assert_eq!(approved.program(), "cargo");
        assert_eq!(approved.args(), &["test".to_string(), "--lib".to_string()]);
    }

    #[tokio::test]
    async fn sanitized_environment_has_no_secrets() {
        unsafe { std::env::set_var("NVIDIA_API_KEY", "nvapi-should-never-leak-1234567890") };
        let out = execute_governed_verification(
            "cargo --version",
            &ws(),
            Duration::from_secs(30),
            vec![],
            None,
        )
        .await
        .expect("cargo --version must run governed");
        assert_eq!(out.exit_code, 0);
        assert!(
            !out.stdout.contains("nvapi-should-never-leak"),
            "child-visible output must not matter; env check below is authoritative"
        );
        unsafe { std::env::remove_var("NVIDIA_API_KEY") };
        // Authoritative check: the builder itself excludes the secret.
        let builder = EnvironmentBuilder::new(ws());
        let map = builder.build_map();
        assert!(!map.contains_key("NVIDIA_API_KEY"));
        assert!(!map.contains_key("LD_PRELOAD"));
    }
}
