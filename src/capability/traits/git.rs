//! Git version control service trait (CTL-01, CTL-02).

use crate::capability::error::CapabilityError;
use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Working tree status summary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct GitStatusResult {
    pub branch: String,
    pub is_clean: bool,
    pub staged: Vec<String>,
    pub unstaged: Vec<String>,
    pub untracked: Vec<String>,
}

/// Commit metadata record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct GitCommitInfo {
    pub commit_hash: String,
    pub author: String,
    pub date: String,
    pub message: String,
}

/// Branch summary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct GitBranchInfo {
    pub current: String,
    pub all_branches: Vec<String>,
}

/// Asynchronous service seam for git operations.
#[async_trait]
pub trait GitService: Send + Sync + 'static {
    /// Retrieve working copy status.
    async fn status(&self) -> Result<GitStatusResult, CapabilityError>;

    /// Compute repository diff.
    async fn diff(&self, staged: bool) -> Result<String, CapabilityError>;

    /// Retrieve commit log up to max_count.
    async fn log(&self, max_count: usize) -> Result<Vec<GitCommitInfo>, CapabilityError>;

    /// Show contents of a specific revision or object.
    async fn show(&self, revision: &str) -> Result<String, CapabilityError>;

    /// Query current and available branches.
    async fn branch(&self) -> Result<GitBranchInfo, CapabilityError>;

    /// Checkout a branch or commit.
    async fn checkout(&self, branch_or_commit: &str) -> Result<(), CapabilityError>;

    /// Stage paths for commit.
    async fn add(&self, paths: &[PathBuf]) -> Result<(), CapabilityError>;

    /// Create commit with the given message, returning new commit hash.
    async fn commit(&self, message: &str) -> Result<String, CapabilityError>;

    /// Stage all tracked and untracked changes in the working tree (`git add -A`).
    async fn add_all(&self) -> Result<(), CapabilityError>;

    /// Unstage specific paths from index (`git reset -q -- <paths...>`).
    async fn reset(&self, paths: &[&str]) -> Result<(), CapabilityError>;

    /// Commit with metadata trailers.
    async fn commit_with_trailers(
        &self,
        message: &str,
        trailers: &crate::git::trailers::CommitTrailers,
    ) -> Result<String, CapabilityError>;

    /// Merge the specified branch into the current branch (`git merge <branch>`).
    async fn merge(&self, branch: &str, no_edit: bool) -> Result<String, CapabilityError>;

    /// Abort an ongoing merge conflict (`git merge --abort`).
    async fn merge_abort(&self) -> Result<(), CapabilityError>;

    /// Restore tracked files to clean HEAD state (`git checkout HEAD -- <path>`).
    async fn restore_head(&self, path: &str) -> Result<(), CapabilityError>;

    /// Retrieve short porcelain status (`git status -s`).
    async fn status_porcelain(&self) -> Result<String, CapabilityError>;

    /// Compute diff against an arbitrary revision range (e.g. "HEAD", "--cached", "HEAD~1..HEAD").
    async fn diff_range(&self, range: &str) -> Result<String, CapabilityError>;
}
