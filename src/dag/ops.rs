//! Domain-neutral, strongly-typed, deterministic directed acyclic graph (DAG) algorithms.
//!
//! This module serves as the single authoritative implementation for graph algorithms
//! across the entire M31A codebase, replacing previously duplicated ad-hoc implementations.
//!
//! Supported capabilities:
//! 1. Defensive graph validation (self-loops, dangling endpoints, duplicate edges)
//! 2. Cycle detection and deterministic cycle path extraction
//! 3. Deterministic topological sorting (Kahn's algorithm with `BTreeSet` tie-breaking)
//! 4. Topological execution wave / tier calculation
//! 5. Dependency indexing (dependents, prerequisites, in-degrees, roots, leaves)
//! 6. Transitive reachability (ancestors, descendants)
//! 7. Readiness evaluation against completed prerequisites
//!
//! Domain aggregates (Roadmap, Workflow, TaskGraph, Scheduler waves) preserve their
//! distinct domain semantics and boundaries while delegating all graph operations to this module.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use thiserror::Error;

/// Error type for domain-neutral graph algorithms and validation.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum GraphError<N: Clone + Ord + std::fmt::Display> {
    #[error("Cycle detected involving {} nodes: {nodes:?}", nodes.len())]
    CycleDetected { nodes: Vec<N>, path: Option<Vec<N>> },

    #[error("Self-loop detected on node: {node}")]
    SelfLoop { node: N },

    #[error("Missing dependency / dangling endpoint: {node}")]
    MissingEndpoint { node: N },

    #[error("Duplicate edge from {from} to {to}")]
    DuplicateEdge { from: N, to: N },

    #[error("Graph contains no nodes")]
    EmptyGraph,
}

/// Structured collection of dependency anomalies detected during non-terminating inspection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DependencyAnomalies<N: Clone + Ord> {
    pub self_loops: Vec<N>,
    pub missing_prerequisites: Vec<(N, N)>, // (dependent, missing_prerequisite)
    pub duplicate_dependencies: Vec<(N, N)>, // (dependent, duplicate_prerequisite)
}

impl<N: Clone + Ord> Default for DependencyAnomalies<N> {
    fn default() -> Self {
        Self {
            self_loops: Vec::new(),
            missing_prerequisites: Vec::new(),
            duplicate_dependencies: Vec::new(),
        }
    }
}

impl<N: Clone + Ord> DependencyAnomalies<N> {
    pub fn is_empty(&self) -> bool {
        self.self_loops.is_empty()
            && self.missing_prerequisites.is_empty()
            && self.duplicate_dependencies.is_empty()
    }
}

/// Domain-neutral in-memory representation of directed graph adjacency and in-degree indexes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphAdjacency<N: Clone + Ord> {
    pub nodes: BTreeSet<N>,
    /// Outgoing edges: prerequisite -> set of dependents (tasks that depend on prerequisite).
    pub dependents_of: BTreeMap<N, BTreeSet<N>>,
    /// Incoming edges: dependent -> set of prerequisites (tasks prerequisite must complete first).
    pub prerequisites_of: BTreeMap<N, BTreeSet<N>>,
    /// Number of unsatisfied prerequisites for each node.
    pub in_degrees: BTreeMap<N, usize>,
}

impl<N: Clone + Ord + std::fmt::Display> GraphAdjacency<N> {
    /// Construct a GraphAdjacency permissive of duplicate or unknown edges, initializing all node slots.
    pub fn build(
        nodes: impl IntoIterator<Item = N>,
        edges: impl IntoIterator<Item = (N, N)>,
    ) -> Self {
        let node_set: BTreeSet<N> = nodes.into_iter().collect();
        let mut dependents_of: BTreeMap<N, BTreeSet<N>> = BTreeMap::new();
        let mut prerequisites_of: BTreeMap<N, BTreeSet<N>> = BTreeMap::new();
        let mut in_degrees: BTreeMap<N, usize> = BTreeMap::new();

        for node in &node_set {
            dependents_of.insert(node.clone(), BTreeSet::new());
            prerequisites_of.insert(node.clone(), BTreeSet::new());
            in_degrees.insert(node.clone(), 0);
        }

        for (prereq, dep) in edges {
            if node_set.contains(&prereq)
                && node_set.contains(&dep)
                && prereq != dep
                && let Some(prereq_set) = prerequisites_of.get_mut(&dep)
                && prereq_set.insert(prereq.clone())
            {
                if let Some(dep_set) = dependents_of.get_mut(&prereq) {
                    dep_set.insert(dep.clone());
                }
                if let Some(deg) = in_degrees.get_mut(&dep) {
                    *deg += 1;
                }
            }
        }

        Self {
            nodes: node_set,
            dependents_of,
            prerequisites_of,
            in_degrees,
        }
    }

