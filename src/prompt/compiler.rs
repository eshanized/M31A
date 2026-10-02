//! PromptCompiler: Deterministic 7-layer composition and trust-aware prompt compilation pipeline.
//!
//! # Responsibilities & Architectural Ownership
//! The `PromptCompiler` is the canonical engine that transforms an abstract [`PromptContract`]
//! and structured runtime [`PromptContext`] into an [`EffectivePrompt`].
//!
//! # The Seven Canonical Layers (L0 through L6)
//! The compiler enforces a strictly ordered, non-reorderable 7-layer composition model:
//! 1. **L0 — Runtime Safety Invariants:** Immutable P0 constraints (`RUNTIME_SAFETY_INVARIANTS`),
//!    unprunable, prepended unconditionally before any other content.
//! 2. **L1 — Agent Role & Profile:** Operating role identity and stage constraints (P1, unprunable).
//! 3. **L2 — Workflow Step Objective & Contract:** Current task purpose, acceptance criteria,
//!    expected output format, and user intent wrapped in `<user_intent>` (P1, unprunable).
//! 4. **L3 — Project Charter & Durable State:** `.planning/` project memory extracts (P2, compactable).
//! 5. **L4 — Upstream Artifact Evidence & Tool Results:** Prior step outputs and tool execution logs
//!    wrapped in `<untrusted_evidence>` with closing tag escaping (P2/P3, compactable).
//! 6. **L5 — Repository Topology & Context:** Repository file extracts and AST summaries (P3/P4, disposable).
//! 7. **L6 — Quality Gate & Verification Checklist:** Acceptance assertions and checklist items (P1, unprunable).
//!
//! # Trust Handling & Delimiter Smuggling Defense
//! External and untrusted data (user intent, tool outputs, repository files, git diffs) are
//! encapsulated in typed XML trust envelopes (`<user_intent>` and `<untrusted_evidence>`).
//! Any occurrences of closing tags within payload text are escaped to prevent prompt injection
//! delimiter hijacking. External content never gains system-level instruction authority.
//!
//! # Deterministic Compilation & Hashing
//! Compilation is strictly deterministic. On identical contracts, versions, parameters, and contexts,
//! `PromptCompiler` produces byte-for-byte identical assembled prompts and cryptographic SHA-256
//! `content_hash` values. No random UUIDs, timestamps, or unordered collections are used in hashing.
//!
//! # Context Budgeting & Reverse Compaction
//! When total assembled prompt bytes exceed configured budgets, the compiler applies deterministic
//! reverse compaction (dropping lowest-priority disposable context first: L5 -> L4 -> L3).
//! Protected layers (L0, L1, L2, L6) are never dropped. If protected layers exceed budget, compilation
//! fails closed with [`PromptError::PromptBudgetExceeded`].
//!
//! # Architectural Relationships
//! - **PromptCatalog:** Sourcing authority for versioned prompt contract assets.
//! - **ModelGateway:** Downstream consumer of `EffectivePrompt`. The compiler does NOT invoke LLMs,
//!   select models, or format provider-specific wire JSON.
//! - **Runtime Authority:** The core invariant remains: *"The model proposes. The runtime decides."*
//!   `PromptCompiler` never grants permissions, approves tool calls, or changes workflow state.

use crate::context::envelope::{TrustEnvelope, TrustLevel};
use crate::prompt::composer::{PromptLayerEntry, PromptLayerKind};
use crate::prompt::context::PromptContext;
use crate::prompt::contract::{PromptContract, RUNTIME_SAFETY_INVARIANTS};
use crate::prompt::error::PromptError;
use crate::prompt::model_profile::ModelProfile;
use crate::prompt::parameter::MAX_PARAMETER_BYTES;
use crate::prompt::provenance::{
    PromptCompactionMetadata, PromptInvocationProvenance, PromptLayerMetadata, PromptProvenance,
    PromptSourceKind,
};
use crate::prompt::renderer::render_prompt;
use crate::prompt::strategy::{PromptStrategy, resolve_strategy};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashSet};

/// Compilation options controlling parameter validation, section partitioning, and budget overrides.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompilationOptions {
    /// Whether parameters not declared in the contract should cause compilation failure.
    pub strict_parameters: bool,
    /// Whether to separate system layers (L0, L1) from user/task layers (L2..L6).
    pub separate_system_user: bool,
    /// Optional total byte budget override (defaults to PromptContext.budget.max_total_bytes).
    pub max_total_bytes: Option<usize>,
    /// Optional ModelProfile describing target model capabilities and limits.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_profile: Option<ModelProfile>,
    /// Optional explicit PromptStrategy override.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strategy: Option<PromptStrategy>,
    /// Optional origin source kind of the contract being compiled.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_kind: Option<PromptSourceKind>,
}

impl Default for CompilationOptions {
    fn default() -> Self {
        Self {
            strict_parameters: false,
            separate_system_user: true,
            max_total_bytes: None,
            model_profile: None,
            strategy: None,
            source_kind: None,
        }
    }
}

