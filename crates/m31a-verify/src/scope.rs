//! Evaluation Scope Resolver for M31A.
//!
//! Resolves the exact union of files touched by task commits to eliminate noise from interleaved branch activity:
//!
//! Solves the fundamental flaw of naive verification scopes (`HEAD~1..HEAD`, `DIFF_BASE..HEAD`, `git log --all`):
//! 1. Scope is the UNION of each commit's own file set, never an arbitrary commit range.
//!    Ranges retain interleaved non-phase commits.
//! 2. Only commits reachable from the evaluated ref (`HEAD`) count. Commits on other branches
//!    or dropped by rebases are tracked in `unreachable` and contribute nothing.
//! 3. An empty union never becomes an empty scope that reports false success. It degrades to
//!    wider evidence (the phase directory range) and explicitly says so (`status: Degraded`).
//! 4. Files are partitioned into `files` (currently present on disk), `missing_on_disk`
//!    (deleted during or after the unit), and `outside_union` (modified by interleaved commits).

use anyhow::{Context, Result};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

/// What unit of work is being evaluated.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScopeUnit {
    Phase {
        phase: String,
        phase_dir: Option<PathBuf>,
    },
    Plan {
        plan_id: String,
    },
    Quick {
        id: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScopeStatus {
    /// Exactly what the unit's own commits touched.
    Resolved,
    /// Preferred evidence was absent or empty; wider evidence used with reason.
    Degraded { reason: String },
    /// Git or phase directory could not be read.
    Unresolvable { reason: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScopeSource {
    TaskCommits,
    PlanSubjects,
    QuickSubjects,
    PhaseRange,
    None,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScopeCommit {
    pub sha: String,
    pub subject: String,
    pub files: Vec<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScopeOptions {
    /// Ref to evaluate against (defaults to "HEAD").
    pub ref_name: String,
    /// Commits that are ancestors of (or equal to) this commit are excluded.
    pub since: Option<String>,
    /// Maximum commits to scan for subject lookup.
    pub max_commits: usize,
}

impl Default for ScopeOptions {
    fn default() -> Self {
        Self {
            ref_name: "HEAD".to_string(),
            since: None,
            max_commits: 200,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluationScope {
    pub unit: ScopeUnit,
    pub status: ScopeStatus,
    pub source: ScopeSource,
    pub commits: Vec<ScopeCommit>,
    pub changed_files: Vec<PathBuf>,
    pub files: Vec<PathBuf>,
    pub missing_on_disk: Vec<PathBuf>,
    pub outside_union: Vec<PathBuf>,
    pub unreachable: Vec<String>,
    pub range_base: Option<String>,
}

impl EvaluationScope {
    /// Resolves the authoritative scope for a phase, plan, or quick task.
    pub fn resolve(repo_root: &Path, unit: ScopeUnit, options: &ScopeOptions) -> Result<Self> {
        match &unit {
            ScopeUnit::Phase { phase, phase_dir } => {
                Self::resolve_phase(repo_root, phase, phase_dir.as_deref(), options)
            }
            ScopeUnit::Plan { plan_id } => Self::resolve_plan(repo_root, plan_id, options),
            ScopeUnit::Quick { id } => Self::resolve_quick(repo_root, id, options),
        }
    }

    /// Resolves scope for a full phase by extracting task commit SHAs from SUMMARY.md files.
    fn resolve_phase(
        repo_root: &Path,
        phase: &str,
        custom_phase_dir: Option<&Path>,
        options: &ScopeOptions,
    ) -> Result<Self> {
        let phase_dir = if let Some(dir) = custom_phase_dir {
            dir.to_path_buf()
        } else {
            Self::find_phase_dir(repo_root, phase)?
        };

        if !phase_dir.exists() {
            return Ok(Self {
                unit: ScopeUnit::Phase {
                    phase: phase.to_string(),
                    phase_dir: Some(phase_dir),
                },
                status: ScopeStatus::Unresolvable {
                    reason: format!("Phase directory does not exist for phase '{phase}'"),
                },
                source: ScopeSource::None,
                commits: Vec::new(),
                changed_files: Vec::new(),
                files: Vec::new(),
                missing_on_disk: Vec::new(),
                outside_union: Vec::new(),
                unreachable: Vec::new(),
                range_base: None,
            });
        }

        // 1. Parse all SUMMARY.md files in phase_dir for task commits
        let summary_commits = Self::extract_task_commits_from_summaries(&phase_dir)?;

        if summary_commits.is_empty() {
            // Degrade to phase directory commit range
            return Self::resolve_degraded_phase_range(
                repo_root,
                phase,
                phase_dir,
                options,
                "No task commits found in phase SUMMARY files",
            );
        }

        // 2. Validate reachability against target ref
        let mut reachable_commits = Vec::new();
        let mut unreachable = Vec::new();

        for sha in summary_commits {
            if Self::is_commit_reachable(repo_root, &sha, &options.ref_name) {
                let subject = Self::get_commit_subject(repo_root, &sha).unwrap_or_default();
                let files = Self::get_commit_files(repo_root, &sha).unwrap_or_default();
                reachable_commits.push(ScopeCommit {
                    sha,
                    subject,
                    files,
                });
            } else {
                unreachable.push(sha);
            }
        }

        if reachable_commits.is_empty() {
            return Self::resolve_degraded_phase_range(
                repo_root,
                phase,
                phase_dir,
                options,
                "Listed task commits are unreachable from evaluated ref",
            );
        }

        // 3. Compute union of changed files
        let mut union_files = BTreeSet::new();
        for commit in &reachable_commits {
            for f in &commit.files {
                union_files.insert(f.clone());
            }
        }

        // 4. Partition into existing vs missing on disk
        let (files, missing_on_disk) = Self::partition_disk_presence(repo_root, &union_files);

        // 5. Detect outside_union files touched in the same range
        let range_base =
            Self::get_phase_start_anchor(repo_root, &phase_dir).or_else(|| options.since.clone());
        let outside_union = if let Some(base) = &range_base {
            Self::get_files_in_range(repo_root, base, &options.ref_name)
                .map(|range_files| {
                    range_files
                        .into_iter()
                        .filter(|f| !union_files.contains(f))
                        .collect()
                })
                .unwrap_or_default()
        } else {
            Vec::new()
        };

        Ok(Self {
            unit: ScopeUnit::Phase {
                phase: phase.to_string(),
                phase_dir: Some(phase_dir),
            },
            status: ScopeStatus::Resolved,
            source: ScopeSource::TaskCommits,
            commits: reachable_commits,
            changed_files: union_files.into_iter().collect(),
            files,
            missing_on_disk,
            outside_union,
            unreachable,
            range_base,
        })
    }

    /// Resolves scope for a specific plan ID using commit subject matching.
    fn resolve_plan(repo_root: &Path, plan_id: &str, options: &ScopeOptions) -> Result<Self> {
        let pattern = Self::plan_subject_regex(plan_id)?;
        let commits = Self::find_commits_by_subject(
            repo_root,
            &pattern,
            &options.ref_name,
            options.since.as_deref(),
            options.max_commits,
        )?;

        if commits.is_empty() {
            return Ok(Self {
                unit: ScopeUnit::Plan {
                    plan_id: plan_id.to_string(),
                },
                status: ScopeStatus::Degraded {
                    reason: format!("No commits found matching plan subject for '{plan_id}'"),
                },
                source: ScopeSource::PlanSubjects,
                commits: Vec::new(),
                changed_files: Vec::new(),
                files: Vec::new(),
                missing_on_disk: Vec::new(),
                outside_union: Vec::new(),
                unreachable: Vec::new(),
                range_base: None,
            });
        }

        let mut union_files = BTreeSet::new();
        for commit in &commits {
            for f in &commit.files {
                union_files.insert(f.clone());
            }
        }

        let (files, missing_on_disk) = Self::partition_disk_presence(repo_root, &union_files);

        Ok(Self {
            unit: ScopeUnit::Plan {
                plan_id: plan_id.to_string(),
            },
            status: ScopeStatus::Resolved,
            source: ScopeSource::PlanSubjects,
            commits,
            changed_files: union_files.into_iter().collect(),
            files,
            missing_on_disk,
            outside_union: Vec::new(),
            unreachable: Vec::new(),
            range_base: options.since.clone(),
        })
    }

    /// Resolves scope for a quick task.
    fn resolve_quick(repo_root: &Path, quick_id: &str, options: &ScopeOptions) -> Result<Self> {
        let pattern = Regex::new(&format!(r"(?i)\bquick\({}\):", regex::escape(quick_id)))?;
        let commits = Self::find_commits_by_subject(
            repo_root,
            &pattern,
            &options.ref_name,
            options.since.as_deref(),
            options.max_commits,
        )?;

        let mut union_files = BTreeSet::new();
        for commit in &commits {
            for f in &commit.files {
                union_files.insert(f.clone());
            }
        }

        let (files, missing_on_disk) = Self::partition_disk_presence(repo_root, &union_files);

        Ok(Self {
            unit: ScopeUnit::Quick {
                id: quick_id.to_string(),
            },
            status: if commits.is_empty() {
                ScopeStatus::Degraded {
                    reason: format!("No commits found matching quick task '{quick_id}'"),
                }
            } else {
                ScopeStatus::Resolved
            },
            source: ScopeSource::QuickSubjects,
            commits,
            changed_files: union_files.into_iter().collect(),
            files,
            missing_on_disk,
            outside_union: Vec::new(),
            unreachable: Vec::new(),
            range_base: options.since.clone(),
        })
    }

    /// Degrades scope resolution to the phase directory commit range when task commits are absent.
    fn resolve_degraded_phase_range(
        repo_root: &Path,
        phase: &str,
        phase_dir: PathBuf,
        options: &ScopeOptions,
        reason: &str,
    ) -> Result<Self> {
        let range_base =
            Self::get_phase_start_anchor(repo_root, &phase_dir).or_else(|| options.since.clone());

        let range_files = if let Some(base) = &range_base {
            Self::get_files_in_range(repo_root, base, &options.ref_name).unwrap_or_default()
        } else {
            Vec::new()
        };

        let file_set: BTreeSet<PathBuf> = range_files.into_iter().collect();
        let (files, missing_on_disk) = Self::partition_disk_presence(repo_root, &file_set);

        Ok(Self {
            unit: ScopeUnit::Phase {
                phase: phase.to_string(),
                phase_dir: Some(phase_dir),
            },
            status: ScopeStatus::Degraded {
                reason: reason.to_string(),
            },
            source: ScopeSource::PhaseRange,
            commits: Vec::new(),
            changed_files: file_set.into_iter().collect(),
            files,
            missing_on_disk,
            outside_union: Vec::new(),
            unreachable: Vec::new(),
            range_base,
        })
    }

    // ─── Helpers ──────────────────────────────────────────────────────────────

    /// Partitions a set of paths into those existing on disk and those missing.
    pub fn partition_disk_presence(
        repo_root: &Path,
        paths: &BTreeSet<PathBuf>,
    ) -> (Vec<PathBuf>, Vec<PathBuf>) {
        let mut files = Vec::new();
        let mut missing = Vec::new();

        for p in paths {
            let full = repo_root.join(p);
            if full.exists() {
                files.push(p.clone());
            } else {
                missing.push(p.clone());
            }
        }

        (files, missing)
    }

    /// Scans a phase directory for all `*-SUMMARY.md` files and parses task commit SHAs.
    pub fn extract_task_commits_from_summaries(phase_dir: &Path) -> Result<Vec<String>> {
        let mut commits = Vec::new();
        if !phase_dir.is_dir() {
            return Ok(commits);
        }

        let sha_regex = Regex::new(r"\b([0-9a-f]{7,40})\b")?;

        for entry in std::fs::read_dir(phase_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file()
                && path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(|s| s.ends_with("-SUMMARY.md") || s.ends_with("_SUMMARY.md"))
                    .unwrap_or(false)
            {
                let content = std::fs::read_to_string(&path)?;
                let mut in_task_commits_section = false;

                for line in content.lines() {
                    let trimmed = line.trim();
                    if trimmed.starts_with("## Task Commits")
                        || trimmed.starts_with("### Task Commits")
                        || trimmed.starts_with("### Completed Tasks")
                    {
                        in_task_commits_section = true;
                        continue;
                    }

                    if in_task_commits_section && trimmed.starts_with('#') {
                        in_task_commits_section = false;
                        continue;
                    }

                    if in_task_commits_section && trimmed.starts_with('|') {
                        // Markdown table row
                        for cap in sha_regex.captures_iter(trimmed) {
                            let sha = cap[1].to_string();
                            if !commits.contains(&sha) {
                                commits.push(sha);
                            }
                        }
                    }
                }
            }
        }

        Ok(commits)
    }

    /// Generates subject pattern matching `<type>(<phase>-<plan>):` with zero-padding tolerance.
    fn plan_subject_regex(plan_id: &str) -> Result<Regex> {
        let parts: Vec<&str> = plan_id.split('-').collect();
        if parts.len() == 2 {
            let phase_num = parts[0].trim_start_matches('0');
            let plan_num = parts[1].trim_start_matches('0');
            let pattern = format!(
                r"(?i)^[a-z0-9_-]+\(0*{}-0*{}\):",
                regex::escape(if phase_num.is_empty() { "0" } else { phase_num }),
                regex::escape(if plan_num.is_empty() { "0" } else { plan_num })
            );
            Ok(Regex::new(&pattern)?)
        } else {
            let pattern = format!(r"(?i)^[a-z0-9_-]+\({}\):", regex::escape(plan_id));
            Ok(Regex::new(&pattern)?)
        }
    }

    /// Finds commits matching subject regex up to `max_commits`.
    fn find_commits_by_subject(
        repo_root: &Path,
        subject_regex: &Regex,
        ref_name: &str,
        since: Option<&str>,
        max_commits: usize,
    ) -> Result<Vec<ScopeCommit>> {
        let mut args = vec!["log".to_string(), "--format=%H|%s".to_string()];
        if let Some(s) = since {
            args.push(format!("{s}..{ref_name}"));
        } else {
            args.push(ref_name.to_string());
        }
        args.push(format!("-n{max_commits}"));

        let output = Command::new("git")
            .args(&args)
            .current_dir(repo_root)
            .output()
            .context("Failed executing git log")?;

        let text = String::from_utf8_lossy(&output.stdout);
        let mut results = Vec::new();

        for line in text.lines() {
            if let Some((sha, subject)) = line.split_once('|')
                && subject_regex.is_match(subject)
            {
                let files = Self::get_commit_files(repo_root, sha).unwrap_or_default();
                results.push(ScopeCommit {
                    sha: sha.to_string(),
                    subject: subject.to_string(),
                    files,
                });
            }
        }

        Ok(results)
    }

    /// Checks if a commit is an ancestor of target ref.
    fn is_commit_reachable(repo_root: &Path, sha: &str, ref_name: &str) -> bool {
        let status = Command::new("git")
            .args(["merge-base", "--is-ancestor", sha, ref_name])
            .current_dir(repo_root)
            .status();

        status.map(|s| s.success()).unwrap_or(false)
    }

    /// Fetches the subject line of a commit.
    fn get_commit_subject(repo_root: &Path, sha: &str) -> Result<String> {
        let output = Command::new("git")
            .args(["log", "-1", "--format=%s", sha])
            .current_dir(repo_root)
            .output()?;

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    /// Fetches files touched by a specific commit.
    fn get_commit_files(repo_root: &Path, sha: &str) -> Result<Vec<PathBuf>> {
        let output = Command::new("git")
            .args(["diff-tree", "--no-commit-id", "--name-only", "-r", sha])
            .current_dir(repo_root)
            .output()?;

        let mut files = Vec::new();
        for line in String::from_utf8_lossy(&output.stdout).lines() {
            let trimmed = line.trim();
            if !trimmed.is_empty() {
                files.push(PathBuf::from(trimmed));
            }
        }
        Ok(files)
    }

    /// Fetches files changed between two refs (`base..head`).
    fn get_files_in_range(repo_root: &Path, base: &str, head: &str) -> Result<Vec<PathBuf>> {
        let output = Command::new("git")
            .args(["diff", "--name-only", &format!("{base}..{head}")])
            .current_dir(repo_root)
            .output()?;

        let mut files = Vec::new();
        for line in String::from_utf8_lossy(&output.stdout).lines() {
            let trimmed = line.trim();
            if !trimmed.is_empty() {
                files.push(PathBuf::from(trimmed));
            }
        }
        Ok(files)
    }

    /// Obtains phase start anchor (commit where the phase dir was first introduced).
    fn get_phase_start_anchor(repo_root: &Path, phase_dir: &Path) -> Option<String> {
        let rel_path = phase_dir.strip_prefix(repo_root).ok()?;
        let output = Command::new("git")
            .args([
                "log",
                "--diff-filter=A",
                "--format=%H~1",
                "-1",
                "--",
                &rel_path.to_string_lossy(),
            ])
            .current_dir(repo_root)
            .output()
            .ok()?;

        let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if text.is_empty() { None } else { Some(text) }
    }

    /// Finds directory for phase inside `.planning/phases/`.
    fn find_phase_dir(repo_root: &Path, phase: &str) -> Result<PathBuf> {
        let phases_root = repo_root.join(".planning").join("phases");
        if !phases_root.exists() {
            return Ok(phases_root.join(phase));
        }

        let clean_phase = phase.trim_start_matches('0');
        for entry in std::fs::read_dir(&phases_root)? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().to_string();
            let prefix = name.split('-').next().unwrap_or("").trim_start_matches('0');
            if prefix == clean_phase || name == phase {
                return Ok(entry.path());
            }
        }

        Ok(phases_root.join(phase))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_extract_task_commits_from_summaries() {
        let dir = tempdir().unwrap();
        let summary_file = dir.path().join("01-01-SUMMARY.md");
        let content = r#"
# Phase 1 Summary

## Task Commits

| Task | Name | Commit | Status |
|------|------|--------|--------|
| 1 | Setup project | `a1b2c3d` | Done |
| 2 | Add models | e4f5a6b7c8d9e0f1a2b3c4d5e6f7a8b9c0d1e2f3 | Done |

## Notes
Some other details.
"#;
        std::fs::write(&summary_file, content).unwrap();

        let commits = EvaluationScope::extract_task_commits_from_summaries(dir.path()).unwrap();
        assert_eq!(commits.len(), 2);
        assert_eq!(commits[0], "a1b2c3d");
        assert_eq!(commits[1], "e4f5a6b7c8d9e0f1a2b3c4d5e6f7a8b9c0d1e2f3");
    }

    #[test]
    fn test_plan_subject_regex() {
        let regex = EvaluationScope::plan_subject_regex("01-02").unwrap();
        assert!(regex.is_match("feat(01-02): add login page"));
        assert!(regex.is_match("fix(1-2): correct validation"));
        assert!(regex.is_match("chore(001-002): cleanup imports"));
        assert!(!regex.is_match("feat(01-03): wrong plan"));
    }
}
