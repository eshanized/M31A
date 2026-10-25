//! Embedded Markdown builtin prompt contracts.
//!
//! Provides compile-time embedding of the canonical prompt contracts located under `prompts/`.
//! All built-in prompts are parsed and validated upon catalog initialization.

use crate::prompt::contract::PromptContract;
use crate::prompt::error::PromptError;

pub const GENESIS_DISCOVERY: &str = include_str!("../../prompts/genesis/discovery.md");
pub const GENESIS_CHARTER: &str = include_str!("../../prompts/genesis/charter.md");
pub const GENESIS_RESEARCH_STACK: &str = include_str!("../../prompts/genesis/research_stack.md");
pub const GENESIS_RESEARCH_FEATURES: &str =
    include_str!("../../prompts/genesis/research_features.md");
pub const GENESIS_RESEARCH_ARCHITECTURE: &str =
    include_str!("../../prompts/genesis/research_architecture.md");
pub const GENESIS_RESEARCH_PITFALLS: &str =
    include_str!("../../prompts/genesis/research_pitfalls.md");
pub const GENESIS_RESEARCH_SECURITY: &str =
    include_str!("../../prompts/genesis/research_security.md");
pub const GENESIS_RESEARCH_DEPLOYMENT: &str =
    include_str!("../../prompts/genesis/research_deployment.md");
pub const GENESIS_RESEARCH_SYNTHESIS: &str =
    include_str!("../../prompts/genesis/research_synthesis.md");
pub const GENESIS_SYNTHESIS: &str = include_str!("../../prompts/genesis/synthesis.md");
pub const GENESIS_REQUIREMENTS: &str = include_str!("../../prompts/genesis/requirements.md");
pub const GENESIS_ARCHITECTURE: &str = include_str!("../../prompts/genesis/architecture.md");
pub const GENESIS_ADR: &str = include_str!("../../prompts/genesis/adr.md");
pub const GENESIS_RISKS: &str = include_str!("../../prompts/genesis/risks.md");
pub const GENESIS_ROADMAP: &str = include_str!("../../prompts/genesis/roadmap.md");
pub const EXECUTION_IMPLEMENTER: &str =
    include_str!("../../prompts/execution/compat/implementer.v1.md");
pub const EXECUTION_REVIEWER: &str =
    include_str!("../../prompts/verification/compat/reviewer.v1.md");
pub const PLANNING_DECOMPOSE: &str = include_str!("../../prompts/planning/compat/decompose.v1.md");
pub const EXECUTION_DIAGNOSTICIAN: &str =
    include_str!("../../prompts/recovery/compat/diagnostician.v1.md");
pub const EXECUTION_VERIFIER: &str = include_str!("../../prompts/execution/verifier.md");
pub const RUNTIME_SAFETY_INVARIANTS: &str = include_str!("../../prompts/core/compat/safety.v1.md");
pub const IMPLEMENT: &str = include_str!("../../prompts/execution/implement.md");
pub const VERIFY: &str = include_str!("../../prompts/execution/verify.md");
pub const DIAGNOSE: &str = include_str!("../../prompts/execution/diagnose.md");
pub const REVIEW: &str = include_str!("../../prompts/execution/review.md");

pub const AGENT_PLANNER: &str = include_str!("../../prompts/agents/planner.md");
pub const AGENT_RESEARCHER: &str = include_str!("../../prompts/agents/researcher.md");
pub const AGENT_ARCHITECT: &str = include_str!("../../prompts/agents/architect.md");
pub const AGENT_IMPLEMENTER: &str = include_str!("../../prompts/agents/compat/implementer.v1.md");
pub const AGENT_REVIEWER: &str = include_str!("../../prompts/agents/compat/reviewer.v1.md");
pub const AGENT_VERIFIER: &str = include_str!("../../prompts/agents/compat/verifier.v1.md");
pub const AGENT_DIAGNOSTICIAN: &str =
    include_str!("../../prompts/agents/compat/diagnostician.v1.md");
