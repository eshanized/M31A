//! Phase 23 — Adversarial Code Mutation & Safety Authority Tests
//!
//! Tests adversarial runtime defenses:
//! 1. Target file changed after planning (stale patch / hash mismatch rejected)
//! 2. Target file missing
//! 3. Target symbol missing
//! 4. Direct edit of generated file rejected
//! 5. Accidental scope creep rejected before mutation
//! 6. Partial edit failure triggers complete atomic rollback of all files
//! 7. Diff review rejects fake implementation (`todo!()`, `unimplemented!()`) and rolls back
//! 8. Diff review rejects commented-out or deleted test assertions
//! 9. Concurrent task surface conflict detected and rejected

use m31a::capability::providers::local_fs::LocalFileSystemProvider;
use m31a::capability::traits::FileSystemService;
use m31a::change::authority::{ChangeAuthority, ChangeAuthorityError};
use m31a::ids::{MissionId, TaskId};
use m31a::kernel::change::{
    ChangeProposal, ChangeSurface, FileMutationOp, FileMutationProposal, FilePrecondition,
    ImplementationHypothesis,
};
use m31a::state_machine::change_set::ChangeSetState;
use std::path::Path;
use std::sync::Arc;
use tempfile::tempdir;

// =========================================================================
// 1. Target file changed after planning (stale patch / hash mismatch)
// =========================================================================
#[tokio::test]
async fn test_adversarial_target_file_hash_mismatch() {
    let dir = tempdir().unwrap();
    let ws = dir.path();
    let fs: Arc<dyn FileSystemService> = Arc::new(LocalFileSystemProvider::new(ws).unwrap());

    let original = b"pub fn compute() -> i32 { 100 }\n";
    fs.write_file(Path::new("src/compute.rs"), original)
        .await
        .unwrap();

    let stale_hash = "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789".to_string();

    let hypothesis = ImplementationHypothesis::new("Fix", "Cause", "Change", "Result", "Test");
    let surface = ChangeSurface::new(vec!["src/compute.rs".to_string()]);

    let mut proposal = ChangeProposal::new(
        TaskId::new(),
        MissionId::new(),
        hypothesis,
        surface,
        vec![FileMutationProposal::new(
            "src/compute.rs",
            FileMutationOp::Substring {
                old_content: "100".to_string(),
                new_content: "200".to_string(),
            },
            "Update compute",
        )],
    );
    proposal
        .preconditions
        .push(FilePrecondition::exists_with_hash(
            "src/compute.rs",
            stale_hash,
        ));

    let authority = ChangeAuthority::new();
    let err = authority
        .execute_change_proposal(ws, &proposal, &fs, None, None)
        .await
        .unwrap_err();

    assert!(matches!(
        err,
        ChangeAuthorityError::ReconciliationFailed(..)
    ));
    assert_eq!(
        authority.get_state(proposal.id),
        Some(ChangeSetState::Conflicted)
    );

    // Verify workspace file unmodified
    let content = fs
        .read_file(Path::new("src/compute.rs"), None, None)
        .await
        .unwrap();
    assert_eq!(content, original);
}

// =========================================================================
// 2. Target file missing
// =========================================================================
#[tokio::test]
async fn test_adversarial_target_file_missing() {
    let dir = tempdir().unwrap();
    let ws = dir.path();
    let fs: Arc<dyn FileSystemService> = Arc::new(LocalFileSystemProvider::new(ws).unwrap());

    let hypothesis = ImplementationHypothesis::new("Fix", "Cause", "Change", "Result", "Test");
    let surface = ChangeSurface::new(vec!["src/does_not_exist.rs".to_string()]);

    let proposal = ChangeProposal::new(
        TaskId::new(),
        MissionId::new(),
        hypothesis,
        surface,
        vec![FileMutationProposal::new(
            "src/does_not_exist.rs",
            FileMutationOp::Substring {
                old_content: "foo".to_string(),
                new_content: "bar".to_string(),
            },
            "Modify ghost file",
        )],
    );

    let authority = ChangeAuthority::new();
    let err = authority
        .execute_change_proposal(ws, &proposal, &fs, None, None)
        .await
        .unwrap_err();

    assert!(matches!(
        err,
        ChangeAuthorityError::ReconciliationFailed(..)
    ));
    assert_eq!(
        authority.get_state(proposal.id),
        Some(ChangeSetState::Rejected)
    );
}

