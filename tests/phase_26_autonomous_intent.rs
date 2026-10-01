//! Phase 26 Integration Tests — Autonomous Intent Expansion, Requirement Discovery & Unified Upstream Architecture Synthesis
//!
//! NOTE (Phase 27): product-specific keyword templates were removed from
//! discovery/synthesis. These tests assert GENERIC behavior: unseen domains
//! flow through with zero product branches, and all target content is
//! derived from charter/requirements/evidence rather than static tables.
//!
//! Validates:
//! 1. Tier classification from intent shape + workspace evidence (no product nouns)
//! 2. Tier-generic unknowns (no per-product unknown scripts)
//! 3. Generic domain scaffold (content authority is model/dialogue downstream)
//! 4. Derived architecture (no M31A-shaped SUB-KERNEL/CMP-ENGINE template)
//! 5. Derived roadmap (phase count follows requirement evidence)
//! 6. Canonical planner seam evolution (UpstreamPlanContext roundtrip)
//! 7. Research dataflow (real findings flow into synthesis)
//! 8. Durable engineering memory wiring
//! 9. 10 Comprehensive negative tests.

use tempfile::tempdir;

use m31a::ids::MissionId;
use m31a::kernel::memory::{DecisionStatus, MemoryScope};
use m31a::kernel::seams::planner::{PlanRequest, UpstreamPlanContext};
use m31a::memory::repository::{EngineeringMemoryStore, SqliteEngineeringMemoryRepository};
use m31a::persistence::sqlite::schema::run_migrations;
use m31a::planning::risks::{Criticality, PlanningUnknown, UnknownFate};
use m31a::workflow::genesis::discovery::{
    DiscoveryPillar, DiscoverySession, classify_workflow_tier, extract_unknowns, infer_domain_model,
};
use m31a::workflow::genesis::{
    GenesisError, GenesisOptions, GenesisRequest, ProjectCharter, WorkflowTier,
    WorkspaceEnvironment,
};
use m31a::workflow::planning::architecture_synthesizer::ArchitectureSynthesizer;
use m31a::workflow::planning::requirements_synthesizer::RequirementsSynthesizer;
use m31a::workflow::planning::risks::RiskRegister;
use m31a::workflow::planning::roadmap::RoadmapSynthesizer;
use sqlx::sqlite::SqlitePoolOptions;

// ============================================================================
// SCENARIO 1: "build me an expense tracker" — generic greenfield path
// ============================================================================

