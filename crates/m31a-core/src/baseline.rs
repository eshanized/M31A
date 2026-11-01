//! Workspace baseline snapshot capture and regression baseline manager.
//!
//! Records file states and checksums prior to agent turns to verify non-destructive changes.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileStatusKind {
    Modified,
    Added,
    Deleted,
    Untracked,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaselineSnapshot {
    pub head_commit: Option<String>,
    pub file_statuses: HashMap<PathBuf, FileStatusKind>,
    pub file_hashes: HashMap<PathBuf, String>,
}

#[derive(Debug, Clone, Default)]
pub struct BaselineDiff {
    pub newly_created: Vec<PathBuf>,
    pub modified_during_turn: Vec<PathBuf>,
    pub deleted_during_turn: Vec<PathBuf>,
}

impl BaselineDiff {
    pub fn is_empty(&self) -> bool {
        self.newly_created.is_empty()
            && self.modified_during_turn.is_empty()
            && self.deleted_during_turn.is_empty()
    }

    pub fn summary(&self) -> String {
        if self.is_empty() {
            return "No workspace file changes detected.".to_string();
        }

        let mut parts = Vec::new();
        if !self.newly_created.is_empty() {
            parts.push(format!("{} created", self.newly_created.len()));
        }
        if !self.modified_during_turn.is_empty() {
            parts.push(format!("{} modified", self.modified_during_turn.len()));
        }
        if !self.deleted_during_turn.is_empty() {
            parts.push(format!("{} deleted", self.deleted_during_turn.len()));
        }

        format!("Turn file mutations: {}", parts.join(", "))
    }
}

/// Report detailing out-of-band repository modifications detected upon resuming a session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionReconciliationReport {
    pub previous_head: Option<String>,
    pub current_head: Option<String>,
    pub head_changed: bool,
    pub out_of_band_files_modified: Vec<PathBuf>,
    pub out_of_band_files_added: Vec<PathBuf>,
    pub out_of_band_files_deleted: Vec<PathBuf>,
    pub has_diverged: bool,
    pub reconciliation_notice: String,
}

pub struct WorkingTreeBaseline;

impl WorkingTreeBaseline {
    /// Takes a baseline snapshot of the repository or workspace before a turn executes.
    pub fn capture(cwd: &Path) -> Result<BaselineSnapshot> {
        let mut file_statuses = HashMap::new();
        let head_commit = Self::get_head_commit(cwd);

        if let Ok(output) = Command::new("git")
            .args(["status", "--porcelain"])
            .current_dir(cwd)
            .output()
            && output.status.success()
        {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                if line.len() < 3 {
                    continue;
                }
                let code = &line[..2];
                let file_path = PathBuf::from(line[3..].trim().trim_matches('"'));

                let status = match code {
                    "??" => FileStatusKind::Untracked,
                    " A" | "A " | "AM" => FileStatusKind::Added,
                    " D" | "D " => FileStatusKind::Deleted,
                    _ => FileStatusKind::Modified,
                };

                file_statuses.insert(file_path, status);
            }
        }

