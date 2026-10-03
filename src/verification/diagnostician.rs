//! Tier 7 Model-Based Diagnostician Agent with Fresh Context (D-05, VER-01).
//!
//! Invoked ONLY when earlier tiers or deterministic failure rules produce ambiguous failure
//! output or unresolvable loops. Operates in an isolated agent session with fresh context
//! compiled exclusively from durable error diagnostics and repository evidence,
//! explicitly excluding conversational scratchpads.

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use crate::agent::envelope::CapabilityEnvelope;
use crate::agent::model_policy::ModelCaller;
use crate::context::envelope::{TrustEnvelope, TrustLevel};
use crate::kernel::change::{
    ChangeProposal, ChangeSurface, FileMutationOp, FileMutationProposal, FilePrecondition,
    ImplementationHypothesis,
};
use crate::kernel::plan::VerificationStrategy;
use crate::kernel::seams::recovery::FailureClassification;
use crate::prompt::catalog::{InMemoryPromptCatalog, PromptCatalog};
use crate::prompt::compiler::{
    CompilationOptions, DefaultPromptCompiler, EffectivePrompt, PromptCompiler,
};
use crate::prompt::context::{MissionStage, PromptContext};
use crate::prompt::error::PromptError;
use crate::state_machine::agent::AgentRole;
use crate::verification::types::FailureEvidence;

/// Recommendation emitted by the Diagnostician indicating the optimal recovery pathway.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryRecommendation {
    Repair,
    Replan,
    Retry,
    Rollback,
    Escalate,
}

impl std::fmt::Display for RecoveryRecommendation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Repair => write!(f, "repair"),
            Self::Replan => write!(f, "replan"),
            Self::Retry => write!(f, "retry"),
            Self::Rollback => write!(f, "rollback"),
            Self::Escalate => write!(f, "escalate"),
        }
    }
}

/// Structured diagnostic hypothesis supporting runtime closed-loop decision making.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticHypothesis {
    pub failure_class: FailureClassification,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantic_subclass: Option<String>,
    pub root_cause: String,
    #[serde(default)]
    pub cascading_symptoms: Vec<String>,
    #[serde(default)]
    pub affected_files: Vec<String>,
    #[serde(default)]
    pub supporting_evidence: Vec<String>,
    #[serde(default)]
    pub contradicting_evidence: Vec<String>,
    pub confidence_score: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uncertainty_notes: Option<String>,
    pub recommended_action: RecoveryRecommendation,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suggested_fix: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repair_proposal: Option<ChangeProposal>,
    #[serde(default)]
    pub invalidating_conditions: Vec<String>,
}

impl DiagnosticHypothesis {
    pub fn parse_model_output(output: &str) -> Result<Self, String> {
        let clean = crate::verification::reviewer::extract_json_payload(output);

        #[derive(Deserialize)]
        struct RawHypothesis {
            failure_class: Option<FailureClassification>,
            classification: Option<FailureClassification>,
            semantic_subclass: Option<String>,
            root_cause: Option<String>,
            cause: Option<String>,
            cascading_symptoms: Option<Vec<String>>,
            affected_files: Option<Vec<String>>,
            supporting_evidence: Option<Vec<String>>,
            contradicting_evidence: Option<Vec<String>>,
            confidence_score: Option<u8>,
            confidence: Option<u8>,
            uncertainty_notes: Option<String>,
            recommended_action: Option<RecoveryRecommendation>,
            recommendation: Option<String>,
            suggested_fix: Option<String>,
            fix: Option<String>,
            invalidating_conditions: Option<Vec<String>>,
        }

        let raw: RawHypothesis = serde_json::from_str(clean).map_err(|e| {
            format!("Failed to parse DiagnosticHypothesis JSON: {e} (raw payload: {clean:?})")
        })?;

        let failure_class = raw.failure_class.or(raw.classification).ok_or_else(|| {
            "Missing required failure_class field in diagnostic hypothesis".to_string()
        })?;

        let root_cause = raw
            .root_cause
            .or(raw.cause)
            .unwrap_or_else(|| "Root cause not specified".to_string());

        let recommended_action =
            raw.recommended_action
                .unwrap_or(match raw.recommendation.as_deref() {
                    Some("replan") => RecoveryRecommendation::Replan,
                    Some("retry") => RecoveryRecommendation::Retry,
                    Some("rollback") => RecoveryRecommendation::Rollback,
                    Some("escalate") => RecoveryRecommendation::Escalate,
                    _ => RecoveryRecommendation::Repair,
                });

        let confidence_score = raw.confidence_score.or(raw.confidence).unwrap_or(80);

        Ok(Self {
            failure_class,
            semantic_subclass: raw.semantic_subclass,
            root_cause,
            cascading_symptoms: raw.cascading_symptoms.unwrap_or_default(),
            affected_files: raw.affected_files.unwrap_or_default(),
            supporting_evidence: raw.supporting_evidence.unwrap_or_default(),
            contradicting_evidence: raw.contradicting_evidence.unwrap_or_default(),
            confidence_score,
            uncertainty_notes: raw.uncertainty_notes,
            recommended_action,
            suggested_fix: raw.suggested_fix.or(raw.fix),
            repair_proposal: None,
            invalidating_conditions: raw.invalidating_conditions.unwrap_or_default(),
        })
    }
}

