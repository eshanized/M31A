//! Comprehensive tests for System Architecture Synthesis, Topology, and Quality Gates (Package 5).

use m31a::planning::requirements::{
    EngineeringRequirement, EpistemicStatus, Provenance, ProvenanceSourceType, RequirementCategory,
    RequirementKey, RequirementPriority, TrustLevel,
};
use m31a::workflow::genesis::brownfield::{BrownfieldMap, CodebaseTopology};
use m31a::workflow::genesis::project::ProjectCharter;
use m31a::workflow::planning::architecture::{
    ArchitectureComponent, ArchitectureDocument, ArchitectureInterface, ComponentStatus,
};
use m31a::workflow::planning::architecture_synthesizer::ArchitectureSynthesizer;
use m31a::workflow::planning::requirements::RequirementsDocument;
use m31a::workflow::planning::validation::PlanningQualityGates;
use std::collections::HashSet;
use std::path::PathBuf;
use tempfile::tempdir;

fn sample_charter() -> ProjectCharter {
    let mut charter = ProjectCharter::new(
        "M31A Architecture Test",
        "Autonomous code review and repository refactoring platform",
    );
    charter.boundaries.in_scope = vec![
        "Autonomous git patch inspection".to_string(),
        "Deterministic test execution gate".to_string(),
    ];
    charter.operational_invariants.security_requirements =
        vec!["Strict sandbox isolation for external execution".to_string()];
    charter.operational_invariants.performance_targets =
        vec!["Cold start analysis completed within 5 seconds".to_string()];
    charter
}

fn sample_requirements() -> RequirementsDocument {
    let mut doc = RequirementsDocument::new("M31A Architecture Test", "Test Scope");
    let prov = Provenance::new(
        ProvenanceSourceType::RepositoryFile,
        TrustLevel::VerifiedRepository,
        "operator",
    );

    let mut r1 = EngineeringRequirement::new(
        "REQ-TEST-001",
        "Engine must analyze patch diffs",
        EpistemicStatus::ExplicitUserRequirement,
        prov.clone(),
        vec!["Parses unified diff into AST delta".to_string()],
    );
    r1.category = RequirementCategory::Functional;
    r1.priority = RequirementPriority::Must;
    doc.add_requirement(r1).expect("valid req1");

    let mut r2 = EngineeringRequirement::new(
        "REQ-TEST-002",
        "Durable storage in SQLite",
        EpistemicStatus::ExplicitUserRequirement,
        prov.clone(),
        vec!["All transactions committed to WAL".to_string()],
    );
    r2.category = RequirementCategory::Operational;
    r2.priority = RequirementPriority::Must;
    doc.add_requirement(r2).expect("valid req2");

    let mut r3 = EngineeringRequirement::new(
        "REQ-TEST-003",
        "Strict policy execution gate",
        EpistemicStatus::ExplicitUserRequirement,
        prov,
        vec!["Sandboxed child process execution".to_string()],
    );
    r3.category = RequirementCategory::Security;
    r3.priority = RequirementPriority::Must;
    doc.add_requirement(r3).expect("valid req3");

    doc
}

