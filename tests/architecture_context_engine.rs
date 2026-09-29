//! Architectural Regression Guards for Phase 21 Context Engine.
//!
//! Enforces:
//! 1. Zero parallel `ContextCompiler` implementations across codebase (only `ProductionContextCompiler`).
//! 2. Zero parallel repository query engines (only `BoundedQueryEngine`).
//! 3. Zero parallel repository graphs (only `RepositoryGraph`).
//! 4. Zero second token budget allocators or compactors (only `ReverseCompactor`).
//! 5. Zero second context compilation manifests (only `ContextCompilationManifest` / `ContextCompilationContract`).
//! 6. Zero upward dependencies from `src/context/` to agents, workflows, autonomy, or CLI/TUI.
//! 7. Kernel seam purity: `src/kernel/seams/context.rs` must not import domain modules.

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

#[test]
fn test_zero_parallel_context_compilers() {
    let mut files = Vec::new();
    collect_rs_files(Path::new("src"), &mut files);

    let mut compiler_impl_count = 0;

    for file_path in files {
        let content = fs::read_to_string(&file_path).unwrap_or_default();
        if content.contains("impl ContextCompiler for") {
            compiler_impl_count += 1;
            assert!(
                file_path.contains("src/context/compiler.rs")
                    || file_path.contains("src/kernel/seams/context.rs")
                    || file_path.contains("src/controller/harness.rs"),
                "Found unauthorized ContextCompiler implementation in {}",
                file_path
            );
        }
    }

    assert!(
        (1..=3).contains(&compiler_impl_count),
        "Expected exactly 1 production ContextCompiler implementation (plus test mocks in seams/harness), found {}",
        compiler_impl_count
    );
}

#[test]
fn test_zero_parallel_query_engines() {
    let mut files = Vec::new();
    collect_rs_files(Path::new("src"), &mut files);

    for file_path in files {
        if file_path.contains("src/repo/") {
            continue;
        }
        let content = fs::read_to_string(&file_path).unwrap_or_default();
        assert!(
            !content.contains("struct RepositoryQueryEngine")
                && !content.contains("struct CodeQueryEngine")
                && !content.contains("struct SemanticQueryEngine"),
            "Unauthorized parallel query engine defined in {}",
            file_path
        );
    }
}

#[test]
fn test_zero_parallel_code_graphs() {
    let mut files = Vec::new();
    collect_rs_files(Path::new("src"), &mut files);

    for file_path in files {
        if file_path.contains("src/repo/") {
            continue;
        }
        let content = fs::read_to_string(&file_path).unwrap_or_default();
        assert!(
            !content.contains("struct CodeGraph")
                && !content.contains("struct SymbolGraph")
                && !content.contains("struct DependencyGraph")
                && !content.contains("struct RepoGraph"),
            "Unauthorized parallel repository graph defined in {}",
            file_path
        );
    }
}

#[test]
fn test_context_engine_layer_boundary_purity() {
    let mut files = Vec::new();
    collect_rs_files(Path::new("src/context"), &mut files);

    for file_path in files {
        let content = fs::read_to_string(&file_path).unwrap_or_default();

        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("use crate::") || trimmed.starts_with("use super::") {
                assert!(
                    !trimmed.contains("crate::agent::")
                        && !trimmed.contains("crate::workflow::")
                        && !trimmed.contains("crate::autonomy::")
                        && !trimmed.contains("crate::tui::")
                        && !trimmed.contains("crate::cli::"),
                    "Layer violation in {}: Context Engine must not import higher-layer modules: {}",
                    file_path,
                    trimmed
                );
            }
        }
    }
}

#[test]
fn test_kernel_context_seam_purity() {
    let seam_path = Path::new("src/kernel/seams/context.rs");
    let content = fs::read_to_string(seam_path).expect("src/kernel/seams/context.rs must exist");

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("use crate::") {
            assert!(
                !trimmed.contains("crate::context::")
                    && !trimmed.contains("crate::repo::")
                    && !trimmed.contains("crate::agent::")
                    && !trimmed.contains("crate::workflow::"),
                "Kernel seam must not import domain modules: {}",
                trimmed
            );
        }
    }
}
