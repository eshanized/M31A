use crate::repo::graph::{RepositoryEdgeKind, RepositoryGraph, RepositoryNode};
use crate::repo::types::{
    EntryPoint, EntryPointKind, FileStaleness, IndexStatus, RepositorySymbol, SourceSlice,
    SourceSliceKind, SubsystemKind,
};
use petgraph::visit::EdgeRef;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet, HashSet, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Non-bypassable runtime ceilings for bounded repository queries (D-12, T-07-10).
///
/// Ceilings are immutable safety boundaries in code. Operator-configurable
/// defaults live in `crate::config::canonical` and are applied via
/// [`QueryBounds::from_config`]; configuration may never raise a ceiling.
pub const CEILING_MAX_RESULTS: usize = 200;

pub const CEILING_MAX_DEPTH: usize = 4;

pub const CEILING_MAX_BYTES: usize = 65536; // 64 KB

/// Operator-configurable defaults — single authority is
/// `crate::config::canonical` (re-exported for compatibility).
pub const DEFAULT_MAX_RESULTS: usize = crate::config::canonical::DEFAULT_QUERY_MAX_RESULTS;
pub const DEFAULT_MAX_DEPTH: usize = crate::config::canonical::DEFAULT_QUERY_MAX_DEPTH;
pub const DEFAULT_MAX_BYTES: usize = crate::config::canonical::DEFAULT_QUERY_MAX_BYTES;

/// Caller-specified query bounds with enforced runtime ceilings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueryBounds {
    pub max_results: usize,
    pub max_depth: usize,
    pub max_bytes: usize,
}

impl Default for QueryBounds {
    fn default() -> Self {
        Self {
            max_results: DEFAULT_MAX_RESULTS,
            max_depth: DEFAULT_MAX_DEPTH,
            max_bytes: DEFAULT_MAX_BYTES,
        }
    }
}

impl QueryBounds {
    pub fn new(max_results: usize, max_depth: usize, max_bytes: usize) -> Self {
        Self {
            max_results,
            max_depth,
            max_bytes,
        }
    }

    /// Operator-configured bounds resolved from `[resources]`-adjacent query
    /// configuration. Currently the query defaults live in
    /// `crate::config::canonical`; this constructor is the single boundary
    /// where configured values enter the query layer (still clamped below).
    pub fn from_configured(max_results: usize, max_depth: usize, max_bytes: usize) -> Self {
        Self::new(max_results, max_depth, max_bytes).clamped()
    }

    /// Enforce non-bypassable runtime ceilings.
    pub fn clamped(&self) -> Self {
        Self {
            max_results: self.max_results.clamp(1, CEILING_MAX_RESULTS),
            max_depth: self.max_depth.clamp(1, CEILING_MAX_DEPTH),
            max_bytes: self.max_bytes.clamp(256, CEILING_MAX_BYTES),
        }
    }
}

/// Generic bounded query response envelope containing truncation and provenance metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoundedQueryResult<T> {
    pub data: T,
    pub truncated: bool,
    pub total_matched: usize,
    pub bytes_used: usize,
    pub bounds_applied: QueryBounds,
}

/// Graph neighborhood of a symbol containing immediate relationships.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SymbolNeighborhood {
    pub root: RepositorySymbol,
    pub callers: Vec<RepositorySymbol>,
    pub callees: Vec<RepositorySymbol>,
    pub dependencies: Vec<RepositorySymbol>,
    pub enclosing_file: String,
}

/// Impact analysis report for one or more modified files.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ChangeImpactReport {
    pub changed_files: Vec<String>,
    pub directly_affected_symbols: Vec<RepositorySymbol>,
    pub downstream_affected_symbols: Vec<RepositorySymbol>,
    pub affected_files: Vec<String>,
    pub recommended_tests: Vec<String>,
    pub affected_subsystems: Vec<SubsystemKind>,
}

fn paginate_and_bound<T: Serialize + Clone>(
    items: Vec<T>,
    bounds: QueryBounds,
) -> BoundedQueryResult<Vec<T>> {
    let clamped_bounds = bounds.clamped();
    let total_matched = items.len();
    let mut results = Vec::new();
    let mut bytes_accum = 0;
    let mut truncated = total_matched > clamped_bounds.max_results;

    for item in items {
        if results.len() >= clamped_bounds.max_results {
            truncated = true;
            break;
        }

        let item_bytes = serde_json::to_vec(&item).map(|v| v.len()).unwrap_or(256);
        if bytes_accum + item_bytes > clamped_bounds.max_bytes && !results.is_empty() {
            truncated = true;
            break;
        }

        bytes_accum += item_bytes;
        results.push(item);
    }

    BoundedQueryResult {
        data: results,
        truncated,
        total_matched,
        bytes_used: bytes_accum,
        bounds_applied: clamped_bounds,
    }
}

