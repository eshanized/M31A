//! Dedicated integration tests for `m31a-fuzzy` (src/candidates.rs).
//!
//! Validates:
//! - `CandidateType` badges across all variants (File, Directory, Skill, Tool, GitCommit, Symbol)
//! - `CandidateType` ordering and equality traits
//! - `Candidate` constructor defaults, builder methods (`with_description`), and score tracking
//! - Serde JSON serialization and round-trip fidelity

use m31a_fuzzy::candidates::{Candidate, CandidateType};

#[test]
fn test_candidate_type_badges_and_variants() {
    assert_eq!(CandidateType::File.badge(), "FILE");
    assert_eq!(CandidateType::Directory.badge(), "DIR");
    assert_eq!(CandidateType::Skill.badge(), "SKILL");
    assert_eq!(CandidateType::Tool.badge(), "TOOL");
    assert_eq!(CandidateType::GitCommit.badge(), "GIT");
    assert_eq!(CandidateType::Symbol.badge(), "SYM");
}

#[test]
fn test_candidate_type_ordering() {
    // Ordering follows derive(PartialOrd, Ord) based on enum variant declaration order
    assert!(CandidateType::File < CandidateType::Directory);
    assert!(CandidateType::Directory < CandidateType::Skill);
    assert!(CandidateType::Skill < CandidateType::Tool);
    assert!(CandidateType::Tool < CandidateType::GitCommit);
    assert!(CandidateType::GitCommit < CandidateType::Symbol);
}

#[test]
fn test_candidate_builder_and_mutation() {
    let mut cand = Candidate::new("m31a-core", CandidateType::Directory, "crates/m31a-core/")
        .with_description("Core workspace abstractions");

    assert_eq!(cand.display_name, "m31a-core");
    assert_eq!(cand.candidate_type, CandidateType::Directory);
    assert_eq!(cand.insert_text, "crates/m31a-core/");
    assert_eq!(
        cand.description.as_deref(),
        Some("Core workspace abstractions")
    );
    assert_eq!(cand.score, 0);
    assert!(cand.match_indices.is_empty());

    // Update match score and indices
    cand.score = 250;
    cand.match_indices = vec![0, 1, 2, 3];
    assert_eq!(cand.score, 250);
    assert_eq!(cand.match_indices, vec![0, 1, 2, 3]);
}

#[test]
fn test_candidate_serde_roundtrip() {
    let cand = Candidate::new("search_code", CandidateType::Tool, "tools:search_code")
        .with_description("AST-indexed code search");

    let json = serde_json::to_string(&cand).expect("serialize candidate");
    let deserialized: Candidate = serde_json::from_str(&json).expect("deserialize candidate");

    assert_eq!(deserialized.display_name, cand.display_name);
    assert_eq!(deserialized.candidate_type, CandidateType::Tool);
    assert_eq!(deserialized.insert_text, "tools:search_code");
    assert_eq!(deserialized.description, cand.description);
}
