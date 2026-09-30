//! Comprehensive tests for Bidirectional Traceability Matrix and Orphan Detection (Package 5).

use m31a::planning::requirements::{
    EngineeringRequirement, EpistemicStatus, Provenance, ProvenanceSourceType, RequirementCategory,
    RequirementPriority, TrustLevel,
};
use m31a::workflow::genesis::project::ProjectCharter;
use m31a::workflow::planning::architecture_synthesizer::ArchitectureSynthesizer;
use m31a::workflow::planning::requirements::RequirementsDocument;
use m31a::workflow::planning::risks::RiskRegister;
use m31a::workflow::planning::roadmap::RoadmapSynthesizer;
use m31a::workflow::planning::traceability::TraceabilityMatrix;

fn sample_charter() -> ProjectCharter {
    let mut charter = ProjectCharter::new(
        "M31A Traceability Test",
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
    let mut doc = RequirementsDocument::new("M31A Traceability Test", "Traceability Scope");
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
fn test_traceability_matrix_construction_and_validation() {
    let charter = sample_charter();
    let reqs = sample_requirements();
    let (arch, adrs) =
        ArchitectureSynthesizer::synthesize(&reqs, &charter, None, &[], None).unwrap();
    let risks = RiskRegister::new();
    let roadmap =
        RoadmapSynthesizer::synthesize(&reqs, &arch, &adrs, &risks, &charter, None).unwrap();

    let matrix = TraceabilityMatrix::build(&reqs, &arch, &adrs, &roadmap, &risks);

    assert_eq!(matrix.links.len(), 3);

    for (key, link) in &matrix.links {
        assert_eq!(&link.requirement_key, key);
        assert!(
            !link.components.is_empty(),
            "Requirement {} must be linked to component",
            key
        );
        assert!(
            !link.phases.is_empty(),
            "Requirement {} must be linked to phase",
            key
        );
        assert!(
            !link.verification_criteria.is_empty(),
            "Requirement {} must have verification criteria",
            key
        );
    }

    // Validation passes cleanly
    assert!(matrix.validate().is_ok());
}

#[test]
fn test_orphan_requirement_detection() {
    let charter = sample_charter();
    let mut reqs = sample_requirements();

    // Add an orphan requirement that won't be mapped by architecture or roadmap
    let prov = Provenance::new(
        ProvenanceSourceType::RepositoryFile,
        TrustLevel::VerifiedRepository,
        "operator",
    );
    let mut orphan = EngineeringRequirement::new(
        "REQ-ORPHAN-01",
        "Unmapped floating requirement",
        EpistemicStatus::ExplicitUserRequirement,
        prov,
        vec!["Criterion".to_string()],
    );
    orphan.category = RequirementCategory::Functional;
    orphan.priority = RequirementPriority::Must;
    reqs.add_requirement(orphan).unwrap();

    let (arch, adrs) =
        ArchitectureSynthesizer::synthesize(&reqs, &charter, None, &[], None).unwrap();
    let risks = RiskRegister::new();
    let mut roadmap =
        RoadmapSynthesizer::synthesize(&reqs, &arch, &adrs, &risks, &charter, None).unwrap();

    // Explicitly unmap the orphan from phases and components
    for phase in &mut roadmap.phases {
        phase
            .requirement_refs
            .retain(|r| r.as_str() != "REQ-ORPHAN-01");
    }

    let mut matrix = TraceabilityMatrix::build(&reqs, &arch, &adrs, &roadmap, &risks);
    // Ensure orphan has no phase mapping in the matrix
    if let Some(link) = matrix
        .links
        .get_mut(&m31a::planning::requirements::RequirementKey::new(
            "REQ-ORPHAN-01",
        ))
    {
        link.phases.clear();
    }

    let val_res = matrix.validate();
    assert!(
        val_res.is_err(),
        "Orphan requirement must cause validation failure"
    );
    let errs = val_res.unwrap_err();
    assert!(
        errs.iter()
            .any(|e| e.contains("REQ-ORPHAN-01") && e.contains("no mapped roadmap phases"))
    );
}

#[test]
fn test_deferred_requirement_is_exempt_from_orphan_validation() {
    let charter = sample_charter();
    let mut reqs = sample_requirements();

    let prov = Provenance::new(
        ProvenanceSourceType::RepositoryFile,
        TrustLevel::VerifiedRepository,
        "operator",
    );
    let mut deferred_req = EngineeringRequirement::new(
        "REQ-DEF-01",
        "Deferred futuristic requirement",
        EpistemicStatus::ExplicitUserRequirement,
        prov,
        vec![],
    );
    deferred_req.category = RequirementCategory::Functional;
    deferred_req.priority = RequirementPriority::Deferred;
    reqs.add_requirement(deferred_req).unwrap();

    let (arch, adrs) =
        ArchitectureSynthesizer::synthesize(&reqs, &charter, None, &[], None).unwrap();
    let risks = RiskRegister::new();
    let roadmap =
        RoadmapSynthesizer::synthesize(&reqs, &arch, &adrs, &risks, &charter, None).unwrap();

    let matrix = TraceabilityMatrix::build(&reqs, &arch, &adrs, &roadmap, &risks);
    // Deferred requirement should not fail validation even if it has no phase or verification criteria
    assert!(matrix.validate().is_ok());
}

#[test]
fn test_traceability_markdown_table_generation() {
    let charter = sample_charter();
    let reqs = sample_requirements();
    let (arch, adrs) =
        ArchitectureSynthesizer::synthesize(&reqs, &charter, None, &[], None).unwrap();
    let risks = RiskRegister::new();
    let roadmap =
        RoadmapSynthesizer::synthesize(&reqs, &arch, &adrs, &risks, &charter, None).unwrap();

    let matrix = TraceabilityMatrix::build(&reqs, &arch, &adrs, &roadmap, &risks);
    let md = matrix.to_markdown();

    assert!(md.contains("## Traceability Matrix"));
    assert!(md.contains(
        "| Requirement | Category | Priority | Components | ADRs | Phases | Verification |"
    ));
    assert!(md.contains("`REQ-FUNC-01`"));
    assert!(md.contains("`REQ-OPS-01`"));
    assert!(md.contains("`REQ-SEC-01`"));
}
