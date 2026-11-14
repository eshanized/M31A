//! Managed Git Worktree Sessions
//!
//! Inspired by OpenAI Codex's `codex-rs/worktree`.
//! Provides isolated Git worktrees for subagents and parallel wave execution,
//! protecting the developer's working directory from dirty state and merge contention.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::SystemTime;

use anyhow::{Context, Result, anyhow};
use git2::{BranchType, Repository, WorktreeAddOptions};
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

use crate::WorkflowStore;
use m31a_core::workflow::{WorktreeRecord, WorktreeStatus};

pub const DEFAULT_WORKTREE_KEEP_COUNT: usize = 10;

/// Configuration settings for the worktree manager.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorktreeSettings {
    pub repo_root: PathBuf,
    pub worktree_dir: PathBuf,
    pub keep_count: usize,
}

impl WorktreeSettings {
    pub fn new(repo_root: impl Into<PathBuf>) -> Self {
        let repo_root = repo_root.into();
        let worktree_dir = repo_root.join(".m31a").join("worktrees");
        Self {
            repo_root,
            worktree_dir,
            keep_count: DEFAULT_WORKTREE_KEEP_COUNT,
        }
    }

    pub fn with_worktree_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.worktree_dir = dir.into();
        self
    }

    pub fn with_keep_count(mut self, count: usize) -> Self {
        self.keep_count = count;
        self
    }
}

/// Metadata describing an active or archived managed worktree.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManagedWorktree {
    pub thread_id: String,
    pub worktree_path: PathBuf,
    pub branch: String,
    pub head_sha: String,
    pub source_root: PathBuf,
    pub created_at: u64,
    #[serde(default)]
    pub base_sha: Option<String>,
    #[serde(default)]
    pub workflow_id: Option<String>,
    #[serde(default)]
    pub task_id: Option<String>,
    #[serde(default)]
    pub status: WorktreeStatus,
}

/// Orchestrates creation, tracking, and pruning of isolated Git worktrees.
#[derive(Debug, Clone)]
pub struct WorktreeManager {
    settings: WorktreeSettings,
    store: Option<WorkflowStore>,
}

impl WorktreeManager {
    pub fn new(settings: WorktreeSettings) -> Self {
        Self {
            settings,
            store: None,
        }
    }

    pub fn with_store(mut self, store: WorkflowStore) -> Self {
        self.store = Some(store);
        self
    }

    pub fn store(&self) -> Option<&WorkflowStore> {
        self.store.as_ref()
    }

    pub fn settings(&self) -> &WorktreeSettings {
        &self.settings
    }

    /// Spawns a new isolated Git worktree for an agent thread or task.
    pub fn create_worktree(
        &self,
        thread_id: &str,
        base_commit_or_ref: Option<&str>,
    ) -> Result<ManagedWorktree> {
        self.create_task_worktree(None, None, thread_id, base_commit_or_ref)
    }

