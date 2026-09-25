//! Typed 7-layer prompt composition model (Architecture Section 16).
//!
//! Enforces the canonical layered prompt hierarchy:
//! - Layer 0: Runtime Safety Invariants (P0, immutable, unprunable)
//! - Layer 1: Agent Role & Profile (P1, unprunable)
//! - Layer 2: Workflow Step Objective & Output Contract (P1, unprunable)
//! - Layer 3: Project Charter & Boundaries (P2, compactable)
//! - Layer 4: Upstream Artifact Evidence & Tool Results (P2/P3, compactable)
//! - Layer 5: Repository Constraints & Context (P3/P4, disposable)
//! - Layer 6: Quality Gate Assertions (P1, unprunable)

use crate::context::envelope::TrustLevel;
use crate::context::priority::{ContextPriority, ContextSection};
use crate::context::tokenizer::TokenizerAdapter;
use crate::prompt::context::PromptContext;
use crate::prompt::contract::RUNTIME_SAFETY_INVARIANTS;
use crate::prompt::error::PromptError;
use serde::{Deserialize, Serialize};

/// Canonical 7-layer hierarchy enum enforcing deterministic ordering (L0 -> L6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum PromptLayerKind {
    /// Layer 0: Runtime Safety Invariants (P0, immutable, unprunable).
    L0Safety = 0,
    /// Layer 1: Agent Role & Profile (P1, unprunable).
    L1Role = 1,
    /// Layer 2: Workflow Step Objective & Output Contract (P1, unprunable).
    L2Objective = 2,
    /// Layer 3: Project Charter & Durable State (P2, compactable).
    L3DurableState = 3,
    /// Layer 4: Upstream Artifact Evidence & Tool Results (P2/P3, compactable).
    L4Evidence = 4,
    /// Layer 5: Repository Constraints & Context (P3/P4, disposable).
    L5RepoContext = 5,
    /// Layer 6: Quality Gate Assertions (P1, unprunable).
    L6QualityGate = 6,
}

impl PromptLayerKind {
    /// String identifier for layer kind.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::L0Safety => "layer_0_safety",
            Self::L1Role => "layer_1_role",
            Self::L2Objective => "layer_2_objective",
            Self::L3DurableState => "layer_3_durable_state",
            Self::L4Evidence => "layer_4_evidence",
            Self::L5RepoContext => "layer_5_repo_context",
            Self::L6QualityGate => "layer_6_quality_gate",
        }
    }

    /// Priority level (lower number = higher protection against budget truncation).
    pub fn priority(&self) -> u8 {
        match self {
            Self::L0Safety => 0,
            Self::L1Role | Self::L2Objective | Self::L6QualityGate => 1,
            Self::L3DurableState | Self::L4Evidence => 2,
            Self::L5RepoContext => 3,
        }
    }

    /// Whether this layer can be compacted or dropped under budget pressure.
    pub fn is_prunable(&self) -> bool {
        match self {
            Self::L0Safety | Self::L1Role | Self::L2Objective | Self::L6QualityGate => false,
            Self::L3DurableState | Self::L4Evidence | Self::L5RepoContext => true,
        }
    }
}

impl std::fmt::Display for PromptLayerKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// An individual layer entry in a compiled 7-layer prompt composition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PromptLayerEntry {
    /// Layer kind (L0 through L6).
    pub kind: PromptLayerKind,
    /// Human-readable section header name.
    pub name: String,
    /// Rendered text content of this layer.
    pub content: String,
    /// Trust classification of this layer's content.
    pub trust_level: TrustLevel,
    /// Whether this layer may be compacted or dropped.
    pub is_prunable: bool,
    /// Total byte length of the content.
    pub byte_size: usize,
}

impl PromptLayerEntry {
    /// Create a new layer entry.
    pub fn new(
        kind: PromptLayerKind,
        name: impl Into<String>,
        content: impl Into<String>,
        trust_level: TrustLevel,
    ) -> Self {
        let name_str = name.into();
        let text = content.into();
        let is_prunable = kind.is_prunable();
        // Formatted section byte size: "## <name>\n<content>\n\n"
        let byte_size = 6 + name_str.len() + text.len();
        Self {
            kind,
            name: name_str,
            content: text,
            trust_level,
            is_prunable,
            byte_size,
        }
    }
}

/// Upstream artifact excerpt injected into Layer 4.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactEvidenceSection {
    pub source_step_key: String,
    pub artifact_name: String,
    pub content: String,
}

