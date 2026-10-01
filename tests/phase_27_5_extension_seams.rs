//! Phase 27.5 — Open Extension Seams & Remove Remaining Fabrication.
//!
//! Proves, through production entry points (never registry-only unit code):
//! - Test A: a synthetic role registers and flows through plan validation,
//!   dispatch, profile materialization, and verification-default mapping.
//! - Test B: a synthetic research dimension registers and flows through
//!   research compilation, collection, synthesis, and requirements without
//!   touching generic orchestration.
//! - Test C: two target inputs yield distinct target-derived architecture
//!   identities (deterministic per input); no universal table is injected.
//! - Test D: two brownfield delta shapes yield different phase structures;
//!   the roadmap is never a fixed three-phase template.
//! - Test E: absent evidence produces no fabricated requirements, domain
//!   models, findings, or architecture facts.
//! - Test F: derived artifacts preserve epistemic/provenance information.
//! - Test G: invalid definitions fail explicitly, never silently fall back.
//! - Test H: registered roles cannot grant themselves unauthorized
//!   capabilities; ceilings and policy independence hold.
//! - Test I: a complete production pipeline exercises the new mechanisms.

use async_trait::async_trait;
use std::collections::BTreeMap;
use std::sync::Arc;
use tempfile::tempdir;

use m31a::agent::model_policy::{ModelCaller, ModelProposal};
use m31a::agent::registry::{RoleDefinition, RoleError, RoleRegistry};
use m31a::agent::{
    AgentProfile, CapabilityEnvelope, ContextPolicy, ModelPolicy, TerminationPolicy,
};
use m31a::kernel::plan::{CapabilityAccessMode, CapabilityRequirement, VerificationStrategy};
use m31a::kernel::seams::planner::{PlanRequest, PlanService};
use m31a::planning::requirements::{EpistemicStatus, RequirementCategory};
use m31a::planning::service::PlanServiceImpl;
use m31a::planning::validation::PlanValidator;
use m31a::prompt::{MissionStage, PromptReference};
use m31a::state_machine::agent::AgentRole;
use m31a::workflow::genesis::dimension_registry::{
    DimensionError, ResearchDimensionDefinition, ResearchDimensionRegistry,
};
use m31a::workflow::genesis::provenance::ResearchFinding;
use m31a::workflow::genesis::research_decision::{
    ResearchDecision, ResearchDimension, evaluate_research_decision,
};
use m31a::workflow::genesis::researcher::ResearchOrchestrator;
use m31a::workflow::genesis::synthesis::ResearchSynthesizer;
use m31a::workflow::genesis::{
    BrownfieldMap, CodebaseTopology, GenesisOptions, ProjectCharter, WorkspaceEnvironment,
};
use m31a::workflow::planning::architecture_synthesizer::ArchitectureSynthesizer;
use m31a::workflow::planning::requirements_synthesizer::RequirementsSynthesizer;
use m31a::workflow::planning::risks::RiskRegister;
use m31a::workflow::planning::roadmap::RoadmapSynthesizer;

// ============================================================================
// Shared synthetic-extension fixtures (never compiled into production)
// ============================================================================

fn test_role_id() -> AgentRole {
    AgentRole::new("test_db_architect")
}

fn test_dim_id() -> ResearchDimension {
    ResearchDimension::new("test_accessibility")
}

fn test_role_definition() -> RoleDefinition {
    let id = test_role_id();
    RoleDefinition {
        id: id.clone(),
        description: "Test-only database architecture analyst".to_string(),
        aliases: vec![],
        prompt_contract: "agent.test_db_architect".to_string(),
        prompt_version: 1,
        stage: MissionStage::Plan,
        researcher_class: false,
        read_only_verification: false,
        write_tools_permitted: false,
        requires_modification: false,
        default_verification: VerificationStrategy::ArtifactInspection { paths: Vec::new() },
        default_capabilities: vec![CapabilityRequirement::new(
            "fs.read",
            CapabilityAccessMode::Read,
        )],
        concurrency_limit: None,
        profile: AgentProfile {
            id: "test-db-architect-v1".to_string(),
            role: id,
            description: "Test-only database architecture analyst".to_string(),
            model_policy: ModelPolicy {
                min_context_tokens: 8192,
                supports_tool_calling: true,
                preferred_model: "test-model".to_string(),
                fallback_models: vec![],
                temperature_millicelsius: 200,
            },
            capability_policy: CapabilityEnvelope::read_only([
                "fs.read",
                "repo.read",
                "artifacts.read",
                "docs.search",
            ]),
            sandbox_policy: "read_only".to_string(),
            context_policy: ContextPolicy {
                default_max_tokens: 8192,
                reserved_headroom_tokens: 1024,
                compaction_threshold_percent: 80,
            },
            max_steps: 12,
            termination_policy: TerminationPolicy {
                step_stall_timeout_secs: 60,
                task_wall_clock_timeout_secs: 600,
                max_consecutive_action_failures: 3,
            },
            prompt_ref: PromptReference::new("agent.test_db_architect", 1),
            additional_instructions: None,
        },
    }
}