/// Strongly typed bounded query engine over an immutable graph snapshot (REP-03, D-12).
#[derive(Clone)]
pub struct BoundedQueryEngine {
    graph: Arc<RepositoryGraph>,
    workspace_root: Option<PathBuf>,
}

impl BoundedQueryEngine {
    pub fn new(graph: Arc<RepositoryGraph>) -> Self {
        Self {
            graph,
            workspace_root: None,
        }
    }

    /// Attach workspace root directory for reading source slices.
    pub fn with_workspace_root(mut self, root: impl Into<PathBuf>) -> Self {
        self.workspace_root = Some(root.into());
        self
    }

    /// Access underlying repository graph snapshot.
    pub fn graph(&self) -> &Arc<RepositoryGraph> {
        &self.graph
    }

    /// Access workspace root if configured.
    pub fn workspace_root(&self) -> Option<&Path> {
        self.workspace_root.as_deref()
    }

    /// Direct symbol lookup by ID.
    pub fn get_symbol(&self, symbol_id: &str) -> Option<RepositorySymbol> {
        self.graph.get_symbol(symbol_id).cloned()
    }

    /// Find symbols matching query string (case-insensitive search on name & qualified_name).
    pub fn find_symbols(
        &self,
        query: &str,
        bounds: QueryBounds,
    ) -> BoundedQueryResult<Vec<RepositorySymbol>> {
        let clamped_bounds = bounds.clamped();
        let query_lower = query.trim().to_lowercase();

        // Collect all candidates matching query
        let mut candidates = Vec::new();

        for node_idx in self.graph.graph.node_indices() {
            if let Some(RepositoryNode::Symbol(sym)) = self.graph.graph.node_weight(node_idx) {
                let name_lower = sym.name.to_lowercase();
                let qname_lower = sym.qualified_name.to_lowercase();

                if name_lower.contains(&query_lower) || qname_lower.contains(&query_lower) {
                    // Score relevance: exact match > prefix match > substring match
                    let score = if name_lower == query_lower {
                        3
                    } else if name_lower.starts_with(&query_lower) {
                        2
                    } else {
                        1
                    };
                    candidates.push((score, sym.clone()));
                }
            }
        }

        // Sort descending by score, then ascending by name
        candidates
            .sort_by(|(s1, sym1), (s2, sym2)| s2.cmp(s1).then_with(|| sym1.name.cmp(&sym2.name)));

        let total_matched = candidates.len();
        let mut results = Vec::new();
        let mut bytes_accum = 0;
        let mut truncated = total_matched > clamped_bounds.max_results;

        for (_, sym) in candidates {
            if results.len() >= clamped_bounds.max_results {
                truncated = true;
                break;
            }

            let sym_bytes = serde_json::to_vec(&sym).map(|v| v.len()).unwrap_or(256);
            if bytes_accum + sym_bytes > clamped_bounds.max_bytes && !results.is_empty() {
                truncated = true;
                break;
            }

            bytes_accum += sym_bytes;
            results.push(sym);
        }

        BoundedQueryResult {
            data: results,
            truncated,
            total_matched,
            bytes_used: bytes_accum,
            bounds_applied: clamped_bounds,
        }
    }

    /// Retrieve file outline (all symbols in file sorted by line).
    pub fn get_file_outline(
        &self,
        path: &str,
        bounds: QueryBounds,
    ) -> BoundedQueryResult<Vec<RepositorySymbol>> {
        let clamped_bounds = bounds.clamped();
        let mut symbols: Vec<RepositorySymbol> = self
            .graph
            .get_file_symbols(path)
            .into_iter()
            .cloned()
            .collect();
        symbols.sort_by_key(|s| s.start_line);

        let total_matched = symbols.len();
        let mut results = Vec::new();
        let mut bytes_accum = 0;
        let mut truncated = total_matched > clamped_bounds.max_results;

        for sym in symbols {
            if results.len() >= clamped_bounds.max_results {
                truncated = true;
                break;
            }

            let sym_bytes = serde_json::to_vec(&sym).map(|v| v.len()).unwrap_or(256);
            if bytes_accum + sym_bytes > clamped_bounds.max_bytes && !results.is_empty() {
                truncated = true;
                break;
            }

            bytes_accum += sym_bytes;
            results.push(sym);
        }

        BoundedQueryResult {
            data: results,
            truncated,
            total_matched,
            bytes_used: bytes_accum,
            bounds_applied: clamped_bounds,
        }
    }

