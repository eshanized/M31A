//! Typed runtime context abstraction for PromptOS (Architecture Section 8 & Section 46).
//!
//! Enforces strong domain typing, explicit trust classification, and separation of
//! ephemeral reasoning data from durable project memory.

use crate::context::envelope::{TrustEnvelope, TrustLevel};
use crate::prompt::contract::MAX_RENDERED_BYTES;
use crate::prompt::contract::RUNTIME_SAFETY_INVARIANTS;
use crate::prompt::parameter::MAX_PARAMETER_BYTES;
use crate::state_machine::agent::AgentRole;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

/// Canonical workflow execution stages per GSD lifecycle model (Section 17).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MissionStage {
    /// Elicit user requirements & project boundaries.
    Discuss,
    /// Investigate crates, patterns, pitfalls, security.
    Research,
    /// Reconcile research into formal specifications.
    Synthesize,
    /// Decompose architecture into structured DAG tasks.
    Plan,
    /// Verify plan DAG integrity and capability envelopes.
    PlanVerify,
    /// Execute individual task specifications.
    Execute,
    /// Verify implemented code against acceptance criteria.
    ImplVerify,
    /// Independent adversarial review of diffs and safety.
    Review,
    /// Deliver verified changes and update durable roadmap.
    Ship,
    /// Diagnose failure and plan targeted recovery.
    Diagnose,
}

impl MissionStage {
    /// String representation of mission stage.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Discuss => "discuss",
            Self::Research => "research",
            Self::Synthesize => "synthesize",
            Self::Plan => "plan",
            Self::PlanVerify => "plan_verify",
            Self::Execute => "execute",
            Self::ImplVerify => "impl_verify",
            Self::Review => "review",
            Self::Ship => "ship",
            Self::Diagnose => "diagnose",
        }
    }
}

impl std::fmt::Display for MissionStage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Authoritative runtime safety invariants and capability boundaries (P0, TrustedSystem).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrustedSystemContext {
    /// Immutable safety invariants text.
    pub safety_invariants: String,
    /// Permitted capability envelope for the current role.
    pub capability_envelope: Vec<String>,
    /// Maximum execution steps allowed.
    pub max_steps: u32,
    /// Current step turn number within execution.
    pub current_step: u32,
}

impl Default for TrustedSystemContext {
    fn default() -> Self {
        Self {
            safety_invariants: RUNTIME_SAFETY_INVARIANTS.to_string(),
            capability_envelope: Vec::new(),
            max_steps: 30,
            current_step: 1,
        }
    }
}

/// Authoritative runtime security and policy constraints (P0, TrustedPolicy).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrustedPolicyContext {
    /// Active sandbox mode (e.g. "workspace_write", "read_only").
    pub sandbox_mode: String,
    /// Absolute or relative workspace root path.
    pub workspace_root: String,
    /// Protected path prefixes blocked from access or modification.
    pub blocked_paths: Vec<String>,
}

/// Untrusted user-provided intent or instructions (P1, UntrustedUserInput).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserInputContext {
    /// Raw unescaped user intent string.
    pub raw_intent: String,
    /// Trust classification (always UntrustedUserInput).
    pub trust_level: TrustLevel,
    /// Cryptographic SHA-256 hash of the raw user input.
    pub content_hash: String,
}

impl UserInputContext {
    /// Construct a new UserInputContext from raw user text, automatically computing its hash.
    pub fn new(raw_intent: impl Into<String>) -> Self {
        let raw = raw_intent.into();
        let mut hasher = Sha256::new();
        hasher.update(raw.as_bytes());
        let hash = format!("{:x}", hasher.finalize());

        Self {
            raw_intent: raw,
            trust_level: TrustLevel::UntrustedUserInput,
            content_hash: hash,
        }
    }

    /// Render into a secure XML trust envelope with closing tag escaping.
    pub fn to_trust_envelope(&self) -> String {
        TrustEnvelope::wrap_user_intent(&self.raw_intent)
    }
}

/// Task objective and structured completion contract (P1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskObjectiveContext {
    /// Natural language specification of the task objective.
    pub task_objective: String,
    /// Specific acceptance criteria checklist.
    pub task_criteria: Vec<String>,
    /// Expected format of output response (e.g. "markdown", "json").
    pub expected_output_format: Option<String>,
}

impl TaskObjectiveContext {
    /// Construct new task objective context.
    pub fn new(task_objective: impl Into<String>) -> Self {
        Self {
            task_objective: task_objective.into(),
            task_criteria: Vec::new(),
            expected_output_format: None,
        }
    }