fn test_dim_definition() -> ResearchDimensionDefinition {
    ResearchDimensionDefinition {
        id: test_dim_id(),
        display_name: "TestAccessibility".to_string(),
        description: "Test-only accessibility research dimension".to_string(),
        prompt_contract: "genesis.research_test_accessibility".to_string(),
        agent_role: AgentRole::researcher(),
        artifact_filename: "TESTA11Y.md".to_string(),
        requirement_prefix: "TESTA11Y".to_string(),
        requirement_category: RequirementCategory::UxProduct,
        requirement_priority: m31a::planning::requirements::RequirementPriority::Should,
        rationale: "Test accessibility finding".to_string(),
        full_set: false,
        brownfield_targeted: false,
        builtin: false,
    }
}

/// Register the synthetic test role; tolerate prior registration so tests
/// stay independent under parallel execution.
fn ensure_test_role() {
    match RoleRegistry::register_global(test_role_definition()) {
        Ok(()) => {}
        Err(RoleError::DuplicateRole { .. }) => {}
        Err(e) => panic!("test role registration must succeed: {e}"),
    }
}

/// Register the synthetic test dimension; tolerate prior registration.
fn ensure_test_dimension() {
    match ResearchDimensionRegistry::register_global(test_dim_definition()) {
        Ok(()) => {}
        Err(DimensionError::DuplicateDimension { .. }) => {}
        Err(e) => panic!("test dimension registration must succeed: {e}"),
    }
}

// ============================================================================
// Test A — new role through production paths
// ============================================================================

#[tokio::test]
async fn test_a_synthetic_role_registers_and_flows_through_production() {
    ensure_test_role();
    let role = test_role_id();

    // 1. Existence: registry resolves the synthetic role.
    {
        let guard = RoleRegistry::global().read().expect("registry readable");
        assert!(guard.contains(&role));
        assert_eq!(
            guard.prompt_reference_for(&role).id,
            "agent.test_db_architect"
        );
        assert_eq!(guard.stage_for(&role), Some(MissionStage::Plan));
        assert!(!guard.is_compatible(&role, &AgentRole::implementer()));
        assert!(guard.is_compatible(&role, &role));
    }

    // 2. Plan validation accepts the synthetic role (production boundary).
    let validator = PlanValidator::new();
    let validated = validator
        .validate_role("TASK-01", "test_db_architect")
        .expect("registered role must validate");
    assert_eq!(validated, role);

    // 3. Profile materialization carries the declared envelope and ceilings.
    let profile = RoleRegistry::profile_for_global(&role).expect("profile materializes");
    assert_eq!(profile.max_steps, 12);
    assert!(
        profile
            .capability_policy
            .allowed_capabilities
            .contains("fs.read")
    );
    assert!(!profile.capability_policy.allow_file_write);

    // 4. Dispatch allocates a worker for the synthetic role (production path).
    let dispatcher = m31a::agent::ProductionWorkerDispatcher::new();
    let agent = m31a::kernel::seams::execution::WorkerDispatcher::allocate_worker(
        &dispatcher,
        m31a::ids::TaskId::new(),
        m31a::ids::MissionId::new(),
        &["role:test_db_architect".to_string(), "fs.read".to_string()],
    )
    .await
    .expect("dispatch must allocate registered role");
    let _ = agent;

    // 5. Envelope enforcement holds: write caps are rejected for the
    // read-only synthetic role.
    let denied = m31a::kernel::seams::execution::WorkerDispatcher::allocate_worker(
        &dispatcher,
        m31a::ids::TaskId::new(),
        m31a::ids::MissionId::new(),
        &["role:test_db_architect".to_string(), "fs.write".to_string()],
    )
    .await;
    assert!(denied.is_err(), "out-of-envelope caps must fail closed");

    // 6. Verification-default mapping resolves the declared default.
    let step = m31a::workflow::definition::WorkflowStepDefinition {
        key: "test_step".to_string(),
        name: "Test step".to_string(),
        role: role.clone(),
        prompt_template: "agent.test_db_architect.v1".to_string(),
        required_inputs: vec![],
        expected_outputs: vec![],
        required_capabilities: vec![],
        quality_gate: m31a::workflow::definition::QualityGate {
            schema: None,
            required_artifacts: vec![],
            require_human_approval: false,
            max_ambiguity_percent: None,
        },
        depends_on: vec![],
        timeout_secs: 60,
        allows_parallelism: false,
        recovery_strategy: None,
    };
    assert_eq!(
        m31a::workflow::compiler::CompiledWorkflow::map_step_verification(&step),
        VerificationStrategy::ArtifactInspection { paths: Vec::new() }
    );
}

