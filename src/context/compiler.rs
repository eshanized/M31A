//! Production ContextCompiler delivering minimal fresh context, P0-P4 reverse compaction, and prompt-injection defense (D-13, D-14, D-16, CTX-01, CTX-04, CTX-05).
//!
//! Enforces zero conversational transcript leakage across tasks, strict fail-closed headroom protection,
//! XML trust envelopes with closing-tag escaping, and auditable ContextCompilationManifest generation.

use async_trait::async_trait;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use uuid::Uuid;

use crate::context::envelope::{TrustEnvelope, TrustLevel};
use crate::context::evidence::{
    ContextSelectionStatus, EvidenceManifestRecord, EvidenceOrigin, EvidenceSelectionReason,
    OmittedEvidenceRecord, select_task_aware_evidence,
};
use crate::context::manifest::ContextCompilationManifest;
use crate::context::priority::{ContextPriority, ContextSection, ReverseCompactor};
use crate::context::tokenizer::TokenizerAdapter;
use crate::kernel::seams::context::{
    CompiledContext, ContextCompilationRequest, ContextCompiler, ContextError,
};
use crate::prompt::{
    CompilationOptions, DefaultPromptCompiler, InMemoryPromptCatalog, MissionStage, PromptCatalog,
    PromptCompiler, PromptContext, ToolOutputEvidence, render_prompt,
};
use crate::repo::query::{BoundedQueryEngine, QueryBounds};
use crate::repo::types::SourceSliceKind;
use crate::state_machine::agent::AgentRole;

/// Role→stage fallback resolver injected by the composition root.
///
/// `None` means no fallback is configured; stage-less prompt contracts then
/// fail explicitly. Production injects the role-registry lookup.
pub type RoleStageFallback = Arc<dyn Fn(&AgentRole) -> Option<MissionStage> + Send + Sync>;

/// Production implementation of `ContextCompiler` fulfilling autonomous runtime context contracts.
pub struct ProductionContextCompiler {
    tokenizer: TokenizerAdapter,
    compactor: ReverseCompactor,
    query_engine: Option<Arc<BoundedQueryEngine>>,
    prompt_catalog: Arc<dyn PromptCatalog>,
    prompt_compiler: Arc<dyn PromptCompiler>,
    workspace_root: Option<PathBuf>,
    memory_store: Option<Arc<dyn crate::memory::EngineeringMemoryStore>>,
    /// Registry-backed role→stage fallback, injected by the composition root.
    ///
    /// Architectural boundaries forbid src/context from depending on the agent module, so
    /// the role registry (an agent-layer authority) is supplied as a plain
    /// function instead of a direct reference. Production wires the registry
    /// lookup; without it, stage-less contracts fail explicitly.
    role_stage_fallback: Option<RoleStageFallback>,
}

impl ProductionContextCompiler {
    /// Create a new ProductionContextCompiler with default BPE tokenizer and reverse compactor.
    pub fn new() -> Self {
        let tokenizer = TokenizerAdapter::new();
        let compactor = ReverseCompactor::new(TokenizerAdapter::new());
        Self {
            tokenizer,
            compactor,
            query_engine: None,
            prompt_catalog: Arc::new(InMemoryPromptCatalog::with_builtins()),
            prompt_compiler: Arc::new(DefaultPromptCompiler::new()),
            workspace_root: None,
            memory_store: None,
            role_stage_fallback: None,
        }
    }

    /// Create with a specific TokenizerAdapter (e.g. model-specific or conservative).
    pub fn with_tokenizer(tokenizer: TokenizerAdapter) -> Self {
        let compactor = ReverseCompactor::new(TokenizerAdapter::conservative());
        Self {
            tokenizer,
            compactor,
            query_engine: None,
            prompt_catalog: Arc::new(InMemoryPromptCatalog::with_builtins()),
            prompt_compiler: Arc::new(DefaultPromptCompiler::new()),
            workspace_root: None,
            memory_store: None,
            role_stage_fallback: None,
        }
    }

    /// Attach a prompt catalog for sourcing versioned system and safety contracts.
    pub fn with_prompt_catalog(mut self, catalog: Arc<dyn PromptCatalog>) -> Self {
        self.prompt_catalog = catalog;
        self
    }

    /// Attach a prompt compiler for deterministic 7-layer composition.
    pub fn with_prompt_compiler(mut self, compiler: Arc<dyn PromptCompiler>) -> Self {
        self.prompt_compiler = compiler;
        self
    }

    /// Attach a BoundedQueryEngine for repository symbol topology enrichment.
    pub fn with_query_engine(mut self, query_engine: Arc<BoundedQueryEngine>) -> Self {
        self.query_engine = Some(query_engine);
        self
    }

    /// Attach workspace root path for reading source slices.
    pub fn with_workspace_root(mut self, root: impl Into<PathBuf>) -> Self {
        let r = root.into();
        self.workspace_root = Some(r.clone());
        if let Some(qe) = self.query_engine.take() {
            let engine = match Arc::try_unwrap(qe) {
                Ok(e) => e.with_workspace_root(r),
                Err(arc_e) => (*arc_e).clone().with_workspace_root(r),
            };
            self.query_engine = Some(Arc::new(engine));
        }
        self
    }

    /// Attach an EngineeringMemoryStore for retrieving durable engineering knowledge.
    pub fn with_memory_store(
        mut self,
        store: Arc<dyn crate::memory::EngineeringMemoryStore>,
    ) -> Self {
        self.memory_store = Some(store);
        self
    }

    /// Attach a role→stage fallback resolver (normally the role registry
    /// lookup, injected by the composition root to respect the
    /// context/agent architectural dependency boundary).
    pub fn with_role_stage_fallback(mut self, fallback: RoleStageFallback) -> Self {
        self.role_stage_fallback = Some(fallback);
        self
    }

    /// Access tokenizer.
    pub fn tokenizer(&self) -> &TokenizerAdapter {
        &self.tokenizer
    }

    /// Access prompt catalog.
    pub fn prompt_catalog(&self) -> &Arc<dyn PromptCatalog> {
        &self.prompt_catalog
    }

    /// Access prompt compiler.
    pub fn prompt_compiler(&self) -> &Arc<dyn PromptCompiler> {
        &self.prompt_compiler
    }

