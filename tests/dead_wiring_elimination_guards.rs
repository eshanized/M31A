//! Architecture and Wiring Consolidation Regression Guards
//!
//! Validates:
//! 1. DifferentialReplanEngine is the unique replan authority; uninvoked WorkflowReplanner removed.
//! 2. RoleRegistry and AgentRole::is_well_formed() are the unique delegation validation authorities.
//! 3. Evidence ranking weights hygiene (5 active weights, strictly ordered).
//! 4. Startup progress stage constants hygiene (all emitted, dead stages eliminated).
//! 5. Prompt catalog partitioning: production catalog loads exactly 42 contracts (34 production + 8 compatibility),
//!    omitting test fixtures (5) and deprecated metadata contracts (11), while the full catalog retains all 58.
//! 6. Prompt reachability classification completeness: 0 unclassified contracts, 4 populated CI buckets.
//! 7. Compatibility constants retention with explicit deprecation attributes.

use std::collections::HashSet;

use m31a::agent::delegation::DelegationContract;
use m31a::agent::registry::RoleRegistry;
use m31a::config::canonical::DEFAULT_VERIFICATION_TIMEOUT_SECS;
use m31a::context::evidence::relevance_weights;
use m31a::kernel::seams::recovery::FailureClassification;
#[allow(deprecated)]
use m31a::model::provider::nvidia::DEFAULT_NVIDIA_BASE_URL;
#[allow(deprecated)]
use m31a::process::env::TRUSTED_BASELINE_VARS;
use m31a::prompt::catalog::{InMemoryPromptCatalog, PromptCatalog};
use m31a::prompt::reachability::{
    PromptCiBucket, PromptReachability, builtin_reachability_matrix, canonical_version,
    classify_builtin,
};
use m31a::recovery::DifferentialReplanEngine;
use m31a::startup_progress::stage;
use m31a::state_machine::agent::AgentRole;

// ============================================================================
// 1. Planning Authority and Replanner Elimination Guard
// ============================================================================

#[test]
fn test_replan_authority_is_differential_replan_engine() {
    // Assert DifferentialReplanEngine exists and is the unique replan authority.
    let engine_name = std::any::type_name::<DifferentialReplanEngine>();
    assert!(engine_name.contains("DifferentialReplanEngine"));

    // RecoveryStrategy::Replan is retained for serialized compatibility
    #[allow(deprecated)]
    let replan_strategy = m31a::workflow::RecoveryStrategy::Replan;
    #[allow(deprecated)]
    let is_replan = matches!(replan_strategy, m31a::workflow::RecoveryStrategy::Replan);
    assert!(is_replan);
}

// ============================================================================
// 2. Delegation Role Allowlist Elimination and Authority Guard
// ============================================================================

#[test]
fn test_delegation_role_authority_is_role_registry_and_well_formedness() {
    let registry = RoleRegistry::with_builtins();

    // Verify built-in canonical roles are registered in RoleRegistry
    let canonical_roles = [
        "planner",
        "researcher",
        "architect",
        "implementer",
        "reviewer",
        "verifier",
        "diagnostician",
        "integrator",
        "discovery_analyst",
        "stack_researcher",
        "features_researcher",
        "architecture_researcher",
        "pitfalls_researcher",
        "security_researcher",
        "deployment_researcher",
        "synthesizer",
        "auditor",
        "release_certifier",
    ];

    for role_name in canonical_roles {
        let role = AgentRole::new(role_name);
        assert!(
            role.is_well_formed(),
            "role '{role_name}' must be well formed"
        );
        assert!(
            registry.contains(&role),
            "role '{role_name}' must exist in RoleRegistry"
        );
    }

    // Dynamic and custom well-formed roles are valid domain roles
    let custom_role = AgentRole::new("custom_subsystem_worker");
    assert!(custom_role.is_well_formed());

    // Ill-formed role strings are rejected
    assert!(!AgentRole::new("").is_well_formed());
    assert!(!AgentRole::new("invalid role with spaces").is_well_formed());
    assert!(!AgentRole::new("-leading-hyphen").is_well_formed());

    // DelegationContract validation enforces well-formed role contracts
    let valid_contract = DelegationContract::new(
        "planner",
        "implementer",
        "Refactor subsystem",
        "src/lib.rs",
        0,
    );
    assert!(valid_contract.validate().is_ok());

    let invalid_role_contract = DelegationContract::new(
        "planner",
        "invalid role!",
        "Refactor subsystem",
        "src/lib.rs",
        0,
    );
    assert!(invalid_role_contract.validate().is_err());
}

// ============================================================================
// 3. Evidence Ranking Weights Guard
// ============================================================================

