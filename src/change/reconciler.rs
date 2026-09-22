//! Pre-mutation repository reconciler.
//!
//! Validates that a proposed change still corresponds to current repository state:
//! - File hashes match expected base states (stale patch detection)
//! - Required files and expected symbols exist
//! - Direct mutation of generated files is rejected
//! - Protected runtime/repo paths (.git, .m31a) are blocked
//! - Committed migrations/schemas cannot be altered
//! - Competing concurrent task modifications on the same files are blocked
//! - Pre-change impact and recommended tests are calculated via BoundedQueryEngine

use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::capability::providers::LocalFileSystemProvider;
use crate::ids::TaskId;
use crate::kernel::change::{
    ChangeProposal, FileMutationOp, ReconciliationReport, ReconciliationViolation,
};
use crate::kernel::invariants::contains_protected_component;
use crate::repo::drift::RepositoryBaseline;
use crate::repo::query::{BoundedQueryEngine, QueryBounds};

/// Engine for reconciling change proposals against repository state before mutation.
pub struct PreMutationReconciler;

impl PreMutationReconciler {
    /// Reconcile a change proposal against workspace files and repository intelligence.
    pub fn reconcile(
        workspace_root: &Path,
        proposal: &ChangeProposal,
        _baseline: Option<&RepositoryBaseline>,
        query_engine: Option<&BoundedQueryEngine>,
        active_locks: &HashSet<String>,
        active_task_id: Option<TaskId>,
    ) -> ReconciliationReport {
        let mut violations = Vec::new();
        let target_files = proposal.change_surface.all_target_files();

        // 1. Check scope: every mutation must belong to change_surface
        for mutation in &proposal.mutations {
            if !proposal.change_surface.contains_path(&mutation.path) {
                violations.push(ReconciliationViolation::ScopeViolation {
                    path: mutation.path.clone(),
                    reason: format!(
                        "Mutation targets '{}' which is not declared in proposal ChangeSurface",
                        mutation.path
                    ),
                });
            }
        }

        // 2. Check protected paths
        for path_str in &target_files {
            let path = Path::new(path_str);
            if contains_protected_component(path) {
                violations.push(ReconciliationViolation::ProtectedFileAccess {
                    path: path_str.clone(),
                    reason: "Access to protected repository or runtime directory (.git, .m31a) is forbidden".to_string(),
                });
            }
        }

        // 2b. Canonical containment for every model-controlled path. Lexical
        // checks above cannot detect `..` traversal above root, absolute path
        // escapes, or symlink aliases; resolve through the canonical filesystem
        // guard and fail closed with a violation. All reads below use these
        // resolved paths — never a raw `join`.
        let mut resolved: HashMap<String, PathBuf> = HashMap::new();
        match LocalFileSystemProvider::new(workspace_root) {
            Ok(guard) => {
                let mut resolve_one = |rel: &str| {
                    if resolved.contains_key(rel) {
                        return;
                    }
                    match guard.resolve_and_verify(Path::new(rel)) {
                        Ok(full) => {
                            resolved.insert(rel.to_string(), full);
                        }
                        Err(e) => {
                            violations.push(ReconciliationViolation::ProtectedFileAccess {
                                path: rel.to_string(),
                                reason: format!(
                                    "Path failed canonical containment validation: {e}"
                                ),
                            });
                        }
                    }
                };
                for rel in &target_files {
                    resolve_one(rel);
                }
                for pre in &proposal.preconditions {
                    resolve_one(&pre.path);
                }
                for mutation in &proposal.mutations {
                    resolve_one(&mutation.path);
                }
            }
            Err(e) => {
                violations.push(ReconciliationViolation::PolicyBlocked {
                    reason: format!("Workspace root is not a valid containment base: {e}"),
                });
            }
        }

        // 3. Check concurrent task locks
        for path_str in &target_files {
            let clean_path = path_str
                .trim()
                .trim_start_matches('@')
                .trim_start_matches("./");
            if active_locks.contains(clean_path) {
                violations.push(ReconciliationViolation::ConcurrentTaskConflict {
                    path: clean_path.to_string(),
                    conflicting_task_id: active_task_id.unwrap_or_default(),
                });
            }
        }

        // 4. Validate file preconditions
        for pre in &proposal.preconditions {
            // Read only through the contained resolution. Paths that failed
            // containment already carry a violation; skip them here.
            let Some(file_path) = resolved.get(&pre.path) else {
                continue;
            };
            let exists = file_path.exists();

            if pre.must_exist && !exists {
                violations.push(ReconciliationViolation::MissingFile {
                    path: pre.path.clone(),
                });
                continue;
            }

            if !pre.must_exist && exists {
                violations.push(ReconciliationViolation::UnexpectedFileExists {
                    path: pre.path.clone(),
                });
                continue;
            }

            if exists {
                // Hash verification
                if let Some(ref expected_hash) = pre.expected_hash {
                    match std::fs::read(file_path) {
                        Ok(bytes) => {
                            let mut hasher = Sha256::new();
                            hasher.update(&bytes);
                            let actual_hash = format!("{:x}", hasher.finalize());
                            if actual_hash != *expected_hash {
                                violations.push(ReconciliationViolation::StaleFileHash {
                                    path: pre.path.clone(),
                                    expected: expected_hash.clone(),
                                    actual: actual_hash,
                                });
                            }
                        }
                        Err(e) => {
                            violations.push(ReconciliationViolation::PolicyBlocked {
                                reason: format!("Failed to read '{}': {}", pre.path, e),
                            });
                        }
                    }
                }

                // Expected symbols verification
                if !pre.expected_symbols.is_empty() {
                    if let Some(qe) = query_engine {
                        for sym in &pre.expected_symbols {
                            let found = qe.find_symbols(sym, QueryBounds::default());
                            let matches_file = found.data.iter().any(|s| {
                                s.file_path.ends_with(&pre.path) || pre.path.ends_with(&s.file_path)
                            });
                            if !matches_file {
                                violations.push(ReconciliationViolation::MissingSymbol {
                                    symbol: sym.clone(),
                                    path: pre.path.clone(),
                                });
                            }
                        }
                    } else if let Ok(content) = std::fs::read_to_string(file_path) {
                        for sym in &pre.expected_symbols {
                            if !content.contains(sym) {
                                violations.push(ReconciliationViolation::MissingSymbol {
                                    symbol: sym.clone(),
                                    path: pre.path.clone(),
                                });
                            }
                        }
                    }
                }
            }
        }

        // 5. Check mutations against current file state and base hash expectations
        for mutation in &proposal.mutations {
            // Read only through the contained resolution.
            let Some(file_path) = resolved.get(&mutation.path) else {
                continue;
            };
            let exists = file_path.exists();

            // Missing file check for modifications on non-existent files
            if !exists && !matches!(mutation.operation, FileMutationOp::CreateNew { .. }) {
                violations.push(ReconciliationViolation::MissingFile {
                    path: mutation.path.clone(),
                });
                continue;
            }

            // Unexpected existing file check for CreateNew
            if exists && matches!(mutation.operation, FileMutationOp::CreateNew { .. }) {
                violations.push(ReconciliationViolation::UnexpectedFileExists {
                    path: mutation.path.clone(),
                });
                continue;
            }

            // Check if file is generated
            if exists
                && let Ok(content) = std::fs::read_to_string(file_path)
                && let Some(generator) = detect_generated_file(&content, &mutation.path)
            {
                violations.push(ReconciliationViolation::GeneratedFileDirectEdit {
                    path: mutation.path.clone(),
                    generator: Some(generator),
                });
            }

            // Check committed migration alteration protection
            let clean_norm = mutation.path.replace('\\', "/");
            if clean_norm.starts_with("migrations/") && exists {
                // Existing migration files are immutable; only new migrations may be added.
                if !matches!(mutation.operation, FileMutationOp::CreateNew { .. }) {
                    violations.push(ReconciliationViolation::ProtectedFileAccess {
                        path: mutation.path.clone(),
                        reason: "Committed database migrations are immutable. You may only add new migration scripts.".to_string(),
                    });
                }
            }

            // Check base hash on mutation if specified
            if let Some(ref expected_base_hash) = mutation.base_hash
                && exists
                && let Ok(bytes) = std::fs::read(file_path)
            {
                let mut hasher = Sha256::new();
                hasher.update(&bytes);
                let actual_hash = format!("{:x}", hasher.finalize());
                if actual_hash != *expected_base_hash {
                    violations.push(ReconciliationViolation::StaleFileHash {
                        path: mutation.path.clone(),
                        expected: expected_base_hash.clone(),
                        actual: actual_hash,
                    });
                }
            }

            // Check ReplaceFull base hash requirement
            if let FileMutationOp::ReplaceFull {
                ref expected_base_hash,
                ..
            } = mutation.operation
                && exists
            {
                if let Some(expected) = expected_base_hash {
                    if let Ok(bytes) = std::fs::read(file_path) {
                        let mut hasher = Sha256::new();
                        hasher.update(&bytes);
                        let actual_hash = format!("{:x}", hasher.finalize());
                        if actual_hash != *expected {
                            violations.push(ReconciliationViolation::StaleFileHash {
                                path: mutation.path.clone(),
                                expected: expected.clone(),
                                actual: actual_hash,
                            });
                        }
                    }
                } else {
                    violations.push(ReconciliationViolation::ScopeViolation {
                        path: mutation.path.clone(),
                        reason: "Whole-file replacement requires an expected_base_hash to prevent accidental overwrites of drifted files.".to_string(),
                    });
                }
            }
        }

        if !violations.is_empty() {
            return ReconciliationReport::invalid(violations);
        }

        // 6. Pre-change impact analysis via BoundedQueryEngine
        let (impact, recommended_tests) = if let Some(qe) = query_engine {
            let imp = qe.calculate_change_impact(&target_files, QueryBounds::default());
            let tests = imp.data.recommended_tests.clone();
            (Some(imp.data), tests)
        } else {
            (None, Vec::new())
        };

        ReconciliationReport::valid(impact, recommended_tests)
    }
}

