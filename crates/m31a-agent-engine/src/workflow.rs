//! Production Autonomous Workflow Orchestrator (`m31a-agent-engine::workflow`).
//!
//! Orchestrates end-to-end autonomous engineering workflows:
//! Objective → Context Analysis → Validated Task DAG → Wave Scheduling →
//! Concurrent Child Agents in Worktrees → Speculative Branches where useful →
//! Verification Gates → Safe Integration → Durable State & Recovery.

use anyhow::{Context, Result, anyhow, bail};
use chrono::Utc;
use futures::future::BoxFuture;
use m31a_core::workflow::{
    VerificationStatus, WorkflowEvent, WorkflowEventType, WorkflowPlan, WorkflowRun, WorkflowState,
    WorkflowTask, WorkflowTaskStatus, WorkspaceIsolationStrategy,
};
use m31a_execpolicy::ExecPolicy;
use m31a_provider::ProviderClient;
use m31a_storage::worktree::WorktreeManager;
use m31a_storage::{Storage, WorkflowStore};
use m31a_tools::agent_runtime::AgentRuntime;
use m31a_tools::registry::ToolRegistry;
use m31a_verify::AutonomousVerifier;
use m31a_verify::runners::ProjectRunner;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::agents::EngineAgentRuntime;
use crate::integration::{IntegrationRequest, WorkflowIntegrationService};
use crate::speculative::{SpeculateOptions, SpeculativeArena};
use crate::waves::{WaveScheduler, WorkflowTaskExecutionOutcome};

/// Maximum automatic replanning revisions allowed per workflow before failing.
pub const MAX_WORKFLOW_REPLAN_REVISIONS: u32 = 3;

/// Configuration options for workflow execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowExecutionConfig {
    pub execution_mode: m31a_core::workflow::ExecutionMode,
    pub max_concurrency: usize,
    pub task_timeout_seconds: u64,
    pub verify_post_wave: bool,
    pub allow_speculative_stages: bool,
    pub require_approval_for_integration: bool,
    pub max_replan_attempts: u32,
}

impl Default for WorkflowExecutionConfig {
    fn default() -> Self {
        Self {
            execution_mode: m31a_core::workflow::ExecutionMode::Standard,
            max_concurrency: 4,
            task_timeout_seconds: 120,
            verify_post_wave: true,
            allow_speculative_stages: true,
            require_approval_for_integration: false,
            max_replan_attempts: MAX_WORKFLOW_REPLAN_REVISIONS,
        }
    }
}

/// Periodic heartbeat guard maintaining worker lease in durable storage.
struct TaskHeartbeatGuard {
    cancel_token: CancellationToken,
    handle: tokio::task::JoinHandle<()>,
}

impl TaskHeartbeatGuard {
    fn spawn(
        store: WorkflowStore,
        workflow_id: String,
        task_id: String,
        worker_id: String,
        attempt_id: u32,
        lease_duration_ms: i64,
        task_cancel_token: CancellationToken,
    ) -> Self {
        let heartbeat_cancel = CancellationToken::new();
        let cancel_for_task = heartbeat_cancel.clone();
        let interval_ms = (lease_duration_ms / 3).max(1000) as u64;

        let handle = tokio::spawn(async move {
            let mut ticker = tokio::time::interval(Duration::from_millis(interval_ms));
            ticker.tick().await; // first tick is immediate, skip
            loop {
                tokio::select! {
                    _ = cancel_for_task.cancelled() => break,
                    _ = ticker.tick() => {
                        match store.renew_task_lease(&workflow_id, &task_id, &worker_id, attempt_id, lease_duration_ms) {
                            Ok(true) => {}
                            Ok(false) => {
                                warn!(
                                    workflow_id = %workflow_id,
                                    task_id = %task_id,
                                    worker_id = %worker_id,
                                    attempt_id,
                                    "Task lease renewal rejected (fenced or expired); cancelling task"
                                );
                                task_cancel_token.cancel();
                                break;
                            }
                            Err(e) => {
                                warn!(
                                    err = %e,
                                    workflow_id = %workflow_id,
                                    task_id = %task_id,
                                    "Task lease renewal failed; cancelling task"
                                );
                                task_cancel_token.cancel();
                                break;
                            }
                        }
                    }
                }
            }
        });

        Self {
            cancel_token: heartbeat_cancel,
            handle,
        }
    }

    fn stop(self) {
        self.cancel_token.cancel();
        self.handle.abort();
    }
}

/// Central autonomous workflow orchestrator.
#[derive(Clone)]
pub struct WorkflowOrchestrator {
    storage: Storage,
    runtime: Option<Arc<EngineAgentRuntime>>,
    base_client: Option<ProviderClient>,
    tools: Option<ToolRegistry>,
    exec_policy: Option<ExecPolicy>,
    worktree_mgr: Option<Arc<WorktreeManager>>,
    active_tokens: Arc<std::sync::Mutex<HashMap<String, CancellationToken>>>,
    active_agent_ids: Arc<std::sync::Mutex<HashMap<String, Vec<String>>>>,
}

impl WorkflowOrchestrator {
    pub fn new(
        storage: Storage,
        runtime: Option<Arc<EngineAgentRuntime>>,
        base_client: Option<ProviderClient>,
        tools: Option<ToolRegistry>,
        exec_policy: Option<ExecPolicy>,
    ) -> Self {
        Self {
            storage,
            runtime,
            base_client,
            tools,
            exec_policy,
            worktree_mgr: None,
            active_tokens: Arc::new(std::sync::Mutex::new(HashMap::new())),
            active_agent_ids: Arc::new(std::sync::Mutex::new(HashMap::new())),
        }
    }

    pub fn with_worktree_manager(mut self, mgr: Arc<WorktreeManager>) -> Self {
        self.worktree_mgr = Some(mgr);
        self
    }

    pub fn runtime(&self) -> Option<&Arc<EngineAgentRuntime>> {
        self.runtime.as_ref()
    }

    pub fn base_client(&self) -> Option<&ProviderClient> {
        self.base_client.as_ref()
    }

    pub fn tools(&self) -> Option<&ToolRegistry> {
        self.tools.as_ref()
    }

    pub fn exec_policy(&self) -> Option<&ExecPolicy> {
        self.exec_policy.as_ref()
    }

    pub fn store(&self) -> WorkflowStore {
        self.storage.workflow_store()
    }

    /// Registers an active child agent ID with a workflow run for cancellation tracking.
    pub fn register_active_agent(&self, workflow_id: &str, agent_id: &str) {
        if let Ok(mut guard) = self.active_agent_ids.lock() {
            guard
                .entry(workflow_id.to_string())
                .or_default()
                .push(agent_id.to_string());
        }
    }

    /// Creates and persists a new workflow run.
    pub fn create_workflow(
        &self,
        workspace_root: &Path,
        objective: &str,
        workflow_type: &str,
        thread_id: Option<&str>,
        config: Option<WorkflowExecutionConfig>,
    ) -> Result<WorkflowRun> {
        let wf_id = format!("wf-{}", Uuid::new_v4().simple());
        let now = Utc::now().timestamp_millis();
        let cfg = config.unwrap_or_default();
        let cfg_json = serde_json::to_string(&cfg)?;

        let run = WorkflowRun {
            id: wf_id.clone(),
            thread_id: thread_id.map(String::from),
            workspace_root: workspace_root.to_path_buf(),
            objective: objective.to_string(),
            workflow_type: workflow_type.to_string(),
            state: WorkflowState::Created,
            plan_revision: 1,
            configuration_json: cfg_json,
            created_at_epoch_ms: now,
            updated_at_epoch_ms: now,
            started_at_epoch_ms: None,
            completed_at_epoch_ms: None,
            cancellation_reason: None,
            overall_result_json: None,
            resource_usage_json: None,
        };

        self.store().create_workflow(&run)?;

        // Record initial event
        self.store().record_event(&WorkflowEvent {
            id: format!("evt-{}", Uuid::new_v4().simple()),
            workflow_id: wf_id,
            task_id: None,
            agent_id: None,
            event_type: WorkflowEventType::WorkflowCreated,
            timestamp_epoch_ms: now,
            correlation_id: None,
            payload_json: serde_json::json!({
                "objective": objective,
                "workspace_root": workspace_root.display().to_string(),
            })
            .to_string(),
            schema_version: 1,
        })?;

        Ok(run)
    }

    /// Generates a validated task DAG plan for a workflow based on workspace analysis and objective requirements.
    pub async fn generate_plan(&self, workflow_id: &str) -> Result<WorkflowPlan> {
        self.generate_plan_revision(workflow_id, None).await
    }

