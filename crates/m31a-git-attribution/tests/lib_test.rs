//! Dedicated integration tests for `m31a-git-attribution` (src/lib.rs).
//!
//! Validates:
//! - Standardized AI commit trailer generation, composition, and parsing
//! - Real Git repository initialization and commit tree creation (Zero-Mock)
//! - Provenance metrics extraction (total commits, contributors, line counts)
//! - Line-level Git blame extraction and hunk attribution

use git2::{Repository, Signature};
use m31a_git_attribution::{CommitTrailerBuilder, GitAttribution};
use std::fs;
use std::path::Path;
use tempfile::tempdir;

#[test]
fn test_commit_trailer_builder_render_and_parse() {
    let builder = CommitTrailerBuilder::new()
        .with_session_id("sess-9988-uuid")
        .with_model_id("gemini-1.5-pro")
        .with_phase_or_wave("wave-3-execution")
        .with_trailer("Verified-by", "M31A-SafetyGate");

    let headline = "feat: implement high-throughput distributed mesh";
    let body = "Refactors peer discovery and adds heartbeat ping-pong.";

    let full_message = builder.compose_commit_message(headline, Some(body));

    assert!(full_message.starts_with("feat: implement high-throughput distributed mesh\n"));
    assert!(full_message.contains("Refactors peer discovery and adds heartbeat ping-pong."));
    assert!(full_message.contains("Co-authored-by: M31A <m31a@local>"));
    assert!(full_message.contains("X-AI-Session: sess-9988-uuid"));
    assert!(full_message.contains("X-AI-Model: gemini-1.5-pro"));
    assert!(full_message.contains("X-AI-Wave: wave-3-execution"));
    assert!(full_message.contains("Verified-by: M31A-SafetyGate"));

    // Parse trailers back from the generated commit message
    let parsed = CommitTrailerBuilder::parse_trailers(&full_message);
    assert_eq!(
        parsed.get("Co-authored-by").map(|s| s.as_str()),
        Some("M31A <m31a@local>")
    );
    assert_eq!(
        parsed.get("X-AI-Session").map(|s| s.as_str()),
        Some("sess-9988-uuid")
    );
    assert_eq!(
        parsed.get("X-AI-Model").map(|s| s.as_str()),
        Some("gemini-1.5-pro")
    );
    assert_eq!(
        parsed.get("X-AI-Wave").map(|s| s.as_str()),
        Some("wave-3-execution")
    );
    assert_eq!(
        parsed.get("Verified-by").map(|s| s.as_str()),
        Some("M31A-SafetyGate")
    );
}

#[test]
fn test_real_git_attribution_provenance_and_blame() {
    let tmp = tempdir().unwrap();
    let repo_dir = tmp.path();

    // 1. Initialize real Git repo
    let repo = Repository::init(repo_dir).unwrap();

    let sig1 = Signature::now("Alice Engineer", "alice@example.com").unwrap();
    let sig2 = Signature::now("Bob Architect", "bob@example.com").unwrap();

    // 2. First commit by Alice
    let file_path = repo_dir.join("main.rs");
    fs::write(&file_path, "fn main() {\n    println!(\"hello\");\n}\n").unwrap();

    let mut index = repo.index().unwrap();
    index.add_path(Path::new("main.rs")).unwrap();
    index.write().unwrap();
    let tree_id = index.write_tree().unwrap();
    let tree = repo.find_tree(tree_id).unwrap();

    let commit1_id = repo
        .commit(
            Some("HEAD"),
            &sig1,
            &sig1,
            "Initial commit by Alice\n\nCo-authored-by: Alice <alice@example.com>",
            &tree,
            &[],
        )
        .unwrap();

    // 3. Second commit by Bob modifying main.rs
    fs::write(
        &file_path,
        "fn main() {\n    let answer = 42;\n    println!(\"answer: {}\", answer);\n}\n",
    )
    .unwrap();

    index.add_path(Path::new("main.rs")).unwrap();
    index.write().unwrap();
    let tree_id2 = index.write_tree().unwrap();
    let tree2 = repo.find_tree(tree_id2).unwrap();
    let parent_commit = repo.find_commit(commit1_id).unwrap();

    repo.commit(
        Some("HEAD"),
        &sig2,
        &sig2,
        "Update main with answer\n\nCo-authored-by: Bob <bob@example.com>",
        &tree2,
        &[&parent_commit],
    )
    .unwrap();

    // 4. Open with GitAttribution
    let attribution = GitAttribution::open(repo_dir).unwrap();
    let provenance = attribution.get_provenance(10).unwrap();

    assert_eq!(provenance.total_commits, 2, "Should record 2 commits");
    assert_eq!(provenance.total_contributors, 2, "Alice and Bob");
    assert!(provenance.lines_of_code > 0);
    assert_eq!(provenance.recent_commits.len(), 2);

    // 5. Blame verification
    let blame = attribution.get_blame(Path::new("main.rs")).unwrap();
    assert_eq!(blame.file_path, "main.rs");
    assert!(!blame.hunks.is_empty(), "Blame hunks must be present");

    // Ensure blame detects Bob as author of the latest revision
    let bob_hunk = blame.hunks.iter().find(|h| h.author == "Bob Architect");
    assert!(bob_hunk.is_some(), "Blame must attribute edits to Bob");
}
