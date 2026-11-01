//! Dedicated integration tests for `m31a-fuzzy` (src/lib.rs).
//!
//! Validates:
//! - Root re-exports: `Candidate`, `CandidateType`, `WorkspaceScanner`, `fuzzy_match`, `fuzzy_match_indices`
//! - Full pipeline: scanning files, searching candidates, and extracting matching character spans

use m31a_fuzzy::{
    Candidate, CandidateType, WorkspaceScanner, fuzzy_match, fuzzy_match_indices,
};
use std::fs;
use tempfile::tempdir;

#[test]
fn test_root_reexports() {
    let cand = Candidate::new("test", CandidateType::Skill, "test");
    assert_eq!(cand.candidate_type.badge(), "SKILL");

    let score = fuzzy_match("cargo", "cargo.toml");
    assert!(score.is_some());

    let (score, indices) = fuzzy_match_indices("abc", "a_b_c").unwrap();
    assert!(score > 0);
    assert_eq!(indices, vec![0, 2, 4]);
}

#[test]
fn test_end_to_end_fuzzy_search_pipeline() {
    let temp = tempdir().unwrap();
    let root = temp.path();

    let src = root.join("crates").join("engine").join("src");
    fs::create_dir_all(&src).unwrap();
    fs::write(src.join("executor.rs"), b"").unwrap();
    fs::write(src.join("evaluator.rs"), b"").unwrap();

    let candidates = WorkspaceScanner::collect_workspace_files(root, 100);
    assert!(!candidates.is_empty());

    let search_results = WorkspaceScanner::search("exec", &candidates);
    assert!(!search_results.is_empty());
    assert_eq!(search_results[0].display_name, "crates/engine/src/executor.rs");
    assert_eq!(search_results[0].candidate_type, CandidateType::File);
    assert!(search_results[0].score > 0);
}