/// Detect whether a file's content or path indicates it is machine-generated.
pub fn detect_generated_file(content: &str, path: &str) -> Option<String> {
    let lower_path = path.to_lowercase();
    if lower_path.ends_with(".pb.rs") || lower_path.ends_with(".capnp.rs") {
        return Some("protobuf/rpc generator".to_string());
    }

    // Inspect first 25 lines
    for line in content.lines().take(25) {
        let trimmed = line.trim().to_lowercase();
        if trimmed.contains("@generated")
            || trimmed.contains("code generated by")
            || trimmed.contains("autogenerated")
            || trimmed.contains("this file is generated")
            || trimmed.contains("do not edit")
            || trimmed.contains("auto-generated")
        {
            return Some(line.trim().to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::change::{ChangeSurface, FileMutationProposal, ImplementationHypothesis};
    use tempfile::tempdir;

    #[test]
    fn test_reconcile_clean_success() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("src/lib.rs");
        std::fs::create_dir_all(file_path.parent().unwrap()).unwrap();
        std::fs::write(&file_path, "pub fn hello() {}\n").unwrap();

        let hypothesis = ImplementationHypothesis::new(
            "Fix bug",
            "Missing arg",
            "Add arg to hello",
            "hello takes arg",
            "cargo test",
        );
        let surface = ChangeSurface::new(vec!["src/lib.rs".to_string()]);
        let mutation = FileMutationProposal::new(
            "src/lib.rs",
            FileMutationOp::Substring {
                old_content: "hello()".to_string(),
                new_content: "hello(name: &str)".to_string(),
            },
            "Add name parameter",
        );
        let proposal = ChangeProposal::new(
            TaskId::new(),
            crate::ids::MissionId::new(),
            hypothesis,
            surface,
            vec![mutation],
        );

        let report = PreMutationReconciler::reconcile(
            dir.path(),
            &proposal,
            None,
            None,
            &HashSet::new(),
            None,
        );

        assert!(report.is_valid);
        assert!(report.violations.is_empty());
    }

    #[test]
    fn test_reconcile_stale_hash_rejected() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("src/lib.rs");
        std::fs::create_dir_all(file_path.parent().unwrap()).unwrap();
        std::fs::write(&file_path, "pub fn hello() {}\n").unwrap();

        let hypothesis = ImplementationHypothesis::new("Fix", "Cause", "Change", "Result", "Test");
        let surface = ChangeSurface::new(vec!["src/lib.rs".to_string()]);
        let mutation = FileMutationProposal::new(
            "src/lib.rs",
            FileMutationOp::Substring {
                old_content: "hello()".to_string(),
                new_content: "hello(name: &str)".to_string(),
            },
            "Add name",
        )
        .with_base_hash("0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef");

        let proposal = ChangeProposal::new(
            TaskId::new(),
            crate::ids::MissionId::new(),
            hypothesis,
            surface,
            vec![mutation],
        );

        let report = PreMutationReconciler::reconcile(
            dir.path(),
            &proposal,
            None,
            None,
            &HashSet::new(),
            None,
        );

        assert!(!report.is_valid);
        assert!(matches!(
            report.violations[0],
            ReconciliationViolation::StaleFileHash { .. }
        ));
    }

    #[test]
    fn test_reconcile_generated_file_rejected() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("src/generated.rs");
        std::fs::create_dir_all(file_path.parent().unwrap()).unwrap();
        std::fs::write(
            &file_path,
            "// @generated by protoc\npub struct ProtoMsg;\n",
        )
        .unwrap();

        let hypothesis = ImplementationHypothesis::new("Fix", "Cause", "Change", "Result", "Test");
        let surface = ChangeSurface::new(vec!["src/generated.rs".to_string()]);
        let mutation = FileMutationProposal::new(
            "src/generated.rs",
            FileMutationOp::Substring {
                old_content: "ProtoMsg".to_string(),
                new_content: "ProtoMsgV2".to_string(),
            },
            "Direct edit",
        );
        let proposal = ChangeProposal::new(
            TaskId::new(),
            crate::ids::MissionId::new(),
            hypothesis,
            surface,
            vec![mutation],
        );

        let report = PreMutationReconciler::reconcile(
            dir.path(),
            &proposal,
            None,
            None,
            &HashSet::new(),
            None,
        );

        assert!(!report.is_valid);
        assert!(matches!(
            report.violations[0],
            ReconciliationViolation::GeneratedFileDirectEdit { .. }
        ));
    }

    #[test]
    fn test_reconcile_concurrent_conflict_rejected() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("src/lib.rs");
        std::fs::create_dir_all(file_path.parent().unwrap()).unwrap();
        std::fs::write(&file_path, "code\n").unwrap();

        let hypothesis = ImplementationHypothesis::new("Fix", "Cause", "Change", "Result", "Test");
        let surface = ChangeSurface::new(vec!["src/lib.rs".to_string()]);
        let mutation = FileMutationProposal::new(
            "src/lib.rs",
            FileMutationOp::Substring {
                old_content: "code".to_string(),
                new_content: "new code".to_string(),
            },
            "Edit",
        );
        let proposal = ChangeProposal::new(
            TaskId::new(),
            crate::ids::MissionId::new(),
            hypothesis,
            surface,
            vec![mutation],
        );

        let mut locks = HashSet::new();
        locks.insert("src/lib.rs".to_string());

        let report = PreMutationReconciler::reconcile(
            dir.path(),
            &proposal,
            None,
            None,
            &locks,
            Some(TaskId::new()),
        );

        assert!(!report.is_valid);
        assert!(matches!(
            report.violations[0],
            ReconciliationViolation::ConcurrentTaskConflict { .. }
        ));
    }
}
