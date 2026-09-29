//! Workstream E: DAG Algorithm Consolidation Integration Tests
//!
//! Tests complete coverage of:
//! - Graph mathematics (Tests A-N): Linear, Diamond, Merging, Forest, Self-cycle, Multi-cycle,
//!   Missing dependency, Duplicate edge, Determinism across 100 runs, Strict wave tiers,
//!   Large DAG (200 nodes), Empty graph, Single node, Determinism under varying insertion order.
//! - Domain adapters (Tests O-R): Roadmap adapter, Workflow adapter, TaskGraph adapter,
//!   Scheduler wave projection.

use m31a::dag::ops::{
    GraphAdjacency, GraphError, compute_waves_validated, topological_sort,
    topological_sort_validated,
};
use m31a::dag::validator::TaskGraphValidator;
use m31a::dag::{DependencyEdge, TaskGraph};
use m31a::ids::{MissionId, TaskGraphId, TaskId};
use m31a::kernel::plan::{
    CandidatePlan, CandidateTask, CandidateTaskKey, CapabilityAccessMode, CapabilityRequirement,
    ResourceEstimate, VerificationStrategy,
};
use m31a::persistence::sqlite::repositories::SqliteMissionRepository;
use m31a::scheduler::concurrency::ConcurrencyLimits;
use m31a::scheduler::engine::SchedulerEngine;
use m31a::scheduler::resources::ResourceManager;
use m31a::state::mission::Mission;
use m31a::state::task::Task;
use m31a::state_machine::agent::AgentRole;
use m31a::state_machine::task::TaskState;
use m31a::workflow::definition::{
    QualityGate, RecoveryStrategy, WorkflowDefinition, WorkflowStepDefinition,
};
use m31a::workflow::error::WorkflowError;
use m31a::workflow::planning::roadmap::{Roadmap, RoadmapPhase};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use tempfile::tempdir;

// =========================================================================
// Graph Mathematics Tests (Tests A - N)
// =========================================================================

