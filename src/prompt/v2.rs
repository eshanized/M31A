//! PromptOS V2 Canonical Domain Contracts, Layered Authority, and Epistemic Governance.
//!
//! Provides the foundational models and contract invariants for PromptOS V2:
//! 1. **Canonical Taxonomy:** Deterministic, role- and stage-aware prompt categorization.
//! 2. **Explicit Layered Authority:** Non-reorderable 7-layer hierarchy from Kernel (L0) to Dynamic Mission (L6).
//! 3. **Structured Response Kinds:** Strongly typed model response expectations replacing raw string outputs.
//! 4. **Evidence & Verification Semantics:** Mandatory counterexample search, falsification, and empirical evidence validation.
//! 5. **Epistemic Discipline:** Categorization into FACT, INFERENCE, ASSUMPTION, PROPOSAL, OBSERVATION, VERIFICATION, UNKNOWN.
//! 6. **Production Reachability Awareness:** Tracking defined, instantiated, referenced, reachable, production-reachable, exercised, and verified code paths.

use crate::prompt::error::PromptError;
use serde::{Deserialize, Serialize};

/// Canonical taxonomy categories for PromptOS V2 contracts.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default,
)]
#[serde(rename_all = "snake_case")]
pub enum PromptKind {
    /// Protected Layer 0/1 core safety invariants and system constraints.
    #[default]
    Core,
    /// Agent identity, cognitive lifecycle, and behavioral envelope contracts.
    Agent,
    /// Workflow and mission task decomposition and planning contracts.
    Planning,
    /// Task-specific execution, file editing, and patch application contracts.
    Execution,
    /// Independent verification, diff auditing, and quality gate contracts.
    Verification,
    /// Incident investigation, root cause diagnosis, and recovery contracts.
    Recovery,
    /// Project genesis, discovery, research synthesis, and charter contracts.
    Genesis,
    /// Reusable procedural capability and in-task guidance contracts.
    Skill,
    /// Backward-compatibility alias contracts mapped to canonical V2 contracts.
    Compatibility,
}

impl PromptKind {
    /// Parse kind string with fallback.
    pub fn parse_kind(s: &str) -> Result<Self, PromptError> {
        let normalized = s.trim().to_lowercase();
        match normalized.as_str() {
            "core" | "safety" | "runtime" => Ok(Self::Core),
            "agent" => Ok(Self::Agent),
            "planning" | "plan" => Ok(Self::Planning),
            "execution" | "exec" => Ok(Self::Execution),
            "verification" | "verify" => Ok(Self::Verification),
            "recovery" | "diagnose" => Ok(Self::Recovery),
            "genesis" => Ok(Self::Genesis),
            "skill" => Ok(Self::Skill),
            "compatibility" | "alias" | "legacy" => Ok(Self::Compatibility),
            other => Err(PromptError::PromptInvalid {
                id: "<kind>".to_string(),
                version: 0,
                reason: format!("unknown prompt kind '{}'", other),
            }),
        }
    }

    /// String identifier for prompt kind.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Core => "core",
            Self::Agent => "agent",
            Self::Planning => "planning",
            Self::Execution => "execution",
            Self::Verification => "verification",
            Self::Recovery => "recovery",
            Self::Genesis => "genesis",
            Self::Skill => "skill",
            Self::Compatibility => "compatibility",
        }
    }
}

impl std::fmt::Display for PromptKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Explicit hierarchical authority levels for prompt composition layers.
/// Lower numerical value represents higher, non-overrideable authority.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default,
)]
#[repr(u8)]
pub enum AuthorityLevel {
    /// Layer 0: Immutable Runtime Safety Invariants (P0, non-overrideable).
    #[default]
    Kernel = 0,
    /// Layer 1: System Contract & Policy Envelopes.
    SystemPolicy = 1,
    /// Layer 2: Agent Role Contract (Identity, Cognitive Envelope).
    RoleContract = 2,
    /// Layer 3: Workflow Stage Protocol (Execution, Review, Recovery).
    StageContract = 3,
    /// Layer 4: Task Contract & Acceptance Criteria.
    TaskContract = 4,
    /// Layer 5: Evidence & Repository State Context.
    EvidenceContext = 5,
    /// Layer 6: Dynamic Mission Context & User Mentions.
    DynamicMission = 6,
}

impl AuthorityLevel {
    /// Return the numerical authority rank (0 = highest).
    pub fn rank(&self) -> u8 {
        *self as u8
    }

