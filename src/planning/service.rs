//! Production PlanService implementation bridging planning subsystem to Autonomy Controller (PLN-01, PLN-03, PLN-05).

use async_trait::async_trait;
use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use crate::agent::model_policy::{ModelCaller, ModelProposal};
use crate::ids::MissionId;
use crate::kernel::plan::{
    CandidatePlan, CandidateTask, CandidateTaskKey, CapabilityAccessMode, CapabilityRequirement,
    ResourceEstimate, VerificationStrategy,
};
use crate::kernel::seams::planner::{
    PlanError, PlanRequest, PlanResponse, PlanService, ReplanRequest, ReplanResponse,
};
use crate::planning::constraints::ConstraintDiscoveryEngine;
use crate::planning::projections::frontmatter::ProjectionFrontmatter;
use crate::planning::projections::mission::MissionProjection;
use crate::planning::projections::mission_projections_dir;
use crate::planning::projections::plan::PlanProjection;
use crate::planning::projections::renderer::ProjectionRenderer;
use crate::planning::projections::requirements::RequirementsProjection;
use crate::planning::projections::state::StateProjection;
use crate::planning::requirements::{
    EngineeringRequirement, EpistemicStatus, Provenance, ProvenanceSourceType, TrustLevel,
};
use crate::planning::validation::PlanValidator;
use crate::state_machine::agent::AgentRole;

use crate::prompt::{
    CompilationOptions, DefaultPromptCompiler, EffectivePrompt, InMemoryPromptCatalog,
    PromptCatalog, PromptCompiler, PromptContext,
};
use std::collections::BTreeMap;
use std::sync::Arc;

/// Untagged DTO supporting both string and object forms of capability requirements.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(untagged)]
pub enum CapabilityDto {
    String(String),
    Object {
        id: String,
        #[serde(default)]
        mode: Option<String>,
    },
}

impl CapabilityDto {
    pub fn id(&self) -> &str {
        match self {
            Self::String(s) => s.as_str(),
            Self::Object { id, .. } => id.as_str(),
        }
    }

    pub fn mode(&self) -> Option<&str> {
        match self {
            Self::String(_) => None,
            Self::Object { mode, .. } => mode.as_deref(),
        }
    }
}

/// Untagged DTO supporting both string and object forms of verification strategies.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(untagged)]
pub enum VerificationDto {
    String(String),
    Object {
        #[serde(rename = "type")]
        strategy_type: Option<String>,
        #[serde(default)]
        command: Option<String>,
        #[serde(default)]
        tool: Option<String>,
        #[serde(default)]
        paths: Vec<String>,
    },
}

impl VerificationDto {
    pub fn to_strategy(&self, expected_outputs: &[String]) -> Result<VerificationStrategy, String> {
        match self {
            Self::String(s) => parse_proposed_verification(s, expected_outputs),
            Self::Object {
                strategy_type,
                command,
                tool,
                paths,
            } => {
                if let Some(st) = strategy_type {
                    let (name, arg) = match st.split_once(':') {
                        Some((n, a)) => (n.trim().to_lowercase(), Some(a.trim().to_string())),
                        None => (st.trim().to_lowercase(), None),
                    };
                    match name.as_str() {
                        "compilation" | "compile" => Ok(VerificationStrategy::Compilation),
                        "automated_test" | "shell.exec" | "shell" | "test" | "command" | "exec" => {
                            Ok(VerificationStrategy::AutomatedTest {
                                command: command.clone().or(arg),
                            })
                        }
                        "static_analysis" | "lint" => Ok(VerificationStrategy::StaticAnalysis {
                            tool: tool.clone().or(arg),
                        }),
                        "artifact_inspection" | "artifact" | "artifacts" => {
                            let final_paths = if !expected_outputs.is_empty() {
                                expected_outputs.to_vec()
                            } else {
                                paths.clone()
                            };
                            Ok(VerificationStrategy::ArtifactInspection { paths: final_paths })
                        }
                        "review_gate" | "review" => Ok(VerificationStrategy::ReviewGate {
                            reviewer_role: None,
                        }),
                        _ if name.starts_with("cargo")
                            || name.starts_with("npm")
                            || name.starts_with("pytest")
                            || name.starts_with("python")
                            || name.starts_with("node")
                            || name.starts_with("bash")
                            || name.starts_with("sh ") =>
                        {
                            Ok(VerificationStrategy::AutomatedTest {
                                command: Some(st.clone()),
                            })
                        }
                        _ if name.contains("typescript")
                            || name.contains("tsc")
                            || name.contains("compil")
                            || name.contains("build")
                            || name.contains("syntax") =>
                        {
                            Ok(VerificationStrategy::Compilation)
                        }
                        _ if name.contains("test")
                            || name.contains("spec")
                            || name.contains("assert") =>
                        {
                            Ok(VerificationStrategy::AutomatedTest {
                                command: command.clone().or(arg),
                            })
                        }
                        _ if name.contains("lint")
                            || name.contains("format")
                            || name.contains("static") =>
                        {
                            Ok(VerificationStrategy::StaticAnalysis {
                                tool: tool.clone().or(arg),
                            })
                        }
                        _ if name.contains("artifact")
                            || name.contains("inspect")
                            || name.contains("file")
                            || name.contains("render")
                            || name.contains("output")
                            || !expected_outputs.is_empty() =>
                        {
                            let final_paths = if !expected_outputs.is_empty() {
                                expected_outputs.to_vec()
                            } else {
                                paths.clone()
                            };
                            Ok(VerificationStrategy::ArtifactInspection { paths: final_paths })
                        }
                        other => Err(format!(
                            "unknown verification strategy '{other}'; expected one of: compilation, automated_test[:command], static_analysis[:tool], artifact_inspection, review_gate"
                        )),
                    }
                } else if let Some(cmd) = command {
                    Ok(VerificationStrategy::AutomatedTest {
                        command: Some(cmd.clone()),
                    })
                } else if let Some(t) = tool {
                    Ok(VerificationStrategy::StaticAnalysis {
                        tool: Some(t.clone()),
                    })
                } else {
                    let final_paths = if !expected_outputs.is_empty() {
                        expected_outputs.to_vec()
                    } else {
                        paths.clone()
                    };
                    Ok(VerificationStrategy::ArtifactInspection { paths: final_paths })
                }
            }
        }
    }
}

/// Schema for model-generated task plan decomposition (GAP-04).
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct PlanDecompositionDto {
    #[serde(alias = "candidate_tasks")]
    pub tasks: Vec<PlanTaskDto>,
}

#[derive(Debug, Clone, Default, serde::Deserialize, serde::Serialize)]
pub struct PlanTaskDto {
    /// Canonical `id`; the `planning.decompose` prompt template addresses
    /// this field as `task_id`, so the alias is accepted (verified live
    /// against upstream models that follow prompt vocabulary).
    #[serde(alias = "task_id")]
    pub id: String,
    #[serde(default, alias = "objective", alias = "name")]
    pub title: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default, alias = "dependencies")]
    pub depends_on: Vec<String>,
    #[serde(default, alias = "capabilities")]
    pub required_capabilities: Vec<CapabilityDto>,
    #[serde(default, alias = "assigned_role")]
    pub role: Option<String>,
    #[serde(default)]
    pub timeout_seconds: Option<u64>,
    #[serde(default)]
    pub max_steps: Option<u32>,
    #[serde(default)]
    pub affected_paths: Vec<String>,
    #[serde(default)]
    pub expected_outputs: Vec<String>,
    #[serde(default, alias = "acceptance_criteria")]
    pub completion_criteria: Vec<String>,
    #[serde(default)]
    pub assumptions: Vec<String>,
    #[serde(default)]
    pub risk_level: Option<String>,
    #[serde(default)]
    pub requirement_keys: Vec<String>,
    /// Model-proposed verification strategy: one of
    /// `compilation`, `automated_test[:command]`, `static_analysis[:tool]`,
    /// `artifact_inspection`, `review_gate`. Omitted → the role's registered
    /// default. Unknown values fail explicitly (never silent Compilation).
    #[serde(default)]
    pub verification: Option<VerificationDto>,
}

