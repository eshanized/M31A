//! Defensive Task DAG validator with deterministic cycle detection (D-16, DAG-01, DAG-06).

use crate::dag::graph::{DependencyEdge, DependencyKind};
use crate::ids::TaskId;
use crate::kernel::plan::{CandidatePlan, CandidateTaskKey};
use crate::state::Task;
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

/// Validation error for graph structure and cycle detection.
#[derive(Debug, Error, PartialEq, Eq, Clone)]
pub enum GraphValidationError {
    #[error("Graph or plan contains no tasks")]
    EmptyGraph,

    #[error("Task references non-existent endpoint: {task_id}")]
    MissingEndpoint { task_id: String },

    #[error("Task has a self-referential dependency: {task_id}")]
    SelfLoop { task_id: String },

    #[error("Duplicate dependency edge between {from} and {to}")]
    DuplicateEdge { from: String, to: String },

    #[error("Cyclic dependency detected in task graph")]
    CycleDetected,

    #[error("Invalid plan structure: {reason}")]
    InvalidPlan { reason: String },
}

impl<N: Clone + Ord + std::fmt::Display> From<crate::dag::ops::GraphError<N>>
    for GraphValidationError
{
    fn from(err: crate::dag::ops::GraphError<N>) -> Self {
        match err {
            crate::dag::ops::GraphError::EmptyGraph => GraphValidationError::EmptyGraph,
            crate::dag::ops::GraphError::SelfLoop { node } => GraphValidationError::SelfLoop {
                task_id: node.to_string(),
            },
            crate::dag::ops::GraphError::MissingEndpoint { node } => {
                GraphValidationError::MissingEndpoint {
                    task_id: node.to_string(),
                }
            }
            crate::dag::ops::GraphError::DuplicateEdge { from, to } => {
                GraphValidationError::DuplicateEdge {
                    from: from.to_string(),
                    to: to.to_string(),
                }
            }
            crate::dag::ops::GraphError::CycleDetected { .. } => {
                GraphValidationError::CycleDetected
            }
        }
    }
}

/// Defensive validator enforcing DAG sanity and producing deterministic topological orderings.
#[derive(Debug, Default, Clone, Copy)]
pub struct TaskGraphValidator;

impl TaskGraphValidator {
    pub fn new() -> Self {
        Self
    }

    /// Validate a CandidatePlan, checking endpoints, self-loops, and cycles.
    /// Returns a deterministic topological ordering of CandidateTaskKeys on success.
    pub fn validate_candidate_plan(
        &self,
        plan: &CandidatePlan,
    ) -> Result<Vec<CandidateTaskKey>, GraphValidationError> {
        if plan.tasks.is_empty() {
            return Err(GraphValidationError::EmptyGraph);
        }

        let mut known_keys = BTreeSet::new();
        for task in &plan.tasks {
            if !known_keys.insert(task.id.clone()) {
                return Err(GraphValidationError::InvalidPlan {
                    reason: format!("Duplicate candidate task key in plan: {}", task.id),
                });
            }
        }

        let edges: Vec<(CandidateTaskKey, CandidateTaskKey)> = plan
            .tasks
            .iter()
            .flat_map(|t| {
                t.depends_on
                    .iter()
                    .map(move |prereq| (prereq.clone(), t.id.clone()))
            })
            .collect();

        crate::dag::ops::topological_sort_validated(known_keys, edges)
            .map_err(GraphValidationError::from)
    }