/// Typed 7-layer prompt composition model (Architecture Section 16).
///
/// Guaranteed layered hierarchy:
/// - Layer 0: Runtime Safety Invariants (P0, immutable, unprunable)
/// - Layer 1: Agent Role & Profile (P1)
/// - Layer 2: Workflow Step Objective & Output Contract (P1)
/// - Layer 3: Project Charter & Boundaries (P2, prunable)
/// - Layer 4: Upstream Artifact Evidence (P2/P3, prunable)
/// - Layer 5: Repository Constraints & Context (P3/P4, prunable)
/// - Layer 6: Quality Gate Assertions (P1)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PromptComposition {
    /// Layer 0: Runtime Safety Invariants (P0, immutable, unprunable).
    pub layer_0_safety: String,
    /// Layer 1: Agent Role & Profile (P1).
    pub layer_1_role: String,
    /// Layer 2: Workflow Step Objective & Output Contract (P1).
    pub layer_2_objective: String,
    /// Layer 3: Project Charter & Boundaries (P2, optional).
    pub layer_3_charter: Option<String>,
    /// Layer 4: Upstream Artifact Evidence (P2/P3, optional).
    pub layer_4_upstream_evidence: Vec<ArtifactEvidenceSection>,
    /// Layer 5: Repository Constraints & Context (P3/P4, optional).
    pub layer_5_repo_context: Option<String>,
    /// Layer 6: Quality Gate Assertions (P1).
    pub layer_6_quality_gate_assertions: Vec<String>,
}

impl PromptComposition {
    /// Create a new prompt composition with immutable Layer 0 safety invariants.
    pub fn new(
        role_profile: impl Into<String>,
        step_objective: impl Into<String>,
        quality_gate_assertions: Vec<String>,
    ) -> Self {
        Self {
            layer_0_safety: RUNTIME_SAFETY_INVARIANTS.to_string(),
            layer_1_role: role_profile.into(),
            layer_2_objective: step_objective.into(),
            layer_3_charter: None,
            layer_4_upstream_evidence: Vec::new(),
            layer_5_repo_context: None,
            layer_6_quality_gate_assertions: quality_gate_assertions,
        }
    }

    /// Construct a PromptComposition from a typed PromptContext, role profile, and rendered objective.
    pub fn from_context(
        context: &PromptContext,
        role_profile: impl Into<String>,
        rendered_objective: impl Into<String>,
    ) -> Self {
        let mut comp = Self::new(
            role_profile,
            rendered_objective,
            context.quality_gate.assertions.clone(),
        );

        if let Some(charter) = &context.durable_state.project_charter {
            comp = comp.with_charter(charter);
        }

        for artifact in &context.upstream_artifacts {
            comp = comp.with_upstream_artifact(
                &artifact.source_step,
                &artifact.artifact_name,
                &artifact.excerpt,
            );
        }

        for tool_out in &context.tool_outputs {
            let label = format!("{}:{}", tool_out.tool_name, tool_out.call_id);
            comp = comp.with_upstream_artifact("tool_execution", label, &tool_out.output);
        }

        if let Some(topo) = &context.repo_context.topology_summary {
            comp = comp.with_repo_context(topo);
        } else if !context.repo_context.relevant_files.is_empty() {
            let repo_files_summary = context
                .repo_context
                .relevant_files
                .iter()
                .map(|f| format!("--- File: {} ---\n{}", f.path, f.content))
                .collect::<Vec<_>>()
                .join("\n\n");
            comp = comp.with_repo_context(repo_files_summary);
        }

        comp
    }

    /// Attach Layer 3 project charter content.
    pub fn with_charter(mut self, charter: impl Into<String>) -> Self {
        self.layer_3_charter = Some(charter.into());
        self
    }

    /// Add an upstream artifact excerpt to Layer 4.
    pub fn with_upstream_artifact(
        mut self,
        source_step: impl Into<String>,
        artifact_name: impl Into<String>,
        content: impl Into<String>,
    ) -> Self {
        self.layer_4_upstream_evidence
            .push(ArtifactEvidenceSection {
                source_step_key: source_step.into(),
                artifact_name: artifact_name.into(),
                content: content.into(),
            });
        self
    }

    /// Attach Layer 5 repository context.
    pub fn with_repo_context(mut self, repo_context: impl Into<String>) -> Self {
        self.layer_5_repo_context = Some(repo_context.into());
        self
    }