/// Parse a model-proposed verification strategy declaration.
///
/// Accepted forms (case-insensitive, `name[:argument]`):
/// `compilation`, `automated_test[:command]`, `static_analysis[:tool]`,
/// `artifact_inspection` (inspects the task's declared expected outputs),
/// `review_gate`. Anything else is an explicit error listing valid values —
/// the runtime never silently substitutes Compilation for an unknown value.
fn parse_proposed_verification(
    spec: &str,
    expected_outputs: &[String],
) -> Result<VerificationStrategy, String> {
    let (name, arg) = match spec.split_once(':') {
        Some((n, a)) => (n.trim().to_lowercase(), Some(a.trim().to_string())),
        None => (spec.trim().to_lowercase(), None),
    };
    match name.as_str() {
        "compilation" | "compile" => Ok(VerificationStrategy::Compilation),
        "automated_test" | "shell.exec" | "shell" | "test" | "command" | "exec" => {
            Ok(VerificationStrategy::AutomatedTest { command: arg })
        }
        "static_analysis" | "lint" => Ok(VerificationStrategy::StaticAnalysis { tool: arg }),
        "artifact_inspection" | "artifact" | "artifacts" => {
            Ok(VerificationStrategy::ArtifactInspection {
                paths: expected_outputs.to_vec(),
            })
        }
        "review_gate" | "review" => Ok(VerificationStrategy::ReviewGate {
            reviewer_role: None,
        }),
        _ if name.starts_with("cargo")
            || name.starts_with("npm")
            || name.starts_with("pytest")
            || name.starts_with("python")
            || name.starts_with("node")
            || name.starts_with("bash")
            || name.starts_with("sh ") =>
        {
            Ok(VerificationStrategy::AutomatedTest {
                command: Some(spec.trim().to_string()),
            })
        }
        _ if name.contains("typescript")
            || name.contains("tsc")
            || name.contains("compil")
            || name.contains("build")
            || name.contains("syntax") =>
        {
            Ok(VerificationStrategy::Compilation)
        }
        _ if name.contains("test") || name.contains("spec") || name.contains("assert") => {
            Ok(VerificationStrategy::AutomatedTest { command: arg })
        }
        _ if name.contains("lint") || name.contains("format") || name.contains("static") => {
            Ok(VerificationStrategy::StaticAnalysis { tool: arg })
        }
        _ if name.contains("artifact")
            || name.contains("inspect")
            || name.contains("file")
            || name.contains("render")
            || name.contains("output")
            || !expected_outputs.is_empty() =>
        {
            Ok(VerificationStrategy::ArtifactInspection {
                paths: expected_outputs.to_vec(),
            })
        }
        other => Err(format!(
            "unknown verification strategy '{other}'; expected one of: compilation, automated_test[:command], static_analysis[:tool], artifact_inspection, review_gate"
        )),
    }
}

/// Try one JSON slice as a decomposition DTO, accepting both the canonical
/// object form (`{"tasks": [...]}`) and a bare task array (`[...]`, as
/// emitted by models following the prompt template's per-task instructions).
fn try_parse_dto_slice(slice: &str) -> Option<PlanDecompositionDto> {
    if let Ok(parsed) = serde_json::from_str::<PlanDecompositionDto>(slice) {
        return Some(parsed);
    }
    if let Ok(tasks) = serde_json::from_str::<Vec<PlanTaskDto>>(slice) {
        return Some(PlanDecompositionDto { tasks });
    }
    None
}

pub(crate) fn parse_decomposition_json(text: &str) -> Result<PlanDecompositionDto, String> {
    let trimmed = text.trim();
    if let Some(parsed) = try_parse_dto_slice(trimmed) {
        return Ok(parsed);
    }
    if let Some(start) = trimmed.find("```json") {
        let after = &trimmed[start + 7..];
        if let Some(end) = after.find("```") {
            let inner = after[..end].trim();
            if let Some(parsed) = try_parse_dto_slice(inner) {
                return Ok(parsed);
            }
        }
    } else if let Some(start) = trimmed.find("```") {
        let after = &trimmed[start + 3..];
        if let Some(end) = after.find("```") {
            let inner = after[..end].trim();
            if let Some(parsed) = try_parse_dto_slice(inner) {
                return Ok(parsed);
            }
        }
    }
    // Bare-array fallback: slice from the first '[' to the last ']' so a
    // task array embedded in prose still parses without inventing tasks.
    if let (Some(first_bracket), Some(last_bracket)) = (trimmed.find('['), trimmed.rfind(']'))
        && first_bracket < last_bracket
    {
        let slice = &trimmed[first_bracket..=last_bracket];
        if let Ok(tasks) = serde_json::from_str::<Vec<PlanTaskDto>>(slice) {
            return Ok(PlanDecompositionDto { tasks });
        }
    }
    if let (Some(first_brace), Some(last_brace)) = (trimmed.find('{'), trimmed.rfind('}'))
        && first_brace < last_brace
    {
        let slice = &trimmed[first_brace..=last_brace];
        if let Some(parsed) = try_parse_dto_slice(slice) {
            return Ok(parsed);
        }
    }

    Err(format!(
        "Failed to parse PlanDecompositionDto from output: {}",
        if trimmed.len() > 200 {
            &trimmed[..200]
        } else {
            trimmed
        }
    ))
}

pub(crate) fn parse_dto_from_value(val: serde_json::Value) -> Result<PlanDecompositionDto, String> {
    if let Ok(dto) = serde_json::from_value::<PlanDecompositionDto>(val.clone()) {
        return Ok(dto);
    }
    if let Ok(tasks) = serde_json::from_value::<Vec<PlanTaskDto>>(val.clone()) {
        return Ok(PlanDecompositionDto { tasks });
    }
    if let Some(s) = val.as_str() {
        return parse_decomposition_json(s);
    }
    let val_str = val.to_string();
    parse_decomposition_json(&val_str)
}

pub(crate) fn parse_dto_from_proposal(
    proposal: ModelProposal,
) -> Result<PlanDecompositionDto, String> {
    match proposal {
        ModelProposal::Complete { summary, .. } => parse_decomposition_json(&summary),
        ModelProposal::AssistantText { content } => parse_decomposition_json(&content),
        ModelProposal::ToolCalls { mut calls } => {
            if let Some(first) = calls.pop() {
                parse_dto_from_value(first.arguments)
            } else {
                Err("Model emitted empty tool calls during decomposition".to_string())
            }
        }
        ModelProposal::AskUser { question, .. } => Err(format!(
            "Model requested clarification during decomposition: {question}"
        )),
        ModelProposal::Handoff {
            target_role,
            reason,
        } => Err(format!(
            "Model declined decomposition via handoff to {target_role}: {reason}"
        )),
    }
}

/// Determine whether a model caller error string represents a transient, retryable provider failure
/// and extract any indicated retry delay / cooldown.
pub fn is_transient_provider_error(err: &str) -> (bool, Option<Duration>) {
    let lower = err.to_lowercase();

    // 1. Explicit permanent / unrecoverable failures: never retry at the planning stage
    let permanent_markers = [
        "missing configuration",
        "missing credentials",
        "authentication failed",
        "authentication failure",
        "invalid request",
        "unsupported capability",
        "context window exhausted",
        "cancelled by runtime",
        "operation cancelled",
        "protocol violation",
        "http 400",
        "http 401",
        "http 403",
        "http 404",
        "http 422",
        "status: 400",
        "status: 401",
        "status: 403",
        "status: 404",
        "status: 422",
        "status 400",
        "status 401",
        "status 403",
        "status 404",
        "status 422",
        "misconfigured",
        "only nvidia nim is production-supported",
    ];
    for marker in &permanent_markers {
        if lower.contains(marker) {
            return (false, None);
        }
    }

    // 2. Check for rate limit cooldown
    if lower.contains("rate limit") || lower.contains("rate_limit") || lower.contains("429") {
        let delay = if let Some(idx) = lower.find("retry after ") {
            let rest = &lower[idx + 12..];
            let num_str: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
            num_str.parse::<u64>().ok().map(Duration::from_secs)
        } else if let Some(idx) = lower.find("retry-after: ") {
            let rest = &lower[idx + 13..];
            let num_str: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
            num_str.parse::<u64>().ok().map(Duration::from_secs)
        } else {
            None
        };
        return (true, delay.or(Some(Duration::from_millis(500))));
    }

    // 3. Transient HTTP status codes (500, 502, 503, 504) and standard status phrases
    if lower.contains("500")
        || lower.contains("502")
        || lower.contains("503")
        || lower.contains("504")
        || lower.contains("bad gateway")
        || lower.contains("service unavailable")
        || lower.contains("gateway timeout")
        || lower.contains("internal server error")
    {
        return (true, Some(Duration::from_millis(500)));
    }

    // 4. Timeouts and transport errors
    let transient_markers = [
        "timeout",
        "timed out",
        "network connection",
        "transport error",
        "connection reset",
        "broken pipe",
        "stream interrupted",
        "endpoint unavailable",
        "provider internal failure",
        "temporarily unavailable",
        "empty stream completion",
        "stream closed",
        "stream terminated prematurely",
    ];
    for marker in &transient_markers {
        if lower.contains(marker) {
            return (true, Some(Duration::from_millis(500)));
        }
    }

    (false, None)
}

/// Align a CandidateTask's role to ensure it satisfies mutating privilege and capability envelope constraints.
pub fn align_candidate_task_role(task: &mut CandidateTask) {
    use crate::agent::registry::RoleRegistry;
    let caps_ids: Vec<String> = task.capabilities.iter().map(|c| c.id.clone()).collect();
    let needs_write = task.capabilities.iter().any(|c| {
        c.id == "fs.write"
            || c.id.ends_with(".write")
            || c.mode == CapabilityAccessMode::Write
            || c.mode == CapabilityAccessMode::ReadWrite
    });

    let role_allows_write = RoleRegistry::write_tools_permitted_for(&task.role);
    if needs_write && !role_allows_write {
        if let Some(writable) = RoleRegistry::global()
            .read()
            .ok()
            .and_then(|guard| guard.first_writable_role())
        {
            tracing::info!(
                "Task {} requested mutating capability; aligning role {:?} to {:?}",
                task.id,
                task.role,
                writable
            );
            task.role = writable;
        }
    }

    if let Ok(guard) = RoleRegistry::global().read() {
        let envelope_violates = if let Some(def) = guard.resolve(&task.role) {
            let env = &def.profile.capability_policy;
            caps_ids
                .iter()
                .any(|c| !env.allowed_capabilities.contains(c))
        } else {
            true
        };

        if envelope_violates {
            if let Some(aligned) = guard.infer_role_for_capabilities(&caps_ids) {
                tracing::info!(
                    "Task {} role {:?} lacks required capabilities {:?}; aligning to inferred role {:?}",
                    task.id,
                    task.role,
                    caps_ids,
                    aligned
                );
                task.role = aligned;
            }
        }
    }
}

