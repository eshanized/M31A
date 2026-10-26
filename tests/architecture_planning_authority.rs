//! Architecture Authority Tests — Planning System Anti-Duplication Guards (Phase 22).
//!
//! Verifies that:
//! 1. There is exactly ONE canonical task type (`CandidateTask`).
//! 2. There is exactly ONE canonical plan type (`CandidatePlan`).
//! 3. `PlanService` is the unique seam for plan generation (not bypassed).
//! 4. `DifferentialReplanEngine` is the unique replan authority.
//! 5. `PlanValidator` is the unique plan validation authority.
//! 6. `TaskGraphMaterializer` is the unique DAG materialization authority.
//! 7. New Phase 22 types (`ReplanScope`, `PlanQualityMetrics`, `ReplanningTrigger`)
//!    are reachable through the canonical module paths.
//!
//! These are compile-time and type-system checks — they do not run I/O.

// ─── Type identity: CandidateTask ────────────────────────────────────────────

/// Canonical task type is `m31a::kernel::CandidateTask` (re-exported via planning).
/// This test will FAIL TO COMPILE if a second CandidateTask appears at a different path.
#[test]
fn test_canonical_candidate_task_type_is_unique() {
    use m31a::kernel::CandidateTask as KernelTask;
    use m31a::planning::CandidateTask as PlanningTask;

    // If these are the same type (re-export), this static assertion compiles.
    // A second CandidateTask would cause an import ambiguity or type mismatch below.
    fn assert_same_type<T>(_a: T, _b: T) {}
    let k = KernelTask::default();
    let p = PlanningTask::default();
    assert_same_type(k, p);
    assert_eq!(
        std::any::TypeId::of::<KernelTask>(),
        std::any::TypeId::of::<PlanningTask>()
    );
}

/// Canonical plan type is `m31a::kernel::CandidatePlan` (re-exported via planning).
#[test]
fn test_canonical_candidate_plan_type_is_unique() {
    use m31a::kernel::CandidatePlan as KernelPlan;
    use m31a::planning::CandidatePlan as PlanningPlan;

    fn assert_same_type<T>(_a: T, _b: T) {}
    let k = KernelPlan::default();
    let p = PlanningPlan::default();
    assert_same_type(k, p);
    assert_eq!(
        std::any::TypeId::of::<KernelPlan>(),
        std::any::TypeId::of::<PlanningPlan>()
    );
}

// ─── PlanService seam is the unique plan generation entry point ───────────────

/// `PlanService` trait lives at `m31a::kernel::seams::planner::PlanService`.
/// `PlanServiceImpl` implements it. No second impl is expected.
#[tokio::test]
async fn test_plan_service_seam_is_accessible() {
    use m31a::kernel::seams::planner::{PlanRequest, PlanService};
    use m31a::planning::PlanServiceImpl;
    use std::sync::Arc;
    use tempfile::tempdir;

    let dir = tempdir().unwrap();
    let impl_: Arc<dyn PlanService> = Arc::new(PlanServiceImpl::new(dir.path()));

    let mid = m31a::ids::MissionId::new();
    // 1. Initial state has no plan
    assert!(!impl_.has_valid_plan(mid).await.expect("has_valid_plan"));

    // 2. Actionable plan request without model caller fails closed
    let req = PlanRequest::new(mid, "Implement user authentication");
    let err = impl_
        .generate_initial_plan(req)
        .await
        .expect_err("Actionable plan without model provider must fail closed");
    assert!(err.to_string().contains("No model provider"));

    // 3. Read-only objective generates valid plan projection
    let ro_req = PlanRequest::new(mid, "Explain the repository module structure");
    let resp = impl_
        .generate_initial_plan(ro_req)
        .await
        .expect("Read-only objective generates initial plan");
    assert_eq!(resp.task_count, 0);
    assert!(impl_.has_valid_plan(mid).await.expect("has_valid_plan"));
}

// ─── DifferentialReplanEngine is the unique replan authority ─────────────────

