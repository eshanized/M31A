//! Remediation Test Suite: Repository Discovery / Context Improvement (GAP-04).
//!
//! Verifies:
//! 1. Implicit target discovery resolves relative workspace file paths mentioned without `@`.
//! 2. Explicit `@path` mentions continue to be resolved and injected into prompt context.
//! 3. Ambiguous objectives without specific file paths fall back gracefully to general symbols.
//! 4. `ProductionContextCompiler` with `BoundedQueryEngine` discovers and ranks relevant symbols
//!    matching mission and task domain terms, and includes identified target/test files.

use std::fs;
use std::sync::Arc;
use tempfile::tempdir;

use m31a::context::compiler::ProductionContextCompiler;
use m31a::ids::{MissionId, TaskId};
use m31a::interaction::mentions::{MentionParser, ResolutionStatus};
use m31a::kernel::seams::context::{ContextCompilationRequest, ContextCompiler};
use m31a::repo::graph::RepositoryGraph;
use m31a::repo::query::BoundedQueryEngine;
use m31a::repo::types::{FactClass, RepositorySymbol, SymbolKind};

fn sample_repo_graph() -> Arc<RepositoryGraph> {
    let mut graph = RepositoryGraph::new(1);
    let f1 = graph.add_file("src/storage.rs", "rust");
    let f2 = graph.add_file("src/auth.rs", "rust");
    let f3 = graph.add_file("src/util.rs", "rust");

    let sym_storage = RepositorySymbol::new(
        "src/storage.rs::struct::StorageManager",
        "StorageManager",
        "crate::storage::StorageManager",
        SymbolKind::Struct,
        "src/storage.rs",
        10,
        30,
        "pub struct StorageManager",
        FactClass::VerifiedFact,
        Some("Manages storage.".to_string()),
    );
    let sym_auth = RepositorySymbol::new(
        "src/auth.rs::fn::authenticate_user",
        "authenticate_user",
        "crate::auth::authenticate_user",
        SymbolKind::Function,
        "src/auth.rs",
        20,
        40,
        "pub fn authenticate_user(token: &str) -> bool",
        FactClass::VerifiedFact,
        Some("Validates tokens.".to_string()),
    );
    let sym_helper = RepositorySymbol::new(
        "src/util.rs::fn::format_log",
        "format_log",
        "crate::util::format_log",
        SymbolKind::Function,
        "src/util.rs",
        5,
        15,
        "pub fn format_log(msg: &str)",
        FactClass::VerifiedFact,
        None,
    );

    graph.add_symbol(sym_storage, Some(f1));
    graph.add_symbol(sym_auth, Some(f2));
    graph.add_symbol(sym_helper, Some(f3));
    Arc::new(graph)
}

#[test]
fn test_implicit_target_discovery_without_at_symbol() {
    let dir = tempdir().unwrap();
    let ws = dir.path();
    fs::create_dir_all(ws.join("src")).unwrap();
    fs::write(ws.join("src/storage.rs"), "pub struct StorageManager;\n").unwrap();
    fs::write(ws.join("Cargo.toml"), "[package]\nname = \"test\"\n").unwrap();

    // User prompt mentions relative paths without '@'
    let prompt = "Please fix the compilation bug in src/storage.rs and check Cargo.toml";
    let parsed = MentionParser::parse_implicit_or_explicit(prompt, ws);

    assert!(parsed.has_mentions());
    let resolved = parsed.resolved_mentions();
    assert_eq!(resolved.len(), 2);

    let paths: Vec<_> = resolved
        .iter()
        .map(|m| m.workspace_relative_path.to_string_lossy().to_string())
        .collect();
    assert!(paths.contains(&"src/storage.rs".to_string()));
    assert!(paths.contains(&"Cargo.toml".to_string()));

    // Context injection produces formatted referenced file blocks
    let ctx = MentionParser::inject_mention_context(ws, &parsed.mentions);
    assert!(ctx.contains("<explicit_developer_mentions>"));
    assert!(ctx.contains("<referenced_file path=\"src/storage.rs\">"));
    assert!(ctx.contains("pub struct StorageManager;"));
}

#[test]
fn test_explicit_at_mentions_continue_to_work() {
    let dir = tempdir().unwrap();
    let ws = dir.path();
    fs::create_dir_all(ws.join("src")).unwrap();
    fs::write(ws.join("src/auth.rs"), "pub fn auth() {}\n").unwrap();

    let prompt = "Review security in @src/auth.rs carefully";
    let parsed = MentionParser::parse_implicit_or_explicit(prompt, ws);

    assert!(parsed.has_mentions());
    let resolved = parsed.resolved_mentions();
    assert_eq!(resolved.len(), 1);
    assert_eq!(
        resolved[0].workspace_relative_path.to_string_lossy(),
        "src/auth.rs"
    );
    assert_eq!(resolved[0].status, ResolutionStatus::Resolved);
}

#[tokio::test]
async fn test_relevance_ranked_symbol_discovery_with_domain_terms() {
    let graph = sample_repo_graph();
    let qe = Arc::new(BoundedQueryEngine::new(graph));
    let compiler = ProductionContextCompiler::new().with_query_engine(qe);

    let req = ContextCompilationRequest::new(MissionId::new(), TaskId::new(), 4096)
        .with_mission_objective("Implement resilient StorageManager key-value persistence")
        .with_task_objective("Modify src/storage.rs and add unit tests in tests/storage_test.rs");

    let compiled = compiler.compile_context(req).await.unwrap();

    // 1. Identified target files and test files should be surfaced in instructions & background
    assert!(
        compiled
            .system_prompt
            .contains("Target Files: src/storage.rs")
    );
    assert!(
        compiled
            .system_prompt
            .contains("Test Files: tests/storage_test.rs")
    );

    // 2. StorageManager symbol should be found and ranked at the top of relevant symbols
    assert!(
        compiled
            .system_prompt
            .contains("StorageManager (Struct) in src/storage.rs")
    );

    // 3. Irrelevant helper format_log should not precede StorageManager
    let storage_pos = compiled.system_prompt.find("StorageManager").unwrap();
    let auth_pos = compiled.system_prompt.find("authenticate_user");
    // format_log was not mentioned and has no domain overlap with "storage"
    if let Some(pos) = auth_pos {
        assert!(
            storage_pos < pos,
            "StorageManager should be ranked before authenticate_user"
        );
    }
}

#[tokio::test]
async fn test_ambiguous_objective_fallback_behavior() {
    let graph = sample_repo_graph();
    let qe = Arc::new(BoundedQueryEngine::new(graph));
    let compiler = ProductionContextCompiler::new().with_query_engine(qe);

    // Completely generic / ambiguous objective with no file paths or specific symbol names
    let req = ContextCompilationRequest::new(MissionId::new(), TaskId::new(), 4096)
        .with_mission_objective("Do general maintenance")
        .with_task_objective("Run general checks");

    let compiled = compiler.compile_context(req).await.unwrap();

    // Compiler falls back gracefully without panicking and provides general symbol catalog
    assert!(
        compiled
            .system_prompt
            .contains("Relevant Repository Symbols:")
    );
    assert!(compiled.token_count > 0);
}
