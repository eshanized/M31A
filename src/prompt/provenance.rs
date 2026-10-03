//! Prompt provenance, telemetry, and observability models (Architecture Section 24 & Stage 7).
//!
//! Enforces complete auditability and reproducibility for every model-facing prompt execution
//! without introducing new runtime authority.
//!
//! # Core Distinctions
//! 1. **Contract Hash:** SHA-256 hash of the unrendered template in [`crate::prompt::PromptContract`].
//! 2. **Effective Prompt Hash:** Cryptographic composite SHA-256 hash of the fully compiled,
//!    strategy-adapted, parameter-bound, and trust-delimited prompt.
//! 3. **Context Digest:** Deterministic cryptographic SHA-256 digest of the structured runtime context manifest.
//!
//! # Privacy & Security Guarantees
//! - Zero raw secrets or credentials recorded in provenance metadata.
//! - Raw prompt bodies are never stored in telemetry records.
//! - Zero chain-of-thought or hidden reasoning prompts recorded.

use crate::context::envelope::TrustLevel;
use crate::prompt::composer::PromptLayerKind;
use crate::prompt::context::MissionStage;
use crate::prompt::strategy::PromptStrategy;
use crate::state_machine::agent::AgentRole;
use crate::telemetry::redactor::SecretRedactor;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Origin source kind of a prompt contract.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum PromptSourceKind {
    /// Built-in prompt contract compiled directly into the binary.
    #[default]
    Builtin,
    /// Project-level override located in `<workspace>/prompts/*.toml`.
    ProjectOverride,
    /// Workspace-specific override located in `<workspace>/.m31a/prompts/*.toml`.
    WorkspaceOverride,
}

impl std::fmt::Display for PromptSourceKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Builtin => write!(f, "builtin"),
            Self::ProjectOverride => write!(f, "project_override"),
            Self::WorkspaceOverride => write!(f, "workspace_override"),
        }
    }
}

/// Lightweight, portable prompt invocation provenance record (Spec Section 24).
///
/// Designed to attach with zero schema overhead to:
/// - `AgentStepRecord` in `src/agent/runner.rs`
/// - `StepProvenance` in `src/workflow/provenance.rs`
/// - `ModelInvocationRecord` in `src/model/persistence/invocation.rs`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PromptInvocationProvenance {
    /// Canonical prompt contract identifier.
    pub prompt_id: String,
    /// Prompt contract version number.
    pub prompt_version: u32,
    /// Canonical SHA-256 hash of the unrendered prompt contract template.
    pub prompt_content_hash: String,
    /// Effective prompt strategy applied during compilation.
    pub prompt_strategy: PromptStrategy,
    /// Cryptographic SHA-256 hash of the compiled prompt (assembled text + strategy + parameters).
    pub effective_prompt_hash: String,
    /// Target model identifier (or "default").
    pub model_id: String,
    /// Target provider name (e.g. "nvidia", "anthropic", "mock").
    pub provider: String,
    /// Cryptographic SHA-256 digest of the input runtime context manifest.
    pub context_digest: String,
    /// Expected output format or contract identifier (or "unspecified").
    pub output_contract_id: String,
    /// UTC timestamp of prompt compilation.
    pub timestamp: DateTime<Utc>,
    /// Origin source kind of the active prompt contract.
    #[serde(default)]
    pub source_kind: PromptSourceKind,
    /// Mission identity this prompt was compiled for.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mission_id: Option<String>,
    /// Task identity this prompt was compiled for.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_id: Option<String>,
    /// Agent identity this prompt was compiled for.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<String>,
}

impl PromptInvocationProvenance {
    /// Construct a new PromptInvocationProvenance record.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        prompt_id: impl Into<String>,
        prompt_version: u32,
        prompt_content_hash: impl Into<String>,
        prompt_strategy: PromptStrategy,
        effective_prompt_hash: impl Into<String>,
        model_id: impl Into<String>,
        provider: impl Into<String>,
        context_digest: impl Into<String>,
        output_contract_id: impl Into<String>,
        timestamp: DateTime<Utc>,
    ) -> Self {
        Self {
            prompt_id: prompt_id.into(),
            prompt_version,
            prompt_content_hash: prompt_content_hash.into(),
            prompt_strategy,
            effective_prompt_hash: effective_prompt_hash.into(),
            model_id: model_id.into(),
            provider: provider.into(),
            context_digest: context_digest.into(),
            output_contract_id: output_contract_id.into(),
            timestamp,
            source_kind: PromptSourceKind::Builtin,
            mission_id: None,
            task_id: None,
            agent_id: None,
        }
    }

    /// Set the source origin kind for this provenance record.
    pub fn with_source_kind(mut self, source_kind: PromptSourceKind) -> Self {
        self.source_kind = source_kind;
        self
    }

    /// Attach the mission identity this prompt was compiled for.
    pub fn with_mission_id(mut self, mission_id: impl Into<String>) -> Self {
        self.mission_id = Some(mission_id.into());
        self
    }

    /// Attach the task identity this prompt was compiled for.
    pub fn with_task_id(mut self, task_id: impl Into<String>) -> Self {
        self.task_id = Some(task_id.into());
        self
    }

    /// Attach the agent identity this prompt was compiled for.
    pub fn with_agent_id(mut self, agent_id: impl Into<String>) -> Self {
        self.agent_id = Some(agent_id.into());
        self
    }

    /// Produce a sanitized copy with any inadvertent secret tokens scrubbed via SecretRedactor.
    pub fn sanitized(&self, redactor: &SecretRedactor) -> Self {
        Self {
            prompt_id: redactor.redact_string(&self.prompt_id),
            prompt_version: self.prompt_version,
            prompt_content_hash: self.prompt_content_hash.clone(),
            prompt_strategy: self.prompt_strategy,
            effective_prompt_hash: self.effective_prompt_hash.clone(),
            model_id: redactor.redact_string(&self.model_id),
            provider: redactor.redact_string(&self.provider),
            context_digest: self.context_digest.clone(),
            output_contract_id: redactor.redact_string(&self.output_contract_id),
            timestamp: self.timestamp,
            source_kind: self.source_kind,
            mission_id: self.mission_id.clone(),
            task_id: self.task_id.clone(),
            agent_id: self.agent_id.clone(),
        }
    }
}

