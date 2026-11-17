//! Explainable Deterministic Maintenance Triage Engine (`m31a-maintenance::triage`).
//!
//! Enforces:
//! 1. Multi-factor deterministic scoring breakdown (severity, operational impact, blast radius, reproducibility, confidence).
//! 2. Strict policy constraints (priority != automatic execution authorization).
//! 3. Conservative autonomy tier assignment.
//! 4. Audit trail logging of triage judgments.

use anyhow::Result;
use tracing::info;

use crate::domain::{
    AutonomyTier, CaseStatus, MaintenanceCase, MaintenanceCategory, MaintenancePolicy,
    MaintenancePriority, SignalSeverity, TriageEvaluation,
};
use crate::storage::MaintenanceStore;

/// Deterministic multi-factor triage evaluator.
#[derive(Debug, Clone)]
pub struct TriageEngine {
    store: MaintenanceStore,
    policy: MaintenancePolicy,
}

impl TriageEngine {
    pub fn new(store: MaintenanceStore, policy: MaintenancePolicy) -> Self {
        Self { store, policy }
    }

    /// Evaluates and scores an open maintenance case.
    pub fn evaluate_case(&self, case: &MaintenanceCase) -> Result<TriageEvaluation> {
        let mut contributing_factors = Vec::new();

        // 1. Severity weight (0.25 to 1.0)
        let severity_score = match case.severity {
            SignalSeverity::Critical => {
                contributing_factors.push("Critical severity signal received".to_string());
                1.0
            }
            SignalSeverity::High => {
                contributing_factors.push("High severity signal observed".to_string());
                0.75
            }
            SignalSeverity::Medium => {
                contributing_factors.push("Medium severity maintenance issue".to_string());
                0.5
            }
            SignalSeverity::Low => {
                contributing_factors.push("Routine low severity observation".to_string());
                0.25
            }
        };

        // 2. Operational Impact Score
        let operational_impact = match case.category {
            MaintenanceCategory::BrokenCi => {
                contributing_factors.push("Broken CI pipeline blocks developer merges".to_string());
                0.9
            }
            MaintenanceCategory::SecurityAdvisory => {
                contributing_factors
                    .push("Security advisory introduces potential exposure".to_string());
                0.95
            }
            MaintenanceCategory::IssueBug => {
                contributing_factors
                    .push("Reported functional bug impairs runtime correctness".to_string());
                0.7
            }
            MaintenanceCategory::DependencyUpdate => {
                contributing_factors.push("Dependency drift maintenance item".to_string());
                0.4
            }
            MaintenanceCategory::ApiMigration => {
                contributing_factors.push("Deprecation or framework migration item".to_string());
                0.5
            }
            MaintenanceCategory::DocDrift => {
                contributing_factors.push("Documentation drift item".to_string());
                0.2
            }
            MaintenanceCategory::ArchitectureDrift => {
                contributing_factors.push("Architectural coupling or smell detected".to_string());
                0.3
            }
        };

        // 3. Blast radius estimation
        let file_count = case.affected_files.len();
        let blast_radius = if file_count > 10 {
            contributing_factors.push(format!("Large blast radius across {} files", file_count));
            0.85
        } else if file_count > 3 {
            contributing_factors.push(format!("Moderate blast radius across {} files", file_count));
            0.5
        } else {
            contributing_factors.push("Isolated blast radius (<= 3 files)".to_string());
            0.2
        };

        // 4. Reproducibility & confidence
        let reproducibility = 0.8;
        let confidence = case.confidence_score.clamp(0.1, 1.0);

        // 5. Verification Feasibility
        let verification_feasibility = match case.category {
            MaintenanceCategory::BrokenCi => 0.95, // Deterministic compiler / test runner
            MaintenanceCategory::DependencyUpdate => 0.9,
            MaintenanceCategory::IssueBug => 0.7,
            MaintenanceCategory::DocDrift => 0.85,
            MaintenanceCategory::ApiMigration => 0.75,
            MaintenanceCategory::SecurityAdvisory => 0.8,
            MaintenanceCategory::ArchitectureDrift => 0.6,
        };

        // 6. Compute composite priority score
        let composite = (severity_score * 0.35)
            + (operational_impact * 0.30)
            + (blast_radius * 0.15)
            + (confidence * 0.10)
            + (verification_feasibility * 0.10);

        let priority = if case.severity == SignalSeverity::Critical || composite >= 0.80 {
            MaintenancePriority::Critical
        } else if composite >= 0.60 {
            MaintenancePriority::High
        } else if composite >= 0.35 {
            MaintenancePriority::Medium
        } else {
            MaintenancePriority::Low
        };

        // 7. Determine human intervention requirement
        let is_major_upgrade = case.category == MaintenanceCategory::DependencyUpdate
            && case.problem_statement.contains("major");
        let is_security_critical = case.category == MaintenanceCategory::SecurityAdvisory
            && case.severity == SignalSeverity::Critical;
        let human_intervention_required = is_major_upgrade
            || is_security_critical
            || case.category == MaintenanceCategory::ApiMigration
            || blast_radius > 0.8;

        if human_intervention_required {
            contributing_factors.push(
                "Policy requires human supervision for high-impact or critical changes".to_string(),
            );
        }

        // 8. Determine recommended autonomy tier
        let recommended_autonomy_tier = if human_intervention_required {
            AutonomyTier::Tier3RestrictedHighImpact
        } else {
            match case.category {
                MaintenanceCategory::BrokenCi | MaintenanceCategory::DependencyUpdate => {
                    AutonomyTier::Tier2GuardedRemediation
                }
                MaintenanceCategory::DocDrift | MaintenanceCategory::IssueBug => {
                    AutonomyTier::Tier1Prepare
                }
                _ => AutonomyTier::Tier0Observe,
            }
        };

        // Bounded by configured policy autonomy tier
        let effective_autonomy_tier = if recommended_autonomy_tier > self.policy.autonomy_tier {
            self.policy.autonomy_tier
        } else {
            recommended_autonomy_tier
        };

        let rationale = format!(
            "Triage evaluated composite score {:.2} (severity: {:.2}, impact: {:.2}, blast: {:.2}, conf: {:.2}, verif: {:.2}). Priority assigned: {:?}. Recommended autonomy: {:?}.",
            composite,
            severity_score,
            operational_impact,
            blast_radius,
            confidence,
            verification_feasibility,
            priority,
            effective_autonomy_tier
        );

        Ok(TriageEvaluation {
            case_id: case.id.clone(),
            severity: case.severity,
            priority,
            confidence_score: confidence,
            operational_impact_score: operational_impact,
            blast_radius_score: blast_radius,
            reproducibility_score: reproducibility,
            verification_feasibility_score: verification_feasibility,
            human_intervention_required,
            recommended_autonomy_tier: effective_autonomy_tier,
            contributing_factors,
            rationale,
        })
    }