// ============================================================================
// Test B — new research dimension without touching orchestration
// ============================================================================

#[test]
fn test_b_synthetic_dimension_flows_through_generic_orchestration() {
    ensure_test_dimension();
    let dim = test_dim_id();

    // 1. Existence: registry resolves the synthetic dimension.
    let guard = ResearchDimensionRegistry::global()
        .read()
        .expect("registry readable");
    let def = guard.resolve(&dim).expect("dimension registered");
    assert_eq!(def.artifact_filename, "TESTA11Y.md");
    assert_eq!(def.requirement_prefix, "TESTA11Y");
    // Custom dimensions never join default sets implicitly.
    assert!(!guard.is_full_set(std::slice::from_ref(&dim)));
    assert!(!guard.full_set().contains(&dim));
    drop(guard);

    // 2. Research compilation emits a step for the synthetic dimension with
    // zero orchestration changes (filename/contract/role all from the def).
    let charter = ProjectCharter::new("Tidal Scheduler", "design a tidal harvest scheduler");
    let decision = ResearchDecision::execute(vec![dim.clone()], "explicit test selection");
    assert!(!decision.is_full_greenfield());
    assert!(decision.is_targeted_delta());
    let options = GenesisOptions::default();
    let workflow_def =
        ResearchOrchestrator::build_workflow_definition(&charter, &decision, &options)
            .expect("compilation must succeed for registered dimension");
    assert_eq!(workflow_def.steps.len(), 2); // 1 dimension + synthesis
    let step = &workflow_def.steps[0];
    assert_eq!(step.key, "genesis_research_test_accessibility");
    assert_eq!(step.prompt_template, "genesis.research_test_accessibility");
    assert_eq!(step.role, AgentRole::researcher());
    assert_eq!(
        step.expected_outputs[0].artifact_name,
        "TESTA11Y.md".to_string()
    );

    // 3. Collection reads the dimension artifact generically.
    let dir = tempdir().unwrap();
    let research_dir = dir.path().join(".planning/research");
    std::fs::create_dir_all(&research_dir).unwrap();
    std::fs::write(
        research_dir.join("TESTA11Y.md"),
        "# Test accessibility\n\nKeyboard-first navigation audit.\n\n- risk: low contrast\n",
    )
    .unwrap();
    let findings =
        ResearchOrchestrator::collect_findings(dir.path(), ".planning", std::slice::from_ref(&dim))
            .expect("collection must succeed");
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].dimension, dim);

    // 4. Requirements synthesis handles the new dimension generically with
    // the declared mapping and InferredFact status.
    let mut req_charter =
        ProjectCharter::new("Tidal Scheduler", "design a tidal harvest scheduler");
    req_charter.boundaries.in_scope = vec!["Schedule tidal windows".to_string()];
    let doc =
        RequirementsSynthesizer::synthesize(&req_charter, None, &findings, None, None).unwrap();
    let derived: Vec<_> = doc
        .requirements
        .iter()
        .filter(|r| r.key.to_string().starts_with("REQ-TESTA11Y-"))
        .collect();
    assert_eq!(derived.len(), 1);
    assert_eq!(derived[0].current_status, EpistemicStatus::InferredFact);
    assert_eq!(derived[0].category, RequirementCategory::UxProduct);
}

// ============================================================================
// Test C — architecture identity is target-derived and deterministic
// ============================================================================