    /// Assert that Layer 0 contains the uncompromised runtime safety invariants.
    pub fn verify_safety_invariants(&self) -> bool {
        self.layer_0_safety == RUNTIME_SAFETY_INVARIANTS
    }

    /// Validate that composition is structurally sound and enforces immutable Layer 0.
    pub fn validate(&self) -> Result<(), PromptError> {
        if !self.verify_safety_invariants() {
            return Err(PromptError::PromptSecurityViolation {
                prompt_id: "composition".to_string(),
                reason: "Layer 0 safety invariants have been tampered with or modified".to_string(),
            });
        }
        if self.layer_1_role.trim().is_empty() {
            return Err(PromptError::PromptCompositionError {
                prompt_id: "composition".to_string(),
                reason: "Layer 1 role profile cannot be empty".to_string(),
            });
        }
        if self.layer_2_objective.trim().is_empty() {
            return Err(PromptError::PromptCompositionError {
                prompt_id: "composition".to_string(),
                reason: "Layer 2 objective cannot be empty".to_string(),
            });
        }
        Ok(())
    }

    /// Returns the discrete prompt layers in canonical deterministic order (L0 through L6).
    pub fn ordered_layers(&self) -> Vec<PromptLayerEntry> {
        let mut layers = Vec::with_capacity(7);

        // L0: Safety
        layers.push(PromptLayerEntry::new(
            PromptLayerKind::L0Safety,
            "SYSTEM INVARIANTS (P0)",
            &self.layer_0_safety,
            TrustLevel::TrustedSystem,
        ));

        // L1: Role
        layers.push(PromptLayerEntry::new(
            PromptLayerKind::L1Role,
            "AGENT ROLE & PROFILE (P1)",
            &self.layer_1_role,
            TrustLevel::TrustedSystem,
        ));

        // L2: Objective
        layers.push(PromptLayerEntry::new(
            PromptLayerKind::L2Objective,
            "WORKFLOW STEP OBJECTIVE & CONTRACT (P1)",
            &self.layer_2_objective,
            TrustLevel::TrustedSystem,
        ));

        // L3: Durable State
        if let Some(charter) = &self.layer_3_charter {
            layers.push(PromptLayerEntry::new(
                PromptLayerKind::L3DurableState,
                "PROJECT CHARTER & BOUNDARIES (P2)",
                charter,
                TrustLevel::UntrustedRepoContent,
            ));
        }

        // L4: Evidence
        if !self.layer_4_upstream_evidence.is_empty() {
            let mut ev_str = String::new();
            for (idx, ev) in self.layer_4_upstream_evidence.iter().enumerate() {
                if idx > 0 {
                    ev_str.push_str("\n\n");
                }
                ev_str.push_str(&format!(
                    "### Artifact '{}' (from step '{}'):\n{}",
                    ev.artifact_name, ev.source_step_key, ev.content
                ));
            }
            layers.push(PromptLayerEntry::new(
                PromptLayerKind::L4Evidence,
                "UPSTREAM ARTIFACT EVIDENCE (P2/P3)",
                ev_str,
                TrustLevel::UntrustedRepoContent,
            ));
        }

        // L5: Repo Context
        if let Some(ctx) = &self.layer_5_repo_context {
            layers.push(PromptLayerEntry::new(
                PromptLayerKind::L5RepoContext,
                "REPOSITORY CONSTRAINTS & CONTEXT (P3/P4)",
                ctx,
                TrustLevel::UntrustedRepoContent,
            ));
        }

        // L6: Quality Gate
        if !self.layer_6_quality_gate_assertions.is_empty() {
            let checklist = self
                .layer_6_quality_gate_assertions
                .iter()
                .map(|a| format!("- [ ] {}", a))
                .collect::<Vec<_>>()
                .join("\n");
            layers.push(PromptLayerEntry::new(
                PromptLayerKind::L6QualityGate,
                "QUALITY GATE VERIFICATION CHECKLIST (P1)",
                checklist,
                TrustLevel::TrustedSystem,
            ));
        }

        layers
    }