    /// Construct a GraphAdjacency with strict validation.
    ///
    /// Errors on empty graphs, self-loops, missing endpoints, or duplicate edges.
    pub fn build_validated(
        nodes: impl IntoIterator<Item = N>,
        edges: impl IntoIterator<Item = (N, N)>,
    ) -> Result<Self, GraphError<N>> {
        let node_set: BTreeSet<N> = nodes.into_iter().collect();
        if node_set.is_empty() {
            return Err(GraphError::EmptyGraph);
        }

        let mut dependents_of: BTreeMap<N, BTreeSet<N>> = BTreeMap::new();
        let mut prerequisites_of: BTreeMap<N, BTreeSet<N>> = BTreeMap::new();
        let mut in_degrees: BTreeMap<N, usize> = BTreeMap::new();

        for node in &node_set {
            dependents_of.insert(node.clone(), BTreeSet::new());
            prerequisites_of.insert(node.clone(), BTreeSet::new());
            in_degrees.insert(node.clone(), 0);
        }

        let mut seen_edges: BTreeSet<(N, N)> = BTreeSet::new();

        for (prereq, dep) in edges {
            if prereq == dep {
                return Err(GraphError::SelfLoop { node: prereq });
            }
            if !node_set.contains(&prereq) {
                return Err(GraphError::MissingEndpoint { node: prereq });
            }
            if !node_set.contains(&dep) {
                return Err(GraphError::MissingEndpoint { node: dep });
            }
            if !seen_edges.insert((prereq.clone(), dep.clone())) {
                return Err(GraphError::DuplicateEdge {
                    from: prereq,
                    to: dep,
                });
            }

            dependents_of.get_mut(&prereq).unwrap().insert(dep.clone());
            prerequisites_of
                .get_mut(&dep)
                .unwrap()
                .insert(prereq.clone());
            *in_degrees.get_mut(&dep).unwrap() += 1;
        }

        Ok(Self {
            nodes: node_set,
            dependents_of,
            prerequisites_of,
            in_degrees,
        })
    }

    /// Deterministic topological sort using Kahn's algorithm with `BTreeSet` tie-breaking.
    pub fn topological_sort(&self) -> Result<Vec<N>, GraphError<N>> {
        if self.nodes.is_empty() {
            return Ok(Vec::new());
        }

        let mut in_degree = self.in_degrees.clone();
        let mut ready: BTreeSet<N> = in_degree
            .iter()
            .filter(|(_, deg)| **deg == 0)
            .map(|(k, _)| k.clone())
            .collect();

        let mut order = Vec::with_capacity(self.nodes.len());

        while let Some(current) = ready.pop_first() {
            order.push(current.clone());

            if let Some(deps) = self.dependents_of.get(&current) {
                for dep in deps {
                    if let Some(deg) = in_degree.get_mut(dep) {
                        *deg -= 1;
                        if *deg == 0 {
                            ready.insert(dep.clone());
                        }
                    }
                }
            }
        }

        if order.len() < self.nodes.len() {
            let cyclic_nodes: Vec<N> = in_degree
                .into_iter()
                .filter(|(_, deg)| *deg > 0)
                .map(|(k, _)| k)
                .collect();
            let cyclic_set: BTreeSet<N> = cyclic_nodes.iter().cloned().collect();
            let path = find_cycle_path(&cyclic_set, &self.prerequisites_of);
            return Err(GraphError::CycleDetected {
                nodes: cyclic_nodes,
                path,
            });
        }

        Ok(order)
    }

