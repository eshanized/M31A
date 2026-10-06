//! Git integration, worktree isolation, commit trailers, and drift detection (GST-01–GST-04, D-01–D-06).

pub mod attribution;
pub mod authorization;
pub mod drift;
pub mod integration;
pub mod probe;
pub mod stash;
pub mod trailers;
pub mod worktree;

pub use attribution::{CommitAttributionRecord, GitAttributionStore};
pub use authorization::{
    AuthorizationAuthority, GIT_AUTH_DEFAULT_TTL, GitMutationAuthorization,
    authorization_commit_base,
};
pub use drift::{DriftStatus, TreeHashDriftDetector, TreeSnapshot};
pub use integration::{
    IntegrationReport, IntegrationState, MergeStrategy, WorktreeIntegrationStateMachine,
};
pub use probe::GitWorkspaceInfo;
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
    /// Unstage paths from the index without moving HEAD
    /// (`git reset -q -- <paths...>`). Never touches the working tree or
    /// refs; still authorization-bound so a stale grant for one path set
    /// cannot unstage another.
    Unstage {
        paths: Vec<String>,
    },
    Commit {
        message: String,
    },
    /// Abort an in-progress merge (`git merge --abort`). Restores the
    /// pre-merge HEAD; requires explicit authorization because it discards
    /// merge state.
    MergeAbort,
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
    /// Create an isolated worktree (`git worktree add`). Binds the exact
    /// worktree path and branch so the authorization cannot be replayed to
    /// materialize a different worktree.
    WorktreeAdd {
        path: String,
        branch: String,
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
                | Self::WorktreeAdd { .. }
                | Self::BranchDelete { .. }
                | Self::UpdateRef { .. }
                | Self::Merge { .. }
                | Self::MergeAbort
                | Self::StashApply
                | Self::SyncCheckout { .. }
                | Self::RawPassthrough
        )
    }

    /// Whether this operation mutates repository state and therefore requires
    /// a bound [`GitMutationAuthorization`](crate::git::GitMutationAuthorization).
    /// Read-only operations (`Status`, `Diff`, `Log`, `Show`, `Branch`) never
    /// require authorization; every mutation does — including `Commit` and
    /// `Add`, whose default policy decision is `Allow` but whose execution
    /// must still trace to a live runtime authorization.
    pub fn requires_authorization(&self) -> bool {
        !matches!(
            self,
            Self::Status | Self::Diff { .. } | Self::Log { .. } | Self::Show { .. } | Self::Branch
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
            | Self::WorktreeAdd { .. }
            | Self::BranchDelete { .. }
            | Self::UpdateRef { .. }
            | Self::Merge { .. }
            | Self::MergeAbort
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

/// Explicit authorization gate for Git mutations.
///
/// There is no boolean approval primitive: a gate is either `denied()` (every
/// mutation fails closed) or bound to a [`GitMutationAuthorization`] minted by
/// the runtime [`AuthorizationAuthority`](crate::git::AuthorizationAuthority)
/// for the exact operation, workspace, mission, task, and policy generation
/// being executed. Construction verifies the authorization tag, expiry,
/// operation binding, workspace binding, policy hash, and provenance; every
/// `enforce` call re-verifies the operation and workspace binding.
///
/// Stale or replayed authorizations fail closed: an authorization minted for
/// one operation, workspace, or policy hash never verifies for another.
#[derive(Debug, Clone)]
pub struct GitGate {
    authorization: Option<GitMutationAuthorization>,
    authority_tag_verified: bool,
}

impl GitGate {
    /// Bind a runtime-minted authorization to this gate.
    ///
    /// Verifies the authorization tag against `authority` immediately:
    /// authorizations minted by any other authority instance (or tampered
    /// after minting) are rejected here, before any mutation can execute.
    /// Full operation/workspace binding is re-checked on every `enforce`.
    pub fn authorized_verified(
        authorization: GitMutationAuthorization,
        authority: &AuthorizationAuthority,
    ) -> Result<Self, GitError> {
        if !authority.verify(&authorization) {
            return Err(GitError::PolicyViolation(
                "git authorization tag mismatch: not minted by this runtime authority".to_string(),
            ));
        }
        if authorization.is_expired() {
            return Err(GitError::PolicyViolation(
                "git authorization expired".to_string(),
            ));
        }
        if authorization.policy_hash.trim().is_empty() {
            return Err(GitError::PolicyViolation(
                "git authorization carries no policy hash".to_string(),
            ));
        }
        if authorization.provenance.trim().is_empty() {
            return Err(GitError::PolicyViolation(
                "git authorization carries no approval provenance".to_string(),
            ));
        }
        Ok(Self {
            authorization: Some(authorization),
            authority_tag_verified: true,
        })
    }

    /// No approval: every mutation fails closed.
    pub fn denied() -> Self {
        Self {
            authorization: None,
            authority_tag_verified: false,
        }
    }

    /// The bound authorization, if any.
    pub fn authorization(&self) -> Option<&GitMutationAuthorization> {
        self.authorization.as_ref()
    }

    /// Enforce the canonical policy for `op` in `workspace_root` under this
    /// gate's authorization. Fails closed when the gate is denied, when the
    /// authorization covers a different operation or workspace, or when the
    /// operation is denied by the safety veto.
    pub fn enforce(
        &self,
        op: &GitOperation,
        workspace_root: &std::path::Path,
    ) -> Result<(), GitError> {
        // Absolute vetoes are immutable regardless of authorization.
        if matches!(op.default_policy_decision(), PolicyDecision::Deny) {
            return Err(GitError::PolicyViolation(format!(
                "Operation {op:?} is denied by safety policy veto"
            )));
        }
        if op.requires_authorization() {
            let auth = self.authorization.as_ref().ok_or_else(|| {
                GitError::PolicyViolation(format!(
                    "Operation {op:?} requires a bound runtime execution authorization"
                ))
            })?;
            if !self.authority_tag_verified {
                return Err(GitError::PolicyViolation(
                    "git gate authorization was never verified against the runtime authority"
                        .to_string(),
                ));
            }
            if !auth.covers(op, workspace_root) {
                return Err(GitError::PolicyViolation(format!(
                    "git authorization does not cover {op:?} in '{}'",
                    workspace_root.display()
                )));
            }
            return Ok(());
        }
        // Read-only operations never require authorization, but a denied gate
        // still honors Ask-gated remote reads via the legacy boolean path.
        op.enforce_policy(self.authorization.is_some())
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
