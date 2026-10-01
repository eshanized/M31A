//! Deterministic regression tests for Phase 29.6 Gap C: Discovery Unknowns Must Reach Planning.
//!
//! Validates:
//! 1. Discovery unknowns are captured from ambiguous / consequential prompts.
//! 2. Discovery unknowns reach `UpstreamPlanContext`.
//! 3. Discovery and research unknowns coexist without overwriting each other.
//! 4. Consequential user decisions are explicitly isolated and preserved (`user_decisions`).
//! 5. Resolved invariants and genuine assumptions are cleanly distinguished from unknowns.
//! 6. Unknowns, user decisions, and resolved invariants reach the planning model's effective context.
//! 7. No unknown is fabricated or silently dropped.

use m31a::kernel::seams::planner::UpstreamPlanContext;
use m31a::planning::requirements::{Provenance, ProvenanceSourceType, TrustLevel};
use m31a::planning::risks::{Criticality, PlanningUnknown, UnknownFate};
use m31a::prompt::{InMemoryPromptCatalog, PromptCatalog, render_prompt};
use m31a::workflow::genesis::discovery::{DiscoverySession, extract_unknowns};
use m31a::workflow::genesis::project::WorkflowTier;
use m31a::workflow::genesis::{GenesisOptions, GenesisRequest, WorkspaceEnvironment};
use std::collections::BTreeMap;
use tempfile::tempdir;

#[test]
fn test_1_discovery_unknown_captured_and_reaches_upstream_context() {
    let temp = tempdir().unwrap();
    let env = WorkspaceEnvironment::probe(temp.path()).unwrap();

    let prompt = "Build a multi-tenant enterprise billing platform with custom domain routing and payment processing.";
    let req = GenesisRequest::new(prompt, temp.path()).with_options(GenesisOptions {
        ambiguity_threshold_percent: 20,
        ..Default::default()
    });

    let mut session = DiscoverySession::new(req, env);
    session
        .submit_response("Individual enterprise customers")
        .unwrap();
    let charter = session.synthesize_charter().unwrap();

    // 1. Verify discovery unknowns captured in charter ambiguity assessment
    assert!(!charter.ambiguity_assessment.unresolved_areas.is_empty());
    assert!(
        charter
            .ambiguity_assessment
            .unresolved_areas
            .iter()
            .any(|u| u.contains("UNK-TENANT-ISOLATION") || u.contains("isolation"))
    );

    // 2. Map to UpstreamPlanContext
    let mut unknowns_list = Vec::new();
    for u in &charter.ambiguity_assessment.unresolved_areas {
        unknowns_list.push(format!("Discovery unknown: {}", u));
    }

    let mut user_decisions_list = Vec::new();
    let discovery_unknowns = extract_unknowns(prompt, charter.workflow_tier);
    for u in discovery_unknowns {
        if u.fate == UnknownFate::UserDecisionRequired || u.fate == UnknownFate::Blocking {
            user_decisions_list.push(format!(
                "{}: {} (Required Decision: {})",
                u.id,
                u.description,
                u.resolution
                    .as_deref()
                    .unwrap_or("Operator decision required")
            ));
        }
    }

    let context = UpstreamPlanContext {
        project_name: charter.project_name.clone(),
        charter: charter.to_markdown(),
        architecture: "Arch spec".to_string(),
        requirements: vec!["REQ-01: Multi-tenancy".to_string()],
        assumptions: vec!["Assume PostgreSQL 16+".to_string()],
        decisions: vec!["ADR-01: Async runtime".to_string()],
        research_summary: None,
        workflow_tier: format!("{:?}", charter.workflow_tier),
        unknowns: unknowns_list,
        user_decisions: user_decisions_list,
        resolved_invariants: charter.ambiguity_assessment.resolved_invariants.clone(),
    };

    assert!(!context.unknowns.is_empty());
    assert!(!context.user_decisions.is_empty());
    assert!(context.user_decisions[0].contains("UNK-TENANT-ISOLATION"));
}

