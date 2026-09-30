//! Golden End-to-End Integration Test for Project Genesis Planning (Package 5).
//!
//! Synthesizes upstream Genesis discovery + research artifacts into verified:
//! - REQUIREMENTS.md
//! - ARCHITECTURE.md
//! - DECISIONS.md (+ individual adr/ADR-*.md)
//! - RISKS.md
//! - ROADMAP.md
//! - STATE.md
//!
//! Verifies Tier 1 & Tier 2 quality gates, bidirectional traceability,
//! and ensures scope stops strictly at `ImplementationReady` without executing target project code.

use m31a::planning::requirements::RequirementCategory;
use m31a::workflow::genesis::GenesisController;
use m31a::workflow::genesis::brownfield::{BrownfieldMap, CodebaseTopology};
use m31a::workflow::genesis::intake::GenesisOptions;
use m31a::workflow::genesis::project::ProjectCharter;
use m31a::workflow::genesis::provenance::ResearchFinding;
use m31a::workflow::genesis::research_decision::ResearchDimension;
use m31a::workflow::genesis::synthesis::{
    ConsensusPoint, OpenUnknown, ResearchSummary, TradeoffAnalysis,
};
use m31a::workflow::planning::state::PlanningLifecycleState;
use tempfile::tempdir;

fn build_golden_charter() -> ProjectCharter {
    let mut charter = ProjectCharter::new(
        "M31A Autonomous Gateway",
        "High-performance deterministic AI agent gateway and policy controller",
    );
    charter.problem_statement = "Existing agent runtimes leak authorization context and lack deterministic recovery checkpoints".to_string();
    charter.boundaries.in_scope = vec![
        "Deterministic payload parsing and schema validation".to_string(),
        "Transactional checkpointing and WAL state replay".to_string(),
        "Sandboxed capability enforcement for untrusted actions".to_string(),
        "Bounded retry loops with exponential backoff".to_string(),
    ];
    charter.boundaries.non_goals = vec![
        "GUI desktop application".to_string(),
        "Distributed multi-cluster consensus".to_string(),
    ];
    charter.operational_invariants.security_requirements = vec![
        "Enforce strict read-only worktree mounting by default".to_string(),
        "Redact API tokens and secrets from all diagnostic logs".to_string(),
    ];
    charter.operational_invariants.performance_targets =
        vec!["Cold start routing decision completed within 10 milliseconds".to_string()];
    charter
}

fn build_golden_research_summary() -> ResearchSummary {
    let mut summary = ResearchSummary::new(
        "M31A Autonomous Gateway",
        "Research confirms single-crate Rust architecture with embedded SQLite provides optimal safety and predictability.",
    );

    summary.open_unknowns.push(OpenUnknown {
        area: "SQLite Write Concurrency".to_string(),
        risk_level: "Medium".to_string(),
        mitigation_strategy:
            "Configure 5000ms busy timeout and dedicated single-writer background thread"
                .to_string(),
    });

    summary.tradeoffs.push(TradeoffAnalysis {
        tradeoff_axis: "Schema Evolution: Manual SQL Migrations vs ORM".to_string(),
        decision: "Embedded SQL migration scripts managed by deterministic version counter".to_string(),
        rationale: "Eliminates heavy ORM compile-time dependencies while guaranteeing reproducible migrations".to_string(),
    });

    summary.consensus_points.push(ConsensusPoint {
        topic: "Sandboxing Layer".to_string(),
        consensus: "Linux bubblewrap / unshare capability envelopes with process isolation"
            .to_string(),
        supporting_dimensions: vec![
            ResearchDimension::security(),
            ResearchDimension::architecture(),
        ],
    });

    summary
}

fn build_golden_research_findings() -> Vec<ResearchFinding> {
    vec![
        ResearchFinding::new(
            ResearchDimension::security(),
            "Secret Redaction",
            "Scan and scrub regex-matched entropy keys from model diagnostics",
        ),
        ResearchFinding::new(
            ResearchDimension::stack(),
            "Rust Toolchain",
            "Target Rust 2024 edition with strict clippy -D warnings",
        ),
        ResearchFinding::new(
            ResearchDimension::architecture(),
            "State Machine Invariants",
            "Runtime enforces downward layer dependencies and explicit lifecycle state enum transitions",
        ),
        ResearchFinding::new(
            ResearchDimension::deployment(),
            "Resource Limits",
            "Mandate bounded process memory ceilings and timeout bounds for all steps",
        ),
    ]
}

