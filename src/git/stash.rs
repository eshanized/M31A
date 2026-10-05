//! Private Git stash manager (GST-01, D-06).

use std::path::PathBuf;

use crate::git::{GIT_COMMAND_TIMEOUT, GitError, GitGate, GitOperation, run_scoped_git};
use crate::ids::MissionId;

/// Manages private mission stash entries isolated under `refs/m31a/stash/<mission_id>`.
pub struct PrivateStashManager {
    repo_root: PathBuf,
}

impl PrivateStashManager {
    pub fn new(repo_root: impl Into<PathBuf>) -> Self {
        Self {
            repo_root: repo_root.into(),
        }
    }

    /// Canonical private ref for a mission's stash: `refs/m31a/stash/<mission_id>`.
    pub fn stash_ref(mission_id: &MissionId) -> String {
        format!("refs/m31a/stash/{mission_id}")
    }

    /// Creates a private stash using `git stash create` and updates `refs/m31a/stash/<mission_id>`.
    /// Does NOT touch `refs/stash` and never alters the operator's stash stack.
    /// If changes were saved, resets the working tree to HEAD so local changes are set aside.
    /// Returns the stash commit SHA if created, or None if working tree was clean.
    ///
    /// The working-tree reset requires a [`GitGate`] bound to a live runtime
    /// execution authorization covering exactly these stash operations in this
    /// repository (`SyncCheckout` policy); `GitGate::denied()` fails closed
    /// before any mutation. The optional message is validated (never an option).
    pub async fn save(
        &self,
        mission_id: &MissionId,
        message: Option<&str>,
        gate: &GitGate,
    ) -> Result<Option<String>, GitError> {
        gate.enforce(&GitOperation::StashSave, &self.repo_root)?;
        // `git stash create` accepts no message argument; a leading-dash or
        // control-character message would be misparsed as an option, so
        // validate the untrusted edge (spaces are legitimate in messages).
        if let Some(msg) = message {
            if msg.is_empty() || msg.starts_with('-') || msg.chars().any(|c| c.is_control()) {
                return Err(GitError::InvalidRef(format!(
                    "refusing stash message that could inject options: '{msg}'"
                )));
            }
        }
        let mut args = vec!["stash", "create"];
        // NOTE: `stash create` ignores trailing message args; the message is
        // accepted for API compatibility and validated above, then passed
        // through unchanged to preserve historical CLI behavior.
        let owned: Vec<String>;
        if let Some(msg) = message {
            owned = vec![msg.to_string()];
            args.push(&owned[0]);
        }
        let output = run_scoped_git(&self.repo_root, &args, GIT_COMMAND_TIMEOUT).await?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
            return Err(GitError::CommandFailed {
                exit_code: output.status.code(),
                message: format!("git stash create failed: {stderr}"),
            });
        }

        let commit_sha = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if commit_sha.is_empty() {
            return Ok(None);
        }
        crate::git::validate_git_sha("stash commit", &commit_sha)?;

        let ref_name = Self::stash_ref(mission_id);
        gate.enforce(
            &GitOperation::UpdateRef {
                git_ref: ref_name.clone(),
            },
            &self.repo_root,
        )?;
        let update_out = run_scoped_git(
            &self.repo_root,
            &["update-ref", &ref_name, &commit_sha],
            GIT_COMMAND_TIMEOUT,
        )
        .await?;

        if !update_out.status.success() {
            let stderr = String::from_utf8_lossy(&update_out.stderr).into_owned();
            return Err(GitError::CommandFailed {
                exit_code: update_out.status.code(),
                message: format!("git update-ref failed for private stash: {stderr}"),
            });
        }

        // Reset the working tree to clean up working copy without touching refs/stash
        gate.enforce(
            &GitOperation::SyncCheckout {
                target: "HEAD".to_string(),
            },
            &self.repo_root,
        )?;
        let reset_out = run_scoped_git(
            &self.repo_root,
            &["reset", "--hard", "HEAD"],
            GIT_COMMAND_TIMEOUT,
        )
        .await?;

        if !reset_out.status.success() {
            let stderr = String::from_utf8_lossy(&reset_out.stderr).into_owned();
            return Err(GitError::CommandFailed {
                exit_code: reset_out.status.code(),
                message: format!("git reset after stash save failed: {stderr}"),
            });
        }

        Ok(Some(commit_sha))
    }

    /// Applies the private stash commit onto the current working tree.
    ///
    /// Because `stash apply` can overwrite worktree files, it requires
    /// a [`GitGate`] bound to a live runtime execution authorization
    /// covering `StashApply` in this repository.
    pub async fn apply(&self, mission_id: &MissionId, gate: &GitGate) -> Result<(), GitError> {
        gate.enforce(&GitOperation::StashApply, &self.repo_root)?;
        let ref_name = Self::stash_ref(mission_id);
        let output = run_scoped_git(
            &self.repo_root,
            &["stash", "apply", &ref_name],
            GIT_COMMAND_TIMEOUT,
        )
        .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
            return Err(GitError::CommandFailed {
                exit_code: output.status.code(),
                message: format!("git stash apply failed for private stash: {stderr}"),
            });
        }

        Ok(())
    }

    /// Applies and removes the private stash entry.
    pub async fn pop(&self, mission_id: &MissionId, gate: &GitGate) -> Result<(), GitError> {
        self.apply(mission_id, gate).await?;
        self.drop(mission_id, gate).await
    }

    /// Deletes the private stash ref.
    pub async fn drop(&self, mission_id: &MissionId, gate: &GitGate) -> Result<(), GitError> {
        let ref_name = Self::stash_ref(mission_id);
        gate.enforce(
            &GitOperation::UpdateRef {
                git_ref: ref_name.clone(),
            },
            &self.repo_root,
        )?;
        let output = run_scoped_git(
            &self.repo_root,
            &["update-ref", "-d", &ref_name],
            GIT_COMMAND_TIMEOUT,
        )
        .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
            return Err(GitError::CommandFailed {
                exit_code: output.status.code(),
                message: format!("failed to drop private stash: {stderr}"),
            });
        }

        Ok(())
    }

    /// Checks if a private stash ref exists.
    pub async fn has_stash(&self, mission_id: &MissionId) -> Result<bool, GitError> {
        let ref_name = Self::stash_ref(mission_id);
        let output = run_scoped_git(
            &self.repo_root,
            &["show-ref", "--verify", "--quiet", &ref_name],
            GIT_COMMAND_TIMEOUT,
        )
        .await?;

        Ok(output.status.success())
    }
}
