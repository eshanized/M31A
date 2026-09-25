//! Embedded TOML builtin prompt contracts.
//!
//! Provides compile-time embedding of the canonical prompt contracts located under `prompts/`.
//! All built-in prompts are parsed and validated upon catalog initialization.

use crate::prompt::contract::PromptContract;
use crate::prompt::error::PromptError;

pub const GENESIS_DISCOVERY: &str = include_str!("../../prompts/genesis/discovery.v1.toml");
pub const GENESIS_CHARTER: &str = include_str!("../../prompts/genesis/charter.v1.toml");
pub const GENESIS_RESEARCH_STACK: &str =
    include_str!("../../prompts/genesis/research_stack.v1.toml");
pub const GENESIS_RESEARCH_FEATURES: &str =
    include_str!("../../prompts/genesis/research_features.v1.toml");
pub const GENESIS_RESEARCH_ARCHITECTURE: &str =
    include_str!("../../prompts/genesis/research_architecture.v1.toml");
pub const GENESIS_RESEARCH_PITFALLS: &str =
    include_str!("../../prompts/genesis/research_pitfalls.v1.toml");
pub const GENESIS_RESEARCH_SECURITY: &str =
    include_str!("../../prompts/genesis/research_security.v1.toml");
pub const GENESIS_RESEARCH_DEPLOYMENT: &str =
    include_str!("../../prompts/genesis/research_deployment.v1.toml");
pub const GENESIS_RESEARCH_SYNTHESIS: &str =
    include_str!("../../prompts/genesis/research_synthesis.v1.toml");
pub const GENESIS_SYNTHESIS: &str = include_str!("../../prompts/genesis/synthesis.v1.toml");
pub const GENESIS_REQUIREMENTS: &str = include_str!("../../prompts/genesis/requirements.v1.toml");
pub const GENESIS_ARCHITECTURE: &str = include_str!("../../prompts/genesis/architecture.v1.toml");
pub const GENESIS_ADR: &str = include_str!("../../prompts/genesis/adr.v1.toml");
pub const GENESIS_RISKS: &str = include_str!("../../prompts/genesis/risks.v1.toml");
pub const GENESIS_ROADMAP: &str = include_str!("../../prompts/genesis/roadmap.v1.toml");
pub const EXECUTION_IMPLEMENTER: &str = include_str!("../../prompts/execution/implementer.v1.toml");
pub const EXECUTION_REVIEWER: &str = include_str!("../../prompts/verification/reviewer.v1.toml");
pub const VERIFICATION_REVIEWER: &str = EXECUTION_REVIEWER;
pub const PLANNING_DECOMPOSE: &str = include_str!("../../prompts/planning/decompose.v1.toml");
pub const EXECUTION_DIAGNOSTICIAN: &str =
    include_str!("../../prompts/recovery/diagnostician.v1.toml");
pub const RECOVERY_DIAGNOSTICIAN: &str = EXECUTION_DIAGNOSTICIAN;
pub const EXECUTION_VERIFIER: &str = include_str!("../../prompts/execution/verifier.v1.toml");
pub const RUNTIME_SAFETY_INVARIANTS: &str = include_str!("../../prompts/core/safety.v1.toml");
pub const IMPLEMENT: &str = include_str!("../../prompts/execution/implement.v1.toml");
pub const VERIFY: &str = include_str!("../../prompts/execution/verify.v1.toml");
pub const DIAGNOSE: &str = include_str!("../../prompts/execution/diagnose.v1.toml");
pub const REVIEW: &str = include_str!("../../prompts/execution/review.v1.toml");

