//! CLI-based Git capability provider running host git (CTL-02, D-01, D-03).

use crate::capability::error::CapabilityError;
use crate::capability::traits::git::{GitBranchInfo, GitCommitInfo, GitService, GitStatusResult};
use async_trait::async_trait;
use std::path::PathBuf;
use tokio::process::Command;

/// Native Git provider delegating to the host `git` command in workspace directory.
pub struct CliGitProvider {
    workspace_root: PathBuf,
}

impl CliGitProvider {
    /// Create a new CliGitProvider for the given workspace root.
    pub fn new(workspace_root: impl Into<PathBuf>) -> Self {
        Self {
            workspace_root: workspace_root.into(),
        }
    }

    async fn run_git(&self, args: &[&str]) -> Result<String, CapabilityError> {
        for arg in args {
            let trimmed = arg.trim();
            if trimmed.starts_with("--git-dir")
                || trimmed.starts_with("--work-tree")
                || trimmed.starts_with("--separate-git-dir")
                || trimmed.contains("core.gitDir")
                || trimmed.contains("core.worktree")
                || trimmed.starts_with("GIT_DIR=")
                || trimmed.starts_with("GIT_WORK_TREE=")
                || trimmed.starts_with("GIT_INDEX_FILE=")
            {
                return Err(CapabilityError::PermissionDenied(format!(
                    "git repository redirection argument '{}' is prohibited",
                    arg
                )));
            }
        }

        let mut cmd = Command::new("git");
        cmd.args(args);
        cmd.current_dir(&self.workspace_root);

        // Sanitize environment: clear inherited variables and force workspace git repository
        let env_builder = crate::process::env::EnvironmentBuilder::new(&self.workspace_root);
        env_builder.apply(&mut cmd);

        let git_dir = self.workspace_root.join(".git");
        if git_dir.is_dir() {
            cmd.env("GIT_DIR", &git_dir);
        }
        cmd.env("GIT_WORK_TREE", &self.workspace_root);

        let output = cmd.output().await.map_err(|e| {
            CapabilityError::InfrastructureFault(format!("failed to spawn git: {e}"))
        })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
            return Err(CapabilityError::ExecutionFailed {
                exit_code: output.status.code(),
                message: stderr,
            });
        }

        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }
}

#[async_trait]
impl GitService for CliGitProvider {
    async fn status(&self) -> Result<GitStatusResult, CapabilityError> {
        let raw = self.run_git(&["status", "--porcelain=v1", "-b"]).await?;
        let mut branch = String::from("unknown");
        let mut staged = Vec::new();
        let mut unstaged = Vec::new();
        let mut untracked = Vec::new();

        for line in raw.lines() {
            if let Some(branch_part) = line.strip_prefix("## ") {
                branch = branch_part
                    .split("...")
                    .next()
                    .unwrap_or(branch_part)
                    .to_string();
                continue;
            }

            if line.len() < 3 {
                continue;
            }

            let x = line.chars().next().unwrap_or(' ');
            let y = line.chars().nth(1).unwrap_or(' ');
            let path = line[3..].trim().to_string();

            if x == '?' && y == '?' {
                untracked.push(path);
            } else {
                if x != ' ' && x != '?' {
                    staged.push(path.clone());
                }
                if y != ' ' && y != '?' {
                    unstaged.push(path);
                }
            }
        }

        let is_clean = staged.is_empty() && unstaged.is_empty() && untracked.is_empty();

        Ok(GitStatusResult {
            branch,
            is_clean,
            staged,
            unstaged,
            untracked,
        })
    }

    async fn diff(&self, staged: bool) -> Result<String, CapabilityError> {
        let args = if staged {
            vec!["diff", "--staged"]
        } else {
            vec!["diff"]
        };
        self.run_git(&args).await
    }

    async fn log(&self, max_count: usize) -> Result<Vec<GitCommitInfo>, CapabilityError> {
        let count_str = max_count.to_string();
        let raw = self
            .run_git(&["log", "-n", &count_str, "--format=%H%x09%an%x09%ad%x09%s"])
            .await?;

        let mut commits = Vec::new();
        for line in raw.lines() {
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() >= 4 {
                commits.push(GitCommitInfo {
                    commit_hash: parts[0].to_string(),
                    author: parts[1].to_string(),
                    date: parts[2].to_string(),
                    message: parts[3].to_string(),
                });
            }
        }
        Ok(commits)
    }

    async fn show(&self, revision: &str) -> Result<String, CapabilityError> {
        // Prohibit empty values or leading dashes because the revision is passed
        // as an argument to `git show` and could otherwise be interpreted as a CLI flag.
        if revision.trim().is_empty() || revision.trim().starts_with('-') {
            return Err(CapabilityError::PermissionDenied(format!(
                "invalid revision '{revision}': option injection is prohibited"
            )));
        }
        self.run_git(&["show", revision]).await
    }