/// Compile an already-resolved planning/genesis prompt contract through
/// the canonical PromptOS chain (catalog → compiler → EffectivePrompt).
///
/// Shared by the planning service and the pre-execution coordinator so
/// both bind the same compilation semantics. The contract's declared
/// role/stage is authoritative. Fails closed on missing stage or
/// compilation failure — the model invocation MUST NOT happen behind a
/// substitute prompt.
pub(crate) fn compile_resolved_planning_prompt(
    catalog: &dyn PromptCatalog,
    compiler: &dyn PromptCompiler,
    contract: &crate::prompt::PromptContract,
    mission_id: impl Into<String>,
    task_id: impl Into<String>,
    task_objective: impl Into<String>,
    params: BTreeMap<String, String>,
) -> Result<EffectivePrompt, String> {
    let role = contract.role.clone();
    let stage = contract.stage.or_else(|| {
        crate::agent::registry::RoleRegistry::global()
            .read()
            .ok()
            .and_then(|guard| guard.stage_for(&role))
    });
    let stage = stage.ok_or_else(|| {
        format!(
            "unknown agent role '{}': no registered role definition",
            role.as_str()
        )
    })?;
    let mut prompt_ctx = PromptContext::new(
        format!("prompt://planning/{}", contract.id),
        mission_id,
        task_id,
        role,
        stage,
        task_objective,
    );
    prompt_ctx.custom_parameters = params;
    compiler
        .compile_with_guidance(
            catalog,
            contract,
            &prompt_ctx,
            &CompilationOptions::default(),
        )
        .map_err(|e| {
            format!(
                "PromptCompiler failed for '{}' (v{}): {e}",
                contract.id, contract.version
            )
        })
}

/// Production implementation of `PlanService` seam trait.
pub struct PlanServiceImpl {
    workspace_root: PathBuf,
    storage_root: PathBuf,
    validator: PlanValidator,
    model_caller: Option<Arc<dyn ModelCaller>>,
    prompt_catalog: Arc<dyn PromptCatalog>,
    /// Canonical prompt compiler. Isolated default for standalone/test use;
    /// production MUST inject the runtime-shared compiler via
    /// [`PlanServiceImpl::with_prompt_compiler`].
    prompt_compiler: Arc<dyn PromptCompiler>,
}

/// Strongly typed classification of a mission objective's semantic intent.
///
/// Replaces the prior binary `is_read_only_objective` boolean with a richer
/// taxonomy so the planning pipeline can distinguish "direct informational"
/// (zero tasks valid) from "repository research" (executable read-only work
/// expected) and mutating objectives (implementation tasks expected).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ObjectiveClassification {
    /// Purely conversational / informational question — zero execution tasks valid.
    /// Examples: "What is Rust?", "Explain this function", "What does this return?"
    DirectAnswer,
    /// Executable repository research / study / analysis — at least one read-only
    /// Researcher task expected.
    /// Examples: "Study the codebase", "Analyze the architecture", "Investigate the runtime"
    RepositoryResearch,
    /// Executable repository audit (security, reliability, compliance) — at least one
    /// read-only Researcher/Auditor task expected.
    /// Examples: "Audit the codebase for security issues", "Review for reliability problems"
    RepositoryAudit,
    /// Mutating implementation work — one or more implementation tasks expected.
    /// Examples: "Fix the parser", "Implement authentication", "Add logging"
    Implementation,
    /// Verification / test-execution work — verifier-class tasks expected.
    /// Examples: "Run the test suite", "Verify the build passes"
    Verification,
    /// Diagnostic investigation of failures — diagnostician-class tasks expected.
    /// Examples: "Diagnose why tests fail", "Investigate the crash in module X"
    Investigation,
}

impl ObjectiveClassification {
    /// Whether this classification represents read-only (non-mutating) work.
    pub fn is_read_only(self) -> bool {
        matches!(
            self,
            ObjectiveClassification::DirectAnswer
                | ObjectiveClassification::RepositoryResearch
                | ObjectiveClassification::RepositoryAudit
        )
    }

    /// Whether this classification expects at least one executable task
    /// (as opposed to DirectAnswer which may legitimately produce zero tasks).
    pub fn expects_executable_tasks(self) -> bool {
        !matches!(self, ObjectiveClassification::DirectAnswer)
    }
}

/// Classify a mission objective into a strongly typed semantic category.
///
/// Deterministic keyword-based classification with precedence:
/// 1. Mutation keywords present → `Implementation` (mutation takes precedence)
/// 2. Verification keywords (without mutation) → `Verification`
/// 3. Diagnostic keywords (without mutation) → `Investigation`
/// 4. Audit keywords (without mutation) → `RepositoryAudit`
/// 5. Research/study/analysis keywords (without mutation) → `RepositoryResearch`
/// 6. Direct-answer patterns → `DirectAnswer`
/// 7. Fallback: `Implementation` (fail-closed for unrecognized objectives)
pub fn classify_objective(objective: &str) -> ObjectiveClassification {
    let lower = objective.to_lowercase();

    // Keywords that indicate mutating work — highest precedence.
    let mutation_keywords = [
        "fix",
        "repair",
        "implement",
        "add ",
        "modify",
        "update",
        "create",
        "write",
        "delete",
        "remove",
        "build",
        "refactor",
        "migrate",
        "upgrade",
        "patch",
    ];

    // Keywords that specifically indicate security/reliability audit.
    let audit_keywords = [
        "audit",
        "security review",
        "security audit",
        "security",
        "reliability",
        "vulnerability",
        "compliance",
        "penetration",
    ];

    // Keywords that indicate verification / testing work.
    let verification_keywords = [
        "verify",
        "run tests",
        "run the tests",
        "test suite",
        "validate",
        "check compilation",
    ];

    // Keywords that indicate diagnostic investigation.
    let diagnostic_keywords = ["diagnose", "debug", "troubleshoot", "root cause"];

    // Keywords that indicate research / study / analysis of the repository.
    let research_keywords = [
        "study",
        "analyze",
        "analyse",
        "inspect",
        "review",
        "explore",
        "investigate",
        "understand",
        "map ",
        "map the",
        "trace",
        "survey",
        "examine",
        "research",
        "read-only",
        "readonly",
    ];

    // Keywords that indicate purely informational / conversational questions.
    let direct_answer_keywords = [
        "what is ",
        "what are ",
        "what does ",
        "explain ",
        "describe ",
        "how does ",
        "why does ",
        "tell me about ",
        "define ",
        "summarize ",
        "list ",
        "show me ",
    ];

    let has_mutation = mutation_keywords.iter().any(|&kw| lower.contains(kw));
    let has_verification = verification_keywords.iter().any(|&kw| lower.contains(kw));
    let has_diagnostic = diagnostic_keywords.iter().any(|&kw| lower.contains(kw));
    let has_audit = audit_keywords.iter().any(|&kw| lower.contains(kw));
    let has_research = research_keywords.iter().any(|&kw| lower.contains(kw));
    let has_direct_answer = direct_answer_keywords.iter().any(|&kw| lower.contains(kw));

    // 1. Mutation keywords take highest precedence — the objective wants changes.
    if has_mutation {
        return ObjectiveClassification::Implementation;
    }

    // 2. Verification without mutation → verification work.
    if has_verification {
        return ObjectiveClassification::Verification;
    }

    // 3. Diagnostic without mutation → investigation work.
    if has_diagnostic {
        return ObjectiveClassification::Investigation;
    }

    // 4. Audit keywords → repository audit (read-only but executable).
    if has_audit {
        return ObjectiveClassification::RepositoryAudit;
    }

    // 5. Research/study/analysis keywords → executable repository research.
    if has_research {
        return ObjectiveClassification::RepositoryResearch;
    }

    // 6. Direct-answer patterns → conversational answer (0 execution tasks valid).
    if has_direct_answer {
        return ObjectiveClassification::DirectAnswer;
    }

    // 7. Fail-closed: unrecognized objectives are treated as actionable
    //    so the runtime correctly demands a model-generated plan.
    ObjectiveClassification::Implementation
}

/// Determine whether a mission objective is an informational or read-only inquiry.
///
/// Backward-compatible projection of [`classify_objective`] for existing call sites.
/// Returns `true` for `DirectAnswer`, `RepositoryResearch`, and `RepositoryAudit`.
pub fn is_read_only_objective(objective: &str) -> bool {
    classify_objective(objective).is_read_only()
}

impl PlanServiceImpl {
    pub fn new(storage_root: impl Into<PathBuf>) -> Self {
        let root = storage_root.into();
        Self::new_with_roots(root.clone(), root)
    }

