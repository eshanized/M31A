//! Two-stage Worktree Integration State Machine (GST-01, GST-04, D-05).

use std::path::PathBuf;

use crate::git::worktree::IsolatedWorktree;
use crate::git::{
    GIT_COMMAND_TIMEOUT, GitError, GitGate, GitOperation, run_scoped_git, validate_git_ref_arg,
    validate_git_sha,
};
use crate::ids::MissionId;

/// Merge strategy for worktree branch integration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum MergeStrategy {
    FastForwardOnly,
    MergeCommit,
    PreferFastForward,
}

/// Lifecycle states of the two-stage integration state machine (D-05).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum IntegrationState {
    MissionCompleted,
    IntegrationPending,
    Integrating,
    Integrated,
    IntegrationConflict,
}

/// Structured report of an integration attempt.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct IntegrationReport {
    pub mission_id: MissionId,
    pub target_branch: String,
    pub source_branch: String,
    pub state: IntegrationState,
    pub merge_commit: Option<String>,
    pub conflict_files: Vec<String>,
    pub message: String,
}

/// State machine coordinating two-stage merge validation in an ephemeral worktree before target branch mutation.
pub struct WorktreeIntegrationStateMachine {
    repo_root: PathBuf,
    state: IntegrationState,
}

impl WorktreeIntegrationStateMachine {
    pub fn new(repo_root: impl Into<PathBuf>) -> Self {
        Self {
            repo_root: repo_root.into(),
            state: IntegrationState::MissionCompleted,
        }
    }

    pub fn state(&self) -> IntegrationState {
        self.state
    }

    /// Two-stage integration:
    /// Stage 1: Ephemeral temporary integration worktree created with `--detach` from target_branch.
    /// Stage 2: Attempt merge of source branch.
    /// If conflict: state -> IntegrationConflict, report conflicts, abort and remove temp worktree without touching target branch.
    /// If success: state -> Integrated, commit merge on target branch, cleanup temp worktree.
    ///
    /// Strategy enforcement:
    /// - `FastForwardOnly` runs `merge --ff-only` and never creates a merge commit.
    /// - `MergeCommit` runs `merge --no-commit --no-ff` followed by an explicit commit.
    /// - `PreferFastForward` attempts `merge --ff-only` first and falls back to
    ///   the merge-commit path only when fast-forward is impossible.
    ///
    /// Destructive steps (merge, `update-ref`, checkout sync, forced worktree
    /// removal) require a [`GitGate`] bound to a live runtime execution
    /// authorization covering exactly this integration's operations in this
    /// repository; `GitGate::denied()` fails closed before any mutation.
    pub async fn integrate(
        &mut self,
        worktree: &IsolatedWorktree,
        target_branch: &str,
        strategy: MergeStrategy,
        gate: &GitGate,
    ) -> Result<IntegrationReport, GitError> {
        self.state = IntegrationState::IntegrationPending;
        self.state = IntegrationState::Integrating;

        // Validate every dynamic ref before interpolation. No branch
        // name or ref string may become an option.
        validate_git_ref_arg("integration target branch", target_branch)?;
        let mission_id = worktree.mission_id;
        let source_branch = &worktree.branch;
        validate_git_ref_arg("integration source branch", source_branch)?;
        gate.enforce(
            &GitOperation::Merge {
                source: source_branch.clone(),
            },
            &self.repo_root,
        )?;

        let temp_dir_name = format!(".m31a/temp_integration_{mission_id}");
        let temp_path = self.repo_root.join(&temp_dir_name);

        if let Some(parent) = temp_path.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(|e| {
                GitError::Worktree(format!("temp worktree parent setup failed: {e}"))
            })?;
        }

        // Non-UTF8 worktree paths fail closed with a typed error instead
        // of panicking production on to_str().unwrap().
        let temp_path_str = temp_path.to_str().ok_or_else(|| {
            GitError::Worktree("non-UTF8 temporary integration worktree path".to_string())
        })?;
        if temp_path_str.is_empty() || temp_path_str.starts_with('-') {
            return Err(GitError::InvalidRef(format!(
                "refusing integration in suspicious path '{temp_path_str}'"
            )));
        }
        // Clean up any stale worktree at temp_path first
        gate.enforce(
            &GitOperation::WorktreeRemove { force: true },
            &self.repo_root,
        )?;
        let stale_out = run_scoped_git(
            &self.repo_root,
            &["worktree", "remove", "--force", temp_path_str],
            GIT_COMMAND_TIMEOUT,
        )
        .await?;
        if temp_path.exists() {
            tokio::fs::remove_dir_all(&temp_path).await.map_err(|e| {
                GitError::Worktree(format!("stale temp worktree cleanup failed: {e}"))
            })?;
        }
        let _ = stale_out;