    /// Check if this authority level is allowed to override or modify another.
    /// Higher rank (numerically larger) can NEVER override lower rank.
    pub fn can_override(&self, target: AuthorityLevel) -> bool {
        self.rank() < target.rank() && target != AuthorityLevel::Kernel
    }
}

/// Strongly typed model response categories required by PromptOS V2 contracts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PromptResponseKind {
    /// Direct proposal of one or more tool actions.
    ProposeAction,
    /// Formal request for additional repository or environmental context.
    RequestContext,
    /// Empirical findings and observations from research or inspection.
    ReportFinding,
    /// Request to hand off task execution to another specialized role.
    ProposeHandoff,
    /// Proposal that task is complete, accompanied by mandatory proof.
    ProposeCompletion,
    /// Structured evaluation of verification tests or criteria.
    ReportVerification,
    /// Structured classification and diagnosis of an execution failure.
    ReportFailure,
    /// Escalation to human operator or higher runtime authority.
    Escalate,
}

impl PromptResponseKind {
    /// Parse from output type string.
    pub fn from_type_str(s: &str) -> Self {
        let norm = s.trim().to_lowercase();
        match norm.as_str() {
            "action" | "tool_proposals" | "propose_action" | "code" => Self::ProposeAction,
            "request_context" | "context" => Self::RequestContext,
            "finding" | "findings" | "report_finding" | "research" => Self::ReportFinding,
            "handoff" | "propose_handoff" => Self::ProposeHandoff,
            "complete" | "completion" | "propose_completion" => Self::ProposeCompletion,
            "verdict" | "report_verification" | "verification" => Self::ReportVerification,
            "failure" | "diagnose" | "report_failure" | "recovery" | "json_schema" => {
                Self::ReportFailure
            }
            "escalate" | "escalation" => Self::Escalate,
            _ => Self::ReportFinding,
        }
    }

    /// String representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ProposeAction => "propose_action",
            Self::RequestContext => "request_context",
            Self::ReportFinding => "report_finding",
            Self::ProposeHandoff => "propose_handoff",
            Self::ProposeCompletion => "propose_completion",
            Self::ReportVerification => "report_verification",
            Self::ReportFailure => "report_failure",
            Self::Escalate => "escalate",
        }
    }
}

/// Output contract defining the expected format, schema, and response kind.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutputContract {
    /// Semantic classification of the response.
    pub response_kind: PromptResponseKind,
    /// Format type identifier (e.g. "json", "markdown", "code", "verdict").
    pub format_type: String,
    /// Optional schema URI for structured validation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema_uri: Option<String>,
    /// Whether output must strictly adhere to schema without conversational preamble.
    #[serde(default)]
    pub is_strict: bool,
}

impl OutputContract {
    /// Construct a new OutputContract.
    pub fn new(
        response_kind: PromptResponseKind,
        format_type: impl Into<String>,
        schema_uri: Option<String>,
        is_strict: bool,
    ) -> Self {
        Self {
            response_kind,
            format_type: format_type.into(),
            schema_uri,
            is_strict,
        }
    }

    /// Default output contract for code implementation proposals.
    pub fn code_proposal() -> Self {
        Self {
            response_kind: PromptResponseKind::ProposeAction,
            format_type: "tool_proposals".to_string(),
            schema_uri: Some("schema://model/proposal.v1".to_string()),
            is_strict: true,
        }
    }

    /// Default output contract for verification verdicts.
    pub fn verification_verdict() -> Self {
        Self {
            response_kind: PromptResponseKind::ReportVerification,
            format_type: "verdict".to_string(),
            schema_uri: Some("schema://verification/review_verdict.v1".to_string()),
            is_strict: true,
        }
    }

    /// Default output contract for failure diagnosis.
    pub fn failure_diagnosis() -> Self {
        Self {
            response_kind: PromptResponseKind::ReportFailure,
            format_type: "json_schema".to_string(),
            schema_uri: Some("schema://recovery/failure_classification.v1".to_string()),
            is_strict: true,
        }
    }
}

/// Reasoning modes supported by PromptOS V2 contracts.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default,
)]
#[serde(rename_all = "snake_case")]
pub enum ReasoningMode {
    /// Concise, direct output without structured step articulation.
    Direct,
    /// Explicit, sequential step-by-step reasoning.
    #[default]
    StepByStep,
    /// Dynamically adjusted based on task complexity.
    Adaptive,
}