    async fn branch(&self) -> Result<GitBranchInfo, CapabilityError> {
        let raw = self.run_git(&["branch", "--list"]).await?;
        let mut current = String::new();
        let mut all_branches = Vec::new();

        for line in raw.lines() {
            let trimmed = line.trim();
            if let Some(stripped) = trimmed.strip_prefix('*') {
                let name = stripped.trim().to_string();
                current = name.clone();
                all_branches.push(name);
            } else if !trimmed.is_empty() {
                all_branches.push(trimmed.to_string());
            }
        }

        Ok(GitBranchInfo {
            current,
            all_branches,
        })
    }

    async fn checkout(&self, branch_or_commit: &str) -> Result<(), CapabilityError> {
        if branch_or_commit.trim().starts_with('-') {
            return Err(CapabilityError::PermissionDenied(format!(
                "invalid branch or commit name '{}': option injection is prohibited",
                branch_or_commit
            )));
        }
        self.run_git(&["checkout", branch_or_commit]).await?;
        Ok(())
    }

    async fn add(&self, paths: &[PathBuf]) -> Result<(), CapabilityError> {
        for p in paths {
            if p.to_string_lossy().trim().starts_with('-') {
                return Err(CapabilityError::PermissionDenied(format!(
                    "invalid path '{}': option injection is prohibited",
                    p.display()
                )));
            }
            if crate::capability::providers::local_fs::contains_protected_component(p) {
                return Err(CapabilityError::PermissionDenied(format!(
                    "git add on protected path '{}' is prohibited",
                    p.display()
                )));
            }
        }
        let mut args = vec!["add", "--"];
        let path_strings: Vec<String> = paths.iter().map(|p| p.display().to_string()).collect();
        for p in &path_strings {
            args.push(p);
        }
        self.run_git(&args).await?;
        Ok(())
    }

    async fn commit(&self, message: &str) -> Result<String, CapabilityError> {
        self.run_git(&["commit", "-m", message]).await?;
        let hash = self.run_git(&["rev-parse", "HEAD"]).await?;
        Ok(hash.trim().to_string())
    }

    async fn add_all(&self) -> Result<(), CapabilityError> {
        self.run_git(&["add", "-A"]).await?;
        Ok(())
    }

    async fn reset(&self, paths: &[&str]) -> Result<(), CapabilityError> {
        for p in paths {
            if p.trim().starts_with('-') {
                return Err(CapabilityError::PermissionDenied(format!(
                    "invalid path '{}': option injection is prohibited",
                    p
                )));
            }
        }
        let mut args = vec!["reset", "-q", "--"];
        args.extend(paths.iter().copied());
        self.run_git(&args).await?;
        Ok(())
    }

    async fn commit_with_trailers(
        &self,
        message: &str,
        trailers: &crate::git::trailers::CommitTrailers,
    ) -> Result<String, CapabilityError> {
        let full_msg = crate::git::trailers::CommitTrailers::embed_trailers(message, trailers)
            .unwrap_or_else(|_| message.to_string());
        self.commit(&full_msg).await
    }

    async fn merge(&self, branch: &str, no_edit: bool) -> Result<String, CapabilityError> {
        if branch.trim().starts_with('-') {
            return Err(CapabilityError::PermissionDenied(format!(
                "invalid branch '{}': option injection is prohibited",
                branch
            )));
        }
        let mut args = vec!["merge", branch];
        if no_edit {
            args.push("--no-edit");
        }
        self.run_git(&args).await
    }

    async fn merge_abort(&self) -> Result<(), CapabilityError> {
        self.run_git(&["merge", "--abort"]).await?;
        Ok(())
    }

    async fn restore_head(&self, path: &str) -> Result<(), CapabilityError> {
        if path.trim().starts_with('-') {
            return Err(CapabilityError::PermissionDenied(format!(
                "invalid path '{}': option injection is prohibited",
                path
            )));
        }
        self.run_git(&["checkout", "HEAD", "--", path]).await?;
        Ok(())
    }

    async fn status_porcelain(&self) -> Result<String, CapabilityError> {
        self.run_git(&["status", "-s"]).await
    }

    async fn diff_range(&self, range: &str) -> Result<String, CapabilityError> {
        if range.trim().starts_with('-') && range != "--cached" && range != "--staged" {
            return Err(CapabilityError::PermissionDenied(format!(
                "invalid diff option '{}': prohibited",
                range
            )));
        }
        self.run_git(&["diff", range]).await
    }
}
