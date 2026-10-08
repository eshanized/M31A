//! Remote repository hosting integration (GitHub, GitLab) for PR/MR workflows (Issue 16).
//!
//! Provides a typed provider abstraction for creating PRs, checking status, posting review
//! comments, and inspecting CI check runs, with fail-safe local/mock fallbacks and secret redaction.

use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::sync::RwLock;

use crate::git::GitError;
use crate::telemetry::redactor::SecretRedactor;

/// Information about a Pull / Merge Request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PullRequest {
    pub id: u64,
    pub title: String,
    pub body: String,
    pub url: String,
    pub source_branch: String,
    pub target_branch: String,
    pub status: String,
    pub mergeable: bool,
}

/// Status and CI check state for a Pull / Merge Request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PullRequestStatus {
    pub id: u64,
    pub status: String,
    pub mergeable: bool,
    pub checks: Vec<CheckRun>,
    pub comments_count: usize,
    pub url: String,
}

/// Single CI check run or workflow outcome.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CheckRun {
    pub name: String,
    pub status: String,
    pub conclusion: Option<String>,
    pub details_url: Option<String>,
}

/// Review comment on a PR.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ReviewComment {
    pub id: u64,
    pub body: String,
    pub path: Option<String>,
    pub line: Option<usize>,
    pub author: String,
    pub created_at: String,
}

/// Abstract hosting provider trait.
#[async_trait]
pub trait RepoHostingProvider: Send + Sync {
    async fn create_pull_request(
        &self,
        title: &str,
        body: &str,
        source_branch: &str,
        target_branch: &str,
    ) -> Result<PullRequest, GitError>;

    async fn get_pull_request_status(&self, pr_id: u64) -> Result<PullRequestStatus, GitError>;

    async fn post_review_comment(
        &self,
        pr_id: u64,
        body: &str,
        path: Option<&str>,
        line: Option<usize>,
    ) -> Result<ReviewComment, GitError>;

    async fn list_check_runs(&self, ref_name: &str) -> Result<Vec<CheckRun>, GitError>;
}

/// Mock / In-memory provider for testing and offline hermetic operation.
#[derive(Default)]
pub struct MockHostingProvider {
    prs: RwLock<Vec<PullRequest>>,
    comments: RwLock<Vec<ReviewComment>>,
}

impl MockHostingProvider {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl RepoHostingProvider for MockHostingProvider {
    async fn create_pull_request(
        &self,
        title: &str,
        body: &str,
        source_branch: &str,
        target_branch: &str,
    ) -> Result<PullRequest, GitError> {
        let mut prs = self.prs.write().await;
        let id = (prs.len() + 1) as u64;
        let pr = PullRequest {
            id,
            title: title.to_string(),
            body: body.to_string(),
            url: format!("https://github.com/org/repo/pull/{id}"),
            source_branch: source_branch.to_string(),
            target_branch: target_branch.to_string(),
            status: "open".to_string(),
            mergeable: true,
        };
        prs.push(pr.clone());
        Ok(pr)
    }

    async fn get_pull_request_status(&self, pr_id: u64) -> Result<PullRequestStatus, GitError> {
        let prs = self.prs.read().await;
        let pr = prs
            .iter()
            .find(|p| p.id == pr_id)
            .ok_or_else(|| GitError::CommandFailed {
                exit_code: Some(404),
                message: format!("Pull request #{pr_id} not found"),
            })?;

        let comments = self.comments.read().await;
        Ok(PullRequestStatus {
            id: pr.id,
            status: pr.status.clone(),
            mergeable: pr.mergeable,
            checks: vec![CheckRun {
                name: "CI / Test & Lint".to_string(),
                status: "completed".to_string(),
                conclusion: Some("success".to_string()),
                details_url: Some(format!("https://github.com/org/repo/actions/runs/{pr_id}")),
            }],
            comments_count: comments.len(),
            url: pr.url.clone(),
        })
    }

    async fn post_review_comment(
        &self,
        _pr_id: u64,
        body: &str,
        path: Option<&str>,
        line: Option<usize>,
    ) -> Result<ReviewComment, GitError> {
        let mut comments = self.comments.write().await;
        let id = (comments.len() + 1) as u64;
        let comment = ReviewComment {
            id,
            body: body.to_string(),
            path: path.map(|s| s.to_string()),
            line,
            author: "m31a-agent".to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
        };
        comments.push(comment.clone());
        Ok(comment)
    }

    async fn list_check_runs(&self, _ref_name: &str) -> Result<Vec<CheckRun>, GitError> {
        Ok(vec![CheckRun {
            name: "test".to_string(),
            status: "completed".to_string(),
            conclusion: Some("success".to_string()),
            details_url: None,
        }])
    }
}

/// Production CLI-backed provider supporting GitHub (`gh`).
pub struct GitHubCliHostingProvider {
    workspace_root: PathBuf,
}

impl GitHubCliHostingProvider {
    pub fn new(workspace_root: impl Into<PathBuf>) -> Self {
        Self {
            workspace_root: workspace_root.into(),
        }
    }