#[test]
fn test_golden_greenfield_planning_pipeline_end_to_end() {
    let temp_workspace = tempdir().expect("temporary directory");
    let workspace_root = temp_workspace.path();

    let charter = build_golden_charter();
    let summary = build_golden_research_summary();
    let findings = build_golden_research_findings();

    let options = GenesisOptions::default();

    let outcome = GenesisController::run_planning(
        &charter,
        Some(&summary),
        &findings,
        None,
        None,
        &options,
        workspace_root,
    )
    .expect("Genesis planning pipeline must complete successfully");

    // 1. Requirements Layer Validation
    assert!(outcome.requirements.requirements.len() >= 6);
    assert!(
        !outcome
            .requirements
            .by_category(RequirementCategory::Functional)
            .is_empty()
    );
    assert!(
        !outcome
            .requirements
            .by_category(RequirementCategory::Security)
            .is_empty()
    );
    assert!(
        !outcome
            .requirements
            .by_category(RequirementCategory::Operational)
            .is_empty()
    );
    assert!(
        !outcome
            .requirements
            .by_category(RequirementCategory::Compatibility)
            .is_empty()
    );
    assert!(outcome.requirements_path.exists());

    // 2. Architecture Layer Validation (derived: one subsystem per
    // evidenced requirement group — operations, domain, security,
    // integration, quality)
    assert_eq!(outcome.architecture.project_name, "M31A Autonomous Gateway");
    assert!(!outcome.architecture.subsystems.is_empty());
    assert!(outcome.architecture.components.len() >= 4);
    assert!(!outcome.architecture.trust_boundaries.is_empty());
    // Trust boundary id is content-derived (TB-{hash of inside set}).
    assert!(
        outcome.architecture.trust_boundaries[0]
            .id
            .starts_with("TB-"),
        "boundary id must be derived, got {}",
        outcome.architecture.trust_boundaries[0].id
    );
    assert!(!outcome.architecture.deployment_boundaries.is_empty());
    assert!(outcome.architecture_path.exists());

    // 3. ADR Registry Validation
    assert!(outcome.adrs.adrs.len() >= 3);
    assert!(outcome.decisions_path.exists());
    let adr_dir = workspace_root.join(".planning/adr");
    assert!(adr_dir.exists());
    assert!(adr_dir.join("ADR-0001.md").exists());
    assert!(adr_dir.join("ADR-0002.md").exists());
    assert!(adr_dir.join("ADR-0003.md").exists());

    // 4. Risk Register Validation
    assert!(!outcome.risks.risks.is_empty());
    assert!(outcome.risks_path.exists());
    assert!(
        outcome
            .risks
            .risks
            .iter()
            .any(|r| r.description.contains("SQLite Write Concurrency"))
    );

    // 5. Roadmap DAG Validation (derived: one phase per subsystem,
    // sequential chain, full requirement coverage)
    assert_eq!(
        outcome.roadmap.phases.len(),
        outcome.architecture.subsystems.len()
    );
    assert!(outcome.roadmap.phases[0].is_enabling_phase);
    for (idx, phase) in outcome.roadmap.phases.iter().enumerate() {
        assert_eq!(phase.id, format!("PHASE-{:02}", idx + 1));
        if idx > 0 {
            assert_eq!(
                phase.dependencies,
                vec![format!("PHASE-{:02}", idx)],
                "Derived roadmap must form a sequential chain"
            );
        }
    }
    assert!(outcome.roadmap_path.exists());

    // 6. Traceability Matrix Validation
    assert!(!outcome.traceability.links.is_empty());
    assert!(outcome.traceability.validate().is_ok());

    // 7. Planning Lifecycle State Validation
    assert_eq!(
        outcome.state.lifecycle_state,
        PlanningLifecycleState::ImplementationReady
    );
    assert_eq!(outcome.state.requirements_status, "Approved");
    assert_eq!(outcome.state.architecture_status, "Approved");
    assert_eq!(outcome.state.roadmap_status, "Approved DAG");
    assert!(outcome.state_path.exists());

    // 8. Strict Scope Boundary Check:
    // Package 5 strictly stops at ImplementationReady.
    // It must NOT write target application code into src/ or execute implementation phases.
    let src_dir = workspace_root.join("src");
    assert!(
        !src_dir.exists(),
        "Package 5 must not generate target project code"
    );

    // Verify all 7 canonical planning files exist on disk in .planning/
    let planning_dir = workspace_root.join(".planning");
    assert!(planning_dir.join("REQUIREMENTS.md").exists());
    assert!(planning_dir.join("ARCHITECTURE.md").exists());
    assert!(planning_dir.join("DECISIONS.md").exists());
    assert!(planning_dir.join("RISKS.md").exists());
    assert!(planning_dir.join("ROADMAP.md").exists());
    assert!(planning_dir.join("STATE.md").exists());
}