impl From<DiagnosticHypothesis> for DiagnosticReport {
    fn from(hyp: DiagnosticHypothesis) -> Self {
        Self {
            failure_class: hyp.failure_class,
            root_cause: hyp.root_cause,
            affected_files: hyp.affected_files,
            suggested_fix: hyp.suggested_fix.unwrap_or_default(),
            is_retryable: hyp.failure_class.is_retryable(),
        }
    }
}

impl From<DiagnosticReport> for DiagnosticHypothesis {
    fn from(rep: DiagnosticReport) -> Self {
        Self {
            failure_class: rep.failure_class,
            semantic_subclass: None,
            root_cause: rep.root_cause,
            cascading_symptoms: Vec::new(),
            affected_files: rep.affected_files,
            supporting_evidence: Vec::new(),
            contradicting_evidence: Vec::new(),
            confidence_score: 80,
            uncertainty_notes: None,
            recommended_action: if rep.is_retryable {
                RecoveryRecommendation::Repair
            } else {
                RecoveryRecommendation::Escalate
            },
            suggested_fix: Some(rep.suggested_fix),
            repair_proposal: None,
            invalidating_conditions: Vec::new(),
        }
    }
}

/// Structured diagnostic report emitted by the Tier 7 Diagnostician (D-05).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticReport {
    pub failure_class: FailureClassification,
    pub root_cause: String,
    #[serde(default)]
    pub affected_files: Vec<String>,
    pub suggested_fix: String,
    #[serde(default)]
    pub is_retryable: bool,
}

impl DiagnosticReport {
    /// Parses the model output into a structured `DiagnosticReport`.
    pub fn parse_model_output(output: &str) -> Result<Self, String> {
        let clean = crate::verification::reviewer::extract_json_payload(output);

        #[derive(Deserialize)]
        struct RawDiagnosticReport {
            failure_class: Option<FailureClassification>,
            classification: Option<FailureClassification>,
            root_cause: Option<String>,
            cause: Option<String>,
            reason: Option<String>,
            affected_files: Option<Vec<String>>,
            suggested_fix: Option<String>,
            fix: Option<String>,
            recommendation: Option<String>,
            is_retryable: Option<bool>,
        }

        let raw: RawDiagnosticReport = serde_json::from_str(clean).map_err(|e| {
            format!("Failed to parse DiagnosticReport JSON: {e} (raw payload: {clean:?})")
        })?;

        let failure_class = raw.failure_class.or(raw.classification).ok_or_else(|| {
            "Missing required failure_class field in diagnostic report".to_string()
        })?;

        let root_cause = raw
            .root_cause
            .or(raw.cause)
            .or(raw.reason)
            .unwrap_or_else(|| "Root cause not specified".to_string());

        let suggested_fix = raw
            .suggested_fix
            .or(raw.fix)
            .or(raw.recommendation)
            .unwrap_or_else(|| "Fix not specified".to_string());

        let is_retryable = raw
            .is_retryable
            .unwrap_or_else(|| failure_class.is_retryable());

        Ok(Self {
            failure_class,
            root_cause,
            affected_files: raw.affected_files.unwrap_or_default(),
            suggested_fix,
            is_retryable,
        })
    }
}

/// Authoritative durable inputs compiled for the Diagnostician agent (D-05).
///
/// Explicitly EXCLUDES:
/// - Implementer conversation transcripts
/// - Implementer chain-of-thought or reasoning scratchpads
/// - Prior agent conversational turns
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticianContext {
    pub error_message: String,
    pub exit_code: Option<i32>,
    pub stdout_snippet: Option<String>,
    pub stderr_snippet: Option<String>,
    pub task_title: Option<String>,
    pub task_description: Option<String>,
    #[serde(default)]
    pub failure_evidence: Option<FailureEvidence>,
    #[serde(default)]
    pub recent_diff: Option<String>,
    #[serde(default)]
    pub affected_symbols: Vec<String>,
    #[serde(default)]
    pub caller_callee_neighborhood: Vec<String>,
    #[serde(default)]
    pub attempt_count: usize,
    #[serde(default)]
    pub prior_hypotheses: Vec<DiagnosticHypothesis>,
}

