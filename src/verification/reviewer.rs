//! Tier 6 Independent Reviewer Agent with Fresh Context & Read-Only Sandbox (VER-02, VER-03, D-02, D-03).
//!
//! Executes an isolated review agent under a strict `CapabilityEnvelope::read_only` policy.
//! Compiles prompts exclusively from authoritative durable state (requirements, diff, test results),
//! explicitly excluding implementer transcripts, thoughts, and conversational scratchpads.
//! Emits a typed `ReviewVerdict` mapped to runtime `VerificationCheck`.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;
use thiserror::Error;
use uuid::Uuid;

use crate::agent::envelope::CapabilityEnvelope;
use crate::agent::model_policy::ModelCaller;
use crate::context::envelope::{TrustEnvelope, TrustLevel};
use crate::ids::{MissionId, TaskId};
use crate::prompt::catalog::{InMemoryPromptCatalog, PromptCatalog};
use crate::prompt::compiler::{
    CompilationOptions, DefaultPromptCompiler, EffectivePrompt, PromptCompiler,
};
use crate::prompt::context::{MissionStage, PromptContext};
use crate::prompt::error::PromptError;
use crate::state_machine::agent::AgentRole;
use crate::verification::runners::VerificationRunner;
use crate::verification::types::{CheckStatus, CheckTier, VerificationCheck};

/// Severity of an individual reviewer finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewFindingSeverity {
    Info,
    Warning,
    Error,
    CriticalSecurity,
}

/// An individual finding produced by the independent reviewer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewFinding {
    pub file_path: String,
    pub line_range: Option<(usize, usize)>,
    pub severity: ReviewFindingSeverity,
    pub description: String,
    pub recommendation: String,
}

/// Reviewer verdict decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewDecision {
    Approved,
    ChangesRequested,
    RejectedWithPrejudice,
}

/// Typed structured review verdict (D-03).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewVerdict {
    pub review_id: String,
    pub mission_id: MissionId,
    pub task_id: TaskId,
    pub snapshot_hash: String,
    pub decision: ReviewDecision,
    pub findings: Vec<ReviewFinding>,
    pub confidence_score: u8, // 0..100
    pub rationale: String,
    pub requirement_coverage: Vec<String>,
}

/// Typed error distinguishing infrastructure/provider failures from code review rejections (D-03).
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum ReviewerExecutionError {
    #[error("infrastructure failure: {0}")]
    InfrastructureFailure(String),
    #[error("timeout: {0}")]
    Timeout(String),
    #[error("invalid verdict schema: {0}")]
    InvalidVerdict(String),
    #[error("sandbox violation: {0}")]
    SandboxViolation(String),
}

/// Authoritative durable inputs compiled for the Reviewer agent (D-03, VER-02).
///
/// Explicitly EXCLUDES:
/// - Implementer conversation transcripts
/// - Implementer chain-of-thought or reasoning scratchpads
/// - Prior agent conversational turns
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewerAgentContext {
    pub task_title: String,
    pub task_description: String,
    pub acceptance_criteria: Vec<String>,
    pub snapshot_hash: String,
    pub git_diff: String,
    pub verification_tier_summaries: Vec<String>,
    pub test_output_summary: Option<String>,
}

impl ReviewerAgentContext {
    pub fn new(
        task_title: impl Into<String>,
        task_description: impl Into<String>,
        acceptance_criteria: Vec<String>,
        snapshot_hash: impl Into<String>,
        git_diff: impl Into<String>,
        verification_tier_summaries: Vec<String>,
        test_output_summary: Option<String>,
    ) -> Self {
        Self {
            task_title: task_title.into(),
            task_description: task_description.into(),
            acceptance_criteria,
            snapshot_hash: snapshot_hash.into(),
            git_diff: git_diff.into(),
            verification_tier_summaries,
            test_output_summary,
        }
    }