        // Stage 1: Create ephemeral temporary integration worktree in detached HEAD from target_branch
        let add_out = run_scoped_git(
            &self.repo_root,
            &["worktree", "add", "--detach", temp_path_str, target_branch],
            GIT_COMMAND_TIMEOUT,
        )
        .await?;

        if !add_out.status.success() {
            self.state = IntegrationState::IntegrationConflict;
            let stderr = String::from_utf8_lossy(&add_out.stderr).into_owned();
            return Err(GitError::Worktree(format!(
                "Failed to create ephemeral integration worktree: {stderr}"
            )));
        }

        // Stage 2: Attempt merge honoring the requested strategy.
        let want_ff = matches!(
            strategy,
            MergeStrategy::FastForwardOnly | MergeStrategy::PreferFastForward
        );
        let mut merged_ff = false;
        let merge_out = if want_ff {
            let out = run_scoped_git(
                &temp_path,
                &["merge", "--ff-only", source_branch],
                GIT_COMMAND_TIMEOUT,
            )
            .await?;
            merged_ff = out.status.success();
            // PreferFastForward falls through to the merge-commit path when
            // fast-forward is impossible; FastForwardOnly reports the result.
            if merged_ff || strategy == MergeStrategy::FastForwardOnly {
                out
            } else {
                run_scoped_git(
                    &temp_path,
                    &["merge", "--no-commit", "--no-ff", source_branch],
                    GIT_COMMAND_TIMEOUT,
                )
                .await?
            }
        } else {
            run_scoped_git(
                &temp_path,
                &["merge", "--no-commit", "--no-ff", source_branch],
                GIT_COMMAND_TIMEOUT,
            )
            .await?
        };

        if !merge_out.status.success() {
            // FastForwardOnly reports impossibility directly: `merge
            // --ff-only` leaves no merge state behind, so there is nothing
            // to abort and no conflict files to collect. The target ref is
            // untouched by construction.
            if strategy == MergeStrategy::FastForwardOnly {
                self.cleanup_temp_worktree(temp_path_str, &temp_path)
                    .await?;
                self.state = IntegrationState::IntegrationConflict;
                return Ok(IntegrationReport {
                    mission_id,
                    target_branch: target_branch.to_string(),
                    source_branch: source_branch.to_string(),
                    state: IntegrationState::IntegrationConflict,
                    merge_commit: None,
                    conflict_files: Vec::new(),
                    message:
                        "Fast-forward merge not possible for the requested FastForwardOnly strategy"
                            .to_string(),
                });
            }
            // Conflicts occurred! Collect conflicting files
            let diff_out = run_scoped_git(
                &temp_path,
                &["diff", "--name-only", "--diff-filter=U"],
                GIT_COMMAND_TIMEOUT,
            )
            .await?;

            let mut conflict_files = Vec::new();
            if diff_out.status.success() {
                let diff_str = String::from_utf8_lossy(&diff_out.stdout);
                for line in diff_str.lines() {
                    let f = line.trim();
                    if !f.is_empty() {
                        conflict_files.push(f.to_string());
                    }
                }
            }

            // Abort merge in temp worktree
            let abort_out =
                run_scoped_git(&temp_path, &["merge", "--abort"], GIT_COMMAND_TIMEOUT).await?;
            if !abort_out.status.success() {
                let stderr = String::from_utf8_lossy(&abort_out.stderr).into_owned();
                self.cleanup_temp_worktree(temp_path_str, &temp_path)
                    .await?;
                self.state = IntegrationState::IntegrationConflict;
                return Err(GitError::IntegrationConflict(format!(
                    "merge conflict on '{source_branch}' and abort failed ({stderr}); target untouched"
                )));
            }

            // Remove temp worktree
            self.cleanup_temp_worktree(temp_path_str, &temp_path)
                .await?;

            self.state = IntegrationState::IntegrationConflict;

            return Ok(IntegrationReport {
                mission_id,
                target_branch: target_branch.to_string(),
                source_branch: source_branch.to_string(),
                state: IntegrationState::IntegrationConflict,
                merge_commit: None,
                conflict_files,
                message: "Merge conflict detected during integration validation".to_string(),
            });
        }

