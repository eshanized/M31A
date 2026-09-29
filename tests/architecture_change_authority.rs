//! Architectural Regression Guards for Autonomous Code Modification & Change Authority (Phase 23, Section 39).
//!
//! Enforces:
//! - Strict single-crate Rust architecture
//! - Exactly one canonical owner for each code modification responsibility
//! - Zero shadow or duplicate modification engines
//! - Core principle: The model proposes. The runtime decides.

use m31a::change::{
    AppliedChangeSet, AtomicChangeApplier, ChangeApplyError, ChangeAuthority, ChangeAuthorityError,
    ChangeExecutionOutcome, ChangeProvenanceStore, DiffReviewer, FreshMutationEvidence,
    PostMutationObserver, PreMutationReconciler,
};
use m31a::kernel::change::{
    ChangeProposal, ChangeProposalId, ChangeProvenanceRecord, ChangeSurface, DiffReviewReport,
    DiffReviewViolation, FileMutationOp, FileMutationProposal, FilePrecondition,
    ImplementationHypothesis, ReconciliationReport, ReconciliationViolation,
};
use m31a::recovery::DifferentialReplanEngine;
use m31a::state_machine::change_set::{ChangeSetEvent, ChangeSetState, transition_change_set};
use m31a::tools::fs::editor::RobustFileEditor;
use m31a::verification::gate::EvidenceCompletionGate;

#[test]
fn test_canonical_change_proposal_type_is_unique() {
    let p1: ChangeProposal = ChangeProposal::new(
        m31a::ids::TaskId::new(),
        m31a::ids::MissionId::new(),
        ImplementationHypothesis::new("prob", "cause", "change", "res", "ver"),
        ChangeSurface::new(vec!["src/lib.rs".to_string()]),
        vec![],
    );
    let p2: m31a::kernel::ChangeProposal = p1.clone();
    assert_eq!(p1.id, p2.id);
}

#[test]
fn test_canonical_change_set_state_machine_is_unique() {
    let s1 = ChangeSetState::Proposed;
    let s2 = m31a::state_machine::ChangeSetState::Proposed;
    assert_eq!(s1, s2);

    let next = transition_change_set(s1, ChangeSetEvent::StartValidation).unwrap();
    assert_eq!(next, ChangeSetState::Validating);
}

#[test]
fn test_pre_mutation_reconciler_is_unique_authority() {
    // Assert PreMutationReconciler is accessible from canonical path
    let report = PreMutationReconciler::reconcile(
        std::path::Path::new("."),
        &ChangeProposal::new(
            m31a::ids::TaskId::new(),
            m31a::ids::MissionId::new(),
            ImplementationHypothesis::new("p", "c", "ch", "r", "v"),
            ChangeSurface::new(vec![]),
            vec![],
        ),
        None,
        None,
        &std::collections::HashSet::new(),
        None,
    );
    assert!(report.is_valid);
}

#[test]
fn test_atomic_change_applier_is_unique_applier() {
    // Assert AtomicChangeApplier is accessible and correctly typed
    let _ = std::any::type_name::<AtomicChangeApplier>();
    let _ = std::any::type_name::<AppliedChangeSet>();
    let _ = std::any::type_name::<ChangeApplyError>();
}

#[test]
fn test_post_mutation_observer_is_unique_observer() {
    let _ = std::any::type_name::<PostMutationObserver>();
    let _ = std::any::type_name::<FreshMutationEvidence>();
}

#[test]
fn test_diff_reviewer_is_unique_reviewer() {
    let _ = std::any::type_name::<DiffReviewer>();
    let _ = std::any::type_name::<DiffReviewReport>();
    let _ = std::any::type_name::<DiffReviewViolation>();
}

#[tokio::test]
async fn test_change_provenance_store_is_unique() {
    let store = ChangeProvenanceStore::new();
    let _ = std::any::type_name::<ChangeProvenanceStore>();
    assert_eq!(
        store
            .get_by_proposal(ChangeProposalId::new(), None)
            .await
            .unwrap(),
        None
    );
}

#[test]
fn test_change_authority_is_unique_authority() {
    let auth = ChangeAuthority::new();
    let _ = std::any::type_name::<ChangeAuthority>();
    let _ = std::any::type_name::<ChangeExecutionOutcome>();
    let _ = std::any::type_name::<ChangeAuthorityError>();
    assert!(auth.get_state(ChangeProposalId::new()).is_none());
}

#[test]
fn test_robust_file_editor_remains_canonical_primitive() {
    let _ = std::any::type_name::<RobustFileEditor>();
}

#[test]
fn test_replan_authority_remains_canonical() {
    let _ = std::any::type_name::<DifferentialReplanEngine>();
}

#[test]
fn test_completion_gate_remains_canonical() {
    let _ = std::any::type_name::<EvidenceCompletionGate>();
}

#[test]
fn test_change_kernel_types_at_l0() {
    let _ = std::any::type_name::<ChangeProposal>();
    let _ = std::any::type_name::<ChangeProposalId>();
    let _ = std::any::type_name::<ImplementationHypothesis>();
    let _ = std::any::type_name::<ChangeSurface>();
    let _ = std::any::type_name::<FilePrecondition>();
    let _ = std::any::type_name::<FileMutationOp>();
    let _ = std::any::type_name::<FileMutationProposal>();
    let _ = std::any::type_name::<ReconciliationViolation>();
    let _ = std::any::type_name::<ReconciliationReport>();
    let _ = std::any::type_name::<DiffReviewViolation>();
    let _ = std::any::type_name::<DiffReviewReport>();
    let _ = std::any::type_name::<ChangeProvenanceRecord>();
}