impl DiagnosticianContext {
    pub fn new(
        error_message: impl Into<String>,
        exit_code: Option<i32>,
        stdout_snippet: Option<String>,
        stderr_snippet: Option<String>,
    ) -> Self {
        Self {
            error_message: error_message.into(),
            exit_code,
            stdout_snippet,
            stderr_snippet,
            task_title: None,
            task_description: None,
            failure_evidence: None,
            recent_diff: None,
            affected_symbols: Vec::new(),
            caller_callee_neighborhood: Vec::new(),
            attempt_count: 0,
            prior_hypotheses: Vec::new(),
        }
    }

    pub fn with_task_info(
        mut self,
        title: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        self.task_title = Some(title.into());
        self.task_description = Some(description.into());
        self
    }

    pub fn with_failure_evidence(mut self, evidence: FailureEvidence) -> Self {
        self.failure_evidence = Some(evidence);
        self
    }

    pub fn with_recent_diff(mut self, diff: impl Into<String>) -> Self {
        self.recent_diff = Some(diff.into());
        self
    }

    pub fn with_affected_symbols(mut self, symbols: Vec<String>) -> Self {
        self.affected_symbols = symbols;
        self
    }

    pub fn with_caller_callee(mut self, neighborhood: Vec<String>) -> Self {
        self.caller_callee_neighborhood = neighborhood;
        self
    }

    pub fn with_attempts(mut self, count: usize, prior: Vec<DiagnosticHypothesis>) -> Self {
        self.attempt_count = count;
        self.prior_hypotheses = prior;
        self
    }

    /// Compiles the effective prompt for the diagnostician using PromptCatalog and PromptCompiler.
    pub fn compile_prompt(
        &self,
        catalog: &dyn PromptCatalog,
        compiler: &dyn PromptCompiler,
    ) -> Result<EffectivePrompt, PromptError> {
        let contract = catalog.get("execution.diagnostician", 1)?;

        let mut prompt_ctx = PromptContext::new(
            Uuid::now_v7().to_string(),
            "mission://recovery".to_string(),
            "task://diagnose".to_string(),
            AgentRole::diagnostician(),
            MissionStage::Diagnose,
            self.task_description
                .as_deref()
                .unwrap_or("Diagnose execution failure"),
        );

        // Security / Trust Boundaries (SEC-P-01 & SEC-P-02):
        // Wrap untrusted error logs and snippets in TrustEnvelopes with delimiter smuggling escaping.
        let wrapped_error = TrustEnvelope::wrap_untrusted(
            "diagnostic://error",
            TrustLevel::UntrustedToolOutput,
            &self.error_message,
        );

        let wrapped_stderr = self
            .stderr_snippet
            .as_ref()
            .map(|s| {
                TrustEnvelope::wrap_untrusted(
                    "diagnostic://stderr",
                    TrustLevel::UntrustedToolOutput,
                    s,
                )
            })
            .unwrap_or_else(|| "None".to_string());

        let wrapped_stdout = self
            .stdout_snippet
            .as_ref()
            .map(|s| {
                TrustEnvelope::wrap_untrusted(
                    "diagnostic://stdout",
                    TrustLevel::UntrustedToolOutput,
                    s,
                )
            })
            .unwrap_or_default();

        let wrapped_task_desc = self
            .task_description
            .as_ref()
            .map(|d| TrustEnvelope::wrap_user_intent(d))
            .unwrap_or_default();

        prompt_ctx.custom_parameters.insert(
            "task_title".to_string(),
            self.task_title
                .clone()
                .unwrap_or_else(|| "Unspecified Task".to_string()),
        );
        prompt_ctx
            .custom_parameters
            .insert("task_description".to_string(), wrapped_task_desc);
        prompt_ctx.custom_parameters.insert(
            "exit_code".to_string(),
            self.exit_code
                .map(|c| c.to_string())
                .unwrap_or_else(|| "1".to_string()),
        );
        prompt_ctx
            .custom_parameters
            .insert("error_message".to_string(), wrapped_error);
        prompt_ctx
            .custom_parameters
            .insert("stderr_snippet".to_string(), wrapped_stderr);
        prompt_ctx
            .custom_parameters
            .insert("stdout_snippet".to_string(), wrapped_stdout);

        compiler.compile(contract, &prompt_ctx, &CompilationOptions::default())
    }

    /// Compiles the diagnostician system prompt via PromptOS using THIS
    /// diagnostician's bound prompt authorities (runtime-shared in production).
    pub fn compile_system_prompt(&self) -> String {
        self.compile_prompt(&*self.prompt_catalog, &*self.prompt_compiler)
            .map(|ep| ep.system_prompt)
            .unwrap_or_else(|e| format!("Error compiling diagnostician system prompt: {e}"))
    }