    /// Compute execution waves (topological level partitioning).
    ///
    /// Wave 0 contains all zero-in-degree nodes, wave 1 contains nodes whose prerequisites
    /// were in wave 0, etc. Each wave is sorted by natural `Ord`.
    pub fn compute_waves(&self) -> Result<Vec<Vec<N>>, GraphError<N>> {
        let tiers = self.compute_wave_tiers_with_sorter(|slice| slice.sort())?;
        Ok(tiers.into_values().collect())
    }

    /// Compute wave tiers using an arbitrary custom tier sorter.
    ///
    /// Returns `BTreeMap<usize, Vec<N>>` mapping wave index to sorted nodes in that wave.
    pub fn compute_wave_tiers_with_sorter<F>(
        &self,
        mut sorter: F,
    ) -> Result<BTreeMap<usize, Vec<N>>, GraphError<N>>
    where
        F: FnMut(&mut [N]),
    {
        if self.nodes.is_empty() {
            return Ok(BTreeMap::new());
        }

        let mut in_degrees = self.in_degrees.clone();
        let mut current_level: Vec<N> = in_degrees
            .iter()
            .filter(|(_, deg)| **deg == 0)
            .map(|(k, _)| k.clone())
            .collect();

        sorter(&mut current_level);

        let mut wave_tiers = BTreeMap::new();
        let mut wave_idx = 0;
        let mut processed_count = 0;

        while !current_level.is_empty() {
            let count = current_level.len();
            wave_tiers.insert(wave_idx, current_level.clone());
            processed_count += count;

            let mut next_level = Vec::new();
            for node in &current_level {
                if let Some(deps) = self.dependents_of.get(node) {
                    for dep in deps {
                        if let Some(deg) = in_degrees.get_mut(dep) {
                            *deg = deg.saturating_sub(1);
                            if *deg == 0 {
                                next_level.push(dep.clone());
                            }
                        }
                    }
                }
            }

            sorter(&mut next_level);
            current_level = next_level;
            wave_idx += 1;
        }

        if processed_count < self.nodes.len() {
            let cyclic_nodes: Vec<N> = in_degrees
                .into_iter()
                .filter(|(_, deg)| *deg > 0)
                .map(|(k, _)| k)
                .collect();
            let cyclic_set: BTreeSet<N> = cyclic_nodes.iter().cloned().collect();
            let path = find_cycle_path(&cyclic_set, &self.prerequisites_of);
            return Err(GraphError::CycleDetected {
                nodes: cyclic_nodes,
                path,
            });
        }

        Ok(wave_tiers)
    }

    /// Permissive wave tier calculation that assigns unassigned cyclic nodes into a final wave.
    /// Used by `TaskGraph` cache rebuilds to maintain total robustness.
    pub fn compute_wave_tiers_permissive<F>(&self, mut sorter: F) -> BTreeMap<usize, Vec<N>>
    where
        F: FnMut(&mut [N]),
    {
        if self.nodes.is_empty() {
            return BTreeMap::new();
        }

        let mut in_degrees = self.in_degrees.clone();
        let mut current_level: Vec<N> = in_degrees
            .iter()
            .filter(|(_, deg)| **deg == 0)
            .map(|(k, _)| k.clone())
            .collect();

        sorter(&mut current_level);

        let mut wave_tiers = BTreeMap::new();
        let mut wave_idx = 0;
        let mut processed_count = 0;

        while !current_level.is_empty() {
            let count = current_level.len();
            wave_tiers.insert(wave_idx, current_level.clone());
            processed_count += count;

            let mut next_level = Vec::new();
            for node in &current_level {
                if let Some(deps) = self.dependents_of.get(node) {
                    for dep in deps {
                        if let Some(deg) = in_degrees.get_mut(dep) {
                            *deg = deg.saturating_sub(1);
                            if *deg == 0 {
                                next_level.push(dep.clone());
                            }
                        }
                    }
                }
            }

            sorter(&mut next_level);
            current_level = next_level;
            wave_idx += 1;
        }

        if processed_count < self.nodes.len() {
            let mut unassigned: Vec<N> = in_degrees
                .into_iter()
                .filter(|(_, deg)| *deg > 0)
                .map(|(k, _)| k)
                .collect();
            sorter(&mut unassigned);
            if !unassigned.is_empty() {
                wave_tiers.insert(wave_idx, unassigned);
            }
        }

        wave_tiers
    }

