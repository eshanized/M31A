//! Typed reference to a versioned prompt contract.

use crate::prompt::error::PromptError;
use serde::{Deserialize, Serialize};

/// Typed purpose of a prompt reference: role-bound identity vs. stage/workflow
/// execution bindings.
///
/// `agent.*` contracts are **role prompts** (behavioral identity for an
/// [`AgentRole`](crate::state_machine::agent::AgentRole)); `execution.*`,
/// `verification.*`, `recovery.*`, `planning.*`, and `genesis.*` contracts are
/// **stage prompts** (workflow/task-selected execution bindings). The two
/// namespaces MUST NOT be flattened: a workflow step must be able to specify
/// the exact prompt contract it intends to execute without that reference
/// being overwritten by the default role prompt.
///
/// The purpose is advisory metadata carried alongside the exact `(id,
/// version)` binding — resolution, compilation, and provenance always use the
/// exact id/version. Selection precedence
/// (explicit task/workflow > session > role default > failure) is enforced by
/// the context compiler, never by reinterpreting the purpose.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PromptPurpose {
    /// Behavioral identity contract for an agent role (`agent.*`).
    #[default]
    AgentRole,
    /// Workflow/task-selected execution binding (`execution.*` and explicit
    /// per-step workflow bindings).
    WorkflowExecution,
    /// Task decomposition and revision (`planning.*`).
    Planning,
    /// Independent review and quality-gate verdicts (`verification.*`).
    Verification,
    /// Diagnosis and closed-loop recovery (`recovery.*`).
    Recovery,
    /// Discovery, research, charter, and roadmap synthesis (`genesis.*`).
    Genesis,
    /// Immutable runtime safety invariants (`core.*`, `runtime.*`).
    SystemSafety,
    /// Lower-trust reusable procedural guidance (`skill.*`). Never an
    /// authoritative system prompt; injected only as untrusted L5 context.
    SkillGuidance,
}

impl PromptPurpose {
    /// Infer the purpose from a contract id prefix. Used when a reference is
    /// constructed without an explicit purpose (e.g. deserialized legacy
    /// records, `agent.{role}` role defaults).
    pub fn infer_from_id(id: &str) -> Self {
        if id.starts_with("core.") || id.starts_with("runtime.") {
            Self::SystemSafety
        } else if id.starts_with("agent.") {
            Self::AgentRole
        } else if id.starts_with("planning.") {
            Self::Planning
        } else if id.starts_with("execution.") {
            Self::WorkflowExecution
        } else if id.starts_with("verification.") {
            Self::Verification
        } else if id.starts_with("recovery.") {
            Self::Recovery
        } else if id.starts_with("genesis.") {
            Self::Genesis
        } else if id.starts_with("skill.") {
            Self::SkillGuidance
        } else {
            Self::WorkflowExecution
        }
    }

    /// Canonical string identifier.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::AgentRole => "agent_role",
            Self::WorkflowExecution => "workflow_execution",
            Self::Planning => "planning",
            Self::Verification => "verification",
            Self::Recovery => "recovery",
            Self::Genesis => "genesis",
            Self::SystemSafety => "system_safety",
            Self::SkillGuidance => "skill_guidance",
        }
    }
}

impl std::fmt::Display for PromptPurpose {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Typed reference to a versioned prompt contract (e.g. `genesis.discovery.v1` or `discovery` + version 1).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PromptReference {
    /// Logical identifier of the prompt contract (e.g. "genesis.discovery").
    pub id: String,
    /// Exact version number of the contract.
    pub version: u32,
    /// Why this prompt was selected (role identity vs. workflow/stage
    /// execution binding). Advisory only: resolution always uses
    /// the exact `(id, version)`.
    #[serde(default)]
    pub purpose: PromptPurpose,
}

impl PromptReference {
    /// Create a new exact prompt reference, inferring purpose from the id prefix.
    pub fn new(id: impl Into<String>, version: u32) -> Self {
        let id_str = id.into();
        let purpose = PromptPurpose::infer_from_id(&id_str);
        Self {
            id: id_str,
            version,
            purpose,
        }
    }

    /// Create a new exact prompt reference with an explicit purpose.
    pub fn with_purpose(id: impl Into<String>, version: u32, purpose: PromptPurpose) -> Self {
        Self {
            id: id.into(),
            version,
            purpose,
        }
    }

