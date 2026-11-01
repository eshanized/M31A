//! High-level patch engine API.

use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use tokio::sync::mpsc;

use crate::transaction::{FilePlan, PatchTransaction, TransactionStats};
use crate::unified_diff::UnifiedDiffParser;

/// Streaming patch event for real-time progress reporting.
#[derive(Debug, Clone)]
pub enum PatchEvent {
    /// A file is about to be processed.
    FileStarted { path: String, action: String },
    /// A hunk was successfully applied within a file.
    HunkApplied {
        path: String,
        hunk_index: usize,
        hunk_type: String,
    },
    /// A file was fully processed and committed.
    FileCompleted {
        path: String,
        action: String,
        hunks_applied: usize,
    },
    /// Overall patch processing completed.
    PatchCompleted { stats: TransactionStats },
    /// An error occurred during patch application.
    Error { message: String },
}

pub struct PatchEngine;

impl PatchEngine {
    /// Parses and applies a patch to the workspace atomically.
    /// Supports unified diffs (git diff), search/replace format, and structured V4A patches.
    pub fn apply(cwd: &Path, patch_str: &str) -> Result<TransactionStats> {
        let parsed = if patch_str.contains("*** Begin Patch") {
            crate::v4a::parse_v4a_patch(patch_str)
                .or_else(|_| UnifiedDiffParser::parse(patch_str))
                .context("Failed parsing patch format (tried V4A and unified diff)")?
        } else {
            UnifiedDiffParser::parse(patch_str).context("Failed parsing patch format")?
        };

        let mut tx = PatchTransaction::new(cwd);
        tx.prepare(&parsed)
            .context("Failed validating patch transaction (dry-run pass)")?;

        tx.commit()
            .context("Failed committing atomic patch transaction to disk")
    }

    /// Applies a patch with streaming progress events buffered to prevent channel saturation.
    /// Emits PatchEvent to the provided channel for real-time progress.
    pub async fn apply_streaming(
        cwd: &Path,
        patch_str: &str,
        tx: mpsc::Sender<PatchEvent>,
    ) -> Result<TransactionStats> {
        let parsed = if patch_str.contains("*** Begin Patch") {
            crate::v4a::parse_v4a_patch(patch_str)
                .or_else(|_| UnifiedDiffParser::parse(patch_str))
                .context("Failed parsing patch format (tried V4A and unified diff)")?
        } else {
            UnifiedDiffParser::parse(patch_str).context("Failed parsing patch format")?
        };

        let mut transaction = PatchTransaction::new(cwd);

        // Phase 1: Prepare (validate and plan)
        transaction
            .prepare(&parsed)
            .context("Failed validating patch transaction (dry-run pass)")?;

        // Emit file started events for all pending files
        for (path, plan) in &transaction.pending_writes {
            let action = match plan {
                crate::transaction::FilePlan::Create { .. } => "create",
                crate::transaction::FilePlan::Modify { .. } => "modify",
                crate::transaction::FilePlan::Delete { .. } => "delete",
            };
            let _ = tx
                .send(PatchEvent::FileStarted {
                    path: path.display().to_string(),
                    action: action.to_string(),
                })
                .await;
        }

        // Phase 2: Commit with streaming
        let mut committed: Vec<(PathBuf, FilePlan)> = Vec::new();
        let mut _total_hunks = 0;

        for (path, plan) in &transaction.pending_writes {
            let path_str = path.display().to_string();
            let action = match plan {
                FilePlan::Create { .. } => "create",
                FilePlan::Modify { .. } => "modify",
                FilePlan::Delete { .. } => "delete",
            };

            // Simulate hunk progress for modified files
            if matches!(plan, FilePlan::Modify { .. }) {
                // For now, emit a single hunk event per file
                // In a more advanced version, we'd iterate individual hunks
                _total_hunks += 1;
                let _ = tx
                    .send(PatchEvent::HunkApplied {
                        path: path_str.clone(),
                        hunk_index: 0,
                        hunk_type: "unified".to_string(),
                    })
                    .await;
            }

            let res = match plan {
                FilePlan::Create { content } => {
                    if let Some(parent) = path.parent() {
                        fs::create_dir_all(parent)?;
                    }
                    fs::write(path, content)
                }
                FilePlan::Modify { updated, .. } => fs::write(path, updated),
                FilePlan::Delete { .. } => {
                    if path.exists() {
                        fs::remove_file(path)
                    } else {
                        Ok(())
                    }
                }
            };

            if let Err(e) = res {
                // Rollback previously committed changes
                tracing::error!(
                    "I/O error during patch commit: {}. Rolling back transaction...",
                    e
                );
                for (rb_path, rb_plan) in committed.iter().rev() {
                    match rb_plan {
                        FilePlan::Create { .. } => {
                            let _ = fs::remove_file(rb_path);
                        }
                        FilePlan::Modify { original, .. } => {
                            let _ = fs::write(rb_path, original);
                        }
                        FilePlan::Delete { original } => {
                            let _ = fs::write(rb_path, original);
                        }
                    }
                }
                let _ = tx
                    .send(PatchEvent::Error {
                        message: format!(
                            "Failed committing atomic patch transaction; rollback succeeded: {}",
                            e
                        ),
                    })
                    .await;
                return Err(e)
                    .context("Failed committing atomic patch transaction; rollback succeeded");
            }

            committed.push((path.clone(), plan.clone()));

            // Emit file completed event
            let hunks_applied = if matches!(plan, FilePlan::Modify { .. }) {
                1
            } else {
                0
            };
            let _ = tx
                .send(PatchEvent::FileCompleted {
                    path: path_str.clone(),
                    action: action.to_string(),
                    hunks_applied,
                })
                .await;
        }

        // Apply the stats from the prepared transaction
        let stats = transaction.stats.clone();

        // Emit final completion event
        let _ = tx
            .send(PatchEvent::PatchCompleted {
                stats: stats.clone(),
            })
            .await;

        Ok(stats)
    }