    /// Generates a plan with an optional explicit revision number.
    pub async fn generate_plan_revision(
        &self,
        workflow_id: &str,
        target_revision: Option<u32>,
    ) -> Result<WorkflowPlan> {
        let run = self
            .store()
            .get_workflow(workflow_id)?
            .ok_or_else(|| anyhow!("Workflow '{workflow_id}' not found"))?;

        let plan_rev = target_revision.unwrap_or(run.plan_revision);

        self.store()
            .update_workflow_state(workflow_id, WorkflowState::Planning)?;

        let now = Utc::now().timestamp_millis();
        let project_kind = ProjectRunner::detect(&run.workspace_root);
        let objective = &run.objective;

        let cfg: WorkflowExecutionConfig =
            serde_json::from_str(&run.configuration_json).unwrap_or_default();

        // Determine if speculative execution is valuable for this objective
        let obj_lower = objective.to_lowercase();
        let is_speculation_valuable = cfg.allow_speculative_stages
            && (obj_lower.contains("optimi")
                || obj_lower.contains("perf")
                || obj_lower.contains("speed")
                || obj_lower.contains("refactor")
                || obj_lower.contains("architecture")
                || obj_lower.contains("competing")
                || obj_lower.contains("speculative"));

        let mut tasks = Vec::new();

        // 1. Task 1: Repository & Architectural Analysis (Wave 1)
        tasks.push(WorkflowTask {
            id: format!("{}-t1-analysis", workflow_id),
            workflow_id: workflow_id.to_string(),
            plan_revision: plan_rev,
            task_kind: m31a_core::workflow::WorkflowTaskKind::Analysis,
            name: "Repository & Architectural Analysis".to_string(),
            description: format!(
                "Analyze existing codebase structure, dependencies, interfaces, and baseline verification for project kind {:?}",
                project_kind
            ),
            assigned_role: "Repository Analyst".to_string(),
            assigned_agent_id: None,
            priority: 10,
            wave_index: 1,
            dependencies: vec![],
            acceptance_criteria: vec![
                "Identify modified modules and public interfaces".into(),
                "Establish baseline verification status".into(),
            ],
            required_verification: vec!["cargo check".into()],
            declared_read_paths: vec![PathBuf::from(".")],
            declared_write_paths: vec![],
            workspace_isolation: WorkspaceIsolationStrategy::ReadOnlyCopy,
            status: WorkflowTaskStatus::Ready,
            retry_count: 0,
            max_retries: 3,
            attempt_id: 1,
            started_at_epoch_ms: None,
            completed_at_epoch_ms: None,
            worker_id: None,
            lease_expires_at_epoch_ms: None,
            error_details: None,
            input_artifacts: vec![],
            output_artifacts: vec!["analysis_report.json".into()],
            verification_result_json: None,
            execution_result_json: None,
        });

        // 2. Implementation stage: Either Speculative Parallel Branches or Direct Implementation
        if is_speculation_valuable {
            // Task 2: Speculative Parallel Arena
            tasks.push(WorkflowTask {
                id: format!("{}-t2-speculate", workflow_id),
                workflow_id: workflow_id.to_string(),
                plan_revision: plan_rev,
                task_kind: m31a_core::workflow::WorkflowTaskKind::Speculative,
                name: "Speculative Solution Arena".to_string(),
                description: format!(
                    "Generate and evaluate competing parallel candidate implementations for objective: '{}'",
                    objective
                ),
                assigned_role: "Architect".to_string(),
                assigned_agent_id: None,
                priority: 20,
                wave_index: 2,
                dependencies: vec![format!("{}-t1-analysis", workflow_id)],
                acceptance_criteria: vec![
                    "Produce at least two distinct viable candidate hypotheses".into(),
                    "Evaluate candidates with strict verification hard gates".into(),
                    "Select winning candidate with reviewable patch".into(),
                ],
                required_verification: vec!["verify_check".into(), "verify_tests".into()],
                declared_read_paths: vec![PathBuf::from(".")],
                declared_write_paths: vec![PathBuf::from("src")],
                workspace_isolation: WorkspaceIsolationStrategy::Worktree,
                status: WorkflowTaskStatus::Pending,
                retry_count: 0,
                max_retries: 2,
                attempt_id: 1,
                started_at_epoch_ms: None,
                completed_at_epoch_ms: None,
                worker_id: None,
                lease_expires_at_epoch_ms: None,
                error_details: None,
                input_artifacts: vec!["analysis_report.json".into()],
                output_artifacts: vec!["speculative_report.json".into(), "winning_patch.diff".into()],
                verification_result_json: None,
                execution_result_json: None,
            });

            // Task 3: Solution Integration & Verification
            tasks.push(WorkflowTask {
                id: format!("{}-t3-integrate", workflow_id),
                workflow_id: workflow_id.to_string(),
                plan_revision: plan_rev,
                task_kind: m31a_core::workflow::WorkflowTaskKind::Integration,
                name: "Winning Solution Integration".to_string(),
                description: "Integrate verified winning speculative candidate into primary workspace with conflict checks".to_string(),
                assigned_role: "Integrator".to_string(),
                assigned_agent_id: None,
                priority: 30,
                wave_index: 3,
                dependencies: vec![format!("{}-t2-speculate", workflow_id)],
                acceptance_criteria: vec![
                    "Apply winning diff cleanly onto primary working tree".into(),
                    "Verify compiler checks pass post-integration".into(),
                ],
                required_verification: vec!["verify_check".into()],
                declared_read_paths: vec![PathBuf::from(".")],
                declared_write_paths: vec![PathBuf::from("src")],
                workspace_isolation: WorkspaceIsolationStrategy::SharedWorkspace,
                status: WorkflowTaskStatus::Pending,
                retry_count: 0,
                max_retries: 2,
                attempt_id: 1,
                started_at_epoch_ms: None,
                completed_at_epoch_ms: None,
                worker_id: None,
                lease_expires_at_epoch_ms: None,
                error_details: None,
                input_artifacts: vec!["winning_patch.diff".into()],
                output_artifacts: vec!["integration_report.json".into()],
                verification_result_json: None,
                execution_result_json: None,
            });
        } else {
            // Standard direct implementation task
            tasks.push(WorkflowTask {
                id: format!("{}-t2-implement", workflow_id),
                workflow_id: workflow_id.to_string(),
                plan_revision: plan_rev,
                task_kind: m31a_core::workflow::WorkflowTaskKind::Implementation,
                name: "Core Implementation".to_string(),
                description: format!("Implement required changes for objective: '{}'", objective),
                assigned_role: "Implementer".to_string(),
                assigned_agent_id: None,
                priority: 20,
                wave_index: 2,
                dependencies: vec![format!("{}-t1-analysis", workflow_id)],
                acceptance_criteria: vec![
                    "Modify source files to satisfy acceptance criteria".into(),
                    "Ensure build compiles without warnings".into(),
                ],
                required_verification: vec!["verify_check".into()],
                declared_read_paths: vec![PathBuf::from(".")],
                declared_write_paths: vec![PathBuf::from("src")],
                workspace_isolation: WorkspaceIsolationStrategy::Worktree,
                status: WorkflowTaskStatus::Pending,
                retry_count: 0,
                max_retries: 3,
                attempt_id: 1,
                started_at_epoch_ms: None,
                completed_at_epoch_ms: None,
                worker_id: None,
                lease_expires_at_epoch_ms: None,
                error_details: None,
                input_artifacts: vec!["analysis_report.json".into()],
                output_artifacts: vec!["implementation_patch.diff".into()],
                verification_result_json: None,
                execution_result_json: None,
            });
        }

        // Final Task: Verification, Security Audit & Delivery
        let last_dep = if is_speculation_valuable {
            format!("{}-t3-integrate", workflow_id)
        } else {
            format!("{}-t2-implement", workflow_id)
        };

        tasks.push(WorkflowTask {
            id: format!("{}-t4-final-verification", workflow_id),
            workflow_id: workflow_id.to_string(),
            plan_revision: plan_rev,
            task_kind: m31a_core::workflow::WorkflowTaskKind::Verification,
            name: "Final Verification & Security Audit".to_string(),
            description: "Execute test suite, audit semantic blast radius, and generate final delivery report".to_string(),
            assigned_role: "Test Engineer".to_string(),
            assigned_agent_id: None,
            priority: 40,
            wave_index: if is_speculation_valuable { 4 } else { 3 },
            dependencies: vec![last_dep],
            acceptance_criteria: vec![
                "Pass all regression and unit tests".into(),
                "Pass codebase safety audit and blast-radius gate".into(),
                "Generate structured delivery summary".into(),
            ],
            required_verification: vec!["verify_tests".into(), "verify_safety".into()],
            declared_read_paths: vec![PathBuf::from(".")],
            declared_write_paths: vec![],
            workspace_isolation: WorkspaceIsolationStrategy::SharedWorkspace,
            status: WorkflowTaskStatus::Pending,
            retry_count: 0,
            max_retries: 2,
            attempt_id: 1,
            started_at_epoch_ms: None,
            completed_at_epoch_ms: None,
            worker_id: None,
            lease_expires_at_epoch_ms: None,
            error_details: None,
            input_artifacts: vec![],
            output_artifacts: vec!["delivery_report.json".into()],
            verification_result_json: None,
            execution_result_json: None,
        });

        let rationale = if is_speculation_valuable {
            "Decomposed into 4-stage pipeline with Speculative Parallel Reality execution for architectural optimization"
        } else {
            "Decomposed into 3-stage targeted linear pipeline with isolated worktree implementation"
        };

        let plan = WorkflowPlan {
            workflow_id: workflow_id.to_string(),
            revision: plan_rev,
            rationale: rationale.to_string(),
            created_at_epoch_ms: now,
            tasks,
        };

        // Validate DAG integrity before persisting
        plan.validate()
            .context("Generated WorkflowPlan failed DAG validation")?;

        self.store().save_workflow_plan(&plan)?;
        self.store()
            .update_workflow_state(workflow_id, WorkflowState::PlanReady)?;

        self.store().record_event(&WorkflowEvent {
            id: format!("evt-{}", Uuid::new_v4().simple()),
            workflow_id: workflow_id.to_string(),
            task_id: None,
            agent_id: None,
            event_type: WorkflowEventType::PlanGenerated,
            timestamp_epoch_ms: now,
            correlation_id: None,
            payload_json: serde_json::json!({
                "revision": plan.revision,
                "tasks_count": plan.tasks.len(),
                "speculative": is_speculation_valuable,
                "rationale": rationale,
            })
            .to_string(),
            schema_version: 1,
        })?;

        Ok(plan)
    }

