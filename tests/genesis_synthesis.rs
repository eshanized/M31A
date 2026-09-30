//! Test suite for Project Genesis research synthesis and SUMMARY.md generation (Package 4).

use m31a::workflow::genesis::{
    ProjectCharter, ResearchDimension, ResearchEvidence, ResearchFinding, ResearchSourceType,
    ResearchSummary, ResearchSynthesizer,
};
use tempfile::tempdir;

#[test]
fn test_synthesize_from_structured_findings() {
    let charter = ProjectCharter::new(
        "Mercury Hosting",
        "Build a high-performance Mercurial VCS hosting platform in Rust",
    );

    let mut stack_finding = ResearchFinding::new(
        ResearchDimension::stack(),
        "Async Runtime & Core Crates",
        "Adopt Tokio runtime with Axum for HTTP handling and SQLite via SQLx.",
    );
    stack_finding
        .recommendations
        .push("Use Rust 2024 edition".to_string());
    stack_finding
        .tradeoffs
        .push("SQLx compile-time check vs runtime queries".to_string());
    stack_finding
        .risks
        .push("Mercurial wire protocol streaming memory usage".to_string());
    stack_finding.evidence.push(ResearchEvidence::new(
        ResearchDimension::stack(),
        ResearchSourceType::OfficialDocs,
        "https://tokio.rs",
        "Tokio Async Runtime",
        "Tokio provides reliable asynchronous I/O primitives.",
    ));

    let mut sec_finding = ResearchFinding::new(
        ResearchDimension::security(),
        "SSH & HTTP Authentication",
        "Implement russh for SSH access and Argon2id for password hashing.",
    );
    sec_finding
        .recommendations
        .push("Strict constant-time token comparison".to_string());

    let summary = ResearchSynthesizer::synthesize(&charter, &[stack_finding, sec_finding]).unwrap();

    summary.validate().unwrap();

    assert_eq!(summary.project_name, "Mercury Hosting");
    assert_eq!(summary.consensus_points.len(), 2);
    assert!(!summary.tradeoffs.is_empty());
    assert!(!summary.open_unknowns.is_empty());

    let md = summary.to_markdown();
    assert!(md.contains("# Research Summary: Mercury Hosting"));
    assert!(md.contains("## Executive Summary"));
    assert!(md.contains("## Consensus Recommendations"));
    assert!(md.contains("## Tradeoff Analysis"));
    // Phase 27: finding tradeoffs propagate; no M31A-shaped packaging
    // tradeoff is imposed on the target project.
    assert!(md.contains("SQLx compile-time check vs runtime queries"));
    assert!(!md.contains("Single crate with Rust module boundaries"));
    assert!(!md.contains("Single compiled binary"));
    assert!(md.contains("## Known Uncertainties & Risks"));
}

#[test]
fn test_synthesize_from_dimension_markdown_texts() {
    let charter = ProjectCharter::new("GraphDB", "Embedded graph database in Rust");

    let stack_md = r#"
# Stack Research
Use petgraph and redb for embedded high-performance graph indexing.
## Crates
- petgraph 0.6
- redb 2.0
"#;

    let arch_md = r#"
# Architecture Research
Use append-only write-ahead log with in-memory adjacency index.
## Topologies
- Single process embedded engine
"#;

    let summary = ResearchSynthesizer::synthesize_from_dimension_texts(
        &charter,
        &[("STACK.md", stack_md), ("ARCHITECTURE.md", arch_md)],
    )
    .unwrap();

    summary.validate().unwrap();
    assert_eq!(summary.consensus_points.len(), 2);
    assert_eq!(summary.consensus_points[0].topic, "STACK Findings");
}

#[test]
fn test_summary_save_to_dir() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    let summary = ResearchSummary::new("Test App", "Executive summary of test app");

    let path = summary.save_to_dir(root, ".planning").unwrap();
    assert!(path.exists());
    assert_eq!(path, root.join(".planning/research/SUMMARY.md"));

    let content = std::fs::read_to_string(&path).unwrap();
    assert!(content.contains("# Research Summary: Test App"));
}