    /// Attach acceptance criteria.
    pub fn with_criteria(mut self, criteria: Vec<String>) -> Self {
        self.task_criteria = criteria;
        self
    }

    /// Set expected output format.
    pub fn with_output_format(mut self, format: impl Into<String>) -> Self {
        self.expected_output_format = Some(format.into());
        self
    }
}

/// Durable project memory derived from `.planning/` files (P2, UntrustedRepoContent).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DurableStateContext {
    /// Project charter excerpt (PROJECT.md).
    pub project_charter: Option<String>,
    /// Requirements specification excerpt (REQUIREMENTS.md).
    pub requirements: Option<String>,
    /// Architectural boundaries excerpt (ARCHITECTURE.md).
    pub architecture: Option<String>,
    /// Current mission/task state summary (STATE.md).
    pub state_summary: Option<String>,
}

/// Upstream artifact evidence excerpt (P2/P3, UntrustedRepoContent).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactEvidence {
    /// Workflow step that produced this artifact.
    pub source_step: String,
    /// Artifact file or logical name.
    pub artifact_name: String,
    /// SHA-256 hash of artifact content.
    pub content_hash: String,
    /// Excerpt text content.
    pub excerpt: String,
    /// Trust classification.
    pub trust_level: TrustLevel,
}

impl ArtifactEvidence {
    /// Construct new artifact evidence with SHA-256 calculation.
    pub fn new(
        source_step: impl Into<String>,
        artifact_name: impl Into<String>,
        excerpt: impl Into<String>,
    ) -> Self {
        let text = excerpt.into();
        let mut hasher = Sha256::new();
        hasher.update(text.as_bytes());
        let hash = format!("{:x}", hasher.finalize());

        Self {
            source_step: source_step.into(),
            artifact_name: artifact_name.into(),
            content_hash: hash,
            excerpt: text,
            trust_level: TrustLevel::UntrustedRepoContent,
        }
    }

    /// Render into a secure XML trust envelope.
    pub fn to_trust_envelope(&self) -> String {
        TrustEnvelope::wrap_untrusted(
            &format!("artifact://{}/{}", self.source_step, self.artifact_name),
            self.trust_level,
            &self.excerpt,
        )
    }
}

/// Tool execution output evidence (P2/P3, UntrustedToolOutput).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolOutputEvidence {
    /// Name of tool executed.
    pub tool_name: String,
    /// Invocating call identifier.
    pub call_id: String,
    /// Output payload (stdout or stderr).
    pub output: String,
    /// Whether execution resulted in an error.
    pub is_error: bool,
    /// Trust classification.
    pub trust_level: TrustLevel,
}

impl ToolOutputEvidence {
    /// Construct new tool output evidence.
    pub fn new(
        tool_name: impl Into<String>,
        call_id: impl Into<String>,
        output: impl Into<String>,
        is_error: bool,
    ) -> Self {
        Self {
            tool_name: tool_name.into(),
            call_id: call_id.into(),
            output: output.into(),
            is_error,
            trust_level: TrustLevel::UntrustedToolOutput,
        }
    }

    /// Render into a secure XML trust envelope.
    pub fn to_trust_envelope(&self) -> String {
        TrustEnvelope::wrap_untrusted(
            &format!("tool://{}/{}", self.tool_name, self.call_id),
            self.trust_level,
            &self.output,
        )
    }
}

/// Relevant repository file context (P4, UntrustedRepoContent).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoFileContext {
    /// Repository path.
    pub path: String,
    /// File content excerpt.
    pub content: String,
    /// Trust classification.
    pub trust_level: TrustLevel,
}

impl RepoFileContext {
    /// Construct new repo file context.
    pub fn new(path: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            content: content.into(),
            trust_level: TrustLevel::UntrustedRepoContent,
        }
    }

    /// Render into a secure XML trust envelope.
    pub fn to_trust_envelope(&self) -> String {
        TrustEnvelope::wrap_untrusted(
            &format!("file://{}", self.path),
            self.trust_level,
            &self.content,
        )
    }
}

/// Repository context and topology (P3/P4, UntrustedRepoContent).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoContext {
    /// Repository structure or directory topology description.
    pub topology_summary: Option<String>,
    /// Selected relevant source files.
    pub relevant_files: Vec<RepoFileContext>,
    /// AST symbol or query summary.
    pub ast_context: Option<String>,
}

/// Quality gate assertions and acceptance rules (P1, TrustedSystem).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualityGateContext {
    /// Verification assertion strings.
    pub assertions: Vec<String>,
}

