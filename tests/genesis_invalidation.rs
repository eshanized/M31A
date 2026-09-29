//! Comprehensive tests for Cascading Planning Invalidation and Change Impact Analysis (Package 5).
//!
//! Covers all 7 required invalidation scenarios specified in Section 53.

use m31a::planning::requirements::{
    EngineeringRequirement, EpistemicStatus, Provenance, ProvenanceSourceType, RequirementCategory,
    RequirementKey, RequirementPriority, TrustLevel,
};
use m31a::workflow::genesis::project::ProjectCharter;
use m31a::workflow::planning::architecture::ComponentStatus;
use m31a::workflow::planning::architecture_synthesizer::ArchitectureSynthesizer;
use m31a::workflow::planning::decision::DecisionStatus;
use m31a::workflow::planning::invalidation::PlanningInvalidator;
use m31a::workflow::planning::requirements::RequirementsDocument;
use m31a::workflow::planning::risks::RiskRegister;
use m31a::workflow::planning::roadmap::{RoadmapPhaseStatus, RoadmapSynthesizer};
use m31a::workflow::planning::state::{PlanningLifecycleState, PlanningState};

fn sample_charter() -> ProjectCharter {
    let mut charter = ProjectCharter::new(
        "M31A Invalidation Test",
        "Autonomous code review and repository refactoring platform",
    );
    charter.boundaries.in_scope = vec![
        "Autonomous git patch inspection".to_string(),
        "Deterministic test execution gate".to_string(),
    ];
    charter.operational_invariants.security_requirements =
        vec!["Strict sandbox isolation for external execution".to_string()];
    charter
}

fn sample_requirements() -> RequirementsDocument {
    let mut doc = RequirementsDocument::new("M31A Invalidation Test", "Invalidation Scope");
    let prov = Provenance::new(
        ProvenanceSourceType::RepositoryFile,
        TrustLevel::VerifiedRepository,
        "operator",
    );

    let mut r1 = EngineeringRequirement::new(
        "REQ-FUNC-01",
        "Analyze patch AST diffs",
        EpistemicStatus::ExplicitUserRequirement,
        prov.clone(),
        vec!["Diff parsed into AST delta".to_string()],
    );
    r1.category = RequirementCategory::Functional;
    r1.priority = RequirementPriority::Must;
    doc.add_requirement(r1).unwrap();

    let mut r2 = EngineeringRequirement::new(
        "REQ-OPS-01",
        "Transactional persistence in SQLite",
        EpistemicStatus::ExplicitUserRequirement,
        prov.clone(),
        vec!["ACID transactions guaranteed".to_string()],
    );
    r2.category = RequirementCategory::Operational;
    r2.priority = RequirementPriority::Must;
    doc.add_requirement(r2).unwrap();

    let mut r3 = EngineeringRequirement::new(
        "REQ-SEC-01",
        "Policy enforcement sandbox",
        EpistemicStatus::ExplicitUserRequirement,
        prov,
        vec!["Sandboxed child processes".to_string()],
    );
    r3.category = RequirementCategory::Security;
    r3.priority = RequirementPriority::Must;
    doc.add_requirement(r3).unwrap();

    doc
}

#[test]
fn test_scenario_1_unchanged_requirements_noop() {
    let charter = sample_charter();
    let reqs1 = sample_requirements();
    let reqs2 = sample_requirements();

    let (arch, adrs) =
        ArchitectureSynthesizer::synthesize(&reqs1, &charter, None, &[], None).unwrap();
    let risks = RiskRegister::new();
    let roadmap =
        RoadmapSynthesizer::synthesize(&reqs1, &arch, &adrs, &risks, &charter, None).unwrap();

    let changeset = PlanningInvalidator::compute_changeset(&reqs1, &reqs2, &arch, &adrs, &roadmap);

    assert!(!changeset.has_changes());
    assert!(changeset.added_requirements.is_empty());
    assert!(changeset.changed_requirements.is_empty());
    assert!(changeset.removed_requirements.is_empty());
    assert!(changeset.affected_architecture_components.is_empty());
    assert!(changeset.affected_adrs.is_empty());
    assert!(changeset.affected_phases.is_empty());
}