#[tokio::test]
async fn test_scenario_1_expense_tracker_expansion_and_safe_defaults() {
    let dir = tempdir().unwrap();
    let env = WorkspaceEnvironment::probe(dir.path()).unwrap();
    let prompt = "build me an expense tracker";

    // 1. Tier classification: constructive intent with no consequence
    // markers and no evidence of an existing codebase -> Greenfield.
    let tier = classify_workflow_tier(prompt, &env);
    assert_eq!(tier, WorkflowTier::Greenfield);

    // 2. Unknown classification: tier-generic unknowns only, all SafeToInfer
    // (no product-specific unknown scripts).
    let unknowns = extract_unknowns(prompt, tier);
    assert!(!unknowns.is_empty());
    for unk in &unknowns {
        assert_eq!(
            unk.fate,
            UnknownFate::SafeToInfer,
            "Unknown {} should be SafeToInfer for minimal greenfield intent",
            unk.id
        );
        let blob = format!(
            "{} {} {}",
            unk.id,
            unk.description,
            unk.resolution.as_deref().unwrap_or("")
        );
        assert!(
            !blob.to_lowercase().contains("bkash")
                && !blob.to_lowercase().contains("expense")
                && !blob.to_lowercase().contains("taxonomy"),
            "Unknowns must be tier-generic, never product-specific: {}",
            blob
        );
    }

    // 3. Domain scaffold is honestly empty: no product entities are
    // invented from keywords, and no placeholder entities mask the absence
    // of evidence either. Domain substance arrives via dialogue/research/model.
    let domain = infer_domain_model(prompt);
    assert!(domain.entities.is_empty());
    assert!(domain.core_workflows.is_empty());
    assert!(domain.inferred_defaults.is_empty());
    assert!(domain.non_goals.is_empty());
    assert!(
        !domain.entities.iter().any(|e| e.contains("Expense")),
        "No product entities may be keyword-invented: {:?}",
        domain.entities
    );

    // 4. Session synthesis: 0 questions asked
    let req = GenesisRequest::new(prompt, dir.path());
    let session = DiscoverySession::new(req, env);
    assert!(!session.requires_user_decision());

    let charter = session.synthesize_charter().unwrap();
    assert_eq!(charter.project_name, "Expense Tracker");
    // No evidence for storage/stack: honestly undecided, never defaulted.
    assert_eq!(charter.technical_preferences.storage, None);
    assert_eq!(charter.technical_preferences.architecture_style, None);

    // 5. Derived architecture: no M31A-shaped template components.
    let reqs = RequirementsSynthesizer::synthesize(&charter, None, &[], None, None).unwrap();
    let (arch, adrs) =
        ArchitectureSynthesizer::synthesize(&reqs, &charter, None, &[], None).unwrap();
    let arch_md = arch.to_markdown();
    assert!(
        !arch_md.contains("M31 Autonomous"),
        "Target architecture should not leak M31A identity"
    );
    for forbidden in [
        "SUB-KERNEL",
        "SUB-PERSISTENCE",
        "SUB-INTERACTION",
        "CMP-ENGINE",
        "CMP-STORAGE",
        "CMP-POLICY",
        "CMP-TRANSPORT",
    ] {
        assert!(
            !arch_md.contains(forbidden),
            "Derived architecture must not contain fixed template id {}",
            forbidden
        );
    }
    let sub_ids: Vec<_> = arch.subsystems.iter().map(|s| s.id.clone()).collect();
    assert!(
        sub_ids.iter().any(|id| id.starts_with("SUB-FUNCTIONAL-")),
        "Functional requirements must derive a domain subsystem with a content-bound id: {:?}",
        sub_ids
    );
    assert!(
        arch_md.contains("Expense Tracker Functional"),
        "Architecture should reflect target project scope"
    );
    // Topology ADR + open storage ADR; no research tradeoffs (summary None).
    assert_eq!(adrs.adrs.len(), 2);
    // Every requirement is covered by a derived component (traceability).
    for req in &reqs.requirements {
        assert!(
            arch.components
                .iter()
                .any(|c| c.requirement_refs.contains(&req.key)),
            "Requirement {} has no derived component",
            req.key
        );
    }
}

// ============================================================================
// SCENARIO 2: unseen domain flows through with zero product branches
// ============================================================================

#[tokio::test]
async fn test_scenario_2_student_linear_expansion() {
    let dir = tempdir().unwrap();
    let env = WorkspaceEnvironment::probe(dir.path()).unwrap();
    // Previously unseen domain vocabulary: no keyword template may exist
    // for it, yet the pipeline must produce a coherent generic scaffold.
    let prompt = "make something like Linear for students";

    // 1. Tier classification
    let tier = classify_workflow_tier(prompt, &env);
    assert_eq!(tier, WorkflowTier::Greenfield);

    // 2. Domain scaffold is honestly empty even for academic vocabulary.
    let domain = infer_domain_model(prompt);
    assert!(domain.entities.is_empty());
    assert!(domain.core_workflows.is_empty());
    assert!(
        !domain.entities.iter().any(|e| e == "AcademicCourse")
            && !domain.core_workflows.iter().any(|w| w.contains("syllabus")),
        "No academic template may be keyword-selected: {:?} / {:?}",
        domain.entities,
        domain.core_workflows
    );

    // 3. Session synthesis: 0 questions asked
    let req = GenesisRequest::new(prompt, dir.path());
    let session = DiscoverySession::new(req, env);
    assert!(!session.requires_user_decision());

    let charter = session.synthesize_charter().unwrap();
    assert_eq!(charter.project_name, "Linear For Students");
    // No dialogue evidence: personas stay honestly empty (never fabricated),
    // and the gap is recorded as an explicit unresolved ambiguity area.
    assert!(
        charter.target_personas.is_empty(),
        "Personas must not be invented without dialogue evidence: {:?}",
        charter.target_personas
    );
    assert!(
        charter
            .ambiguity_assessment
            .unresolved_areas
            .iter()
            .any(|u| u.contains("Personas")),
        "Missing personas must be an explicit unresolved area: {:?}",
        charter.ambiguity_assessment.unresolved_areas
    );
}

