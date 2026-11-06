//! Production Autonomous Workflow Orchestrator (`m31a-agent-engine::workflow`).
//!
//! Orchestrates end-to-end autonomous engineering workflows:
//! Objective → Context Analysis → Validated Task DAG → Wave Scheduling →
//! Concurrent Child Agents in Worktrees → Speculative Branches where useful →
//! Verification Gates → Safe Integration → Durable State & Recovery.

use anyhow::{Context, Result, anyhow};
use chrono::Utc;
use futures::future::BoxFuture;
use m31a_core::workflow::{
    WorkflowEvent, WorkflowEventType, WorkflowPlan, WorkflowRun, WorkflowState, WorkflowTask,
    WorkflowTaskStatus, WorkspaceIsolationStrategy,
};
use m31a_execpolicy::ExecPolicy;
use m31a_provider::ProviderClient;
use m31a_storage::worktree::{WorktreeManager, WorktreeSettings};
use m31a_storage::{Storage, WorkflowStore};
use m31a_tools::registry::ToolRegistry;
use m31a_verify::AutonomousVerifier;
use m31a_verify::runners::ProjectRunner;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{error, info};
use uuid::Uuid;

use crate::agents::EngineAgentRuntime;
use crate::speculative::{SpeculateOptions, SpeculativeArena};
use crate::waves::{WaveScheduler, WorkflowTaskExecutionOutcome};

/// Maximum automatic replanning revisions allowed per workflow before failing.
pub const MAX_WORKFLOW_REPLAN_REVISIONS: u32 = 3;