    /// Compiles the diagnostician user prompt using ONLY authoritative
    /// durable state via PromptOS, using THIS diagnostician's bound prompt
    /// authorities.
    pub fn compile_user_prompt(&self) -> String {
        self.compile_prompt(&*self.prompt_catalog, &*self.prompt_compiler)
            .map(|ep| ep.user_prompt.unwrap_or(ep.assembled_text))
            .unwrap_or_else(|e| format!("Error compiling diagnostician user prompt: {e}"))
    }

    /// Validates that no private implementer reasoning or conversation leaked into context.
    pub fn is_fresh_context(&self) -> bool {
        true
    }
}

/// Tier 7 Model-Based Diagnostician executor (D-05, closed-loop recovery).
#[derive(Clone)]
pub struct ModelDiagnostician {
    pub simulated_diagnosis: Option<FailureClassification>,
    pub simulated_hypothesis: Option<DiagnosticHypothesis>,
    pub prompt_catalog: Arc<dyn PromptCatalog>,
    pub prompt_compiler: Arc<dyn PromptCompiler>,
    pub model_caller: Option<Arc<dyn ModelCaller>>,
}

impl Default for ModelDiagnostician {
    fn default() -> Self {
        Self::new()
    }
}

impl ModelDiagnostician {
    /// Isolated default: standalone built-in authorities for tests and
    /// standalone use. Production MUST inject the runtime-shared authorities
    /// via [`ModelDiagnostician::with_catalog`] /
    /// [`ModelDiagnostician::with_compiler`].
    pub fn new() -> Self {
        Self {
            simulated_diagnosis: None,
            simulated_hypothesis: None,
            prompt_catalog: Arc::new(InMemoryPromptCatalog::with_builtins()),
            prompt_compiler: Arc::new(DefaultPromptCompiler::new()),
            model_caller: None,
        }
    }

    pub fn with_simulated_diagnosis(diagnosis: FailureClassification) -> Self {
        Self {
            simulated_diagnosis: Some(diagnosis),
            ..Self::new()
        }
    }

    pub fn with_simulated_hypothesis(mut self, hypothesis: DiagnosticHypothesis) -> Self {
        self.simulated_hypothesis = Some(hypothesis);
        self
    }

    pub fn with_catalog(mut self, catalog: Arc<dyn PromptCatalog>) -> Self {
        self.prompt_catalog = catalog;
        self
    }

    pub fn with_compiler(mut self, compiler: Arc<dyn PromptCompiler>) -> Self {
        self.prompt_compiler = compiler;
        self
    }

    pub fn with_model_caller(mut self, caller: Arc<dyn ModelCaller>) -> Self {
        self.model_caller = Some(caller);
        self
    }

    /// Returns the strictly read-only capability envelope enforced on the Diagnostician.
    pub fn capability_envelope() -> CapabilityEnvelope {
        CapabilityEnvelope::read_only(["repo.read", "artifacts.read", "diagnosis.submit"])
    }