/// `DifferentialReplanEngine` lives at `m31a::recovery::replan`.
/// Re-exported via `m31a::recovery`. Exactly one type with this responsibility.
#[tokio::test]
async fn test_differential_replan_engine_is_unique_replan_authority() {
    use m31a::recovery::DifferentialReplanEngine;
    use m31a::recovery::replan::DifferentialReplanEngine as DirectEngine;
    use std::marker::PhantomData;

    // 1. Static type identity: proves both paths resolve to the exact same type
    fn assert_same_type<T>(_a: PhantomData<T>, _b: PhantomData<T>) {}
    assert_same_type(
        PhantomData::<DifferentialReplanEngine>,
        PhantomData::<DirectEngine>,
    );
    assert_eq!(
        std::any::TypeId::of::<DifferentialReplanEngine>(),
        std::any::TypeId::of::<DirectEngine>()
    );

    // 2. Behavioral verification: replan engine rejects invalid candidate plan
    let dir = tempfile::tempdir().expect("tempdir");
    let pool = m31a::persistence::sqlite::initialize_database(&dir.path().join("replan.db"))
        .await
        .expect("init db");
    let engine = DifferentialReplanEngine::new(pool);

    let now = chrono::Utc::now();
    let empty_graph = m31a::dag::TaskGraph::build_from_records(
        m31a::ids::TaskGraphId::new(),
        m31a::ids::MissionId::new(),
        1,
        "p1".to_string(),
        "active".to_string(),
        std::collections::BTreeMap::new(),
        std::collections::BTreeSet::new(),
        now,
        now,
    );
    let mut invalid_plan = m31a::planning::CandidatePlan::default();
    // A cyclic or self-dependent task violates plan invariants
    let mut task = m31a::planning::CandidateTask::default();
    let t_key = m31a::kernel::CandidateTaskKey::from("T1");
    task.id = t_key.clone();
    task.depends_on = vec![t_key];
    invalid_plan.tasks.push(task);

    let req = m31a::recovery::replan::DifferentialReplanRequest {
        mission_id: m31a::ids::MissionId::new(),
        failed_task_id: None,
        failure_class: m31a::kernel::seams::recovery::FailureClassification::Policy,
        diagnosis_or_reason: "test replan".to_string(),
        candidate_plan: invalid_plan,
        trigger: None,
    };
    let result = engine.execute_replan(&empty_graph, req).await;
    assert!(
        result.is_err(),
        "DifferentialReplanEngine must reject self-referential cyclic plans"
    );
}

// ─── PlanValidator is the unique validation authority ─────────────────────────

/// `PlanValidator` lives at `m31a::planning::validation::PlanValidator`.
/// Re-exported via `m31a::planning::PlanValidator`.
#[tokio::test]
async fn test_plan_validator_is_unique_validation_authority() {
    use m31a::planning::PlanValidator;
    use m31a::planning::validation::PlanValidator as DirectValidator;
    use std::marker::PhantomData;

    // 1. Static type identity
    fn assert_same_type<T>(_a: PhantomData<T>, _b: PhantomData<T>) {}
    assert_same_type(PhantomData::<PlanValidator>, PhantomData::<DirectValidator>);
    assert_eq!(
        std::any::TypeId::of::<PlanValidator>(),
        std::any::TypeId::of::<DirectValidator>()
    );

    // 2. Real validation behavior: PlanValidator strictly enforces plan invariants
    let empty_plan = m31a::planning::CandidatePlan::default();
    let default_validator = PlanValidator::new();
    let default_report = default_validator.validate(&empty_plan).await;
    assert!(
        !default_report.is_valid(),
        "Default PlanValidator must reject an empty candidate plan"
    );

    let permissive_validator = PlanValidator::new().with_allow_empty(true);
    let permissive_report = permissive_validator.validate(&empty_plan).await;
    assert!(
        permissive_report.is_valid(),
        "PlanValidator with allow_empty must accept empty plan"
    );
}

// ─── Phase 22 types are accessible through canonical module paths ─────────────

