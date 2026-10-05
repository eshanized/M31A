//! Git Worktree Isolation Manager (GST-01, D-01).

use chrono::{DateTime, Utc};
use std::path::PathBuf;

use crate::git::{
    GIT_COMMAND_TIMEOUT, GitError, GitGate, GitOperation, run_scoped_git, validate_git_ref_arg,
};
use crate::ids::MissionId;

/// Policy for retaining worktrees after mission completion or failure (D-01).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum WorktreeRetentionPolicy {
    /// Keep the worktree on failure for developer inspection, delete on success.
    #[default]
    KeepOnFailure,
    /// Always remove the worktree regardless of outcome.
    AlwaysRemove,
    /// Always keep the worktree (useful for debugging/auditing).
    AlwaysKeep,
}

/// Configuration for the WorktreeManager.
#[derive(Debug, Clone)]
pub struct WorktreeConfig {
    /// Root path of the main Git repository.
    pub repo_root: PathBuf,
    /// Subdirectory for worktrees relative to repo_root (default: `.m31a/worktrees`).
    pub worktrees_dir: PathBuf,
    /// Retention policy for worktrees.
    pub retention_policy: WorktreeRetentionPolicy,
}

impl WorktreeConfig {
    pub fn new(repo_root: impl Into<PathBuf>) -> Self {
        Self {
            repo_root: repo_root.into(),
            worktrees_dir: PathBuf::from(".m31a/worktrees"),
            retention_policy: WorktreeRetentionPolicy::default(),
        }
    }

    pub fn with_retention(mut self, policy: WorktreeRetentionPolicy) -> Self {
        self.retention_policy = policy;
        self
    }

    pub fn with_worktrees_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.worktrees_dir = dir.into();
        self
    }
}

/// Represents an isolated Git worktree dedicated to a mission (D-01).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct IsolatedWorktree {
    pub mission_id: MissionId,
    pub path: PathBuf,
    pub branch: String,
    pub base_commit: String,
    pub created_at: DateTime<Utc>,
}

/// Manages creation, lifecycle, and cleanup of isolated Git worktrees.
pub struct WorktreeManager {
    config: WorktreeConfig,
}

impl WorktreeManager {
    pub fn new(config: WorktreeConfig) -> Self {
        Self { config }
    }

    pub fn config(&self) -> &WorktreeConfig {
        &self.config
    }

    /// Resolve absolute worktree path for a given mission.
    pub fn worktree_path(&self, mission_id: &MissionId) -> PathBuf {
        if self.config.worktrees_dir.is_absolute() {
            self.config.worktrees_dir.join(mission_id.to_string())
        } else {
            self.config
                .repo_root
                .join(&self.config.worktrees_dir)
                .join(mission_id.to_string())
        }
    }

    /// Canonical branch name for a mission: `m31a/<mission_id>` (D-01, D-06).
    pub fn branch_name(mission_id: &MissionId) -> String {
        format!("m31a/{mission_id}")
    }

    /// Ensures that common build and runtime artifact directories are excluded from Git tracking
    /// via `.git/info/exclude` in the common Git directory.
    pub async fn ensure_git_excludes(repo_root: &std::path::Path) -> Result<(), GitError> {
        let git_dir = repo_root.join(".git");
        if !git_dir.exists() {
            return Ok(());
        }

        // In a worktree or standard repo, find the gitdir info directory
        let info_dir = if git_dir.is_file() {
            // It's a worktree (.git is a file containing `gitdir: <path>`); resolve target gitdir
            if let Ok(content) = tokio::fs::read_to_string(&git_dir).await {
                if let Some(path_str) = content.strip_prefix("gitdir: ") {
                    let p = PathBuf::from(path_str.trim());
                    // In Git worktrees, p is `<common-gitdir>/worktrees/<id>`
                    p.parent()
                        .and_then(|parent| parent.parent())
                        .map(|cd| cd.join("info"))
                        .unwrap_or_else(|| p.join("info"))
                } else {
                    git_dir.join("info")
                }
            } else {
                git_dir.join("info")
            }
        } else {
            git_dir.join("info")
        };

        let exclude_file = info_dir.join("exclude");
        tokio::fs::create_dir_all(&info_dir)
            .await
            .map_err(|e| GitError::Worktree(format!("git exclude dir setup failed: {e}")))?;

        let existing = tokio::fs::read_to_string(&exclude_file)
            .await
            .unwrap_or_default();

        let patterns_to_add = [
            ".m31a/",
            "target/",
            "node_modules/",
            "__pycache__/",
            "*.pyc",
        ];

        let mut to_append = String::new();
        for pat in &patterns_to_add {
            if !existing.lines().any(|l| l.trim() == *pat) {
                to_append.push_str(pat);
                to_append.push('\n');
            }
        }

        if !to_append.is_empty() {
            use tokio::io::AsyncWriteExt;
            let mut file = tokio::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&exclude_file)
                .await
                .map_err(|e| GitError::Worktree(format!("git exclude file open failed: {e}")))?;
            file.write_all(to_append.as_bytes())
                .await
                .map_err(|e| GitError::Worktree(format!("git exclude file write failed: {e}")))?;
        }

