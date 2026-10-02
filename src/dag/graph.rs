//! TaskGraph authoritative domain aggregate and dependency edge model (D-09, D-13, D-15).

use crate::ids::{MissionId, TaskGraphId, TaskId};
use crate::kernel::plan::CandidateTaskKey;
use crate::state::Task;
use crate::state_machine::TaskState;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Semantics of a dependency edge between tasks (D-05, D-06).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DependencyKind {
    /// Failure of prerequisite blocks the dependent task.
    HardPrerequisite,
    /// Informational dependency; failure of prerequisite does not block dependent.
    SoftReference,
}

impl std::fmt::Display for DependencyKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::HardPrerequisite => write!(f, "hard_prerequisite"),
            Self::SoftReference => write!(f, "soft_reference"),
        }
    }
}

impl std::str::FromStr for DependencyKind {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().as_str() {
            "hard_prerequisite" | "hard" => Ok(Self::HardPrerequisite),
            "soft_reference" | "soft" => Ok(Self::SoftReference),
            other => Err(format!("Unknown dependency kind: {}", other)),
        }
    }
}

/// A directed edge in the Task DAG representing a dependency between two tasks.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct DependencyEdge {
    pub prerequisite_id: TaskId,
    pub dependent_id: TaskId,
    pub kind: DependencyKind,
}

impl DependencyEdge {
    pub fn hard(prerequisite_id: TaskId, dependent_id: TaskId) -> Self {
        Self {
            prerequisite_id,
            dependent_id,
            kind: DependencyKind::HardPrerequisite,
        }
    }

    pub fn soft(prerequisite_id: TaskId, dependent_id: TaskId) -> Self {
        Self {
            prerequisite_id,
            dependent_id,
            kind: DependencyKind::SoftReference,
        }
    }
}

/// Authoritative TaskGraph domain aggregate.
///
/// Encapsulates revisioned task structures and provides in-memory derived index caches
/// for high-performance scheduling checks and logical wave tier partitioning (D-09, D-13).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TaskGraph {
    pub id: TaskGraphId,
    pub mission_id: MissionId,
    pub revision: u32,
    pub plan_id: String,
    pub status: String,
    pub tasks: BTreeMap<TaskId, Task>,
    pub edges: BTreeSet<DependencyEdge>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,

    // In-memory derived caches (rebuilt on construction and deserialization)
    #[serde(skip)]
    pub dependents_of: BTreeMap<TaskId, BTreeSet<TaskId>>,
    #[serde(skip)]
    pub prerequisites_of: BTreeMap<TaskId, BTreeSet<TaskId>>,
    #[serde(skip)]
    pub in_degrees: BTreeMap<TaskId, usize>,
    #[serde(skip)]
    pub wave_tiers: BTreeMap<usize, Vec<TaskId>>,
    #[serde(skip)]
    pub candidate_to_task: BTreeMap<CandidateTaskKey, TaskId>,
    #[serde(skip)]
    pub task_to_candidate: BTreeMap<TaskId, CandidateTaskKey>,
}

impl TaskGraph {
    /// Construct a TaskGraph from records, automatically computing all derived in-memory index caches.
    #[allow(clippy::too_many_arguments)]
    pub fn build_from_records(
        id: TaskGraphId,
        mission_id: MissionId,
        revision: u32,
        plan_id: String,
        status: String,
        tasks: BTreeMap<TaskId, Task>,
        edges: BTreeSet<DependencyEdge>,
        created_at: DateTime<Utc>,
        updated_at: DateTime<Utc>,
    ) -> Self {
        let mut graph = Self {
            id,
            mission_id,
            revision,
            plan_id,
            status,
            tasks,
            edges,
            created_at,
            updated_at,
            dependents_of: BTreeMap::new(),
            prerequisites_of: BTreeMap::new(),
            in_degrees: BTreeMap::new(),
            wave_tiers: BTreeMap::new(),
            candidate_to_task: BTreeMap::new(),
            task_to_candidate: BTreeMap::new(),
        };

        graph.rebuild_derived_indexes();
        graph
    }

