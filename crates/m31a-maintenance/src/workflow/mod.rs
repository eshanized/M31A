//! Maintenance Workflow Adapter & Execution Engine (`m31a-maintenance::workflow`).
//!
//! Enforces:
//! 1. All maintenance modifications execute in dedicated isolated Git worktrees.
//! 2. Translates `MaintenancePlan` into DAG waves of `WorkflowTask` nodes.
//! 3. Reuses authoritative `WorkflowOrchestrator` and `WorkflowStore`.
//! 4. Synchronizes workflow execution lifecycle with `MaintenanceExecution` and `MaintenanceCase`.
//! 5. Enforces attempt fencing and replanning budgets.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use anyhow::{Result, anyhow, bail};
use chrono::Utc;
use tracing::info;
use uuid::Uuid;

use m31a_agent_engine::workflow::WorkflowOrchestrator;
use m31a_core::workflow::{
    WorkflowPlan, WorkflowTask, WorkflowTaskKind,
    WorkflowTaskStatus, WorkspaceIsolationStrategy,
};
use m31a_storage::WorkflowStore;

use crate::domain::{
    AuditTrailEvent, CaseStatus, ExecutionLifecycleState, MaintenanceExecution,
    MaintenancePlan, MaintenancePolicy, RemediationAttempt,
};
use crate::storage::MaintenanceStore;

/// Translates a `MaintenancePlan` into authoritative `WorkflowTask` DAG nodes.
pub struct MaintenanceWorkflowAdapter;