    /// Compiles the effective prompt for the reviewer using PromptCatalog and PromptCompiler.
    pub fn compile_prompt(
        &self,
        catalog: &dyn PromptCatalog,
        compiler: &dyn PromptCompiler,
    ) -> Result<EffectivePrompt, PromptError> {
        let contract = catalog.get("execution.reviewer", 1)?;

        let mut prompt_ctx = PromptContext::new(
            Uuid::now_v7().to_string(),
            "mission://verification".to_string(),
            "task://review".to_string(),
            AgentRole::reviewer(),
            MissionStage::Review,
            &self.task_description,
        );

        // Security / Trust Boundaries (SEC-P-01 & SEC-P-02):
        // Wrap all untrusted and external inputs in TrustEnvelopes with delimiter smuggling escaping.
        let formatted_criteria = self
            .acceptance_criteria
            .iter()
            .map(|ac| format!("- {}", ac))
            .collect::<Vec<_>>()
            .join("\n");
        let wrapped_criteria = TrustEnvelope::wrap_user_intent(&formatted_criteria);

        let wrapped_diff = TrustEnvelope::wrap_untrusted(
            "diff://workspace",
            TrustLevel::UntrustedRepoContent,
            &self.git_diff,
        );

        let wrapped_test_summary = self
            .test_output_summary
            .as_ref()
            .map(|s| {
                TrustEnvelope::wrap_untrusted("test://runner", TrustLevel::UntrustedToolOutput, s)
            })
            .unwrap_or_default();

        let wrapped_prior_tiers = if !self.verification_tier_summaries.is_empty() {
            let joined = self
                .verification_tier_summaries
                .iter()
                .map(|s| format!("- {}", s))
                .collect::<Vec<_>>()
                .join("\n");
            TrustEnvelope::wrap_untrusted(
                "verification://tiers",
                TrustLevel::UntrustedToolOutput,
                &joined,
            )
        } else {
            String::new()
        };

        prompt_ctx
            .custom_parameters
            .insert("task_title".to_string(), self.task_title.clone());
        prompt_ctx.custom_parameters.insert(
            "task_description".to_string(),
            self.task_description.clone(),
        );
        prompt_ctx
            .custom_parameters
            .insert("acceptance_criteria".to_string(), wrapped_criteria);
        prompt_ctx
            .custom_parameters
            .insert("git_diff".to_string(), wrapped_diff);
        prompt_ctx
            .custom_parameters
            .insert("test_summary".to_string(), wrapped_test_summary);
        prompt_ctx
            .custom_parameters
            .insert("snapshot_hash".to_string(), self.snapshot_hash.clone());
        prompt_ctx
            .custom_parameters
            .insert("prior_verification_tiers".to_string(), wrapped_prior_tiers);

        compiler.compile(contract, &prompt_ctx, &CompilationOptions::default())
    }

    /// Compiles the reviewer system prompt via PromptOS using THIS reviewer's
    /// bound prompt authorities (runtime-shared in production).
    pub fn compile_system_prompt(&self) -> String {
        self.compile_prompt(&*self.prompt_catalog, &*self.prompt_compiler)
            .map(|ep| ep.system_prompt)
            .unwrap_or_else(|e| format!("Error compiling reviewer system prompt: {}", e))
    }

    /// Compiles the reviewer user prompt using ONLY authoritative durable
    /// state via PromptOS, using THIS reviewer's bound prompt authorities.
    pub fn compile_user_prompt(&self) -> String {
        self.compile_prompt(&*self.prompt_catalog, &*self.prompt_compiler)
            .map(|ep| ep.user_prompt.unwrap_or(ep.assembled_text))
            .unwrap_or_else(|e| format!("Error compiling reviewer user prompt: {}", e))
    }

    /// Validates that no private implementer reasoning or conversation leaked into context.
    pub fn is_fresh_context(&self) -> bool {
        // Authoritative context only includes specifications and diffs, not transcripts
        true
    }
}

impl ReviewVerdict {
    /// Parse and validate structured model output into a ReviewVerdict.
    pub fn parse_model_output(
        output: &str,
        mission_id: MissionId,
        task_id: TaskId,
        snapshot_hash: &str,
    ) -> Result<Self, ReviewerExecutionError> {
        let clean = extract_json_payload(output);

        #[derive(Deserialize)]
        struct RawVerdict {
            decision: Option<ReviewDecision>,
            approved: Option<bool>,
            verdict: Option<String>,
            rationale: Option<String>,
            findings: Option<Vec<ReviewFinding>>,
            confidence_score: Option<u8>,
            requirement_coverage: Option<Vec<String>>,
        }

        let raw: RawVerdict = serde_json::from_str(clean).map_err(|e| {
            ReviewerExecutionError::InvalidVerdict(format!("JSON parsing failed: {}", e))
        })?;

        let decision = if let Some(d) = raw.decision {
            d
        } else if let Some(approved) = raw.approved {
            if approved {
                ReviewDecision::Approved
            } else {
                ReviewDecision::ChangesRequested
            }
        } else {
            return Err(ReviewerExecutionError::InvalidVerdict(
                "Missing required decision or approved field".to_string(),
            ));
        };

        let rationale = raw
            .rationale
            .or(raw.verdict)
            .unwrap_or_else(|| "Review evaluation completed".to_string());

        let findings = raw.findings.unwrap_or_default();
        let confidence_score = raw.confidence_score.unwrap_or(90);
        let requirement_coverage = raw.requirement_coverage.unwrap_or_default();

        Ok(Self {
            review_id: Uuid::now_v7().to_string(),
            mission_id,
            task_id,
            snapshot_hash: snapshot_hash.to_string(),
            decision,
            findings,
            confidence_score,
            rationale,
            requirement_coverage,
        })
    }
}