    /// Rebuild all derived in-memory caches (dependents, prerequisites, in-degrees, candidate mappings, and wave tiers).
    pub fn rebuild_derived_indexes(&mut self) {
        let mut candidate_to_task: BTreeMap<CandidateTaskKey, TaskId> = BTreeMap::new();
        let mut task_to_candidate: BTreeMap<TaskId, CandidateTaskKey> = BTreeMap::new();

        for (task_id, task) in &self.tasks {
            if !task.candidate_key.is_empty() {
                let key = CandidateTaskKey::new(task.candidate_key.clone());
                candidate_to_task.insert(key.clone(), *task_id);
                task_to_candidate.insert(*task_id, key);
            }
        }

        let hard_edges = self
            .edges
            .iter()
            .filter(|e| e.kind == DependencyKind::HardPrerequisite)
            .map(|e| (e.prerequisite_id, e.dependent_id));

        let adj = crate::dag::ops::GraphAdjacency::build(self.tasks.keys().copied(), hard_edges);

        let wave_tiers = adj.compute_wave_tiers_permissive(|slice| {
            slice.sort_by(|&a, &b| {
                let prio_a = self.tasks.get(&a).map(|t| t.priority).unwrap_or(100);
                let prio_b = self.tasks.get(&b).map(|t| t.priority).unwrap_or(100);
                prio_b.cmp(&prio_a).then_with(|| a.cmp(&b))
            });
        });

        self.dependents_of = adj.dependents_of;
        self.prerequisites_of = adj.prerequisites_of;
        self.in_degrees = adj.in_degrees;
        self.wave_tiers = wave_tiers;
        self.candidate_to_task = candidate_to_task;
        self.task_to_candidate = task_to_candidate;
    }

    /// Check whether all hard prerequisite dependencies for a task are satisfied (TaskState::Succeeded).
    pub fn is_dependency_satisfied(&self, task_id: TaskId) -> bool {
        if !self.tasks.contains_key(&task_id) {
            return false;
        }

        let completed: BTreeSet<TaskId> = self
            .tasks
            .iter()
            .filter(|(_, t)| t.status == TaskState::Succeeded)
            .map(|(&id, _)| id)
            .collect();

        crate::dag::ops::is_ready(&task_id, &self.prerequisites_of, &completed)
    }

    pub fn find_task_by_candidate_key(&self, key: &CandidateTaskKey) -> Option<&Task> {
        let task_id = self.candidate_to_task.get(key)?;
        self.tasks.get(task_id)
    }

    pub fn find_candidate_key(&self, id: TaskId) -> Option<&CandidateTaskKey> {
        self.task_to_candidate.get(&id)
    }

    pub fn get_task(&self, id: TaskId) -> Option<&Task> {
        self.tasks.get(&id)
    }

    pub fn get_task_mut(&mut self, id: TaskId) -> Option<&mut Task> {
        self.tasks.get_mut(&id)
    }

    pub fn topological_order(&self) -> Vec<TaskId> {
        self.wave_tiers.values().flatten().copied().collect()
    }