impl MaintenanceWorkflowAdapter {
    /// Constructs a valid, multi-stage DAG plan for a maintenance plan.
    pub fn build_workflow_plan(
        workflow_id: &str,
        plan: &MaintenancePlan,
        policy: &MaintenancePolicy,
    ) -> WorkflowPlan {
        let now = Utc::now().timestamp_millis();
        let mut tasks = Vec::new();

        // Wave 0: Analysis Task
        let analysis_id = format!("task-maint-analysis-{}", Uuid::new_v4().simple());
        let analysis_task = WorkflowTask {
            id: analysis_id.clone(),
            workflow_id: workflow_id.to_string(),
            plan_revision: 1,
            task_kind: WorkflowTaskKind::Analysis,
            name: format!("Analyze {}", plan.category.as_str()),
            description: format!(
                "Reconnaissance and evidence collection for case '{}' ({})",
                plan.case_id, plan.problem_classification
            ),
            assigned_role: "maintainer".to_string(),
            assigned_agent_id: None,
            priority: 1,
            wave_index: 0,
            dependencies: Vec::new(),
            acceptance_criteria: vec![
                "Identify affected modules and exact failure root cause".to_string(),
                "Formulate minimal isolated correction scope".to_string(),
            ],
            required_verification: vec!["build_check".to_string()],
            declared_read_paths: plan.expected_files.clone(),
            declared_write_paths: Vec::new(),
            workspace_isolation: WorkspaceIsolationStrategy::ReadOnlyCopy,
            status: WorkflowTaskStatus::Ready,
            retry_count: 0,
            max_retries: policy.max_retries_per_case,
            attempt_id: 1,
            started_at_epoch_ms: None,
            completed_at_epoch_ms: None,
            worker_id: None,
            lease_expires_at_epoch_ms: None,
            error_details: None,
            input_artifacts: Vec::new(),
            output_artifacts: Vec::new(),
            verification_result_json: None,
            execution_result_json: None,
        };
        tasks.push(analysis_task);

        // Wave 1: Implementation Task (always in isolated worktree!)
        let impl_id = format!("task-maint-impl-{}", Uuid::new_v4().simple());
        let impl_task = WorkflowTask {
            id: impl_id.clone(),
            workflow_id: workflow_id.to_string(),
            plan_revision: 1,
            task_kind: WorkflowTaskKind::Implementation,
            name: format!("Execute {}", plan.selected_strategy),
            description: format!(
                "Synthesize minimal change-set for strategy '{}'",
                plan.selected_strategy
            ),
            assigned_role: "implementer".to_string(),
            assigned_agent_id: None,
            priority: 2,
            wave_index: 1,
            dependencies: vec![analysis_id.clone()],
            acceptance_criteria: vec![
                "Apply bounded patch to isolated worktree".to_string(),
                "Preserve existing public API and unaffected behavior".to_string(),
            ],
            required_verification: vec!["build_check".to_string()],
            declared_read_paths: plan.expected_files.clone(),
            declared_write_paths: if plan.expected_files.is_empty() {
                vec![PathBuf::from("src/")]
            } else {
                plan.expected_files.clone()
            },
            workspace_isolation: WorkspaceIsolationStrategy::Worktree,
            status: WorkflowTaskStatus::Pending,
            retry_count: 0,
            max_retries: policy.max_retries_per_case,
            attempt_id: 1,
            started_at_epoch_ms: None,
            completed_at_epoch_ms: None,
            worker_id: None,
            lease_expires_at_epoch_ms: None,
            error_details: None,
            input_artifacts: vec![analysis_id.clone()],
            output_artifacts: Vec::new(),
            verification_result_json: None,
            execution_result_json: None,
        };
        tasks.push(impl_task);

        // Wave 2: Independent Verification Task (Initiative 2 integration)
        let verif_id = format!("task-maint-verif-{}", Uuid::new_v4().simple());
        let mut verif_criteria: Vec<String> = plan
            .verification_requirements
            .iter()
            .map(|r| r.description.clone())
            .collect();
        if verif_criteria.is_empty() {
            verif_criteria.push("Pass compiler check and test suite without regressions".to_string());
        }

        let verif_task = WorkflowTask {
            id: verif_id.clone(),
            workflow_id: workflow_id.to_string(),
            plan_revision: 1,
            task_kind: WorkflowTaskKind::Verification,
            name: "Independent Verification".to_string(),
            description: "Execute independent verification plan against frozen candidate change-set".to_string(),
            assigned_role: "verifier".to_string(),
            assigned_agent_id: None,
            priority: 3,
            wave_index: 2,
            dependencies: vec![impl_id.clone()],
            acceptance_criteria: verif_criteria,
            required_verification: vec![
                "build_check".to_string(),
                "unit_test".to_string(),
                "safety_audit".to_string(),
            ],
            declared_read_paths: plan.expected_files.clone(),
            declared_write_paths: Vec::new(),
            workspace_isolation: WorkspaceIsolationStrategy::ReadOnlyCopy,
            status: WorkflowTaskStatus::Pending,
            retry_count: 0,
            max_retries: 1,
            attempt_id: 1,
            started_at_epoch_ms: None,
            completed_at_epoch_ms: None,
            worker_id: None,
            lease_expires_at_epoch_ms: None,
            error_details: None,
            input_artifacts: vec![impl_id.clone()],
            output_artifacts: Vec::new(),
            verification_result_json: None,
            execution_result_json: None,
        };
        tasks.push(verif_task);

        // Wave 3: Delivery Task
        let delivery_id = format!("task-maint-delivery-{}", Uuid::new_v4().simple());
        let delivery_task = WorkflowTask {
            id: delivery_id,
            workflow_id: workflow_id.to_string(),
            plan_revision: 1,
            task_kind: WorkflowTaskKind::Delivery,
            name: "Deliver Maintenance Package".to_string(),
            description: "Prepare reviewable draft pull request or final verification report".to_string(),
            assigned_role: "deliverer".to_string(),
            assigned_agent_id: None,
            priority: 4,
            wave_index: 3,
            dependencies: vec![verif_id.clone()],
            acceptance_criteria: vec![
                "Consolidate verified change-set and verification evidence".to_string(),
                "Create reviewable pull request or deliverable artifact".to_string(),
            ],
            required_verification: Vec::new(),
            declared_read_paths: Vec::new(),
            declared_write_paths: Vec::new(),
            workspace_isolation: WorkspaceIsolationStrategy::ReadOnlyCopy,
            status: WorkflowTaskStatus::Pending,
            retry_count: 0,
            max_retries: 1,
            attempt_id: 1,
            started_at_epoch_ms: None,
            completed_at_epoch_ms: None,
            worker_id: None,
            lease_expires_at_epoch_ms: None,
            error_details: None,
            input_artifacts: vec![verif_id],
            output_artifacts: Vec::new(),
            verification_result_json: None,
            execution_result_json: None,
        };
        tasks.push(delivery_task);

        WorkflowPlan {
            workflow_id: workflow_id.to_string(),
            revision: 1,
            rationale: format!(
                "Maintenance execution for case '{}' via strategy '{}'",
                plan.case_id, plan.selected_strategy
            ),
            created_at_epoch_ms: now,
            tasks,
        }
    }
}