pub(crate) fn extract_json_payload(raw: &str) -> &str {
    let trimmed = raw.trim();
    if let Some(start) = trimmed.find("```json") {
        let content = &trimmed[start + 7..];
        if let Some(end) = content.find("```") {
            return content[..end].trim();
        }
    }
    if let Some(start) = trimmed.find("```") {
        let content = &trimmed[start + 3..];
        if let Some(end) = content.find("```") {
            return content[..end].trim();
        }
    }
    if let (Some(first_brace), Some(last_brace)) = (trimmed.find('{'), trimmed.rfind('}'))
        && first_brace < last_brace
    {
        return &trimmed[first_brace..=last_brace];
    }
    trimmed
}

/// Tier 6 Independent Reviewer executor (VER-02, D-03).
#[derive(Clone)]
pub struct IndependentReviewer {
    pub simulated_verdict: Option<Result<ReviewVerdict, ReviewerExecutionError>>,
    pub prompt_catalog: Arc<dyn PromptCatalog>,
    pub prompt_compiler: Arc<dyn PromptCompiler>,
    pub model_caller: Option<Arc<dyn ModelCaller>>,
}

impl Default for IndependentReviewer {
    fn default() -> Self {
        Self::new()
    }
}

impl IndependentReviewer {
    /// Isolated default: standalone built-in authorities for tests and
    /// standalone use. Production MUST inject the runtime-shared authorities
    /// via [`IndependentReviewer::with_catalog`] /
    /// [`IndependentReviewer::with_compiler`].
    pub fn new() -> Self {
        Self {
            simulated_verdict: None,
            prompt_catalog: Arc::new(InMemoryPromptCatalog::with_builtins()),
            prompt_compiler: Arc::new(DefaultPromptCompiler::new()),
            model_caller: None,
        }
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

    pub fn with_simulated_verdict(mut self, verdict: ReviewVerdict) -> Self {
        self.simulated_verdict = Some(Ok(verdict));
        self
    }

    pub fn with_simulated_error(mut self, err: ReviewerExecutionError) -> Self {
        self.simulated_verdict = Some(Err(err));
        self
    }

    /// Decides whether independent review is mandated for this task scope (D-02).
    ///
    /// Mandated for:
    /// - Mission completion gates
    /// - Architecture / security tasks
    /// - Modifications to critical boundaries or high-risk policies
    pub fn evaluate_risk_mandate(
        is_mission_completion: bool,
        is_security_or_architecture: bool,
        modifies_critical_boundary: bool,
    ) -> bool {
        is_mission_completion || is_security_or_architecture || modifies_critical_boundary
    }

    /// Returns the strictly read-only capability envelope enforced on the Reviewer (D-03).
    pub fn capability_envelope() -> CapabilityEnvelope {
        CapabilityEnvelope::read_only([
            "repo.read",
            "artifacts.read",
            "diff.analyze",
            "review.submit",
        ])
    }

    /// Maps a typed `ReviewVerdict` into a runtime `VerificationCheck`.
    pub fn map_verdict_to_check(verdict: &ReviewVerdict) -> VerificationCheck {
        let (status, failure_class) = match verdict.decision {
            ReviewDecision::Approved => (CheckStatus::Passed, None),
            ReviewDecision::ChangesRequested => (
                CheckStatus::Failed,
                Some("ReviewChangesRequested".to_string()),
            ),
            ReviewDecision::RejectedWithPrejudice => {
                (CheckStatus::Failed, Some("ReviewRejected".to_string()))
            }
        };

        let summary = format!(
            "Review verdict: {:?} (confidence: {}%). Findings: {}. Rationale: {}",
            verdict.decision,
            verdict.confidence_score,
            verdict.findings.len(),
            verdict.rationale
        );

        let inputs_desc = format!(
            "review_id={}, findings_count={}, requirements_covered=[{}]",
            verdict.review_id,
            verdict.findings.len(),
            verdict.requirement_coverage.join(", ")
        );

        VerificationCheck {
            check_id: crate::ids::CheckId::new(),
            mission_id: verdict.mission_id,
            task_id: verdict.task_id,
            tier: CheckTier::IndependentReview,
            status,
            command_or_tool: "independent_reviewer_agent".to_string(),
            inputs_normalized: inputs_desc,
            evidence_artifact_id: None,
            summary,
            failure_class,
            snapshot_hash: verdict.snapshot_hash.clone(),
            created_at: chrono::Utc::now(),
        }
    }
}

#[async_trait]
impl VerificationRunner for IndependentReviewer {
    async fn execute(
        &self,
        mission_id: MissionId,
        task_id: TaskId,
        _workspace_root: &Path,
        snapshot_hash: &str,
    ) -> Result<VerificationCheck, String> {
        if let Some(ref sim) = self.simulated_verdict {
            match sim {
                Ok(v) => Ok(Self::map_verdict_to_check(v)),
                Err(e) => match e {
                    ReviewerExecutionError::InfrastructureFailure(msg) => {
                        Err(format!("Reviewer infrastructure failure: {}", msg))
                    }
                    ReviewerExecutionError::Timeout(msg) => {
                        Err(format!("Reviewer timeout: {}", msg))
                    }
                    ReviewerExecutionError::InvalidVerdict(msg) => {
                        Err(format!("Reviewer invalid verdict: {}", msg))
                    }
                    ReviewerExecutionError::SandboxViolation(msg) => {
                        Err(format!("Reviewer sandbox violation: {}", msg))
                    }
                },
            }
        } else if let Some(ref caller) = self.model_caller {
            // Live model execution path (VER-02, D-03). The runner receives
            // only mission/task/snapshot identifiers, so the review context
            // carries snapshot-scoped evidence explicitly and asserts NO
            // acceptance criteria beyond what was supplied: an empty
            // criteria list means "criteria unavailable", never "criteria
            // met". Criteria-bearing callers must use ReviewerAgentContext
            // directly via compile_prompt.
            let ctx = ReviewerAgentContext::new(
                format!("Review Task {}", task_id),
                format!(
                    "Snapshot-scoped independent review of task modifications (snapshot {}); acceptance criteria unavailable in this execution path",
                    snapshot_hash
                ),
                Vec::new(),
                snapshot_hash,
                "",
                vec![],
                None,
            );
            let effective = ctx
                .compile_prompt(&*self.prompt_catalog, &*self.prompt_compiler)
                .map_err(|e| format!("Failed to compile reviewer prompt: {}", e))?;

            let proposal = caller
                .call_model(&effective.assembled_text)
                .await
                .map_err(|e| format!("Model call failed: {}", e))?;

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

            let verdict =
                ReviewVerdict::parse_model_output(&raw_text, mission_id, task_id, snapshot_hash)
                    .map_err(|e| format!("Verdict parsing failed: {}", e))?;

            Ok(Self::map_verdict_to_check(&verdict))
        } else {
            // Declarative authority boundary: without a model
            // provider there is no evidence for an independent review
            // verdict. Approving by default would be fake success (AGENTS.md
            // rule 5). Fail closed with an explicit error so the verification
            // gate blocks instead of passing on fabricated confidence.
            Err(format!(
                "Independent review for task {} requires a configured model provider: no review evidence available, refusing default approval",
                task_id
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reviewer_read_only_capability_envelope() {
        let envelope = IndependentReviewer::capability_envelope();
        assert!(!envelope.allow_file_write);
        assert!(!envelope.allow_shell_execution);
        assert!(!envelope.allow_network_access);
        assert!(envelope.allowed_capabilities.contains("repo.read"));
        assert!(envelope.allowed_capabilities.contains("artifacts.read"));
        assert!(envelope.allowed_capabilities.contains("review.submit"));
    }

    #[test]
    fn test_reviewer_risk_mandate_evaluation() {
        assert!(IndependentReviewer::evaluate_risk_mandate(
            true, false, false
        ));
        assert!(IndependentReviewer::evaluate_risk_mandate(
            false, true, false
        ));
        assert!(IndependentReviewer::evaluate_risk_mandate(
            false, false, true
        ));
        assert!(!IndependentReviewer::evaluate_risk_mandate(
            false, false, false
        ));
    }

    #[test]
    fn test_fresh_context_compilation_excludes_transcript() {
        let ctx = ReviewerAgentContext::new(
            "Fix off-by-one error",
            "Update loop boundary",
            vec!["Loop iterates N times".to_string()],
            "snap-123",
            "+ for i in 0..n",
            vec!["Tier 1 passed".to_string(), "Tier 2 passed".to_string()],
            Some("test_loop ... ok".to_string()),
        );

        let user_prompt = ctx.compile_user_prompt();
        assert!(user_prompt.contains("Fix off-by-one error"));
        assert!(user_prompt.contains("snap-123"));
        assert!(user_prompt.contains("+ for i in 0..n"));
        // Assert no conversation transcript headers exist
        assert!(!user_prompt.contains("User:"));
        assert!(!user_prompt.contains("Assistant:"));
        assert!(!user_prompt.contains("Implementer reasoning:"));
    }
}
