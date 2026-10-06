//! CLI-based Git capability provider running host git (CTL-02, D-01, D-03).

use crate::capability::error::CapabilityError;
use crate::capability::traits::git::{GitBranchInfo, GitCommitInfo, GitService, GitStatusResult};
use crate::git::{GitGate, GitOperation};
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
        // Contain repository discovery to this workspace: git must never walk
        // up into a parent repository's tree when this workspace is not
        // itself a repository (fail with "not a git repository" instead of
        // silently operating on an ancestor's history).
        cmd.env("GIT_CEILING_DIRECTORIES", &self.workspace_root);

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

    async fn checkout(
        &self,
        branch_or_commit: &str,
        gate: &GitGate,
    ) -> Result<(), CapabilityError> {
        if branch_or_commit.trim().starts_with('-') {
            return Err(CapabilityError::PermissionDenied(format!(
                "invalid branch or commit name '{}': option injection is prohibited",
                branch_or_commit
            )));
        }
        gate.enforce(
            &GitOperation::Checkout {
                target: branch_or_commit.to_string(),
                force: false,
            },
            &self.workspace_root,
        )
        .map_err(|e| CapabilityError::PermissionDenied(e.to_string()))?;
        self.run_git(&["checkout", branch_or_commit]).await?;
        Ok(())
    }

    async fn add(&self, paths: &[PathBuf], gate: &GitGate) -> Result<(), CapabilityError> {
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
        let path_strings: Vec<String> = paths.iter().map(|p| p.display().to_string()).collect();
        gate.enforce(
            &GitOperation::Add {
                paths: path_strings.clone(),
            },
            &self.workspace_root,
        )
        .map_err(|e| CapabilityError::PermissionDenied(e.to_string()))?;
        let mut args = vec!["add", "--"];
        for p in &path_strings {
            args.push(p);
        }
        self.run_git(&args).await?;
        Ok(())
    }

    async fn commit(&self, message: &str, gate: &GitGate) -> Result<String, CapabilityError> {
        gate.enforce(
            &GitOperation::Commit {
                message: message.to_string(),
            },
            &self.workspace_root,
        )
        .map_err(|e| CapabilityError::PermissionDenied(e.to_string()))?;
        self.run_git(&["commit", "-m", message]).await?;
        let hash = self.run_git(&["rev-parse", "HEAD"]).await?;
        Ok(hash.trim().to_string())
    }

    async fn add_all(&self, gate: &GitGate) -> Result<(), CapabilityError> {
        gate.enforce(
            &GitOperation::Add {
                paths: vec!["-A".to_string()],
            },
            &self.workspace_root,
        )
        .map_err(|e| CapabilityError::PermissionDenied(e.to_string()))?;
        self.run_git(&["add", "-A"]).await?;
        Ok(())
    }

    async fn reset(&self, paths: &[&str], gate: &GitGate) -> Result<(), CapabilityError> {
        for p in paths {
            if p.trim().starts_with('-') {
                return Err(CapabilityError::PermissionDenied(format!(
                    "invalid path '{}': option injection is prohibited",
                    p
                )));
            }
        }
        // `git reset -q -- <paths>` only unstages; it never moves HEAD. The
        // gate still binds the exact paths so a stale authorization for one
        // path set cannot unstage another.
        gate.enforce(
            &GitOperation::Unstage {
                paths: paths.iter().map(|s| s.to_string()).collect(),
            },
            &self.workspace_root,
        )
        .map_err(|e| CapabilityError::PermissionDenied(e.to_string()))?;
        let mut args = vec!["reset", "-q", "--"];
        args.extend(paths.iter().copied());
        self.run_git(&args).await?;
        Ok(())
    }

    async fn commit_with_trailers(
        &self,
        message: &str,
        trailers: &crate::git::trailers::CommitTrailers,
        gate: &GitGate,
    ) -> Result<String, CapabilityError> {
        let full_msg = crate::git::trailers::CommitTrailers::embed_trailers(message, trailers)
            .unwrap_or_else(|_| message.to_string());
        gate.enforce(
            &GitOperation::Commit {
                message: full_msg.clone(),
            },
            &self.workspace_root,
        )
        .map_err(|e| CapabilityError::PermissionDenied(e.to_string()))?;
        // Enforced above; delegate to the raw commit path without
        // re-enforcing (avoids double message-encoding divergence).
        self.run_git(&["commit", "-m", &full_msg]).await?;
        let hash = self.run_git(&["rev-parse", "HEAD"]).await?;
        Ok(hash.trim().to_string())
    }

    async fn merge(
        &self,
        branch: &str,
        no_edit: bool,
        gate: &GitGate,
    ) -> Result<String, CapabilityError> {
        if branch.trim().starts_with('-') {
            return Err(CapabilityError::PermissionDenied(format!(
                "invalid branch '{}': option injection is prohibited",
                branch
            )));
        }
        crate::git::validate_git_ref_arg("merge branch", branch)
            .map_err(|e| CapabilityError::PermissionDenied(e.to_string()))?;
        gate.enforce(
            &GitOperation::Merge {
                source: branch.to_string(),
            },
            &self.workspace_root,
        )
        .map_err(|e| CapabilityError::PermissionDenied(e.to_string()))?;
        let mut args = vec!["merge", branch];
        if no_edit {
            args.push("--no-edit");
        }
        self.run_git(&args).await
    }

    async fn merge_abort(&self, gate: &GitGate) -> Result<(), CapabilityError> {
        gate.enforce(&GitOperation::MergeAbort, &self.workspace_root)
            .map_err(|e| CapabilityError::PermissionDenied(e.to_string()))?;
        self.run_git(&["merge", "--abort"]).await?;
        Ok(())
    }

    async fn restore_head(&self, path: &str, gate: &GitGate) -> Result<(), CapabilityError> {
        if path.trim().starts_with('-') {
            return Err(CapabilityError::PermissionDenied(format!(
                "invalid path '{}': option injection is prohibited",
                path
            )));
        }
        gate.enforce(
            &GitOperation::SyncCheckout {
                target: path.to_string(),
            },
            &self.workspace_root,
        )
        .map_err(|e| CapabilityError::PermissionDenied(e.to_string()))?;
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

/// A no-op GitService implementation used when Git integration is disabled by user preference.
#[derive(Debug, Clone, Default)]
pub struct DisabledGitProvider;

#[async_trait]
impl GitService for DisabledGitProvider {
    async fn status(&self) -> Result<GitStatusResult, CapabilityError> {
        Ok(GitStatusResult {
            branch: "disabled".to_string(),
            is_clean: true,
            staged: Vec::new(),
            unstaged: Vec::new(),
            untracked: Vec::new(),
        })
    }

    async fn diff(&self, _staged: bool) -> Result<String, CapabilityError> {
        Ok(String::new())
    }

    async fn log(&self, _max_count: usize) -> Result<Vec<GitCommitInfo>, CapabilityError> {
        Ok(Vec::new())
    }

    async fn show(&self, _revision: &str) -> Result<String, CapabilityError> {
        Ok(String::new())
    }

    async fn branch(&self) -> Result<GitBranchInfo, CapabilityError> {
        Ok(GitBranchInfo {
            current: "disabled".to_string(),
            all_branches: Vec::new(),
        })
    }

    async fn checkout(
        &self,
        _branch_or_commit: &str,
        _gate: &GitGate,
    ) -> Result<(), CapabilityError> {
        Err(CapabilityError::Unavailable(
            "Git integration is disabled by user preference".to_string(),
        ))
    }

    async fn add(&self, _paths: &[PathBuf], _gate: &GitGate) -> Result<(), CapabilityError> {
        Err(CapabilityError::Unavailable(
            "Git integration is disabled by user preference".to_string(),
        ))
    }

    async fn commit(&self, _message: &str, _gate: &GitGate) -> Result<String, CapabilityError> {
        Err(CapabilityError::Unavailable(
            "Git integration is disabled by user preference".to_string(),
        ))
    }

    async fn add_all(&self, _gate: &GitGate) -> Result<(), CapabilityError> {
        Err(CapabilityError::Unavailable(
            "Git integration is disabled by user preference".to_string(),
        ))
    }

    async fn reset(&self, _paths: &[&str], _gate: &GitGate) -> Result<(), CapabilityError> {
        Err(CapabilityError::Unavailable(
            "Git integration is disabled by user preference".to_string(),
        ))
    }

    async fn commit_with_trailers(
        &self,
        _message: &str,
        _trailers: &crate::git::trailers::CommitTrailers,
        _gate: &GitGate,
    ) -> Result<String, CapabilityError> {
        Err(CapabilityError::Unavailable(
            "Git integration is disabled by user preference".to_string(),
        ))
    }

    async fn merge(
        &self,
        _branch: &str,
        _no_edit: bool,
        _gate: &GitGate,
    ) -> Result<String, CapabilityError> {
        Err(CapabilityError::Unavailable(
            "Git integration is disabled by user preference".to_string(),
        ))
    }

    async fn merge_abort(&self, _gate: &GitGate) -> Result<(), CapabilityError> {
        Err(CapabilityError::Unavailable(
            "Git integration is disabled by user preference".to_string(),
        ))
    }

    async fn restore_head(&self, _path: &str, _gate: &GitGate) -> Result<(), CapabilityError> {
        Err(CapabilityError::Unavailable(
            "Git integration is disabled by user preference".to_string(),
        ))
    }

    async fn status_porcelain(&self) -> Result<String, CapabilityError> {
        Ok(String::new())
    }

    async fn diff_range(&self, _range: &str) -> Result<String, CapabilityError> {
        Ok(String::new())
    }
}