#[test]
fn test_architecture_synthesis_greenfield() {
    let charter = sample_charter();
    let reqs = sample_requirements();

    let (arch, adrs) = ArchitectureSynthesizer::synthesize(&reqs, &charter, None, &[], None)
        .expect("synthesis must succeed");

    assert_eq!(arch.project_name, "M31A Architecture Test");
    assert!(!arch.subsystems.is_empty());
    assert!(!arch.components.is_empty());

    // Subsystem IDs are derived from the requirement categories present
    // plus a content hash (e.g. SUB-FUNCTIONAL-a1b2c3). No fixed universal
    // table is emitted: identities differ per target evidence.
    let sub_ids: HashSet<_> = arch.subsystems.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(sub_ids.len(), 3);
    for forbidden in [
        "SUB-KERNEL",
        "SUB-PERSISTENCE",
        "SUB-INTERACTION",
        "SUB-OPERATIONS",
        "SUB-DOMAIN",
        "SUB-SECURITY",
        "SUB-INTEGRATION",
        "SUB-QUALITY",
    ] {
        assert!(
            !sub_ids.contains(forbidden),
            "universal subsystem id {forbidden} must not be emitted"
        );
    }
    for expected_slug in ["SUB-FUNCTIONAL-", "SUB-OPERATIONAL-", "SUB-SECURITY-"] {
        assert!(
            sub_ids.iter().any(|id| id.starts_with(expected_slug)),
            "expected a derived subsystem starting with {expected_slug}, got {sub_ids:?}"
        );
    }

    // Component IDs are derived from the same evidence
    let comp_ids: HashSet<_> = arch.components.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(comp_ids.len(), 3);
    for forbidden in [
        "CMP-ENGINE",
        "CMP-STORAGE",
        "CMP-POLICY",
        "CMP-TRANSPORT",
        "CMP-OPERATIONS",
        "CMP-DOMAIN",
        "CMP-SECURITY",
        "CMP-INTEGRATION",
        "CMP-QUALITY",
    ] {
        assert!(
            !comp_ids.contains(forbidden),
            "universal component id {forbidden} must not be emitted"
        );
    }
    for expected_slug in ["CMP-FUNCTIONAL-", "CMP-OPERATIONAL-", "CMP-SECURITY-"] {
        assert!(
            comp_ids.iter().any(|id| id.starts_with(expected_slug)),
            "expected a derived component starting with {expected_slug}, got {comp_ids:?}"
        );
    }

    for comp in &arch.components {
        assert_eq!(comp.status, ComponentStatus::New);
        assert!(!comp.responsibility.is_empty());
    }

    // Every sample requirement is covered by a derived component
    for key in ["REQ-TEST-001", "REQ-TEST-002", "REQ-TEST-003"] {
        assert!(
            arch.components
                .iter()
                .any(|c| c.requirement_refs.iter().any(|k| k.as_str() == key)),
            "Requirement {} uncovered",
            key
        );
    }

    // Check trust boundaries (derived: the security-category component
    // forms the core; the boundary id binds to the inside-component set).
    assert!(!arch.trust_boundaries.is_empty());
    let tb = &arch.trust_boundaries[0];
    assert!(
        tb.id.starts_with("TB-"),
        "boundary id must be derived, got {}",
        tb.id
    );
    let security_comp = arch
        .components
        .iter()
        .find(|c| c.id.starts_with("CMP-SECURITY-"))
        .expect("derived security component");
    assert!(tb.inside_components.contains(&security_comp.id));
    assert!(
        tb.outside_components
            .contains(&"ExternalClients".to_string())
    );

    // Check candidate ADRs generated
    assert!(!adrs.adrs.is_empty());
    assert!(arch.adr_refs.len() >= 2);
}

#[test]
fn test_architecture_synthesis_brownfield() {
    let charter = sample_charter();
    let reqs = sample_requirements();

    let topology = CodebaseTopology {
        root: PathBuf::from("/tmp/repo"),
        primary_language: "Rust".to_string(),
        detected_frameworks: vec!["tokio".to_string()],
        entry_points: vec!["src/main.rs".to_string()],
        module_tree: vec!["core::kernel".to_string(), "core::storage".to_string()],
        test_frameworks: vec!["cargo test".to_string()],
        ci_cd: vec!["GitHub Actions".to_string()],
        package_manifests: vec!["Cargo.toml".to_string()],
    };
    let brownfield = BrownfieldMap {
        topology,
        existing_conventions: vec!["Strict clippy warnings".to_string()],
        subsystem_boundaries: vec!["core".to_string()],
        delta_scope: Some("WorkflowExtension".to_string()),
    };

    let (arch, _adrs) =
        ArchitectureSynthesizer::synthesize(&reqs, &charter, None, &[], Some(&brownfield))
            .expect("brownfield synthesis must succeed");

    // Pre-existing components should have Existing status
    let existing_comps: Vec<_> = arch
        .components
        .iter()
        .filter(|c| c.status == ComponentStatus::Existing)
        .collect();
    assert_eq!(existing_comps.len(), 2);

    // Delta component should have New status with a derived identity
    // bound to the delta content (SUB-DELTA-{hash} / CMP-DELTA-{hash}).
    let new_comps: Vec<_> = arch
        .components
        .iter()
        .filter(|c| c.status == ComponentStatus::New)
        .collect();
    assert_eq!(new_comps.len(), 1);
    assert!(
        new_comps[0].id.starts_with("CMP-DELTA-"),
        "delta component id must be derived, got {}",
        new_comps[0].id
    );
    let delta_subs: Vec<_> = arch
        .subsystems
        .iter()
        .filter(|s| s.id.starts_with("SUB-DELTA-"))
        .collect();
    assert_eq!(delta_subs.len(), 1);
    assert_eq!(new_comps[0].subsystem, delta_subs[0].id);

    // Legacy components are named after scanned modules.
    let existing_ids: Vec<_> = existing_comps.iter().map(|c| c.id.as_str()).collect();
    assert!(
        existing_ids
            .iter()
            .any(|id| id.starts_with("CMP-CORE-KERNEL-")),
        "expected module-derived legacy id, got {existing_ids:?}"
    );
}

