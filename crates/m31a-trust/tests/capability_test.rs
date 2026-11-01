//! Dedicated integration tests for `m31a-trust` (src/capability.rs).
//!
//! Validates:
//! - Capability classification and user-friendly labeling
//! - Trust requirement boundary predicates (`requires_workspace_trust`)
//! - Authorization outcome variants (`Allowed`, `RequiresConfirmation`, `Denied`)
//! - Serde JSON snake_case serialization fidelity

use m31a_trust::capability::{AuthorizationOutcome, OperationCapability};
use std::path::PathBuf;

#[test]
fn test_operation_capability_trust_requirements() {
    // Unrestricted / trust-free capabilities
    assert!(!OperationCapability::HarmlessUI.requires_workspace_trust());
    assert!(!OperationCapability::WorkspaceMetadataRead.requires_workspace_trust());
    assert!(!OperationCapability::TrustManagement.requires_workspace_trust());

    // Security-sensitive capabilities requiring trust confirmation
    let sensitive_caps = [
        OperationCapability::WorkspaceFileRead,
        OperationCapability::WorkspaceMutation,
        OperationCapability::PatchApplication,
        OperationCapability::ShellExecution,
        OperationCapability::GitMutation,
        OperationCapability::NetworkAccess,
        OperationCapability::CredentialAccess,
        OperationCapability::SandboxSensitive,
    ];

    for cap in sensitive_caps {
        assert!(
            cap.requires_workspace_trust(),
            "Capability {:?} must require workspace trust",
            cap
        );
    }
}

#[test]
fn test_operation_capability_labels() {
    assert_eq!(OperationCapability::HarmlessUI.label(), "UI Operation");
    assert_eq!(
        OperationCapability::WorkspaceMetadataRead.label(),
        "Metadata Read"
    );
    assert_eq!(OperationCapability::WorkspaceFileRead.label(), "File Read");
    assert_eq!(
        OperationCapability::WorkspaceMutation.label(),
        "Workspace Mutation"
    );
    assert_eq!(
        OperationCapability::PatchApplication.label(),
        "Patch Application"
    );
    assert_eq!(
        OperationCapability::ShellExecution.label(),
        "Shell Execution"
    );
    assert_eq!(OperationCapability::GitMutation.label(), "Git Mutation");
    assert_eq!(OperationCapability::NetworkAccess.label(), "Network Access");
    assert_eq!(
        OperationCapability::CredentialAccess.label(),
        "Credential Access"
    );
    assert_eq!(
        OperationCapability::SandboxSensitive.label(),
        "Sandbox Operation"
    );
    assert_eq!(
        OperationCapability::TrustManagement.label(),
        "Trust Management"
    );
}

#[test]
fn test_authorization_outcome_predicates() {
    let allowed = AuthorizationOutcome::Allowed;
    assert!(allowed.is_allowed());

    let confirmation = AuthorizationOutcome::RequiresConfirmation {
        path: PathBuf::from("/workspace"),
        capability: OperationCapability::ShellExecution,
        reason: "Untrusted directory".to_string(),
    };
    assert!(!confirmation.is_allowed());

    let denied = AuthorizationOutcome::Denied {
        path: PathBuf::from("/workspace"),
        capability: OperationCapability::ShellExecution,
        reason: "Explicitly forbidden".to_string(),
    };
    assert!(!denied.is_allowed());
}

#[test]
fn test_capability_serde_snake_case() {
    let cap = OperationCapability::ShellExecution;
    let json = serde_json::to_string(&cap).unwrap();
    assert_eq!(json, "\"shell_execution\"");

    let deserialized: OperationCapability = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, cap);
}