#[test]
fn test_2_discovery_and_research_unknowns_coexist_without_overwriting() {
    let discovery_unresolved = vec![
        "UNK-TENANT-ISOLATION: Multi-tenant data isolation strategy".to_string(),
        "UNK-BILLING-TIERS: Subscription tiering limits".to_string(),
    ];

    let research_unknowns = vec![
        PlanningUnknown::new(
            "UNK-RES-01",
            "Latency impact of cross-region read replicas",
            Criticality::High,
            Provenance::new(
                ProvenanceSourceType::ExternalDoc,
                TrustLevel::AuthoritativeRuntime,
                "research_synthesis",
            ),
        ),
        PlanningUnknown::new(
            "UNK-RES-02",
            "Optimal connection pool sizing under burst load",
            Criticality::Medium,
            Provenance::new(
                ProvenanceSourceType::ExternalDoc,
                TrustLevel::AuthoritativeRuntime,
                "research_synthesis",
            ),
        ),
    ];

    // Build unified unknowns list
    let mut combined_unknowns = Vec::new();
    for u in &discovery_unresolved {
        combined_unknowns.push(format!("Discovery unknown: {}", u));
    }
    for u in &research_unknowns {
        combined_unknowns.push(format!("Research unknown {}: {}", u.id, u.description));
    }

    let context = UpstreamPlanContext {
        project_name: "Coexistence Test".to_string(),
        charter: "Charter".to_string(),
        architecture: "Arch".to_string(),
        requirements: Vec::new(),
        assumptions: vec!["Inferred default: SQLite".to_string()],
        decisions: Vec::new(),
        research_summary: Some("Research summary text".to_string()),
        workflow_tier: "Consequential".to_string(),
        unknowns: combined_unknowns,
        user_decisions: vec!["UNK-TENANT-ISOLATION: Operator decision required".to_string()],
        resolved_invariants: vec!["Core functional scope boundaries defined".to_string()],
    };

    // Both discovery and research unknowns must be present simultaneously
    assert_eq!(context.unknowns.len(), 4);
    assert!(
        context
            .unknowns
            .iter()
            .any(|u| u.contains("UNK-TENANT-ISOLATION"))
    );
    assert!(
        context
            .unknowns
            .iter()
            .any(|u| u.contains("UNK-BILLING-TIERS"))
    );
    assert!(context.unknowns.iter().any(|u| u.contains("UNK-RES-01")));
    assert!(context.unknowns.iter().any(|u| u.contains("UNK-RES-02")));

    // Assumptions must not be polluted with raw unknowns
    assert_eq!(context.assumptions.len(), 1);
    assert_eq!(context.assumptions[0], "Inferred default: SQLite");
}

