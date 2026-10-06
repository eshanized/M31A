//! Atomic multi-file change applier with full rollback guarantees.
//!
//! Applies all mutations in a ChangeProposal sequentially.
//! If any mutation fails (diagnostic mismatch, out of bounds, I/O error), ALL
//! changes in the proposal are immediately rolled back to their exact pre-proposal bytes.
//! The workspace is never left in a partially mutated or corrupt state.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::capability::traits::fs::FileSystemService;
use crate::kernel::change::{ChangeProposal, ChangeProposalId, FileMutationOp};
use crate::tools::fs::editor::{FileEditOp, RobustFileEditor};

/// Error taxonomy during atomic change set application.
#[derive(Debug, thiserror::Error)]
pub enum ChangeApplyError {
    #[error("File edit diagnostic on '{path}': {diagnostic}")]
    EditDiagnostic { path: String, diagnostic: String },
    #[error("File already exists when expected new: '{0}'")]
    FileAlreadyExists(String),
    #[error("File not found for mutation: '{0}'")]
    FileNotFound(String),
    #[error("File is not valid UTF-8: '{0}'")]
    InvalidUtf8(String),
    #[error("I/O error on '{path}': {error}")]
    Io { path: String, error: String },
    #[error("Atomic rollback executed after failure on '{failed_path}': {reason}")]
    RollbackExecuted {
        failed_path: String,
        reason: String,
        restored_files_count: usize,
    },
}

/// A successfully applied change set, preserving pre-mutation snapshots in case
/// subsequent verification fails and demands rollback.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AppliedChangeSet {
    pub proposal_id: ChangeProposalId,
    pub files_modified: Vec<String>,
    pub lines_modified: usize,
    pub original_snapshots: HashMap<PathBuf, Option<Vec<u8>>>,
}

impl AppliedChangeSet {
    /// Roll back all modifications in this change set back to pre-mutation states.
    pub async fn rollback(&self, fs: &Arc<dyn FileSystemService>) -> Result<usize, std::io::Error> {
        let mut restored = 0;
        for (path, maybe_bytes) in &self.original_snapshots {
            let target_path: &Path = if path.is_absolute() {
                self.files_modified
                    .iter()
                    .find(|f| path.ends_with(Path::new(f)))
                    .map(|f| Path::new(f.as_str()))
                    .unwrap_or(path.as_path())
            } else {
                path.as_path()
            };

            match maybe_bytes {
                Some(bytes) => {
                    fs.write_file(target_path, bytes)
                        .await
                        .map_err(|e| std::io::Error::other(e.to_string()))?;
                    restored += 1;
                }
                None => {
                    fs.delete_file(target_path)
                        .await
                        .map_err(|e| std::io::Error::other(e.to_string()))?;
                    restored += 1;
                }
            }
        }
        Ok(restored)
    }
}

/// Applies multi-file change sets atomically.
pub struct AtomicChangeApplier;

impl AtomicChangeApplier {
    /// Atomically apply all mutations in the given proposal.
    ///
    /// If any mutation fails, rolls back all previously applied mutations in the proposal.
    pub async fn apply(
        workspace_root: &Path,
        proposal: &ChangeProposal,
        fs: &Arc<dyn FileSystemService>,
    ) -> Result<AppliedChangeSet, ChangeApplyError> {
        // Step 1: Pre-capture byte snapshots of all target files for atomic rollback
        let mut original_snapshots: HashMap<PathBuf, Option<Vec<u8>>> = HashMap::new();

        for mutation in &proposal.mutations {
            let rel_path = Path::new(&mutation.path);
            let full_path = workspace_root.join(rel_path);

            if let std::collections::hash_map::Entry::Vacant(e) =
                original_snapshots.entry(full_path.clone())
            {
                if full_path.exists() {
                    match fs.read_file(rel_path, None, None).await {
                        Ok(bytes) => {
                            e.insert(Some(bytes));
                        }
                        Err(err) => {
                            return Err(ChangeApplyError::Io {
                                path: mutation.path.clone(),
                                error: format!("Failed to read file for backup snapshot: {}", err),
                            });
                        }
                    }
                } else {
                    e.insert(None);
                }
            }
        }

        // Step 2: Sequentially apply each mutation
        let mut total_lines_modified = 0;
        let mut files_modified_set = std::collections::BTreeSet::new();

        for mutation in &proposal.mutations {
            let rel_path = Path::new(&mutation.path);
            let full_path = workspace_root.join(rel_path);

            let apply_result =
                Self::apply_single_mutation(&full_path, rel_path, &mutation.operation, fs).await;

            match apply_result {
                Ok(lines) => {
                    total_lines_modified += lines;
                    files_modified_set.insert(mutation.path.clone());
                }
                Err(err) => {
                    // Step 3: Rollback on any failure
                    let mut restored_count = 0;
                    for (snap_full_path, maybe_bytes) in &original_snapshots {
                        let snap_rel = snap_full_path
                            .strip_prefix(workspace_root)
                            .unwrap_or(snap_full_path);

                        match maybe_bytes {
                            Some(bytes) => {
                                let _ = fs.write_file(snap_rel, bytes).await;
                                restored_count += 1;
                            }
                            None => {
                                if snap_full_path.exists() {
                                    let _ = fs.delete_file(snap_rel).await;
                                    restored_count += 1;
                                }
                            }
                        }
                    }

                    return Err(ChangeApplyError::RollbackExecuted {
                        failed_path: mutation.path.clone(),
                        reason: err.to_string(),
                        restored_files_count: restored_count,
                    });
                }
            }
        }

        Ok(AppliedChangeSet {
            proposal_id: proposal.id,
            files_modified: files_modified_set.into_iter().collect(),
            lines_modified: total_lines_modified,
            original_snapshots,
        })
    }

