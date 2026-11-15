//! Codebase-wide AST symbol index, multi-language relationship engine, and cross-reference search.

use crate::chunker::AstChunker;
use crate::graph::{ModuleGraph, PackageGraph, SymbolGraph};
use crate::language::LanguageAdapterRegistry;
use crate::parser::AstParser;
use crate::retrieval::{ContextPacket, RetrievalQuery, SemanticRetriever};
use crate::test_discovery::{RelatedTestTarget, TestDiscoveryEngine};
use crate::types::{
    AstSymbol, CallReference, IndexStatus, ParserDiagnostic, SemanticChunk, SymbolRelationship,
};
use crate::workspace::{TraversalOptions, WorkspaceDiscovery, WorkspaceInventory};
use anyhow::Result;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

/// A hierarchical call graph node representing an entry point and its downstream callees.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct CallGraphNode {
    pub symbol_name: String,
    pub file_path: PathBuf,
    pub line: usize,
    pub callees: Vec<CallGraphNode>,
}

#[derive(Debug, Clone, Default)]
pub struct AstCodebaseIndex {
    root_path: PathBuf,
    symbols_by_name: HashMap<String, Vec<AstSymbol>>,
    symbols_by_file: HashMap<PathBuf, Vec<AstSymbol>>,
    symbols_by_id: HashMap<String, AstSymbol>,
    relationships: Vec<SymbolRelationship>,
    file_hashes: HashMap<PathBuf, String>,
    test_files: Vec<PathBuf>,
    symbol_graph: SymbolGraph,
    package_graph: PackageGraph,
    module_graph: ModuleGraph,
    inventory: Option<WorkspaceInventory>,
    diagnostics: Vec<ParserDiagnostic>,
    status: IndexStatus,
    files_indexed: usize,
}

impl AstCodebaseIndex {
    pub fn new() -> Self {
        Self::default()
    }

    /// Recursively scans workspace and builds an authoritative in-memory AST and relationship index.
    pub fn build(root: &Path, max_files: usize) -> Self {
        let options = TraversalOptions {
            max_files,
            ..Default::default()
        };
        Self::build_with_options(root, options)
    }

    /// Recursively scans workspace with custom traversal options and builds the complete index.
    pub fn build_with_options(root: &Path, options: TraversalOptions) -> Self {
        let mut index = Self::new();
        index.root_path = root.to_path_buf();

        let inventory = WorkspaceDiscovery::discover_workspace(root);
        index.package_graph = PackageGraph::build(&inventory);

        let scan_report = WorkspaceDiscovery::scan_files(root, &options);
        index.status = scan_report.status;
        index.inventory = Some(inventory);

        let registry = LanguageAdapterRegistry::new();

        for file in &scan_report.files {
            if file.is_test {
                index.test_files.push(file.path.clone());
            }

            if let Ok(content) = fs::read_to_string(&file.path) {
                let hash = compute_hash(&content);
                index.file_hashes.insert(file.path.clone(), hash);

                let parse_result = registry.parse_file(&file.path, &content);
                index.diagnostics.extend(parse_result.diagnostics);

                // If specialized adapter returned symbols, use those; otherwise fall back to AstParser
                let syms = if !parse_result.symbols.is_empty() {
                    parse_result.symbols
                } else {
                    AstParser::parse_file(&file.path, &content)
                };

                for s in &syms {
                    index
                        .symbols_by_name
                        .entry(s.name.clone())
                        .or_default()
                        .push(s.clone());
                    index.symbols_by_id.insert(s.id.clone(), s.clone());
                    index.symbol_graph.insert_symbol(s.clone());
                }

                for rel in parse_result.relationships {
                    index.symbol_graph.add_relationship(rel.clone());
                    index.relationships.push(rel);
                }

                // Add to module graph
                let imported_paths: std::collections::HashSet<PathBuf> = parse_result
                    .imports
                    .iter()
                    .map(|imp| PathBuf::from(&imp.from_module))
                    .collect();
                let rel_path = file.path.strip_prefix(root).unwrap_or(&file.path).to_path_buf();
                index.module_graph.insert_module(
                    file.path.clone(),
                    rel_path,
                    file.package_id.clone(),
                    imported_paths,
                );

                index.symbols_by_file.insert(file.path.clone(), syms);
                index.files_indexed += 1;
            }
        }

        index.module_graph.recompute_coupling_metrics();
        index
    }

    pub fn files_indexed(&self) -> usize {
        self.files_indexed
    }