// =========================================================================
// 3. Target symbol missing
// =========================================================================
#[tokio::test]
async fn test_adversarial_target_symbol_missing() {
    let dir = tempdir().unwrap();
    let ws = dir.path();
    let fs: Arc<dyn FileSystemService> = Arc::new(LocalFileSystemProvider::new(ws).unwrap());

    fs.write_file(
        Path::new("src/service.rs"),
        b"pub fn execute() -> bool { true }\n",
    )
    .await
    .unwrap();

    let hypothesis = ImplementationHypothesis::new("Fix", "Cause", "Change", "Result", "Test");
    let surface = ChangeSurface::new(vec!["src/service.rs".to_string()]);

    let mut proposal = ChangeProposal::new(
        TaskId::new(),
        MissionId::new(),
        hypothesis,
        surface,
        vec![FileMutationProposal::new(
            "src/service.rs",
            FileMutationOp::Substring {
                old_content: "true".to_string(),
                new_content: "false".to_string(),
            },
            "Toggle flag",
        )],
    );
    proposal.preconditions.push(
        FilePrecondition::exists("src/service.rs")
            .with_expected_symbols(vec!["MissingStruct".to_string()]),
    );

    let authority = ChangeAuthority::new();
    let err = authority
        .execute_change_proposal(ws, &proposal, &fs, None, None)
        .await
        .unwrap_err();

    assert!(matches!(
        err,
        ChangeAuthorityError::ReconciliationFailed(..)
    ));
    assert_eq!(
        authority.get_state(proposal.id),
        Some(ChangeSetState::Rejected)
    );
}

// =========================================================================
// 4. Direct edit of generated file rejected
// =========================================================================
#[tokio::test]
async fn test_adversarial_direct_edit_of_generated_file_rejected() {
    let dir = tempdir().unwrap();
    let ws = dir.path();
    let fs: Arc<dyn FileSystemService> = Arc::new(LocalFileSystemProvider::new(ws).unwrap());

    let gen_content =
        b"// @generated by protobuf compiler. DO NOT EDIT.\npub struct GeneratedMsg;\n";
    fs.write_file(Path::new("src/proto.rs"), gen_content)
        .await
        .unwrap();

    let hypothesis = ImplementationHypothesis::new("Fix", "Cause", "Change", "Result", "Test");
    let surface = ChangeSurface::new(vec!["src/proto.rs".to_string()]);

    let proposal = ChangeProposal::new(
        TaskId::new(),
        MissionId::new(),
        hypothesis,
        surface,
        vec![FileMutationProposal::new(
            "src/proto.rs",
            FileMutationOp::Substring {
                old_content: "GeneratedMsg".to_string(),
                new_content: "ManualMsg".to_string(),
            },
            "Manually modify protobuf stub",
        )],
    );

    let authority = ChangeAuthority::new();
    let err = authority
        .execute_change_proposal(ws, &proposal, &fs, None, None)
        .await
        .unwrap_err();

    assert!(matches!(
        err,
        ChangeAuthorityError::ReconciliationFailed(..)
    ));
    assert_eq!(
        authority.get_state(proposal.id),
        Some(ChangeSetState::Rejected)
    );

    let content = fs
        .read_file(Path::new("src/proto.rs"), None, None)
        .await
        .unwrap();
    assert_eq!(content, gen_content);
}

