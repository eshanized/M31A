//! Multi-Tier Repository Dependency Graph Engine (`m31a-ast::graph`).
//!
//! Provides structured, cycle-safe graph representations across three distinct tiers:
//! 1. Package Graph: Package/crate dependencies, internal vs external, version bounds, and transitive depth.
//! 2. Module Graph: File/module imports, re-exports, circular dependencies, Martin coupling (Ca/Ce/Instability).
//! 3. Symbol Graph: Function/method calls, type usage, interface implementations, transitive caller/callee traversal.

use crate::types::{
    AstSymbol, AstSymbolKind, CallReference, RelationshipKind, SymbolRelationship,
};
use crate::workspace::{PackageDependency, WorkspaceInventory};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};

// ===========================================================================
// Tier 1: Package Graph
// ===========================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageNode {
    pub package_id: String,
    pub name: String,
    pub version: Option<String>,
    pub manifest_path: PathBuf,
    pub is_root: bool,
    pub dependencies: Vec<PackageDependency>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PackageGraph {
    pub packages: HashMap<String, PackageNode>,
    /// Adjacency: package_name -> set of dependent package names
    pub edges: HashMap<String, HashSet<String>>,
}

impl PackageGraph {
    pub fn build(inventory: &WorkspaceInventory) -> Self {
        let mut graph = Self::default();

        for pkg in &inventory.packages {
            let node = PackageNode {
                package_id: pkg.id.clone(),
                name: pkg.name.clone(),
                version: pkg.version.clone(),
                manifest_path: pkg.manifest_path.clone(),
                is_root: pkg.is_root,
                dependencies: pkg.dependencies.clone(),
            };
            graph.packages.insert(pkg.name.clone(), node);

            let mut deps_set = HashSet::new();
            for dep in &pkg.dependencies {
                deps_set.insert(dep.name.clone());
            }
            graph.edges.insert(pkg.name.clone(), deps_set);
        }

        graph
    }

