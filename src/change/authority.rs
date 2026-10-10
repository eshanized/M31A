//! Canonical Change Authority coordinating software modification lifecycles.
//!
//! Enforces: The model proposes. The runtime decides.
//!
//! Mutation-lane ownership (explicit architecture: two canonical lanes with
//! type-visible boundaries, not duplicate authorities over the same semantic object):
//!
//! ```text
//! Lane A — governed single-file tool writes:
//!   model/tool action → ToolPipeline (11 stages: capability → policy →
//!   approval → sandbox) → workspace file → tool result + artifact
//!   Owner: ToolPipelineRunner. Scope: exactly one file per action.
//!
//! Lane B — multi-file change proposals:
//!   model/tool action → ToolPipeline → ChangeAuthority::
//!   execute_change_proposal (reconcile → reserve surface → atomic apply →
//!   observe → diff-review → provenance) → workspace mutation → change record
//!   Owner: ChangeAuthority. Scope: one proposal, N files, atomic with
//!   rollback.
//! ```
//!
//! Ownership boundaries:
//! - Lane A never performs multi-file atomic proposals, reconciliation,
//!   diff self-review, or provenance recording.
//! - Lane B never bypasses Lane A's invariants: proposal application goes
//!   through the same `FileSystemService` containment (workspace-root
//!   confinement, `../` escape rejection) and executes only after the
//!   pipeline's capability/policy/approval stages admitted the action.
//! - Neither lane can bypass the other's safety invariants: Lane A actions
//!   cannot fabricate Lane B provenance records; Lane B proposals cannot
//!   skip pipeline policy/approval (the proposal originates from an
//!   already-admitted tool action or an operator-driven repair).
//! - Both lanes are canonical because they govern DIFFERENT semantic
//!   objects (single file write vs. atomic multi-file change set).
//!   Cross-lane consistency is verified by architectural regression suites.
//!
//! Owns:
//! - Pre-mutation reconciliation against repository baselines and intelligence
//! - Concurrency coordination preventing overlapping task file modifications
//! - Change set state machine transitions
//! - Atomic multi-file mutation application with instant rollback on failure
//! - Post-mutation re-observation and fresh context generation
//! - Independent diff self-review rejecting fake implementations (Rule 5)
//! - Change provenance recording for requirement traceability

use sqlx::SqlitePool;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::{Arc, RwLock};

use crate::capability::traits::fs::FileSystemService;
use crate::change::applier::{AppliedChangeSet, AtomicChangeApplier, ChangeApplyError};
use crate::change::observer::{FreshMutationEvidence, PostMutationObserver};
use crate::change::provenance::ChangeProvenanceStore;
use crate::change::reconciler::PreMutationReconciler;
use crate::change::review::DiffReviewer;
use crate::ids::TaskId;
use crate::kernel::change::{
    ChangeProposal, ChangeProposalId, ChangeProvenanceRecord, ChangeSurface, DiffReviewReport,
    ReconciliationReport, ReconciliationViolation,
};
use crate::pipeline::dedup::{
    FenceVerdict, MutationDedupFence, canonical_json, fingerprint_parts, sha_hex,
};
use crate::repo::drift::RepositoryBaseline;
use crate::repo::query::BoundedQueryEngine;
use crate::state_machine::change_set::{ChangeSetEvent, ChangeSetState, transition_change_set};

