use crate::repo::types::{EntryPoint, FileClassification, RepositorySymbol, SubsystemKind};
use petgraph::Directed;
use petgraph::stable_graph::{NodeIndex, StableGraph};
use petgraph::visit::EdgeRef;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::Arc;

/// Nodes in the repository code graph.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RepositoryNode {
    File { path: String, language: String },
    Symbol(RepositorySymbol),
    Module { name: String },
}

impl RepositoryNode {
    pub fn as_symbol(&self) -> Option<&RepositorySymbol> {
        match self {
            Self::Symbol(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_file_path(&self) -> Option<&str> {
        match self {
            Self::File { path, .. } => Some(path),
            Self::Symbol(s) => Some(&s.file_path),
            _ => None,
        }
    }
}

/// Relationships between repository nodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RepositoryEdgeKind {
    Contains,
    Calls,
    Imports,
    Implements,
    DependsOn,
    Defines,
    /// Links test symbols or files to the production symbols/files they verify
    Tests,
    /// Syntactic or semantic reference from one symbol to another
    References,
}

impl RepositoryEdgeKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Contains => "contains",
            Self::Calls => "calls",
            Self::Imports => "imports",
            Self::Implements => "implements",
            Self::DependsOn => "depends_on",
            Self::Defines => "defines",
            Self::Tests => "tests",
            Self::References => "references",
        }
    }

    pub fn from_str_name(s: &str) -> Option<Self> {
        match s.trim().to_lowercase().as_str() {
            "contains" => Some(Self::Contains),
            "calls" => Some(Self::Calls),
            "imports" => Some(Self::Imports),
            "implements" => Some(Self::Implements),
            "depends_on" | "dependson" => Some(Self::DependsOn),
            "defines" => Some(Self::Defines),
            "tests" | "test" => Some(Self::Tests),
            "references" | "refs" => Some(Self::References),
            _ => None,
        }
    }
}

impl fmt::Display for RepositoryEdgeKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// In-memory code graph backed by `petgraph::stable_graph::StableGraph`.
/// Supports generation-isolated snapshots for concurrent context compilation.
#[derive(Debug, Clone)]
pub struct RepositoryGraph {
    pub generation_id: u64,
    pub graph: StableGraph<RepositoryNode, RepositoryEdgeKind, Directed>,
    pub file_nodes: BTreeMap<String, NodeIndex>,
    pub file_to_nodes: BTreeMap<String, Vec<NodeIndex>>,
    pub symbol_to_node: BTreeMap<String, NodeIndex>,
    pub file_classifications: BTreeMap<String, FileClassification>,
    pub entry_points: Vec<EntryPoint>,
    pub file_subsystems: BTreeMap<String, SubsystemKind>,
    pub file_hashes: BTreeMap<String, String>,
}

impl RepositoryGraph {
    pub fn new(generation_id: u64) -> Self {
        Self {
            generation_id,
            graph: StableGraph::new(),
            file_nodes: BTreeMap::new(),
            file_to_nodes: BTreeMap::new(),
            symbol_to_node: BTreeMap::new(),
            file_classifications: BTreeMap::new(),
            entry_points: Vec::new(),
            file_subsystems: BTreeMap::new(),
            file_hashes: BTreeMap::new(),
        }
    }

    /// Associate a cryptographic content hash with a file path.
    pub fn set_file_hash(&mut self, path: impl Into<String>, hash: impl Into<String>) {
        self.file_hashes.insert(path.into(), hash.into());
    }

    /// Retrieve the indexed cryptographic hash for a file.
    pub fn get_file_hash(&self, path: &str) -> Option<&str> {
        self.file_hashes.get(path).map(|s| s.as_str())
    }

    /// Build a complete in-memory `RepositoryGraph` from a `ScanSummary`.
    pub fn from_scan_summary(
        summary: &crate::repo::scanner::ScanSummary,
        generation_id: u64,
    ) -> Self {
        let mut graph = Self::new(generation_id);

        for file in &summary.files {
            let f_idx = graph.add_file(&file.relative_path, file.language.as_str());
            graph.set_file_classification(&file.relative_path, file.classification);
            graph.set_file_hash(&file.relative_path, &file.content_hash);
            if let Some(sub) = file.subsystem {
                graph.set_file_subsystem(&file.relative_path, sub);
            }

            for sym in &file.symbols {
                graph.add_symbol(sym.clone(), Some(f_idx));
            }
        }

        for (source_id, target_id, edge_kind) in &summary.edges {
            let _ = graph.add_relationship(source_id, target_id, *edge_kind);
        }

        for ep in &summary.entry_points {
            graph.add_entry_point(ep.clone());
        }

        graph
    }