/// Executor connecting maintenance engine to authoritative `WorkflowOrchestrator`.
#[derive(Clone)]
pub struct MaintenanceWorkflowRunner {
    maint_store: MaintenanceStore,
    workflow_store: WorkflowStore,
    orchestrator: Arc<WorkflowOrchestrator>,
    policy: MaintenancePolicy,
}

impl MaintenanceWorkflowRunner {
    pub fn new(
        maint_store: MaintenanceStore,
        workflow_store: WorkflowStore,
        orchestrator: Arc<WorkflowOrchestrator>,
        policy: MaintenancePolicy,
    ) -> Self {
        Self {
            maint_store,
            workflow_store,
            orchestrator,
            policy,
        }
    }

    /// Launches an authorized maintenance plan into execution.
    pub async fn start_execution(
        &self,
        case_id: &str,
        plan_id: &str,
        workspace_root: &Path,
    ) -> Result<MaintenanceExecution> {
        let mut case = self
            .maint_store
            .get_case(case_id)?
            .ok_or_else(|| anyhow!("Case '{}' not found", case_id))?;

        let plan = self
            .maint_store
            .get_plan(plan_id)?
            .ok_or_else(|| anyhow!("Plan '{}' not found", plan_id))?;

        // 1. Enforce approval gating
        if plan.requires_approval && plan.approval_state != crate::domain::PlanApprovalState::Approved {
            bail!(
                "Maintenance plan '{}' requires explicit human approval before execution",
                plan.id
            );
        }

        // 2. Create authoritative workflow run in WorkflowStore
        let wf_run = self.orchestrator.create_workflow(
            workspace_root,
            &format!("Execute maintenance for case '{}'", case_id),
            "maintenance",
            None,
            None,
        )?;

        // 3. Translate plan to tasks and persist
        let wf_plan =
            MaintenanceWorkflowAdapter::build_workflow_plan(&wf_run.id, &plan, &self.policy);
        self.workflow_store.save_workflow_plan(&wf_plan)?;

        // 4. Create and persist MaintenanceExecution record
        let now = Utc::now().timestamp_millis();
        let exec = MaintenanceExecution {
            id: format!("exec_{}", Uuid::now_v7()),
            case_id: case.id.clone(),
            workflow_id: wf_run.id.clone(),
            plan_id: plan.id.clone(),
            manifest_hash: None,
            attempt_id: 1,
            worker_id: "m31a-maintenance-worker-1".to_string(),
            execution_state: ExecutionLifecycleState::Created,
            verification_report_hash: None,
            promotion_state: "none".to_string(),
            pr_number: None,
            pr_url: None,
            failure_classification: None,
            duration_ms: 0,
            tokens_used: 0,
            created_at_epoch_ms: now,
            completed_at_epoch_ms: None,
        };
        self.maint_store.record_execution(&exec)?;

        // 5. Update case state
        case.workflow_id = Some(wf_run.id.clone());
        case.transition_to(CaseStatus::Executing)?;
        self.maint_store.update_case(&case)?;

        // 6. Record audit event
        self.maint_store.record_audit_event(&AuditTrailEvent {
            id: format!("audit_{}", Uuid::now_v7()),
            case_id: Some(case.id.clone()),
            repo_id: case.repo_id.clone(),
            event_type: "execution_started".to_string(),
            actor: "m31a-maintenance-runner".to_string(),
            policy_version: self.policy.version.clone(),
            details: serde_json::json!({
                "execution_id": exec.id,
                "workflow_id": wf_run.id,
                "plan_id": plan.id,
            }),
            timestamp_epoch_ms: now,
        })?;

        info!(
            case_id = %case.id,
            workflow_id = %wf_run.id,
            execution_id = %exec.id,
            "Started maintenance workflow execution"
        );

        Ok(exec)
    }