    /// Validate an assembled TaskGraph components (tasks and edges).
    /// Returns a deterministic topological ordering of TaskIds on success.
    pub fn validate_graph(
        &self,
        tasks: &BTreeMap<TaskId, Task>,
        edges: &BTreeSet<DependencyEdge>,
    ) -> Result<Vec<TaskId>, GraphValidationError> {
        if tasks.is_empty() {
            return Err(GraphValidationError::EmptyGraph);
        }

        let mut seen_edges = BTreeSet::new();

        for edge in edges {
            if edge.prerequisite_id == edge.dependent_id {
                return Err(GraphValidationError::SelfLoop {
                    task_id: edge.prerequisite_id.to_string(),
                });
            }

            if !tasks.contains_key(&edge.prerequisite_id) {
                return Err(GraphValidationError::MissingEndpoint {
                    task_id: edge.prerequisite_id.to_string(),
                });
            }

            if !tasks.contains_key(&edge.dependent_id) {
                return Err(GraphValidationError::MissingEndpoint {
                    task_id: edge.dependent_id.to_string(),
                });
            }

            if !seen_edges.insert((edge.prerequisite_id, edge.dependent_id)) {
                return Err(GraphValidationError::DuplicateEdge {
                    from: edge.prerequisite_id.to_string(),
                    to: edge.dependent_id.to_string(),
                });
            }
        }

        let hard_edges: Vec<(TaskId, TaskId)> = edges
            .iter()
            .filter(|e| e.kind == DependencyKind::HardPrerequisite)
            .map(|e| (e.prerequisite_id, e.dependent_id))
            .collect();

        crate::dag::ops::topological_sort(tasks.keys().copied(), hard_edges)
            .map_err(GraphValidationError::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::{MissionId, TaskId};
    use crate::kernel::plan::{
        CandidatePlan, CandidateTask, CandidateTaskKey, ResourceEstimate, VerificationStrategy,
    };
    use crate::state::Task;
    use crate::state_machine::agent::AgentRole;

    fn make_candidate(id: &str, depends_on: Vec<&str>) -> CandidateTask {
        let mut task = CandidateTask::new(
            id,
            format!("Objective for {}", id),
            AgentRole::implementer(),
            VerificationStrategy::Compilation,
            ResourceEstimate::default(),
        );
        task.depends_on = depends_on.into_iter().map(CandidateTaskKey::new).collect();
        task
    }

    #[test]
    fn test_reject_empty_plan() {
        let validator = TaskGraphValidator::new();
        let plan = CandidatePlan::new("p1", "Empty plan", Vec::new());
        assert_eq!(
            validator.validate_candidate_plan(&plan),
            Err(GraphValidationError::EmptyGraph)
        );
    }

    #[test]
    fn test_reject_self_loop() {
        let validator = TaskGraphValidator::new();
        let plan = CandidatePlan::new(
            "p1",
            "Self-loop plan",
            vec![make_candidate("task_a", vec!["task_a"])],
        );

        assert_eq!(
            validator.validate_candidate_plan(&plan),
            Err(GraphValidationError::SelfLoop {
                task_id: "task_a".to_string()
            })
        );
    }

    #[test]
    fn test_reject_direct_two_node_cycle() {
        let validator = TaskGraphValidator::new();
        let plan = CandidatePlan::new(
            "p1",
            "Cycle plan",
            vec![
                make_candidate("task_a", vec!["task_b"]),
                make_candidate("task_b", vec!["task_a"]),
            ],
        );

        assert_eq!(
            validator.validate_candidate_plan(&plan),
            Err(GraphValidationError::CycleDetected)
        );
    }

    #[test]
    fn test_reject_three_node_indirect_cycle() {
        let validator = TaskGraphValidator::new();
        let plan = CandidatePlan::new(
            "p1",
            "3-node cycle plan",
            vec![
                make_candidate("task_a", vec!["task_c"]),
                make_candidate("task_b", vec!["task_a"]),
                make_candidate("task_c", vec!["task_b"]),
            ],
        );

        assert_eq!(
            validator.validate_candidate_plan(&plan),
            Err(GraphValidationError::CycleDetected)
        );
    }

    #[test]
    fn test_reject_missing_endpoint() {
        let validator = TaskGraphValidator::new();
        let plan = CandidatePlan::new(
            "p1",
            "Missing endpoint plan",
            vec![make_candidate("task_a", vec!["nonexistent"])],
        );

        assert_eq!(
            validator.validate_candidate_plan(&plan),
            Err(GraphValidationError::MissingEndpoint {
                task_id: "nonexistent".to_string()
            })
        );
    }

    #[test]
    fn test_reject_duplicate_dependency() {
        let validator = TaskGraphValidator::new();
        let plan = CandidatePlan::new(
            "p1",
            "Dup edge plan",
            vec![
                make_candidate("task_a", vec![]),
                make_candidate("task_b", vec!["task_a", "task_a"]),
            ],
        );

        assert_eq!(
            validator.validate_candidate_plan(&plan),
            Err(GraphValidationError::DuplicateEdge {
                from: "task_a".to_string(),
                to: "task_b".to_string()
            })
        );
    }

    #[test]
    fn test_deterministic_topological_sort() {
        let validator = TaskGraphValidator::new();
        let plan = CandidatePlan::new(
            "p1",
            "Diamond plan",
            vec![
                make_candidate("task_d", vec!["task_b", "task_c"]),
                make_candidate("task_c", vec!["task_a"]),
                make_candidate("task_b", vec!["task_a"]),
                make_candidate("task_a", vec![]),
            ],
        );

        let order1 = validator.validate_candidate_plan(&plan).unwrap();
        let order2 = validator.validate_candidate_plan(&plan).unwrap();

        assert_eq!(order1, order2);
        let order_names: Vec<&str> = order1.iter().map(|k| k.as_str()).collect();
        assert_eq!(order_names, vec!["task_a", "task_b", "task_c", "task_d"]);
    }

    #[test]
    fn test_validate_graph_roundtrip() {
        let validator = TaskGraphValidator::new();
        let mission_id = MissionId::new();
        let t1 = TaskId::new();
        let t2 = TaskId::new();

        let mut tasks = BTreeMap::new();
        tasks.insert(t1, Task::new(t1, mission_id, "T1".into()));
        tasks.insert(t2, Task::new(t2, mission_id, "T2".into()));

        let mut edges = BTreeSet::new();
        edges.insert(DependencyEdge::hard(t1, t2));

        let order = validator.validate_graph(&tasks, &edges).unwrap();
        assert_eq!(order, vec![t1, t2]);
    }
}