#[test]
fn test_c_architecture_identities_are_target_derived() {
    use m31a::workflow::genesis::discovery::infer_domain_model;

    let mut alpha = ProjectCharter::new("Warehouse Tracker", "warehouse inventory control");
    alpha.boundaries.in_scope = vec!["Track pallet positions".to_string()];
    alpha.technical_preferences.storage = Some("PostgreSQL".to_string());
    alpha.domain_model = infer_domain_model("warehouse inventory control");

    let mut beta = ProjectCharter::new("Choir Planner", "music collaboration platform");
    beta.boundaries.in_scope = vec!["Schedule rehearsal rooms".to_string()];
    beta.technical_preferences.storage = Some("SQLite".to_string());
    beta.domain_model = infer_domain_model("music collaboration platform");

    let reqs_a = RequirementsSynthesizer::synthesize(&alpha, None, &[], None, None).unwrap();
    let reqs_b = RequirementsSynthesizer::synthesize(&beta, None, &[], None, None).unwrap();
    let (arch_a, _) =
        ArchitectureSynthesizer::synthesize(&reqs_a, &alpha, None, &[], None).unwrap();
    let (arch_b, _) = ArchitectureSynthesizer::synthesize(&reqs_b, &beta, None, &[], None).unwrap();

    let ids_a: Vec<_> = arch_a.subsystems.iter().map(|s| s.id.clone()).collect();
    let ids_b: Vec<_> = arch_b.subsystems.iter().map(|s| s.id.clone()).collect();
    // Distinct target evidence yields distinct identities.
    assert_ne!(ids_a, ids_b, "identities must be target-derived");
    // No universal M31A table is injected into either architecture.
    for forbidden in [
        "SUB-OPERATIONS",
        "SUB-DOMAIN",
        "SUB-SECURITY",
        "SUB-INTEGRATION",
        "SUB-QUALITY",
        "SUB-KERNEL",
        "CMP-OPERATIONS",
        "CMP-DOMAIN",
        "CMP-ENGINE",
    ] {
        assert!(
            !ids_a.contains(&forbidden.to_string()),
            "table leak: {forbidden}"
        );
        assert!(
            !ids_b.contains(&forbidden.to_string()),
            "table leak: {forbidden}"
        );
    }
    // Evidence is preserved: storage selections reach the persistence model.
    assert!(arch_a.persistence_model.contains("PostgreSQL"));
    assert!(arch_b.persistence_model.contains("SQLite"));

    // Determinism: identical inputs yield identical identities.
    let reqs_a2 = RequirementsSynthesizer::synthesize(&alpha, None, &[], None, None).unwrap();
    let (arch_a2, _) =
        ArchitectureSynthesizer::synthesize(&reqs_a2, &alpha, None, &[], None).unwrap();
    let ids_a2: Vec<_> = arch_a2.subsystems.iter().map(|s| s.id.clone()).collect();
    assert_eq!(ids_a, ids_a2, "derived identities must be deterministic");
}

// ============================================================================
// Test D — brownfield roadmap structure follows delta evidence
// ============================================================================

fn brownfield_map_with_modules(modules: Vec<&str>, delta: Option<&str>) -> BrownfieldMap {
    BrownfieldMap {
        topology: CodebaseTopology {
            root: std::path::PathBuf::from("/tmp/repo"),
            primary_language: "Rust".to_string(),
            detected_frameworks: vec![],
            entry_points: vec![],
            module_tree: modules.into_iter().map(str::to_string).collect(),
            test_frameworks: vec!["cargo test".to_string()],
            ci_cd: vec![],
            package_manifests: vec!["Cargo.toml".to_string()],
        },
        existing_conventions: vec![],
        subsystem_boundaries: vec![],
        delta_scope: delta.map(str::to_string),
    }
}