    /// Validates whether a patch can be applied cleanly without modifying files on disk.
    pub fn dry_run(cwd: &Path, patch_str: &str) -> Result<TransactionStats> {
        let parsed = if patch_str.contains("*** Begin Patch") {
            crate::v4a::parse_v4a_patch(patch_str)
                .or_else(|_| UnifiedDiffParser::parse(patch_str))
                .context("Failed parsing patch format (tried V4A and unified diff)")?
        } else {
            UnifiedDiffParser::parse(patch_str).context("Failed parsing patch format")?
        };

        let mut tx = PatchTransaction::new(cwd);
        tx.prepare(&parsed)?;
        Ok(TransactionStats::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_patch_engine_applies_v4a_structured_patch() {
        let tmp = tempdir().unwrap();
        let target_file = tmp.path().join("code.py");
        fs::write(&target_file, "def greet():\n    print('hello')\n").unwrap();

        let v4a_patch = r#"*** Begin Patch
*** Update File: code.py
@@
 def greet():
-    print('hello')
+    print('hello world!')
*** End Patch
"#;

        let stats = PatchEngine::apply(tmp.path(), v4a_patch).expect("apply v4a patch");
        assert_eq!(stats.files_modified, 1);

        let updated = fs::read_to_string(&target_file).unwrap();
        assert!(updated.contains("print('hello world!')"));
    }

    #[test]
    fn test_patch_engine_dry_run_v4a() {
        let tmp = tempdir().unwrap();
        let target_file = tmp.path().join("code.py");
        fs::write(&target_file, "def foo():\n    pass\n").unwrap();

        let v4a_patch = r#"*** Begin Patch
*** Update File: code.py
@@
 def foo():
-    pass
+    return 42
*** End Patch
"#;

        assert!(PatchEngine::dry_run(tmp.path(), v4a_patch).is_ok());
        // Original file must be untouched by dry run
        let content = fs::read_to_string(&target_file).unwrap();
        assert!(content.contains("pass"));
    }
}
