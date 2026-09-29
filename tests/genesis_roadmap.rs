//! Comprehensive tests for Roadmap DAG Compilation, Kahn's Algorithm, and Quality Gates (Package 5).

use m31a::planning::requirements::{
    EngineeringRequirement, EpistemicStatus, Provenance, ProvenanceSourceType, RequirementCategory,
    RequirementKey, RequirementPriority, TrustLevel,
};
use m31a::workflow::genesis::brownfield::{BrownfieldMap, CodebaseTopology};
use m31a::workflow::genesis::project::ProjectCharter;
use m31a::workflow::planning::architecture_synthesizer::ArchitectureSynthesizer;
use m31a::workflow::planning::requirements::RequirementsDocument;
use m31a::workflow::planning::risks::RiskRegister;
use m31a::workflow::planning::roadmap::{Roadmap, RoadmapPhase, RoadmapSynthesizer};
use m31a::workflow::planning::validation::PlanningQualityGates;
use std::path::PathBuf;
use tempfile::tempdir;

fn sample_charter() -> ProjectCharter {
    let mut charter = ProjectCharter::new(
        "M31A Roadmap Test",
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
    let mut doc = RequirementsDocument::new("M31A Roadmap Test", "Roadmap Scope");
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
        prov.clone(),
        vec!["Sandboxed child processes".to_string()],
    );
    r3.category = RequirementCategory::Security;
    r3.priority = RequirementPriority::Must;
    doc.add_requirement(r3).unwrap();

    let mut r4 = EngineeringRequirement::new(
        "REQ-REL-01",
        "Bounded retry recovery",
        EpistemicStatus::ExplicitUserRequirement,
        prov,
        vec!["Max 3 retry attempts".to_string()],
    );
    r4.category = RequirementCategory::Reliability;
    r4.priority = RequirementPriority::Must;
    doc.add_requirement(r4).unwrap();

    doc
}

#[test]
fn test_greenfield_roadmap_synthesis_and_sequencing() {
    let charter = sample_charter();
    let reqs = sample_requirements();
    let (arch, adrs) =
        ArchitectureSynthesizer::synthesize(&reqs, &charter, None, &[], None).unwrap();
    let risks = RiskRegister::new();

    let roadmap = RoadmapSynthesizer::synthesize(&reqs, &arch, &adrs, &risks, &charter, None)
        .expect("roadmap synthesis succeeds");

    assert_eq!(roadmap.phases.len(), 4);

    // Phase 1: Foundation (Enabling phase)
    assert_eq!(roadmap.phases[0].id, "PHASE-01");
    assert!(roadmap.phases[0].is_enabling_phase);
    assert!(roadmap.phases[0].dependencies.is_empty());

    // Phase 2: Functional Engine
    assert_eq!(roadmap.phases[1].id, "PHASE-02");
    assert_eq!(roadmap.phases[1].dependencies, vec!["PHASE-01".to_string()]);

    // Phase 3: Security & Interfaces
    assert_eq!(roadmap.phases[2].id, "PHASE-03");
    assert_eq!(roadmap.phases[2].dependencies, vec!["PHASE-02".to_string()]);

    // Phase 4: Reliability & Hardening
    assert_eq!(roadmap.phases[3].id, "PHASE-04");
    assert_eq!(roadmap.phases[3].dependencies, vec!["PHASE-03".to_string()]);

    // Verify DAG validation passes
    let topo_order = roadmap
        .validate_dag()
        .expect("DAG must be valid and acyclic");
    assert_eq!(
        topo_order,
        vec!["PHASE-01", "PHASE-02", "PHASE-03", "PHASE-04"]
    );
}