/// Reasoning depths supported by PromptOS V2 contracts.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default,
)]
#[serde(rename_all = "snake_case")]
pub enum ReasoningDepth {
    /// Minimal reasoning for simple rote tasks.
    Minimal,
    /// Standard engineering reasoning depth.
    #[default]
    Standard,
    /// Deep reasoning for architecture, diagnosis, and security analysis.
    Deep,
    /// Dynamically modulated based on task risk and change surface.
    TaskDependent,
}

/// Contract governing reasoning mode, depth, and falsification rules.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ReasoningPolicy {
    #[serde(default)]
    pub mode: ReasoningMode,
    #[serde(default)]
    pub depth: ReasoningDepth,
    #[serde(default)]
    pub mandatory_falsification: bool,
    #[serde(default)]
    pub claim_categorization: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chain_of_thought_budget: Option<usize>,
}

/// Required empirical evidence criteria for a contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceRequirements {
    /// Whether verifiable empirical evidence is required to complete this task.
    pub required: bool,
    /// Minimum count of discrete evidence items required.
    pub min_evidence_count: usize,
    /// Classes of evidence accepted (e.g. "test_execution", "diff_inspection", "compiler_diagnostic").
    pub trusted_classes: Vec<String>,
    /// Assumptions explicitly forbidden from serving as evidence.
    pub forbidden_assumptions: Vec<String>,
    /// Whether evidence must be observed fresh post-mutation.
    pub freshness_required: bool,
    /// Whether evidence must include provenance records.
    pub provenance_required: bool,
}

impl Default for EvidenceRequirements {
    fn default() -> Self {
        Self {
            required: false,
            min_evidence_count: 0,
            trusted_classes: Vec::new(),
            forbidden_assumptions: vec![
                "compilation_implies_correctness".to_string(),
                "summary_is_evidence".to_string(),
                "tests_are_complete".to_string(),
            ],
            freshness_required: true,
            provenance_required: true,
        }
    }
}

impl EvidenceRequirements {
    /// Create evidence requirements for an implementer.
    pub fn for_implementer() -> Self {
        Self {
            required: true,
            min_evidence_count: 1,
            trusted_classes: vec!["test_execution".to_string(), "diff_inspection".to_string()],
            forbidden_assumptions: vec![
                "compilation_implies_correctness".to_string(),
                "summary_is_evidence".to_string(),
                "no_errors_means_verified".to_string(),
            ],
            freshness_required: true,
            provenance_required: true,
        }
    }

    /// Create evidence requirements for a verifier.
    pub fn for_verifier() -> Self {
        Self {
            required: true,
            min_evidence_count: 2,
            trusted_classes: vec![
                "test_execution".to_string(),
                "counterexample_search".to_string(),
                "assertion_audit".to_string(),
            ],
            forbidden_assumptions: vec![
                "implementer_summary_is_truth".to_string(),
                "passing_tests_prove_no_bugs".to_string(),
            ],
            freshness_required: true,
            provenance_required: true,
        }
    }
}

/// Verification invariants and falsification obligations for a contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct VerificationRequirements {
    /// Whether the agent is obligated to actively search for counterexamples.
    #[serde(default)]
    pub falsification_required: bool,
    /// Whether counterexample searches must be documented.
    #[serde(default)]
    pub counterexample_search: bool,
    /// Whether production reachability must be verified.
    #[serde(default)]
    pub reachability_check: bool,
    /// Whether verification must execute in an independent context.
    #[serde(default)]
    pub independent_execution: bool,
    /// Discrete verification gates that must pass.
    #[serde(default)]
    pub required_gates: Vec<String>,
    /// Concrete acceptance criteria that must be empirically verified.
    #[serde(default)]
    pub acceptance_criteria: Vec<String>,
}

impl VerificationRequirements {
    /// Strict verification requirements for Reviewer, Verifier, and Auditor roles.
    pub fn strict_adversarial() -> Self {
        Self {
            falsification_required: true,
            counterexample_search: true,
            reachability_check: true,
            independent_execution: true,
            required_gates: vec![
                "cargo check".to_string(),
                "cargo clippy".to_string(),
                "cargo test".to_string(),
            ],
            acceptance_criteria: vec![
                "No regressions in existing functionality".to_string(),
                "All acceptance assertions verified".to_string(),
            ],
        }
    }
}