pub const AGENT_INTEGRATOR: &str = include_str!("../../prompts/agents/integrator.md");
pub const AGENT_DISCOVERY_ANALYST: &str = include_str!("../../prompts/agents/discovery_analyst.md");
pub const AGENT_STACK_RESEARCHER: &str = include_str!("../../prompts/agents/stack_researcher.md");
pub const AGENT_FEATURES_RESEARCHER: &str =
    include_str!("../../prompts/agents/features_researcher.md");
pub const AGENT_ARCHITECTURE_RESEARCHER: &str =
    include_str!("../../prompts/agents/architecture_researcher.md");
pub const AGENT_PITFALLS_RESEARCHER: &str =
    include_str!("../../prompts/agents/pitfalls_researcher.md");
pub const AGENT_SECURITY_RESEARCHER: &str =
    include_str!("../../prompts/agents/security_researcher.md");
pub const AGENT_DEPLOYMENT_RESEARCHER: &str =
    include_str!("../../prompts/agents/deployment_researcher.md");
pub const AGENT_SYNTHESIZER: &str = include_str!("../../prompts/agents/synthesizer.md");
pub const SKILL_IN_TASK_GUIDANCE: &str = include_str!("../../prompts/skills/in_task_guidance.md");

pub const GENESIS_DYNAMIC_QUESTIONS: &str =
    include_str!("../../prompts/genesis/dynamic_questions.md");
pub const PLANNING_REVISION: &str = include_str!("../../prompts/planning/plan_revision.md");
pub const PLANNING_TASK_REVISION: &str = include_str!("../../prompts/planning/task_revision.md");
pub const EXECUTION_AUTHORIZATION_EXPLANATION: &str =
    include_str!("../../prompts/execution/authorization_explanation.md");

/// Complete set of embedded builtin prompt Markdown assets in deterministic order.
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

pub const CORE_SAFETY_V2: &str = include_str!("../../prompts/core/safety.md");
pub const AGENT_AUDITOR_V2: &str = include_str!("../../prompts/agents/auditor.md");
pub const AGENT_RELEASE_CERTIFIER_V2: &str =
    include_str!("../../prompts/agents/release_certifier.md");
pub const AGENT_IMPLEMENTER_V2: &str = include_str!("../../prompts/agents/implementer.md");
pub const AGENT_REVIEWER_V2: &str = include_str!("../../prompts/agents/reviewer.md");
pub const AGENT_VERIFIER_V2: &str = include_str!("../../prompts/agents/verifier.md");
pub const AGENT_DIAGNOSTICIAN_V2: &str = include_str!("../../prompts/agents/diagnostician.md");
pub const EXECUTION_IMPLEMENTER_V2: &str = include_str!("../../prompts/execution/implementer.md");
pub const VERIFICATION_REVIEWER_V2: &str = include_str!("../../prompts/verification/reviewer.md");
pub const VERIFICATION_TASK_V2: &str = include_str!("../../prompts/verification/task.md");
pub const RECOVERY_DIAGNOSTICIAN_V2: &str = include_str!("../../prompts/recovery/diagnostician.md");
pub const PLANNING_DECOMPOSE_V2: &str = include_str!("../../prompts/planning/decompose.md");

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