    /// Handles a structured conflict reported by an executing agent/task.
    /// In accordance with demand-driven coordination:
    /// - Genuine dependencies are resolved by dynamically updating the task DAG (blocks/depends_on).
    /// - Integration conflicts defer to sequential verified integration.
    /// - Cycles in suggested dependencies are prevented and rejected.
    /// - Human intervention is requested when automatic DAG replanning cannot resolve the conflict safely.
    pub fn handle_task_conflict(
        &self,
        report: &m31a_core::workflow::ConflictReport,
    ) -> Result<m31a_core::workflow::ConflictResolution> {
        use m31a_core::workflow::{ConflictKind, ConflictResolution};

        // Record the incoming conflict report in durable storage
        self.store().record_conflict(report)?;

        let resolution = match report.conflict_kind {
            ConflictKind::GenuineDependency => {
                if let Some(ref blocker_id) = report.suggested_dependency {
                    match self.store().replan_with_dependency(
                        &report.workflow_id,
                        &report.reporting_task_id,
                        blocker_id,
                        &report.description,
                    ) {
                        Ok(_) => {
                            info!(
                                workflow_id = %report.workflow_id,
                                task_id = %report.reporting_task_id,
                                blocker_id = %blocker_id,
                                "Dynamic replanning successfully added task dependency; yielding task"
                            );
                            ConflictResolution::AddDependency {
                                task_id: report.reporting_task_id.clone(),
                                depends_on: blocker_id.clone(),
                            }
                        }
                        Err(e) => {
                            warn!(
                                workflow_id = %report.workflow_id,
                                err = %e,
                                "Replanning rejected (e.g. cycle detected); escalating to human intervention"
                            );
                            ConflictResolution::EscalateToHuman {
                                task_id: report.reporting_task_id.clone(),
                                reason: format!("Cyclic or invalid dependency replanning error: {e}"),
                            }
                        }
                    }
                } else {
                    ConflictResolution::EscalateToHuman {
                        task_id: report.reporting_task_id.clone(),
                        reason: "Genuine dependency reported without suggested blocker task".into(),
                    }
                }
            }
            ConflictKind::IntegrationConflict => {
                info!(
                    workflow_id = %report.workflow_id,
                    task_id = %report.reporting_task_id,
                    "Conflict classified as integration overlap; deferring to sequential verified integration"
                );
                ConflictResolution::DeferToIntegration {
                    task_id: report.reporting_task_id.clone(),
                    resource_path: report.resource_path.clone(),
                }
            }
            ConflictKind::SharedResourceContention => {
                info!(
                    workflow_id = %report.workflow_id,
                    task_id = %report.reporting_task_id,
                    "Shared resource contention reported; managing via coordinated resource locks"
                );
                ConflictResolution::DeferToIntegration {
                    task_id: report.reporting_task_id.clone(),
                    resource_path: report.resource_path.clone(),
                }
            }
            ConflictKind::RequiresHumanIntervention => {
                warn!(
                    workflow_id = %report.workflow_id,
                    task_id = %report.reporting_task_id,
                    "Conflict requires explicit human intervention"
                );
                ConflictResolution::EscalateToHuman {
                    task_id: report.reporting_task_id.clone(),
                    reason: report.description.clone(),
                }
            }
        };

        self.store().resolve_conflict(&report.id, &resolution)?;
        Ok(resolution)
    }

    /// Generates a new plan revision for a failed or partially completed workflow if within budget.
    pub async fn replan_workflow(
        &self,
        workflow_id: &str,
        failure_reason: &str,
    ) -> Result<WorkflowPlan> {
        let run = self
            .store()
            .get_workflow(workflow_id)?
            .ok_or_else(|| anyhow!("Workflow '{workflow_id}' not found"))?;

        let config: WorkflowExecutionConfig =
            serde_json::from_str(&run.configuration_json).unwrap_or_default();

        if run.plan_revision >= config.max_replan_attempts {
            bail!(
                "Cannot replan workflow '{}': reached maximum plan revisions ({} >= {})",
                workflow_id,
                run.plan_revision,
                config.max_replan_attempts
            );
        }

        let new_revision = run.plan_revision + 1;
        self.store()
            .update_workflow_state(workflow_id, WorkflowState::Planning)?;

        let mut plan = self
            .generate_plan_revision(workflow_id, Some(new_revision))
            .await?;
        plan.rationale = format!(
            "Replanning revision {} due to: {}",
            new_revision, failure_reason
        );
        self.store().save_workflow_plan(&plan)?;

        self.store().record_event(&WorkflowEvent {
            id: format!("evt-replan-{}", Uuid::new_v4().simple()),
            workflow_id: workflow_id.to_string(),
            task_id: None,
            agent_id: None,
            event_type: WorkflowEventType::PlanGenerated,
            timestamp_epoch_ms: Utc::now().timestamp_millis(),
            correlation_id: None,
            payload_json: serde_json::json!({
                "revision": new_revision,
                "replan_reason": failure_reason,
                "tasks_count": plan.tasks.len(),
            })
            .to_string(),
            schema_version: 1,
        })?;

        Ok(plan)
    }

    /// Executes a workflow through planning, wave scheduling, child agents, verification, and integration.
    pub async fn execute_workflow(&self, workflow_id: &str) -> Result<WorkflowRun> {
        let mut run = self
            .store()
            .get_workflow(workflow_id)?
            .ok_or_else(|| anyhow!("Workflow '{workflow_id}' not found"))?;

        if run.state.is_terminal() {
            return Ok(run);
        }

        // Register cancellation token for this workflow
        let wf_cancel_token = CancellationToken::new();
        self.active_tokens
            .lock()
            .unwrap()
            .insert(workflow_id.to_string(), wf_cancel_token.clone());

        // Parse workflow configuration
        let config: WorkflowExecutionConfig =
            serde_json::from_str(&run.configuration_json).unwrap_or_default();

        // If plan not yet generated, generate it now
        let plan = if let Some(p) = self.store().get_workflow_plan(workflow_id, None)? {
            p
        } else {
            self.generate_plan(workflow_id).await?
        };

        // Transition to Running
        self.store()
            .update_workflow_state(workflow_id, WorkflowState::Running)?;
        run.state = WorkflowState::Running;

        let waves = plan
            .partition_into_waves()
            .context("Failed partitioning workflow plan into waves")?;

        let repo_root = run.workspace_root.clone();
        let orchestrator = self.clone();

        // Concurrency semaphore
        let semaphore = Arc::new(tokio::sync::Semaphore::new(
            config.max_concurrency.clamp(1, 16),
        ));

        // Executor closure dispatched by WaveScheduler
        let executor_fn = Arc::new(
            move |task: WorkflowTask| -> BoxFuture<'static, Result<WorkflowTaskExecutionOutcome>> {
                let orch = orchestrator.clone();
                let sem = semaphore.clone();
                Box::pin(async move {
                    let _permit = sem
                        .acquire()
                        .await
                        .map_err(|e| anyhow!("Failed acquiring concurrency permit: {e}"))?;
                    orch.execute_task(task).await
                })
            },
        );

        // Dispatch waves via WaveScheduler
        let wave_result = WaveScheduler::execute_workflow_waves(
            &repo_root,
            waves,
            executor_fn,
            config.verify_post_wave,
        )
        .await?;