    pub fn new_with_roots(
        workspace_root: impl Into<PathBuf>,
        storage_root: impl Into<PathBuf>,
    ) -> Self {
        let ws = workspace_root.into();
        let root = storage_root.into();
        Self {
            workspace_root: ws,
            storage_root: root,
            validator: PlanValidator::new(),
            model_caller: None,
            // Isolated defaults for standalone/test use. Production binds
            // the runtime-shared authorities via `with_prompt_catalog` /
            // `with_prompt_compiler` (see controller dependencies).
            prompt_catalog: Arc::new(InMemoryPromptCatalog::with_builtins()),
            prompt_compiler: Arc::new(DefaultPromptCompiler::new()),
        }
    }

    pub fn with_validator(mut self, validator: PlanValidator) -> Self {
        self.validator = validator;
        self
    }

    pub fn with_model_caller(mut self, caller: Arc<dyn ModelCaller>) -> Self {
        self.model_caller = Some(caller);
        self
    }

    pub fn with_prompt_catalog(mut self, catalog: Arc<dyn PromptCatalog>) -> Self {
        self.prompt_catalog = catalog;
        self
    }

    /// Bind the canonical prompt compiler (runtime-shared in production).
    pub fn with_prompt_compiler(mut self, compiler: Arc<dyn PromptCompiler>) -> Self {
        self.prompt_compiler = compiler;
        self
    }
    /// Compile an already-resolved prompt contract through the canonical
    /// PromptOS chain with the given template parameters.
    pub(crate) fn compile_resolved_prompt(
        &self,
        contract: &crate::prompt::PromptContract,
        mission_id: impl Into<String>,
        task_id: impl Into<String>,
        task_objective: impl Into<String>,
        params: BTreeMap<String, String>,
    ) -> Result<EffectivePrompt, PlanError> {
        compile_resolved_planning_prompt(
            self.prompt_catalog.as_ref(),
            self.prompt_compiler.as_ref(),
            contract,
            mission_id,
            task_id,
            task_objective,
            params,
        )
        .map_err(PlanError::GenerationFailed)
    }

    /// Map a model-produced decomposition DTO into validated candidate tasks.
    ///
    /// Declarative authority boundary: the model proposes
    /// task shape (count, titles, dependencies, requested capabilities); role
    /// identity, capability defaults, and verification defaults resolve
    /// through the role registry. This mapping is a generic mechanism — it
    /// contains no role-specific branches and no domain task content.
    pub(crate) fn map_decomposition_to_tasks(
        &self,
        dto: PlanDecompositionDto,
    ) -> Result<Vec<CandidateTask>, PlanError> {
        use crate::agent::registry::RoleRegistry;
        let mut mapped_tasks = Vec::new();
        for t in dto.tasks {
            let caps_ids: Vec<String> = t
                .required_capabilities
                .iter()
                .map(|c| c.id().to_string())
                .collect();

            // Validate AgentRole against registered definitions; reject unknown roles.
            let mut role = if let Some(ref r) = t.role {
                self.validator
                    .validate_role(t.id.as_str(), r)
                    .map_err(|e| {
                        PlanError::GenerationFailed(format!(
                            "Candidate plan validation rejected: {:?}",
                            e
                        ))
                    })?
            } else {
                // No role proposed: consult the registry's ordered inference
                // rules (shared with the dispatcher). Researcher remains the
                // documented fallback for capability-free investigation tasks.
                RoleRegistry::global()
                    .read()
                    .ok()
                    .and_then(|guard| guard.infer_role_for_capabilities(&caps_ids))
                    .unwrap_or_else(AgentRole::researcher)
            };

            // Runtime authority: a task requesting mutating write capabilities
            // must run under a role whose definition permits write tools. The
            // target is the first writable role in registry inference order
            // (implementer among built-ins), not a hardcoded variant.
            let needs_write = t
                .required_capabilities
                .iter()
                .any(|c| c.id() == "fs.write" || c.id().ends_with(".write"));
            let role_allows_write = RoleRegistry::write_tools_permitted_for(&role);
            if needs_write && !role_allows_write {
                let writable = RoleRegistry::global()
                    .read()
                    .ok()
                    .and_then(|guard| guard.first_writable_role())
                    .ok_or_else(|| {
                        PlanError::GenerationFailed(
                            "Task requests mutating capabilities but no writable role is registered"
                                .to_string(),
                        )
                    })?;
                tracing::info!(
                    "Task {} requested mutating capability; aligning proposed role {:?} to {:?}",
                    t.id,
                    role,
                    writable,
                );
                role = writable;
            }

            let caps = if !t.required_capabilities.is_empty() {
                t.required_capabilities
                    .iter()
                    .map(|c| {
                        let mode = if let Some(m) = c.mode() {
                            m.parse::<CapabilityAccessMode>().unwrap_or_else(|_| {
                                if c.id() == "fs.write" || c.id().ends_with(".write") {
                                    CapabilityAccessMode::Write
                                } else if c.id() == "proc.exec" || c.id() == "evidence.record" {
                                    CapabilityAccessMode::ReadWrite
                                } else {
                                    CapabilityAccessMode::Read
                                }
                            })
                        } else if c.id() == "fs.write" || c.id().ends_with(".write") {
                            CapabilityAccessMode::Write
                        } else if c.id() == "proc.exec" || c.id() == "evidence.record" {
                            CapabilityAccessMode::ReadWrite
                        } else {
                            CapabilityAccessMode::Read
                        };
                        CapabilityRequirement::new(c.id(), mode)
                    })
                    .collect()
            } else {
                // Role-declared capability defaults (registry data, not a match).
                RoleRegistry::global()
                    .read()
                    .ok()
                    .and_then(|guard| {
                        guard
                            .resolve(&role)
                            .map(|def| def.default_capabilities.clone())
                    })
                    .unwrap_or_else(|| {
                        vec![CapabilityRequirement::new(
                            "fs.read",
                            CapabilityAccessMode::Read,
                        )]
                    })
            };

            let estimates = ResourceEstimate::new(
                t.max_steps.unwrap_or(30),
                t.timeout_seconds.unwrap_or(300),
                20_000,
                0.20,
            );

            let risk_level =
                t.risk_level
                    .as_deref()
                    .and_then(|r| match r.to_lowercase().as_str() {
                        "low" => Some(crate::kernel::plan::TaskRiskLevel::Low),
                        "medium" => Some(crate::kernel::plan::TaskRiskLevel::Medium),
                        "high" => Some(crate::kernel::plan::TaskRiskLevel::High),
                        "critical" => Some(crate::kernel::plan::TaskRiskLevel::Critical),
                        _ => None,
                    });

            // Verification strategy: the model may propose
            // one; the runtime validates the value and honors it downstream.
            // Omitted → the role's registered default. Unknown values are explicit failures.
            let verification = match t.verification.as_ref() {
                None => RoleRegistry::global()
                    .read()
                    .ok()
                    .and_then(|guard| {
                        guard
                            .resolve(&role)
                            .map(|def| def.default_verification.clone())
                    })
                    .unwrap_or(VerificationStrategy::Compilation),
                Some(dto) => dto.to_strategy(&t.expected_outputs).map_err(|e| {
                    PlanError::GenerationFailed(format!(
                        "Task {} declares an invalid verification strategy: {}",
                        t.id, e
                    ))
                })?,
            };

            let mut candidate = CandidateTask {
                id: CandidateTaskKey::new(t.id),
                objective: t.title,
                description: t.description,
                depends_on: t
                    .depends_on
                    .into_iter()
                    .map(CandidateTaskKey::new)
                    .collect(),
                capabilities: caps,
                role,
                verification,
                estimates,
                affected_paths: t.affected_paths,
                expected_outputs: t.expected_outputs,
                completion_criteria: t.completion_criteria,
                assumptions: t.assumptions,
                risk_level,
                requirement_keys: t.requirement_keys,
                // Planning-generated tasks carry no workflow-step binding:
                // the role default resolves at context compilation time.
                prompt_ref: None,
            };
            align_candidate_task_role(&mut candidate);
            mapped_tasks.push(candidate);
        }
        Ok(mapped_tasks)
    }

    /// Synthesize a deterministic read-only researcher task for repository
    /// research or audit objectives when the model incorrectly returns zero
    /// tasks.
    ///
    /// This is NOT fabricating arbitrary implementation work. It is a
    /// deterministic recovery for a well-understood semantic class:
    /// - Uses the existing `researcher` role from the RoleRegistry
    /// - Grants only `fs.read` and `repo.read` capabilities (read-only)
    /// - Does NOT grant `fs.write` or any mutating capability
    /// - The task objective and acceptance criteria are derived from the
    ///   operator's original objective
    fn synthesize_research_task(
        &self,
        objective: &str,
        classification: ObjectiveClassification,
    ) -> CandidateTask {
        let (task_title, task_description) = match classification {
            ObjectiveClassification::RepositoryAudit => (
                format!("Audit: {}", objective),
                format!(
                    "Conduct a read-only audit of the repository as requested: {}. \
                     Inspect repository structure, relevant source files, tests, and \
                     documentation. Produce evidence-backed findings.",
                    objective
                ),
            ),
            _ => (
                format!("Research: {}", objective),
                format!(
                    "Conduct read-only research on the repository as requested: {}. \
                     Inspect repository structure, important entry points, trace major \
                     module relationships, inspect relevant implementation and tests, \
                     and produce evidence-backed findings.",
                    objective
                ),
            ),
        };

        CandidateTask {
            id: CandidateTaskKey::new("TASK-RESEARCH-01"),
            objective: task_title,
            description: Some(task_description),
            depends_on: Vec::new(),
            capabilities: vec![
                CapabilityRequirement::new("fs.read", CapabilityAccessMode::Read),
                CapabilityRequirement::new("repo.read", CapabilityAccessMode::Read),
            ],
            role: AgentRole::researcher(),
            verification: VerificationStrategy::ArtifactInspection { paths: Vec::new() },
            estimates: ResourceEstimate::default(),
            affected_paths: Vec::new(),
            expected_outputs: vec!["Research findings and analysis".to_string()],
            completion_criteria: vec![
                "Repository structure inspected".to_string(),
                "Key entry points and modules identified".to_string(),
                "Evidence-backed findings produced".to_string(),
            ],
            assumptions: Vec::new(),
            risk_level: None,
            requirement_keys: Vec::new(),
            // Deterministic recovery task: no workflow-step binding; the
            // researcher role default resolves at compilation time.
            prompt_ref: None,
        }
    }

