//! Shared governed-Git test mint helper.
//!
//! Production Git mutations require a [`GitGate`] bound to a live runtime
//! authorization. Integration tests hold their own [`AuthorizationAuthority`]
//! (one per test scope) and mint exactly-scoped gates through it — the same
//! canonical path as production, never a boolean bypass.

#![allow(dead_code)]

use m31a::git::{AuthorizationAuthority, GIT_AUTH_DEFAULT_TTL, GitGate, GitOperation};
use m31a::ids::{MissionId, TaskId};
use std::path::{Path, PathBuf};

/// Per-test authorization scope: mints exactly-scoped gates.
pub struct TestGitAuth {
    pub authority: AuthorizationAuthority,
    pub mission_id: MissionId,
    pub task_id: TaskId,
    pub policy_hash: String,
}

impl TestGitAuth {
    pub fn new() -> Self {
        Self {
            authority: AuthorizationAuthority::new(),
            mission_id: MissionId::new(),
            task_id: TaskId::new(),
            policy_hash: "test-policy-hash".to_string(),
        }
    }

    pub fn with_mission(mut self, mission_id: MissionId) -> Self {
        self.mission_id = mission_id;
        self
    }

    /// Mint a gate covering exactly `op` (+ `additional`) in `workspace`.
    pub fn gate(
        &self,
        op: GitOperation,
        additional: Vec<GitOperation>,
        workspace: &Path,
    ) -> GitGate {
        let auth = self.authority.mint_git_authorization(
            self.mission_id,
            Some(self.task_id),
            None,
            self.policy_hash.clone(),
            workspace.to_path_buf(),
            op,
            additional,
            "test-scope",
            "test-mint",
            GIT_AUTH_DEFAULT_TTL,
        );
        GitGate::authorized_verified(auth, &self.authority).expect("test gate mint must verify")
    }

    pub fn worktree_add_gate(&self, repo_root: &Path, path: &str, branch: &str) -> GitGate {
        self.gate(
            GitOperation::WorktreeAdd {
                path: path.to_string(),
                branch: branch.to_string(),
            },
            Vec::new(),
            repo_root,
        )
    }

    pub fn worktree_remove_gate(
        &self,
        repo_root: &Path,
        force: bool,
        branch: Option<String>,
    ) -> GitGate {
        let additional = branch
            .map(|b| vec![GitOperation::BranchDelete { branch: b }])
            .unwrap_or_default();
        self.gate(
            GitOperation::WorktreeRemove { force },
            additional,
            repo_root,
        )
    }

    pub fn raw_gate(&self, workspace: &Path) -> GitGate {
        self.gate(GitOperation::RawPassthrough, Vec::new(), workspace)
    }

    pub fn commit_gate(&self, message: &str, workspace: &Path) -> GitGate {
        self.gate(
            GitOperation::Commit {
                message: message.to_string(),
            },
            Vec::new(),
            workspace,
        )
    }

    pub fn add_all_gate(&self, workspace: &Path) -> GitGate {
        self.gate(
            GitOperation::Add {
                paths: vec!["-A".to_string()],
            },
            Vec::new(),
            workspace,
        )
    }

    pub fn unstage_gate(&self, paths: &[&str], workspace: &Path) -> GitGate {
        self.gate(
            GitOperation::Unstage {
                paths: paths.iter().map(|s| s.to_string()).collect(),
            },
            Vec::new(),
            workspace,
        )
    }

    pub fn add_paths_gate(&self, paths: &[String], workspace: &Path) -> GitGate {
        self.gate(
            GitOperation::Add {
                paths: paths.to_vec(),
            },
            Vec::new(),
            workspace,
        )
    }

    pub fn checkout_gate(&self, target: &str, workspace: &Path) -> GitGate {
        self.gate(
            GitOperation::Checkout {
                target: target.to_string(),
                force: false,
            },
            Vec::new(),
            workspace,
        )
    }

    pub fn merge_gate(&self, source: &str, workspace: &Path) -> GitGate {
        self.gate(
            GitOperation::Merge {
                source: source.to_string(),
            },
            vec![GitOperation::MergeAbort],
            workspace,
        )
    }

    pub fn merge_abort_gate(&self, workspace: &Path) -> GitGate {
        self.gate(GitOperation::MergeAbort, Vec::new(), workspace)
    }

    pub fn restore_gate(&self, target: &str, workspace: &Path) -> GitGate {
        self.gate(
            GitOperation::SyncCheckout {
                target: target.to_string(),
            },
            Vec::new(),
            workspace,
        )
    }

    pub fn integrate_gate(&self, source: &str, target_branch: &str, repo_root: &Path) -> GitGate {
        self.gate(
            GitOperation::Merge {
                source: source.to_string(),
            },
            vec![
                GitOperation::WorktreeRemove { force: true },
                GitOperation::UpdateRef {
                    git_ref: format!("refs/heads/{target_branch}"),
                },
                GitOperation::SyncCheckout {
                    target: target_branch.to_string(),
                },
            ],
            repo_root,
        )
    }

    pub fn stash_save_gate(&self, mission_id: &MissionId, repo_root: &Path) -> GitGate {
        self.gate(
            GitOperation::StashSave,
            vec![
                GitOperation::UpdateRef {
                    git_ref: format!("refs/m31a/stash/{mission_id}"),
                },
                GitOperation::SyncCheckout {
                    target: "HEAD".to_string(),
                },
            ],
            repo_root,
        )
    }

    pub fn stash_apply_gate(&self, repo_root: &Path) -> GitGate {
        self.gate(GitOperation::StashApply, Vec::new(), repo_root)
    }

    pub fn stash_drop_gate(&self, mission_id: &MissionId, repo_root: &Path) -> GitGate {
        self.gate(
            GitOperation::UpdateRef {
                git_ref: format!("refs/m31a/stash/{mission_id}"),
            },
            Vec::new(),
            repo_root,
        )
    }

    /// Gate covering both apply and drop (for `pop`).
    pub fn stash_pop_gate(&self, mission_id: &MissionId, repo_root: &Path) -> GitGate {
        self.gate(
            GitOperation::StashApply,
            vec![GitOperation::UpdateRef {
                git_ref: format!("refs/m31a/stash/{mission_id}"),
            }],
            repo_root,
        )
    }

    /// Workspace path helper for tests using `TempDir`.
    pub fn ws(path: &Path) -> PathBuf {
        path.to_path_buf()
    }
}

impl Default for TestGitAuth {
    fn default() -> Self {
        Self::new()
    }
}
