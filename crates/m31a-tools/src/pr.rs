//! Issue-to-PR Lifecycle & Pull Request Tooling (`m31a-tools::pr`).
//!
//! Investigates issues, prepares reviewable pull request packages,
//! and creates pull requests on GitHub/GitLab via REST API or CLI fallback.
//!
//! Capabilities:
//! 1. Issue Ingestion: Fetches issue descriptions and acceptance criteria.
//! 2. Pull Request Synthesis: Assembles reviewable PR markdown with summary, verification ledger, and closing keywords.
//! 3. PR Creation: Publishes PR via GitHub REST API, `gh pr create`, or structured `.git/PULL_REQUEST.md` fallback.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueDetails {
    pub number: u64,
    pub title: String,
    pub body: String,
    pub labels: Vec<String>,
    pub url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PullRequestResult {
    pub pr_number: Option<u64>,
    pub pr_url: String,
    pub title: String,
    pub branch: String,
    pub base: String,
    pub is_draft: bool,
    pub delivery_method: String,
}

pub struct PullRequestManager;

impl PullRequestManager {
    /// Detects the GitHub/GitLab owner and repository name from git remote.
    pub fn detect_repo_coordinates(repo_root: &Path) -> Result<(String, String)> {
        let output = Command::new("git")
            .args(["config", "--get", "remote.origin.url"])
            .current_dir(repo_root)
            .output()?;

        if !output.status.success() {
            return Ok(("origin".to_string(), "repo".to_string()));
        }

        let url = String::from_utf8_lossy(&output.stdout).trim().to_string();
        // Handle git@github.com:owner/repo.git or https://github.com/owner/repo.git
        let cleaned = url
            .trim_end_matches(".git")
            .replace("git@github.com:", "https://github.com/");

        if let Some(pos) = cleaned.find("github.com/") {
            let path_part = &cleaned[pos + "github.com/".len()..];
            let mut parts = path_part.split('/');
            if let (Some(owner), Some(repo)) = (parts.next(), parts.next()) {
                return Ok((owner.to_string(), repo.to_string()));
            }
        }

        Ok(("local".to_string(), "workspace".to_string()))
    }

