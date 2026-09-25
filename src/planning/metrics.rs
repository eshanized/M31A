//! Plan quality metrics and telemetry for adaptive planning (PLN-22).
//!
//! `PlanQualityMetrics` captures structural quality indicators for a `CandidatePlan`
//! without imposing policy or validation logic — it is a pure measurement artifact.
//! The runtime records these for observability, revision auditing, and planning-quality
//! analysis. It does NOT alter plan state.

use serde::{Deserialize, Serialize};

use crate::kernel::plan::CandidatePlan;

/// Structural quality indicators for a candidate plan (PLN-22).
///
/// Computed deterministically from a `CandidatePlan` snapshot.
/// All fields are counts or ratios derived from existing plan data — no heuristics.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlanQualityMetrics {
    /// Plan identifier this measurement applies to.
    pub plan_id: String,
    /// Revision number at time of measurement.
    pub revision: u32,
    /// Total number of tasks in the plan.
    pub task_count: usize,
    /// Number of tasks that declare at least one affected path.
    pub tasks_with_affected_paths: usize,
    /// Number of tasks that declare at least one expected output.
    pub tasks_with_expected_outputs: usize,
    /// Number of tasks with at least one explicit completion criterion.
    pub tasks_with_completion_criteria: usize,
    /// Number of tasks whose `completion_criteria` is empty (no verification contract).
    pub tasks_without_completion_criteria: usize,
    /// Fraction of tasks that have completion criteria (0.0–1.0).
    pub verification_coverage_ratio: f64,
    /// Number of tasks that declare at least one explicit assumption.
    pub tasks_with_assumptions: usize,
    /// Number of plan-level assumptions (`CandidatePlan::assumptions`).
    pub plan_assumption_count: usize,
    /// Number of plan-level assumptions already marked invalidated.
    pub invalidated_assumption_count: usize,
    /// Number of tasks that reference at least one requirement key.
    pub tasks_with_requirement_keys: usize,
    /// Number of tasks classified as High or Critical risk.
    pub high_risk_task_count: usize,
    /// Number of tasks that declare mutating capabilities (`is_mutating()`).
    pub mutating_task_count: usize,
    /// How many times this plan has been revised (revision - 1).
    pub revision_count: u32,
    /// Whether this plan carries a revision record (was derived from a prior plan).
    pub has_revision_record: bool,
}

impl PlanQualityMetrics {
    /// Compute metrics from a plan snapshot. Pure — does not mutate anything.
    pub fn compute(plan: &CandidatePlan) -> Self {
        let task_count = plan.tasks.len();

        let tasks_with_affected_paths = plan
            .tasks
            .iter()
            .filter(|t| !t.affected_paths.is_empty())
            .count();

        let tasks_with_expected_outputs = plan
            .tasks
            .iter()
            .filter(|t| !t.expected_outputs.is_empty())
            .count();

        let tasks_with_completion_criteria = plan
            .tasks
            .iter()
            .filter(|t| !t.completion_criteria.is_empty())
            .count();

        let tasks_without_completion_criteria = task_count - tasks_with_completion_criteria;

        let verification_coverage_ratio = if task_count == 0 {
            0.0
        } else {
            tasks_with_completion_criteria as f64 / task_count as f64
        };

        let tasks_with_assumptions = plan
            .tasks
            .iter()
            .filter(|t| !t.assumptions.is_empty())
            .count();

        let plan_assumption_count = plan.assumptions.len();

        let invalidated_assumption_count =
            plan.assumptions.iter().filter(|a| a.invalidated).count();

        let tasks_with_requirement_keys = plan
            .tasks
            .iter()
            .filter(|t| !t.requirement_keys.is_empty())
            .count();

        let high_risk_task_count = plan
            .tasks
            .iter()
            .filter(|t| {
                t.risk_level
                    .as_ref()
                    .map(|r| r.is_blocking_threshold())
                    .unwrap_or(false)
            })
            .count();

        let mutating_task_count = plan.tasks.iter().filter(|t| t.is_mutating()).count();

        let revision_count = plan.revision.saturating_sub(1);
        let has_revision_record = plan.revision_record.is_some();

        Self {
            plan_id: plan.plan_id.clone(),
            revision: plan.revision,
            task_count,
            tasks_with_affected_paths,
            tasks_with_expected_outputs,
            tasks_with_completion_criteria,
            tasks_without_completion_criteria,
            verification_coverage_ratio,
            tasks_with_assumptions,
            plan_assumption_count,
            invalidated_assumption_count,
            tasks_with_requirement_keys,
            high_risk_task_count,
            mutating_task_count,
            revision_count,
            has_revision_record,
        }
    }