    /// Check whether all prerequisites of a node are in `completed`.
    pub fn is_ready(&self, node: &N, completed: &BTreeSet<N>) -> bool {
        if !self.nodes.contains(node) {
            return false;
        }
        match self.prerequisites_of.get(node) {
            Some(prereqs) => prereqs.iter().all(|p| completed.contains(p)),
            None => true,
        }
    }

    /// Filter candidates to those whose prerequisites are completely satisfied.
    pub fn ready_nodes<'a>(
        &self,
        candidates: impl IntoIterator<Item = &'a N>,
        completed: &BTreeSet<N>,
    ) -> Vec<N>
    where
        N: 'a,
    {
        candidates
            .into_iter()
            .filter(|&c| self.is_ready(c, completed))
            .cloned()
            .collect()
    }

    /// Compute transitive prerequisites (ancestors) of a node.
    pub fn ancestors_of(&self, node: &N) -> BTreeSet<N> {
        compute_ancestors(node, &self.prerequisites_of)
    }

    /// Compute transitive dependents (descendants) of a node.
    pub fn descendants_of(&self, node: &N) -> BTreeSet<N> {
        compute_descendants(node, &self.dependents_of)
    }

    /// Nodes with in-degree == 0 (no prerequisites).
    pub fn roots(&self) -> BTreeSet<N> {
        compute_roots(&self.in_degrees)
    }

    /// Nodes with out-degree == 0 (no dependents).
    pub fn leaves(&self) -> BTreeSet<N> {
        compute_leaves(&self.nodes, &self.dependents_of)
    }
}

/// Standalone topological sort over arbitrary node and edge iterators.
pub fn topological_sort<N: Clone + Ord + std::fmt::Display>(
    nodes: impl IntoIterator<Item = N>,
    edges: impl IntoIterator<Item = (N, N)>,
) -> Result<Vec<N>, GraphError<N>> {
    let adj = GraphAdjacency::build(nodes, edges);
    adj.topological_sort()
}

/// Standalone topological sort with strict input edge validation.
pub fn topological_sort_validated<N: Clone + Ord + std::fmt::Display>(
    nodes: impl IntoIterator<Item = N>,
    edges: impl IntoIterator<Item = (N, N)>,
) -> Result<Vec<N>, GraphError<N>> {
    let adj = GraphAdjacency::build_validated(nodes, edges)?;
    adj.topological_sort()
}

/// Standalone execution wave calculation over arbitrary node and edge iterators.
pub fn compute_waves<N: Clone + Ord + std::fmt::Display>(
    nodes: impl IntoIterator<Item = N>,
    edges: impl IntoIterator<Item = (N, N)>,
) -> Result<Vec<Vec<N>>, GraphError<N>> {
    let adj = GraphAdjacency::build(nodes, edges);
    adj.compute_waves()
}

/// Standalone execution wave calculation with strict input edge validation.
pub fn compute_waves_validated<N: Clone + Ord + std::fmt::Display>(
    nodes: impl IntoIterator<Item = N>,
    edges: impl IntoIterator<Item = (N, N)>,
) -> Result<Vec<Vec<N>>, GraphError<N>> {
    let adj = GraphAdjacency::build_validated(nodes, edges)?;
    adj.compute_waves()
}

/// Non-terminating dependency inspection collecting all self-loops, missing endpoints, and duplicate edges.
pub fn inspect_dependencies<N: Clone + Ord>(
    known_nodes: &BTreeSet<N>,
    node_dependencies: impl IntoIterator<Item = (N, Vec<N>)>,
) -> DependencyAnomalies<N> {
    let mut anomalies = DependencyAnomalies::default();

    for (node, deps) in node_dependencies {
        let mut seen = BTreeSet::new();
        for dep in deps {
            if dep == node {
                anomalies.self_loops.push(node.clone());
            }
            if !known_nodes.contains(&dep) {
                anomalies
                    .missing_prerequisites
                    .push((node.clone(), dep.clone()));
            }
            if !seen.insert(dep.clone()) {
                anomalies
                    .duplicate_dependencies
                    .push((node.clone(), dep.clone()));
            }
        }
    }

    anomalies
}