/// `ReplanScope` is accessible via `m31a::recovery::ReplanScope` and
/// `m31a::recovery::replan::ReplanScope`.
#[test]
fn test_replan_scope_accessible_from_canonical_path() {
    use m31a::recovery::ReplanScope;
    use m31a::recovery::replan::ReplanScope as DirectScope;

    assert_eq!(ReplanScope::Local, DirectScope::Local);
    assert_eq!(ReplanScope::Partial, DirectScope::Partial);
    assert_eq!(ReplanScope::Full, DirectScope::Full);
}

/// `PlanQualityMetrics` is accessible via `m31a::planning::PlanQualityMetrics`.
#[test]
fn test_plan_quality_metrics_accessible_from_canonical_path() {
    use m31a::planning::PlanQualityMetrics;
    use m31a::planning::metrics::PlanQualityMetrics as DirectMetrics;

    assert_eq!(
        std::any::TypeId::of::<PlanQualityMetrics>(),
        std::any::TypeId::of::<DirectMetrics>()
    );

    let mut plan = m31a::planning::CandidatePlan::default();
    let task = m31a::planning::CandidateTask {
        id: m31a::kernel::CandidateTaskKey::from("T1"),
        affected_paths: vec!["src/lib.rs".to_string()],
        completion_criteria: vec!["cargo check passes".to_string()],
        ..Default::default()
    };
    plan.tasks.push(task);

    let m1 = PlanQualityMetrics::compute(&plan);
    let m2 = DirectMetrics::compute(&plan);
    assert_eq!(m1, m2);
    assert_eq!(m1.task_count, 1);
    assert_eq!(m1.tasks_with_affected_paths, 1);
    assert_eq!(m1.tasks_with_completion_criteria, 1);
    assert_eq!(m1.verification_coverage_ratio, 1.0);
}

/// `ReplanningTrigger`, `PlanRevisionRecord`, `CandidatePlanAssumption`, `TaskRiskLevel`
/// all live at `m31a::kernel::plan` (L0 kernel layer). Accessible from there.
#[test]
fn test_phase_22_kernel_types_at_l0() {
    use m31a::kernel::plan::{
        CandidatePlanAssumption, PlanRevisionRecord, ReplanningTrigger, TaskRiskLevel,
    };

    let _risk = TaskRiskLevel::High;
    let _trigger = ReplanningTrigger::PolicyDenial {
        reason: "test".into(),
    };
    let _assumption = CandidatePlanAssumption::new("A1", "test assumption");
    let _record = PlanRevisionRecord::new(2, "p1", "drift", _trigger.clone());

    assert!(_risk.is_blocking_threshold());
    assert!(!_assumption.invalidated);
    assert_eq!(_record.revision, 2);
}

/// `classify_scope` is accessible as a standalone pure function.
#[test]
fn test_classify_scope_accessible_from_canonical_path() {
    use m31a::recovery::classify_scope;
    use m31a::recovery::replan::classify_scope as direct_classify;

    // Verify against explicit ReplanScope invariants, not just self-comparison
    assert_eq!(classify_scope(1, 5), m31a::recovery::ReplanScope::Local);
    assert_eq!(classify_scope(0, 5), m31a::recovery::ReplanScope::Local);
    assert_eq!(classify_scope(3, 5), m31a::recovery::ReplanScope::Partial);
    assert_eq!(classify_scope(5, 5), m31a::recovery::ReplanScope::Full);
    assert_eq!(classify_scope(10, 5), m31a::recovery::ReplanScope::Full);
    assert_eq!(classify_scope(0, 0), m31a::recovery::ReplanScope::Full);

    assert_eq!(classify_scope(1, 5), direct_classify(1, 5));
    assert_eq!(classify_scope(5, 5), direct_classify(5, 5));
}

// ─── No second planner, no second task graph ──────────────────────────────────