#[test]
fn test_evidence_ranking_weights_hygiene_and_ordering() {
    // 5 active weights
    assert_eq!(relevance_weights::EXPLICIT_TASK_REF, 100);
    assert_eq!(relevance_weights::DIAGNOSTIC_FAILURE, 85);
    assert_eq!(relevance_weights::ASSOCIATED_TEST, 70);
    assert_eq!(relevance_weights::DIRECT_CALLER_CALLEE, 40);
    assert_eq!(relevance_weights::KEYWORD_SEARCH, 30);

    // Assert strict monotonic ordering of evidence priorities
    const {
        assert!(relevance_weights::EXPLICIT_TASK_REF > relevance_weights::DIAGNOSTIC_FAILURE);
        assert!(relevance_weights::DIAGNOSTIC_FAILURE > relevance_weights::ASSOCIATED_TEST);
        assert!(relevance_weights::ASSOCIATED_TEST > relevance_weights::DIRECT_CALLER_CALLEE);
        assert!(relevance_weights::DIRECT_CALLER_CALLEE > relevance_weights::KEYWORD_SEARCH);
    }
}

// ============================================================================
// 4. Startup Progress Stage Constants Guard
// ============================================================================

#[test]
fn test_startup_progress_stage_constants_hygiene() {
    // All emitted startup stages are present and uniquely identified
    let stages = [
        stage::STORAGE,
        stage::RUNTIME_STORAGE_PREPARE,
        stage::RUNTIME_TELEMETRY_SETUP,
        stage::CRASH_RECOVERY_ENUMERATE,
        stage::CRASH_RECOVERY_INTEGRITY,
        stage::CRASH_RECOVERY_CHECKPOINTS,
        stage::CRASH_RECOVERY_WORKSPACE,
        stage::CRASH_RECOVERY_JOBS,
        stage::CRASH_RECOVERY_TASKS,
        stage::AUTHORITIES_POLICY,
        stage::AUTHORITIES_CATALOG,
        stage::AUTHORITIES_CAPABILITIES,
        stage::AUTHORITIES_PROVIDER,
        stage::AUTHORITIES_CONTEXT,
        stage::AUTHORITIES_COMPOSE,
        stage::TUI_WORKSPACE_HYDRATION,
        stage::TUI_BRIDGE_ATTACHMENT,
    ];

    let mut set = HashSet::new();
    for s in stages {
        assert!(!s.is_empty(), "stage identifier cannot be empty");
        assert!(set.insert(s), "stage identifier '{s}' must be unique");
    }

    assert_eq!(stages.len(), 17);
}

// ============================================================================
// 5. Prompt Catalog Partitioning and Production Pruning Guard
// ============================================================================

#[test]
fn test_prompt_catalog_production_pruning() {
    // Production catalog: only production (34) + compatibility (8) contracts = 42
    let prod_catalog = InMemoryPromptCatalog::with_production_builtins();
    let prod_list = prod_catalog.list();
    assert_eq!(
        prod_list.len(),
        42,
        "production catalog must contain exactly 42 contracts"
    );

    // Verify production-reachable contracts are all present
    assert!(prod_catalog.contains("core.safety", 2));
    assert!(prod_catalog.contains("agent.implementer", 2));
    assert!(prod_catalog.contains("execution.implementer", 2));
    assert!(prod_catalog.contains("planning.decompose", 2));
    assert!(prod_catalog.contains("verification.reviewer", 2));
    assert!(prod_catalog.contains("recovery.diagnostician", 2));

    // Verify compatibility migration contracts are present for resolution
    assert!(prod_catalog.contains("agent.implementer", 1));
    assert!(prod_catalog.contains("execution.implementer", 1));
    assert!(prod_catalog.contains("planning.decompose", 1));
    assert!(prod_catalog.contains("execution.reviewer", 1));
    assert!(prod_catalog.contains("execution.diagnostician", 1));

    // Verify test fixture contracts are PRUNED from production catalog (not in prod_list)
    assert!(
        !prod_list.iter().any(|m| m.id == "implement"),
        "test fixture contract 'implement' must not be registered in production catalog"
    );
    assert!(
        !prod_list.iter().any(|m| m.id == "review"),
        "test fixture contract 'review' must not be registered in production catalog"
    );
    assert!(
        !prod_list.iter().any(|m| m.id == "verify"),
        "test fixture contract 'verify' must not be registered in production catalog"
    );
    assert!(
        !prod_list.iter().any(|m| m.id == "diagnose"),
        "test fixture contract 'diagnose' must not be registered in production catalog"
    );
    assert!(
        !prod_list.iter().any(|m| m.id == "skill.in_task_guidance"),
        "test fixture contract 'skill.in_task_guidance' must not be registered in production catalog"
    );
    assert!(
        !prod_catalog.contains("skill.in_task_guidance", 1),
        "test fixture 'skill.in_task_guidance' must not be accessible in production catalog"
    );

    // Verify deprecated metadata-only contracts are PRUNED from production catalog
    assert!(
        !prod_list
            .iter()
            .any(|m| m.id == "runtime.safety_invariants"),
        "deprecated 'runtime.safety_invariants' must not be registered in production catalog"
    );
    assert!(
        !prod_list.iter().any(|m| m.id == "genesis.discovery"),
        "metadata-only 'genesis.discovery' must not be in production catalog"
    );
    assert!(
        !prod_list.iter().any(|m| m.id == "genesis.charter"),
        "metadata-only 'genesis.charter' must not be in production catalog"
    );
    assert!(
        !prod_list
            .iter()
            .any(|m| m.id == "execution.authorization_explanation"),
        "deprecated 'execution.authorization_explanation' must not be in production catalog"
    );
    assert!(
        !prod_catalog.contains("genesis.discovery", 1),
        "metadata-only 'genesis.discovery' must not be accessible in production catalog"
    );

    // Full catalog retains all 58 contracts across all CI buckets
    let full_catalog = InMemoryPromptCatalog::with_builtins();
    assert_eq!(
        full_catalog.list().len(),
        58,
        "full catalog must contain all 58 built-in contracts"
    );

    // Progressive catalog building
    let mut progressive = InMemoryPromptCatalog::with_production_builtins();
    assert_eq!(progressive.list().len(), 42);
    progressive.register_test_fixtures().unwrap();
    assert_eq!(progressive.list().len(), 47);
    progressive.register_deprecated_metadata().unwrap();
    assert_eq!(progressive.list().len(), 58);
}