    async fn is_gh_available(&self) -> bool {
        tokio::process::Command::new("gh")
            .arg("--version")
            .output()
            .await
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
}

#[async_trait]
impl RepoHostingProvider for GitHubCliHostingProvider {
    async fn create_pull_request(
        &self,
        title: &str,
        body: &str,
        source_branch: &str,
        target_branch: &str,
    ) -> Result<PullRequest, GitError> {
        if !self.is_gh_available().await {
            return Err(GitError::CommandFailed {
                exit_code: None,
                message: "GitHub CLI (`gh`) is not available or not installed in PATH".to_string(),
            });
        }

        let output = tokio::process::Command::new("gh")
            .current_dir(&self.workspace_root)
            .args([
                "pr",
                "create",
                "--title",
                title,
                "--body",
                body,
                "--head",
                source_branch,
                "--base",
                target_branch,
            ])
            .output()
            .await
            .map_err(|e| GitError::CommandFailed {
                exit_code: None,
                message: e.to_string(),
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(GitError::CommandFailed {
                exit_code: output.status.code(),
                message: SecretRedactor::new().redact(&stderr),
            });
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let url = stdout.trim().to_string();
        let pr_id = url
            .split('/')
            .next_back()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(1);

        Ok(PullRequest {
            id: pr_id,
            title: title.to_string(),
            body: body.to_string(),
            url,
            source_branch: source_branch.to_string(),
            target_branch: target_branch.to_string(),
            status: "open".to_string(),
            mergeable: true,
        })
    }

    async fn get_pull_request_status(&self, pr_id: u64) -> Result<PullRequestStatus, GitError> {
        if !self.is_gh_available().await {
            return Err(GitError::CommandFailed {
                exit_code: None,
                message: "GitHub CLI (`gh`) is not available or not installed in PATH".to_string(),
            });
        }

        let output = tokio::process::Command::new("gh")
            .current_dir(&self.workspace_root)
            .args([
                "pr",
                "view",
                &pr_id.to_string(),
                "--json",
                "state,url,mergeable",
            ])
            .output()
            .await
            .map_err(|e| GitError::CommandFailed {
                exit_code: None,
                message: e.to_string(),
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(GitError::CommandFailed {
                exit_code: output.status.code(),
                message: SecretRedactor::new().redact(&stderr),
            });
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let parsed: serde_json::Value =
            serde_json::from_str(&stdout).unwrap_or(serde_json::Value::Null);

        let status = parsed["state"].as_str().unwrap_or("OPEN").to_lowercase();
        let url = parsed["url"].as_str().unwrap_or("").to_string();
        let mergeable = parsed["mergeable"]
            .as_str()
            .map(|m| m == "MERGEABLE")
            .unwrap_or(true);

        Ok(PullRequestStatus {
            id: pr_id,
            status,
            mergeable,
            checks: Vec::new(),
            comments_count: 0,
            url,
        })
    }

    async fn post_review_comment(
        &self,
        pr_id: u64,
        body: &str,
        path: Option<&str>,
        line: Option<usize>,
    ) -> Result<ReviewComment, GitError> {
        if !self.is_gh_available().await {
            return Err(GitError::CommandFailed {
                exit_code: None,
                message: "GitHub CLI (`gh`) is not available or not installed in PATH".to_string(),
            });
        }

        let mut cmd = tokio::process::Command::new("gh");
        cmd.current_dir(&self.workspace_root);
        cmd.args(["pr", "comment", &pr_id.to_string(), "--body", body]);

        let output = cmd.output().await.map_err(|e| GitError::CommandFailed {
            exit_code: None,
            message: e.to_string(),
        })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(GitError::CommandFailed {
                exit_code: output.status.code(),
                message: SecretRedactor::new().redact(&stderr),
            });
        }

        Ok(ReviewComment {
            id: 1,
            body: body.to_string(),
            path: path.map(|s| s.to_string()),
            line,
            author: "operator".to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
        })
    }

    async fn list_check_runs(&self, ref_name: &str) -> Result<Vec<CheckRun>, GitError> {
        if !self.is_gh_available().await {
            return Err(GitError::CommandFailed {
                exit_code: None,
                message: "GitHub CLI (`gh`) is not available or not installed in PATH".to_string(),
            });
        }

        let output = tokio::process::Command::new("gh")
            .current_dir(&self.workspace_root)
            .args([
                "run",
                "list",
                "--commit",
                ref_name,
                "--json",
                "name,status,conclusion,url",
            ])
            .output()
            .await
            .map_err(|e| GitError::CommandFailed {
                exit_code: None,
                message: e.to_string(),
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(GitError::CommandFailed {
                exit_code: output.status.code(),
                message: SecretRedactor::new().redact(&stderr),
            });
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let runs: Vec<serde_json::Value> = serde_json::from_str(&stdout).unwrap_or_default();

        Ok(runs
            .into_iter()
            .map(|r| CheckRun {
                name: r["name"].as_str().unwrap_or("").to_string(),
                status: r["status"].as_str().unwrap_or("").to_string(),
                conclusion: r["conclusion"].as_str().map(|s| s.to_string()),
                details_url: r["url"].as_str().map(|s| s.to_string()),
            })
            .collect())
    }
}