#[test]
fn test_architecture_dag_and_unique_ids() {
    let mut arch = ArchitectureDocument::new("Test", "Context");

    let c1 = ArchitectureComponent::new("CMP-01", "Engine", "SUB-1", "Responsibility 1");
    let c2 = ArchitectureComponent::new("CMP-02", "Storage", "SUB-1", "Responsibility 2")
        .with_depends_on(vec!["CMP-01".to_string()]);

    assert!(arch.add_component(c1).is_ok());
    assert!(arch.add_component(c2).is_ok());

    // Duplicate ID must fail
    let duplicate = ArchitectureComponent::new("CMP-01", "Engine Duplicate", "SUB-1", "Resp");
    assert!(arch.add_component(duplicate).is_err());
}

#[test]
fn test_architecture_quality_gates() {
    let charter = sample_charter();
    let reqs = sample_requirements();

    let (mut arch, adrs) = ArchitectureSynthesizer::synthesize(&reqs, &charter, None, &[], None)
        .expect("synthesis must succeed");

    // Clean architecture passes Tier 1 & Tier 2
    let t1 = PlanningQualityGates::evaluate_architecture_tier1(&arch);
    assert!(
        t1.is_passed,
        "Clean arch must pass Tier 1: {:?}",
        t1.violations
    );

    let t2 = PlanningQualityGates::evaluate_architecture_tier2(&arch, &reqs, &adrs);
    assert!(
        t2.is_passed,
        "Clean arch must pass Tier 2: {:?}",
        t2.violations
    );

    // Tier 1 Failure: Self dependency
    let self_dep_comp = ArchitectureComponent::new("CMP-SELF", "Self", "SUB-1", "Resp")
        .with_depends_on(vec!["CMP-SELF".to_string()]);
    arch.components.push(self_dep_comp);
    let t1_fail = PlanningQualityGates::evaluate_architecture_tier1(&arch);
    assert!(!t1_fail.is_passed);
    assert!(
        t1_fail
            .violations
            .iter()
            .any(|v| v.code == "ARCH_T1_SELF_DEPENDENCY")
    );

    // Tier 1 Failure: Interface provider does not exist
    arch.components.pop(); // Remove self-dep comp
    arch.interfaces.push(ArchitectureInterface::new(
        "IFACE-GHOST",
        "Ghost Interface",
        "CMP-NONEXISTENT",
        "Trait",
        "Desc",
    ));
    let t1_iface_fail = PlanningQualityGates::evaluate_architecture_tier1(&arch);
    assert!(!t1_iface_fail.is_passed);
    assert!(
        t1_iface_fail
            .violations
            .iter()
            .any(|v| v.code == "ARCH_T1_IFACE_PROVIDER")
    );

    // Tier 2 Failure: Component references non-existent requirement
    arch.interfaces.pop(); // Remove bad interface
    arch.components[0]
        .requirement_refs
        .push(RequirementKey::new("REQ-GHOST-999"));
    let t2_fail = PlanningQualityGates::evaluate_architecture_tier2(&arch, &reqs, &adrs);
    assert!(!t2_fail.is_passed);
    assert!(
        t2_fail
            .violations
            .iter()
            .any(|v| v.code == "ARCH_T2_REQ_REF_EXISTS")
    );
}

#[test]
fn test_architecture_markdown_projection_and_persistence() {
    let charter = sample_charter();
    let reqs = sample_requirements();

    let (arch, _adrs) = ArchitectureSynthesizer::synthesize(&reqs, &charter, None, &[], None)
        .expect("synthesis must succeed");

    let md = arch.to_markdown();
    assert!(md.contains("# System Architecture: M31A Architecture Test"));
    assert!(md.contains("## Architectural Goals"));
    assert!(md.contains("## Subsystem Boundaries"));
    assert!(md.contains("## Component Model"));
    // Every derived component id must be projected (no fixed ids remain).
    for comp in &arch.components {
        assert!(
            md.contains(&format!("`{}`", comp.id)),
            "markdown must project derived component {}",
            comp.id
        );
    }
    for forbidden in ["CMP-DOMAIN", "CMP-OPERATIONS", "SUB-DOMAIN", "TB-PRIMARY"] {
        assert!(
            !md.contains(forbidden),
            "markdown must not contain universal id {forbidden}"
        );
    }
    assert!(md.contains("## Security Architecture & Trust Boundaries"));
    assert!(md.contains("## Deployment Topology"));

    let dir = tempdir().unwrap();
    let file_path = arch.save_to_dir(dir.path(), ".planning").unwrap();
    assert!(file_path.exists());
    let disk_content = std::fs::read_to_string(&file_path).unwrap();
    assert_eq!(disk_content, md);
}
