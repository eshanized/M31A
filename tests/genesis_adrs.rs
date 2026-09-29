//! Comprehensive tests for Architecture Decision Records (ADRs), Lifecycle, and Projections (Package 5).

use m31a::planning::requirements::{Provenance, ProvenanceSourceType, RequirementKey, TrustLevel};
use m31a::workflow::planning::adr::{AdrId, AdrRegistry, ArchitectureDecisionRecord};
use m31a::workflow::planning::decision::DecisionStatus;
use tempfile::tempdir;

fn sample_provenance() -> Provenance {
    Provenance::new(
        ProvenanceSourceType::RepositoryFile,
        TrustLevel::VerifiedRepository,
        "lead_architect",
    )
    .with_location("ARCHITECTURE.md")
}

fn sample_adr(id_num: usize, title: &str) -> ArchitectureDecisionRecord {
    let adr_id = AdrId::from_number(id_num);
    ArchitectureDecisionRecord::new(
        adr_id,
        title,
        "Context explaining why this architectural decision is needed.",
        "The chosen design decision.",
        "Rationale supporting this choice over alternatives.",
        sample_provenance(),
    )
    .with_alternatives(vec![
        "Alternative A: simple but unscalable".to_string(),
        "Alternative B: complex external dependency".to_string(),
    ])
    .with_consequences(vec![
        "Consequence 1: high modularity".to_string(),
        "Consequence 2: migration overhead".to_string(),
    ])
    .with_residual_risks(vec!["Risk: lock contention under peak load".to_string()])
    .with_linked_requirements(vec![RequirementKey::new("REQ-FUNC-01")])
    .with_linked_research(vec!["research/01_stack.md".to_string()])
}

#[test]
fn test_adr_id_generation_and_formatting() {
    let id1 = AdrId::from_number(1);
    assert_eq!(id1.as_str(), "ADR-0001");
    assert_eq!(id1.to_string(), "ADR-0001");

    let id42 = AdrId::from_number(42);
    assert_eq!(id42.as_str(), "ADR-0042");

    let custom = AdrId::new("ADR-CUSTOM-01");
    assert_eq!(custom.as_str(), "ADR-CUSTOM-01");
}

#[test]
fn test_adr_registration_and_duplicate_rejection() {
    let mut registry = AdrRegistry::new();
    let adr1 = sample_adr(1, "Storage Engine");
    let adr2 = sample_adr(2, "Transport Protocol");

    assert!(registry.register(adr1.clone()).is_ok());
    assert!(registry.register(adr2).is_ok());
    assert_eq!(registry.adrs.len(), 2);

    // Registering duplicate ID must fail
    let duplicate = sample_adr(1, "Storage Engine Duplicate");
    let err = registry.register(duplicate);
    assert!(err.is_err());
}

#[test]
fn test_adr_status_transitions() {
    let mut registry = AdrRegistry::new();
    let adr1 = sample_adr(1, "Storage Engine");
    let adr_id = adr1.id.clone();
    registry.register(adr1).unwrap();

    assert_eq!(
        registry.get(&adr_id).unwrap().status,
        DecisionStatus::Proposed
    );

    // Accept ADR
    registry
        .accept(&adr_id, "human_operator")
        .expect("accept succeeds");
    let accepted = registry.get(&adr_id).unwrap();
    assert_eq!(accepted.status, DecisionStatus::Accepted);
    assert_eq!(accepted.provenance.actor, "human_operator");

    // Once accepted, transitioning to rejected is illegal
    let err = registry.reject(&adr_id, "changed mind", "operator");
    assert!(err.is_err());
}

#[test]
fn test_adr_rejection() {
    let mut registry = AdrRegistry::new();
    let adr = sample_adr(3, "Microservice Architecture");
    let adr_id = adr.id.clone();
    registry.register(adr).unwrap();

    registry
        .reject(
            &adr_id,
            "Violates single-binary architecture invariant",
            "operator",
        )
        .expect("rejection succeeds");

    let rejected = registry.get(&adr_id).unwrap();
    assert_eq!(rejected.status, DecisionStatus::Rejected);
    assert_eq!(rejected.provenance.actor, "operator");
    assert_eq!(
        rejected.provenance.reason.as_deref(),
        Some("Violates single-binary architecture invariant")
    );
}

#[test]
fn test_adr_supersession_chain() {
    let mut registry = AdrRegistry::new();
    let adr1 = sample_adr(1, "Original Key-Value Storage");
    let adr1_id = adr1.id.clone();
    registry.register(adr1).unwrap();

    // Accept original
    registry.accept(&adr1_id, "lead_arch").unwrap();

    // Supersede with ADR-0002
    let adr2 = sample_adr(2, "SQLite Relational Storage");
    let adr2_id = adr2.id.clone();
    registry
        .supersede(&adr1_id, adr2)
        .expect("supersede succeeds");

    let old = registry.get(&adr1_id).unwrap();
    assert_eq!(old.status, DecisionStatus::Superseded);
    assert_eq!(old.superseded_by, Some(adr2_id.clone()));

    let new_adr = registry.get(&adr2_id).unwrap();
    assert_eq!(new_adr.version, 2);
    assert_eq!(new_adr.status, DecisionStatus::Proposed);
}

#[test]
fn test_adr_markdown_and_decisions_file_generation() {
    let mut registry = AdrRegistry::new();
    let adr1 = sample_adr(1, "Embedded SQLite Engine");
    let adr2 = sample_adr(2, "Capability Sandbox");
    let adr1_id = adr1.id.clone();

    registry.register(adr1).unwrap();
    registry.register(adr2).unwrap();
    registry.accept(&adr1_id, "lead_arch").unwrap();

    let dec_md = registry.to_decisions_markdown();
    assert!(dec_md.contains("# Architectural Decisions (ADRs)"));
    assert!(dec_md.contains("ADR-0001"));
    assert!(dec_md.contains("ADR-0002"));
    assert!(dec_md.contains("Embedded SQLite Engine"));
    assert!(dec_md.contains("Capability Sandbox"));

    // Check individual markdown
    let ind_md = registry.get(&adr1_id).unwrap().to_markdown();
    assert!(ind_md.contains("# ADR-0001: Embedded SQLite Engine"));
    assert!(ind_md.contains("## Context"));
    assert!(ind_md.contains("## Decision"));
    assert!(ind_md.contains("## Rationale"));
    assert!(ind_md.contains("## Alternatives Considered"));
    assert!(ind_md.contains("## Consequences"));
    assert!(ind_md.contains("## Residual Risks"));

    // Check disk write
    let dir = tempdir().unwrap();
    let dec_path = registry.save_to_dir(dir.path(), ".planning").unwrap();
    assert!(dec_path.exists());
    let adr1_file = dir.path().join(".planning/adr/ADR-0001.md");
    let adr2_file = dir.path().join(".planning/adr/ADR-0002.md");
    assert!(adr1_file.exists());
    assert!(adr2_file.exists());
}