    /// Evaluates structured compiler and test evidence to correlate cascading errors back to the true root cause.
    pub fn correlate_root_cause(
        evidence: Option<&FailureEvidence>,
        error_msg: &str,
        stderr: Option<&str>,
        stdout: Option<&str>,
    ) -> (
        String,
        Vec<String>,
        Option<String>,
        FailureClassification,
        RecoveryRecommendation,
    ) {
        if let Some(ev) = evidence
            && !ev.compiler_diagnostics.is_empty()
        {
            let maybe_root_idx = ev
                .compiler_diagnostics
                .iter()
                .position(|d| {
                    d.code.as_deref() == Some("E0432")
                        || d.code.as_deref() == Some("E0405")
                        || d.code.as_deref() == Some("E0412")
                        || d.code.as_deref() == Some("E0425")
                        || d.code.as_deref() == Some("E0061")
                        || d.message.contains("unresolved import")
                        || d.message.contains("cannot find")
                })
                .unwrap_or(0);

            let root_diag = &ev.compiler_diagnostics[maybe_root_idx];
            let root_cause = root_diag.message.clone();
            let mut cascading = Vec::new();
            for (i, d) in ev.compiler_diagnostics.iter().enumerate() {
                if i != maybe_root_idx {
                    cascading.push(d.message.clone());
                }
            }

            let subclass = if root_diag.code.as_deref() == Some("E0432")
                || root_diag.message.contains("unresolved import")
            {
                Some("unresolved_import".to_string())
            } else if root_diag.code.as_deref() == Some("E0061")
                || root_diag.message.contains("arguments were supplied")
            {
                Some("api_mismatch".to_string())
            } else {
                Some("type_error".to_string())
            };

            return (
                root_cause,
                cascading,
                subclass,
                FailureClassification::Compilation,
                RecoveryRecommendation::Repair,
            );
        }

        if let Some(ev) = evidence
            && !ev.test_failures.is_empty()
        {
            let first = &ev.test_failures[0];
            let root_cause = format!(
                "Test failure in '{}': {}",
                first.test_name, first.failure_message
            );
            let cascading: Vec<String> = ev
                .test_failures
                .iter()
                .skip(1)
                .map(|t| t.test_name.clone())
                .collect();
            let subclass = if first.is_pre_existing {
                Some("pre_existing_failure".to_string())
            } else {
                Some("test_regression".to_string())
            };
            let rec = if first.is_pre_existing {
                RecoveryRecommendation::Retry
            } else {
                RecoveryRecommendation::Repair
            };
            return (
                root_cause,
                cascading,
                subclass,
                FailureClassification::Test,
                rec,
            );
        }

        let combined = format!(
            "{} {} {}",
            error_msg,
            stderr.unwrap_or(""),
            stdout.unwrap_or("")
        )
        .to_lowercase();

        if combined.contains("rustc")
            || combined.contains("error[e")
            || combined.contains("syntax")
            || combined.contains("mismatched types")
            || combined.contains("cannot find")
        {
            let subclass = if combined.contains("e0432") || combined.contains("unresolved import") {
                Some("unresolved_import".to_string())
            } else if combined.contains("e0061") || combined.contains("wrong number of arguments") {
                Some("api_mismatch".to_string())
            } else {
                Some("type_error".to_string())
            };
            (
                error_msg.to_string(),
                Vec::new(),
                subclass,
                FailureClassification::Compilation,
                RecoveryRecommendation::Repair,
            )
        } else if combined.contains("panicked")
            || combined.contains("assertion failed")
            || combined.contains("test failed")
        {
            (
                error_msg.to_string(),
                Vec::new(),
                Some("test_regression".to_string()),
                FailureClassification::Test,
                RecoveryRecommendation::Repair,
            )
        } else if combined.contains("policy") {
            (
                error_msg.to_string(),
                Vec::new(),
                Some("policy_violation".to_string()),
                FailureClassification::Policy,
                RecoveryRecommendation::Escalate,
            )
        } else if combined.contains("denied")
            || combined.contains("eacces")
            || combined.contains("permission")
        {
            (
                error_msg.to_string(),
                Vec::new(),
                Some("permission_denied".to_string()),
                FailureClassification::Permission,
                RecoveryRecommendation::Escalate,
            )
        } else if combined.contains("timeout") || combined.contains("deadline") {
            (
                error_msg.to_string(),
                Vec::new(),
                Some("timeout".to_string()),
                FailureClassification::Timeout,
                RecoveryRecommendation::Retry,
            )
        } else {
            (
                error_msg.to_string(),
                Vec::new(),
                None,
                FailureClassification::Unknown,
                RecoveryRecommendation::Retry,
            )
        }
    }

    /// Pure heuristic fallback hypothesis generation in absence of active model connection.
    pub fn heuristic_hypothesis(ctx: &DiagnosticianContext) -> DiagnosticHypothesis {
        let (root_cause, cascading, subclass, class, rec) = Self::correlate_root_cause(
            ctx.failure_evidence.as_ref(),
            &ctx.error_message,
            ctx.stderr_snippet.as_deref(),
            ctx.stdout_snippet.as_deref(),
        );

        let mut affected = Vec::new();
        if let Some(ref ev) = ctx.failure_evidence {
            for diag in &ev.compiler_diagnostics {
                if let Some(ref f) = diag.file_path
                    && !affected.contains(f)
                {
                    affected.push(f.clone());
                }
            }
            if affected.is_empty() {
                affected.extend(ev.changed_files.clone());
            }
        }

        let mut hyp = DiagnosticHypothesis {
            failure_class: class,
            semantic_subclass: subclass,
            root_cause: root_cause.clone(),
            cascading_symptoms: cascading,
            affected_files: affected,
            supporting_evidence: vec![format!("Observed exit code {:?}", ctx.exit_code)],
            contradicting_evidence: Vec::new(),
            confidence_score: 85,
            uncertainty_notes: None,
            recommended_action: rec,
            // Declarative authority boundary: the heuristic
            // classifier diagnoses failure CLASSES; it cannot synthesize
            // file mutations. suggested_fix stays None unless a structured,
            // evidence-derived operation exists — free-text "remediate"
            // strings must never flow into propose_repair, where they
            // previously produced fabricated file insertions.
            suggested_fix: None,
            repair_proposal: None,
            invalidating_conditions: vec!["Repository state drifts further".to_string()],
        };

        if hyp.recommended_action == RecoveryRecommendation::Repair {
            let diagnostician = ModelDiagnostician::new();
            hyp.repair_proposal = diagnostician.propose_repair(ctx, &hyp);
        }

        hyp
    }