    /// Renders and writes all one-way planning projections to disk.
    ///
    /// Threat mitigation: Fail-safe I/O — projection write errors are logged and
    /// do not cause core loop panics.
    fn write_projections_fail_safe(
        &self,
        mission_id: MissionId,
        sequence: u64,
        objective: &str,
        plan: &CandidatePlan,
        requirements: &[EngineeringRequirement],
        constraints: &[String],
    ) {
        let proj_dir = mission_projections_dir(&self.storage_root, mission_id);
        if let Err(e) = fs::create_dir_all(&proj_dir) {
            eprintln!(
                "Warning: Failed to create projections directory {}: {}",
                proj_dir.display(),
                e
            );
            return;
        }

        // 1. PLAN.md
        let plan_fm = ProjectionFrontmatter::new("plan", mission_id, sequence);
        let plan_proj = PlanProjection::new(plan_fm, plan.clone());
        let _ = fs::write(proj_dir.join("PLAN.md"), plan_proj.render());

        // 2. MISSION.md
        let mission_fm = ProjectionFrontmatter::new("mission", mission_id, sequence);
        let mission_proj =
            MissionProjection::new(mission_fm, objective).with_constraints(constraints.to_vec());
        let _ = fs::write(proj_dir.join("MISSION.md"), mission_proj.render());

        // 3. REQUIREMENTS.md
        let req_fm = ProjectionFrontmatter::new("requirements", mission_id, sequence);
        let req_proj = RequirementsProjection::new(req_fm, requirements.to_vec());
        let _ = fs::write(proj_dir.join("REQUIREMENTS.md"), req_proj.render());

        // 4. STATE.md
        let state_fm = ProjectionFrontmatter::new("state", mission_id, sequence);
        let state_proj = StateProjection::new(state_fm, "Planning", sequence)
            .with_stage("IdentifyReadyWork")
            .with_task_counts(plan.task_count(), 0);
        let _ = fs::write(proj_dir.join("STATE.md"), state_proj.render());

        // 5. plan.json (Machine-readable plan authority, D-01, FINDING-06)
        if let Ok(plan_json) = serde_json::to_string_pretty(plan) {
            let _ = fs::write(proj_dir.join("plan.json"), plan_json);
        }
    }
}

#[async_trait]
impl PlanService for PlanServiceImpl {
    async fn has_valid_plan(&self, mission_id: MissionId) -> Result<bool, PlanError> {
        let plan_file = mission_projections_dir(&self.storage_root, mission_id).join("plan.json");
        if !plan_file.is_file() {
            return Ok(false);
        }
        match fs::read_to_string(&plan_file) {
            Ok(content) => match serde_json::from_str::<CandidatePlan>(&content) {
                Ok(_) => Ok(true),
                Err(_) => Ok(false),
            },
            Err(_) => Ok(false),
        }
    }

    async fn generate_initial_plan(&self, req: PlanRequest) -> Result<PlanResponse, PlanError> {
        // 1. Discover static repository constraints from the true workspace root (GAP-03)
        let discovered_constraints = ConstraintDiscoveryEngine::discover(&self.workspace_root);
        let constraint_strings: Vec<String> = discovered_constraints
            .iter()
            .map(|c| c.description())
            .collect();

        let plan_id = format!("plan-{}", req.mission_id);

        let candidate_tasks = if let Some(ref caller) = self.model_caller {
            // Source prompt contract from PromptCatalog (planning.decompose v2 with v1 fallback)
            let mut prompt_params = BTreeMap::new();
            prompt_params.insert("mission_objective".to_string(), req.objective.clone());
            prompt_params.insert(
                "constraints".to_string(),
                if constraint_strings.is_empty() {
                    "None detected".to_string()
                } else {
                    constraint_strings.join("\n")
                },
            );
            // Available roles are listed from the registry so newly
            // registered roles are visible to the planner without code edits.
            let available_roles = crate::agent::registry::RoleRegistry::global()
                .read()
                .map(|guard| guard.builtin_ids().join(", "))
                .unwrap_or_else(|_| "implementer, researcher, verifier".to_string());
            prompt_params.insert("available_roles".to_string(), available_roles);

            // Populate PromptOS V2 parameters
            let (charter_param, arch_param) = if let Some(ref ctx) = req.upstream_context {
                (ctx.charter.clone(), ctx.architecture.clone())
            } else {
                (
                    format!("Project charter for objective: {}", req.objective),
                    "Unspecified target architecture; derive work structure from the mission objective, repository evidence, and declared constraints"
                        .to_string(),
                )
            };
            prompt_params.insert("goal".to_string(), req.objective.clone());
            prompt_params.insert("charter".to_string(), charter_param);
            prompt_params.insert("architecture".to_string(), arch_param);
            // Upstream planning artifacts:
            // Pass requirements, assumptions, decisions, research, unknowns,
            // user_decisions, and resolved_invariants into prompt params so
            // uncertainty is preserved and surfaced rather than dropped.
            let (
                req_param,
                assume_param,
                decide_param,
                research_param,
                unknowns_param,
                user_decide_param,
                resolved_param,
            ) = if let Some(ref ctx) = req.upstream_context {
                (
                    ctx.requirements.join("\n"),
                    ctx.assumptions.join("\n"),
                    ctx.decisions.join("\n"),
                    ctx.research_summary.clone().unwrap_or_default(),
                    ctx.unknowns.join("\n"),
                    ctx.user_decisions.join("\n"),
                    ctx.resolved_invariants.join("\n"),
                )
            } else {
                (
                    String::new(),
                    String::new(),
                    String::new(),
                    String::new(),
                    String::new(),
                    String::new(),
                    String::new(),
                )
            };
            prompt_params.insert("requirements".to_string(), req_param);
            prompt_params.insert("assumptions".to_string(), assume_param);
            prompt_params.insert("decisions".to_string(), decide_param);
            prompt_params.insert("research_summary".to_string(), research_param);
            prompt_params.insert("unknowns".to_string(), unknowns_param);
            prompt_params.insert("user_decisions".to_string(), user_decide_param);
            prompt_params.insert("resolved_invariants".to_string(), resolved_param);

            let contract = match self
                .prompt_catalog
                .get("planning.decompose", 2)
                .or_else(|_| self.prompt_catalog.get("planning.decompose", 1))
            {
                Ok(contract) => contract,
                Err(e) => {
                    return Err(PlanError::GenerationFailed(format!(
                        "Failed to retrieve planning prompt contract: {}",
                        e
                    )));
                }
            };

            let max_retries = match contract.failure_policy.as_ref() {
                Some(fp) => match fp.behavior {
                    crate::prompt::v2::FailureBehavior::Retryable => fp.max_retries,
                    _ => 0,
                },
                None => 2,
            };

            // Canonical PromptOS compilation: the resolved decompose
            // contract compiles through the 7-layer PromptCompiler (same
            // authority as worker execution), not template-only rendering.
            let planning_prompt = self
                .compile_resolved_prompt(
                    contract,
                    req.mission_id.to_string(),
                    plan_id.clone(),
                    req.objective.clone(),
                    prompt_params,
                )
                .map_err(|e| {
                    PlanError::GenerationFailed(format!("Failed to compile planning prompt: {}", e))
                })?
                .assembled_text;

            let mut attempt = 0;
            loop {
                // Typed tool-free authority: decomposition prompts are
                // reasoning-only; executable tool schemas are never served
                // for them (no prompt-text inference).
                let tool_free_cancel = tokio_util::sync::CancellationToken::new();
                let proposal_res = caller
                    .call_model_tool_free_cancellable(&planning_prompt, &tool_free_cancel)
                    .await;
                match proposal_res {
                    Ok(proposal) => {
                        // Declarative authority boundary: the model proposes
                        // the decomposition; the runtime validates it. A malformed,
                        // empty, or declined model response is an explicit planning
                        // failure surfaced for recovery/re-planning — the runtime
                        // MUST NOT fabricate domain-specific tasks in its place.
                        let dto = match parse_dto_from_proposal(proposal) {
                            Ok(dto) => dto,
                            Err(e) => {
                                return Err(PlanError::GenerationFailed(format!(
                                    "Model returned malformed plan JSON: {e}; explicit re-planning or recovery required — no fallback tasks substituted"
                                )));
                            }
                        };

                        if dto.tasks.is_empty() {
                            let classification = classify_objective(&req.objective);
                            match classification {
                                ObjectiveClassification::DirectAnswer => {
                                    // Purely conversational: zero tasks is valid.
                                    break Vec::new();
                                }
                                ObjectiveClassification::RepositoryResearch
                                | ObjectiveClassification::RepositoryAudit => {
                                    // The model incorrectly returned zero tasks for an
                                    // executable research/audit objective. Synthesize a
                                    // deterministic researcher task — this is NOT fabricating
                                    // arbitrary implementation work; it is deterministic
                                    // recovery for a well-understood semantic class.
                                    break vec![
                                        self.synthesize_research_task(
                                            &req.objective,
                                            classification,
                                        ),
                                    ];
                                }
                                _ => {
                                    // Actionable objective: retry or fail explicitly.
                                    if attempt < max_retries {
                                        attempt += 1;
                                        tokio::time::sleep(Duration::from_millis(500)).await;
                                        continue;
                                    }
                                    return Err(PlanError::GenerationFailed(
                                        "Model returned zero tasks for an actionable objective; explicit re-planning or recovery required — no fallback tasks substituted"
                                            .to_string(),
                                    ));
                                }
                            }
                        } else {
                            break self.map_decomposition_to_tasks(dto)?;
                        }
                    }
                    Err(err) => {
                        let (is_transient, cooldown) = is_transient_provider_error(&err);
                        if is_transient && attempt < max_retries {
                            attempt += 1;
                            let delay = cooldown.unwrap_or_else(|| {
                                Duration::from_millis(50 * (1 << attempt.min(5)))
                            });
                            let capped_delay = delay.min(Duration::from_secs(60));
                            tokio::time::sleep(capped_delay).await;
                            continue;
                        }

                        return Err(PlanError::GenerationFailed(format!(
                            "Model planner invocation failed ({err}); explicit re-planning or recovery required — no fallback tasks substituted"
                        )));
                    }
                }
            }
        } else {
            // No model provider configured. The runtime refuses to fabricate
            // domain-specific work: a direct-informational inquiry legitimately yields
            // an empty plan, a repository-research/audit objective synthesizes a
            // deterministic researcher task, while an actionable objective is an
            // explicit failure directing the operator to configure a model provider
            // or supply an upstream plan context (recovery path: replan with evidence).
            let classification = classify_objective(&req.objective);
            match classification {
                ObjectiveClassification::DirectAnswer => Vec::new(),
                ObjectiveClassification::RepositoryResearch
                | ObjectiveClassification::RepositoryAudit => {
                    vec![self.synthesize_research_task(&req.objective, classification)]
                }
                _ => {
                    return Err(PlanError::GenerationFailed(
                        "No model provider configured for plan decomposition and objective requires action; configure a model provider or supply upstream plan context — refusing to fabricate tasks"
                            .to_string(),
                    ));
                }
            }
        };

        let plan = CandidatePlan::new(&plan_id, &req.objective, candidate_tasks);

        // 3. Multi-stage validation pipeline
        let validator = if plan.tasks.is_empty()
            && classify_objective(&req.objective) == ObjectiveClassification::DirectAnswer
        {
            self.validator.clone().with_allow_empty(true)
        } else {
            self.validator.clone()
        };

        let report = validator.validate(&plan).await;
        if !report.is_valid() {
            return Err(PlanError::GenerationFailed(format!(
                "Candidate plan validation rejected: {} blocking error(s) ({:?}); explicit re-planning or recovery required — no fallback tasks substituted",
                report.error_count(),
                report.errors
            )));
        }

        // 4. Initial requirements tracking
        let prov = Provenance::new(
            ProvenanceSourceType::UserPrompt,
            TrustLevel::AuthoritativeRuntime,
            "planner_service",
        );
        let req1 = EngineeringRequirement::new(
            "REQ-INIT-01",
            format!("Fulfill mission objective: {}", req.objective),
            EpistemicStatus::ExplicitUserRequirement,
            prov,
            vec!["Automated verification passing".to_string()],
        );

        // 5. Render and persist one-way projections
        self.write_projections_fail_safe(
            req.mission_id,
            1,
            &req.objective,
            &plan,
            &[req1],
            &constraint_strings,
        );

        Ok(PlanResponse {
            plan_id,
            task_count: plan.task_count(),
            candidate_plan: plan,
        })
    }