    /// Render composition into a structured markdown prompt document.
    pub fn to_full_prompt(&self) -> String {
        let mut out = String::new();

        out.push_str("## SYSTEM INVARIANTS (P0)\n");
        out.push_str(&self.layer_0_safety);
        out.push_str("\n\n");

        out.push_str("## AGENT ROLE & PROFILE (P1)\n");
        out.push_str(&self.layer_1_role);
        out.push_str("\n\n");

        out.push_str("## WORKFLOW STEP OBJECTIVE & CONTRACT (P1)\n");
        out.push_str(&self.layer_2_objective);
        out.push_str("\n\n");

        if let Some(charter) = &self.layer_3_charter {
            out.push_str("## PROJECT CHARTER & BOUNDARIES (P2)\n");
            out.push_str(charter);
            out.push_str("\n\n");
        }

        if !self.layer_4_upstream_evidence.is_empty() {
            out.push_str("## UPSTREAM ARTIFACT EVIDENCE (P2/P3)\n");
            for ev in &self.layer_4_upstream_evidence {
                out.push_str(&format!(
                    "### Artifact '{}' (from step '{}'):\n{}\n\n",
                    ev.artifact_name, ev.source_step_key, ev.content
                ));
            }
        }

        if let Some(ctx) = &self.layer_5_repo_context {
            out.push_str("## REPOSITORY CONSTRAINTS & CONTEXT (P3/P4)\n");
            out.push_str(ctx);
            out.push_str("\n\n");
        }

        if !self.layer_6_quality_gate_assertions.is_empty() {
            out.push_str("## QUALITY GATE VERIFICATION CHECKLIST (P1)\n");
            for assertion in &self.layer_6_quality_gate_assertions {
                out.push_str(&format!("- [ ] {}\n", assertion));
            }
            out.push('\n');
        }

        out
    }

    /// Convert the 7 composition layers into prioritized ContextSections for reverse compaction.
    pub fn to_context_sections(&self, tokenizer: &TokenizerAdapter) -> Vec<ContextSection> {
        let mut sections = Vec::new();

        // Layer 0: Runtime Safety (P0, immutable, unprunable)
        sections.push(ContextSection::new(
            "layer_0_safety",
            ContextPriority::P0RuntimeSafety,
            &self.layer_0_safety,
            "system://m31a/runtime_safety",
            TrustLevel::TrustedSystem,
            false,
            tokenizer,
        ));

        // Layer 1: Agent Role & Profile (P1, unprunable)
        sections.push(ContextSection::new(
            "layer_1_role",
            ContextPriority::P1TaskCompletion,
            &self.layer_1_role,
            "workflow://profile",
            TrustLevel::TrustedSystem,
            false,
            tokenizer,
        ));

        // Layer 2: Workflow Step Objective (P1, unprunable)
        sections.push(ContextSection::new(
            "layer_2_objective",
            ContextPriority::P1TaskCompletion,
            &self.layer_2_objective,
            "workflow://objective",
            TrustLevel::TrustedSystem,
            false,
            tokenizer,
        ));

        // Layer 3: Project Charter (P2, loss-preserving compactable)
        if let Some(charter) = &self.layer_3_charter {
            sections.push(ContextSection::new(
                "layer_3_charter",
                ContextPriority::P2RequiredEvidence,
                charter,
                "project://PROJECT.md",
                TrustLevel::UntrustedRepoContent,
                true,
                tokenizer,
            ));
        }

        // Layer 4: Upstream Artifact Evidence (P2, compactable)
        for (idx, ev) in self.layer_4_upstream_evidence.iter().enumerate() {
            sections.push(ContextSection::new(
                format!("layer_4_evidence_{}_{}", ev.source_step_key, idx),
                ContextPriority::P2RequiredEvidence,
                &ev.content,
                format!("artifact://{}/{}", ev.source_step_key, ev.artifact_name),
                TrustLevel::UntrustedRepoContent,
                true,
                tokenizer,
            ));
        }

        // Layer 5: Repository Context (P3, droppable if budget exceeded)
        if let Some(ctx) = &self.layer_5_repo_context {
            sections.push(ContextSection::new(
                "layer_5_repo_context",
                ContextPriority::P3ActiveWorkingContext,
                ctx,
                "repo://topology",
                TrustLevel::UntrustedRepoContent,
                true,
                tokenizer,
            ));
        }

        // Layer 6: Quality Gate Assertions (P1, unprunable)
        if !self.layer_6_quality_gate_assertions.is_empty() {
            let checklist = self
                .layer_6_quality_gate_assertions
                .iter()
                .map(|a| format!("- [ ] {}", a))
                .collect::<Vec<_>>()
                .join("\n");
            sections.push(ContextSection::new(
                "layer_6_quality_gate",
                ContextPriority::P1TaskCompletion,
                checklist,
                "workflow://quality_gate",
                TrustLevel::TrustedSystem,
                false,
                tokenizer,
            ));
        }

        sections
    }
}