#[test]
fn test_scenario_2_modifying_requirement_invalidates_architecture() {
    let charter = sample_charter();
    let old_reqs = sample_requirements();
    let mut new_reqs = sample_requirements();

    // Modify REQ-FUNC-01 statement
    let prov = Provenance::new(
        ProvenanceSourceType::RepositoryFile,
        TrustLevel::VerifiedRepository,
        "operator",
    );
    let mut modified = EngineeringRequirement::new(
        "REQ-FUNC-01",
        "Analyze patch AST diffs with semantic rust-analyzer bindings",
        EpistemicStatus::ExplicitUserRequirement,
        prov,
        vec!["Diff parsed into AST delta".to_string()],
    );
    modified.category = RequirementCategory::Functional;
    modified.priority = RequirementPriority::Must;
    new_reqs.add_or_update(modified);

    let (mut arch, mut adrs) =
        ArchitectureSynthesizer::synthesize(&old_reqs, &charter, None, &[], None).unwrap();
    let risks = RiskRegister::new();
    let mut roadmap =
        RoadmapSynthesizer::synthesize(&old_reqs, &arch, &adrs, &risks, &charter, None).unwrap();
    let mut state = PlanningState::new("Test", PlanningLifecycleState::ImplementationReady);

    let changeset =
        PlanningInvalidator::compute_changeset(&old_reqs, &new_reqs, &arch, &adrs, &roadmap);
    assert!(changeset.has_changes());
    assert_eq!(
        changeset.changed_requirements,
        vec![RequirementKey::new("REQ-FUNC-01")]
    );
    // The derived component covering REQ-FUNC-01 must be invalidated
    // (identity is content-derived, resolved dynamically).
    let func_comp_id = arch
        .components
        .iter()
        .find(|c| {
            c.requirement_refs
                .iter()
                .any(|k| k == &RequirementKey::new("REQ-FUNC-01"))
        })
        .expect("derived component covers REQ-FUNC-01")
        .id
        .clone();
    assert!(
        changeset
            .affected_architecture_components
            .contains(&func_comp_id)
    );

    PlanningInvalidator::apply_invalidation(
        &changeset,
        &mut arch,
        &mut adrs,
        &mut roadmap,
        &mut state,
    );

    // Verify affected architecture component status changed to Modified
    let engine_comp = arch
        .components
        .iter()
        .find(|c| c.id == func_comp_id)
        .unwrap();
    assert_eq!(engine_comp.status, ComponentStatus::Modified);
}

#[test]
fn test_scenario_3_modifying_requirement_invalidates_adrs() {
    let charter = sample_charter();
    let old_reqs = sample_requirements();
    let mut new_reqs = sample_requirements();

    let prov = Provenance::new(
        ProvenanceSourceType::RepositoryFile,
        TrustLevel::VerifiedRepository,
        "operator",
    );
    let mut modified = EngineeringRequirement::new(
        "REQ-OPS-01",
        "Migrate to distributed RocksDB storage",
        EpistemicStatus::ExplicitUserRequirement,
        prov,
        vec!["Transactions committed to SST files".to_string()],
    );
    modified.category = RequirementCategory::Operational;
    modified.priority = RequirementPriority::Must;
    new_reqs.add_or_update(modified);

    let (mut arch, mut adrs) =
        ArchitectureSynthesizer::synthesize(&old_reqs, &charter, None, &[], None).unwrap();
    let risks = RiskRegister::new();
    let mut roadmap =
        RoadmapSynthesizer::synthesize(&old_reqs, &arch, &adrs, &risks, &charter, None).unwrap();
    let mut state = PlanningState::new("Test", PlanningLifecycleState::ImplementationReady);

    // Accept ADR-0001
    let adr1_id = m31a::workflow::planning::adr::AdrId::from_number(1);
    adrs.accept(&adr1_id, "architect").unwrap();
    assert_eq!(adrs.get(&adr1_id).unwrap().status, DecisionStatus::Accepted);

    let changeset =
        PlanningInvalidator::compute_changeset(&old_reqs, &new_reqs, &arch, &adrs, &roadmap);
    assert!(changeset.affected_adrs.contains(&adr1_id));

    PlanningInvalidator::apply_invalidation(
        &changeset,
        &mut arch,
        &mut adrs,
        &mut roadmap,
        &mut state,
    );

    // Accepted ADR linked to REQ-OPS-01 must transition to NeedsOperatorDecision
    assert_eq!(
        adrs.get(&adr1_id).unwrap().status,
        DecisionStatus::NeedsOperatorDecision
    );
}