#[test]
fn test_d_brownfield_roadmap_follows_delta_evidence() {
    let risks = RiskRegister::new();

    // Shape 1: narrow delta (Functional only, no Compatibility) — the
    // roadmap cannot be a fixed three-phase template.
    let mut narrow = ProjectCharter::new("Narrow Delta", "add search filter");
    narrow.boundaries.in_scope = vec!["Filter records by query".to_string()];
    let bm_narrow = brownfield_map_with_modules(vec!["src/app"], Some("SearchFilter"));
    let reqs_narrow =
        RequirementsSynthesizer::synthesize(&narrow, None, &[], Some(&bm_narrow), None).unwrap();
    let (arch_narrow, adrs_narrow) =
        ArchitectureSynthesizer::synthesize(&reqs_narrow, &narrow, None, &[], Some(&bm_narrow))
            .unwrap();
    let roadmap_narrow = RoadmapSynthesizer::synthesize(
        &reqs_narrow,
        &arch_narrow,
        &adrs_narrow,
        &risks,
        &narrow,
        Some(&bm_narrow),
    )
    .unwrap();
    // Shape 1: narrow delta (Functional scope + brownfield compatibility
    // evidence). The brownfield compatibility requirement is genuine
    // scan evidence, so the derived structure is baseline + Functional
    // delta + regression. What matters: every name/reference is derived
    // from the delta evidence — never fixed template text.
    assert_eq!(roadmap_narrow.phases.len(), 3);
    assert!(roadmap_narrow.phases[0].name.contains("Pre-Flight"));
    assert!(roadmap_narrow.phases[1].name.contains("SearchFilter"));
    assert!(
        !roadmap_narrow.phases[1].name.contains("Delta Subsystem:"),
        "phase names must be derived, not templated: {}",
        roadmap_narrow.phases[1].name
    );
    assert!(
        roadmap_narrow.phases[2].name.contains("Regression"),
        "compatibility evidence must derive a regression phase"
    );
    for phase in &roadmap_narrow.phases {
        for aref in &phase.architecture_refs {
            assert!(
                !["SUB-CORE", "SUB-DELTA", "CMP-DELTA-01"].contains(&aref.as_str()),
                "universal arch id must not appear: {aref}"
            );
        }
    }
    roadmap_narrow.validate_dag().expect("narrow DAG valid");

    // Shape 2: different module tree + compatibility evidence — different
    // identities AND a different phase structure (regression present).
    let mut wide = ProjectCharter::new("Wide Delta", "add sync engine");
    wide.boundaries.in_scope = vec!["Sync records to remote".to_string()];
    wide.operational_invariants
        .security_requirements
        .push("Authenticate sync peers".to_string());
    let bm_wide =
        brownfield_map_with_modules(vec!["src/app", "src/net", "src/store"], Some("SyncEngine"));
    let reqs_wide =
        RequirementsSynthesizer::synthesize(&wide, None, &[], Some(&bm_wide), None).unwrap();
    // Compatibility evidence forces the regression phase.
    let compat_prov = m31a::planning::requirements::Provenance::new(
        m31a::planning::requirements::ProvenanceSourceType::RepositoryFile,
        m31a::planning::requirements::TrustLevel::VerifiedRepository,
        "test",
    );
    let mut compat_req = m31a::planning::requirements::EngineeringRequirement::new(
        "REQ-COMPAT-90",
        "Preserve host API stability",
        EpistemicStatus::InferredFact,
        compat_prov,
        vec!["Host API unchanged".to_string()],
    );
    compat_req.category = RequirementCategory::Compatibility;
    let mut reqs_wide_mut = reqs_wide;
    reqs_wide_mut.add_requirement(compat_req).unwrap();
    let (arch_wide, adrs_wide) =
        ArchitectureSynthesizer::synthesize(&reqs_wide_mut, &wide, None, &[], Some(&bm_wide))
            .unwrap();
    let roadmap_wide = RoadmapSynthesizer::synthesize(
        &reqs_wide_mut,
        &arch_wide,
        &adrs_wide,
        &risks,
        &wide,
        Some(&bm_wide),
    )
    .unwrap();
    assert!(
        roadmap_wide.phases.len() >= 4,
        "wide delta must produce baseline + categories + regression, got {}",
        roadmap_wide.phases.len()
    );
    assert!(
        roadmap_wide
            .phases
            .last()
            .expect("last phase")
            .name
            .contains("Regression"),
        "final phase must be the derived regression phase"
    );
    roadmap_wide.validate_dag().expect("wide DAG valid");

    // Different delta evidence yields different architecture identities.
    let ids_narrow: Vec<_> = arch_narrow.subsystems.iter().map(|s| &s.id).collect();
    let ids_wide: Vec<_> = arch_wide.subsystems.iter().map(|s| &s.id).collect();
    assert_ne!(ids_narrow, ids_wide);
}

// ============================================================================
// Test E — no fabrication when evidence is absent
// ============================================================================

#[test]
fn test_e_absent_evidence_produces_no_fabricated_content() {
    use m31a::workflow::genesis::discovery::infer_domain_model;

    // 1. Empty domain scaffold: no placeholder entities/workflows/defaults.
    let model = infer_domain_model("build a quantum timesheet analyzer");
    assert!(model.entities.is_empty());
    assert!(model.core_workflows.is_empty());
    assert!(model.inferred_defaults.is_empty());
    assert!(model.non_goals.is_empty());

    // 2. No fabricated security baseline: missing security evidence yields
    // an explicit unresolved question, never a VerifiedFact requirement.
    let mut charter = ProjectCharter::new("Empty Project", "do something eventually");
    charter.boundaries.in_scope = vec!["Do the thing".to_string()];
    let doc = RequirementsSynthesizer::synthesize(&charter, None, &[], None, None).unwrap();
    assert!(
        doc.by_category(RequirementCategory::Security).is_empty(),
        "no security requirement may be invented"
    );
    assert!(
        !doc.unresolved_questions.is_empty(),
        "the security gap must be an explicit unresolved question"
    );
    for req in &doc.requirements {
        assert_ne!(
            req.current_status,
            EpistemicStatus::VerifiedFact,
            "no unevidenced requirement may claim VerifiedFact: {}",
            req.key
        );
    }

    // 3. Empty research documents produce no consensus points; unknown
    // filenames are preserved in an explicitly-marked bucket, never
    // misattributed to an unrelated dimension.
    let summary = ResearchSynthesizer::synthesize_from_dimension_texts(
        &charter,
        &[
            ("STACK.md", "# Stack\n\n"),
            (
                "MYSTERY.md",
                "# Mystery\n\nGenuine but unclassified evidence.",
            ),
        ],
    )
    .unwrap();
    assert!(
        summary
            .consensus_points
            .iter()
            .all(|cp| !cp.topic.starts_with("STACK")),
        "empty STACK.md must not fabricate a consensus point"
    );
    let mystery: Vec<_> = summary
        .consensus_points
        .iter()
        .filter(|cp| cp.topic.contains("MYSTERY"))
        .collect();
    assert_eq!(mystery.len(), 1);
    assert!(mystery[0].topic.contains("unregistered dimension"));
    assert!(mystery[0].supporting_dimensions.is_empty());

    // 4. Unevidenced synthesis attributes no dimension.
    let empty = ResearchSynthesizer::synthesize(&charter, &[]).unwrap();
    assert!(
        empty
            .consensus_points
            .iter()
            .all(|cp| cp.supporting_dimensions.is_empty()),
        "unevidenced baseline must not claim dimension support"
    );
}

