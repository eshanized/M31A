//! Explicit capability model and operation authorization (`m31a-trust::capability`).

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Auditable operation classification representing system capabilities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationCapability {
    /// Safe UI rendering, navigation, and display (help, theme, modal viewing)
    HarmlessUI,
    /// Safe read-only inspection of repository structure/metadata (status, procs, git status)
    WorkspaceMetadataRead,
    /// Reading workspace file contents
    WorkspaceFileRead,
    /// Creating, editing, or deleting files in workspace
    WorkspaceMutation,
    /// Applying diffs/patches to workspace files
    PatchApplication,
    /// Executing shell scripts, commands, or spawned subprocesses
    ShellExecution,
    /// Mutating Git repository state (commit, branch, reset, checkout)
    GitMutation,
    /// Outbound network/HTTP connectivity
    NetworkAccess,
    /// Accessing credentials, API keys, or environment secrets
    CredentialAccess,
    /// Operations requiring or sensitive to sandbox containment
    SandboxSensitive,
    /// Modifying or inspecting workspace trust policies
    TrustManagement,
}

impl OperationCapability {
    /// Returns true if this capability requires workspace trust authorization.
    pub fn requires_workspace_trust(&self) -> bool {
        match self {
            Self::HarmlessUI | Self::TrustManagement => false,
            Self::WorkspaceMetadataRead => false,
            Self::WorkspaceFileRead
            | Self::WorkspaceMutation
            | Self::PatchApplication
            | Self::ShellExecution
            | Self::GitMutation
            | Self::NetworkAccess
            | Self::CredentialAccess
            | Self::SandboxSensitive => true,
        }
    }

    /// User-friendly label for auditing and notifications.
    pub fn label(&self) -> &'static str {
        match self {
            Self::HarmlessUI => "UI Operation",
            Self::WorkspaceMetadataRead => "Metadata Read",
            Self::WorkspaceFileRead => "File Read",
            Self::WorkspaceMutation => "Workspace Mutation",
            Self::PatchApplication => "Patch Application",
            Self::ShellExecution => "Shell Execution",
            Self::GitMutation => "Git Mutation",
            Self::NetworkAccess => "Network Access",
            Self::CredentialAccess => "Credential Access",
            Self::SandboxSensitive => "Sandbox Operation",
            Self::TrustManagement => "Trust Management",
        }
    }
}

/// Structured outcome of a capability authorization check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthorizationOutcome {
    /// Operation is explicitly permitted.
    Allowed,
    /// Operation requires user approval or interactive trust confirmation.
    RequiresConfirmation {
        path: PathBuf,
        capability: OperationCapability,
        reason: String,
    },
    /// Operation is strictly denied by security trust policy.
    Denied {
        path: PathBuf,
        capability: OperationCapability,
        reason: String,
    },
}

impl AuthorizationOutcome {
    pub fn is_allowed(&self) -> bool {
        matches!(self, Self::Allowed)
    }
}