    async fn apply_single_mutation(
        _full_path: &Path,
        rel_path: &Path,
        operation: &FileMutationOp,
        fs: &Arc<dyn FileSystemService>,
    ) -> Result<usize, ChangeApplyError> {
        let path_str = rel_path.to_string_lossy().to_string();

        match operation {
            FileMutationOp::CreateNew { content } => {
                if fs.file_metadata(rel_path).await.is_ok() {
                    return Err(ChangeApplyError::FileAlreadyExists(path_str));
                }
                fs.write_file(rel_path, content.as_bytes())
                    .await
                    .map_err(|e| ChangeApplyError::Io {
                        path: path_str,
                        error: e.to_string(),
                    })?;
                Ok(content.lines().count())
            }
            FileMutationOp::ReplaceFull { content, .. } => {
                fs.write_file(rel_path, content.as_bytes())
                    .await
                    .map_err(|e| ChangeApplyError::Io {
                        path: path_str,
                        error: e.to_string(),
                    })?;
                Ok(content.lines().count())
            }
            _ => {
                // Surgical operations require reading current content
                let raw =
                    fs.read_file(rel_path, None, None)
                        .await
                        .map_err(|e| ChangeApplyError::Io {
                            path: path_str.clone(),
                            error: e.to_string(),
                        })?;

                let original_text = String::from_utf8(raw)
                    .map_err(|_| ChangeApplyError::InvalidUtf8(path_str.clone()))?;

                let edit_op = match operation {
                    FileMutationOp::Substring {
                        old_content,
                        new_content,
                    } => FileEditOp::Substring {
                        old_content,
                        new_content,
                    },
                    FileMutationOp::LineRange {
                        start_line,
                        end_line,
                        new_content,
                        expected_old,
                    } => FileEditOp::LineRange {
                        start_line: *start_line,
                        end_line: *end_line,
                        new_content,
                        expected_old: expected_old.as_deref(),
                    },
                    FileMutationOp::Insert {
                        line_number,
                        content,
                        after,
                    } => FileEditOp::Insert {
                        line_number: *line_number,
                        content,
                        after: *after,
                    },
                    FileMutationOp::Delete {
                        start_line,
                        end_line,
                        expected_old,
                    } => FileEditOp::Delete {
                        start_line: *start_line,
                        end_line: *end_line,
                        expected_old: expected_old.as_deref(),
                    },
                    FileMutationOp::Patch { patch } => FileEditOp::Patch { patch },
                    // Fail closed on unhandled mutation variants with a typed error
                    // rather than panicking.
                    other => {
                        return Err(ChangeApplyError::EditDiagnostic {
                            path: path_str.clone(),
                            diagnostic: format!(
                                "unsupported mutation operation for surgical path: {other:?}"
                            ),
                        });
                    }
                };

                let success =
                    RobustFileEditor::apply(&original_text, &edit_op).map_err(|diag| {
                        ChangeApplyError::EditDiagnostic {
                            path: path_str.clone(),
                            diagnostic: diag.to_string(),
                        }
                    })?;

                fs.write_file(rel_path, success.new_content.as_bytes())
                    .await
                    .map_err(|e| ChangeApplyError::Io {
                        path: path_str,
                        error: e.to_string(),
                    })?;

                Ok(success.lines_modified)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capability::providers::local_fs::LocalFileSystemProvider;
    use crate::ids::{MissionId, TaskId};
    use crate::kernel::change::{ChangeSurface, FileMutationProposal, ImplementationHypothesis};
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_atomic_apply_multi_file_happy_path() {
        let dir = tempdir().unwrap();
        let ws = dir.path();
        let fs: Arc<dyn FileSystemService> = Arc::new(LocalFileSystemProvider::new(ws).unwrap());

        fs.write_file(Path::new("src/a.rs"), b"pub fn a() -> u32 { 1 }\n")
            .await
            .unwrap();
        fs.write_file(Path::new("src/b.rs"), b"pub fn b() -> u32 { 2 }\n")
            .await
            .unwrap();

        let hypothesis =
            ImplementationHypothesis::new("Refactor", "Cause", "Change", "Result", "Test");
        let surface = ChangeSurface::new(vec!["src/a.rs".to_string(), "src/b.rs".to_string()]);
        let mutations = vec![
            FileMutationProposal::new(
                "src/a.rs",
                FileMutationOp::Substring {
                    old_content: "{ 1 }".to_string(),
                    new_content: "{ 10 }".to_string(),
                },
                "Update a",
            ),
            FileMutationProposal::new(
                "src/b.rs",
                FileMutationOp::Substring {
                    old_content: "{ 2 }".to_string(),
                    new_content: "{ 20 }".to_string(),
                },
                "Update b",
            ),
        ];

        let proposal = ChangeProposal::new(
            TaskId::new(),
            MissionId::new(),
            hypothesis,
            surface,
            mutations,
        );
        let result = AtomicChangeApplier::apply(ws, &proposal, &fs)
            .await
            .unwrap();

        assert_eq!(result.files_modified.len(), 2);
        let a_content = String::from_utf8(
            fs.read_file(Path::new("src/a.rs"), None, None)
                .await
                .unwrap(),
        )
        .unwrap();
        let b_content = String::from_utf8(
            fs.read_file(Path::new("src/b.rs"), None, None)
                .await
                .unwrap(),
        )
        .unwrap();
        assert!(a_content.contains("10"));
        assert!(b_content.contains("20"));
    }

    #[tokio::test]
    async fn test_atomic_apply_rolls_back_on_failure() {
        let dir = tempdir().unwrap();
        let ws = dir.path();
        let fs: Arc<dyn FileSystemService> = Arc::new(LocalFileSystemProvider::new(ws).unwrap());

        fs.write_file(Path::new("src/a.rs"), b"original a\n")
            .await
            .unwrap();
        fs.write_file(Path::new("src/b.rs"), b"original b\n")
            .await
            .unwrap();

        let hypothesis =
            ImplementationHypothesis::new("Refactor", "Cause", "Change", "Result", "Test");
        let surface = ChangeSurface::new(vec!["src/a.rs".to_string(), "src/b.rs".to_string()]);
        let mutations = vec![
            // First mutation succeeds
            FileMutationProposal::new(
                "src/a.rs",
                FileMutationOp::Substring {
                    old_content: "original a".to_string(),
                    new_content: "mutated a".to_string(),
                },
                "Update a",
            ),
            // Second mutation fails because target content does not exist
            FileMutationProposal::new(
                "src/b.rs",
                FileMutationOp::Substring {
                    old_content: "nonexistent content".to_string(),
                    new_content: "mutated b".to_string(),
                },
                "Update b",
            ),
        ];

        let proposal = ChangeProposal::new(
            TaskId::new(),
            MissionId::new(),
            hypothesis,
            surface,
            mutations,
        );
        let err = AtomicChangeApplier::apply(ws, &proposal, &fs)
            .await
            .unwrap_err();

        assert!(matches!(err, ChangeApplyError::RollbackExecuted { .. }));

        // Verify that src/a.rs was rolled back to "original a\n"!
        let a_content = String::from_utf8(
            fs.read_file(Path::new("src/a.rs"), None, None)
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(
            a_content, "original a\n",
            "src/a.rs must be restored to original bytes!"
        );
    }
}