#[test]
fn test_brownfield_roadmap_synthesis() {
    let charter = sample_charter();
    let reqs = sample_requirements();

    let topology = CodebaseTopology {
        root: PathBuf::from("/tmp/repo"),
        primary_language: "Rust".to_string(),
        detected_frameworks: vec!["tokio".to_string()],
        entry_points: vec!["src/main.rs".to_string()],
        module_tree: vec!["core::engine".to_string()],
        test_frameworks: vec!["cargo test".to_string()],
        ci_cd: vec!["GitHub Actions".to_string()],
        package_manifests: vec!["Cargo.toml".to_string()],
    };
    let brownfield = BrownfieldMap {
        topology,
        existing_conventions: vec!["Clippy clean".to_string()],
        subsystem_boundaries: vec!["core".to_string()],
        delta_scope: Some("LinterExtension".to_string()),
    };

    let (arch, adrs) =
        ArchitectureSynthesizer::synthesize(&reqs, &charter, None, &[], Some(&brownfield)).unwrap();
    let risks = RiskRegister::new();

    let roadmap =
        RoadmapSynthesizer::synthesize(&reqs, &arch, &adrs, &risks, &charter, Some(&brownfield))
            .expect("brownfield roadmap synthesis succeeds");

    // Derived structure (Phase 27.5, R-04): baseline + one delivery phase
    // per non-compatibility requirement category present. The fixture has
    // Functional, Operational, Security, and Reliability requirements and no
    // Compatibility requirements, so no regression phase is emitted: 5 phases.
    assert_eq!(roadmap.phases.len(), 5);
    // Phase 1 must be Pre-Flight Baseline
    assert_eq!(roadmap.phases[0].id, "PHASE-01");
    assert!(roadmap.phases[0].name.contains("Pre-Flight"));
    assert!(roadmap.phases[0].is_enabling_phase);
    assert!(roadmap.phases[0].dependencies.is_empty());
    // Baseline references the derived existing subsystem, never SUB-CORE.
    assert!(
        roadmap.phases[0]
            .architecture_refs
            .iter()
            .any(|r| r.starts_with("SUB-EXISTING-")),
        "baseline must reference derived existing subsystem, got {:?}",
        roadmap.phases[0].architecture_refs
    );

    // Phases 2..N are derived delta deliveries chained linearly.
    for (i, phase) in roadmap.phases.iter().enumerate().skip(1) {
        let expected_id = format!("PHASE-{:02}", i + 1);
        assert_eq!(phase.id, expected_id);
        assert!(
            phase.name.starts_with("Delta "),
            "phase name must be derived, got {}",
            phase.name
        );
        assert!(phase.name.contains("LinterExtension"));
        assert_eq!(
            phase.dependencies,
            vec![format!("PHASE-{:02}", i)],
            "phases must chain linearly"
        );
        assert!(
            !phase.requirement_refs.is_empty(),
            "delta phase must schedule requirements"
        );
        // Delta phases reference derived delta architecture, never
        // SUB-DELTA / CMP-DELTA-01.
        assert!(
            phase
                .architecture_refs
                .iter()
                .any(|r| r.starts_with("SUB-DELTA-") || r.starts_with("CMP-DELTA-")),
            "delta phase must reference derived delta ids, got {:?}",
            phase.architecture_refs
        );
        for forbidden in ["SUB-CORE", "SUB-DELTA", "CMP-DELTA-01"] {
            assert!(
                !phase.architecture_refs.contains(&forbidden.to_string()),
                "universal id {forbidden} must not appear"
            );
        }
    }

    // Every requirement is scheduled exactly once across delta phases.
    let mut scheduled: Vec<String> = roadmap
        .phases
        .iter()
        .flat_map(|p| p.requirement_refs.iter().map(|k| k.to_string()))
        .collect();
    scheduled.sort();
    let mut expected: Vec<String> = reqs
        .requirements
        .iter()
        .map(|r| r.key.to_string())
        .collect();
    expected.sort();
    assert_eq!(scheduled, expected);

    // DAG must validate.
    roadmap
        .validate_dag()
        .expect("brownfield DAG must be valid");
}

#[test]
fn test_kahns_algorithm_cycle_and_invalid_dag_detection() {
    let mut roadmap = Roadmap::new("Cycle Test", "Testing cycle detection");

    let p1 = RoadmapPhase::new("P1", "Phase 1", "Obj 1", "Verif 1")
        .with_dependencies(vec!["P3".to_string()])
        .with_exit_criteria(vec!["Exit 1".to_string()]);
    let p2 = RoadmapPhase::new("P2", "Phase 2", "Obj 2", "Verif 2")
        .with_dependencies(vec!["P1".to_string()])
        .with_exit_criteria(vec!["Exit 2".to_string()]);
    let p3 = RoadmapPhase::new("P3", "Phase 3", "Obj 3", "Verif 3")
        .with_dependencies(vec!["P2".to_string()])
        .with_exit_criteria(vec!["Exit 3".to_string()]);

    roadmap.add_phase(p1);
    roadmap.add_phase(p2);
    roadmap.add_phase(p3);

    // Cycle: P1 -> P3 -> P2 -> P1 must fail Kahn's algorithm
    let result = roadmap.validate_dag();
    assert!(result.is_err(), "Cycle must be detected and rejected");

    // Self dependency test
    let mut self_dep_rm = Roadmap::new("Self Dep", "Testing self dependency");
    let p_self = RoadmapPhase::new("P1", "Phase 1", "Obj", "Verif")
        .with_dependencies(vec!["P1".to_string()])
        .with_exit_criteria(vec!["Exit".to_string()]);
    self_dep_rm.add_phase(p_self);
    assert!(
        self_dep_rm.validate_dag().is_err(),
        "Self-dependency must fail"
    );

    // Non-existent dependency test
    let mut non_exist_rm = Roadmap::new("Non-existent Dep", "Testing missing dep");
    let p_missing = RoadmapPhase::new("P1", "Phase 1", "Obj", "Verif")
        .with_dependencies(vec!["P99".to_string()])
        .with_exit_criteria(vec!["Exit".to_string()]);
    non_exist_rm.add_phase(p_missing);
    assert!(
        non_exist_rm.validate_dag().is_err(),
        "Non-existent dependency must fail"
    );
}