    /// Spawns a new isolated Git worktree explicitly tagged with workflow and task metadata.
    pub fn create_task_worktree(
        &self,
        workflow_id: Option<&str>,
        task_id: Option<&str>,
        thread_id: &str,
        base_commit_or_ref: Option<&str>,
    ) -> Result<ManagedWorktree> {
        let repo = Repository::discover(&self.settings.repo_root)
            .context("Failed to open source Git repository for worktree creation")?;

        fs::create_dir_all(&self.settings.worktree_dir)
            .context("Failed to create worktree container directory")?;

        let worktree_path = self.settings.worktree_dir.join(thread_id);
        if worktree_path.exists() {
            info!(
                "Worktree already exists at {:?}; reusing existing directory",
                worktree_path
            );
            let head_sha = self
                .get_head_sha(&worktree_path)
                .unwrap_or_else(|_| "HEAD".to_string());
            let managed = ManagedWorktree {
                thread_id: thread_id.to_string(),
                worktree_path,
                branch: format!("m31a/thread-{}", thread_id),
                head_sha: head_sha.clone(),
                source_root: self.settings.repo_root.clone(),
                created_at: SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
                base_sha: Some(head_sha),
                workflow_id: workflow_id.map(String::from),
                task_id: task_id.map(String::from),
                status: WorktreeStatus::Active,
            };
            return Ok(managed);
        }

        let branch_name = format!("m31a/thread-{}", thread_id);

        // Resolve target commit
        let target_commit = match base_commit_or_ref {
            Some(rev) => {
                let obj = repo
                    .revparse_single(rev)
                    .context(format!("Failed to resolve base ref {}", rev))?;
                obj.peel_to_commit()?
            }
            None => {
                let head = repo.head().context("Failed to resolve repository HEAD")?;
                head.peel_to_commit()?
            }
        };

        let head_sha = target_commit.id().to_string();
        let base_sha = head_sha.clone();

        // Create the branch if not exists
        let branch = match repo.find_branch(&branch_name, BranchType::Local) {
            Ok(b) => b,
            Err(_) => repo.branch(&branch_name, &target_commit, true)?,
        };

        let reference = branch.into_reference();

        // Add worktree using git2 API or CLI fallback if unsupported by target platform
        let mut add_opts = WorktreeAddOptions::new();
        add_opts.reference(Some(&reference));

        match repo.worktree(thread_id, &worktree_path, Some(&add_opts)) {
            Ok(_) => {
                debug!("Created git2 worktree at {:?}", worktree_path);
            }
            Err(e) => {
                warn!(
                    "git2 worktree add encountered error ({}); falling back to git CLI",
                    e
                );
                let output = Command::new("git")
                    .current_dir(&self.settings.repo_root)
                    .args([
                        "worktree",
                        "add",
                        "--force",
                        "-B",
                        &branch_name,
                        worktree_path.to_str().unwrap(),
                        &head_sha,
                    ])
                    .output()
                    .context("Failed to invoke git worktree CLI fallback")?;

                if !output.status.success() {
                    let err = String::from_utf8_lossy(&output.stderr);
                    return Err(anyhow!("git worktree add failed: {}", err));
                }
            }
        }

        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let managed = ManagedWorktree {
            thread_id: thread_id.to_string(),
            worktree_path: worktree_path.clone(),
            branch: branch_name.clone(),
            head_sha: head_sha.clone(),
            source_root: self.settings.repo_root.clone(),
            created_at: now,
            base_sha: Some(base_sha.clone()),
            workflow_id: workflow_id.map(String::from),
            task_id: task_id.map(String::from),
            status: WorktreeStatus::Active,
        };

        // Persist worktree record in storage if store is configured
        if let Some(ref store) = self.store {
            let record = WorktreeRecord {
                id: format!("wt-{}", thread_id),
                thread_id: thread_id.to_string(),
                workflow_id: workflow_id.map(String::from),
                task_id: task_id.map(String::from),
                worktree_path: worktree_path.clone(),
                branch: branch_name,
                base_sha,
                head_sha,
                source_root: self.settings.repo_root.clone(),
                status: WorktreeStatus::Active,
                unintegrated_diff: None,
                created_at_epoch_ms: (now as i64) * 1000,
                updated_at_epoch_ms: (now as i64) * 1000,
            };
            let _ = store.save_worktree(&record);
        }

        // Enforce LRU keep count
        let _ = self.prune_excess_worktrees();

        Ok(managed)
    }

    /// Lists all managed worktrees under the configured worktree directory.
    pub fn list_worktrees(&self) -> Result<Vec<ManagedWorktree>> {
        if !self.settings.worktree_dir.exists() {
            return Ok(Vec::new());
        }

        let mut results = Vec::new();
        for entry in fs::read_dir(&self.settings.worktree_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                let name = entry.file_name().to_string_lossy().to_string();
                let head_sha = self.get_head_sha(&path).unwrap_or_default();
                let created_at = entry
                    .metadata()
                    .and_then(|m| m.modified())
                    .unwrap_or_else(|_| SystemTime::now())
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();

                let head_sha_clone = head_sha.clone();
                results.push(ManagedWorktree {
                    thread_id: name.clone(),
                    worktree_path: path,
                    branch: format!("m31a/thread-{}", name),
                    head_sha,
                    source_root: self.settings.repo_root.clone(),
                    created_at,
                    base_sha: Some(head_sha_clone),
                    workflow_id: None,
                    task_id: None,
                    status: WorktreeStatus::Active,
                });
            }
        }