/// Byte budget settings for prompt compilation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PromptBudget {
    /// Hard limit on total assembled prompt bytes.
    pub max_total_bytes: usize,
    /// Limit on parameter binding bytes.
    pub max_parameter_bytes: usize,
    /// Limit on template rendered body bytes.
    pub max_rendered_bytes: usize,
}

impl Default for PromptBudget {
    fn default() -> Self {
        Self {
            max_total_bytes: 524_288,                 // 512 KB
            max_parameter_bytes: MAX_PARAMETER_BYTES, // 2 MB
            max_rendered_bytes: MAX_RENDERED_BYTES,   // 5 MB
        }
    }
}

/// Typed, trust-classified runtime context prepared by the runtime for prompt compilation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PromptContext {
    /// Unique context session identifier.
    pub context_id: String,
    /// Owning mission identifier.
    pub mission_id: String,
    /// Active task identifier.
    pub task_id: String,
    /// Assigned agent role.
    pub role: AgentRole,
    /// Current workflow stage.
    pub stage: MissionStage,
    /// Layer 0 trusted system invariants.
    pub trusted_system: TrustedSystemContext,
    /// Layer 0/1 trusted policy settings.
    pub trusted_policy: TrustedPolicyContext,
    /// Untrusted user intent, if supplied.
    pub user_input: Option<UserInputContext>,
    /// Layer 2 workflow task objective and criteria.
    pub task_objective: TaskObjectiveContext,
    /// Layer 3 durable project memory.
    pub durable_state: DurableStateContext,
    /// Layer 4 upstream artifact evidence.
    pub upstream_artifacts: Vec<ArtifactEvidence>,
    /// Layer 4 tool execution outputs.
    pub tool_outputs: Vec<ToolOutputEvidence>,
    /// Layer 5 repository context.
    pub repo_context: RepoContext,
    /// Layer 6 quality gate verification assertions.
    pub quality_gate: QualityGateContext,
    /// Custom parameters for contract template binding.
    pub custom_parameters: BTreeMap<String, String>,
    /// Size and budget configuration.
    pub budget: PromptBudget,
}

impl PromptContext {
    /// Create a new minimal PromptContext with mandatory execution coordinates.
    pub fn new(
        context_id: impl Into<String>,
        mission_id: impl Into<String>,
        task_id: impl Into<String>,
        role: AgentRole,
        stage: MissionStage,
        task_objective: impl Into<String>,
    ) -> Self {
        Self {
            context_id: context_id.into(),
            mission_id: mission_id.into(),
            task_id: task_id.into(),
            role,
            stage,
            trusted_system: TrustedSystemContext::default(),
            trusted_policy: TrustedPolicyContext::default(),
            user_input: None,
            task_objective: TaskObjectiveContext::new(task_objective),
            durable_state: DurableStateContext::default(),
            upstream_artifacts: Vec::new(),
            tool_outputs: Vec::new(),
            repo_context: RepoContext::default(),
            quality_gate: QualityGateContext::default(),
            custom_parameters: BTreeMap::new(),
            budget: PromptBudget::default(),
        }
    }

    /// Attach untrusted user input intent.
    pub fn with_user_intent(mut self, intent: impl Into<String>) -> Self {
        self.user_input = Some(UserInputContext::new(intent));
        self
    }

    /// Attach Layer 3 project charter.
    pub fn with_charter(mut self, charter: impl Into<String>) -> Self {
        self.durable_state.project_charter = Some(charter.into());
        self
    }

    /// Attach Layer 3 requirements.
    pub fn with_requirements(mut self, requirements: impl Into<String>) -> Self {
        self.durable_state.requirements = Some(requirements.into());
        self
    }

    /// Attach Layer 3 architecture specification.
    pub fn with_architecture(mut self, architecture: impl Into<String>) -> Self {
        self.durable_state.architecture = Some(architecture.into());
        self
    }

    /// Add an upstream artifact to Layer 4 evidence.
    pub fn with_upstream_artifact(
        mut self,
        source_step: impl Into<String>,
        artifact_name: impl Into<String>,
        excerpt: impl Into<String>,
    ) -> Self {
        self.upstream_artifacts
            .push(ArtifactEvidence::new(source_step, artifact_name, excerpt));
        self
    }

    /// Add a tool execution result to Layer 4 evidence.
    pub fn with_tool_output(
        mut self,
        tool_name: impl Into<String>,
        call_id: impl Into<String>,
        output: impl Into<String>,
        is_error: bool,
    ) -> Self {
        self.tool_outputs.push(ToolOutputEvidence::new(
            tool_name, call_id, output, is_error,
        ));
        self
    }