    /// Retrieve neighborhood of a symbol up to clamped max_depth using BFS traversal.
    pub fn get_neighborhood(
        &self,
        symbol_id: &str,
        bounds: QueryBounds,
    ) -> Option<BoundedQueryResult<SymbolNeighborhood>> {
        let root_idx = self.graph.symbol_to_node.get(symbol_id).copied()?;
        let root_sym = match self.graph.graph.node_weight(root_idx)? {
            RepositoryNode::Symbol(s) => s.clone(),
            _ => return None,
        };

        let clamped_bounds = bounds.clamped();

        let mut callers = Vec::new();
        let mut callees = Vec::new();
        let mut dependencies = Vec::new();

        let mut total_items = 0;
        let mut truncated = false;

        // 1. Downstream BFS: outgoing edges for callees and dependencies
        let mut downstream_visited = HashSet::new();
        downstream_visited.insert(root_idx);
        let mut downstream_queue = VecDeque::new();
        downstream_queue.push_back((root_idx, 0));

        while let Some((curr_idx, depth)) = downstream_queue.pop_front() {
            if depth >= clamped_bounds.max_depth {
                continue;
            }

            for edge in self.graph.graph.edges(curr_idx) {
                let target_idx = edge.target();
                let edge_kind = *edge.weight();

                if let Some(RepositoryNode::Symbol(target_sym)) =
                    self.graph.graph.node_weight(target_idx)
                {
                    total_items += 1;
                    if total_items > clamped_bounds.max_results {
                        truncated = true;
                        break;
                    }

                    match edge_kind {
                        RepositoryEdgeKind::Calls => {
                            if !callees
                                .iter()
                                .any(|s: &RepositorySymbol| s.id == target_sym.id)
                            {
                                callees.push(target_sym.clone());
                            }
                        }
                        RepositoryEdgeKind::DependsOn
                        | RepositoryEdgeKind::Imports
                        | RepositoryEdgeKind::Implements
                            if !dependencies
                                .iter()
                                .any(|s: &RepositorySymbol| s.id == target_sym.id) =>
                        {
                            dependencies.push(target_sym.clone());
                        }
                        _ => {}
                    }

                    if downstream_visited.insert(target_idx) {
                        downstream_queue.push_back((target_idx, depth + 1));
                    }
                }
            }

            if truncated {
                break;
            }
        }

        // 2. Upstream BFS: incoming Calls edges for callers
        if !truncated {
            let mut upstream_visited = HashSet::new();
            upstream_visited.insert(root_idx);
            let mut upstream_queue = VecDeque::new();
            upstream_queue.push_back((root_idx, 0));

            while let Some((curr_idx, depth)) = upstream_queue.pop_front() {
                if depth >= clamped_bounds.max_depth {
                    continue;
                }

                for edge in self
                    .graph
                    .graph
                    .edges_directed(curr_idx, petgraph::Direction::Incoming)
                {
                    let source_idx = edge.source();
                    let edge_kind = *edge.weight();

                    if let Some(RepositoryNode::Symbol(source_sym)) =
                        self.graph.graph.node_weight(source_idx)
                    {
                        total_items += 1;
                        if total_items > clamped_bounds.max_results {
                            truncated = true;
                            break;
                        }

                        if edge_kind == RepositoryEdgeKind::Calls
                            && !callers
                                .iter()
                                .any(|s: &RepositorySymbol| s.id == source_sym.id)
                        {
                            callers.push(source_sym.clone());
                        }

                        if upstream_visited.insert(source_idx) {
                            upstream_queue.push_back((source_idx, depth + 1));
                        }
                    }
                }

                if truncated {
                    break;
                }
            }
        }

        let neighborhood = SymbolNeighborhood {
            enclosing_file: root_sym.file_path.clone(),
            root: root_sym,
            callers,
            callees,
            dependencies,
        };

        let bytes_used = serde_json::to_vec(&neighborhood)
            .map(|v| v.len())
            .unwrap_or(512);
        if bytes_used > clamped_bounds.max_bytes {
            truncated = true;
        }

        Some(BoundedQueryResult {
            data: neighborhood,
            truncated,
            total_matched: total_items,
            bytes_used,
            bounds_applied: clamped_bounds,
        })
    }