        // Merge succeeded in temporary worktree. Fast-forward needs no commit;
        // merge-commit strategies finalize with an explicit commit.
        let merge_commit = if merged_ff {
            let rev_out =
                run_scoped_git(&temp_path, &["rev-parse", "HEAD"], GIT_COMMAND_TIMEOUT).await?;
            if !rev_out.status.success() {
                let stderr = String::from_utf8_lossy(&rev_out.stderr).into_owned();
                self.cleanup_temp_worktree(temp_path_str, &temp_path)
                    .await?;
                self.state = IntegrationState::IntegrationConflict;
                return Err(GitError::Worktree(format!(
                    "fast-forward merge succeeded but rev-parse failed: {stderr}"
                )));
            }
            Some(String::from_utf8_lossy(&rev_out.stdout).trim().to_string())
        } else {
            let commit_msg = format!("Merge branch '{source_branch}' into '{target_branch}'");
            let commit_out = run_scoped_git(
                &temp_path,
                &["commit", "-m", &commit_msg],
                GIT_COMMAND_TIMEOUT,
            )
            .await?;

            if commit_out.status.success() {
                let rev_out =
                    run_scoped_git(&temp_path, &["rev-parse", "HEAD"], GIT_COMMAND_TIMEOUT).await?;
                Some(String::from_utf8_lossy(&rev_out.stdout).trim().to_string())
            } else {
                None
            }
        };

        // Update target branch reference in the repository to the new merge commit.
        // The update-ref result is checked (never discarded) and the SHA is
        // validated before it touches a ref.
        if let Some(ref sha) = merge_commit {
            validate_git_sha("integration merge commit", sha)?;
            gate.enforce(
                &GitOperation::UpdateRef {
                    git_ref: format!("refs/heads/{target_branch}"),
                },
                &self.repo_root,
            )?;
            let target_ref = format!("refs/heads/{target_branch}");
            let update_out = run_scoped_git(
                &self.repo_root,
                &["update-ref", &target_ref, sha],
                GIT_COMMAND_TIMEOUT,
            )
            .await?;
            if !update_out.status.success() {
                let stderr = String::from_utf8_lossy(&update_out.stderr).into_owned();
                self.cleanup_temp_worktree(temp_path_str, &temp_path)
                    .await?;
                self.state = IntegrationState::IntegrationConflict;
                return Err(GitError::CommandFailed {
                    exit_code: update_out.status.code(),
                    message: format!("git update-ref failed for '{target_ref}': {stderr}"),
                });
            }

            // If main working tree is on target_branch, sync working tree.
            // Reset to the exact integrated SHA (never a bare HEAD that
            // silently no-ops after the ref move).
            let cur_branch_out = run_scoped_git(
                &self.repo_root,
                &["rev-parse", "--abbrev-ref", "HEAD"],
                GIT_COMMAND_TIMEOUT,
            )
            .await?;
            if String::from_utf8_lossy(&cur_branch_out.stdout).trim() == target_branch {
                gate.enforce(
                    &GitOperation::SyncCheckout {
                        target: target_branch.to_string(),
                    },
                    &self.repo_root,
                )?;
                let reset_out = run_scoped_git(
                    &self.repo_root,
                    &["reset", "--hard", sha],
                    GIT_COMMAND_TIMEOUT,
                )
                .await?;
                if !reset_out.status.success() {
                    let stderr = String::from_utf8_lossy(&reset_out.stderr).into_owned();
                    self.cleanup_temp_worktree(temp_path_str, &temp_path)
                        .await?;
                    self.state = IntegrationState::IntegrationConflict;
                    return Err(GitError::CommandFailed {
                        exit_code: reset_out.status.code(),
                        message: format!(
                            "ref '{target_ref}' moved to {sha} but checkout sync failed: {stderr}"
                        ),
                    });
                }
            }
        }

        // Remove ephemeral temp worktree. Cleanup failure is a typed error,
        // never silently ignored.
        self.cleanup_temp_worktree(temp_path_str, &temp_path)
            .await?;

        self.state = IntegrationState::Integrated;

        Ok(IntegrationReport {
            mission_id,
            target_branch: target_branch.to_string(),
            source_branch: source_branch.to_string(),
            state: IntegrationState::Integrated,
            merge_commit,
            conflict_files: Vec::new(),
            message: "Integration validated and completed cleanly".to_string(),
        })
    }

    /// Remove the ephemeral integration worktree, failing closed with a typed
    /// error when neither `worktree remove --force` nor manual directory
    /// removal succeeds.
    async fn cleanup_temp_worktree(
        &self,
        temp_path_str: &str,
        temp_path: &std::path::Path,
    ) -> Result<(), GitError> {
        let remove_out = run_scoped_git(
            &self.repo_root,
            &["worktree", "remove", "--force", temp_path_str],
            GIT_COMMAND_TIMEOUT,
        )
        .await?;
        if !remove_out.status.success() && temp_path.exists() {
            tokio::fs::remove_dir_all(temp_path)
                .await
                .map_err(|e| GitError::Worktree(format!("temp worktree cleanup failed: {e}")))?;
        } else if temp_path.exists() {
            tokio::fs::remove_dir_all(temp_path)
                .await
                .map_err(|e| GitError::Worktree(format!("temp worktree cleanup failed: {e}")))?;
        }
        if temp_path.exists() {
            return Err(GitError::Worktree(format!(
                "temp worktree at '{temp_path_str}' still exists after cleanup"
            )));
        }
        Ok(())
    }
}