    /// Attach repository topology to Layer 5.
    pub fn with_repo_topology(mut self, topology: impl Into<String>) -> Self {
        self.repo_context.topology_summary = Some(topology.into());
        self
    }

    /// Add a relevant repository file to Layer 5.
    pub fn with_repo_file(mut self, path: impl Into<String>, content: impl Into<String>) -> Self {
        self.repo_context
            .relevant_files
            .push(RepoFileContext::new(path, content));
        self
    }

    /// Attach Layer 6 quality gate verification assertions.
    pub fn with_quality_gate(mut self, assertions: Vec<String>) -> Self {
        self.quality_gate.assertions = assertions;
        self
    }

    /// Add a custom parameter binding for template evaluation.
    pub fn with_parameter(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.custom_parameters.insert(key.into(), value.into());
        self
    }

    /// Configure prompt byte budget.
    pub fn with_budget(mut self, budget: PromptBudget) -> Self {
        self.budget = budget;
        self
    }

    /// Returns true if the context represents an active recovery or diagnostic loop.
    pub fn is_in_recovery(&self) -> bool {
        self.stage == MissionStage::Diagnose
            || self.tool_outputs.iter().any(|t| t.is_error)
            || self
                .task_objective
                .task_objective
                .to_lowercase()
                .contains("recovery")
            || self
                .task_objective
                .task_objective
                .to_lowercase()
                .contains("diagnos")
    }

    /// Returns true if the task has high architectural or contextual complexity.
    pub fn is_high_complexity(&self) -> bool {
        self.upstream_artifacts.len() >= 2
            || self.repo_context.relevant_files.len() >= 3
            || self.quality_gate.assertions.len() >= 3
            || self.task_objective.task_objective.len() >= 250
    }

    /// Extract resolved parameters map suitable for template rendering.
    pub fn extract_parameters(&self) -> BTreeMap<String, String> {
        let mut params = self.custom_parameters.clone();

        // Standard context parameter bindings
        params
            .entry("mission_id".to_string())
            .or_insert_with(|| self.mission_id.clone());
        params
            .entry("task_id".to_string())
            .or_insert_with(|| self.task_id.clone());
        params
            .entry("task_spec".to_string())
            .or_insert_with(|| self.task_objective.task_objective.clone());
        params
            .entry("task_objective".to_string())
            .or_insert_with(|| self.task_objective.task_objective.clone());
        params
            .entry("turn_number".to_string())
            .or_insert_with(|| self.trusted_system.current_step.to_string());

        if let Some(user_in) = &self.user_input {
            params
                .entry("user_intent".to_string())
                .or_insert_with(|| user_in.raw_intent.clone());
        }

        if let Some(charter) = &self.durable_state.project_charter {
            params
                .entry("charter".to_string())
                .or_insert_with(|| charter.clone());
        }

        // Canonical V2 contract bindings
        params
            .entry("task_title".to_string())
            .or_insert_with(|| self.task_objective.task_objective.clone());
        params
            .entry("task_description".to_string())
            .or_insert_with(|| self.task_objective.task_objective.clone());
        params
            .entry("acceptance_criteria".to_string())
            .or_insert_with(|| {
                if self.task_objective.task_criteria.is_empty() {
                    "Verify implementation passes all automated tests without regression."
                        .to_string()
                } else {
                    self.task_objective.task_criteria.join("\n")
                }
            });
        params
            .entry("git_diff".to_string())
            .or_insert_with(|| "No staged git diff available.".to_string());
        params
            .entry("test_command".to_string())
            .or_insert_with(|| "cargo test".to_string());
        params
            .entry("error_message".to_string())
            .or_insert_with(|| "No failure recorded.".to_string());
        params
            .entry("stderr_snippet".to_string())
            .or_insert_with(|| "None".to_string());

        params
    }

