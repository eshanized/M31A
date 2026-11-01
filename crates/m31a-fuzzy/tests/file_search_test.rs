//! Dedicated integration tests for `m31a-fuzzy` (src/file_search.rs).
//!
//! Validates:
//! - Real workspace filesystem traversal and Candidate collection
//! - `.gitignore` parsing, comments handling, and default rule exclusions
//! - Directory vs File candidate differentiation
//! - Max entries capping and caching mechanisms
//! - Ranking and truncation behavior in `WorkspaceScanner::search`

use m31a_fuzzy::candidates::{Candidate, CandidateType};
use m31a_fuzzy::file_search::WorkspaceScanner;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_collect_workspace_files_with_gitignore() {
    let temp = tempdir().unwrap();
    let root = temp.path();

    // 1. Create standard source files
    let src_dir = root.join("src");
    fs::create_dir_all(&src_dir).unwrap();
    fs::write(src_dir.join("main.rs"), b"fn main() {}").unwrap();
    fs::write(src_dir.join("lib.rs"), b"pub fn init() {}").unwrap();

    // 2. Create ignored build artifact directory
    let target_dir = root.join("target");
    fs::create_dir_all(&target_dir).unwrap();
    fs::write(target_dir.join("app"), b"binary").unwrap();

    // 3. Create .git directory
    let git_dir = root.join(".git");
    fs::create_dir_all(&git_dir).unwrap();
    fs::write(git_dir.join("config"), b"git config").unwrap();

    // 4. Create .gitignore with custom rules
    fs::write(
        root.join(".gitignore"),
        b"# Custom comments\nsecret_dir\n*.log\n/build_custom/\n",
    )
    .unwrap();

    // 5. Create files matching gitignore rules
    let secret_dir = root.join("secret_dir");
    fs::create_dir_all(&secret_dir).unwrap();
    fs::write(secret_dir.join("token.txt"), b"secret").unwrap();
    fs::write(root.join("debug.log"), b"log").unwrap();

    let candidates = WorkspaceScanner::collect_workspace_files(root, 100);

    // Verify allowed files are collected
    assert!(
        candidates.iter().any(|c| c.display_name == "src/main.rs" && c.candidate_type == CandidateType::File),
        "src/main.rs must be present"
    );
    assert!(
        candidates.iter().any(|c| c.display_name == "src/lib.rs" && c.candidate_type == CandidateType::File),
        "src/lib.rs must be present"
    );
    assert!(
        candidates.iter().any(|c| c.display_name == "src/" && c.candidate_type == CandidateType::Directory),
        "src/ directory must be present"
    );

    // Verify ignored files are absent
    assert!(
        !candidates.iter().any(|c| c.display_name.contains("target")),
        "target directory must be ignored"
    );
    assert!(
        !candidates.iter().any(|c| c.display_name.contains(".git")),
        ".git directory must be ignored"
    );
    assert!(
        !candidates.iter().any(|c| c.display_name.contains("secret_dir")),
        "secret_dir must be ignored by .gitignore"
    );
    assert!(
        !candidates.iter().any(|c| c.display_name.contains("debug.log")),
        "*.log files must be ignored"
    );
}

#[test]
fn test_collect_workspace_files_max_entries() {
    let temp = tempdir().unwrap();
    let root = temp.path();

    for i in 0..20 {
        fs::write(root.join(format!("file_{:02}.rs", i)), b"").unwrap();
    }

    let capped = WorkspaceScanner::collect_workspace_files(root, 5);
    assert_eq!(capped.len(), 5);
}

#[test]
fn test_cached_workspace_collection() {
    let temp = tempdir().unwrap();
    let root = temp.path();
    fs::write(root.join("initial.rs"), b"").unwrap();

    let files1 = WorkspaceScanner::collect_workspace_files_cached(root, 10);
    assert_eq!(files1.len(), 1);

    // Write a new file immediately; cache within 5000ms should return original snapshot
    fs::write(root.join("second.rs"), b"").unwrap();
    let files2 = WorkspaceScanner::collect_workspace_files_cached(root, 10);
    assert_eq!(files2.len(), 1, "Cached collection should return cached result");
}

#[test]
fn test_workspace_scanner_search_ranking_and_truncation() {
    let mut candidates = Vec::new();
    for i in 0..50 {
        candidates.push(Candidate::new(
            format!("module_{:02}/component.rs", i),
            CandidateType::File,
            format!("module_{:02}/component.rs", i),
        ));
    }
    // Add distinct high-scoring target
    candidates.push(Candidate::new(
        "src/component.rs",
        CandidateType::File,
        "src/component.rs",
    ));

    // 1. Empty query returns first 20
    let empty_res = WorkspaceScanner::search("", &candidates);
    assert_eq!(empty_res.len(), 20);

    // 2. Specific query ranks best match and truncates to 20
    let search_res = WorkspaceScanner::search("component", &candidates);
    assert!(!search_res.is_empty());
    assert!(search_res.len() <= 20);
    // Best match has shortest path and highest boundary score
    assert_eq!(search_res[0].display_name, "src/component.rs");
    assert!(search_res[0].score > 0);
    assert!(!search_res[0].match_indices.is_empty());
}
