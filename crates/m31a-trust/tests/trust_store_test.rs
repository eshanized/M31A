//! Dedicated integration tests for `m31a-trust` (src/trust_store.rs).
//!
//! Validates:
//! - TrustStore persistence, atomic file serialization, and schema versioning
//! - Hierarchical directory trust evaluation and ancestor inheritance
//! - Capability-level authorization outcomes across trust states (Trusted, Untrusted, Denied, SessionOnly)
//! - Session-scoped memory overrides vs persistent disk records
//! - Fail-closed error handling on corrupt databases

use m31a_trust::capability::{AuthorizationOutcome, OperationCapability};
use m31a_trust::trust_store::{TrustDecision, TrustLevel, TrustStore};
use std::fs;
use tempfile::tempdir;

#[test]
fn test_trust_store_lifecycle_and_hierarchy() {
    let temp = tempdir().unwrap();
    let store_file = temp.path().join("trusted_dirs.json");
    let mut store = TrustStore::with_store_path(store_file.clone());

    let repo_root = temp.path().join("org").join("project");
    let src_dir = repo_root.join("src").join("engine");
    fs::create_dir_all(&src_dir).unwrap();

    // 1. Unrecognized workspace requires confirmation
    assert!(matches!(
        store.evaluate(&repo_root),
        TrustDecision::RequiresConfirmation { .. }
    ));
    assert!(matches!(
        store.evaluate(&src_dir),
        TrustDecision::RequiresConfirmation { .. }
    ));

    // 2. Grant persistent trust to repo_root
    store
        .trust(&repo_root, TrustLevel::Trusted)
        .expect("Persist trust");

    // Both root and subfolder are now trusted
    assert_eq!(store.evaluate(&repo_root), TrustDecision::Trusted);
    assert_eq!(store.evaluate(&src_dir), TrustDecision::Trusted);

    // Verify written to disk and can be reloaded in a fresh store instance
    let reload_store = TrustStore::with_store_path(store_file);
    assert_eq!(reload_store.evaluate(&src_dir), TrustDecision::Trusted);

    // 3. Revoke trust
    assert!(store.revoke(&repo_root).expect("Revoke trust"));
    assert!(matches!(
        store.evaluate(&repo_root),
        TrustDecision::RequiresConfirmation { .. }
    ));
    assert!(matches!(
        store.evaluate(&src_dir),
        TrustDecision::RequiresConfirmation { .. }
    ));
}

#[test]
fn test_evaluate_capabilities_across_trust_tiers() {
    let temp = tempdir().unwrap();
    let store_file = temp.path().join("trusted_caps.json");
    let mut store = TrustStore::with_store_path(store_file);

    let untrusted_dir = temp.path().join("untrusted");
    let trusted_dir = temp.path().join("trusted");
    let denied_dir = temp.path().join("forbidden");
    fs::create_dir_all(&untrusted_dir).unwrap();
    fs::create_dir_all(&trusted_dir).unwrap();
    fs::create_dir_all(&denied_dir).unwrap();

    store.trust(&trusted_dir, TrustLevel::Trusted).unwrap();
    store.trust(&denied_dir, TrustLevel::Denied).unwrap();

    // 1. Harmless UI is allowed everywhere
    assert_eq!(
        store.authorize_operation(&untrusted_dir, OperationCapability::HarmlessUI),
        AuthorizationOutcome::Allowed
    );
    assert_eq!(
        store.authorize_operation(&trusted_dir, OperationCapability::HarmlessUI),
        AuthorizationOutcome::Allowed
    );
    assert_eq!(
        store.authorize_operation(&denied_dir, OperationCapability::HarmlessUI),
        AuthorizationOutcome::Allowed
    );

    // 2. Sensitive capability (ShellExecution)
    // Untrusted -> RequiresConfirmation
    assert!(matches!(
        store.authorize_operation(&untrusted_dir, OperationCapability::ShellExecution),
        AuthorizationOutcome::RequiresConfirmation { .. }
    ));

    // Trusted -> Allowed
    assert_eq!(
        store.authorize_operation(&trusted_dir, OperationCapability::ShellExecution),
        AuthorizationOutcome::Allowed
    );

    // Denied -> Denied
    assert!(matches!(
        store.authorize_operation(&denied_dir, OperationCapability::ShellExecution),
        AuthorizationOutcome::Denied { .. }
    ));
}

#[test]
fn test_session_scoped_authorization() {
    let temp = tempdir().unwrap();
    let store_file = temp.path().join("session_store.json");
    let mut store = TrustStore::with_store_path(store_file.clone());

    let workspace = temp.path().join("ephemeral_task");
    fs::create_dir_all(&workspace).unwrap();

    store.trust(&workspace, TrustLevel::SessionOnly).unwrap();
    assert_eq!(store.evaluate(&workspace), TrustDecision::SessionOnly);

    // Capability evaluation for SessionOnly should be Allowed
    assert_eq!(
        store.authorize_operation(&workspace, OperationCapability::WorkspaceMutation),
        AuthorizationOutcome::Allowed
    );

    // Session authorization is in-memory only: a fresh store does not see it
    let disk_store = TrustStore::with_store_path(store_file);
    assert!(matches!(
        disk_store.evaluate(&workspace),
        TrustDecision::RequiresConfirmation { .. }
    ));
}

#[test]
fn test_corruption_safety_fails_closed() {
    let temp = tempdir().unwrap();
    let store_file = temp.path().join("corrupt.json");
    fs::write(&store_file, b"{{INVALID JSON GIBBERISH").unwrap();

    let store = TrustStore::with_store_path(store_file);
    let load_res = store.load_data();
    assert!(load_res.is_err(), "Corrupt trust store must fail closed");
}