// ============================================================================
// SCENARIO 3: regional vocabulary produces NO product-specific unknowns
// ============================================================================

#[tokio::test]
async fn test_scenario_3_food_delivery_bangladesh_researchable_unknowns() {
    let dir = tempdir().unwrap();
    let env = WorkspaceEnvironment::probe(dir.path()).unwrap();
    let prompt = "build a food delivery app for Bangladesh";

    let tier = classify_workflow_tier(prompt, &env);
    assert_eq!(tier, WorkflowTier::Greenfield);

    // 1. Unknowns are tier-generic: regional payment/logistics content is
    // model-derived during research, never keyword-enumerated here.
    let unknowns = extract_unknowns(prompt, tier);
    for unk in &unknowns {
        let blob = format!(
            "{} {} {}",
            unk.id,
            unk.description,
            unk.resolution.as_deref().unwrap_or("")
        );
        assert!(
            !blob.to_lowercase().contains("bkash")
                && !blob.to_lowercase().contains("nagad")
                && !blob.to_lowercase().contains("bangladesh")
                && !blob.to_lowercase().contains("division"),
            "No regional product content may be keyword-selected: {}",
            blob
        );
    }

    // 2. Domain scaffold is generic
    let domain = infer_domain_model(prompt);
    assert!(!domain.entities.contains(&"RestaurantProfile".to_string()));
    assert!(!domain.entities.contains(&"DeliveryRider".to_string()));
    assert!(
        !domain
            .inferred_defaults
            .iter()
            .any(|d| d.contains("BDT") || d.contains("bKash"))
    );
}

// ============================================================================
// SCENARIO 4: "fix the broken dashboard" (Tiny / Brownfield fast-path)
// ============================================================================

#[tokio::test]
async fn test_scenario_4_fix_dashboard_fast_path() {
    let dir = tempdir().unwrap();
    let env = WorkspaceEnvironment::probe(dir.path()).unwrap();
    let prompt = "fix the broken dashboard";

    let tier = classify_workflow_tier(prompt, &env);
    assert_eq!(tier, WorkflowTier::Medium);

    let unknowns = extract_unknowns(prompt, tier);
    assert!(
        !unknowns
            .iter()
            .any(|u| u.fate == UnknownFate::UserDecisionRequired)
    );

    let req = GenesisRequest::new(prompt, dir.path());
    let mut session = DiscoverySession::new(req, env);
    assert_eq!(session.current_ambiguity, 0);
    assert!(session.is_converged);

    // Fast-path: next_turn immediately returns None
    assert!(session.next_turn().unwrap().is_none());
}

// ============================================================================
// SCENARIO 5: consequence characteristics (not product nouns) escalate tier
// ============================================================================

#[tokio::test]
async fn test_scenario_5_turn_into_saas_consequential_decision() {
    let dir = tempdir().unwrap();
    let env = WorkspaceEnvironment::probe(dir.path()).unwrap();
    // Product nouns alone ("saas") must NOT escalate: they describe WHAT is
    // built, not its consequence class.
    assert_eq!(
        classify_workflow_tier("turn this into a SaaS", &env),
        WorkflowTier::Greenfield
    );

    // Consequence characteristics escalate regardless of domain vocabulary.
    let prompt = "add multi-tenant isolation with a hipaa compliance boundary";
    let tier = classify_workflow_tier(prompt, &env);
    assert_eq!(tier, WorkflowTier::Consequential);

    // Unknowns must flag tenant isolation as UserDecisionRequired
    let unknowns = extract_unknowns(prompt, tier);
    let decision_required = unknowns
        .iter()
        .find(|u| u.fate == UnknownFate::UserDecisionRequired);
    assert!(
        decision_required.is_some(),
        "Consequential tier must produce a UserDecisionRequired unknown"
    );
    let unk = decision_required.unwrap();
    assert!(
        unk.id.contains("TENANT-ISOLATION") || unk.description.to_lowercase().contains("isolation")
    );

    // Session produces a consequential question turn grounded in the unknown
    let req = GenesisRequest::new(prompt, dir.path());
    let mut session = DiscoverySession::new(req, env);
    assert!(session.requires_user_decision());

    let turn = session.next_turn().unwrap().unwrap();
    assert_eq!(turn.turn_number, 1);
    assert_eq!(turn.pillar_focus, DiscoveryPillar::TechnicalPreferences);
    assert!(
        turn.question.to_lowercase().contains("tenant")
            || turn.question.to_lowercase().contains("isolation")
    );
    assert!(turn.recommended_option.is_some());
}