        results.sort_by_key(|w| w.created_at);
        Ok(results)
    }

    /// Preserves an isolated worktree and its unintegrated changes for forensic investigation or recovery.
    pub fn preserve_worktree(&self, thread_id: &str, unintegrated_diff: Option<&str>) -> Result<()> {
        if let Some(ref store) = self.store {
            let _ = store.update_worktree_status(thread_id, WorktreeStatus::Preserved, unintegrated_diff);
        }
        Ok(())
    }

    /// Recovers all active or preserved worktrees for a workflow without losing pending changes.
    pub fn recover_interrupted_worktrees(&self, workflow_id: &str) -> Result<Vec<ManagedWorktree>> {
        let mut results = Vec::new();
        if let Some(ref store) = self.store {
            let records = store.recover_interrupted_worktrees(workflow_id)?;
            for rec in records {
                if rec.worktree_path.exists() {
                    let uncommitted = self
                        .export_worktree_diff(&rec.worktree_path, &rec.base_sha)
                        .ok();
                    let diff_to_save = uncommitted.or(rec.unintegrated_diff.clone());
                    let _ = store.update_worktree_status(
                        &rec.thread_id,
                        WorktreeStatus::Preserved,
                        diff_to_save.as_deref(),
                    );
                    results.push(ManagedWorktree {
                        thread_id: rec.thread_id,
                        worktree_path: rec.worktree_path,
                        branch: rec.branch,
                        head_sha: rec.head_sha,
                        source_root: rec.source_root,
                        created_at: (rec.created_at_epoch_ms / 1000) as u64,
                        base_sha: Some(rec.base_sha),
                        workflow_id: rec.workflow_id,
                        task_id: rec.task_id,
                        status: WorktreeStatus::Preserved,
                    });
                }
            }
        }
        Ok(results)
    }

    /// Removes an isolated worktree and prunes its associated Git branch.
    pub fn cleanup_worktree(&self, thread_id: &str) -> Result<()> {
        let worktree_path = self.settings.worktree_dir.join(thread_id);

        if self.settings.repo_root.exists() {
            let _ = Command::new("git")
                .current_dir(&self.settings.repo_root)
                .args(["worktree", "remove", "--force", thread_id])
                .output();

            let _ = Command::new("git")
                .current_dir(&self.settings.repo_root)
                .args(["worktree", "prune"])
                .output();
        }

        if worktree_path.exists() {
            let _ = fs::remove_dir_all(&worktree_path);
        }

        if let Some(ref store) = self.store {
            let _ = store.update_worktree_status(thread_id, WorktreeStatus::CleanedUp, None);
        }

        Ok(())
    }

    /// Prunes excess worktrees beyond `keep_count` according to LRU timestamp.
    pub fn prune_excess_worktrees(&self) -> Result<usize> {
        let worktrees = self.list_worktrees()?;
        if worktrees.len() <= self.settings.keep_count {
            return Ok(0);
        }

        let excess_count = worktrees.len() - self.settings.keep_count;
        let mut pruned = 0;

        for wt in worktrees.iter().take(excess_count) {
            info!("Pruning excess worktree: {:?}", wt.worktree_path);
            if self.cleanup_worktree(&wt.thread_id).is_ok() {
                pruned += 1;
            }
        }

        Ok(pruned)
    }

    /// Validates all file paths referenced in a patch diff.
    /// Rejects directory traversal (..), root escapes, and protected paths (.git, .ssh, .m31a, .aws, .env, etc.).
    pub fn validate_patch_paths(patch_diff: &str) -> Result<Vec<PathBuf>> {
        let mut modified = Vec::new();
        for line in patch_diff.lines() {
            let trimmed = line.trim();
            let target_str = if let Some(stripped) = trimmed.strip_prefix("+++ b/") {
                Some(stripped)
            } else if let Some(stripped) = trimmed.strip_prefix("--- a/") {
                Some(stripped)
            } else if let Some(stripped) = trimmed.strip_prefix("diff --git a/") {
                stripped.split_whitespace().next()
            } else {
                None
            };

            if let Some(p_str) = target_str {
                if p_str == "/dev/null" || p_str.is_empty() {
                    continue;
                }
                let path = Path::new(p_str);
                for component in path.components() {
                    if matches!(component, std::path::Component::ParentDir) {
                        return Err(anyhow!(
                            "Directory traversal attempt rejected in patch path: '{}'",
                            p_str
                        ));
                    }
                    if matches!(component, std::path::Component::RootDir) {
                        return Err(anyhow!(
                            "Absolute root path rejected in patch path: '{}'",
                            p_str
                        ));
                    }
                }

                let norm = p_str.replace('\\', "/");
                let norm_lower = norm.to_lowercase();
                if norm_lower.starts_with(".git/")
                    || norm_lower == ".git"
                    || norm_lower.starts_with(".ssh/")
                    || norm_lower == ".ssh"
                    || norm_lower.starts_with(".m31a/")
                    || norm_lower == ".m31a"
                    || norm_lower.starts_with(".aws/")
                    || norm_lower == ".aws"
                    || norm_lower.starts_with(".gnupg/")
                    || norm_lower == ".gnupg"
                    || norm_lower == ".env"
                    || norm_lower.starts_with(".env.")
                    || norm_lower.contains("/.git/")
                    || norm_lower.contains("/.ssh/")
                    || norm_lower.contains("/.m31a/")
                {
                    return Err(anyhow!(
                        "Patch targets forbidden protected path: '{}'",
                        p_str
                    ));
                }

                let pb = PathBuf::from(p_str);
                if !modified.contains(&pb) {
                    modified.push(pb);
                }
            }
        }
        Ok(modified)
    }

    /// Checks if a git repository directory has uncommitted tracked or untracked changes,
    /// ignoring internal `.m31a` metadata/worktree directories.
    pub fn is_dirty(&self, repo_path: &Path) -> Result<bool> {
        let output = Command::new("git")
            .current_dir(repo_path)
            .args(["status", "--porcelain"])
            .output()
            .context("Failed running git status --porcelain")?;
        if !output.status.success() {
            return Err(anyhow!("git status failed"));
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        let has_dirty = stdout.lines().any(|line| {
            let path_part = line.get(3..).unwrap_or("").trim();
            !path_part.is_empty()
                && !path_part.starts_with(".m31a")
                && !path_part.starts_with(".git")
                && !path_part.starts_with("artifacts")
                && !path_part.starts_with("target")
        });
        Ok(has_dirty)
    }

    /// Evaluates whether a patch diff from a child worktree can be cleanly applied
    /// to the parent repository HEAD without conflicts (Two-Phase Git Preflight).
    pub fn git_preflight_check(&self, patch_diff: &str) -> Result<bool> {
        Self::validate_patch_paths(patch_diff)?;

        let mut child = Command::new("git")
            .current_dir(&self.settings.repo_root)
            .args(["apply", "--check", "--ignore-whitespace", "-"])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .context("Failed to spawn git apply --check preflight")?;

        if let Some(mut stdin) = child.stdin.take() {
            let _ = std::io::Write::write_all(&mut stdin, patch_diff.as_bytes());
        }

        let output = child.wait_with_output()?;
        if output.status.success() {
            Ok(true)
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            debug!("Git preflight check failed: {}", stderr);
            Ok(false)
        }
    }

    /// Performs an isolated integration test of a patch diff in a temporary worktree.
    /// Spawns a temporary integration worktree from HEAD, applies the patch, runs test verification,
    /// and cleans up the temporary worktree.
    pub fn test_patch_in_isolated_worktree<F, R>(&self, patch_diff: &str, test_fn: F) -> Result<R>
    where
        F: FnOnce(&Path) -> Result<R>,
    {
        Self::validate_patch_paths(patch_diff)?;
        let integration_id = format!("integrate-{}", uuid::Uuid::new_v4());
        let wt = self.create_worktree(&integration_id, None)?;

        // Apply diff in isolated integration worktree
        let apply_result = (|| {
            let mut apply_cmd = Command::new("git")
                .current_dir(&wt.worktree_path)
                .args(["apply", "--ignore-whitespace", "-"])
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()
                .context("Failed to spawn git apply in isolated integration worktree")?;

            if let Some(mut stdin) = apply_cmd.stdin.take() {
                let _ = std::io::Write::write_all(&mut stdin, patch_diff.as_bytes());
            }

            let apply_out = apply_cmd.wait_with_output()?;
            if !apply_out.status.success() {
                let err = String::from_utf8_lossy(&apply_out.stderr);
                return Err(anyhow!(
                    "Failed applying patch in integration worktree: {}",
                    err
                ));
            }

            test_fn(&wt.worktree_path)
        })();

        // Clean up on success; preserve worktree on rejection for diagnostic investigation
        if apply_result.is_ok() {
            let _ = self.cleanup_worktree(&integration_id);
        } else {
            tracing::warn!(
                "Preserving rejected integration worktree at '{}' for forensic diagnostics",
                wt.worktree_path.display()
            );
        }
        apply_result
    }

    /// Exports the cumulative diff produced in a child worktree against its base commit.
    pub fn export_worktree_diff(&self, worktree_path: &Path, base_sha: &str) -> Result<String> {
        // Stage untracked files with intent-to-add so git diff includes newly created files
        let _ = Command::new("git")
            .current_dir(worktree_path)
            .args(["add", "-N", "."])
            .output();

        let output = Command::new("git")
            .current_dir(worktree_path)
            .args(["diff", base_sha])
            .output()
            .context("Failed to run git diff in worktree")?;

        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!("git diff error: {}", err));
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    /// Two-phase preflight verification and commit of child worktree changes into the parent
    /// repository with structured multi-agent Git trailers (`Agent-Id`, `Agent-Role`).
    pub fn apply_and_commit_worktree_diff(
        &self,
        patch_diff: &str,
        commit_msg: &str,
        agent_id: &str,
        agent_role: &str,
    ) -> Result<String> {
        if patch_diff.trim().is_empty() {
            return self.get_head_sha(&self.settings.repo_root);
        }

        // Validate patch paths
        Self::validate_patch_paths(patch_diff)?;

        // Never apply directly onto a dirty parent working tree
        if self.is_dirty(&self.settings.repo_root)? {
            return Err(anyhow!(
                "Cannot apply worktree diff: parent repository working directory contains uncommitted changes. Clean working directory required."
            ));
        }

        // Phase 1: Preflight check before touching working directory
        if !self.git_preflight_check(patch_diff)? {
            return Err(anyhow!(
                "Git preflight validation failed: child worktree diff conflicts with parent repository HEAD"
            ));
        }

        // Phase 2: Apply the diff to the repository root
        let mut apply_cmd = Command::new("git")
            .current_dir(&self.settings.repo_root)
            .args(["apply", "--ignore-whitespace", "-"])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .context("Failed to spawn git apply in parent repository")?;

        if let Some(mut stdin) = apply_cmd.stdin.take() {
            let _ = std::io::Write::write_all(&mut stdin, patch_diff.as_bytes());
        }

        let apply_out = apply_cmd.wait_with_output()?;
        if !apply_out.status.success() {
            let err = String::from_utf8_lossy(&apply_out.stderr);
            return Err(anyhow!("Failed applying child worktree diff: {}", err));
        }

        // Stage applied changes
        let stage_out = Command::new("git")
            .current_dir(&self.settings.repo_root)
            .args(["add", "-A"])
            .output()
            .context("Failed staging changes with git add -A")?;
        if !stage_out.status.success() {
            let err = String::from_utf8_lossy(&stage_out.stderr);
            return Err(anyhow!("git add failed: {}", err));
        }

        // Format commit message with structured Codex agent trailers
        let full_commit_message = format!(
            "{}\n\nAgent-Id: {}\nAgent-Role: {}\nCo-authored-by: M31A Agent <agent-{}@m31a.local>",
            commit_msg.trim(),
            agent_id,
            agent_role,
            agent_id
        );

        let commit_out = Command::new("git")
            .current_dir(&self.settings.repo_root)
            .args([
                "-c",
                "user.name=M31A Agent Swarm",
                "-c",
                "user.email=agent-swarm@m31a.local",
                "commit",
                "-m",
                &full_commit_message,
            ])
            .output()
            .context("Failed executing git commit for agent changes")?;

        if !commit_out.status.success() {
            let err = String::from_utf8_lossy(&commit_out.stderr);
            return Err(anyhow!("git commit failed: {}", err));
        }

        self.get_head_sha(&self.settings.repo_root)
    }

    fn get_head_sha(&self, dir: &Path) -> Result<String> {
        let repo = Repository::open(dir)?;
        let head = repo.head()?;
        let commit = head.peel_to_commit()?;
        Ok(commit.id().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn init_dummy_repo() -> (tempfile::TempDir, PathBuf) {
        let dir = tempdir().unwrap();
        let path = dir.path().to_path_buf();

        let repo = Repository::init(&path).unwrap();
        if let Ok(mut config) = repo.config() {
            let _ = config.set_str("user.name", "M31A Test");
            let _ = config.set_str("user.email", "test@m31a.local");
        }
        let sig = git2::Signature::now("M31A Test", "test@m31a.local").unwrap();

        let file_path = path.join("README.md");
        fs::write(&file_path, "# Test Repository\n").unwrap();

        let mut index = repo.index().unwrap();
        index.add_path(Path::new("README.md")).unwrap();
        index.write().unwrap();
        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();

        repo.commit(Some("HEAD"), &sig, &sig, "Initial commit", &tree, &[])
            .unwrap();

        (dir, path)
    }

    #[test]
    fn test_worktree_settings_defaults() {
        let settings = WorktreeSettings::new("/tmp/myrepo");
        assert_eq!(settings.repo_root, PathBuf::from("/tmp/myrepo"));
        assert_eq!(
            settings.worktree_dir,
            PathBuf::from("/tmp/myrepo/.m31a/worktrees")
        );
        assert_eq!(settings.keep_count, DEFAULT_WORKTREE_KEEP_COUNT);
    }

    #[test]
    fn test_create_and_cleanup_worktree() {
        let (_temp, repo_path) = init_dummy_repo();
        let settings = WorktreeSettings::new(&repo_path);
        let manager = WorktreeManager::new(settings);

        let managed = manager.create_worktree("test-agent-1", None).unwrap();
        assert_eq!(managed.thread_id, "test-agent-1");
        assert!(managed.worktree_path.exists());
        assert!(managed.worktree_path.join("README.md").exists());

        let list = manager.list_worktrees().unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].thread_id, "test-agent-1");

        manager.cleanup_worktree("test-agent-1").unwrap();
        assert!(!managed.worktree_path.exists());

        let list_after = manager.list_worktrees().unwrap();
        assert_eq!(list_after.len(), 0);
    }

    #[test]
    fn test_prune_excess_worktrees() {
        let (_temp, repo_path) = init_dummy_repo();
        let settings = WorktreeSettings::new(&repo_path).with_keep_count(2);
        let manager = WorktreeManager::new(settings);

        manager.create_worktree("agent-1", None).unwrap();
        manager.create_worktree("agent-2", None).unwrap();
        manager.create_worktree("agent-3", None).unwrap();

        let remaining = manager.list_worktrees().unwrap();
        assert!(remaining.len() <= 2);
    }

    #[test]
    fn test_git_preflight_and_trailer_commit() {
        let (_temp, repo_path) = init_dummy_repo();
        let settings = WorktreeSettings::new(&repo_path);
        let manager = WorktreeManager::new(settings);

        let patch = "--- a/README.md\n+++ b/README.md\n@@ -1 +1,2 @@\n # Test Repository\n+Added by agent\n";
        let preflight_ok = manager.git_preflight_check(patch).unwrap();
        assert!(preflight_ok);

        let commit_sha = manager
            .apply_and_commit_worktree_diff(
                patch,
                "feat: update readme via swarm",
                "subagent-42",
                "Code Writer",
            )
            .unwrap();

        assert!(!commit_sha.is_empty());
        let updated_content = fs::read_to_string(repo_path.join("README.md")).unwrap();
        assert!(updated_content.contains("Added by agent"));
    }
}