impl CompilationOptions {
    /// Builder method to specify a target ModelProfile.
    pub fn with_profile(mut self, profile: ModelProfile) -> Self {
        self.model_profile = Some(profile);
        self
    }

    /// Builder method to specify an explicit PromptStrategy override.
    pub fn with_strategy(mut self, strategy: PromptStrategy) -> Self {
        self.strategy = Some(strategy);
        self
    }

    /// Builder method to specify a byte budget ceiling.
    pub fn with_budget(mut self, max_bytes: usize) -> Self {
        self.max_total_bytes = Some(max_bytes);
        self
    }

    /// Builder method to toggle strict parameter validation.
    pub fn with_strict_parameters(mut self, strict: bool) -> Self {
        self.strict_parameters = strict;
        self
    }

    /// Builder method to specify the origin source kind of the contract.
    pub fn with_source_kind(mut self, source_kind: PromptSourceKind) -> Self {
        self.source_kind = Some(source_kind);
        self
    }
}

/// The fully compiled, trust-delimited, budget-enforced effective prompt ready for model gateway dispatch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EffectivePrompt {
    /// Prompt contract identifier.
    pub prompt_id: String,
    /// Prompt contract version number.
    pub prompt_version: u32,
    /// Content hash of the underlying PromptContract.
    pub contract_hash: String,
    /// Primary system prompt (L0 Safety Invariants and L1 Role Profile).
    pub system_prompt: String,
    /// Primary user/task prompt (L2 Objective, L3 Durable State, L4 Evidence, L5 Repo, L6 Quality Gate).
    pub user_prompt: Option<String>,
    /// Full assembled prompt text combining all layers in strict L0..L6 order.
    pub assembled_text: String,
    /// Structured breakdown of all active layers.
    pub layers: Vec<PromptLayerEntry>,
    /// Cryptographic composite SHA-256 content hash of the compiled prompt.
    pub content_hash: String,
    /// Total byte length of assembled prompt text.
    pub total_bytes: usize,
    /// Declared expected output format, if specified.
    pub expected_output_format: Option<String>,
    /// Parameters that were bound during compilation.
    pub supplied_parameters: Vec<String>,
    /// Effective prompt strategy applied during compilation.
    pub strategy: PromptStrategy,
    /// Target model profile identifier if adapted for a specific model.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_profile_id: Option<String>,
    /// Full cryptographic provenance and compaction telemetry record.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenance: Option<PromptProvenance>,
    /// Origin source kind of the resolved prompt contract.
    #[serde(default)]
    pub source_kind: PromptSourceKind,
}

impl EffectivePrompt {
    /// Return lightweight invocation provenance if available.
    pub fn invocation_provenance(&self) -> Option<PromptInvocationProvenance> {
        self.provenance
            .as_ref()
            .map(|p| p.to_invocation_provenance())
    }

    /// Return the origin source kind of the contract.
    pub fn source_kind(&self) -> PromptSourceKind {
        self.source_kind
    }
}

/// Formal PromptOS compilation engine trait.
pub trait PromptCompiler: Send + Sync {
    /// Compile a PromptContract and PromptContext into an EffectivePrompt.
    fn compile(
        &self,
        contract: &PromptContract,
        context: &PromptContext,
        options: &CompilationOptions,
    ) -> Result<EffectivePrompt, PromptError>;

    /// Compile with lower-trust project guidance injection (P0-04).
    ///
    /// Default: compiles the authoritative contract, then appends recorded
    /// repository guidance for the resolved contract identity as a prunable,
    /// `UntrustedRepoContent` L5 layer on the user side only. The trusted
    /// system prompt is byte-identical with or without guidance.
    fn compile_with_guidance(
        &self,
        catalog: &dyn crate::prompt::catalog::PromptCatalog,
        contract: &PromptContract,
        context: &PromptContext,
        options: &CompilationOptions,
    ) -> Result<EffectivePrompt, PromptError> {
        let mut effective = self.compile(contract, context, options)?;
        // Guidance is keyed by the resolved contract identity; also consult
        // the caller-visible identity when canonicalization upgraded it.
        let mut guidance = catalog.project_guidance_for(&contract.id, contract.version);
        if contract.version != 1 {
            guidance.extend(catalog.project_guidance_for(&contract.id, 1));
        }
        if guidance.is_empty() {
            return Ok(effective);
        }
        let mut sections = Vec::new();
        for entry in &guidance {
            let source = format!(
                "project-guidance:{}:{}",
                entry.source_kind,
                entry.source_path.as_deref().unwrap_or("workspace")
            );
            sections.push(TrustEnvelope::wrap_untrusted(
                &source,
                TrustLevel::UntrustedRepoContent,
                &entry.contract.template_body,
            ));
        }
        let guidance_text = sections.join("\n");
        let layer = PromptLayerEntry::new(
            PromptLayerKind::L5RepoContext,
            "project_guidance",
            guidance_text.clone(),
            TrustLevel::UntrustedRepoContent,
        );
        effective.layers.push(layer);
        let section = format!("## project_guidance\n{guidance_text}\n\n");
        effective.assembled_text.push_str(&section);
        effective.user_prompt = Some(match effective.user_prompt.take() {
            Some(existing) => format!("{existing}\n{section}").trim_end().to_string(),
            None => section.trim_end().to_string(),
        });
        effective.total_bytes = effective.assembled_text.len();
        Ok(effective)
    }
}

