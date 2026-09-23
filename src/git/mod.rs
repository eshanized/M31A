//! Git integration, worktree isolation, commit trailers, and drift detection (GST-01–GST-04, D-01–D-06).

pub mod attribution;
pub mod drift;
pub mod integration;
pub mod stash;
pub mod trailers;
pub mod worktree;

pub use attribution::{CommitAttributionRecord, GitAttributionStore};
pub use drift::{DriftStatus, TreeHashDriftDetector, TreeSnapshot};
pub use integration::{
    IntegrationReport, IntegrationState, MergeStrategy, WorktreeIntegrationStateMachine,
};
pub use stash::PrivateStashManager;
pub use trailers::CommitTrailers;
pub use worktree::{IsolatedWorktree, WorktreeConfig, WorktreeManager, WorktreeRetentionPolicy};

use crate::kernel::seams::policy::PolicyDecision;

/// Comprehensive Git subsystem error taxonomy.
#[derive(Debug, thiserror::Error)]
pub enum GitError {
    #[error("Git command failed ({exit_code:?}): {message}")]
    CommandFailed {
        exit_code: Option<i32>,
        message: String,
    },
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Worktree error: {0}")]
    Worktree(String),
    #[error("Invalid commit hash or ref: {0}")]
    InvalidRef(String),
    #[error("Attribution error: {0}")]
    Attribution(String),
    #[error("Drift detected: expected {expected}, actual {actual}")]
    DriftDetected {
        expected: String,
        actual: String,
        modified_files: Vec<String>,
    },
    #[error("Integration conflict: {0}")]
    IntegrationConflict(String),
    #[error("Policy violation: {0}")]
    PolicyViolation(String),
    #[error("Stash error: {0}")]
    Stash(String),
    #[error("Database error: {0}")]
    Database(String),
}

/// Typed Git operations subject to policy control (GST-01, GST-02, D-05).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum GitOperation {
    Status,
    Diff {
        staged: bool,
    },
    Log {
        max_count: usize,
    },
    Show {
        revision: String,
    },
    Branch,
    Checkout {
        target: String,
        force: bool,
    },
    Add {
        paths: Vec<String>,
    },
    Commit {
        message: String,
    },
    StashSave,
    StashPop,
    Fetch {
        remote: String,
    },
    Pull {
        remote: String,
        branch: String,
    },
    Push {
        remote: String,
        branch: String,
        force: bool,
    },
    HardReset {
        target: String,
    },
    HardClean {
        force: bool,
    },
    WorktreeRemove {
        force: bool,
    },
    BranchDelete {
        branch: String,
    },
    UpdateRef {
        git_ref: String,
    },
    Merge {
        source: String,
    },
    StashApply,
    SyncCheckout {
        target: String,
    },
    RawPassthrough,
}

impl GitOperation {
    pub fn is_remote(&self) -> bool {
        matches!(
            self,
            Self::Fetch { .. } | Self::Pull { .. } | Self::Push { .. }
        )
    }

    pub fn is_destructive(&self) -> bool {
        matches!(
            self,
            Self::HardReset { .. }
                | Self::HardClean { .. }
                | Self::Checkout { force: true, .. }
                | Self::Push { force: true, .. }
                | Self::WorktreeRemove { .. }
                | Self::BranchDelete { .. }
                | Self::UpdateRef { .. }
                | Self::Merge { .. }
                | Self::StashApply
                | Self::SyncCheckout { .. }
                | Self::RawPassthrough
        )
    }

    /// Evaluates the default policy decision for this operation (GST-02, D-05).
    /// - Safe local ops default to Allow
    /// - Remote push defaults to Ask (interactive operator approval with diff preview)
    /// - Fetch and pull default to Ask
    /// - Destructive commands (hard reset, hard clean) default to Deny
    /// - Worktree, branch, ref, merge, stash, and passthrough destructive ops
    ///   default to Ask so governed flows proceed only with explicit approval
    ///   while unapproved callers fail closed. (Ask — not Deny — for
    ///   RawPassthrough: the audited escape hatch must remain usable by
    ///   governed callers; the gate object itself is the audit trail.)
    pub fn default_policy_decision(&self) -> PolicyDecision {
        match self {
            Self::HardReset { .. } | Self::HardClean { .. } => PolicyDecision::Deny,
            Self::Push { .. } => PolicyDecision::Ask,
            Self::Checkout { force: true, .. } => PolicyDecision::Ask,
            Self::Fetch { .. } | Self::Pull { .. } => PolicyDecision::Ask,
            Self::WorktreeRemove { .. }
            | Self::BranchDelete { .. }
            | Self::UpdateRef { .. }
            | Self::Merge { .. }
            | Self::StashApply
            | Self::SyncCheckout { .. }
            | Self::RawPassthrough => PolicyDecision::Ask,
            _ => PolicyDecision::Allow,
        }
    }

    /// Validates the operation against policy. Returns Ok(()) if allowed,
    /// or Err(GitError::PolicyViolation) if denied or if approval is required.
    pub fn enforce_policy(&self, approved: bool) -> Result<(), GitError> {
        match self.default_policy_decision() {
            PolicyDecision::Allow => Ok(()),
            PolicyDecision::Ask => {
                if approved {
                    Ok(())
                } else {
                    Err(GitError::PolicyViolation(format!(
                        "Operation {:?} requires explicit operator approval",
                        self
                    )))
                }
            }
            PolicyDecision::Deny => Err(GitError::PolicyViolation(format!(
                "Operation {:?} is denied by safety policy veto",
                self
            ))),
            PolicyDecision::Escalate => Err(GitError::PolicyViolation(format!(
                "Operation {:?} requires escalation to supervisor",
                self
            ))),
        }
    }
}