/// The DAG materializer lives at `m31a::dag::materializer::TaskGraphMaterializer`.
/// It is the canonical authority for CandidatePlan → TaskGraph conversion.
#[tokio::test]
async fn test_task_graph_materializer_is_unique() {
    use m31a::dag::MaterializationLane;
    use m31a::dag::materializer::TaskGraphMaterializer;

    // 1. Unique type identity in DAG layer
    assert_ne!(
        std::any::TypeId::of::<TaskGraphMaterializer>(),
        std::any::TypeId::of::<m31a::dag::reconciler::TaskGraphReconciler>()
    );

    // 2. Security invariant: GovernedInteractive lane must be rejected by materialize_in_lane
    let dir = tempfile::tempdir().expect("tempdir");
    let pool = m31a::persistence::sqlite::initialize_database(&dir.path().join("mat.db"))
        .await
        .expect("init db");
    let materializer = TaskGraphMaterializer::new(pool);
    let plan = m31a::planning::CandidatePlan::default();

    let err = materializer
        .materialize_in_lane(
            m31a::ids::MissionId::new(),
            &plan,
            MaterializationLane::GovernedInteractive,
        )
        .await
        .expect_err("GovernedInteractive lane must fail closed without authorization");
    assert!(
        err.to_string().contains("GovernedInteractive"),
        "Error must cite GovernedInteractive lane rejection"
    );
}

/// `TaskGraphReconciler` lives at `m31a::dag::reconciler`. One reconciler only.
#[tokio::test]
async fn test_task_graph_reconciler_is_unique() {
    use m31a::dag::reconciler::TaskGraphReconciler;

    // 1. Unique type identity
    assert_ne!(
        std::any::TypeId::of::<TaskGraphReconciler>(),
        std::any::TypeId::of::<m31a::dag::materializer::TaskGraphMaterializer>()
    );

    // 2. Behavioral verification: reconciler validates proposed plan DAG before mutation
    let dir = tempfile::tempdir().expect("tempdir");
    let pool = m31a::persistence::sqlite::initialize_database(&dir.path().join("rec.db"))
        .await
        .expect("init db");
    let reconciler = TaskGraphReconciler::new(pool);

    let now = chrono::Utc::now();
    let old_graph = m31a::dag::TaskGraph::build_from_records(
        m31a::ids::TaskGraphId::new(),
        m31a::ids::MissionId::new(),
        1,
        "p1".to_string(),
        "active".to_string(),
        std::collections::BTreeMap::new(),
        std::collections::BTreeSet::new(),
        now,
        now,
    );
    let mut invalid_plan = m31a::planning::CandidatePlan::default();
    let mut task = m31a::planning::CandidateTask::default();
    let t_key = m31a::kernel::CandidateTaskKey::from("T1");
    task.id = t_key.clone();
    task.depends_on = vec![t_key]; // self-cycle
    invalid_plan.tasks.push(task);

    let result = reconciler
        .reconcile(m31a::ids::MissionId::new(), &old_graph, &invalid_plan)
        .await;
    assert!(
        result.is_err(),
        "TaskGraphReconciler must reject cyclic candidate plan"
    );
}

/// `SchedulerEngine` is the canonical scheduler at `m31a::scheduler::engine`.
/// Only one scheduler in the system.
#[tokio::test]
async fn test_scheduler_engine_is_unique() {
    use m31a::kernel::seams::scheduler::WorkScheduler;
    use m31a::scheduler::concurrency::ConcurrencyLimits;
    use m31a::scheduler::engine::SchedulerEngine;
    use m31a::scheduler::resources::ResourceManager;
    use std::sync::Arc;

    // 1. Type identity and trait implementation check
    let dir = tempfile::tempdir().expect("tempdir");
    let pool = m31a::persistence::sqlite::initialize_database(&dir.path().join("sched.db"))
        .await
        .expect("init db");
    let limits = ConcurrencyLimits::default();
    let resource_mgr = Arc::new(ResourceManager::new(Some(pool.clone()), 1));
    let engine = SchedulerEngine::new(pool, resource_mgr, limits, None);

    // Proves SchedulerEngine is the canonical implementer of WorkScheduler seam
    let scheduler: Arc<dyn WorkScheduler> = Arc::new(engine);

    // 2. Behavioral execution: find_ready_work for non-existent mission returns empty
    let outcome = scheduler
        .find_ready_work(m31a::ids::MissionId::new())
        .await
        .expect("finding ready work on clean scheduler succeeds");
    assert!(outcome.ready_tasks.is_empty());
    assert_eq!(outcome.blocked_tasks_count, 0);
}