/// Extract a deterministic cycle path (`[u1, u2, ..., u1]`) from a cyclic subgraph.
pub fn find_cycle_path<N: Clone + Ord>(
    cyclic_nodes: &BTreeSet<N>,
    prerequisites_of: &BTreeMap<N, BTreeSet<N>>,
) -> Option<Vec<N>> {
    let mut visited: BTreeSet<N> = BTreeSet::new();
    let mut visiting: Vec<N> = Vec::new();

    for start_node in cyclic_nodes {
        if visited.contains(start_node) {
            continue;
        }

        if let Some(path) = dfs_find_cycle(
            start_node,
            cyclic_nodes,
            prerequisites_of,
            &mut visited,
            &mut visiting,
        ) {
            return Some(path);
        }
    }

    None
}

/// Find all distinct simple cycle paths in a graph.
pub fn find_all_cycle_paths<N: Clone + Ord>(
    nodes: &BTreeSet<N>,
    node_prerequisites: &BTreeMap<N, BTreeSet<N>>,
) -> Vec<Vec<N>> {
    let mut visited: BTreeSet<N> = BTreeSet::new();
    let mut visiting: Vec<N> = Vec::new();
    let mut recorded_cycles: BTreeSet<Vec<N>> = BTreeSet::new();

    for start_node in nodes {
        if !visited.contains(start_node) {
            dfs_collect_cycles(
                start_node,
                node_prerequisites,
                &mut visited,
                &mut visiting,
                &mut recorded_cycles,
            );
        }
    }

    recorded_cycles.into_iter().collect()
}

fn dfs_find_cycle<N: Clone + Ord>(
    current: &N,
    cyclic_nodes: &BTreeSet<N>,
    prerequisites_of: &BTreeMap<N, BTreeSet<N>>,
    visited: &mut BTreeSet<N>,
    visiting: &mut Vec<N>,
) -> Option<Vec<N>> {
    if let Some(pos) = visiting.iter().position(|k| k == current) {
        let mut path = visiting[pos..].to_vec();
        path.push(current.clone());
        return Some(path);
    }

    if visited.contains(current) {
        return None;
    }

    visiting.push(current.clone());

    if let Some(prereqs) = prerequisites_of.get(current) {
        for prereq in prereqs {
            if cyclic_nodes.contains(prereq)
                && let Some(path) =
                    dfs_find_cycle(prereq, cyclic_nodes, prerequisites_of, visited, visiting)
            {
                return Some(path);
            }
        }
    }

    visiting.pop();
    visited.insert(current.clone());
    None
}

fn dfs_collect_cycles<N: Clone + Ord>(
    current: &N,
    prerequisites_of: &BTreeMap<N, BTreeSet<N>>,
    visited: &mut BTreeSet<N>,
    visiting: &mut Vec<N>,
    recorded_cycles: &mut BTreeSet<Vec<N>>,
) {
    if let Some(pos) = visiting.iter().position(|k| k == current) {
        let mut path = visiting[pos..].to_vec();
        path.push(current.clone());
        recorded_cycles.insert(path);
        return;
    }

    if visited.contains(current) {
        return;
    }

    visiting.push(current.clone());

    if let Some(prereqs) = prerequisites_of.get(current) {
        for prereq in prereqs {
            if prerequisites_of.contains_key(prereq) {
                dfs_collect_cycles(prereq, prerequisites_of, visited, visiting, recorded_cycles);
            }
        }
    }

    visiting.pop();
    visited.insert(current.clone());
}

/// Compute all transitive ancestors (prerequisites) of `node`.
pub fn compute_ancestors<N: Clone + Ord>(
    node: &N,
    prerequisites_of: &BTreeMap<N, BTreeSet<N>>,
) -> BTreeSet<N> {
    let mut ancestors = BTreeSet::new();
    let mut queue = VecDeque::new();
    queue.push_back(node.clone());

    while let Some(current) = queue.pop_front() {
        if let Some(prereqs) = prerequisites_of.get(&current) {
            for prereq in prereqs {
                if ancestors.insert(prereq.clone()) {
                    queue.push_back(prereq.clone());
                }
            }
        }
    }

    ancestors
}