    /// Triages an open case, updates its priority, and transitions it to Triaged.
    pub fn triage_case(&self, case_id: &str) -> Result<TriageEvaluation> {
        let mut case = self
            .store
            .get_case(case_id)?
            .ok_or_else(|| anyhow::anyhow!("Maintenance case '{}' not found", case_id))?;

        let eval = self.evaluate_case(&case)?;

        case.priority = eval.priority;
        case.transition_to(CaseStatus::Triaged)?;
        self.store.update_case(&case)?;

        info!(
            case_id = %case.id,
            priority = %eval.priority.as_str(),
            tier = %eval.recommended_autonomy_tier.as_str(),
            "Successfully triaged maintenance case"
        );

        Ok(eval)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use m31a_storage::{BUILTIN_MIGRATIONS, MigrationEngine};
    use rusqlite::Connection;
    use std::sync::{Arc, Mutex};

    fn setup_test_triage() -> TriageEngine {
        let mut conn = Connection::open_in_memory().unwrap();
        MigrationEngine::apply_all(&mut conn, BUILTIN_MIGRATIONS).unwrap();
        let store = MaintenanceStore::new(Arc::new(Mutex::new(conn)));
        let policy = MaintenancePolicy::default();
        TriageEngine::new(store, policy)
    }

    #[test]
    fn test_triage_critical_broken_ci() {
        let engine = setup_test_triage();
        let case = MaintenanceCase::new(
            "repo_1",
            MaintenanceCategory::BrokenCi,
            "Broken build on main branch",
            "Fatal compilation error",
            "ci_main_fail",
            SignalSeverity::Critical,
        );

        let eval = engine.evaluate_case(&case).unwrap();
        assert_eq!(eval.priority, MaintenancePriority::Critical);
        assert!(eval.operational_impact_score >= 0.8);
        assert_eq!(eval.severity, SignalSeverity::Critical);
    }

    #[test]
    fn test_triage_low_doc_drift() {
        let engine = setup_test_triage();
        let case = MaintenanceCase::new(
            "repo_1",
            MaintenanceCategory::DocDrift,
            "Minor doc drift in README",
            "Missing documentation for new helper function",
            "doc_drift_readme",
            SignalSeverity::Low,
        );

        let eval = engine.evaluate_case(&case).unwrap();
        assert_eq!(eval.priority, MaintenancePriority::Low);
        assert!(!eval.human_intervention_required);
    }
}
