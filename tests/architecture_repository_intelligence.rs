//! Phase 20 — Repository Intelligence Engine Architectural Regression Guards
//!
//! Enforces:
//! 1. Single canonical graph: RepositoryGraph in `src/repo/graph.rs` is the only graph struct.
//!    No parallel `CallGraph`, `SymbolGraph`, `FileGraph`, or `DependencyGraph`.
//! 2. Single canonical scanner: RepositoryScanner in `src/repo/scanner.rs` is the only scanner.
//!    No parallel `CodeScanner`, `ProjectScanner`, or `ASTScanner`.
//! 3. Zero inline DDL in repo module: Storage relies exclusively on Migration 005.
//! 4. Architectural hierarchy: `src/repo/` must not depend upward into higher layers
//!    (agents, planning, execution, verification, autonomy, cli, tui, tools).
//! 5. Zero shell execution: `src/repo/` must not invoke `std::process::Command` or `tokio::process::Command`.
//! 6. Query bounds ceilings: Query bounds strictly enforce non-bypassable limits.

use std::fs;
use std::path::Path;

fn collect_rs_files(dir: &Path, out: &mut Vec<String>) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_rs_files(&path, out);
            } else if path.extension().and_then(|e| e.to_str()) == Some("rs")
                && let Some(p) = path.to_str()
            {
                out.push(p.to_string());
            }
        }
    }
}

fn find_violations(dir: &str, forbidden_patterns: &[&str]) -> Vec<(String, String, usize, String)> {
    let mut files = Vec::new();
    let root = Path::new(dir);
    if root.exists() {
        collect_rs_files(root, &mut files);
    }

    let mut violations = Vec::new();
    for file in files {
        if let Ok(content) = fs::read_to_string(&file) {
            for (line_no, line) in content.lines().enumerate() {
                for pattern in forbidden_patterns {
                    if line.contains(pattern) {
                        violations.push((
                            file.clone(),
                            pattern.to_string(),
                            line_no + 1,
                            line.to_string(),
                        ));
                    }
                }
            }
        }
    }
    violations
}

#[test]
fn test_no_parallel_code_graphs() {
    let mut files = Vec::new();
    collect_rs_files(Path::new("src"), &mut files);

    let forbidden_structs = [
        "struct CallGraph",
        "struct SymbolGraph",
        "struct FileGraph",
        "struct DependencyGraph",
        "enum CallGraph",
        "enum SymbolGraph",
    ];

    let mut violations = Vec::new();
    for file in files {
        if let Ok(content) = fs::read_to_string(&file) {
            for (line_no, line) in content.lines().enumerate() {
                for pat in forbidden_structs {
                    if line.contains(pat) {
                        violations.push((file.clone(), line_no + 1, line.to_string()));
                    }
                }
            }
        }
    }

    assert!(
        violations.is_empty(),
        "Detected parallel code graph definitions: {:?}",
        violations
    );
}

#[test]
fn test_no_parallel_scanners() {
    let mut files = Vec::new();
    collect_rs_files(Path::new("src"), &mut files);

    let forbidden_scanners = [
        "struct CodeScanner",
        "struct ProjectScanner",
        "struct ASTScanner",
        "struct RepoIndexer",
    ];

    let mut violations = Vec::new();
    for file in files {
        if let Ok(content) = fs::read_to_string(&file) {
            for (line_no, line) in content.lines().enumerate() {
                for pat in forbidden_scanners {
                    if line.contains(pat) {
                        violations.push((file.clone(), line_no + 1, line.to_string()));
                    }
                }
            }
        }
    }

    assert!(
        violations.is_empty(),
        "Detected parallel repository scanners: {:?}",
        violations
    );
}

#[test]
fn test_no_inline_ddl_in_repo_module() {
    let forbidden_ddl = [
        "CREATE TABLE",
        "create table",
        "DROP TABLE",
        "drop table",
        "ALTER TABLE",
        "alter table",
    ];

    let violations = find_violations("src/repo", &forbidden_ddl);
    assert!(
        violations.is_empty(),
        "Detected inline DDL inside src/repo: {:?}",
        violations
    );
}

#[test]
fn test_repo_has_zero_upward_dependencies() {
    let forbidden = [
        "crate::agent::",
        "crate::agents::",
        "crate::planning::",
        "crate::dag::",
        "crate::scheduler::",
        "crate::execution::",
        "crate::job::",
        "crate::verification::",
        "crate::recovery::",
        "crate::autonomy::",
        "crate::mission::",
        "crate::controller::",
        "crate::cli::",
        "crate::tui::",
        "crate::tools::",
        "use crate::agent",
        "use crate::agents",
        "use crate::planning",
        "use crate::dag",
        "use crate::scheduler",
        "use crate::execution",
        "use crate::verification",
        "use crate::autonomy",
        "use crate::cli",
        "use crate::tui",
        "use crate::tools",
    ];

    let violations = find_violations("src/repo", &forbidden);
    assert!(
        violations.is_empty(),
        "Detected upward architectural dependencies from src/repo: {:?}",
        violations
    );
}

#[test]
fn test_repo_has_zero_shell_execution() {
    let forbidden = [
        "std::process::Command",
        "tokio::process::Command",
        "Command::new",
    ];

    let violations = find_violations("src/repo", &forbidden);
    assert!(
        violations.is_empty(),
        "Detected shell command execution inside src/repo: {:?}",
        violations
    );
}

#[test]
fn test_query_bounds_runtime_ceilings_invariant() {
    use m31a::repo::query::{
        CEILING_MAX_BYTES, CEILING_MAX_DEPTH, CEILING_MAX_RESULTS, QueryBounds,
    };

    let unbounded = QueryBounds::new(100_000, 50, 100_000_000);
    let clamped = unbounded.clamped();

    assert_eq!(clamped.max_results, CEILING_MAX_RESULTS);
    assert_eq!(clamped.max_depth, CEILING_MAX_DEPTH);
    assert_eq!(clamped.max_bytes, CEILING_MAX_BYTES);
    assert!(clamped.max_results <= 200);
    assert!(clamped.max_depth <= 4);
    assert!(clamped.max_bytes <= 65536);
}