    /// Fetches issue details by number.
    pub async fn fetch_issue(repo_root: &Path, issue_number: u64) -> Result<IssueDetails> {
        let (owner, repo) = Self::detect_repo_coordinates(repo_root)?;

        // Try gh cli first
        if let Ok(output) = Command::new("gh")
            .args([
                "issue",
                "view",
                &issue_number.to_string(),
                "--json",
                "title,body,labels,url",
            ])
            .current_dir(repo_root)
            .output()
            && output.status.success()
            && let Ok(val) = serde_json::from_slice::<serde_json::Value>(&output.stdout)
        {
            let title = val["title"].as_str().unwrap_or("").to_string();
            let body = val["body"].as_str().unwrap_or("").to_string();
            let url = val["url"].as_str().map(|s| s.to_string());
            let labels = val["labels"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|l| l["name"].as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default();

            return Ok(IssueDetails {
                number: issue_number,
                title,
                body,
                labels,
                url,
            });
        }

        // Fallback: mock / template issue
        Ok(IssueDetails {
            number: issue_number,
            title: format!("Issue #{} on {}/{}", issue_number, owner, repo),
            body: format!("Automated retrieval for issue #{}", issue_number),
            labels: vec!["bug".to_string()],
            url: Some(format!(
                "https://github.com/{}/{}/issues/{}",
                owner, repo, issue_number
            )),
        })
    }

    /// Creates or packages a reviewable pull request.
    pub async fn create_pull_request(
        repo_root: &Path,
        title: &str,
        body: &str,
        base_branch: &str,
        head_branch: &str,
        draft: bool,
    ) -> Result<PullRequestResult> {
        let (owner, repo) = Self::detect_repo_coordinates(repo_root)?;

        // Audit regression compliance for PR
        let status = crate::git_ops::get_repository_status(repo_root).unwrap_or_default();
        let touched: Vec<std::path::PathBuf> = status
            .staged
            .iter()
            .chain(status.unstaged.iter())
            .map(|f| std::path::PathBuf::from(&f.path))
            .collect();
        let regression_report =
            m31a_verify::regression::RegressionGuard::audit_changeset(title, &touched);
        let mut pr_body_attributed = format!(
            "{}\n\n---\n*Delivered with [M31A](https://github.com/eshanized/M31A) autonomous engineering assistant*",
            body
        );

        if let m31a_verify::regression::RegressionAuditStatus::Verified {
            ref test_file,
            ref test_name,
        } = regression_report.status
        {
            pr_body_attributed.push_str(&format!(
                "\n\n### Regression Prevention\n- Verified targeted test: `{}` in `{}`\n",
                test_name,
                test_file.display()
            ));
        } else if let m31a_verify::regression::RegressionAuditStatus::MissingTest { ref reason } =
            regression_report.status
        {
            pr_body_attributed.push_str(&format!(
                "\n\n> [!WARNING]\n> Regression Audit: {}\n",
                reason
            ));
        }

        // 1. Check if GITHUB_TOKEN is available for direct API delivery
        if let Ok(token) = std::env::var("GITHUB_TOKEN") {
            let client = reqwest::Client::new();
            let url = format!("https://api.github.com/repos/{}/{}/pulls", owner, repo);
            let payload = serde_json::json!({
                "title": title,
                "body": pr_body_attributed,
                "base": base_branch,
                "head": head_branch,
                "draft": draft
            });

            if let Ok(resp) = client
                .post(&url)
                .header("Authorization", format!("Bearer {}", token))
                .header("User-Agent", "m31a-pr-agent/1.0")
                .header("Accept", "application/vnd.github+json")
                .json(&payload)
                .send()
                .await
                && resp.status().is_success()
                && let Ok(val) = resp.json::<serde_json::Value>().await
            {
                let pr_num = val["number"].as_u64();
                let html_url = val["html_url"].as_str().unwrap_or("").to_string();
                return Ok(PullRequestResult {
                    pr_number: pr_num,
                    pr_url: html_url,
                    title: title.to_string(),
                    branch: head_branch.to_string(),
                    base: base_branch.to_string(),
                    is_draft: draft,
                    delivery_method: "GitHub REST API".to_string(),
                });
            }
        }

        // 2. Fallback to `gh pr create`
        let mut gh_cmd = Command::new("gh");
        gh_cmd.args([
            "pr",
            "create",
            "--title",
            title,
            "--body",
            &pr_body_attributed,
            "--base",
            base_branch,
            "--head",
            head_branch,
        ]);
        if draft {
            gh_cmd.arg("--draft");
        }
        gh_cmd.current_dir(repo_root);

        if let Ok(output) = gh_cmd.output()
            && output.status.success()
        {
            let pr_url = String::from_utf8_lossy(&output.stdout).trim().to_string();
            return Ok(PullRequestResult {
                pr_number: None,
                pr_url,
                title: title.to_string(),
                branch: head_branch.to_string(),
                base: base_branch.to_string(),
                is_draft: draft,
                delivery_method: "gh CLI".to_string(),
            });
        }

        // 3. Fallback: Local reviewable PR bundle file
        let bundle_path = repo_root.join(".git").join("PULL_REQUEST.md");
        let content = format!(
            "# Pull Request: {}\n\n**Base:** `{}` ← **Head:** `{}`\n**Status:** {}\n\n{}\n",
            title,
            base_branch,
            head_branch,
            if draft { "Draft" } else { "Ready for Review" },
            pr_body_attributed
        );
        let _ = fs::write(&bundle_path, &content);

        Ok(PullRequestResult {
            pr_number: None,
            pr_url: bundle_path.to_string_lossy().to_string(),
            title: title.to_string(),
            branch: head_branch.to_string(),
            base: base_branch.to_string(),
            is_draft: draft,
            delivery_method: "Local Reviewable PR Bundle".to_string(),
        })
    }

    /// Assembles a complete PR description bundle from git logs and verification artifacts.
    pub fn assemble_pr_body(repo_root: &Path, summary: &str, closes_issue: Option<u64>) -> String {
        let mut body = String::new();
        body.push_str("## Description\n\n");
        body.push_str(summary);
        body.push_str("\n\n");

        if let Some(issue) = closes_issue {
            body.push_str(&format!("Closes #{}\n\n", issue));
        }

        // Append verification evidence if VERIFICATION.md exists
        let verif_path = repo_root.join("VERIFICATION.md");
        if let Ok(verif_text) = fs::read_to_string(&verif_path) {
            body.push_str("## Verification Evidence\n\n");
            let snippet = verif_text
                .lines()
                .take(30)
                .collect::<Vec<&str>>()
                .join("\n");
            body.push_str(&snippet);
            body.push_str("\n\n");
        }

        body
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_assemble_pr_body() {
        let temp = tempfile::tempdir().unwrap();
        let body = PullRequestManager::assemble_pr_body(
            temp.path(),
            "Fix edge cases in tokenizer and update tests.",
            Some(42),
        );

        assert!(body.contains("Fix edge cases"));
        assert!(body.contains("Closes #42"));
    }

    #[tokio::test]
    async fn test_create_pull_request_local_bundle_fallback() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(temp.path().join(".git")).unwrap();

        let result = PullRequestManager::create_pull_request(
            temp.path(),
            "feat: add PR creation capability",
            "This implements issue-to-pr automation",
            "main",
            "feature/pr-tool",
            false,
        )
        .await
        .unwrap();

        assert_eq!(result.title, "feat: add PR creation capability");
        assert_eq!(result.delivery_method, "Local Reviewable PR Bundle");
        assert!(PathBuf::from(&result.pr_url).is_file());
    }
}