// ============================================================================
// Test F — provenance is preserved on derived artifacts
// ============================================================================

#[test]
fn test_f_derived_artifacts_preserve_epistemic_provenance() {
    let mut charter = ProjectCharter::new("Provenance Farm", "grow audit logs");
    charter.boundaries.in_scope = vec!["Record growth events".to_string()];
    charter
        .operational_invariants
        .security_requirements
        .push("Operators authenticate with hardware keys".to_string());

    let finding = ResearchFinding::new(
        ResearchDimension::security(),
        "Key Hygiene",
        "Rotate hardware keys quarterly",
    );
    let doc = RequirementsSynthesizer::synthesize(&charter, None, &[finding], None, None).unwrap();

    // Charter scope: explicit user requirement with charter provenance.
    let scope_req = doc
        .requirements
        .iter()
        .find(|r| r.key.to_string() == "REQ-FUNC-01")
        .expect("charter scope requirement");
    assert_eq!(
        scope_req.current_status,
        EpistemicStatus::ExplicitUserRequirement
    );

    // Charter security invariant: user-stated, never VerifiedFact.
    let sec_req = doc
        .requirements
        .iter()
        .find(|r| r.key.to_string() == "REQ-SEC-01")
        .expect("charter security requirement");
    assert_eq!(
        sec_req.current_status,
        EpistemicStatus::ExplicitUserRequirement
    );

    // Finding-derived: InferredFact with research location + role actor.
    let finding_req = doc
        .requirements
        .iter()
        .find(|r| r.key.to_string() == "REQ-SEC-02")
        .expect("finding-derived security requirement");
    assert_eq!(finding_req.current_status, EpistemicStatus::InferredFact);
    assert_eq!(
        finding_req.provenance.location.as_deref(),
        Some("research/SECURITY.md")
    );
    assert_eq!(finding_req.provenance.actor, "security_researcher");

    // The formerly-silently-dropped Architecture dimension now flows
    // through the same generic path (no `_ => {}` loss).
    let arch_finding = ResearchFinding::new(
        ResearchDimension::architecture(),
        "Layer Discipline",
        "Runtime owns state transitions below the model boundary",
    );
    let doc2 =
        RequirementsSynthesizer::synthesize(&charter, None, &[arch_finding], None, None).unwrap();
    let arch_req = doc2
        .requirements
        .iter()
        .find(|r| r.key.to_string().starts_with("REQ-ARCH-"))
        .expect("architecture finding must yield a requirement, never be dropped");
    assert_eq!(arch_req.current_status, EpistemicStatus::InferredFact);
}

// ============================================================================
// Test G — invalid definitions fail explicitly
// ============================================================================

