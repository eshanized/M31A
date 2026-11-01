//! Isolated Git worktree manager for sandboxed subagent modifications.
//!
//! Creates and manages dedicated git worktrees allowing subagents to test code mutations in isolation.

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use uuid::Uuid;

pub const DEFAULT_WORKTREE_KEEP_COUNT: usize = 5;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManagedWorktree {
    pub id: String,
    pub path: PathBuf,
    pub source_root: PathBuf,
    pub branch: Option<String>,
    pub head_sha: String,
    pub is_detached: bool,
    pub is_locked: bool,
    pub is_prunable: bool,
    pub created_at: i64,
}

impl std::fmt::Display for ManagedWorktree {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "[{}] {} (branch: {}, head: {}{})",
            self.id,
            self.path.display(),
            self.branch.as_deref().unwrap_or("detached"),
            &self.head_sha[..self.head_sha.len().min(8)],
            if self.is_locked {
                ", locked"
            } else if self.is_prunable {
                ", prunable"
            } else {
                ""
            }
        )
    }
}

#[derive(Debug, Clone)]
pub struct WorktreeManager {
    worktrees_root: PathBuf,
    keep_count: usize,
}

impl WorktreeManager {
    pub fn default_root() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        PathBuf::from(home).join(".m31a").join("worktrees")
    }

    pub fn new() -> Result<Self> {
        let root = Self::default_root();
        fs::create_dir_all(&root)?;
        Ok(Self {
            worktrees_root: root,
            keep_count: DEFAULT_WORKTREE_KEEP_COUNT,
        })
    }

    pub fn with_root(root: PathBuf) -> Self {
        Self {
            worktrees_root: root,
            keep_count: DEFAULT_WORKTREE_KEEP_COUNT,
        }
    }

    pub fn root(&self) -> &Path {
        &self.worktrees_root
    }

    pub fn keep_count(&self) -> usize {
        self.keep_count
    }

    pub fn with_keep_count(mut self, count: usize) -> Self {
        self.keep_count = count;
        self
    }

    pub fn repository_root(cwd: &Path) -> Result<PathBuf> {
        let out = Command::new("git")
            .args([
                "-c",
                "core.hooksPath=/dev/null",
                "rev-parse",
                "--show-toplevel",
            ])
            .current_dir(cwd)
            .output()
            .with_context(|| format!("Failed to run git rev-parse in {:?}", cwd))?;

        if !out.status.success() {
            bail!("Directory {:?} is not inside a git repository", cwd);
        }

        let root_str = String::from_utf8_lossy(&out.stdout).trim().to_string();
        Ok(PathBuf::from(root_str))
    }

    pub fn default_worktree_base(cwd: &Path) -> Result<String> {
        let repo_root = Self::repository_root(cwd)?;
        let out = Command::new("git")
            .args([
                "-c",
                "core.hooksPath=/dev/null",
                "rev-parse",
                "--verify",
                "HEAD",
            ])
            .current_dir(&repo_root)
            .output()?;

        if out.status.success() {
            return Ok("HEAD".to_string());
        }
        bail!("Failed to determine default worktree base revision")
    }

    pub fn create_worktree(&self, cwd: &Path, base_ref: Option<&str>) -> Result<ManagedWorktree> {
        let repo_root = Self::repository_root(cwd)?;
        let repo_name = repo_root
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("repo");

        let revision = base_ref.unwrap_or("HEAD");
        let rev_out = Command::new("git")
            .args(["-c", "core.hooksPath=/dev/null", "rev-parse", revision])
            .current_dir(&repo_root)
            .output()?;

        if !rev_out.status.success() {
            bail!("Failed to resolve git revision `{}`", revision);
        }

        let head_sha = String::from_utf8_lossy(&rev_out.stdout).trim().to_string();

        let branch_out = Command::new("git")
            .args([
                "-c",
                "core.hooksPath=/dev/null",
                "rev-parse",
                "--abbrev-ref",
                revision,
            ])
            .current_dir(&repo_root)
            .output()?;
        let branch = if branch_out.status.success() {
            let b = String::from_utf8_lossy(&branch_out.stdout)
                .trim()
                .to_string();
            if b != "HEAD" { Some(b) } else { None }
        } else {
            None
        };

        let id = Uuid::new_v4().to_string()[..8].to_string();
        let worktree_dir = self.worktrees_root.join(repo_name).join(&id);
        if let Some(parent) = worktree_dir.parent() {
            fs::create_dir_all(parent)?;
        }

        let add_out = Command::new("git")
            .args([
                "-c",
                "core.hooksPath=/dev/null",
                "worktree",
                "add",
                "--detach",
                worktree_dir.to_str().unwrap(),
                &head_sha,
            ])
            .current_dir(&repo_root)
            .output()?;

        if !add_out.status.success() {
            let err = String::from_utf8_lossy(&add_out.stderr);
            bail!("git worktree add failed: {}", err);
        }

        // Pin core.worktree in config.worktree if git supports it
        let _ = Command::new("git")
            .args([
                "-c",
                "core.hooksPath=/dev/null",
                "config",
                "core.worktree",
                worktree_dir.to_str().unwrap(),
            ])
            .current_dir(&worktree_dir)
            .output();

        Ok(ManagedWorktree {
            id,
            path: worktree_dir,
            source_root: repo_root,
            branch,
            head_sha,
            is_detached: true,
            is_locked: false,
            is_prunable: false,
            created_at: chrono::Utc::now().timestamp(),
        })
    }

    pub fn list_worktrees(&self, cwd: &Path) -> Result<Vec<ManagedWorktree>> {
        let repo_root = Self::repository_root(cwd)?;
        let out = Command::new("git")
            .args([
                "-c",
                "core.hooksPath=/dev/null",
                "worktree",
                "list",
                "--porcelain",
            ])
            .current_dir(&repo_root)
            .output()?;

        if !out.status.success() {
            bail!("git worktree list --porcelain failed");
        }

        let stdout = String::from_utf8_lossy(&out.stdout);
        let parsed = parse_porcelain_worktree_list(&stdout, &repo_root);
        Ok(parsed)
    }

    pub fn remove_worktree(&self, worktree_path: &Path) -> Result<()> {
        let parent = worktree_path.parent().unwrap_or(worktree_path);
        let out = Command::new("git")
            .args([
                "-c",
                "core.hooksPath=/dev/null",
                "worktree",
                "remove",
                "--force",
                worktree_path.to_str().unwrap(),
            ])
            .current_dir(parent)
            .output()?;

        if !out.status.success() {
            // If git command fails, remove directory directly
            let _ = fs::remove_dir_all(worktree_path);
        }

        // Run git worktree prune to clean up git metadata
        let _ = self.prune(parent);
        Ok(())
    }

    pub fn prune(&self, cwd: &Path) -> Result<()> {
        let _ = Command::new("git")
            .args(["-c", "core.hooksPath=/dev/null", "worktree", "prune"])
            .current_dir(cwd)
            .output();
        Ok(())
    }

    pub fn clean_prunable(&self, cwd: &Path) -> Result<usize> {
        let worktrees = self.list_worktrees(cwd)?;
        let mut cleaned = 0;
        for wt in worktrees {
            if wt.is_prunable || !wt.path.exists() {
                let _ = self.remove_worktree(&wt.path);
                cleaned += 1;
            }
        }
        Ok(cleaned)
    }
}