        Ok(())
    }

    /// Creates an isolated worktree at `.m31a/worktrees/<mission_id>` on branch `m31a/<mission_id>`.
    ///
    /// Worktree creation mutates repository state (`worktree add` + branch
    /// creation) and requires a [`GitGate`] bound to a live runtime execution
    /// authorization covering exactly this path and branch;
    /// `GitGate::denied()` fails closed before any mutation.
    pub async fn create_worktree(
        &self,
        mission_id: &MissionId,
        base_commit: Option<&str>,
        gate: &GitGate,
    ) -> Result<IsolatedWorktree, GitError> {
        // Runtime-owned hygiene: keep mission scratch dirs out of commits.
        // Failures propagate — a worktree whose excludes cannot be written
        // must not silently accept dirty commits.
        Self::ensure_git_excludes(&self.config.repo_root).await?;

        let base = match base_commit {
            Some(c) => c.to_string(),
            None => self.resolve_head().await?,
        };

        let target_path = self.worktree_path(mission_id);
        let branch = Self::branch_name(mission_id);

        if let Some(parent) = target_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        // Check if branch already exists
        let branch_exists = self.check_branch_exists(&branch).await?;

        // Non-UTF8 worktree paths fail closed instead of causing undefined behavior.
        let target_str = target_path
            .to_str()
            .ok_or_else(|| GitError::Worktree("non-UTF8 isolated worktree path".to_string()))?;
        // Validate dynamic ref inputs before interpolation.
        validate_git_ref_arg("worktree branch", &branch)?;
        validate_git_ref_arg("worktree base", &base)?;
        // Authorization is enforced before the mutation: the gate must cover
        // exactly this worktree path and branch in this repository.
        gate.enforce(
            &GitOperation::WorktreeAdd {
                path: target_str.to_string(),
                branch: branch.clone(),
            },
            &self.config.repo_root,
        )?;
        let output = if branch_exists {
            run_scoped_git(
                &self.config.repo_root,
                &["worktree", "add", target_str, &branch],
                GIT_COMMAND_TIMEOUT,
            )
            .await?
        } else {
            run_scoped_git(
                &self.config.repo_root,
                &["worktree", "add", "-b", &branch, target_str, &base],
                GIT_COMMAND_TIMEOUT,
            )
            .await?
        };

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
            return Err(GitError::CommandFailed {
                exit_code: output.status.code(),
                message: format!("git worktree add failed: {stderr}"),
            });
        }

        Ok(IsolatedWorktree {
            mission_id: *mission_id,
            path: target_path,
            branch,
            base_commit: base,
            created_at: Utc::now(),
        })
    }

    /// Remove a worktree and cleans up references.
    ///
    /// Worktree removal and branch deletion require a [`GitGate`] bound to a
    /// live runtime execution authorization covering exactly these operations
    /// in this repository; `GitGate::denied()` fails closed before any
    /// mutation.
    pub async fn remove_worktree(
        &self,
        worktree: &IsolatedWorktree,
        force: bool,
        gate: &GitGate,
    ) -> Result<(), GitError> {
        gate.enforce(
            &GitOperation::WorktreeRemove { force },
            &self.config.repo_root,
        )?;
        // Non-UTF8 paths and leading-dash branches fail closed.
        let path_str = worktree
            .path
            .to_str()
            .ok_or_else(|| GitError::Worktree("non-UTF8 isolated worktree path".to_string()))?;
        if path_str.is_empty() || path_str.starts_with('-') {
            return Err(GitError::InvalidRef(format!(
                "refusing worktree remove on suspicious path '{path_str}'"
            )));
        }
        validate_git_ref_arg("worktree branch", &worktree.branch)?;
        let mut args = vec!["worktree", "remove"];
        if force {
            args.push("--force");
        }
        args.push(path_str);

        let output = run_scoped_git(&self.config.repo_root, &args, GIT_COMMAND_TIMEOUT).await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
            // If worktree remove failed but force is true, try to clean up manually
            if force && worktree.path.exists() {
                tokio::fs::remove_dir_all(&worktree.path)
                    .await
                    .map_err(|e| {
                        GitError::Worktree(format!(
                            "git worktree remove failed ({stderr}) and manual cleanup failed: {e}"
                        ))
                    })?;
            } else {
                return Err(GitError::CommandFailed {
                    exit_code: output.status.code(),
                    message: format!("git worktree remove failed: {stderr}"),
                });
            }
        }

        // Delete the mission branch if force or requested
        if force {
            gate.enforce(
                &GitOperation::BranchDelete {
                    branch: worktree.branch.clone(),
                },
                &self.config.repo_root,
            )?;
            let branch_out = run_scoped_git(
                &self.config.repo_root,
                &["branch", "-D", &worktree.branch],
                GIT_COMMAND_TIMEOUT,
            )
            .await?;
            if !branch_out.status.success() {
                let stderr = String::from_utf8_lossy(&branch_out.stderr).into_owned();
                return Err(GitError::CommandFailed {
                    exit_code: branch_out.status.code(),
                    message: format!("git branch -D failed: {stderr}"),
                });
            }
        }

        // Prune stale worktrees
        self.prune_worktrees().await?;

        Ok(())
    }

    /// Prune stale worktree administrative metadata.
    pub async fn prune_worktrees(&self) -> Result<usize, GitError> {
        let output = run_scoped_git(
            &self.config.repo_root,
            &["worktree", "prune"],
            GIT_COMMAND_TIMEOUT,
        )
        .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
            return Err(GitError::CommandFailed {
                exit_code: output.status.code(),
                message: format!("git worktree prune failed: {stderr}"),
            });
        }

        Ok(0)
    }

    /// List all registered worktrees via `git worktree list --porcelain`.
    pub async fn list_worktrees(&self) -> Result<Vec<PathBuf>, GitError> {
        let output = run_scoped_git(
            &self.config.repo_root,
            &["worktree", "list", "--porcelain"],
            GIT_COMMAND_TIMEOUT,
        )
        .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
            return Err(GitError::CommandFailed {
                exit_code: output.status.code(),
                message: format!("git worktree list failed: {stderr}"),
            });
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut worktrees = Vec::new();
        for line in stdout.lines() {
            if let Some(path_str) = line.strip_prefix("worktree ") {
                worktrees.push(PathBuf::from(path_str.trim()));
            }
        }

        Ok(worktrees)
    }

    /// Run an arbitrary git command inside the isolated worktree directory.
    ///
    /// The arbitrary-argument passthrough requires a [`GitGate`] bound to a
    /// live runtime execution authorization covering
    /// `GitOperation::RawPassthrough` in exactly this worktree directory.
    /// Only callers holding a genuine bound authorization may use this escape
    /// hatch.
    pub async fn run_git_in_worktree(
        &self,
        worktree: &IsolatedWorktree,
        args: &[&str],
        gate: &GitGate,
    ) -> Result<String, GitError> {
        gate.enforce(&GitOperation::RawPassthrough, &worktree.path)?;
        for arg in args {
            if arg.is_empty() {
                return Err(GitError::InvalidRef(
                    "refusing git passthrough with empty argument".to_string(),
                ));
            }
        }
        let output = run_scoped_git(&worktree.path, args, GIT_COMMAND_TIMEOUT).await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
            return Err(GitError::CommandFailed {
                exit_code: output.status.code(),
                message: stderr,
            });
        }

        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }

    async fn resolve_head(&self) -> Result<String, GitError> {
        let output = run_scoped_git(
            &self.config.repo_root,
            &["rev-parse", "HEAD"],
            GIT_COMMAND_TIMEOUT,
        )
        .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
            return Err(GitError::CommandFailed {
                exit_code: output.status.code(),
                message: format!("git rev-parse HEAD failed: {stderr}"),
            });
        }

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    async fn check_branch_exists(&self, branch: &str) -> Result<bool, GitError> {
        // `refs/heads/` prefix neuters leading-dash injection; empty branch
        // yields `refs/heads/` which git rejects as an error, not an option.
        let output = run_scoped_git(
            &self.config.repo_root,
            &[
                "show-ref",
                "--verify",
                "--quiet",
                &format!("refs/heads/{branch}"),
            ],
            GIT_COMMAND_TIMEOUT,
        )
        .await?;

        Ok(output.status.success())
    }
}
