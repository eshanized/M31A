//! Approval and execution security presets.
//!
//! Pairs approval policies with sandbox permission profiles into intuitive,
//! high-level security presets (ReadOnly, Standard, Permissive, Airgapped, DangerFullAccess).

use m31a_core::{ApprovalPolicy, SandboxPolicy};
use serde::{Deserialize, Serialize};

/// High-level preset classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ApprovalPresetKind {
    ReadOnly,
    Standard,
    Permissive,
    Airgapped,
    DangerFullAccess,
}

/// A structured preset pairing an approval policy with a sandbox isolation profile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApprovalPreset {
    /// Stable string identifier (e.g. "read-only", "standard", "permissive", "airgapped", "danger-full-access").
    pub id: &'static str,
    /// Human-friendly display label.
    pub label: &'static str,
    /// Detailed description of security posture and operational permissions.
    pub description: &'static str,
    /// Preset kind classification.
    pub kind: ApprovalPresetKind,
    /// Approval policy governing model tool calls and command execution.
    pub approval: ApprovalPolicy,
    /// Sandbox isolation profile.
    pub sandbox: SandboxPolicy,
    /// Whether outbound network access is allowed.
    pub network_allowed: bool,
    /// Whether workspace file modifications are permitted.
    pub workspace_write_allowed: bool,
}

impl ApprovalPreset {
    /// Return the list of built-in security presets.
    pub fn builtin_presets() -> Vec<Self> {
        vec![
            Self {
                id: "read-only",
                label: "Read Only",
                description: "M31A can inspect and read files in the workspace. Approval is strictly required to modify files or invoke network tools.",
                kind: ApprovalPresetKind::ReadOnly,
                approval: ApprovalPolicy::OnRequest,
                sandbox: SandboxPolicy::ReadOnly,
                network_allowed: false,
                workspace_write_allowed: false,
            },
            Self {
                id: "standard",
                label: "Standard (Agent Mode)",
                description: "M31A can read and edit workspace files and run build/test commands inside the sandbox container. Approval is required for external internet or out-of-workspace operations.",
                kind: ApprovalPresetKind::Standard,
                approval: ApprovalPolicy::OnRequest,
                sandbox: SandboxPolicy::WorkspaceWrite,
                network_allowed: false,
                workspace_write_allowed: true,
            },
            Self {
                id: "permissive",
                label: "Permissive (Guardian Auto-Review)",
                description: "M31A can read, write, and execute development commands. Approvals are routed through Guardian auto-review without human prompt interruptions unless high risk is detected.",
                kind: ApprovalPresetKind::Permissive,
                approval: ApprovalPolicy::ApproveForMe,
                sandbox: SandboxPolicy::WorkspaceWrite,
                network_allowed: true,
                workspace_write_allowed: true,
            },
            Self {
                id: "airgapped",
                label: "Airgapped (Strictly Offline)",
                description: "M31A operates in a strictly disconnected environment. All outbound network sockets and metadata endpoints are blocked; sandbox enforces local-only execution.",
                kind: ApprovalPresetKind::Airgapped,
                approval: ApprovalPolicy::OnRequest,
                sandbox: SandboxPolicy::WorkspaceWrite,
                network_allowed: false,
                workspace_write_allowed: true,
            },
            Self {
                id: "danger-full-access",
                label: "Danger Full Access",
                description: "M31A executes unconstrained outside the Bubblewrap sandbox with approvals disabled. Exercise extreme caution; recommended only for isolated VM containers.",
                kind: ApprovalPresetKind::DangerFullAccess,
                approval: ApprovalPolicy::Never,
                sandbox: SandboxPolicy::DangerFullAccess,
                network_allowed: true,
                workspace_write_allowed: true,
            },
        ]
    }

    /// Resolve preset from string identifier (case-insensitive, kebab or snake case).
    pub fn from_id(id_str: &str) -> Option<Self> {
        let normalized = id_str.to_ascii_lowercase().replace('_', "-");
        Self::builtin_presets()
            .into_iter()
            .find(|p| p.id == normalized || p.id.replace('-', "") == normalized.replace('-', ""))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builtin_presets_catalog() {
        let presets = ApprovalPreset::builtin_presets();
        assert_eq!(presets.len(), 5);

        let standard = ApprovalPreset::from_id("standard").expect("standard preset");
        assert_eq!(standard.kind, ApprovalPresetKind::Standard);
        assert_eq!(standard.sandbox, SandboxPolicy::WorkspaceWrite);
        assert_eq!(standard.approval, ApprovalPolicy::OnRequest);
        assert!(standard.workspace_write_allowed);

        let read_only = ApprovalPreset::from_id("read-only").expect("read-only preset");
        assert_eq!(read_only.kind, ApprovalPresetKind::ReadOnly);
        assert_eq!(read_only.sandbox, SandboxPolicy::ReadOnly);
        assert!(!read_only.workspace_write_allowed);

        let airgapped = ApprovalPreset::from_id("airgapped").expect("airgapped preset");
        assert_eq!(airgapped.kind, ApprovalPresetKind::Airgapped);
        assert!(!airgapped.network_allowed);

        let permissive = ApprovalPreset::from_id("permissive").expect("permissive preset");
        assert_eq!(permissive.approval, ApprovalPolicy::ApproveForMe);
    }

    #[test]
    fn test_from_id_normalization() {
        assert!(ApprovalPreset::from_id("read_only").is_some());
        assert!(ApprovalPreset::from_id("DANGER-FULL-ACCESS").is_some());
        assert!(ApprovalPreset::from_id("unknown-preset").is_none());
    }
}