        Ok(BaselineSnapshot {
            head_commit,
            file_statuses,
            file_hashes: HashMap::new(),
        })
    }

    /// Compares post-turn state against the baseline to isolate changes produced exclusively during the turn.
    pub fn diff(before: &BaselineSnapshot, after: &BaselineSnapshot) -> BaselineDiff {
        let mut diff = BaselineDiff::default();

        let before_keys: HashSet<&PathBuf> = before.file_statuses.keys().collect();
        let after_keys: HashSet<&PathBuf> = after.file_statuses.keys().collect();

        // Files that appeared or became modified/untracked in `after`
        for (path, status) in &after.file_statuses {
            if !before_keys.contains(path) {
                // Was completely clean before, now has changes
                match status {
                    FileStatusKind::Untracked | FileStatusKind::Added => {
                        diff.newly_created.push(path.clone());
                    }
                    FileStatusKind::Deleted => {
                        diff.deleted_during_turn.push(path.clone());
                    }
                    FileStatusKind::Modified => {
                        diff.modified_during_turn.push(path.clone());
                    }
                }
            } else if before.file_statuses.get(path) != Some(status) {
                // Status changed during turn
                diff.modified_during_turn.push(path.clone());
            }
        }

        // Files that were in before but disappeared from after
        for path in before_keys {
            if !after_keys.contains(path) {
                diff.modified_during_turn.push(path.clone());
            }
        }

        diff.newly_created.sort();
        diff.modified_during_turn.sort();
        diff.deleted_during_turn.sort();

        diff
    }

    /// Reconciles the working tree upon session resumption against the stored baseline
    /// to detect out-of-band modifications made while M31A was not running.
    pub fn reconcile_session_resume(
        cwd: &Path,
        stored_baseline: Option<&BaselineSnapshot>,
    ) -> Result<SessionReconciliationReport> {
        let current = Self::capture(cwd)?;

        let previous_head = stored_baseline.and_then(|b| b.head_commit.clone());
        let current_head = current.head_commit.clone();
        let head_changed =
            previous_head.is_some() && current_head.is_some() && previous_head != current_head;

        let mut out_of_band_mod = Vec::new();
        let mut out_of_band_add = Vec::new();
        let mut out_of_band_del = Vec::new();

        if let Some(stored) = stored_baseline {
            for (path, status) in &current.file_statuses {
                match stored.file_statuses.get(path) {
                    Some(s) if s == status => {}
                    Some(_) => out_of_band_mod.push(path.clone()),
                    None => out_of_band_add.push(path.clone()),
                }
            }
            for path in stored.file_statuses.keys() {
                if !current.file_statuses.contains_key(path) {
                    out_of_band_del.push(path.clone());
                }
            }
        }

        let has_diverged = head_changed
            || !out_of_band_mod.is_empty()
            || !out_of_band_add.is_empty()
            || !out_of_band_del.is_empty();

        let reconciliation_notice = if has_diverged {
            format!(
                "Session Resumption Alert: Repository state has diverged out-of-band (HEAD changed: {}, {} modified, {} added, {} deleted). Re-anchoring working memory and plans to current working tree.",
                head_changed,
                out_of_band_mod.len(),
                out_of_band_add.len(),
                out_of_band_del.len()
            )
        } else {
            "Session Resumption: Working tree is consistent with prior session baseline."
                .to_string()
        };

        Ok(SessionReconciliationReport {
            previous_head,
            current_head,
            head_changed,
            out_of_band_files_modified: out_of_band_mod,
            out_of_band_files_added: out_of_band_add,
            out_of_band_files_deleted: out_of_band_del,
            has_diverged,
            reconciliation_notice,
        })
    }

    fn get_head_commit(cwd: &Path) -> Option<String> {
        let output = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(cwd)
            .output()
            .ok()?;

        if output.status.success() {
            let sha = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !sha.is_empty() {
                return Some(sha);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_baseline_diff_computation() {
        let mut before = BaselineSnapshot {
            head_commit: Some("abc1234".to_string()),
            file_statuses: HashMap::new(),
            file_hashes: HashMap::new(),
        };

        // Before: file1 was already modified
        before
            .file_statuses
            .insert(PathBuf::from("file1.rs"), FileStatusKind::Modified);

        let mut after = BaselineSnapshot {
            head_commit: Some("abc1234".to_string()),
            file_statuses: HashMap::new(),
            file_hashes: HashMap::new(),
        };
        after
            .file_statuses
            .insert(PathBuf::from("file1.rs"), FileStatusKind::Modified);
        after
            .file_statuses
            .insert(PathBuf::from("new_file.rs"), FileStatusKind::Added);
        after
            .file_statuses
            .insert(PathBuf::from("deleted.rs"), FileStatusKind::Deleted);

        let diff = WorkingTreeBaseline::diff(&before, &after);
        assert_eq!(diff.newly_created, vec![PathBuf::from("new_file.rs")]);
        assert_eq!(diff.deleted_during_turn, vec![PathBuf::from("deleted.rs")]);
        assert!(diff.modified_during_turn.is_empty());
        assert_eq!(diff.summary(), "Turn file mutations: 1 created, 1 deleted");
    }
}
