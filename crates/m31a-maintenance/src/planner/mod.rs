//! Evidence-Based Maintenance Planning Engine (`m31a-maintenance::planner`).
//!
//! Synthesizes revision-bound, strategy-driven maintenance plans using repository intelligence,
//! AST blast radius estimation, and autonomous verification requirements.

pub mod strategies;

use anyhow::{Result, anyhow};
use chrono::Utc;
use std::path::Path;
use tracing::info;

pub use strategies::{
    BrokenCiStrategy, CiFailureClassification, DependencyUpdateStrategy, DocDriftStrategy,
    IssueBugFixStrategy, MaintenanceStrategy, SecurityAdvisoryStrategy, StrategyPlanContext,
};

use crate::domain::{
    AuditTrailEvent, CaseStatus, MaintenanceCase, MaintenancePlan, MaintenancePolicy,
    PlanApprovalState,
};
use crate::storage::MaintenanceStore;

/// Central planner coordinating strategy execution and plan persistence.
pub struct MaintenancePlanner {
    store: MaintenanceStore,
    policy: MaintenancePolicy,
    strategies: Vec<Box<dyn MaintenanceStrategy>>,
}

impl MaintenancePlanner {
    pub fn new(store: MaintenanceStore, policy: MaintenancePolicy) -> Self {
        let mut planner = Self {
            store,
            policy,
            strategies: Vec::new(),
        };
        // Register canonical built-in strategies
        planner.register_strategy(Box::new(BrokenCiStrategy));
        planner.register_strategy(Box::new(DependencyUpdateStrategy));
        planner.register_strategy(Box::new(IssueBugFixStrategy));
        planner.register_strategy(Box::new(DocDriftStrategy));
        planner.register_strategy(Box::new(SecurityAdvisoryStrategy));
        planner
    }

    pub fn register_strategy(&mut self, strategy: Box<dyn MaintenanceStrategy>) {
        self.strategies.push(strategy);
    }

    /// Selects appropriate strategy for a case.
    pub fn find_strategy(&self, case: &MaintenanceCase) -> Option<&dyn MaintenanceStrategy> {
        self.strategies
            .iter()
            .find(|s| s.can_handle(case))
            .map(|s| s.as_ref())
    }

    /// Generates and persists a revision-bound maintenance plan for an active case.
    pub fn generate_plan(
        &self,
        case_id: &str,
        workspace_root: &Path,
        baseline_sha: &str,
    ) -> Result<MaintenancePlan> {
        let mut case = self
            .store
            .get_case(case_id)?
            .ok_or_else(|| anyhow!("Maintenance case '{}' not found", case_id))?;

        // 1. Transition case into Planning state if eligible
        if case.status != CaseStatus::Planning {
            if case.status == CaseStatus::Open || case.status == CaseStatus::Triaged {
                case.transition_to(CaseStatus::Investigating)?;
            }
            case.transition_to(CaseStatus::Planning)?;
            self.store.update_case(&case)?;
        }

        // 2. Select strategy
        let strategy = self.find_strategy(&case).ok_or_else(|| {
            anyhow!(
                "No maintenance strategy available for category {:?}",
                case.category
            )
        })?;

        // 3. Formulate plan
        let ctx = StrategyPlanContext {
            workspace_root,
            baseline_sha,
            repo_id: &case.repo_id,
        };

        let mut plan = strategy.create_plan(&case, &ctx)?;

        // Enforce policy risk threshold: if plan risk >= policy approval threshold, require approval
        if plan.estimated_risk >= self.policy.risk_threshold_for_approval {
            plan.requires_approval = true;
            plan.approval_state = PlanApprovalState::Pending;
        }

        // 4. Persist plan
        self.store.save_plan(&plan)?;

        // 5. Update case state
        case.active_plan_id = Some(plan.id.clone());
        if plan.requires_approval {
            case.transition_to(CaseStatus::AwaitingApproval)?;
        } else {
            case.transition_to(CaseStatus::PlanReady)?;
        }
        self.store.update_case(&case)?;

        // 6. Record audit event
        self.store.record_audit_event(&AuditTrailEvent {
            id: format!("audit_{}", uuid::Uuid::now_v7()),
            case_id: Some(case.id.clone()),
            repo_id: case.repo_id.clone(),
            event_type: "plan_generated".to_string(),
            actor: "m31a-maintenance-planner".to_string(),
            policy_version: self.policy.version.clone(),
            details: serde_json::json!({
                "plan_id": plan.id,
                "strategy": plan.selected_strategy,
                "plan_hash": plan.plan_hash,
                "requires_approval": plan.requires_approval,
            }),
            timestamp_epoch_ms: Utc::now().timestamp_millis(),
        })?;

        info!(
            case_id = %case.id,
            plan_id = %plan.id,
            strategy = %plan.selected_strategy,
            requires_approval = plan.requires_approval,
            "Synthesized evidence-based maintenance plan"
        );

        Ok(plan)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{MaintenanceCategory, SignalSeverity};
    use m31a_storage::{BUILTIN_MIGRATIONS, MigrationEngine};
    use rusqlite::Connection;
    use std::sync::{Arc, Mutex};

    fn setup_test_planner() -> MaintenancePlanner {
        let mut conn = Connection::open_in_memory().unwrap();
        MigrationEngine::apply_all(&mut conn, BUILTIN_MIGRATIONS).unwrap();
        let store = MaintenanceStore::new(Arc::new(Mutex::new(conn)));
        let policy = MaintenancePolicy::default();
        MaintenancePlanner::new(store, policy)
    }

    #[test]
    fn test_planner_generates_and_persists_plan() {
        let planner = setup_test_planner();
        let case = MaintenanceCase::new(
            "repo_1",
            MaintenanceCategory::BrokenCi,
            "Broken build on test commit",
            "could not compile crate foo: error[E0308]: mismatched types",
            "ci_run_777",
            SignalSeverity::High,
        );
        planner.store.create_case(&case, None).unwrap();

        let plan = planner
            .generate_plan(&case.id, Path::new("/workspace"), "commit_sha_123")
            .unwrap();

        assert_eq!(plan.category, MaintenanceCategory::BrokenCi);
        assert_eq!(plan.selected_strategy, "minimal_source_correction");
        assert_eq!(plan.baseline_sha, "commit_sha_123");
        assert!(!plan.verification_requirements.is_empty());

        let fetched_case = planner.store.get_case(&case.id).unwrap().unwrap();
        assert_eq!(
            fetched_case.active_plan_id.as_deref(),
            Some(plan.id.as_str())
        );
        assert_eq!(fetched_case.status, CaseStatus::PlanReady);
    }
}
