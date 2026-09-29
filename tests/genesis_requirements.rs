//! Comprehensive tests for Requirements Synthesis, normalization, and quality gates (Package 5).

use m31a::planning::requirements::{
    EngineeringRequirement, EpistemicStatus, Provenance, ProvenanceSourceType, RequirementCategory,
    TrustLevel,
};
use m31a::workflow::genesis::brownfield::{BrownfieldMap, CodebaseTopology};
use m31a::workflow::genesis::project::ProjectCharter;
use m31a::workflow::genesis::provenance::ResearchFinding;
use m31a::workflow::genesis::research_decision::ResearchDimension;
use m31a::workflow::genesis::synthesis::{
    ConsensusPoint, OpenUnknown, ResearchSummary, TradeoffAnalysis,
};
use m31a::workflow::planning::requirements::RequirementsDocument;
use m31a::workflow::planning::requirements_synthesizer::RequirementsSynthesizer;
use m31a::workflow::planning::validation::PlanningQualityGates;
use std::path::PathBuf;
use tempfile::tempdir;

fn sample_charter() -> ProjectCharter {
    let mut charter = ProjectCharter::new(
        "M31A Test Project",
        "Autonomous code review and repository refactoring platform",
    );
    charter.problem_statement =
        "Teams struggle with slow manual code review and brittle refactoring workflows".to_string();
    charter.boundaries.in_scope = vec![
        "Autonomous git patch inspection".to_string(),
        "AST-based semantic diff analysis".to_string(),
        "Deterministic test execution gate".to_string(),
    ];
    charter.boundaries.non_goals = vec!["Cloud IDE hosting".to_string()];
    charter.operational_invariants.security_requirements = vec![
        "Strict sandbox isolation for external test commands".to_string(),
        "Redaction of secrets from all diagnostics".to_string(),
    ];
    charter.operational_invariants.performance_targets =
        vec!["Cold start analysis completed within 5 seconds".to_string()];
    charter
}

#[test]
fn test_requirements_extraction_and_stable_ids() {
    let charter = sample_charter();
    let findings = vec![
        ResearchFinding::new(
            ResearchDimension::security(),
            "Sandboxing Policy",
            "Enforce cgroups and read-only worktree mounting",
        ),
        ResearchFinding::new(
            ResearchDimension::stack(),
            "Rust Toolchain",
            "Pin to Rust 2024 edition with strict clippy checks",
        ),
    ];

    let doc = RequirementsSynthesizer::synthesize(&charter, None, &findings, None, None)
        .expect("synthesis succeeds");

    assert!(!doc.requirements.is_empty());

    // Verify stable ID prefixes
    let func_reqs: Vec<_> = doc
        .requirements
        .iter()
        .filter(|r| r.category == RequirementCategory::Functional)
        .collect();
    let sec_reqs: Vec<_> = doc
        .requirements
        .iter()
        .filter(|r| r.category == RequirementCategory::Security)
        .collect();
    let compat_reqs: Vec<_> = doc
        .requirements
        .iter()
        .filter(|r| r.category == RequirementCategory::Compatibility)
        .collect();

    assert!(
        func_reqs
            .iter()
            .all(|r| r.key.as_str().starts_with("REQ-FUNC-"))
    );
    assert!(
        sec_reqs
            .iter()
            .all(|r| r.key.as_str().starts_with("REQ-SEC-"))
    );
    assert!(
        compat_reqs
            .iter()
            .all(|r| r.key.as_str().starts_with("REQ-COMPAT-"))
    );

    // Verify deterministic ordering
    for i in 1..doc.requirements.len() {
        assert!(doc.requirements[i - 1].key <= doc.requirements[i].key);
    }
}

#[test]
fn test_requirements_duplicate_detection() {
    let mut doc = RequirementsDocument::new("Test", "Scope");
    let prov = Provenance::new(
        ProvenanceSourceType::RepositoryFile,
        TrustLevel::VerifiedRepository,
        "test",
    );

    let req1 = EngineeringRequirement::new(
        "REQ-FUNC-01",
        "Parse AST diffs",
        EpistemicStatus::ExplicitUserRequirement,
        prov.clone(),
        vec!["Pass test".to_string()],
    );
    let req2 = EngineeringRequirement::new(
        "REQ-FUNC-01",
        "Duplicate ID with different statement",
        EpistemicStatus::ExplicitUserRequirement,
        prov,
        vec!["Pass test".to_string()],
    );

    assert!(doc.add_requirement(req1).is_ok());
    let err = doc.add_requirement(req2);
    assert!(err.is_err());
}