    pub fn total_symbols(&self) -> usize {
        self.symbols_by_name.values().map(|v| v.len()).sum()
    }

    pub fn root_path(&self) -> &Path {
        &self.root_path
    }

    pub fn status(&self) -> IndexStatus {
        self.status
    }

    pub fn symbol_graph(&self) -> &SymbolGraph {
        &self.symbol_graph
    }

    pub fn package_graph(&self) -> &PackageGraph {
        &self.package_graph
    }

    pub fn module_graph(&self) -> &ModuleGraph {
        &self.module_graph
    }

    pub fn inventory(&self) -> Option<&WorkspaceInventory> {
        self.inventory.as_ref()
    }

    pub fn all_relationships(&self) -> &[SymbolRelationship] {
        &self.relationships
    }

    pub fn all_diagnostics(&self) -> &[ParserDiagnostic] {
        &self.diagnostics
    }

    pub fn file_hashes(&self) -> &HashMap<PathBuf, String> {
        &self.file_hashes
    }

    /// Returns a flat list of all indexed symbols.
    pub fn all_symbols(&self) -> Vec<&AstSymbol> {
        self.symbols_by_name.values().flatten().collect()
    }

    /// Returns the map of symbols organized by file path.
    pub fn symbols_by_file_map(&self) -> &HashMap<PathBuf, Vec<AstSymbol>> {
        &self.symbols_by_file
    }

    /// Inserts a symbol into the index manually.
    pub fn insert_symbol(&mut self, symbol: AstSymbol) {
        self.symbols_by_file
            .entry(symbol.file_path.clone())
            .or_default()
            .push(symbol.clone());
        self.symbols_by_name
            .entry(symbol.name.clone())
            .or_default()
            .push(symbol.clone());
        self.symbols_by_id.insert(symbol.id.clone(), symbol.clone());
        self.symbol_graph.insert_symbol(symbol);
    }

    /// Removes a file from the index and cleans up all associated symbol mappings and graph nodes.
    pub fn remove_file(&mut self, path: &Path) -> bool {
        self.file_hashes.remove(path);
        if let Some(removed_syms) = self.symbols_by_file.remove(path) {
            for sym in removed_syms {
                self.symbols_by_id.remove(&sym.id);
                if let Some(list) = self.symbols_by_name.get_mut(&sym.name) {
                    list.retain(|s| s.file_path != path);
                    if list.is_empty() {
                        self.symbols_by_name.remove(&sym.name);
                    }
                }
            }
            self.relationships.retain(|r| r.file_path != path);
            self.files_indexed = self.files_indexed.saturating_sub(1);
            true
        } else {
            false
        }
    }

    /// Incrementally updates or adds a file's symbols and relationships in the index with content hashing.
    pub fn update_file(&mut self, path: &Path, content: &str) {
        let hash = compute_hash(content);
        if let Some(existing_hash) = self.file_hashes.get(path) {
            if existing_hash == &hash {
                return; // Content unchanged, avoid unnecessary re-indexing
            }
        }

        self.remove_file(path);
        if is_supported(path) {
            self.file_hashes.insert(path.to_path_buf(), hash);
            let registry = LanguageAdapterRegistry::new();
            let parse_result = registry.parse_file(path, content);
            self.diagnostics.extend(parse_result.diagnostics);

            let syms = if !parse_result.symbols.is_empty() {
                parse_result.symbols
            } else {
                AstParser::parse_file(path, content)
            };

            for s in &syms {
                self.symbols_by_name
                    .entry(s.name.clone())
                    .or_default()
                    .push(s.clone());
                self.symbols_by_id.insert(s.id.clone(), s.clone());
                self.symbol_graph.insert_symbol(s.clone());
            }

            for rel in parse_result.relationships {
                self.symbol_graph.add_relationship(rel.clone());
                self.relationships.push(rel);
            }

            self.symbols_by_file.insert(path.to_path_buf(), syms);
            self.files_indexed += 1;
        }
    }