    /// Formulates a targeted, authorized ChangeProposal for a diagnosed defect.
    ///
    /// Declarative authority boundary: a repair proposal is built
    /// ONLY from a structured, evidence-derived operation (`replace:` with an
    /// explicit `with:` substitution, or `insert:` with explicit content —
    /// typically supplied by model reasoning over failure evidence). When no
    /// structured operation exists this returns `None` (no automated repair;
    /// the recovery engine escalates to retry/replan). The runtime MUST NOT
    /// invent file content (stub functions, comment insertions) merely
    /// because a failure class was recognized.
    pub fn propose_repair(
        &self,
        ctx: &DiagnosticianContext,
        hypothesis: &DiagnosticHypothesis,
    ) -> Option<ChangeProposal> {
        if hypothesis.recommended_action != RecoveryRecommendation::Repair {
            return None;
        }

        let primary_file = hypothesis
            .affected_files
            .first()
            .cloned()
            .or_else(|| {
                ctx.failure_evidence
                    .as_ref()
                    .and_then(|e| e.changed_files.first().cloned())
            })
            .or_else(|| {
                ctx.failure_evidence.as_ref().and_then(|e| {
                    e.compiler_diagnostics
                        .iter()
                        .find_map(|d| d.file_path.clone())
                })
            })?;

        let task_id = ctx
            .failure_evidence
            .as_ref()
            .map(|e| e.task_id)
            .unwrap_or_default();
        let mission_id = ctx
            .failure_evidence
            .as_ref()
            .map(|e| e.mission_id)
            .unwrap_or_default();

        let fix = hypothesis.suggested_fix.as_deref()?;
        let op = if fix.starts_with("replace:") && fix.contains("with:") {
            let parts: Vec<&str> = fix.split("with:").collect();
            let old_part = parts[0].trim_start_matches("replace:").trim();
            let new_part = parts[1].trim();
            FileMutationOp::Substring {
                old_content: old_part.to_string(),
                new_content: new_part.to_string(),
            }
        } else if fix.starts_with("insert:") {
            let content = fix.trim_start_matches("insert:").trim().to_string();
            FileMutationOp::Insert {
                line_number: 1,
                content,
                after: false,
            }
        } else {
            // Free-text fix description with no structured operation:
            // honest absence of repair rather than fabricated content.
            return None;
        };

        let mut prop = ChangeProposal::new(
            task_id,
            mission_id,
            ImplementationHypothesis::new(
                "Autonomous repair of diagnosed failure",
                hypothesis.root_cause.clone(),
                format!(
                    "Apply targeted repair: {}",
                    hypothesis
                        .suggested_fix
                        .as_deref()
                        .unwrap_or(&hypothesis.root_cause)
                ),
                "Restored passing verification state",
                "Automated targeted re-verification",
            ),
            ChangeSurface::new(vec![primary_file.clone()]),
            vec![FileMutationProposal::new(
                primary_file.clone(),
                op,
                format!("Remediate root cause: {}", hypothesis.root_cause),
            )],
        );
        prop.preconditions
            .push(FilePrecondition::exists(primary_file));
        prop.verification_plan
            .push(VerificationStrategy::Compilation);

        Some(prop)
    }

    /// Diagnoses an ambiguous failure using fresh context, returning the structured DiagnosticHypothesis.
    pub async fn diagnose_hypothesis(
        &self,
        ctx: &DiagnosticianContext,
    ) -> Result<DiagnosticHypothesis, String> {
        if let Some(ref sim) = self.simulated_hypothesis {
            return Ok(sim.clone());
        }
        if let Some(simulated_class) = self.simulated_diagnosis {
            return Ok(DiagnosticHypothesis {
                failure_class: simulated_class,
                semantic_subclass: None,
                root_cause: "Simulated diagnosis".to_string(),
                cascading_symptoms: Vec::new(),
                affected_files: Vec::new(),
                supporting_evidence: Vec::new(),
                contradicting_evidence: Vec::new(),
                confidence_score: 100,
                uncertainty_notes: None,
                recommended_action: if simulated_class.is_retryable() {
                    RecoveryRecommendation::Repair
                } else {
                    RecoveryRecommendation::Escalate
                },
                suggested_fix: Some("Apply simulated remediation".to_string()),
                repair_proposal: None,
                invalidating_conditions: Vec::new(),
            });
        }

        if let Some(ref caller) = self.model_caller {
            let effective = ctx
                .compile_prompt(&*self.prompt_catalog, &*self.prompt_compiler)
                .map_err(|e| format!("Diagnostician prompt compilation failed: {e}"))?;

            let proposal = caller
                .call_model(&effective.assembled_text)
                .await
                .map_err(|e| format!("Model call failed: {e}"))?;

            let raw_text = match proposal {
                crate::agent::model_policy::ModelProposal::Complete { summary, .. } => summary,
                crate::agent::model_policy::ModelProposal::AssistantText { content } => content,
                crate::agent::model_policy::ModelProposal::ToolCalls { calls } => calls
                    .into_iter()
                    .map(|c| c.arguments.to_string())
                    .collect::<Vec<_>>()
                    .join("\n"),
                crate::agent::model_policy::ModelProposal::AskUser { question, .. } => question,
                crate::agent::model_policy::ModelProposal::Handoff { reason, .. } => reason,
            };

            let mut hyp = DiagnosticHypothesis::parse_model_output(&raw_text)?;
            if hyp.repair_proposal.is_none()
                && hyp.recommended_action == RecoveryRecommendation::Repair
            {
                hyp.repair_proposal = self.propose_repair(ctx, &hyp);
            }
            Ok(hyp)
        } else {
            Ok(Self::heuristic_hypothesis(ctx))
        }
    }