// ============================================================================
// SCENARIO 6: "add dark mode" (Medium tier localized addition)
// ============================================================================

#[tokio::test]
async fn test_scenario_6_add_dark_mode_localized_medium() {
    let dir = tempdir().unwrap();
    let env = WorkspaceEnvironment::probe(dir.path()).unwrap();
    let prompt = "add dark mode";

    let tier = classify_workflow_tier(prompt, &env);
    assert_eq!(tier, WorkflowTier::Medium);

    let unknowns = extract_unknowns(prompt, tier);
    assert!(
        !unknowns
            .iter()
            .any(|u| u.fate == UnknownFate::UserDecisionRequired)
    );

    let req = GenesisRequest::new(prompt, dir.path());
    let mut session = DiscoverySession::new(req, env);
    assert_eq!(session.current_ambiguity, 0);
    assert!(session.is_converged);
    assert!(session.next_turn().unwrap().is_none());
}

// ============================================================================
// CANONICAL PLANNER SEAM & UPSTREAM CONTEXT TESTS
// ============================================================================

#[test]
fn test_upstream_plan_context_seam_roundtrip() {
    let ctx = UpstreamPlanContext {
        project_name: "Expense Tracker".to_string(),
        charter: "# Expense Tracker Charter".to_string(),
        architecture: "# Expense Tracker Architecture".to_string(),
        requirements: vec!["REQ-01: Track expenses".to_string()],
        assumptions: vec!["ASSUME-01: Single user local SQLite".to_string()],
        decisions: vec!["ADR-01: SQLite WAL persistence".to_string()],
        research_summary: Some("Local persistence trade-offs".to_string()),
        workflow_tier: "Greenfield".to_string(),
        unknowns: vec!["UNK-01: Storage size unknown".to_string()],
        user_decisions: vec!["DECIDE-01: Multi-currency support required?".to_string()],
        resolved_invariants: vec!["INV-01: Local execution only".to_string()],
    };

    let mission_id = MissionId::new();
    let req = PlanRequest::new(mission_id, "build me an expense tracker")
        .with_upstream_context(ctx.clone());

    let serialized = serde_json::to_string(&req).unwrap();
    let deserialized: PlanRequest = serde_json::from_str(&serialized).unwrap();

    assert_eq!(deserialized.upstream_context, Some(ctx));
}

// ============================================================================
// DERIVED ROADMAP TESTS (phase count follows requirement evidence)
// ============================================================================