/// Metadata describing an individual layer during compilation (without exposing raw prompt text).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PromptLayerMetadata {
    /// The canonical layer kind (L0 through L6).
    pub kind: PromptLayerKind,
    /// Display name of the layer.
    pub name: String,
    /// Byte length of the layer content.
    pub byte_size: usize,
    /// Trust boundary level of the layer.
    pub trust_level: TrustLevel,
    /// Whether this layer is protected from budget compaction.
    pub is_protected: bool,
    /// Whether this layer was included in the final effective prompt (false if dropped by compaction).
    pub included: bool,
}

impl PromptLayerMetadata {
    /// Construct metadata for a layer that was evaluated during compilation.
    pub fn new(
        kind: PromptLayerKind,
        name: impl Into<String>,
        byte_size: usize,
        trust_level: TrustLevel,
        is_protected: bool,
        included: bool,
    ) -> Self {
        Self {
            kind,
            name: name.into(),
            byte_size,
            trust_level,
            is_protected,
            included,
        }
    }
}

/// Compaction and budgeting telemetry for prompt compilation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PromptCompactionMetadata {
    /// Maximum allowed budget in bytes.
    pub budget_max_bytes: usize,
    /// Sum of bytes from protected layers (L0, L1, L2, L6).
    pub protected_bytes: usize,
    /// Total bytes in the final assembled prompt text.
    pub total_compiled_bytes: usize,
    /// Canonical layer kinds that were retained in the prompt.
    pub layers_retained: Vec<PromptLayerKind>,
    /// Canonical layer kinds that were dropped due to budget constraints.
    pub layers_dropped: Vec<PromptLayerKind>,
    /// Whether the protected layers alone exceeded the budget (causing failure).
    pub budget_exceeded: bool,
}

impl PromptCompactionMetadata {
    /// Construct a new PromptCompactionMetadata record.
    pub fn new(
        budget_max_bytes: usize,
        protected_bytes: usize,
        total_compiled_bytes: usize,
        layers_retained: Vec<PromptLayerKind>,
        layers_dropped: Vec<PromptLayerKind>,
        budget_exceeded: bool,
    ) -> Self {
        Self {
            budget_max_bytes,
            protected_bytes,
            total_compiled_bytes,
            layers_retained,
            layers_dropped,
            budget_exceeded,
        }
    }

    /// Check whether any layers were dropped during compilation.
    pub fn has_dropped_layers(&self) -> bool {
        !self.layers_dropped.is_empty()
    }
}

/// Rich audit provenance record capturing end-to-end prompt compilation telemetry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PromptProvenance {
    /// Core invocation provenance as required by Spec Section 24.
    pub invocation: PromptInvocationProvenance,
    /// Mission identifier.
    pub mission_id: String,
    /// Task identifier.
    pub task_id: String,
    /// Agent role assigned.
    pub role: AgentRole,
    /// Workflow execution stage.
    pub stage: MissionStage,
    /// Metadata for all canonical layers evaluated.
    pub layers: Vec<PromptLayerMetadata>,
    /// Compaction and budget execution metadata.
    pub compaction: PromptCompactionMetadata,
}

impl PromptProvenance {
    /// Extract lightweight invocation provenance for transport and storage.
    pub fn to_invocation_provenance(&self) -> PromptInvocationProvenance {
        self.invocation.clone()
    }

    /// Origin source kind of the active prompt contract.
    pub fn source_kind(&self) -> PromptSourceKind {
        self.invocation.source_kind
    }

    /// Check if any layers were dropped during compilation.
    pub fn has_dropped_layers(&self) -> bool {
        self.compaction.has_dropped_layers()
    }

    /// Get list of layer kinds retained in the final prompt.
    pub fn retained_layers(&self) -> &[PromptLayerKind] {
        &self.compaction.layers_retained
    }

    /// Get list of layer kinds dropped during compaction.
    pub fn dropped_layers(&self) -> &[PromptLayerKind] {
        &self.compaction.layers_dropped
    }

    /// Returns true if a given layer kind is immutable or protected from compaction.
    pub fn is_protected_layer(kind: PromptLayerKind) -> bool {
        matches!(
            kind,
            PromptLayerKind::L0Safety
                | PromptLayerKind::L1Role
                | PromptLayerKind::L2Objective
                | PromptLayerKind::L6QualityGate
        )
    }
}

/// Deterministic trace of a prompt compilation operation for telemetry events.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PromptCompilationTrace {
    /// Mission identifier.
    pub mission_id: String,
    /// Task identifier.
    pub task_id: String,
    /// Prompt contract identifier.
    pub prompt_id: String,
    /// Prompt contract version.
    pub prompt_version: u32,
    /// Effective compiled prompt hash.
    pub effective_prompt_hash: String,
    /// Context manifest digest.
    pub context_digest: String,
    /// Strategy applied.
    pub strategy: PromptStrategy,
    /// Model identifier.
    pub model_id: String,
    /// Total compiled bytes.
    pub total_bytes: usize,
    /// Compaction metadata.
    pub compaction: PromptCompactionMetadata,
    /// Compilation duration in microseconds.
    pub duration_us: u64,
}