/// Compute all transitive descendants (dependents) of `node`.
pub fn compute_descendants<N: Clone + Ord>(
    node: &N,
    dependents_of: &BTreeMap<N, BTreeSet<N>>,
) -> BTreeSet<N> {
    let mut descendants = BTreeSet::new();
    let mut queue = VecDeque::new();
    queue.push_back(node.clone());

    while let Some(current) = queue.pop_front() {
        if let Some(deps) = dependents_of.get(&current) {
            for dep in deps {
                if descendants.insert(dep.clone()) {
                    queue.push_back(dep.clone());
                }
            }
        }
    }

    descendants
}

/// Compute roots (nodes with in-degree 0).
pub fn compute_roots<N: Clone + Ord>(in_degrees: &BTreeMap<N, usize>) -> BTreeSet<N> {
    in_degrees
        .iter()
        .filter(|(_, deg)| **deg == 0)
        .map(|(k, _)| k.clone())
        .collect()
}

/// Compute leaves (nodes with out-degree 0).
pub fn compute_leaves<N: Clone + Ord>(
    nodes: &BTreeSet<N>,
    dependents_of: &BTreeMap<N, BTreeSet<N>>,
) -> BTreeSet<N> {
    nodes
        .iter()
        .filter(|n| {
            dependents_of
                .get(n)
                .map(|deps| deps.is_empty())
                .unwrap_or(true)
        })
        .cloned()
        .collect()
}

/// Check if a node is ready given a set of completed nodes.
pub fn is_ready<N: Clone + Ord>(
    node: &N,
    prerequisites_of: &BTreeMap<N, BTreeSet<N>>,
    completed: &BTreeSet<N>,
) -> bool {
    match prerequisites_of.get(node) {
        Some(prereqs) => prereqs.iter().all(|p| completed.contains(p)),
        None => true,
    }
}

