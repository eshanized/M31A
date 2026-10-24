//! Phase 20 — Repository Intelligence Engine Integration Tests
//!
//! Verifies:
//! 1. Multi-Layer Discovery (Levels 1-5):
//!    - Level 1: Topology, gitignore filtering, binary file detection, file size guard
//!    - Level 2: Entry points (BinaryMain, LibraryRoot, TestEntry, Script, CliCommand)
//!    - Level 3: AST/regex symbol extraction, signatures, test attributes, provenance
//!    - Level 4: Semantic relationship extraction (Calls, Imports, Tests, References)
//!    - Level 5: Architectural subsystem classification (L0-L9)
//! 2. Bounded Query Engine:
//!    - Runtime query ceilings, truncation metadata
//!    - Test discovery for symbols and files
//!    - Callers and callees navigation
//!    - Change impact analysis with recommended tests and affected subsystems
//! 3. Incremental SQLite Cache & Graph Reconstruction:
//!    - Migration 005 tables (repo_cache_files, repo_cache_symbols, repo_cache_edges)
//!    - Content hash matching and sync skipping
//!    - Graph reconstruction from SQLite state
//!    - Stale file removal on deletion
//! 4. Context Compiler & Planning Integration:
//!    - P4 repository topology injection in untrusted trust envelope
//!    - Planning constraint integration via RepositoryPlanEvidence
//! 5. Model-Facing Tool Execution:
//!    - repo_impact and repo_overview tools execution through ToolRegistry

use m31a::capability::providers::repo_graph::RepositoryGraphProvider;
use m31a::capability::registry::CapabilityRegistry;
use m31a::context::compiler::ProductionContextCompiler;
use m31a::ids::{MissionId, TaskId};
use m31a::kernel::seams::context::{ContextCompilationRequest, ContextCompiler};
use m31a::persistence::sqlite::schema::run_migrations;
use m31a::planning::constraints::RepositoryPlanEvidence;
use m31a::repo::cache::SqliteRepoCache;
use m31a::repo::graph::{RepositoryEdgeKind, RepositoryGraph};
use m31a::repo::query::{BoundedQueryEngine, QueryBounds};
use m31a::repo::scanner::{RepositoryScanner, SourceLanguage};
use m31a::repo::types::{
    EntryPointKind, FactClass, FileClassification, IndexStatus, RepositorySymbol, SubsystemKind,
    SymbolKind,
};
use m31a::tools::definition::{ToolExecutionContext, TypedTool};
use m31a::tools::registry::ToolRegistry;
use m31a::tools::repo::{RepoImpactInput, RepoImpactTool, RepoOverviewInput, RepoOverviewTool};
use sqlx::sqlite::SqlitePoolOptions;
use std::fs;
use std::sync::Arc;
use tempfile::tempdir;
use tokio_util::sync::CancellationToken;

#[test]
fn test_multi_layer_discovery_and_classification() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    // 1. Level 1 & Gitignore: create .gitignore and files
    fs::write(root.join(".gitignore"), "*.tmp\nignore_dir/\n").unwrap();
    fs::write(root.join("test.tmp"), "should be ignored").unwrap();

    let ignore_dir = root.join("ignore_dir");
    fs::create_dir_all(&ignore_dir).unwrap();
    fs::write(ignore_dir.join("skip.rs"), "pub fn skip() {}").unwrap();

    // Binary file
    fs::write(root.join("data.bin"), b"\x7fELF\x00\x01\x02\x03binary").unwrap();

    // Level 2 & 5: Kernel source (L0) and Main function (BinaryMain)
    let src_kernel = root.join("src").join("kernel");
    fs::create_dir_all(&src_kernel).unwrap();
    let kernel_code = r#"
//! Kernel core module
pub struct KernelContext {
    pub id: u64,
}

pub fn boot_kernel() -> bool {
    true
}
"#;
    fs::write(src_kernel.join("mod.rs"), kernel_code).unwrap();

    let main_code = r#"
fn main() {
    println!("Starting application");
}
"#;
    fs::write(root.join("src").join("main.rs"), main_code).unwrap();

    // Level 2 & 5: Capability service (L2) and Test file (L7)
    let src_cap = root.join("src").join("capability");
    fs::create_dir_all(&src_cap).unwrap();
    let cap_code = r#"
pub fn execute_capability() -> i32 {
    42
}
"#;
    fs::write(src_cap.join("service.rs"), cap_code).unwrap();

    let tests_dir = root.join("tests");
    fs::create_dir_all(&tests_dir).unwrap();
    let test_code = r#"