    /// Reconciles the workspace index against filesystem state (incremental update for modified/deleted/new files).
    pub fn reconcile_workspace(&mut self) -> usize {
        if self.root_path.as_os_str().is_empty() {
            return 0;
        }

        let options = TraversalOptions::default();
        let scan_report = WorkspaceDiscovery::scan_files(&self.root_path, &options);
        let mut changed_count = 0;

        let on_disk_paths: HashSet<PathBuf> = scan_report.files.iter().map(|f| f.path.clone()).collect();
        let indexed_paths: Vec<PathBuf> = self.symbols_by_file.keys().cloned().collect();

        // 1. Remove deleted files
        for p in indexed_paths {
            if !on_disk_paths.contains(&p) {
                self.remove_file(&p);
                changed_count += 1;
            }
        }

        // 2. Add or update modified files
        for file in scan_report.files {
            if let Ok(content) = fs::read_to_string(&file.path) {
                let hash = compute_hash(&content);
                let needs_update = self.file_hashes.get(&file.path) != Some(&hash);
                if needs_update {
                    self.update_file(&file.path, &content);
                    changed_count += 1;
                }
            }
        }

        changed_count
    }

    /// Returns all symbols defined in a file.
    pub fn symbols_in_file(&self, path: &Path) -> Vec<AstSymbol> {
        self.symbols_by_file.get(path).cloned().unwrap_or_default()
    }

    /// Finds all symbols matching the name.
    pub fn find_symbols_by_name(&self, name: &str) -> Vec<AstSymbol> {
        self.find_definition(name)
    }

    /// Finds all references / callers across files in the index.
    /// Prefers high-fidelity AST-derived callers from SymbolGraph, falling back to token-bounded scan.
    pub fn find_references_to(&self, symbol_name: &str) -> Vec<CallReference> {
        let ast_callers = self.symbol_graph.find_callers(symbol_name);
        if !ast_callers.is_empty() {
            return ast_callers;
        }

        let mut refs = Vec::new();
        for (path, syms) in &self.symbols_by_file {
            if let Ok(content) = fs::read_to_string(path) {
                for (idx, line) in content.lines().enumerate() {
                    if contains_ident(line, symbol_name) {
                        let line_num = idx + 1;
                        let caller_name = syms
                            .iter()
                            .find(|s| {
                                s.range.start_line <= line_num && line_num <= s.range.end_line
                            })
                            .map(|s| s.name.clone())
                            .unwrap_or_else(|| "<module>".to_string());

                        if caller_name != symbol_name {
                            refs.push(CallReference {
                                caller_name,
                                callee_name: symbol_name.to_string(),
                                file_path: path.clone(),
                                line: line_num,
                            });
                        }
                    }
                }
            }
        }
        refs
    }

    /// Finds symbol definitions matching the query name.
    pub fn find_definition(&self, symbol_name: &str) -> Vec<AstSymbol> {
        self.symbols_by_name
            .get(symbol_name)
            .cloned()
            .unwrap_or_default()
    }

    /// Returns the complete symbol outline of a specific file.
    pub fn file_outline(&self, path: &Path) -> Result<Vec<AstSymbol>> {
        if let Some(syms) = self.symbols_by_file.get(path) {
            return Ok(syms.clone());
        }
        let content = fs::read_to_string(path)?;
        let registry = LanguageAdapterRegistry::new();
        let res = registry.parse_file(path, &content);
        if !res.symbols.is_empty() {
            Ok(res.symbols)
        } else {
            Ok(AstParser::parse_file(path, &content))
        }
    }

    /// Extracts the exact body of a symbol by name from a file.
    pub fn extract_symbol_body(&self, path: &Path, symbol_name: &str) -> Result<Option<String>> {
        let content = fs::read_to_string(path)?;
        let lines: Vec<&str> = content.lines().collect();
        let syms = self.file_outline(path)?;

        if let Some(sym) = syms.iter().find(|s| s.name == symbol_name) {
            let start = sym.range.start_line.saturating_sub(1);
            let end = sym.range.end_line.min(lines.len());
            if start < end {
                return Ok(Some(lines[start..end].join("\n")));
            }
        }

        Ok(None)
    }

    /// Chunks a specific file along AST boundaries.
    pub fn chunk_file(&self, path: &Path) -> Result<Vec<SemanticChunk>> {
        let content = fs::read_to_string(path)?;
        let syms = self.file_outline(path)?;
        Ok(AstChunker::chunk_file(path, &content, &syms))
    }

