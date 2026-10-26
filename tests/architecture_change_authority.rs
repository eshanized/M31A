//! Architectural Regression Guards for Autonomous Code Modification & Change Authority (Phase 23, Section 39).
//!
//! Enforces:
//! - Strict single-crate Rust architecture
//! - Exactly one canonical owner for each code modification responsibility
//! - Zero shadow or duplicate modification engines
//! - Core principle: The model proposes. The runtime decides.

use m31a::change::{
    AppliedChangeSet, AtomicChangeApplier, ChangeAuthority, ChangeProvenanceStore, DiffReviewer,
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
    use std::any::TypeId;
    assert_eq!(
        TypeId::of::<ChangeProposal>(),
        TypeId::of::<m31a::kernel::ChangeProposal>(),
        "ChangeProposal must be the exact same canonical type across kernel and change subsystems"
    );

    let p1 = ChangeProposal::new(
        m31a::ids::TaskId::new(),
        m31a::ids::MissionId::new(),
        ImplementationHypothesis::new("prob", "cause", "change", "res", "ver"),
        ChangeSurface::new(vec!["src/lib.rs".to_string()]),
        vec![],
    );
    let p2 = ChangeProposal::new(
        m31a::ids::TaskId::new(),
        m31a::ids::MissionId::new(),
        ImplementationHypothesis::new("prob2", "cause2", "change2", "res2", "ver2"),
        ChangeSurface::new(vec!["src/main.rs".to_string()]),
        vec![],
    );
    assert_ne!(
        p1.id, p2.id,
        "Distinct proposals must receive distinct identities"
    );
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

#[tokio::test]
async fn test_atomic_change_applier_is_unique_applier() {
    let dir = tempfile::tempdir().expect("tempdir");
    let fs: std::sync::Arc<dyn m31a::capability::traits::fs::FileSystemService> =
        std::sync::Arc::new(
            m31a::capability::providers::LocalFileSystemProvider::new(dir.path()).unwrap(),
        );
    let proposal = ChangeProposal::new(
        m31a::ids::TaskId::new(),
        m31a::ids::MissionId::new(),
        ImplementationHypothesis::new("prob", "cause", "change", "res", "ver"),
        ChangeSurface::new(vec![]),
        vec![],
    );
    let outcome: AppliedChangeSet = AtomicChangeApplier::apply(dir.path(), &proposal, &fs)
        .await
        .expect("empty change proposal applies");
    assert_eq!(outcome.files_modified.len(), 0);
    assert_eq!(outcome.lines_modified, 0);
}

#[tokio::test]
async fn test_post_mutation_observer_is_unique_observer() {
    let dir = tempfile::tempdir().expect("tempdir");
    let test_file = dir.path().join("foo.rs");
    std::fs::write(&test_file, b"fn main() {}").unwrap();

    // 1. Observing contained file succeeds
    let evidence = PostMutationObserver::observe(dir.path(), &["foo.rs".to_string()], None)
        .await
        .expect("observe contained file");
    assert!(evidence.fresh_file_hashes.contains_key("foo.rs"));

    // 2. Observing escaping path fails closed
    let escape_res =
        PostMutationObserver::observe(dir.path(), &["../escape.rs".to_string()], None).await;
    assert!(
        escape_res.is_err(),
        "Observing escaping path must fail closed"
    );
}

#[test]
fn test_diff_reviewer_is_unique_reviewer() {
    let surface = ChangeSurface::new(vec!["src/lib.rs".to_string()]);

    // Authorized file: approved
    let report_ok = DiffReviewer::review("", &surface, &["src/lib.rs".to_string()]);
    assert!(report_ok.passed);
    assert!(report_ok.violations.is_empty());

    // Unauthorized file: rejected with AccidentalEdit
    let report_bad = DiffReviewer::review("", &surface, &["src/secret.rs".to_string()]);
    assert!(!report_bad.passed);
    assert!(
        report_bad
            .violations
            .iter()
            .any(|v| matches!(v, DiffReviewViolation::AccidentalEdit { .. }))
    );
}

#[tokio::test]
async fn test_change_provenance_store_is_unique() {
    let store = ChangeProvenanceStore::new();
    let proposal_id = ChangeProposalId::new();
    let task_id = m31a::ids::TaskId::new();
    let mission_id = m31a::ids::MissionId::new();

    // 1. Initial lookup on clean store yields None
    assert_eq!(
        store.get_by_proposal(proposal_id, None).await.unwrap(),
        None,
        "Unknown proposal ID must return None"
    );

    // 2. Behavioral verification: record provenance and assert faithful retrieval
    let record = ChangeProvenanceRecord {
        proposal_id,
        task_id,
        mission_id,
        requirement_keys: vec!["REQ-101".to_string()],
        affected_files: vec!["src/provenance.rs".to_string()],
        affected_symbols: vec!["ChangeProvenanceStore".to_string()],
        pre_mutation_hash: "a1b2c3d4".to_string(),
        post_mutation_hash: "e5f6g7h8".to_string(),
        diff_summary: "+10 -2".to_string(),
        verification_passed: true,
        timestamp: chrono::Utc::now(),
    };

    store
        .record(&record, None)
        .await
        .expect("recording provenance in memory cache succeeds");

    let retrieved = store
        .get_by_proposal(proposal_id, None)
        .await
        .expect("retrieval succeeds")
        .expect("recorded provenance must be found");

    assert_eq!(retrieved.proposal_id, proposal_id);
    assert_eq!(retrieved.task_id, task_id);
    assert_eq!(retrieved.pre_mutation_hash, "a1b2c3d4");
    assert_eq!(retrieved.post_mutation_hash, "e5f6g7h8");
    assert!(retrieved.verification_passed);

    // Unrelated proposal remains None
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
    let task1 = m31a::ids::TaskId::new();
    let task2 = m31a::ids::TaskId::new();
    let surface = ChangeSurface::new(vec!["src/auth.rs".to_string()]);

    // 1. Task 1 reserves the change surface
    assert!(
        auth.reserve_surface(task1, &surface).is_ok(),
        "Task 1 surface reservation must succeed"
    );

    // 2. Task 2 attempts to reserve the overlapping surface -> must fail closed with ConcurrentTaskConflict
    let conflict = auth
        .reserve_surface(task2, &surface)
        .expect_err("Overlapping surface reservation must be rejected");
    match conflict {
        ReconciliationViolation::ConcurrentTaskConflict {
            path,
            conflicting_task_id,
        } => {
            assert_eq!(path, "src/auth.rs");
            assert_eq!(conflicting_task_id, task1);
        }
        other => panic!("Expected ConcurrentTaskConflict, got: {other:?}"),
    }

    // 3. Task 1 releases its surface; now Task 2 can reserve it
    auth.release_surface(task1);
    assert!(
        auth.reserve_surface(task2, &surface).is_ok(),
        "After release, Task 2 can acquire surface"
    );

    // 4. Initial state of arbitrary proposal is None
    assert!(auth.get_state(ChangeProposalId::new()).is_none());
}

#[test]
fn test_robust_file_editor_remains_canonical_primitive() {
    use m31a::tools::fs::editor::FileEditOp;

    let original = "hello world\n";
    let edit_op = FileEditOp::Substring {
        old_content: "world",
        new_content: "m31a",
    };
    let success = RobustFileEditor::apply(original, &edit_op).expect("substring replacement");
    assert_eq!(success.new_content, "hello m31a\n");

    let bad_op = FileEditOp::Substring {
        old_content: "missing_token",
        new_content: "replacement",
    };
    assert!(
        RobustFileEditor::apply(original, &bad_op).is_err(),
        "missing substring must return diagnostic error"
    );
}

#[test]
fn test_replan_authority_remains_canonical() {
    assert_eq!(
        std::any::TypeId::of::<DifferentialReplanEngine>(),
        std::any::TypeId::of::<m31a::recovery::replan::DifferentialReplanEngine>()
    );
    assert_ne!(
        std::any::TypeId::of::<DifferentialReplanEngine>(),
        std::any::TypeId::of::<DiffReviewer>()
    );
}

#[tokio::test]
async fn test_completion_gate_remains_canonical() {
    use m31a::kernel::seams::verifier::VerificationEngine;
    use m31a::persistence::sqlite::schema::run_migrations;
    use sqlx::sqlite::SqlitePoolOptions;

    fn assert_implements<T: VerificationEngine>() {}
    assert_implements::<EvidenceCompletionGate>();
    assert_eq!(
        std::any::TypeId::of::<EvidenceCompletionGate>(),
        std::any::TypeId::of::<m31a::verification::gate::EvidenceCompletionGate>()
    );

    // Behavioral verification: evaluate task completion against authoritative gate
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("in-memory db");
    run_migrations(&pool).await.expect("migrations succeed");

    let dir = tempfile::tempdir().expect("tempdir");
    let store = std::sync::Arc::new(
        m31a::persistence::artifacts::fs_store::FsArtifactStore::new(dir.path().join("artifacts")),
    );
    let gate = EvidenceCompletionGate::new(pool.clone(), store, dir.path());

    let mission_id = m31a::ids::MissionId::new();
    let task_id = m31a::ids::TaskId::new();

    // 1. Missing task record in DB fails closed with error
    let missing_err = gate
        .evaluate_task_completion(mission_id, task_id, "snap_0")
        .await;
    assert!(
        missing_err.is_err(),
        "Gate must reject evaluation for unpersisted task record"
    );
    assert!(missing_err.unwrap_err().contains("not found in database"));

    // 2. Insert mission and task requiring tests verification into DB
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO missions (id, objective, status, created_at, updated_at) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(mission_id.as_bytes().as_slice())
    .bind("Gate Test Mission")
    .bind("Running")
    .bind(&now)
    .bind(&now)
    .execute(&pool)
    .await
    .unwrap();

    let verif_spec = serde_json::json!({
        "required": true,
        "strategy": "tests",
        "tiers": ["unit_test"],
        "evidence_expected": ["test_execution"]
    })
    .to_string();

    sqlx::query(
        "INSERT INTO tasks (id, mission_id, title, role, status, verification, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(task_id.as_bytes().as_slice())
    .bind(mission_id.as_bytes().as_slice())
    .bind("Task with tests requirement")
    .bind("implementer")
    .bind("Running")
    .bind(&verif_spec)
    .bind(&now)
    .bind(&now)
    .execute(&pool)
    .await
    .unwrap();

    // 3. Evaluate task before any verification checks exist: must be unsatisfied
    let decision_no_evidence = gate
        .evaluate_task_completion(mission_id, task_id, "snap_1")
        .await
        .expect("evaluation query succeeds");
    assert!(
        !decision_no_evidence.is_satisfied,
        "Completion gate must reject completion when required test evidence is absent"
    );
}

#[test]
fn test_change_kernel_types_at_l0() {
    let task_id = m31a::ids::TaskId::new();
    let mission_id = m31a::ids::MissionId::new();
    let hypothesis = ImplementationHypothesis::new("prob", "cause", "change", "res", "ver");
    let surface = ChangeSurface::new(vec!["src/main.rs".to_string()]);
    let proposal = ChangeProposal::new(task_id, mission_id, hypothesis, surface.clone(), vec![]);

    assert_eq!(proposal.task_id, task_id);
    assert_eq!(proposal.mission_id, mission_id);
    assert!(surface.contains_path("src/main.rs"));
    assert!(!surface.contains_path("src/other.rs"));

    let pre = FilePrecondition::exists("src/main.rs");
    assert!(pre.must_exist);
    let op = FileMutationOp::CreateNew {
        content: "fn main() {}".to_string(),
    };
    let mut_prop = FileMutationProposal::new("src/main.rs", op, "create new entrypoint");
    assert_eq!(mut_prop.path, "src/main.rs");

    let rec_viol = ReconciliationViolation::MissingFile { path: "x".into() };
    let rec_report = ReconciliationReport::invalid(vec![rec_viol]);
    assert!(!rec_report.is_valid);

    let diff_viol = DiffReviewViolation::AccidentalEdit {
        path: "y".into(),
        reason: "unauthorized".into(),
    };
    let diff_report = DiffReviewReport::with_violations(vec![diff_viol], 1, 0, 0);
    assert!(!diff_report.passed);

    let prov = ChangeProvenanceRecord {
        proposal_id: proposal.id,
        task_id,
        mission_id,
        requirement_keys: vec!["REQ-1".into()],
        affected_files: vec!["src/main.rs".into()],
        affected_symbols: vec!["main".into()],
        pre_mutation_hash: "hash_pre".into(),
        post_mutation_hash: "hash_post".into(),
        diff_summary: "+1 -0".into(),
        verification_passed: true,
        timestamp: chrono::Utc::now(),
    };
    assert_eq!(prov.proposal_id, proposal.id);
}