    /// Find all test symbols that test or reference the given symbol.
    pub fn find_tests_for_symbol(
        &self,
        symbol_id: &str,
        bounds: QueryBounds,
    ) -> BoundedQueryResult<Vec<RepositorySymbol>> {
        let tests = self
            .graph
            .find_tests_for_symbol(symbol_id)
            .into_iter()
            .cloned()
            .collect();
        paginate_and_bound(tests, bounds)
    }

    /// Find all test files associated with a given file path.
    pub fn find_tests_for_file(
        &self,
        file_path: &str,
        bounds: QueryBounds,
    ) -> BoundedQueryResult<Vec<String>> {
        let tests = self.graph.find_tests_for_file(file_path);
        paginate_and_bound(tests, bounds)
    }

    /// Find all immediate callers of a given symbol.
    pub fn find_callers(
        &self,
        symbol_id: &str,
        bounds: QueryBounds,
    ) -> BoundedQueryResult<Vec<RepositorySymbol>> {
        let callers = self
            .graph
            .get_callers(symbol_id)
            .into_iter()
            .cloned()
            .collect();
        paginate_and_bound(callers, bounds)
    }

    /// Find all immediate callees invoked by a given symbol.
    pub fn find_callees(
        &self,
        symbol_id: &str,
        bounds: QueryBounds,
    ) -> BoundedQueryResult<Vec<RepositorySymbol>> {
        let callees = self
            .graph
            .get_callees(symbol_id)
            .into_iter()
            .cloned()
            .collect();
        paginate_and_bound(callees, bounds)
    }

    /// Retrieve entry points, optionally filtered by kind.
    pub fn find_entry_points(
        &self,
        kind: Option<EntryPointKind>,
        bounds: QueryBounds,
    ) -> BoundedQueryResult<Vec<EntryPoint>> {
        let eps = self
            .graph
            .entry_points()
            .iter()
            .filter(|ep| kind.is_none_or(|k| ep.kind == k))
            .cloned()
            .collect();
        paginate_and_bound(eps, bounds)
    }

    /// Find related files based on imports, calls, tests, or references.
    pub fn find_related_files(
        &self,
        file_path: &str,
        bounds: QueryBounds,
    ) -> BoundedQueryResult<Vec<String>> {
        let files = self.graph.find_related_files(&[file_path.to_string()]);
        paginate_and_bound(files, bounds)
    }

    /// Group known files by architectural subsystem.
    pub fn inspect_subsystems(&self) -> BTreeMap<SubsystemKind, Vec<String>> {
        let mut map: BTreeMap<SubsystemKind, Vec<String>> = BTreeMap::new();
        for file in self.graph.get_all_files() {
            let sub = self.graph.get_file_subsystem(&file);
            map.entry(sub).or_default().push(file);
        }
        map
    }

    /// Current synchronization / staleness status of the in-memory index.
    pub fn get_index_status(&self) -> IndexStatus {
        if self.graph.file_nodes.is_empty() {
            IndexStatus::Empty
        } else {
            IndexStatus::Current
        }
    }