    /// Add a file node to the graph.
    pub fn add_file(&mut self, path: &str, language: &str) -> NodeIndex {
        if let Some(&existing_idx) = self.file_nodes.get(path) {
            return existing_idx;
        }

        let idx = self.graph.add_node(RepositoryNode::File {
            path: path.to_string(),
            language: language.to_string(),
        });

        self.file_nodes.insert(path.to_string(), idx);
        self.file_to_nodes
            .entry(path.to_string())
            .or_default()
            .push(idx);
        idx
    }

    /// Add a symbol node attached to a parent file node.
    pub fn add_symbol(
        &mut self,
        symbol: RepositorySymbol,
        parent_file_idx: Option<NodeIndex>,
    ) -> NodeIndex {
        let symbol_id = symbol.id.clone();
        let file_path = symbol.file_path.clone();

        let sym_idx = self.graph.add_node(RepositoryNode::Symbol(symbol));
        self.symbol_to_node.insert(symbol_id, sym_idx);
        self.file_to_nodes
            .entry(file_path)
            .or_default()
            .push(sym_idx);

        if let Some(file_idx) = parent_file_idx {
            self.graph
                .add_edge(file_idx, sym_idx, RepositoryEdgeKind::Contains);
        }

        sym_idx
    }

    /// Add a relationship between two symbols by ID.
    pub fn add_relationship(
        &mut self,
        source_id: &str,
        target_id: &str,
        kind: RepositoryEdgeKind,
    ) -> bool {
        let source_node = self.symbol_to_node.get(source_id).copied();
        let target_node = self.symbol_to_node.get(target_id).copied();

        if let (Some(src), Some(tgt)) = (source_node, target_node) {
            self.graph.add_edge(src, tgt, kind);
            true
        } else {
            false
        }
    }

    /// Invalidate all nodes and relationships associated with a file.
    /// Preserves existing NodeIndex stability for all other files via `StableGraph`.
    pub fn invalidate_file(&mut self, path: &str) {
        if let Some(nodes) = self.file_to_nodes.remove(path) {
            for node_idx in nodes {
                if let Some(RepositoryNode::Symbol(sym)) = self.graph.node_weight(node_idx) {
                    self.symbol_to_node.remove(&sym.id);
                }
                self.graph.remove_node(node_idx);
            }
        }
        self.file_nodes.remove(path);
    }

    /// Get symbol by ID.
    pub fn get_symbol(&self, symbol_id: &str) -> Option<&RepositorySymbol> {
        let node_idx = self.symbol_to_node.get(symbol_id)?;
        self.graph.node_weight(*node_idx)?.as_symbol()
    }

    /// Get all symbols defined in a file.
    pub fn get_file_symbols(&self, path: &str) -> Vec<&RepositorySymbol> {
        let mut symbols = Vec::new();
        if let Some(indices) = self.file_to_nodes.get(path) {
            for &idx in indices {
                if let Some(RepositoryNode::Symbol(sym)) = self.graph.node_weight(idx) {
                    symbols.push(sym);
                }
            }
        }
        symbols
    }

    /// Total node count.
    pub fn node_count(&self) -> usize {
        self.graph.node_count()
    }

    /// Total edge count.
    pub fn edge_count(&self) -> usize {
        self.graph.edge_count()
    }

    /// Produces a read-only snapshot for concurrent context compilation.
    pub fn snapshot(&self) -> Arc<Self> {
        Arc::new(self.clone())
    }

    /// Advances to next generation.
    pub fn next_generation(&self) -> Self {
        let mut next = self.clone();
        next.generation_id += 1;
        next
    }

    /// Set file classification.
    pub fn set_file_classification(&mut self, path: &str, classification: FileClassification) {
        self.file_classifications
            .insert(path.to_string(), classification);
    }

    /// Get file classification (defaults to Source if unknown).
    pub fn get_file_classification(&self, path: &str) -> FileClassification {
        self.file_classifications
            .get(path)
            .copied()
            .unwrap_or(FileClassification::Source)
    }

    /// Add an entry point to the repository.
    pub fn add_entry_point(&mut self, entry_point: EntryPoint) {
        if !self.entry_points.iter().any(|e| e.id == entry_point.id) {
            self.entry_points.push(entry_point);
        }
    }

    /// List all discovered entry points.
    pub fn entry_points(&self) -> &[EntryPoint] {
        &self.entry_points
    }

    /// Set subsystem classification for a file.
    pub fn set_file_subsystem(&mut self, path: &str, subsystem: SubsystemKind) {
        self.file_subsystems.insert(path.to_string(), subsystem);
    }