    /// Access configured workspace root path.
    pub fn workspace_root(&self) -> Option<&Path> {
        self.workspace_root.as_deref()
    }

    /// Expose task-aware evidence selection for planning or diagnostic inspection.
    pub fn select_evidence(
        &self,
        req: &ContextCompilationRequest,
        max_evidence_tokens: usize,
    ) -> crate::context::evidence::TaskEvidenceSelectionResult {
        select_task_aware_evidence(
            self.query_engine.as_deref(),
            req,
            max_evidence_tokens,
            &self.tokenizer,
        )
    }

    /// Compile a slice of `ContextSection`s deterministically against a token budget,
    /// enforcing reverse-priority compaction and output headroom reservation.
    pub fn compile_sections(
        &self,
        sections: Vec<ContextSection>,
        total_budget: usize,
        reserved_headroom: usize,
    ) -> Result<CompiledContext, ContextError> {
        self.compile_sections_with_evidence(
            sections,
            total_budget,
            reserved_headroom,
            Vec::new(),
            Vec::new(),
            None,
            ContextSelectionStatus::Complete,
        )
    }

    /// Compile sections preserving evidence records, repository revision, and selection status.
    #[allow(clippy::too_many_arguments)]
    pub fn compile_sections_with_evidence(
        &self,
        sections: Vec<ContextSection>,
        total_budget: usize,
        reserved_headroom: usize,
        selected_evidence: Vec<EvidenceManifestRecord>,
        omitted_evidence: Vec<OmittedEvidenceRecord>,
        repository_revision: Option<String>,
        selection_status: ContextSelectionStatus,
    ) -> Result<CompiledContext, ContextError> {
        let admissible_budget = total_budget.saturating_sub(reserved_headroom);

        // Run 5-priority reverse compaction engine (P4 -> P3 -> P2), failing closed on P0+P1 overflow
        let compaction_res = self.compactor.compact(sections, admissible_budget)?;

        // Assemble formatted system prompt from admitted sections
        let mut prompt = String::new();
        for section in &compaction_res.compacted_sections {
            prompt.push_str(&format!(
                "=== SECTION: {} (Priority: {}) ===\n{}\n\n",
                section.id,
                section.priority.as_str(),
                section.content
            ));
        }

        let final_token_count = self.tokenizer.count_tokens(&prompt);

        let manifest = ContextCompilationManifest::new(
            1,
            total_budget,
            reserved_headroom,
            final_token_count,
            compaction_res.manifest_entries,
        )
        .with_evidence(
            selected_evidence,
            omitted_evidence,
            repository_revision,
            selection_status,
        );

        Ok(
            CompiledContext::new(Uuid::now_v7().to_string(), final_token_count, prompt)
                .with_manifest(manifest),
        )
    }
}

impl Default for ProductionContextCompiler {
    fn default() -> Self {
        Self::new()
    }
}

use crate::model::types::{ChatMessage, ModelToolCall};

fn truncate_output(output: &str, max_chars: usize) -> String {
    if output.len() <= max_chars {
        output.to_string()
    } else {
        let half = max_chars / 2;
        let mut prefix_end = half.min(output.len());
        while prefix_end > 0 && !output.is_char_boundary(prefix_end) {
            prefix_end -= 1;
        }
        let prefix = &output[..prefix_end];

        let target_suffix = output.len().saturating_sub(half);
        let mut suffix_start = target_suffix.min(output.len());
        while suffix_start < output.len() && !output.is_char_boundary(suffix_start) {
            suffix_start += 1;
        }
        let suffix = &output[suffix_start..];
        format!(
            "{}\n... [Output truncated: showing first and last parts of {} total bytes] ...\n{}",
            prefix,
            output.len(),
            suffix
        )
    }
}

#[async_trait]
impl ContextCompiler for ProductionContextCompiler {
    fn prompt_catalog(&self) -> Option<&Arc<dyn PromptCatalog>> {
        Some(&self.prompt_catalog)
    }

    fn prompt_compiler(&self) -> Option<&Arc<dyn PromptCompiler>> {
        Some(&self.prompt_compiler)
    }