/// Canonical production prompt contracts (34 contracts).
pub const PRODUCTION_PROMPTS: &[&str] = &[
    CORE_SAFETY_V2,
    AGENT_AUDITOR_V2,
    AGENT_RELEASE_CERTIFIER_V2,
    AGENT_IMPLEMENTER_V2,
    AGENT_REVIEWER_V2,
    AGENT_VERIFIER_V2,
    AGENT_DIAGNOSTICIAN_V2,
    AGENT_PLANNER,
    AGENT_RESEARCHER,
    AGENT_ARCHITECT,
    AGENT_INTEGRATOR,
    AGENT_DISCOVERY_ANALYST,
    AGENT_STACK_RESEARCHER,
    AGENT_FEATURES_RESEARCHER,
    AGENT_ARCHITECTURE_RESEARCHER,
    AGENT_PITFALLS_RESEARCHER,
    AGENT_SECURITY_RESEARCHER,
    AGENT_DEPLOYMENT_RESEARCHER,
    AGENT_SYNTHESIZER,
    EXECUTION_IMPLEMENTER_V2,
    GENESIS_RESEARCH_STACK,
    GENESIS_RESEARCH_FEATURES,
    GENESIS_RESEARCH_ARCHITECTURE,
    GENESIS_RESEARCH_PITFALLS,
    GENESIS_RESEARCH_SECURITY,
    GENESIS_RESEARCH_DEPLOYMENT,
    GENESIS_RESEARCH_SYNTHESIS,
    GENESIS_DYNAMIC_QUESTIONS,
    PLANNING_DECOMPOSE_V2,
    PLANNING_REVISION,
    PLANNING_TASK_REVISION,
    VERIFICATION_REVIEWER_V2,
    VERIFICATION_TASK_V2,
    RECOVERY_DIAGNOSTICIAN_V2,
];

/// Intentional compatibility prompt contracts retained for version migration (8 contracts).
pub const COMPATIBILITY_PROMPTS: &[&str] = &[
    AGENT_IMPLEMENTER,
    AGENT_REVIEWER,
    AGENT_VERIFIER,
    AGENT_DIAGNOSTICIAN,
    EXECUTION_IMPLEMENTER,
    PLANNING_DECOMPOSE,
    EXECUTION_REVIEWER,
    EXECUTION_DIAGNOSTICIAN,
];

/// Active production and migration contracts (42 contracts total).
pub const PRODUCTION_AND_COMPATIBILITY_PROMPTS: &[&str] = &[
    CORE_SAFETY_V2,
    AGENT_AUDITOR_V2,
    AGENT_RELEASE_CERTIFIER_V2,
    AGENT_IMPLEMENTER_V2,
    AGENT_REVIEWER_V2,
    AGENT_VERIFIER_V2,
    AGENT_DIAGNOSTICIAN_V2,
    AGENT_PLANNER,
    AGENT_RESEARCHER,
    AGENT_ARCHITECT,
    AGENT_INTEGRATOR,
    AGENT_DISCOVERY_ANALYST,
    AGENT_STACK_RESEARCHER,
    AGENT_FEATURES_RESEARCHER,
    AGENT_ARCHITECTURE_RESEARCHER,
    AGENT_PITFALLS_RESEARCHER,
    AGENT_SECURITY_RESEARCHER,
    AGENT_DEPLOYMENT_RESEARCHER,
    AGENT_SYNTHESIZER,
    EXECUTION_IMPLEMENTER_V2,
    GENESIS_RESEARCH_STACK,
    GENESIS_RESEARCH_FEATURES,
    GENESIS_RESEARCH_ARCHITECTURE,
    GENESIS_RESEARCH_PITFALLS,
    GENESIS_RESEARCH_SECURITY,
    GENESIS_RESEARCH_DEPLOYMENT,
    GENESIS_RESEARCH_SYNTHESIS,
    GENESIS_DYNAMIC_QUESTIONS,
    PLANNING_DECOMPOSE_V2,
    PLANNING_REVISION,
    PLANNING_TASK_REVISION,
    VERIFICATION_REVIEWER_V2,
    VERIFICATION_TASK_V2,
    RECOVERY_DIAGNOSTICIAN_V2,
    AGENT_IMPLEMENTER,
    AGENT_REVIEWER,
    AGENT_VERIFIER,
    AGENT_DIAGNOSTICIAN,
    EXECUTION_IMPLEMENTER,
    PLANNING_DECOMPOSE,
    EXECUTION_REVIEWER,
    EXECUTION_DIAGNOSTICIAN,
];

/// Intentional test-fixture prompt contracts (5 contracts).
pub const TEST_FIXTURE_PROMPTS: &[&str] =
    &[IMPLEMENT, REVIEW, VERIFY, DIAGNOSE, SKILL_IN_TASK_GUIDANCE];