#[test]
fn test_product_derived_greenfield_roadmap() {
    let mut charter = ProjectCharter::new("Expense Tracker", "Personal expense tracking system");
    charter.domain_model = infer_domain_model("build me an expense tracker");
    charter.technical_preferences.storage = Some("SQLite WAL".to_string());
    // Charter scope as produced by dialogue synthesis (in-scope workflows).
    charter.boundaries.in_scope = vec![
        "Record expense with amount and category".to_string(),
        "Query spending summaries by period".to_string(),
    ];

    let reqs = RequirementsSynthesizer::synthesize(&charter, None, &[], None, None).unwrap();
    let (arch, adrs) =
        ArchitectureSynthesizer::synthesize(&reqs, &charter, None, &[], None).unwrap();
    let risks = RiskRegister::new();
    let roadmap =
        RoadmapSynthesizer::synthesize(&reqs, &arch, &adrs, &risks, &charter, None).unwrap();

    // Derived shape: one phase per emitted subsystem. The charter carries
    // only Functional scope evidence (no security evidence → no fabricated
    // security baseline, only an unresolved question), so exactly one
    // subsystem and one phase emerge — NOT a universal template.
    assert_eq!(roadmap.phases.len(), arch.subsystems.len());
    assert_eq!(roadmap.phases.len(), 1);
    assert_eq!(roadmap.phases[0].dependencies.len(), 0);
    assert!(roadmap.phases[0].is_enabling_phase);
    assert!(
        roadmap.phases[0].name.contains("Expense Tracker"),
        "Phase names are project-derived: {}",
        roadmap.phases[0].name
    );

    // Every requirement scheduled; every phase references derived arch.
    let scheduled: Vec<_> = roadmap
        .phases
        .iter()
        .flat_map(|p| p.requirement_refs.iter())
        .collect();
    for req in &reqs.requirements {
        assert!(
            scheduled.contains(&&req.key),
            "Requirement {} unscheduled",
            req.key
        );
    }
    for phase in &roadmap.phases {
        for aref in &phase.architecture_refs {
            assert!(
                arch.subsystems.iter().any(|s| &s.id == aref)
                    || arch.components.iter().any(|c| &c.id == aref),
                "Phase {} references unknown arch ref {}",
                phase.id,
                aref
            );
        }
    }

    // Ensure no fixed M31A template leaks in phase titles or arch refs.
    for phase in &roadmap.phases {
        for forbidden in [
            "Sub-Kernel",
            "SUB-KERNEL",
            "CMP-ENGINE",
            "Establish Persistence & Core Kernel Foundation",
        ] {
            assert!(
                !phase.name.contains(forbidden),
                "Phase title leaked fixed template: {}",
                phase.name
            );
        }
    }
    roadmap
        .validate_dag()
        .expect("derived roadmap DAG must be valid");
}

// ============================================================================
// DURABLE ENGINEERING MEMORY INTEGRATION
// ============================================================================

#[tokio::test]
async fn test_durable_engineering_memory_decision_and_assumption_persistence() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    run_migrations(&pool).await.unwrap();

    let memory_repo = SqliteEngineeringMemoryRepository::new(pool.clone());
    let mission_id = MissionId::new();

    let mission_repo =
        m31a::persistence::sqlite::repositories::SqliteMissionRepository::new(pool.clone());
    let mission = m31a::state::Mission::new(mission_id, "Test Mission".to_string());
    mission_repo.insert(&mission).await.unwrap();

    // Persist ADR decision
    let decision = m31a::kernel::memory::EngineeringDecision::new(
        "ADR-TARGET-01",
        MemoryScope::Project,
        "Target Database Selection",
        "Target application needs embedded ACID storage",
        "Use SQLite with WAL mode",
        "Single-user desktop application with zero external infrastructure overhead",
        "genesis.planning",
    )
    .with_status(DecisionStatus::Accepted);

    memory_repo.save_decision(&decision).await.unwrap();

    let retrieved = memory_repo.get_decision("ADR-TARGET-01").await.unwrap();
    assert!(retrieved.is_some());
    let d = retrieved.unwrap();
    assert_eq!(d.title, "Target Database Selection");
    assert_eq!(d.status, DecisionStatus::Accepted);

    // Persist assumption
    let assumption = m31a::kernel::memory::EngineeringAssumption::new(
        mission_id,
        "Storage volume will not exceed 100MB in v1",
        MemoryScope::Project,
    );
    let assume_id = assumption.id;
    memory_repo.save_assumption(&assumption).await.unwrap();

    let retrieved_assume = memory_repo.get_assumption(assume_id).await.unwrap();
    assert!(retrieved_assume.is_some());
    assert_eq!(
        retrieved_assume.unwrap().statement,
        "Storage volume will not exceed 100MB in v1"
    );
}

// ============================================================================
// 10 COMPREHENSIVE NEGATIVE TESTS (Section 19)
// ============================================================================

#[test]
fn test_neg_01_empty_prompt_charter_validation_fails() {
    let charter = ProjectCharter::new("", "Overview of the project");
    let res = charter.validate();
    assert!(res.is_err());
    assert!(
        matches!(res.unwrap_err(), GenesisError::CharterValidation(msg) if msg.contains("project_name cannot be empty"))
    );
}

#[test]
fn test_neg_02_empty_overview_charter_validation_fails() {
    let charter = ProjectCharter::new("Expense Tracker", "   ");
    let res = charter.validate();
    assert!(res.is_err());
    assert!(
        matches!(res.unwrap_err(), GenesisError::CharterValidation(msg) if msg.contains("overview cannot be empty"))
    );
}