    pub fn task_count(&self) -> usize {
        self.tasks.len()
    }

    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    /// Extract bounded task summaries preserving task identities, roles, states, and graph dependencies (D-12, DAG-01, TUI-03).
    pub fn task_summaries(&self) -> Vec<crate::events::types::TaskSummary> {
        self.tasks
            .values()
            .map(|t| {
                let deps = self
                    .prerequisites_of
                    .get(&t.id)
                    .map(|set| set.iter().copied().collect())
                    .unwrap_or_default();
                crate::events::types::TaskSummary {
                    id: t.id,
                    title: t.title.clone(),
                    role: t.role.clone(),
                    status: t.status,
                    dependencies: deps,
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::{MissionId, TaskGraphId, TaskId};
    use crate::state::Task;
    use crate::state_machine::TaskState;

    #[test]
    fn test_task_graph_cache_reconstruction_and_wave_tiers() {
        let mission_id = MissionId::new();
        let graph_id = TaskGraphId::new();

        let t1_id = TaskId::new();
        let t2_id = TaskId::new();
        let t3_id = TaskId::new();
        let t4_id = TaskId::new();

        let mut t1 = Task::new(t1_id, mission_id, "T1".to_string());
        t1.candidate_key = "t1".to_string();
        t1.priority = 100;

        let mut t2 = Task::new(t2_id, mission_id, "T2".to_string());
        t2.candidate_key = "t2".to_string();
        t2.priority = 200;

        let mut t3 = Task::new(t3_id, mission_id, "T3".to_string());
        t3.candidate_key = "t3".to_string();
        t3.priority = 150;

        let mut t4 = Task::new(t4_id, mission_id, "T4".to_string());
        t4.candidate_key = "t4".to_string();
        t4.priority = 50;

        let mut tasks = BTreeMap::new();
        tasks.insert(t1_id, t1);
        tasks.insert(t2_id, t2);
        tasks.insert(t3_id, t3);
        tasks.insert(t4_id, t4);

        // Diamond: T1 -> T2, T1 -> T3, T2 -> T4, T3 -> T4
        let mut edges = BTreeSet::new();
        edges.insert(DependencyEdge::hard(t1_id, t2_id));
        edges.insert(DependencyEdge::hard(t1_id, t3_id));
        edges.insert(DependencyEdge::hard(t2_id, t4_id));
        edges.insert(DependencyEdge::hard(t3_id, t4_id));

        let now = Utc::now();
        let graph = TaskGraph::build_from_records(
            graph_id,
            mission_id,
            1,
            "plan-01".to_string(),
            "active".to_string(),
            tasks,
            edges,
            now,
            now,
        );

        // In-degrees
        assert_eq!(graph.in_degrees.get(&t1_id), Some(&0));
        assert_eq!(graph.in_degrees.get(&t2_id), Some(&1));
        assert_eq!(graph.in_degrees.get(&t3_id), Some(&1));
        assert_eq!(graph.in_degrees.get(&t4_id), Some(&2));

        // Wave tiers: Wave 0 = [T1], Wave 1 = [T2, T3] (sorted by priority: T2=200 > T3=150), Wave 2 = [T4]
        assert_eq!(graph.wave_tiers.len(), 3);
        assert_eq!(graph.wave_tiers.get(&0), Some(&vec![t1_id]));
        assert_eq!(graph.wave_tiers.get(&1), Some(&vec![t2_id, t3_id]));
        assert_eq!(graph.wave_tiers.get(&2), Some(&vec![t4_id]));

        // Candidate lookups
        let key_t2 = CandidateTaskKey::new("t2");
        assert_eq!(
            graph.find_task_by_candidate_key(&key_t2).map(|t| t.id),
            Some(t2_id)
        );
        assert_eq!(
            graph.find_candidate_key(t3_id),
            Some(&CandidateTaskKey::new("t3"))
        );

        // Dependency satisfaction
        assert!(graph.is_dependency_satisfied(t1_id));
        assert!(!graph.is_dependency_satisfied(t2_id));
        assert!(!graph.is_dependency_satisfied(t4_id));

        // Mutate t1 to succeeded
        let mut graph = graph;
        graph.get_task_mut(t1_id).unwrap().status = TaskState::Succeeded;
        assert!(graph.is_dependency_satisfied(t2_id));
        assert!(graph.is_dependency_satisfied(t3_id));
        assert!(!graph.is_dependency_satisfied(t4_id));

        // Mutate t2 to succeeded, t4 still unsatisfied (needs t3)
        graph.get_task_mut(t2_id).unwrap().status = TaskState::Succeeded;
        assert!(!graph.is_dependency_satisfied(t4_id));

        // Mutate t3 to succeeded, now t4 is satisfied
        graph.get_task_mut(t3_id).unwrap().status = TaskState::Succeeded;
        assert!(graph.is_dependency_satisfied(t4_id));
    }
}