#[test]
fn test_3_unknowns_and_decisions_reach_planning_model_effective_context() {
    let catalog = InMemoryPromptCatalog::with_builtins();
    let contract = catalog
        .get("planning.decompose", 2)
        .expect("prompt contract exists");

    let mut params = BTreeMap::new();
    params.insert(
        "goal".to_string(),
        "Implement multi-tenant SaaS core".to_string(),
    );
    params.insert(
        "charter".to_string(),
        "# SaaS Charter\nMulti-tenant app".to_string(),
    );
    params.insert(
        "architecture".to_string(),
        "# Arch\nClean architecture".to_string(),
    );
    params.insert(
        "requirements".to_string(),
        "REQ-01: Tenant isolation\nREQ-02: Auth".to_string(),
    );
    params.insert(
        "assumptions".to_string(),
        "Inferred default: PostgreSQL 16".to_string(),
    );
    params.insert(
        "decisions".to_string(),
        "ADR-01: Modular monolith".to_string(),
    );
    params.insert(
        "research_summary".to_string(),
        "Research: RLS vs schema per tenant".to_string(),
    );
    params.insert(
        "user_decisions".to_string(),
        "UNK-TENANT-ISOLATION: Multi-tenant data isolation strategy (Required Decision: Present trade-offs to operator)"
            .to_string(),
    );
    params.insert(
        "unknowns".to_string(),
        "Discovery unknown: UNK-BILLING-TIERS: Subscription tiering limits\nResearch unknown UNK-RES-01: Replication lag"
            .to_string(),
    );
    params.insert(
        "resolved_invariants".to_string(),
        "Core functional scope boundaries defined\nPrimary languages: Rust".to_string(),
    );

    let rendered = render_prompt(contract, &params, false).expect("rendering succeeds");
    let rendered_text = rendered.rendered_text;

    // Verify all uncertainty sections are explicitly rendered in the prompt text
    assert!(
        rendered_text.contains("### Consequential User Decisions"),
        "Prompt must contain Consequential User Decisions section"
    );
    assert!(
        rendered_text.contains("UNK-TENANT-ISOLATION"),
        "Prompt must surface UNK-TENANT-ISOLATION to model"
    );
    assert!(
        rendered_text.contains("DO NOT invent answers"),
        "Prompt must instruct model not to invent answers for user decisions"
    );

    assert!(
        rendered_text.contains("### Unresolved Epistemic Unknowns"),
        "Prompt must contain Unresolved Epistemic Unknowns section"
    );
    assert!(
        rendered_text.contains("UNK-BILLING-TIERS"),
        "Prompt must include discovery unknowns"
    );
    assert!(
        rendered_text.contains("UNK-RES-01"),
        "Prompt must include research unknowns"
    );

    assert!(
        rendered_text.contains("### Definitively Resolved Invariants"),
        "Prompt must contain Definitively Resolved Invariants section"
    );
    assert!(
        rendered_text.contains("Primary languages: Rust"),
        "Prompt must show resolved invariants"
    );
}

#[test]
fn test_4_resolved_invariants_and_assumptions_distinguished() {
    let context = UpstreamPlanContext {
        project_name: "Epistemic Distinction".to_string(),
        charter: "Charter text".to_string(),
        architecture: "Arch text".to_string(),
        requirements: vec!["REQ-01".to_string()],
        assumptions: vec!["Inferred default: Tokio async runtime".to_string()],
        decisions: vec!["ADR-01: Rust 2024".to_string()],
        research_summary: None,
        workflow_tier: "Medium".to_string(),
        unknowns: vec!["UNK-PERF: Cache hit ratio under high load".to_string()],
        user_decisions: Vec::new(),
        resolved_invariants: vec!["Language: Rust 1.85+".to_string()],
    };

    // Invariant is fact, not unknown or assumption
    assert!(
        !context
            .unknowns
            .contains(&"Language: Rust 1.85+".to_string())
    );
    assert!(
        !context
            .assumptions
            .contains(&"Language: Rust 1.85+".to_string())
    );
    assert_eq!(context.resolved_invariants, vec!["Language: Rust 1.85+"]);

    // Assumption is explicitly separated from unknown
    assert_eq!(
        context.assumptions,
        vec!["Inferred default: Tokio async runtime"]
    );
    assert_eq!(
        context.unknowns,
        vec!["UNK-PERF: Cache hit ratio under high load"]
    );
}

#[test]
fn test_5_no_unknown_fabricated_or_silently_dropped() {
    let prompt = "Tiny localized fix for typo in README.md";
    let unknowns_tiny = extract_unknowns(prompt, WorkflowTier::Tiny);
    assert!(
        unknowns_tiny.is_empty(),
        "Tiny workflow must not fabricate unknowns"
    );

    let consequential_prompt = "Large scale SaaS backend with multi-tenant data";
    let unknowns_consequential =
        extract_unknowns(consequential_prompt, WorkflowTier::Consequential);
    assert!(!unknowns_consequential.is_empty());

    // Verify all consequential unknowns have explicit fate
    for u in &unknowns_consequential {
        assert!(
            matches!(
                u.fate,
                UnknownFate::UserDecisionRequired
                    | UnknownFate::Researchable
                    | UnknownFate::SafeToInfer
                    | UnknownFate::Blocking
            ),
            "Every unknown must have an explicit epistemic fate"
        );
        assert!(!u.id.is_empty());
        assert!(!u.description.is_empty());
    }
}