#[test]
fn test_g_invalid_definitions_fail_explicitly() {
    ensure_test_role();

    // 1. Role missing required metadata.
    let mut bad = test_role_definition();
    bad.id = AgentRole::new("test_broken_role");
    bad.prompt_contract = String::new();
    assert!(matches!(
        RoleRegistry::register_global(bad),
        Err(RoleError::InvalidDefinition { .. })
    ));

    // 2. Role claiming write tools with a read-only envelope.
    let mut incoherent = test_role_definition();
    incoherent.id = AgentRole::new("test_incoherent_role");
    incoherent.write_tools_permitted = true;
    assert!(matches!(
        RoleRegistry::register_global(incoherent),
        Err(RoleError::InvalidDefinition { .. })
    ));

    // 3. Role with unknown sandbox policy.
    let mut sandbox = test_role_definition();
    sandbox.id = AgentRole::new("test_sandbox_role");
    sandbox.profile.sandbox_policy = "yolo_no_limits".to_string();
    assert!(matches!(
        RoleRegistry::register_global(sandbox),
        Err(RoleError::InvalidDefinition { .. })
    ));

    // 4. Duplicate role registration.
    assert!(matches!(
        RoleRegistry::register_global(test_role_definition()),
        Err(RoleError::DuplicateRole { .. })
    ));

    // 5. Dimension bound to an unregistered role.
    let mut bad_dim = test_dim_definition();
    bad_dim.id = ResearchDimension::new("test_broken_dim");
    bad_dim.artifact_filename = "TESTBROKEN.md".to_string();
    bad_dim.agent_role = AgentRole::new("test_ghost_role");
    assert!(matches!(
        ResearchDimensionRegistry::register_global(bad_dim),
        Err(DimensionError::InvalidDefinition { .. })
    ));

    // 6. Custom dimension joining default sets.
    let mut greedy = test_dim_definition();
    greedy.id = ResearchDimension::new("test_greedy_dim");
    greedy.artifact_filename = "TESTGREEDY.md".to_string();
    greedy.full_set = true;
    assert!(matches!(
        ResearchDimensionRegistry::register_global(greedy),
        Err(DimensionError::InvalidDefinition { .. })
    ));

    // 7. Unknown role rejected at plan validation (production boundary).
    let validator = PlanValidator::new();
    assert!(validator.validate_role("T-1", "test_ghost_role").is_err());

    // 8. Unknown dimension rejected at collection (production boundary).
    let dir = tempdir().unwrap();
    assert!(
        ResearchOrchestrator::collect_findings(
            dir.path(),
            ".planning",
            &[ResearchDimension::new("test_ghost_dim")]
        )
        .is_err()
    );
}

// ============================================================================
// Test H — registered roles cannot self-grant authority
// ============================================================================

#[test]
fn test_h_registered_roles_cannot_self_grant_authority() {
    ensure_test_role();
    let role = test_role_id();

    // 1. Ceilings bind custom roles: tightening works, loosening fails.
    let profile = RoleRegistry::profile_for_global(&role).expect("profile");
    assert_eq!(profile.max_steps, 12);
    let tightened = profile
        .apply_override(m31a::agent::ProfileOverride {
            max_steps: Some(6),
            ..Default::default()
        })
        .expect("tightening allowed");
    assert_eq!(tightened.max_steps, 6);
    assert!(
        profile
            .apply_override(m31a::agent::ProfileOverride {
                max_steps: Some(13),
                ..Default::default()
            })
            .is_err(),
        "custom role ceiling must bind"
    );

    // 2. Dispatch-time envelope intersection binds custom roles.
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(async {
        let dispatcher = m31a::agent::ProductionWorkerDispatcher::new();
        let denied = m31a::kernel::seams::execution::WorkerDispatcher::allocate_worker(
            &dispatcher,
            m31a::ids::TaskId::new(),
            m31a::ids::MissionId::new(),
            &[
                "role:test_db_architect".to_string(),
                "shell.exec".to_string(),
            ],
        )
        .await;
        assert!(denied.is_err(), "shell.exec outside envelope must fail");
    });

    // 3. Policy matching is role-id generic and independent of the
    // registry: a DENY rule naming the custom role is honored.
    let rule = m31a::policy::rule::PolicyRule {
        id: "test-deny-shell".to_string(),
        description: None,
        decision: m31a::kernel::seams::policy::PolicyDecision::Deny,
        tools: vec!["shell_exec".to_string()],
        paths: vec![],
        args: None,
        roles: vec![role.clone()],
        modes: vec![],
    };
    let ctx = m31a::policy::matcher::PolicyEvaluationContext {
        tool_id: "shell_exec".to_string(),
        target_paths: vec![],
        args: serde_json::json!({}),
        role: Some(role.clone()),
        mode: m31a::state_machine::autonomy::AutonomyMode::Safe,
        workspace_root: std::path::PathBuf::from("/tmp"),
    };
    assert!(m31a::policy::matcher::PolicyMatcher::matches_rule(
        &rule, &ctx
    ));
    let other_ctx = m31a::policy::matcher::PolicyEvaluationContext {
        role: Some(AgentRole::implementer()),
        ..ctx
    };
    assert!(!m31a::policy::matcher::PolicyMatcher::matches_rule(
        &rule, &other_ctx
    ));

    // 4. Concurrency bounds custom roles without scheduler changes.
    let limiter = m31a::scheduler::concurrency::ConcurrencyLimiter::new();
    let mut limiter = limiter;
    let limits = m31a::scheduler::concurrency::ConcurrencyLimits {
        max_global_workers: 100,
        max_per_role: BTreeMap::new(),
    };
    assert!(limiter.try_reserve(role.clone(), &limits).is_ok());
    assert!(limiter.try_reserve(role.clone(), &limits).is_ok());
    assert!(
        limiter.try_reserve(role, &limits).is_err(),
        "third concurrent custom worker must hit the registry default bound"
    );
}