    async fn compile_context(
        &self,
        req: ContextCompilationRequest,
    ) -> Result<CompiledContext, ContextError> {
        let mut sections = Vec::new();

        let mission_obj = req
            .mission_objective
            .as_deref()
            .unwrap_or("Execute assigned engineering work autonomously.");
        let task_obj = req.task_objective.as_deref().unwrap_or(mission_obj);

        let mut target_files = Vec::new();
        let mut test_files = Vec::new();
        for word in mission_obj
            .split_whitespace()
            .chain(task_obj.split_whitespace())
        {
            let clean = word.trim_matches(|c: char| {
                c == '@'
                    || c == '"'
                    || c == '\''
                    || c == '.'
                    || c == ','
                    || c == ';'
                    || c == ':'
                    || c == '('
                    || c == ')'
            });
            if clean.ends_with(".rs")
                || clean.ends_with(".py")
                || clean.ends_with(".ts")
                || clean.ends_with(".js")
                || clean.ends_with(".go")
            {
                if clean.contains("test") {
                    if !test_files.contains(&clean.to_string()) {
                        test_files.push(clean.to_string());
                    }
                } else {
                    if !target_files.contains(&clean.to_string()) {
                        target_files.push(clean.to_string());
                    }
                }
            }
        }

        // P0: Runtime Safety Invariants & Role Policies (unprunable)
        let mut p0_params = BTreeMap::new();
        p0_params.insert("mission_id".to_string(), req.mission_id.to_string());

        let contract = self
            .prompt_catalog
            // Canonical P0 contract (wiring remediation v0.1.1):
            // `core.safety` v2 is the single Layer-0 authority. The legacy
            // `runtime.safety_invariants` v1 id is retained only as a
            // deprecated compatibility asset and MUST NOT anchor P0.
            .resolve_canonical("core.safety", 2)
            .map_err(|e| {
                ContextError::CompilationFailed(format!(
                    "failed to resolve core.safety prompt contract: {}",
                    e
                ))
            })?;
        let rendered = render_prompt(contract, &p0_params, false).map_err(|e| {
            ContextError::CompilationFailed(format!(
                "failed to render runtime.safety_invariants prompt: {}",
                e
            ))
        })?;
        let p0_content = rendered.rendered_text;

        sections.push(ContextSection::new(
            "p0_safety_invariants",
            ContextPriority::P0RuntimeSafety,
            p0_content,
            "runtime://policy/safety_invariants",
            TrustLevel::TrustedPolicy,
            false,
            &self.tokenizer,
        ));

        let clean_mission_obj = if let Some(idx) = mission_obj.find("<explicit_developer_mentions>")
        {
            mission_obj[..idx].trim()
        } else {
            mission_obj
        };

        // Resolve role and versioned prompt contract.
        //
        // Explicit selection precedence (never prompt-text inference):
        //   explicit task/workflow PromptReference
        //       > explicit agent/session PromptReference
        //       > role default PromptReference
        //       > failure (fail closed — no silent substitution).
        //
        // The caller classifies an explicit reference via
        // `prompt_source`; an explicit reference without a caller
        // classification is treated as a task binding (the strongest
        // explicit claim). The resolved contract is canonicalized through
        // the catalog (`resolve_canonical`: v1 upgrades to the canonical
        // v2 generation, deprecated contracts follow their replacement
        // pointer), and the RESOLVED identity is what compiles and what
        // provenance records — never the requested alias.
        let role = req.role.clone().unwrap_or(AgentRole::implementer());
        let (prompt_ref, prompt_source) = match (req.prompt_ref.clone(), req.prompt_source) {
            (Some(explicit), Some(source)) => (explicit, source),
            (Some(explicit), None) => (
                explicit,
                crate::kernel::seams::context::PromptSelectionSource::ExplicitTask,
            ),
            (None, _) => (
                crate::prompt::PromptReference::for_role(role.clone()),
                crate::kernel::seams::context::PromptSelectionSource::RoleDefault,
            ),
        };

        let contract = self
            .prompt_catalog
            .resolve_canonical(&prompt_ref.id, prompt_ref.version)
            .map_err(|e| {
                ContextError::CompilationFailed(format!(
                    "Failed to resolve prompt contract '{}' (v{}): {}",
                    prompt_ref.id, prompt_ref.version, e
                ))
            })?;

        // Operating stage: the resolved prompt contract's declared `stage`
        // is authoritative. The injected role→stage fallback (production:
        // the role registry) applies only when a contract omits its stage.
        // An unregistered role with a stage-less contract fails explicitly
        // rather than assuming a stage.
        let stage = contract.stage.or_else(|| {
            self.role_stage_fallback
                .as_ref()
                .and_then(|fallback| fallback(&role))
        });
        let stage = stage.ok_or_else(|| {
            ContextError::CompilationFailed(format!(
                "unknown agent role '{}': no registered role definition",
                role.as_str()
            ))
        })?;

        // Wrap user intent in XML trust envelope with smuggling defense (SEC-P-01)
        let wrapped_task_intent = TrustEnvelope::wrap_user_intent(task_obj);

        let target_guidance = if !target_files.is_empty() {
            target_files.join(", ")
        } else {
            String::new()
        };
        let test_guidance = if !test_files.is_empty() {
            test_files.join(", ")
        } else {
            String::new()
        };

        let mut role_params = BTreeMap::new();
        role_params.insert("mission_id".to_string(), req.mission_id.to_string());
        role_params.insert("task_id".to_string(), req.task_id.to_string());
        role_params.insert("task_objective".to_string(), wrapped_task_intent.clone());
        role_params.insert("task_spec".to_string(), wrapped_task_intent.clone());
        role_params.insert("user_intent".to_string(), wrapped_task_intent.clone());
        role_params.insert("target_files".to_string(), target_guidance.clone());
        role_params.insert("test_files".to_string(), test_guidance.clone());
        role_params.insert("previous_attempt_summary".to_string(), String::new());

        let rendered_role = render_prompt(contract, &role_params, false).map_err(|e| {
            ContextError::CompilationFailed(format!(
                "Failed to render prompt contract '{}': {}",
                contract.id, e
            ))
        })?;

        // Construct PromptContext and invoke PromptCompiler to enforce 7-layer composition & budgeting
        let mut prompt_ctx = PromptContext::new(
            Uuid::now_v7().to_string(),
            req.mission_id.to_string(),
            req.task_id.to_string(),
            role.clone(),
            stage,
            task_obj,
        );
        prompt_ctx = prompt_ctx.with_user_intent(clean_mission_obj);
        if !target_guidance.is_empty() {
            prompt_ctx
                .custom_parameters
                .insert("target_files".to_string(), target_guidance.clone());
        }
        if !test_guidance.is_empty() {
            prompt_ctx
                .custom_parameters
                .insert("test_files".to_string(), test_guidance.clone());
        }
        prompt_ctx
            .custom_parameters
            .insert("previous_attempt_summary".to_string(), String::new());

        // Task contract + upstream evidence into the 7-layer composition.
        // Criteria make the L2 objective layer and the acceptance-criteria fallback
        // target-specific; durable state feeds the L3 charter/requirements/architecture
        // layer. Absent data stays absent — the compiler renders honest fallbacks,
        // never invented content for missing fields.
        if !req.task_criteria.is_empty() {
            let criteria = req.task_criteria.clone();
            prompt_ctx.task_objective = prompt_ctx.task_objective.clone().with_criteria(criteria);
        }
        if let Some(ref charter) = req.upstream_charter {
            prompt_ctx = prompt_ctx.with_charter(charter.clone());
        }
        if !req.upstream_requirements.is_empty() {
            prompt_ctx = prompt_ctx.with_requirements(req.upstream_requirements.join("\n"));
        }
        if let Some(ref arch) = req.upstream_architecture {
            prompt_ctx = prompt_ctx.with_architecture(arch.clone());
        }

        for step in &req.step_history {
            prompt_ctx.tool_outputs.push(ToolOutputEvidence::new(
                &step.tool_name,
                format!("call_{}_{}", step.step_number, step.tool_name),
                &step.output,
                !step.success || step.error.is_some(),
            ));
        }

        // P0-04: behavioral contract customization from repository files is
        // injected ONLY as lower-trust, delimiter-escaped project guidance
        // (L5 user side). The built-in role contract stays authoritative.
        let effective_prompt = self
            .prompt_compiler
            .compile_with_guidance(
                self.prompt_catalog.as_ref(),
                contract,
                &prompt_ctx,
                &CompilationOptions::default(),
            )
            .map_err(|e| {
                ContextError::CompilationFailed(format!("PromptCompiler failed: {}", e))
            })?;

        // Fetch or use memory snapshot
        let memory_snapshot = if req.memory_snapshot.is_some() {
            req.memory_snapshot.clone()
        } else if let Some(ref store) = self.memory_store {
            let retriever = crate::memory::retrieval::TaskAwareMemoryRetriever::new(store.as_ref());
            let criteria = crate::memory::retrieval::MemoryRetrievalCriteria {
                mission_id: req.mission_id,
                task_id: Some(req.task_id),
                task_objective: task_obj,
                target_files: &target_files,
                target_symbols: &[],
                requirement_keys: &req.task_requirement_keys,
                error_context: req.error_context.as_deref(),
            };
            retriever.retrieve_snapshot(&criteria).await.ok()
        } else {
            None
        };

        // P1: Task Objectives & Role Protocol (unprunable, Task Context Injection)
        let mut p1_content = if rendered_role.rendered_text.contains("<user_intent") {
            rendered_role.rendered_text
        } else {
            let mut text = format!(
                "{}\n\nTask Context:\n- Mission ID: {}\n- Task ID: {}\n- Task Objective:\n{}",
                rendered_role.rendered_text, req.mission_id, req.task_id, wrapped_task_intent
            );
            if !target_guidance.is_empty() {
                text.push_str(&format!("\n- Target Files: {}", target_guidance));
            }
            if !test_guidance.is_empty() {
                text.push_str(&format!("\n- Test Files: {}", test_guidance));
            }
            text
        };

        if clean_mission_obj != task_obj && !p1_content.contains(clean_mission_obj) {
            p1_content.push_str(&format!("\nMission Objective: {}", clean_mission_obj));
        }

        // Task-scoped planning context. Every block is emitted only when the
        // corresponding evidence exists; nothing here is defaulted or invented.
        // Descriptions, criteria, assumptions, and requirement links come from
        // the durable task record; upstream charter/architecture/requirements/
        // decisions/research come from mission planning when it ran. All inserted
        // strings are model- or file-derived and therefore delimiter-escaped like
        // any other untrusted content (SEC-02).
        if let Some(ref desc) = req.task_description {
            p1_content.push_str(&format!(
                "\nTask Description:\n{}",
                TrustEnvelope::escape_closing_tags(desc)
            ));
        }
        if !req.task_criteria.is_empty() {
            p1_content.push_str("\nAcceptance Criteria:\n");
            for (i, criterion) in req.task_criteria.iter().enumerate() {
                p1_content.push_str(&format!(
                    "{}. {}\n",
                    i + 1,
                    TrustEnvelope::escape_closing_tags(criterion)
                ));
            }
        }
        if !req.task_requirement_keys.is_empty() {
            p1_content.push_str(&format!(
                "\nSatisfies Requirements: {}",
                TrustEnvelope::escape_closing_tags(&req.task_requirement_keys.join(", "))
            ));
        }
        if !req.task_assumptions.is_empty() {
            p1_content.push_str("\nTask Assumptions (challenge rather than assume silently):\n");
            for assumption in &req.task_assumptions {
                p1_content.push_str(&format!(
                    "- {}\n",
                    TrustEnvelope::escape_closing_tags(assumption)
                ));
            }
        }
        if !req.upstream_requirements.is_empty() {
            p1_content.push_str("\nProject Requirements:\n");
            for requirement in &req.upstream_requirements {
                p1_content.push_str(&format!(
                    "- {}\n",
                    TrustEnvelope::escape_closing_tags(requirement)
                ));
            }
        }
        if !req.upstream_assumptions.is_empty() {
            p1_content.push_str("\nProject Assumptions:\n");
            for assumption in &req.upstream_assumptions {
                p1_content.push_str(&format!(
                    "- {}\n",
                    TrustEnvelope::escape_closing_tags(assumption)
                ));
            }
        }
        if !req.upstream_decisions.is_empty() {
            p1_content.push_str("\nProject Decisions:\n");
            for decision in &req.upstream_decisions {
                p1_content.push_str(&format!(
                    "- {}\n",
                    TrustEnvelope::escape_closing_tags(decision)
                ));
            }
        }
        if let Some(ref summary) = req.upstream_research_summary {
            p1_content.push_str(&format!(
                "\nResearch Summary:\n{}",
                TrustEnvelope::escape_closing_tags(summary)
            ));
        }

        if let Some(ref mem) = memory_snapshot {
            if !mem.active_decisions.is_empty() {
                p1_content.push_str("\n\nAuthoritative Architectural Decisions:\n");
                for d in &mem.active_decisions {
                    let raw = format!(
                        "Decision: {}\nStatus: {:?}\nRationale: {}\nAlternatives Considered: {:?}\nConsequences: {:?}",
                        d.title, d.status, d.rationale, d.alternatives_considered, d.consequences
                    );
                    let wrapped = TrustEnvelope::wrap_untrusted_attributed(
                        &format!("memory://decision/{}", d.id),
                        TrustLevel::UntrustedToolOutput,
                        "memory",
                        "architectural_decision",
                        &[("status", &format!("{:?}", d.status))],
                        &raw,
                    );
                    p1_content.push_str(&format!("{}\n", wrapped));
                }
            }

            if !mem.active_assumptions.is_empty() {
                p1_content.push_str("\n\nEngineering Assumptions:\n");
                for a in &mem.active_assumptions {
                    let raw = format!(
                        "Assumption: {}\nStatus: {:?}\nBasis: {:?}\nTarget File: {:?}",
                        a.statement, a.status, a.evidence_summary, a.target_file
                    );
                    let wrapped = TrustEnvelope::wrap_untrusted_attributed(
                        &format!("memory://assumption/{}", a.id),
                        TrustLevel::UntrustedToolOutput,
                        "memory",
                        "engineering_assumption",
                        &[("status", &format!("{:?}", a.status))],
                        &raw,
                    );
                    p1_content.push_str(&format!("{}\n", wrapped));
                }
            }
        }

        sections.push(ContextSection::new(
            "p1_task_criteria",
            ContextPriority::P1TaskCompletion,
            p1_content,
            format!("agent://profile/{}", role.as_str()),
            TrustLevel::TrustedSystem,
            false,
            &self.tokenizer,
        ));

        // Previously computed EffectivePrompt layers are preserved:
        // the L2 step-objective contract (criteria-aware, falsification protocol,
        // epistemic categorization) and the L3 durable-state layer
        // (charter/requirements/architecture) join the sent prompt as their own
        // sections. Other layers (safety, role, evidence, repo, quality gates)
        // already have dedicated sections, so only L2/L3 are appended — no duplication.
        // Layer contents embed model- and file-derived strings, so delimiter closings
        // are escaped exactly like other untrusted content (SEC-02).
        for layer in &effective_prompt.layers {
            match layer.kind {
                crate::prompt::composer::PromptLayerKind::L2Objective => {
                    // Compactable reinforcement: the acceptance criteria themselves
                    // live unprunably in P1 above; this appended directive text
                    // (falsification protocol, epistemic rules) joins as P2 evidence
                    // so the protected-budget gate (P0+P1 only) keeps failing
                    // closed on true safety/role/task content while this
                    // reinforcement compacts last under extreme budgets.
                    sections.push(ContextSection::new(
                        "p1_step_objective_contract",
                        ContextPriority::P2RequiredEvidence,
                        TrustEnvelope::escape_closing_tags(&layer.content),
                        format!("prompt://{}/objective", effective_prompt.prompt_id),
                        TrustLevel::TrustedSystem,
                        true,
                        &self.tokenizer,
                    ));
                }
                crate::prompt::composer::PromptLayerKind::L3DurableState => {
                    sections.push(ContextSection::new(
                        "p2_durable_state",
                        ContextPriority::P2RequiredEvidence,
                        TrustEnvelope::escape_closing_tags(&layer.content),
                        format!("prompt://{}/durable_state", effective_prompt.prompt_id),
                        TrustLevel::UntrustedRepoContent,
                        true,
                        &self.tokenizer,
                    ));
                }
                // P0-04: lower-trust project guidance (repository files
                // targeting behavioral contract IDs) joins ONLY as
                // discardable-first P4 untrusted context with envelope
                // delimiters — never as the authoritative role contract.
                // The built-in L1 role layer is unaffected.
                crate::prompt::composer::PromptLayerKind::L5RepoContext
                    if layer.name == "project_guidance" =>
                {
                    sections.push(ContextSection::new(
                        "p4_project_guidance",
                        ContextPriority::P4OptionalBackground,
                        TrustEnvelope::escape_closing_tags(&layer.content),
                        format!("prompt://{}/project_guidance", effective_prompt.prompt_id),
                        TrustLevel::UntrustedRepoContent,
                        true,
                        &self.tokenizer,
                    ));
                }
                _ => {}
            }
        }

        // Task-Aware Evidence Selection
        let effective_root = req
            .workspace_root
            .clone()
            .or_else(|| self.workspace_root.clone());
        let effective_qe = self.query_engine.as_ref().map(|qe| {
            if let Some(ref root) = effective_root {
                if qe.workspace_root().is_none() {
                    Arc::new((**qe).clone().with_workspace_root(root.clone()))
                } else {
                    qe.clone()
                }
            } else {
                qe.clone()
            }
        });

        // Reserve budget for evidence: max_tokens - headroom(1024) - safety/role/working context reservation
        let evidence_budget = req.max_tokens.saturating_sub(2048).min(8192);
        let evidence_result = select_task_aware_evidence(
            effective_qe.as_deref(),
            &req,
            evidence_budget,
            &self.tokenizer,
        );

        // Partition selected evidence into P2 (Required Evidence) vs P4 (Optional Background)
        let mut p2_evidence = Vec::new();
        let mut p4_evidence = Vec::new();

        for item in &evidence_result.selected_items {
            if item.reason == EvidenceSelectionReason::DiagnosticFailure
                || item.origin == EvidenceOrigin::Explicit
                || item.reason == EvidenceSelectionReason::AssociatedTest
                || item.slice_kind == SourceSliceKind::ContiguousSlice
                || item.slice_kind == SourceSliceKind::SymbolBody
            {
                p2_evidence.push(item);
            } else {
                p4_evidence.push(item);
            }
        }

        // P2: Required Evidence (wrapped in XML trust envelope with smuggling defense).
        // Declarative authority boundary: P2 contains ONLY evidence-backed
        // material (diagnostic failures, task-relevant source slices, recorded
        // history). The runtime MUST NOT assert prerequisite success ("baseline
        // captured", "verified clean") without artifacts — unevidenced success
        // claims are fake success.
        let mut p2_body = String::new();

        if let Some(ref err) = req.error_context {
            let wrapped_err = TrustEnvelope::wrap_untrusted_attributed(
                "diagnostic://failure",
                TrustLevel::UntrustedToolOutput,
                EvidenceOrigin::Historical.as_str(),
                EvidenceSelectionReason::DiagnosticFailure.as_str(),
                &[],
                err,
            );
            p2_body.push_str(&format!("\nActive Diagnostic Failure:\n{}\n", wrapped_err));
        }

        if !p2_evidence.is_empty() {
            p2_body.push_str("\nTask-Relevant Source & Test Evidence:\n");
            for item in p2_evidence {
                if item.id == "diagnostic://failure" {
                    continue;
                }
                let mut attrs = vec![
                    ("fact_class", item.fact_class.as_str()),
                    ("slice_kind", item.slice_kind.as_str()),
                ];
                let s_str;
                let e_str;
                if let (Some(s), Some(e)) = (item.start_line, item.end_line) {
                    s_str = s.to_string();
                    e_str = e.to_string();
                    attrs.push(("start_line", &s_str));
                    attrs.push(("end_line", &e_str));
                }

                let wrapped_item = TrustEnvelope::wrap_untrusted_attributed(
                    &item.provenance,
                    item.trust_level,
                    item.origin.as_str(),
                    item.reason.as_str(),
                    &attrs,
                    &item.content,
                );
                p2_body.push_str(&format!("{}\n", wrapped_item));
            }
        }

        if let Some(ref mem) = memory_snapshot {
            if !mem.recent_failures.is_empty() {
                p2_body.push_str("\nHistorical Failure Diagnoses & Repair Attempts:\n");
                for fd in &mem.recent_failures {
                    let raw = format!(
                        "Failure Signature: {}\nHypothesis: {}\nRoot Cause: {}\nRecommended Action: {}\nRepair Status: {:?}\nRepair Proposal: {:?}\nRecurrence Count: {}",
                        fd.failure_signature,
                        fd.hypothesis,
                        fd.root_cause,
                        fd.recommended_action,
                        fd.repair_status,
                        fd.repair_proposal_json,
                        fd.recurrence_count
                    );
                    let wrapped = TrustEnvelope::wrap_untrusted_attributed(
                        &format!("memory://diagnosis/{}", fd.id),
                        TrustLevel::UntrustedToolOutput,
                        "memory",
                        "failure_diagnosis",
                        &[("repair_status", &format!("{:?}", fd.repair_status))],
                        &raw,
                    );
                    p2_body.push_str(&format!("{}\n", wrapped));
                }
            }

            if !mem.open_findings.is_empty() {
                p2_body.push_str("\nDurable Review Findings:\n");
                for rf in &mem.open_findings {
                    let raw = format!(
                        "Finding [{} / {:?}]: {}\nFile: {}:{:?}-{:?}\nStatus: {:?}\nRecommendation: {}",
                        rf.id,
                        rf.severity,
                        rf.description,
                        rf.file_path,
                        rf.line_start,
                        rf.line_end,
                        rf.status,
                        rf.recommendation
                    );
                    let wrapped = TrustEnvelope::wrap_untrusted_attributed(
                        &format!("memory://review/{}", rf.id),
                        TrustLevel::UntrustedToolOutput,
                        "memory",
                        "review_finding",
                        &[
                            ("severity", &format!("{:?}", rf.severity)),
                            ("status", &format!("{:?}", rf.status)),
                        ],
                        &raw,
                    );
                    p2_body.push_str(&format!("{}\n", wrapped));
                }
            }

            if !mem.verified_requirements.is_empty() {
                p2_body.push_str("\nDurable Verification State:\n");
                for vr in &mem.verified_requirements {
                    let raw = format!(
                        "Requirement: {}\nValidity: {:?}\nSnapshot Hash: {}\nPassed: {}\nInvalidated At: {:?}",
                        vr.requirement_key,
                        vr.validity_status,
                        vr.snapshot_hash,
                        vr.passed,
                        vr.invalidated_at
                    );
                    let wrapped = TrustEnvelope::wrap_untrusted_attributed(
                        &format!("memory://verification/{}", vr.id),
                        TrustLevel::UntrustedToolOutput,
                        "memory",
                        "verification_record",
                        &[("validity", &format!("{:?}", vr.validity_status))],
                        &raw,
                    );
                    p2_body.push_str(&format!("{}\n", wrapped));
                }
            }
        }

        let wrapped_p2 = TrustEnvelope::wrap_untrusted(
            "task://prerequisites",
            TrustLevel::UntrustedToolOutput,
            &p2_body,
        );
        sections.push(ContextSection::new(
            "p2_prerequisite_evidence",
            ContextPriority::P2RequiredEvidence,
            wrapped_p2,
            "task://prerequisites",
            TrustLevel::UntrustedToolOutput,
            true,
            &self.tokenizer,
        ));

        // P3: Active Working Context (recent turns / step history)
        let p3_content = if req.step_history.is_empty() {
            "Initial step: Ready to plan and execute task actions.".to_string()
        } else {
            let mut history_str = String::from("Prior Step History and Tool Results:\n");
            for step in &req.step_history {
                history_str.push_str(&format!(
                    "\n--- Step {} ---\nTool: {}\nParameters: {}\nStatus: {}\n",
                    step.step_number,
                    step.tool_name,
                    step.parameters,
                    if step.success { "success" } else { "failed" }
                ));
                if let Some(ref err) = step.error {
                    let wrapped_err = TrustEnvelope::wrap_untrusted(
                        "tool://error",
                        TrustLevel::UntrustedToolOutput,
                        err,
                    );
                    history_str.push_str(&format!("Error: {}\n", wrapped_err));
                }
                if !step.output.is_empty() {
                    let safe_out = truncate_output(&step.output, 4000);
                    history_str.push_str(&format!("Output:\n{}\n", safe_out));
                }
            }
            history_str
        };
        let wrapped_working_memory = TrustEnvelope::wrap_untrusted(
            "agent://working_memory",
            TrustLevel::UntrustedToolOutput,
            &p3_content,
        );
        sections.push(ContextSection::new(
            "p3_working_memory",
            ContextPriority::P3ActiveWorkingContext,
            wrapped_working_memory,
            "agent://working_memory",
            TrustLevel::UntrustedToolOutput,
            true,
            &self.tokenizer,
        ));

        // P4: Optional Background / Repository Topology & Relational Neighborhood
        let mut p4_body = String::new();
        if let Some(ref ov) = evidence_result.overview_summary {
            p4_body.push_str(ov);
            p4_body.push('\n');
        }

        if !target_files.is_empty() || !test_files.is_empty() {
            p4_body.push_str("Identified Workspace Files:\n");
            for f in &target_files {
                p4_body.push_str(&format!("  - Target: {}\n", f));
            }
            for f in &test_files {
                p4_body.push_str(&format!("  - Test: {}\n", f));
            }
            p4_body.push('\n');
        }

        if !p4_evidence.is_empty() {
            p4_body.push_str("Relational Code Neighborhood & Signatures:\n");
            for item in p4_evidence {
                let wrapped_item = TrustEnvelope::wrap_untrusted_attributed(
                    &item.provenance,
                    item.trust_level,
                    item.origin.as_str(),
                    item.reason.as_str(),
                    &[
                        ("fact_class", item.fact_class.as_str()),
                        ("slice_kind", item.slice_kind.as_str()),
                    ],
                    &item.content,
                );
                p4_body.push_str(&format!("{}\n", wrapped_item));
            }
            p4_body.push('\n');
        }

        if let Some(ref imp) = evidence_result.change_impact_summary {
            p4_body.push_str(imp);
            p4_body.push('\n');
        }

        if let Some(ref qe) = effective_qe {
            let mut search_terms = Vec::new();
            let stop_words: std::collections::HashSet<&'static str> = [
                "the",
                "a",
                "an",
                "and",
                "or",
                "but",
                "in",
                "on",
                "at",
                "to",
                "for",
                "with",
                "from",
                "by",
                "about",
                "as",
                "into",
                "like",
                "through",
                "after",
                "over",
                "between",
                "out",
                "against",
                "during",
                "without",
                "before",
                "under",
                "around",
                "among",
                "is",
                "are",
                "was",
                "were",
                "be",
                "been",
                "being",
                "have",
                "has",
                "had",
                "do",
                "does",
                "did",
                "will",
                "would",
                "shall",
                "should",
                "may",
                "might",
                "must",
                "can",
                "could",
                "this",
                "that",
                "these",
                "those",
                "it",
                "its",
                "you",
                "your",
                "he",
                "she",
                "we",
                "they",
                "fix",
                "implement",
                "update",
                "run",
                "verify",
                "task",
                "objective",
                "mission",
                "test",
                "tests",
                "code",
                "file",
                "files",
            ]
            .into_iter()
            .collect();

            for word in mission_obj
                .split_whitespace()
                .chain(task_obj.split_whitespace())
            {
                let raw_token = word.trim_matches(|c: char| !c.is_alphanumeric() && c != '_');
                if raw_token.len() < 3 {
                    continue;
                }
                let clean = raw_token.to_lowercase();
                if !stop_words.contains(clean.as_str()) && !search_terms.contains(&clean) {
                    search_terms.push(clean);
                }
                if raw_token.contains('_') {
                    for part in raw_token.split('_') {
                        let p_clean = part.to_lowercase();
                        if p_clean.len() >= 3
                            && !stop_words.contains(p_clean.as_str())
                            && !search_terms.contains(&p_clean)
                        {
                            search_terms.push(p_clean);
                        }
                    }
                }
                let mut current_sub = String::new();
                for ch in raw_token.chars() {
                    if ch.is_uppercase() && !current_sub.is_empty() {
                        let sub_lower = current_sub.to_lowercase();
                        if sub_lower.len() >= 3
                            && !stop_words.contains(sub_lower.as_str())
                            && !search_terms.contains(&sub_lower)
                        {
                            search_terms.push(sub_lower);
                        }
                        current_sub.clear();
                    }
                    current_sub.push(ch);
                }
                if !current_sub.is_empty() {
                    let sub_lower = current_sub.to_lowercase();
                    if sub_lower.len() >= 3
                        && !stop_words.contains(sub_lower.as_str())
                        && !search_terms.contains(&sub_lower)
                    {
                        search_terms.push(sub_lower);
                    }
                }
            }

            for file in target_files.iter().chain(test_files.iter()) {
                if let Some(stem) = std::path::Path::new(file)
                    .file_stem()
                    .and_then(|s| s.to_str())
                {
                    let clean_stem = stem.to_lowercase();
                    if clean_stem.len() >= 3
                        && !stop_words.contains(clean_stem.as_str())
                        && !search_terms.contains(&clean_stem)
                    {
                        search_terms.push(clean_stem);
                    }
                }
            }

            let mut symbol_scores: std::collections::HashMap<
                String,
                (usize, crate::repo::types::RepositorySymbol),
            > = std::collections::HashMap::new();

            for term in &search_terms {
                let res = qe.find_symbols(term, QueryBounds::new(15, 8192, 1));
                for sym in res.data {
                    let key = format!("{}:{}", sym.file_path, sym.name);
                    let entry = symbol_scores.entry(key).or_insert((0, sym.clone()));
                    entry.0 += 1;
                    if target_files.iter().any(|tf| sym.file_path.ends_with(tf))
                        || test_files.iter().any(|tf| sym.file_path.ends_with(tf))
                    {
                        entry.0 += 5;
                    }
                }
            }

            if symbol_scores.is_empty() {
                let fallback = qe.find_symbols("", QueryBounds::new(10, 4096, 1));
                for sym in fallback.data {
                    let key = format!("{}:{}", sym.file_path, sym.name);
                    symbol_scores.insert(key, (1, sym));
                }
            }

            let mut ranked_symbols: Vec<_> = symbol_scores.into_values().collect();
            ranked_symbols.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.name.cmp(&b.1.name)));

            if target_files.is_empty() && test_files.is_empty() {
                let mut candidate_files: Vec<String> = Vec::new();
                for (_score, sym) in &ranked_symbols {
                    if !candidate_files.contains(&sym.file_path) {
                        candidate_files.push(sym.file_path.clone());
                    }
                    if candidate_files.len() >= 5 {
                        break;
                    }
                }
                if !candidate_files.is_empty() {
                    p4_body.push_str("Relevant candidate files based on mission terms:\n");
                    for f in &candidate_files {
                        p4_body.push_str(&format!("  - Candidate: {}\n", f));
                    }
                    p4_body.push_str("Recommendation: Inspect candidate files with 'read_file' or discover definitions with 'repo_map'.\n\n");
                }
            }

            p4_body.push_str("Relevant Repository Symbols:\n");
            for (_score, sym) in ranked_symbols.into_iter().take(15) {
                p4_body.push_str(&format!(
                    "  - {} ({:?}) in {}\n",
                    sym.name, sym.kind, sym.file_path
                ));
            }
        }

        if p4_body.is_empty() {
            p4_body =
                "Repository Context: M31A autonomous software engineering runtime.\n".to_string();
        }

        let wrapped_repo_bg = TrustEnvelope::wrap_untrusted(
            "repo://topology",
            TrustLevel::UntrustedRepoContent,
            &p4_body,
        );

        sections.push(ContextSection::new(
            "p4_repository_topology",
            ContextPriority::P4OptionalBackground,
            wrapped_repo_bg,
            "repo://topology",
            TrustLevel::UntrustedRepoContent,
            false,
            &self.tokenizer,
        ));

        // Compile with reserved headroom (default 1024 tokens) and full evidence manifest
        let manifest_records: Vec<_> = evidence_result
            .selected_items
            .iter()
            .map(|item| item.to_manifest_record())
            .collect();
        let repo_rev = effective_qe
            .as_ref()
            .map(|qe| format!("gen:{}", qe.graph().generation_id));

        let mut compiled = self.compile_sections_with_evidence(
            sections,
            req.max_tokens,
            1024,
            manifest_records,
            evidence_result.omitted_items,
            repo_rev,
            evidence_result.status,
        )?;

        // Record the ACTUAL prompt selection in the compilation manifest:
        // the resolved (canonicalized) contract identity, the winning
        // precedence source, and the contract hash. Provenance MUST
        // describe the prompt that was actually compiled — never the
        // requested alias when canonicalization upgraded it.
        if let Some(ref mut manifest) = compiled.manifest {
            manifest.prompt_id = Some(contract.id.clone());
            manifest.prompt_version = Some(contract.version);
            manifest.prompt_source = Some(prompt_source);
            manifest.prompt_content_hash = Some(contract.content_hash.clone());
        }
        if let Some(mut invocation) = effective_prompt.invocation_provenance() {
            invocation = invocation
                .with_mission_id(req.mission_id.to_string())
                .with_task_id(req.task_id.to_string());
            if let Some(agent_id) = req.agent_id {
                invocation = invocation.with_agent_id(agent_id.to_string());
            }
            compiled.prompt_provenance = Some(invocation);
        };

        // Assemble structured multi-turn ChatMessages
        let mut messages = Vec::new();
        messages.push(ChatMessage::System {
            content: compiled.system_prompt.clone(),
        });
        messages.push(ChatMessage::User {
            content: format!("Mission Objective: {}\nTask: {}", mission_obj, task_obj),
        });

        for step in &req.step_history {
            let call_id = format!("call_{}_{}", step.step_number, step.tool_name);
            messages.push(ChatMessage::Assistant {
                content: None,
                tool_calls: vec![ModelToolCall {
                    id: call_id.clone(),
                    name: step.tool_name.clone(),
                    arguments: step.parameters.clone(),
                }],
            });

            // P0-03: model context receives ONLY the scrubbed projection of
            // tool output. The trust envelope marks provenance; redaction
            // removes secret material before it becomes ChatMessage content.
            let redactor = crate::telemetry::redactor::SecretRedactor::new();
            let tool_body = if let Some(ref err) = step.error {
                let wrapped_err = TrustEnvelope::wrap_untrusted(
                    "tool://error",
                    TrustLevel::UntrustedToolOutput,
                    &redactor.redact_text(err),
                );
                format!(
                    "Error: {}\n{}",
                    wrapped_err,
                    redactor.redact_text(&truncate_output(&step.output, 4000))
                )
            } else {
                redactor.redact_text(&truncate_output(&step.output, 4000))
            };
            messages.push(ChatMessage::Tool {
                tool_call_id: call_id,
                content: tool_body,
            });
        }

        compiled.messages = messages;
        Ok(compiled)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::{MissionId, TaskId};

    #[tokio::test]
    async fn test_compile_context_emits_manifest_and_envelopes() {
        let compiler = ProductionContextCompiler::new();
        let req = ContextCompilationRequest::new(MissionId::new(), TaskId::new(), 4096);

        let compiled = compiler.compile_context(req).await.unwrap();

        assert!(!compiled.system_prompt.is_empty());
        assert!(compiled.token_count > 0);
        assert!(compiled.manifest.is_some());

        let manifest = compiled.manifest.unwrap();
        assert_eq!(manifest.total_budget, 4096);
        assert_eq!(manifest.reserved_headroom, 1024);
        // 5 base sections + the L2 step-objective contract appended from the
        // computed EffectivePrompt (L3 absent without durable state).
        assert_eq!(manifest.sections.len(), 6);

        // Verify XML trust envelope presence
        assert!(
            compiled
                .system_prompt
                .contains("<untrusted_evidence source=\"task://prerequisites\"")
        );
        assert!(compiled.system_prompt.contains("</untrusted_evidence>"));
    }

    #[test]
    fn test_compile_sections_protected_overflow_fails_closed() {
        let compiler = ProductionContextCompiler::new();
        let tokenizer = TokenizerAdapter::conservative();

        let p0 = ContextSection::new(
            "p0",
            ContextPriority::P0RuntimeSafety,
            "A".repeat(1000), // ~285 tokens
            "test://p0",
            TrustLevel::TrustedSystem,
            false,
            &tokenizer,
        );
        let p1 = ContextSection::new(
            "p1",
            ContextPriority::P1TaskCompletion,
            "B".repeat(1000), // ~285 tokens
            "test://p1",
            TrustLevel::TrustedPolicy,
            false,
            &tokenizer,
        );

        // Total P0 + P1 = ~570 tokens. Budget is 300, reserved headroom is 100 -> admissible = 200 tokens
        let err = compiler
            .compile_sections(vec![p0, p1], 300, 100)
            .unwrap_err();
        match err {
            ContextError::CompilationFailed(msg) => {
                assert!(msg.contains("ContextBudgetExceeded"));
            }
        }
    }

    #[test]
    fn test_compile_sections_reverse_compaction_drops_p4() {
        let compiler = ProductionContextCompiler::new();
        let tokenizer = TokenizerAdapter::conservative();

        let p0 = ContextSection::new(
            "p0",
            ContextPriority::P0RuntimeSafety,
            "P0 Content",
            "test://p0",
            TrustLevel::TrustedSystem,
            false,
            &tokenizer,
        );
        let p1 = ContextSection::new(
            "p1",
            ContextPriority::P1TaskCompletion,
            "P1 Content",
            "test://p1",
            TrustLevel::TrustedPolicy,
            false,
            &tokenizer,
        );
        let p2 = ContextSection::new(
            "p2",
            ContextPriority::P2RequiredEvidence,
            "P2 Content",
            "test://p2",
            TrustLevel::UntrustedToolOutput,
            true,
            &tokenizer,
        );
        let p4 = ContextSection::new(
            "p4",
            ContextPriority::P4OptionalBackground,
            "Large P4 Background Content ".repeat(20),
            "test://p4",
            TrustLevel::UntrustedRepoContent,
            false,
            &tokenizer,
        );

        // Budget fits p0, p1, p2, but not p4
        let tight_budget = p0.token_count + p1.token_count + p2.token_count + 10;
        let compiled = compiler
            .compile_sections(vec![p0, p1, p2, p4], tight_budget + 10, 10)
            .unwrap();

        let manifest = compiled.manifest.unwrap();
        let p4_entry = manifest
            .sections
            .iter()
            .find(|s| s.section_id == "p4")
            .unwrap();
        assert_eq!(p4_entry.compaction_action, "dropped");
        assert_eq!(p4_entry.final_tokens, 0);

        // Prompt shouldn't have p4
        assert!(
            !compiled
                .system_prompt
                .contains("Large P4 Background Content")
        );
    }
}
