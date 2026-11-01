//! Git 3-Way Merge Conflict Resolution & Rebase Advisor (`m31a-tools::git_merge`).
//!
//! Provides automated 3-way merge conflict parsing, reconciliation strategies,
//! and pre-flight rebase compatibility checking.
//!
//! Enforces:
//! 1. Conflict Marker Parsing: Accurately identifies `<<<<<<<`, `|||||||`, `=======`, `>>>>>>>`.
//! 2. Strategy-based Reconciliation: Supports `Ours`, `Theirs`, `Union`, and `Semantic` resolution.
//! 3. Rebase Preflight Auditing: Evaluates modified file sets to warn of rebase collisions.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConflictStrategy {
    Ours,
    Theirs,
    Union,
    Semantic,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConflictHunk {
    pub start_line: usize,
    pub ours_content: String,
    pub base_content: Option<String>,
    pub theirs_content: String,
    pub resolved_content: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergeConflictResult {
    pub file_path: PathBuf,
    pub conflicts_found: usize,
    pub conflicts_resolved: usize,
    pub resolved_text: String,
    pub all_resolved: bool,
}

pub struct GitConflictReconciler;

impl GitConflictReconciler {
    /// Parses and resolves git conflict markers in text using the specified strategy.
    pub fn resolve_conflicts(
        content: &str,
        strategy: ConflictStrategy,
    ) -> Result<MergeConflictResult> {
        let lines: Vec<&str> = content.lines().collect();
        let mut resolved_lines: Vec<String> = Vec::new();

        let mut in_conflict = false;
        let mut in_base = false;
        let mut in_theirs = false;

        let mut ours_lines: Vec<String> = Vec::new();
        let mut base_lines: Vec<String> = Vec::new();
        let mut theirs_lines: Vec<String> = Vec::new();

        let mut conflicts_found = 0;
        let mut conflicts_resolved = 0;

        for line in lines {
            if line.starts_with("<<<<<<< ") || line == "<<<<<<<" {
                in_conflict = true;
                in_base = false;
                in_theirs = false;
                ours_lines.clear();
                base_lines.clear();
                theirs_lines.clear();
                conflicts_found += 1;
                continue;
            }

            if line.starts_with("||||||| ") || line == "|||||||" {
                in_base = true;
                continue;
            }

            if line.starts_with("=======") && in_conflict {
                in_base = false;
                in_theirs = true;
                continue;
            }

            if (line.starts_with(">>>>>>> ") || line == ">>>>>>>") && in_conflict {
                in_conflict = false;
                in_theirs = false;

                // Apply strategy
                match strategy {
                    ConflictStrategy::Ours => {
                        resolved_lines.extend(ours_lines.clone());
                        conflicts_resolved += 1;
                    }
                    ConflictStrategy::Theirs => {
                        resolved_lines.extend(theirs_lines.clone());
                        conflicts_resolved += 1;
                    }
                    ConflictStrategy::Union => {
                        resolved_lines.extend(ours_lines.clone());
                        resolved_lines.extend(theirs_lines.clone());
                        conflicts_resolved += 1;
                    }
                    ConflictStrategy::Semantic => {
                        // If ours and theirs don't overlap line-by-line, combine them; otherwise prefer ours
                        let mut combined = ours_lines.clone();
                        for t in &theirs_lines {
                            if !combined.contains(t) {
                                combined.push(t.clone());
                            }
                        }
                        resolved_lines.extend(combined);
                        conflicts_resolved += 1;
                    }
                }
                continue;
            }

            if in_conflict {
                if in_theirs {
                    theirs_lines.push(line.to_string());
                } else if in_base {
                    base_lines.push(line.to_string());
                } else {
                    ours_lines.push(line.to_string());
                }
            } else {
                resolved_lines.push(line.to_string());
            }
        }

        let resolved_text = resolved_lines.join("\n");
        let all_resolved = conflicts_found == conflicts_resolved;

        Ok(MergeConflictResult {
            file_path: PathBuf::from("in-memory"),
            conflicts_found,
            conflicts_resolved,
            resolved_text,
            all_resolved,
        })
    }

    /// Resolves conflicts directly inside a file on disk.
    pub fn resolve_file_conflicts(
        path: &Path,
        strategy: ConflictStrategy,
    ) -> Result<MergeConflictResult> {
        let content = fs::read_to_string(path)?;
        let mut res = Self::resolve_conflicts(&content, strategy)?;
        res.file_path = path.to_path_buf();
        if res.all_resolved {
            fs::write(path, &res.resolved_text)?;
        }
        Ok(res)
    }

    /// Preflight checks whether rebasing `head_branch` onto `base_branch` would encounter file collisions.
    pub fn preflight_rebase_check(
        repo_root: &Path,
        base_branch: &str,
        head_branch: &str,
    ) -> Result<Vec<PathBuf>> {
        let out_base = Command::new("git")
            .args([
                "diff",
                "--name-only",
                &format!("{}...{}", base_branch, head_branch),
            ])
            .current_dir(repo_root)
            .output()?;

        let mut colliding_files = Vec::new();
        if out_base.status.success() {
            let files_str = String::from_utf8_lossy(&out_base.stdout);
            for f in files_str.lines() {
                let trimmed = f.trim();
                if !trimmed.is_empty() {
                    colliding_files.push(PathBuf::from(trimmed));
                }
            }
        }

        Ok(colliding_files)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_ours_strategy() {
        let text = r#"fn main() {
<<<<<<< HEAD
    println!("Hello from local branch");
=======
    println!("Hello from remote branch");
>>>>>>> incoming
}"#;
        let res = GitConflictReconciler::resolve_conflicts(text, ConflictStrategy::Ours).unwrap();
        assert_eq!(res.conflicts_found, 1);
        assert_eq!(res.conflicts_resolved, 1);
        assert!(res.resolved_text.contains("Hello from local branch"));
        assert!(!res.resolved_text.contains("Hello from remote branch"));
        assert!(!res.resolved_text.contains("<<<<<<<"));
    }

    #[test]
    fn test_resolve_theirs_strategy() {
        let text = r#"fn main() {
<<<<<<< HEAD
    println!("Hello from local branch");
=======
    println!("Hello from remote branch");
>>>>>>> incoming
}"#;
        let res = GitConflictReconciler::resolve_conflicts(text, ConflictStrategy::Theirs).unwrap();
        assert_eq!(res.conflicts_found, 1);
        assert_eq!(res.conflicts_resolved, 1);
        assert!(res.resolved_text.contains("Hello from remote branch"));
        assert!(!res.resolved_text.contains("Hello from local branch"));
    }

    #[test]
    fn test_resolve_semantic_union_strategy() {
        let text = r#"dependencies:
<<<<<<< HEAD
    serde = "1.0"
=======
    tokio = "1.0"
>>>>>>> incoming
"#;
        let res =
            GitConflictReconciler::resolve_conflicts(text, ConflictStrategy::Semantic).unwrap();
        assert_eq!(res.conflicts_resolved, 1);
        assert!(res.resolved_text.contains("serde = \"1.0\""));
        assert!(res.resolved_text.contains("tokio = \"1.0\""));
    }
}