#[test]
fn test_condition_a_linear_dag() {
    // A -> B -> C
    let nodes = vec!["A".to_string(), "B".to_string(), "C".to_string()];
    let edges = vec![
        ("A".to_string(), "B".to_string()),
        ("B".to_string(), "C".to_string()),
    ];

    let order = topological_sort_validated(nodes.clone(), edges.clone()).unwrap();
    assert_eq!(order, vec!["A", "B", "C"]);

    let waves = compute_waves_validated(nodes.clone(), edges.clone()).unwrap();
    assert_eq!(
        waves,
        vec![
            vec!["A".to_string()],
            vec!["B".to_string()],
            vec!["C".to_string()]
        ]
    );

    let adj = GraphAdjacency::build_validated(nodes, edges).unwrap();
    assert_eq!(adj.roots(), BTreeSet::from(["A".to_string()]));
    assert_eq!(adj.leaves(), BTreeSet::from(["C".to_string()]));
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
fn test_condition_b_diamond_branching_dag() {
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

    let order = topological_sort_validated(nodes.clone(), edges.clone()).unwrap();
    assert_eq!(order, vec!["A", "B", "C", "D"]);

    let waves = compute_waves_validated(nodes.clone(), edges.clone()).unwrap();
    assert_eq!(
        waves,
        vec![
            vec!["A".to_string()],
            vec!["B".to_string(), "C".to_string()],
            vec!["D".to_string()]
        ]
    );

    let adj = GraphAdjacency::build_validated(nodes, edges).unwrap();
    assert_eq!(adj.roots(), BTreeSet::from(["A".to_string()]));
    assert_eq!(adj.leaves(), BTreeSet::from(["D".to_string()]));
}

#[test]
fn test_condition_c_merging_dag() {
    // Multiple roots converging to single sink: A -> D, B -> D, C -> D
    let nodes = vec![
        "A".to_string(),
        "B".to_string(),
        "C".to_string(),
        "D".to_string(),
    ];
    let edges = vec![
        ("A".to_string(), "D".to_string()),
        ("B".to_string(), "D".to_string()),
        ("C".to_string(), "D".to_string()),
    ];

    let waves = compute_waves_validated(nodes.clone(), edges.clone()).unwrap();
    assert_eq!(
        waves,
        vec![
            vec!["A".to_string(), "B".to_string(), "C".to_string()],
            vec!["D".to_string()]
        ]
    );

    let adj = GraphAdjacency::build_validated(nodes, edges).unwrap();
    assert_eq!(
        adj.roots(),
        BTreeSet::from(["A".to_string(), "B".to_string(), "C".to_string()])
    );
    assert_eq!(adj.leaves(), BTreeSet::from(["D".to_string()]));
}

#[test]
fn test_condition_d_disconnected_forest() {
    // Independent components: A -> B, C -> D
    let nodes = vec![
        "A".to_string(),
        "B".to_string(),
        "C".to_string(),
        "D".to_string(),
    ];
    let edges = vec![
        ("A".to_string(), "B".to_string()),
        ("C".to_string(), "D".to_string()),
    ];

    let order = topological_sort_validated(nodes.clone(), edges.clone()).unwrap();
    assert_eq!(order, vec!["A", "B", "C", "D"]);

    let waves = compute_waves_validated(nodes.clone(), edges.clone()).unwrap();
    assert_eq!(
        waves,
        vec![
            vec!["A".to_string(), "C".to_string()],
            vec!["B".to_string(), "D".to_string()]
        ]
    );
}

#[test]
fn test_condition_e_self_cycle() {
    // A -> A
    let nodes = vec!["A".to_string()];
    let edges = vec![("A".to_string(), "A".to_string())];

    let err = topological_sort_validated(nodes, edges).unwrap_err();
    assert_eq!(
        err,
        GraphError::SelfLoop {
            node: "A".to_string()
        }
    );
}

#[test]
fn test_condition_f_multi_node_cycle() {
    // A -> B -> C -> A
    let nodes = vec!["A".to_string(), "B".to_string(), "C".to_string()];
    let edges = vec![
        ("A".to_string(), "B".to_string()),
        ("B".to_string(), "C".to_string()),
        ("C".to_string(), "A".to_string()),
    ];

    let err = topological_sort_validated(nodes, edges).unwrap_err();
    match err {
        GraphError::CycleDetected {
            nodes: cycle_nodes,
            path,
        } => {
            assert_eq!(cycle_nodes.len(), 3);
            assert!(path.is_some());
            let p = path.unwrap();
            assert_eq!(p.first(), p.last());
        }
        other => panic!("Expected CycleDetected, got: {:?}", other),
    }
}

#[test]
fn test_condition_g_missing_dependency() {
    // Edges reference non-existent node B
    let nodes = vec!["A".to_string()];
    let edges = vec![("B".to_string(), "A".to_string())];

    let err = topological_sort_validated(nodes, edges).unwrap_err();
    assert_eq!(
        err,
        GraphError::MissingEndpoint {
            node: "B".to_string()
        }
    );
}

#[test]
fn test_condition_h_duplicate_edge() {
    // A -> B defined twice
    let nodes = vec!["A".to_string(), "B".to_string()];
    let edges = vec![
        ("A".to_string(), "B".to_string()),
        ("A".to_string(), "B".to_string()),
    ];

    let err = topological_sort_validated(nodes.clone(), edges.clone()).unwrap_err();
    assert_eq!(
        err,
        GraphError::DuplicateEdge {
            from: "A".to_string(),
            to: "B".to_string(),
        }
    );

    // Permissive mode handles duplicates idempotently
    let permissive_order = topological_sort(nodes, edges).unwrap();
    assert_eq!(permissive_order, vec!["A", "B"]);
}

#[test]
fn test_condition_i_stable_deterministic_topological_order() {
    // Parallel fan-out/fan-in: A -> {B, C, D, E} -> F
    let nodes = vec![
        "A".to_string(),
        "B".to_string(),
        "C".to_string(),
        "D".to_string(),
        "E".to_string(),
        "F".to_string(),
    ];
    let edges = vec![
        ("A".to_string(), "B".to_string()),
        ("A".to_string(), "C".to_string()),
        ("A".to_string(), "D".to_string()),
        ("A".to_string(), "E".to_string()),
        ("B".to_string(), "F".to_string()),
        ("C".to_string(), "F".to_string()),
        ("D".to_string(), "F".to_string()),
        ("E".to_string(), "F".to_string()),
    ];

    let baseline = topological_sort_validated(nodes.clone(), edges.clone()).unwrap();
    assert_eq!(baseline, vec!["A", "B", "C", "D", "E", "F"]);

    for _ in 0..100 {
        let iteration = topological_sort_validated(nodes.clone(), edges.clone()).unwrap();
        assert_eq!(iteration, baseline);
    }
}

#[test]
fn test_condition_j_stable_wave_construction() {
    // Strict multi-tier dependencies
    // Wave 0: A, B
    // Wave 1: C (depends on A), D (depends on A, B)
    // Wave 2: E (depends on C, D)
    let nodes = vec![
        "A".to_string(),
        "B".to_string(),
        "C".to_string(),
        "D".to_string(),
        "E".to_string(),
    ];
    let edges = vec![
        ("A".to_string(), "C".to_string()),
        ("A".to_string(), "D".to_string()),
        ("B".to_string(), "D".to_string()),
        ("C".to_string(), "E".to_string()),
        ("D".to_string(), "E".to_string()),
    ];

    let waves = compute_waves_validated(nodes, edges).unwrap();
    assert_eq!(waves.len(), 3);
    assert_eq!(waves[0], vec!["A".to_string(), "B".to_string()]);
    assert_eq!(waves[1], vec!["C".to_string(), "D".to_string()]);
    assert_eq!(waves[2], vec!["E".to_string()]);
}

#[test]
fn test_condition_k_large_dag() {
    // 20 layers of 10 nodes each = 200 nodes
    let layers = 20;
    let nodes_per_layer = 10;
    let mut nodes = Vec::new();
    let mut edges = Vec::new();

    for layer in 0..layers {
        for idx in 0..nodes_per_layer {
            let node_id = format!("L{:02}_N{:02}", layer, idx);
            nodes.push(node_id.clone());

            if layer > 0 {
                let prev_id = format!("L{:02}_N{:02}", layer - 1, idx);
                edges.push((prev_id, node_id));
            }
        }
    }

    assert_eq!(nodes.len(), 200);

    let start = std::time::Instant::now();
    let order = topological_sort_validated(nodes.clone(), edges.clone()).unwrap();
    let duration = start.elapsed();

    assert_eq!(order.len(), 200);
    assert!(
        duration.as_millis() < 50,
        "Topological sort took too long: {:?}",
        duration
    );

    let waves = compute_waves_validated(nodes, edges).unwrap();
    assert_eq!(waves.len(), 20);
    for wave in &waves {
        assert_eq!(wave.len(), 10);
    }
}

#[test]
fn test_condition_l_empty_graph() {
    let nodes: Vec<String> = vec![];
    let edges: Vec<(String, String)> = vec![];

    assert_eq!(
        topological_sort_validated(nodes.clone(), edges.clone()),
        Err(GraphError::EmptyGraph)
    );

    let permissive = topological_sort(nodes, edges).unwrap();
    assert!(permissive.is_empty());
}

#[test]
fn test_condition_m_single_node_graph() {
    let nodes = vec!["SOLO".to_string()];
    let edges: Vec<(String, String)> = vec![];

    let order = topological_sort_validated(nodes.clone(), edges.clone()).unwrap();
    assert_eq!(order, vec!["SOLO"]);

    let waves = compute_waves_validated(nodes.clone(), edges.clone()).unwrap();
    assert_eq!(waves, vec![vec!["SOLO".to_string()]]);

    let adj = GraphAdjacency::build_validated(nodes, edges).unwrap();
    assert_eq!(adj.roots(), BTreeSet::from(["SOLO".to_string()]));
    assert_eq!(adj.leaves(), BTreeSet::from(["SOLO".to_string()]));
}

#[test]
fn test_condition_n_deterministic_repetition_different_insertion_order() {
    let nodes_order1 = vec![
        "Alpha".to_string(),
        "Beta".to_string(),
        "Gamma".to_string(),
        "Delta".to_string(),
    ];
    let edges_order1 = vec![
        ("Alpha".to_string(), "Beta".to_string()),
        ("Alpha".to_string(), "Gamma".to_string()),
        ("Beta".to_string(), "Delta".to_string()),
        ("Gamma".to_string(), "Delta".to_string()),
    ];

    let nodes_order2 = vec![
        "Delta".to_string(),
        "Gamma".to_string(),
        "Beta".to_string(),
        "Alpha".to_string(),
    ];
    let edges_order2 = vec![
        ("Gamma".to_string(), "Delta".to_string()),
        ("Alpha".to_string(), "Gamma".to_string()),
        ("Beta".to_string(), "Delta".to_string()),
        ("Alpha".to_string(), "Beta".to_string()),
    ];

    let sort1 = topological_sort_validated(nodes_order1, edges_order1).unwrap();
    let sort2 = topological_sort_validated(nodes_order2, edges_order2).unwrap();
    assert_eq!(sort1, sort2);
    assert_eq!(sort1, vec!["Alpha", "Beta", "Gamma", "Delta"]);
}

// =========================================================================
// Domain Adapter Tests (Tests O - R)
// =========================================================================

#[test]
fn test_condition_o_roadmap_domain_adapter() {
    // 1. Valid RoadmapPhase sequence
    let mut roadmap = Roadmap::new("RM-01", "Architecture Foundation");
    let mut p1 = RoadmapPhase::new("phase-01", "Kernel Setup", "Build kernel", "cargo test");
    p1.dependencies = vec![];
    roadmap.phases.push(p1);

    let mut p2 = RoadmapPhase::new("phase-02", "Execution Engine", "Build engine", "cargo test");
    p2.dependencies = vec!["phase-01".to_string()];
    roadmap.phases.push(p2);

    let ordered = roadmap.validate_dag().unwrap();
    assert_eq!(ordered, vec!["phase-01", "phase-02"]);

    // 2. Cyclic Roadmap rejected
    let mut cyclic_roadmap = Roadmap::new("RM-CYCLE", "Cyclic Roadmap");
    let mut cp1 = RoadmapPhase::new("p1", "P1", "P1", "test");
    cp1.dependencies = vec!["p2".to_string()];
    cyclic_roadmap.phases.push(cp1);

    let mut cp2 = RoadmapPhase::new("p2", "P2", "P2", "test");
    cp2.dependencies = vec!["p1".to_string()];
    cyclic_roadmap.phases.push(cp2);

    let err = cyclic_roadmap.validate_dag().unwrap_err();
    match err {
        WorkflowError::CycleDetected { cycle } => {
            assert_eq!(cycle.len(), 2);
        }
        other => panic!("Expected CycleDetected, got: {:?}", other),
    }
}

fn sample_step(key: &str, deps: Vec<&str>) -> WorkflowStepDefinition {
    WorkflowStepDefinition {
        key: key.to_string(),
        name: format!("Step {}", key),
        role: AgentRole::implementer(),
        prompt_template: format!("prompts/{}", key),
        required_inputs: Vec::new(),
        expected_outputs: Vec::new(),
        required_capabilities: Vec::new(),
        quality_gate: QualityGate::default(),
        depends_on: deps.into_iter().map(String::from).collect(),
        timeout_secs: 300,
        allows_parallelism: true,
        recovery_strategy: None,
    }
}

#[test]
fn test_condition_p_workflow_domain_adapter() {
    // 1. Valid WorkflowDefinition topological order
    let def = WorkflowDefinition {
        id: "wf-diamond".to_string(),
        name: "Diamond Workflow".to_string(),
        description: "Test".to_string(),
        version: 1,
        steps: vec![
            sample_step("compile", vec![]),
            sample_step("lint", vec!["compile"]),
            sample_step("test", vec!["compile"]),
            sample_step("package", vec!["lint", "test"]),
        ],
        default_recovery_strategy: RecoveryStrategy::Fail,
    };

    let order = def.topological_order().unwrap();
    assert_eq!(order, vec!["compile", "lint", "test", "package"]);

    // 2. Cyclic workflow rejected
    let cyclic_def = WorkflowDefinition {
        id: "wf-cycle".to_string(),
        name: "Cyclic Workflow".to_string(),
        description: "Test".to_string(),
        version: 1,
        steps: vec![
            sample_step("stepA", vec!["stepB"]),
            sample_step("stepB", vec!["stepA"]),
        ],
        default_recovery_strategy: RecoveryStrategy::Fail,
    };

    let err = cyclic_def.topological_order().unwrap_err();
    match err {
        WorkflowError::CycleDetected { cycle } => {
            assert_eq!(cycle.len(), 2);
        }
        other => panic!("Expected CycleDetected, got: {:?}", other),
    }
}

fn candidate_task(id: &str, deps: Vec<&str>) -> CandidateTask {
    let mut task = CandidateTask::new(
        id,
        format!("Task {}", id),
        AgentRole::implementer(),
        VerificationStrategy::Compilation,
        ResourceEstimate::default(),
    );
    task.depends_on = deps.into_iter().map(CandidateTaskKey::new).collect();
    task.capabilities = vec![CapabilityRequirement::new(
        "workspace",
        CapabilityAccessMode::Write,
    )];
    task
}

#[test]
fn test_condition_q_taskgraph_domain_adapter() {
    // 1. Validate CandidatePlan via TaskGraphValidator
    let tasks = vec![
        candidate_task("t1", vec![]),
        candidate_task("t2", vec!["t1"]),
        candidate_task("t3", vec!["t1"]),
        candidate_task("t4", vec!["t2", "t3"]),
    ];
    let plan = CandidatePlan::new("plan-tg-01", "TaskGraph Adapter Test", tasks);

    let validator = TaskGraphValidator::new();
    let order = validator.validate_candidate_plan(&plan).unwrap();
    assert_eq!(
        order,
        vec![
            CandidateTaskKey::new("t1"),
            CandidateTaskKey::new("t2"),
            CandidateTaskKey::new("t3"),
            CandidateTaskKey::new("t4")
        ]
    );

    // 2. Materialize into TaskGraph and verify wave tiers and in-degrees
    let mission_id = MissionId::new();
    let graph_id = TaskGraphId::new();
    let t1_id = TaskId::new();
    let t2_id = TaskId::new();
    let t3_id = TaskId::new();
    let t4_id = TaskId::new();

    let mut tasks_map = BTreeMap::new();
    let mut t1 = Task::new(t1_id, mission_id, "T1".to_string());
    t1.priority = 100;
    tasks_map.insert(t1_id, t1);

    let mut t2 = Task::new(t2_id, mission_id, "T2".to_string());
    t2.priority = 200;
    tasks_map.insert(t2_id, t2);

    let mut t3 = Task::new(t3_id, mission_id, "T3".to_string());
    t3.priority = 150;
    tasks_map.insert(t3_id, t3);

    let mut t4 = Task::new(t4_id, mission_id, "T4".to_string());
    t4.priority = 50;
    tasks_map.insert(t4_id, t4);

    let mut edges = BTreeSet::new();
    edges.insert(DependencyEdge::hard(t1_id, t2_id));
    edges.insert(DependencyEdge::hard(t1_id, t3_id));
    edges.insert(DependencyEdge::hard(t2_id, t4_id));
    edges.insert(DependencyEdge::hard(t3_id, t4_id));

    let now = chrono::Utc::now();
    let mut graph = TaskGraph::build_from_records(
        graph_id,
        mission_id,
        1,
        "plan-tg-01".to_string(),
        "active".to_string(),
        tasks_map,
        edges,
        now,
        now,
    );

    assert_eq!(graph.in_degrees.get(&t1_id), Some(&0));
    assert_eq!(graph.in_degrees.get(&t2_id), Some(&1));
    assert_eq!(graph.in_degrees.get(&t3_id), Some(&1));
    assert_eq!(graph.in_degrees.get(&t4_id), Some(&2));

    // Wave tiers sorted by priority descending (t2=200 > t3=150)
    assert_eq!(graph.wave_tiers.get(&0), Some(&vec![t1_id]));
    assert_eq!(graph.wave_tiers.get(&1), Some(&vec![t2_id, t3_id]));
    assert_eq!(graph.wave_tiers.get(&2), Some(&vec![t4_id]));

    // Readiness determination via graph.is_dependency_satisfied
    assert!(graph.is_dependency_satisfied(t1_id));
    assert!(!graph.is_dependency_satisfied(t2_id));

    graph.get_task_mut(t1_id).unwrap().status = TaskState::Succeeded;
    assert!(graph.is_dependency_satisfied(t2_id));
    assert!(graph.is_dependency_satisfied(t3_id));
    assert!(!graph.is_dependency_satisfied(t4_id));

    graph.get_task_mut(t2_id).unwrap().status = TaskState::Succeeded;
    graph.get_task_mut(t3_id).unwrap().status = TaskState::Succeeded;
    assert!(graph.is_dependency_satisfied(t4_id));
}

#[tokio::test]
async fn test_condition_r_scheduler_wave_projection() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("scheduler_wave_test.db");
    let pool = m31a::persistence::initialize_database(&db_path)
        .await
        .unwrap();

    let mission_repo = SqliteMissionRepository::new(pool.clone());
    let mission_id = MissionId::new();
    let mission = Mission::new(mission_id, "Scheduler Wave Test Mission".to_string());
    mission_repo.insert(&mission).await.unwrap();

    let limits = ConcurrencyLimits::default();
    let resource_mgr = Arc::new(ResourceManager::new(Some(pool.clone()), 1));
    let engine = SchedulerEngine::new(pool, resource_mgr, limits, None);

    let tasks = vec![
        candidate_task("task1", vec![]),
        candidate_task("task2", vec!["task1"]),
        candidate_task("task3", vec!["task2"]),
    ];
    let plan = CandidatePlan::new("plan-sched-01", "Scheduler Plan", tasks);

    let graph_id = engine.materialize_plan(mission_id, &plan).await.unwrap();
    let snapshot = engine.get_snapshot(mission_id).await.unwrap();

    assert_eq!(snapshot.graph_id, graph_id);
    assert_eq!(snapshot.wave_tiers.len(), 3);
    assert_eq!(snapshot.wave_tiers.get(&0).unwrap().len(), 1);
    assert_eq!(snapshot.wave_tiers.get(&1).unwrap().len(), 1);
    assert_eq!(snapshot.wave_tiers.get(&2).unwrap().len(), 1);
}