/// Filter candidate nodes to those whose prerequisites are all completed.
pub fn find_ready_nodes<'a, N: Clone + Ord + 'a>(
    candidates: impl IntoIterator<Item = &'a N>,
    prerequisites_of: &BTreeMap<N, BTreeSet<N>>,
    completed: &BTreeSet<N>,
) -> Vec<N> {
    candidates
        .into_iter()
        .filter(|&node| is_ready(node, prerequisites_of, completed))
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_graph() {
        let nodes: Vec<String> = vec![];
        let edges: Vec<(String, String)> = vec![];
        assert_eq!(
            GraphAdjacency::build_validated(nodes, edges),
            Err(GraphError::EmptyGraph)
        );

        let nodes2: Vec<String> = vec![];
        let edges2: Vec<(String, String)> = vec![];
        let adj = GraphAdjacency::build(nodes2, edges2);
        assert_eq!(adj.topological_sort().unwrap(), Vec::<String>::new());
    }

    #[test]
    fn test_single_node() {
        let nodes = vec!["A".to_string()];
        let edges: Vec<(String, String)> = vec![];
        let adj = GraphAdjacency::build_validated(nodes, edges).unwrap();
        assert_eq!(adj.topological_sort().unwrap(), vec!["A".to_string()]);
        assert_eq!(adj.compute_waves().unwrap(), vec![vec!["A".to_string()]]);
        assert_eq!(adj.roots(), BTreeSet::from(["A".to_string()]));
        assert_eq!(adj.leaves(), BTreeSet::from(["A".to_string()]));
    }

    #[test]
    fn test_linear_dag() {
        // A -> B -> C
        let nodes = vec!["A".to_string(), "B".to_string(), "C".to_string()];
        let edges = vec![
            ("A".to_string(), "B".to_string()),
            ("B".to_string(), "C".to_string()),
        ];
        let adj = GraphAdjacency::build_validated(nodes, edges).unwrap();
        assert_eq!(
            adj.topological_sort().unwrap(),
            vec!["A".to_string(), "B".to_string(), "C".to_string()]
        );
        assert_eq!(
            adj.compute_waves().unwrap(),
            vec![
                vec!["A".to_string()],
                vec!["B".to_string()],
                vec!["C".to_string()]
            ]
        );
        assert_eq!(
            adj.ancestors_of(&"C".to_string()),
            BTreeSet::from(["A".to_string(), "B".to_string()])
        );
        assert_eq!(
            adj.descendants_of(&"A".to_string()),
            BTreeSet::from(["B".to_string(), "C".to_string()])
        );
    }

    #[test]
    fn test_diamond_dag() {
        // A -> B, A -> C, B -> D, C -> D
        let nodes = vec![
            "A".to_string(),
            "B".to_string(),
            "C".to_string(),
            "D".to_string(),
        ];
        let edges = vec![
            ("A".to_string(), "B".to_string()),
            ("A".to_string(), "C".to_string()),
            ("B".to_string(), "D".to_string()),
            ("C".to_string(), "D".to_string()),
        ];
        let adj = GraphAdjacency::build_validated(nodes, edges).unwrap();
        assert_eq!(
            adj.topological_sort().unwrap(),
            vec![
                "A".to_string(),
                "B".to_string(),
                "C".to_string(),
                "D".to_string()
            ]
        );
        assert_eq!(
            adj.compute_waves().unwrap(),
            vec![
                vec!["A".to_string()],
                vec!["B".to_string(), "C".to_string()],
                vec!["D".to_string()]
            ]
        );
    }

    #[test]
    fn test_cycle_detection() {
        // A -> B -> C -> A
        let nodes = vec!["A".to_string(), "B".to_string(), "C".to_string()];
        let edges = vec![
            ("A".to_string(), "B".to_string()),
            ("B".to_string(), "C".to_string()),
            ("C".to_string(), "A".to_string()),
        ];
        let adj = GraphAdjacency::build_validated(nodes, edges).unwrap();
        match adj.topological_sort() {
            Err(GraphError::CycleDetected { nodes, path }) => {
                assert_eq!(nodes.len(), 3);
                assert!(path.is_some());
            }
            other => panic!("Expected cycle error, got: {:?}", other),
        }
    }

    #[test]
    fn test_self_loop() {
        let nodes = vec!["A".to_string()];
        let edges = vec![("A".to_string(), "A".to_string())];
        assert_eq!(
            GraphAdjacency::build_validated(nodes, edges),
            Err(GraphError::SelfLoop {
                node: "A".to_string()
            })
        );
    }

    #[test]
    fn test_missing_endpoint() {
        let nodes = vec!["A".to_string()];
        let edges = vec![("A".to_string(), "B".to_string())];
        assert_eq!(
            GraphAdjacency::build_validated(nodes, edges),
            Err(GraphError::MissingEndpoint {
                node: "B".to_string()
            })
        );
    }

    #[test]
    fn test_duplicate_edge() {
        let nodes = vec!["A".to_string(), "B".to_string()];
        let edges = vec![
            ("A".to_string(), "B".to_string()),
            ("A".to_string(), "B".to_string()),
        ];
        assert_eq!(
            GraphAdjacency::build_validated(nodes, edges),
            Err(GraphError::DuplicateEdge {
                from: "A".to_string(),
                to: "B".to_string()
            })
        );
    }

    #[test]
    fn test_readiness_determination() {
        let nodes = vec!["A".to_string(), "B".to_string(), "C".to_string()];
        let edges = vec![
            ("A".to_string(), "B".to_string()),
            ("B".to_string(), "C".to_string()),
        ];
        let adj = GraphAdjacency::build_validated(nodes, edges).unwrap();

        let mut completed = BTreeSet::new();
        assert!(adj.is_ready(&"A".to_string(), &completed));
        assert!(!adj.is_ready(&"B".to_string(), &completed));
        assert!(!adj.is_ready(&"C".to_string(), &completed));

        completed.insert("A".to_string());
        assert!(adj.is_ready(&"B".to_string(), &completed));
        assert!(!adj.is_ready(&"C".to_string(), &completed));

        completed.insert("B".to_string());
        assert!(adj.is_ready(&"C".to_string(), &completed));
    }
}