    /// Calculate change impact for a set of target files, mapping direct symbols, downstream callers,
    /// affected files, recommended tests, and architectural subsystems.
    pub fn calculate_change_impact(
        &self,
        changed_files: &[String],
        bounds: QueryBounds,
    ) -> BoundedQueryResult<ChangeImpactReport> {
        let clamped = bounds.clamped();
        let mut directly_affected_symbols = Vec::new();
        let mut downstream_affected_symbols = Vec::new();
        let mut affected_files_set = BTreeSet::new();
        let mut recommended_tests_set = BTreeSet::new();
        let mut affected_subsystems_set = BTreeSet::new();

        for file in changed_files {
            affected_files_set.insert(file.clone());
            let sub = self.graph.get_file_subsystem(file);
            affected_subsystems_set.insert(sub);

            // Find tests for changed file
            for test_file in self.graph.find_tests_for_file(file) {
                recommended_tests_set.insert(test_file);
            }

            // Collect direct symbols
            for sym in self.graph.get_file_symbols(file) {
                directly_affected_symbols.push(sym.clone());

                // Find tests for symbol
                for test_sym in self.graph.find_tests_for_symbol(&sym.id) {
                    recommended_tests_set.insert(test_sym.file_path.clone());
                }

                // Collect downstream callers
                for caller in self.graph.get_callers(&sym.id) {
                    if !downstream_affected_symbols
                        .iter()
                        .any(|s: &RepositorySymbol| s.id == caller.id)
                    {
                        downstream_affected_symbols.push(caller.clone());
                        affected_files_set.insert(caller.file_path.clone());
                        let caller_sub = self.graph.get_file_subsystem(&caller.file_path);
                        affected_subsystems_set.insert(caller_sub);
                    }
                }
            }
        }

        let total_matched = directly_affected_symbols.len()
            + downstream_affected_symbols.len()
            + affected_files_set.len()
            + recommended_tests_set.len();

        let mut truncated = false;
        if directly_affected_symbols.len() > clamped.max_results {
            directly_affected_symbols.truncate(clamped.max_results);
            truncated = true;
        }
        if downstream_affected_symbols.len() > clamped.max_results {
            downstream_affected_symbols.truncate(clamped.max_results);
            truncated = true;
        }

        let mut affected_files: Vec<String> = affected_files_set.into_iter().collect();
        if affected_files.len() > clamped.max_results {
            affected_files.truncate(clamped.max_results);
            truncated = true;
        }

        let mut recommended_tests: Vec<String> = recommended_tests_set.into_iter().collect();
        if recommended_tests.len() > clamped.max_results {
            recommended_tests.truncate(clamped.max_results);
            truncated = true;
        }

        let affected_subsystems: Vec<SubsystemKind> = affected_subsystems_set.into_iter().collect();

        let report = ChangeImpactReport {
            changed_files: changed_files.to_vec(),
            directly_affected_symbols,
            downstream_affected_symbols,
            affected_files,
            recommended_tests,
            affected_subsystems,
        };

        let bytes_used = serde_json::to_vec(&report).map(|v| v.len()).unwrap_or(512);
        if bytes_used > clamped.max_bytes {
            truncated = true;
        }

        BoundedQueryResult {
            data: report,
            truncated,
            total_matched,
            bytes_used,
            bounds_applied: clamped,
        }
    }

    /// Check synchronization and staleness of a specific file against the indexed graph.
    pub fn check_file_staleness(&self, file_path: &str) -> FileStaleness {
        let root = match self.workspace_root.as_deref() {
            Some(r) => r,
            None => return FileStaleness::Unknown,
        };

        let full_path = root.join(file_path);
        let in_graph = self.graph.get_file_hash(file_path).is_some()
            || self.graph.file_nodes.contains_key(file_path);

        if !full_path.exists() {
            if in_graph {
                return FileStaleness::MissingFromDisk;
            } else {
                return FileStaleness::MissingFromGraph;
            }
        }

        let content_bytes = match fs::read(&full_path) {
            Ok(b) => b,
            Err(_) => return FileStaleness::Unknown,
        };

        let mut hasher = Sha256::new();
        hasher.update(&content_bytes);
        let disk_hash = format!("{:x}", hasher.finalize());

        if let Some(indexed_hash) = self.graph.get_file_hash(file_path) {
            if disk_hash == indexed_hash {
                FileStaleness::Current
            } else {
                FileStaleness::Modified
            }
        } else {
            FileStaleness::MissingFromGraph
        }
    }