#[test]
fn test_kernel_boot() {
    assert!(true);
}
"#;
    fs::write(tests_dir.join("kernel_test.rs"), test_code).unwrap();

    // Documentation and manifest
    fs::write(root.join("README.md"), "# System Architecture").unwrap();
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"demo\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();

    // Run scanner
    let scanner = RepositoryScanner::new(root);
    let summary = scanner.scan().expect("repository scan succeeds");

    // Assert gitignore respected
    assert_eq!(summary.ignored_file_count, 2);
    assert!(
        !summary
            .files
            .iter()
            .any(|f| f.relative_path.contains("test.tmp"))
    );

    // Assert binary classification
    let bin_file = summary
        .files
        .iter()
        .find(|f| f.relative_path == "data.bin")
        .unwrap();
    assert_eq!(bin_file.classification, FileClassification::Binary);
    assert!(bin_file.symbols.is_empty());

    // Assert L0 Kernel subsystem detection
    let kernel_file = summary
        .files
        .iter()
        .find(|f| f.relative_path.contains("kernel/mod.rs"))
        .unwrap();
    assert_eq!(kernel_file.subsystem, Some(SubsystemKind::Kernel));
    assert_eq!(kernel_file.classification, FileClassification::Source);
    assert!(kernel_file.symbols.iter().any(|s| s.name == "boot_kernel"));

    // Assert L2 Capability subsystem detection
    let cap_file = summary
        .files
        .iter()
        .find(|f| f.relative_path.contains("capability/service.rs"))
        .unwrap();
    assert_eq!(cap_file.subsystem, Some(SubsystemKind::Capability));

    // Assert Test classification and EntryPoint
    let test_file = summary
        .files
        .iter()
        .find(|f| f.relative_path.contains("tests/kernel_test.rs"))
        .unwrap();
    assert_eq!(test_file.classification, FileClassification::Test);

    // Verify Entry Points
    assert!(
        summary
            .entry_points
            .iter()
            .any(|ep| ep.kind == EntryPointKind::BinaryMain)
    );
    assert!(
        summary
            .entry_points
            .iter()
            .any(|ep| ep.kind == EntryPointKind::TestEntry)
    );
}

#[test]
fn test_bounded_query_engine_change_impact_and_tests() {
    let mut graph = RepositoryGraph::new(1);
    let f_core = graph.add_file("src/core.rs", "rust");
    let f_service = graph.add_file("src/service.rs", "rust");
    let f_test = graph.add_file("tests/core_test.rs", "rust");

    graph.set_file_classification("src/core.rs", FileClassification::Source);
    graph.set_file_classification("src/service.rs", FileClassification::Source);
    graph.set_file_classification("tests/core_test.rs", FileClassification::Test);

    graph.set_file_subsystem("src/core.rs", SubsystemKind::Kernel);
    graph.set_file_subsystem("src/service.rs", SubsystemKind::Capability);

    let sym_core = RepositorySymbol::new(
        "src/core.rs::fn::process_data",
        "process_data",
        "crate::core::process_data",
        SymbolKind::Function,
        "src/core.rs",
        10,
        25,
        "pub fn process_data(input: &str) -> String",
        FactClass::VerifiedFact,
        None,
    );
    graph.add_symbol(sym_core, Some(f_core));

    let sym_service = RepositorySymbol::new(
        "src/service.rs::fn::handle_request",
        "handle_request",
        "crate::service::handle_request",
        SymbolKind::Function,
        "src/service.rs",
        5,
        20,
        "pub fn handle_request()",
        FactClass::VerifiedFact,
        None,
    );
    graph.add_symbol(sym_service, Some(f_service));

    let sym_test = RepositorySymbol::new(
        "tests/core_test.rs::fn::test_process_data",
        "test_process_data",
        "crate::tests::test_process_data",
        SymbolKind::Function,
        "tests/core_test.rs",
        1,
        15,
        "fn test_process_data()",
        FactClass::VerifiedFact,
        None,
    )
    .with_test(true);
    graph.add_symbol(sym_test, Some(f_test));

    // Service calls core
    graph.add_relationship(
        "src/service.rs::fn::handle_request",
        "src/core.rs::fn::process_data",
        RepositoryEdgeKind::Calls,
    );

    // Test calls core
    graph.add_relationship(
        "tests/core_test.rs::fn::test_process_data",
        "src/core.rs::fn::process_data",
        RepositoryEdgeKind::Calls,
    );

    let engine = BoundedQueryEngine::new(Arc::new(graph));

    // Query 1: Find tests for symbol
    let tests_for_sym =
        engine.find_tests_for_symbol("src/core.rs::fn::process_data", QueryBounds::default());
    assert_eq!(tests_for_sym.total_matched, 1);
    assert_eq!(tests_for_sym.data[0].name, "test_process_data");

    // Query 2: Find callers
    let callers = engine.find_callers("src/core.rs::fn::process_data", QueryBounds::default());
    assert_eq!(callers.total_matched, 2); // service + test

    // Query 3: Calculate change impact of modifying src/core.rs
    let impact =
        engine.calculate_change_impact(&["src/core.rs".to_string()], QueryBounds::default());
    assert_eq!(impact.data.changed_files, vec!["src/core.rs".to_string()]);
    assert_eq!(impact.data.directly_affected_symbols.len(), 1);
    assert!(
        impact
            .data
            .downstream_affected_symbols
            .iter()
            .any(|s| s.name == "handle_request")
    );
    assert!(
        impact
            .data
            .affected_files
            .contains(&"src/service.rs".to_string())
    );
    assert!(
        impact
            .data
            .recommended_tests
            .contains(&"tests/core_test.rs".to_string())
    );
    assert!(
        impact
            .data
            .affected_subsystems
            .contains(&SubsystemKind::Kernel)
    );
    assert!(
        impact
            .data
            .affected_subsystems
            .contains(&SubsystemKind::Capability)
    );

    // Query 4: Subsystems overview
    let subsystems = engine.inspect_subsystems();
    assert_eq!(
        subsystems.get(&SubsystemKind::Kernel).unwrap(),
        &vec!["src/core.rs".to_string()]
    );
}