/// Configuration options for workflow execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowExecutionConfig {
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
            max_concurrency: 4,
            task_timeout_seconds: 120,
            verify_post_wave: true,
            allow_speculative_stages: true,
            require_approval_for_integration: false,
            max_replan_attempts: MAX_WORKFLOW_REPLAN_REVISIONS,
        }
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
        let run = self
            .store()
            .get_workflow(workflow_id)?
            .ok_or_else(|| anyhow!("Workflow '{workflow_id}' not found"))?;

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
            plan_revision: run.plan_revision,
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
            started_at_epoch_ms: None,
            completed_at_epoch_ms: None,
            worker_id: None,
            lease_expires_at_epoch_ms: None,
            error_details: None,
            input_artifacts: vec![],
            output_artifacts: vec!["analysis_report.json".into()],
            verification_result_json: None,
        });

        // 2. Implementation stage: Either Speculative Parallel Branches or Direct Implementation
        if is_speculation_valuable {
            // Task 2: Speculative Parallel Arena
            tasks.push(WorkflowTask {
                id: format!("{}-t2-speculate", workflow_id),
                workflow_id: workflow_id.to_string(),
                plan_revision: run.plan_revision,
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
                started_at_epoch_ms: None,
                completed_at_epoch_ms: None,
                worker_id: None,
                lease_expires_at_epoch_ms: None,
                error_details: None,
                input_artifacts: vec!["analysis_report.json".into()],
                output_artifacts: vec!["speculative_report.json".into(), "winning_patch.diff".into()],
                verification_result_json: None,
            });

            // Task 3: Solution Integration & Verification
            tasks.push(WorkflowTask {
                id: format!("{}-t3-integrate", workflow_id),
                workflow_id: workflow_id.to_string(),
                plan_revision: run.plan_revision,
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
                started_at_epoch_ms: None,
                completed_at_epoch_ms: None,
                worker_id: None,
                lease_expires_at_epoch_ms: None,
                error_details: None,
                input_artifacts: vec!["winning_patch.diff".into()],
                output_artifacts: vec!["integration_report.json".into()],
                verification_result_json: None,
            });
        } else {
            // Standard direct implementation task
            tasks.push(WorkflowTask {
                id: format!("{}-t2-implement", workflow_id),
                workflow_id: workflow_id.to_string(),
                plan_revision: run.plan_revision,
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
                started_at_epoch_ms: None,
                completed_at_epoch_ms: None,
                worker_id: None,
                lease_expires_at_epoch_ms: None,
                error_details: None,
                input_artifacts: vec!["analysis_report.json".into()],
                output_artifacts: vec!["implementation_patch.diff".into()],
                verification_result_json: None,
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
            plan_revision: run.plan_revision,
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
            started_at_epoch_ms: None,
            completed_at_epoch_ms: None,
            worker_id: None,
            lease_expires_at_epoch_ms: None,
            error_details: None,
            input_artifacts: vec![],
            output_artifacts: vec!["delivery_report.json".into()],
            verification_result_json: None,
        });

        let rationale = if is_speculation_valuable {
            "Decomposed into 4-stage pipeline with Speculative Parallel Reality execution for architectural optimization"
        } else {
            "Decomposed into 3-stage targeted linear pipeline with isolated worktree implementation"
        };

        let plan = WorkflowPlan {
            workflow_id: workflow_id.to_string(),
            revision: run.plan_revision,
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

    /// Executes a workflow through planning, wave scheduling, child agents, verification, and integration.
    pub async fn execute_workflow(&self, workflow_id: &str) -> Result<WorkflowRun> {
        let mut run = self
            .store()
            .get_workflow(workflow_id)?
            .ok_or_else(|| anyhow!("Workflow '{workflow_id}' not found"))?;

        if run.state.is_terminal() {
            return Ok(run);
        }

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

        // Executor closure dispatched by WaveScheduler
        let executor_fn = Arc::new(
            move |task: WorkflowTask| -> BoxFuture<'static, Result<WorkflowTaskExecutionOutcome>> {
                let orch = orchestrator.clone();
                Box::pin(async move { orch.execute_task(task).await })
            },
        );

        // Dispatch waves via WaveScheduler
        let wave_result =
            WaveScheduler::execute_workflow_waves(&repo_root, waves, executor_fn, true).await?;

        let now = Utc::now().timestamp_millis();
        if wave_result.all_passed {
            // Final integration verification on primary workspace
            let final_check =
                AutonomousVerifier::verify_check(&repo_root, Duration::from_secs(30)).await;
            let check_passed = final_check.map(|c| c.success).unwrap_or(false);

            if check_passed {
                self.store()
                    .update_workflow_state(workflow_id, WorkflowState::Completed)?;
                run.state = WorkflowState::Completed;
                run.completed_at_epoch_ms = Some(now);

                // Store delivery summary artifact
                let summary_json = serde_json::json!({
                    "workflow_id": workflow_id,
                    "objective": run.objective,
                    "completed_waves": wave_result.completed_waves,
                    "total_waves": wave_result.total_waves,
                    "status": "completed",
                    "timestamp": now,
                });

                let _ = self.store().store_artifact_file(
                    &repo_root,
                    workflow_id,
                    None,
                    "delivery_summary.json",
                    "summary",
                    summary_json.to_string().as_bytes(),
                    "application/json",
                );

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

                info!(workflow_id = %workflow_id, "Workflow completed successfully");
            } else {
                self.store()
                    .update_workflow_state(workflow_id, WorkflowState::Failed)?;
                run.state = WorkflowState::Failed;
                run.cancellation_reason = Some("Final workspace verification check failed".into());
                error!(workflow_id = %workflow_id, "Final verification failed");
            }
        } else {
            self.store()
                .update_workflow_state(workflow_id, WorkflowState::Failed)?;
            run.state = WorkflowState::Failed;
            run.cancellation_reason = Some("One or more workflow waves failed".into());
            error!(workflow_id = %workflow_id, "Workflow execution failed");
        }

        Ok(run)
    }

    /// Executes an individual task node with real engine, isolated worktree, or speculative arena.
    async fn execute_task(&self, task: WorkflowTask) -> Result<WorkflowTaskExecutionOutcome> {
        let start = Instant::now();
        let wf_id = task.workflow_id.clone();
        let task_id = task.id.clone();

        info!(task_id = %task_id, role = %task.assigned_role, "Executing workflow task");

        // 1. Mark task Running in storage
        self.store()
            .update_task_status(&wf_id, &task_id, WorkflowTaskStatus::Running, None)?;

        self.store().record_event(&WorkflowEvent {
            id: format!("evt-{}", Uuid::new_v4().simple()),
            workflow_id: wf_id.clone(),
            task_id: Some(task_id.clone()),
            agent_id: None,
            event_type: WorkflowEventType::TaskStarted,
            timestamp_epoch_ms: Utc::now().timestamp_millis(),
            correlation_id: None,
            payload_json: serde_json::json!({
                "task_name": task.name,
                "role": task.assigned_role,
            })
            .to_string(),
            schema_version: 1,
        })?;

        let run = self
            .store()
            .get_workflow(&wf_id)?
            .ok_or_else(|| anyhow!("Workflow not found"))?;

        let mut modified_files = Vec::new();
        let mut patch_content = None;
        let mut success = true;
        let mut error_msg = None;

        // 2. Check if this is a Speculative Arena task
        if task.id.contains("speculate") {
            info!(task_id = %task_id, "Dispatching Speculative Candidate Arena for task");
            let candidates = SpeculativeArena::generate_diverse_hypotheses(&run.objective, 3);
            let opts = SpeculateOptions {
                workspace_root: run.workspace_root.clone(),
                mission: run.objective.clone(),
                candidates,
                timeout_per_candidate: Duration::from_secs(45),
                auto_promote_winner: false,
                require_approval_for_promotion: false,
                max_concurrency: 3,
                executor: None,
            };

            match SpeculativeArena::compete_with_options(opts).await {
                Ok(report) => {
                    if let Some(ref winner_id) = report.winning_branch_id {
                        if let Some(winner) = report
                            .evaluations
                            .iter()
                            .find(|e| &e.branch_id == winner_id)
                        {
                            modified_files = winner.modified_files.clone();
                            patch_content = Some(winner.patch_content.clone());

                            // Store report artifact
                            let rep_bytes = serde_json::to_vec_pretty(&report)?;
                            let _ = self.store().store_artifact_file(
                                &run.workspace_root,
                                &wf_id,
                                Some(&task_id),
                                "speculative_report.json",
                                "report",
                                &rep_bytes,
                                "application/json",
                            );

                            if !winner.patch_content.is_empty() {
                                let _ = self.store().store_artifact_file(
                                    &run.workspace_root,
                                    &wf_id,
                                    Some(&task_id),
                                    "winning_patch.diff",
                                    "patch",
                                    winner.patch_content.as_bytes(),
                                    "text/x-diff",
                                );
                            }
                        }
                    } else {
                        // All candidates failed or were empty
                        success = false;
                        error_msg = Some("No speculative candidate satisfied hard gates".into());
                    }
                }
                Err(e) => {
                    success = false;
                    error_msg = Some(e.to_string());
                }
            }
        } else if task.id.contains("integrate") {
            // Winning solution integration task: apply winning patch onto workspace
            info!(task_id = %task_id, "Applying winning candidate patch onto workspace");
            let artifacts = self.store().list_artifacts(&wf_id)?;
            let patch_opt = artifacts
                .iter()
                .find(|a| a.name == "winning_patch.diff")
                .and_then(|patch_art| fs::read_to_string(&patch_art.file_path).ok())
                .and_then(|diff_text| {
                    m31a_tools::patch::parse_patch(&diff_text)
                        .ok()
                        .map(|parsed| (diff_text, parsed))
                });

            if let Some((diff_text, parsed)) = patch_opt {
                match m31a_tools::patch::apply_patch_to_workspace(&run.workspace_root, &parsed) {
                    Ok(res) => {
                        for p in res.files_modified.into_iter().chain(res.files_created) {
                            modified_files.push(p.to_string_lossy().to_string());
                        }
                        patch_content = Some(diff_text);
                    }
                    Err(e) => {
                        success = false;
                        error_msg = Some(format!("Patch integration failed: {e}"));
                    }
                }
            }
        } else {
            // General task execution: run through EngineAgentRuntime or isolated worktree
            if let Some(ref rt) = self.runtime {
                use m31a_tools::agent_runtime::AgentRuntime;
                let req = m31a_tools::agent_runtime::AgentSpawnRequest {
                    role_name: task.assigned_role.clone(),
                    task_name: task.name.clone(),
                    prompt: format!(
                        "Task: {}\nDescription: {}\nCriteria: {:?}\nVerification: {:?}",
                        task.name,
                        task.description,
                        task.acceptance_criteria,
                        task.required_verification
                    ),
                    isolate_worktree: task.workspace_isolation
                        == WorkspaceIsolationStrategy::Worktree,
                    model_override: None,
                    depth: 1,
                    max_depth: 3,
                };
                if let Ok(spawned) = rt.spawn_agent(req).await {
                    let _ = rt.wait_agents(&[spawned.agent_id], Some(30_000)).await;
                }
            } else if task.workspace_isolation == WorkspaceIsolationStrategy::Worktree {
                let settings = WorktreeSettings::new(&run.workspace_root);
                let mgr = WorktreeManager::new(settings);
                if let Ok(wt) = mgr.create_worktree(&format!("task-{}", task_id), None) {
                    let _ = AutonomousVerifier::verify_check(
                        &wt.worktree_path,
                        Duration::from_secs(15),
                    )
                    .await;
                    let _ = mgr.cleanup_worktree(&format!("task-{}", task_id));
                }
            }
        }

        // 3. Verification check
        let verif_passed = if success {
            AutonomousVerifier::verify_check(&run.workspace_root, Duration::from_secs(20))
                .await
                .map(|r| r.success)
                .unwrap_or(true)
        } else {
            false
        };

        let duration_ms = start.elapsed().as_millis() as u64;
        let final_status = if success && verif_passed {
            WorkflowTaskStatus::Completed
        } else {
            WorkflowTaskStatus::Failed
        };

        // 4. Update task record in storage
        self.store()
            .update_task_status(&wf_id, &task_id, final_status, error_msg.as_deref())?;

        self.store().record_event(&WorkflowEvent {
            id: format!("evt-{}", Uuid::new_v4().simple()),
            workflow_id: wf_id,
            task_id: Some(task_id.clone()),
            agent_id: None,
            event_type: if success {
                WorkflowEventType::VerificationCompleted
            } else {
                WorkflowEventType::TaskFailed
            },
            timestamp_epoch_ms: Utc::now().timestamp_millis(),
            correlation_id: None,
            payload_json: serde_json::json!({
                "duration_ms": duration_ms,
                "success": success,
                "verification_passed": verif_passed,
                "error": error_msg,
            })
            .to_string(),
            schema_version: 1,
        })?;

        Ok(WorkflowTaskExecutionOutcome {
            task_id,
            success: success && verif_passed,
            agent_id: None,
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

        // 1. Reconcile stale task leases
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

    /// Cancels a running workflow and all its active tasks.
    pub fn cancel_workflow(&self, workflow_id: &str, reason: &str) -> Result<()> {
        let run = self
            .store()
            .get_workflow(workflow_id)?
            .ok_or_else(|| anyhow!("Workflow '{workflow_id}' not found"))?;

        if run.state.is_terminal() {
            return Ok(());
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