/// Explicit approval gate for destructive Git operations.
///
/// Construct `GitGate::authorized()` only where a live execution authorization
/// (or equivalent explicit operator approval) governs the call. All other
/// callers must use `GitGate::denied()`, which fails closed on every
/// destructive operation. The gate makes approval auditable: every authorized
/// destructive Git mutation in production traces to a governed lane.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GitGate {
    approved: bool,
}

impl GitGate {
    /// Approval backed by a live execution authorization / operator approval.
    pub fn authorized() -> Self {
        Self { approved: true }
    }

    /// No approval: every destructive operation fails closed.
    pub fn denied() -> Self {
        Self { approved: false }
    }

    /// Enforce the canonical policy for `op` under this gate's approval.
    pub fn enforce(&self, op: &GitOperation) -> Result<(), GitError> {
        op.enforce_policy(self.approved)
    }
}

/// Validate a branch name, ref component, or commit-ish argument before it is
/// interpolated into a Git command line to prevent option-injection attacks.
///
/// Rejects empty values, leading dashes (option injection), control
/// characters and whitespace, `..` traversal, ref-syntax metacharacters
/// (`~ ^ : ? * [ \` and `@{`), trailing slashes, and `.lock` suffixes —
/// mirroring `git check-ref-format` safety rules for the dynamic inputs
/// M31A actually passes (branch names, worktree bases, target refs).
pub fn validate_git_ref_arg(kind: &str, value: &str) -> Result<(), GitError> {
    if value.is_empty() {
        return Err(GitError::InvalidRef(format!("empty {kind}")));
    }
    if value.len() > 255 {
        return Err(GitError::InvalidRef(format!("{kind} exceeds 255 chars")));
    }
    if value.starts_with('-') {
        return Err(GitError::InvalidRef(format!(
            "{kind} must not start with '-': '{value}'"
        )));
    }
    if value
        .chars()
        .any(|c| c.is_control() || c == ' ' || c == '\t' || c == '\n' || c == '\r')
    {
        return Err(GitError::InvalidRef(format!(
            "{kind} contains whitespace/control characters"
        )));
    }
    for forbidden in ["..", "~", "^", ":", "?", "*", "[", "\\", "@{"] {
        if value.contains(forbidden) {
            return Err(GitError::InvalidRef(format!(
                "{kind} contains forbidden sequence '{forbidden}': '{value}'"
            )));
        }
    }
    if value.ends_with('/') || value.ends_with(".lock") {
        return Err(GitError::InvalidRef(format!("malformed {kind}: '{value}'")));
    }
    Ok(())
}

/// Validate a full commit SHA (as produced by `rev-parse`) before it is
/// written to a ref via `update-ref`.
pub fn validate_git_sha(kind: &str, value: &str) -> Result<(), GitError> {
    if value.len() != 40 || !value.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(GitError::InvalidRef(format!(
            "{kind} must be a 40-char hex SHA, got '{value}'"
        )));
    }
    Ok(())
}

/// Build a `git` command with dangerous repository and worktree redirection
/// variables removed from the inherited environment.
///
/// Callers must still set `.current_dir(...)` to the intended repository or
/// worktree; this helper only neutralizes `GIT_DIR`/`GIT_WORK_TREE`-style
/// escapes and credential-helper overrides inherited from the host process.
pub fn scoped_git_command() -> tokio::process::Command {
    let mut cmd = tokio::process::Command::new("git");
    for var in crate::process::env::FORBIDDEN_GIT_REDIRECT_VARS {
        cmd.env_remove(var);
    }
    // Never let host Git config/credential helpers leak into governed runs.
    cmd.env_remove("GIT_SSH");
    cmd.env_remove("GIT_SSH_COMMAND");
    cmd.env_remove("GIT_ASKPASS");
    cmd.env_remove("GIT_EDITOR");
    cmd.env_remove("GIT_PAGER");
    cmd
}

/// Default ceiling for a single Git child process.
pub const GIT_COMMAND_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(120);

/// Run a scoped Git command to completion with timeout + kill.
///
/// The child is spawned with `kill_on_drop` so a timeout cancels a
/// half-finished mutation instead of leaving it running detached. Timeouts
/// surface as typed `GitError::CommandFailed`, never silent continuation.
pub async fn run_scoped_git(
    current_dir: &std::path::Path,
    args: &[&str],
    timeout: std::time::Duration,
) -> Result<std::process::Output, GitError> {
    let mut cmd = scoped_git_command();
    cmd.kill_on_drop(true);
    // Pipes are mandatory: a bare spawn() inherits parent stdio, which makes
    // wait_with_output return empty buffers while output leaks to the
    // terminal (and parsers see nothing).
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());
    cmd.args(args);
    cmd.current_dir(current_dir);
    let child = cmd.spawn().map_err(GitError::Io)?;
    match tokio::time::timeout(timeout, child.wait_with_output()).await {
        Ok(Ok(out)) => Ok(out),
        Ok(Err(e)) => Err(GitError::Io(e)),
        Err(_) => Err(GitError::CommandFailed {
            exit_code: None,
            message: format!(
                "git {} timed out after {:?}; child killed",
                args.join(" "),
                timeout
            ),
        }),
    }
}