    /// Retrieve bounded source slice for a file with surrounding context lines.
    pub fn get_source_slice(
        &self,
        file_path: &str,
        start_line: usize,
        end_line: usize,
        context_lines: usize,
        bounds: QueryBounds,
    ) -> Option<BoundedQueryResult<SourceSlice>> {
        let clamped = bounds.clamped();
        let root = self.workspace_root.as_deref()?;
        let full_path = root.join(file_path);
        let raw_bytes = fs::read(&full_path).ok()?;
        let content_str = String::from_utf8_lossy(&raw_bytes);
        let all_lines: Vec<&str> = content_str.lines().collect();

        if all_lines.is_empty() {
            return None;
        }

        let is_stale = self.check_file_staleness(file_path) != FileStaleness::Current;

        let s_line = start_line.saturating_sub(context_lines).max(1);
        let e_line = (end_line.saturating_add(context_lines)).min(all_lines.len());
        if s_line > e_line || s_line > all_lines.len() {
            return None;
        }

        let mut slice_text = String::new();
        let mut bytes_used = 0;
        let mut truncated = false;

        for line_no in s_line..=e_line {
            let line_content = all_lines[line_no - 1];
            let formatted = format!("{:4}: {}\n", line_no, line_content);
            if bytes_used + formatted.len() > clamped.max_bytes && !slice_text.is_empty() {
                truncated = true;
                break;
            }
            bytes_used += formatted.len();
            slice_text.push_str(&formatted);
        }

        let mut hasher = Sha256::new();
        hasher.update(slice_text.as_bytes());
        let hash = format!("{:x}", hasher.finalize());

        let slice = SourceSlice::new(
            file_path,
            s_line,
            e_line,
            slice_text,
            SourceSliceKind::ContiguousSlice,
            hash,
            is_stale,
        );

        Some(BoundedQueryResult {
            data: slice,
            truncated,
            total_matched: e_line.saturating_sub(s_line) + 1,
            bytes_used,
            bounds_applied: clamped,
        })
    }

    /// Retrieve source slice for a specific symbol ID, falling back to signature if file unavailable.
    pub fn get_symbol_slice(
        &self,
        symbol_id: &str,
        context_lines: usize,
        bounds: QueryBounds,
    ) -> Option<BoundedQueryResult<SourceSlice>> {
        let sym = self.graph.get_symbol(symbol_id)?;
        let clamped = bounds.clamped();

        // Attempt reading source slice from disk
        if let Some(res) = self.get_source_slice(
            &sym.file_path,
            sym.start_line,
            sym.end_line,
            context_lines,
            bounds,
        ) {
            let mut data = res.data;
            data.slice_kind = SourceSliceKind::SymbolBody;
            return Some(BoundedQueryResult {
                data,
                truncated: res.truncated,
                total_matched: res.total_matched,
                bytes_used: res.bytes_used,
                bounds_applied: res.bounds_applied,
            });
        }

        // Graceful fallback to signature
        let mut signature_content = format!(
            "// {} in {}:{}-{}\n{}",
            sym.kind.as_str(),
            sym.file_path,
            sym.start_line,
            sym.end_line,
            sym.signature
        );
        if let Some(ref doc) = sym.doc_comment {
            signature_content = format!("/// {}\n{}", doc, signature_content);
        }

        let bytes_used = signature_content.len();
        let mut hasher = Sha256::new();
        hasher.update(signature_content.as_bytes());
        let hash = format!("{:x}", hasher.finalize());

        let slice = SourceSlice::new(
            &sym.file_path,
            sym.start_line,
            sym.end_line,
            signature_content,
            SourceSliceKind::SignatureOnly,
            hash,
            false,
        );

        Some(BoundedQueryResult {
            data: slice,
            truncated: false,
            total_matched: 1,
            bytes_used,
            bounds_applied: clamped,
        })
    }