// =========================================================================
// 5. Accidental scope creep rejected before mutation
// =========================================================================
#[tokio::test]
async fn test_adversarial_scope_creep_rejected() {
    let dir = tempdir().unwrap();
    let ws = dir.path();
    let fs: Arc<dyn FileSystemService> = Arc::new(LocalFileSystemProvider::new(ws).unwrap());

    fs.write_file(Path::new("src/allowed.rs"), b"pub fn a() {}\n")
        .await
        .unwrap();
    fs.write_file(Path::new("src/unauthorized.rs"), b"pub fn b() {}\n")
        .await
        .unwrap();

    let hypothesis = ImplementationHypothesis::new("Fix", "Cause", "Change", "Result", "Test");
    let surface = ChangeSurface::new(vec!["src/allowed.rs".to_string()]);

    let proposal = ChangeProposal::new(
        TaskId::new(),
        MissionId::new(),
        hypothesis,
        surface,
        vec![
            FileMutationProposal::new(
                "src/allowed.rs",
                FileMutationOp::Substring {
                    old_content: "pub fn a() {}".to_string(),
                    new_content: "pub fn a1() {}".to_string(),
                },
                "Update allowed",
            ),
            FileMutationProposal::new(
                "src/unauthorized.rs",
                FileMutationOp::Substring {
                    old_content: "pub fn b() {}".to_string(),
                    new_content: "pub fn b1() {}".to_string(),
                },
                "Update unauthorized",
            ),
        ],
    );

    let authority = ChangeAuthority::new();
    let err = authority
        .execute_change_proposal(ws, &proposal, &fs, None, None)
        .await
        .unwrap_err();

    assert!(matches!(
        err,
        ChangeAuthorityError::ReconciliationFailed(..)
    ));
    assert_eq!(
        authority.get_state(proposal.id),
        Some(ChangeSetState::Rejected)
    );

    // Verify allowed file was never touched
    let a_content = fs
        .read_file(Path::new("src/allowed.rs"), None, None)
        .await
        .unwrap();
    assert_eq!(a_content, b"pub fn a() {}\n");
}

// =========================================================================
// 6. Partial edit failure triggers complete atomic rollback of all files
// =========================================================================
#[tokio::test]
async fn test_adversarial_partial_failure_atomic_rollback() {
    let dir = tempdir().unwrap();
    let ws = dir.path();
    let fs: Arc<dyn FileSystemService> = Arc::new(LocalFileSystemProvider::new(ws).unwrap());

    let original_a = b"pub fn alpha() -> u32 { 10 }\n";
    let original_b = b"pub fn beta() -> u32 { 20 }\n";

    fs.write_file(Path::new("src/a.rs"), original_a)
        .await
        .unwrap();
    fs.write_file(Path::new("src/b.rs"), original_b)
        .await
        .unwrap();

    let hypothesis = ImplementationHypothesis::new("Fix", "Cause", "Change", "Result", "Test");
    let surface = ChangeSurface::new(vec!["src/a.rs".to_string(), "src/b.rs".to_string()]);

    let proposal = ChangeProposal::new(
        TaskId::new(),
        MissionId::new(),
        hypothesis,
        surface,
        vec![
            // First edit succeeds
            FileMutationProposal::new(
                "src/a.rs",
                FileMutationOp::Substring {
                    old_content: "10".to_string(),
                    new_content: "999".to_string(),
                },
                "Update a",
            ),
            // Second edit fails (target substring does not exist!)
            FileMutationProposal::new(
                "src/b.rs",
                FileMutationOp::Substring {
                    old_content: "NON_EXISTENT_SUBSTRING".to_string(),
                    new_content: "something".to_string(),
                },
                "Failing edit on b",
            ),
        ],
    );

    let authority = ChangeAuthority::new();
    let err = authority
        .execute_change_proposal(ws, &proposal, &fs, None, None)
        .await
        .unwrap_err();

    assert!(matches!(err, ChangeAuthorityError::ApplyFailed(..)));
    assert_eq!(
        authority.get_state(proposal.id),
        Some(ChangeSetState::RolledBack)
    );

    // Atomicity guarantee: src/a.rs MUST have rolled back to original!
    let current_a = fs
        .read_file(Path::new("src/a.rs"), None, None)
        .await
        .unwrap();
    assert_eq!(
        current_a, original_a,
        "src/a.rs must be rolled back completely after partial failure"
    );

    let current_b = fs
        .read_file(Path::new("src/b.rs"), None, None)
        .await
        .unwrap();
    assert_eq!(current_b, original_b);
}

