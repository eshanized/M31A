//! Authoritative TaskGraph domain model, defensive DAG validation, CPM, and reconciler.

pub mod cpm;
pub mod graph;
pub mod materializer;
pub mod ops;
pub mod reconciler;
pub mod validator;

pub use cpm::{CriticalPathInfo, ScheduleDrift, calculate_cpm, calculate_drift};
pub use graph::{DependencyEdge, DependencyKind, TaskGraph};
pub use materializer::{MaterializationLane, TaskGraphMaterializer};
pub use ops::{
    DependencyAnomalies, GraphAdjacency, GraphError, compute_ancestors, compute_descendants,
    compute_leaves, compute_roots, compute_waves, compute_waves_validated, find_all_cycle_paths,
    find_cycle_path, find_ready_nodes, inspect_dependencies, is_ready, topological_sort,
    topological_sort_validated,
};
pub use reconciler::{ReconciliationResult, TaskGraphReconciler, compute_task_fingerprint};
pub use validator::{GraphValidationError, TaskGraphValidator};