    /// Get subsystem for a file (falls back to detection from path).
    pub fn get_file_subsystem(&self, path: &str) -> SubsystemKind {
        self.file_subsystems
            .get(path)
            .copied()
            .unwrap_or_else(|| SubsystemKind::detect_from_path(path))
    }

    /// List all known files in the graph.
    pub fn get_all_files(&self) -> Vec<String> {
        self.file_nodes.keys().cloned().collect()
    }

    /// Get all test symbols in the repository.
    pub fn get_test_symbols(&self) -> Vec<&RepositorySymbol> {
        let mut tests = Vec::new();
        for node_idx in self.graph.node_indices() {
            if let Some(RepositoryNode::Symbol(sym)) = self.graph.node_weight(node_idx)
                && sym.is_test
            {
                tests.push(sym);
            }
        }
        tests
    }

    /// Find all test symbols that test or reference the given symbol.
    pub fn find_tests_for_symbol(&self, symbol_id: &str) -> Vec<&RepositorySymbol> {
        let Some(&sym_idx) = self.symbol_to_node.get(symbol_id) else {
            return Vec::new();
        };

        let mut tests = Vec::new();
        // 1. Check incoming Tests edges
        for edge in self
            .graph
            .edges_directed(sym_idx, petgraph::Direction::Incoming)
        {
            if *edge.weight() == RepositoryEdgeKind::Tests
                && let Some(RepositoryNode::Symbol(tester)) = self.graph.node_weight(edge.source())
            {
                tests.push(tester);
            }
        }

        // 2. Check incoming Calls edges from test functions
        for edge in self
            .graph
            .edges_directed(sym_idx, petgraph::Direction::Incoming)
        {
            if *edge.weight() == RepositoryEdgeKind::Calls
                && let Some(RepositoryNode::Symbol(caller)) = self.graph.node_weight(edge.source())
                && caller.is_test
                && !tests.iter().any(|t| t.id == caller.id)
            {
                tests.push(caller);
            }
        }

        tests
    }