    /// Finds all symbols called directly from inside the body of `caller_name`.
    pub fn find_callees(&self, caller_name: &str) -> Vec<CallReference> {
        let graph_callees = self.symbol_graph.find_callees(caller_name);
        if !graph_callees.is_empty() {
            return graph_callees;
        }

        let mut callees = Vec::new();
        let caller_defs = self.find_definition(caller_name);

        for caller_sym in caller_defs {
            if let Ok(Some(body)) = self.extract_symbol_body(&caller_sym.file_path, caller_name) {
                for (target_name, target_syms) in &self.symbols_by_name {
                    if target_name == caller_name {
                        continue;
                    }
                    if contains_ident(&body, target_name) {
                        for (idx, line) in body.lines().enumerate() {
                            if contains_ident(line, target_name) {
                                callees.push(CallReference {
                                    caller_name: caller_name.to_string(),
                                    callee_name: target_name.clone(),
                                    file_path: target_syms
                                        .first()
                                        .map(|s| s.file_path.clone())
                                        .unwrap_or_else(|| caller_sym.file_path.clone()),
                                    line: caller_sym.range.start_line + idx,
                                });
                                break;
                            }
                        }
                    }
                }
            }
        }
        callees
    }

    /// Recursively builds a call graph tree starting at `entry_symbol` up to `max_depth`.
    pub fn build_call_graph(&self, entry_symbol: &str, max_depth: usize) -> CallGraphNode {
        let mut visited = HashSet::new();
        self.build_call_graph_internal(entry_symbol, 0, max_depth, &mut visited)
    }

    fn build_call_graph_internal(
        &self,
        symbol: &str,
        current_depth: usize,
        max_depth: usize,
        visited: &mut HashSet<String>,
    ) -> CallGraphNode {
        let def = self.find_definition(symbol).into_iter().next();
        let (file_path, line) = def
            .map(|s| (s.file_path, s.range.start_line))
            .unwrap_or_else(|| (PathBuf::new(), 0));

        if current_depth >= max_depth || visited.contains(symbol) {
            return CallGraphNode {
                symbol_name: symbol.to_string(),
                file_path,
                line,
                callees: Vec::new(),
            };
        }

        visited.insert(symbol.to_string());
        let direct_callees = self.find_callees(symbol);
        let mut callee_nodes = Vec::new();

        for c in direct_callees {
            callee_nodes.push(self.build_call_graph_internal(
                &c.callee_name,
                current_depth + 1,
                max_depth,
                visited,
            ));
        }

        visited.remove(symbol);

        CallGraphNode {
            symbol_name: symbol.to_string(),
            file_path,
            line,
            callees: callee_nodes,
        }
    }

    /// Finds all symbols implemented under a specific parent scope (e.g. struct or trait name).
    pub fn find_interface_implementations(&self, target_scope: &str) -> Vec<AstSymbol> {
        let from_graph = self.symbol_graph.find_implementations(target_scope);
        if !from_graph.is_empty() {
            return from_graph;
        }

        let mut results = Vec::new();
        for syms in self.symbols_by_file.values() {
            for s in syms {
                if let Some(ref parent) = s.parent_scope
                    && parent == target_scope
                {
                    results.push(s.clone());
                }
            }
        }
        results
    }

    /// Retrieves task-aware repository context for engineering agents within a token budget.
    pub fn retrieve_context(&self, query: &RetrievalQuery) -> ContextPacket {
        let all_syms: Vec<AstSymbol> = self.symbols_by_id.values().cloned().collect();
        let inv = self.inventory.clone().unwrap_or_else(|| {
            WorkspaceInventory {
                root_path: self.root_path.clone(),
                repo_name: "workspace".to_string(),
                git_branch: None,
                git_head_sha: None,
                detected_languages: vec![],
                manifests: vec![],
                packages: vec![],
                is_monorepo: false,
            }
        });

        SemanticRetriever::retrieve_context(
            query,
            &all_syms,
            &self.relationships,
            &self.symbol_graph,
            &inv,
            &self.test_files,
        )
    }

    /// Finds all tests related to a symbol or file.
    pub fn find_related_tests(&self, target_symbol: &str, target_file: &Path) -> Vec<RelatedTestTarget> {
        let all_syms: Vec<AstSymbol> = self.symbols_by_id.values().cloned().collect();
        TestDiscoveryEngine::find_related_tests(
            target_symbol,
            target_file,
            &all_syms,
            &self.relationships,
            &self.test_files,
        )
    }

    /// Finds potential callers/references to a symbol across the workspace.
    pub fn find_callers(root: &Path, symbol_name: &str, max_results: usize) -> Vec<CallReference> {
        let index = Self::build(root, 1000);
        let refs = index.find_references_to(symbol_name);
        refs.into_iter().take(max_results).collect()
    }
}