    /// Return a copy of this reference with an explicit purpose attached.
    pub fn with_explicit_purpose(mut self, purpose: PromptPurpose) -> Self {
        self.purpose = purpose;
        self
    }

    /// Return the canonical default prompt reference for a given role id.
    ///
    /// Resolves through the role registry: registered roles use their
    /// declared prompt contract. Unregistered ids follow the `agent.{id}` v1
    /// naming convention without granting existence — prompt existence is
    /// enforced by the catalog and role existence by the registry at
    /// validation boundaries (planning, dispatch, workflow compilation).
    pub fn for_role(role: crate::state_machine::agent::AgentRole) -> Self {
        let registry = crate::agent::registry::RoleRegistry::global();
        let guard = registry.read().expect("role registry lock poisoned");
        guard.prompt_reference_for(&role)
    }

    /// Parse a prompt reference string.
    ///
    /// Accepts formats:
    /// - `name.v1` -> `id: "name", version: 1`
    /// - `genesis.discovery.v2` -> `id: "genesis.discovery", version: 2`
    /// - `discovery:v1` -> `id: "discovery", version: 1`
    /// - `discovery:1` -> `id: "discovery", version: 1`
    /// - `discovery@1` -> `id: "discovery", version: 1`
    pub fn parse(s: &str) -> Result<Self, PromptError> {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            return Err(PromptError::PromptInvalid {
                id: "<empty>".to_string(),
                version: 0,
                reason: "prompt reference string cannot be empty".to_string(),
            });
        }

        // Try colon separator: "name:v1" or "name:1"
        if let Some((id_part, ver_part)) = trimmed.split_once(':') {
            let ver_clean = ver_part.trim_start_matches('v').trim_start_matches('V');
            if let Ok(ver) = ver_clean.parse::<u32>()
                && ver > 0
            {
                return Ok(Self::new(id_part.trim(), ver));
            }
        }

        // Try @ separator: "name@1" or "name@v1"
        if let Some((id_part, ver_part)) = trimmed.split_once('@') {
            let ver_clean = ver_part.trim_start_matches('v').trim_start_matches('V');
            if let Ok(ver) = ver_clean.parse::<u32>()
                && ver > 0
            {
                return Ok(Self::new(id_part.trim(), ver));
            }
        }

        // Try dot separator ending in .v<N>: "genesis.discovery.v1"
        if let Some(idx) = trimmed.rfind(".v") {
            let (id_part, ver_part) = trimmed.split_at(idx);
            let ver_digits = &ver_part[2..];
            if let Ok(ver) = ver_digits.parse::<u32>()
                && ver > 0
                && !id_part.trim().is_empty()
            {
                return Ok(Self::new(id_part.trim(), ver));
            }
        }

        // If no version suffix is present, fail explicitly: exact versioning required
        Err(PromptError::PromptInvalid {
            id: trimmed.to_string(),
            version: 0,
            reason: format!(
                "prompt reference '{}' must include an exact version (e.g. '{}.v1' or '{}:1')",
                trimmed, trimmed, trimmed
            ),
        })
    }

    /// Lenient parse for workflow step bindings that may omit the version
    /// (e.g. `"execution.implementer"`, `"genesis/research_stack"`).
    ///
    /// Slash separators are normalized to dots. A missing version defaults to
    /// v1; the catalog's `resolve_canonical` upgrades to the canonical v2
    /// generation where one exists, so the default never pins production to
    /// a deprecated generation. Fails closed on empty input only.
    pub fn parse_lenient(s: &str) -> Result<Self, PromptError> {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            return Err(PromptError::PromptInvalid {
                id: "<empty>".to_string(),
                version: 0,
                reason: "prompt reference string cannot be empty".to_string(),
            });
        }
        let normalized = trimmed.replace('/', ".");
        if let Ok(exact) = Self::parse(&normalized) {
            return Ok(exact);
        }
        // No version suffix: strip a trailing bare ".v" fragment if present,
        // then bind v1 (catalog canonicalization upgrades to v2).
        let id_part = normalized.strip_suffix(".v").unwrap_or(&normalized).trim();
        if id_part.is_empty() {
            return Err(PromptError::PromptInvalid {
                id: trimmed.to_string(),
                version: 0,
                reason: format!("prompt reference '{}' must name a prompt contract", trimmed),
            });
        }
        Ok(Self::new(id_part, 1))
    }
}

impl std::fmt::Display for PromptReference {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.v{}", self.id, self.version)
    }
}
