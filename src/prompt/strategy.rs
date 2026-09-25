//! PromptStrategy enum and deterministic strategy resolution for PromptOS (Section 10 & 11).
//!
//! # Core Invariant: "The model proposes. The runtime decides."
//!
//! A single logical prompt contract is executed across different models and operational
//! contexts by applying a runtime-selected [`PromptStrategy`].
//!
//! # DeepReasoning Invariant
//! `DeepReasoning` injects explicit architectural directives (task decomposition, invariant validation,
//! and pre-execution verification checklists). **DeepReasoning MUST NOT request or instruct the model
//! to disclose private chain-of-thought or hidden reasoning.**

use crate::prompt::context::MissionStage;
use crate::prompt::model_profile::ModelProfile;
use crate::state_machine::agent::AgentRole;
use serde::{Deserialize, Serialize};

/// Canonical prompt compilation strategy (Section 10).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PromptStrategy {
    /// Fast models, simple tasks: high density, zero fluff, concise steps, 0 examples.
    Minimal,
    /// Balanced models: balanced explanations, full section structure, adaptive reasoning.
    Standard,
    /// Complex architecture, hard bugs: in-depth invariants, architectural decomposition, verification checklists.
    DeepReasoning,
    /// Smaller/local models: highly explicit procedural steps, strict guardrails, few-shot examples.
    Constrained,
    /// Tight context windows (<= 16k tokens) or near budget limit: compact abbreviations, minimal history.
    LowContext,
    /// Active recovery loop following tool failure or test rejection: error-focused, failure evidence.
    Recovery,
}

impl PromptStrategy {
    /// Return the canonical wire string representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Minimal => "minimal",
            Self::Standard => "standard",
            Self::DeepReasoning => "deep_reasoning",
            Self::Constrained => "constrained",
            Self::LowContext => "low_context",
            Self::Recovery => "recovery",
        }
    }

    /// Target few-shot example budget for this strategy.
    pub fn example_budget(&self) -> usize {
        match self {
            Self::Minimal => 0,
            Self::Standard => 1,
            Self::DeepReasoning => 1,
            Self::Constrained => 3,
            Self::LowContext => 0,
            Self::Recovery => 1,
        }
    }

    /// Canonical description of instruction density and style.
    pub fn instruction_density(&self) -> &'static str {
        match self {
            Self::Minimal => "high_density_zero_fluff",
            Self::Standard => "balanced",
            Self::DeepReasoning => "in_depth_invariants",
            Self::Constrained => "explicit_procedural",
            Self::LowContext => "compact_abbreviations",
            Self::Recovery => "error_focused",
        }
    }

    /// Concrete structural directives injected into prompt composition.
    ///
    /// GUARANTEE: Contains NO requests for chain-of-thought, private reasoning, or hidden step-by-step thinking.
    pub fn structural_directives(&self) -> &'static str {
        match self {
            Self::Minimal => {
                "### Execution Directives:\n- Direct execution only. Omit conversational filler.\n- Emit only valid tool actions or requested output format."
            }
            Self::Standard => {
                "### Execution Directives:\n- Proceed methodically with task implementation.\n- Verify assumptions against durable state and project boundaries."
            }
            Self::DeepReasoning => {
                "### Architectural & Invariant Analysis Directives:\n- Decompose the problem into verified sub-components before making modifications.\n- Explicitly validate all system invariants, architectural boundaries, and safety constraints.\n- Identify downstream implications and potential regression risks across the crate.\n- Formulate a pre-execution verification checklist for your proposed tool actions."
            }
            Self::Constrained => {
                "### Procedural Execution Directives:\n- Execute exactly one action per step.\n- Validate every argument against the parameter schema before invoking.\n- Do not extrapolate or attempt unsupported operations."
            }
            Self::LowContext => {
                "### Compact Directives:\n- Strict brevity. Concise responses only."
            }
            Self::Recovery => {
                "### Recovery & Diagnostic Directives:\n- Prioritize root-cause diagnosis of the observed failure.\n- Inspect the failure evidence and error snippets before proposing corrective action.\n- Do not repeat the failed action without modifying inputs or addressing the failure condition."
            }
        }
    }

    /// Canonical few-shot procedural examples for strategies that require them (e.g. `Constrained`, `Recovery`).
    pub fn few_shot_examples(&self) -> Option<&'static str> {
        match self {
            Self::Constrained => Some(
                "### Procedural Execution Example:\nStep 1: Check file existence\nAction: view_file\nArguments: {\"AbsolutePath\": \"src/lib.rs\", \"StartLine\": 1, \"EndLine\": 20}\nExpectation: Confirm module declarations before editing.",
            ),
            Self::Recovery => Some(
                "### Corrective Action Example:\nFailure: Compilation error in src/prompt/compiler.rs: mismatched types\nDiagnostic: Variable expected usize but received u64\nCorrective Action: Apply explicit type conversion or update function signature.",
            ),
            _ => None,
        }
    }
}

impl std::fmt::Display for PromptStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Deterministically resolve the effective [`PromptStrategy`] based on model capabilities and task context.
///
/// # Evaluation Rules (in strict priority order):
/// 1. Recovery state (`in_recovery` or `stage == MissionStage::Diagnose`) -> [`PromptStrategy::Recovery`]
/// 2. Physical context window limit (`context_capacity <= 16,384`) -> [`PromptStrategy::LowContext`]
/// 3. Constrained instruction following (`instruction_following <= 2`) -> [`PromptStrategy::Constrained`]
/// 4. High task complexity on frontier reasoning model (`reasoning_strength >= 5 && task_complexity_high`) -> [`PromptStrategy::DeepReasoning`]
/// 5. Validated strategy override (`override_strategy`) -> applied if not precluded by physical constraints
/// 6. Default strategy configured on the profile -> [`ModelProfile::default_strategy`]
pub fn resolve_strategy(
    _role: AgentRole,
    stage: MissionStage,
    task_complexity_high: bool,
    in_recovery: bool,
    profile: &ModelProfile,
    override_strategy: Option<PromptStrategy>,
) -> PromptStrategy {
    // 1. Recovery condition: in active recovery loop or in diagnostic stage
    if in_recovery || stage == MissionStage::Diagnose {
        return PromptStrategy::Recovery;
    }

    // 2. Hardware / Context limit: context capacity <= 16,384
    if profile.context_capacity <= 16_384 {
        return PromptStrategy::LowContext;
    }

    // 3. Model instruction capability limit: instruction_following <= 2
    if profile.instruction_following.as_u8() <= 2 {
        return PromptStrategy::Constrained;
    }

    // 4. High complexity with strong reasoning: reasoning_strength >= 5 && task_complexity_high
    if profile.reasoning_strength.as_u8() >= 5 && task_complexity_high {
        return PromptStrategy::DeepReasoning;
    }

    // 5. Validated strategy override
    if let Some(override_strat) = override_strategy {
        return override_strat;
    }

    // 6. Profile default strategy
    profile.default_strategy
}