pub const AGENT_PLANNER: &str = include_str!("../../prompts/agents/planner.v1.toml");
pub const AGENT_RESEARCHER: &str = include_str!("../../prompts/agents/researcher.v1.toml");
pub const AGENT_ARCHITECT: &str = include_str!("../../prompts/agents/architect.v1.toml");
pub const AGENT_IMPLEMENTER: &str = include_str!("../../prompts/agents/implementer.v1.toml");
pub const AGENT_REVIEWER: &str = include_str!("../../prompts/agents/reviewer.v1.toml");
pub const AGENT_VERIFIER: &str = include_str!("../../prompts/agents/verifier.v1.toml");
pub const AGENT_DIAGNOSTICIAN: &str = include_str!("../../prompts/agents/diagnostician.v1.toml");
pub const AGENT_INTEGRATOR: &str = include_str!("../../prompts/agents/integrator.v1.toml");
pub const AGENT_DISCOVERY_ANALYST: &str =
    include_str!("../../prompts/agents/discovery_analyst.v1.toml");
pub const AGENT_STACK_RESEARCHER: &str =
    include_str!("../../prompts/agents/stack_researcher.v1.toml");
pub const AGENT_FEATURES_RESEARCHER: &str =
    include_str!("../../prompts/agents/features_researcher.v1.toml");
pub const AGENT_ARCHITECTURE_RESEARCHER: &str =
    include_str!("../../prompts/agents/architecture_researcher.v1.toml");
pub const AGENT_PITFALLS_RESEARCHER: &str =
    include_str!("../../prompts/agents/pitfalls_researcher.v1.toml");
pub const AGENT_SECURITY_RESEARCHER: &str =
    include_str!("../../prompts/agents/security_researcher.v1.toml");
pub const AGENT_DEPLOYMENT_RESEARCHER: &str =
    include_str!("../../prompts/agents/deployment_researcher.v1.toml");
pub const AGENT_SYNTHESIZER: &str = include_str!("../../prompts/agents/synthesizer.v1.toml");
pub const SKILL_IN_TASK_GUIDANCE: &str =
    include_str!("../../prompts/skills/in_task_guidance.v1.toml");

pub const GENESIS_DYNAMIC_QUESTIONS: &str =
    include_str!("../../prompts/genesis/dynamic_questions.v1.toml");
pub const PLANNING_REVISION: &str = include_str!("../../prompts/planning/plan_revision.v1.toml");
pub const PLANNING_TASK_REVISION: &str =
    include_str!("../../prompts/planning/task_revision.v1.toml");
pub const EXECUTION_AUTHORIZATION_EXPLANATION: &str =
    include_str!("../../prompts/execution/authorization_explanation.v1.toml");

/// Complete set of embedded builtin prompt TOML assets in deterministic order.
pub const BUILTIN_PROMPTS: &[&str] = &[
    GENESIS_DISCOVERY,
    GENESIS_CHARTER,
    GENESIS_RESEARCH_STACK,
    GENESIS_RESEARCH_FEATURES,
    GENESIS_RESEARCH_ARCHITECTURE,
    GENESIS_RESEARCH_PITFALLS,
    GENESIS_RESEARCH_SECURITY,
    GENESIS_RESEARCH_DEPLOYMENT,
    GENESIS_RESEARCH_SYNTHESIS,
    GENESIS_SYNTHESIS,
    GENESIS_REQUIREMENTS,
    GENESIS_ARCHITECTURE,
    GENESIS_ADR,
    GENESIS_RISKS,
    GENESIS_ROADMAP,
    GENESIS_DYNAMIC_QUESTIONS,
    EXECUTION_IMPLEMENTER,
    EXECUTION_REVIEWER,
    PLANNING_DECOMPOSE,
    PLANNING_REVISION,
    PLANNING_TASK_REVISION,
    EXECUTION_DIAGNOSTICIAN,
    EXECUTION_VERIFIER,
    EXECUTION_AUTHORIZATION_EXPLANATION,
    RUNTIME_SAFETY_INVARIANTS,
    IMPLEMENT,
    VERIFY,
    DIAGNOSE,
    REVIEW,
    AGENT_PLANNER,
    AGENT_RESEARCHER,
    AGENT_ARCHITECT,
    AGENT_IMPLEMENTER,
    AGENT_REVIEWER,
    AGENT_VERIFIER,
    AGENT_DIAGNOSTICIAN,
    AGENT_INTEGRATOR,
    AGENT_DISCOVERY_ANALYST,
    AGENT_STACK_RESEARCHER,
    AGENT_FEATURES_RESEARCHER,
    AGENT_ARCHITECTURE_RESEARCHER,
    AGENT_PITFALLS_RESEARCHER,
    AGENT_SECURITY_RESEARCHER,
    AGENT_DEPLOYMENT_RESEARCHER,
    AGENT_SYNTHESIZER,
    SKILL_IN_TASK_GUIDANCE,
];