#[test]
fn test_scenario_4_modifying_requirement_invalidates_roadmap_phases() {
    let charter = sample_charter();
    let old_reqs = sample_requirements();
    let mut new_reqs = sample_requirements();

    let prov = Provenance::new(
        ProvenanceSourceType::RepositoryFile,
        TrustLevel::VerifiedRepository,
        "operator",
    );
    let mut modified = EngineeringRequirement::new(
        "REQ-FUNC-01",
        "Radically changed engine requirements",
        EpistemicStatus::ExplicitUserRequirement,
        prov,
        vec!["New criteria".to_string()],
    );
    modified.category = RequirementCategory::Functional;
    modified.priority = RequirementPriority::Must;
    new_reqs.add_or_update(modified);

    let (mut arch, mut adrs) =
        ArchitectureSynthesizer::synthesize(&old_reqs, &charter, None, &[], None).unwrap();
    let risks = RiskRegister::new();
    let mut roadmap =
        RoadmapSynthesizer::synthesize(&old_reqs, &arch, &adrs, &risks, &charter, None).unwrap();
    let mut state = PlanningState::new("Test", PlanningLifecycleState::ImplementationReady);

    let changeset =
        PlanningInvalidator::compute_changeset(&old_reqs, &new_reqs, &arch, &adrs, &roadmap);
    // The derived phase scheduling REQ-FUNC-01 must be invalidated
    // (phase identity is positional, resolved dynamically).
    let func_phase_id = roadmap
        .phases
        .iter()
        .find(|p| {
            p.requirement_refs
                .iter()
                .any(|k| k == &RequirementKey::new("REQ-FUNC-01"))
        })
        .expect("derived phase schedules REQ-FUNC-01")
        .id
        .clone();
    assert!(changeset.affected_phases.contains(&func_phase_id));

    PlanningInvalidator::apply_invalidation(
        &changeset,
        &mut arch,
        &mut adrs,
        &mut roadmap,
        &mut state,
    );

    // Affected phase must be marked Superseded
    let phase = roadmap.get_phase(&func_phase_id).unwrap();
    assert_eq!(phase.status, RoadmapPhaseStatus::Superseded);
}

#[test]
fn test_scenario_5_adding_new_requirement_preserves_accepted_state() {
    let charter = sample_charter();
    let old_reqs = sample_requirements();
    let mut new_reqs = sample_requirements();

    let prov = Provenance::new(
        ProvenanceSourceType::RepositoryFile,
        TrustLevel::VerifiedRepository,
        "operator",
    );
    let mut new_req = EngineeringRequirement::new(
        "REQ-NEW-01",
        "Add telemetry exporter",
        EpistemicStatus::ExplicitUserRequirement,
        prov,
        vec!["Prometheus metrics emitted".to_string()],
    );
    new_req.category = RequirementCategory::Operational;
    new_req.priority = RequirementPriority::Should;
    new_reqs.add_requirement(new_req).unwrap();

    let (arch, adrs) =
        ArchitectureSynthesizer::synthesize(&old_reqs, &charter, None, &[], None).unwrap();
    let risks = RiskRegister::new();
    let roadmap =
        RoadmapSynthesizer::synthesize(&old_reqs, &arch, &adrs, &risks, &charter, None).unwrap();

    let changeset =
        PlanningInvalidator::compute_changeset(&old_reqs, &new_reqs, &arch, &adrs, &roadmap);
    assert_eq!(
        changeset.added_requirements,
        vec![RequirementKey::new("REQ-NEW-01")]
    );
    assert!(changeset.changed_requirements.is_empty());
    assert!(changeset.removed_requirements.is_empty());
    // Adding a completely new requirement should not invalidate pre-existing components that do not reference it
    assert!(changeset.affected_architecture_components.is_empty());
}