/// Canonical production implementation of `PromptCompiler`.
#[derive(Debug, Default, Clone)]
pub struct DefaultPromptCompiler;

impl DefaultPromptCompiler {
    /// Create a new DefaultPromptCompiler.
    pub fn new() -> Self {
        Self
    }

    /// Compile a contract directly by resolving it from the provided PromptCatalog.
    pub fn compile_from_catalog(
        &self,
        catalog: &dyn crate::prompt::catalog::PromptCatalog,
        id: &str,
        version: u32,
        context: &PromptContext,
        options: &CompilationOptions,
    ) -> Result<EffectivePrompt, PromptError> {
        let contract = catalog.resolve_canonical(id, version)?;
        let mut opts = options.clone();
        if opts.source_kind.is_none() {
            opts.source_kind = catalog.source_kind(&contract.id, contract.version);
        }
        self.compile(contract, context, &opts)
    }

    /// Compile with lower-trust project guidance injection (P0-04).
    ///
    /// Effective structure enforced:
    ///
    /// ```text
    /// TRUSTED SYSTEM INSTRUCTIONS (L0/L1, system_prompt, untouched)
    ///     + TRUSTED RUNTIME SAFETY CONTRACTS (L0, untouched)
    ///     + UNTRUSTED PROJECT GUIDANCE (appended L5 layer, user_prompt only)
    ///     + CURRENT USER INTENT (task layers, untouched)
    /// ```
    ///
    /// Repository files targeting the requested behavioral contract ID NEVER
    /// replace the built-in role/stage contract: `resolve_canonical` returns
    /// the built-in, and recorded guidance is appended as a prunable,
    /// `UntrustedRepoContent` L5 layer with explicit delimiters. The
    /// authoritative system prompt is byte-identical with or without guidance.
    pub fn compile_from_catalog_with_guidance(
        &self,
        catalog: &dyn crate::prompt::catalog::PromptCatalog,
        id: &str,
        version: u32,
        context: &PromptContext,
        options: &CompilationOptions,
    ) -> Result<EffectivePrompt, PromptError> {
        let contract = catalog.resolve_canonical(id, version)?.clone();
        let mut opts = options.clone();
        if opts.source_kind.is_none() {
            opts.source_kind = catalog.source_kind(&contract.id, contract.version);
        }
        // Single guidance implementation: PromptCompiler::compile_with_guidance
        // (trait default). The authoritative system prompt is invariant there.
        PromptCompiler::compile_with_guidance(self, catalog, &contract, context, &opts)
    }

    /// Calculate deterministic composite hash of an assembled prompt including strategy and model profile.
    pub fn calculate_effective_hash(
        contract: &PromptContract,
        parameters: &BTreeMap<String, String>,
        layers: &[PromptLayerEntry],
        strategy: PromptStrategy,
        model_profile_id: Option<&str>,
    ) -> String {
        let mut hasher = Sha256::new();
        hasher.update(contract.id.as_bytes());
        hasher.update(b":v");
        hasher.update(contract.version.to_be_bytes());
        hasher.update(b":");
        hasher.update(contract.content_hash.as_bytes());
        hasher.update(b"|strategy:");
        hasher.update(strategy.as_str().as_bytes());
        if let Some(profile_id) = model_profile_id {
            hasher.update(b"|model:");
            hasher.update(profile_id.as_bytes());
        }
        hasher.update(b"|params:");

        for (k, v) in parameters {
            hasher.update(k.as_bytes());
            hasher.update(b"=");
            hasher.update(v.as_bytes());
            hasher.update(b";");
        }

        hasher.update(b"|layers:");
        for layer in layers {
            hasher.update(layer.kind.as_str().as_bytes());
            hasher.update(b":");
            hasher.update(layer.content.as_bytes());
            hasher.update(b";");
        }

        format!("{:x}", hasher.finalize())
    }

    /// Calculate deterministic hash using standard strategy and no model profile override.
    pub fn calculate_legacy_hash(
        contract: &PromptContract,
        parameters: &BTreeMap<String, String>,
        layers: &[PromptLayerEntry],
    ) -> String {
        Self::calculate_effective_hash(contract, parameters, layers, PromptStrategy::Standard, None)
    }
}