    /// Compute a deterministic cryptographic SHA-256 digest of this runtime context manifest.
    ///
    /// The digest strictly captures all semantic inputs (role, stage, mission/task, safety invariants,
    /// policy sandbox, user input hash, task objective/criteria, durable state, upstream evidence,
    /// repo files, quality gate assertions, and custom parameters) while omitting nondeterministic
    /// runtime coordinates (timestamps, memory addresses, process IDs).
    pub fn compute_context_digest(&self) -> String {
        let mut hasher = Sha256::new();

        hasher.update(b"mission:");
        hasher.update(self.mission_id.as_bytes());
        hasher.update(b"|task:");
        hasher.update(self.task_id.as_bytes());
        hasher.update(b"|role:");
        hasher.update(self.role.as_str().as_bytes());
        hasher.update(b"|stage:");
        hasher.update(self.stage.as_str().as_bytes());

        hasher.update(b"|trusted_system:");
        hasher.update(self.trusted_system.safety_invariants.as_bytes());

        hasher.update(b"|trusted_policy:");
        hasher.update(self.trusted_policy.sandbox_mode.as_bytes());
        hasher.update(b":");
        hasher.update(self.trusted_policy.workspace_root.as_bytes());
        for path in &self.trusted_policy.blocked_paths {
            hasher.update(b";blocked:");
            hasher.update(path.as_bytes());
        }

        hasher.update(b"|user_input:");
        if let Some(user_in) = &self.user_input {
            hasher.update(user_in.content_hash.as_bytes());
        } else {
            hasher.update(b"none");
        }

        hasher.update(b"|task_objective:");
        hasher.update(self.task_objective.task_objective.as_bytes());
        for criterion in &self.task_objective.task_criteria {
            hasher.update(b";criterion:");
            hasher.update(criterion.as_bytes());
        }
        if let Some(format) = &self.task_objective.expected_output_format {
            hasher.update(b";format:");
            hasher.update(format.as_bytes());
        }

        hasher.update(b"|durable_state:");
        if let Some(charter) = &self.durable_state.project_charter {
            hasher.update(b"charter:");
            hasher.update(charter.as_bytes());
        }
        if let Some(reqs) = &self.durable_state.requirements {
            hasher.update(b"|reqs:");
            hasher.update(reqs.as_bytes());
        }
        if let Some(arch) = &self.durable_state.architecture {
            hasher.update(b"|arch:");
            hasher.update(arch.as_bytes());
        }
        if let Some(state) = &self.durable_state.state_summary {
            hasher.update(b"|state:");
            hasher.update(state.as_bytes());
        }

        hasher.update(b"|upstream_artifacts:");
        let mut sorted_artifacts: Vec<_> = self.upstream_artifacts.iter().collect();
        sorted_artifacts.sort_by(|a, b| {
            (&a.source_step, &a.artifact_name).cmp(&(&b.source_step, &b.artifact_name))
        });
        for a in sorted_artifacts {
            hasher.update(a.source_step.as_bytes());
            hasher.update(b":");
            hasher.update(a.artifact_name.as_bytes());
            hasher.update(b":");
            hasher.update(a.content_hash.as_bytes());
            hasher.update(b";");
        }

        hasher.update(b"|tool_outputs:");
        let mut sorted_tools: Vec<_> = self.tool_outputs.iter().collect();
        sorted_tools.sort_by(|a, b| (&a.tool_name, &a.call_id).cmp(&(&b.tool_name, &b.call_id)));
        for t in sorted_tools {
            hasher.update(t.tool_name.as_bytes());
            hasher.update(b":");
            hasher.update(t.call_id.as_bytes());
            hasher.update(b":");
            hasher.update(t.output.as_bytes());
            hasher.update(b":err=");
            hasher.update(if t.is_error { b"1" } else { b"0" });
            hasher.update(b";");
        }

        hasher.update(b"|repo_context:");
        if let Some(topo) = &self.repo_context.topology_summary {
            hasher.update(topo.as_bytes());
        }
        let mut sorted_files: Vec<_> = self.repo_context.relevant_files.iter().collect();
        sorted_files.sort_by(|a, b| a.path.cmp(&b.path));
        for f in sorted_files {
            hasher.update(b"file:");
            hasher.update(f.path.as_bytes());
            hasher.update(b":");
            hasher.update(f.content.as_bytes());
            hasher.update(b";");
        }
        if let Some(ast) = &self.repo_context.ast_context {
            hasher.update(ast.as_bytes());
        }

        hasher.update(b"|quality_gate:");
        let mut sorted_assertions = self.quality_gate.assertions.clone();
        sorted_assertions.sort();
        for a in sorted_assertions {
            hasher.update(a.as_bytes());
            hasher.update(b";");
        }

        hasher.update(b"|custom_parameters:");
        for (k, v) in &self.custom_parameters {
            hasher.update(k.as_bytes());
            hasher.update(b"=");
            hasher.update(v.as_bytes());
            hasher.update(b";");
        }

        format!("{:x}", hasher.finalize())
    }
}