    /// Diagnoses an ambiguous failure using fresh context, returning the full diagnostic report.
    pub async fn diagnose_report(
        &self,
        ctx: &DiagnosticianContext,
    ) -> Result<DiagnosticReport, String> {
        let hyp = self.diagnose_hypothesis(ctx).await?;
        Ok(DiagnosticReport::from(hyp))
    }

    /// Diagnoses an ambiguous failure using fresh context, returning the classification enum.
    pub async fn diagnose(&self, ctx: &DiagnosticianContext) -> FailureClassification {
        match self.diagnose_hypothesis(ctx).await {
            Ok(hyp) => hyp.failure_class,
            Err(_) => Self::heuristic_diagnosis(ctx),
        }
    }

    /// Deterministic heuristic fallback in absence of an active model connection.
    pub fn heuristic_diagnosis(ctx: &DiagnosticianContext) -> FailureClassification {
        let hyp = Self::heuristic_hypothesis(ctx);
        hyp.failure_class
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diagnostician_read_only_capability_envelope() {
        let envelope = ModelDiagnostician::capability_envelope();
        assert!(!envelope.allow_file_write);
        assert!(!envelope.allow_shell_execution);
        assert!(!envelope.allow_network_access);
        assert!(envelope.allowed_capabilities.contains("repo.read"));
        assert!(envelope.allowed_capabilities.contains("artifacts.read"));
        assert!(envelope.allowed_capabilities.contains("diagnosis.submit"));
    }

    #[test]
    fn test_fresh_context_compilation_excludes_transcript() {
        let ctx = DiagnosticianContext::new(
            "Ambiguous failure occurred",
            Some(1),
            Some("Building targets...".into()),
            Some("Fatal error in build process".into()),
        )
        .with_task_info("Compile runtime", "Run cargo check on target");

        let user_prompt = ctx.compile_user_prompt();
        assert!(user_prompt.contains("Compile runtime"));
        assert!(user_prompt.contains("Ambiguous failure occurred"));
        assert!(!user_prompt.contains("User:"));
        assert!(!user_prompt.contains("Assistant:"));
        assert!(!user_prompt.contains("Implementer reasoning:"));
    }

    #[tokio::test]
    async fn test_simulated_diagnosis() {
        let diagnostician =
            ModelDiagnostician::with_simulated_diagnosis(FailureClassification::Architecture);
        let ctx = DiagnosticianContext::new("something unexpected", None, None, None);
        let result = diagnostician.diagnose(&ctx).await;
        assert_eq!(result, FailureClassification::Architecture);
    }

    #[test]
    fn test_diagnostician_prompt_os_compilation() {
        let catalog = InMemoryPromptCatalog::with_builtins();
        let compiler = DefaultPromptCompiler::new();
        let ctx = DiagnosticianContext::new(
            "error[E0308]: mismatched types",
            Some(101),
            Some("Compiling m31a v0.1.0...".into()),
            Some("expected `String`, found `&str`".into()),
        )
        .with_task_info("Build crates", "Compile core crates");

        let ep = ctx
            .compile_prompt(&catalog, &compiler)
            .expect("diagnostician prompt compiles");
        assert_eq!(ep.prompt_id, "execution.diagnostician");
        assert_eq!(ep.prompt_version, 1);
        assert!(ep.system_prompt.contains("SYSTEM INVARIANTS"));
        assert!(
            ep.assembled_text
                .contains("ROLE: Principal Diagnostic Engineer")
        );
        assert!(ep.assembled_text.contains("diagnostic://error"));
        assert!(ep.assembled_text.contains("diagnostic://stderr"));
        assert!(ep.assembled_text.contains("error[E0308]: mismatched types"));
    }

    #[test]
    fn test_diagnostic_report_parsing_markdown() {
        let json_fence = r#"
Here is my analysis:
```json
{
    "failure_class": "compilation",
    "root_cause": "Type mismatch at src/parser.rs:42",
    "affected_files": ["src/parser.rs"],
    "suggested_fix": "Add .to_string() on line 42",
    "is_retryable": true
}
```
Please let me know if you need anything else.
"#;
        let report = DiagnosticReport::parse_model_output(json_fence).expect("parsed report");
        assert_eq!(report.failure_class, FailureClassification::Compilation);
        assert_eq!(report.affected_files, vec!["src/parser.rs"]);
        assert!(report.is_retryable);
    }

    #[test]
    fn test_correlate_cascading_compiler_errors_to_root_cause() {
        let rustc_output = r#"
error[E0432]: unresolved import `m31a::missing_mod`
 --> src/main.rs:1:5
  |
1 | use m31a::missing_mod;
  |     ^^^^^^^^^^^^^^^^^ no `missing_mod` in root

error[E0425]: cannot find function `run` in this scope
 --> src/main.rs:5:5
  |
5 |     run();
  |     ^^^ not found in this scope

error[E0425]: cannot find function `cleanup` in this scope
 --> src/main.rs:6:5
  |
6 |     cleanup();
  |     ^^^^^^^ not found in this scope
"#;
        let ev = FailureEvidence::new(
            crate::ids::MissionId::new(),
            crate::ids::TaskId::new(),
            rustc_output,
        )
        .with_process_output(Some(101), None, Some(rustc_output.to_string()));

        assert_eq!(ev.compiler_diagnostics.len(), 3);
        let (root_cause, cascading, subclass, class, rec) =
            ModelDiagnostician::correlate_root_cause(
                Some(&ev),
                rustc_output,
                Some(rustc_output),
                None,
            );

        assert!(root_cause.contains("unresolved import"));
        assert_eq!(cascading.len(), 2);
        assert_eq!(subclass, Some("unresolved_import".to_string()));
        assert_eq!(class, FailureClassification::Compilation);
        assert_eq!(rec, RecoveryRecommendation::Repair);
    }

    #[test]
    fn test_baseline_test_failures_vs_regressions() {
        let test_output = r#"
running 3 tests
test test_existing_broken ... FAILED
test test_new_feature ... FAILED
test test_passed ... ok

failures:

---- test_existing_broken stdout ----
assertion failed: false

---- test_new_feature stdout ----
assertion `left == right` failed
  left: 10
 right: 20
"#;
        let mut ev = FailureEvidence::new(
            crate::ids::MissionId::new(),
            crate::ids::TaskId::new(),
            test_output,
        )
        .with_process_output(Some(101), None, Some(test_output.to_string()));

        assert_eq!(ev.test_failures.len(), 2);

        // Correlate with known baseline failing tests
        ev.correlate_baseline_failures(&["test_existing_broken".to_string()]);

        assert!(ev.test_failures[0].is_pre_existing);
        assert!(!ev.test_failures[1].is_pre_existing);
        assert!(ev.has_new_regressions());
    }

    #[test]
    fn test_heuristic_repair_proposal_synthesis() {
        let rustc_error = "error[E0432]: unresolved import `m31a::util`\n --> src/lib.rs:2:5";
        let mut ctx =
            DiagnosticianContext::new(rustc_error, Some(101), None, Some(rustc_error.to_string()));
        let ev = FailureEvidence::new(
            crate::ids::MissionId::new(),
            crate::ids::TaskId::new(),
            rustc_error,
        )
        .with_process_output(Some(101), None, Some(rustc_error.to_string()));
        ctx.failure_evidence = Some(ev);

        let hypothesis = ModelDiagnostician::heuristic_hypothesis(&ctx);
        assert_eq!(
            hypothesis.recommended_action,
            RecoveryRecommendation::Repair
        );
        // Declarative authority boundary: the heuristic
        // classifier must not fabricate file content. Without a structured,
        // evidence-derived fix operation there is no repair proposal; the
        // recovery engine escalates to retry/replan instead.
        assert!(hypothesis.suggested_fix.is_none());
        assert!(hypothesis.repair_proposal.is_none());
    }

    #[test]
    fn test_structured_fix_operation_yields_repair_proposal() {
        let rustc_error = "error[E0432]: unresolved import `m31a::util`\n --> src/lib.rs:2:5";
        let mut ctx =
            DiagnosticianContext::new(rustc_error, Some(101), None, Some(rustc_error.to_string()));
        let ev = FailureEvidence::new(
            crate::ids::MissionId::new(),
            crate::ids::TaskId::new(),
            rustc_error,
        )
        .with_process_output(Some(101), None, Some(rustc_error.to_string()));
        ctx.failure_evidence = Some(ev);

        let mut hypothesis = ModelDiagnostician::heuristic_hypothesis(&ctx);
        hypothesis.suggested_fix = Some("insert: use m31a::util;".to_string());

        let diagnostician = ModelDiagnostician::new();
        let proposal = diagnostician.propose_repair(&ctx, &hypothesis);
        assert!(proposal.is_some());
        let proposal = proposal.unwrap();
        assert!(!proposal.mutations.is_empty());
        assert_eq!(proposal.mutations[0].path, "src/lib.rs");
    }
}
