//! Integration tests for fuzzy matching and candidate ranking.
//!
//! Validates fuzzy scoring algorithms, path matching, and autocomplete query performance.

use m31a_fuzzy::{Candidate, CandidateType, WorkspaceScanner, fuzzy_match, fuzzy_match_indices};
use std::fs;
use tempfile::tempdir;

#[test]
fn test_fuzzy_matcher_scoring_bonuses() {
    // 1. Prefix match should score higher than mid-string match
    let score_prefix = fuzzy_match("main", "main.rs").unwrap();
    let score_mid = fuzzy_match("main", "src/domain/types.rs").unwrap();
    assert!(score_prefix > score_mid);

    // 2. Word boundary after '/' or '_' bonus
    let score_boundary = fuzzy_match("app", "src/app.rs").unwrap();
    let score_scattered = fuzzy_match("app", "grape_pipeline.rs").unwrap();
    assert!(score_boundary > score_scattered);

    // 3. Consecutive match bonus
    let score_consec = fuzzy_match("test", "test_runner.rs").unwrap();
    let score_spread = fuzzy_match("test", "the_end_is_not_first.rs").unwrap();
    assert!(score_consec > score_spread);

    // 4. Non-matching returns None
    assert!(fuzzy_match("xyz", "abc.rs").is_none());
}

#[test]
fn test_fuzzy_match_indices_accuracy() {
    let target = "crates/m31a-tools/src/registry.rs";
    let query = "reg";
    let (score, indices) = fuzzy_match_indices(query, target).unwrap();

    assert!(score > 0);
    assert_eq!(indices.len(), 3);
    let matched_chars: String = indices
        .iter()
        .map(|&i| target.chars().nth(i).unwrap())
        .collect();
    assert_eq!(matched_chars.to_lowercase(), "reg");
}

#[test]
fn test_candidate_badges_and_types() {
    let file_cand = Candidate::new("src/main.rs", CandidateType::File, "src/main.rs")
        .with_description("Entry point");
    assert_eq!(file_cand.candidate_type.badge(), "FILE");
    assert_eq!(file_cand.description.as_deref(), Some("Entry point"));

    assert_eq!(CandidateType::Directory.badge(), "DIR");
    assert_eq!(CandidateType::Skill.badge(), "SKILL");
    assert_eq!(CandidateType::Tool.badge(), "TOOL");
    assert_eq!(CandidateType::GitCommit.badge(), "GIT");
    assert_eq!(CandidateType::Symbol.badge(), "SYM");
}

#[test]
fn test_workspace_scanner_real_fs_collection() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();

    // Create folder structure
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("src").join("main.rs"), "fn main() {}").unwrap();
    fs::write(root.join("src").join("lib.rs"), "pub mod foo;").unwrap();

    // Ignored directories
    fs::create_dir_all(root.join(".git")).unwrap();
    fs::write(root.join(".git").join("config"), "git config").unwrap();
    fs::create_dir_all(root.join("target").join("debug")).unwrap();
    fs::write(root.join("target").join("debug").join("out"), "bin").unwrap();

    let candidates = WorkspaceScanner::collect_workspace_files(root, 100);

    // Should include src, main.rs, lib.rs
    assert!(candidates.iter().any(|c| c.display_name == "src/main.rs"));
    assert!(candidates.iter().any(|c| c.display_name == "src/lib.rs"));

    // Should NOT include .git or target
    assert!(!candidates.iter().any(|c| c.display_name.contains(".git")));
    assert!(!candidates.iter().any(|c| c.display_name.contains("target")));
}

#[test]
fn test_workspace_scanner_ranked_search() {
    let candidates = vec![
        Candidate::new(
            "src/utils/file_helper.rs",
            CandidateType::File,
            "src/utils/file_helper.rs",
        ),
        Candidate::new("src/file.rs", CandidateType::File, "src/file.rs"),
        Candidate::new("README.md", CandidateType::File, "README.md"),
    ];

    let results = WorkspaceScanner::search("file", &candidates);
    assert_eq!(results.len(), 2);
    // src/file.rs should rank first because of word boundary / shorter path
    assert_eq!(results[0].display_name, "src/file.rs");
    assert_eq!(results[1].display_name, "src/utils/file_helper.rs");
}