pub fn parse_porcelain_worktree_list(porcelain: &str, source_root: &Path) -> Vec<ManagedWorktree> {
    let mut results = Vec::new();
    let mut current_path: Option<PathBuf> = None;
    let mut current_sha = String::new();
    let mut current_branch: Option<String> = None;
    let mut is_detached = false;
    let mut is_locked = false;
    let mut is_prunable = false;

    let flush = |results: &mut Vec<ManagedWorktree>,
                 path: &mut Option<PathBuf>,
                 sha: &mut String,
                 branch: &mut Option<String>,
                 detached: &mut bool,
                 locked: &mut bool,
                 prunable: &mut bool| {
        if let Some(p) = path.take() {
            let id = p
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("default")
                .to_string();
            results.push(ManagedWorktree {
                id,
                path: p,
                source_root: source_root.to_path_buf(),
                branch: branch.take(),
                head_sha: std::mem::take(sha),
                is_detached: *detached,
                is_locked: *locked,
                is_prunable: *prunable,
                created_at: chrono::Utc::now().timestamp(),
            });
            *detached = false;
            *locked = false;
            *prunable = false;
        }
    };

    for line in porcelain.lines() {
        let line = line.trim();
        if line.is_empty() {
            flush(
                &mut results,
                &mut current_path,
                &mut current_sha,
                &mut current_branch,
                &mut is_detached,
                &mut is_locked,
                &mut is_prunable,
            );
            continue;
        }

        if let Some(p) = line.strip_prefix("worktree ") {
            flush(
                &mut results,
                &mut current_path,
                &mut current_sha,
                &mut current_branch,
                &mut is_detached,
                &mut is_locked,
                &mut is_prunable,
            );
            current_path = Some(PathBuf::from(p.trim()));
        } else if let Some(sha) = line.strip_prefix("HEAD ") {
            current_sha = sha.trim().to_string();
        } else if let Some(b) = line.strip_prefix("branch ") {
            let b_str = b.trim();
            let short = b_str.strip_prefix("refs/heads/").unwrap_or(b_str);
            current_branch = Some(short.to_string());
        } else if line == "detached" {
            is_detached = true;
        } else if line.starts_with("locked") {
            is_locked = true;
        } else if line.starts_with("prunable") {
            is_prunable = true;
        }
    }

    flush(
        &mut results,
        &mut current_path,
        &mut current_sha,
        &mut current_branch,
        &mut is_detached,
        &mut is_locked,
        &mut is_prunable,
    );

    results
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_worktree_manager_initialization() {
        let tmp = tempdir().unwrap();
        let mgr = WorktreeManager::with_root(tmp.path().to_path_buf());
        assert_eq!(mgr.worktrees_root, tmp.path());
    }

    #[test]
    fn test_parse_porcelain_worktree_list() {
        let raw = r#"worktree /home/user/project
HEAD 0123456789abcdef0123456789abcdef01234567
branch refs/heads/main

worktree /home/user/.m31a/worktrees/project/wt-1234
HEAD fedcba9876543210fedcba9876543210fedcba98
detached
locked testing

worktree /home/user/.m31a/worktrees/project/wt-5678
HEAD 1111111111111111111111111111111111111111
prunable gitdir file points to non-existent location
"#;

        let root = PathBuf::from("/home/user/project");
        let list = parse_porcelain_worktree_list(raw, &root);

        assert_eq!(list.len(), 3);
        assert_eq!(list[0].path, PathBuf::from("/home/user/project"));
        assert_eq!(list[0].branch, Some("main".to_string()));
        assert!(!list[0].is_detached);

        assert_eq!(list[1].id, "wt-1234");
        assert!(list[1].is_detached);
        assert!(list[1].is_locked);

        assert_eq!(list[2].id, "wt-5678");
        assert!(list[2].is_prunable);
    }
}