// ============================================================================
// Test I — production pipeline end to end (incl. synthetic role planning)
// ============================================================================

struct CannedDecompose {
    body: String,
}

#[async_trait]
impl ModelCaller for CannedDecompose {
    async fn call_model(&self, _context: &str) -> Result<ModelProposal, String> {
        Ok(ModelProposal::Complete {
            summary: self.body.clone(),
            artifacts: vec![],
        })
    }

    async fn call_model_with_context(
        &self,
        _compiled: &m31a::kernel::seams::context::CompiledContext,
        _cancellation: &tokio_util::sync::CancellationToken,
    ) -> Result<ModelProposal, String> {
        self.call_model("").await
    }
}

#[tokio::test]
async fn test_i_production_pipeline_with_registered_extensions() {
    ensure_test_role();
    ensure_test_dimension();
    let dir = tempdir().unwrap();
    let env = WorkspaceEnvironment::probe(dir.path()).unwrap();

    // 1. Discovery → charter through the real session path.
    let prompt = "design a tidal harvest scheduler";
    let req = m31a::workflow::genesis::GenesisRequest::new(prompt, dir.path());
    let session = m31a::workflow::genesis::DiscoverySession::new(req, env);
    let charter = session.synthesize_charter().expect("charter synthesizes");
    charter.validate().expect("charter validates");

    // 2. Model planning with a SYNTHETIC role through the real service path.
    // The model proposes; the registry validates. No orchestration change.
    let body = serde_json::json!({ "tasks": [{
        "id": "TASK-01",
        "title": "Design tidal schema",
        "description": "Model schema for tidal windows",
        "role": "test_db_architect",
        "depends_on": [],
        "required_capabilities": ["fs.read", "repo.read"],
    }] })
    .to_string();
    let service =
        PlanServiceImpl::new(dir.path()).with_model_caller(Arc::new(CannedDecompose { body }));
    let resp = service
        .generate_initial_plan(PlanRequest::new(
            m31a::ids::MissionId::new(),
            "Tidal harvest scheduler exploration",
        ))
        .await
        .expect("plan with synthetic role must succeed");
    assert_eq!(resp.task_count, 1);
    assert_eq!(resp.candidate_plan.tasks[0].role, test_role_id());

    // 3. Research → requirements → architecture → roadmap through real
    // synthesizers with an explicitly selected synthetic dimension.
    let research_dir = dir.path().join(".planning/research");
    std::fs::create_dir_all(&research_dir).unwrap();
    std::fs::write(
        research_dir.join("TESTA11Y.md"),
        "# Access\n\nKeyboard-first tidal console.\n",
    )
    .unwrap();
    let decision = ResearchDecision::execute(
        vec![ResearchDimension::security(), test_dim_id()],
        "test selection",
    );
    let findings = ResearchOrchestrator::collect_findings(
        dir.path(),
        ".planning",
        &decision.selected_dimensions,
    )
    .expect("collection succeeds");
    assert_eq!(
        findings.len(),
        1,
        "only the evidenced dimension yields a finding"
    );
    // (SECURITY.md absent → honest absence, no fabrication.)

    let reqs = RequirementsSynthesizer::synthesize(&charter, None, &findings, None, None).unwrap();
    assert!(
        reqs.requirements
            .iter()
            .any(|r| r.key.to_string().starts_with("REQ-TESTA11Y-")),
        "synthetic dimension finding must reach requirements"
    );
    let (arch, _adrs) =
        ArchitectureSynthesizer::synthesize(&reqs, &charter, None, &findings, None).unwrap();
    assert!(!arch.subsystems.is_empty());
    let risks = RiskRegister::new();
    let roadmap =
        RoadmapSynthesizer::synthesize(&reqs, &arch, &_adrs, &risks, &charter, None).unwrap();
    roadmap.validate_dag().expect("roadmap DAG valid");
    // Coverage: every non-deferred requirement scheduled.
    for req in &reqs.requirements {
        if req.priority != m31a::planning::requirements::RequirementPriority::Deferred {
            assert!(
                roadmap
                    .phases
                    .iter()
                    .any(|p| p.requirement_refs.contains(&req.key)),
                "unscheduled: {}",
                req.key
            );
        }
    }

    // 4. Research decision still evaluates generically on live inputs.
    let env2 = WorkspaceEnvironment::probe(dir.path()).unwrap();
    let options = GenesisOptions::default();
    let _live = evaluate_research_decision(&charter, &env2, &options);
}