fn compute_hash(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn is_supported(path: &Path) -> bool {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    matches!(
        ext,
        "rs" | "py" | "ts" | "js" | "tsx" | "jsx" | "go" | "c" | "cpp" | "h" | "hpp"
    )
}

/// Checks if an identifier appears in a line with exact word/identifier boundaries.
pub fn contains_ident(line: &str, ident: &str) -> bool {
    if ident.is_empty() {
        return false;
    }
    let mut start = 0;
    while let Some(pos) = line[start..].find(ident) {
        let idx = start + pos;
        let before_ok = if idx == 0 {
            true
        } else {
            let prev_char = line[..idx].chars().next_back().unwrap();
            !is_ident_char(prev_char)
        };
        let after_idx = idx + ident.len();
        let after_ok = if after_idx >= line.len() {
            true
        } else {
            let next_char = line[after_idx..].chars().next().unwrap();
            !is_ident_char(next_char)
        };

        if before_ok && after_ok {
            return true;
        }
        start = idx + ident.len();
    }
    false
}

fn is_ident_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_contains_ident_exact_token_boundaries() {
        assert!(contains_ident(
            "let hash = compute_hash(val);",
            "compute_hash"
        ));
        assert!(contains_ident("compute_hash(val)", "compute_hash"));
        assert!(contains_ident("fn compute_hash()", "compute_hash"));
        assert!(contains_ident("val.compute_hash();", "compute_hash"));

        // Negative cases - substring of another ident
        assert!(!contains_ident(
            "let hash = compute_hash_v2(val);",
            "compute_hash"
        ));
        assert!(!contains_ident(
            "let hash = pre_compute_hash(val);",
            "compute_hash"
        ));
        assert!(!contains_ident("is_compute_hash_valid", "compute_hash"));
        assert!(!contains_ident("", "compute_hash"));
    }

    #[test]
    fn test_incremental_index_update_and_removal() {
        let tmp = tempdir().unwrap();
        let file_path = tmp.path().join("service.rs");

        let mut index = AstCodebaseIndex::new();

        let initial_code = r#"
pub struct ServiceA {
    pub id: u64,
}

pub fn start_service() {}
"#;
        index.update_file(&file_path, initial_code);
        assert_eq!(index.files_indexed(), 1);
        assert_eq!(index.find_definition("ServiceA").len(), 1);
        assert_eq!(index.find_definition("start_service").len(), 1);
        assert_eq!(index.total_symbols(), 2);

        // Update file with new symbols and removed old symbols
        let updated_code = r#"
pub struct ServiceB {
    pub id: u64,
}
"#;
        index.update_file(&file_path, updated_code);
        assert_eq!(index.files_indexed(), 1);
        assert_eq!(
            index.find_definition("ServiceA").len(),
            0,
            "Old symbol should be removed"
        );
        assert_eq!(
            index.find_definition("start_service").len(),
            0,
            "Old symbol should be removed"
        );
        assert_eq!(
            index.find_definition("ServiceB").len(),
            1,
            "New symbol should be indexed"
        );
        assert_eq!(index.total_symbols(), 1);

        // Remove file
        let removed = index.remove_file(&file_path);
        assert!(removed);
        assert_eq!(index.files_indexed(), 0);
        assert_eq!(index.find_definition("ServiceB").len(), 0);
        assert_eq!(index.total_symbols(), 0);
    }

    #[test]
    fn test_call_graph_and_callee_navigation() {
        let tmp = tempdir().unwrap();
        let file_path = tmp.path().join("graph.rs");

        let code = r#"
pub fn helper_b() {
    println!("hello");
}

pub fn helper_a() {
    helper_b();
}

pub fn main_entry() {
    helper_a();
}
"#;
        fs::write(&file_path, code).unwrap();
        let mut index = AstCodebaseIndex::new();
        index.update_file(&file_path, code);

        let callees_of_main = index.find_callees("main_entry");
        assert_eq!(callees_of_main.len(), 1);
        assert_eq!(callees_of_main[0].callee_name, "helper_a");

        let graph = index.build_call_graph("main_entry", 3);
        assert_eq!(graph.symbol_name, "main_entry");
        assert_eq!(graph.callees.len(), 1);
        assert_eq!(graph.callees[0].symbol_name, "helper_a");
        assert_eq!(graph.callees[0].callees.len(), 1);
        assert_eq!(graph.callees[0].callees[0].symbol_name, "helper_b");
    }
}