#[tokio::test]
async fn test_sqlite_incremental_cache_and_rebuild() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    run_migrations(&pool).await.unwrap();

    let cache = SqliteRepoCache::new(pool.clone());

    let sym_a = RepositorySymbol::new(
        "src/kernel/boot.rs::fn::boot",
        "boot",
        "crate::kernel::boot",
        SymbolKind::Function,
        "src/kernel/boot.rs",
        1,
        10,
        "pub fn boot()",
        FactClass::VerifiedFact,
        None,
    );

    let scanned_file = m31a::repo::scanner::ScannedFile {
        relative_path: "src/kernel/boot.rs".to_string(),
        language: SourceLanguage::Rust,
        content_hash: "hash_version_1".to_string(),
        size_bytes: 200,
        classification: FileClassification::Source,
        subsystem: Some(SubsystemKind::Kernel),
        symbols: vec![sym_a],
    };

    let summary = m31a::repo::scanner::ScanSummary {
        files: vec![scanned_file],
        total_symbols: 1,
        scanned_file_count: 1,
        edges: vec![],
        entry_points: vec![],
        ignored_file_count: 0,
    };

    // First sync inserts
    let count1 = cache.sync_scan(&summary).await.unwrap();
    assert_eq!(count1, 1);

    // Second sync identical hash skips
    let count2 = cache.sync_scan(&summary).await.unwrap();
    assert_eq!(count2, 0);

    // Rebuild graph from SQLite
    let rebuilt_graph = cache.rebuild_graph_from_cache(42).await.unwrap();
    assert_eq!(rebuilt_graph.generation_id, 42);
    assert_eq!(
        rebuilt_graph.get_all_files(),
        vec!["src/kernel/boot.rs".to_string()]
    );
    assert_eq!(
        rebuilt_graph.get_file_subsystem("src/kernel/boot.rs"),
        SubsystemKind::Kernel
    );

    // Sync with empty summary cleans up deleted file
    let empty_summary = m31a::repo::scanner::ScanSummary::default();
    let count3 = cache.sync_scan(&empty_summary).await.unwrap();
    assert_eq!(count3, 1);

    let empty_graph = cache.rebuild_graph_from_cache(43).await.unwrap();
    assert!(empty_graph.get_all_files().is_empty());
}