        let now = Utc::now().timestamp_millis();
        if wave_result.all_passed {
            // 12-POINT COMPLETION INTEGRITY GATE
            let mut gate_passed = true;
            let mut gate_failure_reason = None;

            // Point 1: Workflow cancellation token check
            if wf_cancel_token.is_cancelled() {
                gate_passed = false;
                gate_failure_reason =
                    Some("Workflow was cancelled prior to final completion gate".to_string());
            }

            // Point 2: Storage state check
            if gate_passed {
                let live_state = self.store().get_workflow(workflow_id)?.map(|w| w.state);
                if matches!(
                    live_state,
                    Some(WorkflowState::Cancelled)
                        | Some(WorkflowState::Failed)
                        | Some(WorkflowState::AwaitingApproval)
                ) {
                    gate_passed = false;
                    gate_failure_reason = Some(format!(
                        "Workflow entered non-runnable state: {:?}",
                        live_state
                    ));
                }
            }

            // Point 3, 4, 5, 6, 7: Tasks completion, count, fencing, and execution results
            if gate_passed {
                let all_tasks = self.store().list_tasks(workflow_id)?;
                if all_tasks.len() != plan.tasks.len() {
                    gate_passed = false;
                    gate_failure_reason = Some(format!(
                        "Task count mismatch: expected {} from plan, found {}",
                        plan.tasks.len(),
                        all_tasks.len()
                    ));
                } else {
                    for t in &all_tasks {
                        if t.status != WorkflowTaskStatus::Completed {
                            gate_passed = false;
                            gate_failure_reason = Some(format!(
                                "Task '{}' is in non-completed status '{:?}'",
                                t.id, t.status
                            ));
                            break;
                        }
                        if t.execution_result_json.is_none() {
                            gate_passed = false;
                            gate_failure_reason = Some(format!(
                                "Task '{}' lacks persisted execution result evidence",
                                t.id
                            ));
                            break;
                        }
                        if let Some(ref res_json) = t.execution_result_json {
                            match serde_json::from_str::<m31a_core::workflow::TaskExecutionResult>(
                                res_json,
                            ) {
                                Ok(res) => {
                                    if res.status != WorkflowTaskStatus::Completed {
                                        gate_passed = false;
                                        gate_failure_reason = Some(format!(
                                            "Task '{}' execution result reports non-completed status '{:?}'",
                                            t.id, res.status
                                        ));
                                        break;
                                    }
                                }
                                Err(e) => {
                                    gate_passed = false;
                                    gate_failure_reason = Some(format!(
                                        "Task '{}' has corrupted execution result JSON: {e}",
                                        t.id
                                    ));
                                    break;
                                }
                            }
                        }
                    }
                }
            }

            // Point 8 & 9: Artifact existence and SHA-256 integrity verification
            if gate_passed {
                let artifacts = self.store().list_artifacts(workflow_id)?;
                for art in &artifacts {
                    if !art.file_path.exists() {
                        gate_passed = false;
                        gate_failure_reason = Some(format!(
                            "Artifact '{}' missing on disk at {}",
                            art.name,
                            art.file_path.display()
                        ));
                        break;
                    }
                    match fs::read(&art.file_path) {
                        Ok(bytes) => {
                            let mut hasher = Sha256::new();
                            hasher.update(&bytes);
                            let computed_sha256 = format!("{:x}", hasher.finalize());
                            if computed_sha256 != art.content_sha256 {
                                gate_passed = false;
                                gate_failure_reason = Some(format!(
                                    "Artifact '{}' SHA-256 mismatch: recorded {}, computed {}",
                                    art.name, art.content_sha256, computed_sha256
                                ));
                                break;
                            }
                        }
                        Err(e) => {
                            gate_passed = false;
                            gate_failure_reason = Some(format!(
                                "Failed reading artifact '{}' at {}: {e}",
                                art.name,
                                art.file_path.display()
                            ));
                            break;
                        }
                    }
                }
            }

            // Point 10: Final AutonomousVerifier::verify_check
            if gate_passed {
                let check_res =
                    AutonomousVerifier::verify_check(&repo_root, Duration::from_secs(30)).await;
                let check_passed = check_res.as_ref().map(|c| c.success).unwrap_or(false);
                if !check_passed {
                    gate_passed = false;
                    let diag = check_res
                        .ok()
                        .map(|c| {
                            c.diagnostics
                                .into_iter()
                                .map(|d| d.message)
                                .collect::<Vec<_>>()
                                .join("; ")
                        })
                        .filter(|s| !s.is_empty())
                        .unwrap_or_else(|| "Unknown compiler error".into());
                    gate_failure_reason =
                        Some(format!("Final workspace compiler check failed: {diag}"));
                }
            }

            // Point 11: Final AutonomousVerifier::verify_tests (if project has tests)
            if gate_passed && repo_root.join("Cargo.toml").exists() {
                let test_res =
                    AutonomousVerifier::verify_tests(&repo_root, Duration::from_secs(45)).await;
                let test_passed = test_res.as_ref().map(|c| c.success).unwrap_or(false);
                if !test_passed {
                    gate_passed = false;
                    let diag = test_res
                        .ok()
                        .map(|c| {
                            c.diagnostics
                                .into_iter()
                                .map(|d| d.message)
                                .collect::<Vec<_>>()
                                .join("; ")
                        })
                        .filter(|s| !s.is_empty())
                        .unwrap_or_else(|| "Test suite failure".into());
                    gate_failure_reason =
                        Some(format!("Final workspace test verification failed: {diag}"));
                }
            }

            // Point 12: Final CodebaseSafetyGate audit
            if gate_passed {
                let modified = self
                    .store()
                    .list_tasks(workflow_id)?
                    .into_iter()
                    .filter_map(|t| t.execution_result_json)
                    .filter_map(|j| {
                        serde_json::from_str::<m31a_core::workflow::TaskExecutionResult>(&j).ok()
                    })
                    .flat_map(|r| r.modified_files)
                    .map(PathBuf::from)
                    .collect::<Vec<_>>();

                match m31a_verify::CodebaseSafetyGate::audit_workspace_changes(
                    &repo_root,
                    &modified,
                    &[],
                ) {
                    Ok(audit) => {
                        if audit.verdict == m31a_verify::SafetyGateVerdict::Blocked {
                            gate_passed = false;
                            gate_failure_reason = Some(
                                "Final CodebaseSafetyGate audit blocked workspace changes"
                                    .to_string(),
                            );
                        }
                    }
                    Err(e) => {
                        warn!(err = %e, "Final safety audit check error (non-fatal if non-rust codebase)");
                    }
                }
            }

            if gate_passed {
                self.store()
                    .update_workflow_state(workflow_id, WorkflowState::Completed)?;
                run.state = WorkflowState::Completed;
                run.completed_at_epoch_ms = Some(now);

                // Store delivery summary artifact - fail closed if write fails
                let summary_json = serde_json::json!({
                    "workflow_id": workflow_id,
                    "objective": run.objective,
                    "completed_waves": wave_result.completed_waves,
                    "total_waves": wave_result.total_waves,
                    "status": "completed",
                    "timestamp": now,
                });

                self.store().store_artifact_file(
                    &repo_root,
                    workflow_id,
                    None,
                    "delivery_summary.json",
                    "summary",
                    summary_json.to_string().as_bytes(),
                    "application/json",
                )?;

                self.store().record_event(&WorkflowEvent {
                    id: format!("evt-{}", Uuid::new_v4().simple()),
                    workflow_id: workflow_id.to_string(),
                    task_id: None,
                    agent_id: None,
                    event_type: WorkflowEventType::WorkflowCompleted,
                    timestamp_epoch_ms: now,
                    correlation_id: None,
                    payload_json: summary_json.to_string(),
                    schema_version: 1,
                })?;

                info!(workflow_id = %workflow_id, "Workflow completed successfully passing 12-point integrity gate");
            } else {
                let reason = gate_failure_reason
                    .unwrap_or_else(|| "Final workspace verification check failed".into());
                self.store()
                    .update_workflow_state(workflow_id, WorkflowState::Failed)?;
                run.state = WorkflowState::Failed;
                run.cancellation_reason = Some(reason.clone());
                error!(workflow_id = %workflow_id, reason = %reason, "Final verification failed");
            }
        } else {
            let current_wf = self
                .store()
                .get_workflow(workflow_id)?
                .unwrap_or_else(|| run.clone());
            if current_wf.state == WorkflowState::AwaitingApproval {
                run.state = WorkflowState::AwaitingApproval;
                info!(
                    workflow_id = %workflow_id,
                    "Workflow paused: awaiting integration approval"
                );
            } else {
                self.store()
                    .update_workflow_state(workflow_id, WorkflowState::Failed)?;
                run.state = WorkflowState::Failed;
                run.cancellation_reason = Some("One or more workflow waves failed".into());
                error!(workflow_id = %workflow_id, "Workflow execution failed");
            }
        }

        // Clean up active workflow registrations
        self.active_tokens.lock().unwrap().remove(workflow_id);
        self.active_agent_ids.lock().unwrap().remove(workflow_id);