#[test]
fn test_acceptance_criteria_and_provenance() {
    let charter = sample_charter();
    let doc = RequirementsSynthesizer::synthesize(&charter, None, &[], None, None)
        .expect("synthesis succeeds");

    for r in &doc.requirements {
        assert!(!r.statement().trim().is_empty());
        assert!(!r.provenance.actor.trim().is_empty());
        assert!(r.provenance.location.is_some());
        if r.current_status != EpistemicStatus::Unknown {
            assert!(
                !r.satisfaction_criteria.is_empty(),
                "Requirement {} lacks criteria",
                r.key
            );
        }
    }
}

#[test]
fn test_unknown_and_tradeoff_preservation() {
    let charter = sample_charter();
    let mut summary = ResearchSummary::new(&charter.project_name, "Executive summary");
    summary.open_unknowns.push(OpenUnknown {
        area: "Database Concurrency".to_string(),
        risk_level: "High".to_string(),
        mitigation_strategy: "Benchmark SQLite WAL under concurrent writes".to_string(),
    });
    summary.tradeoffs.push(TradeoffAnalysis {
        tradeoff_axis: "Memory vs Speed".to_string(),
        decision: "In-memory caching".to_string(),
        rationale: "Analysis requires sub-second queries".to_string(),
    });
    summary.consensus_points.push(ConsensusPoint {
        topic: "Compiler Target".to_string(),
        consensus: "x86_64-unknown-linux-gnu baseline".to_string(),
        supporting_dimensions: vec![ResearchDimension::stack(), ResearchDimension::deployment()],
    });

    let doc = RequirementsSynthesizer::synthesize(&charter, Some(&summary), &[], None, None)
        .expect("synthesis succeeds");

    assert!(!doc.unknown_requirements.is_empty());
    assert!(doc.unknown_requirements[0].contains("Database Concurrency"));
    assert!(!doc.requirement_decisions.is_empty());
    assert!(doc.requirement_decisions[0].contains("Memory vs Speed"));
}

#[test]
fn test_brownfield_delta_requirements() {
    let charter = sample_charter();
    let topology = CodebaseTopology {
        root: PathBuf::from("/tmp/repo"),
        primary_language: "Rust".to_string(),
        detected_frameworks: vec!["tokio".to_string()],
        entry_points: vec!["src/main.rs".to_string()],
        module_tree: vec!["src/parser/".to_string()],
        test_frameworks: vec!["cargo test".to_string()],
        ci_cd: vec!["GitHub Actions".to_string()],
        package_manifests: vec!["Cargo.toml".to_string()],
    };
    let brownfield = BrownfieldMap {
        topology,
        existing_conventions: vec!["Strict clippy warnings".to_string()],
        subsystem_boundaries: vec!["src/parser".to_string()],
        delta_scope: Some("Linter Extension".to_string()),
    };

    let doc = RequirementsSynthesizer::synthesize(&charter, None, &[], Some(&brownfield), None)
        .expect("synthesis succeeds");

    let compat_reqs = doc.by_category(RequirementCategory::Compatibility);
    assert!(!compat_reqs.is_empty());
    assert!(
        compat_reqs
            .iter()
            .any(|r| r.statement().contains("Rust codebase"))
    );
    assert!(compat_reqs.iter().any(|r| {
        r.satisfaction_criteria
            .iter()
            .any(|c| c.contains("Strict clippy warnings"))
    }));
}

#[test]
fn test_requirements_quality_gates() {
    let charter = sample_charter();
    let doc = RequirementsSynthesizer::synthesize(&charter, None, &[], None, None)
        .expect("synthesis succeeds");

    let t1 = PlanningQualityGates::evaluate_requirements_tier1(&doc);
    assert!(t1.is_passed, "Tier 1 must pass: {:?}", t1.violations);

    let t2 = PlanningQualityGates::evaluate_requirements_tier2(&doc);
    assert!(t2.is_passed, "Tier 2 must pass: {:?}", t2.violations);
}

#[test]
fn test_requirements_markdown_projection() {
    let charter = sample_charter();
    let doc = RequirementsSynthesizer::synthesize(&charter, None, &[], None, None)
        .expect("synthesis succeeds");

    let dir = tempdir().unwrap();
    let path = doc
        .save_to_dir(dir.path(), ".planning")
        .expect("save succeeds");
    assert!(path.exists());

    let content = std::fs::read_to_string(&path).expect("read succeeds");
    assert!(content.contains("# Requirements:"));
    assert!(content.contains("## Scope"));
    assert!(content.contains("## Functional Requirements"));
    assert!(content.contains("## Security Requirements"));
    assert!(content.contains("## Acceptance Criteria"));
    assert!(content.contains("## Traceability"));
}
