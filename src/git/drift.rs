//! Cryptographic Tree-Hash Drift Detection (GST-04, D-04).

use chrono::{DateTime, Utc};
use std::path::Path;
use std::sync::Arc;

use crate::events::bus::EventBus;
use crate::events::envelope::EventEnvelope;
use crate::events::types::EventType;
use crate::git::{GIT_COMMAND_TIMEOUT, GitError, run_scoped_git, scoped_git_command};
use crate::ids::MissionId;

/// Snapshot of the repository/worktree state at a point in time.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TreeSnapshot {
    pub head_commit: String,
    pub tree_hash: String,
    pub recorded_at: DateTime<Utc>,
}

/// Status of drift comparison against expected snapshot.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum DriftStatus {
    Clean,
    Drifted {
        expected_hash: String,
        actual_hash: String,
        modified_files: Vec<String>,
    },
}

/// Detects repository drift via `git write-tree` and emits events on unexpected modifications.
pub struct TreeHashDriftDetector {
    event_bus: Option<Arc<dyn EventBus>>,
}

impl Default for TreeHashDriftDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl TreeHashDriftDetector {
    pub fn new() -> Self {
        Self { event_bus: None }
    }

    pub fn with_event_bus(mut self, event_bus: Arc<dyn EventBus>) -> Self {
        self.event_bus = Some(event_bus);
        self
    }

    /// Compute the current working tree hash using a temporary index to include all modifications.
    pub async fn compute_tree_hash(&self, worktree_path: &Path) -> Result<String, GitError> {
        let temp_index_name = format!(".git/temp_drift_idx_{}", uuid::Uuid::now_v7());
        let temp_index_path = worktree_path.join(&temp_index_name);

        // 1. Stage current working tree into temp index: git add -A
        // Scoped environment (host GIT_DIR-style redirection stripped) with
        // only the internally generated temp index pinned, plus timeout and kill.
        let add_out =
            Self::run_with_temp_index(worktree_path, &temp_index_path, &["add", "-A"]).await?;

        if !add_out.status.success() {
            let _ = tokio::fs::remove_file(&temp_index_path).await;
            let stderr = String::from_utf8_lossy(&add_out.stderr).into_owned();
            return Err(GitError::CommandFailed {
                exit_code: add_out.status.code(),
                message: format!("git add to temp index failed: {stderr}"),
            });
        }

        // 2. Write tree: git write-tree
        let write_out =
            Self::run_with_temp_index(worktree_path, &temp_index_path, &["write-tree"]).await?;

        let _ = tokio::fs::remove_file(&temp_index_path).await;

        if !write_out.status.success() {
            let stderr = String::from_utf8_lossy(&write_out.stderr).into_owned();
            return Err(GitError::CommandFailed {
                exit_code: write_out.status.code(),
                message: format!("git write-tree failed: {stderr}"),
            });
        }

        Ok(String::from_utf8_lossy(&write_out.stdout)
            .trim()
            .to_string())
    }

    /// Run a fixed-argument git command with the temp index pinned and host
    /// redirection variables stripped.
    async fn run_with_temp_index(
        worktree_path: &Path,
        temp_index_path: &Path,
        args: &[&str],
    ) -> Result<std::process::Output, GitError> {
        let mut cmd = scoped_git_command();
        cmd.kill_on_drop(true);
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());
        cmd.env("GIT_INDEX_FILE", temp_index_path);
        cmd.args(args);
        cmd.current_dir(worktree_path);
        let child = cmd.spawn().map_err(GitError::Io)?;
        match tokio::time::timeout(GIT_COMMAND_TIMEOUT, child.wait_with_output()).await {
            Ok(Ok(out)) => Ok(out),
            Ok(Err(e)) => Err(GitError::Io(e)),
            Err(_) => Err(GitError::CommandFailed {
                exit_code: None,
                message: format!("git {} timed out; child killed", args.join(" ")),
            }),
        }
    }

    /// Record a baseline snapshot of the worktree state.
    pub async fn record_snapshot(&self, worktree_path: &Path) -> Result<TreeSnapshot, GitError> {
        let head_out =
            run_scoped_git(worktree_path, &["rev-parse", "HEAD"], GIT_COMMAND_TIMEOUT).await?;

        if !head_out.status.success() {
            let stderr = String::from_utf8_lossy(&head_out.stderr).into_owned();
            return Err(GitError::CommandFailed {
                exit_code: head_out.status.code(),
                message: format!("git rev-parse HEAD failed: {stderr}"),
            });
        }

        let head_commit = String::from_utf8_lossy(&head_out.stdout).trim().to_string();
        let tree_hash = self.compute_tree_hash(worktree_path).await?;

        Ok(TreeSnapshot {
            head_commit,
            tree_hash,
            recorded_at: Utc::now(),
        })
    }

    /// Check if the worktree has drifted from the expected snapshot.
    pub async fn check_drift(
        &self,
        mission_id: &MissionId,
        worktree_path: &Path,
        expected: &TreeSnapshot,
    ) -> Result<DriftStatus, GitError> {
        let actual_hash = self.compute_tree_hash(worktree_path).await?;

        if actual_hash == expected.tree_hash {
            return Ok(DriftStatus::Clean);
        }

        // Identify modified/untracked files
        let status_out = run_scoped_git(
            worktree_path,
            &["status", "--porcelain"],
            GIT_COMMAND_TIMEOUT,
        )
        .await?;

        let mut modified_files = Vec::new();
        if status_out.status.success() {
            let out_str = String::from_utf8_lossy(&status_out.stdout);
            for line in out_str.lines() {
                if line.len() >= 3 {
                    modified_files.push(line[3..].trim().to_string());
                }
            }
        }

        // If event_bus is present, emit RepositoryDriftDetected
        if let Some(ref bus) = self.event_bus {
            let event = EventEnvelope::new(
                0,
                Some(*mission_id),
                None,
                "drift_detector".to_string(),
                EventType::RepositoryDriftDetected {
                    mission_id: *mission_id,
                    expected_hash: expected.tree_hash.clone(),
                    actual_hash: actual_hash.clone(),
                    modified_files: modified_files.clone(),
                },
            );
            let _ = bus.publish(event).await;
        }

        Ok(DriftStatus::Drifted {
            expected_hash: expected.tree_hash.clone(),
            actual_hash,
            modified_files,
        })
    }
}