    /// Find all test files associated with a given source file path.
    pub fn find_tests_for_file(&self, file_path: &str) -> Vec<String> {
        let mut test_files = BTreeSet::new();

        // 1. Check symbols in the file for direct test links
        if let Some(sym_indices) = self.file_to_nodes.get(file_path) {
            for &idx in sym_indices {
                if let Some(RepositoryNode::Symbol(sym)) = self.graph.node_weight(idx) {
                    for test_sym in self.find_tests_for_symbol(&sym.id) {
                        test_files.insert(test_sym.file_path.clone());
                    }
                }
            }
        }

        // 2. Check incoming Tests / References edges on the file node itself
        if let Some(&file_idx) = self.file_nodes.get(file_path) {
            for edge in self
                .graph
                .edges_directed(file_idx, petgraph::Direction::Incoming)
            {
                if *edge.weight() == RepositoryEdgeKind::Tests
                    && let Some(src_path) = self
                        .graph
                        .node_weight(edge.source())
                        .and_then(|n| n.as_file_path())
                {
                    test_files.insert(src_path.to_string());
                }
            }
        }

        // 3. Fallback: convention-based matching (e.g. tests/<stem>_test.rs or src/<stem>_test.rs)
        let stem = std::path::Path::new(file_path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("");
        if !stem.is_empty() {
            let possible_patterns = [
                format!("{stem}_test"),
                format!("test_{stem}"),
                format!("{stem}test"),
            ];

            for known_file in self.file_nodes.keys() {
                if self.get_file_classification(known_file) == FileClassification::Test {
                    let known_stem = std::path::Path::new(known_file)
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("");
                    if possible_patterns
                        .iter()
                        .any(|p| known_stem.contains(p.as_str()))
                    {
                        test_files.insert(known_file.clone());
                    }
                }
            }
        }

        test_files.into_iter().collect()
    }

    /// Get all callers of a symbol (incoming Calls edges).
    pub fn get_callers(&self, symbol_id: &str) -> Vec<&RepositorySymbol> {
        let Some(&sym_idx) = self.symbol_to_node.get(symbol_id) else {
            return Vec::new();
        };

        let mut callers = Vec::new();
        for edge in self
            .graph
            .edges_directed(sym_idx, petgraph::Direction::Incoming)
        {
            if *edge.weight() == RepositoryEdgeKind::Calls
                && let Some(RepositoryNode::Symbol(caller)) = self.graph.node_weight(edge.source())
                && !callers
                    .iter()
                    .any(|s: &&RepositorySymbol| s.id == caller.id)
            {
                callers.push(caller);
            }
        }
        callers
    }

    /// Get all callees of a symbol (outgoing Calls edges).
    pub fn get_callees(&self, symbol_id: &str) -> Vec<&RepositorySymbol> {
        let Some(&sym_idx) = self.symbol_to_node.get(symbol_id) else {
            return Vec::new();
        };

        let mut callees = Vec::new();
        for edge in self.graph.edges(sym_idx) {
            if *edge.weight() == RepositoryEdgeKind::Calls
                && let Some(RepositoryNode::Symbol(callee)) = self.graph.node_weight(edge.target())
                && !callees
                    .iter()
                    .any(|s: &&RepositorySymbol| s.id == callee.id)
            {
                callees.push(callee);
            }
        }
        callees
    }

    /// Get direct dependencies of a symbol (outgoing DependsOn, Imports, Implements edges).
    pub fn get_dependencies(&self, symbol_id: &str) -> Vec<&RepositorySymbol> {
        let Some(&sym_idx) = self.symbol_to_node.get(symbol_id) else {
            return Vec::new();
        };

        let mut deps = Vec::new();
        for edge in self.graph.edges(sym_idx) {
            match *edge.weight() {
                RepositoryEdgeKind::DependsOn
                | RepositoryEdgeKind::Imports
                | RepositoryEdgeKind::Implements => {
                    if let Some(RepositoryNode::Symbol(dep)) = self.graph.node_weight(edge.target())
                        && !deps.iter().any(|s: &&RepositorySymbol| s.id == dep.id)
                    {
                        deps.push(dep);
                    }
                }
                _ => {}
            }
        }
        deps
    }

    /// Find related files for a set of target files by traversing graph edges.
    pub fn find_related_files(&self, target_files: &[String]) -> Vec<String> {
        let mut related = BTreeSet::new();

        for file in target_files {
            // Include tests
            for test_file in self.find_tests_for_file(file) {
                related.insert(test_file);
            }

            // Include files connected via symbol dependencies or calls
            if let Some(nodes) = self.file_to_nodes.get(file) {
                for &idx in nodes {
                    // Outgoing edges
                    for edge in self.graph.edges(idx) {
                        if let Some(target_path) = self
                            .graph
                            .node_weight(edge.target())
                            .and_then(|n| n.as_file_path())
                            && target_path != file
                        {
                            related.insert(target_path.to_string());
                        }
                    }
                    // Incoming edges
                    for edge in self
                        .graph
                        .edges_directed(idx, petgraph::Direction::Incoming)
                    {
                        if let Some(source_path) = self
                            .graph
                            .node_weight(edge.source())
                            .and_then(|n| n.as_file_path())
                            && source_path != file
                        {
                            related.insert(source_path.to_string());
                        }
                    }
                }
            }
        }

        // Remove targets themselves
        for file in target_files {
            related.remove(file);
        }

        related.into_iter().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repo::types::{FactClass, SymbolKind};

    #[test]
    fn test_repository_graph_add_and_invalidate() {
        let mut graph = RepositoryGraph::new(1);

        let f1 = graph.add_file("src/lib.rs", "rust");
        let sym1 = RepositorySymbol::new(
            "src/lib.rs::fn::run",
            "run",
            "crate::run",
            SymbolKind::Function,
            "src/lib.rs",
            10,
            20,
            "pub fn run()",
            FactClass::VerifiedFact,
            None,
        );
        let _s1 = graph.add_symbol(sym1, Some(f1));

        let f2 = graph.add_file("src/utils.rs", "rust");
        let sym2 = RepositorySymbol::new(
            "src/utils.rs::fn::helper",
            "helper",
            "crate::utils::helper",
            SymbolKind::Function,
            "src/utils.rs",
            5,
            12,
            "pub fn helper()",
            FactClass::VerifiedFact,
            None,
        );
        let s2 = graph.add_symbol(sym2, Some(f2));

        // Add relationship
        let rel_added = graph.add_relationship(
            "src/lib.rs::fn::run",
            "src/utils.rs::fn::helper",
            RepositoryEdgeKind::Calls,
        );
        assert!(rel_added);

        assert_eq!(graph.node_count(), 4);
        assert_eq!(graph.edge_count(), 3); // 2 Contains, 1 Calls

        // Invalidate file 1
        graph.invalidate_file("src/lib.rs");

        // Nodes for file 1 should be gone
        assert!(graph.get_symbol("src/lib.rs::fn::run").is_none());
        assert_eq!(graph.get_file_symbols("src/lib.rs").len(), 0);

        // Node for file 2 must remain completely intact at its stable index!
        assert_eq!(
            graph.get_symbol("src/utils.rs::fn::helper").unwrap().name,
            "helper"
        );
        assert!(graph.graph.contains_node(s2));

        // Node count should be 2 (file 2 and symbol 2)
        assert_eq!(graph.node_count(), 2);
    }

    #[test]
    fn test_generation_isolation() {
        let mut graph = RepositoryGraph::new(1);
        graph.add_file("src/lib.rs", "rust");
        let snapshot1 = graph.snapshot();

        assert_eq!(snapshot1.generation_id, 1);
        assert_eq!(snapshot1.node_count(), 1);

        // Mutate original
        let mut gen2 = graph.next_generation();
        gen2.add_file("src/main.rs", "rust");

        assert_eq!(gen2.generation_id, 2);
        assert_eq!(gen2.node_count(), 2);

        // Snapshot 1 remains unchanged
        assert_eq!(snapshot1.node_count(), 1);
    }

    #[test]
    fn test_tests_and_relationship_queries() {
        let mut graph = RepositoryGraph::new(1);
        let src_file = graph.add_file("src/service.rs", "rust");
        graph.set_file_classification("src/service.rs", FileClassification::Source);

        let test_file = graph.add_file("tests/service_test.rs", "rust");
        graph.set_file_classification("tests/service_test.rs", FileClassification::Test);

        let prod_sym = RepositorySymbol::new(
            "src/service.rs::fn::process",
            "process",
            "crate::service::process",
            SymbolKind::Function,
            "src/service.rs",
            10,
            20,
            "pub fn process()",
            FactClass::VerifiedFact,
            None,
        );
        let p_idx = graph.add_symbol(prod_sym, Some(src_file));

        let test_sym = RepositorySymbol::new(
            "tests/service_test.rs::fn::test_process",
            "test_process",
            "tests::test_process",
            SymbolKind::Function,
            "tests/service_test.rs",
            5,
            15,
            "fn test_process()",
            FactClass::VerifiedFact,
            None,
        )
        .with_test(true);
        let t_idx = graph.add_symbol(test_sym, Some(test_file));

        // Add Tests edge
        graph
            .graph
            .add_edge(t_idx, p_idx, RepositoryEdgeKind::Tests);
        // Add Calls edge from test to prod
        graph
            .graph
            .add_edge(t_idx, p_idx, RepositoryEdgeKind::Calls);

        // Verify test queries
        let test_syms = graph.get_test_symbols();
        assert_eq!(test_syms.len(), 1);
        assert_eq!(test_syms[0].name, "test_process");

        let found_tests = graph.find_tests_for_symbol("src/service.rs::fn::process");
        assert_eq!(found_tests.len(), 1);
        assert_eq!(found_tests[0].name, "test_process");

        let found_test_files = graph.find_tests_for_file("src/service.rs");
        assert!(found_test_files.contains(&"tests/service_test.rs".to_string()));

        // Callers / Callees
        let callers = graph.get_callers("src/service.rs::fn::process");
        assert_eq!(callers.len(), 1);
        assert_eq!(callers[0].name, "test_process");

        let callees = graph.get_callees("tests/service_test.rs::fn::test_process");
        assert_eq!(callees.len(), 1);
        assert_eq!(callees[0].name, "process");

        // Related files
        let related = graph.find_related_files(&["src/service.rs".to_string()]);
        assert!(related.contains(&"tests/service_test.rs".to_string()));
    }

    #[test]
    fn test_entry_points_and_classifications() {
        let mut graph = RepositoryGraph::new(1);
        let main_file = graph.add_file("src/main.rs", "rust");
        graph.set_file_classification("src/main.rs", FileClassification::Source);
        graph.set_file_subsystem("src/main.rs", SubsystemKind::UserInterface);

        let ep = EntryPoint::new(
            "ep::main",
            crate::repo::types::EntryPointKind::BinaryMain,
            "src/main.rs",
            Some("main".to_string()),
            1,
            "Main executable entry",
        );
        graph.add_entry_point(ep);

        assert_eq!(graph.entry_points().len(), 1);
        assert_eq!(
            graph.get_file_classification("src/main.rs"),
            FileClassification::Source
        );
        assert_eq!(
            graph.get_file_subsystem("src/main.rs"),
            SubsystemKind::UserInterface
        );
        assert!(graph.get_all_files().contains(&"src/main.rs".to_string()));
        assert_eq!(graph.graph.node_count(), 1);
        assert_eq!(main_file.index(), 0);
    }
}