// =========================================================================
// 7. Diff review rejects fake implementation (todo!() / unimplemented!())
// =========================================================================
#[tokio::test]
async fn test_adversarial_diff_review_rejects_fake_implementation() {
    let dir = tempdir().unwrap();
    let ws = dir.path();
    let fs: Arc<dyn FileSystemService> = Arc::new(LocalFileSystemProvider::new(ws).unwrap());

    let original = b"pub fn authenticate() -> bool {\n    false\n}\n";
    fs.write_file(Path::new("src/auth.rs"), original)
        .await
        .unwrap();

    let hypothesis = ImplementationHypothesis::new("Fix", "Cause", "Change", "Result", "Test");
    let surface = ChangeSurface::new(vec!["src/auth.rs".to_string()]);

    let proposal = ChangeProposal::new(
        TaskId::new(),
        MissionId::new(),
        hypothesis,
        surface,
        vec![FileMutationProposal::new(
            "src/auth.rs",
            FileMutationOp::Substring {
                old_content: "false".to_string(),
                new_content: "unimplemented!(\"later\")".to_string(), // Fake stub!
            },
            "Add stub",
        )],
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

    // Workspace rolled back
    let content = fs
        .read_file(Path::new("src/auth.rs"), None, None)
        .await
        .unwrap();
    assert_eq!(content, original);
}

// =========================================================================
// 8. Diff review rejects weakened test assertions
// =========================================================================
#[tokio::test]
async fn test_adversarial_diff_review_rejects_weakened_test_assertion() {
    let dir = tempdir().unwrap();
    let ws = dir.path();
    let fs: Arc<dyn FileSystemService> = Arc::new(LocalFileSystemProvider::new(ws).unwrap());

    let original = b"#[test]\nfn test_check() {\n    assert_eq!(run(), 42);\n}\n";
    fs.write_file(Path::new("tests/check_test.rs"), original)
        .await
        .unwrap();

    let hypothesis = ImplementationHypothesis::new("Fix", "Cause", "Change", "Result", "Test");
    let surface = ChangeSurface::new(vec!["tests/check_test.rs".to_string()]);

    // Attempting to comment out assertion to fake test pass
    let proposal = ChangeProposal::new(
        TaskId::new(),
        MissionId::new(),
        hypothesis,
        surface,
        vec![FileMutationProposal::new(
            "tests/check_test.rs",
            FileMutationOp::Substring {
                old_content: "    assert_eq!(run(), 42);".to_string(),
                new_content: "    // assert_eq!(run(), 42);".to_string(),
            },
            "Comment out failing assertion",
        )],
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

    let content = fs
        .read_file(Path::new("tests/check_test.rs"), None, None)
        .await
        .unwrap();
    assert_eq!(content, original);
}

// =========================================================================
// 9. Concurrent task surface conflict detected and rejected
// =========================================================================
#[tokio::test]
async fn test_adversarial_concurrent_surface_conflict() {
    let dir = tempdir().unwrap();
    let ws = dir.path();
    let fs: Arc<dyn FileSystemService> = Arc::new(LocalFileSystemProvider::new(ws).unwrap());

    let original = b"pub fn shared() {}\n";
    fs.write_file(Path::new("src/shared.rs"), original)
        .await
        .unwrap();

    let authority = ChangeAuthority::new();
    let task_1 = TaskId::new();
    let task_2 = TaskId::new();

    // Task 1 reserves src/shared.rs
    authority
        .reserve_surface(
            task_1,
            &ChangeSurface::new(vec!["src/shared.rs".to_string()]),
        )
        .unwrap();

    // Task 2 attempts to propose and execute mutation on src/shared.rs
    let proposal_2 = ChangeProposal::new(
        task_2,
        MissionId::new(),
        ImplementationHypothesis::new("Fix", "Cause", "Change", "Result", "Test"),
        ChangeSurface::new(vec!["src/shared.rs".to_string()]),
        vec![FileMutationProposal::new(
            "src/shared.rs",
            FileMutationOp::Substring {
                old_content: "shared".to_string(),
                new_content: "shared_2".to_string(),
            },
            "Task 2 mutation",
        )],
    );

    let err = authority
        .execute_change_proposal(ws, &proposal_2, &fs, None, None)
        .await
        .unwrap_err();

    let is_concurrency_conflict = match &err {
        ChangeAuthorityError::ConcurrencyConflict(..) => true,
        ChangeAuthorityError::ReconciliationFailed(_, report) => {
            report.violations.iter().any(|v| {
                matches!(
                    v,
                    m31a::kernel::change::ReconciliationViolation::ConcurrentTaskConflict { .. }
                )
            })
        }
        _ => false,
    };
    assert!(is_concurrency_conflict);
    assert_eq!(
        authority.get_state(proposal_2.id),
        Some(ChangeSetState::Conflicted)
    );

    // Task 1 finishes, releases surface
    authority.release_surface(task_1);

    // Now Task 2 can succeed!
    let outcome = authority
        .execute_change_proposal(ws, &proposal_2, &fs, None, None)
        .await
        .unwrap();
    assert_eq!(outcome.state, ChangeSetState::Accepted);
}