#[test]
fn test_roadmap_quality_gates() {
    let charter = sample_charter();
    let reqs = sample_requirements();
    let (arch, adrs) =
        ArchitectureSynthesizer::synthesize(&reqs, &charter, None, &[], None).unwrap();
    let risks = RiskRegister::new();

    let mut roadmap =
        RoadmapSynthesizer::synthesize(&reqs, &arch, &adrs, &risks, &charter, None).unwrap();

    // Clean roadmap passes Tier 1 and Tier 2
    let t1 = PlanningQualityGates::evaluate_roadmap_tier1(&roadmap);
    assert!(
        t1.is_passed,
        "Clean roadmap must pass Tier 1: {:?}",
        t1.violations
    );

    let t2 = PlanningQualityGates::evaluate_roadmap_tier2(&roadmap, &reqs, &arch);
    assert!(
        t2.is_passed,
        "Clean roadmap must pass Tier 2: {:?}",
        t2.violations
    );

    // Tier 1 Failure: Missing exit criteria
    roadmap.phases[0].exit_criteria.clear();
    let t1_fail = PlanningQualityGates::evaluate_roadmap_tier1(&roadmap);
    assert!(!t1_fail.is_passed);
    assert!(
        t1_fail
            .violations
            .iter()
            .any(|v| v.code == "ROADMAP_T1_EXIT_CRITERIA")
    );

    // Tier 2 Failure: Mandatory requirement omitted from all phases
    roadmap.phases[0]
        .exit_criteria
        .push("Restored criteria".to_string());
    // Remove REQ-FUNC-01 from all phases
    for p in &mut roadmap.phases {
        p.requirement_refs.retain(|r| r.as_str() != "REQ-FUNC-01");
    }
    let t2_fail = PlanningQualityGates::evaluate_roadmap_tier2(&roadmap, &reqs, &arch);
    assert!(!t2_fail.is_passed);
    assert!(
        t2_fail
            .violations
            .iter()
            .any(|v| v.code == "ROADMAP_T2_REQ_COVERAGE")
    );

    // Tier 2 Failure: Phase references non-existent architecture component
    roadmap.phases[0]
        .requirement_refs
        .push(RequirementKey::new("REQ-FUNC-01"));
    roadmap.phases[0]
        .architecture_refs
        .push("CMP-NON-EXISTENT".to_string());
    let t2_arch_fail = PlanningQualityGates::evaluate_roadmap_tier2(&roadmap, &reqs, &arch);
    assert!(!t2_arch_fail.is_passed);
    assert!(
        t2_arch_fail
            .violations
            .iter()
            .any(|v| v.code == "ROADMAP_T2_ARCH_REF_EXISTS")
    );
}

#[test]
fn test_roadmap_markdown_projection_and_persistence() {
    let charter = sample_charter();
    let reqs = sample_requirements();
    let (arch, adrs) =
        ArchitectureSynthesizer::synthesize(&reqs, &charter, None, &[], None).unwrap();
    let risks = RiskRegister::new();

    let roadmap =
        RoadmapSynthesizer::synthesize(&reqs, &arch, &adrs, &risks, &charter, None).unwrap();

    let md = roadmap.to_markdown();
    assert!(md.contains("# Engineering Roadmap: M31A Roadmap Test"));
    assert!(md.contains("## Dependency Graph"));
    assert!(md.contains("## Implementation Phases"));
    assert!(md.contains("`PHASE-01`"));
    assert!(md.contains("`PHASE-02`"));
    assert!(md.contains("`PHASE-03`"));
    assert!(md.contains("`PHASE-04`"));
    assert!(md.contains("- **Enabling Phase**: true"));
    assert!(md.contains("- **Verification Strategy**:"));

    let dir = tempdir().unwrap();
    let file_path = roadmap.save_to_dir(dir.path(), ".planning").unwrap();
    assert!(file_path.exists());
    let disk_content = std::fs::read_to_string(&file_path).unwrap();
    assert_eq!(disk_content, md);
}