#[test]
fn test_golden_brownfield_planning_pipeline_end_to_end() {
    let temp_workspace = tempdir().expect("temporary directory");
    let workspace_root = temp_workspace.path();

    let charter = build_golden_charter();
    let summary = build_golden_research_summary();
    let findings = build_golden_research_findings();

    let topology = CodebaseTopology {
        root: workspace_root.to_path_buf(),
        primary_language: "Rust".to_string(),
        detected_frameworks: vec!["tokio".to_string(), "sqlx".to_string()],
        entry_points: vec!["src/main.rs".to_string()],
        module_tree: vec!["core::kernel".to_string(), "core::policy".to_string()],
        test_frameworks: vec!["cargo test".to_string()],
        ci_cd: vec!["GitHub Actions".to_string()],
        package_manifests: vec!["Cargo.toml".to_string()],
    };
    let brownfield = BrownfieldMap {
        topology,
        existing_conventions: vec![
            "Rust 2024 idioms".to_string(),
            "Zero unwrap policy".to_string(),
        ],
        subsystem_boundaries: vec!["core".to_string()],
        delta_scope: Some("GatewayPluginSubsystem".to_string()),
    };

    let options = GenesisOptions::default();

    let outcome = GenesisController::run_planning(
        &charter,
        Some(&summary),
        &findings,
        Some(&brownfield),
        None,
        &options,
        workspace_root,
    )
    .expect("Brownfield Genesis planning pipeline must complete successfully");

    // Verify brownfield architectural differentiation
    let existing_comps: Vec<_> = outcome
        .architecture
        .components
        .iter()
        .filter(|c| c.status == m31a::workflow::planning::architecture::ComponentStatus::Existing)
        .collect();
    assert_eq!(existing_comps.len(), 2);

    let new_comps: Vec<_> = outcome
        .architecture
        .components
        .iter()
        .filter(|c| c.status == m31a::workflow::planning::architecture::ComponentStatus::New)
        .collect();
    assert_eq!(new_comps.len(), 1);
    // Delta component identity is content-derived (CMP-DELTA-{hash}).
    assert!(
        new_comps[0].id.starts_with("CMP-DELTA-"),
        "delta component id must be derived, got {}",
        new_comps[0].id
    );

    // Verify brownfield roadmap sequencing (Phase 27.5, R-04): baseline +
    // one delivery phase per non-compatibility requirement category present
    // (Functional, Security, Performance, NonFunctional, Operational) + a
    // final regression phase for the Compatibility requirements.
    assert_eq!(outcome.roadmap.phases.len(), 7);
    assert_eq!(outcome.roadmap.phases[0].id, "PHASE-01");
    assert!(outcome.roadmap.phases[0].name.contains("Pre-Flight"));
    assert_eq!(outcome.roadmap.phases[1].id, "PHASE-02");
    assert!(
        outcome.roadmap.phases[1]
            .name
            .contains("GatewayPluginSubsystem")
    );
    assert_eq!(outcome.roadmap.phases[6].id, "PHASE-07");
    assert!(
        outcome.roadmap.phases[6].name.contains("Regression"),
        "final phase must be the derived regression phase, got {}",
        outcome.roadmap.phases[6].name
    );

    // State machine reaches ImplementationReady
    assert_eq!(
        outcome.state.lifecycle_state,
        PlanningLifecycleState::ImplementationReady
    );

    // Verify files on disk
    let planning_dir = workspace_root.join(".planning");
    assert!(planning_dir.join("REQUIREMENTS.md").exists());
    assert!(planning_dir.join("ARCHITECTURE.md").exists());
    assert!(planning_dir.join("DECISIONS.md").exists());
    assert!(planning_dir.join("ROADMAP.md").exists());
    assert!(planning_dir.join("STATE.md").exists());
}