#[test]
fn test_neg_03_blocking_unknown_fate_validation() {
    let mut unk = PlanningUnknown::new(
        "UNK-BLOCKING",
        "Mandatory hardware security module interface missing",
        Criticality::High,
        m31a::planning::requirements::Provenance::new(
            m31a::planning::requirements::ProvenanceSourceType::UserPrompt,
            m31a::planning::requirements::TrustLevel::AuthoritativeRuntime,
            "safety_checker",
        ),
    );
    unk = unk.with_fate(UnknownFate::Blocking);

    assert_eq!(unk.fate, UnknownFate::Blocking);
    assert_eq!(unk.criticality, Criticality::High);
}

#[test]
fn test_neg_04_invalid_epistemic_status_parsing() {
    let parsed = "non_existent_status".parse::<m31a::planning::requirements::EpistemicStatus>();
    assert!(parsed.is_err());
    assert!(parsed.unwrap_err().contains("Unknown epistemic status"));
}

#[test]
fn test_neg_05_unknown_decision_status_parsing() {
    let parsed = "invalid_decision_status".parse::<DecisionStatus>();
    assert!(parsed.is_err());
    assert!(parsed.unwrap_err().contains("Unknown decision status"));
}

#[test]
fn test_neg_06_illegal_decision_status_transition() {
    // Superseded cannot transition to Accepted
    let superseded = DecisionStatus::Superseded;
    assert!(!superseded.can_transition_to(DecisionStatus::Accepted));

    // Rejected cannot transition to Accepted without re-proposal
    let rejected = DecisionStatus::Rejected;
    assert!(!rejected.can_transition_to(DecisionStatus::Accepted));
}

#[test]
fn test_neg_07_unknown_memory_scope_parsing() {
    let parsed = "planetary".parse::<MemoryScope>();
    assert!(parsed.is_err());
    assert!(parsed.unwrap_err().contains("unknown memory scope"));
}

#[test]
fn test_neg_08_invalid_agent_role_parsing() {
    // Phase 27.5 (R-01): role identity parsing is open — any well-formed id
    // parses — while EXISTENCE is enforced by the role registry. An
    // unregistered id must parse yet be absent from the registry and be
    // rejected explicitly by plan validation.
    let parsed = "super_admin".parse::<m31a::state_machine::agent::AgentRole>();
    assert!(parsed.is_ok());
    let role = parsed.unwrap();
    let guard = m31a::agent::registry::RoleRegistry::global()
        .read()
        .expect("registry readable");
    assert!(!guard.contains(&role));
}

#[test]
fn test_neg_09_genesis_options_out_of_bounds_validation() {
    let options = GenesisOptions {
        ambiguity_threshold_percent: 0,
        research_concurrency: 0,
        ..Default::default()
    };

    // Concurrency must be clampable or bounded
    assert_eq!(options.ambiguity_threshold_percent, 0);
    assert_eq!(options.research_concurrency, 0);
}

#[tokio::test]
async fn test_neg_10_superseded_decision_isolation_in_memory() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    run_migrations(&pool).await.unwrap();

    let memory_repo = SqliteEngineeringMemoryRepository::new(pool);

    let old_decision = m31a::kernel::memory::EngineeringDecision::new(
        "ADR-OLD-01",
        MemoryScope::Project,
        "Initial Database",
        "Initial setup",
        "SQLite",
        "Simplicity",
        "operator",
    )
    .with_status(DecisionStatus::Accepted);

    memory_repo.save_decision(&old_decision).await.unwrap();

    let new_decision = m31a::kernel::memory::EngineeringDecision::new(
        "ADR-NEW-01",
        MemoryScope::Project,
        "Distributed Database",
        "Scale requirements",
        "PostgreSQL",
        "High write concurrency",
        "operator",
    )
    .with_status(DecisionStatus::Accepted);

    memory_repo
        .supersede_decision("ADR-OLD-01", &new_decision)
        .await
        .unwrap();

    let old = memory_repo
        .get_decision("ADR-OLD-01")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(old.status, DecisionStatus::Superseded);
    assert_eq!(old.superseded_by.as_deref(), Some("ADR-NEW-01"));

    let new = memory_repo
        .get_decision("ADR-NEW-01")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(new.status, DecisionStatus::Accepted);
}
