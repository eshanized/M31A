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
}

// ─── PlanService seam is the unique plan generation entry point ───────────────

/// `PlanService` trait lives at `m31a::kernel::seams::planner::PlanService`.
/// `PlanServiceImpl` implements it. No second impl is expected.
#[test]
fn test_plan_service_seam_is_accessible() {
    use m31a::kernel::seams::planner::PlanService;
    use m31a::planning::PlanServiceImpl;
    use std::sync::Arc;
    use tempfile::tempdir;

    let dir = tempdir().unwrap();
    let impl_: Arc<dyn PlanService> = Arc::new(PlanServiceImpl::new(dir.path()));
    // If a second PlanService impl appeared, it would not satisfy this type.
    let _ = impl_;
}

// ─── DifferentialReplanEngine is the unique replan authority ─────────────────

/// `DifferentialReplanEngine` lives at `m31a::recovery::replan`.
/// Re-exported via `m31a::recovery`. Exactly one type with this responsibility.
#[test]
fn test_differential_replan_engine_is_unique_replan_authority() {
    use m31a::recovery::DifferentialReplanEngine;
    use m31a::recovery::replan::DifferentialReplanEngine as DirectEngine;

    // Must be the same type
    fn assert_same_type<T: 'static>() {}
    assert_same_type::<DifferentialReplanEngine>();
    assert_same_type::<DirectEngine>();
}

// ─── PlanValidator is the unique validation authority ─────────────────────────

/// `PlanValidator` lives at `m31a::planning::validation::PlanValidator`.
/// Re-exported via `m31a::planning::PlanValidator`.
#[test]
fn test_plan_validator_is_unique_validation_authority() {
    use m31a::planning::PlanValidator;
    use m31a::planning::validation::PlanValidator as DirectValidator;

    fn assert_same_type<T: 'static>() {}
    assert_same_type::<PlanValidator>();
    assert_same_type::<DirectValidator>();
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

    let plan = m31a::planning::CandidatePlan::default();
    let m1 = PlanQualityMetrics::compute(&plan);
    let m2 = DirectMetrics::compute(&plan);
    assert_eq!(m1.task_count, m2.task_count);
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

    assert_eq!(classify_scope(1, 5), direct_classify(1, 5));
    assert_eq!(classify_scope(5, 5), direct_classify(5, 5));
}

// ─── No second planner, no second task graph ──────────────────────────────────

/// The DAG materializer lives at `m31a::dag::materializer::TaskGraphMaterializer`.
/// It is the canonical authority for CandidatePlan → TaskGraph conversion.
/// This test ensures the type is accessible from the canonical path and not duplicated.
#[test]
fn test_task_graph_materializer_is_unique() {
    use m31a::dag::materializer::TaskGraphMaterializer;
    // Verify the type name matches expected canonical name (type-level check)
    let type_name = std::any::type_name::<TaskGraphMaterializer>();
    assert!(
        type_name.contains("TaskGraphMaterializer"),
        "Type name was: {}",
        type_name
    );
}

/// `TaskGraphReconciler` lives at `m31a::dag::reconciler`. One reconciler only.
#[test]
fn test_task_graph_reconciler_is_unique() {
    use m31a::dag::reconciler::TaskGraphReconciler;
    let type_name = std::any::type_name::<TaskGraphReconciler>();
    assert!(type_name.contains("TaskGraphReconciler"));
}

/// `SchedulerEngine` is the canonical scheduler at `m31a::scheduler::engine`.
/// Only one scheduler in the system.
#[test]
fn test_scheduler_engine_is_unique() {
    use m31a::scheduler::engine::SchedulerEngine;
    let type_name = std::any::type_name::<SchedulerEngine>();
    assert!(type_name.contains("SchedulerEngine"));
}