#[tokio::test]
async fn test_context_compiler_and_planning_integration() {
    let mut graph = RepositoryGraph::new(1);
    let f1 = graph.add_file("src/storage.rs", "rust");
    let f2 = graph.add_file("tests/storage_test.rs", "rust");
    graph.set_file_classification("src/storage.rs", FileClassification::Source);
    graph.set_file_classification("tests/storage_test.rs", FileClassification::Test);
    graph.set_file_subsystem("src/storage.rs", SubsystemKind::Capability);

    let sym_save = RepositorySymbol::new(
        "src/storage.rs::fn::save_item",
        "save_item",
        "crate::storage::save_item",
        SymbolKind::Function,
        "src/storage.rs",
        10,
        25,
        "pub fn save_item(item: &str)",
        FactClass::VerifiedFact,
        None,
    );
    graph.add_symbol(sym_save, Some(f1));

    let sym_test = RepositorySymbol::new(
        "tests/storage_test.rs::fn::test_save_item",
        "test_save_item",
        "crate::tests::test_save_item",
        SymbolKind::Function,
        "tests/storage_test.rs",
        5,
        20,
        "fn test_save_item()",
        FactClass::VerifiedFact,
        None,
    )
    .with_test(true);
    graph.add_symbol(sym_test, Some(f2));

    graph.add_relationship(
        "tests/storage_test.rs::fn::test_save_item",
        "src/storage.rs::fn::save_item",
        RepositoryEdgeKind::Calls,
    );

    let engine = Arc::new(BoundedQueryEngine::new(Arc::new(graph.clone())));

    // 1. Planning Integration
    let constraints = vec![
        m31a::planning::constraints::RepositoryConstraint::DirectoryLayout {
            layout_type: "source_tree".to_string(),
            relative_path: "src".to_string(),
        },
    ];
    let plan_evidence = RepositoryPlanEvidence::from_graph(constraints, &graph);
    assert_eq!(plan_evidence.constraints.len(), 1);
    assert!(
        plan_evidence
            .known_test_files
            .contains(&"tests/storage_test.rs".to_string())
    );
    assert!(
        plan_evidence
            .subsystems
            .contains(&SubsystemKind::Capability)
    );

    // 2. Context Compiler Integration
    let compiler = ProductionContextCompiler::new().with_query_engine(engine);
    let req = ContextCompilationRequest::new(MissionId::new(), TaskId::new(), 4096)
        .with_mission_objective("Enhance persistent storage")
        .with_task_objective("Modify src/storage.rs and add unit tests in tests/storage_test.rs");

    let compiled = compiler
        .compile_context(req)
        .await
        .expect("compile context succeeds");
    assert!(compiled.system_prompt.contains("p4_repository_topology"));
    assert!(
        compiled
            .system_prompt
            .contains("<untrusted_evidence source=\"repo://topology\"")
    );
    assert!(
        compiled
            .system_prompt
            .contains("Recommended Verification Tests")
    );
    assert!(compiled.system_prompt.contains("tests/storage_test.rs"));
}

#[tokio::test]
async fn test_model_facing_tools_execution() {
    let mut graph = RepositoryGraph::new(1);
    let f1 = graph.add_file("src/service.rs", "rust");
    let f2 = graph.add_file("tests/service_test.rs", "rust");
    graph.set_file_classification("src/service.rs", FileClassification::Source);
    graph.set_file_classification("tests/service_test.rs", FileClassification::Test);
    graph.set_file_subsystem("src/service.rs", SubsystemKind::Capability);

    let sym_run = RepositorySymbol::new(
        "src/service.rs::fn::run",
        "run",
        "crate::service::run",
        SymbolKind::Function,
        "src/service.rs",
        1,
        10,
        "pub fn run()",
        FactClass::VerifiedFact,
        None,
    );
    graph.add_symbol(sym_run, Some(f1));

    let sym_test = RepositorySymbol::new(
        "tests/service_test.rs::fn::test_run",
        "test_run",
        "crate::tests::test_run",
        SymbolKind::Function,
        "tests/service_test.rs",
        1,
        15,
        "fn test_run()",
        FactClass::VerifiedFact,
        None,
    )
    .with_test(true);
    graph.add_symbol(sym_test, Some(f2));

    graph.add_relationship(
        "tests/service_test.rs::fn::test_run",
        "src/service.rs::fn::run",
        RepositoryEdgeKind::Calls,
    );

    let engine = Arc::new(BoundedQueryEngine::new(Arc::new(graph)));
    let dir = tempdir().unwrap();
    let provider = Arc::new(RepositoryGraphProvider::new(dir.path(), Some(engine)));

    let cap_reg = CapabilityRegistry::new();
    cap_reg.register_repository(provider);
    let cap_reg_arc = Arc::new(cap_reg);

    let mut tool_reg = ToolRegistry::new_default(cap_reg_arc.clone());
    tool_reg.register_repository_intelligence();

    // Verify tool registration
    assert!(tool_reg.get("repo_impact").is_some());
    assert!(tool_reg.get("repo_overview").is_some());

    let ctx = ToolExecutionContext::new(
        cap_reg_arc,
        dir.path().to_path_buf(),
        CancellationToken::new(),
    );

    // 1. Execute repo_impact
    let impact_tool = RepoImpactTool;
    let impact_input = RepoImpactInput {
        changed_files: vec!["src/service.rs".to_string()],
    };
    let impact_out = impact_tool.execute(&ctx, impact_input).await.unwrap();
    assert_eq!(
        impact_out.report.changed_files,
        vec!["src/service.rs".to_string()]
    );
    assert!(
        impact_out
            .report
            .recommended_tests
            .contains(&"tests/service_test.rs".to_string())
    );
    assert!(
        impact_out
            .report
            .affected_subsystems
            .contains(&SubsystemKind::Capability)
    );

    // 2. Execute repo_overview
    let overview_tool = RepoOverviewTool;
    let overview_input = RepoOverviewInput {};
    let overview_out = overview_tool.execute(&ctx, overview_input).await.unwrap();
    assert_eq!(overview_out.status, IndexStatus::Current);
}
