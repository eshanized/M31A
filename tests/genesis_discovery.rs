//! Test suite for Socratic discovery, ambiguity assessment, and PROJECT.md projection (Package 4).

use m31a::workflow::genesis::{
    ConvergenceReason, DiscoveryPillar, DiscoverySession, GenesisRequest, ProjectCharter,
    WorkspaceEnvironment,
};
use tempfile::tempdir;

#[test]
fn test_discovery_session_initialization() {
    let dir = tempdir().unwrap();
    let env = WorkspaceEnvironment::probe(dir.path()).unwrap();
    let req = GenesisRequest::new("Build a distributed key-value store", dir.path());

    let session = DiscoverySession::new(req, env);

    assert_eq!(session.current_ambiguity, 95);
    assert!(!session.is_converged);
    assert_eq!(session.turns.len(), 0);
    assert!(session.facts.is_empty());
}

#[test]
fn test_socratic_turn_generation_cycles_four_pillars() {
    let dir = tempdir().unwrap();
    let env = WorkspaceEnvironment::probe(dir.path()).unwrap();
    let req = GenesisRequest::new(
        "Build an embedded analytics database engine in Rust",
        dir.path(),
    );

    let mut session = DiscoverySession::new(req, env);

    let turn1 = session.next_turn().unwrap().unwrap();
    assert_eq!(turn1.turn_number, 1);
    assert_eq!(turn1.pillar_focus, DiscoveryPillar::ProblemAndPersonas);
    assert!(!turn1.options.is_empty());
    assert!(turn1.recommended_option.is_some());
    session
        .submit_response("Data engineers building real-time dashboard aggregation pipelines")
        .unwrap();

    let turn2 = session.next_turn().unwrap().unwrap();
    assert_eq!(turn2.turn_number, 2);
    assert_eq!(turn2.pillar_focus, DiscoveryPillar::BoundariesAndNonGoals);
    session
        .submit_response("Single-node embedded only, no distributed clustering in v1")
        .unwrap();

    let turn3 = session.next_turn().unwrap().unwrap();
    assert_eq!(turn3.turn_number, 3);
    assert_eq!(turn3.pillar_focus, DiscoveryPillar::TechnicalPreferences);
    session
        .submit_response("Rust 2024 edition, memory mapped files via memmap2")
        .unwrap();

    let turn4 = session.next_turn().unwrap().unwrap();
    assert_eq!(turn4.turn_number, 4);
    assert_eq!(turn4.pillar_focus, DiscoveryPillar::OperationalInvariants);
    session
        .submit_response("Apache-2.0 license, sub-1ms point lookup latency")
        .unwrap();

    // After 4 turns, should converge
    assert!(session.is_converged);
    assert!(matches!(
        session.convergence_reason,
        Some(ConvergenceReason::MaxTurnsReached)
            | Some(ConvergenceReason::AmbiguityThresholdReached)
    ));
}

#[test]
fn test_max_turns_reached_with_minimal_input() {
    let dir = tempdir().unwrap();
    let env = WorkspaceEnvironment::probe(dir.path()).unwrap();
    let req = GenesisRequest::new("Build a tool", dir.path());

    let mut session = DiscoverySession::new(req, env);

    for _ in 1..=4 {
        let _ = session.next_turn().unwrap();
        session.submit_response("ok").unwrap();
    }

    assert!(session.is_converged);
    assert_eq!(
        session.convergence_reason,
        Some(ConvergenceReason::MaxTurnsReached)
    );
}

#[test]
fn test_operator_override_skip_convergence() {
    let dir = tempdir().unwrap();
    let env = WorkspaceEnvironment::probe(dir.path()).unwrap();
    let req = GenesisRequest::new("Build a fast markdown renderer", dir.path());

    let mut session = DiscoverySession::new(req, env);

    let _ = session.next_turn().unwrap();
    let converged = session.submit_response("/skip").unwrap();

    assert!(converged);
    assert!(session.is_converged);
    assert_eq!(
        session.convergence_reason,
        Some(ConvergenceReason::OperatorOverride)
    );
    assert!(session.current_ambiguity <= session.request.options.ambiguity_threshold_percent);
}

#[test]
fn test_operator_override_proceed_keyword() {
    let dir = tempdir().unwrap();
    let env = WorkspaceEnvironment::probe(dir.path()).unwrap();
    let req = GenesisRequest::new("Build a fast markdown renderer", dir.path());

    let mut session = DiscoverySession::new(req, env);

    let _ = session.next_turn().unwrap();
    let converged = session.submit_response("good enough, proceed").unwrap();

    assert!(converged);
    assert!(session.is_converged);
    assert_eq!(
        session.convergence_reason,
        Some(ConvergenceReason::OperatorOverride)
    );
}

#[test]
fn test_charter_synthesis_and_markdown_projection() {
    let dir = tempdir().unwrap();
    let env = WorkspaceEnvironment::probe(dir.path()).unwrap();
    let req = GenesisRequest::new("Build a terminal file manager in Rust", dir.path());

    let mut session = DiscoverySession::new(req, env);

    let _ = session.next_turn().unwrap();
    session
        .submit_response("CLI developers needing fast dual-pane navigation")
        .unwrap();

    let _ = session.next_turn().unwrap();
    session
        .submit_response("No GUI support, strict terminal TUI with ratatui")
        .unwrap();

    let _ = session.next_turn().unwrap();
    session.submit_response("/skip").unwrap();

    let charter = session.synthesize_charter().unwrap();
    charter.validate().unwrap();

    assert_eq!(charter.project_name, "Terminal File Manager In Rust");
    assert!(!charter.problem_statement.is_empty());
    assert!(!charter.target_personas.is_empty());
    assert!(!charter.boundaries.in_scope.is_empty());
    assert!(charter.confirmed_by_user);

    // Test Markdown roundtrip
    let md = charter.to_markdown();
    assert!(md.contains("# Project Charter:"));
    assert!(md.contains("## Overview"));
    assert!(md.contains("## Problem Statement"));
    assert!(md.contains("## Scope & Boundaries"));

    let parsed = ProjectCharter::from_markdown(&md).unwrap();
    assert_eq!(parsed.project_name, charter.project_name);
    assert_eq!(parsed.confirmed_by_user, charter.confirmed_by_user);
    assert_eq!(
        parsed.ambiguity_assessment.score_percent,
        charter.ambiguity_assessment.score_percent
    );
}

#[test]
fn test_charter_validation_rejects_empty_fields() {
    let mut charter = ProjectCharter::new("", "overview");
    assert!(charter.validate().is_err());

    charter.project_name = "Valid Project".to_string();
    charter.overview = "".to_string();
    assert!(charter.validate().is_err());

    charter.overview = "Overview".to_string();
    charter.problem_statement = "".to_string();
    assert!(charter.validate().is_err());

    charter.problem_statement = "Problem".to_string();
    assert!(charter.validate().is_err()); // in_scope empty

    charter
        .boundaries
        .in_scope
        .push("Initial feature".to_string());
    assert!(charter.validate().is_ok());
}