pub const CORE_SAFETY_V2: &str = include_str!("../../prompts/core/safety.v2.toml");
pub const AGENT_AUDITOR_V2: &str = include_str!("../../prompts/agents/auditor.v2.toml");
pub const AGENT_RELEASE_CERTIFIER_V2: &str =
    include_str!("../../prompts/agents/release_certifier.v2.toml");
pub const AGENT_IMPLEMENTER_V2: &str = include_str!("../../prompts/agents/implementer.v2.toml");
pub const AGENT_REVIEWER_V2: &str = include_str!("../../prompts/agents/reviewer.v2.toml");
pub const AGENT_VERIFIER_V2: &str = include_str!("../../prompts/agents/verifier.v2.toml");
pub const AGENT_DIAGNOSTICIAN_V2: &str = include_str!("../../prompts/agents/diagnostician.v2.toml");
pub const EXECUTION_IMPLEMENTER_V2: &str =
    include_str!("../../prompts/execution/implementer.v2.toml");
pub const VERIFICATION_REVIEWER_V2: &str =
    include_str!("../../prompts/verification/reviewer.v2.toml");
pub const VERIFICATION_TASK_V2: &str = include_str!("../../prompts/verification/task.v2.toml");
pub const RECOVERY_DIAGNOSTICIAN_V2: &str =
    include_str!("../../prompts/recovery/diagnostician.v2.toml");
pub const PLANNING_DECOMPOSE_V2: &str = include_str!("../../prompts/planning/decompose.v2.toml");

/// Canonical PromptOS V2 embedded builtin prompt contracts.
pub const BUILTIN_PROMPTS_V2: &[&str] = &[
    CORE_SAFETY_V2,
    AGENT_AUDITOR_V2,
    AGENT_RELEASE_CERTIFIER_V2,
    AGENT_IMPLEMENTER_V2,
    AGENT_REVIEWER_V2,
    AGENT_VERIFIER_V2,
    AGENT_DIAGNOSTICIAN_V2,
    EXECUTION_IMPLEMENTER_V2,
    VERIFICATION_REVIEWER_V2,
    VERIFICATION_TASK_V2,
    RECOVERY_DIAGNOSTICIAN_V2,
    PLANNING_DECOMPOSE_V2,
];

/// Load and parse all legacy embedded builtin prompt contracts (V1).
pub fn load_builtin_contracts() -> Result<Vec<PromptContract>, PromptError> {
    let mut contracts = Vec::with_capacity(BUILTIN_PROMPTS.len());
    for toml_str in BUILTIN_PROMPTS {
        let contract = PromptContract::from_toml_str(toml_str)?;
        contracts.push(contract);
    }
    Ok(contracts)
}

/// Load and parse all canonical PromptOS V2 embedded prompt contracts.
pub fn load_builtin_contracts_v2() -> Result<Vec<PromptContract>, PromptError> {
    let mut contracts = Vec::with_capacity(BUILTIN_PROMPTS_V2.len());
    for toml_str in BUILTIN_PROMPTS_V2 {
        let contract = PromptContract::from_toml_str(toml_str)?;
        contracts.push(contract);
    }
    Ok(contracts)
}

/// Load all builtin contracts across both legacy V1 and canonical V2 tiers.
pub fn load_all_builtin_contracts() -> Result<Vec<PromptContract>, PromptError> {
    let mut contracts = load_builtin_contracts()?;
    let v2_contracts = load_builtin_contracts_v2()?;
    contracts.extend(v2_contracts);
    Ok(contracts)
}