    /// Returns true if every task has at least one completion criterion.
    pub fn has_full_verification_coverage(&self) -> bool {
        self.task_count > 0 && self.tasks_without_completion_criteria == 0
    }

    /// Returns true if any plan-level assumptions have been invalidated.
    pub fn has_invalidated_assumptions(&self) -> bool {
        self.invalidated_assumption_count > 0
    }

    /// Returns true if this plan has a revision record (was derived from a prior plan).
    pub fn has_revision_record(&self) -> bool {
        self.has_revision_record
    }

    /// Returns a human-readable summary line.
    pub fn summary(&self) -> String {
        format!(
            "plan={} rev={} tasks={} mutating={} high_risk={} verification_coverage={:.0}% invalidated_assumptions={}",
            self.plan_id,
            self.revision,
            self.task_count,
            self.mutating_task_count,
            self.high_risk_task_count,
            self.verification_coverage_ratio * 100.0,
            self.invalidated_assumption_count,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::plan::{CandidatePlanAssumption, TaskRiskLevel};
    use crate::kernel::plan::{
        CandidateTask, CapabilityAccessMode, CapabilityRequirement, ResourceEstimate,
        VerificationStrategy,
    };
    use crate::state_machine::agent::AgentRole;

    fn task(id: &str, mutating: bool) -> CandidateTask {
        let caps = if mutating {
            vec![CapabilityRequirement::new(
                "fs.write",
                CapabilityAccessMode::Write,
            )]
        } else {
            vec![]
        };
        CandidateTask::new(
            id,
            format!("Objective for {}", id),
            AgentRole::implementer(),
            VerificationStrategy::Compilation,
            ResourceEstimate::default(),
        )
        .with_capabilities(caps)
    }

    #[test]
    fn test_metrics_empty_plan() {
        let plan = CandidatePlan::new("p1", "Empty", vec![]);
        let m = PlanQualityMetrics::compute(&plan);
        assert_eq!(m.task_count, 0);
        assert_eq!(m.verification_coverage_ratio, 0.0);
        assert!(!m.has_full_verification_coverage());
    }

    #[test]
    fn test_metrics_no_criteria() {
        let plan = CandidatePlan::new("p2", "No criteria", vec![task("t1", false)]);
        let m = PlanQualityMetrics::compute(&plan);
        assert_eq!(m.task_count, 1);
        assert_eq!(m.tasks_with_completion_criteria, 0);
        assert_eq!(m.tasks_without_completion_criteria, 1);
        assert_eq!(m.verification_coverage_ratio, 0.0);
        assert!(!m.has_full_verification_coverage());
    }

    #[test]
    fn test_metrics_full_criteria() {
        let t = task("t1", true).with_completion_criteria(vec!["cargo test passes".to_string()]);
        let plan = CandidatePlan::new("p3", "Full criteria", vec![t]);
        let m = PlanQualityMetrics::compute(&plan);
        assert_eq!(m.tasks_with_completion_criteria, 1);
        assert_eq!(m.tasks_without_completion_criteria, 0);
        assert!((m.verification_coverage_ratio - 1.0).abs() < f64::EPSILON);
        assert!(m.has_full_verification_coverage());
        assert_eq!(m.mutating_task_count, 1);
    }

    #[test]
    fn test_metrics_high_risk_and_assumptions() {
        let t = task("t1", false)
            .with_risk_level(TaskRiskLevel::Critical)
            .with_assumptions(vec!["auth module exists".to_string()]);
        let mut plan = CandidatePlan::new("p4", "Risk", vec![t]);
        plan.assumptions = vec![
            CandidatePlanAssumption::new("A1", "postgres is available"),
            CandidatePlanAssumption {
                id: "A2".to_string(),
                description: "redis available".to_string(),
                invalidated: true,
            },
        ];
        let m = PlanQualityMetrics::compute(&plan);
        assert_eq!(m.high_risk_task_count, 1);
        assert_eq!(m.tasks_with_assumptions, 1);
        assert_eq!(m.plan_assumption_count, 2);
        assert_eq!(m.invalidated_assumption_count, 1);
        assert!(m.has_invalidated_assumptions());
        assert!(m.summary().contains("invalidated_assumptions=1"));
    }

    #[test]
    fn test_metrics_revision_tracking() {
        let plan = CandidatePlan::new("p5", "Revision", vec![]).with_revision(3);
        let m = PlanQualityMetrics::compute(&plan);
        assert_eq!(m.revision, 3);
        assert_eq!(m.revision_count, 2);
    }
}