/// Explicitly deprecated / metadata-only prompt contracts (11 contracts).
pub const DEPRECATED_METADATA_PROMPTS: &[&str] = &[
    RUNTIME_SAFETY_INVARIANTS,
    EXECUTION_AUTHORIZATION_EXPLANATION,
    EXECUTION_VERIFIER,
    GENESIS_DISCOVERY,
    GENESIS_CHARTER,
    GENESIS_SYNTHESIS,
    GENESIS_REQUIREMENTS,
    GENESIS_ARCHITECTURE,
    GENESIS_ADR,
    GENESIS_RISKS,
    GENESIS_ROADMAP,
];

/// Load and parse all legacy embedded builtin prompt contracts (V1).
pub fn load_builtin_contracts() -> Result<Vec<PromptContract>, PromptError> {
    let mut contracts = Vec::with_capacity(BUILTIN_PROMPTS.len());
    for prompt_str in BUILTIN_PROMPTS {
        let contract = PromptContract::from_markdown_str(prompt_str)?;
        contracts.push(contract);
    }
    Ok(contracts)
}

/// Load and parse all canonical PromptOS V2 embedded prompt contracts.
pub fn load_builtin_contracts_v2() -> Result<Vec<PromptContract>, PromptError> {
    let mut contracts = Vec::with_capacity(BUILTIN_PROMPTS_V2.len());
    for prompt_str in BUILTIN_PROMPTS_V2 {
        let contract = PromptContract::from_markdown_str(prompt_str)?;
        contracts.push(contract);
    }
    Ok(contracts)
}

/// Load and parse all production-reachable prompt contracts (34 contracts).
pub fn load_production_contracts() -> Result<Vec<PromptContract>, PromptError> {
    let mut contracts = Vec::with_capacity(PRODUCTION_PROMPTS.len());
    for prompt_str in PRODUCTION_PROMPTS {
        let contract = PromptContract::from_markdown_str(prompt_str)?;
        contracts.push(contract);
    }
    Ok(contracts)
}

/// Load and parse all intentional compatibility prompt contracts (8 contracts).
pub fn load_compatibility_contracts() -> Result<Vec<PromptContract>, PromptError> {
    let mut contracts = Vec::with_capacity(COMPATIBILITY_PROMPTS.len());
    for prompt_str in COMPATIBILITY_PROMPTS {
        let contract = PromptContract::from_markdown_str(prompt_str)?;
        contracts.push(contract);
    }
    Ok(contracts)
}

/// Load and parse all production and compatibility contracts required for live execution and migration (42 contracts).
pub fn load_production_and_compatibility_contracts() -> Result<Vec<PromptContract>, PromptError> {
    let mut contracts = Vec::with_capacity(PRODUCTION_AND_COMPATIBILITY_PROMPTS.len());
    for prompt_str in PRODUCTION_AND_COMPATIBILITY_PROMPTS {
        let contract = PromptContract::from_markdown_str(prompt_str)?;
        contracts.push(contract);
    }
    Ok(contracts)
}

/// Load and parse test fixture prompt contracts (5 contracts).
pub fn load_test_fixture_contracts() -> Result<Vec<PromptContract>, PromptError> {
    let mut contracts = Vec::with_capacity(TEST_FIXTURE_PROMPTS.len());
    for prompt_str in TEST_FIXTURE_PROMPTS {
        let contract = PromptContract::from_markdown_str(prompt_str)?;
        contracts.push(contract);
    }
    Ok(contracts)
}

/// Load and parse explicitly deprecated / metadata-only prompt contracts (11 contracts).
pub fn load_deprecated_metadata_contracts() -> Result<Vec<PromptContract>, PromptError> {
    let mut contracts = Vec::with_capacity(DEPRECATED_METADATA_PROMPTS.len());
    for prompt_str in DEPRECATED_METADATA_PROMPTS {
        let contract = PromptContract::from_markdown_str(prompt_str)?;
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