/// Failure behaviors supported by PromptOS V2 contracts.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default,
)]
#[serde(rename_all = "snake_case")]
pub enum FailureBehavior {
    /// Bounded retry with updated diagnostic context.
    #[default]
    Retryable,
    /// Immediate halt without retry.
    NonRetryable,
    /// Escalate to human operator or runtime controller.
    Escalate,
    /// Request additional repository context before attempting action.
    RequestContext,
    /// Request independent verification or second-opinion review.
    RequestVerification,
}

impl FailureBehavior {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Retryable => "retryable",
            Self::NonRetryable => "non_retryable",
            Self::Escalate => "escalate",
            Self::RequestContext => "request_context",
            Self::RequestVerification => "request_verification",
        }
    }
}

/// Failure policy governing retries and escalation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FailurePolicy {
    #[serde(default)]
    pub behavior: FailureBehavior,
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
    #[serde(default)]
    pub escalation_path: String,
}

fn default_max_retries() -> u32 {
    2
}

impl Default for FailurePolicy {
    fn default() -> Self {
        Self {
            behavior: FailureBehavior::Retryable,
            max_retries: 2,
            escalation_path: "runtime.supervisor".to_string(),
        }
    }
}

/// Epistemic status of a statement or observation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EpistemicStatus {
    /// Direct empirical observation from tool output, compiler, or file contents.
    Fact,
    /// Logical deduction derived directly from observed facts.
    Inference,
    /// Unverified working premise that must be tested before being trusted.
    Assumption,
    /// Proposed modification or action submitted for runtime authorization.
    Proposal,
    /// Reading taken immediately after a mutation or tool execution.
    Observation,
    /// Proven true via active falsification and counterexample search.
    Verification,
    /// Information that is currently missing or unproven.
    Unknown,
}

/// Verification status of a claim.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimVerificationStatus {
    Verified,
    Partial,
    Unverified,
    Contradicted,
}

/// Structured claim and evidence record for auditable reasoning.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClaimRecord {
    /// The concrete assertion being made.
    pub claim: String,
    /// Concrete evidence supporting the assertion (e.g. file:line, tool output, test name).
    pub evidence: Vec<String>,
    /// Confidence score (0 to 100).
    pub confidence: u8,
    /// Source origin of the evidence.
    pub source: String,
    /// Architectural or operational implication of the claim.
    pub implication: String,
    /// Questions left unresolved by the evidence.
    pub unresolved_questions: Vec<String>,
    /// Current verification status.
    pub status: ClaimVerificationStatus,
}

/// Production reachability state of a symbol, subsystem, or feature.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default,
)]
#[serde(rename_all = "snake_case")]
pub enum ProductionReachability {
    /// Code or contract is defined in source.
    #[default]
    Defined = 0,
    /// Type or component is instantiated somewhere in the codebase.
    Instantiated = 1,
    /// Component is referenced by at least one caller.
    Referenced = 2,
    /// Call chain exists from some entry point.
    Reachable = 3,
    /// Call chain is provably reachable from production CLI / runtime startup path.
    ProductionReachable = 4,
    /// Code path is executed by automated test suites.
    Exercised = 5,
    /// Invariants are verified by integration / property / adversarial tests.
    Verified = 6,
}

impl ProductionReachability {
    /// Check if the component is safe for production release.
    pub fn is_production_certified(&self) -> bool {
        *self >= Self::ProductionReachable
    }
}

/// Backward compatibility and deprecation metadata.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompatibilityMetadata {
    /// Legacy alias IDs that resolve to this contract.
    pub legacy_aliases: Vec<String>,
    /// Whether this contract itself is deprecated in favor of a canonical replacement.
    pub is_deprecated: bool,
    /// The canonical replacement prompt ID if deprecated.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub canonical_replacement: Option<String>,
    /// Deprecation rationale or migration instruction.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deprecation_note: Option<String>,
}

/// Full inventory audit record classifying all prompt contracts in the system.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PromptInventoryAudit {
    /// Fully canonical PromptOS V2 contracts.
    pub canonical_contracts: Vec<String>,
    /// Deprecated contracts with their canonical replacements: (alias, replacement).
    pub deprecated_aliases: Vec<(String, String)>,
    /// Contracts that are defined or embedded but never reachable in production.
    pub dead_or_unused: Vec<String>,
    /// Contracts utilized exclusively in unit or integration test fixtures.
    pub test_only_contracts: Vec<String>,
    /// Contracts actively reached from production execution pathways.
    pub production_reachable: Vec<String>,
}