impl PromptCompiler for DefaultPromptCompiler {
    fn compile(
        &self,
        contract: &PromptContract,
        context: &PromptContext,
        options: &CompilationOptions,
    ) -> Result<EffectivePrompt, PromptError> {
        // Step 1: Validate Contract Invariants
        if contract.version == 0 || contract.id.trim().is_empty() {
            return Err(PromptError::PromptInvalid {
                id: contract.id.clone(),
                version: contract.version,
                reason: "contract has invalid id or zero version".to_string(),
            });
        }

        // Step 2: Extract and Validate Parameters
        let mut resolved_params = context.custom_parameters.clone();

        for param in &contract.input_parameters {
            if !resolved_params.contains_key(&param.name) {
                match param.name.as_str() {
                    "task_spec" | "task_objective" | "task_title" | "task_description" | "goal" => {
                        resolved_params.insert(
                            param.name.clone(),
                            context.task_objective.task_objective.clone(),
                        );
                    }
                    "mission_id" => {
                        resolved_params.insert(param.name.clone(), context.mission_id.clone());
                    }
                    "task_id" => {
                        resolved_params.insert(param.name.clone(), context.task_id.clone());
                    }
                    "turn_number" => {
                        resolved_params.insert(
                            param.name.clone(),
                            context.trusted_system.current_step.to_string(),
                        );
                    }
                    "user_intent" => {
                        if let Some(user_in) = &context.user_input {
                            resolved_params.insert(param.name.clone(), user_in.raw_intent.clone());
                        }
                    }
                    "charter" => {
                        if let Some(charter) = &context.durable_state.project_charter {
                            resolved_params.insert(param.name.clone(), charter.clone());
                        }
                    }
                    "architecture" => {
                        if let Some(arch) = &context.durable_state.architecture {
                            resolved_params.insert(param.name.clone(), arch.clone());
                        } else {
                            resolved_params.insert(
                                param.name.clone(),
                                "Target system architecture derived from requirements.".to_string(),
                            );
                        }
                    }
                    "acceptance_criteria" => {
                        let criteria_val = if context.task_objective.task_criteria.is_empty() {
                            "Verify implementation passes all automated tests without regression."
                                .to_string()
                        } else {
                            context.task_objective.task_criteria.join("\n")
                        };
                        resolved_params.insert(param.name.clone(), criteria_val);
                    }
                    "git_diff" => {
                        resolved_params.insert(
                            param.name.clone(),
                            "No staged git diff available.".to_string(),
                        );
                    }
                    "test_command" => {
                        resolved_params.insert(param.name.clone(), "cargo test".to_string());
                    }
                    _ => {}
                }
            }
        }

        // Parameter byte size ceiling check
        let total_param_bytes: usize = resolved_params.iter().map(|(k, v)| k.len() + v.len()).sum();
        if total_param_bytes > MAX_PARAMETER_BYTES {
            return Err(PromptError::PromptRenderFailure {
                prompt_id: contract.id.clone(),
                reason: format!(
                    "parameters byte size ({}) exceeds maximum permitted bound ({})",
                    total_param_bytes, MAX_PARAMETER_BYTES
                ),
            });
        }

        // Check required parameters exist
        for param in &contract.input_parameters {
            if !resolved_params.contains_key(&param.name) {
                if let Some(default) = &param.default_value {
                    resolved_params.insert(param.name.clone(), default.clone());
                } else if param.is_required {
                    return Err(PromptError::PromptParameterMissing {
                        prompt_id: contract.id.clone(),
                        parameter: param.name.clone(),
                    });
                }
            }
        }

        // Check strict parameter bounds if requested
        if options.strict_parameters {
            let declared: HashSet<&str> = contract
                .input_parameters
                .iter()
                .map(|p| p.name.as_str())
                .collect();
            for key in resolved_params.keys() {
                if !declared.contains(key.as_str()) {
                    return Err(PromptError::PromptParameterInvalid {
                        prompt_id: contract.id.clone(),
                        parameter: key.clone(),
                        reason: "parameter not declared in prompt contract".to_string(),
                    });
                }
            }
        }

        // Step 3: Render Contract Template with MiniJinja 2
        let rendered_contract =
            render_prompt(contract, &resolved_params, options.strict_parameters)?;

        // Step 4: Resolve PromptStrategy & ModelProfile
        let requested_strat = options.strategy.or(contract.strategy);
        let (effective_strategy, effective_profile_id) =
            match (&options.model_profile, requested_strat) {
                (Some(profile), Some(strat)) => {
                    let resolved = resolve_strategy(
                        context.role.clone(),
                        context.stage,
                        context.is_high_complexity(),
                        context.is_in_recovery(),
                        profile,
                        Some(strat),
                    );
                    (resolved, Some(profile.model_id.clone()))
                }
                (Some(profile), None) => {
                    let resolved = resolve_strategy(
                        context.role.clone(),
                        context.stage,
                        context.is_high_complexity(),
                        context.is_in_recovery(),
                        profile,
                        None,
                    );
                    (resolved, Some(profile.model_id.clone()))
                }
                (None, Some(strat)) => (strat, None),
                (None, None) => {
                    let strat = if context.is_in_recovery() {
                        PromptStrategy::Recovery
                    } else {
                        PromptStrategy::Standard
                    };
                    (strat, None)
                }
            };

        // Step 5: Assemble 7 Canonical Layers
        // Layer 0: Runtime Safety Invariants (P0, immutable, unprunable)
        let l0 = PromptLayerEntry::new(
            PromptLayerKind::L0Safety,
            "SYSTEM INVARIANTS (P0)",
            RUNTIME_SAFETY_INVARIANTS,
            TrustLevel::TrustedSystem,
        );

        // Layer 1: Agent Role & Profile (P1, unprunable)
        let role_text = format!(
            "You are the M31A {} agent.\nOperating Stage: {}\nMission: {}\nTask: {}",
            context.role.as_str(),
            context.stage.as_str(),
            context.mission_id,
            context.task_id
        );
        let l1 = PromptLayerEntry::new(
            PromptLayerKind::L1Role,
            "AGENT ROLE & PROFILE (P1)",
            role_text,
            TrustLevel::TrustedSystem,
        );

        // Layer 2: Workflow Step Objective & Output Contract (P1, unprunable)
        let mut l2_text = String::new();
        l2_text.push_str("### Step Objective & Contract:\n");
        l2_text.push_str(&rendered_contract.rendered_text);

        // Strategy-specific execution directives
        let directives = effective_strategy.structural_directives();
        if !directives.is_empty() {
            l2_text.push_str("\n\n");
            l2_text.push_str(directives);
        }

        // Strategy-specific few-shot examples
        if let Some(examples) = effective_strategy.few_shot_examples() {
            l2_text.push_str("\n\n");
            l2_text.push_str(examples);
        }

        // Model-aware structured output and tool invocation adaptation
        if let Some(ref profile) = options.model_profile {
            if profile
                .structured_output_support
                .requires_emulated_guidance()
            {
                l2_text.push_str("\n\n### Structured Output Guidance:\nFormat response strictly adhering to the specified schema, without external commentary or markdown formatting wrappers.");
            }
            if !profile.supports_parallel_tools || profile.tool_calling_fidelity.as_u8() <= 2 {
                l2_text.push_str("\n\n### Tool Invocation Policy:\nExecute exactly one tool action per turn. Parallel tool execution is disabled for this runtime environment.");
            }
        }

        // If user intent is present, safely enclose it inside XML delimiter envelope
        if let Some(user_in) = &context.user_input {
            l2_text.push_str("\n\n### User Intent (Untrusted Input):\n");
            l2_text.push_str(&user_in.to_trust_envelope());
        }

        // V2 Reasoning Policy
        if let Some(ref policy) = contract.reasoning_policy {
            if policy.mandatory_falsification {
                l2_text.push_str("\n\n### Mandatory Falsification Protocol:\nYou must explicitly search for counterexamples, failure modes, edge cases, and falsification vectors before proposing or finalizing conclusions. Do not declare success without attempting to disprove your assertions.");
            }
            if policy.claim_categorization {
                l2_text.push_str("\n\n### Epistemic Claim Categorization:\nEvery technical claim must be explicitly tagged with its epistemic status:\n- [FACT]: Verified directly against codebase or tool execution output.\n- [INFERENCE]: Derived logically from verified facts.\n- [ASSUMPTION]: Unverified premise requiring runtime validation.\n- [PROPOSAL]: Proposed change or implementation step awaiting runtime decision.");
            }
            if let Some(cot) = policy.chain_of_thought_budget {
                l2_text.push_str(&format!(
                    "\n\n### Reasoning Budget:\nTarget chain-of-thought within {} tokens.",
                    cot
                ));
            }
        }

        // V2 Evidence Requirements
        if let Some(ref ev) = contract.evidence_requirements {
            if !ev.trusted_classes.is_empty() {
                l2_text.push_str("\n\n### Mandatory Evidence Requirements:\nCompletion requires genuine empirical evidence. Trusted evidence classes:\n");
                for req in &ev.trusted_classes {
                    l2_text.push_str(&format!("- {}\n", req));
                }
                if ev.min_evidence_count > 0 {
                    l2_text.push_str(&format!(
                        "Minimum distinct evidence items required: {}\n",
                        ev.min_evidence_count
                    ));
                }
            }
            if !ev.forbidden_assumptions.is_empty() {
                l2_text.push_str(
                    "\nForbidden assumptions (must be verified empirically, never assumed):\n",
                );
                for fa in &ev.forbidden_assumptions {
                    l2_text.push_str(&format!("- {}\n", fa));
                }
            }
        }

        // V2 Failure Policy
        if let Some(ref fp) = contract.failure_policy {
            l2_text.push_str(&format!(
                "\n\n### Failure Policy:\nMax retries: {}. Failure mode: {}. Escalation: {}.",
                fp.max_retries,
                fp.behavior.as_str(),
                fp.escalation_path
            ));
        }

        // If expected output format/contract is defined, append output contract
        if let Some(ref oc) = contract.output_contract {
            let schema_info = oc.schema_uri.as_deref().unwrap_or("unspecified");
            l2_text.push_str(&format!(
                "\n\n### Expected Output Contract:\nResponse kind: '{}'. Format type: '{}'. Schema: '{}'. Strict compliance: {}.",
                oc.response_kind.as_str(),
                oc.format_type,
                schema_info,
                oc.is_strict
            ));
        } else if let Some(format) = &contract.expected_output_format {
            l2_text.push_str(&format!(
                "\n\n### Expected Output Format:\nProduce output conforming to format: {}",
                format
            ));
        } else if let Some(format) = &context.task_objective.expected_output_format {
            l2_text.push_str(&format!(
                "\n\n### Expected Output Format:\nProduce output conforming to format: {}",
                format
            ));
        }

        let l2 = PromptLayerEntry::new(
            PromptLayerKind::L2Objective,
            "WORKFLOW STEP OBJECTIVE & CONTRACT (P1)",
            l2_text,
            TrustLevel::TrustedSystem,
        );

        // Layer 3: Project Charter & Durable State (P2, compactable)
        let mut l3_parts = Vec::new();
        if let Some(charter) = &context.durable_state.project_charter {
            l3_parts.push(format!(
                "#### Project Charter:\n{}",
                TrustEnvelope::wrap_untrusted(
                    "project://PROJECT.md",
                    TrustLevel::UntrustedRepoContent,
                    charter
                )
            ));
        }
        if let Some(reqs) = &context.durable_state.requirements {
            l3_parts.push(format!(
                "#### Requirements:\n{}",
                TrustEnvelope::wrap_untrusted(
                    "project://REQUIREMENTS.md",
                    TrustLevel::UntrustedRepoContent,
                    reqs
                )
            ));
        }
        if let Some(arch) = &context.durable_state.architecture {
            l3_parts.push(format!(
                "#### Architecture:\n{}",
                TrustEnvelope::wrap_untrusted(
                    "project://ARCHITECTURE.md",
                    TrustLevel::UntrustedRepoContent,
                    arch
                )
            ));
        }
        if let Some(state) = &context.durable_state.state_summary {
            l3_parts.push(format!(
                "#### Current State:\n{}",
                TrustEnvelope::wrap_untrusted(
                    "project://STATE.md",
                    TrustLevel::UntrustedRepoContent,
                    state
                )
            ));
        }

        let l3 = if !l3_parts.is_empty() {
            Some(PromptLayerEntry::new(
                PromptLayerKind::L3DurableState,
                "PROJECT CHARTER & BOUNDARIES (P2)",
                l3_parts.join("\n\n"),
                TrustLevel::UntrustedRepoContent,
            ))
        } else {
            None
        };

        // Layer 4: Upstream Artifact Evidence & Tool Results (P2/P3, compactable)
        let mut l4_parts = Vec::new();
        for artifact in &context.upstream_artifacts {
            l4_parts.push(artifact.to_trust_envelope());
        }
        for tool_out in &context.tool_outputs {
            l4_parts.push(tool_out.to_trust_envelope());
        }

        let l4 = if !l4_parts.is_empty() {
            Some(PromptLayerEntry::new(
                PromptLayerKind::L4Evidence,
                "UPSTREAM ARTIFACT EVIDENCE (P2/P3)",
                l4_parts.join("\n\n"),
                TrustLevel::UntrustedRepoContent,
            ))
        } else {
            None
        };

        // Layer 5: Repository Constraints & Context (P3/P4, disposable)
        let mut l5_parts = Vec::new();
        if let Some(topo) = &context.repo_context.topology_summary {
            l5_parts.push(format!(
                "#### Directory Topology:\n{}",
                TrustEnvelope::wrap_untrusted(
                    "repo://topology",
                    TrustLevel::UntrustedRepoContent,
                    topo
                )
            ));
        }
        for file in &context.repo_context.relevant_files {
            l5_parts.push(file.to_trust_envelope());
        }
        if let Some(ast) = &context.repo_context.ast_context {
            l5_parts.push(format!(
                "#### AST Symbols:\n{}",
                TrustEnvelope::wrap_untrusted("repo://ast", TrustLevel::UntrustedRepoContent, ast)
            ));
        }

        let l5 = if !l5_parts.is_empty() {
            Some(PromptLayerEntry::new(
                PromptLayerKind::L5RepoContext,
                "REPOSITORY CONSTRAINTS & CONTEXT (P3/P4)",
                l5_parts.join("\n\n"),
                TrustLevel::UntrustedRepoContent,
            ))
        } else {
            None
        };

        // Layer 6: Quality Gate Assertions (P1, unprunable)
        let mut quality_assertions = context.quality_gate.assertions.clone();
        if let Some(ref vr) = contract.verification_requirements {
            for gate in &vr.required_gates {
                let gate_assertion = format!("Required verification gate: {}", gate);
                if !quality_assertions.contains(&gate_assertion) {
                    quality_assertions.push(gate_assertion);
                }
            }
            for criterion in &vr.acceptance_criteria {
                let crit_assertion = format!("Acceptance criterion: {}", criterion);
                if !quality_assertions.contains(&crit_assertion) {
                    quality_assertions.push(crit_assertion);
                }
            }
        }
        let l6 = if !quality_assertions.is_empty() {
            let checklist = quality_assertions
                .iter()
                .map(|a| format!("- [ ] {}", a))
                .collect::<Vec<_>>()
                .join("\n");
            Some(PromptLayerEntry::new(
                PromptLayerKind::L6QualityGate,
                "QUALITY GATE VERIFICATION CHECKLIST (P1)",
                checklist,
                TrustLevel::TrustedSystem,
            ))
        } else {
            None
        };

        // Step 6: Enforce Context Budget and Reverse Compaction
        let max_budget = if let Some(custom) = options.max_total_bytes {
            custom
        } else if let Some(ref profile) = options.model_profile {
            std::cmp::min(context.budget.max_total_bytes, profile.max_input_bytes())
        } else {
            context.budget.max_total_bytes
        };

        // Required protected layers: L0, L1, L2, L6
        let l0_size = l0.byte_size;
        let l1_size = l1.byte_size;
        let l2_size = l2.byte_size;
        let l6_size = l6.as_ref().map(|l| l.byte_size).unwrap_or(0);
        let protected_bytes = l0_size + l1_size + l2_size + l6_size;

        if protected_bytes > max_budget {
            return Err(PromptError::PromptBudgetExceeded {
                prompt_id: contract.id.clone(),
                size_bytes: protected_bytes,
                max_bytes: max_budget,
                reason: format!(
                    "immutable and protected layers (L0, L1, L2, L6) size ({} bytes) exceed total prompt budget ({} bytes)",
                    protected_bytes, max_budget
                ),
            });
        }

        // Budget remainder for compactable layers (L3, L4, L5)
        // Strict reverse compaction priority: L5 (lowest) dropped first -> L4 dropped next -> L3 dropped last.
        let mut active_layers: Vec<PromptLayerEntry> = Vec::with_capacity(7);
        active_layers.push(l0);
        active_layers.push(l1);
        active_layers.push(l2);

        let l3_size = l3.as_ref().map(|l| l.byte_size).unwrap_or(0);
        let l4_size = l4.as_ref().map(|l| l.byte_size).unwrap_or(0);
        let l5_size = l5.as_ref().map(|l| l.byte_size).unwrap_or(0);

        let (include_l3, include_l4, include_l5) =
            if protected_bytes + l3_size + l4_size + l5_size <= max_budget {
                (true, true, true)
            } else if protected_bytes + l3_size + l4_size <= max_budget {
                (true, true, false)
            } else if protected_bytes + l3_size <= max_budget {
                (true, false, false)
            } else {
                (false, false, false)
            };

        let mut layers_retained = vec![
            PromptLayerKind::L0Safety,
            PromptLayerKind::L1Role,
            PromptLayerKind::L2Objective,
        ];
        let mut layers_dropped = Vec::new();

        if l3.is_some() {
            if include_l3 {
                layers_retained.push(PromptLayerKind::L3DurableState);
            } else {
                layers_dropped.push(PromptLayerKind::L3DurableState);
            }
        }
        if l4.is_some() {
            if include_l4 {
                layers_retained.push(PromptLayerKind::L4Evidence);
            } else {
                layers_dropped.push(PromptLayerKind::L4Evidence);
            }
        }
        if l5.is_some() {
            if include_l5 {
                layers_retained.push(PromptLayerKind::L5RepoContext);
            } else {
                layers_dropped.push(PromptLayerKind::L5RepoContext);
            }
        }
        if l6.is_some() {
            layers_retained.push(PromptLayerKind::L6QualityGate);
        }

        let mut layer_metadata = vec![
            PromptLayerMetadata::new(
                PromptLayerKind::L0Safety,
                "SYSTEM INVARIANTS (P0)",
                l0_size,
                TrustLevel::TrustedSystem,
                true,
                true,
            ),
            PromptLayerMetadata::new(
                PromptLayerKind::L1Role,
                "AGENT ROLE & PROFILE (P1)",
                l1_size,
                TrustLevel::TrustedSystem,
                true,
                true,
            ),
            PromptLayerMetadata::new(
                PromptLayerKind::L2Objective,
                "WORKFLOW STEP OBJECTIVE & CONTRACT (P1)",
                l2_size,
                TrustLevel::TrustedSystem,
                true,
                true,
            ),
        ];

        if let Some(ref layer) = l3 {
            layer_metadata.push(PromptLayerMetadata::new(
                PromptLayerKind::L3DurableState,
                "PROJECT CHARTER & BOUNDARIES (P2)",
                layer.byte_size,
                TrustLevel::UntrustedRepoContent,
                false,
                include_l3,
            ));
        }
        if let Some(ref layer) = l4 {
            layer_metadata.push(PromptLayerMetadata::new(
                PromptLayerKind::L4Evidence,
                "UPSTREAM ARTIFACT EVIDENCE (P2/P3)",
                layer.byte_size,
                TrustLevel::UntrustedRepoContent,
                false,
                include_l4,
            ));
        }
        if let Some(ref layer) = l5 {
            layer_metadata.push(PromptLayerMetadata::new(
                PromptLayerKind::L5RepoContext,
                "REPOSITORY CONSTRAINTS & CONTEXT (P3/P4)",
                layer.byte_size,
                TrustLevel::UntrustedRepoContent,
                false,
                include_l5,
            ));
        }
        if let Some(ref layer) = l6 {
            layer_metadata.push(PromptLayerMetadata::new(
                PromptLayerKind::L6QualityGate,
                "QUALITY GATE VERIFICATION CHECKLIST (P1)",
                layer.byte_size,
                TrustLevel::TrustedSystem,
                true,
                true,
            ));
        }

        if let Some(layer) = l3.filter(|_| include_l3) {
            active_layers.push(layer);
        }
        if let Some(layer) = l4.filter(|_| include_l4) {
            active_layers.push(layer);
        }
        if let Some(layer) = l5.filter(|_| include_l5) {
            active_layers.push(layer);
        }
        if let Some(l6_layer) = l6 {
            active_layers.push(l6_layer);
        }

        // Verify that safety invariants are at index 0 and intact
        if active_layers.is_empty()
            || active_layers[0].kind != PromptLayerKind::L0Safety
            || active_layers[0].content != RUNTIME_SAFETY_INVARIANTS
        {
            return Err(PromptError::PromptSecurityViolation {
                prompt_id: contract.id.clone(),
                reason: "Layer 0 safety invariants corrupted or missing from compiled prompt"
                    .to_string(),
            });
        }

        // Step 7: Assemble Text Representations
        let mut system_text = String::new();
        let mut user_text = String::new();
        let mut full_text = String::new();

        for layer in &active_layers {
            let section = format!("## {}\n{}\n\n", layer.name, layer.content);
            full_text.push_str(&section);

            if layer.kind == PromptLayerKind::L0Safety || layer.kind == PromptLayerKind::L1Role {
                system_text.push_str(&section);
            } else {
                user_text.push_str(&section);
            }
        }

        let full_trimmed = full_text.trim_end().to_string();
        let system_trimmed = system_text.trim_end().to_string();
        let user_opt = if options.separate_system_user && !user_text.trim().is_empty() {
            Some(user_text.trim_end().to_string())
        } else {
            None
        };

        // Step 8: Deterministic Content Hash Calculation
        let content_hash = Self::calculate_effective_hash(
            contract,
            &resolved_params,
            &active_layers,
            effective_strategy,
            effective_profile_id.as_deref(),
        );

        let output_format = contract
            .expected_output_format
            .clone()
            .or_else(|| context.task_objective.expected_output_format.clone());

        let total_bytes = full_trimmed.len();

        let compaction_meta = PromptCompactionMetadata::new(
            max_budget,
            protected_bytes,
            total_bytes,
            layers_retained,
            layers_dropped,
            false,
        );

        let context_digest = context.compute_context_digest();
        let model_id = effective_profile_id
            .clone()
            .unwrap_or_else(|| "default".to_string());
        let provider = options
            .model_profile
            .as_ref()
            .map(|p| p.provider.clone())
            .unwrap_or_else(|| "default".to_string());
        let output_contract_id = output_format
            .clone()
            .unwrap_or_else(|| "unspecified".to_string());

        let effective_source_kind = options.source_kind.unwrap_or(PromptSourceKind::Builtin);

        let invocation_prov = PromptInvocationProvenance::new(
            contract.id.clone(),
            contract.version,
            contract.content_hash.clone(),
            effective_strategy,
            content_hash.clone(),
            model_id,
            provider,
            context_digest,
            output_contract_id,
            Utc::now(),
        )
        .with_source_kind(effective_source_kind);

        let prompt_provenance = PromptProvenance {
            invocation: invocation_prov,
            mission_id: context.mission_id.clone(),
            task_id: context.task_id.clone(),
            role: context.role.clone(),
            stage: context.stage,
            layers: layer_metadata,
            compaction: compaction_meta,
        };

        Ok(EffectivePrompt {
            prompt_id: contract.id.clone(),
            prompt_version: contract.version,
            contract_hash: contract.content_hash.clone(),
            system_prompt: system_trimmed,
            user_prompt: user_opt,
            assembled_text: full_trimmed,
            layers: active_layers,
            content_hash,
            total_bytes,
            expected_output_format: output_format,
            supplied_parameters: resolved_params.into_keys().collect(),
            strategy: effective_strategy,
            model_profile_id: effective_profile_id,
            provenance: Some(prompt_provenance),
            source_kind: effective_source_kind,
        })
    }
}