    /// Reconciles execution outcome upon workflow completion or interruption.
    pub fn reconcile_execution(
        &self,
        exec_id: &str,
        success: bool,
        manifest_hash: Option<String>,
        pr_url: Option<String>,
        summary: &str,
    ) -> Result<()> {
        let mut exec = self
            .maint_store
            .get_execution(exec_id)?
            .ok_or_else(|| anyhow!("Execution '{}' not found", exec_id))?;

        let mut case = self
            .maint_store
            .get_case(&exec.case_id)?
            .ok_or_else(|| anyhow!("Case '{}' not found", exec.case_id))?;

        let now = Utc::now().timestamp_millis();
        exec.completed_at_epoch_ms = Some(now);
        exec.duration_ms = (now - exec.created_at_epoch_ms).max(0) as u64;
        exec.manifest_hash = manifest_hash.clone();
        exec.pr_url = pr_url;

        let attempt_record = RemediationAttempt {
            attempt_id: exec.attempt_id,
            timestamp_epoch_ms: now,
            workflow_id: exec.workflow_id.clone(),
            plan_id: exec.plan_id.clone(),
            strategy: "autonomous_remediation".to_string(),
            success,
            summary: summary.to_string(),
            manifest_hash,
        };

        case.remediation_history.push(attempt_record);

        if success {
            exec.execution_state = ExecutionLifecycleState::Completed;
            if case.status != CaseStatus::Resolved {
                let _ = case.transition_to(CaseStatus::Verifying);
                let _ = case.transition_to(CaseStatus::Resolved);
            }
        } else {
            exec.execution_state = ExecutionLifecycleState::Failed;
            let _ = case.transition_to(CaseStatus::Failed);
        }

        self.maint_store.record_execution(&exec)?;
        self.maint_store.update_case(&case)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;
    use std::sync::Mutex;
    use m31a_storage::{BUILTIN_MIGRATIONS, MigrationEngine};
    use crate::domain::{MaintenanceCategory, MaintenancePlan, MaintenanceRiskLevel, AutonomyTier};

    fn setup_test_adapter_env() -> (MaintenanceStore, WorkflowStore) {
        let mut conn = Connection::open_in_memory().unwrap();
        MigrationEngine::apply_all(&mut conn, BUILTIN_MIGRATIONS).unwrap();
        let arc_conn = Arc::new(Mutex::new(conn));
        let m_store = MaintenanceStore::new(Arc::clone(&arc_conn));
        let w_store = WorkflowStore::new(arc_conn);
        (m_store, w_store)
    }

    #[test]
    fn test_adapter_generates_valid_workflow_dag() {
        let (_m_store, _w_store) = setup_test_adapter_env();
        let plan = MaintenancePlan::new(
            "case_001",
            "repo_test",
            "commit_111",
            MaintenanceCategory::BrokenCi,
            "source_compilation_failure",
            "minimal_source_correction",
            MaintenanceRiskLevel::Low,
            AutonomyTier::Tier2GuardedRemediation,
            false,
        );

        let policy = MaintenancePolicy::default();
        let wf_plan = MaintenanceWorkflowAdapter::build_workflow_plan("wf_test_1", &plan, &policy);

        assert_eq!(wf_plan.tasks.len(), 4);
        assert_eq!(wf_plan.tasks[0].task_kind, WorkflowTaskKind::Analysis);
        assert_eq!(wf_plan.tasks[1].task_kind, WorkflowTaskKind::Implementation);
        assert_eq!(
            wf_plan.tasks[1].workspace_isolation,
            WorkspaceIsolationStrategy::Worktree
        );
        assert_eq!(wf_plan.tasks[2].task_kind, WorkflowTaskKind::Verification);
        assert_eq!(wf_plan.tasks[3].task_kind, WorkflowTaskKind::Delivery);
        assert!(wf_plan.validate().is_ok());
    }
}