        Ok(run)
    }

    /// Executes an individual task node with real engine, isolated worktree, or speculative arena.
    pub async fn execute_task(&self, task: WorkflowTask) -> Result<WorkflowTaskExecutionOutcome> {
        let start = Instant::now();
        let start_ms = Utc::now().timestamp_millis();
        let wf_id = task.workflow_id.clone();
        let task_id = task.id.clone();

        // 0. Idempotent check: If task is already completed, return outcome immediately without re-executing
        if let Ok(Some(existing_task)) = self.store().get_task(&wf_id, &task_id)
            && existing_task.status == WorkflowTaskStatus::Completed
        {
            info!(task_id = %task_id, "Task is already completed; skipping execution");
            return Ok(WorkflowTaskExecutionOutcome {
                task_id,
                success: true,
                agent_id: existing_task.assigned_agent_id,
                modified_files: Vec::new(),
                patch_content: None,
                duration_ms: 0,
                verification_passed: true,
                error: None,
            });
        }

        // 1. Claim task in storage with fencing & lease duration
        let worker_id = format!("worker-{}", Uuid::new_v4().simple());
        let lease_duration_ms: i64 = 30_000;
        let claimed_task = match self.store().claim_task(
            &wf_id,
            &task_id,
            &worker_id,
            lease_duration_ms,
        )? {
            Some(t) => t,
            None => {
                // If claim returned None, check if another worker completed it
                if let Ok(Some(existing)) = self.store().get_task(&wf_id, &task_id)
                    && existing.status == WorkflowTaskStatus::Completed
                {
                    return Ok(WorkflowTaskExecutionOutcome {
                        task_id,
                        success: true,
                        agent_id: existing.assigned_agent_id,
                        modified_files: Vec::new(),
                        patch_content: None,
                        duration_ms: 0,
                        verification_passed: true,
                        error: None,
                    });
                }
                bail!(
                    "Failed to claim task '{}': dependencies not satisfied or write-path conflict",
                    task_id
                );
            }
        };

        info!(
            task_id = %task_id,
            attempt_id = claimed_task.attempt_id,
            worker_id = %worker_id,
            kind = ?task.task_kind,
            role = %task.assigned_role,
            "Claimed and executing workflow task"
        );

        // 2. Setup task cancellation token and heartbeat guard
        let task_cancel_token = CancellationToken::new();
        if let Some(wf_token) = self.active_tokens.lock().unwrap().get(&wf_id) {
            let tc = task_cancel_token.clone();
            let wt = wf_token.clone();
            tokio::spawn(async move {
                wt.cancelled().await;
                tc.cancel();
            });
        }

        let heartbeat = TaskHeartbeatGuard::spawn(
            self.store(),
            wf_id.clone(),
            task_id.clone(),
            worker_id.clone(),
            claimed_task.attempt_id,
            lease_duration_ms,
            task_cancel_token.clone(),
        );

        // Transition task status to Running
        self.store()
            .update_task_status(&wf_id, &task_id, WorkflowTaskStatus::Running, None)?;

        self.store().record_event(&WorkflowEvent {
            id: format!("evt-{}", Uuid::new_v4().simple()),
            workflow_id: wf_id.clone(),
            task_id: Some(task_id.clone()),
            agent_id: None,
            event_type: WorkflowEventType::TaskStarted,
            timestamp_epoch_ms: start_ms,
            correlation_id: None,
            payload_json: serde_json::json!({
                "task_name": task.name,
                "task_kind": task.task_kind.as_str(),
                "role": task.assigned_role,
                "attempt_id": claimed_task.attempt_id,
                "worker_id": worker_id,
            })
            .to_string(),
            schema_version: 1,
        })?;

        let run = self
            .store()
            .get_workflow(&wf_id)?
            .ok_or_else(|| anyhow!("Workflow not found"))?;

        let config: WorkflowExecutionConfig =
            serde_json::from_str(&run.configuration_json).unwrap_or_default();
        let task_timeout = Duration::from_secs(config.task_timeout_seconds);

        let mut modified_files = Vec::new();
        let mut patch_content = None;
        let mut executed_agent_id = None;
        let mut success = true;
        let mut error_msg = None;

        let mut build_status = VerificationStatus::NotExecuted;
        let mut tests_status = VerificationStatus::NotExecuted;
        let mut safety_status = VerificationStatus::NotExecuted;
        let mut review_status = VerificationStatus::NotExecuted;
        let mut verif_duration_ms = 0u64;
        let exit_code = None;
        let verification_details = None;

        // 3. Dispatch strictly by typed WorkflowTaskKind with cancellation & timeout check
        let execution_fut = async {
            match task.task_kind {
                m31a_core::workflow::WorkflowTaskKind::Analysis => {
                    info!(task_id = %task_id, "Executing repository analysis task");
                    let ws_root = &run.workspace_root;
                    let project_kind = ProjectRunner::detect(ws_root);

                    let mut crates_count = 0;
                    let mut dependencies = Vec::new();
                    let cargo_toml_path = ws_root.join("Cargo.toml");
                    if cargo_toml_path.exists()
                        && let Ok(content) = fs::read_to_string(&cargo_toml_path)
                    {
                        if content.contains("[workspace]") {
                            crates_count = content.matches("crates/").count();
                        } else {
                            crates_count = 1;
                        }
                        for line in content.lines() {
                            if line.starts_with("[dependencies")
                                || line.starts_with("[dev-dependencies")
                            {
                                continue;
                            }
                            if let Some((dep, _)) = line.split_once('=') {
                                let trimmed = dep.trim();
                                if !trimmed.is_empty()
                                    && !trimmed.starts_with('#')
                                    && !trimmed.starts_with('[')
                                {
                                    dependencies.push(trimmed.to_string());
                                }
                            }
                        }
                    }

                    // Check baseline compiler status
                    let v_start = Instant::now();
                    let check_report =
                        AutonomousVerifier::verify_check(ws_root, Duration::from_secs(30)).await;
                    verif_duration_ms = v_start.elapsed().as_millis() as u64;

                    let baseline_clean = check_report.as_ref().map(|r| r.success).unwrap_or(false);
                    build_status = if baseline_clean {
                        VerificationStatus::Passed
                    } else {
                        VerificationStatus::Failed
                    };
                    tests_status = VerificationStatus::NotExecuted;
                    safety_status = VerificationStatus::NotExecuted;
                    review_status = VerificationStatus::NotApplicable;

                    let analysis_report = serde_json::json!({
                        "task_id": task_id,
                        "workspace_root": ws_root.display().to_string(),
                        "objective": run.objective,
                        "project_kind": format!("{:?}", project_kind),
                        "crates_count": crates_count,
                        "dependencies_count": dependencies.len(),
                        "baseline_compiler_clean": baseline_clean,
                        "status": "analysis_completed",
                    });

                    self.store().store_artifact_file(
                        ws_root,
                        &wf_id,
                        Some(&task_id),
                        "analysis_report.json",
                        "analysis",
                        analysis_report.to_string().as_bytes(),
                        "application/json",
                    )?;
                }

                m31a_core::workflow::WorkflowTaskKind::Speculative => {
                    info!(
                        task_id = %task_id,
                        "Dispatching Speculative Candidate Arena for task"
                    );
                    let candidates = SpeculativeArena::generate_diverse_hypotheses_with_provider(
                        &run.objective,
                        3,
                        self.base_client.as_ref(),
                    )
                    .await;

                    let executor: Option<Arc<dyn crate::speculative::CandidateExecutor>> =
                        crate::speculative::create_candidate_executor(
                            self.base_client.clone(),
                            self.tools.clone(),
                            Some(self.storage.clone()),
                            self.exec_policy.clone(),
                            None,
                        )
                        .ok();

                    let opts = SpeculateOptions {
                        workspace_root: run.workspace_root.clone(),
                        mission: run.objective.clone(),
                        candidates,
                        timeout_per_candidate: Duration::from_secs(45),
                        auto_promote_winner: false,
                        require_approval_for_promotion: false,
                        max_concurrency: 3,
                        executor,
                    };

                    let v_start = Instant::now();
                    match SpeculativeArena::compete_with_options(opts).await {
                        Ok(report) => {
                            verif_duration_ms = v_start.elapsed().as_millis() as u64;
                            if let Some(ref winner_id) = report.winning_branch_id {
                                if let Some(winner) = report
                                    .evaluations
                                    .iter()
                                    .find(|e| &e.branch_id == winner_id)
                                {
                                    modified_files = winner.modified_files.clone();
                                    patch_content = Some(winner.patch_content.clone());

                                    let rep_bytes = serde_json::to_vec_pretty(&report)?;
                                    self.store().store_artifact_file(
                                        &run.workspace_root,
                                        &wf_id,
                                        Some(&task_id),
                                        "speculative_report.json",
                                        "report",
                                        &rep_bytes,
                                        "application/json",
                                    )?;

                                    if !winner.patch_content.is_empty() {
                                        self.store().store_artifact_file(
                                            &run.workspace_root,
                                            &wf_id,
                                            Some(&task_id),
                                            "winning_patch.diff",
                                            "patch",
                                            winner.patch_content.as_bytes(),
                                            "text/x-diff",
                                        )?;
                                    }
                                    build_status = VerificationStatus::Passed;
                                    tests_status = VerificationStatus::Passed;
                                    safety_status = VerificationStatus::Passed;
                                    review_status = VerificationStatus::Passed;
                                }
                            } else {
                                success = false;
                                error_msg = Some(
                                    "No speculative candidate satisfied mandatory hard gates"
                                        .into(),
                                );
                                build_status = VerificationStatus::Failed;
                                tests_status = VerificationStatus::Failed;
                            }
                        }
                        Err(e) => {
                            success = false;
                            error_msg = Some(e.to_string());
                            build_status = VerificationStatus::Failed;
                        }
                    }
                }

                m31a_core::workflow::WorkflowTaskKind::Integration => {
                    info!(
                        task_id = %task_id,
                        "Applying winning candidate patch onto primary workspace via WorkflowIntegrationService"
                    );
                    let artifacts = self.store().list_artifacts(&wf_id)?;
                    let patch_opt = artifacts
                        .iter()
                        .find(|a| a.name == "winning_patch.diff")
                        .and_then(|patch_art| fs::read_to_string(&patch_art.file_path).ok());

                    if let Some(diff_text) = patch_opt {
                        if diff_text.trim().is_empty() {
                            success = false;
                            error_msg =
                                Some("Winning patch diff is empty; cannot integrate".into());
                        } else {
                            let change_set_hash =
                                crate::integration::compute_change_set_hash(&diff_text);
                            let is_approved = self.store().is_patch_approved(
                                &wf_id,
                                &task_id,
                                claimed_task.attempt_id,
                                &change_set_hash,
                            )?;

                            if config.require_approval_for_integration && !is_approved {
                                let pending_req = self.store().get_pending_approval(
                                    &wf_id,
                                    &task_id,
                                    claimed_task.attempt_id,
                                )?;
                                let approval_id = if let Some(p) = pending_req {
                                    p.id
                                } else {
                                    let new_id = format!("appr-{}", Uuid::new_v4().simple());
                                    let preview =
                                        diff_text.lines().take(40).collect::<Vec<_>>().join("\n");
                                    let app_req = m31a_core::workflow::WorkflowApprovalRequest {
                                        id: new_id.clone(),
                                        workflow_id: wf_id.clone(),
                                        task_id: task_id.clone(),
                                        attempt_id: claimed_task.attempt_id,
                                        change_set_hash: change_set_hash.clone(),
                                        patch_preview: Some(preview),
                                        status:
                                            m31a_core::workflow::WorkflowApprovalStatus::Pending,
                                        requested_at_epoch_ms: Utc::now().timestamp_millis(),
                                        decided_at_epoch_ms: None,
                                        decision_actor: None,
                                        decision_reason: None,
                                    };
                                    self.store().create_approval_request(&app_req)?;
                                    new_id
                                };

                                self.store().record_event(&WorkflowEvent {
                                    id: format!("evt-{}", Uuid::new_v4().simple()),
                                    workflow_id: wf_id.clone(),
                                    task_id: Some(task_id.clone()),
                                    agent_id: None,
                                    event_type: WorkflowEventType::ApprovalRequested,
                                    timestamp_epoch_ms: Utc::now().timestamp_millis(),
                                    correlation_id: None,
                                    payload_json: serde_json::json!({
                                        "approval_id": approval_id,
                                        "attempt_id": claimed_task.attempt_id,
                                        "change_set_hash": change_set_hash,
                                    })
                                    .to_string(),
                                    schema_version: 1,
                                })?;

                                self.store().update_task_status(
                                    &wf_id,
                                    &task_id,
                                    WorkflowTaskStatus::NeedsApproval,
                                    None,
                                )?;
                                self.store().update_workflow_state(
                                    &wf_id,
                                    WorkflowState::AwaitingApproval,
                                )?;

                                success = false;
                                error_msg =
                                    Some("Integration blocked: human approval required".into());
                                build_status = VerificationStatus::Pending;
                                tests_status = VerificationStatus::Pending;
                                safety_status = VerificationStatus::Pending;
                                review_status = VerificationStatus::Pending;
                            } else {
                                let worktree_record = self.store().get_worktree(&task_id).ok().flatten();
                                let source_branch = worktree_record.as_ref().map(|w| w.branch.clone());
                                let source_base_sha = worktree_record.as_ref().map(|w| w.base_sha.clone());
                                let svc = WorkflowIntegrationService::new().with_store(self.store());
                                let req = IntegrationRequest {
                                    workspace_root: run.workspace_root.clone(),
                                    patch_content: diff_text.clone(),
                                    source_label: format!("workflow-task-{}", task_id),
                                    commit_message: Some(format!(
                                        "feat(workflow): integrate task {}",
                                        task_id
                                    )),
                                    require_approval: config.require_approval_for_integration,
                                    is_approved: true,
                                    author_role: Some(task.assigned_role.clone()),
                                    skip_dirty_check: false,
                                    skip_verification: false,
                                    verification_timeout: Some(Duration::from_secs(30)),
                                    workflow_id: Some(wf_id.clone()),
                                    task_id: Some(task_id.clone()),
                                    attempt_id: Some(claimed_task.attempt_id),
                                    expected_change_set_hash: Some(change_set_hash),
                                    source_branch,
                                    source_base_sha,
                                    target_base_sha: None,
                                };

                                let v_start = Instant::now();
                                match svc.integrate(&req).await {
                                    Ok(outcome) => {
                                        verif_duration_ms = v_start.elapsed().as_millis() as u64;
                                        if outcome.success {
                                            for p in outcome.modified_files {
                                                modified_files
                                                    .push(p.to_string_lossy().to_string());
                                            }
                                            patch_content = Some(diff_text);
                                            build_status = VerificationStatus::Passed;
                                            tests_status = VerificationStatus::Passed;
                                            safety_status = VerificationStatus::Passed;
                                            review_status = VerificationStatus::Passed;
                                        } else {
                                            success = false;
                                            error_msg = outcome.error.or(Some(outcome.summary));
                                            build_status = if outcome.verification_passed {
                                                VerificationStatus::Passed
                                            } else {
                                                VerificationStatus::Failed
                                            };
                                            tests_status = build_status;
                                            safety_status = VerificationStatus::Passed;
                                            review_status = VerificationStatus::Failed;
                                        }
                                    }
                                    Err(e) => {
                                        success = false;
                                        error_msg = Some(format!("Integration error: {e}"));
                                        build_status = VerificationStatus::Failed;
                                    }
                                }
                            }
                        }
                    } else {
                        success = false;
                        error_msg = Some(
                            "Winning patch artifact 'winning_patch.diff' not found for integration"
                                .into(),
                        );
                    }
                }

                m31a_core::workflow::WorkflowTaskKind::Implementation => {
                    info!(
                        task_id = %task_id,
                        "Executing implementation task via child agent"
                    );
                    if task_cancel_token.is_cancelled() {
                        success = false;
                        error_msg = Some("Task cancelled prior to agent spawning".into());
                        build_status = VerificationStatus::Blocked;
                    } else if let Some(ref rt) = self.runtime {
                        use m31a_tools::agent_runtime::AgentRuntime;
                        let req = m31a_tools::agent_runtime::AgentSpawnRequest {
                            role_name: task.assigned_role.clone(),
                            task_name: task.name.clone(),
                            prompt: format!(
                                "Task: {}\nDescription: {}\nCriteria: {:?}\nVerification: {:?}\n\nObjective: {}",
                                task.name,
                                task.description,
                                task.acceptance_criteria,
                                task.required_verification,
                                run.objective
                            ),
                            isolate_worktree: task.workspace_isolation
                                == WorkspaceIsolationStrategy::Worktree,
                            model_override: None,
                            depth: 1,
                            max_depth: 3,
                        };

                        match rt.spawn_agent(req).await {
                            Ok(spawned) => {
                                executed_agent_id = Some(spawned.agent_id.clone());
                                self.active_agent_ids
                                    .lock()
                                    .unwrap()
                                    .entry(wf_id.clone())
                                    .or_default()
                                    .push(spawned.agent_id.clone());

                                if task_cancel_token.is_cancelled() {
                                    let _ = rt.terminate_tree(&spawned.agent_id).await;
                                    success = false;
                                    error_msg =
                                        Some("Task cancelled immediately after agent spawn".into());
                                    build_status = VerificationStatus::Blocked;
                                } else {
                                    let wait_fut = rt.wait_agents(
                                        std::slice::from_ref(&spawned.agent_id),
                                        Some(config.task_timeout_seconds * 1000),
                                    );
                                    let wait_outcomes = tokio::select! {
                                        _ = task_cancel_token.cancelled() => {
                                            warn!(agent_id = %spawned.agent_id, "Task cancel token fired while waiting for agent; terminating tree");
                                            let _ = rt.terminate_tree(&spawned.agent_id).await;
                                            Err(anyhow!("Task cancelled while awaiting child agent"))
                                        }
                                        res = wait_fut => res,
                                    };
                                    match wait_outcomes {
                                        Ok(outcomes) => {
                                            let outcome = outcomes.into_iter().next().unwrap_or(
                                                m31a_tools::agent_runtime::AgentWaitOutcome {
                                                    agent_id: spawned.agent_id.clone(),
                                                    status: "timeout".to_string(),
                                                    answer: None,
                                                },
                                            );

                                            match outcome.status.as_str() {
                                                "completed" | "integration_pending" => {
                                                    let diff =
                                                        if let Some(ref mgr) = self.worktree_mgr {
                                                            let worktrees = mgr
                                                                .list_worktrees()
                                                                .unwrap_or_default();
                                                            if let Some(wt) =
                                                                worktrees.iter().find(|w| {
                                                                    w.thread_id == spawned.agent_id
                                                                })
                                                            {
                                                                mgr.export_worktree_diff(
                                                                    &wt.worktree_path,
                                                                    &wt.head_sha,
                                                                )
                                                                .unwrap_or_default()
                                                            } else {
                                                                String::new()
                                                            }
                                                        } else {
                                                            String::new()
                                                        };

                                                    let change_set_hash =
                                                        crate::integration::compute_change_set_hash(
                                                            &diff,
                                                        );
                                                    let is_approved = if diff.trim().is_empty() {
                                                        true
                                                    } else {
                                                        self.store().is_patch_approved(
                                                            &wf_id,
                                                            &task_id,
                                                            claimed_task.attempt_id,
                                                            &change_set_hash,
                                                        )?
                                                    };

                                                    if config.require_approval_for_integration
                                                        && !diff.trim().is_empty()
                                                        && !is_approved
                                                    {
                                                        let pending_req =
                                                            self.store().get_pending_approval(
                                                                &wf_id,
                                                                &task_id,
                                                                claimed_task.attempt_id,
                                                            )?;
                                                        let approval_id = if let Some(p) =
                                                            pending_req
                                                        {
                                                            p.id
                                                        } else {
                                                            let new_id = format!(
                                                                "appr-{}",
                                                                Uuid::new_v4().simple()
                                                            );
                                                            let preview = diff
                                                                .lines()
                                                                .take(40)
                                                                .collect::<Vec<_>>()
                                                                .join("\n");
                                                            let app_req = m31a_core::workflow::WorkflowApprovalRequest {
                                                                id: new_id.clone(),
                                                                workflow_id: wf_id.clone(),
                                                                task_id: task_id.clone(),
                                                                attempt_id: claimed_task.attempt_id,
                                                                change_set_hash: change_set_hash.clone(),
                                                                patch_preview: Some(preview),
                                                                status: m31a_core::workflow::WorkflowApprovalStatus::Pending,
                                                                requested_at_epoch_ms: Utc::now().timestamp_millis(),
                                                                decided_at_epoch_ms: None,
                                                                decision_actor: None,
                                                                decision_reason: None,
                                                            };
                                                            self.store().create_approval_request(
                                                                &app_req,
                                                            )?;
                                                            new_id
                                                        };

                                                        self.store().record_event(&WorkflowEvent {
                                                            id: format!("evt-{}", Uuid::new_v4().simple()),
                                                            workflow_id: wf_id.clone(),
                                                            task_id: Some(task_id.clone()),
                                                            agent_id: Some(spawned.agent_id.clone()),
                                                            event_type: WorkflowEventType::ApprovalRequested,
                                                            timestamp_epoch_ms: Utc::now().timestamp_millis(),
                                                            correlation_id: None,
                                                            payload_json: serde_json::json!({
                                                                "approval_id": approval_id,
                                                                "attempt_id": claimed_task.attempt_id,
                                                                "change_set_hash": change_set_hash,
                                                            }).to_string(),
                                                            schema_version: 1,
                                                        })?;

                                                        self.store().update_task_status(
                                                            &wf_id,
                                                            &task_id,
                                                            WorkflowTaskStatus::NeedsApproval,
                                                            None,
                                                        )?;
                                                        self.store().update_workflow_state(
                                                            &wf_id,
                                                            WorkflowState::AwaitingApproval,
                                                        )?;

                                                        success = false;
                                                        error_msg = Some("Integration blocked: human approval required".into());
                                                        build_status = VerificationStatus::Pending;
                                                        tests_status = VerificationStatus::Pending;
                                                        safety_status = VerificationStatus::Pending;
                                                        review_status = VerificationStatus::Pending;
                                                    } else {
                                                        match rt
                                                            .integrate_agent(
                                                                &spawned.agent_id,
                                                                None,
                                                            )
                                                            .await
                                                        {
                                                            Ok(summary) => {
                                                                info!(agent_id = %spawned.agent_id, summary = %summary, "Child agent integrated successfully");
                                                                if let Some(ref path) =
                                                                    spawned.worktree_path
                                                                {
                                                                    modified_files.push(
                                                                        path.to_string_lossy()
                                                                            .to_string(),
                                                                    );
                                                                }
                                                                patch_content = Some(diff);
                                                                build_status =
                                                                    VerificationStatus::Passed;
                                                                tests_status =
                                                                    VerificationStatus::Passed;
                                                                safety_status =
                                                                    VerificationStatus::Passed;
                                                                review_status =
                                                                    VerificationStatus::Passed;
                                                            }
                                                            Err(e) => {
                                                                success = false;
                                                                error_msg = Some(format!(
                                                                    "Child agent integration failed: {e}"
                                                                ));
                                                                build_status =
                                                                    VerificationStatus::Failed;
                                                            }
                                                        }
                                                    }
                                                }
                                                other => {
                                                    success = false;
                                                    error_msg = Some(format!(
                                                        "Child agent terminated with non-success status '{other}'"
                                                    ));
                                                    build_status = VerificationStatus::Failed;
                                                    let _ =
                                                        rt.terminate_tree(&spawned.agent_id).await;
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            success = false;
                                            error_msg = Some(format!(
                                                "Failed waiting for child agent: {e}"
                                            ));
                                            build_status = VerificationStatus::Failed;
                                            let _ = rt.terminate_tree(&spawned.agent_id).await;
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                success = false;
                                error_msg = Some(format!("Failed to spawn child agent: {e}"));
                                build_status = VerificationStatus::Failed;
                            }
                        }
                    } else {
                        success = false;
                        error_msg = Some(
                            "Execution runtime (EngineAgentRuntime) is not configured; cannot execute implementation task".into(),
                        );
                        build_status = VerificationStatus::Blocked;
                    }
                }

                m31a_core::workflow::WorkflowTaskKind::Verification => {
                    info!(
                        task_id = %task_id,
                        "Executing comprehensive verification suite"
                    );
                    let v_start = Instant::now();
                    let check_res = AutonomousVerifier::verify_check(
                        &run.workspace_root,
                        Duration::from_secs(30),
                    )
                    .await;
                    let check_ok = check_res.map(|r| r.success).unwrap_or(false);
                    build_status = if check_ok {
                        VerificationStatus::Passed
                    } else {
                        VerificationStatus::Failed
                    };

                    let test_res = if check_ok {
                        AutonomousVerifier::verify_tests(
                            &run.workspace_root,
                            Duration::from_secs(45),
                        )
                        .await
                    } else {
                        Err(anyhow!("Check failed; skipping tests"))
                    };
                    let test_ok = test_res.map(|r| r.success).unwrap_or(false);
                    tests_status = if test_ok {
                        VerificationStatus::Passed
                    } else {
                        VerificationStatus::Failed
                    };

                    let mut mod_paths = Vec::new();
                    let all_tasks = self.store().list_tasks(&wf_id)?;
                    for t in &all_tasks {
                        if let Some(ref rjson) = t.execution_result_json
                            && let Ok(r) = serde_json::from_str::<
                                m31a_core::workflow::TaskExecutionResult,
                            >(rjson)
                        {
                            for f in r.modified_files {
                                mod_paths.push(PathBuf::from(f));
                            }
                        }
                    }

                    let safety_report = m31a_verify::CodebaseSafetyGate::audit_workspace_changes(
                        &run.workspace_root,
                        &mod_paths,
                        &[],
                    );
                    let safety_verdict = safety_report
                        .as_ref()
                        .map(|r| r.verdict)
                        .unwrap_or(m31a_verify::SafetyGateVerdict::Passed);

                    safety_status = match safety_verdict {
                        m31a_verify::SafetyGateVerdict::Blocked => VerificationStatus::Failed,
                        _ => VerificationStatus::Passed,
                    };
                    review_status = VerificationStatus::NotApplicable;
                    verif_duration_ms = v_start.elapsed().as_millis() as u64;

                    if !check_ok || !test_ok || safety_status != VerificationStatus::Passed {
                        success = false;
                        error_msg = Some(format!(
                            "Verification failed (check: {check_ok}, tests: {test_ok}, safety: {:?})",
                            safety_verdict
                        ));
                    }
                }

                m31a_core::workflow::WorkflowTaskKind::Delivery => {
                    info!(
                        task_id = %task_id,
                        "Executing delivery report task with consolidated evidence"
                    );
                    let all_tasks = self.store().list_tasks(&wf_id)?;
                    let mut all_modified = Vec::new();
                    let mut task_summaries = Vec::new();

                    for t in &all_tasks {
                        if let Some(ref res_json) = t.execution_result_json
                            && let Ok(res) = serde_json::from_str::<
                                m31a_core::workflow::TaskExecutionResult,
                            >(res_json)
                        {
                            for f in res.modified_files {
                                if !all_modified.contains(&f) {
                                    all_modified.push(f);
                                }
                            }
                        }
                        task_summaries.push(serde_json::json!({
                            "task_id": t.id,
                            "task_kind": t.task_kind.as_str(),
                            "status": t.status.as_str(),
                            "attempt_id": t.attempt_id,
                        }));
                    }

                    let artifacts = self.store().list_artifacts(&wf_id)?;
                    let artifact_names: Vec<String> =
                        artifacts.into_iter().map(|a| a.name).collect();

                    let git_head = std::process::Command::new("git")
                        .current_dir(&run.workspace_root)
                        .args(["rev-parse", "HEAD"])
                        .output()
                        .ok()
                        .and_then(|o| {
                            if o.status.success() {
                                String::from_utf8(o.stdout).ok()
                            } else {
                                None
                            }
                        })
                        .map(|s| s.trim().to_string());

                    let delivery_summary = serde_json::json!({
                        "workflow_id": wf_id,
                        "objective": run.objective,
                        "status": "delivery_ready",
                        "git_head_commit": git_head,
                        "total_tasks": all_tasks.len(),
                        "tasks": task_summaries,
                        "modified_files": all_modified,
                        "artifacts": artifact_names,
                        "timestamp": Utc::now().timestamp_millis(),
                    });

                    self.store().store_artifact_file(
                        &run.workspace_root,
                        &wf_id,
                        Some(&task_id),
                        "delivery_report.json",
                        "delivery",
                        delivery_summary.to_string().as_bytes(),
                        "application/json",
                    )?;

                    build_status = VerificationStatus::NotApplicable;
                    tests_status = VerificationStatus::NotApplicable;
                    safety_status = VerificationStatus::NotApplicable;
                    review_status = VerificationStatus::NotApplicable;
                }
            }
            Ok::<(), anyhow::Error>(())
        };

        // Enforce task deadline and cancellation
        tokio::select! {
            _ = task_cancel_token.cancelled() => {
                success = false;
                error_msg = Some("Task cancelled or lease renewal expired/fenced".to_string());
                build_status = VerificationStatus::Blocked;
            }
            res = tokio::time::timeout(task_timeout, execution_fut) => {
                match res {
                    Ok(Ok(())) => {}
                    Ok(Err(e)) => {
                        success = false;
                        error_msg = Some(e.to_string());
                    }
                    Err(_) => {
                        success = false;
                        error_msg = Some(format!("Task execution timed out after {:?}", task_timeout));
                    }
                }
            }
        }

        // If task failed, timed out, or cancelled, terminate any spawned child agent tree immediately
        if (!success || task_cancel_token.is_cancelled())
            && let (Some(rt), Some(aid)) = (self.runtime.as_ref(), executed_agent_id.as_ref())
        {
            info!(agent_id = %aid, "Terminating child agent tree due to task non-success/timeout/cancellation");
            let _ = rt.terminate_tree(aid).await;
        }

        // Stop the heartbeat guard
        heartbeat.stop();

        // 4. If implementation task succeeded, run verification check to confirm workspace integrity
        let verif_passed = if success {
            if task.task_kind == m31a_core::workflow::WorkflowTaskKind::Implementation {
                let v_check =
                    AutonomousVerifier::verify_check(&run.workspace_root, Duration::from_secs(20))
                        .await;
                let ok = v_check.map(|r| r.success).unwrap_or(false);
                if !ok {
                    success = false;
                    build_status = VerificationStatus::Failed;
                    error_msg = Some(
                        "Post-implementation compiler check failed on primary workspace".into(),
                    );
                }
                ok
            } else {
                build_status == VerificationStatus::Passed
                    || build_status == VerificationStatus::NotApplicable
                    || build_status == VerificationStatus::NotExecuted
            }
        } else {
            false
        };

        let duration_ms = start.elapsed().as_millis() as u64;
        let current_task_status = self.store().get_task(&wf_id, &task_id)?.map(|t| t.status);
        let final_status = if current_task_status == Some(WorkflowTaskStatus::NeedsApproval) {
            WorkflowTaskStatus::NeedsApproval
        } else if success && verif_passed {
            WorkflowTaskStatus::Completed
        } else {
            WorkflowTaskStatus::Failed
        };

        // 5. Update task record with full TaskExecutionResult and ATTEMPT FENCING in storage
        let exec_result = m31a_core::workflow::TaskExecutionResult {
            task_id: task_id.clone(),
            workflow_id: wf_id.clone(),
            attempt_id: claimed_task.attempt_id,
            worker_id: Some(worker_id.clone()),
            agent_id: executed_agent_id.clone(),
            task_kind: task.task_kind,
            started_at_epoch_ms: start_ms,
            completed_at_epoch_ms: Utc::now().timestamp_millis(),
            status: final_status,
            modified_files: modified_files.clone(),
            patch_content: patch_content.clone(),
            build_passed: build_status == VerificationStatus::Passed,
            tests_passed: tests_status == VerificationStatus::Passed,
            safety_passed: safety_status == VerificationStatus::Passed,
            build_status,
            tests_status,
            safety_status,
            review_status,
            verification_duration_ms: verif_duration_ms,
            exit_code,
            verification_details,
            failure_reason: error_msg.clone(),
        };

        self.store().update_task_execution_result(
            &wf_id,
            &task_id,
            Some(claimed_task.attempt_id),
            Some(&worker_id),
            final_status,
            Some(&exec_result),
            error_msg.as_deref(),
        )?;

        self.store().record_event(&WorkflowEvent {
            id: format!("evt-{}", Uuid::new_v4().simple()),
            workflow_id: wf_id,
            task_id: Some(task_id.clone()),
            agent_id: executed_agent_id.clone(),
            event_type: if success && verif_passed {
                WorkflowEventType::VerificationCompleted
            } else {
                WorkflowEventType::TaskFailed
            },
            timestamp_epoch_ms: Utc::now().timestamp_millis(),
            correlation_id: None,
            payload_json: serde_json::json!({
                "status": final_status.as_str(),
                "duration_ms": duration_ms,
                "error": error_msg,
                "attempt_id": claimed_task.attempt_id,
                "worker_id": worker_id,
            })
            .to_string(),
            schema_version: 1,
        })?;

        Ok(WorkflowTaskExecutionOutcome {
            task_id,
            success: success && verif_passed,
            agent_id: executed_agent_id,
            modified_files,
            patch_content,
            duration_ms,
            verification_passed: verif_passed,
            error: error_msg,
        })
    }

    /// Resumes an interrupted or nonterminal workflow from durable storage.
    pub async fn resume_workflow(&self, workflow_id: &str) -> Result<WorkflowRun> {
        let run = self
            .store()
            .get_workflow(workflow_id)?
            .ok_or_else(|| anyhow!("Workflow '{workflow_id}' not found"))?;

        if run.state.is_terminal() {
            return Ok(run);
        }

        info!(workflow_id = %workflow_id, "Resuming interrupted workflow");

        // 1. Recover interrupted worktrees and reconcile stale task leases
        if let Some(ref mgr) = self.worktree_mgr {
            let recovered = mgr.recover_interrupted_worktrees(workflow_id)?;
            info!(workflow_id = %workflow_id, recovered_worktrees = recovered.len(), "Recovered interrupted worktrees");
        } else {
            let _ = self.store().recover_interrupted_worktrees(workflow_id);
        }

        let now = Utc::now().timestamp_millis();
        let recovered_count = self.store().reconcile_stale_tasks(workflow_id, now)?;
        info!(workflow_id = %workflow_id, recovered_tasks = recovered_count, "Reconciled stale tasks");

        self.store().record_event(&WorkflowEvent {
            id: format!("evt-{}", Uuid::new_v4().simple()),
            workflow_id: workflow_id.to_string(),
            task_id: None,
            agent_id: None,
            event_type: WorkflowEventType::WorkflowResumed,
            timestamp_epoch_ms: now,
            correlation_id: None,
            payload_json: serde_json::json!({ "recovered_tasks": recovered_count }).to_string(),
            schema_version: 1,
        })?;

        // 2. Continue execution
        self.execute_workflow(workflow_id).await
    }

    /// Approves a pending integration request for a workflow task attempt.
    /// `target_id` can be either an approval request ID or a task ID.
    pub async fn approve_integration(
        &self,
        workflow_id: &str,
        target_id: &str,
        actor: &str,
        reason: Option<&str>,
    ) -> Result<()> {
        let run = self
            .store()
            .get_workflow(workflow_id)?
            .ok_or_else(|| anyhow!("Workflow '{workflow_id}' not found"))?;

        if run.state.is_terminal() {
            bail!(
                "Cannot approve integration on terminal workflow '{}'",
                workflow_id
            );
        }

        let (task_id, pending) = if let Ok(Some(app)) = self.store().get_approval_request(target_id)
        {
            (app.task_id.clone(), app)
        } else {
            let task = self
                .store()
                .get_task(workflow_id, target_id)?
                .ok_or_else(|| {
                    anyhow!(
                        "Neither approval nor task '{target_id}' found in workflow '{workflow_id}'"
                    )
                })?;
            let p = self
                .store()
                .get_pending_approval(workflow_id, target_id, task.attempt_id)?
                .ok_or_else(|| {
                    anyhow!(
                        "No pending approval request found for task '{target_id}' attempt {}",
                        task.attempt_id
                    )
                })?;
            (target_id.to_string(), p)
        };

        let now = Utc::now().timestamp_millis();
        let updated = self.store().record_approval_decision(
            &pending.id,
            m31a_core::workflow::WorkflowApprovalStatus::Approved,
            actor,
            reason,
        )?;

        if !updated {
            bail!(
                "Failed to record approval decision for request '{}'",
                pending.id
            );
        }

        self.store().record_event(&WorkflowEvent {
            id: format!("evt-{}", Uuid::new_v4().simple()),
            workflow_id: workflow_id.to_string(),
            task_id: Some(task_id.clone()),
            agent_id: None,
            event_type: WorkflowEventType::ApprovalDecisionRecorded,
            timestamp_epoch_ms: now,
            correlation_id: None,
            payload_json: serde_json::json!({
                "approval_id": pending.id,
                "decision": "approved",
                "actor": actor,
                "reason": reason,
                "change_set_hash": pending.change_set_hash,
                "attempt_id": pending.attempt_id,
            })
            .to_string(),
            schema_version: 1,
        })?;

        // Transition task back to Ready so it can be claimed and executed
        self.store()
            .update_task_status(workflow_id, &task_id, WorkflowTaskStatus::Ready, None)?;

        // Transition workflow state back to Ready if currently AwaitingApproval
        if run.state == WorkflowState::AwaitingApproval {
            self.store()
                .update_workflow_state(workflow_id, WorkflowState::Ready)?;
        }

        info!(
            workflow_id = %workflow_id,
            task_id = %task_id,
            actor = %actor,
            "Integration approval granted and recorded"
        );

        Ok(())
    }

    /// Rejects a pending integration request for a workflow task attempt.
    /// `target_id` can be either an approval request ID or a task ID.
    pub async fn reject_integration(
        &self,
        workflow_id: &str,
        target_id: &str,
        actor: &str,
        reason: Option<&str>,
    ) -> Result<()> {
        let run = self
            .store()
            .get_workflow(workflow_id)?
            .ok_or_else(|| anyhow!("Workflow '{workflow_id}' not found"))?;

        let (task_id, pending) = if let Ok(Some(app)) = self.store().get_approval_request(target_id)
        {
            (app.task_id.clone(), app)
        } else {
            let task = self
                .store()
                .get_task(workflow_id, target_id)?
                .ok_or_else(|| {
                    anyhow!(
                        "Neither approval nor task '{target_id}' found in workflow '{workflow_id}'"
                    )
                })?;
            let p = self
                .store()
                .get_pending_approval(workflow_id, target_id, task.attempt_id)?
                .ok_or_else(|| {
                    anyhow!(
                        "No pending approval request found for task '{target_id}' attempt {}",
                        task.attempt_id
                    )
                })?;
            (target_id.to_string(), p)
        };

        let now = Utc::now().timestamp_millis();
        let updated = self.store().record_approval_decision(
            &pending.id,
            m31a_core::workflow::WorkflowApprovalStatus::Rejected,
            actor,
            reason,
        )?;

        if !updated {
            bail!(
                "Failed to record rejection decision for request '{}'",
                pending.id
            );
        }

        self.store().record_event(&WorkflowEvent {
            id: format!("evt-{}", Uuid::new_v4().simple()),
            workflow_id: workflow_id.to_string(),
            task_id: Some(task_id.clone()),
            agent_id: None,
            event_type: WorkflowEventType::ApprovalDecisionRecorded,
            timestamp_epoch_ms: now,
            correlation_id: None,
            payload_json: serde_json::json!({
                "approval_id": pending.id,
                "decision": "rejected",
                "actor": actor,
                "reason": reason,
                "change_set_hash": pending.change_set_hash,
                "attempt_id": pending.attempt_id,
            })
            .to_string(),
            schema_version: 1,
        })?;

        let rej_reason = reason.unwrap_or("Integration rejected by operator");
        self.store().update_task_status(
            workflow_id,
            &task_id,
            WorkflowTaskStatus::Failed,
            Some(&format!("Rejected by {actor}: {rej_reason}")),
        )?;

        // If workflow was awaiting approval and task failed, transition workflow to Failed
        if run.state == WorkflowState::AwaitingApproval {
            self.store()
                .update_workflow_state(workflow_id, WorkflowState::Failed)?;
        }

        info!(
            workflow_id = %workflow_id,
            task_id = %task_id,
            actor = %actor,
            "Integration approval rejected and recorded"
        );

        Ok(())
    }

    /// Cancels a running workflow and all its active tasks and child agent trees.
    pub async fn cancel_workflow(&self, workflow_id: &str, reason: &str) -> Result<()> {
        let run = self
            .store()
            .get_workflow(workflow_id)?
            .ok_or_else(|| anyhow!("Workflow '{workflow_id}' not found"))?;

        if run.state.is_terminal() {
            return Ok(());
        }

        // 1. Signal cancellation token for active tasks in this workflow
        if let Ok(guard) = self.active_tokens.lock()
            && let Some(token) = guard.get(workflow_id)
        {
            token.cancel();
        }

        // 2. Terminate active child agents and agent trees
        if let Some(ref rt) = self.runtime {
            let agent_ids: Vec<String> = if let Ok(guard) = self.active_agent_ids.lock() {
                guard.get(workflow_id).cloned().unwrap_or_default()
            } else {
                Vec::new()
            };

            for agent_id in agent_ids {
                info!(
                    workflow_id = %workflow_id,
                    agent_id = %agent_id,
                    "Terminating child agent tree on workflow cancellation"
                );
                if let Err(e) = rt.terminate_tree(&agent_id).await {
                    warn!(
                        workflow_id = %workflow_id,
                        agent_id = %agent_id,
                        err = %e,
                        "Failed to terminate child agent tree during workflow cancellation"
                    );
                }
            }
        }

        // 3. Preserve interrupted worktrees and unintegrated artifacts for forensic inspection
        if let Some(ref mgr) = self.worktree_mgr {
            let recovered = mgr.recover_interrupted_worktrees(workflow_id)?;
            info!(workflow_id = %workflow_id, preserved_worktrees = recovered.len(), "Preserved worktree artifacts on cancellation");
        } else {
            let _ = self.store().recover_interrupted_worktrees(workflow_id);
        }

        let now = Utc::now().timestamp_millis();
        self.store()
            .update_workflow_state(workflow_id, WorkflowState::Cancelled)?;

        // Cancel all pending or running tasks
        let tasks = self.store().list_tasks(workflow_id)?;
        for task in tasks {
            if !task.status.is_terminal() {
                let _ = self.store().update_task_status(
                    workflow_id,
                    &task.id,
                    WorkflowTaskStatus::Cancelled,
                    Some(reason),
                );
            }
        }

        self.store().record_event(&WorkflowEvent {
            id: format!("evt-{}", Uuid::new_v4().simple()),
            workflow_id: workflow_id.to_string(),
            task_id: None,
            agent_id: None,
            event_type: WorkflowEventType::WorkflowCancelled,
            timestamp_epoch_ms: now,
            correlation_id: None,
            payload_json: serde_json::json!({ "reason": reason }).to_string(),
            schema_version: 1,
        })?;

        info!(workflow_id = %workflow_id, reason = %reason, "Workflow cancelled");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn setup_test_orchestrator() -> (WorkflowOrchestrator, tempfile::TempDir) {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let storage = Storage::open(&db_path).unwrap();
        let orch = WorkflowOrchestrator::new(storage, None, None, None, None);
        (orch, temp_dir)
    }

    #[tokio::test]
    async fn test_orchestrator_create_plan_and_dag_generation() {
        let (orch, temp_dir) = setup_test_orchestrator();

        let run = orch
            .create_workflow(
                temp_dir.path(),
                "Implement secure project backup system",
                "autonomous",
                None,
                None,
            )
            .unwrap();

        assert_eq!(run.state, WorkflowState::Created);

        let plan = orch.generate_plan(&run.id).await.unwrap();
        assert!(plan.tasks.len() >= 3);
        assert!(plan.validate().is_ok());

        let waves = plan.partition_into_waves().unwrap();
        assert!(waves.len() >= 2);

        let events = orch.store().list_events(&run.id, None).unwrap();
        assert!(events.len() >= 2);
    }

    #[tokio::test]
    async fn test_orchestrator_cancellation_flow() {
        let (orch, temp_dir) = setup_test_orchestrator();

        let run = orch
            .create_workflow(
                temp_dir.path(),
                "Cancelled task test",
                "autonomous",
                None,
                None,
            )
            .unwrap();

        let _ = orch.generate_plan(&run.id).await.unwrap();
        orch.cancel_workflow(&run.id, "User requested stop")
            .await
            .unwrap();

        let fetched = orch.store().get_workflow(&run.id).unwrap().unwrap();
        assert_eq!(fetched.state, WorkflowState::Cancelled);

        let tasks = orch.store().list_tasks(&run.id).unwrap();
        for t in tasks {
            assert_eq!(t.status, WorkflowTaskStatus::Cancelled);
        }
    }

    #[tokio::test]
    async fn test_orchestrator_resume_and_reconciliation() {
        let (orch, temp_dir) = setup_test_orchestrator();

        let run = orch
            .create_workflow(
                temp_dir.path(),
                "Resume test objective",
                "autonomous",
                None,
                None,
            )
            .unwrap();

        let _ = orch.generate_plan(&run.id).await.unwrap();

        // Simulate an interrupted task
        let tasks = orch.store().list_tasks(&run.id).unwrap();
        let _first_task = &tasks[0];
        let _ = orch.store().claim_ready_task(&run.id, "worker-1", -100); // already expired

        // Resume workflow
        let resumed = orch.resume_workflow(&run.id).await.unwrap();
        assert!(resumed.state != WorkflowState::Created);
    }
}