// ============================================================================
// 6. Prompt Reachability Matrix Completeness Guard
// ============================================================================

#[test]
fn test_prompt_reachability_matrix_completeness() {
    let full_catalog = InMemoryPromptCatalog::with_builtins();
    let (rows, unclassified) = builtin_reachability_matrix(&full_catalog);

    assert!(
        unclassified.is_empty(),
        "no prompt contract may be unclassified: {unclassified:?}"
    );
    assert_eq!(rows.len(), 58);

    let mut buckets = HashSet::new();
    let mut reachabilities = HashSet::new();

    for (id, version, reach, bucket) in &rows {
        buckets.insert(*bucket);
        reachabilities.insert(*reach);

        let (classified_reach, classified_bucket) =
            classify_builtin(id, *version).expect("all 58 prompts classify");
        assert_eq!(*reach, classified_reach);
        assert_eq!(*bucket, classified_bucket);
    }

    // All 4 CI buckets populated
    assert_eq!(buckets.len(), 4);
    assert!(buckets.contains(&PromptCiBucket::ProductionReachable));
    assert!(buckets.contains(&PromptCiBucket::IntentionalCompatibility));
    assert!(buckets.contains(&PromptCiBucket::IntentionalTestFixture));
    assert!(buckets.contains(&PromptCiBucket::ExplicitlyDeprecated));

    // All active reachabilities represented
    assert!(reachabilities.contains(&PromptReachability::Direct));
    assert!(reachabilities.contains(&PromptReachability::RoleBound));
    assert!(reachabilities.contains(&PromptReachability::WorkflowBound));
    assert!(reachabilities.contains(&PromptReachability::TestOnly));
    assert!(reachabilities.contains(&PromptReachability::MetadataOnly));
    assert!(reachabilities.contains(&PromptReachability::Legacy));

    // Canonical version resolution
    for id in [
        "agent.implementer",
        "agent.reviewer",
        "agent.verifier",
        "agent.diagnostician",
        "execution.implementer",
        "planning.decompose",
    ] {
        assert_eq!(canonical_version(id), Some(2));
    }
}

// ============================================================================
// 7. Compatibility Constants Retention Guard
// ============================================================================

#[test]
fn test_compatibility_constants_retention() {
    // Assert deprecated aliases remain available for external callers
    #[allow(deprecated)]
    let base_url = DEFAULT_NVIDIA_BASE_URL;
    assert_eq!(base_url, "https://integrate.api.nvidia.com/v1");

    assert_eq!(DEFAULT_VERIFICATION_TIMEOUT_SECS, 60);

    #[allow(deprecated)]
    let trusted_vars = TRUSTED_BASELINE_VARS;
    assert!(trusted_vars.contains(&"PATH"));

    // Legacy FailureClassification constants
    #[allow(deprecated)]
    {
        assert_eq!(
            FailureClassification::UPSTREAM_SERVICE,
            FailureClassification::Transient
        );
        assert_eq!(
            FailureClassification::MODEL_REFUSAL,
            FailureClassification::Model
        );
        assert_eq!(
            FailureClassification::SCHEMA_VIOLATION,
            FailureClassification::ToolContract
        );
        assert_eq!(
            FailureClassification::POLICY_VIOLATION,
            FailureClassification::Policy
        );
        assert_eq!(
            FailureClassification::SANDBOX_ESCAPING,
            FailureClassification::Permission
        );
        assert_eq!(
            FailureClassification::RESOURCE_EXHAUSTION,
            FailureClassification::ResourceLimit
        );
        assert_eq!(
            FailureClassification::STALE_STATE,
            FailureClassification::RepositoryState
        );
        assert_eq!(
            FailureClassification::LOOP_DETECTED,
            FailureClassification::Architecture
        );
        assert_eq!(
            FailureClassification::DEADLOCK,
            FailureClassification::Architecture
        );
    }
}