    /// Finds all direct internal dependencies of a package.
    pub fn direct_internal_dependencies(&self, package_name: &str) -> Vec<String> {
        if let Some(deps) = self.edges.get(package_name) {
            deps.iter()
                .filter(|d| self.packages.contains_key(*d))
                .cloned()
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Finds all transitive internal dependencies of a package with cycle protection.
    pub fn transitive_dependencies(&self, package_name: &str, max_depth: usize) -> Vec<String> {
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();
        queue.push_back((package_name.to_string(), 0));
        visited.insert(package_name.to_string());

        let mut result = Vec::new();

        while let Some((curr, depth)) = queue.pop_front() {
            if depth >= max_depth {
                continue;
            }

            if let Some(deps) = self.edges.get(&curr) {
                for dep in deps {
                    if !visited.contains(dep) {
                        visited.insert(dep.clone());
                        if self.packages.contains_key(dep) {
                            result.push(dep.clone());
                        }
                        queue.push_back((dep.clone(), depth + 1));
                    }
                }
            }
        }

        result
    }

    /// Finds packages that depend directly on `package_name`.
    pub fn direct_dependents(&self, package_name: &str) -> Vec<String> {
        let mut callers = Vec::new();
        for (pkg, deps) in &self.edges {
            if deps.contains(package_name) {
                callers.push(pkg.clone());
            }
        }
        callers
    }

    /// Detects circular package dependencies using DFS.
    pub fn detect_cycles(&self) -> Vec<Vec<String>> {
        let mut cycles = Vec::new();
        let mut visited = HashSet::new();
        let mut on_stack = HashSet::new();
        let mut path = Vec::new();

        for node in self.packages.keys() {
            if !visited.contains(node) {
                self.dfs_cycles(
                    node,
                    &mut visited,
                    &mut on_stack,
                    &mut path,
                    &mut cycles,
                );
            }
        }

        cycles
    }

    fn dfs_cycles(
        &self,
        node: &str,
        visited: &mut HashSet<String>,
        on_stack: &mut HashSet<String>,
        path: &mut Vec<String>,
        cycles: &mut Vec<Vec<String>>,
    ) {
        visited.insert(node.to_string());
        on_stack.insert(node.to_string());
        path.push(node.to_string());

        if let Some(neighbors) = self.edges.get(node) {
            for neighbor in neighbors {
                if !self.packages.contains_key(neighbor) {
                    continue; // Skip external third-party crates
                }

                if on_stack.contains(neighbor) {
                    if let Some(idx) = path.iter().position(|p| p == neighbor) {
                        let mut cycle = path[idx..].to_vec();
                        cycle.push(neighbor.to_string());
                        cycles.push(cycle);
                    }
                } else if !visited.contains(neighbor) {
                    self.dfs_cycles(neighbor, visited, on_stack, path, cycles);
                }
            }
        }

        path.pop();
        on_stack.remove(node);
    }
}

// ===========================================================================
// Tier 2: Module Graph & Coupling Metrics
// ===========================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleNode {
    pub file_path: PathBuf,
    pub relative_path: PathBuf,
    pub package_name: Option<String>,
    pub imported_modules: HashSet<String>,
    pub afferent_coupling_ca: usize,
    pub efferent_coupling_ce: usize,
    pub instability_i: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModuleGraph {
    pub modules: HashMap<PathBuf, ModuleNode>,
    /// Adjacency: module_path -> set of imported module paths
    pub edges: HashMap<PathBuf, HashSet<PathBuf>>,
}

impl ModuleGraph {
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a module and its discovered imports to the graph.
    pub fn insert_module(
        &mut self,
        file_path: PathBuf,
        relative_path: PathBuf,
        package_name: Option<String>,
        imported_files: HashSet<PathBuf>,
    ) {
        let efferent = imported_files.len();
        self.edges.insert(file_path.clone(), imported_files.clone());

        let imported_module_names: HashSet<String> = imported_files
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect();

        self.modules.insert(
            file_path.clone(),
            ModuleNode {
                file_path,
                relative_path,
                package_name,
                imported_modules: imported_module_names,
                afferent_coupling_ca: 0,
                efferent_coupling_ce: efferent,
                instability_i: 0.0,
            },
        );

        self.recompute_coupling_metrics();
    }

    /// Recomputes Martin's Ca, Ce, and Instability (I = Ce / (Ca + Ce)).
    pub fn recompute_coupling_metrics(&mut self) {
        let mut afferent_counts: HashMap<PathBuf, usize> = HashMap::new();

        for deps in self.edges.values() {
            for dep in deps {
                *afferent_counts.entry(dep.clone()).or_insert(0) += 1;
            }
        }

        for (path, node) in self.modules.iter_mut() {
            let ca = *afferent_counts.get(path).unwrap_or(&0);
            let ce = node.efferent_coupling_ce;
            node.afferent_coupling_ca = ca;
            node.instability_i = if ca + ce > 0 {
                ce as f64 / (ca + ce) as f64
            } else {
                0.0
            };
        }
    }

    /// Detects circular module imports.
    pub fn detect_cycles(&self) -> Vec<Vec<PathBuf>> {
        let mut cycles = Vec::new();
        let mut visited = HashSet::new();
        let mut on_stack = HashSet::new();
        let mut path = Vec::new();

        for node in self.edges.keys() {
            if !visited.contains(node) {
                self.dfs_module_cycles(
                    node,
                    &mut visited,
                    &mut on_stack,
                    &mut path,
                    &mut cycles,
                );
            }
        }

        cycles
    }

    fn dfs_module_cycles(
        &self,
        node: &Path,
        visited: &mut HashSet<PathBuf>,
        on_stack: &mut HashSet<PathBuf>,
        path: &mut Vec<PathBuf>,
        cycles: &mut Vec<Vec<PathBuf>>,
    ) {
        visited.insert(node.to_path_buf());
        on_stack.insert(node.to_path_buf());
        path.push(node.to_path_buf());

        if let Some(neighbors) = self.edges.get(node) {
            for neighbor in neighbors {
                if on_stack.contains(neighbor) {
                    if let Some(idx) = path.iter().position(|p| p == neighbor) {
                        let mut cycle = path[idx..].to_vec();
                        cycle.push(neighbor.to_path_buf());
                        cycles.push(cycle);
                    }
                } else if !visited.contains(neighbor) {
                    self.dfs_module_cycles(neighbor, visited, on_stack, path, cycles);
                }
            }
        }

        path.pop();
        on_stack.remove(node);
    }
}

// ===========================================================================
// Tier 3: Symbol Graph & Call Traversal
// ===========================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolGraphNode {
    pub symbol: AstSymbol,
    pub outgoing_edges: Vec<SymbolRelationship>,
    pub incoming_edges: Vec<SymbolRelationship>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SymbolGraph {
    /// symbol_id -> SymbolGraphNode
    pub nodes: HashMap<String, SymbolGraphNode>,
    /// symbol_name -> list of symbol_ids sharing that simple name
    pub name_index: HashMap<String, Vec<String>>,
}

impl SymbolGraph {
    pub fn new() -> Self {
        Self::default()
    }

    /// Inserts an AST symbol definition into the graph.
    pub fn insert_symbol(&mut self, symbol: AstSymbol) {
        self.name_index
            .entry(symbol.name.clone())
            .or_default()
            .push(symbol.id.clone());

        self.nodes.insert(
            symbol.id.clone(),
            SymbolGraphNode {
                symbol,
                outgoing_edges: Vec::new(),
                incoming_edges: Vec::new(),
            },
        );
    }

    /// Adds a relationship edge between symbols or targets.
    pub fn add_relationship(&mut self, rel: SymbolRelationship) {
        // Add to source node if present
        if let Some(src_node) = self.nodes.get_mut(&rel.source_id) {
            src_node.outgoing_edges.push(rel.clone());
        }

        // Add incoming edge to target nodes matching target_symbol
        if let Some(target_ids) = self.name_index.get(&rel.target_symbol).cloned() {
            for tgt_id in target_ids {
                if let Some(tgt_node) = self.nodes.get_mut(&tgt_id) {
                    tgt_node.incoming_edges.push(rel.clone());
                }
            }
        }
    }

    /// Finds all direct callers of `target_symbol` across files.
    pub fn find_callers(&self, target_symbol: &str) -> Vec<CallReference> {
        let mut callers = Vec::new();
        if let Some(ids) = self.name_index.get(target_symbol) {
            for id in ids {
                if let Some(node) = self.nodes.get(id) {
                    for inc in &node.incoming_edges {
                        if inc.kind == RelationshipKind::Calls {
                            callers.push(CallReference {
                                caller_name: inc.source_symbol.clone(),
                                callee_name: target_symbol.to_string(),
                                file_path: inc.file_path.clone(),
                                line: inc.line,
                            });
                        }
                    }
                }
            }
        }
        callers
    }

    /// Finds all callees directly called by `caller_symbol`.
    pub fn find_callees(&self, caller_symbol: &str) -> Vec<CallReference> {
        let mut callees = Vec::new();
        if let Some(ids) = self.name_index.get(caller_symbol) {
            for id in ids {
                if let Some(node) = self.nodes.get(id) {
                    for out in &node.outgoing_edges {
                        if out.kind == RelationshipKind::Calls {
                            callers_push_unique(
                                &mut callees,
                                CallReference {
                                    caller_name: caller_symbol.to_string(),
                                    callee_name: out.target_symbol.clone(),
                                    file_path: out.file_path.clone(),
                                    line: out.line,
                                },
                            );
                        }
                    }
                }
            }
        }
        callees
    }

    /// Recursively traces transitive callers with bounded depth and cycle immunity.
    pub fn trace_transitive_callers(
        &self,
        symbol_name: &str,
        max_depth: usize,
    ) -> Vec<CallReference> {
        let mut result = Vec::new();
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();

        queue.push_back((symbol_name.to_string(), 0));
        visited.insert(symbol_name.to_string());

        while let Some((curr_symbol, depth)) = queue.pop_front() {
            if depth >= max_depth {
                continue;
            }

            let direct = self.find_callers(&curr_symbol);
            for d in direct {
                if !visited.contains(&d.caller_name) {
                    visited.insert(d.caller_name.clone());
                    queue.push_back((d.caller_name.clone(), depth + 1));
                    result.push(d);
                }
            }
        }

        result
    }

    /// Finds all symbols implementing an interface or trait.
    pub fn find_implementations(&self, interface_or_trait: &str) -> Vec<AstSymbol> {
        let mut implementations = Vec::new();

        for node in self.nodes.values() {
            for out in &node.outgoing_edges {
                if out.kind == RelationshipKind::Implements
                    && out.target_symbol == interface_or_trait
                {
                    implementations.push(node.symbol.clone());
                    break;
                }
            }
        }

        implementations
    }

    /// Detects candidate dead or orphaned symbols (non-public functions with 0 callers).
    pub fn find_orphaned_symbols(&self) -> Vec<AstSymbol> {
        let mut orphans = Vec::new();

        for node in self.nodes.values() {
            if !node.symbol.is_public
                && matches!(
                    node.symbol.kind,
                    AstSymbolKind::Function | AstSymbolKind::Method
                )
                && node.incoming_edges.is_empty()
                && !node.symbol.name.starts_with("test_")
                && !node.symbol.name.starts_with("Test")
            {
                orphans.push(node.symbol.clone());
            }
        }

        orphans
    }
}

fn callers_push_unique(vec: &mut Vec<CallReference>, item: CallReference) {
    if !vec.iter().any(|c| {
        c.caller_name == item.caller_name
            && c.callee_name == item.callee_name
            && c.line == item.line
    }) {
        vec.push(item);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{ResolutionStatus, SourceRange};
    use crate::workspace::WorkspacePackage;

    #[test]
    fn test_package_graph_and_cycles() {
        let inventory = WorkspaceInventory {
            root_path: PathBuf::from("/repo"),
            repo_name: "test_repo".to_string(),
            git_branch: Some("master".to_string()),
            git_head_sha: None,
            detected_languages: vec![],
            manifests: vec![],
            packages: vec![
                WorkspacePackage {
                    id: "cargo:core".to_string(),
                    name: "core".to_string(),
                    version: Some("0.1.0".to_string()),
                    language: crate::types::LanguageId::Rust,
                    manifest_path: PathBuf::from("/repo/crates/core/Cargo.toml"),
                    package_root: PathBuf::from("/repo/crates/core"),
                    source_dirs: vec![],
                    test_dirs: vec![],
                    dependencies: vec![],
                    is_root: false,
                },
                WorkspacePackage {
                    id: "cargo:auth".to_string(),
                    name: "auth".to_string(),
                    version: Some("0.1.0".to_string()),
                    language: crate::types::LanguageId::Rust,
                    manifest_path: PathBuf::from("/repo/crates/auth/Cargo.toml"),
                    package_root: PathBuf::from("/repo/crates/auth"),
                    source_dirs: vec![],
                    test_dirs: vec![],
                    dependencies: vec![PackageDependency {
                        name: "core".to_string(),
                        version_req: None,
                        is_dev: false,
                        is_internal: true,
                        path: None,
                    }],
                    is_root: false,
                },
            ],
            is_monorepo: true,
        };

        let graph = PackageGraph::build(&inventory);
        let auth_deps = graph.direct_internal_dependencies("auth");
        assert_eq!(auth_deps, vec!["core"]);

        let core_dependents = graph.direct_dependents("core");
        assert_eq!(core_dependents, vec!["auth"]);

        let cycles = graph.detect_cycles();
        assert!(cycles.is_empty());
    }

    #[test]
    fn test_symbol_graph_callers_and_transitive_traversal() {
        let mut graph = SymbolGraph::new();

        let sym_a = AstSymbol::new(
            "entry_point",
            AstSymbolKind::Function,
            PathBuf::from("src/main.rs"),
            SourceRange::new(1, 10, 0, 0),
            "fn entry_point()",
        );
        let sym_b = AstSymbol::new(
            "service_call",
            AstSymbolKind::Function,
            PathBuf::from("src/service.rs"),
            SourceRange::new(1, 10, 0, 0),
            "fn service_call()",
        );
        let sym_c = AstSymbol::new(
            "db_query",
            AstSymbolKind::Function,
            PathBuf::from("src/db.rs"),
            SourceRange::new(1, 10, 0, 0),
            "fn db_query()",
        );

        let id_a = sym_a.id.clone();
        let id_b = sym_b.id.clone();

        graph.insert_symbol(sym_a);
        graph.insert_symbol(sym_b);
        graph.insert_symbol(sym_c);

        // A -> B
        graph.add_relationship(SymbolRelationship::new(
            id_a,
            "entry_point",
            "service_call",
            RelationshipKind::Calls,
            PathBuf::from("src/main.rs"),
            5,
            ResolutionStatus::AstDerived,
        ));

        // B -> C
        graph.add_relationship(SymbolRelationship::new(
            id_b,
            "service_call",
            "db_query",
            RelationshipKind::Calls,
            PathBuf::from("src/service.rs"),
            8,
            ResolutionStatus::AstDerived,
        ));

        // Direct callers of db_query: should be service_call
        let direct = graph.find_callers("db_query");
        assert_eq!(direct.len(), 1);
        assert_eq!(direct[0].caller_name, "service_call");

        // Transitive callers of db_query: should include entry_point
        let transitive = graph.trace_transitive_callers("db_query", 5);
        assert_eq!(transitive.len(), 2);
        assert!(transitive.iter().any(|c| c.caller_name == "service_call"));
        assert!(transitive.iter().any(|c| c.caller_name == "entry_point"));
    }
}