#[test]
fn test_scenario_6_removing_requirement_triggers_clean_invalidation() {
    let charter = sample_charter();
    let old_reqs = sample_requirements();
    let mut new_reqs = RequirementsDocument::new("M31A Invalidation Test", "Invalidation Scope");

    // Keep only REQ-OPS-01 and REQ-SEC-01 (removing REQ-FUNC-01)
    let prov = Provenance::new(
        ProvenanceSourceType::RepositoryFile,
        TrustLevel::VerifiedRepository,
        "operator",
    );
    let mut r2 = EngineeringRequirement::new(
        "REQ-OPS-01",
        "Transactional persistence in SQLite",
        EpistemicStatus::ExplicitUserRequirement,
        prov.clone(),
        vec!["ACID transactions guaranteed".to_string()],
    );
    r2.category = RequirementCategory::Operational;
    r2.priority = RequirementPriority::Must;
    new_reqs.add_requirement(r2).unwrap();

    let mut r3 = EngineeringRequirement::new(
        "REQ-SEC-01",
        "Policy enforcement sandbox",
        EpistemicStatus::ExplicitUserRequirement,
        prov,
        vec!["Sandboxed child processes".to_string()],
    );
    r3.category = RequirementCategory::Security;
    r3.priority = RequirementPriority::Must;
    new_reqs.add_requirement(r3).unwrap();

    let (arch, adrs) =
        ArchitectureSynthesizer::synthesize(&old_reqs, &charter, None, &[], None).unwrap();
    let risks = RiskRegister::new();
    let roadmap =
        RoadmapSynthesizer::synthesize(&old_reqs, &arch, &adrs, &risks, &charter, None).unwrap();

    let changeset =
        PlanningInvalidator::compute_changeset(&old_reqs, &new_reqs, &arch, &adrs, &roadmap);
    assert_eq!(
        changeset.removed_requirements,
        vec![RequirementKey::new("REQ-FUNC-01")]
    );
    let func_comp_id = arch
        .components
        .iter()
        .find(|c| {
            c.requirement_refs
                .iter()
                .any(|k| k == &RequirementKey::new("REQ-FUNC-01"))
        })
        .expect("derived component covers REQ-FUNC-01")
        .id
        .clone();
    assert!(
        changeset
            .affected_architecture_components
            .contains(&func_comp_id)
    );
    let func_phase_id = roadmap
        .phases
        .iter()
        .find(|p| {
            p.requirement_refs
                .iter()
                .any(|k| k == &RequirementKey::new("REQ-FUNC-01"))
        })
        .expect("derived phase schedules REQ-FUNC-01")
        .id
        .clone();
    assert!(changeset.affected_phases.contains(&func_phase_id));
}

#[test]
fn test_scenario_7_lifecycle_state_reset_and_invalidation_idempotency() {
    let charter = sample_charter();
    let old_reqs = sample_requirements();
    let mut new_reqs = sample_requirements();

    let prov = Provenance::new(
        ProvenanceSourceType::RepositoryFile,
        TrustLevel::VerifiedRepository,
        "operator",
    );
    let mut modified = EngineeringRequirement::new(
        "REQ-FUNC-01",
        "Modified functional statement",
        EpistemicStatus::ExplicitUserRequirement,
        prov,
        vec!["New verification".to_string()],
    );
    modified.category = RequirementCategory::Functional;
    modified.priority = RequirementPriority::Must;
    new_reqs.add_or_update(modified);

    let (mut arch, mut adrs) =
        ArchitectureSynthesizer::synthesize(&old_reqs, &charter, None, &[], None).unwrap();
    let risks = RiskRegister::new();
    let mut roadmap =
        RoadmapSynthesizer::synthesize(&old_reqs, &arch, &adrs, &risks, &charter, None).unwrap();
    let mut state = PlanningState::new("Test", PlanningLifecycleState::ImplementationReady);
    state.planning_version = 1;

    let changeset =
        PlanningInvalidator::compute_changeset(&old_reqs, &new_reqs, &arch, &adrs, &roadmap);

    // First invalidation application
    PlanningInvalidator::apply_invalidation(
        &changeset,
        &mut arch,
        &mut adrs,
        &mut roadmap,
        &mut state,
    );

    assert_eq!(state.planning_version, 2);
    assert_eq!(
        state.lifecycle_state,
        PlanningLifecycleState::RequirementsDraft
    );
    assert!(!state.invalidated_artifacts.is_empty());

    // Applying same invalidation again does not produce duplicate artifact entries
    let invalidated_count = state.invalidated_artifacts.len();
    PlanningInvalidator::apply_invalidation(
        &changeset,
        &mut arch,
        &mut adrs,
        &mut roadmap,
        &mut state,
    );
    assert_eq!(state.invalidated_artifacts.len(), invalidated_count);
}