/// Error taxonomy during ChangeAuthority operations.
#[allow(clippy::result_large_err)]
#[derive(Debug, thiserror::Error)]
pub enum ChangeAuthorityError {
    #[error("Pre-mutation reconciliation failed with {0} violations")]
    ReconciliationFailed(usize, ReconciliationReport),
    #[error("Concurrent task conflict: {0}")]
    ConcurrencyConflict(String),
    #[error("Mutation application failed: {0}")]
    ApplyFailed(#[from] ChangeApplyError),
    #[error("Diff review rejected proposed changes: {0:?}")]
    DiffReviewRejected(DiffReviewReport),
    #[error("Observation failed: {observation_error}; rollback restored {restored_count} files")]
    ObservationFailedWithRollback {
        observation_error: String,
        restored_count: usize,
    },
    #[error("Unresolved mutation / partial recovery: original error: '{original_error}', rollback error: '{rollback_error}'")]
    PartialRecovery {
        original_error: String,
        rollback_error: String,
    },
    #[error("Provenance recording failed: {0}")]
    ProvenanceRecordingFailed(String),
    #[error("I/O error during observation: {0}")]
    Io(#[from] std::io::Error),
    #[error("State machine transition error: {0}")]
    InvalidStateTransition(#[from] crate::state_machine::error::TransitionError),
}

/// Comprehensive outcome of an executed change proposal.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ChangeExecutionOutcome {
    pub proposal_id: ChangeProposalId,
    pub state: ChangeSetState,
    pub files_modified: Vec<String>,
    pub lines_modified: usize,
    pub fresh_evidence: FreshMutationEvidence,
    pub diff_review: DiffReviewReport,
    pub reconciliation_report: ReconciliationReport,
    pub applied_set: Option<AppliedChangeSet>,
}

/// Authoritative software-change runtime subsystem.
pub struct ChangeAuthority {
    pub provenance_store: Arc<ChangeProvenanceStore>,
    pub db_pool: Option<SqlitePool>,
    active_locks: Arc<RwLock<HashMap<String, TaskId>>>,
    proposal_states: Arc<RwLock<HashMap<ChangeProposalId, ChangeSetState>>>,
}

impl Default for ChangeAuthority {
    fn default() -> Self {
        Self::new()
    }
}

impl ChangeAuthority {
    pub fn new() -> Self {
        Self {
            provenance_store: Arc::new(ChangeProvenanceStore::new()),
            db_pool: None,
            active_locks: Arc::new(RwLock::new(HashMap::new())),
            proposal_states: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn with_pool(mut self, pool: SqlitePool) -> Self {
        self.db_pool = Some(pool);
        self
    }

    /// Retrieve the current lifecycle state of a change proposal.
    pub fn get_state(&self, proposal_id: ChangeProposalId) -> Option<ChangeSetState> {
        self.proposal_states
            .read()
            .ok()
            .and_then(|m| m.get(&proposal_id).cloned())
    }

    /// Reserve target files in a ChangeSurface for a task to prevent concurrent modification.
    pub fn reserve_surface(
        &self,
        task_id: TaskId,
        surface: &ChangeSurface,
    ) -> Result<(), ReconciliationViolation> {
        let mut locks =
            self.active_locks
                .write()
                .map_err(|_| ReconciliationViolation::PolicyBlocked {
                    reason: "Lock acquisition failed".to_string(),
                })?;

        for file in surface.all_target_files() {
            let clean = file.trim().trim_start_matches('@').trim_start_matches("./");
            if let Some(existing_task) = locks.get(clean)
                && *existing_task != task_id
            {
                return Err(ReconciliationViolation::ConcurrentTaskConflict {
                    path: clean.to_string(),
                    conflicting_task_id: *existing_task,
                });
            }
        }

        for file in surface.all_target_files() {
            let clean = file.trim().trim_start_matches('@').trim_start_matches("./");
            locks.insert(clean.to_string(), task_id);
        }

        Ok(())
    }

    /// Release all file reservations held by a task.
    pub fn release_surface(&self, task_id: TaskId) {
        if let Ok(mut locks) = self.active_locks.write() {
            locks.retain(|_, tid| *tid != task_id);
        }
    }

    /// Reconcile a change proposal against repository state prior to mutation.
    pub fn reconcile(
        &self,
        workspace_root: &Path,
        proposal: &ChangeProposal,
        baseline: Option<&RepositoryBaseline>,
        query_engine: Option<&BoundedQueryEngine>,
    ) -> ReconciliationReport {
        let active_locks_set: HashSet<String> = self
            .active_locks
            .read()
            .map(|l| {
                l.iter()
                    .filter(|(_, tid)| **tid != proposal.task_id)
                    .map(|(path, _)| path.clone())
                    .collect()
            })
            .unwrap_or_default();

        PreMutationReconciler::reconcile(
            workspace_root,
            proposal,
            baseline,
            query_engine,
            &active_locks_set,
            Some(proposal.task_id),
        )
    }

    /// Execute the complete, authorized change lifecycle:
    /// Proposed -> Validating -> Authorized -> Applying -> Applied -> Observing -> Verifying -> Accepted/Rejected/Failed.
    #[allow(clippy::result_large_err)]
    pub async fn execute_change_proposal(
        &self,
        workspace_root: &Path,
        proposal: &ChangeProposal,
        fs: &Arc<dyn FileSystemService>,
        baseline: Option<&RepositoryBaseline>,
        query_engine: Option<&BoundedQueryEngine>,
    ) -> Result<ChangeExecutionOutcome, ChangeAuthorityError> {
        // 0. Durable deduplication fence. An identical already-committed
        // proposal replays its recorded outcome with zero side effects;
        // recorded failure (or no record) proceeds so legitimate retry is
        // never blocked. The post-manifest check below proves the workspace
        // still holds the committed result before replaying it.
        let fence_fp = Self::proposal_fingerprint(proposal);
        if let Some(pool) = self.db_pool.clone() {
            let fence = MutationDedupFence::new(pool);
            if let FenceVerdict::DuplicateSuppressed {
                output_hash: Some(recorded),
            } = fence
                .check(&proposal.task_id, "change_proposal", &fence_fp)
                .await
            {
                // The workspace must still hold the committed result: recompute
                // the post-manifest live and compare against the recorded one.
                // Match → replay the recorded outcome with zero side effects.
                // Mismatch (or unreadable record) → fall through and
                // re-execute; reconciliation below judges the live state.
                if let Some(stored) = fence
                    .recorded_outcome(&proposal.task_id, "change_proposal", &fence_fp)
                    .await
                    && let Ok(prior) = serde_json::from_str::<ChangeExecutionOutcome>(&stored)
                    && Self::post_manifest(workspace_root, &prior.files_modified)
                        .await
                        .as_deref()
                        == Some(recorded.as_str())
                {
                    self.set_state(proposal.id, ChangeSetState::Accepted);
                    return Ok(prior);
                }
            }
        }

        // 1. Initial State: Proposed
        let mut current_state = ChangeSetState::Proposed;
        self.set_state(proposal.id, current_state);

        // 2. Transition to Validating
        current_state = transition_change_set(current_state, ChangeSetEvent::StartValidation)?;
        self.set_state(proposal.id, current_state);

        // 3. Pre-mutation reconciliation
        let reconciliation_report =
            self.reconcile(workspace_root, proposal, baseline, query_engine);

        if !reconciliation_report.is_valid {
            let is_conflict = reconciliation_report.violations.iter().any(|v| {
                matches!(
                    v,
                    ReconciliationViolation::ConcurrentTaskConflict { .. }
                        | ReconciliationViolation::StaleFileHash { .. }
                )
            });

            current_state = if is_conflict {
                transition_change_set(current_state, ChangeSetEvent::ReportConflict)?
            } else {
                transition_change_set(current_state, ChangeSetEvent::Reject)?
            };
            self.set_state(proposal.id, current_state);

            self.fence_record(proposal, &fence_fp, false, None, None)
                .await;
            return Err(ChangeAuthorityError::ReconciliationFailed(
                reconciliation_report.violations.len(),
                reconciliation_report,
            ));
        }

        // 4. Transition to Authorized
        current_state = transition_change_set(current_state, ChangeSetEvent::Authorize)?;
        self.set_state(proposal.id, current_state);

        // 5. Reserve change surface for this task
        if let Err(viol) = self.reserve_surface(proposal.task_id, &proposal.change_surface) {
            current_state = transition_change_set(current_state, ChangeSetEvent::ReportConflict)?;
            self.set_state(proposal.id, current_state);
            self.fence_record(proposal, &fence_fp, false, None, None)
                .await;
            return Err(ChangeAuthorityError::ConcurrencyConflict(viol.to_string()));
        }

        // 6. Transition to Applying
        current_state = transition_change_set(current_state, ChangeSetEvent::StartApplying)?;
        self.set_state(proposal.id, current_state);

        // 7. Atomic change application
        let applied_res = AtomicChangeApplier::apply(workspace_root, proposal, fs).await;

        let applied_set = match applied_res {
            Ok(set) => {
                current_state =
                    transition_change_set(current_state, ChangeSetEvent::MutationApplied)?;
                self.set_state(proposal.id, current_state);
                set
            }
            Err(apply_err) => {
                let is_unresolved = matches!(apply_err, ChangeApplyError::RollbackFailed { .. });
                current_state = if is_unresolved {
                    transition_change_set(current_state, ChangeSetEvent::RecoveryFailed)?
                } else {
                    transition_change_set(current_state, ChangeSetEvent::Rollback)?
                };
                self.set_state(proposal.id, current_state);
                self.release_surface(proposal.task_id);
                self.fence_record(proposal, &fence_fp, false, None, None)
                    .await;
                if let ChangeApplyError::RollbackFailed {
                    failed_path,
                    reason,
                    failed_restorations,
                    ..
                } = &apply_err
                {
                    return Err(ChangeAuthorityError::PartialRecovery {
                        original_error: format!("Apply failed on '{failed_path}': {reason}"),
                        rollback_error: format!(
                            "Rollback verification failed on {} files: {:?}",
                            failed_restorations.len(),
                            failed_restorations
                        ),
                    });
                }
                return Err(ChangeAuthorityError::ApplyFailed(apply_err));
            }
        };

        // 8. Transition to Observing & capture fresh repository observation
        current_state = transition_change_set(current_state, ChangeSetEvent::StartObserving)?;
        self.set_state(proposal.id, current_state);

        let fresh_evidence = match PostMutationObserver::observe_with_snapshots(
            workspace_root,
            &applied_set.files_modified,
            query_engine,
            Some(&applied_set.original_snapshots),
        )
        .await
        {
            Ok(evidence) => evidence,
            Err(e) => {
                // Truthful rollback on observation failure: do not leave unobserved mutations in workspace
                let rollback_res = applied_set.rollback(fs).await;
                self.release_surface(proposal.task_id);
                self.fence_record(proposal, &fence_fp, false, None, None)
                    .await;

                match rollback_res {
                    Ok(report) if report.is_fully_restored => {
                        current_state =
                            transition_change_set(current_state, ChangeSetEvent::Rollback)?;
                        self.set_state(proposal.id, current_state);
                        return Err(ChangeAuthorityError::ObservationFailedWithRollback {
                            observation_error: e.to_string(),
                            restored_count: report.restored_count,
                        });
                    }
                    Ok(report) => {
                        current_state =
                            transition_change_set(current_state, ChangeSetEvent::RecoveryFailed)?;
                        self.set_state(proposal.id, current_state);
                        return Err(ChangeAuthorityError::PartialRecovery {
                            original_error: format!("Observation failed: {e}"),
                            rollback_error: format!(
                                "Rollback failed to restore {} paths: {:?}",
                                report.failed_paths.len(),
                                report.failed_paths
                            ),
                        });
                    }
                    Err(rb_err) => {
                        current_state =
                            transition_change_set(current_state, ChangeSetEvent::RecoveryFailed)?;
                        self.set_state(proposal.id, current_state);
                        return Err(ChangeAuthorityError::PartialRecovery {
                            original_error: format!("Observation failed: {e}"),
                            rollback_error: rb_err.to_string(),
                        });
                    }
                }
            }
        };

        // 9. Transition to Verifying & run diff review
        current_state = transition_change_set(current_state, ChangeSetEvent::StartVerifying)?;
        self.set_state(proposal.id, current_state);

        let diff_review = DiffReviewer::review(
            &fresh_evidence.unified_diff,
            &proposal.change_surface,
            &applied_set.files_modified,
        );

        if !diff_review.passed {
            // Anti-fake rule violation: roll back workspace changes immediately
            let rollback_res = applied_set.rollback(fs).await;
            self.release_surface(proposal.task_id);
            self.fence_record(proposal, &fence_fp, false, None, None)
                .await;

            match rollback_res {
                Ok(report) if report.is_fully_restored => {
                    current_state =
                        transition_change_set(current_state, ChangeSetEvent::Rollback)?;
                    self.set_state(proposal.id, current_state);
                    return Err(ChangeAuthorityError::DiffReviewRejected(diff_review));
                }
                Ok(report) => {
                    current_state =
                        transition_change_set(current_state, ChangeSetEvent::RecoveryFailed)?;
                    self.set_state(proposal.id, current_state);
                    return Err(ChangeAuthorityError::PartialRecovery {
                        original_error: format!(
                            "Diff review rejected proposed changes: {:?}",
                            diff_review.violations
                        ),
                        rollback_error: format!(
                            "Rollback failed to restore {} paths: {:?}",
                            report.failed_paths.len(),
                            report.failed_paths
                        ),
                    });
                }
                Err(rb_err) => {
                    current_state =
                        transition_change_set(current_state, ChangeSetEvent::RecoveryFailed)?;
                    self.set_state(proposal.id, current_state);
                    return Err(ChangeAuthorityError::PartialRecovery {
                        original_error: format!(
                            "Diff review rejected proposed changes: {:?}",
                            diff_review.violations
                        ),
                        rollback_error: rb_err.to_string(),
                    });
                }
            }
        }

        // 10. Transition to Accepted
        current_state = transition_change_set(current_state, ChangeSetEvent::Accept)?;
        self.set_state(proposal.id, current_state);

        // 11. Record durable provenance with real cryptographic hashes
        let mut per_file_pre: Vec<(String, Option<String>)> = Vec::new();
        for file in &applied_set.files_modified {
            let path = Path::new(file);
            let snap = applied_set.original_snapshots.iter().find(|(p, _)| {
                p.ends_with(path) || p.as_path() == path
            });
            match snap {
                Some((_, Some(bytes))) => {
                    per_file_pre.push((
                        file.clone(),
                        Some(crate::release::integrity::sha256_bytes(bytes)),
                    ));
                }
                _ => {
                    per_file_pre.push((file.clone(), None));
                }
            }
        }

        let mut per_file_post: Vec<(String, Option<String>)> = Vec::new();
        for file in &applied_set.files_modified {
            let h = fresh_evidence.fresh_file_hashes.get(file).cloned();
            per_file_post.push((file.clone(), h));
        }

        let pre_hash = compute_aggregate_hash(&per_file_pre);
        let post_hash = compute_aggregate_hash(&per_file_post);

        let prov_record = ChangeProvenanceRecord {
            proposal_id: proposal.id,
            task_id: proposal.task_id,
            mission_id: proposal.mission_id,
            requirement_keys: Vec::new(),
            affected_files: applied_set.files_modified.clone(),
            affected_symbols: fresh_evidence.affected_symbols.clone(),
            pre_mutation_hash: pre_hash,
            post_mutation_hash: post_hash,
            diff_summary: format!(
                "Modified {} files, {} lines. Files: {:?}",
                applied_set.files_modified.len(),
                applied_set.lines_modified,
                applied_set.files_modified
            ),
            verification_passed: false, // Truthful: test execution pipeline has not run yet
            timestamp: chrono::Utc::now(),
        };

        self.provenance_store
            .record(&prov_record, self.db_pool.as_ref())
            .await
            .map_err(|e| ChangeAuthorityError::ProvenanceRecordingFailed(e.to_string()))?;

        self.release_surface(proposal.task_id);

        let outcome = ChangeExecutionOutcome {
            proposal_id: proposal.id,
            state: current_state,
            files_modified: applied_set.files_modified.clone(),
            lines_modified: applied_set.lines_modified,
            fresh_evidence,
            diff_review,
            reconciliation_report,
            applied_set: Some(applied_set),
        };
        // Claim the fence with the post-manifest so a crash replay recognizes
        // this exact committed result. Snapshots are stripped from the stored
        // outcome (rollback needs live state, not history).
        let manifest = Self::post_manifest(workspace_root, &outcome.files_modified).await;
        let mut stored = outcome.clone();
        stored.applied_set = None;
        let stored_json = serde_json::to_string(&stored).ok();
        self.fence_record(
            proposal,
            &fence_fp,
            true,
            manifest.as_deref(),
            stored_json.as_deref(),
        )
        .await;
        Ok(outcome)
    }

    /// Semantic fingerprint for a proposal: task identity plus canonical
    /// surface/mutation content. Stable across restarts for identical work,
    /// never timestamp-only.
    fn proposal_fingerprint(proposal: &ChangeProposal) -> String {
        let content = serde_json::to_value((&proposal.change_surface, &proposal.mutations))
            .unwrap_or(serde_json::Value::Null);
        fingerprint_parts(&[
            proposal.task_id.as_bytes(),
            canonical_json(&content).as_bytes(),
        ])
    }

    /// Post-manifest: canonical `path:content-hash` manifest of the files a
    /// committed proposal produced. Recomputed on fence hits to prove the
    /// workspace still holds the recorded result before replaying it.
    async fn post_manifest(workspace_root: &Path, files: &[String]) -> Option<String> {
        let mut entries = Vec::new();
        for rel in files {
            let full = workspace_root.join(rel);
            let bytes = tokio::fs::read(&full).await.ok()?;
            entries.push(format!("{rel}:{}", sha_hex(&bytes)));
        }
        entries.sort();
        Some(sha_hex(entries.join("\n").as_bytes()))
    }

    /// Record a fence outcome. Fence failures never fail the primary flow:
    /// they degrade to at-least-once execution with a warning trace.
    async fn fence_record(
        &self,
        proposal: &ChangeProposal,
        fingerprint: &str,
        success: bool,
        output_hash: Option<&str>,
        outcome_json: Option<&str>,
    ) {
        if let Some(pool) = self.db_pool.clone() {
            MutationDedupFence::new(pool)
                .record(
                    &proposal.task_id,
                    "change_proposal",
                    fingerprint,
                    success,
                    output_hash,
                    outcome_json,
                )
                .await;
        }
    }

    fn set_state(&self, proposal_id: ChangeProposalId, state: ChangeSetState) {
        if let Ok(mut states) = self.proposal_states.write() {
            states.insert(proposal_id, state);
        }
    }
}

/// Compute a deterministic aggregate cryptographic hash for a set of file hashes.
fn compute_aggregate_hash(file_hashes: &[(String, Option<String>)]) -> String {
    use sha2::{Digest, Sha256};
    if file_hashes.is_empty() {
        return "none".to_string();
    }
    if file_hashes.len() == 1 {
        let (file, hash) = &file_hashes[0];
        return format!("{}:{}", file, hash.as_deref().unwrap_or("none"));
    }
    let mut sorted = file_hashes.to_vec();
    sorted.sort_by(|a, b| a.0.cmp(&b.0));
    let mut hasher = Sha256::new();
    for (file, hash) in sorted {
        hasher.update(file.as_bytes());
        hasher.update(b":");
        hasher.update(hash.as_deref().unwrap_or("none").as_bytes());
        hasher.update(b"\n");
    }
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capability::providers::local_fs::LocalFileSystemProvider;
    use crate::ids::MissionId;
    use crate::kernel::change::{FileMutationOp, FileMutationProposal, ImplementationHypothesis};
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_change_authority_end_to_end_accepted() {
        let dir = tempdir().unwrap();
        let ws = dir.path();
        let fs: Arc<dyn FileSystemService> = Arc::new(LocalFileSystemProvider::new(ws).unwrap());

        fs.write_file(
            Path::new("src/math.rs"),
            b"pub fn add(a: i32, b: i32) -> i32 { a }\n",
        )
        .await
        .unwrap();

        let hypothesis = ImplementationHypothesis::new(
            "Fix add bug",
            "Returns a instead of a + b",
            "Change return to a + b",
            "add returns sum",
            "cargo test",
        );
        let surface = ChangeSurface::new(vec!["src/math.rs".to_string()]);
        let mutation = FileMutationProposal::new(
            "src/math.rs",
            FileMutationOp::Substring {
                old_content: "{ a }".to_string(),
                new_content: "{ a + b }".to_string(),
            },
            "Return sum",
        );

        let proposal = ChangeProposal::new(
            TaskId::new(),
            MissionId::new(),
            hypothesis,
            surface,
            vec![mutation],
        );

        let authority = ChangeAuthority::new();
        let outcome = authority
            .execute_change_proposal(ws, &proposal, &fs, None, None)
            .await
            .unwrap();

        assert_eq!(outcome.state, ChangeSetState::Accepted);
        assert_eq!(outcome.files_modified, vec!["src/math.rs"]);
        assert!(outcome.diff_review.passed);

        let new_content = String::from_utf8(
            fs.read_file(Path::new("src/math.rs"), None, None)
                .await
                .unwrap(),
        )
        .unwrap();
        assert!(new_content.contains("{ a + b }"));
    }

    #[tokio::test]
    async fn test_change_authority_rejects_fake_implementation_and_rolls_back() {
        let dir = tempdir().unwrap();
        let ws = dir.path();
        let fs: Arc<dyn FileSystemService> = Arc::new(LocalFileSystemProvider::new(ws).unwrap());

        let original = "pub fn calculate() -> u32 { 0 }\n";
        fs.write_file(Path::new("src/calc.rs"), original.as_bytes())
            .await
            .unwrap();

        let hypothesis =
            ImplementationHypothesis::new("Implement", "Empty", "Add todo", "calc", "test");
        let surface = ChangeSurface::new(vec!["src/calc.rs".to_string()]);
        let mutation = FileMutationProposal::new(
            "src/calc.rs",
            FileMutationOp::Substring {
                old_content: "0".to_string(),
                new_content: "todo!()".to_string(), // Fake implementation!
            },
            "Fake it",
        );

        let proposal = ChangeProposal::new(
            TaskId::new(),
            MissionId::new(),
            hypothesis,
            surface,
            vec![mutation],
        );

        let authority = ChangeAuthority::new();
        let err = authority
            .execute_change_proposal(ws, &proposal, &fs, None, None)
            .await
            .unwrap_err();

        assert!(matches!(err, ChangeAuthorityError::DiffReviewRejected(..)));
        assert_eq!(
            authority.get_state(proposal.id),
            Some(ChangeSetState::RolledBack)
        );

        // Workspace must be rolled back!
        let rolled_back_content = String::from_utf8(
            fs.read_file(Path::new("src/calc.rs"), None, None)
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(rolled_back_content, original);
    }

    #[tokio::test]
    async fn test_provenance_cryptographic_hashes_accurate_and_verification_truthful() {
        let dir = tempdir().unwrap();
        let ws = dir.path();
        let fs: Arc<dyn FileSystemService> = Arc::new(LocalFileSystemProvider::new(ws).unwrap());

        let initial_bytes = b"pub fn initial() {}\n";
        fs.write_file(Path::new("src/lib.rs"), initial_bytes)
            .await
            .unwrap();

        let initial_sha = crate::release::integrity::sha256_bytes(initial_bytes);

        let hypothesis = ImplementationHypothesis::new("Refactor", "None", "Update", "Done", "cargo check");
        let surface = ChangeSurface::new(vec!["src/lib.rs".to_string()]);
        let mutation = FileMutationProposal::new(
            "src/lib.rs",
            FileMutationOp::Substring {
                old_content: "pub fn initial() {}".to_string(),
                new_content: "pub fn updated() {}".to_string(),
            },
            "Update func",
        );

        let proposal = ChangeProposal::new(
            TaskId::new(),
            MissionId::new(),
            hypothesis,
            surface,
            vec![mutation],
        );

        let authority = ChangeAuthority::new();
        let outcome = authority
            .execute_change_proposal(ws, &proposal, &fs, None, None)
            .await
            .unwrap();

        assert_eq!(outcome.state, ChangeSetState::Accepted);

        let recorded_prov = authority
            .provenance_store
            .get_by_proposal(proposal.id, None)
            .await
            .unwrap()
            .expect("provenance record must exist");

        // Pre-mutation hash must match real SHA-256 of initial bytes, never a fallback string!
        assert!(recorded_prov.pre_mutation_hash.contains(&initial_sha));
        assert_ne!(recorded_prov.pre_mutation_hash, "pre_mutation");
        assert_ne!(recorded_prov.post_mutation_hash, "post_mutation");

        // Verification must be truthful: false, because compiler/test pipeline has not run yet!
        assert!(!recorded_prov.verification_passed);
    }

    #[tokio::test]
    async fn test_surface_reservation_released_on_rejected_diff() {
        let dir = tempdir().unwrap();
        let ws = dir.path();
        let fs: Arc<dyn FileSystemService> = Arc::new(LocalFileSystemProvider::new(ws).unwrap());

        fs.write_file(Path::new("src/item.rs"), b"item 1\n")
            .await
            .unwrap();

        let task_id = TaskId::new();
        let hypothesis = ImplementationHypothesis::new("Todo", "None", "Todo", "Done", "cargo check");
        let surface = ChangeSurface::new(vec!["src/item.rs".to_string()]);
        let mutation = FileMutationProposal::new(
            "src/item.rs",
            FileMutationOp::Substring {
                old_content: "1".to_string(),
                new_content: "todo!()".to_string(),
            },
            "Fake implementation",
        );

        let proposal = ChangeProposal::new(
            task_id,
            MissionId::new(),
            hypothesis,
            surface.clone(),
            vec![mutation],
        );

        let authority = ChangeAuthority::new();
        let _ = authority
            .execute_change_proposal(ws, &proposal, &fs, None, None)
            .await;

        // Surface must be cleanly released, allowing new reservation
        assert!(authority.reserve_surface(task_id, &surface).is_ok());
    }
}