    async fn replan(&self, req: ReplanRequest) -> Result<ReplanResponse, PlanError> {
        // 1. Load existing plan.json if present — preserve completed/unaffected tasks.
        let plan_file =
            mission_projections_dir(&self.storage_root, req.mission_id).join("plan.json");
        let existing_plan: Option<CandidatePlan> = if plan_file.is_file() {
            fs::read_to_string(&plan_file)
                .ok()
                .and_then(|c| serde_json::from_str(&c).ok())
        } else {
            None
        };

        // 2. Determine next revision and parent plan id.
        let (current_revision, parent_plan_id) = if let Some(ref ep) = existing_plan {
            (ep.revision, ep.plan_id.clone())
        } else {
            (1u32, format!("plan-{}", req.mission_id))
        };
        let next_revision = current_revision + 1;
        let new_plan_id = format!("replan-{}-{}", req.mission_id, next_revision);

        let failed_task_id_str = req.failed_task_id.to_string();
        let (failed_task_deps, failed_task_role, failed_task_verification, failed_task_estimates) =
            if let Some(ref ep) = existing_plan {
                if let Some(original) = ep
                    .tasks
                    .iter()
                    .find(|t| t.id.as_str() == failed_task_id_str.as_str())
                {
                    (
                        original.depends_on.clone(),
                        original.role.clone(),
                        original.verification.clone(),
                        original.estimates.clone(),
                    )
                } else {
                    (
                        vec![],
                        AgentRole::implementer(),
                        VerificationStrategy::Compilation,
                        ResourceEstimate::default(),
                    )
                }
            } else {
                (
                    vec![],
                    AgentRole::implementer(),
                    VerificationStrategy::Compilation,
                    ResourceEstimate::default(),
                )
            };

        let surviving_tasks: Vec<CandidateTask> = if let Some(ref ep) = existing_plan {
            ep.tasks
                .iter()
                .filter(|t| t.id.as_str() != failed_task_id_str.as_str())
                .cloned()
                .collect()
        } else {
            Vec::new()
        };

        let objective = existing_plan
            .as_ref()
            .map(|ep| ep.objective.clone())
            .unwrap_or_else(|| format!("Replan for failure: {}", req.reason));

        // 3. Decompose remediation: Model proposes if caller is configured,
        //    otherwise derive structured repair task dynamically from the failed task.
        let remediation_tasks: Vec<CandidateTask> = if let Some(ref caller) = self.model_caller {
            let mut prompt_params = BTreeMap::new();
            prompt_params.insert("mission_objective".to_string(), objective.clone());
            prompt_params.insert(
                "goal".to_string(),
                format!(
                    "Remediate failed task {}: {}",
                    req.failed_task_id, req.reason
                ),
            );
            prompt_params.insert(
                "charter".to_string(),
                format!(
                    "Adaptive replanning to recover mission after task {} failed: {}",
                    req.failed_task_id, req.reason
                ),
            );
            let surviving_summary = surviving_tasks
                .iter()
                .map(|t| format!("- Task {}: {}", t.id, t.objective))
                .collect::<Vec<_>>()
                .join("\n");
            prompt_params.insert(
                "architecture".to_string(),
                format!(
                    "Surviving plan DAG:\n{}\nFailed task: {}\nFailure reason: {}",
                    if surviving_summary.is_empty() {
                        "None"
                    } else {
                        &surviving_summary
                    },
                    req.failed_task_id,
                    req.reason
                ),
            );
            let available_roles = crate::agent::registry::RoleRegistry::global()
                .read()
                .map(|guard| guard.builtin_ids().join(", "))
                .unwrap_or_else(|_| "implementer, researcher, verifier".to_string());
            prompt_params.insert("available_roles".to_string(), available_roles);
            prompt_params.insert(
                "constraints".to_string(),
                format!("Failed task id: {}", req.failed_task_id),
            );
            prompt_params.insert(
                "requirements".to_string(),
                format!("Remediate failure: {}", req.reason),
            );
            prompt_params.insert("assumptions".to_string(), String::new());
            prompt_params.insert("decisions".to_string(), String::new());
            prompt_params.insert("research_summary".to_string(), String::new());
            prompt_params.insert("unknowns".to_string(), String::new());
            prompt_params.insert("user_decisions".to_string(), String::new());
            prompt_params.insert("resolved_invariants".to_string(), String::new());

            let contract = self
                .prompt_catalog
                .get("planning.decompose", 2)
                .or_else(|_| self.prompt_catalog.get("planning.decompose", 1))
                .map_err(|e| {
                    PlanError::GenerationFailed(format!(
                        "Failed to retrieve planning prompt contract for replan: {}",
                        e
                    ))
                })?;

            let planning_prompt = self
                .compile_resolved_prompt(
                    contract,
                    req.mission_id.to_string(),
                    format!("replan-{}", req.mission_id),
                    objective.clone(),
                    prompt_params,
                )
                .map_err(|e| {
                    PlanError::GenerationFailed(format!("Failed to compile replan prompt: {}", e))
                })?
                .assembled_text;

            let max_retries = 2;
            let mut attempt = 0;
            let dto = loop {
                // Typed tool-free authority: replan prompts are
                // reasoning-only (see decomposition call above).
                let tool_free_cancel = tokio_util::sync::CancellationToken::new();
                let proposal = match caller
                    .call_model_tool_free_cancellable(&planning_prompt, &tool_free_cancel)
                    .await
                {
                    Ok(p) => p,
                    Err(err) => {
                        let (is_transient, cooldown) = is_transient_provider_error(&err);
                        if is_transient && attempt < max_retries {
                            attempt += 1;
                            let delay = cooldown.unwrap_or_else(|| {
                                Duration::from_millis(50 * (1 << attempt.min(5)))
                            });
                            let capped_delay = delay.min(Duration::from_secs(60));
                            tokio::time::sleep(capped_delay).await;
                            continue;
                        }
                        return Err(PlanError::GenerationFailed(format!(
                            "Model replanner invocation failed ({err}); explicit recovery required"
                        )));
                    }
                };

                let parsed_dto = match proposal {
                    ModelProposal::Complete { summary, .. } => parse_decomposition_json(&summary),
                    ModelProposal::AssistantText { content } => parse_decomposition_json(&content),
                    ModelProposal::ToolCalls { mut calls } => {
                        if let Some(first) = calls.pop() {
                            parse_dto_from_value(first.arguments)
                        } else {
                            if attempt < max_retries {
                                attempt += 1;
                                tokio::time::sleep(Duration::from_millis(500)).await;
                                continue;
                            }
                            return Err(PlanError::GenerationFailed(
                                "Model emitted empty tool calls during replan".to_string(),
                            ));
                        }
                    }
                    ModelProposal::AskUser { question, .. } => {
                        return Err(PlanError::GenerationFailed(format!(
                            "Model requested clarification during replan: {}",
                            question
                        )));
                    }
                    ModelProposal::Handoff {
                        target_role,
                        reason,
                    } => {
                        return Err(PlanError::GenerationFailed(format!(
                            "Model declined replan via handoff to {}: {}",
                            target_role, reason
                        )));
                    }
                };

                match parsed_dto {
                    Ok(d) => break d,
                    Err(e) => {
                        if attempt < max_retries {
                            attempt += 1;
                            tokio::time::sleep(Duration::from_millis(500)).await;
                            continue;
                        }
                        return Err(PlanError::GenerationFailed(format!(
                            "Model returned malformed replan JSON: {}",
                            e
                        )));
                    }
                }
            };

            if dto.tasks.is_empty() {
                return Err(PlanError::GenerationFailed(
                    "Model returned zero tasks for replan remediation; explicit re-planning or recovery required — refusing to fabricate remediation tasks".to_string(),
                ));
            } else {
                self.map_decomposition_to_tasks(dto)?
            }
        } else {
            vec![CandidateTask {
                id: CandidateTaskKey::new(format!("fix-{}", req.failed_task_id)),
                objective: format!(
                    "Remediate failure on task {}: {}",
                    req.failed_task_id, req.reason
                ),
                description: Some("Correct state and rerun verification".to_string()),
                depends_on: failed_task_deps,
                capabilities: vec![CapabilityRequirement::new(
                    "fs.write",
                    CapabilityAccessMode::Write,
                )],
                role: failed_task_role,
                verification: failed_task_verification,
                estimates: failed_task_estimates,
                completion_criteria: vec![format!(
                    "Remediation for task {} verified",
                    req.failed_task_id
                )],
                risk_level: Some(crate::kernel::plan::TaskRiskLevel::High),
                ..Default::default()
            }]
        };

        // 4. Assemble revised task list (remediation tasks replace surviving tasks with same ID)
        let remediation_ids: std::collections::HashSet<_> =
            remediation_tasks.iter().map(|t| t.id.clone()).collect();
        let mut revised_tasks: Vec<CandidateTask> = surviving_tasks
            .into_iter()
            .filter(|t| !remediation_ids.contains(&t.id))
            .collect();
        let mut affected_keys = Vec::new();
        for task in remediation_tasks {
            affected_keys.push(task.id.clone());
            revised_tasks.push(task);
        }

        // 5. Build revision record with trigger provenance.
        let trigger = crate::kernel::plan::ReplanningTrigger::TaskFailure {
            failed_task_id: req.failed_task_id.to_string(),
            reason: req.reason.clone(),
        };
        let mut revision_record = crate::kernel::plan::PlanRevisionRecord::new(
            next_revision,
            &parent_plan_id,
            &req.reason,
            trigger,
        );
        revision_record.affected_tasks = affected_keys;

        let plan = CandidatePlan::new(&new_plan_id, objective, revised_tasks)
            .with_revision(next_revision)
            .with_parent_plan_id(&parent_plan_id)
            .with_revision_record(revision_record);

        // 6. Validate the revised plan.
        let report = self.validator.validate(&plan).await;
        if !report.is_valid() {
            return Err(PlanError::ReplanFailed(format!(
                "Replan validation rejected: {:?}",
                report.errors
            )));
        }

        // 7. Render projections.
        self.write_projections_fail_safe(
            req.mission_id,
            u64::from(next_revision),
            &format!("Remediate {}", req.failed_task_id),
            &plan,
            &[],
            &[],
        );

        Ok(ReplanResponse {
            new_plan_id,
            modified_tasks: vec![req.failed_task_id],
            candidate_plan: Some(plan),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    /// Verifies that the `planning.decompose` prompt template aliases
    /// (`task_id`, `dependencies`, `assigned_role`, `acceptance_criteria`)
    /// parse correctly into the DTO — the runtime validates semantics, not spelling.
    #[test]
    fn test_parse_decomposition_accepts_prompt_vocabulary_aliases() {
        let text = r#"{
            "tasks": [{
                "task_id": "TASK-001",
                "title": "Project scaffolding and toolchain setup",
                "dependencies": [],
                "assigned_role": "implementer",
                "acceptance_criteria": ["cargo test passes"]
            }]
        }"#;
        let dto = parse_decomposition_json(text).expect("prompt-vocabulary JSON parses");
        assert_eq!(dto.tasks.len(), 1);
        assert_eq!(dto.tasks[0].id, "TASK-001");
        assert!(dto.tasks[0].depends_on.is_empty());
        assert_eq!(dto.tasks[0].role.as_deref(), Some("implementer"));
        assert_eq!(dto.tasks[0].completion_criteria, vec!["cargo test passes"]);
    }

    /// Verifies that a bare task array (no `{"tasks": …}` envelope) parses
    /// without inventing tasks.
    #[test]
    fn test_parse_decomposition_accepts_bare_task_array() {
        let text = r#"[
            {"task_id": "TASK-001", "title": "Scaffold", "dependencies": [], "assigned_role": "implementer", "acceptance_criteria": ["done"]},
            {"id": "TASK-002", "title": "Implement", "depends_on": ["TASK-001"]}
        ]"#;
        let dto = parse_decomposition_json(text).expect("bare array parses");
        assert_eq!(dto.tasks.len(), 2);
        assert_eq!(dto.tasks[1].depends_on, vec!["TASK-001".to_string()]);
    }

    /// Garbage still fails explicitly — alias tolerance never fabricates.
    /// An empty array parses to zero tasks; the service layer rejects
    /// zero-task plans for actionable objectives downstream.
    #[test]
    fn test_parse_decomposition_rejects_garbage() {
        assert!(parse_decomposition_json("not json {{{").is_err());
        let empty = parse_decomposition_json("[]").expect("empty array parses");
        assert!(empty.tasks.is_empty());
    }

    #[tokio::test]
    async fn test_plan_service_no_model_actionable_objective_fails_explicitly() {
        // Declarative authority boundary: without a model
        // provider the runtime refuses to fabricate tasks for actionable
        // objectives. Recovery path: configure a model or supply upstream
        // context, then replan.
        let dir = tempdir().unwrap();
        let service = PlanServiceImpl::new(dir.path());

        let mid = MissionId::new();
        assert!(!service.has_valid_plan(mid).await.unwrap());

        let req = PlanRequest::new(mid, "Build autonomous planning contracts");
        let err = service
            .generate_initial_plan(req)
            .await
            .expect_err("actionable objective without model must fail explicitly");
        let msg = err.to_string();
        assert!(
            msg.contains("No model provider") && msg.contains("refusing to fabricate"),
            "Unexpected error: {}",
            msg
        );
        assert!(!service.has_valid_plan(mid).await.unwrap());
    }

    #[tokio::test]
    async fn test_plan_service_no_model_read_only_yields_empty_plan() {
        // Read-only inquiries remain the one semantically safe generic
        // empty plan: nothing to execute, nothing fabricated.
        let dir = tempdir().unwrap();
        let service = PlanServiceImpl::new(dir.path());

        let mid = MissionId::new();
        let req = PlanRequest::new(mid, "Explain the repository module structure");
        assert!(is_read_only_objective(
            "Explain the repository module structure"
        ));

        let resp = service
            .generate_initial_plan(req)
            .await
            .expect("read-only plan");
        assert_eq!(resp.task_count, 0);
        assert!(service.has_valid_plan(mid).await.unwrap());

        let proj_dir = mission_projections_dir(dir.path(), mid);
        assert!(proj_dir.join("PLAN.md").is_file());
        let plan_md = fs::read_to_string(proj_dir.join("PLAN.md")).unwrap();
        assert!(plan_md.contains("Explain the repository module structure"));
    }

    #[tokio::test]
    async fn test_plan_service_replan() {
        let dir = tempdir().unwrap();
        let service = PlanServiceImpl::new(dir.path());

        let mid = MissionId::new();
        let failed_task_id = crate::ids::TaskId::new();

        let replan_req = ReplanRequest {
            mission_id: mid,
            failed_task_id,
            reason: "Compilation error: missing module".to_string(),
        };

        let resp = service.replan(replan_req).await.expect("replan");
        assert_eq!(resp.modified_tasks, vec![failed_task_id]);
        assert!(resp.new_plan_id.contains(&mid.to_string()));
    }

    #[test]
    fn test_classify_objective_table_driven() {
        let cases = [
            // RepositoryResearch
            (
                "Study the codebase",
                ObjectiveClassification::RepositoryResearch,
            ),
            (
                "Study the repository",
                ObjectiveClassification::RepositoryResearch,
            ),
            (
                "Analyze the codebase",
                ObjectiveClassification::RepositoryResearch,
            ),
            (
                "Analyze the architecture",
                ObjectiveClassification::RepositoryResearch,
            ),
            (
                "Inspect the repository",
                ObjectiveClassification::RepositoryResearch,
            ),
            (
                "Review the codebase",
                ObjectiveClassification::RepositoryResearch,
            ),
            (
                "Understand the project structure",
                ObjectiveClassification::RepositoryResearch,
            ),
            (
                "Investigate the runtime architecture",
                ObjectiveClassification::RepositoryResearch,
            ),
            (
                "Map the modules",
                ObjectiveClassification::RepositoryResearch,
            ),
            (
                "Explore the codebase",
                ObjectiveClassification::RepositoryResearch,
            ),
            // RepositoryAudit
            (
                "Audit the codebase for security issues",
                ObjectiveClassification::RepositoryAudit,
            ),
            (
                "Review the codebase for security issues",
                ObjectiveClassification::RepositoryAudit,
            ),
            (
                "Review the repository for reliability problems",
                ObjectiveClassification::RepositoryAudit,
            ),
            // DirectAnswer
            (
                "What does src/main.rs do?",
                ObjectiveClassification::DirectAnswer,
            ),
            ("What is Rust?", ObjectiveClassification::DirectAnswer),
            (
                "What does this function return?",
                ObjectiveClassification::DirectAnswer,
            ),
            (
                "Explain this error message.",
                ObjectiveClassification::DirectAnswer,
            ),
            (
                "Explain the repository module structure",
                ObjectiveClassification::DirectAnswer,
            ),
            // Implementation (mutating)
            ("Fix the parser", ObjectiveClassification::Implementation),
            (
                "Implement authentication",
                ObjectiveClassification::Implementation,
            ),
            ("Add logging", ObjectiveClassification::Implementation),
            (
                "Refactor the runtime",
                ObjectiveClassification::Implementation,
            ),
            (
                "Explain this function and fix the bug",
                ObjectiveClassification::Implementation,
            ),
            (
                "Analyze the parser and fix the bug",
                ObjectiveClassification::Implementation,
            ),
            (
                "Study the architecture and implement the feature",
                ObjectiveClassification::Implementation,
            ),
            // Verification
            (
                "Verify the test suite",
                ObjectiveClassification::Verification,
            ),
            (
                "Run tests for the engine",
                ObjectiveClassification::Verification,
            ),
            // Investigation
            (
                "Diagnose why the test fails",
                ObjectiveClassification::Investigation,
            ),
            (
                "Debug the panic in worker loop",
                ObjectiveClassification::Investigation,
            ),
        ];

        for (objective, expected) in cases {
            let actual = classify_objective(objective);
            assert_eq!(
                actual, expected,
                "Objective {:?} expected {:?}, got {:?}",
                objective, expected, actual
            );
        }
    }

    #[tokio::test]
    async fn test_study_codebase_produces_non_empty_researcher_plan() {
        let dir = tempdir().unwrap();
        // Model incorrectly returns zero tasks for "Study the codebase"
        let caller = Arc::new(crate::agent::model_policy::TestModelCaller::new(
            r#"{"tasks": []}"#,
        ));
        let service = PlanServiceImpl::new(dir.path()).with_model_caller(caller);

        let mid = MissionId::new();
        let req = PlanRequest::new(mid, "Study the codebase");

        let resp = service
            .generate_initial_plan(req)
            .await
            .expect("planning must succeed via deterministic research-task recovery");

        // Non-empty candidate plan
        assert_eq!(resp.task_count, 1);
        let task = &resp.candidate_plan.tasks[0];

        // Researcher role
        assert_eq!(task.role, AgentRole::researcher());

        // Read-only capabilities: fs.read, repo.read, NO fs.write
        let cap_ids: Vec<&str> = task.capabilities.iter().map(|c| c.id.as_str()).collect();
        assert!(cap_ids.contains(&"fs.read"), "must contain fs.read");
        assert!(cap_ids.contains(&"repo.read"), "must contain repo.read");
        assert!(
            !cap_ids.contains(&"fs.write"),
            "must NOT contain fs.write capability"
        );
        assert!(
            task.capabilities
                .iter()
                .all(|c| c.mode == CapabilityAccessMode::Read),
            "all capabilities must be Read mode"
        );

        // Verification strategy is read-only artifact inspection
        assert!(matches!(
            task.verification,
            VerificationStrategy::ArtifactInspection { .. }
        ));

        // Plan passes deterministic validation
        let report = service.validator.validate(&resp.candidate_plan).await;
        assert!(
            report.is_valid(),
            "Candidate plan must pass validation: {:?}",
            report.errors
        );
    }

    #[tokio::test]
    async fn test_security_audit_produces_read_only_envelope() {
        let dir = tempdir().unwrap();
        let caller = Arc::new(crate::agent::model_policy::TestModelCaller::new(
            r#"{"tasks": []}"#,
        ));
        let service = PlanServiceImpl::new(dir.path()).with_model_caller(caller);

        let mid = MissionId::new();
        let req = PlanRequest::new(mid, "Review the codebase for security issues");

        let resp = service
            .generate_initial_plan(req)
            .await
            .expect("audit planning succeeds");

        assert_eq!(resp.task_count, 1);
        let task = &resp.candidate_plan.tasks[0];
        assert_eq!(task.role, AgentRole::researcher());

        // Validate that all capabilities intersect cleanly with a read-only envelope
        let ro_envelope =
            crate::agent::envelope::CapabilityEnvelope::read_only(["fs.read", "repo.read"]);
        let eligible = crate::agent::envelope::calculate_eligible_capabilities(
            &task.capabilities,
            &ro_envelope,
        );
        assert!(
            eligible.is_ok(),
            "Task capabilities must pass read-only envelope check: {:?}",
            eligible
        );
    }

    #[tokio::test]
    async fn test_actionable_objective_empty_model_fails_explicitly() {
        let dir = tempdir().unwrap();
        // Model returns empty tasks for mutating objective -> MUST fail explicitly
        let caller = Arc::new(crate::agent::model_policy::TestModelCaller::new(
            r#"{"tasks": []}"#,
        ));
        let service = PlanServiceImpl::new(dir.path()).with_model_caller(caller);

        let mid = MissionId::new();
        let req = PlanRequest::new(mid, "Fix the parser");

        let err = service
            .generate_initial_plan(req)
            .await
            .expect_err("empty model response for actionable objective must fail");

        let msg = err.to_string();
        assert!(
            msg.contains("Model returned zero tasks for an actionable objective")
                && msg.contains("no fallback tasks substituted"),
            "Error must indicate failure without fallback fabrication: {}",
            msg
        );
    }

    #[tokio::test]
    async fn test_mixed_objective_mutation_takes_precedence_and_fails_on_empty() {
        let dir = tempdir().unwrap();
        let caller = Arc::new(crate::agent::model_policy::TestModelCaller::new(
            r#"{"tasks": []}"#,
        ));
        let service = PlanServiceImpl::new(dir.path()).with_model_caller(caller);

        let mid = MissionId::new();
        let req = PlanRequest::new(mid, "Explain this function and fix the bug");

        let err = service
            .generate_initial_plan(req)
            .await
            .expect_err("mutation objective must not be treated as informational");

        let msg = err.to_string();
        assert!(
            msg.contains("Model returned zero tasks for an actionable objective"),
            "Error must treat mixed objective as actionable: {}",
            msg
        );
    }

    #[tokio::test]
    async fn test_study_codebase_no_model_provider_recovers_research_task() {
        let dir = tempdir().unwrap();
        let service = PlanServiceImpl::new(dir.path());

        let mid = MissionId::new();
        let req = PlanRequest::new(mid, "Study the codebase");

        let resp = service
            .generate_initial_plan(req)
            .await
            .expect("offline planning for research synthesizes researcher task");

        assert_eq!(resp.task_count, 1);
        let task = &resp.candidate_plan.tasks[0];
        assert_eq!(task.role, AgentRole::researcher());
        assert!(
            task.capabilities
                .iter()
                .all(|c| c.id != "fs.write" && c.mode == CapabilityAccessMode::Read)
        );
    }
}