    /// Retrieve whole file content up to bounds max_bytes.
    pub fn get_file_content(
        &self,
        file_path: &str,
        bounds: QueryBounds,
    ) -> Option<BoundedQueryResult<SourceSlice>> {
        let clamped = bounds.clamped();
        let root = self.workspace_root.as_deref()?;
        let full_path = root.join(file_path);
        let raw_bytes = fs::read(&full_path).ok()?;
        let content_str = String::from_utf8_lossy(&raw_bytes);
        let is_stale = self.check_file_staleness(file_path) != FileStaleness::Current;

        let total_bytes = raw_bytes.len();
        let truncated = total_bytes > clamped.max_bytes;
        let content_to_use = if truncated {
            let safe_slice = &raw_bytes[..clamped.max_bytes];
            String::from_utf8_lossy(safe_slice).to_string()
        } else {
            content_str.to_string()
        };

        let mut hasher = Sha256::new();
        hasher.update(content_to_use.as_bytes());
        let hash = format!("{:x}", hasher.finalize());
        let line_count = content_to_use.lines().count();

        let slice = SourceSlice::new(
            file_path,
            1,
            line_count,
            content_to_use,
            SourceSliceKind::FullFile,
            hash,
            is_stale,
        );

        Some(BoundedQueryResult {
            data: slice,
            truncated,
            total_matched: total_bytes,
            bytes_used: raw_bytes.len().min(clamped.max_bytes),
            bounds_applied: clamped,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repo::types::{FactClass, SymbolKind};

    fn sample_graph() -> Arc<RepositoryGraph> {
        let mut graph = RepositoryGraph::new(1);
        let f1 = graph.add_file("src/service.rs", "rust");

        let sym_a = RepositorySymbol::new(
            "src/service.rs::fn::handle_request",
            "handle_request",
            "crate::service::handle_request",
            SymbolKind::Function,
            "src/service.rs",
            10,
            25,
            "pub async fn handle_request()",
            FactClass::VerifiedFact,
            None,
        );
        let _a_idx = graph.add_symbol(sym_a, Some(f1));

        let sym_b = RepositorySymbol::new(
            "src/service.rs::fn::validate_input",
            "validate_input",
            "crate::service::validate_input",
            SymbolKind::Function,
            "src/service.rs",
            30,
            40,
            "fn validate_input() -> bool",
            FactClass::VerifiedFact,
            None,
        );
        let _b_idx = graph.add_symbol(sym_b, Some(f1));

        let sym_c = RepositorySymbol::new(
            "src/service.rs::fn::save_db",
            "save_db",
            "crate::service::save_db",
            SymbolKind::Function,
            "src/service.rs",
            45,
            60,
            "fn save_db()",
            FactClass::VerifiedFact,
            None,
        );
        let _c_idx = graph.add_symbol(sym_c, Some(f1));

        // Relationships: handle_request calls validate_input and save_db
        graph.add_relationship(
            "src/service.rs::fn::handle_request",
            "src/service.rs::fn::validate_input",
            RepositoryEdgeKind::Calls,
        );
        graph.add_relationship(
            "src/service.rs::fn::handle_request",
            "src/service.rs::fn::save_db",
            RepositoryEdgeKind::Calls,
        );

        Arc::new(graph)
    }

    #[test]
    fn test_query_bounds_ceiling_enforcement() {
        let excessive = QueryBounds::new(1000, 10, 10_000_000);
        let clamped = excessive.clamped();

        assert_eq!(clamped.max_results, CEILING_MAX_RESULTS); // 200
        assert_eq!(clamped.max_depth, CEILING_MAX_DEPTH); // 4
        assert_eq!(clamped.max_bytes, CEILING_MAX_BYTES); // 64 KB
    }

    #[test]
    fn test_find_symbols_bounded() {
        let engine = BoundedQueryEngine::new(sample_graph());

        // Exact & prefix match ranking
        let res = engine.find_symbols("handle", QueryBounds::default());
        assert_eq!(res.total_matched, 1);
        assert_eq!(res.data[0].name, "handle_request");
        assert!(!res.truncated);

        // Clamped result limit with truncation metadata
        let tight_bounds = QueryBounds::new(1, 2, 65536);
        let res2 = engine.find_symbols("service", tight_bounds);
        assert_eq!(res2.total_matched, 3);
        assert_eq!(res2.data.len(), 1);
        assert!(res2.truncated);
    }

    #[test]
    fn test_get_neighborhood_traversal() {
        let engine = BoundedQueryEngine::new(sample_graph());

        // Neighborhood for handle_request should find callees validate_input and save_db
        let neighborhood = engine
            .get_neighborhood("src/service.rs::fn::handle_request", QueryBounds::default())
            .expect("neighborhood found");

        assert_eq!(neighborhood.data.root.name, "handle_request");
        assert_eq!(neighborhood.data.callees.len(), 2);
        assert!(
            neighborhood
                .data
                .callees
                .iter()
                .any(|s| s.name == "validate_input")
        );
        assert!(
            neighborhood
                .data
                .callees
                .iter()
                .any(|s| s.name == "save_db")
        );
        assert_eq!(neighborhood.data.callers.len(), 0);

        // Reverse lookup: neighborhood for validate_input should list handle_request as caller
        let caller_neighborhood = engine
            .get_neighborhood("src/service.rs::fn::validate_input", QueryBounds::default())
            .expect("caller neighborhood found");

        assert_eq!(caller_neighborhood.data.callers.len(), 1);
        assert_eq!(caller_neighborhood.data.callers[0].name, "handle_request");
    }

    #[test]
    fn test_get_file_outline() {
        let engine = BoundedQueryEngine::new(sample_graph());
        let outline = engine.get_file_outline("src/service.rs", QueryBounds::default());

        assert_eq!(outline.total_matched, 3);
        assert_eq!(outline.data.len(), 3);
        // Sorted by start_line: handle_request (10), validate_input (30), save_db (45)
        assert_eq!(outline.data[0].name, "handle_request");
        assert_eq!(outline.data[1].name, "validate_input");
        assert_eq!(outline.data[2].name, "save_db");
    }

    #[test]
    fn test_repository_query_methods() {
        let mut graph = RepositoryGraph::new(2);
        let f1 = graph.add_file("src/service.rs", "rust");
        let f2 = graph.add_file("tests/service_test.rs", "rust");
        graph.set_file_classification(
            "src/service.rs",
            crate::repo::types::FileClassification::Source,
        );
        graph.set_file_classification(
            "tests/service_test.rs",
            crate::repo::types::FileClassification::Test,
        );
        graph.set_file_subsystem("src/service.rs", SubsystemKind::Capability);

        let sym_svc = RepositorySymbol::new(
            "src/service.rs::fn::run",
            "run",
            "crate::service::run",
            SymbolKind::Function,
            "src/service.rs",
            10,
            20,
            "pub fn run()",
            FactClass::VerifiedFact,
            None,
        );
        let _svc_idx = graph.add_symbol(sym_svc, Some(f1));

        let test_sym = RepositorySymbol::new(
            "tests/service_test.rs::fn::test_run",
            "test_run",
            "crate::service_test::test_run",
            SymbolKind::Function,
            "tests/service_test.rs",
            5,
            15,
            "fn test_run()",
            FactClass::VerifiedFact,
            None,
        )
        .with_test(true);
        let _test_idx = graph.add_symbol(test_sym, Some(f2));

        // Test calls run
        graph.add_relationship(
            "tests/service_test.rs::fn::test_run",
            "src/service.rs::fn::run",
            RepositoryEdgeKind::Calls,
        );

        // Add entry point
        graph.add_entry_point(EntryPoint::new(
            "ep:main",
            EntryPointKind::BinaryMain,
            "src/main.rs",
            Some("main".to_string()),
            1,
            "Main entry",
        ));

        let engine = BoundedQueryEngine::new(Arc::new(graph));

        // 1. Find tests for symbol
        let test_res =
            engine.find_tests_for_symbol("src/service.rs::fn::run", QueryBounds::default());
        assert_eq!(test_res.total_matched, 1);
        assert_eq!(test_res.data[0].name, "test_run");

        // 2. Find tests for file
        let file_tests = engine.find_tests_for_file("src/service.rs", QueryBounds::default());
        assert!(
            file_tests
                .data
                .contains(&"tests/service_test.rs".to_string())
        );

        // 3. Find callers & callees
        let callers = engine.find_callers("src/service.rs::fn::run", QueryBounds::default());
        assert_eq!(callers.total_matched, 1);
        assert_eq!(callers.data[0].name, "test_run");

        let callees = engine.find_callees(
            "tests/service_test.rs::fn::test_run",
            QueryBounds::default(),
        );
        assert_eq!(callees.total_matched, 1);
        assert_eq!(callees.data[0].name, "run");

        // 4. Entry points
        let eps =
            engine.find_entry_points(Some(EntryPointKind::BinaryMain), QueryBounds::default());
        assert_eq!(eps.total_matched, 1);
        assert_eq!(eps.data[0].id, "ep:main");

        // 5. Change impact
        let impact =
            engine.calculate_change_impact(&["src/service.rs".to_string()], QueryBounds::default());
        assert_eq!(
            impact.data.changed_files,
            vec!["src/service.rs".to_string()]
        );
        assert_eq!(impact.data.directly_affected_symbols.len(), 1);
        assert_eq!(impact.data.downstream_affected_symbols.len(), 1);
        assert!(
            impact
                .data
                .recommended_tests
                .contains(&"tests/service_test.rs".to_string())
        );
        assert!(
            impact
                .data
                .affected_subsystems
                .contains(&SubsystemKind::Capability)
        );

        // 6. Subsystems inspect
        let subsystems = engine.inspect_subsystems();
        assert!(subsystems.contains_key(&SubsystemKind::Capability));
        assert!(
            subsystems
                .get(&SubsystemKind::Capability)
                .unwrap()
                .contains(&"src/service.rs".to_string())
        );

        // 7. Index status
        assert_eq!(engine.get_index_status(), IndexStatus::Current);
    }
}
